//! Plugin service — sakura-rs wasm 插件加载与原生插件包导入 (desktop).
//!
//! 与 web 版 `POST /api/plugin_load` 语义一致: 传入 wasm 路径与输入文本,
//! 返回 `{ ok, output }` 或 `{ ok: false, error }`.
//! 同时提供系统原生文件选择器 `native_pick_plugin_zip`，支持多选插件压缩包 (.zip) 导入.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickedZip {
    pub name: String,
    pub path: String,
    pub data: String, // Base64 编码的压缩包内容
}

/// 纯标准库 Base64 编码器 (避免额外依赖)
pub fn b64_encode(bytes: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARSET[(triple >> 18) & 0x3F] as char);
        out.push(CHARSET[(triple >> 12) & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(triple >> 6) & 0x3F] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[triple & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// 系统原生「选择插件压缩包」对话框 (支持多选 .zip，返回文件名及 base64 内容)
#[tauri::command]
pub fn native_pick_plugin_zip() -> Result<Vec<PickedZip>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<Vec<PathBuf>>>();
    std::thread::spawn(move || {
        let picked = rfd::FileDialog::new()
            .set_title("选择插件压缩包 (.zip)")
            .add_filter("插件压缩包 (*.zip)", &["zip"])
            .pick_files();
        let _ = tx.send(picked);
    });
    let picked = rx
        .recv_timeout(std::time::Duration::from_secs(3600))
        .ok()
        .flatten()
        .unwrap_or_default();

    let mut out = Vec::new();
    for p in picked {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(&p)
            .map_err(|e| format!("读取文件失败 {}: {e}", p.display()))?;
        out.push(PickedZip {
            name,
            path: p.to_string_lossy().into_owned(),
            data: b64_encode(&bytes),
        });
    }
    Ok(out)
}

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
    match editcore::plugin::load_and_run(std::path::Path::new(wasm_path), input) {
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
