// auto_backup.rs — 自動バックアップ (参考实现 7.7)
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
        let dir = std::env::temp_dir().join(format!(
            "sakura-rs-test-{}-{}-restore",
            std::process::id(),
            n
        ));
        fs::create_dir_all(&dir).unwrap();
        let backup = dir.join("restore.bak");
        let dst = dir.join("restore_dst.txt");
        fs::write(&backup, "data").unwrap();
        AutoBackup::restore_from(&backup, &dst).unwrap();
        assert_eq!(fs::read_to_string(&dst).unwrap(), "data");
        let _ = fs::remove_dir_all(&dir);
    }
}

// ============================================================ 保存时备份 ===
// WBS-02 1.7: 覆盖保存之前, 把磁盘上的原文件按字节复制到备份目录 (所以天然保留文档原编码/BOM/换行),
// 命名可选 "原名.时间戳" 或 "原名.1 / .2 ..." (1 = 最新), 超过代数的旧备份删除。

/// 备份命名方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupNaming {
    /// `<原名>.<YYYYMMDD-HHMMSS-mmm>` (UTC)
    Timestamp,
    /// `<原名>.1` 最新, `.2` 次新 ... 每次备份先把已有的编号顺延
    Numbered,
}

/// 保存时备份的配置。
#[derive(Debug, Clone)]
pub struct SaveBackupConfig {
    pub enabled: bool,
    /// 相对路径 = 相对被保存文件所在目录的子目录; 绝对路径 = 统一备份目录 (不同目录下的同名文件会共用代数)
    pub dir: String,
    pub naming: BackupNaming,
    /// 保留的代数 (最少 1)
    pub generations: usize,
}

impl Default for SaveBackupConfig {
    fn default() -> Self {
        Self { enabled: true, dir: ".sakura-backup".to_string(), naming: BackupNaming::Timestamp, generations: 5 }
    }
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 毫秒时间戳 -> `YYYYMMDD-HHMMSS-mmm` (UTC)。
pub fn format_backup_timestamp(unix_ms: u64) -> String {
    let secs = (unix_ms / 1000) as i64;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!("{y:04}{mo:02}{d:02}-{:02}{:02}{:02}-{:03}", s / 3600, (s % 3600) / 60, s % 60, unix_ms % 1000)
}

fn is_timestamp_suffix(s: &str) -> bool {
    // YYYYMMDD-HHMMSS-mmm, 同毫秒冲突时可能再带 "-n"
    let parts: Vec<&str> = s.split('-').collect();
    (parts.len() == 3 || parts.len() == 4)
        && parts[0].len() == 8 && parts[1].len() == 6 && parts[2].len() == 3
        && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// 备份目录的绝对位置。
pub fn backup_dir_for(file: &Path, cfg: &SaveBackupConfig) -> Result<PathBuf, BackupError> {
    let d = Path::new(&cfg.dir);
    if d.is_absolute() { return Ok(d.to_path_buf()); }
    let parent = file.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    Ok(parent.join(d))
}

/// 列出 `file` 已有的备份 (新 -> 旧)。
pub fn list_save_backups(file: &Path, cfg: &SaveBackupConfig) -> Result<Vec<PathBuf>, BackupError> {
    let root = backup_dir_for(file, cfg)?;
    let name = file.file_name().ok_or_else(|| BackupError::InvalidFileName(file.to_path_buf()))?.to_string_lossy().into_owned();
    let prefix = format!("{name}.");
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let rd = match fs::read_dir(&root) { Ok(r) => r, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()), Err(e) => return Err(e.into()) };
    for e in rd.filter_map(|e| e.ok()) {
        let fname = e.file_name().to_string_lossy().into_owned();
        let Some(suffix) = fname.strip_prefix(&prefix) else { continue };
        let ok = match cfg.naming {
            BackupNaming::Timestamp => is_timestamp_suffix(suffix),
            BackupNaming::Numbered => !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()),
        };
        if ok { found.push((suffix.to_string(), e.path())); }
    }
    match cfg.naming {
        // 时间戳字典序 = 时间序; 同毫秒的 "-n" 变体排在无后缀之后 (更新)
        BackupNaming::Timestamp => found.sort_by(|a, b| b.0.cmp(&a.0)),
        BackupNaming::Numbered => found.sort_by_key(|(s, _)| s.parse::<u64>().unwrap_or(u64::MAX)),
    }
    Ok(found.into_iter().map(|(_, p)| p).collect())
}

