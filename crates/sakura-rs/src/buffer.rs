//! sakura-rs — 文档 buffer (逻辑行 + 物理行).
//!
//! 对标 sakura `CDocLine` / `CDocLineMgr` / `CLogic`:
//! - `DocLine` = 一行物理行 (UTF-8 string + 顺序 seq).
//! - `DocLineMgr` = `Vec<DocLine>` + `Vec<usize>` seq 索引, 维护逻辑行 ↔ 物理行映射.
//!
//! 设计要点:
//! - 物理行 = 一个 `\n` 分隔的 raw string (不含 `\n`).
//! - 逻辑行 = 物理行按软换行规则切分 (tab 展开 + 行宽限制). 多个逻辑行可对应一个物理行.
//! - 当前实现: **简单模式** — 一个物理行 = 一个逻辑行 (无软换行). 后续扩展.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use crate::cursor::{LogicPos, LogicRange};

/// 单行物理行.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocLine {
    /// UTF-8 内容 (不含尾部 `\n`).
    pub text: String,
    /// 全局唯一顺序号 (与 sakura `CLogicInt` 一致). 用于 undo 的有序记录.
    pub seq: u64,
}

impl DocLine {
    pub fn new(text: impl Into<String>, seq: u64) -> Self {
        Self { text: text.into(), seq }
    }

    /// char 数量 (UTF-8 按 char).
    pub fn char_len(&self) -> i32 {
        self.text.chars().count() as i32
    }

    /// byte 数量.
    pub fn byte_len(&self) -> usize {
        self.text.len()
    }

    /// 是否为空行 (无任何字符).
    pub fn is_empty_line(&self) -> bool {
        self.text.is_empty()
    }

    /// 在 col 位置插入字符串 (UTF-8 safe via chars).
    /// 返回插入后 cursor 应在的位置 (col + text.chars().count()).
    pub fn insert_str(&mut self, col: i32, s: &str) -> i32 {
        let char_col = col.max(0) as usize;
        let idx = self.text.char_indices().nth(char_col).map(|(b, _)| b).unwrap_or(self.text.len());
        self.text.insert_str(idx, s);
        col + s.chars().count() as i32
    }

    /// 删除 [col, col+n) 范围 (n chars). 返回删除的字符串.
    pub fn remove_range_chars(&mut self, col: i32, n: i32) -> String {
        let char_col = col.max(0) as usize;
        let char_end = char_col + n.max(0) as usize;
        let start_byte = self.text.char_indices().nth(char_col).map(|(b, _)| b).unwrap_or(self.text.len());
        let end_byte = self.text.char_indices().nth(char_end).map(|(b, _)| b).unwrap_or(self.text.len());
        self.text.drain(start_byte..end_byte).collect()
    }

    /// 在 col 位置删除一个 char.
    pub fn delete_char(&mut self, col: i32) {
        self.remove_range_chars(col, 1);
    }

    /// 在 col 位置之前删除一个 char (Backspace).
    pub fn backspace_char(&mut self, col: i32) {
        if col > 0 {
            self.remove_range_chars(col - 1, 1);
        }
    }

    /// 在 col 位置插入单个字符.
    pub fn insert_char(&mut self, col: i32, c: char) {
        self.insert_str(col, &c.to_string());
    }

    /// 获取第 n 个字符 (0-indexed). 越界返回 None.
    pub fn char_at(&self, col: i32) -> Option<char> {
        self.text.chars().nth(col.max(0) as usize)
    }

    /// 行 substring [col_start, col_end) chars. 越界 clamp.
    pub fn slice_chars(&self, col_start: i32, col_end: i32) -> String {
        let s = col_start.max(0) as usize;
        let e = col_end.max(s as i32) as usize;
        self.text.chars().skip(s).take(e - s).collect()
    }
}

