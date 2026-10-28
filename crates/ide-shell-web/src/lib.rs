//! ide-shell-web — HTTP server 暴露 ide-shell 状态给浏览器, 复用 web render backend.
//!
//! 极简实现: std::net::TcpListener + 手写 HTTP/1.1 解析 (避免拉 axum 等大依赖).
//!
//! 端点 (ULYS-191 §5 桌面版镜像, 一份前端 HTML 复用 desktop + tauri):
//! - GET  /                  → 经典 shell HTML (40 case 老 UI)
//! - GET  /editor            → Cursor 风格编辑器 HTML (新功能测试目标)
//! - GET  /api/frame         → 当前 WebFrame JSON
//! - POST /api/event         → 提交 KeyEvent 或 MouseEvent JSON, 返回新 WebFrame JSON
//! - POST /api/reset         → 重置 app 到 Normal 空 buffer
//! - POST /api/key           → (editor.html 用) 单 key 派发, 形同 /api/event {kind:key}
//! - POST /api/mouse         → (editor.html 用) 单 mouse 派发
//! - GET  /api/kernel_banner → IDE1.0 kernel banner
//! - GET  /api/help_wiki     → 完整内置 wiki (桌面 APP_WIKI 同源)
//! - POST /api/pick_folder   → mock: 返回 IDE_SHELL_WEB_TEST_ROOT 环境变量或 cwd
//! - POST /api/open_project  → body: {"path": "..."} → canonical 化, 设为项目根
//! - POST /api/list_dir      → body: {"path": "..."} → 沙箱内列目录
//! - POST /api/read_file     → body: {"path": "..."} → 沙箱内读 UTF-8 文件
//! - POST /api/write_file    → body: {"path": "...", "content": "..."} → 沙箱内写
//!
//! 设计: server 内持有单一 App 状态 (单用户 demo). 端口默认 8123 (CI 与本地都用).

pub mod auth;
pub mod doc_api;
pub mod path_safety;
pub mod wiki_data;

mod outline;
mod scm;
mod search;
mod terminal;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;

use ide_shell::render::web::{render as render_web, WebFrame};
use ide_shell::{App, AppConfig};
use serde::{Deserialize, Serialize};

use ide_kernel::document::LineEnding;
use path_safety::{list_dir, read_file_enc, write_file_spec, DirEntry, FileContent};

/// 单用户 demo 状态。
/// 请求头大小上限 (防 header 放大攻击).
pub const MAX_HEADER_BYTES: usize = 16 * 1024;
/// 请求体大小上限. 文件写入本身限 4 MB, 留一倍余量给 JSON 包裹.
pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

pub struct ShellState {
    app: Mutex<App>,
    /// 请求认证配置 (token / 允许的 Host).
    auth: auth::Auth,
    /// 内核文档会话 — /api/doc/* 的状态 (ide-kernel::Session).
    ///
    /// 与 `read_file`/`write_file` 并存: 后者是无状态文件读写 (前端自己管
    /// buffer), 前者是有状态的编辑会话 (服务端管光标/undo/选区).
    /// 迁移期间两者共存, 见 doc_api.rs 顶部的迁移路径说明.
    docs: std::sync::Mutex<doc_api::DocStore>,
    /// 当前打开的项目根 (单 user, 安全沙箱 = 限此根内访问文件).
    project_root: Mutex<Option<PathBuf>>,
}

impl ShellState {
    pub fn new() -> Self {
        Self::with_port(8123)
    }

    /// 指定服务端口 (用于 Host 白名单).
    pub fn with_port(port: u16) -> Self {
        Self {
            app: Mutex::new(App::new(AppConfig::default())),
            project_root: Mutex::new(None),
            auth: auth::Auth::from_env(port),
            docs: Mutex::new(doc_api::DocStore::new()),
        }
    }

    /// 测试用: 指定 token.
    pub fn with_auth(a: auth::Auth) -> Self {
        Self {
            app: Mutex::new(App::new(AppConfig::default())),
            project_root: Mutex::new(None),
            auth: a,
            docs: Mutex::new(doc_api::DocStore::new()),
        }
    }

    pub fn frame(&self) -> WebFrame {
        let app = self.app.lock().unwrap();
        render_web(&app)
    }

