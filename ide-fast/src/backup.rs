//! 自动备份定时器与文件锁 (都是可选项, 默认关闭)。
//!
//! 自动备份: 开着时, 每隔 N 分钟把**有未保存修改**的文档的当前内容写到 `<设置目录>/autosave/` (崩溃 / 断电后可从「工具 > 打开自动备份文件」找回),
//! 保存或关闭标签时删掉对应备份。没有脏文档时不排任何定时刷新; 写盘放在一条用完即退的线程里, 不卡界面。
//! 文件锁: 开着时, 打开的文件用 Windows 的「共享读、禁止写」打开并一直持有 —— 别的程序 (含 Tauri 版) 仍可读, 但改不了、删不了、换不了名;
//! 保存前先放掉再重新持有。非 Windows 平台没有等价的强制锁, 不做 (返回 `Unsupported`)。

use crate::doc::Doc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 间隔上下限 (分钟)
pub const MIN_INTERVAL: u32 = 1;
pub const MAX_INTERVAL: u32 = 1440;
/// 单个文档超过这个字节数就不备份 (整篇拼接 + 写盘的成本)
const MAX_DOC_BYTES: usize = 32 * 1024 * 1024;
/// 备份文件保留天数 (更久的在写新备份时顺手清掉: 无标题文档的 id 每次启动都变, 不清会越积越多)
const KEEP_DAYS: u64 = 14;

pub fn clamp_interval(min: u32) -> u32 {
    min.clamp(MIN_INTERVAL, MAX_INTERVAL)
}

/// 备份目录; 设置落盘被关 (`IDE_FAST_SETTINGS=off`) 时没有
pub fn dir() -> Option<PathBuf> {
    crate::settings::path().and_then(|p| p.parent().map(|d| d.join("autosave")))
}

/// 备份文件名: 清洗过的文件名 + 路径 (无标题 = 文档 id) 的指纹, 不同目录的同名文件互不覆盖
pub fn file_name(path: Option<&Path>, name: &str, id: u64) -> String {
    let key = match path {
        Some(p) => p.to_string_lossy().to_lowercase(),
        None => format!("untitled-{id}"),
    };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a
    for b in key.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect();
    format!("{clean}.{h:016x}.autosave")
}

/// 启动时发现的遗留备份 (上次没正常关: 崩溃 / 断电 / 结束进程)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leftover {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    /// 距今多少秒 (拿不到修改时间 = None)
    pub age_secs: Option<u64>,
}

/// 最多列几个 (备份目录被塞满也不卡启动)
const MAX_LEFTOVERS: usize = 40;

/// 扫描备份目录里的 `.autosave` 文件, 新的在前。只认普通文件 (不跟符号链接), 空文件跳过。目录不存在 = 空
pub fn leftovers_in(dir: &Path) -> Vec<Leftover> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let now = std::time::SystemTime::now();
    let mut v: Vec<(std::time::SystemTime, Leftover)> = vec![];
    for e in rd.flatten().take(2000) {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("autosave") {
            continue;
        }
        let Ok(m) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !m.is_file() || m.len() == 0 {
            continue;
        }
        let mt = m.modified().unwrap_or(std::time::UNIX_EPOCH);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // 文件名 = <原名>.<16 位指纹>.autosave, 给人看的只要原名
        let shown = name
            .strip_suffix(".autosave")
            .and_then(|r| r.rsplit_once('.'))
            .map(|(a, _)| a.to_string())
            .unwrap_or(name);
        v.push((
            mt,
            Leftover {
                path,
                name: shown,
                size: m.len(),
                age_secs: now.duration_since(mt).ok().map(|d| d.as_secs()),
            },
        ));
    }
    v.sort_by_key(|a| std::cmp::Reverse(a.0));
    v.into_iter().take(MAX_LEFTOVERS).map(|x| x.1).collect()
}

/// 默认备份目录里的遗留备份
pub fn leftovers() -> Vec<Leftover> {
    dir().map(|d| leftovers_in(&d)).unwrap_or_default()
}

/// 距离下一次备份还有多久; 0 = 到点了
pub fn remaining(last: Instant, now: Instant, interval_min: u32) -> Duration {
    Duration::from_secs(clamp_interval(interval_min) as u64 * 60)
        .saturating_sub(now.saturating_duration_since(last))
}

struct Job {
    file: String,
    text: String,
}

