//! sakura-rs — Undo / Redo.
//!
//! 对标 sakura `COpe` / `COpeBlk` / `COpeBuf`:
//! - `EOpeCode` = { Insert, Delete, Replace, MoveCaret } (与 sakura 一致).
//! - `Ope` = 单个原子操作 (insert/delete + 位置 + 内容).
//! - `OpeBlk` = 一组连续操作 (sakura: 通常一个 keystroke 一个 OpeBlk, 包括 Ope + caret move).
//! - `OpeBuf` = `Vec<OpeBlk>` + `cursor` + `no_modified_index`. Apply/Undo/Redo 走 Ope stream.
//!
//! 设计要点:
//! - **Apply 永远向前**: `apply_blk(&mut DocLineMgr, blk)` 应用 + 记录 cursor 变化.
//! - **Undo 永远反向**: `undo_blk(&mut DocLineMgr, blk)` 反向应用 + 恢复光标.
//! - **Undo 单调性**: 每次 `push_blk` 都 +cursor (越界由 push_blk 自己 clip).
//! - **AI agent 集成**: OpeBuf 提供 `to_json()` / `from_json()` 序列化 buffer + undo history,
//!   任意 host (HTTP server / LSP / IDE) 都能 sync buffer state.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use crate::buffer::DocLineMgr;
use crate::cursor::{LogicPos, LogicRange};

/// 操作码 — 与 sakura `EOpeCode` 一致.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EOpeCode {
    /// 未知 (保留).
    Unknown = 0,
    /// 插入字符串.
    Insert = 1,
    /// 删除字符串.
    Delete = 2,
    /// 替换字符串 (delete + insert).
    Replace = 3,
    /// 光标移动 (用于复合 op).
    MoveCaret = 4,
}

/// 单个原子操作.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ope {
    /// 操作类型.
    pub code: EOpeCode,
    /// 操作前 cursor 位置.
    pub caret_before: LogicPos,
    /// 操作后 cursor 位置.
    pub caret_after: LogicPos,
    /// Insert/Delete/Replace 的范围.
    pub range: LogicRange,
    /// Insert/Replace 的插入文本 (Delete 时为空).
    pub inserted: String,
    /// Delete/Replace 的被删文本 (Insert 时为空).
    pub deleted: String,
}

impl Ope {
    pub fn new(code: EOpeCode, caret_before: LogicPos) -> Self {
        Self {
            code,
            caret_before,
            caret_after: caret_before,
            range: LogicRange::new(caret_before, caret_before),
            inserted: String::new(),
            deleted: String::new(),
        }
    }

    /// 应用此 Ope — 修改 buffer + 移动 cursor.
    pub fn apply(&self, mgr: &mut DocLineMgr) {
        match self.code {
            EOpeCode::Insert => {
                mgr.set_caret(self.caret_before);
                mgr.insert_str(self.caret_before, &self.inserted);
                mgr.set_caret(self.caret_after);
            }
            EOpeCode::Delete => {
                mgr.remove_range(self.range);
                mgr.set_caret(self.caret_after);
            }
            EOpeCode::Replace => {
                // Replace = 先 Delete range, 再 Insert at start
                mgr.remove_range(self.range);
                mgr.set_caret(self.range.start);
                mgr.insert_str(self.range.start, &self.inserted);
                mgr.set_caret(self.caret_after);
            }
            EOpeCode::MoveCaret => {
                mgr.set_caret(self.caret_after);
            }
            EOpeCode::Unknown => {}
        }
    }

    /// 反向应用 (Undo) — 用 Ope 自带的反向信息.
    pub fn undo(&self, mgr: &mut DocLineMgr) {
        match self.code {
            EOpeCode::Insert => {
                // Undo Insert = Delete inserted range
                let start = self.caret_before;
                let end = self.caret_after;
                mgr.remove_range(LogicRange::new(start, end));
                mgr.set_caret(self.caret_before);
            }
            EOpeCode::Delete => {
                // Undo Delete = Re-insert at range.start
                mgr.set_caret(self.range.start);
                mgr.insert_str(self.range.start, &self.deleted);
                mgr.set_caret(self.caret_before);
            }
            EOpeCode::Replace => {
                // Undo Replace = Delete inserted, Re-insert deleted at range.start
                let new_start = self.range.start;
                let new_end = self.caret_after;
                mgr.remove_range(LogicRange::new(new_start, new_end));
                mgr.set_caret(self.range.start);
                mgr.insert_str(self.range.start, &self.deleted);
                mgr.set_caret(self.caret_before);
            }
            EOpeCode::MoveCaret => {
                mgr.set_caret(self.caret_before);
            }
            EOpeCode::Unknown => {}
        }
    }
}