    /// 派发一个 KeyEvent JSON. Returns true if app still alive, false if Quit triggered.
    pub fn key(&self, code: &str, modifiers: &[String]) -> bool {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let kc = match code {
            "Char" => {
                // 简化:Playwright 发 "Char:x" 形式, 提取 x
                if let Some(rest) = code.strip_prefix("Char:") {
                    if let Some(c) = rest.chars().next() {
                        KeyCode::Char(c)
                    } else {
                        return true;
                    }
                } else {
                    return true;
                }
            }
            "Enter" => KeyCode::Enter,
            "Esc" => KeyCode::Esc,
            "Backspace" => KeyCode::Backspace,
            "Delete" => KeyCode::Delete,
            "Tab" => KeyCode::Tab,
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            other => {
                // "Char:x" 形式
                if let Some(rest) = other.strip_prefix("Char:") {
                    if let Some(c) = rest.chars().next() {
                        KeyCode::Char(c)
                    } else {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        };
        let mut mods = KeyModifiers::NONE;
        for m in modifiers {
            match m.as_str() {
                "CONTROL" | "ctrl" => mods |= KeyModifiers::CONTROL,
                "ALT" | "alt" => mods |= KeyModifiers::ALT,
                "SHIFT" | "shift" => mods |= KeyModifiers::SHIFT,
                _ => {}
            }
        }
        let event = KeyEvent::new(kc, mods);
        let mut app = self.app.lock().unwrap();
        app.on_key(event);
        // keep_going 以 should_quit 为准 (:q command 执行路径 on_key 返回 true, 但也应退出)
        !app.should_quit()
    }

    /// 派发一个 mouse click JSON { button: "Left"|"Right" }. 简化:任何 mouse 都先 LeftClick=Insert / RightClick=Normal。
    pub fn mouse(&self, button: &str) -> bool {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mb = match button {
            "Right" => MouseButton::Right,
            _ => MouseButton::Left,
        };
        let event = MouseEvent {
            kind: MouseEventKind::Down(mb),
            column: 0,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        let mut app = self.app.lock().unwrap();
        app.on_mouse(event)
    }

    pub fn reset(&self) {
        let mut app = self.app.lock().unwrap();
        *app = App::new(AppConfig::default());
    }

    pub fn project_root_get(&self) -> Option<PathBuf> {
        self.project_root.lock().unwrap().clone()
    }

    pub fn project_root_set(&self, root: PathBuf) -> Result<PathBuf, String> {
        let canon = path_safety::safe_canonicalize(&root)
            .map_err(|e| format!("无法打开目录 {}: {e}", root.display()))?;
        // Strip Windows verbatim prefix if present
        let canon = path_safety::strip_verbatim(&canon);
        if !canon.is_dir() {
            return Err(format!("不是目录: {}", canon.display()));
        }
        *self.project_root.lock().unwrap() = Some(canon.clone());
        Ok(canon)
    }

    pub fn kernel_banner(&self) -> String {
        format!(
            "ide-kernel-core {}/{}",
            ide_kernel_core::version(),
            ide_kernel_core::kernel_status()
        )
    }
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyRequest {
    pub code: String,
    #[serde(default)]
    pub modifiers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MouseRequest {
    pub button: String,
}

/// 简易 HTTP 响应。
/// 回 CORS 头.
///
/// # 安全 (P0)
///
/// 原实现对**所有**响应写 `Access-Control-Allow-Origin: *`, 等于让用户浏览器里
/// 打开的任意恶意网页都能读本机 shell 的响应 —— 配合 `/api/terminal_*` 即 RCE。
///
/// 现在改为: 有合法 Origin 才回显该 Origin (同源); 无 Origin 或不可信 Origin
/// 一律**不回** CORS 头, 浏览器因此读不到响应。
fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    send_response_cors(stream, status, content_type, body, None)
}

/// 业务错误响应 —— 一律用 `application/json` 包 `{"error": "..."}`.
///
/// 这里的错误分支以前发的是 `text/plain` + 中文原因, 而成功分支是 JSON。
/// 契约不一致导致调用方只要无条件 `.json()` 就会在错误路径上炸
/// (`SyntaxError: Unexpected token '不'`) —— UAT 里 30 个用例栽在这上面。
fn send_error_json(stream: &mut TcpStream, status: &str, reason: &str) {
    let j = serde_json::json!({ "error": reason });
    send_response(
        stream,
        status,
        "application/json",
        &serde_json::to_vec(&j).unwrap_or_default(),
    );
}

/// 无状态纯计算端点的统一收尾: 反序列化请求 -> `editcore::pure_api` 里的类型化入口 -> 200 JSON;
/// 请求体坏了 / 命令未知 = 400 + 给用户看的文字。
fn respond_pure<Req: serde::de::DeserializeOwned>(
    stream: &mut TcpStream,
    body: &[u8],
    invalid: &str,
    f: impl FnOnce(&Req) -> Result<serde_json::Value, String>,
) {
    match serde_json::from_slice::<Req>(body) {
        Ok(req) => match f(&req) {
            Ok(v) => send_response(stream, "200 OK", "application/json", &serde_json::to_vec(&v).unwrap_or_default()),
            Err(msg) => send_error_json(stream, "400 Bad Request", &msg),
        },
        Err(e) => send_error_json(stream, "400 Bad Request", &format!("{invalid}: {e}")),
    }
}

/// 带 Origin 回显的响应 (只有可信同源 Origin 才会被传入 Some)。
fn send_response_cors(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    origin: Option<&str>,
) {
    let mut header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(o) = origin {
        header.push_str(&format!("Access-Control-Allow-Origin: {o}\r\n"));
    }
    header.push_str("\r\n");
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// 从 JSON 取 (row, col), 缺省 0.
fn doc_pos(req: &serde_json::Value) -> (i32, i32) {
    (
        req.get("row").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        req.get("col").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
    )
}

/// 统一响应 /api/doc/* 的结果.
fn doc_respond<T: serde::Serialize>(stream: &mut TcpStream, value: Option<T>) {
    let (status, j) = match value {
        Some(v) => ("200 OK", serde_json::json!({ "ok": true, "result": v })),
        None => (
            "404 Not Found",
            serde_json::json!({ "ok": false, "error": "no active document" }),
        ),
    };
    send_response(
        stream,
        status,
        "application/json",
        &serde_json::to_vec(&j).unwrap_or_default(),
    );
}

/// 处理单个 HTTP 请求 (单连接, 短生命周期)。
pub fn handle_request_pub(stream: &mut TcpStream, state: &ShellState) {
    handle_request(stream, state)
}

/// 处理单个 HTTP 请求 (单连接, 短生命周期)。
fn handle_request(stream: &mut TcpStream, state: &ShellState) {
    let auth = &state.auth;
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path_full = parts.next().unwrap_or("");
    // 分离 path 与 query string: /api/terminal_output?id=term-0 → /api/terminal_output
    let path = path_full.split('?').next().unwrap_or(path_full);

    // 读 header 完 (收集全部 header 供认证用)
    let mut content_length = 0usize;
    let mut headers: auth::Headers = Vec::new();
    let mut header_bytes = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
            break;
        }
        header_bytes += line.len();
        if header_bytes > MAX_HEADER_BYTES {
            send_response(
                stream,
                "431 Request Header Fields Too Large",
                "text/plain",
                b"",
            );
            return;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
        if let Some((k, v)) = headers.last() {
            if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            }
        }
    }

    // --- 认证 / Host / Origin 校验 (P0) ---
    // 必须在校验通过后才分配 body 内存, 否则恶意 Content-Length 可先吃掉 GB 级 RAM.
    if let Err(e) = auth.check(method, &headers) {
        let body = serde_json::json!({ "error": e.reason }).to_string();
        send_response(stream, e.status, "application/json", body.as_bytes());
        return;
    }
    // 同源请求: 回显 Origin 让浏览器能读响应 (跨站的已在上面被拒)
    let _echo_origin = headers
        .iter()
        .find(|(k, _)| k == "origin")
        .map(|(_, v)| v.as_str())
        .filter(|o| auth.origin_is_allowed(o));

    // 读 body (上限保护, 防止单请求分配过大内存)
    if content_length > MAX_BODY_BYTES {
        send_response(
            stream,
            "413 Payload Too Large",
            "application/json",
            br#"{"error":"body-too-large"}"#,
        );
        return;
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    // 路由
    match (method, path) {
        ("POST", "/api/plugin_load") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let input = req.get("input").and_then(|v| v.as_str()).unwrap_or("");
            match editcore::plugin::load_and_run(std::path::Path::new(path), input) {
                Ok(out) => {
                    let j = serde_json::json!({ "ok": true, "output": out });
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&j).unwrap_or_default(),
                    );
                }
                Err(e) => {
                    let j = serde_json::json!({ "ok": false, "error": e.to_string() });
                    send_response(
                        stream,
                        "400 Bad Request",
                        "application/json",
                        &serde_json::to_vec(&j).unwrap_or_default(),
                    );
                }
            }
        }
        // ---------- 内核文档会话 (/api/doc/*) ----------
        //
        // 编辑语义在 Rust 侧 (ide-kernel), 前端只发"语义操作".
        // 与 /api/read_file + /api/write_file 并存 —— 那是无状态的
        // 文件读写, 供前端自管 buffer 的旧路径继续使用.
        ("POST", "/api/doc/open") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let text = req
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // 有 path 且无 text 时从磁盘读 (仍走沙箱)
            let content = if text.is_empty() && !path.is_empty() {
                let root = state.project_root_get();
                match root.as_ref() {
                    Some(r) => path_safety::read_file(r, &path)
                        .map(|fc| fc.content)
                        .unwrap_or_else(|e| format!("<read error: {e}>")),
                    None => "<project root not set>".to_string(),
                }
            } else {
                text
            };
            let opt_path = if path.is_empty() {
                None
            } else {
                Some(path.as_str())
            };
            let view = state
                .docs
                .lock()
                .ok()
                .map(|mut d| d.open(opt_path, &content));
            doc_respond(stream, view);
        }
        ("GET", "/api/doc/active") => {
            let view = state.docs.lock().ok().and_then(|d| d.active());
            doc_respond(stream, view);
        }
        ("GET", "/api/doc/tabs") => {
            let tabs = state.docs.lock().map(|d| d.tabs()).unwrap_or_default();
            doc_respond(stream, Some(tabs));
        }
        ("POST", "/api/doc/activate") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let id = req.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let view = state.docs.lock().ok().and_then(|mut d| d.activate(id));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/close") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let id = req.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let view = state.docs.lock().ok().and_then(|mut d| d.close(id));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/new_tab") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.new_tab());
            doc_respond(stream, view);
        }
        // ---- 编辑原语 ----
        ("POST", "/api/doc/insert") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let (r, c) = doc_pos(&req);
            let text = req.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let view = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.insert(r, c, text));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/type") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let (r, c) = doc_pos(&req);
            let ch = req
                .get("char")
                .and_then(|v| v.as_str())
                .and_then(|s| s.chars().next())
                .unwrap_or(' ');
            let view = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.type_char(r, c, ch));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/newline") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.insert_newline());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/backspace") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.backspace());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/delete") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.delete_forward());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/delete_range") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let (r0, c0) = doc_pos(&req);
            let r1 = req.get("end_row").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let c1 = req.get("end_col").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let view = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.delete_range(r0, c0, r1, c1));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/caret") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let (r, c) = doc_pos(&req);
            let view = state.docs.lock().ok().and_then(|mut d| d.set_caret(r, c));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/select_all") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.select_all());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/clear_selection") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.clear_selection());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/undo") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.undo());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/redo") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.redo());
            doc_respond(stream, view);
        }
        // ---- 多光标 ----
        ("POST", "/api/doc/add_cursor") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let (r, c) = doc_pos(&req);
            let view = state.docs.lock().ok().and_then(|mut d| d.add_cursor(r, c));
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/clear_cursors") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.clear_cursors());
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/insert_all") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let text = req.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let view = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.insert_all_cursors(text));
            doc_respond(stream, view);
        }
        // ---- 查找替换 ----
        ("POST", "/api/doc/find") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let pat = req
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cs = req
                .get("case_sensitive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let (r, c) = doc_pos(&req);
            let hit = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.find(&pat, cs, r, c));
            doc_respond(stream, hit);
        }
        ("POST", "/api/doc/find_all") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let pat = req
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cs = req
                .get("case_sensitive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let limit = req.get("limit").and_then(|v| v.as_u64()).unwrap_or(1000) as usize;
            let hits = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.find_all(&pat, cs, limit));
            doc_respond(stream, hits);
        }
        ("POST", "/api/doc/replace_all") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let pat = req
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let rep = req
                .get("replacement")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cs = req
                .get("case_sensitive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let limit = req.get("limit").and_then(|v| v.as_u64()).unwrap_or(1000) as usize;
            let st = state
                .docs
                .lock()
                .ok()
                .and_then(|mut d| d.replace_all(&pat, &rep, cs, limit));
            doc_respond(stream, st);
        }
        // ---- 格式 / 保存 ----
        ("POST", "/api/doc/line_ending") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let label = req
                .get("line_ending")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let eol = match label.to_ascii_lowercase().as_str() {
                "crlf" | "windows (crlf)" => Some(LineEnding::Crlf),
                "cr" | "mac (cr)" => Some(LineEnding::Cr),
                "lf" | "unix (lf)" => Some(LineEnding::Lf),
                _ => None,
            };
            let view = eol.and_then(|e| {
                state
                    .docs
                    .lock()
                    .ok()
                    .and_then(|mut d| d.set_line_ending(e))
            });
            doc_respond(stream, view);
        }
        ("POST", "/api/doc/encoding") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let label = req.get("encoding").and_then(|v| v.as_str()).unwrap_or("");
            let view = ide_kernel::document::Encoding::from_label(label)
                .and_then(|e| state.docs.lock().ok().and_then(|mut d| d.set_encoding(e)));
            doc_respond(stream, view);
        }
        ("GET", "/api/doc/export") => {
            let r = state.docs.lock().ok().and_then(|d| d.export());
            match r {
                Some((path, text)) => {
                    let j = serde_json::json!({
                        "ok": true,
                        "result": { "path": path, "text": text }
                    });
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&j).unwrap_or_default(),
                    );
                }
                None => doc_respond::<serde_json::Value>(stream, None),
            }
        }
        ("POST", "/api/doc/mark_saved") => {
            let view = state.docs.lock().ok().and_then(|mut d| d.mark_saved());
            doc_respond(stream, view);
        }

        ("POST", "/api/migemo_patterns") => {
            // MIGEMO 検索: ローマ字 → ひらがな / カタカナ / 半角カナ 等の候補パターン
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let roma = req.get("roma").and_then(|v| v.as_str()).unwrap_or("");
            let patterns = editcore::migemo::expand_pattern(roma);
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&serde_json::json!({ "patterns": patterns })).unwrap_or_default(),
            );
        }
        ("POST", "/api/edit_cmd") => {
            // 编辑命令 (editcore::pure_api::edit_cmd); 无状态纯计算
            respond_pure::<editcore::edit_cmd::EditReq>(stream, &body, "编辑请求无效", editcore::pure_api::edit_cmd);
        }
        ("POST", "/api/bracket_pair") => {
            // 对括弧 (editcore::pure_api::bracket_pair); 无状态纯计算
            respond_pure::<editcore::bracket::BracketReq>(stream, &body, "括弧请求无效", |r| Ok(editcore::pure_api::bracket_pair(r)));
        }
        ("POST", "/api/bookmark") => {
            // ブックマーク 切り替え / 次へ / 前へ; 无状态纯计算
            respond_pure::<editcore::bookmark::BmReq>(stream, &body, "书签请求无效", editcore::pure_api::bookmark);
        }
        ("POST", "/api/search_text") => {
            // 文档内查找 / 置換 (含正文镜像); 无状态纯计算
            respond_pure::<editcore::search_ops::SearchReq>(stream, &body, "查找请求无效", editcore::pure_api::search_text);
        }
        ("POST", "/api/plugins") => {
            // 插件: 列表 / 读文件 / 启停 / 卸载 / 检查 / 安装更新 / 冲突仲裁 (网络类只在用户点按钮时才会被调用)
            respond_pure::<editcore::pure_api::PluginReq>(stream, &body, "插件请求无效", editcore::pure_api::plugins);
        }
        ("POST", "/api/agent") => {
            // 智能功能 API: 工具表 / 调用工具 (写类工具只产出补丁提案); 没人调用就零开销
            respond_pure::<editcore::pure_api::AgentReq>(stream, &body, "Agent 请求无效", editcore::pure_api::agent);
        }
        ("POST", "/api/md_render") => {
            // Markdown 预览: 文本 -> 已转义 HTML (只读)
            respond_pure::<editcore::pure_api::MdRenderReq>(stream, &body, "Markdown 预览请求无效", |r| Ok(editcore::pure_api::md_render(r)));
        }
        ("POST", "/api/history") => {
            // 撤销 / 重做栈; 按标签页 id 分开的进程内登记
            respond_pure::<editcore::history::HistReq>(stream, &body, "历史请求无效", editcore::pure_api::history);
        }
        ("POST", "/api/nav_step") => {
            // 键盘光标移动 / 范围选择; 无状态纯计算
            respond_pure::<editcore::nav::NavReq>(stream, &body, "移动请求无效", |r| Ok(editcore::pure_api::nav_step(r)));
        }
        ("POST", "/api/edit_text") => {
            // 选中文字的变换 (editcore::edit_ops::transform_text); 纯计算
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let op = req.get("op").and_then(|v| v.as_str()).unwrap_or("");
            let text = req.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let tab = req.get("tab_width").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
            match editcore::edit_ops::transform_text(op, text, tab) {
                Some(out) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&serde_json::json!({ "text": out })).unwrap_or_default(),
                ),
                None => send_error_json(stream, "400 Bad Request", "未知的变换操作"),
            }
        }
        ("POST", "/api/edit_lines") => {
            // 选区涉及整行的变换 (editcore::edit_ops::transform_lines); 纯计算
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let op = req.get("op").and_then(|v| v.as_str()).unwrap_or("");
            let lines: Vec<String> = req
                .get("lines")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            match editcore::edit_ops::transform_lines(op, &lines) {
                Some(out) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&serde_json::json!({ "lines": out })).unwrap_or_default(),
                ),
                None => send_error_json(stream, "400 Bad Request", "未知的行操作"),
            }
        }
        ("POST", "/api/reinterpret_text") => {
            // コード変換: 文本按 encode_as 编成字节再按 decode_as 解读 (纯计算, 不碰磁盘)
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let text = req.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let enc = req.get("encode_as").and_then(|v| v.as_str()).unwrap_or("");
            let dec = req.get("decode_as").and_then(|v| v.as_str()).unwrap_or("");
            let parsed = (
                editcore::document_io::Charset::from_label(enc),
                editcore::document_io::Charset::from_label(dec),
            );
            match parsed {
                (Some(e), Some(d)) => match editcore::document_io::reinterpret_text(text, e, d) {
                    Ok(out) => send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&serde_json::json!({ "text": out })).unwrap_or_default(),
                    ),
                    Err(e) => send_error_json(stream, "400 Bad Request", &e),
                },
                _ => send_error_json(stream, "400 Bad Request", "不支持的编码"),
            }
        }
        ("POST", "/api/outline") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let file_path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            // Outline 直接读 (read-only, IDE 内部) — sandbox 的 \?\ prefix 已知 bug
            let root = state.project_root_get();
            let root_path = match root {
                Some(r) => r,
                None => {
                    let resp =
                        serde_json::json!({ "path": file_path, "error": "project root not set" });
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&resp).unwrap_or_default(),
                    );
                    return;
                }
            };
            // 走沙箱读文件 (P0 安全): 原实现直接 `fs::read_to_string(&full)`,
            // `full` 只做了 root.join 而**没有校验是否在 root 之内** ——
            // 传 `../../../../Windows/win.ini` 即可任意文件读取.
            // 注释里"绕过沙箱"的理由 (sandbox 的 \\?\ prefix bug) 早已由
            // stripVerbatim 修复, 不再成立.
            match path_safety::read_file(&root_path, file_path) {
                Ok(fc) => {
                    let result = outline::outline_file(file_path, &fc.content);
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&result).unwrap_or_default(),
                    );
                }
                Err(e) => {
                    let resp = serde_json::json!({ "path": file_path, "error": e.to_string() });
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&resp).unwrap_or_default(),
                    );
                }
            }
        }

        ("POST", "/api/terminal_create") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let shell = req
                .get("shell")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let cwd = req
                .get("cwd")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            match terminal::create(shell, cwd) {
                Some(id) => {
                    let info = serde_json::json!({
                        "id": id,
                        "title": format!("{} #{}", terminal::shell_display(&id), terminal::counter(&id)),
                        "shell": terminal::shell_cmd(&id),
                        "cwd": terminal::cwd(&id),
                        "alive": true,
                    });
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&info).unwrap_or_default(),
                    );
                }
                None => send_response(
                    stream,
                    "500 Internal Server Error",
                    "text/plain",
                    b"spawn failed",
                ),
            }
        }
        ("POST", "/api/terminal_input") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let id = req.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let data = req.get("data").and_then(|v| v.as_str()).unwrap_or("");
            match terminal::input(id, data) {
                Ok(()) => send_response(stream, "200 OK", "text/plain", b"ok"),
                Err(e) => send_response(stream, "404 Not Found", "text/plain", e.as_bytes()),
            }
        }
        ("POST", "/api/terminal_close") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let id = req.get("id").and_then(|v| v.as_str()).unwrap_or("");
            terminal::close(id);
            send_response(stream, "200 OK", "text/plain", b"ok");
        }
        ("GET", "/api/terminal_output") => {
            // Query: ?id=term-N (path_full 包含 query, path 已剥除)
            let id = path_full
                .split("?id=")
                .nth(1)
                .unwrap_or("")
                .split("&")
                .next()
                .unwrap_or("");
            // 先取 alive 再取输出: 读线程是「先推完所有数据, 再把 alive 置 false」, 所以看到 false 时这次 drain 一定拿全了
            let alive = terminal::is_alive(id);
            let buf = terminal::drain_output(id);
            let resp = serde_json::json!({ "id": id, "chunks": buf, "alive": alive });
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&resp).unwrap_or_default(),
            );
        }
        ("GET", "/api/terminal_list") => {
            let list = terminal::list();
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&list).unwrap_or_default(),
            );
        }
        ("POST", "/api/scm_status") => match scm::status() {
            Ok((files, branch)) => {
                let resp = serde_json::json!({ "branch": branch, "files": files });
                send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&resp).unwrap_or_default(),
                );
            }
            Err(e) => {
                let resp = serde_json::json!({ "branch": "", "files": [], "error": e });
                send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&resp).unwrap_or_default(),
                );
            }
        },
        ("POST", "/api/scm_stage") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let paths: Vec<String> = req
                .get("paths")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            match scm::stage(&paths) {
                Ok(()) => send_response(stream, "200 OK", "text/plain", b"ok"),
                Err(e) => send_response(
                    stream,
                    "500 Internal Server Error",
                    "text/plain",
                    e.as_bytes(),
                ),
            }
        }
        ("POST", "/api/scm_unstage") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let paths: Vec<String> = req
                .get("paths")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            match scm::unstage(&paths) {
                Ok(()) => send_response(stream, "200 OK", "text/plain", b"ok"),
                Err(e) => send_response(
                    stream,
                    "500 Internal Server Error",
                    "text/plain",
                    e.as_bytes(),
                ),
            }
        }
        ("POST", "/api/scm_diff") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let staged = req.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
            match scm::diff(path, staged) {
                Ok(diff) => {
                    let resp = serde_json::json!({ "diff": diff });
                    send_response(
                        stream,
                        "200 OK",
                        "application/json",
                        &serde_json::to_vec(&resp).unwrap_or_default(),
                    );
                }
                Err(e) => send_response(
                    stream,
                    "500 Internal Server Error",
                    "text/plain",
                    e.as_bytes(),
                ),
            }
        }
        ("POST", "/api/scm_commit") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let message = req.get("message").and_then(|v| v.as_str()).unwrap_or("");
            match scm::commit(message) {
                Ok(_) => send_response(stream, "200 OK", "text/plain", b"ok"),
                Err(e) => send_response(
                    stream,
                    "500 Internal Server Error",
                    "text/plain",
                    e.as_bytes(),
                ),
            }
        }
        ("POST", "/api/scm_log") => match scm::log(10) {
            Ok(commits) => {
                let resp = serde_json::json!({ "commits": commits });
                send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&resp).unwrap_or_default(),
                );
            }
            Err(e) => {
                let resp = serde_json::json!({ "commits": [], "error": e });
                send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&resp).unwrap_or_default(),
                );
            }
        },
        ("POST", "/api/search") => {
            // 项目内检索 / Grep: 请求直接反序列化成 SearchOptions (含 charset 等可选字段, 不再手工逐字段取)
            match serde_json::from_slice::<search::SearchOptions>(&body) {
                Ok(opts) => {
                    let cwd = std::env::var("IDE_SHELL_WEB_TEST_ROOT")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
                    match search::search(&cwd, &opts) {
                        Ok(results) => send_response(stream, "200 OK", "application/json", &serde_json::to_vec(&serde_json::json!({ "results": results })).unwrap_or_default()),
                        // 搜索条件非法 (例如「フォルダー」栏全部越出项目根) 必须报错, 不能回 200 + 空结果:
                        // 那样前端只看到「无匹配」, 用户不知道自己写的目录被拒了。
                        Err(e) => send_response(stream, "400 Bad Request", "application/json", &serde_json::to_vec(&serde_json::json!({ "results": [], "error": e })).unwrap_or_default()),
                    }
                }
                Err(e) => send_error_json(stream, "400 Bad Request", &format!("搜索请求无效: {e}")),
            }
        }

        ("GET", "/") | ("GET", "/index.html") => {
            let html = include_str!("index.html");
            send_response(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                html.as_bytes(),
            );
        }
        ("GET", "/api/frame") => {
            let frame = state.frame();
            let json = serde_json::to_vec(&frame).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &json);
        }
        ("POST", "/api/event") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            // 区分 key / mouse: {"kind":"key","code":"i","modifiers":[]} 或 {"kind":"mouse","button":"Left"}
            let keep_going = match req.get("kind").and_then(|v| v.as_str()) {
                Some("mouse") => {
                    let button = req.get("button").and_then(|v| v.as_str()).unwrap_or("Left");
                    state.mouse(button)
                }
                _ => {
                    let code = req.get("code").and_then(|v| v.as_str()).unwrap_or("");
                    let modifiers: Vec<String> = req
                        .get("modifiers")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    state.key(code, &modifiers)
                }
            };
            let frame = state.frame();
            let resp = serde_json::json!({
                "frame": frame,
                "keep_going": keep_going,
            });
            let json = serde_json::to_vec(&resp).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &json);
        }
        ("POST", "/api/reset") => {
            state.reset();
            // 同时清空项目根 — 单一 server 跨 case 隔离 (老 40 case 不依赖 project_root)
            *state.project_root.lock().unwrap() = None;
            let frame = state.frame();
            let json = serde_json::to_vec(&frame).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &json);
        }
        ("GET", "/editor") => {
            let html = include_str!("editor.html");
            send_response(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                html.as_bytes(),
            );
        }
        ("GET", "/PixelFontMulti-12.ttf") | ("GET", "/editor/PixelFontMulti-12.ttf") => {
            let bytes = std::fs::read("crates/ide-shell-web/src/PixelFontMulti-12.ttf")
                .or_else(|_| std::fs::read("crates/ide-shell-desktop/dist/PixelFontMulti-12.ttf"))
                .or_else(|_| std::fs::read("E:/PixelFont/dist/PixelFontMulti-12.ttf"))
                .or_else(|_| std::fs::read("PixelFontMulti-12.ttf"));
            if let Ok(b) = bytes {
                send_response(stream, "200 OK", "font/ttf", &b);
            } else {
                send_error_json(stream, "404 Not Found", "font not found");
            }
        }
        ("GET", "/logo.png") | ("GET", "/editor/logo.png") => {
            let bytes = std::fs::read("crates/ide-shell-web/src/logo.png")
                .or_else(|_| std::fs::read("crates/ide-shell-desktop/dist/logo.png"))
                .or_else(|_| std::fs::read("assets/logo/IDE1.0_Crystal_A_128x128.png"));
            if let Ok(b) = bytes {
                send_response(stream, "200 OK", "image/png", &b);
            } else {
                send_error_json(stream, "404 Not Found", "logo not found");
            }
        }
        ("GET", "/favicon.ico") | ("GET", "/editor/favicon.ico") => {
            let bytes = std::fs::read("crates/ide-shell-web/src/favicon.ico")
                .or_else(|_| std::fs::read("crates/ide-shell-desktop/dist/favicon.ico"))
                .or_else(|_| std::fs::read("crates/ide-shell-desktop/icons/icon.ico"));
            if let Ok(b) = bytes {
                send_response(stream, "200 OK", "image/x-icon", &b);
            } else {
                send_error_json(stream, "404 Not Found", "favicon not found");
            }
        }
        ("GET", "/favicon.png") | ("GET", "/editor/favicon.png") => {
            let bytes = std::fs::read("crates/ide-shell-web/src/favicon.png")
                .or_else(|_| std::fs::read("crates/ide-shell-desktop/dist/favicon.png"))
                .or_else(|_| std::fs::read("assets/logo/IDE1.0_Crystal_A_32x32.png"));
            if let Ok(b) = bytes {
                send_response(stream, "200 OK", "image/png", &b);
            } else {
                send_error_json(stream, "404 Not Found", "favicon not found");
            }
        }
        // ===== editor.html 用端点 — 与 desktop Tauri command 同名 =====
        ("POST", "/api/key") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let code = req.get("code").and_then(|v| v.as_str()).unwrap_or("");
            let modifiers: Vec<String> = req
                .get("modifiers")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let keep_going = state.key(code, &modifiers);
            let resp = serde_json::json!({"frame": state.frame(), "keep_going": keep_going});
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&resp).unwrap_or_default(),
            );
        }
        ("POST", "/api/mouse") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let button = req.get("button").and_then(|v| v.as_str()).unwrap_or("Left");
            let keep_going = state.mouse(button);
            let resp = serde_json::json!({"frame": state.frame(), "keep_going": keep_going});
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&resp).unwrap_or_default(),
            );
        }
        ("GET", "/api/kernel_banner") => {
            // Also handle POST for web invoke()
            let j =
                serde_json::to_vec(&serde_json::json!(state.kernel_banner())).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &j);
        }
        ("POST", "/api/kernel_banner") => {
            let j =
                serde_json::to_vec(&serde_json::json!(state.kernel_banner())).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &j);
        }
        ("GET", "/api/help_wiki") => {
            let j = serde_json::to_vec(&wiki_data::APP_WIKI).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &j);
        }
        // POST (不是 GET): 前端 invoke() 一律发 POST (见 editor.html invoke()),
        // 写成 GET 时 openFolder() 永远拿不到路径 → 静默回退到原生 prompt(),
        // 无头浏览器里直接挂死 (web-editor.spec 3 个 + desktop-ui.spec 2 个 红 的 根因).
        // 透明主题的桌面明暗探测。**这两个端点原来只存在于桌面端**
        // (theme_luminance.rs 里的 #[tauri::command]), web server 没有对应路由,
        // 于是前端的 1s 反色轮询每轮都 404 → 落进 catch → 静默保持旧颜色,
        // 表现为「开了透明主题但不会动态反色」。实现已搬到 sakura-rs, 两端共用。
        ("POST", "/api/desktop_luminance") | ("GET", "/api/desktop_luminance") => {
            // 返回裸字符串, 与桌面端 #[tauri::command] desktop_luminance() -> String 保持一致.
            // 之前包成 {luminance:...} 会让前端 `lum !== VIEW_STATE.desktopLuminance` 每轮都成立,
            // 变成每秒重刷一整套 setProperty.
            let j = serde_json::json!(editcore::theme_luminance::detect().as_str());
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&j).unwrap_or_default(),
            );
        }
        ("POST", "/api/clear_theme_vars") | ("GET", "/api/clear_theme_vars") => {
            let j = editcore::theme_luminance::build_clear_theme_vars();
            send_response(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&j).unwrap_or_default(),
            );
        }
        // GET 也放行: UAT 的 serverRoot() 用 GET 问「当前项目根是什么」,
        // 而这条路由以前只注册了 POST —— GET 落到兜底分支拿到 404 + text/plain,
        // 调用方的 res.json() 于是抛 SyntaxError, 一串用例连锁失败.
        ("GET", "/api/pick_folder") | ("POST", "/api/pick_folder") => {
            // web 版无原生 dialog: 返回 IDE_SHELL_WEB_TEST_ROOT 环境变量或 cwd
            let default_root = std::env::var("IDE_SHELL_WEB_TEST_ROOT")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            match state.project_root_set(default_root) {
                Ok(c) => {
                    let j =
                        serde_json::to_vec(&c.to_string_lossy().into_owned()).unwrap_or_default();
                    send_response(stream, "200 OK", "application/json", &j);
                }
                Err(e) => send_error_json(stream, "400 Bad Request", &e),
            }
        }
        ("POST", "/api/open_project") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            eprintln!("[DEBUG] /api/open_project received path: {:?}", path);
            match state.project_root_set(PathBuf::from(path)) {
                Ok(c) => {
                    let j =
                        serde_json::to_vec(&c.to_string_lossy().into_owned()).unwrap_or_default();
                    send_response(stream, "200 OK", "application/json", &j);
                }
                Err(e) => {
                    eprintln!("[DEBUG] /api/open_project error: {}", e);
                    send_error_json(stream, "400 Bad Request", &e);
                }
            }
        }
        ("POST", "/api/list_dir") => {
            let root = match state.project_root_get() {
                Some(r) => r,
                None => {
                    send_error_json(stream, "400 Bad Request", "尚未打开项目");
                    return;
                }
            };
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let resp: Result<Vec<DirEntry>, String> = list_dir(&root, path);
            match resp {
                Ok(es) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&es).unwrap_or_default(),
                ),
                Err(e) => send_error_json(stream, "400 Bad Request", &e),
            }
        }
        ("POST", "/api/read_file") => {
            let root = match state.project_root_get() {
                Some(r) => r,
                None => {
                    send_error_json(stream, "400 Bad Request", "尚未打开项目");
                    return;
                }
            };
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let enc = req.get("encoding").and_then(|v| v.as_str());
            let resp: Result<FileContent, String> = read_file_enc(&root, path, enc);
            match resp {
                Ok(fc) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&fc).unwrap_or_default(),
                ),
                Err(e) => send_error_json(stream, "400 Bad Request", &e),
            }
        }
        ("POST", "/api/write_file") => {
            let root = match state.project_root_get() {
                Some(r) => r,
                None => {
                    send_error_json(stream, "400 Bad Request", "尚未打开项目");
                    return;
                }
            };
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let spec = editcore::document_io::WriteSpec {
                encoding: req.get("encoding").and_then(|v| v.as_str()).map(String::from),
                bom: req.get("bom").and_then(|v| v.as_str()).map(String::from),
                eol: req.get("eol").and_then(|v| v.as_str()).map(String::from),
                force: req.get("force").and_then(|v| v.as_bool()).unwrap_or(false),
            };
            let resp: Result<usize, String> = write_file_spec(&root, path, content, &spec);
            match resp {
                Ok(n) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&n).unwrap_or_default(),
                ),
                Err(e) => send_error_json(stream, "400 Bad Request", &e),
            }
        }
        ("OPTIONS", _) => {
            send_response(stream, "204 No Content", "text/plain", b"");
        }
        _ => {
            let body = b"Not Found";
            send_response(stream, "404 Not Found", "text/plain", body);
        }
    }
}

