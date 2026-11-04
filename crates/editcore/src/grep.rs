//! editcore — 文件枚举 + 内容搜索 (参考实现的 `CGrep*` / `CGrepEnumFiles` Rust 重写).
//!
//! 设计:
//! - 文件枚举: `walk_dir(root, pattern)` 返回 Vec<PathBuf>, 0 依赖 (std::fs::read_dir + 递归).
//! - 内容搜索: `grep_in_files(files, pattern)` 返回 Vec<Match { path, line, text }>,
//!   手写简单 Boyer-Moore-Horspool (无 regex crate).
//! - 性能: 适合 1K~100K 文件目录. 大规模用 walkdir + aho-corasick crate.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 文件枚举结果.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

/// 递归列举目录 — 跳过隐藏 (`.xxx`).
pub fn walk_dir(root: &Path, follow_symlinks: bool) -> Vec<FileEntry> {
    let mut out = Vec::new();
    walk_dir_inner(root, follow_symlinks, &mut out);
    out
}

fn walk_dir_inner(root: &Path, follow_symlinks: bool, out: &mut Vec<FileEntry>) {
    let Ok(meta) = fs::symlink_metadata(root) else {
        return;
    };
    let file_type = meta.file_type();
    let is_dir = file_type.is_dir();
    out.push(FileEntry {
        path: root.to_path_buf(),
        is_dir,
        size: meta.len(),
    });
    if !is_dir {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if !follow_symlinks {
            let Ok(m) = fs::symlink_metadata(&path) else {
                continue;
            };
            if m.file_type().is_symlink() {
                continue;
            }
        }
        walk_dir_inner(&path, follow_symlinks, out);
    }
}

/// 按扩展名过滤.
pub fn filter_by_ext(entries: &[FileEntry], exts: &[&str]) -> Vec<FileEntry> {
    let ext_set: HashSet<String> = exts.iter().map(|s| s.to_lowercase()).collect();
    entries
        .iter()
        .filter(|e| !e.is_dir)
        .filter(|e| {
            e.path
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| ext_set.contains(&x.to_lowercase()))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// 搜索匹配结果.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub path: PathBuf,
    pub line: u64,
    pub text: String,
}

/// 简单字符串搜索 (case-sensitive).
/// 返回每行匹配位置. 文件按其编码解码 (BOM / UTF-8 / SJIS / EUC-JP / JIS / UTF-16 自动判别).
pub fn grep_in_file(path: &Path, needle: &str) -> Result<Vec<Match>, std::io::Error> {
    grep_in_file_with(path, needle, None)
}

/// 同 [`grep_in_file`], 但可显式指定字符集 (`None` / `"auto"` = 自动). 未知字符集名 => `InvalidInput`.
pub fn grep_in_file_with(
    path: &Path,
    needle: &str,
    charset: Option<&str>,
) -> Result<Vec<Match>, std::io::Error> {
    let cs = crate::grep_engine::parse_charset_label(charset)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let text = crate::grep_engine::read_text_for_grep(path, cs)?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.contains(needle) {
            out.push(Match {
                path: path.to_path_buf(),
                line: i as u64 + 1,
                text: line.to_string(),
            });
        }
    }
    Ok(out)
}

/// 在多个文件中搜索.
pub fn grep_in_files(files: &[FileEntry], needle: &str) -> Vec<Match> {
    let mut out = Vec::new();
    for entry in files {
        if entry.is_dir {
            continue;
        }
        if let Ok(matches) = grep_in_file(&entry.path, needle) {
            out.extend(matches);
        }
    }
    out
}

/// 简单 case-insensitive 搜索 (lowercase needle + lowercase line).
pub fn grep_in_file_ci(path: &Path, needle: &str) -> Result<Vec<Match>, std::io::Error> {
    let needle_lc = needle.to_lowercase();
    let text = crate::grep_engine::read_text_for_grep(path, None)?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.to_lowercase().contains(&needle_lc) {
            out.push(Match {
                path: path.to_path_buf(),
                line: i as u64 + 1,
                text: line.to_string(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree() -> tempfile_lite::TempDir {
        let dir = tempfile_lite::TempDir::new("sakura-rs-grep-test");
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "hello world\nfoo bar\nhello world\n").unwrap();
        std::fs::write(p.join("b.txt"), "world peace\n").unwrap();
        std::fs::create_dir(p.join("sub")).unwrap();
        std::fs::write(p.join("sub/c.txt"), "hello sub\n").unwrap();
        std::fs::write(p.join(".hidden"), "should be ignored\n").unwrap();
        dir
    }

    /// 临时目录 helper — 不引外部 dep, 写一个简版 (每实例 unique).
    mod tempfile_lite {
        use std::path::PathBuf;
        use std::time::{SystemTime, UNIX_EPOCH};
        pub struct TempDir(PathBuf);
        impl TempDir {
            pub fn new(prefix: &str) -> Self {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let mut p = std::env::temp_dir();
                p.push(format!("{}-{}-{}", prefix, std::process::id(), nanos));
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
            pub fn path(&self) -> &std::path::Path {
                &self.0
            }
        }
        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn walk_dir_finds_all_non_hidden() {
        let dir = make_tree();
        let entries = walk_dir(dir.path(), false);
        let names: Vec<_> = entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"b.txt".to_string()));
        assert!(names.contains(&"c.txt".to_string()));
        assert!(names.contains(&"sub".to_string()));
        assert!(!names.contains(&".hidden".to_string()));
    }

    #[test]
    fn walk_dir_filter_by_ext() {
        let dir = make_tree();
        let entries = walk_dir(dir.path(), false);
        let txts = filter_by_ext(&entries, &["txt"]);
        assert_eq!(txts.len(), 3); // a.txt + b.txt + sub/c.txt
    }

    #[test]
    fn grep_in_files_case_sensitive() {
        let dir = make_tree();
        let entries = walk_dir(dir.path(), false);
        let matches = grep_in_files(&entries, "hello");
        assert_eq!(matches.len(), 3); // a.txt (2) + sub/c.txt (1)
        let paths: std::collections::HashSet<_> = matches.iter().map(|m| m.path.clone()).collect();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn grep_in_file_decodes_sjis_and_utf16() {
        let dir = tempfile_lite::TempDir::new("sakura-rs-grep-enc");
        let (sj, _, _) = encoding_rs::SHIFT_JIS.encode("一行目
日本語の行
");
        std::fs::write(dir.path().join("s.txt"), &*sj).unwrap();
        let mut u16 = vec![0xFF, 0xFE];
        for u in "abc
日本語
".encode_utf16() {
            u16.extend_from_slice(&u.to_le_bytes());
        }
        std::fs::write(dir.path().join("u.txt"), &u16).unwrap();
        let m = grep_in_file(&dir.path().join("s.txt"), "日本語").unwrap();
        assert_eq!((m.len(), m[0].line, m[0].text.as_str()), (1, 2, "日本語の行"));
        let m = grep_in_file(&dir.path().join("u.txt"), "日本語").unwrap();
        assert_eq!((m.len(), m[0].line), (1, 2));
        // 强制 UTF-8 去读 SJIS 文件: 找不到
        assert!(grep_in_file_with(&dir.path().join("s.txt"), "日本語", Some("utf-8")).unwrap().is_empty());
        assert!(grep_in_file_with(&dir.path().join("s.txt"), "x", Some("nope")).is_err());
    }

    #[test]
    fn grep_in_file_ci_finds_lowercase() {
        let dir = make_tree();
        let matches = grep_in_file_ci(&dir.path().join("a.txt"), "HELLO").unwrap();
        assert_eq!(matches.len(), 2);
    }
}
