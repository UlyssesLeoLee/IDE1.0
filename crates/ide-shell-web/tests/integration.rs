//! ide-shell-web integration tests — HTTP server 级 (跨 crate: ide-shell-web ↔ ide-shell).
//!
//! 启真 server (随机端口), 走 HTTP 请求验证 frame / event / reset 端点行为。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// 测试用固定 token。
///
/// web shell 自 P0 修复起要求 `X-IDE-Token` 头 —— 缺失即 401。
/// 测试固定 token 而非随机, 这样 `no_token_is_rejected` 这类用例可预测。
const TEST_TOKEN: &str = "test-token-ide10";

/// 找一个空闲端口。
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn http_post(addr: &str, path: &str, body: &str) -> (u16, String) {
    http_post_with(addr, path, body, Some(TEST_TOKEN))
}

/// 同 http_post, 但可指定 token (None = 完全不发该头, 用于验证 401)。
fn http_post_with(addr: &str, path: &str, body: &str, token: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let token_hdr = token.map(|t| format!("X-IDE-Token: {t}\r\n")).unwrap_or_default();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n{token_hdr}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status = resp
        .split_whitespace()
        .nth(1)
        .unwrap_or("000")
        .parse()
        .unwrap_or(0);
    let body = resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn http_get(addr: &str, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nX-IDE-Token: {TEST_TOKEN}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status = resp
        .split_whitespace()
        .nth(1)
        .unwrap_or("000")
        .parse()
        .unwrap_or(0);
    let body = resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

/// 起一个一次性 server 线程跑单个请求处理循环。
struct TestServer {
    addr: String,
    handle: Option<thread::JoinHandle<()>>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TestServer {
    fn start() -> Self {
        let port = free_port();
        let addr = format!("127.0.0.1:{port}");
        let listener = std::net::TcpListener::bind(&addr).unwrap();
        listener.set_nonblocking(false).unwrap();
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_c = shutdown.clone();
        let state = std::sync::Arc::new(ide_shell_web::ShellState::with_auth(
            ide_shell_web::auth::Auth::with_token(TEST_TOKEN, port),
        ));
        // 用另一个线程 accept, 主测试线程发请求。shutdown 后 accept 超时退出。
        listener.set_nonblocking(true).unwrap();
        let handle = thread::spawn(move || loop {
            if shutdown_c.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let state = state.clone();
                    thread::spawn(move || {
                        ide_shell_web::handle_request_pub(&mut stream, &state);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        });
        TestServer {
            addr,
            handle: Some(handle),
            shutdown,
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[test]
fn test_server_initial_frame_is_normal() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, body) = http_get(&srv.addr, "/api/frame");
    assert_eq!(status, 200);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["mode"], "Normal");
    assert_eq!(v["buffer"], "");
}

#[test]
fn test_server_key_event_enters_insert() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, body) = http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Char:i","modifiers":[]}"#,
    );
    assert_eq!(status, 200);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["frame"]["mode"], "Insert");
    assert_eq!(v["keep_going"], true);
}

#[test]
fn test_server_quit_command_sets_keep_going_false() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    // : → Command
    http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Char::","modifiers":[]}"#,
    );
    // q
    http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Char:q","modifiers":[]}"#,
    );
    // Enter → 执行 ":q"
    let (_, body) = http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Enter","modifiers":[]}"#,
    );
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["keep_going"], false,
        ":q should signal quit via keep_going=false"
    );
    // server 仍活着 — reset 可调用
    let (status, _) = http_post(&srv.addr, "/api/reset", "");
    assert_eq!(status, 200, "server should stay alive after :q");
}

#[test]
fn test_server_mouse_left_enters_insert() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (_, body) = http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"mouse","button":"Left"}"#,
    );
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["frame"]["mode"], "Insert");
}

#[test]
fn test_server_reset_clears_state() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    // 弄脏: i + 字符
    http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Char:i","modifiers":[]}"#,
    );
    http_post(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Char:x","modifiers":[]}"#,
    );
    // reset
    let (_, body) = http_post(&srv.addr, "/api/reset", "");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["mode"], "Normal");
    assert_eq!(v["buffer"], "");
}

#[test]
fn test_server_index_html_served() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, body) = http_get(&srv.addr, "/");
    assert_eq!(status, 200);
    assert!(
        body.contains("IDE1.0 IDE Shell"),
        "index.html should contain title"
    );
    assert!(
        body.contains("data-testid=\"shell\""),
        "index.html should contain shell container"
    );
}

