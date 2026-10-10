//! keymap unit tests — vim-like 键位映射 (Normal/Insert/Command modes).
//!
//! 补充 lib 内 tests — 覆盖更多 branches 提升 keymap.rs 覆盖率.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ide_shell::keymap::{map_key, Action};
use ide_shell::mode::Mode;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key_ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn key_shift(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
}

#[test]
fn test_normal_mode_x_is_delete() {
    assert_eq!(
        map_key(key(KeyCode::Char('x')), Mode::Normal),
        Action::Delete
    );
}

#[test]
fn test_normal_mode_ctrl_x_is_delete() {
    // Ctrl-x 实测映射 Delete
    let r = map_key(key_ctrl('x'), Mode::Normal);
    assert_eq!(r, Action::Delete);
}

#[test]
fn test_normal_mode_esc_is_noop() {
    let r = map_key(key(KeyCode::Esc), Mode::Normal);
    assert_eq!(r, Action::Noop);
}

#[test]
fn test_normal_mode_ctrl_c_is_enter_normal() {
    let r = map_key(key_ctrl('c'), Mode::Normal);
    assert_eq!(r, Action::EnterNormal);
}

#[test]
fn test_normal_mode_backspace() {
    let r = map_key(key(KeyCode::Backspace), Mode::Normal);
    assert_eq!(r, Action::Noop);
}

#[test]
fn test_normal_mode_delete_key() {
    let r = map_key(key(KeyCode::Delete), Mode::Normal);
    assert_eq!(r, Action::Noop);
}

#[test]
fn test_normal_mode_function_keys() {
    assert_eq!(map_key(key(KeyCode::F(1)), Mode::Normal), Action::Noop);
    assert_eq!(map_key(key(KeyCode::F(12)), Mode::Normal), Action::Noop);
}

#[test]
fn test_normal_mode_tab() {
    assert_eq!(map_key(key(KeyCode::Tab), Mode::Normal), Action::Noop);
}

#[test]
fn test_normal_mode_up_down() {
    assert_eq!(map_key(key(KeyCode::Up), Mode::Normal), Action::Noop);
    assert_eq!(map_key(key(KeyCode::Down), Mode::Normal), Action::Noop);
}

#[test]
fn test_normal_mode_shift_letters() {
    // shift 不影响 normal mode 单字母
    assert_eq!(map_key(key_shift('q'), Mode::Normal), Action::Quit);
    assert_eq!(map_key(key_shift('h'), Mode::Normal), Action::MoveLeft);
    assert_eq!(map_key(key_shift('l'), Mode::Normal), Action::MoveRight);
}

#[test]
fn test_insert_mode_unicode_chars() {
    assert_eq!(
        map_key(key(KeyCode::Char('中')), Mode::Insert),
        Action::Insert('中')
    );
    assert_eq!(
        map_key(key(KeyCode::Char('日')), Mode::Insert),
        Action::Insert('日')
    );
    assert_eq!(
        map_key(key(KeyCode::Char('🎉')), Mode::Insert),
        Action::Insert('🎉')
    );
}

#[test]
fn test_insert_mode_punctuation() {
    assert_eq!(
        map_key(key(KeyCode::Char('.')), Mode::Insert),
        Action::Insert('.')
    );
    assert_eq!(
        map_key(key(KeyCode::Char(',')), Mode::Insert),
        Action::Insert(',')
    );
    assert_eq!(
        map_key(key(KeyCode::Char(';')), Mode::Insert),
        Action::Insert(';')
    );
}

#[test]
fn test_insert_mode_function_keys_noop() {
    assert_eq!(map_key(key(KeyCode::F(5)), Mode::Insert), Action::Noop);
}

#[test]
fn test_insert_mode_tab_is_noop() {
    assert_eq!(map_key(key(KeyCode::Tab), Mode::Insert), Action::Noop);
}

#[test]
fn test_command_mode_chars() {
    assert_eq!(
        map_key(key(KeyCode::Char('a')), Mode::Command),
        Action::Insert('a')
    );
    assert_eq!(
        map_key(key(KeyCode::Char('z')), Mode::Command),
        Action::Insert('z')
    );
    assert_eq!(
        map_key(key(KeyCode::Char('9')), Mode::Command),
        Action::Insert('9')
    );
}

#[test]
fn test_command_mode_enter() {
    assert_eq!(
        map_key(key(KeyCode::Enter), Mode::Command),
        Action::Execute
    );
}

#[test]
fn test_command_mode_backspace() {
    assert_eq!(
        map_key(key(KeyCode::Backspace), Mode::Command),
        Action::Backspace
    );
}

#[test]
fn test_command_mode_movement_keys_noop() {
    assert_eq!(map_key(key(KeyCode::Left), Mode::Command), Action::Noop);
    assert_eq!(map_key(key(KeyCode::Right), Mode::Command), Action::Noop);
    assert_eq!(map_key(key(KeyCode::Home), Mode::Command), Action::Noop);
    assert_eq!(map_key(key(KeyCode::End), Mode::Command), Action::Noop);
    assert_eq!(map_key(key(KeyCode::Delete), Mode::Command), Action::Noop);
}

#[test]
fn test_command_mode_unicode() {
    assert_eq!(
        map_key(key(KeyCode::Char('中')), Mode::Command),
        Action::Insert('中')
    );
}

#[test]
fn test_command_mode_function_keys() {
    assert_eq!(map_key(key(KeyCode::F(1)), Mode::Command), Action::Noop);
    assert_eq!(map_key(key(KeyCode::Tab), Mode::Command), Action::Noop);
}

#[test]
fn test_insert_mode_char_quoted_no_special() {
    assert_eq!(
        map_key(key(KeyCode::Char('q')), Mode::Insert),
        Action::Insert('q')
    );
    assert_eq!(
        map_key(key(KeyCode::Char('i')), Mode::Insert),
        Action::Insert('i')
    );
    assert_eq!(
        map_key(key(KeyCode::Char(':')), Mode::Insert),
        Action::Insert(':')
    );
}