/// 覆盖保存之前调用: 把磁盘上现有的 `file` 原样备份。
/// 文件不存在 (新文档第一次保存) 或备份被关闭时返回 `Ok(None)`。
/// 备份失败返回 Err, 由调用方决定是否继续保存 (建议提示用户而不是静默吞掉)。
pub fn backup_before_save(file: &Path, cfg: &SaveBackupConfig) -> Result<Option<PathBuf>, BackupError> {
    backup_before_save_at(file, cfg, SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
}

/// 同上, 时间由调用方给出 (便于测试)。
pub fn backup_before_save_at(file: &Path, cfg: &SaveBackupConfig, now_ms: u64) -> Result<Option<PathBuf>, BackupError> {
    if !cfg.enabled { return Ok(None); }
    match fs::metadata(file) {
        Ok(m) if m.is_file() => {}
        Ok(_) => return Err(BackupError::NotAFile(file.to_path_buf())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let gens = cfg.generations.max(1);
    let root = backup_dir_for(file, cfg)?;
    fs::create_dir_all(&root)?;
    let name = file.file_name().ok_or_else(|| BackupError::InvalidFileName(file.to_path_buf()))?.to_string_lossy().into_owned();

    let target = match cfg.naming {
        BackupNaming::Timestamp => {
            let base = format!("{name}.{}", format_backup_timestamp(now_ms));
            let mut p = root.join(&base);
            let mut n = 1;
            while p.exists() { p = root.join(format!("{base}-{n}")); n += 1; }
            p
        }
        BackupNaming::Numbered => {
            // 顺延: 先删掉会超出代数的, 再从大到小改名
            let existing = list_save_backups(file, cfg)?; // 按编号升序
            let mut by_num: Vec<(u64, PathBuf)> = existing.into_iter()
                .filter_map(|p| p.file_name().and_then(|f| f.to_str()).and_then(|f| f.rsplit('.').next().and_then(|s| s.parse::<u64>().ok())).map(|n| (n, p)))
                .collect();
            by_num.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
            for (n, p) in by_num {
                if (n as usize) + 1 > gens { let _ = fs::remove_file(&p); }
                else { fs::rename(&p, root.join(format!("{name}.{}", n + 1)))?; }
            }
            root.join(format!("{name}.1"))
        }
    };
    // 先复制到临时名再改名, 避免留下半截备份
    let tmp = root.join(format!(".{name}.backup-{}.tmp", std::process::id()));
    if let Err(e) = fs::copy(file, &tmp) { let _ = fs::remove_file(&tmp); return Err(e.into()); }
    if let Err(e) = fs::rename(&tmp, &target) { let _ = fs::remove_file(&tmp); return Err(e.into()); }

    if cfg.naming == BackupNaming::Timestamp {
        for old in list_save_backups(file, cfg)?.into_iter().skip(gens) { let _ = fs::remove_file(old); }
    }
    Ok(Some(target))
}

#[cfg(test)]
mod save_backup_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("sakura-sb-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn cfg(naming: BackupNaming, gens: usize) -> SaveBackupConfig {
        SaveBackupConfig { enabled: true, dir: "bk".into(), naming, generations: gens }
    }

    #[test]
    fn timestamp_format_is_utc_and_sortable() {
        assert_eq!(format_backup_timestamp(0), "19700101-000000-000");
        assert_eq!(format_backup_timestamp(951_782_400_000 + 3_723_004), "20000229-010203-004");
        assert!(is_timestamp_suffix("20000229-010203-004"));
        assert!(is_timestamp_suffix("20000229-010203-004-2"));
        assert!(!is_timestamp_suffix("tmp"));
    }

    #[test]
    fn backup_is_a_byte_copy_keeping_document_encoding() {
        let d = tdir();
        let f = d.join("doc.txt");
        // SJIS + CRLF 的原始字节, 备份必须逐字节相同
        let raw: Vec<u8> = vec![0x93, 0xFA, 0x96, 0x7B, 0x0D, 0x0A, 0x61];
        fs::write(&f, &raw).unwrap();
        let b = backup_before_save_at(&f, &cfg(BackupNaming::Timestamp, 3), 1_000).unwrap().unwrap();
        assert_eq!(fs::read(&b).unwrap(), raw);
        assert!(b.starts_with(d.join("bk")));
        assert_eq!(b.file_name().unwrap().to_string_lossy(), "doc.txt.19700101-000001-000");
        assert_eq!(fs::read(&f).unwrap(), raw, "原文件不动");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn new_file_and_disabled_make_no_backup() {
        let d = tdir();
        let f = d.join("new.txt");
        assert!(backup_before_save(&f, &cfg(BackupNaming::Timestamp, 3)).unwrap().is_none());
        assert!(!d.join("bk").exists());
        fs::write(&f, "x").unwrap();
        let mut c = cfg(BackupNaming::Timestamp, 3);
        c.enabled = false;
        assert!(backup_before_save(&f, &c).unwrap().is_none());
        assert!(!d.join("bk").exists());
        assert!(matches!(backup_before_save(&d, &cfg(BackupNaming::Timestamp, 3)), Err(BackupError::NotAFile(_))));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn timestamp_generations_are_pruned_oldest_first() {
        let d = tdir();
        let f = d.join("a.txt");
        let c = cfg(BackupNaming::Timestamp, 2);
        for i in 0..4u64 {
            fs::write(&f, format!("v{i}")).unwrap();
            backup_before_save_at(&f, &c, 1_000 * (i + 1)).unwrap();
        }
        // 无关文件不能被误删
        fs::write(d.join("bk").join("a.txt.notes"), "keep").unwrap();
        fs::write(d.join("bk").join("b.txt.19700101-000001-000"), "other").unwrap();
        backup_before_save_at(&f, &c, 5_000).unwrap();
        let list = list_save_backups(&f, &c).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(fs::read_to_string(&list[0]).unwrap(), "v3", "最新在前");
        assert_eq!(fs::read_to_string(&list[1]).unwrap(), "v3");
        assert!(d.join("bk").join("a.txt.notes").exists());
        assert!(d.join("bk").join("b.txt.19700101-000001-000").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn same_millisecond_does_not_overwrite() {
        let d = tdir();
        let f = d.join("a.txt");
        let c = cfg(BackupNaming::Timestamp, 5);
        fs::write(&f, "one").unwrap();
        let p1 = backup_before_save_at(&f, &c, 7_000).unwrap().unwrap();
        fs::write(&f, "two").unwrap();
        let p2 = backup_before_save_at(&f, &c, 7_000).unwrap().unwrap();
        assert_ne!(p1, p2);
        assert_eq!(fs::read_to_string(&p1).unwrap(), "one");
        let list = list_save_backups(&f, &c).unwrap();
        assert_eq!(list[0], p2, "同毫秒冲突的后一份排在前面");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn numbered_generations_shift_and_cap() {
        let d = tdir();
        let f = d.join("n.txt");
        let c = cfg(BackupNaming::Numbered, 3);
        for i in 1..=5 {
            fs::write(&f, format!("v{i}")).unwrap();
            backup_before_save(&f, &c).unwrap();
        }
        let bk = d.join("bk");
        assert_eq!(fs::read_to_string(bk.join("n.txt.1")).unwrap(), "v5");
        assert_eq!(fs::read_to_string(bk.join("n.txt.2")).unwrap(), "v4");
        assert_eq!(fs::read_to_string(bk.join("n.txt.3")).unwrap(), "v3");
        assert!(!bk.join("n.txt.4").exists());
        assert_eq!(list_save_backups(&f, &c).unwrap().len(), 3);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn absolute_backup_dir_and_zero_generations_clamped() {
        let d = tdir();
        let store = tdir();
        let f = d.join("z.txt");
        fs::write(&f, "z").unwrap();
        let c = SaveBackupConfig { enabled: true, dir: store.to_string_lossy().into_owned(), naming: BackupNaming::Numbered, generations: 0 };
        let b = backup_before_save(&f, &c).unwrap().unwrap();
        assert_eq!(b, store.join("z.txt.1"));
        fs::write(&f, "z2").unwrap();
        backup_before_save(&f, &c).unwrap();
        assert_eq!(list_save_backups(&f, &c).unwrap().len(), 1, "代数最少 1");
        assert_eq!(fs::read_to_string(store.join("z.txt.1")).unwrap(), "z2");
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&store);
    }

    #[test]
    fn backup_then_atomic_save_flow() {
        use crate::document_io::{atomic_write, AtomicWriteOpts};
        let d = tdir();
        let f = d.join("flow.txt");
        fs::write(&f, "old").unwrap();
        let c = cfg(BackupNaming::Numbered, 2);
        backup_before_save(&f, &c).unwrap();
        atomic_write(&f, b"new", AtomicWriteOpts::default()).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "new");
        assert_eq!(fs::read_to_string(d.join("bk").join("flow.txt.1")).unwrap(), "old");
        let _ = fs::remove_dir_all(&d);
    }
}
