//! 守卫: 网页版内嵌的前端 (src/editor.html) 基本结构完整性校验。

#[test]
fn embedded_editor_html_is_valid() {
    let web = include_str!("../src/editor.html");
    assert!(!web.trim().is_empty(), "crates/ide-shell-web/src/editor.html 不应为空");
    assert!(
        web.contains("<!DOCTYPE html>") || web.contains("<!doctype html>"),
        "crates/ide-shell-web/src/editor.html 应包含 HTML 文档声明"
    );
}