/// 启动 HTTP server 监听指定端口 (阻塞).
/// 单连接读超时 (秒). 单线程 server 下, 一个只发请求行不发完 header 的
/// 半开连接会永久阻塞整个服务 —— 这是手写同步 HTTP server 的经典 DoS.
const READ_TIMEOUT_SECS: u64 = 15;

pub fn serve(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let port = addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8123);
    let state = ShellState::with_port(port);
    eprintln!("ide-shell-web listening on http://{addr}");
    if state.auth.open {
        eprintln!("[auth] 开放模式 (IDE_SHELL_WEB_NO_AUTH=1) — 仅限本地 UAT, 不要暴露");
    } else {
        eprintln!("[auth] token = {}", state.auth.token());
        eprintln!("[auth] 请求需带 header: X-IDE-Token: <token>");
    }
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                // 读超时: 防半开连接永久占用唯一的服务线程
                let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(READ_TIMEOUT_SECS)));
                let _ =
                    s.set_write_timeout(Some(std::time::Duration::from_secs(READ_TIMEOUT_SECS)));
                handle_request(&mut s, &state);
            }
            Err(e) => eprintln!("connection error: {e}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_state_frame_normal() {
        let s = ShellState::new();
        let f = s.frame();
        assert_eq!(f.mode, ide_shell::Mode::Normal);
        assert_eq!(f.buffer, "");
    }

    #[test]
    fn test_shell_state_key_enter_insert() {
        let s = ShellState::new();
        s.key("Char:i", &[]);
        let f = s.frame();
        assert_eq!(f.mode, ide_shell::Mode::Insert);
    }

    #[test]
    fn test_shell_state_key_typed_chars() {
        let s = ShellState::new();
        s.key("Char:i", &[]);
        for c in "abc".chars() {
            s.key(&format!("Char:{c}"), &[]);
        }
        let f = s.frame();
        assert_eq!(f.buffer, "abc");
        assert_eq!(f.cursor, 3);
    }

    #[test]
    fn test_shell_state_key_enter_command() {
        let s = ShellState::new();
        s.key("Char::", &[]);
        let f = s.frame();
        assert_eq!(f.mode, ide_shell::Mode::Command);
    }

    #[test]
    fn test_shell_state_mouse_left_insert() {
        let s = ShellState::new();
        s.mouse("Left");
        let f = s.frame();
        assert_eq!(f.mode, ide_shell::Mode::Insert);
    }

    #[test]
    fn test_shell_state_mouse_right_normal() {
        let s = ShellState::new();
        s.mouse("Left");
        s.mouse("Right");
        let f = s.frame();
        assert_eq!(f.mode, ide_shell::Mode::Normal);
    }

    #[test]
    fn test_shell_state_reset() {
        let s = ShellState::new();
        s.key("Char:i", &[]);
        for c in "test".chars() {
            s.key(&format!("Char:{c}"), &[]);
        }
        s.reset();
        let f = s.frame();
        assert_eq!(f.mode, ide_shell::Mode::Normal);
        assert_eq!(f.buffer, "");
    }

    #[test]
    fn test_key_request_parse() {
        let json = r#"{"code":"Char:i","modifiers":["CONTROL"]}"#;
        let req: KeyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.code, "Char:i");
        assert_eq!(req.modifiers, vec!["CONTROL"]);
    }
}
