//! 原子写盘: 同目录唯一临时文件 + 换名; 出错时清掉临时文件; `private` 时在 unix 上只允许属主读写 (0600)。
//! 备份、设置、AI 配置 (含密钥)、导出都走这里, 不再各写一份。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// 同目录下不会撞名的临时文件路径 (进程号 + 自增序号, 多实例 / 多线程同时写同一个文件也不互相踩)
fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)))
}

/// 同目录下唯一的临时文件路径 `<stem>.<进程号>.<序号>.<ext>` (下载更新包用; 不创建文件, 调用方负责写入和清理)
pub fn unique_temp(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    dir.join(format!("{stem}.{}.{}.{ext}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)))
}

/// 原子写。`private` = 内容含密钥, unix 上文件权限 0600 (Windows 靠用户目录的 ACL)。
pub fn atomic_write(path: &Path, bytes: &[u8], private: bool) -> std::io::Result<()> {
    let tmp = temp_path(path);
    let r = write_tmp(&tmp, bytes, private).and_then(|()| std::fs::rename(&tmp, path));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

fn write_tmp(tmp: &Path, bytes: &[u8], private: bool) -> std::io::Result<()> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut f = o.open(tmp)?;
    f.write_all(bytes)?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("idefast-fsx-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn replaces_and_leaves_no_temp_files() {
        let d = dir("ok");
        let f = d.join("a.json");
        atomic_write(&f, b"one", false).unwrap();
        atomic_write(&f, b"two", true).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn failure_reports_error_and_cleans_up() {
        let d = dir("err");
        let target = d.join("sub");
        std::fs::create_dir(&target).unwrap(); // 目标是目录: 换名必失败
        assert!(atomic_write(&target, b"x", false).is_err());
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "临时文件应被清掉");
        assert!(atomic_write(&d.join("no/such/dir/f"), b"x", false).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unique_temp_names_differ_and_keep_the_extension() {
        let (a, b) = (unique_temp(Path::new("/d"), "upd", "msi"), unique_temp(Path::new("/d"), "upd", "msi"));
        assert_ne!(a, b);
        assert!(a.to_string_lossy().ends_with(".msi") && a.starts_with("/d"));
    }

    #[test]
    fn temp_names_are_unique() {
        let p = Path::new("/x/a.json");
        assert_ne!(temp_path(p), temp_path(p));
    }

    #[cfg(unix)]
    #[test]
    fn private_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("perm");
        let f = d.join("ai.json");
        atomic_write(&f, b"k", true).unwrap();
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }
}
