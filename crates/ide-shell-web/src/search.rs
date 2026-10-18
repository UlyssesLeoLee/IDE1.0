//! ide-shell-web — Search backend 薄封装.
//!
//! 实现已搬到 `sakura-rs::grep_engine`, 由 web 端与桌面端**共用一份** ——
//! 之前两端各写一份 (桌面端那份还是 `Command::new("rg")`), 行为必然漂移。
//! 本文件只保留类型别名与 HTTP 端点的取参, 语义见 sakura-rs 的模块文档。

#![allow(dead_code)]

pub use editcore::grep_engine::{search as search_impl, SearchOptions, SearchResult};

use std::path::Path;

/// 在项目根下搜索. 语义见 `editcore::grep_engine::search`.
pub fn search(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    search_impl(cwd, opts)
}

/// 兼容旧调用点: 与 `search` 同一实现 (不再是「rg 缺失时的弱化降级」).
pub fn search_fallback(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    search_impl(cwd, opts)
}

#[cfg(test)]
mod tests {

    #[test]
    fn re_exports_are_usable() {
        // 确认搬过去的符号在本 crate 真的可调用 (而不是只编译过)
        let re = editcore::grep_engine::MiniRegex::new("a+", true).unwrap();
        assert!(re.is_match("aaa"));
        assert!(editcore::grep_engine::glob_match("main.rs", "*.rs", true));
        assert!(!editcore::grep_engine::SKIP_DIRS.is_empty());
    }
}
