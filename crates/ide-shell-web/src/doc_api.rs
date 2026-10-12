//! Web ↔ 内核文档会话的薄适配层.
//!
//! 实现已移到 `ide_kernel::doc_store` —— 那是产品逻辑, 不是传输细节;
//! Tauri 桌面版需要同一份, 放在 web crate 里会导致两份实现分叉.
//!
//! 本文件只做两件事:
//! 1. 重导出内核类型, 让 web crate 与既有引用兼容;
//! 2. 提供 JSON 位置解析 (HTTP 边界关注点)。

pub use ide_kernel::doc_store::{DocStore, DocView, FindHit, ReplaceStat};

use serde_json::Value;

/// 从 JSON 取 (row, col), 缺省 0.
pub fn doc_pos(req: &Value) -> (i32, i32) {
    (
        req.get("row").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        req.get("col").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
    )
}
