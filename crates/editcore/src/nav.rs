//! 键盘光标移动 / 范围选择 (ARCH-01: 移动语义放 Rust).
//!
//! 对应前端 `handleEditNavKey` 里的 ← → ↑ ↓ Home End PageUp PageDown (含 Ctrl / Shift 组合),
//! 语义按真实 Sakura Editor 的键盘宏逐条比对 (tests/sakura-diff).
//!
//! 无状态: 前端把光标周围需要的几行 (`rows`: 行号 → 文本) 连同选区一起交上来, 这里算出新的光标 / 选区 / 目标列。
//! 列一律按码点 (`char`) 计, 与前端 `charLen` 同一套。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct NavReq {
    /// DOM `KeyboardEvent.key`
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub row: usize,
    pub col: usize,
    /// 竖向移动时记住的目标列
    #[serde(default)]
    pub goal_col: Option<usize>,
    /// 当前选区 [r0, c0, r1, c1] (前端已把"到行尾"解析成具体列)
    #[serde(default)]
    pub sel: Option<[usize; 4]>,
    /// 范围选择的锚点 [row, col] (前端已按 selAnchorOf 解析)
    #[serde(default)]
    pub anchor: Option<[usize; 2]>,
    /// 文档总行数
    pub total_rows: usize,
    /// 需要用到的行: [行号, 文本]; 缺的行按空行处理
    pub rows: Vec<(usize, String)>,
    /// 一页的行数 (PageUp / PageDown)
    #[serde(default)]
    pub page_rows: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NavRes {
    pub row: usize,
    pub col: usize,
    pub goal_col: Option<usize>,
    pub sel: Option<[usize; 4]>,
    pub anchor: Option<[usize; 2]>,
}

/// 单词字符: 字母 / 数字 / 下划线 (日文没有空格分词, 与 Sakura 一样"非单词字符即边界")
pub fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c.is_numeric() || c == '_'
}

/// Sakura S_WordRight: 词内 -> 跳过本词再跳过后面的分隔符, 落在下一个词首; 已在分隔符上 -> 直接到下一个词首
pub fn word_right(line: &[char], from: usize) -> usize {
    let mut i = from.min(line.len());
    if i >= line.len() {
        return line.len();
    }
    if is_word_char(line[i]) {
        while i < line.len() && is_word_char(line[i]) {
            i += 1;
        }
    }
    while i < line.len() && !is_word_char(line[i]) {
        i += 1;
    }
    i
}

/// Sakura S_WordLeft: 回到前一个词的词首
pub fn word_left(line: &[char], from: usize) -> usize {
    let mut i = from.min(line.len());
    while i > 0 && !is_word_char(line[i - 1]) {
        i -= 1;
    }
    while i > 0 && is_word_char(line[i - 1]) {
        i -= 1;
    }
    i
}

/// 行里第一个单词字符的列 (没有则 0)
pub fn first_word_col(line: &[char]) -> usize {
    line.iter().position(|c| is_word_char(*c)).unwrap_or(0)
}

fn normalize(a: [usize; 2], b: [usize; 2]) -> Option<[usize; 4]> {
    if a == b {
        return None;
    }
    let fwd = a[0] < b[0] || (a[0] == b[0] && a[1] <= b[1]);
    let (s0, s1) = if fwd { (a, b) } else { (b, a) };
    Some([s0[0], s0[1], s1[0], s1[1]])
}

