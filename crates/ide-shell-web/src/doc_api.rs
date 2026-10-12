//! ide-shell-web ↔ ide-kernel 桥接 —— 文档会话的 HTTP 适配层.
//!
//! # 为什么不直接复用 `ShellState` 的 read_file/write_file
//!
//! 那两个端点是**无状态**的文件读写: 前端拿到内容后自己维护 buffer 行数组、
//! 光标、选区、undo 栈。结果是编辑语义只存在于 JS 里 (editor.html ≈4400 行),
//! Rust 侧的 `ide-kernel::Document` 完全用不上 —— 这正是两套引擎不一致的根源。
//!
//! 本模块提供**有状态**的文档 API: 服务端持有 `Session`, 前端只发
//! 「在 (row,col) 插入 X」「撤销」这类**语义操作**, 不再自己算文本。
//!
//! # 迁移路径
//!
//! 前端可以分阶段切换:
//! 1. 先用本模块的 `/api/doc/*` 做新功能 (如十六进制、比较), 旧路径不动;
//! 2. 编辑内核功能 (undo/redo/查找替换/多光标) 逐步改走本模块;
//! 3. 最终 `read_file`/`write_file` 只用于「装载/保存」, 编辑全程在服务端。
//!
//! 步骤 1 和 2 可并存 —— 这也是本模块不改现有路由的原因。

use ide_kernel::document::{Document, Encoding, LineEnding};
use ide_kernel::session::Session;
use ide_kernel::text_ops::{self, FindOptions};
use serde::{Deserialize, Serialize};

/// 单文档的 HTTP 表示.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocView {
    pub id: u64,
    pub path: Option<String>,
    pub text: String,
    pub line_count: i32,
    pub caret_row: i32,
    pub caret_col: i32,
    pub line_ending: &'static str,
    pub encoding: &'static str,
    pub dirty: bool,
    pub has_selection: bool,
    pub selected_text: String,
    pub cursor_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
}

fn view(id: u64, d: &Document) -> DocView {
    DocView {
        id,
        path: d.path().map(str::to_string),
        text: d.text(),
        line_count: d.line_count(),
        caret_row: d.caret().row,
        caret_col: d.caret().col,
        line_ending: d.line_ending().label(),
        encoding: d.encoding().label(),
        dirty: d.is_dirty(),
        has_selection: d.has_selection(),
        selected_text: d.selected_text(),
        cursor_count: d.cursor_count(),
        can_undo: d.can_undo(),
        can_redo: d.can_redo(),
    }
}

/// 服务端持有的文档会话.
pub struct DocStore {
    session: Session,
}

impl Default for DocStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DocStore {
    pub fn new() -> Self {
        Self { session: Session::new() }
    }

    /// 打开文本为新标签, 返回视图.
    pub fn open(&mut self, path: Option<&str>, text: &str) -> DocView {
        let id = self.session.open_text(path, text);
        let d = self.session.get(id).expect("刚打开的文档必然存在");
        view(id, d)
    }

    /// 当前活动文档的视图 (无文档时 None).
    pub fn active(&self) -> Option<DocView> {
        let id = self.session.active_id()?;
        self.session.get(id).map(|d| view(id, d))
    }

    /// 全部标签.
    pub fn tabs(&self) -> Vec<DocView> {
        self.session
            .tab_ids()
            .iter()
            .filter_map(|id| self.session.get(*id).map(|d| view(*id, d)))
            .collect()
    }

    pub fn activate(&mut self, id: u64) -> Option<DocView> {
        if self.session.activate(id) {
            self.active()
        } else {
            None
        }
    }

    pub fn close(&mut self, id: u64) -> Option<DocView> {
        self.session.close(id);
        self.active()
    }

    fn with_doc_mut<T>(&mut self, f: impl FnOnce(&mut Document) -> T) -> Option<T> {
        let id = self.session.active_id()?;
        let d = self.session.get_mut(id)?;
        Some(f(d))
    }

    // ---------- 编辑原语 ----------

    pub fn insert(&mut self, row: i32, col: i32, s: &str) -> Option<DocView> {
        let r = self.with_doc_mut(|d| {
            let p = d.insert(ide_kernel::document::Pos { row, col }, s);
            d.set_caret(p);
        });
        r.and_then(|_| self.active())
    }

    /// 在指定位置输入一个字符.
    ///
    /// 注意: **不走 `set_caret`** —— 它会清掉选区, 而 `type_char` 的语义
    /// 恰恰是"有选区则替换"(上書きモード). 因此仅在无选区时才移动光标.
    pub fn type_char(&mut self, row: i32, col: i32, ch: char) -> Option<DocView> {
        self.with_doc_mut(|d| {
            if !d.has_selection() {
                d.set_caret(ide_kernel::document::Pos { row, col });
            }
            d.type_char(ch);
        });
        self.active()
    }

