//! Tauri ↔ 内核文档会话的 command 适配层.
//!
//! 与 `ide-shell-web/src/doc_api.rs` 结构完全对称 —— 同一个
//! `ide_kernel::DocStore`, 只是传输方式从 HTTP/JSON 换成 Tauri IPC.
//! 两端共用内核实现, 不存在"桌面版和网页版行为不一致"的可能.
//!
//! # 为什么需要它
//!
//! 桌面版前端 (`dist/index.html`, 与 web 版同源的 `editor.html`) 目前
//! 仍用 `read_file`/`write_file` + 前端自管 buffer. 本模块提供同名的
//! 状态化路径, 让前端能逐步把编辑语义交给 Rust.
//!
//! # 命令命名
//!
//! 全部 `doc_*` 前缀, 避免与既有 `read_file` / `write_file` 混淆.
//! Tauri 侧 snake_case → 前端 `invoke('doc_open', {...})`.

use ide_kernel::doc_store::{DocStore, DocView, FindHit, ReplaceStat};
use serde::Serialize;
use std::sync::Mutex;

/// 文档会话状态 —— 由 `run()` 注册进 Tauri.
#[derive(Default)]
pub struct DocState(pub Mutex<DocStore>);

/// 统一响应形状 —— 与 web 的 `{ok, result}` 对齐, 便于前端共用解析代码.
#[derive(Debug, Serialize)]
pub struct DocResp<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> DocResp<T> {
    fn some(v: T) -> Self {
        Self {
            ok: true,
            result: Some(v),
            error: None,
        }
    }
    fn none() -> Self {
        Self {
            ok: false,
            result: None,
            error: Some("no active document".into()),
        }
    }
}

type R<T> = Result<DocResp<T>, String>;

fn view<T: Serialize>(store: &Mutex<DocStore>, f: impl FnOnce(&mut DocStore) -> Option<T>) -> R<T> {
    let mut g = store.lock().map_err(|e| e.to_string())?;
    Ok(match f(&mut g) {
        Some(v) => DocResp::some(v),
        None => DocResp::none(),
    })
}

/// 活动文档视图 (无文档时 ok=false).
#[tauri::command]
pub fn doc_active(state: tauri::State<'_, DocState>) -> R<DocView> {
    let g = state.0.lock().map_err(|e| e.to_string())?;
    Ok(match g.active() {
        Some(v) => DocResp::some(v),
        None => DocResp::none(),
    })
}

/// 全部标签.
#[tauri::command]
pub fn doc_tabs(state: tauri::State<'_, DocState>) -> R<Vec<DocView>> {
    let g = state.0.lock().map_err(|e| e.to_string())?;
    Ok(DocResp::some(g.tabs()))
}

/// 打开文本为新标签.
#[tauri::command]
pub fn doc_open(
    state: tauri::State<'_, DocState>,
    path: Option<String>,
    text: String,
) -> R<DocView> {
    let mut g = state.0.lock().map_err(|e| e.to_string())?;
    Ok(DocResp::some(g.open(path.as_deref(), &text)))
}

/// 新建空白标签.
#[tauri::command]
pub fn doc_new_tab(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.new_tab())
}

/// 切换标签.
#[tauri::command]
pub fn doc_activate(state: tauri::State<'_, DocState>, id: u64) -> R<DocView> {
    view(&state.0, |d| d.activate(id))
}

/// 关闭标签.
#[tauri::command]
pub fn doc_close(state: tauri::State<'_, DocState>, id: u64) -> R<DocView> {
    view(&state.0, |d| d.close(id))
}

/// 在 (row,col) 插入文本.
#[tauri::command]
pub fn doc_insert(
    state: tauri::State<'_, DocState>,
    row: i32,
    col: i32,
    text: String,
) -> R<DocView> {
    view(&state.0, |d| d.insert(row, col, &text))
}

/// 输入一个字符 (有选区则替换).
#[tauri::command]
pub fn doc_type(state: tauri::State<'_, DocState>, row: i32, col: i32, ch: String) -> R<DocView> {
    let c = ch.chars().next().unwrap_or(' ');
    view(&state.0, |d| d.type_char(row, col, c))
}

#[tauri::command]
pub fn doc_newline(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.insert_newline())
}

#[tauri::command]
pub fn doc_backspace(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.backspace())
}

#[tauri::command]
pub fn doc_delete(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.delete_forward())
}

#[tauri::command]
pub fn doc_delete_range(
    state: tauri::State<'_, DocState>,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
) -> R<DocView> {
    view(&state.0, |d| {
        d.delete_range(start_row, start_col, end_row, end_col)
    })
}

