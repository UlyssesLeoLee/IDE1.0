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
    let mut args = vec!["--line-number", "--no-heading", "--color", "never"];
    if !opts.case_sensitive.unwrap_or(false) {
        args.push("--ignore-case");
    }
    if !opts.regex.unwrap_or(false) {
        args.push("--fixed-strings");
    }
    if let Some(max) = opts.max_results {
        args.push("--max-count");
        args.push(&max.to_string());
    }
    if let Some(inc) = &opts.include_globs {
        for g in inc {
            args.push("--glob");
            args.push(g);
        }
    }
    if let Some(exc) = &opts.exclude_globs {
        for g in exc {
            args.push("--glob");
            args.push(g);
            args.push("--invert-match");
        }
    }
    args.push(&opts.pattern);

    let output = Command::new("rg")
        .args(&args)
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
