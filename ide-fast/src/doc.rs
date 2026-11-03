//! 文档模型: 行向量 + 光标 / 选区 + 撤销栈, 编辑和移动全部交给 `editcore` (和 Tauri 版前端走同一套 Rust 语义)。
//!
//! editcore 的编辑 / 移动 / 查找是**无状态**的: 调用方交上光标周围需要的几行, 它返回一个行补丁 + 新光标 / 选区。
//! 这里就是那个"调用方": 组请求 -> 调 `editcore` -> 套补丁 -> 记撤销。所以 Sakura 的键位语义 (词移动、行首智能 Home、
//! 自动缩进、整行剪切……) 不用在 GUI 里重写第二份。

use editcore::bookmark::{self, Bm, BmReq};
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
    /// 文本版本号: 每次改行内容 +1 (大纲等缓存据此判断要不要重算)
    pub rev: u64,
    /// 书签 (按行; 行增删时跟着移动)
    pub bookmarks: Vec<Bm>,
    /// 矩形选区 [r0, c0, r1, c1] (未归一化: 锚点在前, 光标在后)
    pub box_sel: Option<[usize; 4]>,
    /// 额外的光标 (多光标; 主光标是 `cursor`)
    pub extra: Vec<[usize; 2]>,
    /// 键盘宏录制中 / 上一次录下的宏
    pub rec: Option<Vec<RecEv>>,
    pub last_macro: Vec<RecEv>,
    /// 已折叠的范围 (起始行, 结束行含): 起始行还显示, 后面的行隐藏
    pub folds: Vec<(usize, usize)>,
    /// 标签分组 (0 = 不分组, 1..=5 = 红 橙 绿 蓝 紫)
    pub group: u8,
    /// 改写模式 (Ins) / 只读
    pub overwrite: bool,
    pub readonly: bool,
}

/// 录下来的一步 (回放时原样再走一遍 editcore)
#[derive(Clone, Debug, PartialEq)]
pub enum RecEv {
    Edit(String, Option<String>),
    Nav(String, bool, bool),
}

impl Doc {
    pub fn untitled(id: u64, n: usize) -> Self {
        Doc { id, path: None, name: if n <= 1 { "无标题".into() } else { format!("无标题 {n}") }, lines: vec![String::new()], cursor: [0, 0], sel: None, anchor: None, goal_col: None, dirty: false,
              charset: Charset::Utf8, bom: Bom::None, eol: Eol::Crlf, undo: vec![], redo: vec![], tip: String::new(), reveal: false, rev: 0, bookmarks: vec![], box_sel: None, extra: vec![], rec: None, last_macro: vec![], folds: vec![], group: 0, overwrite: false, readonly: false }
    }

    pub fn open(id: u64, path: &Path) -> Result<Self, String> {
        let d = document_io::read_doc(path, None)?;
        let mut lines: Vec<String> = d.content.split('\n').map(str::to_string).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "?".into());
        Ok(Doc { id, path: Some(path.to_path_buf()), name, lines, cursor: [0, 0], sel: None, anchor: None, goal_col: None, dirty: false, charset: d.charset, bom: d.bom, eol: d.eol,
                 undo: vec![], redo: vec![], tip: format!("{} · {} · {}", d.charset.label(), d.eol.label(), if d.lossy { "有无法解码的字节" } else { "已读取" }), reveal: true, rev: 0, bookmarks: vec![], box_sel: None, extra: vec![], rec: None, last_macro: vec![], folds: vec![], group: 0, overwrite: false, readonly: false })
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

    /// 光标跳到 (行, 列) 并滚到可见处 (大纲 / Grep 结果 / 书签用)
    pub fn goto(&mut self, row: usize, col: usize) {
        self.cursor = [row.min(self.lines.len() - 1), col];
        self.clamp();
        self.sel = None;
        self.anchor = None;
        self.goal_col = None;
        self.reveal = true;
    }

    fn shift_bookmarks(&mut self, start: usize, remove: usize, insert: usize) {
        self.bookmarks.retain_mut(|b| {
            if b.row >= start + remove {
                b.row = b.row + insert - remove;
                true
            } else {
                b.row < start + insert || b.row < start // 被替换范围里的: 还在新范围内就保留
            }
        });
    }

    /// 缩进折叠: 从 `row` 往下, 缩进比它深的连续行 (中间空行算在内, 结尾空行不算) 是它的折叠范围
    pub fn fold_range_at(&self, row: usize) -> Option<(usize, usize)> {
        let indent = |l: &str| -> Option<usize> {
            if l.trim().is_empty() { return None; }
            Some(l.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum())
        };
        let base = indent(self.lines.get(row)?)?;
        let mut end = row;
        for r in row + 1..self.lines.len() {
            match indent(&self.lines[r]) {
                None => continue,
                Some(i) if i > base => end = r,
                Some(_) => break,
            }
        }
        (end > row).then_some((row, end))
    }

