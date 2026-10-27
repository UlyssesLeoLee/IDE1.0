//! 单行内的文字编辑: 删字符 / 删词 / 删到行首行末 / 键入 / 选词。语义按真实 Sakura 比对 (tests/sakura-diff)。

use super::ctx::Ctx;
use super::req_res::EditRes;
use crate::word::{word_left, word_right, word_run};
use crate::text_util::slice;

/// Backspace: 有选区删选区; 否则删光标前一个字符, 行首则并到上一行
pub(super) fn backspace(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if let Some(s) = cx.sel {
        cx.delete_sel(s)
    } else if col > 0 {
        let new = format!("{}{}", slice(&cx.line, 0, col - 1), slice(&cx.line, col, usize::MAX));
        EditRes::replace_line(row, new, [row, col - 1])
    } else if row > 0 {
        cx.join_into_prev(false)
    } else {
        cx.nothing("")
    }
}

/// Delete: 有选区删选区; 否则删光标后一个字符, 行末则把下一行并上来 (光标不动)
pub(super) fn delete(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if let Some(s) = cx.sel {
        cx.delete_sel(s)
    } else if col < cx.line.len() {
        let new = format!("{}{}", slice(&cx.line, 0, col), slice(&cx.line, col + 1, usize::MAX));
        EditRes::replace_line(row, new, [row, col])
    } else if row < cx.last_row {
        cx.join_next(row, false).cursor([row, col])
    } else {
        cx.nothing("")
    }
}

/// 删到词首: 行首 = 并到上一行
pub(super) fn delete_word_start(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if col == 0 {
        if row == 0 {
            return cx.nothing("光标已在词首, 无可删内容");
        }
        return cx.join_into_prev(false);
    }
    let to = word_left(&cx.line, col);
    if to >= col {
        return cx.nothing("");
    }
    EditRes::replace_line(row, format!("{}{}", slice(&cx.line, 0, to), slice(&cx.line, col, usize::MAX)), [row, to])
}

/// 删到词尾: 行末 = 并入下一行并吃掉它开头的空白 (到下一个词首)
pub(super) fn delete_word_end(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if col >= cx.line.len() {
        if row >= cx.last_row {
            return cx.nothing("光标已在词尾, 无可删内容");
        }
        return cx.join_next(row, true);
    }
    let to = word_right(&cx.line, col);
    if to <= col {
        return cx.nothing("");
    }
    EditRes::replace_line(row, format!("{}{}", slice(&cx.line, 0, col), slice(&cx.line, to, usize::MAX)), [row, col])
}

/// 删 / 剪切光标所在字符种类的连续段 (漢字 / かな / 英数 / 記号 / 空白 各成一段); 行末 = 删掉换行。撤销后光标回到词尾
pub(super) fn word_cut(cx: &Ctx, cut: bool) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if col >= cx.line.len() {
        // Sakura: 行末的词删除 = 删掉换行
        return if row < cx.last_row { cx.join_next(row, false).cursor([row, col]) } else { cx.nothing("") };
    }
    let (i, j) = word_run(&cx.line, col);
    let removed = slice(&cx.line, i, j);
    EditRes::replace_line(row, format!("{}{}", slice(&cx.line, 0, i), slice(&cx.line, j, usize::MAX)), [row, i])
        .clip(cut.then_some(removed))
        .undo_cursor([row, j])
}

/// 删 / 剪切到行首
pub(super) fn line_to_start(cx: &Ctx, cut: bool) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if let Some(s) = cx.sel {
        return if cut { cx.cut_sel(s) } else { cx.delete_sel(s) };
    }
    if col == 0 {
        return cx.nothing("光标已在行首, 无可删内容");
    }
    EditRes::replace_line(row, slice(&cx.line, col, usize::MAX), [row, 0]).clip(cut.then(|| slice(&cx.line, 0, col)))
}

/// 删到行末 (有选区只删选区)
pub(super) fn delete_to_line_end(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if let Some(s) = cx.sel {
        return cx.delete_sel(s);
    }
    if col >= cx.line.len() {
        return cx.nothing("光标已在行末, 无可删内容");
    }
    EditRes::replace_line(row, slice(&cx.line, 0, col), [row, col])
}

/// 剪切到行末; Sakura: 行末再按什么也不做 (不吃换行)
pub(super) fn cut_to_line_end(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if let Some(s) = cx.sel {
        return cx.cut_sel(s);
    }
    if col >= cx.line.len() {
        return cx.nothing("光标已在行末, 无可剪内容");
    }
    EditRes::replace_line(row, slice(&cx.line, 0, col), [row, col]).clip(Some(slice(&cx.line, col, usize::MAX)))
}

/// 键入文字 (键盘上的可见字符 / TAB / 输入法上屏): 有选区则替换选区; 覆盖模式吃掉光标后同样个数的字符
pub(super) fn insert_text(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    let text = cx.req.text.clone().unwrap_or_default();
    if text.is_empty() {
        return cx.nothing("");
    }
    let n = text.chars().count();
    if let Some(s) = cx.sel {
        let head = slice(&cx.line(s[0]), 0, s[1]);
        let tail = slice(&cx.line(s[2]), s[3], usize::MAX);
        return EditRes::splice(s[0], s[2] - s[0] + 1, vec![format!("{head}{text}{tail}")], [s[0], s[1] + n]);
    }
    let skip = if cx.req.overwrite { n.min(cx.line.len().saturating_sub(col)) } else { 0 };
    let new = format!("{}{}{}", slice(&cx.line, 0, col), text, slice(&cx.line, col + skip, usize::MAX));
    EditRes::replace_line(row, new, [row, col + n])
}

/// 选词 (Sakura S_SelectWord): 光标所在字符类的连续段; 行末 = 选整行 (含换行)
pub(super) fn select_word(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    if col >= cx.line.len() {
        if cx.line.is_empty() {
            return cx.nothing("");
        }
        let (sel, cursor) = if row < cx.last_row { ([row, 0, row + 1, 0], [row + 1, 0]) } else { ([row, 0, row, cx.line.len()], [row, cx.line.len()]) };
        return EditRes::selecting(sel, [row, 0], cursor);
    }
    let (i, j) = word_run(&cx.line, col);
    EditRes::selecting([row, i, row, j], [row, i], [row, j])
}
