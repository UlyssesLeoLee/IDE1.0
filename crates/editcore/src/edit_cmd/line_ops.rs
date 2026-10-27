//! 整行级的编辑: 剪切 / 删除整行、上下删行、并行、移行、复制行、加行、回车换行。

use super::ctx::Ctx;
use super::req_res::EditRes;
use crate::text_util::{indent_len, slice};

/// 删掉光标行之后该怎么拼: 只剩一行 = 清成空行; 否则整行拿掉, 删的是末行时光标上移。返回 (替换进去的行, 光标新行号)
pub(super) fn remove_current_row(cx: &Ctx) -> (Vec<String>, usize) {
    if cx.req.total_rows <= 1 {
        (vec![String::new()], 0)
    } else {
        (vec![], if cx.row == cx.last_row { cx.row - 1 } else { cx.row })
    }
}

/// 剪切 / 删除整行; 光标落到新的同一行位置的行首
pub(super) fn cut_line(cx: &Ctx, cut: bool) -> EditRes {
    let (insert, new_row) = remove_current_row(cx);
    EditRes::splice(cx.row, 1, insert, [new_row, 0]).clip(cut.then(|| cx.line_str()))
}

/// 删除上一行 (光标不动在原行, 实际上行号上移一格)
pub(super) fn delete_line_up(cx: &Ctx) -> EditRes {
    if cx.row == 0 {
        return cx.nothing("已在首行, 上方无可删");
    }
    let (insert, cur) = if cx.req.total_rows <= 1 { (vec![String::new()], [0, 0]) } else { (vec![], [cx.row - 1, 0]) };
    EditRes::splice(cx.row - 1, 1, insert, cur).tip("已删除上方行")
}

/// 删除下一行
pub(super) fn delete_line_down(cx: &Ctx) -> EditRes {
    if cx.row >= cx.last_row {
        return cx.nothing("已在末行, 下方无可删");
    }
    EditRes::splice(cx.row + 1, 1, vec![], [cx.row, 0]).tip("已删除下方行")
}

/// 把光标行并到上一行 (并入前去掉光标行开头的空白)
pub(super) fn join_with_prev(cx: &Ctx) -> EditRes {
    if cx.row == 0 {
        return cx.nothing("");
    }
    cx.join_into_prev(true)
}

/// 上移 / 下移一行 (与相邻行互换), 光标列不变
pub(super) fn move_line(cx: &Ctx, up: bool) -> EditRes {
    let row = cx.row;
    if (up && row == 0) || (!up && row >= cx.last_row) {
        return cx.nothing("");
    }
    let other_row = if up { row - 1 } else { row + 1 };
    let other: String = cx.line(other_row).iter().collect();
    let cur = cx.line_str();
    let (start, insert) = if up { (row - 1, vec![cur, other]) } else { (row, vec![other, cur]) };
    EditRes::splice(start, 2, insert, [other_row, cx.col]).tip(format!("移动行: Ln {} → Ln {}", row + 1, other_row + 1))
}

/// 复制当前行到下面一行
pub(super) fn duplicate_line(cx: &Ctx) -> EditRes {
    let text = cx.line_str();
    EditRes::splice(cx.row, 1, vec![text.clone(), text], [cx.row + 1, cx.col])
}

/// 在上方 / 下方加一个空行 (带当前行的缩进)
pub(super) fn add_line(cx: &Ctx, above: bool) -> EditRes {
    let indent = slice(&cx.line, 0, indent_len(&cx.line));
    let n = indent.chars().count();
    let at = if above { cx.row } else { cx.row + 1 };
    EditRes::splice(at, 0, vec![indent], [at, n])
}

/// 在 `col` 处把一行断成两行: 自动缩进沿用头部的缩进, 智能缩进在头部以 { [ ( 或空白结尾时再多一级。
/// 返回 (断开后的两行, 新行的光标列)。回车和矩形里的回车共用。
pub(super) fn split_line_at(line: &[char], col: usize, auto_indent: bool, smart_indent: bool) -> (Vec<String>, usize) {
    let head = slice(line, 0, col);
    let tail = slice(line, col, usize::MAX);
    let hc: Vec<char> = head.chars().collect();
    let indent = if auto_indent { slice(&hc, 0, indent_len(&hc)) } else { String::new() };
    let ends_open = hc.last().map(|c| matches!(c, '{' | '[' | '(') || c.is_whitespace()).unwrap_or(false);
    let extra = if auto_indent && smart_indent && ends_open { "  " } else { "" };
    let pad = format!("{indent}{extra}");
    let pad_len = pad.chars().count();
    (vec![head, format!("{pad}{tail}")], pad_len)
}

/// 回车
pub(super) fn new_line(cx: &Ctx) -> EditRes {
    let (insert, pad_len) = split_line_at(&cx.line, cx.col, cx.req.auto_indent, cx.req.smart_indent);
    EditRes::splice(cx.row, 1, insert, [cx.row + 1, pad_len])
}