    pub fn toggle_fold(&mut self, row: usize) {
        if let Some(i) = self.folds.iter().position(|f| f.0 == row) {
            self.folds.remove(i);
        } else if let Some(r) = self.fold_range_at(row) {
            self.folds.push(r);
            // 光标落在被折起来的行里: 移回折叠起始行
            if self.cursor[0] > r.0 && self.cursor[0] <= r.1 {
                self.goto(r.0, 0);
            }
        }
    }

    pub fn is_hidden(&self, row: usize) -> bool {
        self.folds.iter().any(|&(s, e)| row > s && row <= e)
    }

    fn splice(&mut self, start: usize, remove: usize, insert: Vec<String>) {
        if remove != insert.len() {
            self.folds.clear(); // 行数变了: 折叠范围作废 (改行内容不影响)
        }
        self.rev += 1;
        self.shift_bookmarks(start, remove, insert.len());
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
        let r = self.cursor[0];
        self.folds.retain(|&(s, e)| !(r > s && r <= e)); // 光标移进折叠范围就展开它
    }

    /// 执行一条 editcore 编辑命令; 返回要写进剪贴板的文字 (复制 / 剪切类)
    pub fn edit(&mut self, cmd: &str, text: Option<String>) -> Option<String> {
        if self.readonly && !matches!(cmd, "copy" | "copy-lines" | "copy-crlf" | "copy-linenum" | "copy-passage" | "select-word") {
            self.tip = "只读: 不能修改 (编辑 → 只读 可以关掉)".into();
            return None;
        }
        if let Some(r) = self.rec.as_mut() {
            r.push(RecEv::Edit(cmd.to_string(), text.clone()));
        }
        // 矩形选区 / 多光标: 键入、退格、删除、回车、复制、剪切走各自的命令; 别的命令先退出这两种模式
        let mut cmd = cmd.to_string();
        let mut text = text;
        if self.box_sel.is_some() {
            cmd = match cmd.as_str() {
                "insert-text" => "box-insert".into(),
                "paste" => { text = text.map(|t| t.lines().next().unwrap_or("").to_string()); "box-insert".into() }
                "backspace" => "box-backspace".into(),
                "delete" => "box-delete".into(),
                "new-line" => "box-enter".into(),
                "copy" => "box-copy".into(),
                "cut" => "box-cut".into(),
                c if c.starts_with("box-") => c.to_string(),
                other => { self.box_sel = None; other.to_string() }
            };
        } else if !self.extra.is_empty() {
            cmd = match cmd.as_str() {
                "insert-text" => "multi-insert".into(),
                "paste" => "multi-insert".into(),
                "backspace" => "multi-backspace".into(),
                c if c.starts_with("multi-") => c.to_string(),
                other => { self.extra.clear(); other.to_string() }
            };
        }
        let multi = cmd.starts_with("multi-");
        let mut all_cursors: Vec<[usize; 2]> = vec![];
        let mut extra_rows: Vec<usize> = vec![];
        if multi {
            all_cursors.push(self.cursor);
            all_cursors.extend(self.extra.iter().copied());
            all_cursors.sort();
            extra_rows = all_cursors.iter().flat_map(|c| [c[0].saturating_sub(1), c[0], c[0] + 1]).collect();
        }
        if let Some(b) = self.box_sel {
            extra_rows.extend(b[0].min(b[2])..=b[0].max(b[2]));
        }
        let req = EditReq {
            cmd: cmd.clone(), row: self.cursor[0], col: self.cursor[1], sel: self.sel, total_rows: self.lines.len(), auto_indent: true, smart_indent: false, overwrite: self.overwrite, text,
            box_sel: self.box_sel, cursors: all_cursors, rows: self.rows_for(&extra_rows), ..Default::default()
        };
        let Some(res) = edit_cmd::run(&req) else {
            self.tip = format!("未知命令: {cmd}");
            return None;
        };
        if let Some(t) = &res.tip {
            self.tip = t.clone();
        }
        if let Some(patches) = res.patches.clone() {
            // 多光标: 几段补丁 (起始行是改动前的行号), 从下往上套; 整体记成一步撤销
            let old = self.lines.clone();
            let before = (self.cursor, self.sel);
            let mut ps = patches;
            ps.sort_by(|a, b| b.0.cmp(&a.0));
            for (s, n, ins) in ps {
                self.splice(s, n, ins);
            }
            self.undo.push(Undo { start: 0, removed: old, inserted: self.lines.clone(), before, after: res.cursor });
            self.redo.clear();
            self.dirty = true;
        }
        if let Some(cs) = &res.cursors {
            self.extra = cs.iter().skip(1).copied().collect();
        }
        if cmd.starts_with("box-") {
            self.box_sel = res.box_sel;
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

    /// 把 [start, start+remove) 行换成 insert, 记一步撤销, 光标放到 cursor
    pub fn replace_rows(&mut self, start: usize, remove: usize, insert: Vec<String>, cursor: [usize; 2]) {
        let removed: Vec<String> = self.lines.iter().skip(start).take(remove).cloned().collect();
        if removed == insert {
            return;
        }
        let before = (self.cursor, self.sel);
        self.splice(start, remove, insert.clone());
        self.undo.push(Undo { start, removed, inserted: insert, before, after: cursor });
        self.redo.clear();
        self.dirty = true;
        self.cursor = cursor;
        self.sel = None;
        self.anchor = None;
        self.clamp();
        self.reveal = true;
    }

    /// 选区覆盖的整行 (含首尾); 选区终点在行首且跨行时不算那一行。没有选区 = None
    pub fn selected_rows(&self) -> Option<(usize, usize)> {
        let s = self.sel?;
        Some((s[0], if s[3] == 0 && s[2] > s[0] { s[2] - 1 } else { s[2] }))
    }

    /// 整行变换 (排序 / 去重 / 去空白…): 作用在选中的行, 没有选区 = 全文
    pub fn lines_op(&mut self, kind: &str) {
        let (r0, r1) = self.selected_rows().unwrap_or((0, self.lines.len().saturating_sub(1)));
        let r1 = r1.min(self.lines.len().saturating_sub(1));
        let new = crate::cmds::lines_op(kind, self.lines[r0..=r1].to_vec());
        let n = new.len();
        self.replace_rows(r0, r1 - r0 + 1, if n == 0 { vec![String::new()] } else { new }, [r0, 0]);
        self.tip = format!("已处理 {} 行", r1 - r0 + 1);
    }

    /// 选区文字变换 (大小写 / 全半角 / Tab 与空格); 没有选区什么也不做
    pub fn transform_sel(&mut self, kind: &str, tab: usize) {
        if self.sel.is_none() {
            self.tip = "先选中要转换的文字".into();
            return;
        }
        let (cursor, sel, anchor) = (self.cursor, self.sel, self.anchor);
        let Some(text) = self.edit("copy", None) else { return };
        // copy 会把选区清掉; 放回去, paste 才是「替换选区」
        self.cursor = cursor;
        self.sel = sel;
        self.anchor = anchor;
        self.edit("paste", Some(crate::cmds::transform(kind, &text, tab)));
    }

    /// 缩进 / 反缩进: 多行选区时对每个非空行加 / 减一级; 单行且不是选区时 Tab 照常插入制表符
    pub fn indent(&mut self, outdent: bool, tab: usize) {
        let multi = self.selected_rows().map(|(a, b)| b > a).unwrap_or(false);
        if !outdent && !multi {
            self.type_text("\t");
            return;
        }
        let (r0, r1) = self.selected_rows().unwrap_or((self.cursor[0], self.cursor[0]));
        let r1 = r1.min(self.lines.len() - 1);
        let new: Vec<String> = self.lines[r0..=r1]
            .iter()
            .map(|l| {
                if !outdent {
                    if l.is_empty() { String::new() } else { format!("\t{l}") }
                } else if let Some(rest) = l.strip_prefix('\t') {
                    rest.to_string()
                } else {
                    let n = l.chars().take(tab.max(1)).take_while(|c| *c == ' ').count();
                    l.chars().skip(n).collect()
                }
            })
            .collect();
        let last_len = new.last().map(|l| l.chars().count()).unwrap_or(0);
        let keep_sel = multi;
        self.replace_rows(r0, r1 - r0 + 1, new, [r1, last_len]);
        if keep_sel {
            self.sel = Some([r0, 0, r1, last_len]);
            self.anchor = Some([r0, 0]);
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
        if let Some(r) = self.rec.as_mut() {
            r.push(RecEv::Nav(key.to_string(), ctrl, shift));
        }
        self.box_sel = None;
        self.extra.clear();
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

    /// 命令存在才执行 (宏里用: 不认识的命令要报错而不是悄悄跳过)
    pub fn try_edit(&mut self, cmd: &str) -> bool {
        let known = edit_cmd::run(&EditReq { cmd: cmd.into(), row: 0, col: 0, total_rows: 1, rows: vec![(0, String::new())], ..Default::default() }).is_some();
        if known {
            self.edit(cmd, None);
        }
        known
    }

    /// 宏的「移动光标」: select 为真时从当前位置 (或锚点) 选到那里
    pub fn move_to(&mut self, row: usize, col: usize, select: bool) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        if select {
            let a = self.anchor.unwrap_or(self.cursor);
            self.anchor = Some(a);
            self.set_sel(a, [row, col]);
        } else {
            self.sel = None;
            self.anchor = None;
        }
        self.cursor = [row, col];
        self.reveal = true;
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

    pub fn box_start(&mut self, row: usize, col: usize) {
        let row = row.min(self.lines.len() - 1);
        self.sel = None;
        self.extra.clear();
        self.box_sel = Some([row, col, row, col]);
        self.cursor = [row, col];
    }

    pub fn box_drag(&mut self, row: usize, col: usize) {
        if let Some(b) = self.box_sel.as_mut() {
            b[2] = row.min(self.lines.len() - 1);
            b[3] = col;
            self.cursor = [b[2], col.min(self.lines.get(b[2]).map(|l| l.chars().count()).unwrap_or(0))];
        }
    }

    /// Ctrl+点击: 加 / 去一个光标
    pub fn toggle_cursor(&mut self, row: usize, col: usize) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        self.box_sel = None;
        self.sel = None;
        if let Some(i) = self.extra.iter().position(|c| *c == [row, col]) {
            self.extra.remove(i);
        } else if [row, col] != self.cursor {
            self.extra.push([row, col]);
        }
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
        self.box_sel = None;
        self.extra.clear();
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
        self.box_sel = None;
        self.extra.clear();
        self.splice(u.start, u.removed.len(), u.inserted.clone());
        self.cursor = u.after;
        self.sel = None;
        self.anchor = None;
        self.undo.push(u);
        self.dirty = true;
        self.clamp();
        self.reveal = true;
    }

    // ---------------- 书签 (editcore::bookmark) ----------------
    pub fn bookmark(&mut self, op: &str) {
        let req = BmReq { op: op.into(), list: self.bookmarks.clone(), row: self.cursor[0], col: 0 };
        let Some(res) = bookmark::run(&req) else { return };
        if let Some(l) = res.list {
            self.bookmarks = l;
            self.tip = if res.added { "已设置书签".into() } else { "已取消书签".into() };
        }
        if let Some(t) = res.target {
            self.goto(t.row, 0);
            self.tip = format!("书签 {}/{}", res.index + 1, res.count);
        } else if op != "toggle" {
            self.tip = "没有书签".into();
        }
    }

    // ---------------- 键盘宏 (录制 / 回放: 把录下的编辑和移动原样再走一遍 editcore) ----------------
    pub fn macro_toggle(&mut self) {
        match self.rec.take() {
            Some(r) => {
                self.tip = format!("宏录制结束 ({} 步) — Ctrl+Shift+L 回放", r.len());
                self.last_macro = r;
            }
            None => {
                self.rec = Some(vec![]);
                self.tip = "● 录制宏… 再按 Ctrl+Shift+M 结束".into();
            }
        }
    }

    pub fn macro_play(&mut self) {
        let m = self.last_macro.clone();
        if m.is_empty() {
            self.tip = "还没有录下的宏".into();
            return;
        }
        for ev in m {
            match ev {
                RecEv::Edit(c, t) => { self.edit(&c, t); }
                RecEv::Nav(k, c, s) => self.nav(&k, c, s, 20),
            }
        }
        self.tip = "宏已回放".into();
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
            self.rev += 1;
            self.undo.push(Undo { start: 0, removed: old, inserted: self.lines.clone(), before, after: self.cursor });
            self.redo.clear();
            self.dirty = true;
        }
        Ok(res.count)
    }
}

/// 把一行里的 Tab 展开成空格 (到下一个 Tab 位), 并给出 "字符下标 -> 显示列" 的映射 (长度 = 字符数 + 1)
pub fn expand_tabs(line: &str, tab: usize) -> (String, Vec<usize>) {
    let tab = tab.max(1);
    let mut out = String::with_capacity(line.len());
    let mut map = Vec::with_capacity(line.len() + 1);
    let mut col = 0usize;
    for ch in line.chars() {
        map.push(col);
        if ch == '\t' {
            let n = tab - (col % tab);
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
        let (s, m) = expand_tabs("a\tb", 4);
        assert_eq!(s, "a   b");
        assert_eq!(m, vec![0, 1, 4, 5]);
    }

    #[test]
    fn bookmarks_toggle_navigate_and_follow_line_edits() {
        let mut d = doc(&["a", "b", "c", "d"]);
        d.goto(1, 0);
        d.bookmark("toggle");
        d.goto(3, 0);
        d.bookmark("toggle");
        d.goto(0, 0);
        d.bookmark("next");
        assert_eq!(d.cursor[0], 1);
        d.bookmark("next");
        assert_eq!(d.cursor[0], 3);
        d.goto(0, 0);
        d.edit("add-line-above", None); // 上面多一行: 书签整体下移
        assert_eq!(d.bookmarks.iter().map(|b| b.row).collect::<Vec<_>>(), vec![2, 4]);
    }

    #[test]
    fn box_selection_types_into_every_row() {
        let mut d = doc(&["abcd", "efgh", "ijkl"]);
        d.box_start(0, 1);
        d.box_drag(2, 1);
        d.type_text("X");
        assert_eq!(d.lines, vec!["aXbcd", "eXfgh", "iXjkl"]);
        d.undo();
        assert_eq!(d.lines, vec!["abcd", "efgh", "ijkl"]);
    }

    #[test]
    fn multi_cursor_inserts_everywhere_in_one_undo_step() {
        let mut d = doc(&["ab", "cd"]);
        d.goto(0, 0);
        d.toggle_cursor(1, 0);
        d.type_text("-");
        assert_eq!(d.lines, vec!["-ab", "-cd"]);
        assert_eq!(d.extra.len(), 1);
        d.edit("backspace", None);
        assert_eq!(d.lines, vec!["ab", "cd"]);
        d.undo();
        d.undo();
        assert_eq!(d.lines, vec!["ab", "cd"]);
    }

    #[test]
    fn keyboard_macro_records_and_replays() {
        let mut d = doc(&["x", "y"]);
        d.macro_toggle();
        d.nav("End", false, false, 10);
        d.type_text("!");
        d.nav("ArrowDown", false, false, 10);
        d.macro_toggle();
        assert_eq!(d.lines, vec!["x!", "y"]);
        d.goto(0, 0);
        d.macro_play();
        assert_eq!(d.lines[0], "x!!");
        d.macro_play();
        assert_eq!(d.lines[1], "y!", "第二次回放是在第 2 行接着做");
    }

    #[test]
    fn indentation_folds_hide_rows_and_unfold_when_the_cursor_enters() {
        let mut d = doc(&["fn a() {", "    x", "", "    y", "}", "fn b() {", "    z", "}"]);
        assert_eq!(d.fold_range_at(0), Some((0, 3)));
        assert_eq!(d.fold_range_at(1), None);
        d.toggle_fold(0);
        assert!(d.is_hidden(1) && d.is_hidden(3) && !d.is_hidden(4) && !d.is_hidden(0));
        d.goto(2, 0); // 光标进折叠范围 -> 展开
        assert!(d.folds.is_empty());
        d.toggle_fold(5);
        d.edit("add-line-below", None); // 行数变了: 折叠清掉
        assert!(d.folds.is_empty());
    }

    #[test]
    fn line_ops_indent_readonly_and_overwrite() {
        let mut d = Doc::untitled(1, 1);
        d.type_text("b
a
b
");
        d.select_all();
        d.lines_op("sort-asc");
        assert_eq!(d.lines, vec!["a", "b", "b", ""], "选区止于行首的那一行不算");
        d.select_all();
        d.lines_op("unique");
        assert_eq!(d.lines, vec!["a", "b", ""]);
        d.undo();
        assert_eq!(d.lines, vec!["a", "b", "b", ""], "整行变换是一步撤销");
        // 多行选区 Tab 缩进 / Shift+Tab 反缩进
        d.select_all();
        d.indent(false, 4);
        assert_eq!(d.lines, vec!["	a", "	b", "	b", ""]);
        d.indent(true, 4);
        assert_eq!(d.lines, vec!["a", "b", "b", ""]);
        // 选区大小写
        d.goto(0, 0);
        d.nav("ArrowRight", false, true, 10);
        d.transform_sel("upper", 4);
        assert_eq!(d.lines[0], "A");
        // 只读: 改不了
        d.readonly = true;
        let before = d.lines.clone();
        d.type_text("zzz");
        assert_eq!(d.lines, before);
        d.readonly = false;
        // 改写: 吃掉光标后的字符
        d.goto(1, 0);
        d.overwrite = true;
        d.type_text("X");
        assert_eq!(d.lines[1], "X");
    }
}
