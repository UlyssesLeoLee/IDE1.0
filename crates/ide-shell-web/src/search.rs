//! ide-shell-web — Search panel backend (HTTP version).

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
        // 排除文件 = ripgrep 的取反 glob `--glob '!PATTERN'`.
        //
        // 原实现 `--glob X --invert-match` 语义错误: `--invert-match`
        // 反转的是**匹配行** (只影响有无匹配的行), 不是排除文件;
        // 且放在循环内会被重复 push 多次。
        for g in exc {
            args.push("--glob".into());
            args.push(format!("!{}", g.trim_start_matches('!')));
        }
    }
    // `--` 终止选项解析: 否则以 `-` 开头的 pattern 会被 rg 当成选项,
    // 例如 `--pre=<cmd>` 可让 rg 执行任意外部程序 (参数注入)。
    args.push("--".into());
    args.push(opts.pattern.clone());


    let output = Command::new("rg")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("rg not found: {} (use fallback)", e))?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(format!("rg failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() == 3 {
            if let Ok(line_num) = parts[1].parse::<u64>() {
                results.push(SearchResult {
                    path: parts[0].to_string(),
                    line: line_num,
                    text: parts[2].to_string(),
                });
            }
        }
    }
    Ok(results)
}

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
                if matches!(name.as_str(), "node_modules" | "target" | ".git" | "dist") { continue; }
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
                                results.push(SearchResult {
                                    path: path.to_string_lossy().into_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 端到端验证 P0 修复: exclude 语义 + pattern 参数注入防护。
    /// 需要 PATH 上有 rg; 没有则跳过 (不静默通过)。
    fn have_rg() -> bool {
        Command::new("rg").arg("--version").output().is_ok()
    }

    fn setup(dir: &Path) {
        let _ = fs::create_dir_all(dir);
        fs::write(dir.join("keep.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
        fs::write(dir.join("skip.rs"), "fn alpha() {}\n").unwrap();
        fs::write(dir.join("skip.txt"), "alpha beta\n").unwrap();
    }

    #[test]
    fn exclude_glob_excludes_files_not_inverts_matches() {
        if !have_rg() { eprintln!("skip: rg 不可用"); return; }
        let dir = std::env::temp_dir().join("ide10-search-exclude");
        let _ = fs::remove_dir_all(&dir);
        setup(&dir);

        let opts = SearchOptions {
            pattern: "alpha".into(),
            case_sensitive: Some(true),
            regex: Some(false),
            include_globs: Some(vec!["*.rs".into()]),
            exclude_globs: Some(vec!["skip*".into()]),
            max_results: None,
        };
        let hits = search(&dir, &opts).expect("search 应成功");

        // 原实现用 --invert-match: 语义是"只保留**不**匹配的行",
        // 会把 skip.rs 的**非匹配行**也返回, 且 exclude 语义完全错位.
        assert!(
            hits.iter().all(|h| !h.path.contains("skip")),
            "exclude 应排除文件, 实际返回: {:?}",
            hits.iter().map(|h| &h.path).collect::<Vec<_>>()
        );
        assert!(hits.iter().any(|h| h.path.contains("keep")), "keep.rs 应有命中");
    }

    #[test]
    fn dash_leading_pattern_not_treated_as_option() {
        if !have_rg() { eprintln!("skip: rg 不可用"); return; }
        let dir = std::env::temp_dir().join("ide10-search-dash");
        let _ = fs::remove_dir_all(&dir);
        setup(&dir);

        // `--pre=<cmd>` 若被 rg 当选项, rg 会执行外部程序.
        // `--` 之后它只是普通 pattern.
        let opts = SearchOptions {
            pattern: "--pre=echo PWNED".into(),
            case_sensitive: Some(true),
            regex: Some(false),
            include_globs: None,
            exclude_globs: None,
            max_results: None,
        };
        // 不应崩溃 / 不应执行; 结果为空是正确行为 (文件里没这个字符串)
        let r = search(&dir, &opts);
        assert!(r.is_ok(), "带 -- 的 pattern 应被当字面量处理, 得到 {:?}", r.err());
        let hits = r.unwrap();
        assert!(hits.is_empty(), "字面量不该命中, 却得到 {:?}", hits);
    }

    #[test]
    fn already_negated_exclude_not_double_negated() {
        if !have_rg() { eprintln!("skip: rg 不可用"); return; }
        let dir = std::env::temp_dir().join("ide10-search-excl2");
        let _ = fs::remove_dir_all(&dir);
        setup(&dir);

        // 前端已加 `!` 的情况不应变成 `!!`
        let opts = SearchOptions {
            pattern: "alpha".into(),
            case_sensitive: Some(true),
            regex: Some(false),
            include_globs: Some(vec!["*.rs".into()]),
            exclude_globs: Some(vec!["!skip*".into()]),
            max_results: None,
        };
        let hits = search(&dir, &opts).expect("search 应成功");
        assert!(hits.iter().all(|h| !h.path.contains("skip")), "得到 {:?}", hits);
    }
}
