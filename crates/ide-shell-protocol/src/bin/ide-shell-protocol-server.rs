//! ide-shell-protocol-server — 三协议 (HTTP + WS + stdio) 入口.
//!
//! ## 用法
//!
//! ```bash
//! # stdio JSON-RPC (AI agent 当 subprocess)
//! ide-shell-protocol-server stdio
//!
//! # HTTP + WS (远程集成)
//! ide-shell-protocol-server http --addr 127.0.0.1:8123
//! ```
//!
//! HTTP 端点 (axum):
//!   POST /rpc            — JSON-RPC 2.0 单条
//!   GET  /ws             — WebSocket (client 收事件推送)
//!   GET  /info           — KernelInfo
//!   GET  /healthz        — 健康检查
//!   WS   /rpc            — WebSocket JSON-RPC (双向)
//!
//! Stdio: 每行一条 JSON-RPC request, 输出每行一条 JSON-RPC response.

#![forbid(unsafe_code)]

use std::sync::Arc;


use axum::{
    extract::ws::WebSocketUpgrade,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use ide_shell_protocol::Kernel;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Parser)]
#[command(name = "ide-shell-protocol-server", version, about = "Headless IDE kernel server (HTTP/WS/stdio)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 单进程 stdio JSON-RPC — AI agent 当 subprocess spawn, stdin/stdout JSON 帧.
    /// 适合最低延迟的单机集成.
    Stdio,
    /// HTTP + WebSocket server — 远程集成, AI agent 跑在另一台机器/容器.
    Http {
        /// bind address, 默认 127.0.0.1:8123
        #[arg(long, default_value = "127.0.0.1:8123")]
        addr: String,
    },
    /// 同时跑 stdio + HTTP — 当 daemon 模式.
    Both {
        #[arg(long, default_value = "127.0.0.1:8123")]
        addr: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let kernel = Arc::new(Kernel::new());

    // 演示用: 注册一个 mock provider. 实际部署时由 AI agent 提供 provider 注入.
    kernel
        .register_provider("mock", Arc::new(MockProvider))
        .await;

    match cli.cmd {
        Cmd::Stdio => run_stdio(kernel).await?,
        Cmd::Http { addr } => run_http(kernel, &addr).await?,
        Cmd::Both { addr } => {
            // 并发跑两个 server
            let k2 = kernel.clone();
            let addr2 = addr.clone();
            tokio::spawn(async move {
                if let Err(e) = run_stdio(k2).await {
                    eprintln!("stdio error: {e}");
                }
            });
            run_http(kernel, &addr2).await?;
        }
    }
    Ok(())
}

/// stdio JSON-RPC server — 每行一条 request, 输出每行一条 response.
async fn run_stdio(kernel: Arc<Kernel>) -> anyhow::Result<()> {
    eprintln!("[ide-protocol] stdio server ready (newline-delimited JSON-RPC 2.0)");
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = reader.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match kernel.handle_rpc(line).await {
            Some(resp) => {
                stdout.write_all(resp.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
            None => {
                // notification — no reply
            }
        }
    }
    Ok(())
}

/// HTTP + WebSocket server.
async fn run_http(kernel: Arc<Kernel>, addr: &str) -> anyhow::Result<()> {
    eprintln!("[ide-protocol] HTTP server ready at http://{addr}");
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/info", get(info))
        .route("/rpc", post(http_rpc))
        .route("/ws", get(ws_handler))
        .route("/rpcws", get(rpc_ws_handler))
        .with_state(kernel);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok\n"
}

async fn info(State(kernel): State<Arc<Kernel>>) -> Json<serde_json::Value> {
    let info = ide_shell_protocol::KernelInfo::current();
    let ids = kernel.list_buffers().await;
    let providers = kernel.list_providers().await;
    Json(serde_json::json!({
        "kernel": info,
        "buffers": ids,
        "providers": providers,
    }))
}

/// 单条 JSON-RPC over HTTP.
async fn http_rpc(
    State(kernel): State<Arc<Kernel>>,
    body: String,
) -> impl IntoResponse {
    match kernel.handle_rpc(&body).await {
        Some(resp) => (StatusCode::OK, resp).into_response(),
        None => (StatusCode::ACCEPTED, "").into_response(),
    }
}

/// WebSocket — 接收客户端发来的 request, 同时 push kernel 事件给客户端.
/// client 主动 close socket 即退订.
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(kernel): State<Arc<Kernel>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, kernel))
}

