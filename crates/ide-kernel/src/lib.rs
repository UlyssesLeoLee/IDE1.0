//! `ide-kernel` — IDE1.0 唯一编辑内核.
//!
//! # 为什么存在
//!
//! 修复前 IDE1.0 有**两个互不相通的编辑引擎**:
//!
//! - 前端 JS 引擎 (`editor.html`, ≈4400 行): 117 个命令、语法高亮、多光标、
//!   列块选择、自动补全、宏、书签 —— 产品实际用的全部功能在这;
//! - `sakura-rs` Rust 库 (6058 行 / 245 pub fn / 161 测试): Buffer / Undo /
//!   Cursor / Grep / Encoding 齐全, 但**产品主路径没在用**。
//!
//! 结果: 同一产品三个外壳 (Web / Tauri / TUI) 编辑语义不一致,
//! 而 sakura-rs 的数百个测试保护的是一个没人调用的库。
//!
//! # 本 crate 的职责
//!
//! 把 `sakura-rs` 里已经验证过的部件聚合成**一个会话级编辑内核**:
//! 内容 + 光标 + undo/redo + 选区 + 多光标 + 查找替换 + 编码 + 行尾符,
//! 并通过 `Document` 这一唯一入口对外暴露。
//!
//! 三个外壳 (Web HTTP / Tauri command / TUI) 都只做**渲染**,
//! 所有编辑语义走这里 —— 这才是「单一事实来源」。
//!
//! # 分层
//!
//! ```text
//!   外壳 (web / desktop / tui)   ← 只渲染 + 转发按键
//!        ↓
//!   ide_kernel::Session          ← 会话: 多标签 + 窗口状态
//!        ↓
//!   ide_kernel::Document         ← 单文档: 本 crate 的核心
//!        ↓
//!   sakura_rs::{DocLineMgr, OpeBuf, LogicPos}   ← 已验证的基础部件
//! ```
//!
//! # 设计约束
//!
//! - **不引入新依赖**。只用 `sakura-rs` 已有依赖 (serde / serde_json)。
//! - **零 unsafe** (`#![forbid(unsafe_code)]`, 与 sakura-rs 一致)。
//! - **所有编辑走 undo**: 每个修改点必须经 `Document::apply`, 否则 undo 栈失真。
//! - **char 语义**: 行列按 char 计数, 与 `LogicPos` / `DocLineMgr` 一致,
//!   不按 byte (否则多字节字符编辑会错位 —— 日语编辑器的核心承诺)。

#![forbid(unsafe_code)]

pub mod doc_store;
pub mod document;
pub mod session;
pub mod text_ops;

pub use doc_store::{DocStore, DocView};
pub use document::{Document, DocumentSnapshot, LineEnding, Pos};
pub use session::{Session, TabState};
pub use text_ops::{FindOptions, FindResult, ReplaceOutcome};

/// 内核版本 —— 前端/外壳用于确认协议兼容.
pub const KERNEL_VERSION: &str = env!("CARGO_PKG_VERSION");
