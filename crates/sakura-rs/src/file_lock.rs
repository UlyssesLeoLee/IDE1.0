// file_lock.rs — ファイル排他制御 (sakura 7.6)
//
// Windows: LockFileEx + UNICODE ファイル名
// Unix: flock() 相当 (fs2 crate なし, 自作: ロックファイル方式)
//
// シンプル化のため, クロスプラットフォームで「ロックファイル」方式を採用.
// ロックファイル: <target>.lock

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// ファイルロック管理
#[derive(Debug)]
pub struct FileLock {
    /// ロック対象ファイル
    pub target: PathBuf,
    /// ロックファイルパス
    lock_path: PathBuf,
    /// ロック所有者 (PID + timestamp)
    owner: String,
    /// ファイルハンドル (保持中)
    _handle: Option<File>,
}

impl FileLock {
    /// ロック試行
    /// `wait` で指定時間待機 (None = 失敗時すぐ return)
    pub fn try_lock(target: &Path, wait: Option<Duration>) -> io::Result<Self> {
        let lock_path = Self::lock_path_for(target);
        let owner = format!("{}-{}", std::process::id(), SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0));

        let deadline = wait.map(|d| SystemTime::now() + d);

        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", owner);
                    return Ok(Self {
                        target: target.to_path_buf(),
                        lock_path,
                        owner,
                        _handle: Some(f),
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // 既存ロック確認
                    if let Ok(content) = fs::read_to_string(&lock_path) {
                        // 自分が所有者なら OK
                        if content.trim() == owner {
                            return Ok(Self {
                                target: target.to_path_buf(),
                                lock_path,
                                owner,
                                _handle: None,
                            });
                        }
                        // 古いロック (>30s) なら奪取
                        if Self::is_stale_lock(&lock_path) {
                            let _ = fs::remove_file(&lock_path);
                            continue;
                        }
                    }
                    // 待機判定
                    if let Some(deadline) = deadline {
                        if SystemTime::now() < deadline {
                            std::thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        format!("File is locked: {}", target.display()),
                    ));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// ロック解除
    pub fn unlock(&mut self) -> io::Result<()> {
        // 自分が所有者か確認
        if let Ok(content) = fs::read_to_string(&self.lock_path) {
            if content.trim() == self.owner {
                let _ = fs::remove_file(&self.lock_path);
            }
        }
        self._handle = None;
        Ok(())
    }

    /// ロックファイルパス
    pub fn lock_path_for(target: &Path) -> PathBuf {
        let mut s = target.as_os_str().to_owned();
        s.push(".lock");
        PathBuf::from(s)
    }

    /// 30 秒以上古いロック = stale
    fn is_stale_lock(lock_path: &Path) -> bool {
        if let Ok(meta) = fs::metadata(lock_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(d) = SystemTime::now().duration_since(modified) {
                    return d > Duration::from_secs(30);
                }
            }
        }
        false
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("sakura-rs-lock-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = File::create(&path).unwrap();
        f.write_all(b"data").unwrap();
        path
    }

    #[test]
    fn test_acquire_and_release() {
        let target = temp_path("a.txt");
        let lock_path = FileLock::lock_path_for(&target);
        let mut lock = FileLock::try_lock(&target, None).unwrap();
        assert!(lock_path.exists());
        lock.unlock().unwrap();
        assert!(!lock_path.exists());
        let _ = fs::remove_dir_all(target.parent().unwrap());
    }

    #[test]
    fn test_double_lock_fails() {
        let target = temp_path("b.txt");
        let _lock1 = FileLock::try_lock(&target, None).unwrap();
        let lock2 = FileLock::try_lock(&target, Some(Duration::from_millis(200)));
        assert!(lock2.is_err());
        let _ = fs::remove_dir_all(target.parent().unwrap());
    }

    #[test]
    fn test_drop_releases() {
        let target = temp_path("c.txt");
        let lock_path = FileLock::lock_path_for(&target);
        {
            let _lock = FileLock::try_lock(&target, None).unwrap();
            assert!(lock_path.exists());
        }
        // Drop 後
        assert!(!lock_path.exists());
        let _ = fs::remove_dir_all(target.parent().unwrap());
    }

    #[test]
    fn test_stale_lock_takeover() {
        let target = temp_path("d.txt");
        let lock_path = FileLock::lock_path_for(&target);

        // 古いロックファイル作成
        fs::write(&lock_path, "old-pid-0\n").unwrap();
        // mtime を 31 秒前に設定
        let old = SystemTime::now() - Duration::from_secs(31);
        let _ = filetime_set(&lock_path, old);

        // 取得できるはず (stale なので奪取)
        let _lock = FileLock::try_lock(&target, None).unwrap();
        let _ = fs::remove_dir_all(target.parent().unwrap());
    }

    fn filetime_set(path: &Path, t: SystemTime) -> io::Result<()> {
        let f = File::options().write(true).open(path)?;
        f.set_modified(t)?;
        Ok(())
    }
}