async fn handle_ws(socket: axum::extract::ws::WebSocket, kernel: Arc<Kernel>) {
    let mut rx = kernel.subscribe().await;

    use futures::{SinkExt, StreamExt};
    let (mut sender, mut receiver) = socket.split();

    // 1. 推 events
    let send_task = tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let payload = serde_json::to_string(&ev).unwrap_or_default();
            if sender
                .send(axum::extract::ws::Message::Text(payload))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // 2. 收 RPC
    let recv_task = {
        let k = kernel.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                if let axum::extract::ws::Message::Text(text) = msg {
                    if let Some(_resp) = k.handle_rpc(&text).await {
                        // 简化: 不回 — 单向 RPC. 完整双向用 /rpcws
                    }
                }
            }
        })
    };

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// 双向 JSON-RPC over WebSocket — client 发 request, server 回 response + 推 events.
async fn rpc_ws_handler(
    ws: WebSocketUpgrade,
    State(kernel): State<Arc<Kernel>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_rpc_ws(socket, kernel))
}

async fn handle_rpc_ws(mut socket: axum::extract::ws::WebSocket, kernel: Arc<Kernel>) {

    let mut event_rx = kernel.subscribe().await;

    loop {
        tokio::select! {
            // 推 events
            ev = event_rx.recv() => {
                if let Some(ev) = ev {
                    let payload = serde_json::to_string(&ev).unwrap_or_default();
                    if socket.send(axum::extract::ws::Message::Text(payload)).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
            // 收 RPC request
            msg = socket.recv() => {
                match msg {
                    Some(Ok(axum::extract::ws::Message::Text(text))) => {
                        if let Some(resp) = kernel.handle_rpc(&text).await {
                            if socket.send(axum::extract::ws::Message::Text(resp)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(axum::extract::ws::Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

/// Mock provider — 演示. 实际生产用 HTTPProvider / AnthropicProvider / OpenAIProvider.
struct MockProvider;

#[async_trait::async_trait]
impl ide_shell_protocol::AiProvider for MockProvider {
    fn name(&self) -> &str { "mock" }
    async fn complete(&self, req: ide_shell_protocol::CompletionRequest) -> Result<Vec<ide_shell_protocol::CompletionItem>, String> {
        Ok(vec![
            ide_shell_protocol::CompletionItem {
                label: req.prefix.clone(),
                insert_text: format!("{}_completed", req.prefix),
                kind: "function".into(),
                detail: Some("mock provider".into()),
                documentation: None,
            }
        ])
    }
    async fn edit(&self, _req: ide_shell_protocol::EditRequest) -> Result<ide_shell_protocol::EditResponse, String> {
        Ok(ide_shell_protocol::EditResponse {
            new_text: "// mocked edit response\n".into(),
            explanation: Some("This is a mock edit from the demo provider.".into()),
        })
    }
    async fn chat(&self, _req: ide_shell_protocol::ChatRequest) -> Result<ide_shell_protocol::ChatResponse, String> {
        Ok(ide_shell_protocol::ChatResponse {
            message: ide_shell_protocol::ChatMessage {
                role: "assistant".into(),
                content: "Mock reply from the demo provider. Use a real provider for production.".into(),
                name: None,
                tool_call_id: None,
            },
            tool_calls: vec![],
            finish_reason: Some("stop".into()),
        })
    }
}
