//! IDE1.0 IDE Shell — TUI + AI + Vim + 鼠标 (per ULYS-191 §3).
//!
//! 架构 (Stage 3.0 brief v0.1):
//! - `mode`        Vim normal/insert/command 模式状态机
//! - `input`       编辑缓冲 (Rope-like 简化为 Vec<char>)
//! - `keymap`      键位 → 动作 (KeyEvent → Action)
//! - `ai`          AI bridge trait + Placeholder impl
//! - `render`      ratatui 渲染 (顶部 status + 中部 buffer + 底部 prompt)
//! - `app`         顶层 App 状态 (拼装上面)
//!
//! 复用 IDE1.0 现有 kernel (ide-kernel-core 提供 version / kernel_status / init).

pub mod ai;
pub mod app;
pub mod input;
pub mod keymap;
pub mod mode;
pub mod render;

pub use app::{App, AppConfig};
pub use mode::Mode;