#[derive(Default)]
pub struct AutoBackup {
    last: Option<Instant>,
    /// 文档 id -> 上次备份时的修订号 (没变就不重写)
    done: HashMap<u64, u64>,
}

impl AutoBackup {
    /// 每帧调用。返回「最晚该在多久之后再被叫到」: None = 现在不需要任何定时刷新 (选项关着 / 没有脏文档)。
    pub fn tick(
        &mut self,
        enabled: bool,
        interval_min: u32,
        docs: &[Doc],
        now: Instant,
    ) -> Option<Duration> {
        let dirty = |d: &&Doc| d.dirty && !d.grep_result;
        if !enabled || !docs.iter().any(|d| dirty(&d)) {
            self.last = None; // 下次出现脏文档时从那一刻重新计时
            return None;
        }
        let last = *self.last.get_or_insert(now);
        let left = remaining(last, now, interval_min);
        if !left.is_zero() {
            return Some(left);
        }
        self.last = Some(now);
        let jobs = self.collect_jobs(docs);
        if !jobs.is_empty() {
            if let Some(dir) = dir() {
                let _ = std::thread::Builder::new()
                    .name("autosave".into())
                    .spawn(move || write_all(&dir, &jobs));
            }
        }
        Some(Duration::from_secs(
            clamp_interval(interval_min) as u64 * 60,
        ))
    }

    /// 该备份的文档 → 要写的备份: 只收有未保存修改的 (不含 Grep 结果页), 这一版已经备份过的跳过, 太大的跳过; 收下的记为「已备份这一版」
    fn collect_jobs(&mut self, docs: &[Doc]) -> Vec<Job> {
        let dirty = |d: &&Doc| d.dirty && !d.grep_result;
        let jobs: Vec<Job> = docs
            .iter()
            .filter(dirty)
            .filter(|d| self.done.get(&d.id) != Some(&d.rev))
            .filter_map(|d| {
                let sep = match d.eol {
                    editcore::document_io::Eol::Crlf => "\r\n",
                    editcore::document_io::Eol::Lf => "\n",
                    editcore::document_io::Eol::Cr => "\r",
                };
                let text = d.lines.join(sep);
                (text.len() <= MAX_DOC_BYTES).then(|| Job {
                    file: file_name(d.path.as_deref(), &d.name, d.id),
                    text,
                })
            })
            .collect();
        for d in docs.iter().filter(dirty) {
            self.done.insert(d.id, d.rev);
        }
        jobs
    }

    /// 立即备份 (Tauri 的「立即备份」): 不等定时器, 把所有有未保存修改的文档现在就写进备份目录 (已备份过的这一版也重写); 返回写了几份
    pub fn flush_now(&mut self, docs: &[Doc]) -> Result<usize, String> {
        let dir = dir().ok_or("找不到备份目录")?;
        Ok(self.flush_to(docs, &dir))
    }

    fn flush_to(&mut self, docs: &[Doc], dir: &Path) -> usize {
        for d in docs {
            self.done.remove(&d.id);
        }
        let jobs = self.collect_jobs(docs);
        if !jobs.is_empty() {
            write_all(dir, &jobs);
        }
        jobs.len()
    }

    /// 保存 / 关闭后: 删掉这个文档的备份, 下次改动重新备份
    pub fn forget(&mut self, d: &Doc) {
        self.done.remove(&d.id);
        if let Some(dir) = dir() {
            let _ = std::fs::remove_file(dir.join(file_name(d.path.as_deref(), &d.name, d.id)));
        }
    }
}

fn write_all(dir: &Path, jobs: &[Job]) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    prune(dir, Duration::from_secs(KEEP_DAYS * 86_400));
    for j in jobs {
        let _ = write_atomic(&dir.join(&j.file), j.text.as_bytes());
    }
}

/// 同目录临时文件 + 换名 (见 `fsx`)
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    crate::fsx::atomic_write(path, bytes, false)
}

fn prune(dir: &Path, max_age: Duration) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > max_age);
        if old && e.path().extension().is_some_and(|x| x == "autosave") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---------- 文件锁 ----------

#[derive(Debug, PartialEq, Eq)]
pub enum LockOutcome {
    /// 别的程序正以可写方式占着它
    Busy,
    /// 这个平台没有「共享读、禁止写」的强制锁
    #[cfg_attr(windows, allow(dead_code))]
    Unsupported,
    Failed(String),
}