/// Ope 块 — 一组同时操作的 Ope (sakura: 一个 keystroke 一个 OpeBlk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeBlk {
    pub opes: Vec<Ope>,
}

impl OpeBlk {
    pub fn new() -> Self {
        Self { opes: Vec::new() }
    }

    pub fn push(&mut self, ope: Ope) {
        self.opes.push(ope);
    }

    /// 应用整个 OpeBlk.
    pub fn apply(&self, mgr: &mut DocLineMgr) {
        for ope in &self.opes {
            ope.apply(mgr);
        }
    }

    /// 反向应用整个 OpeBlk (按相反顺序, 但因每个 Ope 自带反向信息, 顺序不重要).
    pub fn undo(&self, mgr: &mut DocLineMgr) {
        for ope in self.opes.iter().rev() {
            ope.undo(mgr);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.opes.is_empty()
    }
}

impl Default for OpeBlk {
    fn default() -> Self {
        Self::new()
    }
}

/// Ope 缓冲 — undo/redo 历史.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpeBuf {
    blocks: Vec<OpeBlk>,
    /// 当前 undo 位置 (下一个要 undo 的 blk index, 也是下一个要 push blk 的 index).
    cursor: usize,
    /// "无修改" 状态时的 cursor 快照 (mark_saved 时记录).
    no_modified_index: usize,
    /// 最大历史长度 (超过时丢弃最旧的).
    max_len: usize,
}

impl OpeBuf {
    pub fn new(max_len: usize) -> Self {
        Self { max_len: max_len.max(1), ..Default::default() }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.blocks.len()
    }

    pub fn is_modified(&self) -> bool {
        self.cursor != self.no_modified_index
    }

    pub fn mark_saved(&mut self) {
        self.no_modified_index = self.cursor;
    }

    /// 推入新 OpeBlk — 同时裁剪 redo 历史 (cursor 之后丢弃).
    pub fn push(&mut self, blk: OpeBlk) {
        if blk.is_empty() {
            return;
        }
        // 丢弃 redo 部分
        self.blocks.truncate(self.cursor);
        self.blocks.push(blk);
        self.cursor = self.blocks.len();
        // 超过 max_len 丢弃最旧
        if self.blocks.len() > self.max_len {
            let drop = self.blocks.len() - self.max_len;
            self.blocks.drain(..drop);
            self.cursor = self.blocks.len();
            // 调整 no_modified_index
            if self.no_modified_index >= drop {
                self.no_modified_index -= drop;
            } else {
                self.no_modified_index = 0;
            }
        }
    }

    /// Undo 一个 OpeBlk — 返回被 undo 的 blk (供 AI agent 同步用).
    pub fn undo(&mut self, mgr: &mut DocLineMgr) -> Option<OpeBlk> {
        if !self.can_undo() {
            return None;
        }
        self.cursor -= 1;
        let blk = self.blocks[self.cursor].clone();
        blk.undo(mgr);
        Some(blk)
    }

    /// Redo 一个 OpeBlk.
    pub fn redo(&mut self, mgr: &mut DocLineMgr) -> Option<OpeBlk> {
        if !self.can_redo() {
            return None;
        }
        let blk = self.blocks[self.cursor].clone();
        blk.apply(mgr);
        self.cursor += 1;
        Some(blk)
    }

    /// 序列化全部 history (for AI agent sync).
    pub fn history_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op_insert(caret_before: LogicPos, text: &str) -> Ope {
        // 计算插入后 caret 末尾位置 (跨行文本最后一行末尾).
        let mut row = caret_before.row;
        let mut col = caret_before.col;
        for c in text.chars() {
            if c == '\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        let caret_after = LogicPos::new(row, col);
        Ope {
            code: EOpeCode::Insert,
            caret_before,
            caret_after,
            range: LogicRange::new(caret_before, caret_before),
            inserted: text.into(),
            deleted: String::new(),
        }
    }

    fn op_delete(range: LogicRange) -> Ope {
        let caret_after = range.start;
        let deleted = String::new();
        Ope {
            code: EOpeCode::Delete,
            caret_before: range.end,
            caret_after,
            range,
            inserted: String::new(),
            deleted,
        }
    }

    #[test]
    fn undo_redo_single_insert() {
        let mut mgr = DocLineMgr::new();
        let mut ob = OpeBuf::new(100);
        let ope = op_insert(LogicPos::ZERO, "hello");
        let after = ope.caret_after;
        ope.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![ope] });
        assert_eq!(mgr.line(0).unwrap().text, "hello");

