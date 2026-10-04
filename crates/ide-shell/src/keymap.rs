//! 键位映射 — 把 KeyEvent + 当前 Mode 转成 Action.
//!
//! Stage 3.0 brief 范围(最小可用):
//! - Normal:  hjkl 移动 / i a o 进 Insert / : 进 Command / q 退出
//!   x 删除右侧 / 0 $ 行首行尾
//! - Insert:  字符插入 / Backspace / Enter(回车执行命令)/ Esc 回 Normal
//! - Command:  字符追加到 cmd / Backspace / Enter 执行 / Esc 回 Normal
//!
//! 鼠标 (mouse) 在 app 层单独处理,不进 keymap。

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::mode::Mode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 退出整个 app
    Quit,
    /// 切到 Insert
    EnterInsert,
    /// 切到 Normal
    EnterNormal,
    /// 切到 Command(并预置 ':')
    EnterCommand,
    /// 光标左/右/行首/行尾
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    /// Insert: 插入字符
    Insert(char),
    /// Backspace / Delete
    Backspace,
    Delete,
    /// Insert: Enter 触发执行当前 buffer 作为命令
    Execute,
    /// Command: Enter 触发执行 command buffer(去掉 ':' 前缀)
    ExecuteCommand(String),
    /// No-op (modifier 不识别 / 不在当前 mode 生效)
    Noop,
}

/// 把 KeyEvent + 当前 Mode 转成 Action。
pub fn map_key(event: KeyEvent, mode: Mode) -> Action {
    match mode {
        Mode::Normal => map_normal(event),
        Mode::Insert => map_insert(event),
        Mode::Command => map_command(event),
    }
}

fn map_normal(event: KeyEvent) -> Action {
    match event.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('i') | KeyCode::Char('a') => Action::EnterInsert,
        KeyCode::Char(':') => Action::EnterCommand,
        KeyCode::Char('h') | KeyCode::Left => Action::MoveLeft,
        KeyCode::Char('l') | KeyCode::Right => Action::MoveRight,
        KeyCode::Char('0') | KeyCode::Home => Action::MoveHome,
        KeyCode::Char('$') | KeyCode::End => Action::MoveEnd,
        KeyCode::Char('x') => Action::Delete,
        KeyCode::Esc | KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::EnterNormal
        }
        _ => Action::Noop,
    }
}

fn map_insert(event: KeyEvent) -> Action {
    match event.code {
        KeyCode::Esc => Action::EnterNormal,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Delete => Action::Delete,
        KeyCode::Enter => Action::Execute,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Char(c) => Action::Insert(c),
        _ => Action::Noop,
    }
}

fn map_command(event: KeyEvent) -> Action {
    match event.code {
        KeyCode::Esc => Action::EnterNormal,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Enter => Action::Execute, // Execute 由 app 层处理 (转 ExecuteCommand)
        KeyCode::Char(c) => Action::Insert(c),
        _ => Action::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn k(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn kc(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_normal_quit() {
        assert_eq!(map_key(k('q'), Mode::Normal), Action::Quit);
    }

    #[test]
    fn test_normal_enter_insert() {
        assert_eq!(map_key(k('i'), Mode::Normal), Action::EnterInsert);
        assert_eq!(map_key(k('a'), Mode::Normal), Action::EnterInsert);
    }

    #[test]
    fn test_normal_enter_command() {
        assert_eq!(map_key(k(':'), Mode::Normal), Action::EnterCommand);
    }

    #[test]
    fn test_normal_hjkl_arrows() {
        assert_eq!(map_key(k('h'), Mode::Normal), Action::MoveLeft);
        assert_eq!(map_key(k('l'), Mode::Normal), Action::MoveRight);
        assert_eq!(map_key(kc(KeyCode::Left), Mode::Normal), Action::MoveLeft);
        assert_eq!(map_key(kc(KeyCode::Right), Mode::Normal), Action::MoveRight);
    }

    #[test]
    fn test_normal_home_end() {
        assert_eq!(map_key(k('0'), Mode::Normal), Action::MoveHome);
        assert_eq!(map_key(kc(KeyCode::Home), Mode::Normal), Action::MoveHome);
        assert_eq!(map_key(k('$'), Mode::Normal), Action::MoveEnd);
    }

    #[test]
    fn test_insert_typed() {
        assert_eq!(map_key(k('x'), Mode::Insert), Action::Insert('x'));
    }

    #[test]
    fn test_insert_esc_to_normal() {
        assert_eq!(map_key(kc(KeyCode::Esc), Mode::Insert), Action::EnterNormal);
    }

    #[test]
    fn test_insert_enter_execute() {
        assert_eq!(map_key(kc(KeyCode::Enter), Mode::Insert), Action::Execute);
    }

    #[test]
    fn test_command_typed() {
        assert_eq!(map_key(k('q'), Mode::Command), Action::Insert('q'));
    }

    #[test]
    fn test_command_esc_to_normal() {
        assert_eq!(
            map_key(kc(KeyCode::Esc), Mode::Command),
            Action::EnterNormal
        );
    }

    #[test]
    fn test_noop_for_unmapped() {
        assert_eq!(map_key(k('z'), Mode::Normal), Action::Noop);
        assert_eq!(map_key(kc(KeyCode::F(1)), Mode::Normal), Action::Noop);
    }
}