/// 打开文件时持有的锁 (文档 id -> 句柄); 句柄一放, 锁就没了
#[derive(Default)]
pub struct FileLocks {
    held: HashMap<u64, (PathBuf, std::fs::File)>,
    /// 试过但没锁上的 (id, 路径): 不每帧重试
    tried: HashMap<u64, PathBuf>,
}

#[cfg(windows)]
fn open_shared_read(p: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 1;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(p)
}

pub fn lock_file(p: &Path) -> Result<std::fs::File, LockOutcome> {
    #[cfg(windows)]
    {
        open_shared_read(p).map_err(|e| {
            if e.raw_os_error() == Some(32) || e.raw_os_error() == Some(33) {
                LockOutcome::Busy
            } else {
                LockOutcome::Failed(e.to_string())
            }
        })
    }
    #[cfg(not(windows))]
    {
        let _ = p;
        Err(LockOutcome::Unsupported)
    }
}

impl FileLocks {
    /// 每帧调用: 让持有的锁和「开着的文档」保持一致。返回新出现的提示 (哪个文件没锁上)。
    pub fn sync(&mut self, enabled: bool, docs: &[Doc]) -> Vec<(u64, LockOutcome)> {
        let mut news = vec![];
        if !enabled {
            self.held.clear();
            self.tried.clear();
            return news;
        }
        let live: Vec<(u64, &PathBuf)> = docs
            .iter()
            .filter(|d| !d.grep_result)
            .filter_map(|d| d.path.as_ref().map(|p| (d.id, p)))
            .collect();
        self.held
            .retain(|id, (p, _)| live.iter().any(|(i, q)| i == id && *q == p));
        self.tried
            .retain(|id, p| live.iter().any(|(i, q)| i == id && *q == p));
        for (id, p) in live {
            if self.held.contains_key(&id) || self.tried.get(&id) == Some(p) || !p.is_file() {
                continue;
            }
            match lock_file(p) {
                Ok(f) => {
                    self.held.insert(id, (p.clone(), f));
                }
                Err(e) => {
                    self.tried.insert(id, p.clone());
                    news.push((id, e));
                }
            }
        }
        news
    }

    /// 保存前放掉 (自己的写入不能被自己的锁挡住); 下一帧 `sync` 会重新持有
    pub fn release(&mut self, id: u64) {
        self.held.remove(&id);
        self.tried.remove(&id);
    }