/// 执行一次移动. 不是移动键 / 需要前端自己处理的组合 (Ctrl+↑↓ 滚动) 返回 None.
pub fn step(req: &NavReq) -> Option<NavRes> {
    let line_of = |r: usize| -> Vec<char> {
        req.rows.iter().find(|(i, _)| *i == r).map(|(_, s)| s.chars().collect()).unwrap_or_default()
    };
    let len = |r: usize| line_of(r).len();
    let last_row = req.total_rows.saturating_sub(1);
    let (mut row, mut col) = (req.row, req.col);
    let had = req.sel.map(|s| !(s[0] == s[2] && s[1] == s[3])).unwrap_or(false);
    let (ctrl, shift) = (req.ctrl, req.shift);
    let k = req.key.as_str();
    let vertical = matches!(k, "ArrowUp" | "ArrowDown" | "PageUp" | "PageDown");
    if !matches!(k, "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp" | "PageDown") {
        return None;
    }

    // 无 Shift 时, 左右键遇到选区先收缩到选区端点 (Sakura)
    if !shift && !ctrl && had && (k == "ArrowLeft" || k == "ArrowRight") {
        let s = req.sel?;
        let to_end = k == "ArrowRight";
        let (r, c) = if to_end { (s[2], s[3]) } else { (s[0], s[1]) };
        return Some(NavRes { row: r, col: c.min(len(r)), goal_col: None, sel: None, anchor: None });
    }

    let mut goal = req.goal_col;
    match k {
        "ArrowUp" | "ArrowDown" => {
            let d: isize = if k == "ArrowUp" { -1 } else { 1 };
            if ctrl && !shift {
                return None; // Ctrl+↑↓ = 文本滚动, 光标不动 (前端处理)
            }
            let step: isize = if ctrl && shift { 2 } else { 1 };
            let g = *goal.get_or_insert(col);
            row = (row as isize + d * step).clamp(0, last_row as isize) as usize;
            col = g.min(len(row));
        }
        "PageUp" | "PageDown" => {
            let d: isize = if k == "PageUp" { -1 } else { 1 };
            let n = if ctrl { (req.page_rows / 2).max(1) } else { req.page_rows.max(1) } as isize;
            let g = *goal.get_or_insert(col);
            row = (row as isize + d * n).clamp(0, last_row as isize) as usize;
            col = g.min(len(row));
        }
        "ArrowLeft" => {
            if ctrl {
                if col == 0 && row > 0 {
                    row -= 1;
                    col = len(row);
                } else {
                    col = word_left(&line_of(row), col);
                }
            } else if col > 0 {
                col -= 1;
            } else if row > 0 {
                row -= 1;
                col = len(row);
            }
        }
        "ArrowRight" => {
            if ctrl {
                let cur = line_of(row);
                let c = word_right(&cur, col);
                if col >= cur.len() && row < last_row {
                    row += 1;
                    col = first_word_col(&line_of(row));
                } else {
                    col = c;
                }
            } else if col < len(row) {
                col += 1;
            } else if row < last_row {
                row += 1;
                col = 0;
            }
        }
        "Home" => {
            if ctrl {
                row = 0;
                col = 0;
            } else {
                let fnb = line_of(row).iter().take_while(|c| **c == ' ' || **c == '\t').count();
                col = if col != fnb { fnb } else { 0 }; // Sakura: 先到行首非空白, 再按到第 0 列
            }
        }
        "End" => {
            if ctrl {
                row = last_row;
                col = len(last_row);
            } else {
                col = len(row);
            }
        }
        _ => return None,
    }
    if !vertical {
        goal = None;
    }
    let (sel, anchor) = if shift {
        let a = req.anchor.unwrap_or([req.row, req.col]);
        (normalize(a, [row, col]), Some(a))
    } else {
        (None, None)
    };
    Some(NavRes { row, col, goal_col: goal, sel, anchor })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(key: &str, ctrl: bool, shift: bool, row: usize, col: usize, lines: &[&str]) -> NavReq {
        NavReq {
            key: key.into(),
            ctrl,
            shift,
            row,
            col,
            goal_col: None,
            sel: None,
            anchor: None,
            total_rows: lines.len(),
            rows: lines.iter().enumerate().map(|(i, s)| (i, s.to_string())).collect(),
            page_rows: 10,
        }
    }
    fn go(key: &str, ctrl: bool, shift: bool, row: usize, col: usize, lines: &[&str]) -> (usize, usize) {
        let r = step(&req(key, ctrl, shift, row, col, lines)).unwrap();
        (r.row, r.col)
    }

    const DOC: &[&str] = &["hello world foo", "  indented line", "", "third line here", "last Line END"];

    // 以下期望值来自与真实 Sakura 的比对 (tests/sakura-diff)
    #[test]
    fn word_motion() {
        assert_eq!(go("ArrowRight", true, false, 0, 1, DOC), (0, 6));
        assert_eq!(go("ArrowRight", true, false, 0, 5, DOC), (0, 6));
        assert_eq!(go("ArrowRight", true, false, 0, 15, DOC), (1, 2)); // 行末 -> 下一行第一个词首
        assert_eq!(go("ArrowLeft", true, false, 0, 8, DOC), (0, 6));
        assert_eq!(go("ArrowLeft", true, false, 1, 0, DOC), (0, 15));
        assert_eq!(go("ArrowLeft", true, false, 1, 4, DOC), (1, 2));
    }
    #[test]
    fn home_end() {
        assert_eq!(go("Home", false, false, 1, 6, DOC), (1, 2));
        assert_eq!(go("Home", false, false, 1, 2, DOC), (1, 0));
        assert_eq!(go("Home", false, false, 1, 1, DOC), (1, 2));
        assert_eq!(go("End", false, false, 1, 2, DOC), (1, 15));
        assert_eq!(go("Home", true, false, 3, 4, DOC), (0, 0));
        assert_eq!(go("End", true, false, 1, 4, DOC), (4, 13));
    }
    #[test]
    fn left_right_wrap() {
        assert_eq!(go("ArrowLeft", false, false, 1, 0, DOC), (0, 15));
        assert_eq!(go("ArrowRight", false, false, 0, 15, DOC), (1, 0));
        assert_eq!(go("ArrowRight", false, false, 4, 13, DOC), (4, 13));
    }
    #[test]
    fn vertical_keeps_goal_column() {
        let mut r = req("ArrowDown", false, false, 3, 10, DOC);
        let a = step(&r).unwrap();
        assert_eq!((a.row, a.col, a.goal_col), (4, 10, Some(10)));
        r = req("ArrowUp", false, false, 3, 10, DOC);
        let b = step(&r).unwrap();
        assert_eq!((b.row, b.col), (2, 0)); // 空行
        r.row = 2;
        r.col = 0;
        r.goal_col = b.goal_col;
        let c = step(&r).unwrap();
        assert_eq!((c.row, c.col), (1, 10)); // 目标列恢复
    }
    #[test]
    fn shift_extends_selection() {
        let r = step(&req("ArrowRight", false, true, 0, 3, DOC)).unwrap();
        assert_eq!(r.sel, Some([0, 3, 0, 4]));
        assert_eq!(r.anchor, Some([0, 3]));
        let r = step(&req("ArrowDown", false, true, 0, 3, DOC)).unwrap();
        assert_eq!(r.sel, Some([0, 3, 1, 3]));
        let r = step(&req("End", false, true, 0, 3, DOC)).unwrap();
        assert_eq!(r.sel, Some([0, 3, 0, 15]));
    }
    #[test]
    fn plain_arrow_collapses_selection() {
        let mut r = req("ArrowLeft", false, false, 0, 8, DOC);
        r.sel = Some([0, 2, 0, 8]);
        let a = step(&r).unwrap();
        assert_eq!((a.row, a.col, a.sel), (0, 2, None));
        r.key = "ArrowRight".into();
        let b = step(&r).unwrap();
        assert_eq!((b.row, b.col, b.sel), (0, 8, None));
    }
    #[test]
    fn page_keys_use_page_rows() {
        let mut r = req("PageDown", false, false, 0, 2, DOC);
        r.page_rows = 2;
        assert_eq!(step(&r).map(|x| (x.row, x.col)), Some((2, 0)));
        r.ctrl = true; // 半页
        assert_eq!(step(&r).map(|x| (x.row, x.col)), Some((1, 2)));
    }
    #[test]
    fn non_nav_and_scroll_keys_are_not_handled() {
        assert!(step(&req("a", false, false, 0, 0, DOC)).is_none());
        assert!(step(&req("ArrowUp", true, false, 1, 0, DOC)).is_none());
    }
    #[test]
    fn japanese_words() {
        let doc = &["あいう えお かき"];
        assert_eq!(go("ArrowRight", true, false, 0, 0, doc), (0, 4));
    }
}
