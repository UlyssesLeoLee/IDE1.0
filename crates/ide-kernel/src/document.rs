//! `Document` — 内核的编辑单元 (单个文件).
//!
//! 把 `editcore` 的 `DocLineMgr` (内容) + `OpeBuf` (撤销栈) + `LogicPos`
//! (位置) 组合成一个**会话级可编辑文档**, 并补上 editcore 尚未提供、
//! 而 Sakura Editor 必须有的部分:
//!
//! - 选区 (可跨行) 与多光标 (multi-cursor)
//! - 行尾符抽象 (LF / CRLF / CR —— Sakura 的招牌能力, 日语编辑器常混用)
//! - 查找 / 替换 (含跨行匹配)
//! - 编码标记与往返转换
//! - 撤销栈的**事务边界** (一次按键 = 一个 OpeBlk, 而不是每个字符一条)

use editcore::buffer::DocLineMgr;
use editcore::cursor::{LogicPos, LogicRange};
use editcore::undo::{EOpeCode, Ope, OpeBlk, OpeBuf};
use serde::{Deserialize, Serialize};

/// 位置别名 — 对外用更通用的名字.
pub type Pos = LogicPos;

/// 行尾符.
///
/// Sakura Editor 的招牌能力: 同一文件里可能混用 LF/CRLF, 保存时不能"修正"。
/// 因此这里**存**行尾符信息, 而不是简单地全用 `\n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
    Cr,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
            LineEnding::Cr => "\r",
        }
    }

    pub fn detect(text: &str) -> Self {
        // 首次出现的行尾决定整个文件的行尾 (与 Sakura 一致)
        let crlf = text.find("\r\n");
        let lf = text.find('\n');
        let cr = text.find('\r');
        match (crlf, lf, cr) {
            (Some(c), Some(l), _) if c <= l => LineEnding::Crlf,
            (_, _, Some(c)) if Some(c) != crlf => LineEnding::Cr,
            _ => LineEnding::Lf,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LineEnding::Lf => "Unix (LF)",
            LineEnding::Crlf => "Windows (CRLF)",
            LineEnding::Cr => "Mac (CR)",
        }
    }
}

/// 选区 —— `anchor` 是固定端, `head` 是移动端.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Selection {
    pub anchor: Pos,
    pub head: Pos,
}

impl Selection {
    pub fn new(anchor: Pos, head: Pos) -> Self {
        Self { anchor, head }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// 规范化成 start <= end 的范围.
    pub fn normalized(&self) -> LogicRange {
        if pos_le(self.head, self.anchor) {
            LogicRange {
                start: self.head,
                end: self.anchor,
            }
        } else {
            LogicRange {
                start: self.anchor,
                end: self.head,
            }
        }
    }

    pub fn len_in_chars(&self, mgr: &DocLineMgr) -> usize {
        let r = self.normalized();
        mgr.extract_text(r).chars().count()
    }
}

/// 位置比较 (row 先, col 后).
pub fn pos_le(a: Pos, b: Pos) -> bool {
    a.row < b.row || (a.row == b.row && a.col <= b.col)
}

/// 文本编码标记 (仅记录, 转换在 kernel::encoding 或 shell 层).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    ShiftJis,
    EucJp,
    Iso2022Jp,
    Latin1,
    Cp1252,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 (BOM)",
            Encoding::Utf16Le => "UTF-16 LE",
            Encoding::Utf16Be => "UTF-16 BE",
            Encoding::ShiftJis => "Shift_JIS",
            Encoding::EucJp => "EUC-JP",
            Encoding::Iso2022Jp => "ISO-2022-JP",
            Encoding::Latin1 => "Latin-1",
            Encoding::Cp1252 => "CP1252",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        // 归一化: 去空白/连字符/下划线/括号, 大写.
        // "UTF-8 (BOM)" -> "UTF8BOM"; "UTF-16 LE" -> "UTF16LE"; "Shift_JIS" -> "SHIFTJIS"
        let n: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_' && *c != '(' && *c != ')')
            .collect::<String>()
            .to_ascii_uppercase();
        Some(match n.as_str() {
            "UTF8" => Encoding::Utf8,
            "UTF8BOM" => Encoding::Utf8Bom,
            "UTF16LE" | "UTF16" => Encoding::Utf16Le,
            "UTF16BE" => Encoding::Utf16Be,
            "SHIFTJIS" | "SJIS" | "CP932" => Encoding::ShiftJis,
            "EUCJP" => Encoding::EucJp,
            "ISO2022JP" => Encoding::Iso2022Jp,
            "LATIN1" | "ISO88591" => Encoding::Latin1,
            "CP1252" | "WINDOWS1252" => Encoding::Cp1252,
            _ => return None,
        })
    }

    /// 是否多字节编码 —— 影响移动/删除的 char 语义.
    pub fn is_multibyte(self) -> bool {
        !matches!(self, Encoding::Latin1 | Encoding::Cp1252)
    }
}

