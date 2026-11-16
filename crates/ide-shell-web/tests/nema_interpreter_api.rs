//! NEMA 外部与自定义解释器自适应导入 HTTP API 集成测试

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::thread;
use std::time::Duration;

const TEST_TOKEN: &str = "test-nema-interpreter-token";

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct TestServer {
    addr: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TestServer {
    fn start() -> Self {
        let port = free_port();
        let addr = format!("127.0.0.1:{port}");
        let listener = std::net::TcpListener::bind(&addr).unwrap();
        listener.set_nonblocking(true).unwrap();
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sh = shutdown.clone();
        let state = std::sync::Arc::new(ide_shell_web::ShellState::with_auth(
            ide_shell_web::auth::Auth::with_token(TEST_TOKEN, port),
        ));
        thread::spawn(move || loop {
            if sh.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let st = state.clone();
                    thread::spawn(move || ide_shell_web::handle_request_pub(&mut stream, &st));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        });
        thread::sleep(Duration::from_millis(60));
        Self { addr, shutdown }
    }

    fn post(&self, path: &str, body_json: &serde_json::Value) -> (u16, serde_json::Value) {
        let body = serde_json::to_vec(body_json).unwrap();
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {}\r\nX-IDE-Token: {TEST_TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.addr,
            body.len()
        );
        let mut stream = TcpStream::connect(&self.addr).unwrap();
        stream.write_all(req.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        stream.flush().unwrap();

        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        parse_response(&resp)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

fn parse_response(raw: &[u8]) -> (u16, serde_json::Value) {
    let s = String::from_utf8_lossy(raw);
    let mut parts = s.splitn(2, "\r\n\r\n");
    let header_part = parts.next().unwrap_or("");
    let body_part = parts.next().unwrap_or("");
    let status_line = header_part.lines().next().unwrap_or("");
    let code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    let json: serde_json::Value = serde_json::from_str(body_part.trim()).unwrap_or_default();
    (code, json)
}

#[test]
fn test_nema_interpreter_status_and_candidates() {
    let srv = TestServer::start();
    let (code, json) = srv.post("/api/nema/status", &serde_json::json!({}));
    assert_eq!(code, 200);
    assert!(json.get("active").is_some());
    assert!(json.get("candidates").is_some());
    let candidates = json["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty(), "Candidates should find official paths");
}

#[test]
fn test_nema_probe_and_render_roundtrip() {
    let srv = TestServer::start();
    let path = "E:\\.NEMA\\codec\\native.py";
    if !Path::new(path).exists() {
        return;
    }

    // 1. 探针接口测试
    let (code, probe_rep) = srv.post("/api/nema/probe", &serde_json::json!({
        "path": path,
        "runner": "python"
    }));
    assert_eq!(code, 200);
    assert_eq!(probe_rep["success"], true);
    assert_eq!(probe_rep["protocol"], "python_native_view");
    assert!(probe_rep["features"].as_array().unwrap().len() >= 3);

    // 2. 启用外部解释器
    let (set_code, set_rep) = srv.post("/api/nema/set_interpreter", &serde_json::json!({
        "enabled": true,
        "path": path,
        "runner": "python",
        "protocol": "python_native_view",
        "metadata": {
            "engine_name": "Official Python 1.5",
            "version": "1.5.0",
            "supports_graph": true,
            "supports_5w1h": true,
            "supports_table": true,
            "probe_latency_us": probe_rep["latency_us"],
            "probe_timestamp": 123456,
            "last_status": "ready",
            "last_error": null
        }
    }));
    assert_eq!(set_code, 200);
    assert_eq!(set_rep["ok"], true);

    // 3. 渲染接口测试 (调用外部解释器)
    let sample = " >Gflow 1\nA Start\nB Finish\nA>X>B Execute\n\n\n";
    let (render_code, render_rep) = srv.post("/api/nema/render", &serde_json::json!({
        "source": sample
    }));
    assert_eq!(render_code, 200);
    assert_eq!(render_rep["ok"], true);
    assert_eq!(render_rep["is_custom"], true);
    assert_eq!(render_rep["engine"], "Official Python 1.5");
    let ast = render_rep["ast"].as_array().unwrap();
    assert_eq!(ast[0]["id"], "Gflow");
    assert_eq!(ast[0]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(ast[0]["edges"].as_array().unwrap().len(), 1);

    // 4. 重置回内置解释器
    let (reset_code, _) = srv.post("/api/nema/set_interpreter", &serde_json::json!({
        "enabled": false,
        "path": "",
        "runner": "auto",
        "protocol": "builtin_rust",
        "metadata": {}
    }));
    assert_eq!(reset_code, 200);

    // 5. 内置解释器渲染验证
    let (builtin_code, builtin_rep) = srv.post("/api/nema/render", &serde_json::json!({
        "source": sample
    }));
    assert_eq!(builtin_code, 200);
    assert_eq!(builtin_rep["ok"], true);
    assert_eq!(builtin_rep["is_custom"], false);
    assert!(builtin_rep["engine"].as_str().unwrap().contains("内置"));
}
