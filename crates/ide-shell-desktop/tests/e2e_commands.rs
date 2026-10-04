//! ide-shell-desktop 真 e2e: 通过纯函数 pub API 测 command 业务逻辑.
#![forbid(unsafe_code)]
//!
//! 这层不启真 Tauri runtime (Tauri runtime 需要 event loop). 真 e2e UI 留给
//! cargo build → msiexec / launch .exe 跑, 或后续 Playwright + Tauri CDP.
//!
//! 覆盖:
//!   - open_project → 沙箱 set 根 (canonicalize + reject 非目录)
//!   - list_dir    → 目录在前/文件在后/不区分大小写排序, 沙箱拒绝越界
//!   - read_file   → 正常读 / 拒非文件 / 拒大文件 / 拒非 UTF-8 / 沙箱拒绝越界
//!   - write_file  → 正常写/覆盖, 沙箱拒绝越界
//!   - help_wiki   → CLI `--help` 出口完整, 覆盖全部命令文档

use std::fs;
use std::path::PathBuf;

use ide_shell_desktop::{
    help_wiki_text, list_dir_pub, read_file_pub, resolve_in_project_pub, write_file_pub,
    ProjectRoot as _ProjectRoot, MAX_FILE_BYTES,
};

/// 建一个临时项目根, 退出 scope 时自动删除. 不用 tempfile crate (guid 自实现避免新增依赖).
struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(label: &str) -> Self {
        let mut root = std::env::temp_dir();
        let uniq: String = format!(
            "ide-shell-desktop-it-{}-{}",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        root.push(uniq);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, rel: &str, body: &[u8]) -> PathBuf {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, body).unwrap();
        p
    }

    fn mkdir(&self, rel: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn outside_dir(&self) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "ide-shell-desktop-outside-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn path(&self) -> PathBuf {
        // canonicalize 一遍: Windows 上 std::env::temp_dir() 可能返回 8.3 短路径
        // (E:\Temp\...), 而 list_dir/read_file/write_file 内部会再 canonicalize
        // 一次展开成 C:\Users\...\Temp\..., 两边 starts_with 失败.
        self.root.canonicalize().unwrap_or_else(|_| self.root.clone())
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn open_project_set_rejects_non_dir() {
    let pr = _ProjectRoot::default();
    assert!(pr.get().is_none());
    let dir = std::env::temp_dir().join("ide-shell-desktop-opn-non-dir");
    let _ = fs::remove_dir_all(&dir);
    let file = dir.join("f.txt");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&file, "x").unwrap();
    // 文件 → Err
    assert!(pr.set(file).is_err());
    // 目录 → Ok
    let canon = pr.set(dir.clone()).unwrap();
    assert!(canon.is_dir());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_root_shows_dirs_first() {
    let p = TempProject::new("list-root");
    p.write("zeta.txt", b"z");
    p.write("alpha.txt", b"a");
    p.write("Z-dir/inner.txt", b"");
    p.write("a-dir/inner.txt", b"");
    let entries = list_dir_pub(&p.path(), p.path().to_str().unwrap()).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.len() >= 4, "got {names:?}");
    // 目录应排在文件前面
    let first_dir = entries.iter().position(|e| e.is_dir).unwrap();
    let first_file = entries.iter().position(|e| !e.is_dir).unwrap();
    assert!(first_dir < first_file, "目录应在文件前: {names:?}");
    // 不区分大小写排序: a-dir < Z-dir
    let pos_a = names.iter().position(|n| *n == "a-dir").unwrap();
    let pos_z = names.iter().position(|n| *n == "Z-dir").unwrap();
    assert!(pos_a < pos_z, "a-dir 应在 Z-dir 前 (case-insensitive): {names:?}");
    // 文件: alpha < zeta
    let pos_alpha = names.iter().position(|n| *n == "alpha.txt").unwrap();
    let pos_zeta = names.iter().position(|n| *n == "zeta.txt").unwrap();
    assert!(pos_alpha < pos_zeta);
}

#[test]
fn list_dir_subdirectory_lazy_load() {
    let p = TempProject::new("list-sub");
    p.write("src/lib.rs", b"// lib");
    p.write("src/main.rs", b"// main");
    let src = p.path().join("src");
    let entries = list_dir_pub(&p.path(), src.to_str().unwrap()).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"lib.rs"));
    assert!(names.contains(&"main.rs"));
    assert_eq!(entries.iter().filter(|e| e.is_dir).count(), 0);
}

