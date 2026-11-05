//! 多租户 SaaS 云端协同与配额管控 HTTP 端点集成测试 (Pure Rust)

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

const TEST_TOKEN: &str = "test-saas-token";

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
        Self { addr, shutdown }
    }

    fn req(&self, method: &str, path: &str, body: Option<&str>) -> (String, serde_json::Value) {
        let mut s = TcpStream::connect(&self.addr).unwrap();
        let body_bytes = body.unwrap_or("").as_bytes();
        let req = format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: {}\r\n\
             X-IDE-Token: {TEST_TOKEN}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n",
            self.addr,
            body_bytes.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        if !body_bytes.is_empty() {
            s.write_all(body_bytes).unwrap();
        }
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        let status = resp
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        let body_str = resp.split("\r\n\r\n").nth(1).unwrap_or("");
        let val = serde_json::from_str(body_str).unwrap_or(serde_json::Value::Null);
        (status, val)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn test_pure_rust_tenant_context_endpoint() {
    let s = TestServer::start();
    let (status, val) = s.req("GET", "/api/tenant/context?tenant_id=org_default", None);
    assert_eq!(status, "200");
    assert_eq!(val["tenant_id"], "org_default");
    assert_eq!(val["tier"], "Enterprise");
    assert_eq!(val["quota"]["max_concurrent_agents"], 50);
}

#[test]
fn test_pure_rust_collab_presence_and_quota() {
    let s = TestServer::start();
    // 1. 注册 Coder Alpha
    let body = serde_json::json!({
        "tenant_id": "org_default",
        "workspace_id": "main",
        "file_id": "src/math.rs",
        "agent_id": "coder_alpha",
        "display_name": "Coder Alpha",
        "role": "AiCoder",
        "cursor": [5, 12]
    })
    .to_string();

    let (status, val) = s.req("POST", "/api/collab/presence", Some(&body));
    assert_eq!(status, "200");
    assert_eq!(val["agent_id"], "coder_alpha");
    assert_eq!(val["display_name"], "Coder Alpha");
    assert!(val["color"]["hex"].as_str().unwrap().starts_with('#'));

    // 2. 测试 Standard 租户 (配额 3 个) 超额拦截
    for i in 1..=3 {
        let b = serde_json::json!({
            "tenant_id": "org_demo",
            "workspace_id": "main",
            "file_id": "src/math.rs",
            "agent_id": format!("demo_agent_{i}"),
            "role": "AiCoder",
            "cursor": [i, 0]
        })
        .to_string();
        let (st, _) = s.req("POST", "/api/collab/presence", Some(&b));
        assert_eq!(st, "200");
    }

    // 第 4 个应触发 400 配额超出
    let b4 = serde_json::json!({
        "tenant_id": "org_demo",
        "workspace_id": "main",
        "file_id": "src/math.rs",
        "agent_id": "demo_agent_4",
        "role": "AiOptimizer",
        "cursor": [10, 0]
    })
    .to_string();
    let (st4, err) = s.req("POST", "/api/collab/presence", Some(&b4));
    assert_eq!(st4, "400");
    assert!(err["error"].as_str().unwrap().contains("quota"));
}
