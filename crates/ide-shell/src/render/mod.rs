//! 渲染抽象 — 提供两个 backend:
//! - `ratatui` (现有, TTY 终端)
//! - `web` (新, 返回结构化 `WebFrame`,供 Playwright e2e / web 渲染层使用)
//!
//! 两者共享同一份 `&App` 数据,保证 visual parity (per ULYS-191 §3 双外观)。

pub mod ratatui_render;
pub mod web;

pub use web::{WebCell, WebFrame, WebStyle};
