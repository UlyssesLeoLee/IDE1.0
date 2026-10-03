//! ide-shell-desktop — Tauri 2 桌面应用 entry.
//!
//! 复用 crates/ide-shell 的 App 状态机(38 UT 共享),通过 Tauri command 暴露
//! key/mouse/frame/reset 操作给 webview 前端。前端与 crates/ide-shell-web 共用
//! 同份 index.html, JS 端通过 window.__TAURI__ 检测自动选 invoke/fetch.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ide_shell_desktop::run();
}
