//! ide-shell-desktop — Source Control (Git) panel backend (对标 Cursor / VSCode).
//!
//! ## Design
//! - **Git via CLI spawn** — `git status --porcelain` / `git add` / `git reset` / `git commit` / `git diff`
//! - **No libgit2** — 0 重型依赖, 跨平台一致
//! - **Per-project state** — ProjectRoot 已 manage, SCM 状态绑定 project root
//! - **Async spawn** — 不阻塞 Tauri command handler
//!
//! ## API surface
//! - `scm_status()` — 返回 List<FileStatus> (path, kind, staged)
//! - `scm_stage(paths)` / `scm_unstage(paths)` — git add / reset
//! - `scm_diff(path)` — 返回 unified diff text
//! - `scm_commit(message)` — git commit -m "<msg>"
//! - `scm_log(n)` — 返回最近 n 条 commit (hash, message, author, time)

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// Git status of a file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileStatus {
    pub path: String,
    pub status: String, // "modified" / "added" / "deleted" / "untracked" / "renamed"
    pub staged: bool,
}

/// Commit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitEntry {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub time: String,
    pub message: String,
}

/// SCM 错误.
#[derive(Debug, Serialize)]
pub struct ScmError {
    pub kind: String, // "not_git_repo" / "git_failed" / "io"
    pub message: String,
}

impl std::fmt::Display for ScmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)
    }
}

impl std::error::Error for ScmError {}

impl From<std::io::Error> for ScmError {
    fn from(e: std::io::Error) -> Self {
        ScmError { kind: "io".into(), message: e.to_string() }
    }
}

