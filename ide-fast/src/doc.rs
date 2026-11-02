//! 文档模型: 行向量 + 光标 / 选区 + 撤销栈, 编辑和移动全部交给 `editcore` (和 Tauri 版前端走同一套 Rust 语义)。
//!
//! editcore 的编辑 / 移动 / 查找是**无状态**的: 调用方交上光标周围需要的几行, 它返回一个行补丁 + 新光标 / 选区。
//! 这里就是那个"调用方": 组请求 -> 调 `editcore` -> 套补丁 -> 记撤销。所以 Sakura 的键位语义 (词移动、行首智能 Home、
//! 自动缩进、整行剪切……) 不用在 GUI 里重写第二份。

use editcore::document_io::{self, AtomicWriteOpts, Bom, Charset, Eol, SaveSpec};
use editcore::edit_cmd::{self, EditReq};
use editcore::nav::{self, NavReq};
use editcore::search_ops::{self, SearchReq};
use std::path::{Path, PathBuf};

/// 一次可撤销的行补丁 (撤销 = 把插入的行换回删掉的行)
#[derive(Clone)]
struct Undo {
    start: usize,
    removed: Vec<String>,
    inserted: Vec<String>,
    before: ([usize; 2], Option<[usize; 4]>),
    after: [usize; 2],
}

pub struct Doc {
    #[allow(dead_code)]
    pub id: u64,
    pub path: Option<PathBuf>,
    pub name: String,
    pub lines: Vec<String>,
    pub cursor: [usize; 2],
    /// 选区 [r0, c0, r1, c1] (已排好序) 与锚点
    pub sel: Option<[usize; 4]>,
    pub anchor: Option<[usize; 2]>,
    /// 上下移动时记住的目标列
    pub goal_col: Option<usize>,
    pub dirty: bool,
    pub charset: Charset,
    pub bom: Bom,
    pub eol: Eol,
    undo: Vec<Undo>,
    redo: Vec<Undo>,
    pub tip: String,
    /// 编辑区该滚到哪 (行); 光标移动后置位, 绘制时消费
    pub reveal: bool,
}

pub const TAB: usize = 4;

impl Doc {
    pub fn untitled(id: u64, n: usize) -> Self {
        Doc { id, path: None, name: if n <= 1 { "无标题".into() } else { format!("无标题 {n}") }, lines: vec![String::new()], cursor: [0, 0], sel: None, anchor: None, goal_col: None, dirty: false,
              charset: Charset::Utf8, bom: Bom::None, eol: Eol::Crlf, undo: vec![], redo: vec![], tip: String::new(), reveal: false }
    }

