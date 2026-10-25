//! 删除 / 剪切 / 复制行 类编辑命令 (ARCH-01: 编辑语义放 Rust).
//!
//! 无状态: 前端交上光标周围需要的几行 (`rows`) 和选区, 这里算出"把第 `start` 行起的 `remove` 行换成 `insert`"
//! 以及新的光标 / 要写进剪贴板的文字。语义按真实 Sakura Editor 的键盘宏逐条比对 (tests/sakura-diff)。

use crate::nav::{first_word_col, is_word_char, word_left, word_right};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct EditReq {
    pub cmd: String,
    pub row: usize,
    pub col: usize,
    /// 选区 [r0, c0, r1, c1] (前端已把"到行尾"解析成具体列)
    #[serde(default)]
    pub sel: Option<[usize; 4]>,
    pub total_rows: usize,
    /// Enter 自动缩进 / 智能缩进 (new-line 用)
    #[serde(default)]
    pub auto_indent: bool,
    #[serde(default)]
    pub smart_indent: bool,
    /// 粘贴 / 矩形贴付 的文字
    #[serde(default)]
    pub text: Option<String>,
    /// 剪贴板内容来自「无选区的复制 / 剪切」(整行模式)
    #[serde(default)]
    pub line_mode: bool,
    /// 需要用到的行: [行号, 文本] (光标行 ±1, 选区各行)
    pub rows: Vec<(usize, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct EditRes {
    /// false = 什么也没改 (前端只给出提示)
    pub changed: bool,
    /// 把 [start, start+remove) 行换成 insert
    pub start: usize,
    pub remove: usize,
    pub insert: Vec<String>,
    pub cursor: [usize; 2],
    /// 要写进剪贴板的文字 (剪切类命令)
    pub clip: Option<String>,
    /// 剪贴板内容是整行模式 (再粘贴时插在当前行之前)
    pub clip_line: bool,
    /// 命令产生的选区 [r0, c0, r1, c1] 与锚点 (选词等不改文本的命令用)
    pub sel: Option<[usize; 4]>,
    pub anchor: Option<[usize; 2]>,
    /// 提示文字
    pub tip: Option<String>,
    /// 撤销后光标回到哪里 (缺省 = 命令发出时的光标); 词删除是先选中词再删, 撤销回到词尾
    pub undo_cursor: Option<[usize; 2]>,
}

fn chars_of(s: &str) -> Vec<char> {
    s.chars().collect()
}
fn slice(cs: &[char], a: usize, b: usize) -> String {
    cs[a.min(cs.len())..b.min(cs.len())].iter().collect()
}

struct Ctx<'a> {
    req: &'a EditReq,
}
impl Ctx<'_> {
    fn line(&self, r: usize) -> Vec<char> {
        self.req.rows.iter().find(|(i, _)| *i == r).map(|(_, s)| chars_of(s)).unwrap_or_default()
    }
    fn sel_nonempty(&self) -> Option<[usize; 4]> {
        // 起点在前 (请求里的选区可能是反向的; 反向时 r1 - r0 会下溢)
        self.req.sel.map(|s| if (s[2], s[3]) < (s[0], s[1]) { [s[2], s[3], s[0], s[1]] } else { s }).filter(|s| !(s[0] == s[2] && s[1] == s[3]))
    }
    /// 选区文字 (行间用 \n)
    fn sel_text(&self, s: [usize; 4]) -> String {
        if s[0] == s[2] {
            return slice(&self.line(s[0]), s[1], s[3]);
        }
        let mut out = vec![slice(&self.line(s[0]), s[1], usize::MAX)];
        for r in s[0] + 1..s[2] {
            out.push(self.line(r).iter().collect());
        }
        out.push(slice(&self.line(s[2]), 0, s[3]));
        out.join("\n")
    }
    /// 删除选区
    fn delete_sel(&self, s: [usize; 4], clip: bool) -> EditRes {
        let first = self.line(s[0]);
        let last = self.line(s[2]);
        let head = slice(&first, 0, s[1]);
        let tail = slice(&last, s[3], usize::MAX);
        EditRes {
            changed: true,
            start: s[0],
            remove: s[2] - s[0] + 1,
            insert: vec![format!("{head}{tail}")],
            cursor: [s[0], s[1].min(first.len())],
            clip: if clip { Some(self.sel_text(s)) } else { None },
            tip: None,
            undo_cursor: None,
            clip_line: false,
            sel: None,
            anchor: None,
        }
    }
    fn replace_line(&self, r: usize, new: String, cursor: [usize; 2], clip: Option<String>) -> EditRes {
        EditRes { changed: true, start: r, remove: 1, insert: vec![new], cursor, clip, tip: None, undo_cursor: None, clip_line: false, sel: None, anchor: None }
    }
    fn join_next(&self, r: usize, skip_leading: bool) -> EditRes {
        let cur = self.line(r);
        let next = self.line(r + 1);
        let skip = if skip_leading { first_word_col(&next) } else { 0 };
        let joined = format!("{}{}", cur.iter().collect::<String>(), slice(&next, skip, usize::MAX));
        EditRes { changed: true, start: r, remove: 2, insert: vec![joined], cursor: [r, cur.len()], clip: None, tip: None, undo_cursor: None, clip_line: false, sel: None, anchor: None }
    }
    fn nothing(&self, tip: &str) -> EditRes {
        EditRes { changed: false, tip: Some(tip.to_string()), cursor: [self.req.row, self.req.col], ..Default::default() }
    }
}

