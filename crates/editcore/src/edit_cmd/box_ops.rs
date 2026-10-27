//! 矩形(列块)选区上的编辑 (Sakura 的 BOX 选择; 语义按真实 Sakura 实测, 见 tests/sakura-diff 的 BOX 用例)。
//!
//! 前端把**未归一化**的 `box_sel` 和参与的行交上来, 这里归一化后处理:
//!   键入 / 粘贴 / 输入法: 矩形有宽度时先挖掉里面选中的内容 (覆盖), 再在每一行的矩形左端插入, 插完矩形缩成插入点 (接着打);
//!                          多行剪贴板只取第一行
//!   退格 / Delete:         删掉矩形里的内容 (零宽矩形什么也不删), 矩形解除, 光标回左上角
//!   回车:                  先删矩形内容, 再在左上角换行 (只断一行)

use super::ctx::Ctx;
use super::line_ops::split_line_at;
use super::req_res::EditRes;
use crate::text_util::slice;

/// 归一化后的矩形 + 它覆盖的各行
struct BoxView {
    r0: usize,
    r1: usize,
    c0: usize,
    c1: usize,
    /// r0..=r1 各行的字符
    old: Vec<Vec<char>>,
}

impl BoxView {
    /// 归一化: r0<=r1, c0<=c1, 行号夹在文档内, 列夹在参与各行的最长长度内 (End 键可把列拖到行尾之外)。
    /// 没有 box_sel, 或参与的行没交全 (不能拿空行去改) = None。
    fn new(cx: &Ctx) -> Option<Self> {
        let b = cx.req.box_sel?;
        let last = cx.req.total_rows.saturating_sub(1);
        let r0 = b[0].min(b[2]).min(last);
        let r1 = b[0].max(b[2]).min(last).max(r0);
        if (r0..=r1).any(|r| !cx.has_row(r)) {
            return None;
        }
        let old: Vec<Vec<char>> = (r0..=r1).map(|r| cx.line(r)).collect();
        let max_len = old.iter().map(Vec::len).max().unwrap_or(0);
        let c0 = b[1].min(b[3]).min(max_len);
        let c1 = b[1].max(b[3]).min(max_len).max(c0);
        Some(BoxView { r0, r1, c0, c1, old })
    }

    fn n_rows(&self) -> usize {
        self.r1 - self.r0 + 1
    }
    fn width(&self) -> usize {
        self.c1 - self.c0
    }
    /// 挖掉一行里矩形覆盖的那段
    fn hollow(&self, l: &[char]) -> Vec<char> {
        let mut v = l[..self.c0.min(l.len())].to_vec();
        v.extend_from_slice(&l[self.c1.min(l.len())..]);
        v
    }
    fn hollowed_rows(&self) -> Vec<Vec<char>> {
        self.old.iter().map(|l| self.hollow(l)).collect()
    }
    /// 矩形里选中的内容, 每行一段、行间 \n、末尾也带 \n (Sakura 复制矩形到剪贴板的格式; 塌缩的矩形给空行)
    fn clip_text(&self) -> String {
        let mut s = String::new();
        for l in &self.old {
            if self.width() > 0 {
                s.push_str(&slice(l, self.c0, self.c1));
            }
            s.push('\n');
        }
        s
    }
    /// 把矩形覆盖的各行换成 `rows`
    fn patch(&self, rows: Vec<Vec<char>>, cursor: [usize; 2]) -> EditRes {
        EditRes::splice(self.r0, self.n_rows(), rows.into_iter().map(|v| v.into_iter().collect()).collect(), cursor)
    }
}

fn rows_missing(cx: &Ctx) -> EditRes {
    cx.nothing("列块涉及的行没有交全")
}

pub(super) fn copy(cx: &Ctx) -> EditRes {
    let Some(b) = BoxView::new(cx) else { return rows_missing(cx) };
    EditRes::clip_only(b.clip_text(), [cx.row, cx.col]).tip(format!("已复制列块 {} 行", b.n_rows()))
}

/// 退格和 Delete 一样 (Sakura 实测)
pub(super) fn delete(cx: &Ctx) -> EditRes {
    let Some(b) = BoxView::new(cx) else { return rows_missing(cx) };
    let rows = b.hollowed_rows();
    let cursor = [b.r0, b.c0.min(rows[0].len())]; // Sakura: 删除后光标在矩形左上角
    b.patch(rows, cursor).tip(format!("列块已删除 ({}行 ×{}列)", b.n_rows(), b.width()))
}

