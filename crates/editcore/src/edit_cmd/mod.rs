//! 编辑命令 (ARCH-01: 编辑语义放 Rust)。
//!
//! 无状态: 前端交上光标周围需要的几行 (`rows`) 和选区, 这里算出"把第 `start` 行起的 `remove` 行换成 `insert`"
//! 以及新的光标 / 要写进剪贴板的文字 ([`EditRes`])。语义按真实 Sakura Editor 的键盘宏逐条比对 (tests/sakura-diff)。
//!
//! 按命令族分文件, [`dispatch`] 是唯一的命令表 (命令集是封闭的, 编译器检查每个分支, 不用动态注册):
//!
//! | 文件 | 命令 |
//! |---|---|
//! | `text_ops` | backspace delete delete-word-start/end word-cut word-delete cut-line-start delete-line-to-start delete-line-to-end line-cut-to-end insert-text select-word |
//! | `line_ops` | cut-line delete-line delete-line-up/down join-with-prev move-line-up/down duplicate-line add-line-above/below new-line |
//! | `clipboard` | copy cut paste paste-box copy-lines copy-passage copy-linenum copy-crlf |
//! | `box_ops` | box-insert box-enter box-backspace box-delete box-cut box-copy box-replace |
//! | `multi_cursor` | multi-insert multi-backspace |
//! | `fold` | fold-range fold-all |
//! | `md_ops` | md-bold md-italic md-strike md-code md-highlight md-h1..h6 md-quote md-ul md-ol md-task md-link md-image md-codeblock md-mermaid md-table md-hr md-callout |
//!
//! 共用: `req_res` (请求 / 响应 DTO 与响应构造器), `ctx` (执行环境: 预先算好的光标行 / 列 / 选区与取行)。

mod box_ops;
mod clipboard;
mod ctx;
mod fold;
mod line_ops;
mod md_ops;
mod multi_cursor;
mod req_res;
mod text_ops;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use req_res::{EditReq, EditRes, RowPatch};

use clipboard::CopyStyle;
use ctx::Ctx;
use multi_cursor::MultiOp;

/// 执行一条编辑命令. 未知命令返回 None.
pub fn run(req: &EditReq) -> Option<EditRes> {
    dispatch(&Ctx::new(req))
}

fn dispatch(cx: &Ctx) -> Option<EditRes> {
    Some(match cx.req.cmd.as_str() {
        // 单行文字
        "backspace" => text_ops::backspace(cx),
        "delete" => text_ops::delete(cx),
        "delete-word-start" => text_ops::delete_word_start(cx),
        "delete-word-end" => text_ops::delete_word_end(cx),
        "word-cut" => text_ops::word_cut(cx, true),
        "word-delete" => text_ops::word_cut(cx, false),
        "cut-line-start" => text_ops::line_to_start(cx, true),
        "delete-line-to-start" => text_ops::line_to_start(cx, false),
        "delete-line-to-end" => text_ops::delete_to_line_end(cx),
        "line-cut-to-end" => text_ops::cut_to_line_end(cx),
        "insert-text" => text_ops::insert_text(cx),
        "select-word" => text_ops::select_word(cx),
        // 整行
        "cut-line" => line_ops::cut_line(cx, true),
        "delete-line" => line_ops::cut_line(cx, false),
        "delete-line-up" => line_ops::delete_line_up(cx),
        "delete-line-down" => line_ops::delete_line_down(cx),
        "join-with-prev" => line_ops::join_with_prev(cx),
        "move-line-up" => line_ops::move_line(cx, true),
        "move-line-down" => line_ops::move_line(cx, false),
        "duplicate-line" => line_ops::duplicate_line(cx),
        "add-line-above" => line_ops::add_line(cx, true),
        "add-line-below" => line_ops::add_line(cx, false),
        "new-line" => line_ops::new_line(cx),
        // 剪贴板
        "copy" => clipboard::copy(cx),
        "cut" => clipboard::cut(cx),
        "paste" => clipboard::paste(cx),
        "paste-box" => clipboard::paste_box(cx),
        "copy-lines" => clipboard::copy_lines(cx, CopyStyle::Lines),
        "copy-passage" => clipboard::copy_lines(cx, CopyStyle::Passage),
        "copy-linenum" => clipboard::copy_lines(cx, CopyStyle::LineNumbers),
        "copy-crlf" => clipboard::copy_crlf(cx),
        // 矩形(列块)选区
        "box-insert" => box_ops::insert(cx),
        "box-enter" => box_ops::enter(cx),
        "box-backspace" | "box-delete" => box_ops::delete(cx),
        "box-cut" => box_ops::cut(cx),
        "box-copy" => box_ops::copy(cx),
        "box-replace" => box_ops::replace(cx),
        // 多光标
        "multi-insert" => {
            let ins: Vec<char> = cx.req.text.clone().unwrap_or_default().chars().collect();
            multi_cursor::run(cx, MultiOp::Insert(&ins))
        }
        "multi-backspace" => multi_cursor::run(cx, MultiOp::Backspace),
        // 折叠
        "fold-range" => fold::run(cx, false),
        "fold-all" => fold::run(cx, true),
        // Markdown 便捷编辑 (md-*); 不是 md 命令就是未知命令
        _ => return md_ops::run(cx),
    })
}

/// 所有认得的命令名 (测试用: 保证命令表和这份清单一致, 防止写错名字或漏分支)
#[cfg(test)]
pub(crate) const ALL_COMMANDS: &[&str] = &[
    "backspace", "delete", "delete-word-start", "delete-word-end", "word-cut", "word-delete", "cut-line-start", "delete-line-to-start", "delete-line-to-end", "line-cut-to-end", "insert-text", "select-word",
    "cut-line", "delete-line", "delete-line-up", "delete-line-down", "join-with-prev", "move-line-up", "move-line-down", "duplicate-line", "add-line-above", "add-line-below", "new-line",
    "copy", "cut", "paste", "paste-box", "copy-lines", "copy-passage", "copy-linenum", "copy-crlf",
    "box-insert", "box-enter", "box-backspace", "box-delete", "box-cut", "box-copy", "box-replace",
    "multi-insert", "multi-backspace", "fold-range", "fold-all",
];
