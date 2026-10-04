//! ide-shell-desktop — Tauri 2 desktop app 主体.
//!
//! 架构:
//! - Tauri `manage()` 注入 `Mutex<App>` 单例 (per ULYS-191 §4 brief)
//! - 6 个 `#[tauri::command]` 暴露 webview invoke 端点:
//!   * `frame() -> WebFrame`         拉当前 frame
//!   * `reset() -> WebFrame`         重置 app 状态
//!   * `key(code, modifiers) -> ...` 派发键盘事件
//!   * `mouse(button) -> ...`         派发鼠标点击
//!   * `kernel_banner() -> String`    复用过 ide-kernel-core
//!   * `get_mode() -> String`         取当前 mode (避开 Rust 关键字 `mode`)
//! - 复用 crates/ide-shell 的 App + crates/ide-shell::render::web 渲染逻辑
//!
//! 注: Tauri 2.12 已知限制 — `#[tauri::command]` 不能 `pub fn`, 且
//! `__cmd__xxx` macro 是 private 不能跨 module. 所以 commands 必须放 lib.rs root
//! 且不用 `pub`. invoke_handler 在同 module 引用.
//!
//! 测试:
//! - 5 UT 在 #[cfg(test)] mod (state 操作)
//! - 3 IT 在 tests/state_integration.rs (跨 crate, 真 Tauri managed state 模拟)

use std::sync::Mutex;

use ide_shell::render::web::{render as render_web, WebFrame};
use ide_shell::{App, AppConfig, Mode};

/// Tauri managed state wrapper — 单用户 demo, App 由 Tauri runtime 持有.
pub struct ShellState(pub Mutex<App>);

impl Default for ShellState {
    fn default() -> Self {
        Self(Mutex::new(App::new(AppConfig::default())))
    }
}

#[derive(Debug, serde::Serialize)]
pub struct KeyResponse {
    pub frame: WebFrame,
    pub keep_going: bool,
}

impl KeyResponse {
    fn from_app(state: &tauri::State<'_, ShellState>) -> Result<Self, String> {
        let app = state.0.lock().map_err(|e| e.to_string())?;
        Ok(Self {
            frame: render_web(&app),
            keep_going: !app.should_quit(),
        })
    }
}

// ===== Tauri commands =====
// 不写 `pub` (Tauri 2.12 macro private 限制), invoke_handler 在同 module 引用即可.

#[tauri::command]
fn frame(state: tauri::State<'_, ShellState>) -> Result<WebFrame, String> {
    let app = state.0.lock().map_err(|e| e.to_string())?;
    Ok(render_web(&app))
}

#[tauri::command]
fn reset(state: tauri::State<'_, ShellState>) -> Result<WebFrame, String> {
    let mut app = state.0.lock().map_err(|e| e.to_string())?;
    *app = App::new(AppConfig::default());
    Ok(render_web(&app))
}

#[tauri::command]
fn key(
    state: tauri::State<'_, ShellState>,
    code: String,
    modifiers: Vec<String>,
) -> Result<KeyResponse, String> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let kc = match code.as_str() {
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
            if let Some(rest) = other.strip_prefix("Char:") {
                if let Some(c) = rest.chars().next() {
                    KeyCode::Char(c)
                } else {
                    return KeyResponse::from_app(&state);
                }
            } else {
                return KeyResponse::from_app(&state);
            }
        }
    };

    let mut mods = KeyModifiers::NONE;
    for m in &modifiers {
        match m.as_str() {
            "CONTROL" | "ctrl" => mods |= KeyModifiers::CONTROL,
            "ALT" | "alt" => mods |= KeyModifiers::ALT,
            "SHIFT" | "shift" => mods |= KeyModifiers::SHIFT,
            _ => {}
        }
    }

    let event = KeyEvent::new(kc, mods);
    let mut app = state.0.lock().map_err(|e| e.to_string())?;
    app.on_key(event);
    let frame = render_web(&app);
    let keep_going = !app.should_quit();
    Ok(KeyResponse { frame, keep_going })
}

#[tauri::command]
fn mouse(state: tauri::State<'_, ShellState>, button: String) -> Result<WebFrame, String> {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    let mb = match button.as_str() {
        "Right" => MouseButton::Right,
        _ => MouseButton::Left,
    };
    let event = MouseEvent {
        kind: MouseEventKind::Down(mb),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let mut app = state.0.lock().map_err(|e| e.to_string())?;
    app.on_mouse(event);
    Ok(render_web(&app))
}

#[tauri::command]
fn kernel_banner(state: tauri::State<'_, ShellState>) -> Result<String, String> {
    let app = state.0.lock().map_err(|e| e.to_string())?;
    Ok(app.kernel_banner().to_string())
}

#[tauri::command]
fn get_mode(state: tauri::State<'_, ShellState>) -> Result<String, String> {
    let app = state.0.lock().map_err(|e| e.to_string())?;
    Ok(match app.mode() {
        Mode::Normal => "Normal".into(),
        Mode::Insert => "Insert".into(),
        Mode::Command => "Command".into(),
    })
}

/// Tauri app entry — 注册 state + commands + 启动.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ShellState::default())
        .invoke_handler(tauri::generate_handler![
            frame,
            reset,
            key,
            mouse,
            kernel_banner,
            get_mode,
        ])
        .setup(|_app| {
            eprintln!(
                "ide-shell-desktop starting (tauri {}, target {})",
                env!("CARGO_PKG_VERSION"),
                std::env::var("TAURI_TARGET_TRIPLE").unwrap_or_else(|_| "unknown".into()),
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ide-shell-desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_state() -> ShellState {
        ShellState::default()
    }

    #[test]
    fn test_initial_state_is_normal() {
        let s = new_state();
        let app = s.0.lock().unwrap();
        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.buffer().as_str(), "");
    }

    #[test]
    fn test_reset_restores_initial() {
        let mut s = new_state();
        {
            let mut app = s.0.lock().unwrap();
            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
            app.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
            for c in "junk".chars() {
                app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            }
        }
        s.0 = Mutex::new(App::new(AppConfig::default()));
        let app = s.0.lock().unwrap();
        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.buffer().as_str(), "");
    }

    #[test]
    fn test_kernel_banner_contains_kernel_core() {
        let s = new_state();
        let app = s.0.lock().unwrap();
        let banner = app.kernel_banner();
        assert!(banner.contains("ide-kernel-core"), "got: {banner}");
    }

    #[test]
    fn test_mode_serializes_as_expected() {
        let s = new_state();
        let app = s.0.lock().unwrap();
        let mode_str = match app.mode() {
            Mode::Normal => "Normal",
            Mode::Insert => "Insert",
            Mode::Command => "Command",
        };
        assert_eq!(mode_str, "Normal");
    }

    #[test]
    fn test_shell_state_send_sync() {
        fn assert_send<T: Send + Sync>() {}
        assert_send::<ShellState>();
    }
}