pub(super) fn cut(cx: &Ctx) -> EditRes {
    let Some(b) = BoxView::new(cx) else { return rows_missing(cx) };
    let rows = b.hollowed_rows();
    b.patch(rows, [b.r0, b.c0]).tip("已剪切列块").clip(Some(b.clip_text()))
}

/// 回车: 先删矩形内容, 再在左上角换行 (复用普通回车的缩进规则)
pub(super) fn enter(cx: &Ctx) -> EditRes {
    let Some(b) = BoxView::new(cx) else { return rows_missing(cx) };
    let mut rows = b.hollowed_rows();
    let c = b.c0.min(rows[0].len());
    let (mut insert, pad_len) = split_line_at(&rows[0], c, cx.req.auto_indent, cx.req.smart_indent);
    insert.extend(rows.drain(1..).map(|v| v.into_iter().collect::<String>()));
    EditRes::splice(b.r0, b.n_rows(), insert, [b.r0 + 1, pad_len])
}

/// 逐行替换矩形里的内容: 文本用 \n 分行; 不足宽度补空格, 超宽截断 (等宽替换)
pub(super) fn replace(cx: &Ctx) -> EditRes {
    let Some(b) = BoxView::new(cx) else { return rows_missing(cx) };
    let text = cx.req.text.clone().unwrap_or_default();
    let parts: Vec<&str> = text.split('\n').collect();
    let rows: Vec<Vec<char>> = b
        .old
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let mut repl: Vec<char> = parts.get(i).copied().unwrap_or("").chars().collect();
            repl.resize(b.width(), ' ');
            let mut v = l[..b.c0.min(l.len())].to_vec();
            v.extend(repl);
            v.extend_from_slice(&l[b.c1.min(l.len())..]);
            v
        })
        .collect();
    b.patch(rows, [cx.row, cx.col]).tip("列块已替换")
}

/// 键入 / 粘贴: 每一行同一份内容; 矩形有宽度时先挖掉里面选中的内容; 短行补空格对齐
pub(super) fn insert(cx: &Ctx) -> EditRes {
    let Some(b) = BoxView::new(cx) else { return rows_missing(cx) };
    let text = cx.req.text.clone().unwrap_or_default();
    let add: Vec<char> = text.split('\n').next().unwrap_or("").trim_end_matches('\r').chars().collect();
    if add.is_empty() {
        return cx.nothing("");
    }
    let rows: Vec<Vec<char>> = b
        .old
        .iter()
        .map(|l| {
            let mut v = if b.width() > 0 { b.hollow(l) } else { l.clone() };
            v.resize(v.len().max(b.c0), ' ');
            let tail = v.split_off(b.c0);
            v.extend_from_slice(&add);
            v.extend(tail);
            v
        })
        .collect();
    let to = b.c0 + add.len(); // 插入点右移「刚插入的字符数」列, 可以接着连续输入
    b.patch(rows, [cx.row, cx.col]).box_sel(Some([b.r0, to, b.r1, to])).tip(format!("列块插入 {} 行", b.n_rows()))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    const BX: &[&str] = &["abcdef", "gh", "ijklmn"];

    #[test]
    fn normalisation_clamps_rows_and_columns() {
        // 列越过行尾 (End 键) 会夹在参与行的最长长度内
        let r = run_box("box-delete", 0, 0, [0, 3, 1, 9999], "", BX);
        assert_eq!(apply(BX, &r)[..2], ["abc".to_string(), "gh".into()]);
        // 反向矩形和正向一样
        assert_eq!(run_box("box-copy", 2, 1, [2, 4, 0, 1], "", BX).clip.as_deref(), Some("bcd\nh\njkl\n"));
    }

    #[test]
    fn missing_rows_do_nothing_instead_of_blanking_real_lines() {
        let mut req = req("box-insert", 0, 0).with_text("X").with_box([5, 0, 6, 0]);
        req.total_rows = 3;
        req.rows = vec![(0, "abcdef".into()), (1, "gh".into())];
        assert!(!super::super::run(&req).unwrap().changed);
        req.rows.push((2, "ijklmn".into()));
        assert_eq!(apply(BX, &super::super::run(&req).unwrap())[2], "Xijklmn");
    }

    #[test]
    fn enter_breaks_only_one_line_after_clearing_the_rectangle() {
        let r = run_box("box-enter", 2, 4, [0, 1, 2, 4], "", BX);
        assert_eq!((apply(BX, &r), r.cursor, r.box_sel), (vec!["a".to_string(), "ef".into(), "g".into(), "imn".into()], [1, 0], None));
    }
}