    pub fn open(id: u64, path: &Path) -> Result<Self, String> {
        let d = document_io::read_doc(path, None)?;
        let mut lines: Vec<String> = d.content.split('\n').map(str::to_string).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "?".into());
        Ok(Doc { id, path: Some(path.to_path_buf()), name, lines, cursor: [0, 0], sel: None, anchor: None, goal_col: None, dirty: false, charset: d.charset, bom: d.bom, eol: d.eol,
                 undo: vec![], redo: vec![], tip: format!("{} · {} · {}", d.charset.label(), d.eol.label(), if d.lossy { "有无法解码的字节" } else { "已读取" }), reveal: true })
    }

    pub fn text_lf(&self) -> String {
        self.lines.join("\n")
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        let spec = SaveSpec { charset: self.charset, bom: self.bom, eol: self.eol, allow_lossy: false, write: AtomicWriteOpts { clear_readonly: false, sync: true } };
        document_io::save_document(path, &self.text_lf(), &spec).map_err(|e| e.to_string())?;
        self.path = Some(path.to_path_buf());
        self.name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.name.clone());
        self.dirty = false;
        Ok(())
    }

    pub fn row_len(&self, r: usize) -> usize {
        self.lines.get(r).map(|l| l.chars().count()).unwrap_or(0)
    }

    /// 光标周围 + 选区涉及的行 (editcore 只要这些; 大文件里每次按键不用把整篇交出去)
    fn rows_for(&self, extra: &[usize]) -> Vec<(usize, String)> {
        let mut set: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let (r, _) = (self.cursor[0], self.cursor[1]);
        for d in [r.saturating_sub(1), r, r + 1] {
            if d < self.lines.len() {
                set.insert(d);
            }
        }
        if let Some(s) = self.sel {
            for x in s[0]..=s[2].min(self.lines.len().saturating_sub(1)) {
                set.insert(x);
            }
        }
        for &e in extra {
            if e < self.lines.len() {
                set.insert(e);
            }
        }
        set.into_iter().map(|i| (i, self.lines[i].clone())).collect()
    }

    fn splice(&mut self, start: usize, remove: usize, insert: Vec<String>) {
        let end = (start + remove).min(self.lines.len());
        self.lines.splice(start.min(self.lines.len())..end, insert);
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
    }

    fn clamp(&mut self) {
        let last = self.lines.len().saturating_sub(1);
        self.cursor[0] = self.cursor[0].min(last);
        self.cursor[1] = self.cursor[1].min(self.row_len(self.cursor[0]));
    }

    /// 执行一条 editcore 编辑命令; 返回要写进剪贴板的文字 (复制 / 剪切类)
    pub fn edit(&mut self, cmd: &str, text: Option<String>) -> Option<String> {
        let req = EditReq {
            cmd: cmd.into(), row: self.cursor[0], col: self.cursor[1], sel: self.sel, total_rows: self.lines.len(), auto_indent: true, smart_indent: false, text,
            rows: self.rows_for(&[]), ..Default::default()
        };
        let Some(res) = edit_cmd::run(&req) else {
            self.tip = format!("未知命令: {cmd}");
            return None;
        };
        if let Some(t) = &res.tip {
            self.tip = t.clone();
        }
        if res.changed && res.patches.is_none() {
            let (start, remove) = (res.start, res.remove);
            let removed: Vec<String> = self.lines.iter().skip(start).take(remove).cloned().collect();
            let before = (self.cursor, self.sel);
            self.splice(start, remove, res.insert.clone());
            self.push_undo(Undo { start, removed, inserted: res.insert.clone(), before, after: res.cursor });
            self.dirty = true;
            self.redo.clear();
        }
        self.cursor = res.cursor;
        self.sel = res.sel;
        self.anchor = res.anchor;
        if res.changed || res.sel.is_some() {
            self.goal_col = None;
        }
        self.clamp();
        self.reveal = true;
        res.clip
    }

    /// 连续单字符键入合并成一步撤销 (同一行、紧挨着)
    fn push_undo(&mut self, u: Undo) {
        if let Some(p) = self.undo.last_mut() {
            if p.start == u.start && p.inserted.len() == 1 && u.removed.len() == 1 && u.inserted.len() == 1 && p.after == u.before.0 && u.before.1.is_none()
                && u.inserted[0].chars().count() == p.inserted[0].chars().count() + 1
            {
                p.inserted = u.inserted;
                p.after = u.after;
                return;
            }
        }
        self.undo.push(u);
        if self.undo.len() > 5000 {
            self.undo.remove(0);
        }
    }

    pub fn type_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.contains('\n') || text.contains('\r') {
            let t = text.replace("\r\n", "\n").replace('\r', "\n");
            self.edit("paste", Some(t));
        } else {
            self.edit("insert-text", Some(text.to_string()));
        }
    }

    /// 移动键 (← → ↑ ↓ Home End PageUp PageDown, 含 Ctrl / Shift)
    pub fn nav(&mut self, key: &str, ctrl: bool, shift: bool, page_rows: usize) {
        let anchor = if shift { self.anchor.or(Some(self.cursor)).filter(|_| true) } else { None };
        let anchor = match (shift, self.sel, self.anchor) {
            (true, Some(_), Some(a)) => Some(a),
            (true, _, _) => anchor,
            _ => None,
        };
        let req = NavReq { key: key.into(), ctrl, shift, row: self.cursor[0], col: self.cursor[1], goal_col: self.goal_col, sel: self.sel, anchor, total_rows: self.lines.len(), rows: self.rows_for(&[]), page_rows };
        if let Some(r) = nav::step(&req) {
            self.cursor = [r.row, r.col];
            self.goal_col = r.goal_col;
            self.sel = r.sel;
            self.anchor = r.anchor;
            self.clamp();
            self.reveal = true;
        }
    }

    pub fn select_all(&mut self) {
        let last = self.lines.len() - 1;
        self.sel = Some([0, 0, last, self.row_len(last)]);
        self.anchor = Some([0, 0]);
        self.cursor = [last, self.row_len(last)];
    }

    pub fn click(&mut self, row: usize, col: usize, extend: bool) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        if extend {
            let a = self.anchor.unwrap_or(self.cursor);
            self.anchor = Some(a);
            self.set_sel(a, [row, col]);
        } else {
            self.anchor = Some([row, col]);
            self.sel = None;
        }
        self.cursor = [row, col];
        self.goal_col = None;
    }

    pub fn drag_to(&mut self, row: usize, col: usize) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        let a = self.anchor.unwrap_or(self.cursor);
        self.set_sel(a, [row, col]);
        self.cursor = [row, col];
    }

    fn set_sel(&mut self, a: [usize; 2], b: [usize; 2]) {
        self.sel = if a == b { None } else if (a[0], a[1]) <= (b[0], b[1]) { Some([a[0], a[1], b[0], b[1]]) } else { Some([b[0], b[1], a[0], a[1]]) };
    }

    pub fn undo(&mut self) {
        let Some(u) = self.undo.pop() else {
            self.tip = "没有可撤销的操作".into();
            return;
        };
        self.splice(u.start, u.inserted.len(), u.removed.clone());
        self.cursor = u.before.0;
        self.sel = u.before.1;
        self.anchor = None;
        self.redo.push(u);
        self.dirty = true;
        self.clamp();
        self.reveal = true;
    }

    pub fn redo(&mut self) {
        let Some(u) = self.redo.pop() else {
            self.tip = "没有可重做的操作".into();
            return;
        };
        self.splice(u.start, u.removed.len(), u.inserted.clone());
        self.cursor = u.after;
        self.sel = None;
        self.anchor = None;
        self.undo.push(u);
        self.dirty = true;
        self.clamp();
        self.reveal = true;
    }

    // ---------------- 查找 ----------------
    fn search_req(&self, op: &str, needle: &str, case: bool, regex: bool) -> SearchReq {
        SearchReq { op: op.into(), needle: needle.into(), match_case: case, use_regex: regex, lines: self.lines.clone(), ..Default::default() }
    }

    /// 从光标处找下一处 / 上一处; 找到就选中它
    pub fn find_step(&mut self, needle: &str, case: bool, regex: bool, forward: bool) -> Result<bool, String> {
        if needle.is_empty() {
            return Ok(false);
        }
        let mut req = self.search_req("step", needle, case, regex);
        req.row = self.cursor[0];
        req.col = self.cursor[1];
        req.forward = forward;
        req.wrap = true;
        req.sel_start = self.sel.map(|s| [s[0], s[1]]);
        let Some(res) = search_ops::run(&req) else { return Ok(false) };
        if let Some(e) = res.error {
            return Err(e);
        }
        match res.hit {
            Some(h) => {
                self.anchor = Some([h.row, h.col]);
                self.sel = Some([h.row, h.col, h.row, h.col + h.len]);
                self.cursor = [h.row, h.col + h.len];
                self.reveal = true;
                self.tip = format!("{}/{}{}", res.index + 1, res.total, if res.wrapped { " (已折返)" } else { "" });
                Ok(true)
            }
            None => {
                self.tip = "没有找到".into();
                Ok(false)
            }
        }
    }

    pub fn replace_all(&mut self, needle: &str, repl: &str, case: bool, regex: bool) -> Result<usize, String> {
        let mut req = self.search_req("replace_all", needle, case, regex);
        req.repl = repl.into();
        let Some(res) = search_ops::run(&req) else { return Ok(0) };
        if let Some(e) = res.error {
            return Err(e);
        }
        if res.count > 0 {
            // 整篇换成一步撤销: 记成"整份文档替换"
            let old = self.lines.clone();
            for (r, t) in res.rows {
                if r < self.lines.len() {
                    self.lines[r] = t;
                }
            }
            let before = (self.cursor, self.sel);
            self.undo.push(Undo { start: 0, removed: old, inserted: self.lines.clone(), before, after: self.cursor });
            self.redo.clear();
            self.dirty = true;
        }
        Ok(res.count)
    }
}