/// 文档快照 —— 外壳渲染只需要这些.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub text: String,
    pub line_ending: LineEnding,
    pub encoding: Encoding,
    pub caret: Pos,
    pub selection: Option<Selection>,
    pub dirty: bool,
    pub cursor_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// 撤销栈深度.
const UNDO_DEPTH: usize = 1000;

/// 单文档 —— 内核的核心.
pub struct Document {
    mgr: DocLineMgr,
    undo: OpeBuf,
    /// 主选区 (None = 无选区).
    selection: Option<Selection>,
    /// 附加光标 (multi-cursor). 不含主光标.
    extra_cursors: Vec<Pos>,
    /// 附加光标各自的选区 (与 extra_cursors 等长).
    extra_selections: Vec<Option<Selection>>,
    line_ending: LineEnding,
    encoding: Encoding,
    path: Option<String>,
    /// 是否处于「下一处输入要替换选区」的状态 (Sakura 的上書きモード).
    replace_pending: bool,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self {
            mgr: DocLineMgr::new(),
            undo: OpeBuf::new(UNDO_DEPTH),
            selection: None,
            extra_cursors: Vec::new(),
            extra_selections: Vec::new(),
            line_ending: LineEnding::Lf,
            encoding: Encoding::Utf8,
            path: None,
            replace_pending: false,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut d = Self::new();
        d.set_text(text);
        d
    }

