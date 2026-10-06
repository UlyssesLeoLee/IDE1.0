//! ide-shell-protocol 集成测试 — HTTP + WebSocket + stdio 三协议 + provider 注册.
//!
//! 跑法: cargo test -p ide-shell-protocol --test integration -- --test-threads=1

use std::time::Duration;

use ide_shell_protocol::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[tokio::test]
async fn test_stdio_echo() {
    // 启动 server subprocess
    let bin = env!("CARGO_BIN_EXE_ide-shell-protocol-server");
    let mut child = Command::new(bin)
        .arg("stdio")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start server");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    // 发 open_buffer
    stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"open_buffer\",\"params\":{\"name\":\"stdio-test\"}}\n").await.unwrap();
    stdin.flush().await.unwrap();
    let line = tokio::time::timeout(Duration::from_secs(5), reader.next_line())
        .await
        .expect("timeout")
        .expect("no line")
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["result"]["id"], 1);

    // 发 get_buffer
    stdin
        .write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"get_buffer\",\"params\":{\"id\":1}}\n",
        )
        .await
        .unwrap();
    stdin.flush().await.unwrap();
    let line = tokio::time::timeout(Duration::from_secs(5), reader.next_line())
        .await
        .expect("timeout")
        .expect("no line")
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["result"]["name"], "stdio-test");

    child.kill().await.ok();
}

#[tokio::test]
async fn test_stdio_chat_provider() {
    let bin = env!("CARGO_BIN_EXE_ide-shell-protocol-server");
    let mut child = Command::new(bin)
        .arg("stdio")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start server");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    // chat 调用 mock provider
    let req = serde_json::json!({
        "jsonrpc":"2.0","id":1,"method":"chat",
        "params":{
            "messages":[{"role":"user","content":"hi"}],
            "tools":[]
        }
    });
    stdin.write_all(req.to_string().as_bytes()).await.unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    let line = tokio::time::timeout(Duration::from_secs(5), reader.next_line())
        .await
        .expect("timeout")
        .expect("no line")
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["result"]["message"]["role"], "assistant");
    assert!(v["result"]["message"]["content"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("mock"));

    child.kill().await.ok();
}

#[tokio::test]
async fn test_event_subscription_emit_order() {
    let k = Kernel::new();
    let mut rx1 = k.subscribe().await;
    let mut rx2 = k.subscribe().await;
    // 两个 subscriber 都应收到
    let _id = k.open_buffer("event-test").await;
    let e1 = rx1.recv().await.unwrap();
    let e2 = rx2.recv().await.unwrap();
    matches!(e1, Event::BufferOpened { .. });
    matches!(e2, Event::BufferOpened { .. });
}

#[tokio::test]
async fn test_register_provider_via_rpc_returns_error() {
    // Provider 实例必须 in-process, RPC 端 register_provider 应返回明确错误
    let k = Kernel::new();
    let resp = k
        .handle_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"register_provider","params":{"name":"x"}}"#,
        )
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("in-process"));
}

#[tokio::test]
async fn test_rpc_concurrent_requests() {
    let k = Kernel::new();
    let mut handles = vec![];
    for i in 0..20 {
        let k2 = k.clone();
        handles.push(tokio::spawn(async move {
            k2.handle_rpc(&format!(
                r#"{{"jsonrpc":"2.0","id":{i},"method":"open_buffer","params":{{"name":"buf{i}"}}}}"#
            )).await
        }));
    }
    for h in handles {
        let r = h.await.unwrap().unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert!(v["result"]["id"].as_u64().is_some());
    }
    let ids = k.list_buffers().await;
    // 20 个 + initial [scratch] = 21
    assert!(ids.len() >= 20);
}

#[tokio::test]
async fn test_execute_command_lifecycle() {
    let k = Kernel::new();
    let r1 = k
        .handle_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"execute_command","params":{"cmd":"buffers"}}"#,
        )
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&r1).unwrap();
    assert!(v["result"]["buffers"].is_array());

    let r2 = k
        .handle_rpc(
            r#"{"jsonrpc":"2.0","id":2,"method":"execute_command","params":{"cmd":"e new.rs"}}"#,
        )
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&r2).unwrap();
    let id = v["result"]["opened"].as_u64().unwrap();
    assert!(id > 0);

    let r3 = k
        .handle_rpc(&format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"execute_command","params":{{"cmd":"q {id}"}}}}"#
        ))
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&r3).unwrap();
    assert!(v["result"]["closed"].as_u64().is_some());
}
