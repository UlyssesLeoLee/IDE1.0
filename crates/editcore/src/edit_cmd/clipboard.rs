//! 剪贴板命令: 复制 / 剪切 / 粘贴 / 矩形贴付 / 带行号等复制变体。

use super::ctx::Ctx;
use super::line_ops::remove_current_row;
use super::req_res::EditRes;
use crate::text_util::slice;

/// 把前端发来的剪贴板文字换行统一成 \n
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 复制: 有选区复制选区; 否则复制整行 (整行模式, 再粘贴时插在当前行之前)
pub(super) fn copy(cx: &Ctx) -> EditRes {
    let cursor = [cx.row, cx.col];
    match cx.sel {
        Some(s) => {
            let t = cx.sel_text(s);
            let n = t.chars().count();
            EditRes::clip_only(t, cursor).tip(format!("已复制 {n} 字符"))
        }
        None => EditRes::clip_only(format!("{}\n", cx.line_str()), cursor).clip_line().tip("已复制行"),
    }
}

/// 剪切: 有选区剪切选区; 否则剪切整行, 光标列保留 (落到接上来的那一行的同一列)
pub(super) fn cut(cx: &Ctx) -> EditRes {
    if let Some(s) = cx.sel {
        return cx.cut_sel(s).tip("已剪切");
    }
    let (insert, new_row) = remove_current_row(cx);
    // Sakura: 接上来的那一行 = 原来的下一行; 剪的是末行时为上一行
    let below = if cx.req.total_rows <= 1 {
        0
    } else if cx.row == cx.last_row {
        cx.line(cx.row - 1).len()
    } else {
        cx.line(cx.row + 1).len()
    };
    EditRes::splice(cx.row, 1, insert, [new_row, cx.col.min(below)]).clip(Some(format!("{}\n", cx.line_str()))).clip_line().tip("已剪切行")
}

/// 粘贴: 整行模式的剪贴板插在当前行之前 (有选区时先删选区, 插在选区起点那一行之前; Sakura 实测); 否则替换选区 / 插在光标处
pub(super) fn paste(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    let text = normalize_newlines(&cx.req.text.clone().unwrap_or_default());
    if text.is_empty() {
        return cx.nothing("剪贴板为空");
    }
    if cx.req.line_mode && text.ends_with('\n') {
        let mut rows: Vec<String> = text[..text.len() - 1].split('\n').map(String::from).collect();
        let n = rows.len();
        return match cx.sel {
            None => EditRes::splice(row, 0, rows, [row + n, col]).tip("已粘贴行"),
            Some(s) => {
                let joined = format!("{}{}", slice(&cx.line(s[0]), 0, s[1]), slice(&cx.line(s[2]), s[3], usize::MAX));
                rows.push(joined);
                EditRes::splice(s[0], s[2] - s[0] + 1, rows, [s[0] + n, s[1]]).tip("已粘贴行")
            }
        };
    }
    let (r0, r1, head, tail) = match cx.sel {
        Some(s) => (s[0], s[2], slice(&cx.line(s[0]), 0, s[1]), slice(&cx.line(s[2]), s[3], usize::MAX)),
        None => (row, row, slice(&cx.line, 0, col), slice(&cx.line, col, usize::MAX)),
    };
    let parts: Vec<&str> = text.split('\n').collect();
    let last = parts.len() - 1;
    let (insert, cursor) = if last == 0 {
        (vec![format!("{head}{}{tail}", parts[0])], [r0, head.chars().count() + parts[0].chars().count()])
    } else {
        let mut insert = vec![format!("{head}{}", parts[0])];
        insert.extend(parts[1..last].iter().map(|p| (*p).to_string()));
        insert.push(format!("{}{tail}", parts[last]));
        (insert, [r0 + last, parts[last].chars().count()])
    };
    EditRes::splice(r0, r1 - r0 + 1, insert, cursor).tip(format!("已粘贴 {} 字符", text.chars().count()))
}

/// 矩形贴付: 每行贴到 (光标行+i, 光标列); 行不够长补空格, 文档不够长补行
pub(super) fn paste_box(cx: &Ctx) -> EditRes {
    let (row, col) = (cx.row, cx.col);
    let text = normalize_newlines(&cx.req.text.clone().unwrap_or_default());
    let parts: Vec<&str> = text.split('\n').collect();
    if parts.len() == 1 && parts[0].is_empty() {
        return cx.nothing("剪贴板为空");
    }
    let insert: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut l = if row + i < cx.req.total_rows { cx.line(row + i) } else { Vec::new() };
            l.resize(l.len().max(col), ' ');
            format!("{}{}{}", slice(&l, 0, col), p, slice(&l, col, usize::MAX))
        })
        .collect();
    let remove = parts.len().min(cx.req.total_rows.saturating_sub(row));
    EditRes::splice(row, remove, insert, [row, col]).tip(format!("已矩形贴付 {} 行", parts.len()))
}

/// 复制变体: 每行原样 / 前缀 "> " (引用) / 带行号
#[derive(Clone, Copy)]
pub(super) enum CopyStyle {
    Lines,
    Passage,
    LineNumbers,
}

pub(super) fn copy_lines(cx: &Ctx, style: CopyStyle) -> EditRes {
    let (a, b) = cx.sel.map(|s| (s[0], s[2])).unwrap_or((cx.row, cx.row));
    let width = 3.max(cx.req.total_rows.to_string().len());
    let mut out = String::new();
    for r in a..=b {
        let l: String = cx.line(r).iter().collect();
        match style {
            CopyStyle::Lines => out.push_str(&format!("{l}\n")),
            CopyStyle::Passage => out.push_str(&format!("> {l}\n")),
            CopyStyle::LineNumbers => out.push_str(&format!("{:>w$}:{l}\n", r + 1, w = width)),
        }
    }
    EditRes::clip_only(out, [cx.row, cx.col]).tip("已复制")
}

/// 以 CRLF 换行复制选区 (无选区 = 当前行)
pub(super) fn copy_crlf(cx: &Ctx) -> EditRes {
    let t = match cx.sel {
        Some(s) => cx.sel_text(s),
        None => cx.line_str(),
    };
    EditRes::clip_only(t.replace('\n', "\r\n"), [cx.row, cx.col]).tip("已用 CRLF 换行复制")
}
