//! ide-shell-web integration tests — HTTP server 级 (跨 crate: ide-shell-web ↔ ide-shell).
//!
//! 启真 server (随机端口), 走 HTTP 请求验证 frame / event / reset 端点行为。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// 找一个空闲端口。
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn http_post(addr: &str, path: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
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
        let state = std::sync::Arc::new(ide_shell_web::ShellState::new());
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
