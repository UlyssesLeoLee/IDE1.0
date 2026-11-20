//! IDE1.0 MCP 独立服务器命令行入口 (Stdio JSON-RPC 2.0 传输层)。

#![forbid(unsafe_code)]

use ide_mcp::McpServer;
use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("ide10-mcp v0.8.6");
        return;
    }

    if args.iter().any(|a| a == "--tools") {
        let server = McpServer::default();
        let tools = server.list_tools();
        println!("{}", serde_json::to_string_pretty(&tools).unwrap_or_default());
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--eval") {
        if let Some(code) = args.get(pos + 1) {
            let mut server = McpServer::default();
            let res = ide_mcp::rust_console::RustConsoleEvaluator::eval(&mut server.workspace, code);
            println!("{}", res.summary);
            for s in res.steps {
                println!("[{}] {} => {}", if s.ok { "OK" } else { "FAIL" }, s.expr, s.output);
            }
            if !res.ok {
                std::process::exit(1);
            }
            return;
        }
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut server = McpServer::default();

    for line in stdin.lock().lines() {
        let Ok(raw) = line else { break };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            let err = serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": { "code": -32700, "message": "Parse error" }
            });
            let _ = writeln!(out, "{}", serde_json::to_string(&err).unwrap());
            let _ = out.flush();
            continue;
        };

        if let Some(resp) = server.handle_jsonrpc(&req) {
            let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
            let _ = out.flush();
        }
    }
}