#[tauri::command]
pub fn doc_caret(state: tauri::State<'_, DocState>, row: i32, col: i32) -> R<DocView> {
    view(&state.0, |d| d.set_caret(row, col))
}

#[tauri::command]
pub fn doc_select_all(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.select_all())
}

#[tauri::command]
pub fn doc_clear_selection(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.clear_selection())
}

#[tauri::command]
pub fn doc_undo(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.undo())
}

#[tauri::command]
pub fn doc_redo(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.redo())
}

#[tauri::command]
pub fn doc_add_cursor(state: tauri::State<'_, DocState>, row: i32, col: i32) -> R<DocView> {
    view(&state.0, |d| d.add_cursor(row, col))
}

#[tauri::command]
pub fn doc_clear_cursors(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.clear_cursors())
}

#[tauri::command]
pub fn doc_insert_all(state: tauri::State<'_, DocState>, text: String) -> R<DocView> {
    view(&state.0, |d| d.insert_all_cursors(&text))
}

#[tauri::command]
pub fn doc_find(
    state: tauri::State<'_, DocState>,
    pattern: String,
    case_sensitive: bool,
    row: i32,
    col: i32,
) -> R<FindHit> {
    view(&state.0, |d| d.find(&pattern, case_sensitive, row, col))
}

#[tauri::command]
pub fn doc_find_all(
    state: tauri::State<'_, DocState>,
    pattern: String,
    case_sensitive: bool,
    limit: usize,
) -> R<Vec<FindHit>> {
    view(&state.0, |d| d.find_all(&pattern, case_sensitive, limit))
}

#[tauri::command]
pub fn doc_replace_all(
    state: tauri::State<'_, DocState>,
    pattern: String,
    replacement: String,
    case_sensitive: bool,
    limit: usize,
) -> R<ReplaceStat> {
    view(&state.0, |d| {
        d.replace_all(&pattern, &replacement, case_sensitive, limit)
    })
}

#[tauri::command]
pub fn doc_line_ending(state: tauri::State<'_, DocState>, line_ending: String) -> R<DocView> {
    use ide_kernel::document::LineEnding;
    let eol = match line_ending.to_ascii_lowercase().as_str() {
        "crlf" | "windows (crlf)" => Some(LineEnding::Crlf),
        "cr" | "mac (cr)" => Some(LineEnding::Cr),
        "lf" | "unix (lf)" => Some(LineEnding::Lf),
        _ => None,
    };
    let Some(e) = eol else {
        return Ok(DocResp::none());
    };
    view(&state.0, |d| d.set_line_ending(e))
}

#[tauri::command]
pub fn doc_encoding(state: tauri::State<'_, DocState>, encoding: String) -> R<DocView> {
    use ide_kernel::document::Encoding;
    let Some(e) = Encoding::from_label(&encoding) else {
        return Ok(DocResp::none());
    };
    view(&state.0, |d| d.set_encoding(e))
}

/// 导出内容: (path 可选, 含正确行尾符的文本).
#[derive(Debug, Serialize)]
pub struct ExportView {
    pub path: Option<String>,
    pub text: String,
}

#[tauri::command]
pub fn doc_export(state: tauri::State<'_, DocState>) -> R<ExportView> {
    let g = state.0.lock().map_err(|e| e.to_string())?;
    Ok(match g.export() {
        Some((path, text)) => DocResp::some(ExportView { path, text }),
        None => DocResp::none(),
    })
}

#[tauri::command]
pub fn doc_mark_saved(state: tauri::State<'_, DocState>) -> R<DocView> {
    view(&state.0, |d| d.mark_saved())
}

/// 内核文档 command 清单 (供 `generate_handler!` 展开).
///
/// 不用宏_rules —— `tauri::generate_handler!` 内部有自己的宏展开顺序,
/// 宏嵌套会导致 "expected `,`". 这里导出成常量供人工同步, 测试会校验
/// `lib.rs` 的注册列表与本清单一致.
#[cfg(test)]
pub const COMMAND_NAMES: &[&str] = &[
    "doc_active",
    "doc_tabs",
    "doc_open",
    "doc_new_tab",
    "doc_activate",
    "doc_close",
    "doc_insert",
    "doc_type",
    "doc_newline",
    "doc_backspace",
    "doc_delete",
    "doc_delete_range",
    "doc_caret",
    "doc_select_all",
    "doc_clear_selection",
    "doc_undo",
    "doc_redo",
    "doc_add_cursor",
    "doc_clear_cursors",
    "doc_insert_all",
    "doc_find",
    "doc_find_all",
    "doc_replace_all",
    "doc_line_ending",
    "doc_encoding",
    "doc_export",
    "doc_mark_saved",
];
