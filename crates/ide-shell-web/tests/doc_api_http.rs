//! `/api/doc/*` HTTP 层端到端测试.
//!
//! 与 `doc_api.rs` 里的单元测试互补: 这里真的起 HTTP server, 验证
//! 路由分发 / JSON 编解码 / 状态在请求间保持 —— 单元测试不覆盖这些.
//!
//! 沿用 P0 修复后的认证: 每个请求带 `X-IDE-Token`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

const TEST_TOKEN: &str = "doc-api-token";

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct DocServer {
    addr: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl DocServer {
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

    fn post(&self, path: &str, body: &str) -> (u16, serde_json::Value) {
        let mut s = TcpStream::connect(&self.addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {}\r\nX-IDE-Token: {TEST_TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.addr,
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        let status: u16 = resp
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body = resp.split("\r\n\r\n").nth(1).unwrap_or("");
        let json = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    fn get(&self, path: &str) -> (u16, serde_json::Value) {
        let mut s = TcpStream::connect(&self.addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nX-IDE-Token: {TEST_TOKEN}\r\nConnection: close\r\n\r\n",
            self.addr
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        let status: u16 = resp
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body = resp.split("\r\n\r\n").nth(1).unwrap_or("");
        let json = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }
}

impl Drop for DocServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn doc_api_requires_token() {
    let srv = DocServer::start();
    let mut s = TcpStream::connect(&srv.addr).unwrap();
    let req = format!(
        "GET /api/doc/active HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        srv.addr
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    let status: u16 = resp
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    assert_eq!(status, 401, "无 token 必须 401 —— 内核文档不应对外裸奔");
}

#[test]
fn active_returns_404_when_no_document() {
    let srv = DocServer::start();
    let (status, j) = srv.get("/api/doc/active");
    assert_eq!(status, 404);
    assert_eq!(j["ok"], false);
}

#[test]
fn open_then_active_roundtrip() {
    let srv = DocServer::start();
    let (st, j) = srv.post(r#"/api/doc/open"#, r#"{"path":"a.txt","text":"hello"}"#);
    assert_eq!(st, 200);
    assert_eq!(j["ok"], true);
    assert_eq!(j["result"]["text"], "hello");
    assert_eq!(j["result"]["dirty"], false);

    let (_, a) = srv.get("/api/doc/active");
    assert_eq!(a["result"]["text"], "hello", "状态应在请求间保持");
    assert_eq!(a["result"]["path"], "a.txt");
}

#[test]
fn insert_edit_undo_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"ac"}"#);
    // 光标移到 col 1
    srv.post(r#"/api/doc/caret"#, r#"{"row":0,"col":1}"#);
    let (_, ins) = srv.post(r#"/api/doc/insert"#, r#"{"row":0,"col":1,"text":"b"}"#);
    assert_eq!(ins["result"]["text"], "abc");
    assert_eq!(ins["result"]["can_undo"], true);

    let (_, un) = srv.post(r#"/api/doc/undo"#, "{}");
    assert_eq!(un["result"]["text"], "ac");

    let (_, re) = srv.post(r#"/api/doc/redo"#, "{}");
    assert_eq!(re["result"]["text"], "abc");
}

#[test]
fn multibyte_edit_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"日本語"}"#);
    let (_, r) = srv.post(r#"/api/doc/insert"#, r#"{"row":0,"col":2,"text":"X"}"#);
    assert_eq!(r["result"]["text"], "日本X語", "col 应按 char 计");
}

#[test]
fn select_all_then_type_replaces() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"hello"}"#);
    srv.post(r#"/api/doc/select_all"#, "{}");
    let (_, r) = srv.post(r#"/api/doc/type"#, r#"{"row":0,"col":0,"char":"X"}"#);
    assert_eq!(r["result"]["text"], "X", "有选区时输入应替换");
}

#[test]
fn find_via_http_returns_char_positions() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"foo bar"}"#);
    let (_, r) = srv.post(r#"/api/doc/find"#, r#"{"pattern":"bar","row":0,"col":0}"#);
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["start_col"], 4);
    assert_eq!(r["result"]["end_col"], 7);
}

#[test]
fn find_no_match_returns_404() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"abc"}"#);
    let (st, _) = srv.post(r#"/api/doc/find"#, r#"{"pattern":"zzz","row":0,"col":0}"#);
    assert_eq!(st, 404, "无匹配应返回 404 而不是空 result");
}

#[test]
fn replace_all_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"cat cat cat"}"#);
    let (_, r) = srv.post(
        r#"/api/doc/replace_all"#,
        r#"{"pattern":"cat","replacement":"dog"}"#,
    );
    assert_eq!(r["result"]["replaced"], 3);
    let (_, a) = srv.get("/api/doc/active");
    assert_eq!(a["result"]["text"], "dog dog dog");
}

#[test]
fn multi_cursor_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"a\nb\nc"}"#);
    srv.post(r#"/api/doc/add_cursor"#, r#"{"row":1,"col":0}"#);
    srv.post(r#"/api/doc/add_cursor"#, r#"{"row":2,"col":0}"#);
    let (_, cur) = srv.get("/api/doc/active");
    assert_eq!(cur["result"]["cursor_count"], 3, "主光标 + 2 附加");

    let (_, r) = srv.post(r#"/api/doc/insert_all"#, r#"{"text":"X"}"#);
    assert_eq!(r["result"]["text"], "Xa\nXb\nXc");
}

#[test]
fn line_ending_roundtrip_via_http() {
    let srv = DocServer::start();
    // JSON 里 \r\n 是真实 CRLF
    srv.post(
        r#"/api/doc/open"#,
        "{\"path\":\"x.txt\",\"text\":\"a\\r\\nb\"}",
    );
    let (_, a) = srv.get("/api/doc/active");
    assert_eq!(a["result"]["line_ending"], "Windows (CRLF)");

    let (_, e) = srv.get("/api/doc/export");
    assert_eq!(e["result"]["text"], "a\r\nb", "导出应保持 CRLF");
    assert_eq!(e["result"]["path"], "x.txt");
}

#[test]
fn tabs_persist_across_requests() {
    let srv = DocServer::start();
    let (_, a) = srv.post(r#"/api/doc/open"#, r#"{"path":"a.txt","text":"AAA"}"#);
    let aid = a["result"]["id"].as_u64().unwrap();
    let (_, b) = srv.post(r#"/api/doc/open"#, r#"{"path":"b.txt","text":"BBB"}"#);
    let bid = b["result"]["id"].as_u64().unwrap();
    assert_ne!(aid, bid);

    let (_, tabs) = srv.get("/api/doc/tabs");
    assert_eq!(tabs["result"].as_array().unwrap().len(), 2);

    let (_, act) = srv.post(r#"/api/doc/activate"#, &format!(r#"{{"id":{aid}}}"#));
    assert_eq!(act["result"]["text"], "AAA");
}

#[test]
fn close_tab_via_http() {
    let srv = DocServer::start();
    let (_, a) = srv.post(r#"/api/doc/open"#, r#"{"text":"AAA"}"#);
    let aid = a["result"]["id"].as_u64().unwrap();
    srv.post(r#"/api/doc/open"#, r#"{"text":"BBB"}"#);
    srv.post(r#"/api/doc/close"#, &format!(r#"{{"id":{aid}}}"#));
    let (_, tabs) = srv.get("/api/doc/tabs");
    assert_eq!(tabs["result"].as_array().unwrap().len(), 1);
}

#[test]
fn mark_saved_clears_dirty_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"path":"a.txt","text":"A"}"#);
    srv.post(r#"/api/doc/insert"#, r#"{"row":0,"col":1,"text":"x"}"#);
    let (_, d) = srv.get("/api/doc/active");
    assert_eq!(d["result"]["dirty"], true);

    let (_, s) = srv.post(r#"/api/doc/mark_saved"#, "{}");
    assert_eq!(s["result"]["dirty"], false);
    assert_eq!(s["result"]["can_undo"], true, "保存不应清 undo");
}

#[test]
fn backspace_and_delete_forward() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"abc"}"#);
    srv.post(r#"/api/doc/caret"#, r#"{"row":0,"col":3}"#);
    let (_, r) = srv.post(r#"/api/doc/backspace"#, "{}");
    assert_eq!(r["result"]["text"], "ab");

    srv.post(r#"/api/doc/caret"#, r#"{"row":0,"col":0}"#);
    let (_, d) = srv.post(r#"/api/doc/delete"#, "{}");
    assert_eq!(d["result"]["text"], "b");
}

#[test]
fn encoding_switch_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"日本"}"#);
    let (_, r) = srv.post(r#"/api/doc/encoding"#, r#"{"encoding":"Shift_JIS"}"#);
    assert_eq!(r["result"]["encoding"], "Shift_JIS");
}

#[test]
fn line_ending_switch_via_http() {
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"a\nb"}"#);
    let (_, r) = srv.post(r#"/api/doc/line_ending"#, r#"{"line_ending":"crlf"}"#);
    assert_eq!(r["result"]["line_ending"], "Windows (CRLF)");
    let (_, e) = srv.get("/api/doc/export");
    assert_eq!(e["result"]["text"], "a\r\nb");
}

#[test]
fn doc_routes_do_not_break_existing_ones() {
    // 迁移期两条路径并存: /api/doc/* 有文档时, /api/frame 仍应正常
    let srv = DocServer::start();
    srv.post(r#"/api/doc/open"#, r#"{"text":"via kernel"}"#);
    let (st, j) = srv.get("/api/frame");
    assert_eq!(st, 200, "旧 frame 端点不应被内核路由影响");
    assert!(j["buffer"].is_string());
}
