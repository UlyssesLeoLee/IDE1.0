//! ide-shell-desktop 集成测试 (跨 crate: Tauri managed state ↔ ide-shell App).
//!
//! 不启真 Tauri runtime (Tauri runtime 需要 event loop, 不适合 #[test] 同步场景),
//! 而是直接验证 `ShellState` 在并发/多事件下与 `ide-shell::App` 行为一致.
//!
//! Tauri 端的真 e2e 验证留给 cargo build + 用户手动跑 .exe + Playwright(若配置).

use ide_shell::Mode;
use ide_shell_desktop::ShellState;

#[test]
fn test_shell_state_concurrent_key_events_serialized() {
    use std::sync::Arc;
    use std::thread;

    let state = Arc::new(ShellState::default());
    let mut handles = vec![];
    // 4 个并发线程各打 5 次 Char:i 然后 Char:x → 每个线程贡献 'ixixixixix'
    for _ in 0..4 {
        let s = Arc::clone(&state);
        handles.push(thread::spawn(move || {
            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
            for _ in 0..5 {
                let mut app = s.0.lock().unwrap();
                app.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
                app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
                app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    // 验证 mutex 仍可用 (没 deadlock/poison)
    let app = state.0.lock().unwrap();
    assert_eq!(app.mode(), Mode::Normal, "all Esc → Normal");
}

#[test]
fn test_shell_state_key_response_shape() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let state = ShellState::default();
    // 模拟一个 key 操作并验证返回的 KeyResponse 字段
    {
        let mut app = state.0.lock().unwrap();
        app.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
    }
    let app = state.0.lock().unwrap();
    let resp = ide_shell_desktop::KeyResponse {
        frame: ide_shell::render::web::render(&app),
        keep_going: !app.should_quit(),
    };
    assert_eq!(resp.frame.buffer, "h");
    assert!(resp.keep_going);
}

#[test]
fn test_shell_state_quit_signal_propagates() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let state = ShellState::default();
    // 模拟 Normal mode 按 q 触发 quit
    {
        let mut app = state.0.lock().unwrap();
        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    }
    let app = state.0.lock().unwrap();
    assert!(app.should_quit(), "q should set quit_requested");

    // KeyResponse.keep_going 应为 false
    let resp = ide_shell_desktop::KeyResponse {
        frame: ide_shell::render::web::render(&app),
        keep_going: !app.should_quit(),
    };
    assert!(!resp.keep_going, "keep_going should be false after :q / q");
}