/// 把一行里的 Tab 展开成空格 (到下一个 Tab 位), 并给出 "字符下标 -> 显示列" 的映射 (长度 = 字符数 + 1)
pub fn expand_tabs(line: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(line.len());
    let mut map = Vec::with_capacity(line.len() + 1);
    let mut col = 0usize;
    for ch in line.chars() {
        map.push(col);
        if ch == '\t' {
            let n = TAB - (col % TAB);
            out.extend(std::iter::repeat(' ').take(n));
            col += n;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    map.push(col);
    (out, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(lines: &[&str]) -> Doc {
        let mut d = Doc::untitled(1, 1);
        d.lines = lines.iter().map(|s| s.to_string()).collect();
        d
    }

    #[test]
    fn typing_enter_backspace_go_through_editcore() {
        let mut d = doc(&[""]);
        d.type_text("a");
        d.type_text("b");
        assert_eq!(d.lines, vec!["ab"]);
        d.edit("new-line", None);
        assert_eq!((d.lines.clone(), d.cursor), (vec!["ab".to_string(), "".to_string()], [1, 0]));
        d.edit("backspace", None);
        assert_eq!((d.lines.clone(), d.cursor), (vec!["ab".to_string()], [0, 2]));
    }

    #[test]
    fn consecutive_typing_is_one_undo_step() {
        let mut d = doc(&[""]);
        for c in ["h", "e", "l", "l", "o"] {
            d.type_text(c);
        }
        assert_eq!(d.lines[0], "hello");
        d.undo();
        assert_eq!(d.lines[0], "", "整段键入一步撤销");
        d.redo();
        assert_eq!(d.lines[0], "hello");
    }

    #[test]
    fn nav_keys_use_sakura_semantics_and_selection() {
        let mut d = doc(&["foo bar", "x"]);
        d.nav("ArrowRight", true, false, 10);              // Ctrl+→ = 下一个词
        assert_eq!(d.cursor, [0, 4]);
        d.nav("End", false, true, 10);                      // Shift+End 选到行尾
        assert_eq!(d.sel, Some([0, 4, 0, 7]));
        d.nav("ArrowDown", false, false, 10);
        assert_eq!(d.sel, None);
    }

    #[test]
    fn find_and_replace_all() {
        let mut d = doc(&["foo Foo", "foo"]);
        assert!(d.find_step("foo", false, false, true).unwrap());
        assert_eq!(d.sel, Some([0, 0, 0, 3]));
        assert!(d.find_step("foo", false, false, true).unwrap());
        assert_eq!(d.sel, Some([0, 4, 0, 7]));
        assert_eq!(d.replace_all("foo", "X", false, false).unwrap(), 3);
        assert_eq!(d.lines, vec!["X X", "X"]);
        d.undo();
        assert_eq!(d.lines, vec!["foo Foo", "foo"]);
    }

    #[test]
    fn tabs_expand_to_tab_stops() {
        let (s, m) = expand_tabs("a\tb");
        assert_eq!(s, "a   b");
        assert_eq!(m, vec![0, 1, 4, 5]);
    }
}
