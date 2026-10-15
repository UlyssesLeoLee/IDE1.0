//! desktop ↔ ide-kernel 接入测试.
//!
//! 验证 Tauri command 层与 web 端共用同一份 `ide_kernel::DocStore`:
//! 两边的 `DocResp` 形状必须一致, 否则前端同一段解析代码会在某个平台上崩.
//!
//! 注: 这里测的是 command 背后的 `DocStore` 行为与响应形状 (command 本身需要
//! `tauri::State`, 在真实 app 里由 tests/uat/verify-desktop.mjs 经 CDP 覆盖).

use ide_kernel::doc_store::DocStore;

/// 模拟 Tauri 侧 `DocResp` 的序列化形状.
#[derive(serde::Serialize)]
struct Resp<T: serde::Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn ok_resp<T: serde::Serialize>(v: T) -> Resp<T> {
    Resp {
        ok: true,
        result: Some(v),
        error: None,
    }
}

#[test]
fn resp_shape_matches_web_contract() {
    // web 侧返回 {"ok":true,"result":{...}} —— 桌面必须一致
    let mut store = DocStore::new();
    let v = store.open(Some("a.txt"), "hello");
    let json = serde_json::to_string(&ok_resp(v)).unwrap();
    assert!(json.starts_with(r#"{"ok":true,"result":"#), "实际: {json}");
    assert!(json.contains(r#""line_count":1"#));
    assert!(json.contains(r#""dirty":false"#));
}

#[test]
fn resp_omits_error_when_ok() {
    let mut store = DocStore::new();
    let v = store.open(None, "x");
    let json = serde_json::to_string(&ok_resp(v)).unwrap();
    assert!(!json.contains("error"), "成功响应不应带 error 字段: {json}");
}

#[test]
fn doc_view_fields_present() {
    let mut s = DocStore::new();
    s.open(Some("a.txt"), "line1\nline2");
    let v = s.active().expect("应有活动文档");
    let json = serde_json::to_string(&v).unwrap();
    for field in [
        "id",
        "path",
        "text",
        "line_count",
        "caret_row",
        "caret_col",
        "line_ending",
        "encoding",
        "dirty",
        "has_selection",
        "selected_text",
        "cursor_count",
        "can_undo",
        "can_redo",
    ] {
        assert!(
            json.contains(&format!("\"{field}\"")),
            "DocView 缺字段 {field}"
        );
    }
}

#[test]
fn desktop_and_web_share_same_store_semantics() {
    // 同一串操作, 两端结果必须一致 —— 因为它们就是同一个 DocStore
    let mut s = DocStore::new();
    s.open(None, "ac");
    s.set_caret(0, 1);
    s.insert(0, 1, "b");
    s.undo();
    s.redo();
    let v = s.active().unwrap();
    assert_eq!(v.text, "abc");
    assert!(v.can_undo);
    assert!(v.dirty);
}

#[test]
fn multibyte_positions_char_based() {
    let mut s = DocStore::new();
    s.open(None, "日本語");
    let v = s.insert(0, 2, "X").expect("应插入");
    assert_eq!(v.text, "日本X語");
}

#[test]
fn crlf_preserved_on_export() {
    let mut s = DocStore::new();
    s.open(Some("crlf.txt"), "a\r\nb");
    let (path, text) = s.export().expect("应能导出");
    assert_eq!(text, "a\r\nb");
    assert_eq!(path.as_deref(), Some("crlf.txt"));
}

#[test]
fn multi_cursor_count_and_batch_insert() {
    let mut s = DocStore::new();
    s.open(None, "a\nb\nc");
    s.add_cursor(1, 0);
    s.add_cursor(2, 0);
    assert_eq!(s.active().unwrap().cursor_count, 3);
    let v = s.insert_all_cursors("X").expect("应插入");
    assert_eq!(v.text, "Xa\nXb\nXc");
}