/// 文档 buffer — `Vec<DocLine>` + 顺序 seq 分配器.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocLineMgr {
    lines: Vec<DocLine>,
    /// 下一个待分配的 seq.
    next_seq: u64,
    /// 上次保存时的 seq 快照 (用于判断 dirty + undo 回溯到无修改).
    saved_at_seq: u64,
    /// 触发 dirty 的最近 seq (任何 insert/delete/split 都更新).
    last_modified_seq: u64,
    /// 当前 cursor 位置 (用于 undo 记录 / redo 后恢复).
    caret: LogicPos,
}

impl DocLineMgr {
    pub fn new() -> Self {
        Self {
            lines: vec![DocLine::new("", 0)],
            next_seq: 1,
            saved_at_seq: 0,
            last_modified_seq: 0,
            caret: LogicPos::ZERO,
        }
    }

    pub fn caret(&self) -> LogicPos {
        self.caret
    }

    pub fn set_caret(&mut self, p: LogicPos) {
        let max_row = self.lines.len() as i32 - 1;
        self.caret.row = p.row.max(0).min(max_row.max(0));
        let max_col = self.lines.get(self.caret.row as usize).map(|l| l.char_len()).unwrap_or(0);
        self.caret.col = p.col.max(0).min(max_col);
    }

    pub fn lines(&self) -> &[DocLine] {
        &self.lines
    }

    pub fn line(&self, row: i32) -> Option<&DocLine> {
        self.lines.get(row.max(0) as usize)
    }

    pub fn line_count(&self) -> i32 {
        self.lines.len() as i32
    }