    pub fn insert_newline(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.insert_newline();
        });
        self.active()
    }

    pub fn backspace(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.backspace();
        });
        self.active()
    }

    pub fn delete_forward(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.delete_forward();
        });
        self.active()
    }

    pub fn delete_range(&mut self, r0: i32, c0: i32, r1: i32, c1: i32) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.delete_range(sakura_rs::cursor::LogicRange {
                start: ide_kernel::document::Pos { row: r0, col: c0 },
                end: ide_kernel::document::Pos { row: r1, col: c1 },
            });
        });
        self.active()
    }

    pub fn set_caret(&mut self, row: i32, col: i32) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.set_caret(ide_kernel::document::Pos { row, col });
        });
        self.active()
    }

    pub fn select_all(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.select_all();
        });
        self.active()
    }

    pub fn clear_selection(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.clear_selection();
        });
        self.active()
    }

    pub fn undo(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.undo();
        });
        self.active()
    }

    pub fn redo(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.redo();
        });
        self.active()
    }

    // ---------- 多光标 ----------

    pub fn add_cursor(&mut self, row: i32, col: i32) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.add_cursor(ide_kernel::document::Pos { row, col });
        });
        self.active()
    }

    pub fn clear_cursors(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.clear_extra_cursors();
        });
        self.active()
    }

    pub fn insert_all_cursors(&mut self, s: &str) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.insert_all_cursors(s);
        });
        self.active()
    }

    // ---------- 查找替换 ----------

    pub fn find(&mut self, pattern: &str, case_sensitive: bool, from_row: i32, from_col: i32) -> Option<FindHit> {
        let id = self.session.active_id()?;
        let d = self.session.get(id)?;
        let opts = FindOptions { pattern: pattern.to_string(), case_sensitive, ..Default::default() };
        let hit = text_ops::find_from(d, &opts, ide_kernel::document::Pos { row: from_row, col: from_col })
            .or_else(|| text_ops::find_from_start(d, &opts))?;
        Some(FindHit {
            start_row: hit.start.row,
            start_col: hit.start.col,
            end_row: hit.end.row,
            end_col: hit.end.col,
        })
    }

    pub fn replace_all(&mut self, pattern: &str, replacement: &str, case_sensitive: bool, limit: usize) -> Option<ReplaceStat> {
        self.with_doc_mut(|d| {
            let opts = FindOptions { pattern: pattern.to_string(), case_sensitive, ..Default::default() };
            let out = text_ops::replace_all(d, &opts, replacement, limit);
            ReplaceStat { replaced: out.replaced, truncated: out.truncated }
        })
    }

    pub fn find_all(&mut self, pattern: &str, case_sensitive: bool, limit: usize) -> Option<Vec<FindHit>> {
        let id = self.session.active_id()?;
        let d = self.session.get(id)?;
        let opts = FindOptions { pattern: pattern.to_string(), case_sensitive, ..Default::default() };
        Some(
            text_ops::find_all(d, &opts, limit)
                .into_iter()
                .map(|h| FindHit {
                    start_row: h.start.row,
                    start_col: h.start.col,
                    end_row: h.end.row,
                    end_col: h.end.col,
                })
                .collect(),
        )
    }

    // ---------- 格式 ----------

    pub fn set_line_ending(&mut self, e: LineEnding) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.set_line_ending(e);
        });
        self.active()
    }

    pub fn set_encoding(&mut self, e: Encoding) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.set_encoding(e);
        });
        self.active()
    }

    /// 导出内容: 返回 (path 可选, 含正确行尾符的文本).
    ///
    /// path 为 None 表示未保存的新文档 —— 此时前端可拿 `text` 做
    /// "另存为"的初始内容, 不该因为没有路径就整个失败.
    pub fn export(&self) -> Option<(Option<String>, String)> {
        let id = self.session.active_id()?;
        let d = self.session.get(id)?;
        Some((d.path().map(str::to_string), d.export_text()))
    }

    /// 标记已保存 (调用方实际写盘后).
    pub fn mark_saved(&mut self) -> Option<DocView> {
        self.with_doc_mut(|d| {
            d.mark_saved();
        });
        self.active()
    }

    pub fn new_tab(&mut self) -> Option<DocView> {
        let id = self.session.new_tab();
        self.session.get(id).map(|d| view(id, d))
    }

    /// 未保存的标签 id 列表 (前端关闭全部前提示).
    pub fn dirty_tabs(&self) -> Vec<u64> {
        self.session.dirty_tabs()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindHit {
    pub start_row: i32,
    pub start_col: i32,
    pub end_row: i32,
    pub end_col: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceStat {
    pub replaced: usize,
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_view() {
        let mut s = DocStore::new();
        let v = s.open(Some("a.txt"), "hello");
        assert_eq!(v.text, "hello");
        assert_eq!(v.line_count, 1);
        assert!(!v.dirty, "刚打开不应 dirty");
        assert_eq!(v.path.as_deref(), Some("a.txt"));
    }

    #[test]
    fn insert_then_undo() {
        let mut s = DocStore::new();
        s.open(Some("a.txt"), "ac");
        s.set_caret(0, 1);
        s.insert(0, 1, "b");
        assert_eq!(s.active().unwrap().text, "abc");
        s.undo();
        assert_eq!(s.active().unwrap().text, "ac");
        s.redo();
        assert_eq!(s.active().unwrap().text, "abc");
    }

    #[test]
    fn type_char_replaces_selection() {
        let mut s = DocStore::new();
        s.open(None, "hello");
        s.select_all();
        s.type_char(0, 0, 'X');
        assert_eq!(s.active().unwrap().text, "X");
    }

    #[test]
    fn find_returns_char_positions() {
        let mut s = DocStore::new();
        s.open(None, "foo bar\nfoo baz");
        let hit = s.find("foo", false, 0, 0).unwrap();
        assert_eq!((hit.start_row, hit.start_col), (0, 0));
        assert_eq!((hit.end_row, hit.end_col), (0, 3));
    }

    #[test]
    fn find_wraps_to_top() {
        let mut s = DocStore::new();
        s.open(None, "aaa\nbbb");
        // 从 (1,0) 起找 —— 第二行没有, 应绕回第一行
        let hit = s.find("aaa", true, 1, 0).unwrap();
        assert_eq!(hit.start_row, 0);
    }

    #[test]
    fn replace_all_updates_text() {
        let mut s = DocStore::new();
        s.open(None, "cat cat cat");
        let st = s.replace_all("cat", "dog", true, 100).unwrap();
        assert_eq!(st.replaced, 3);
        assert_eq!(s.active().unwrap().text, "dog dog dog");
    }

    #[test]
    fn export_preserves_crlf() {
        let mut s = DocStore::new();
        s.open(Some("crlf.txt"), "a\r\nb");
        let (path, text) = s.export().expect("应能导出");
        assert_eq!(text, "a\r\nb", "CRLF 往返应无损");
        assert_eq!(path.as_deref(), Some("crlf.txt"));
    }

    #[test]
    fn export_works_without_path() {
        let mut s = DocStore::new();
        s.open(None, "draft");
        let (path, text) = s.export().expect("未保存文档也应能导出文本");
        assert_eq!(path, None, "未保存文档没有路径");
        assert_eq!(text, "draft", "文本仍应可用 (供'另存为'初始内容)");
    }

    #[test]
    fn multibyte_positions_are_char_based() {
        let mut s = DocStore::new();
        s.open(None, "日本語");
        // col 2 是 "本"
        let v = s.insert(0, 2, "X").unwrap();
        assert_eq!(v.text, "日本X語");
    }

    #[test]
    fn tabs_are_independent() {
        let mut s = DocStore::new();
        let a = s.open(Some("a.txt"), "AAA").id;
        let b = s.open(Some("b.txt"), "BBB").id;
        assert_eq!(s.active().unwrap().id, b);
        s.activate(a);
        assert_eq!(s.active().unwrap().text, "AAA");
        assert_eq!(s.tabs().len(), 2);
    }

    #[test]
    fn dirty_tracked_per_tab() {
        let mut s = DocStore::new();
        let a = s.open(Some("a.txt"), "A").id;
        let b = s.open(Some("b.txt"), "B").id;
        s.insert(0, 1, "x");
        assert_eq!(s.dirty_tabs(), vec![b], "只有 b 被编辑过");
        // 关闭 a (未修改的那个), 活动文档应仍是 b
        let now_active = s.close(a).expect("关闭后仍有活动文档");
        assert_eq!(now_active.id, b);
        assert_eq!(s.tabs().len(), 1);
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut s = DocStore::new();
        s.open(Some("a.txt"), "A");
        s.insert(0, 1, "x");
        assert!(s.active().unwrap().dirty);
        s.mark_saved();
        assert!(!s.active().unwrap().dirty);
        assert!(s.active().unwrap().can_undo, "保存不应清 undo");
    }
}