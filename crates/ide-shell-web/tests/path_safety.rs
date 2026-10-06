//! path_safety 单元测试 — 沙箱核心 (resolve/list/read/write).
//!
//! 目标覆盖率: ≥ 90% lines + ≥ 80% branches (per repo standard).
//!
//! 这些测试是 hermetic — 在 tempdir 内构造文件, 不读 IDE 工作区.
//!
//! 覆盖 functions: strip_verbatim / safe_canonicalize / path_within /
//! resolve_in_project / list_dir / read_file / write_file + error paths.

use ide_shell_web::path_safety::{
    list_dir, path_within, read_file, resolve_in_project, safe_canonicalize, strip_verbatim,
    write_file, MAX_FILE_BYTES,
};
use std::fs;
use std::path::{Path, PathBuf};

fn tempdir_name() -> &'static str {
    // 仅一个根 dir 名字 — setup() 自己 attach 唯一 id
    "ide_shell_web_test"
}

/// 每个测试自己的子目录 — 防止 parallel tests 干扰.
fn setup(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "{}_{}_{}_{}_{}",
        tempdir_name(),
        label,
        std::process::id(),
        n,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // 失败 ignore — Windows AV 可能瞬时 lock
    let _ = fs::remove_dir_all(&dir);
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("setup failed for {:?}: {}", dir, e);
    }
    dir
}

#[test]
fn test_strip_verbatim_no_prefix() {
    let p = Path::new("C:/work/foo.txt");
    let out = strip_verbatim(p);
    // Not Windows verbatim, should return same path
    assert_eq!(out.to_string_lossy(), p.to_string_lossy());
}

