//! ide-shell-web — Source Control panel backend (HTTP version).
//!
//! Wraps git CLI like the desktop `scm.rs` but exposes stateless HTTP API.
//! All commands walk up from project root to find git dir.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitEntry {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub time: String,
    pub message: String,
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("git not found: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git failed: {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn resolve_repo() -> Result<PathBuf, String> {
    let project = std::env::var("IDE_SHELL_WEB_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
    find_git_root(&project).ok_or_else(|| format!("not a git repo: {}", project.display()))
}

pub fn status() -> Result<(Vec<FileStatus>, String), String> {
    let cwd = resolve_repo()?;
    let raw = run_git(&cwd, &["status", "--porcelain"])?;
    let mut files = Vec::new();
    for line in raw.lines() {
        if line.len() < 3 {
            continue;
        }
        let code = &line[0..2];
        let path = line[3..].trim().trim_matches('"').to_string();
        let (x, y) = (
            code.chars().nth(0).unwrap_or(' '),
            code.chars().nth(1).unwrap_or(' '),
        );
        if x == '?' && y == '?' {
            files.push(FileStatus {
                path,
                status: "untracked".into(),
                staged: false,
            });
            continue;
        }
        let status_str = match (x, y) {
            ('A', _) => "added",
            ('M', _) | (_, 'M') => "modified",
            ('D', _) | (_, 'D') => "deleted",
            ('R', _) => "renamed",
            ('C', _) => "copied",
            _ => "unknown",
        };
        let staged = x != ' ' && x != '?';
        files.push(FileStatus {
            path,
            status: status_str.into(),
            staged,
        });
    }
    let branch = run_git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    Ok((files, branch.trim().to_string()))
}

pub fn stage(paths: &[String]) -> Result<(), String> {
    let cwd = resolve_repo()?;
    let mut args = vec!["add", "--"];
    for p in paths {
        args.push(p);
    }
    run_git(&cwd, &args)?;
    Ok(())
}

pub fn unstage(paths: &[String]) -> Result<(), String> {
    let cwd = resolve_repo()?;
    let mut args = vec!["reset", "HEAD", "--"];
    for p in paths {
        args.push(p);
    }
    run_git(&cwd, &args)?;
    Ok(())
}

pub fn diff(path: &str, staged: bool) -> Result<String, String> {
    let cwd = resolve_repo()?;
    let args: Vec<&str> = if staged {
        vec!["diff", "--cached", "--", path]
    } else {
        vec!["diff", "--", path]
    };
    run_git(&cwd, &args)
}

pub fn commit(message: &str) -> Result<String, String> {
    let cwd = resolve_repo()?;
    run_git(&cwd, &["commit", "-m", message])
}

pub fn log(n: usize) -> Result<Vec<CommitEntry>, String> {
    let cwd = resolve_repo()?;
    let fmt = "%H%x00%h%x00%an%x00%ai%x00%s";
    let raw = run_git(
        &cwd,
        &["log", &format!("-{}", n), &format!("--format={}", fmt)],
    )?;
    let mut entries = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('\0').collect();
        if parts.len() >= 5 {
            entries.push(CommitEntry {
                hash: parts[0].to_string(),
                short: parts[1].to_string(),
                author: parts[2].to_string(),
                time: parts[3].to_string(),
                message: parts[4].to_string(),
            });
        }
    }
    Ok(entries)
}