#[test]
fn test_server_unknown_path_404() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, _) = http_get(&srv.addr, "/api/nonexistent");
    assert_eq!(status, 404);
}

// ============================================================
// P0 安全回归测试
// ============================================================
//
// 背景: web shell 绑定 127.0.0.1 曾等于「无访问控制」——
//   - 响应带 `Access-Control-Allow-Origin: *` → 任意网页可读响应
//   - `/api/terminal_create` 的 `shell` 直接来自请求 → 任意可执行文件
//   - 不校验 Host → DNS rebinding
// 修复后必须有测试证明这些确实被挡住, 否则回归无从发现。

#[test]
fn test_missing_token_is_rejected_401() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, _) = http_post_with(&srv.addr, "/api/event", r#"{"kind":"key","code":"Char:i"}"#, None);
    assert_eq!(status, 401, "无 token 必须 401");
}

#[test]
fn test_wrong_token_is_rejected_403() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, _) = http_post_with(
        &srv.addr,
        "/api/event",
        r#"{"kind":"key","code":"Char:i"}"#,
        Some("wrong-token"),
    );
    assert_eq!(status, 403, "错误 token 必须 403");
}

#[test]
fn test_valid_token_passes() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let (status, _) = http_post(&srv.addr, "/api/event", r#"{"kind":"key","code":"Char:i"}"#);
    assert_eq!(status, 200, "正确 token 应放行");
}

/// 跨站 Origin (模拟恶意网页) 必须被拒 —— 这是 CORS `*` 的替代防线。
#[test]
fn test_cross_origin_request_is_rejected() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let mut stream = TcpStream::connect(&srv.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let body = r#"{"kind":"key","code":"Char:i"}"#;
    let req = format!(
        "POST /api/event HTTP/1.1\r\nHost: {}\r\nOrigin: https://evil.example.com\r\nX-IDE-Token: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        srv.addr, TEST_TOKEN, body.len(), body
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status: u16 = resp.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
    assert_eq!(status, 403, "跨站 Origin 必须 403");
}

/// DNS rebinding: Host 指向外部域名, 即使 token 正确也必须拒。
#[test]
fn test_foreign_host_is_rejected() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let mut stream = TcpStream::connect(&srv.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let body = r#"{"kind":"key","code":"Char:i"}"#;
    let req = format!(
        "POST /api/event HTTP/1.1\r\nHost: evil.example.com\r\nX-IDE-Token: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        TEST_TOKEN, body.len(), body
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status: u16 = resp.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
    assert_eq!(status, 403, "外部 Host 必须 403 (防 DNS rebinding)");
}

/// 响应不得再带 `Access-Control-Allow-Origin: *`。
#[test]
fn test_response_has_no_wildcard_cors() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let mut stream = TcpStream::connect(&srv.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nX-IDE-Token: {}\r\nConnection: close\r\n\r\n",
        srv.addr, TEST_TOKEN
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    assert!(
        !resp.contains("Access-Control-Allow-Origin: *"),
        "响应不得带通配 CORS (会让任意网页读取本机响应)"
    );
    assert!(resp.contains("X-Content-Type-Options: nosniff"));
}

/// 超大 Content-Length 必须 413, 且不得先分配内存。
#[test]
fn test_oversized_content_length_is_rejected() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let mut stream = TcpStream::connect(&srv.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "POST /api/event HTTP/1.1\r\nHost: {}\r\nX-IDE-Token: {}\r\nContent-Length: 9000000000\r\nConnection: close\r\n\r\n",
        srv.addr, TEST_TOKEN
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    let status: u16 = resp.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
    assert_eq!(status, 413, "超大 body 必须 413");
}

/// CORS 预检不得放行 (自定义头触发预检, 拦住它就拦住了跨站真实请求)。
#[test]
fn test_preflight_options_is_not_granted() {
    let srv = TestServer::start();
    thread::sleep(Duration::from_millis(100));
    let mut stream = TcpStream::connect(&srv.addr).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "OPTIONS /api/event HTTP/1.1\r\nHost: {}\r\nOrigin: https://evil.example.com\r\nConnection: close\r\n\r\n",
        srv.addr
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    let status: u16 = resp.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
    assert_ne!(status, 200, "预检不得返回 200 (否则浏览器会放行跨站请求)");
    assert!(
        !resp.contains("Access-Control-Allow-Headers"),
        "不得回 Access-Control-Allow-Headers, 否则 X-IDE-Token 可被跨站读取"
    );
}