#[test]
#[cfg(windows)]
fn test_strip_verbatim_windows_prefix() {
    let raw = PathBuf::from(r"\\?\D:\work\foo.txt");
    let stripped = strip_verbatim(&raw);
    assert_eq!(stripped.to_string_lossy(), r"D:\work\foo.txt");
    assert!(!stripped.to_string_lossy().starts_with(r"\\?\"));
}

#[test]
fn test_safe_canonicalize_existing() {
    let dir = setup("canonicalize_existing");
    let file = dir.join("hello.txt");
    fs::write(&file, "x").unwrap();
    let canon = safe_canonicalize(&file).unwrap();
    assert!(canon.ends_with("hello.txt"));
}

#[test]
fn test_safe_canonicalize_nonexistent_walk_back() {
    // 不存在的 file, 应能 reconstruct parent + basename
    let dir = setup("canonicalize_new");
    let new_file = dir.join("not_yet_created.txt");
    assert!(!new_file.exists());
    let canon = safe_canonicalize(&new_file).unwrap();
    assert!(canon.to_string_lossy().ends_with("not_yet_created.txt"));
    assert!(canon
        .to_string_lossy()
        .contains(dir.file_name().unwrap().to_str().unwrap()));
}

#[test]
fn test_safe_canonicalize_top_level_nonexistent() {
    // root 不存在 + parent 也 None — 应返回 NotFound
    let p = Path::new("Z:/definitely_no_such_dir_xyz/abc");
    assert!(safe_canonicalize(p).is_err());
}

#[test]
fn test_path_within_inside() {
    let dir = setup("within_inside");
    let inner = dir.join("sub");
    fs::create_dir_all(&inner).unwrap();
    assert!(path_within(&dir, &inner));
}

#[test]
fn test_path_within_outside() {
    let dir_a = setup("within_out_a");
    let dir_b = setup("within_out_b");
    assert!(!path_within(&dir_a, &dir_b));
}

#[test]
fn test_path_within_nonexistent_target() {
    // target 不存在 — 应 reconstruct & still inside (path 解析 OK)
    let dir = setup("within_nonexist");
    let fake = dir.join("nope.txt");
    // 不存在的 target: canonicalize 返回 path_with parent canonical + basename
    // 应该仍在 dir 内 (因为 parent 是 dir)
    assert!(path_within(&dir, &fake));
}

#[test]
fn test_resolve_in_project_inside() {
    let dir = setup("resolve_inside");
    let inner = dir.join("inside.txt");
    fs::write(&inner, "x").unwrap();
    let r = resolve_in_project(&dir, "inside.txt").unwrap();
    assert!(r.to_string_lossy().contains("inside.txt"));
}

#[test]
fn test_resolve_in_project_outside() {
    let dir = setup("resolve_outside");
    // Use a fully-separate absolute path that we know exists
    // (Temp dir root itself, NOT inside our `dir`)
    let outside_dir = std::env::temp_dir().join(format!(
        "ide_shell_web_test_outside_root_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outside_dir);
    fs::create_dir_all(&outside_dir).unwrap();
    let file = outside_dir.join("x.txt");
    fs::write(&file, "x").unwrap();
    let r = resolve_in_project(&dir, file.to_str().unwrap());
    let _ = fs::remove_dir_all(&outside_dir);
    assert!(r.is_err());
}

#[test]
fn test_resolve_in_project_invalid_root() {
    let bad_root = Path::new("Z:/non_existent_xyz_999");
    let r = resolve_in_project(bad_root, "x.txt");
    assert!(r.is_err());
}

#[test]
fn test_list_dir_returns_dirs_first() {
    let dir = setup("list_dir_mixed");
    fs::create_dir(dir.join("src")).unwrap();
    fs::write(dir.join("z.txt"), "").unwrap();
    fs::write(dir.join("a.txt"), "").unwrap();

    let entries = list_dir(&dir, ".").unwrap();
    // 必须 ≥ 3 entries (src/z.txt/a.txt)
    assert!(entries.len() >= 3);
    // dirs 排在前面
    let first_dir_idx = entries.iter().position(|e| e.is_dir).unwrap();
    let first_file_idx = entries.iter().position(|e| !e.is_dir).unwrap();
    assert!(first_dir_idx < first_file_idx);
    // 大小写不敏感排序
    let files: Vec<&str> = entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.name.as_str())
        .collect();
    let a_pos = files.iter().position(|n| *n == "a.txt").unwrap();
    let z_pos = files.iter().position(|n| *n == "z.txt").unwrap();
    assert!(a_pos < z_pos, "a.txt should come before z.txt");
}

#[test]
fn test_list_dir_not_a_directory() {
    let dir = setup("list_dir_not_dir");
    let file = dir.join("regular.txt");
    fs::write(&file, "x").unwrap();
    let r = list_dir(&dir, "regular.txt");
    assert!(r.is_err());
    let msg = r.unwrap_err();
    assert!(msg.contains("不是目录"));
}

#[test]
fn test_list_dir_outside_sandbox() {
    let dir = setup("list_dir_outside");
    let outside = std::env::temp_dir().join(format!(
        "ide_shell_web_test_outside_listdir_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();
    let r = list_dir(&dir, outside.to_str().unwrap());
    let _ = fs::remove_dir_all(&outside);
    assert!(r.is_err());
}

#[test]
fn test_read_file_success() {
    let dir = setup("read_ok");
    let file = dir.join("hello.txt");
    fs::write(&file, "hello world").unwrap();
    let fc = read_file(&dir, "hello.txt").unwrap();
    assert_eq!(fc.content, "hello world");
    assert!(fc.path.to_lowercase().contains("hello.txt"));
    assert!(!fc.name.is_empty());
}

#[test]
fn test_read_file_not_a_file() {
    let dir = setup("read_not_file");
    fs::create_dir(dir.join("subdir")).unwrap();
    let r = read_file(&dir, "subdir");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("不是文件"));
}

#[test]
fn test_read_file_too_large() {
    let dir = setup("read_too_large");
    let file = dir.join("big.txt");
    // 写超过 MAX_FILE_BYTES 的内容 (但故意只部分写, 测试早期返回)
    use std::io::Write;
    let mut f = fs::File::create(&file).unwrap();
    // 写 1 MB 然后看 size — 测试 size check (file 实际只有 1MB, 不会真到 4MB+)
    let chunk = vec![b'x'; 1024 * 1024]; // 1 MB
    f.write_all(&chunk).unwrap();
    f.write_all(&chunk).unwrap();
    f.write_all(&chunk).unwrap();
    f.write_all(&chunk).unwrap();
    drop(f);

    // 4 MB 文件 read 应过 (still under limit)
    let r = read_file(&dir, "big.txt");
    assert!(r.is_ok());

    // 写超 limit (5 MB) → 应错
    let f2 = dir.join("huge.txt");
    let mut f2w = fs::File::create(&f2).unwrap();
    let chunk = vec![b'y'; 1024 * 1024];
    for _ in 0..5 {
        f2w.write_all(&chunk).unwrap();
    }
    drop(f2w);

    let r2 = read_file(&dir, "huge.txt");
    assert!(r2.is_err());
    assert!(r2.unwrap_err().contains("文件过大"));
}

#[test]
fn test_read_file_non_utf8() {
    let dir = setup("read_non_utf8");
    let file = dir.join("binary.bin");
    // 写 invalid UTF-8 字节
    fs::write(&file, [0xFF, 0xFE, 0xFD, 0x00, 0x01]).unwrap();
    let r = read_file(&dir, "binary.bin");
    // 不是 valid UTF-8 → read_to_string 错
    assert!(r.is_err());
}

#[test]
fn test_read_file_outside_sandbox() {
    let dir = setup("read_outside");
    let outside = std::env::temp_dir().join(format!(
        "ide_shell_web_test_outside_read_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("x.txt"), "x").unwrap();
    let r = read_file(&dir, outside.join("x.txt").to_str().unwrap());
    let _ = fs::remove_dir_all(&outside);
    assert!(r.is_err());
}

#[test]
fn test_write_file_success() {
    let dir = setup("write_ok");
    let n = write_file(&dir, "new.txt", "content body").unwrap();
    assert_eq!(n, "content body".len());
    assert_eq!(
        fs::read_to_string(dir.join("new.txt")).unwrap(),
        "content body"
    );
}

#[test]
fn test_write_file_overwrite() {
    let dir = setup("write_overwrite");
    fs::write(dir.join("f.txt"), "old").unwrap();
    write_file(&dir, "f.txt", "new").unwrap();
    assert_eq!(fs::read_to_string(dir.join("f.txt")).unwrap(), "new");
}

#[test]
fn test_write_file_outside_sandbox() {
    let dir = setup("write_outside");
    let outside = std::env::temp_dir().join(format!(
        "ide_shell_web_test_outside_write_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();
    let r = write_file(&dir, outside.join("new.txt").to_str().unwrap(), "x");
    let _ = fs::remove_dir_all(&outside);
    assert!(r.is_err());
}

#[test]
fn test_max_file_bytes_constant() {
    // 4 MB
    assert_eq!(MAX_FILE_BYTES, 4 * 1024 * 1024);
}

#[test]
fn test_path_within_drive_letter_match() {
    // strip_verbatim: 两个 path 在 drive 级别必须 match
    let dir = setup("drive_letter");
    let inner = dir.join("inside");
    fs::create_dir_all(&inner).unwrap();
    // 构造一个 root inside path 跑 path_within
    assert!(path_within(&dir, &inner));
}

#[test]
fn test_strip_verbatim_preserves_trailing_path() {
    #[cfg(windows)]
    {
        let raw = PathBuf::from(r"\\?\C:\foo\bar\baz.txt");
        let stripped = strip_verbatim(&raw);
        assert!(stripped.to_string_lossy().ends_with(r"bar\baz.txt"));
    }
}