    /// 整个文档文本 — `\n` 拼接.
    pub fn text(&self) -> String {
        let mut s = String::with_capacity(self.lines.iter().map(|l| l.byte_len() + 1).sum::<usize>());
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(&line.text);
        }
        s
    }

    pub fn is_dirty(&self) -> bool {
        // dirty = 文档变更后没保存 — 通过 saved_at_seq + 最近 seq 比较 (sakura 模式)
        // 简化: 我们用 last_set_seq 跟踪最近修改时的 seq
        self.last_modified_seq > self.saved_at_seq
    }

    /// 标记当前状态为 "已保存" — dirty = false.
    pub fn mark_saved(&mut self) {
        self.saved_at_seq = self.last_modified_seq;
    }

    /// 触发 dirty 的最近 seq (任何 insert/delete/split 都更新) — 已在 struct 中定义 (line 104).

    /// 分配下一个 seq.
    pub fn alloc_seq(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq += 1;
        s
    }

    /// 当前最近 seq (= 已分配的 max seq).
    pub fn current_max_seq(&self) -> u64 {
        self.next_seq.saturating_sub(1)
    }

    /// 在 (row, col) 处插入字符串 — 跨行插入 (含 `\n`) 时拆行.
    /// 返回插入后 caret 末尾位置 (row, col).
    pub fn insert_str(&mut self, at: LogicPos, s: &str) -> LogicPos {
        self.set_caret(at);
        let mut cursor = self.caret;
        let parts: Vec<&str> = s.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                // Enter — split current line at cursor.col, push new line below
                self.split_line_at(cursor);
                cursor.row += 1;
                cursor.col = 0;
                self.lines[cursor.row as usize].seq = self.alloc_seq();
            }
            if !part.is_empty() {
                // alloc 新 seq 给被修改的行 (first iteration 时也会重新 alloc — 覆盖 line 0 的初始 0)
                let new_seq = self.alloc_seq();
                self.lines[cursor.row as usize].seq = new_seq;
                let line = &mut self.lines[cursor.row as usize];
                cursor.col = line.insert_str(cursor.col, part);
                self.last_modified_seq = self.current_max_seq();
            }
        }
        self.caret = cursor;
        cursor
    }

    /// 在 row 行 col 位置切行 — 光标左侧留在 row, 右侧移到新 row+1.
    fn split_line_at(&mut self, at: LogicPos) {
        let row = at.row.max(0) as usize;
        let col = at.col.max(0) as i32;
        let old = self.lines[row].text.clone();
        let char_col = col as usize;
        let split_byte = old.char_indices().nth(char_col).map(|(b, _)| b).unwrap_or(old.len());
        let left = old[..split_byte].to_string();
        let right = old[split_byte..].to_string();
        self.lines[row].text = left;
        let new_seq = self.alloc_seq();
        self.lines.insert(row + 1, DocLine::new(right, new_seq));
        self.last_modified_seq = self.current_max_seq();
    }

    /// 删除范围 — 跨行合并.
    /// 返回删除后 cursor 位置.
    pub fn remove_range(&mut self, range: LogicRange) -> LogicPos {
        let r = range.normalize();
        self.set_caret(r.start);
        let mut s = self.caret;
        if r.start.row == r.end.row {
            // 单行删除
            let line = &mut self.lines[s.row as usize];
            line.remove_range_chars(s.col, r.end.col - r.start.col);
        } else {
            // 多行删除: 保留 start 行 col..end, 删除中间行, 拼接 start.row + end.row
            // 先把 start.row 右侧保留 + end.row 左侧保留 → start.row
            let end_line_text = self.lines[r.end.row as usize].text.clone();
            let end_line_col = r.end.col.max(0) as usize;
            let end_byte = end_line_text.char_indices().nth(end_line_col).map(|(b, _)| b).unwrap_or(end_line_text.len());
            let tail = end_line_text[end_byte..].to_string();
            let start_line = &mut self.lines[s.row as usize];
            let head_byte = start_line.text.char_indices().nth(s.col.max(0) as usize).map(|(b, _)| b).unwrap_or(start_line.text.len());
            start_line.text.truncate(head_byte);
            start_line.text.push_str(&tail);
            // 删除中间行 (start.row+1..=end.row)
            self.lines.drain(s.row as usize + 1..=r.end.row as usize);
        }
        self.last_modified_seq = self.current_max_seq();
        self.caret = s;
        s
    }

    /// Backspace — 在 at 位置删除 1 char (含跨行).
    pub fn backspace(&mut self, at: LogicPos) -> LogicPos {
        self.set_caret(at);
        if self.caret.col > 0 {
            // 同行删除一个 char
            let r = LogicRange { start: LogicPos::new(self.caret.row, self.caret.col - 1), end: self.caret };
            self.remove_range(r)
        } else if self.caret.row > 0 {
            // 行首 Backspace — 合并到上一行末尾
            let prev = LogicPos::new(self.caret.row - 1, self.lines()[self.caret.row as usize - 1].char_len());
            // 为合并后的行 alloc 新 seq
            let new_seq = self.alloc_seq();
            let result = self.remove_range(LogicRange { start: prev, end: self.caret });
            self.lines[result.row as usize].seq = new_seq;
            result
        } else {
            self.caret
        }
    }

    /// Delete — 在 at 位置删除右侧 1 char.
    pub fn delete(&mut self, at: LogicPos) -> LogicPos {
        self.set_caret(at);
        if self.caret.col < self.lines()[self.caret.row as usize].char_len() {
            let r = LogicRange { start: self.caret, end: LogicPos::new(self.caret.row, self.caret.col + 1) };
            self.remove_range(r)
        } else if (self.caret.row as usize) + 1 < self.lines.len() {
            // 行尾 Delete — 合并下一行
            let next = LogicPos::new(self.caret.row + 1, 0);
            self.remove_range(LogicRange { start: self.caret, end: next })
        } else {
            self.caret
        }
    }

    /// 移动 cursor (clamp to valid range).
    pub fn move_caret(&mut self, to: LogicPos) {
        self.set_caret(to);
    }

    /// 解析 `from` -> `to` 区间的纯文本 (用于剪贴板 / search).
    pub fn extract_text(&self, range: LogicRange) -> String {
        let r = range.normalize();
        if r.start.row == r.end.row {
            self.lines[r.start.row as usize].slice_chars(r.start.col, r.end.col)
        } else {
            let mut s = String::new();
            s.push_str(&self.lines[r.start.row as usize].slice_chars(r.start.col, self.lines[r.start.row as usize].char_len()));
            for row in r.start.row + 1..r.end.row {
                s.push('\n');
                s.push_str(&self.lines[row as usize].text);
            }
            if r.end.col > 0 {
                s.push('\n');
                s.push_str(&self.lines[r.end.row as usize].slice_chars(0, r.end.col));
            }
            s
        }
    }
}

