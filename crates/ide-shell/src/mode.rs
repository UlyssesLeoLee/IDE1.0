//! Vim 风格模式状态机 (per ULYS-191 §3 IDE Shell brief).
//!
//! 三态:
//! - Normal   默认, hjkl 移动, i/a/o 进入 Insert, : 进入 Command, q 退出
//! - Insert   字符直接插入 buffer, Esc 回 Normal
//! - Command  以 `:` 开头, 输入命令名, Enter 执行, Esc 回 Normal
//!
//! Cursor 在 mode 切换时保留 (Insert 内 cursor 不被吞)。

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Normal => f.write_str("NORMAL"),
            Mode::Insert => f.write_str("INSERT"),
            Mode::Command => f.write_str("COMMAND"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Normal.to_string(), "NORMAL");
        assert_eq!(Mode::Insert.to_string(), "INSERT");
        assert_eq!(Mode::Command.to_string(), "COMMAND");
    }

    #[test]
    fn test_mode_eq() {
        assert_eq!(Mode::Normal, Mode::Normal);
        assert_ne!(Mode::Normal, Mode::Insert);
    }
}
