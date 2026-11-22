//! 跨供应商无头协同编辑引擎 (CAS + 3-Way 自动合并 + 冲突保护 + 真实磁盘协同)。
//!
//! 不变量:
//! - 纯 Safe Rust (`#![forbid(unsafe_code)]`)
//! - 单一权威 `DocumentStore`
//! - 任何冲突不得污染 HEAD 版本，必须以独立 `ConflictProposal` 保存并返回上下文 Diff
//! - OCC (乐观并发控制) + 作用域幂等性 + 租约 Fencing Token 校验

pub mod algo;
pub mod compat;
pub mod disk;
pub mod hash;
pub mod nema;
pub mod store;
pub mod types;

#[cfg(test)]
mod tests;

pub use algo::{
    check_syntax_integrity, find_single_hunk, three_way_merge, ConflictHunk, MergeOutcome,
};
pub use compat::{CoEditWorkspace, ConcurrentCoEditWorkspace};
pub use disk::{atomic_write_file, git_diff_workspace};
pub use hash::{content_hash, fnv1a_hash, sha256_hex};
pub use nema::nema_merge;
pub use store::DocumentStore;
pub use types::*;