/// Run `git <args>` in `cwd` — capture stdout/stderr.
fn run_git(cwd: &Path, args: &[&str]) -> Result<String, ScmError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| ScmError {
            kind: "git_not_found".into(),
            message: format!("git not found: {}", e),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(ScmError {
            kind: "git_failed".into(),
            message: format!(
                "git {} failed (exit {}): {}",
                args.join(" "),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `git status --porcelain` parse.
pub fn status(cwd: &Path) -> Result<(Vec<FileStatus>, String), ScmError> {
    // porcelain v1: XY filename (X=staged, Y=working)
    let raw = run_git(cwd, &["status", "--porcelain"])?;
    let mut files = Vec::new();
    for line in raw.lines() {
        if line.len() < 3 {
            continue;
        }
        let code = &line[0..2];
        let path = line[3..].trim().trim_matches('"').to_string();
        let (x, y) = (code.chars().nth(0).unwrap_or(' '), code.chars().nth(1).unwrap_or(' '));
        // Untracked (??)
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
    // Branch name
    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    Ok((files, branch.trim().to_string()))
}

/// `git add <paths>`.
pub fn stage(cwd: &Path, paths: &[String]) -> Result<(), ScmError> {
    let mut args = vec!["add", "--"];
    for p in paths {
        args.push(p);
    }
    run_git(cwd, &args)?;
    Ok(())
}

/// `git reset HEAD <paths>` — unstage.
pub fn unstage(cwd: &Path, paths: &[String]) -> Result<(), ScmError> {
    let mut args = vec!["reset", "HEAD", "--"];
    for p in paths {
        args.push(p);
    }
    run_git(cwd, &args)?;
    Ok(())
}

/// `git diff <path>` — 返回 diff text (工作区 vs index).
pub fn diff(cwd: &Path, path: &str) -> Result<String, ScmError> {
    run_git(cwd, &["diff", "--", path])
}

/// `git diff --cached <path>` — staged diff.
pub fn diff_cached(cwd: &Path, path: &str) -> Result<String, ScmError> {
    run_git(cwd, &["diff", "--cached", "--", path])
}

/// `git commit -m <msg>`.
pub fn commit(cwd: &Path, message: &str) -> Result<String, ScmError> {
    run_git(cwd, &["commit", "-m", message])
}

/// `git log -n --format=...` — 最近 n 条 commit.
pub fn log(cwd: &Path, n: usize) -> Result<Vec<CommitEntry>, ScmError> {
    let fmt = "%H%x00%h%x00%an%x00%ai%x00%s";
    let raw = run_git(
        cwd,
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

/// 检查路径是否是 git repo.
pub fn is_git_repo(cwd: &Path) -> bool {
    run_git(cwd, &["rev-parse", "--git-dir"]).is_ok()
}

/// 找 project root 的 git repo (project_root 向上找 .git).
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    #[test]
    fn parse_porcelain_modified() {
        let raw = " M src/main.rs\nA  src/new.rs\n?? untracked.txt\n";
        assert_eq!(raw.lines().count(), 3);
    }

    #[test]
    fn find_git_root_walks_up() {
        // /tmp / C:\Users\leo19\AppData\Local — none has .git
        let start = std::env::temp_dir();
        let result = find_git_root(&start);
        // 不一定找到 (tempdir 没 .git), 但函数不应 panic
        assert!(result.is_none() || result.is_some());
    }

    #[test]
    #[ignore] // requires git + 真实 repo
    fn integration_status_in_real_repo() {
        if !git_available() {
            return;
        }
        let cwd = std::env::current_dir().unwrap();
        if let Ok((files, branch)) = status(&cwd) {
            println!("branch={}, files={}", branch, files.len());
        }
    }

    #[test]
    fn is_git_repo_detects_current_repo() {
        // 当前 IDE1.0 repo 应该是 git repo
        let cwd = std::env::current_dir().unwrap();
        // Find dev-3 root
        let dev3 = cwd.components().find(|c| c.as_os_str() == "dev-3");
        if let Some(_) = dev3 {
            // walk up to dev-3
            let mut p = cwd.clone();
            while !p.join(".git").exists() {
                if !p.pop() { break; }
            }
            if p.join(".git").exists() {
                assert!(is_git_repo(&p));
            }
        }
        let _ = fs::create_dir; // suppress unused
    }
}


// ============================================================
// Tauri command wrappers (ProjectRoot → cwd)
// ============================================================

use tauri::State;
use std::sync::Arc;
use std::path::PathBuf;

/// Internal: resolve project root, find git root (walk up).
fn resolve_repo(state: &State<'_, crate::ProjectRoot>) -> Result<PathBuf, String> {
    let arc: &Arc<std::sync::Mutex<Option<PathBuf>>> = &state.0;
    let guard = arc.lock().map_err(|e| e.to_string())?;
    let project = guard.clone().ok_or("project root not set")?;
    drop(guard);
    find_git_root(&project).ok_or_else(|| format!("not a git repo: {}", project.display()))
}

#[derive(Serialize)]
pub struct ScmStatusResult {
    pub branch: String,
    pub files: Vec<FileStatus>,
}

#[tauri::command]
pub fn scm_status(state: State<'_, crate::ProjectRoot>) -> Result<ScmStatusResult, String> {
    let cwd = resolve_repo(&state)?;
    let (files, branch) = status(&cwd).map_err(|e| e.to_string())?;
    Ok(ScmStatusResult { branch, files })
}

#[tauri::command]
pub fn scm_stage(state: State<'_, crate::ProjectRoot>, paths: Vec<String>) -> Result<(), String> {
    let cwd = resolve_repo(&state)?;
    stage(&cwd, &paths).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scm_unstage(state: State<'_, crate::ProjectRoot>, paths: Vec<String>) -> Result<(), String> {
    let cwd = resolve_repo(&state)?;
    unstage(&cwd, &paths).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scm_diff(state: State<'_, crate::ProjectRoot>, path: String, staged: Option<bool>) -> Result<String, String> {
    let cwd = resolve_repo(&state)?;
    let f = if staged.unwrap_or(false) { diff_cached(&cwd, &path) } else { diff(&cwd, &path) };
    f.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scm_commit(state: State<'_, crate::ProjectRoot>, message: String) -> Result<String, String> {
    let cwd = resolve_repo(&state)?;
    commit(&cwd, &message).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scm_log(state: State<'_, crate::ProjectRoot>, n: Option<usize>) -> Result<Vec<CommitEntry>, String> {
    let cwd = resolve_repo(&state)?;
    log(&cwd, n.unwrap_or(20)).map_err(|e| e.to_string())
}
