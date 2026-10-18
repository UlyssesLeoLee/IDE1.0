//! ide-shell-desktop — Grep 后端 (委托 `sakura-rs::grep_engine`).
//!
//! 这里是 Tauri command 层, 只负责「从 app state 取项目根 → 调共用引擎 → 回传」.
//!
//! # 为什么不 shell out 到外部 rg
//!
//! 本文件原先是 `Command::new("rg")` + 缺 rg 时退到一份手写 walk, 违反
//! docs/SPA-01_sakura_parity.md §3 原则 3「Rust 优先」, 且两端行为会漂移:
//! web 端已经是内置引擎, 桌面端还在调外部 rg, 同一份搜索条件在两个壳里结果可能不同。
//! 现在两端共用 `sakura-rs::grep_engine` 一份实现。

#![allow(dead_code)]

use std::path::Path;

pub use sakura_rs::grep_engine::{SearchOptions, SearchResult};

/// 在项目根下搜索 (委托共用引擎).
pub fn search(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    sakura_rs::grep_engine::search(cwd, opts)
}

/// 兼容旧调用点: 与 `search` 同一实现 (不再是「rg 缺失时的弱化降级」).
pub fn search_fallback(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    sakura_rs::grep_engine::search(cwd, opts)
}

// ============================================================
// Tauri command wrapper
// ============================================================
use tauri::State;

#[tauri::command]
pub fn search_cmd(
    state: State<'_, crate::ProjectRoot>,
    options: SearchOptions,
) -> Result<Vec<SearchResult>, String> {
    let arc: &std::sync::Arc<std::sync::Mutex<Option<std::path::PathBuf>>> = &state.0;
    let guard = arc.lock().map_err(|e| e.to_string())?;
    let project = guard.clone().ok_or("project root not set")?;
    drop(guard);
    search(&project, &options)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 桌面端曾经完全依赖外部 rg, 换成本地引擎后必须证明这条路径真能跑出结果.
    #[test]
    fn search_works_without_rg_binary() {
        let dir = std::env::temp_dir().join(format!(
            "ide10-desktop-search-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.txt"), "hello rust
").unwrap();

        let opts = SearchOptions {
            pattern: "rust".into(),
            case_sensitive: Some(true),
            ..Default::default()
        };
        let hits = search(&dir, &opts).unwrap();
        assert_eq!(hits.len(), 1, "内置引擎应搜到 1 处");
        assert_eq!(hits[0].line, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