        // Undo
        ob.undo(&mut mgr);
        assert_eq!(mgr.line(0).unwrap().text, "");
        assert!(!ob.can_undo());
        assert!(ob.can_redo());

        // Redo
        ob.redo(&mut mgr);
        assert_eq!(mgr.line(0).unwrap().text, "hello");
        assert_eq!(mgr.caret(), after);
        assert!(!ob.can_redo());
    }

    #[test]
    fn undo_redo_delete() {
        let mut mgr = DocLineMgr::new();
        mgr.insert_str(LogicPos::ZERO, "abcdef");
        let mut ob = OpeBuf::new(100);

        // Delete "bc" (col=1..3)
        let range = LogicRange::new(LogicPos::new(0, 1), LogicPos::new(0, 3));
        let deleted = mgr.extract_text(range);
        let mut ope = op_delete(range);
        ope.deleted = deleted;
        ope.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![ope] });
        assert_eq!(mgr.line(0).unwrap().text, "adef");

        ob.undo(&mut mgr);
        assert_eq!(mgr.line(0).unwrap().text, "abcdef");
    }

    #[test]
    fn undo_redo_multi_line_insert() {
        let mut mgr = DocLineMgr::new();
        let mut ob = OpeBuf::new(100);
        let ope = op_insert(LogicPos::ZERO, "line1\nline2\nline3");
        ope.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![ope] });
        assert_eq!(mgr.line_count(), 3);
        ob.undo(&mut mgr);
        assert_eq!(mgr.line_count(), 1);
        assert_eq!(mgr.line(0).unwrap().text, "");
    }

    #[test]
    fn is_modified_and_mark_saved() {
        let mut mgr = DocLineMgr::new();
        let mut ob = OpeBuf::new(100);
        assert!(!ob.is_modified());
        let ope = op_insert(LogicPos::ZERO, "x");
        ope.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![ope] });
        assert!(ob.is_modified());
        ob.mark_saved();
        assert!(!ob.is_modified());
        let ope2 = op_insert(mgr.caret(), "y");
        ope2.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![ope2] });
        assert!(ob.is_modified());
    }

    #[test]
    fn push_after_undo_discards_redo() {
        let mut mgr = DocLineMgr::new();
        let mut ob = OpeBuf::new(100);
        let o1 = op_insert(LogicPos::ZERO, "a");
        o1.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![o1] });
        let o2 = op_insert(mgr.caret(), "b");
        o2.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![o2] });
        assert_eq!(mgr.line(0).unwrap().text, "ab");

        ob.undo(&mut mgr);
        assert_eq!(mgr.line(0).unwrap().text, "a");
        assert!(ob.can_redo());

        // 推入新 op 应丢弃 redo 部分
        let o3 = op_insert(mgr.caret(), "c");
        o3.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![o3] });
        assert!(!ob.can_redo());
        assert_eq!(mgr.line(0).unwrap().text, "ac");
    }

    #[test]
    fn max_len_drops_oldest() {
        let mut mgr = DocLineMgr::new();
        let mut ob = OpeBuf::new(3);
        for c in "abcde".chars() {
            let ope = op_insert(mgr.caret(), &c.to_string());
            ope.apply(&mut mgr);
            ob.push(OpeBlk { opes: vec![ope] });
        }
        assert_eq!(ob.len(), 3);
        // Should be able to undo only 3 times (oldest "a" and "b" dropped)
        ob.undo(&mut mgr);
        ob.undo(&mut mgr);
        ob.undo(&mut mgr);
        assert_eq!(mgr.line(0).unwrap().text, "ab");
        assert!(!ob.can_undo());
    }

    #[test]
    fn serde_roundtrip() {
        let mut mgr = DocLineMgr::new();
        let mut ob = OpeBuf::new(100);
        let ope = op_insert(LogicPos::ZERO, "test\nlines");
        ope.apply(&mut mgr);
        ob.push(OpeBlk { opes: vec![ope] });
        let json = ob.history_json();
        let restored = OpeBuf::from_json(&json).unwrap();
        assert_eq!(restored.len(), 1);
        assert!(restored.can_undo());
    }
}