#[test]
fn list_dir_rejects_outside_root() {
    let p = TempProject::new("list-out");
    let outside = p.outside_dir();
    let outside_file = outside.join("evil.txt");
    fs::write(&outside_file, b"x").unwrap();
    let res = list_dir_pub(&p.path(), outside.to_str().unwrap());
    assert!(res.is_err(), "应拒绝越界: {res:?}");
    assert!(res.unwrap_err().contains("拒绝访问项目外路径"));
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn read_file_normal() {
    let p = TempProject::new("read-normal");
    let f = p.write("hello.txt", "你好, 世界\nline 2".as_bytes());
    let fc = read_file_pub(&p.path(), f.to_str().unwrap()).unwrap();
    assert_eq!(fc.name, "hello.txt");
    assert_eq!(fc.content, "你好, 世界\nline 2");
    assert!(fc.path.ends_with("hello.txt"));
}

#[test]
fn read_file_rejects_non_file() {
    let p = TempProject::new("read-nonfile");
    let d = p.mkdir("subdir");
    let res = read_file_pub(&p.path(), d.to_str().unwrap());
    assert!(res.is_err(), "目录读应为 Err: {res:?}");
    assert!(res.unwrap_err().contains("不是文件"));
}

#[test]
fn read_file_rejects_huge() {
    let p = TempProject::new("read-huge");
    // 写一个 4MB+1 字节文件
    let mut big = vec![b'a'; (MAX_FILE_BYTES as usize) + 1];
    big.push(b'\n');
    let f = p.write("big.bin", &big);
    let res = read_file_pub(&p.path(), f.to_str().unwrap());
    assert!(res.is_err(), "超 4MB 应被拒绝: {res:?}");
    assert!(res.unwrap_err().contains("文件过大"));
}

#[test]
fn read_file_rejects_non_utf8() {
    let p = TempProject::new("read-bin");
    // 写非法 UTF-8 字节
    let bad = [0xFF, 0xFE, 0xFD, b'\n'];
    let f = p.write("bad.bin", &bad);
    let res = read_file_pub(&p.path(), f.to_str().unwrap());
    assert!(res.is_err(), "非法 UTF-8 应被拒绝: {res:?}");
    assert!(res.unwrap_err().contains("UTF-8"));
}

#[test]
fn read_file_rejects_outside_root() {
    let p = TempProject::new("read-out");
    let outside = p.outside_dir();
    let f = outside.join("secret.txt");
    fs::write(&f, b"top secret").unwrap();
    let res = read_file_pub(&p.path(), f.to_str().unwrap());
    assert!(res.is_err(), "应拒绝越界: {res:?}");
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn write_file_then_read_back() {
    let p = TempProject::new("write-normal");
    let target = p.path().join("new.txt");
    let n = write_file_pub(&p.path(), target.to_str().unwrap(), "写入的内容\n第二行").unwrap();
    assert_eq!(n, "写入的内容\n第二行".len());
    let fc = read_file_pub(&p.path(), target.to_str().unwrap()).unwrap();
    assert_eq!(fc.content, "写入的内容\n第二行");
}

#[test]
fn write_file_overwrites_existing() {
    let p = TempProject::new("write-overwrite");
    let target = p.write("f.txt", b"old");
    write_file_pub(&p.path(), target.to_str().unwrap(), "new").unwrap();
    let fc = read_file_pub(&p.path(), target.to_str().unwrap()).unwrap();
    assert_eq!(fc.content, "new");
}

#[test]
fn write_file_rejects_outside_root() {
    let p = TempProject::new("write-out");
    let outside = p.outside_dir();
    let target = outside.join("evil.txt");
    let res = write_file_pub(&p.path(), target.to_str().unwrap(), "pwn");
    assert!(res.is_err(), "应拒绝越界写入: {res:?}");
    assert!(!target.exists(), "越界文件不应被创建");
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn resolve_in_project_no_root() {
    // root 不存在 → Err (canonicalize 失败)
    let bogus = std::env::temp_dir().join("ide-shell-desktop-bogus-root-xyz");
    let _ = fs::remove_dir_all(&bogus);
    let res = resolve_in_project_pub(&bogus, "anything");
    assert!(res.is_err(), "不存在的根应 Err: {res:?}");
}

#[test]
fn resolve_in_project_target_in_subdir() {
    let p = TempProject::new("resolve-sub");
    let sub = p.mkdir("a/b");
    let f = sub.join("c.txt");
    fs::write(&f, "x").unwrap();
    let res = resolve_in_project_pub(&p.path(), f.to_str().unwrap());
    assert!(res.is_ok(), "子目录中的文件应 OK: {res:?}");
}

#[test]
fn help_wiki_text_covers_required() {
    let w = help_wiki_text();
    assert!(w.len() > 2000);
    for section in ["简介", "安装与启动", "界面布局", "项目导入", "Vim 键位表", "Shell 面板", "鼠标悬停说明", "FAQ"] {
        assert!(w.contains(section), "缺段: {section}");
    }
    for cmd in [":w", ":q", ":wq", ":e", ":help", "Ctrl+S", "Ctrl+`", "--help"] {
        assert!(w.contains(cmd), "缺命令: {cmd}");
    }
    // 新加的 vim 命令也要在
    for k in ["dd", "yy", "p", "u", "v", "gg", "G", "w", "b"] {
        assert!(w.contains(k), "vim 键缺: {k}");
    }
}