/// 执行一条编辑命令. 未知命令返回 None.
pub fn run(req: &EditReq) -> Option<EditRes> {
    let cx = Ctx { req };
    let (row, last_row) = (req.row, req.total_rows.saturating_sub(1));
    let line = cx.line(row);
    let col = req.col.min(line.len());
    let sel = cx.sel_nonempty();
    Some(match req.cmd.as_str() {
        // ---- 单字符 ----
        "backspace" => {
            if let Some(s) = sel {
                cx.delete_sel(s, false)
            } else if col > 0 {
                let new = format!("{}{}", slice(&line, 0, col - 1), slice(&line, col, usize::MAX));
                cx.replace_line(row, new, [row, col - 1], None)
            } else if row > 0 {
                let prev = cx.line(row - 1);
                let joined = format!("{}{}", prev.iter().collect::<String>(), line.iter().collect::<String>());
                EditRes { changed: true, start: row - 1, remove: 2, insert: vec![joined], cursor: [row - 1, prev.len()], clip: None, tip: None, undo_cursor: None, clip_line: false, sel: None, anchor: None }
            } else {
                cx.nothing("")
            }
        }
        "delete" => {
            if let Some(s) = sel {
                cx.delete_sel(s, false)
            } else if col < line.len() {
                let new = format!("{}{}", slice(&line, 0, col), slice(&line, col + 1, usize::MAX));
                cx.replace_line(row, new, [row, col], None)
            } else if row < last_row {
                let mut r = cx.join_next(row, false);
                r.cursor = [row, col];
                r
            } else {
                cx.nothing("")
            }
        }
        // ---- 词 ----
        "delete-word-start" => {
            if col == 0 {
                if row == 0 {
                    return Some(cx.nothing("光标已在词首, 无可删内容"));
                }
                let prev = cx.line(row - 1);
                let joined = format!("{}{}", prev.iter().collect::<String>(), line.iter().collect::<String>());
                EditRes { changed: true, start: row - 1, remove: 2, insert: vec![joined], cursor: [row - 1, prev.len()], clip: None, tip: None, undo_cursor: None, clip_line: false, sel: None, anchor: None }
            } else {
                let to = word_left(&line, col);
                if to >= col {
                    return Some(cx.nothing(""));
                }
                cx.replace_line(row, format!("{}{}", slice(&line, 0, to), slice(&line, col, usize::MAX)), [row, to], None)
            }
        }
        "delete-word-end" => {
            if col >= line.len() {
                if row >= last_row {
                    return Some(cx.nothing("光标已在词尾, 无可删内容"));
                }
                cx.join_next(row, true)
            } else {
                let to = word_right(&line, col);
                if to <= col {
                    return Some(cx.nothing(""));
                }
                cx.replace_line(row, format!("{}{}", slice(&line, 0, col), slice(&line, to, usize::MAX)), [row, col], None)
            }
        }
        "word-cut" | "word-delete" => {
            let cut = req.cmd == "word-cut";
            if col >= line.len() {
                // Sakura: 行末的词删除 = 删掉换行
                return Some(if row < last_row {
                    let mut r = cx.join_next(row, false);
                    r.cursor = [row, col];
                    r
                } else {
                    cx.nothing("")
                });
            }
            // 光标所在字符类 (单词 / 空白 / 其它) 的连续段
            let cls = |c: char| if is_word_char(c) { 1 } else if c.is_whitespace() { 2 } else { 3 };
            let k = cls(line[col]);
            let (mut i, mut j) = (col, col);
            while i > 0 && cls(line[i - 1]) == k {
                i -= 1;
            }
            while j < line.len() && cls(line[j]) == k {
                j += 1;
            }
            let removed = slice(&line, i, j);
            let mut r = cx.replace_line(row, format!("{}{}", slice(&line, 0, i), slice(&line, j, usize::MAX)), [row, i], if cut { Some(removed) } else { None });
            r.undo_cursor = Some([row, j]);
            r
        }
        // ---- 行首 / 行尾 ----
        "cut-line-start" | "delete-line-to-start" => {
            let cut = req.cmd == "cut-line-start";
            if let Some(s) = sel {
                return Some(cx.delete_sel(s, cut));
            }
            if col == 0 {
                return Some(cx.nothing("光标已在行首, 无可删内容"));
            }
            cx.replace_line(row, slice(&line, col, usize::MAX), [row, 0], if cut { Some(slice(&line, 0, col)) } else { None })
        }
        "delete-line-to-end" => {
            if let Some(s) = sel {
                return Some(cx.delete_sel(s, false));
            }
            if col >= line.len() {
                return Some(cx.nothing("光标已在行末, 无可删内容"));
            }
            cx.replace_line(row, slice(&line, 0, col), [row, col], None)
        }
        "line-cut-to-end" => {
            if let Some(s) = sel {
                return Some(cx.delete_sel(s, true));
            }
            if col >= line.len() {
                return Some(cx.nothing("光标已在行末, 无可剪内容")); // Sakura: 行末再按什么也不做 (不吃换行)
            }
            cx.replace_line(row, slice(&line, 0, col), [row, col], Some(slice(&line, col, usize::MAX)))
        }
        // ---- 剪贴板 ----
        "copy" => {
            if let Some(s) = sel {
                let t = cx.sel_text(s);
                let n = t.chars().count();
                EditRes { clip: Some(t), tip: Some(format!("已复制 {n} 字符")), cursor: [row, col], ..Default::default() }
            } else {
                EditRes { clip: Some(format!("{}\n", line.iter().collect::<String>())), clip_line: true, tip: Some("已复制行".into()), cursor: [row, col], ..Default::default() }
            }
        }
        "cut" => {
            if let Some(s) = sel {
                let mut r = cx.delete_sel(s, true);
                r.tip = Some("已剪切".into());
                r
            } else {
                let (insert, new_row) = if req.total_rows <= 1 { (vec![String::new()], 0) } else { (vec![], if row == last_row { row - 1 } else { row }) };
                EditRes { changed: true, start: row, remove: 1, insert, cursor: [new_row, 0], clip: Some(format!("{}\n", line.iter().collect::<String>())), clip_line: true, tip: Some("已剪切行".into()), ..Default::default() }
            }
        }
        "paste" => {
            let text = req.text.clone().unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n");
            if text.is_empty() {
                return Some(cx.nothing("剪贴板为空"));
            }
            if req.line_mode && sel.is_none() && text.ends_with('\n') {
                // 整行模式的剪贴板: 插在当前行之前
                let rows: Vec<String> = text[..text.len() - 1].split('\n').map(String::from).collect();
                let n = rows.len();
                return Some(EditRes { changed: true, start: row, remove: 0, insert: rows, cursor: [row + n, col], tip: Some("已粘贴行".into()), ..Default::default() });
            }
            let (r0, r1, head, tail) = match sel {
                Some(s) => (s[0], s[2], slice(&cx.line(s[0]), 0, s[1]), slice(&cx.line(s[2]), s[3], usize::MAX)),
                None => (row, row, slice(&line, 0, col), slice(&line, col, usize::MAX)),
            };
            let parts: Vec<&str> = text.split('\n').collect();
            let last = parts.len() - 1;
            let mut insert: Vec<String> = Vec::new();
            let cursor;
            if last == 0 {
                insert.push(format!("{head}{}{tail}", parts[0]));
                cursor = [r0, head.chars().count() + parts[0].chars().count()];
            } else {
                insert.push(format!("{head}{}", parts[0]));
                for p in &parts[1..last] {
                    insert.push((*p).to_string());
                }
                insert.push(format!("{}{tail}", parts[last]));
                cursor = [r0 + last, parts[last].chars().count()];
            }
            EditRes { changed: true, start: r0, remove: r1 - r0 + 1, insert, cursor, tip: Some(format!("已粘贴 {} 字符", text.chars().count())), ..Default::default() }
        }
        "paste-box" => {
            let text = req.text.clone().unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n");
            let parts: Vec<&str> = text.split('\n').collect();
            if parts.len() == 1 && parts[0].is_empty() {
                return Some(cx.nothing("剪贴板为空"));
            }
            // 每行贴到 (光标行+i, 光标列); 行不够长补空格, 文档不够长补行
            let insert: Vec<String> = parts
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let mut l = if row + i < req.total_rows { cx.line(row + i) } else { Vec::new() };
                    while l.len() < col {
                        l.push(' ');
                    }
                    format!("{}{}{}", slice(&l, 0, col), p, slice(&l, col, usize::MAX))
                })
                .collect();
            let remove = parts.len().min(req.total_rows.saturating_sub(row));
            EditRes { changed: true, start: row, remove, insert, cursor: [row, col], tip: Some(format!("已矩形贴付 {} 行", parts.len())), ..Default::default() }
        }
        "copy-lines" | "copy-passage" | "copy-linenum" => {
            let (a, b) = sel.map(|s| (s[0], s[2])).unwrap_or((row, row));
            let width = 3.max(req.total_rows.to_string().len());
            let mut out = String::new();
            for r in a..=b {
                let l: String = cx.line(r).iter().collect();
                match req.cmd.as_str() {
                    "copy-lines" => out.push_str(&format!("{l}\n")),
                    "copy-passage" => out.push_str(&format!("> {l}\n")),
                    _ => out.push_str(&format!("{:>w$}:{l}\n", r + 1, w = width)),
                }
            }
            EditRes { clip: Some(out), cursor: [row, col], tip: Some("已复制".into()), ..Default::default() }
        }
        "copy-crlf" => {
            let t = match sel {
                Some(s) => cx.sel_text(s),
                None => line.iter().collect(),
            };
            EditRes { clip: Some(t.replace('\n', "\r\n")), cursor: [row, col], tip: Some("已用 CRLF 换行复制".into()), ..Default::default() }
        }
        // ---- 选词 (Sakura S_SelectWord): 光标所在字符类 (单词 / 空白 / 其它) 的连续段; 行末 = 选整行 (含换行) ----
        "select-word" => {
            if col >= line.len() {
                if line.is_empty() {
                    return Some(cx.nothing(""));
                }
                let (sel, cursor) = if row < last_row { ([row, 0, row + 1, 0], [row + 1, 0]) } else { ([row, 0, row, line.len()], [row, line.len()]) };
                return Some(EditRes { sel: Some(sel), anchor: Some([row, 0]), cursor, ..Default::default() });
            }
            let cls = |c: char| if is_word_char(c) { 1 } else if c.is_whitespace() { 2 } else { 3 };
            let k = cls(line[col]);
            let (mut i, mut j) = (col, col);
            while i > 0 && cls(line[i - 1]) == k {
                i -= 1;
            }
            while j < line.len() && cls(line[j]) == k {
                j += 1;
            }
            EditRes { sel: Some([row, i, row, j]), anchor: Some([row, i]), cursor: [row, j], ..Default::default() }
        }
        // ---- 整行 ----
        "cut-line" | "delete-line" => {
            let text: String = line.iter().collect();
            // 删掉最后一行时光标落在新的最后一行; 只剩一行时清空该行
            let (insert, new_row) = if req.total_rows <= 1 {
                (vec![String::new()], 0)
            } else {
                (vec![], if row == last_row { row - 1 } else { row })
            };
            EditRes {
                changed: true,
                start: row,
                remove: 1,
                insert,
                cursor: [new_row, 0],
                clip: if req.cmd == "cut-line" { Some(text) } else { None },
                tip: None,
                undo_cursor: None,
                clip_line: false,
                sel: None,
                anchor: None,
            }
        }
        // ---- 行结构 ----
        "new-line" => {
            let head = slice(&line, 0, col);
            let tail = slice(&line, col, usize::MAX);
            let hc: Vec<char> = head.chars().collect();
            let indent: String = if req.auto_indent { hc.iter().take_while(|c| **c == ' ' || **c == '\t').collect() } else { String::new() };
            let ends_open = hc.last().map(|c| matches!(c, '{' | '[' | '(') || c.is_whitespace()).unwrap_or(false);
            let extra = if req.auto_indent && req.smart_indent && ends_open { "  " } else { "" };
            let pad = format!("{indent}{extra}");
            let pad_len = pad.chars().count();
            EditRes { changed: true, start: row, remove: 1, insert: vec![head, format!("{pad}{tail}")], cursor: [row + 1, pad_len], ..Default::default() }
        }
        "add-line-above" | "add-line-below" => {
            let indent: String = line.iter().take_while(|c| **c == ' ' || **c == '\t').collect();
            let n = indent.chars().count();
            let above = req.cmd == "add-line-above";
            let at = if above { row } else { row + 1 };
            EditRes { changed: true, start: at, remove: 0, insert: vec![indent], cursor: [at, n], ..Default::default() }
        }
        "delete-line-up" => {
            if row == 0 {
                return Some(cx.nothing("已在首行, 上方无可删"));
            }
            let (insert, cur) = if req.total_rows <= 1 { (vec![String::new()], [0, 0]) } else { (vec![], [row - 1, 0]) };
            EditRes { changed: true, start: row - 1, remove: 1, insert, cursor: cur, tip: Some("已删除上方行".into()), ..Default::default() }
        }
        "delete-line-down" => {
            if row >= last_row {
                return Some(cx.nothing("已在末行, 下方无可删"));
            }
            EditRes { changed: true, start: row + 1, remove: 1, insert: vec![], cursor: [row, 0], tip: Some("已删除下方行".into()), ..Default::default() }
        }
        "join-with-prev" => {
            if row == 0 {
                return Some(cx.nothing(""));
            }
            let prev = cx.line(row - 1);
            let cur: String = line.iter().collect();
            let joined = format!("{}{}", prev.iter().collect::<String>(), cur.trim_start());
            EditRes { changed: true, start: row - 1, remove: 2, insert: vec![joined], cursor: [row - 1, prev.len()], ..Default::default() }
        }
        "move-line-up" | "move-line-down" => {
            let up = req.cmd == "move-line-up";
            if (up && row == 0) || (!up && row >= last_row) {
                return Some(cx.nothing(""));
            }
            let other_row = if up { row - 1 } else { row + 1 };
            let other: String = cx.line(other_row).iter().collect();
            let cur: String = line.iter().collect();
            let (start, insert) = if up { (row - 1, vec![cur, other]) } else { (row, vec![other, cur]) };
            EditRes { changed: true, start, remove: 2, insert, cursor: [other_row, col], tip: Some(format!("移动行: Ln {} → Ln {}", row + 1, other_row + 1)), ..Default::default() }
        }
        "duplicate-line" => {
            let text: String = line.iter().collect();
            EditRes { changed: true, start: row, remove: 1, insert: vec![text.clone(), text], cursor: [row + 1, col], clip: None, tip: None, undo_cursor: None, clip_line: false, sel: None, anchor: None }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_on(cmd: &str, row: usize, col: usize, sel: Option<[usize; 4]>, lines: &[&str]) -> EditRes {
        run(&EditReq {
            cmd: cmd.into(),
            row,
            col,
            sel,
            total_rows: lines.len(),
            auto_indent: true,
            smart_indent: true,
            text: None,
            line_mode: false,
            rows: lines.iter().enumerate().map(|(i, s)| (i, s.to_string())).collect(),
        })
        .unwrap()
    }
    /// 把结果套回文本 (和前端一样)
    fn apply(lines: &[&str], r: &EditRes) -> Vec<String> {
        let mut v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        v.splice(r.start..r.start + r.remove, r.insert.clone());
        v
    }
    const DOC: &[&str] = &["hello world foo", "  indented line", "", "third line here", "last Line END"];

    // 期望值来自与真实 Sakura 的比对 (tests/sakura-diff 对应用例)
    #[test]
    fn single_char() {
        let r = run_on("delete", 0, 3, None, DOC);
        assert_eq!(apply(DOC, &r)[0], "helo world foo");
        let r = run_on("backspace", 0, 3, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("helo world foo", [0, 2]));
        let r = run_on("backspace", 1, 0, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("hello world foo  indented line", [0, 15]));
        let r = run_on("delete", 0, 15, None, DOC);
        assert_eq!(apply(DOC, &r)[0], "hello world foo  indented line");
        assert!(!run_on("backspace", 0, 0, None, DOC).changed);
    }
    #[test]
    fn selection_replaced_by_delete() {
        let r = run_on("delete", 0, 8, Some([0, 2, 0, 8]), DOC);
        assert_eq!(apply(DOC, &r)[0], "herld foo");
        let r = run_on("backspace", 1, 1, Some([0, 2, 1, 1]), DOC);
        assert_eq!(apply(DOC, &r)[0], "he indented line");
        assert_eq!(r.cursor, [0, 2]);
    }
    #[test]
    fn word_cut_and_delete() {
        let r = run_on("word-cut", 0, 2, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), (" world foo", Some("hello")));
        let r = run_on("word-delete", 0, 15, None, DOC);
        assert_eq!(apply(DOC, &r)[0], "hello world foo  indented line"); // 行末: 删掉换行
        let r = run_on("word-cut", 0, 5, None, DOC);
        assert_eq!(r.clip.as_deref(), Some(" "));
    }
    #[test]
    fn word_to_start_end() {
        let r = run_on("delete-word-start", 0, 8, None, DOC);
        assert_eq!(apply(DOC, &r)[0], "hello rld foo");
        let r = run_on("delete-word-start", 1, 0, None, DOC);
        assert_eq!(r.cursor, [0, 15]); // 行首: 与上一行合并
        let r = run_on("delete-word-end", 0, 8, None, DOC);
        assert_eq!(apply(DOC, &r)[0], "hello wofoo");
        let r = run_on("delete-word-end", 0, 15, None, DOC);
        assert_eq!(apply(DOC, &r)[0], "hello world foo" .to_string() + "indented line"); // 行末: 合并并吃掉下一行开头的空白
    }
    #[test]
    fn line_start_end() {
        let r = run_on("cut-line-start", 0, 5, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), (" world foo", Some("hello")));
        let r = run_on("line-cut-to-end", 0, 5, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), ("hello", Some(" world foo")));
        assert!(!run_on("line-cut-to-end", 0, 15, None, DOC).changed);
        let r = run_on("delete-line-to-end", 0, 2, Some([0, 2, 0, 4]), DOC);
        assert_eq!(apply(DOC, &r)[0], "heo world foo"); // 有选区: 只删选区
    }
    #[test]
    fn whole_line() {
        let r = run_on("cut-line", 1, 4, None, DOC);
        assert_eq!((apply(DOC, &r).len(), r.clip.as_deref(), r.cursor), (4, Some("  indented line"), [1, 0]));
        let r = run_on("delete-line", 4, 2, None, DOC);
        assert_eq!((apply(DOC, &r).len(), r.cursor), (4, [3, 0])); // 最后一行被删: 光标落在新的最后一行
        let r = run_on("delete-line", 0, 0, None, &["only"]);
        assert_eq!(apply(&["only"], &r), [""]);
        let r = run_on("duplicate-line", 1, 4, None, DOC);
        assert_eq!((apply(DOC, &r)[1].as_str(), apply(DOC, &r)[2].as_str(), r.cursor), ("  indented line", "  indented line", [2, 4]));
    }
    #[test]
    fn line_structure() {
        // Enter: 自动缩进 + 行尾 { [ ( 或空白再多一级
        let r = run_on("new-line", 1, 6, None, DOC);
        assert_eq!((r.insert.clone(), r.cursor), (vec!["  inde".to_string(), "  nted line".to_string()], [2, 2]));
        let d = &["fn f() {"];
        let r = run_on("new-line", 0, 8, None, d);
        assert_eq!((r.insert.clone(), r.cursor), (vec!["fn f() {".to_string(), "  ".to_string()], [1, 2]));
        let r = run_on("add-line-above", 1, 4, None, DOC);
        assert_eq!((apply(DOC, &r)[1].as_str(), r.cursor), ("  ", [1, 2]));
        let r = run_on("add-line-below", 1, 4, None, DOC);
        assert_eq!((apply(DOC, &r)[2].as_str(), r.cursor), ("  ", [2, 2]));
        let r = run_on("move-line-up", 1, 4, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("  indented line", [0, 4]));
        let r = run_on("move-line-down", 0, 4, None, DOC);
        assert_eq!((apply(DOC, &r)[1].as_str(), r.cursor), ("hello world foo", [1, 4]));
        assert!(!run_on("move-line-up", 0, 0, None, DOC).changed);
        let r = run_on("join-with-prev", 1, 0, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("hello world fooindented line", [0, 15]));
        let r = run_on("delete-line-up", 1, 3, None, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("  indented line", [0, 0]));
        let r = run_on("delete-line-down", 0, 3, None, DOC);
        assert_eq!((apply(DOC, &r)[1].as_str(), r.cursor), ("", [0, 0]));
    }
    fn run_text(cmd: &str, row: usize, col: usize, sel: Option<[usize; 4]>, text: &str, line_mode: bool, lines: &[&str]) -> EditRes {
        run(&EditReq {
            cmd: cmd.into(),
            row,
            col,
            sel,
            total_rows: lines.len(),
            auto_indent: false,
            smart_indent: false,
            text: Some(text.into()),
            line_mode,
            rows: lines.iter().enumerate().map(|(i, s)| (i, s.to_string())).collect(),
        })
        .unwrap()
    }
    #[test]
    fn clipboard_copy_cut() {
        let r = run_on("copy", 0, 0, Some([0, 2, 0, 8]), DOC);
        assert_eq!((r.changed, r.clip.as_deref(), r.clip_line), (false, Some("llo wo"), false));
        let r = run_on("copy", 1, 3, None, DOC);
        assert_eq!((r.clip.as_deref(), r.clip_line), (Some("  indented line\n"), true)); // 无选区 = 整行
        let r = run_on("cut", 0, 0, Some([0, 2, 1, 3]), DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), ("hendented line", Some("llo world foo\n  i")));
        let r = run_on("cut", 4, 2, None, DOC);
        assert_eq!((apply(DOC, &r).len(), r.cursor, r.clip_line), (4, [3, 0], true));
    }
    #[test]
    fn clipboard_paste() {
        let r = run_text("paste", 0, 3, None, "XY", false, DOC);
        assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("helXYlo world foo", [0, 5]));
        let r = run_text("paste", 1, 4, None, "ab\ncd\nef", false, DOC); // 多行粘贴不能吞掉后面的行
        let v = apply(DOC, &r);
        assert_eq!((v[1].as_str(), v[2].as_str(), v[3].as_str(), v.len(), r.cursor), ("  inab", "cd", "efdented line", 7, [3, 2]));
        let r = run_text("paste", 0, 5, Some([0, 2, 0, 5]), "XY", false, DOC); // 替换选区
        assert_eq!(apply(DOC, &r)[0], "heXY world foo");
        let r = run_text("paste", 1, 3, None, "L1\nL2\n", true, DOC); // 整行模式
        let v = apply(DOC, &r);
        assert_eq!((v[1].as_str(), v[2].as_str(), v[3].as_str(), r.cursor), ("L1", "L2", "  indented line", [3, 3]));
        assert!(!run_text("paste", 0, 0, None, "", false, DOC).changed);
    }
    #[test]
    fn clipboard_box_and_variants() {
        let r = run_text("paste-box", 0, 3, None, "ab\ncd", false, DOC);
        let v = apply(DOC, &r);
        assert_eq!((v[0].as_str(), v[1].as_str()), ("helablo world foo", "  icdndented line"));
        let r = run_on("copy-lines", 0, 0, Some([0, 2, 1, 3]), DOC);
        assert_eq!(r.clip.as_deref(), Some("hello world foo\n  indented line\n"));
        let r = run_on("copy-passage", 0, 0, Some([0, 2, 1, 3]), DOC);
        assert_eq!(r.clip.as_deref(), Some("> hello world foo\n>   indented line\n"));
        let r = run_on("copy-linenum", 0, 0, Some([0, 2, 1, 3]), DOC);
        assert_eq!(r.clip.as_deref(), Some("  1:hello world foo\n  2:  indented line\n"));
        let r = run_on("copy-crlf", 0, 0, Some([0, 2, 1, 3]), DOC);
        assert_eq!(r.clip.as_deref(), Some("llo world foo\r\n  i"));
    }
    #[test]
    fn select_word() {
        let r = run_on("select-word", 0, 8, None, DOC);
        assert_eq!((r.sel, r.cursor, r.changed), (Some([0, 6, 0, 11]), [0, 11], false));
        let r = run_on("select-word", 0, 5, None, DOC); // 空白段
        assert_eq!(r.sel, Some([0, 5, 0, 6]));
        let r = run_on("select-word", 0, 15, None, DOC); // 行末: 选整行 (含换行)
        assert_eq!((r.sel, r.cursor), (Some([0, 0, 1, 0]), [1, 0]));
        let r = run_on("select-word", 4, 13, None, DOC); // 最后一行行末: 选到行尾
        assert_eq!(r.sel, Some([4, 0, 4, 13]));
        assert!(run_on("select-word", 2, 0, None, DOC).sel.is_none()); // 空行
    }
    #[test]
    fn malformed_requests_do_not_panic() {
        let r = run_on("delete", 0, 0, Some([1, 3, 0, 2]), DOC); // 反向选区
        assert_eq!(apply(DOC, &r)[0], "hendented line"); // 与正向选区 [0,2]-[1,3] 结果相同
        let r = run_text("paste-box", 9, 0, None, "x", false, DOC); // 光标行超出文档
        assert_eq!(r.remove, 0);
    }
    #[test]
    fn unknown() {
        assert!(run(&EditReq { cmd: "nope".into(), row: 0, col: 0, sel: None, total_rows: 1, auto_indent: false, smart_indent: false, text: None, line_mode: false, rows: vec![] }).is_none());
    }
}
