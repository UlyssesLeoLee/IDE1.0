//! 路径沙箱工具 — 在 ide-shell-desktop 和 ide-shell-web 间共享.
//!
//! 复制自 ide-shell-desktop/src/lib.rs 的同名函数 (避免新增 crate 路径).
//! 改动时请两边同步.
//!
//! 包含:
//! - `safe_canonicalize`: 处理不存在的 target 路径 (write_file 写新文件)
//! - `strip_verbatim`: 剥 Windows `\\?\` verbatim 前缀
//! - `path_within`: 沙箱检查 (root ⊇ target)
//! - `resolve_in_project`: 校验 + 返回 canonicalized target

use std::path::{Path, PathBuf};

/// 单文件读取上限 (4 MB).
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Windows: 剥掉 verbatim 前缀 (`\\?\C:\...`) 跟盘符 root 比较.
#[cfg(windows)]
pub fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}
#[cfg(not(windows))]
pub fn strip_verbatim(p: &Path) -> PathBuf {
    p.to_path_buf()
}

/// canonicalize 一个路径; 若路径不存在 (write_file 写入新文件场景),
/// 退到 canonicalize 父目录, 再 join 原 basename.
pub fn safe_canonicalize(p: &Path) -> std::io::Result<PathBuf> {
    match p.canonicalize() {
        Ok(c) => Ok(c),
        Err(_) => {
            let mut cur = p.to_path_buf();
            let mut tail = PathBuf::new();
            loop {
                match cur.canonicalize() {
                    Ok(c) => return Ok(c.join(tail)),
                    Err(_) => {
                        let name = match cur.file_name() {
                            Some(n) => n.to_os_string(),
                            None => return Err(std::io::ErrorKind::NotFound.into()),
                        };
                        if tail.as_os_str().is_empty() {
                            tail = PathBuf::from(name);
                        } else {
                            tail = PathBuf::from(name).join(tail);
                        }
                        match cur.parent() {
                            Some(par) => cur = par.to_path_buf(),
                            None => return Err(std::io::ErrorKind::NotFound.into()),
                        }
                    }
                }
            }
        }
    }
}

/// 沙箱检查: `target` 必须位于 `root` 内.
pub fn path_within(root: &Path, target: &Path) -> bool {
    let (r, t) = match (safe_canonicalize(root), safe_canonicalize(target)) {
        (Ok(r), Ok(t)) => (r, t),
        _ => return false,
    };
    let (r2, t2) = (strip_verbatim(&r), strip_verbatim(&t));
    t2.starts_with(r2)
}

/// 校验请求路径在项目根内, 返回 canonicalized target.
pub fn resolve_in_project(root: &Path, path: &str) -> Result<PathBuf, String> {
    let root =
        safe_canonicalize(root).map_err(|e| format!("项目根无效: {} ({e})", root.display()))?;
    let target = PathBuf::from(path);
    if !path_within(&root, &target) {
        return Err(format!(
            "拒绝访问项目外路径: {path} (安全沙箱限制在项目根内)"
        ));
    }
    safe_canonicalize(&target).map_err(|e| format!("路径无效 {path}: {e}"))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub content: String,
}

pub fn list_dir(root: &Path, path: &str) -> Result<Vec<DirEntry>, String> {
    let target = resolve_in_project(root, path)?;
    if !target.is_dir() {
        return Err(format!("不是目录: {path}"));
    }
    let mut entries: Vec<DirEntry> = std::fs::read_dir(&target)
        .map_err(|e| format!("读取目录失败 {path}: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| {
            let p = e.path();
            DirEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                path: p.to_string_lossy().into_owned(),
                is_dir: p.is_dir(),
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read_file(root: &Path, path: &str) -> Result<FileContent, String> {
    let target = resolve_in_project(root, path)?;
    if !target.is_file() {
        return Err(format!("不是文件: {path}"));
    }
    let size = std::fs::metadata(&target)
        .map_err(|e| format!("读取元数据失败: {e}"))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(format!(
            "文件过大 ({size} bytes > {MAX_FILE_BYTES} bytes) — 保持轻量"
        ));
    }
    let content = std::fs::read_to_string(&target)
        .map_err(|e| format!("读取失败 (仅支持 UTF-8 文本): {e}"))?;
    Ok(FileContent {
        name: target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string()),
        path: target.to_string_lossy().into_owned(),
        content,
    })
}

pub fn write_file(root: &Path, path: &str, content: &str) -> Result<usize, String> {
    let target = resolve_in_project(root, path)?;
    std::fs::write(&target, content.as_bytes()).map_err(|e| format!("写入失败 {path}: {e}"))?;
    Ok(content.len())
}