    #[cfg(test)]
    pub fn holds(&self, id: u64) -> bool {
        self.held.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-fast-bk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn leftovers_list_autosave_files_newest_first_and_skip_junk() {
        let d = tmp("left");
        std::fs::write(d.join("a.txt.0123456789abcdef.autosave"), "hello").unwrap();
        std::fs::write(d.join("empty.0000000000000000.autosave"), "").unwrap();
        std::fs::write(d.join("note.md"), "x").unwrap();
        std::fs::create_dir(d.join("sub.autosave")).unwrap();
        let v = leftovers_in(&d);
        assert_eq!(v.len(), 1, "空文件 / 非 autosave / 目录都不算");
        assert_eq!((v[0].name.as_str(), v[0].size), ("a.txt", 5));
        assert!(leftovers_in(&d.join("nope")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn interval_is_clamped_and_remaining_counts_down() {
        assert_eq!(
            (clamp_interval(0), clamp_interval(5), clamp_interval(99999)),
            (1, 5, 1440)
        );
        let t0 = Instant::now();
        assert_eq!(remaining(t0, t0, 2), Duration::from_secs(120));
        assert_eq!(
            remaining(t0, t0 + Duration::from_secs(90), 2),
            Duration::from_secs(30)
        );
        assert!(remaining(t0, t0 + Duration::from_secs(500), 2).is_zero());
    }

    #[test]
    fn file_names_are_safe_and_distinguish_directories() {
        let a = file_name(Some(Path::new("C:/a/x.txt")), "x.txt", 1);
        let b = file_name(Some(Path::new("C:/b/x.txt")), "x.txt", 1);
        assert_ne!(a, b);
        assert!(a.ends_with(".autosave") && a.starts_with("x.txt."));
        let evil = file_name(None, "..\\..\\a:b*.txt", 7);
        assert!(!evil.contains(['\\', '/', ':', '*']), "{evil}");
        assert_ne!(file_name(None, "u", 1), file_name(None, "u", 2));
    }

    #[test]
    fn timer_is_idle_unless_enabled_and_something_is_dirty() {
        let mut ab = AutoBackup::default();
        let mut d = Doc::untitled(1, 1);
        let now = Instant::now();
        assert_eq!(
            ab.tick(false, 1, std::slice::from_ref(&d), now),
            None,
            "选项关着不排刷新"
        );
        assert_eq!(
            ab.tick(true, 1, std::slice::from_ref(&d), now),
            None,
            "没有脏文档不排刷新"
        );
        d.dirty = true;
        assert_eq!(
            ab.tick(true, 1, std::slice::from_ref(&d), now),
            Some(Duration::from_secs(60)),
            "脏了才开始计时"
        );
        assert_eq!(
            ab.tick(
                true,
                1,
                std::slice::from_ref(&d),
                now + Duration::from_secs(20)
            ),
            Some(Duration::from_secs(40))
        );
    }

    #[test]
    fn atomic_write_replaces_and_prune_only_removes_old_autosaves() {
        let dir = tmp("w");
        let f = dir.join("a.autosave");
        write_atomic(&f, b"one").unwrap();
        write_atomic(&f, b"two").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"two");
        let other = dir.join("keep.txt");
        std::fs::write(&other, "x").unwrap();
        prune(&dir, Duration::ZERO);
        assert!(!f.exists() && other.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn lock_allows_readers_but_blocks_writers_until_released() {
        let dir = tmp("l");
        let p = dir.join("a.txt");
        std::fs::write(&p, "x").unwrap();
        let h = lock_file(&p).unwrap();
        assert!(std::fs::read(&p).is_ok(), "共享读");
        assert!(
            std::fs::OpenOptions::new().write(true).open(&p).is_err(),
            "禁止写"
        );
        assert!(std::fs::remove_file(&p).is_err(), "禁止删除");
        assert_eq!(
            lock_file(&p).map(|_| ()),
            Ok(()),
            "同一进程再读锁一次没问题"
        );
        drop(h);
        assert!(std::fs::OpenOptions::new().write(true).open(&p).is_ok());
        // 别人正在写时锁不上
        let w = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
        assert_eq!(lock_file(&p).unwrap_err(), LockOutcome::Busy);
        drop(w);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn locks_follow_open_documents_and_release_on_save() {
        let dir = tmp("s");
        let p = dir.join("a.txt");
        std::fs::write(&p, "x").unwrap();
        let mut d = Doc::untitled(5, 1);
        d.path = Some(p.clone());
        let mut locks = FileLocks::default();
        assert!(
            locks.sync(false, std::slice::from_ref(&d)).is_empty() && !locks.holds(5),
            "选项关着不锁"
        );
        let news = locks.sync(true, std::slice::from_ref(&d));
        if cfg!(windows) {
            assert!(news.is_empty() && locks.holds(5));
            locks.release(5);
            assert!(
                std::fs::OpenOptions::new().write(true).open(&p).is_ok(),
                "放掉后能写"
            );
            locks.sync(true, std::slice::from_ref(&d));
            assert!(locks.holds(5), "下一帧重新持有");
            locks.sync(true, &[]);
            assert!(!locks.holds(5), "文档关了锁就放");
        } else {
            assert_eq!(news, vec![(5, LockOutcome::Unsupported)]);
            assert!(
                locks.sync(true, std::slice::from_ref(&d)).is_empty(),
                "不每帧重试"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn collect_jobs_takes_only_dirty_new_versions_and_flush_forces_a_rewrite() {
        let mut a = AutoBackup::default();
        let mut d1 = Doc::untitled(1, 1);
        d1.lines = vec!["x".into(), "y".into()];
        d1.dirty = true;
        let d2 = Doc::untitled(2, 2); // 没改过
        let mut d3 = Doc::untitled(3, 3); // Grep 结果页
        d3.dirty = true;
        d3.grep_result = true;
        let docs = vec![d1, d2, d3];
        let jobs = a.collect_jobs(&docs);
        assert_eq!(jobs.len(), 1, "只有改过的普通文档");
        assert_eq!(jobs[0].text, "x\r\ny", "按文档的换行符拼");
        assert!(a.collect_jobs(&docs).is_empty(), "同一版不重复备份");
        let dir = tmp("flush");
        assert_eq!(
            a.flush_to(&docs, &dir),
            1,
            "立即备份: 这一版虽然备份过, 也重写"
        );
        let written = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .count();
        assert_eq!(written, 1);
        assert_eq!(
            a.flush_to(&[Doc::untitled(9, 9)], &dir),
            0,
            "没有未保存修改的文档: 不写"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
