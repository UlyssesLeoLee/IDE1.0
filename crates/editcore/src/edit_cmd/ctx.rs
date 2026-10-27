//! 命令的执行环境: 请求 + 预先算好的光标行 / 列 / 选区, 以及各命令共用的取行 / 选区文字 / 合并行等小操作。
//!
//! 无状态: 前端只交上命令需要的几行 (`req.rows`); 没交上来的行用 [`Ctx::has_row`] 查,
//! 不要拿 [`Ctx::line`] 给的空行去改文档 (会把真实内容写没)。

use super::req_res::{EditReq, EditRes};
use crate::word::first_word_col;
use crate::text_util::{chars_of, slice};
use std::collections::HashMap;

pub(super) struct Ctx<'a> {
    pub req: &'a EditReq,
    /// 行号 -> 在 req.rows 里的位置
    idx: HashMap<usize, usize>,
    /// 光标行 / 列 (列已夹进行长)
    pub row: usize,
    pub col: usize,
    /// 文档最后一行的行号
    pub last_row: usize,
    /// 光标所在行
    pub line: Vec<char>,
    /// 非空选区 (起点在前; 请求里的选区可能是反向的, 反向时 r1 - r0 会下溢)
    pub sel: Option<[usize; 4]>,
}

impl<'a> Ctx<'a> {
    pub fn new(req: &'a EditReq) -> Self {
        let mut idx = HashMap::with_capacity(req.rows.len());
        for (i, (r, _)) in req.rows.iter().enumerate() {
            idx.entry(*r).or_insert(i); // 重复行号: 第一个算数
        }
        let mut cx = Ctx { req, idx, row: req.row, col: 0, last_row: req.total_rows.saturating_sub(1), line: Vec::new(), sel: None };
        cx.line = cx.line(req.row);
        cx.col = req.col.min(cx.line.len());
        cx.sel = req.sel.map(|s| if (s[2], s[3]) < (s[0], s[1]) { [s[2], s[3], s[0], s[1]] } else { s }).filter(|s| !(s[0] == s[2] && s[1] == s[3]));
        cx
    }

    /// 这一行前端交上来了吗
    pub fn has_row(&self, r: usize) -> bool {
        self.idx.contains_key(&r)
    }
    /// 某一行的字符 (没交上来的行是空的, 先用 `has_row` 查)
    pub fn line(&self, r: usize) -> Vec<char> {
        self.idx.get(&r).map(|&i| chars_of(&self.req.rows[i].1)).unwrap_or_default()
    }
    /// 光标行的文本
    pub fn line_str(&self) -> String {
        self.line.iter().collect()
    }

    /// 什么也没改: 光标留在请求发来时的位置
    pub fn nothing(&self, tip: &str) -> EditRes {
        EditRes::nothing([self.req.row, self.req.col], tip)
    }

    /// 选区文字 (行间用 \n)
    pub fn sel_text(&self, s: [usize; 4]) -> String {
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
    /// 删除选区 (不动剪贴板)
    pub fn delete_sel(&self, s: [usize; 4]) -> EditRes {
        let first = self.line(s[0]);
        let head = slice(&first, 0, s[1]);
        let tail = slice(&self.line(s[2]), s[3], usize::MAX);
        EditRes::splice(s[0], s[2] - s[0] + 1, vec![format!("{head}{tail}")], [s[0], s[1].min(first.len())])
    }
    /// 剪切选区 (删除 + 选区文字进剪贴板)
    pub fn cut_sel(&self, s: [usize; 4]) -> EditRes {
        self.delete_sel(s).clip(Some(self.sel_text(s)))
    }

    /// 把第 `r` 行和下一行并成一行, 光标落在原来的行尾; `skip_leading` = 吃掉下一行开头到词首之前的非词字符
    pub fn join_next(&self, r: usize, skip_leading: bool) -> EditRes {
        let cur = self.line(r);
        let next = self.line(r + 1);
        let skip = if skip_leading { first_word_col(&next) } else { 0 };
        let joined = format!("{}{}", cur.iter().collect::<String>(), slice(&next, skip, usize::MAX));
        EditRes::splice(r, 2, vec![joined], [r, cur.len()])
    }
    /// 把光标行并到上一行末尾 (光标行必须不是第 0 行), 光标落在原来的上一行行尾; `trim` = 并入前去掉光标行开头的空白
    pub fn join_into_prev(&self, trim: bool) -> EditRes {
        let prev = self.line(self.row - 1);
        let cur = self.line_str();
        let cur = if trim { cur.trim_start().to_string() } else { cur };
        EditRes::splice(self.row - 1, 2, vec![format!("{}{}", prev.iter().collect::<String>(), cur)], [self.row - 1, prev.len()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(row: usize, col: usize, sel: Option<[usize; 4]>) -> EditReq {
        EditReq { row, col, sel, total_rows: 3, rows: vec![(0, "ab".into()), (1, "cde".into()), (2, "f".into())], ..Default::default() }
    }

    #[test]
    fn column_is_clamped_but_nothing_keeps_the_request_position() {
        let r = req(1, 99, None);
        let cx = Ctx::new(&r);
        assert_eq!((cx.row, cx.col, cx.last_row), (1, 3, 2));
        assert_eq!(cx.nothing("t").cursor, [1, 99]);
    }

    #[test]
    fn reversed_and_empty_selections_are_normalised() {
        assert_eq!(Ctx::new(&req(0, 0, Some([2, 1, 0, 1]))).sel, Some([0, 1, 2, 1]));
        assert_eq!(Ctx::new(&req(0, 0, Some([1, 2, 1, 2]))).sel, None);
    }

    #[test]
    fn missing_rows_are_detected_and_first_duplicate_wins() {
        let mut r = req(0, 0, None);
        r.rows.push((1, "dup".into()));
        r.rows.retain(|(i, _)| *i != 2);
        let cx = Ctx::new(&r);
        assert!(cx.has_row(1) && !cx.has_row(2));
        assert_eq!(cx.line(1).iter().collect::<String>(), "cde");
    }

    #[test]
    fn selection_text_and_joins() {
        let r = req(1, 0, Some([0, 1, 2, 1]));
        let cx = Ctx::new(&r);
        assert_eq!(cx.sel_text([0, 1, 2, 1]), "b\ncde\nf");
        let j = cx.join_into_prev(false);
        assert_eq!((j.insert, j.cursor, j.start, j.remove), (vec!["abcde".to_string()], [0, 2], 0, 2));
        assert_eq!(cx.join_next(1, false).insert, vec!["cdef".to_string()]);
    }
}
