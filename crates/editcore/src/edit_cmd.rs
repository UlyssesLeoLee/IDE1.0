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
        self.req.sel.filter(|s| !(s[0] == s[2] && s[1] == s[3]))
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
        }
    }
    fn replace_line(&self, r: usize, new: String, cursor: [usize; 2], clip: Option<String>) -> EditRes {
        EditRes { changed: true, start: r, remove: 1, insert: vec![new], cursor, clip, tip: None, undo_cursor: None }
    }
    fn join_next(&self, r: usize, skip_leading: bool) -> EditRes {
        let cur = self.line(r);
        let next = self.line(r + 1);
        let skip = if skip_leading { first_word_col(&next) } else { 0 };
        let joined = format!("{}{}", cur.iter().collect::<String>(), slice(&next, skip, usize::MAX));
        EditRes { changed: true, start: r, remove: 2, insert: vec![joined], cursor: [r, cur.len()], clip: None, tip: None, undo_cursor: None }
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
                EditRes { changed: true, start: row - 1, remove: 2, insert: vec![joined], cursor: [row - 1, prev.len()], clip: None, tip: None, undo_cursor: None }
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
                EditRes { changed: true, start: row - 1, remove: 2, insert: vec![joined], cursor: [row - 1, prev.len()], clip: None, tip: None, undo_cursor: None }
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
            }
        }
        "duplicate-line" => {
            let text: String = line.iter().collect();
            EditRes { changed: true, start: row, remove: 1, insert: vec![text.clone(), text], cursor: [row + 1, col], clip: None, tip: None, undo_cursor: None }
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
    fn unknown() {
        assert!(run(&EditReq { cmd: "nope".into(), row: 0, col: 0, sel: None, total_rows: 1, rows: vec![] }).is_none());
    }
}