    // ---------- 只读访问 ----------

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, p: impl Into<String>) {
        self.path = Some(p.into());
    }

    pub fn text(&self) -> String {
        self.mgr.text()
    }

    pub fn line_count(&self) -> i32 {
        self.mgr.line_count()
    }

    /// 按行取文本 (char 语义).
    pub fn line_text(&self, row: i32) -> String {
        self.mgr
            .line(row)
            .map(|l| l.text.clone())
            .unwrap_or_default()
    }

    pub fn lines(&self) -> &[editcore::buffer::DocLine] {
        self.mgr.lines()
    }

    pub fn caret(&self) -> Pos {
        self.mgr.caret()
    }

    pub fn set_caret(&mut self, p: Pos) {
        let p = self.clamp_pos(p);
        self.mgr.set_caret(p);
        self.selection = None;
        self.extra_cursors.clear();
        self.extra_selections.clear();
    }

    pub fn selection(&self) -> Option<Selection> {
        self.selection.filter(|s| !s.is_empty())
    }

    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// 是否已修改 (未保存).
    ///
    /// 用 `OpeBuf::is_modified()` 而不是 `DocLineMgr::is_dirty()`: 后者基于
    /// 行 seq 比较, 而 seq 只在 `remove_range` 与非空插入时推进, 无法正确
    /// 表达"撤销回保存点后应为干净"。`OpeBuf` 用 cursor 与保存快照比较,
    /// 语义与 Sakura 的未保存标记一致。
    pub fn is_dirty(&self) -> bool {
        self.undo.is_modified()
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    /// 当前光标总数 (含主光标).
    pub fn cursor_count(&self) -> usize {
        1 + self.extra_cursors.len()
    }

    pub fn extra_cursors(&self) -> &[Pos] {
        &self.extra_cursors
    }

    pub fn mgr(&self) -> &DocLineMgr {
        &self.mgr
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            text: self.mgr.text(),
            line_ending: self.line_ending,
            encoding: self.encoding,
            caret: self.mgr.caret(),
            selection: self.selection(),
            dirty: self.mgr.is_dirty(),
            cursor_count: self.cursor_count(),
            can_undo: self.undo.can_undo(),
            can_redo: self.undo.can_redo(),
        }
    }

    /// 导出的文本 —— 按声明的行尾符拼接 (Sakura 保存行为).
    pub fn export_text(&self) -> String {
        let raw = self.mgr.text();
        match self.line_ending {
            LineEnding::Lf => raw,
            _ => {
                let eol = self.line_ending.as_str();
                raw.split('\n').collect::<Vec<_>>().join(eol)
            }
        }
    }

    // ---------- 内容装载 ----------

    /// 整体替换内容 (清空 undo 历史, 用于打开文件).
    pub fn set_text(&mut self, text: &str) {
        self.line_ending = LineEnding::detect(text);
        let normalized = match self.line_ending {
            LineEnding::Lf => text.to_string(),
            _ => text.replace("\r\n", "\n").replace('\r', "\n"),
        };
        self.mgr = DocLineMgr::new();
        // 用一次 insert 建立初始内容, 不进 undo 栈
        // 直接灌入内容, **不进 undo 栈** —— 打开文件不是一次"编辑操作",
        // 用户按 Ctrl+Z 不应该把整个文件撤掉.
        self.mgr.insert_str(Pos::ZERO, &normalized);
        self.mgr.set_caret(Pos::ZERO);
        // 打开文件既不产生编辑历史, 也不产生 dirty
        self.reset_history();
        self.selection = None;
        self.extra_cursors.clear();
        self.extra_selections.clear();
        self.replace_pending = false;
    }

    pub fn set_line_ending(&mut self, e: LineEnding) {
        self.line_ending = e;
    }

    pub fn set_encoding(&mut self, e: Encoding) {
        self.encoding = e;
    }

    /// 标记已保存 (清 dirty 标记, 但保留 undo 历史).
    pub fn mark_saved(&mut self) {
        self.mgr.mark_saved();
        self.undo.mark_saved();
    }

    /// 打开/装载内容后调用 —— 同时重置内容与撤销栈的保存点.
    pub fn reset_history(&mut self) {
        self.undo = OpeBuf::new(UNDO_DEPTH);
        self.mgr.mark_saved();
    }

    // ---------- 位置工具 ----------

    /// 把位置夹到合法范围 (行存在, 列不超行长).
    pub fn clamp_pos(&self, p: Pos) -> Pos {
        let row = p.row.clamp(0, (self.mgr.line_count() - 1).max(0));
        let len = self.mgr.line(row).map(|l| l.char_len()).unwrap_or(0);
        Pos {
            row,
            col: p.col.clamp(0, len),
        }
    }

    fn clamp_range(&self, r: LogicRange) -> LogicRange {
        LogicRange {
            start: self.clamp_pos(r.start),
            end: self.clamp_pos(r.end),
        }
    }

    // ---------- 编辑原语 (全部进 undo) ----------

    /// 在位置插入字符串. 返回插入后的光标位置.
    pub fn insert(&mut self, at: Pos, s: &str) -> Pos {
        if s.is_empty() {
            return self.clamp_pos(at);
        }
        let at = self.clamp_pos(at);
        let caret_before = self.mgr.caret();
        let deleted = self.mgr.extract_text(LogicRange { start: at, end: at });
        let _ = deleted;
        let after = self.mgr.insert_str(at, s);

        let mut blk = OpeBlk::new();
        blk.push(Ope {
            code: EOpeCode::Insert,
            caret_before,
            caret_after: after,
            range: LogicRange { start: at, end: at },
            inserted: s.to_string(),
            deleted: String::new(),
        });
        self.undo.push(blk);
        after
    }

    /// 删除范围.
    pub fn delete_range(&mut self, range: LogicRange) -> Pos {
        let r = self.clamp_range(range);
        if r.start == r.end {
            return r.start;
        }
        let caret_before = self.mgr.caret();
        let deleted = self.mgr.extract_text(r);
        let after = self.mgr.remove_range(r);

        let mut blk = OpeBlk::new();
        blk.push(Ope {
            code: EOpeCode::Delete,
            caret_before,
            caret_after: after,
            range: r,
            inserted: String::new(),
            deleted,
        });
        self.undo.push(blk);
        after
    }

    /// 退格: 删除光标前一个 char.
    pub fn backspace(&mut self) -> Pos {
        let c = self.mgr.caret();
        if c.col > 0 {
            return self.delete_range(LogicRange {
                start: Pos {
                    row: c.row,
                    col: c.col - 1,
                },
                end: c,
            });
        }
        if c.row > 0 {
            let prev_len = self.mgr.line(c.row - 1).map(|l| l.char_len()).unwrap_or(0);
            return self.delete_range(LogicRange {
                start: Pos {
                    row: c.row - 1,
                    col: prev_len,
                },
                end: c,
            });
        }
        c
    }

    /// Delete: 删除光标后一个 char.
    pub fn delete_forward(&mut self) -> Pos {
        let c = self.mgr.caret();
        let len = self.mgr.line(c.row).map(|l| l.char_len()).unwrap_or(0);
        if c.col < len {
            return self.delete_range(LogicRange {
                start: c,
                end: Pos {
                    row: c.row,
                    col: c.col + 1,
                },
            });
        }
        if c.row + 1 < self.mgr.line_count() {
            return self.delete_range(LogicRange {
                start: c,
                end: Pos {
                    row: c.row + 1,
                    col: 0,
                },
            });
        }
        c
    }

    /// 插入换行 (按上一行缩进 —— Sakura 行为).
    pub fn insert_newline(&mut self) -> Pos {
        let c = self.mgr.caret();
        let indent: String = self
            .line_text(c.row)
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        let payload = format!("\n{indent}");
        self.insert(c, &payload)
    }

    /// 在当前选区上输入 —— 有选区则替换, 无则插入.
    /// 这是**上書きモード** 的核心: 一次按键 = 一个 undo 单元.
    pub fn type_char(&mut self, ch: char) -> Pos {
        let pos = if self.replace_pending {
            if let Some(sel) = self.selection() {
                let r = sel.normalized();
                self.delete_range(r)
            } else {
                self.mgr.caret()
            }
        } else if let Some(sel) = self.selection() {
            let r = sel.normalized();
            self.delete_range(r)
        } else {
            self.mgr.caret()
        };
        self.replace_pending = false;
        self.insert(pos, &ch.to_string())
    }

    /// 设置「下一处输入替换选区」.
    pub fn set_replace_pending(&mut self, on: bool) {
        self.replace_pending = on;
    }

    pub fn replace_pending(&self) -> bool {
        self.replace_pending
    }

    // ---------- 选区 ----------

    /// 开始/扩展选区: 设 anchor, head 跟随 caret.
    pub fn start_selection(&mut self, at: Pos) {
        let at = self.clamp_pos(at);
        self.selection = Some(Selection::new(at, at));
    }

    /// 扩展选区到指定位置.
    pub fn extend_selection_to(&mut self, p: Pos) {
        let p = self.clamp_pos(p);
        match &mut self.selection {
            Some(s) => s.head = p,
            None => {
                let c = self.mgr.caret();
                self.selection = Some(Selection::new(c, p));
            }
        }
    }

    /// 全选.
    pub fn select_all(&mut self) {
        let last_row = (self.mgr.line_count() - 1).max(0);
        let last_col = self.mgr.line(last_row).map(|l| l.char_len()).unwrap_or(0);
        self.selection = Some(Selection::new(
            Pos::ZERO,
            Pos {
                row: last_row,
                col: last_col,
            },
        ));
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn selected_text(&self) -> String {
        self.selection()
            .map(|s| self.mgr.extract_text(s.normalized()))
            .unwrap_or_default()
    }

    // ---------- 多光标 ----------

    /// 添加一个光标 (Ctrl+Click). 落在已有光标上则忽略.
    pub fn add_cursor(&mut self, p: Pos) {
        let p = self.clamp_pos(p);
        if p == self.mgr.caret() || self.extra_cursors.contains(&p) {
            return;
        }
        self.extra_cursors.push(p);
        self.extra_selections.push(None);
    }

    pub fn clear_extra_cursors(&mut self) {
        self.extra_cursors.clear();
        self.extra_selections.clear();
    }

    /// 所有光标位置 (主 + 附加, 按位置排序).
    pub fn all_cursors(&self) -> Vec<Pos> {
        let mut v = vec![self.mgr.caret()];
        v.extend_from_slice(&self.extra_cursors);
        v.sort_by_key(|p| (p.row, p.col));
        v.dedup();
        v
    }

    /// 在所有光标处插入 —— 一次按键 = 一个 undo 单元.
    ///
    /// 这是 multi-cursor 的关键: 若逐个 insert 会产生 N 条 undo 记录,
    /// 用户按一次 Ctrl+Z 会只撤掉一个光标, 行为违反直觉.
    pub fn insert_all_cursors(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let cursors = self.all_cursors();
        if cursors.len() == 1 {
            self.insert(cursors[0], s);
            return;
        }

        // 从**后往前**插, 这样前面位置的行号不会被前面的插入推移.
        //
        // 每个 Ope 的 caret_before 必须是**它自己的**位置, 而不是共享主光标:
        // `Ope::undo()` 对 Insert 的逆操作是"删除 [caret_before, caret_after)",
        // 若多个 Ope 共享同一个 caret_before, 逆序撤销时会删错位置.
        let first_char: String = s.chars().take(1).collect();
        let mut blk = OpeBlk::new();
        for pos in cursors.iter().rev() {
            let pos = self.clamp_pos(*pos);
            let after = self.mgr.insert_str(pos, &first_char);
            blk.push(Ope {
                code: EOpeCode::Insert,
                caret_before: pos,
                caret_after: after,
                range: LogicRange {
                    start: pos,
                    end: pos,
                },
                inserted: first_char.clone(),
                deleted: String::new(),
            });
        }
        self.undo.push(blk);

        // 主光标落到最靠前的那个光标处并前进一格
        if let Some(first) = cursors.first() {
            let target = Pos {
                row: first.row,
                col: first.col + first_char.chars().count() as i32,
            };
            self.mgr.set_caret(self.clamp_pos(target));
        }
        self.clear_extra_cursors();

        // 剩余字符 (粘贴多字符时) 走普通插入
        let rest: String = s.chars().skip(1).collect();
        if !rest.is_empty() {
            let c = self.mgr.caret();
            self.insert(c, &rest);
        }
    }

    // ---------- undo / redo ----------

    pub fn undo(&mut self) -> bool {
        match self.undo.undo(&mut self.mgr) {
            Some(_) => {
                self.selection = None;
                self.clear_extra_cursors();
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.undo.redo(&mut self.mgr) {
            Some(_) => {
                self.selection = None;
                self.clear_extra_cursors();
                true
            }
            None => false,
        }
    }

    /// 撤销栈的 JSON 快照 (用于持久化 / 调试).
    pub fn history_json(&self) -> String {
        self.undo.history_json()
    }
}
