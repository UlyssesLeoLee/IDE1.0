// auto_backup.rs — 自動バックアップ (sakura 7.7)
//
// 定期的 (N 秒/分) にプロジェクトルートに .sakura-backup を作成.
// 元ファイル名 + タイムスタンプ でユニーク化.
//
// 設計:
// - デフォルト 5 分間隔
// - バックアップ先: プロジェクトルート/.sakura-backup/<original>.<timestamp>
// - 最大 10 世代保持 (古いものから削除)
// - 0 重型依存 (filesystem のみ使用)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// バックアップ設定
#[derive(Debug, Clone)]
pub struct BackupConfig {
    pub interval_secs: u64,
    pub max_generations: usize,
    pub backup_dir: String,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300,
            max_generations: 10,
            backup_dir: ".sakura-backup".to_string(),
        }
    }
}

/// 自動バックアップ管理
#[derive(Debug)]
pub struct AutoBackup {
    config: BackupConfig,
    last_backup: u64,
}

impl AutoBackup {
    pub fn new(config: BackupConfig) -> Self {
        Self {
            config,
            last_backup: 0,
        }
    }

    pub fn with_default() -> Self {
        Self::new(BackupConfig::default())
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn should_backup(&self) -> bool {
        let now = Self::now();
        now >= self.last_backup + self.config.interval_secs
    }

    pub fn backup_file(&mut self, file: &Path) -> Result<PathBuf, BackupError> {
        if !file.exists() {
            return Err(BackupError::FileNotFound(file.to_path_buf()));
        }
        if !file.is_file() {
            return Err(BackupError::NotAFile(file.to_path_buf()));
        }

        let backup_root = self.backup_root_for(file)?;
        fs::create_dir_all(&backup_root)?;

        let timestamp = Self::now();
        let file_name = file
            .file_name()
            .ok_or_else(|| BackupError::InvalidFileName(file.to_path_buf()))?;
        let backup_name = format!("{}.{}", file_name.to_string_lossy(), timestamp);
        let backup_path = backup_root.join(&backup_name);

        fs::copy(file, &backup_path)?;

        self.last_backup = timestamp;
        self.prune_old_generations(&backup_root, file)?;

        Ok(backup_path)
    }

    fn backup_root_for(&self, file: &Path) -> Result<PathBuf, BackupError> {
        let parent = file
            .parent()
            .ok_or_else(|| BackupError::NoParentDir(file.to_path_buf()))?;
        Ok(parent.join(&self.config.backup_dir))
    }

    fn prune_old_generations(
        &self,
        backup_root: &Path,
        original: &Path,
    ) -> Result<(), BackupError> {
        let prefix = original
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if prefix.is_empty() {
            return Ok(());
        }

        let mut entries: Vec<_> = fs::read_dir(backup_root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .collect();

        entries.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
        entries.reverse();

        for entry in entries.iter().skip(self.config.max_generations) {
            let _ = fs::remove_file(entry.path());
        }
        Ok(())
    }

    pub fn restore_from(backup: &Path, target: &Path) -> Result<(), BackupError> {
        if !backup.exists() {
            return Err(BackupError::FileNotFound(backup.to_path_buf()));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(backup, target)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum BackupError {
    FileNotFound(PathBuf),
    NotAFile(PathBuf),
    InvalidFileName(PathBuf),
    NoParentDir(PathBuf),
    Io(std::io::Error),
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound(p) => write!(f, "File not found: {}", p.display()),
            Self::NotAFile(p) => write!(f, "Not a file: {}", p.display()),
            Self::InvalidFileName(p) => write!(f, "Invalid file name: {}", p.display()),
            Self::NoParentDir(p) => write!(f, "No parent directory: {}", p.display()),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for BackupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn make_temp_file(name: &str, content: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("sakura-rs-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_backup_creates_file() {
        let file = make_temp_file("test1.txt", "hello");
        let mut ab = AutoBackup::with_default();
        ab.config.interval_secs = 0;
        let backup = ab.backup_file(&file).unwrap();
        assert!(backup.exists());
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn test_should_backup_respects_interval() {
        let mut ab = AutoBackup::with_default();
        ab.last_backup = AutoBackup::now();
        assert!(!ab.should_backup());
    }

    #[test]
    fn test_max_generations() {
        let file = make_temp_file("test2.txt", "x");
        let mut ab = AutoBackup::new(BackupConfig {
            interval_secs: 0,
            max_generations: 2,
            backup_dir: ".sakura-backup".to_string(),
        });
        for _ in 0..3 {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let _ = ab.backup_file(&file);
        }
        let backup_root = file.parent().unwrap().join(".sakura-backup");
        let count = fs::read_dir(&backup_root).unwrap().count();
        assert!(count <= 2, "max_generations exceeded: {count}");
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn test_restore() {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("sakura-rs-test-{}-{}-restore", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        let backup = dir.join("restore.bak");
        let dst = dir.join("restore_dst.txt");
        fs::write(&backup, "data").unwrap();
        AutoBackup::restore_from(&backup, &dst).unwrap();
        assert_eq!(fs::read_to_string(&dst).unwrap(), "data");
        let _ = fs::remove_dir_all(&dir);
    }
}
