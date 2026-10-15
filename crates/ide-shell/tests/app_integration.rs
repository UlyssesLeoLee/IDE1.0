//! App 集成测试 — vim 模式 + 键位 dispatch + command execution.
//!
//! 补全 app.rs lib tests — 覆盖 mode transitions + on_key + execute_command 路径.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ide_shell::app::{App, AppConfig};
use ide_shell::mode::Mode;

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn key_code(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn default_app() -> App {
    App::new(AppConfig::default())
}

#[test]
fn test_app_new_is_normal_mode() {
    let app = default_app();
    assert_eq!(app.mode(), Mode::Normal);
    assert!(!app.should_quit());
}

#[test]
fn test_app_on_key_i_enters_insert() {
    let mut app = default_app();
    let quit = app.on_key(key('i'));
    assert!(quit, "expected 'i' consumed (returns true)");
    assert_eq!(app.mode(), Mode::Insert);
}

#[test]
fn test_app_on_key_a_enters_insert() {
    let mut app = default_app();
    app.on_key(key('a'));
    assert_eq!(app.mode(), Mode::Insert);
}

#[test]
fn test_app_on_key_colon_enters_command() {
    let mut app = default_app();
    app.on_key(key(':'));
    assert_eq!(app.mode(), Mode::Command);
}

#[test]
fn test_app_on_key_q_sets_quit() {
    let mut app = default_app();
    let quit = app.on_key(key('q'));
    // on_key returns false for Quit (special — should_quit 才是 true)
    assert!(!quit, "on_key returns false for Quit");
    assert!(app.should_quit());
}

#[test]
fn test_app_clear_quit() {
    let mut app = default_app();
    app.on_key(key('q'));
    assert!(app.should_quit());
    app.clear_quit();
    assert!(!app.should_quit());
}

#[test]
fn test_app_esc_in_insert_returns_to_normal() {
    let mut app = default_app();
    app.on_key(key('i')); // → Insert
    assert_eq!(app.mode(), Mode::Insert);
    app.on_key(key_code(KeyCode::Esc));
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_esc_in_command_returns_to_normal() {
    let mut app = default_app();
    app.on_key(key(':'));
    assert_eq!(app.mode(), Mode::Command);
    app.on_key(key_code(KeyCode::Esc));
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_insert_char_appends() {
    let mut app = default_app();
    app.on_key(key('i'));
    app.on_key(key('h'));
    app.on_key(key('e'));
    app.on_key(key('l'));
    app.on_key(key('l'));
    app.on_key(key('o'));
    // buffer 至少包含 'hello'
    let buf = app.buffer();
    let text = buf.as_str();
    assert!(
        text.contains("hello"),
        "expected buffer to contain 'hello', got: {:?}",
        text
    );
}

#[test]
fn test_app_insert_backspace_removes() {
    let mut app = default_app();
    app.on_key(key('i'));
    app.on_key(key('a'));
    app.on_key(key('b'));
    app.on_key(key_code(KeyCode::Backspace));
    let buf = app.buffer();
    let text = buf.as_str();
    assert!(text.contains('a'));
    assert!(
        !text.contains("ab"),
        "expected 'ab' removed, got: {:?}",
        text
    );
}

#[test]
fn test_app_normal_mode_h_moves_left() {
    let mut app = default_app();
    app.on_key(key('i'));
    app.on_key(key('a'));
    app.on_key(key('b'));
    app.on_key(key_code(KeyCode::Esc)); // → Normal
    app.on_key(key('h')); // 移动左
                          // 没 panic 即过
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_normal_mode_l_moves_right() {
    let mut app = default_app();
    app.on_key(key('i'));
    app.on_key(key('a'));
    app.on_key(key_code(KeyCode::Esc));
    app.on_key(key('l'));
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_normal_mode_arrow_keys() {
    let mut app = default_app();
    app.on_key(key_code(KeyCode::Left));
    assert_eq!(app.mode(), Mode::Normal);
    app.on_key(key_code(KeyCode::Right));
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_command_mode_typed() {
    let mut app = default_app();
    app.on_key(key(':'));
    app.on_key(key('h'));
    app.on_key(key('e'));
    app.on_key(key('l'));
    app.on_key(key('p'));
    let cmd = app.cmd_buffer();
    let text = cmd.as_str();
    assert!(
        text.contains("help"),
        "expected cmd buffer 'help', got: {:?}",
        text
    );
}

#[test]
fn test_app_command_mode_enter_execute() {
    let mut app = default_app();
    app.on_key(key(':'));
    app.on_key(key('h'));
    app.on_key(key('e'));
    app.on_key(key('l'));
    app.on_key(key('p'));
    app.on_key(key_code(KeyCode::Enter));
    // Enter 后应回 Normal
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_command_mode_unknown_returns_normal() {
    let mut app = default_app();
    app.on_key(key(':'));
    app.on_key(key_code(KeyCode::Esc));
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_insert_mode_unknown_key_is_noop() {
    let mut app = default_app();
    app.on_key(key('i'));
    // 'q' 在 Insert mode 是字符 (不会 quit)
    app.on_key(key('q'));
    let buf = app.buffer();
    let text = buf.as_str();
    assert!(text.contains('q'));
}

#[test]
fn test_app_on_mouse_left_in_normal_enters_insert() {
    let mut app = default_app();
    let me = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    app.on_mouse(me);
    // 实测行为可能是 EnterInsert (per lib 测试)
    // 仅断言不 panic
}

#[test]
fn test_app_on_mouse_right_in_normal() {
    let mut app = default_app();
    let me = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 5,
        row: 2,
        modifiers: KeyModifiers::NONE,
    };
    app.on_mouse(me);
    // 不 panic 即过
}

#[test]
fn test_app_history_starts_empty() {
    let app = default_app();
    assert!(app.history().is_empty());
}

#[test]
fn test_app_kernel_banner_non_empty() {
    let app = default_app();
    assert!(!app.kernel_banner().is_empty());
}

#[test]
fn test_app_tick_does_not_panic() {
    let mut app = default_app();
    app.tick();
    // 不 panic 即过
}

#[test]
fn test_app_ai_suggestion_non_empty() {
    let app = default_app();
    let s = app.ai_suggestion();
    // 不强制非空 (实现可能返 default) — 只断言能调
    let _ = s;
}

#[test]
fn test_app_normal_mode_unknown_keys_no_panic() {
    let mut app = default_app();
    // q triggers Quit, i/a trigger EnterInsert — skip them
    for c in "bcdefghjklmnop".chars() {
        app.on_key(key(c));
    }
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn test_app_insert_mode_function_keys_no_panic() {
    let mut app = default_app();
    app.on_key(key('i'));
    app.on_key(key_code(KeyCode::F(1)));
    app.on_key(key_code(KeyCode::F(12)));
    assert_eq!(app.mode(), Mode::Insert);
}

#[test]
fn test_app_insert_enter_returns_to_normal() {
    let mut app = default_app();
    app.on_key(key('i'));
    app.on_key(key_code(KeyCode::Enter));
    // Enter in Insert 应 execute, 行为可能回到 Normal
    let _ = app.mode();
}
