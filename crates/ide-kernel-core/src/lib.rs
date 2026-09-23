//! `ide-kernel-core` v0.1.0 — IDE1.0 microkernel core (placeholder).
//!
//! ⚠️ **PLACEHOLDER** — per ULYS-191 §4.2.2 brief v0.1, 本笔仅提供最小骨架.
//! 完整 microkernel / session / transaction buffer / search / plugin loader / TUI / txn
//! 实装待后续 brief (per DD-01 Microkernel Core 详细设计书 + DD-02 Session Transaction
//! Buffer Engine 详细设计书 + DD-04 Plugin Loader 详细设计书).
//!
//! ## 当前交付 (Stage 3.0 方案 A 最小骨架)
//!
//! - 提供 `version()` 函数, 返回 crate 版本号
//! - 0 运行时依赖, 0 unsafe block
//! - 完整 `cargo test --workspace --all-targets` 通过
//!
//! ## 后续 brief 范围
//!
//! - DD-01: microkernel core (process supervisor, signal handling, graceful shutdown)
//! - DD-02: session transaction buffer engine
//! - DD-03: search index
//! - DD-04: plugin loader + TUI

// README.md 不内嵌 crate docs (避免 cargo test --doc 把 README 的 Rust 代码块当 doctest).
// 完整 docs 见 https://docs.rs/ide-kernel-core (待 crates.io publish) 或仓根 README.md.
#![cfg_attr(docsrs, feature(doc_cfg))]

/// 返回 crate 版本号 (`ide-kernel-core v0.1.0`).
///
/// 占位函数 — 完整 microkernel API 待 DD-01 实装后补充.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 返回 IDE1.0 完整内核 status 字符串 (当前 placeholder).
///
/// # Returns
///
/// - `"placeholder"` — Stage 3.0 方案 A 最小骨架状态
/// - `"in-progress"` — DD-01/02/03/04 任一实装中
/// - `"stable"` — 4 详细设计书全部实装完成
#[must_use]
pub fn kernel_status() -> &'static str {
    "placeholder"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_semver() {
        let v = version();
        assert!(!v.is_empty(), "version must not be empty");
        // semver: major.minor.patch (e.g. "0.1.0")
        let parts: Vec<&str> = v.split('.').collect();
        assert_eq!(parts.len(), 3, "version must be semver: got {v}");
    }

    #[test]
    fn test_kernel_status_is_known() {
        let s = kernel_status();
        assert!(
            ["placeholder", "in-progress", "stable"].contains(&s),
            "unknown kernel status: {s}"
        );
    }
}
