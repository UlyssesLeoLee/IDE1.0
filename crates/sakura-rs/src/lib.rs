//! sakura-rs — 轻量高性能的文本编辑器内核.
//!
//! 对标 [sakura editor](https://github.com/sakura-editor/sakura) (C++/Win32/zlib) 架构的 Rust 重写.
//!
//! ## 设计目标
//! - **轻量**: 0 重型依赖 (无 regex crate, 无 tree-sitter WASM). 编译后 ~100KB 体积.
//! - **高性能**: 行级 token 化 (非全文), Boyer-Moore-Horspool grep, O(1) undo/redo cursor.
//! - **可集成**: 暴露 `Buffer` + `OpeBuf` + `TypeRegistry` + `Grep`, AI agent / LSP / Web 工具都能用.
//!
//! ## 模块
//! - [`buffer`]: `DocLine` + `DocLineMgr` (逻辑行/物理行, char-based cursor).
//! - [`undo`]: `Ope` + `OpeBlk` + `OpeBuf` (Insert/Delete/Replace/MoveCaret, JSON 序列化).
//! - [`type_config`]: `TypeConfig` + `TokenKind` + `TypeRegistry` (20+ 语言).
//! - [`types`]: 各语言工厂函数 (Rust / Python / C++ / JS / TS / Go / Java / C# / Ruby / Bash / HTML / CSS / JSON / MD / YAML / TOML / SQL / Dockerfile / Makefile).
//! - [`grep`]: 文件枚举 (`walk_dir`) + 内容搜索 (`grep_in_files`).
//! - [`cursor`]: `LogicPos` + `LogicRange` (对标 sakura `CLogicPoint`).
//!
//! ## 集成示例
//!
//! ```rust,no_run
//! use sakura_rs::buffer::DocLineMgr;
//! use sakura_rs::type_config::TypeRegistry;
//!
//! let mut mgr = DocLineMgr::new();
//! mgr.insert_str(sakura_rs::cursor::LogicPos::ZERO, "hello world\n");
//!
//! let reg = TypeRegistry::default();
//! let lang = reg.detect("foo.py").unwrap();
//! let tokens = lang.highlight_line("def foo():");
//! ```
//!
//! ## License
//! 沿用上游 sakura editor 的 Zlib 协议 (permissive, 商业友好).

#![forbid(unsafe_code)]

pub mod buffer;
pub mod cursor;
pub mod grep;
pub mod type_config;
pub mod types;
pub mod undo;

// Phase 1: 高優先 7 件 (ulys-191-36)
pub mod auto_backup;
pub mod control_code;
pub mod cursor_persistence;
pub mod incremental_search;
pub mod matching_paren;
pub mod syntax_styling;
pub mod zenkaku_hankaku;

// Phase 2: IO + Migemo 5 件 (ulys-191-37)
pub mod encoding_io;
pub mod file_lock;
pub mod migemo;
pub mod keyword_set;
pub mod outline_extended;

/// 当前 crate 版本 + meta 信息 (对标 sakura 的 `SakuraVersion`).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 简洁的内核摘要 — 用于日志 / debug.
#[derive(Debug, Clone)]
pub struct KernelInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub api: Vec<&'static str>,
}

impl KernelInfo {
    pub fn current() -> Self {
        Self {
            name: "sakura-rs",
            version: env!("CARGO_PKG_VERSION"),
            api: vec![
                "buffer: insert_str / remove_range / backspace / delete / split_line / extract_text",
                "undo: Ope / OpeBlk / OpeBuf / push / undo / redo / history_json / from_json",
                "type_config: detect / highlight_line / outline",
                "grep: walk_dir / grep_in_file / grep_in_files / filter_by_ext",
                "cursor: LogicPos / LogicRange",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn kernel_info() {
        let info = KernelInfo::current();
        assert_eq!(info.name, "sakura-rs");
        assert!(info.api.len() >= 5);
    }
}