impl Default for DocLineMgr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_has_one_empty_line() {
        let b = DocLineMgr::new();
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.line(0).unwrap().text, "");
        assert_eq!(b.caret(), LogicPos::ZERO);
        assert!(!b.is_dirty());
    }

    #[test]
    fn insert_text_in_empty_line() {
        let mut b = DocLineMgr::new();
        b.insert_str(LogicPos::ZERO, "hello");
        assert_eq!(b.line(0).unwrap().text, "hello");
        assert_eq!(b.caret(), LogicPos::new(0, 5));
        assert!(b.is_dirty());
    }

    #[test]
    fn insert_text_with_newlines_splits_lines() {
        let mut b = DocLineMgr::new();
        let end = b.insert_str(LogicPos::ZERO, "a\nb\nc");
        assert_eq!(b.line_count(), 3);
        assert_eq!(b.line(0).unwrap().text, "a");
        assert_eq!(b.line(1).unwrap().text, "b");
        assert_eq!(b.line(2).unwrap().text, "c");
        assert_eq!(end, LogicPos::new(2, 1));
    }

    #[test]
    fn backspace_at_line_start_merges_with_prev() {
        let mut b = DocLineMgr::new();
        b.insert_str(LogicPos::ZERO, "ab\ncd");
        // cursor at row=1 col=0, backspace -> merge to row=0 col=2
        b.backspace(LogicPos::new(1, 0));
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.line(0).unwrap().text, "abcd");
        assert_eq!(b.caret(), LogicPos::new(0, 2));
    }

    #[test]
    fn seq_increments_on_every_op() {
        let mut b = DocLineMgr::new();
        let s0 = b.current_max_seq();
        b.insert_str(LogicPos::ZERO, "a");
        let s1 = b.current_max_seq();
        assert!(s1 > s0);
        b.backspace(LogicPos::ZERO);
        let s2 = b.current_max_seq();
        assert!(s2 > s1);
    }

    #[test]
    fn is_dirty_then_mark_saved() {
        let mut b = DocLineMgr::new();
        assert!(!b.is_dirty());
        b.insert_str(LogicPos::ZERO, "x");
        assert!(b.is_dirty());
        b.mark_saved();
        assert!(!b.is_dirty());
    }

    #[test]
    fn extract_text_multi_line() {
        let mut b = DocLineMgr::new();
        b.insert_str(LogicPos::ZERO, "line1\nline2\nline3");
        let txt = b.extract_text(LogicRange::new(LogicPos::new(0, 1), LogicPos::new(2, 3)));
        assert_eq!(txt, "ine1\nline2\nlin");
    }

    #[test]
    fn utf8_handling_emoji() {
        let mut b = DocLineMgr::new();
        b.insert_str(LogicPos::ZERO, "🎉中文");
        // 🎉 = 1 char (4 bytes), 中 = 1 char (3 bytes), 文 = 1 char (3 bytes) → 3 chars
        assert_eq!(b.line(0).unwrap().text, "🎉中文");
        assert_eq!(b.line(0).unwrap().char_len(), 3);
        assert_eq!(b.line(0).unwrap().byte_len(), 10);
        assert_eq!(b.caret(), LogicPos::new(0, 3));
    }

    #[test]
    fn utf8_backspace_handles_emoji() {
        let mut b = DocLineMgr::new();
        b.insert_str(LogicPos::ZERO, "ab🎉cd");
        // at col=2 (between a/b and 🎉), backspace -> "a🎉cd"
        b.backspace(LogicPos::new(0, 2));
        assert_eq!(b.line(0).unwrap().text, "a🎉cd");
    }
}
