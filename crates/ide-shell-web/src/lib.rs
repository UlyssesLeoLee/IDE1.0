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
//! - GET  /api/pick_folder   → mock: 返回 IDE_SHELL_WEB_TEST_ROOT 环境变量或 cwd
//! - POST /api/open_project  → body: {"path": "..."} → canonical 化, 设为项目根
//! - POST /api/list_dir      → body: {"path": "..."} → 沙箱内列目录
//! - POST /api/read_file     → body: {"path": "..."} → 沙箱内读 UTF-8 文件
//! - POST /api/write_file    → body: {"path": "...", "content": "..."} → 沙箱内写
//!
//! 设计: server 内持有单一 App 状态 (单用户 demo). 端口默认 8123 (CI 与本地都用).

pub mod path_safety;
pub mod wiki_data;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ide_shell::render::web::{render as render_web, WebFrame};
use ide_shell::{App, AppConfig};
use serde::{Deserialize, Serialize};

use path_safety::{list_dir, read_file, write_file, DirEntry, FileContent};

/// 单用户 demo 状态。
pub struct ShellState {
    app: Mutex<App>,
    /// 当前打开的项目根 (单 user, 安全沙箱 = 限此根内访问文件).
    project_root: Mutex<Option<PathBuf>>,
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            app: Mutex::new(App::new(AppConfig::default())),
            project_root: Mutex::new(None),
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
fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// 处理单个 HTTP 请求 (单连接, 短生命周期)。
pub fn handle_request_pub(stream: &mut TcpStream, state: &ShellState) {
    handle_request(stream, state)
}

/// 处理单个 HTTP 请求 (单连接, 短生命周期)。
fn handle_request(stream: &mut TcpStream, state: &ShellState) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    // 读 header 完
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }

    // 读 body
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    // 路由
    match (method, path) {
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
            let j =
                serde_json::to_vec(&serde_json::json!(state.kernel_banner())).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &j);
        }
        ("GET", "/api/help_wiki") => {
            let j = serde_json::to_vec(&wiki_data::APP_WIKI).unwrap_or_default();
            send_response(stream, "200 OK", "application/json", &j);
        }
        ("GET", "/api/pick_folder") => {
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
                Err(e) => send_response(stream, "400 Bad Request", "text/plain", e.as_bytes()),
            }
        }
        ("POST", "/api/open_project") => {
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match state.project_root_set(PathBuf::from(path)) {
                Ok(c) => {
                    let j =
                        serde_json::to_vec(&c.to_string_lossy().into_owned()).unwrap_or_default();
                    send_response(stream, "200 OK", "application/json", &j);
                }
                Err(e) => send_response(stream, "400 Bad Request", "text/plain", e.as_bytes()),
            }
        }
        ("POST", "/api/list_dir") => {
            let root = match state.project_root_get() {
                Some(r) => r,
                None => {
                    send_response(
                        stream,
                        "400 Bad Request",
                        "text/plain",
                        "尚未打开项目".as_bytes(),
                    );
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
                Err(e) => send_response(stream, "400 Bad Request", "text/plain", e.as_bytes()),
            }
        }
        ("POST", "/api/read_file") => {
            let root = match state.project_root_get() {
                Some(r) => r,
                None => {
                    send_response(
                        stream,
                        "400 Bad Request",
                        "text/plain",
                        "尚未打开项目".as_bytes(),
                    );
                    return;
                }
            };
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let resp: Result<FileContent, String> = read_file(&root, path);
            match resp {
                Ok(fc) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&fc).unwrap_or_default(),
                ),
                Err(e) => send_response(stream, "400 Bad Request", "text/plain", e.as_bytes()),
            }
        }
        ("POST", "/api/write_file") => {
            let root = match state.project_root_get() {
                Some(r) => r,
                None => {
                    send_response(
                        stream,
                        "400 Bad Request",
                        "text/plain",
                        "尚未打开项目".as_bytes(),
                    );
                    return;
                }
            };
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let resp: Result<usize, String> = write_file(&root, path, content);
            match resp {
                Ok(n) => send_response(
                    stream,
                    "200 OK",
                    "application/json",
                    &serde_json::to_vec(&n).unwrap_or_default(),
                ),
                Err(e) => send_response(stream, "400 Bad Request", "text/plain", e.as_bytes()),
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
pub fn serve(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!("ide-shell-web listening on http://{addr}");
    let state = ShellState::new();
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
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
