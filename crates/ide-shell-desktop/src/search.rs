//! ide-shell-desktop — Search panel backend (ripgrep-style content search).
//!
//! 用 `Command::new("rg")` if available, else fallback to manual walk + line scan.
//! 0 重型依赖.

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub pattern: String,
    pub case_sensitive: Option<bool>,
    pub regex: Option<bool>,
    pub include_globs: Option<Vec<String>>,
    pub exclude_globs: Option<Vec<String>>,
    pub max_results: Option<usize>,
}

/// 用 rg 跑 search. 返回结果列表.
pub fn search(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    let mut args: Vec<String> = vec!["--line-number".into(), "--no-heading".into(), "--color".into(), "never".into()];
    if !opts.case_sensitive.unwrap_or(false) {
        args.push("--ignore-case".into());
    }
    if !opts.regex.unwrap_or(false) {
        args.push("--fixed-strings".into());
    }
    if let Some(max) = opts.max_results {
        args.push("--max-count".into());
        args.push(max.to_string());
    }
    if let Some(inc) = &opts.include_globs {
        for g in inc {
            args.push("--glob".into());
            args.push(g.clone());
        }
    }
    if let Some(exc) = &opts.exclude_globs {
        for g in exc {
            args.push("--glob".into());
            args.push(g.clone());
            args.push("--invert-match".into());
        }
    }
    args.push(opts.pattern.clone());

    let output = Command::new("rg")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("rg not found: {} (please install ripgrep or use fallback)", e))?;

    // rg exit 0/1 (found/not found) are both ok; other codes = error
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(format!(
            "rg failed ({}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in stdout.lines() {
        // rg output: path:line:text
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() == 3 {
            let path = parts[0].to_string();
            let line_num: u64 = parts[1].parse().unwrap_or(0);
            let text = parts[2].to_string();
            results.push(SearchResult { path, line: line_num, text });
        }
    }
    Ok(results)
}

/// Fallback: 没装 rg 时, manual walk + line scan.
pub fn search_fallback(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    use std::fs;
    use std::io::BufRead;

    let mut results = Vec::new();
    let pattern_lc = opts.pattern.to_lowercase();
    let cs = opts.case_sensitive.unwrap_or(false);
    let max = opts.max_results.unwrap_or(500);

    fn walk(cwd: &Path, results: &mut Vec<SearchResult>, pattern: &str, pattern_lc: &str, cs: bool, max: usize) -> std::io::Result<()> {
        if results.len() >= max { return Ok(()); }
        for entry in fs::read_dir(cwd)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            if entry.file_type()?.is_dir() {
                if name == "node_modules" || name == "target" || name == ".git" || name == "dist" { continue; }
                walk(&path, results, pattern, pattern_lc, cs, max)?;
            } else if entry.file_type()?.is_file() {
                if let Ok(f) = fs::File::open(&path) {
                    let reader = std::io::BufReader::new(f);
                    for (i, line) in reader.lines().enumerate() {
                        if let Ok(line) = line {
                            let matches = if cs {
                                line.contains(pattern)
                            } else {
                                line.to_lowercase().contains(pattern_lc)
                            };
                            if matches {
                                let path_s = path.to_string_lossy().into_owned();
                                results.push(SearchResult {
                                    path: path_s,
                                    line: (i + 1) as u64,
                                    text: line,
                                });
                                if results.len() >= max { return Ok(()); }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    walk(cwd, &mut results, &opts.pattern, &pattern_lc, cs, max)
        .map_err(|e| format!("search failed: {}", e))?;
    Ok(results)
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
    // Try rg first, fallback to manual walk
    match crate::search::search(&project, &options) {
        Ok(r) => Ok(r),
        Err(_e) => crate::search::search_fallback(&project, &options),
    }
}
