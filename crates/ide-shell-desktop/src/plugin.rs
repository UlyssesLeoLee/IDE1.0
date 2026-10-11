//! Plugin service — sakura-rs wasm 插件加载 (desktop).
//!
//! 与 web 版 `POST /api/plugin_load` 语义一致: 传入 wasm 路径与输入文本,
//! 返回 `{ ok, output }` 或 `{ ok: false, error }`.
//! 未启用 `wasm` Cargo feature 时如实返回可读的失败原因, 不静默.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRunResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 加载并运行一个 wasm 插件 (需导出 `process(text: &str) -> String`)
pub fn load_and_run(wasm_path: &str, input: &str) -> PluginRunResult {
    match sakura_rs::plugin::load_and_run(std::path::Path::new(wasm_path), input) {
        Ok(out) => PluginRunResult {
            ok: true,
            output: Some(out),
            error: None,
        },
        Err(e) => PluginRunResult {
            ok: false,
            output: None,
            error: Some(e.to_string()),
        },
    }
}

// ============================================================
// Tauri command wrapper
// ============================================================

#[tauri::command]
pub fn plugin_load(path: String, input: String) -> PluginRunResult {
    load_and_run(&path, &input)
}