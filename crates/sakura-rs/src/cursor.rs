//! sakura-rs — 光标 + 位置类型.
//!
//! 对标 sakura `CLogicPoint` / `CLogicInt` / `CLayoutPoint`. 用 i32 而不是 usize, 支持
//! 负值用于 `clamp` 时表示 "未设置" (类似 `std::optional<int>` 习惯).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// 逻辑行/列位置 — 用于文档模型 (按 char 计算, 不是 byte).
///
/// `row` 是 0-based 逻辑行号 (即 buffer 行号).
/// `col` 是 0-based char offset (UTF-8 按 char, 不按 byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LogicPos {
    pub row: i32,
    pub col: i32,
}

impl LogicPos {
    pub const ZERO: LogicPos = LogicPos { row: 0, col: 0 };

    pub fn new(row: i32, col: i32) -> Self {
        Self { row, col }
    }

    /// 设置 row/col, 负数 clamp 到 0.
    pub fn set(&mut self, row: i32, col: i32) {
        self.row = row.max(0);
        self.col = col.max(0);
    }

    /// 顺序比较: 先 row 后 col.
    pub fn cmp(&self, other: &LogicPos) -> std::cmp::Ordering {
        self.row.cmp(&other.row).then(self.col.cmp(&other.col))
    }
}

impl std::fmt::Display for LogicPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ln {}, Col {}", self.row + 1, self.col + 1)
    }
}

/// 范围 — `(start, end)` 闭区间.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LogicRange {
    pub start: LogicPos,
    pub end: LogicPos,
}

impl LogicRange {
    pub fn new(start: LogicPos, end: LogicPos) -> Self {
        Self { start, end }
    }

    /// normalize: start <= end
    pub fn normalize(&self) -> Self {
        if self.start.cmp(&self.end).is_le() {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}
