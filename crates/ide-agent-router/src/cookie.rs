//! 文件即任务 (File-as-a-Task) 会话 Cookie 引擎
//!
//! 把「正在编辑的文件」当作一个 Agent 任务, 并给它一份类似网页 Cookie 的轻量会话记录:
//! - **影子存储**: `<workspace>/.ide/cookies/<fnv64(rel_path)>.json`, 绝不污染源文件与 Git diff;
//! - **硬上限**: 单个 Cookie 序列化后 ≤ 4 KiB (字段数、字段长度都有上限), 防止无界增长;
//! - **惰性**: 只有用户对该文件发起过 Agent 任务才会落盘; 没有任务的文件零文件、零内存;
//! - **原子写**: 先写 `.tmp` 再 `rename`, 崩溃不会留下半截 JSON;
//! - **可清理**: 提供统计 / 全量清除 / 孤儿+过期+总量预算回收, 对应「设置 > 清除文件 Cookie」。

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 单个 Cookie 序列化后的字节硬上限 (和 HTTP Cookie 同量级)
pub const MAX_COOKIE_BYTES: usize = 4096;
/// Cookie 格式版本 (向后兼容: 读到更高版本的文件按「无 Cookie」处理, 不覆盖)
pub const COOKIE_VERSION: u32 = 1;

const MAX_INTENT_CHARS: usize = 512;
const MAX_SUMMARY_CHARS: usize = 600;
const MAX_LIST_ITEMS: usize = 8;
const MAX_ITEM_CHARS: usize = 160;

/// 文件任务状态机: Idle → Planning → Exploring → Vetting → Verified → Committed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    #[default]
    Idle,
    Planning,
    Exploring,
    Vetting,
    Verified,
    Committed,
}

impl TaskState {
    /// 状态短名 (Tab 指示条 / 提示文字用)
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Planning => "Planning",
            Self::Exploring => "Exploring",
            Self::Vetting => "Vetting",
            Self::Verified => "Verified",
            Self::Committed => "Committed",
        }
    }

    /// 是否是「有进行中任务」的状态 (Idle / Committed 不算)
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Idle | Self::Committed)
    }
}

/// 文件会话 Cookie
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCookie {
    pub version: u32,
    /// 相对工作区根的路径 (统一 `/` 分隔); 同时用来防哈希碰撞
    pub rel_path: String,
    #[serde(default)]
    pub state: TaskState,
    /// 当前任务意图 (最近一次用户提问)
    #[serde(default)]
    pub intent: String,
    /// 中间假设
    #[serde(default)]
    pub hypotheses: Vec<String>,
    /// 未解之谜
    #[serde(default)]
    pub unknowns: Vec<String>,
    /// 该文件的局部不变量 (「护照」), 注入系统提示词
    #[serde(default)]
    pub invariants: Vec<String>,
    /// 上一轮结论摘要
    #[serde(default)]
    pub last_summary: String,
    /// 上次停留的行 (0-based)
    #[serde(default)]
    pub cursor_line: usize,
    /// 累计 Agent 轮次
    #[serde(default)]
    pub rounds: u32,
    /// 最近更新 (Unix 秒)
    #[serde(default)]
    pub updated_at: i64,
}

impl FileCookie {
    pub fn new(rel_path: &str) -> Self {
        Self {
            version: COOKIE_VERSION,
            rel_path: normalize_rel(rel_path),
            state: TaskState::Idle,
            intent: String::new(),
            hypotheses: Vec::new(),
            unknowns: Vec::new(),
            invariants: Vec::new(),
            last_summary: String::new(),
            cursor_line: 0,
            rounds: 0,
            updated_at: 0,
        }
    }

    /// 没有任何值得保存的内容 (保存时直接删文件, 不留空壳)
    pub fn is_blank(&self) -> bool {
        self.state == TaskState::Idle
            && self.intent.is_empty()
            && self.hypotheses.is_empty()
            && self.unknowns.is_empty()
            && self.invariants.is_empty()
            && self.last_summary.is_empty()
    }

    /// 把各字段压进上限; 仍超 4 KiB 就从最不重要的字段开始丢
    pub fn enforce_limits(&mut self) {
        self.intent = clip_chars(&self.intent, MAX_INTENT_CHARS);
        self.last_summary = clip_chars(&self.last_summary, MAX_SUMMARY_CHARS);
        for list in [
            &mut self.hypotheses,
            &mut self.unknowns,
            &mut self.invariants,
        ] {
            list.retain(|s| !s.trim().is_empty());
            list.truncate(MAX_LIST_ITEMS);
            for s in list.iter_mut() {
                *s = clip_chars(s, MAX_ITEM_CHARS);
            }
        }
        // 兜底: 多字节字符把字节数撑大时, 按「假设 → 未解 → 摘要 → 意图」顺序丢弃
        while self.encoded_len() > MAX_COOKIE_BYTES {
            if self.hypotheses.pop().is_some() || self.unknowns.pop().is_some() {
                continue;
            }
            if !self.last_summary.is_empty() {
                let half = self.last_summary.chars().count() / 2;
                self.last_summary = clip_chars(&self.last_summary, half);
                continue;
            }
            if !self.intent.is_empty() {
                let half = self.intent.chars().count() / 2;
                self.intent = clip_chars(&self.intent, half);
                continue;
            }
            if self.invariants.pop().is_none() {
                break;
            }
        }
    }

    fn encoded_len(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |v| v.len())
    }

    /// 「断点续航」提示块: 注入系统提示词, 让 Agent 接着上次的进度干
    pub fn format_resume_prompt(&self) -> Option<String> {
        if self.is_blank() {
            return None;
        }
        let mut s = format!(
            "<!-- FILE TASK COOKIE START -->\n本文件 `{}` 有一份会话记录 (状态: {}, 已进行 {} 轮):\n",
            self.rel_path,
            self.state.label(),
            self.rounds
        );
        if !self.intent.is_empty() {
            s.push_str(&format!("- 上次任务意图: {}\n", self.intent));
        }
        if !self.last_summary.is_empty() {
            s.push_str(&format!("- 上轮结论: {}\n", self.last_summary));
        }
        if !self.hypotheses.is_empty() {
            s.push_str(&format!("- 进行中的假设: {}\n", self.hypotheses.join("; ")));
        }
        if !self.unknowns.is_empty() {
            s.push_str(&format!("- 尚未解决: {}\n", self.unknowns.join("; ")));
        }
        if !self.invariants.is_empty() {
            s.push_str(&format!(
                "- 本文件必须遵守的不变量: {}\n",
                self.invariants.join("; ")
            ));
        }
        s.push_str("<!-- FILE TASK COOKIE END -->");
        Some(s)
    }
}

/// Cookie 目录统计
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CookieStats {
    pub count: usize,
    pub bytes: u64,
}

/// 回收策略
#[derive(Debug, Clone, Copy)]
pub struct PrunePolicy {
    /// 超过这么多秒没更新的 Cookie 回收
    pub max_age_secs: i64,
    /// 目录总字节预算; 超出时从最旧的开始回收
    pub max_total_bytes: u64,
}

impl Default for PrunePolicy {
    fn default() -> Self {
        Self {
            max_age_secs: 30 * 24 * 3600,
            max_total_bytes: 8 * 1024 * 1024,
        }
    }
}

/// 某个工作区的 Cookie 罐 (无内存常驻状态, 每次直接读写磁盘上的小文件)
#[derive(Debug, Clone)]
pub struct CookieJar {
    workspace: PathBuf,
}

impl CookieJar {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace: workspace_root.to_path_buf(),
        }
    }

    pub fn dir(&self) -> PathBuf {
        self.workspace.join(".ide").join("cookies")
    }

    fn file_for(&self, rel_path: &str) -> PathBuf {
        self.dir().join(format!("{}.json", cookie_key(rel_path)))
    }

    /// 绝对路径 → 相对工作区路径; 不在工作区内返回 None (不为工作区外的文件建 Cookie)
    pub fn rel_of(&self, abs: &Path) -> Option<String> {
        let rel = abs.strip_prefix(&self.workspace).ok()?;
        let s = rel.to_string_lossy().replace('\\', "/");
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    /// 读取; 不存在 / 损坏 / 版本更高 / 哈希碰撞 都当作没有 Cookie
    pub fn load(&self, rel_path: &str) -> Option<FileCookie> {
        let rel = normalize_rel(rel_path);
        let text = std::fs::read_to_string(self.file_for(&rel)).ok()?;
        let c: FileCookie = serde_json::from_str(&text).ok()?;
        (c.version <= COOKIE_VERSION && c.rel_path == rel).then_some(c)
    }

    /// 原子保存 (先限长); 空白 Cookie 等价于删除
    pub fn save(&self, cookie: &mut FileCookie) -> Result<(), String> {
        cookie.enforce_limits();
        cookie.updated_at = now_secs();
        let path = self.file_for(&cookie.rel_path);
        if cookie.is_blank() {
            return self.remove(&cookie.rel_path);
        }
        std::fs::create_dir_all(self.dir()).map_err(|e| format!("创建 Cookie 目录失败: {e}"))?;
        // 会话记录属于本机临时状态, 默认不进 Git (目录内自带忽略规则; 已存在就不动)
        let ignore = self.dir().join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, "*\n").map_err(|e| format!("写入 Cookie .gitignore 失败: {e}"))?;
        }
        let tmp = path.with_extension("json.tmp");
        let body =
            serde_json::to_vec_pretty(cookie).map_err(|e| format!("序列化 Cookie 失败: {e}"))?;
        std::fs::write(&tmp, body).map_err(|e| format!("写入 Cookie 失败: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("提交 Cookie 失败: {e}")
        })
    }

    /// 删除单个文件的 Cookie (不存在不算错)
    pub fn remove(&self, rel_path: &str) -> Result<(), String> {
        match std::fs::remove_file(self.file_for(&normalize_rel(rel_path))) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("删除 Cookie 失败: {e}")),
        }
    }

    /// 统计 Cookie 目录 (只数 `.json`)
    pub fn stats(&self) -> CookieStats {
        let mut st = CookieStats::default();
        for (_, meta) in self.entries() {
            st.count += 1;
            st.bytes += meta.len();
        }
        st
    }

    /// 清除全部 Cookie (含残留 `.tmp`), 返回清掉的数量与字节; 只动 cookies 目录内的文件
    pub fn clear_all(&self) -> Result<CookieStats, String> {
        let dir = self.dir();
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CookieStats::default())
            }
            Err(e) => return Err(format!("读取 Cookie 目录失败: {e}")),
        };
        let mut removed = CookieStats::default();
        let mut first_err = None;
        for ent in rd.flatten() {
            let p = ent.path();
            let ext_ok = p
                .extension()
                .is_some_and(|x| x == "json" || x == "tmp");
            if !ext_ok || !ent.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let len = ent.metadata().map_or(0, |m| m.len());
            match std::fs::remove_file(&p) {
                Ok(()) => {
                    removed.count += 1;
                    removed.bytes += len;
                }
                Err(e) => first_err = first_err.or(Some(format!("删除 {} 失败: {e}", p.display()))),
            }
        }
        // 目录空了就顺手删掉, 不留空壳 (失败无所谓)
        let _ = std::fs::remove_dir(&dir);
        match first_err {
            Some(e) => Err(e),
            None => Ok(removed),
        }
    }

    /// 回收: 孤儿 (源文件已不存在) + 过期 + 超出总量预算的最旧者
    pub fn prune(&self, policy: PrunePolicy) -> CookieStats {
        let now = now_secs();
        let mut removed = CookieStats::default();
        // (更新时间, 路径, 字节)
        let mut alive: Vec<(i64, PathBuf, u64)> = Vec::new();
        for (path, meta) in self.entries() {
            let len = meta.len();
            let cookie = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str::<FileCookie>(&t).ok());
            let stale = match &cookie {
                None => true, // 损坏
                Some(c) => {
                    !self.workspace.join(&c.rel_path).exists()
                        || now.saturating_sub(c.updated_at) > policy.max_age_secs
                }
            };
            if stale {
                if std::fs::remove_file(&path).is_ok() {
                    removed.count += 1;
                    removed.bytes += len;
                }
            } else {
                let t = cookie.map_or(0, |c| c.updated_at);
                alive.push((t, path, len));
            }
        }
        let mut total: u64 = alive.iter().map(|a| a.2).sum();
        if total > policy.max_total_bytes {
            alive.sort_by_key(|a| a.0); // 最旧在前
            for (_, path, len) in alive {
                if total <= policy.max_total_bytes {
                    break;
                }
                if std::fs::remove_file(&path).is_ok() {
                    total -= len;
                    removed.count += 1;
                    removed.bytes += len;
                }
            }
        }
        removed
    }

    fn entries(&self) -> Vec<(PathBuf, std::fs::Metadata)> {
        let Ok(rd) = std::fs::read_dir(self.dir()) else {
            return Vec::new();
        };
        rd.flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "json") {
                    let m = e.metadata().ok()?;
                    m.is_file().then_some((p, m))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// 路径规范化: 反斜杠转正斜杠, 去掉首部 `./`
fn normalize_rel(p: &str) -> String {
    let s = p.replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

/// FNV-1a 64 位哈希 (确定性, 无依赖); Windows 路径不分大小写, 故转小写
fn cookie_key(rel_path: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in normalize_rel(rel_path).to_lowercase().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn clip_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_ws(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ide_cookie_{tag}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn roundtrip_and_blank_is_deleted() {
        let ws = tmp_ws("rt");
        std::fs::write(ws.join("a.rs"), "x").unwrap();
        let jar = CookieJar::new(&ws);
        let mut c = FileCookie::new("a.rs");
        c.state = TaskState::Exploring;
        c.intent = "修空指针".into();
        jar.save(&mut c).unwrap();
        let got = jar.load("a.rs").expect("cookie");
        assert_eq!(got.intent, "修空指针");
        assert_eq!(got.state, TaskState::Exploring);
        assert_eq!(jar.stats().count, 1);

        // 回到空白 => 文件被删, 不留空壳
        let mut blank = FileCookie::new("a.rs");
        jar.save(&mut blank).unwrap();
        assert!(jar.load("a.rs").is_none());
        assert_eq!(jar.stats().count, 0);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn size_is_hard_capped_even_with_wide_chars() {
        let ws = tmp_ws("cap");
        let jar = CookieJar::new(&ws);
        let mut c = FileCookie::new("big.rs");
        c.state = TaskState::Planning;
        c.intent = "字".repeat(5000);
        c.last_summary = "文".repeat(5000);
        c.hypotheses = (0..50).map(|_| "假".repeat(500)).collect();
        c.unknowns = (0..50).map(|_| "疑".repeat(500)).collect();
        c.invariants = (0..50).map(|_| "律".repeat(500)).collect();
        jar.save(&mut c).unwrap();
        let on_disk = std::fs::metadata(jar.file_for("big.rs")).unwrap().len() as usize;
        // 落盘用 pretty 格式 (多出缩进), 但紧凑编码长度必须在硬上限内
        assert!(c.encoded_len() <= MAX_COOKIE_BYTES, "{}", c.encoded_len());
        assert!(on_disk < MAX_COOKIE_BYTES * 2);
        assert!(c.hypotheses.len() <= MAX_LIST_ITEMS);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn path_style_and_collision_guard() {
        let ws = tmp_ws("key");
        let jar = CookieJar::new(&ws);
        assert_eq!(cookie_key("src\\a.rs"), cookie_key("./src/a.rs"));
        assert_ne!(cookie_key("src/a.rs"), cookie_key("src/b.rs"));

        let mut c = FileCookie::new("src/a.rs");
        c.intent = "x".into();
        jar.save(&mut c).unwrap();
        // 伪造哈希碰撞: 把 a 的文件内容挪到 b 的文件名下 => 读 b 必须得 None
        let a_file = jar.file_for("src/a.rs");
        let b_file = jar.file_for("src/b.rs");
        std::fs::copy(&a_file, &b_file).unwrap();
        assert!(jar.load("src/b.rs").is_none());
        assert!(jar.load("src/a.rs").is_some());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn corrupt_and_future_version_are_ignored_not_fatal() {
        let ws = tmp_ws("bad");
        let jar = CookieJar::new(&ws);
        std::fs::create_dir_all(jar.dir()).unwrap();
        std::fs::write(jar.file_for("a.rs"), "{not json").unwrap();
        assert!(jar.load("a.rs").is_none());

        let mut c = FileCookie::new("b.rs");
        c.intent = "x".into();
        c.version = COOKIE_VERSION + 1;
        std::fs::write(jar.file_for("b.rs"), serde_json::to_vec(&c).unwrap()).unwrap();
        assert!(jar.load("b.rs").is_none());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn clear_all_removes_only_cookie_files_and_reports() {
        let ws = tmp_ws("clr");
        let jar = CookieJar::new(&ws);
        for n in ["a.rs", "b.rs", "c.rs"] {
            std::fs::write(ws.join(n), "x").unwrap();
            let mut c = FileCookie::new(n);
            c.intent = "t".into();
            jar.save(&mut c).unwrap();
        }
        std::fs::write(jar.dir().join("stale.json.tmp"), "x").unwrap();
        // 目录里的无关文件 (如用户误放) 不能被动
        std::fs::write(jar.dir().join("keep.txt"), "keep").unwrap();
        // 工作区里的 memory.json 绝不能被清掉
        std::fs::write(ws.join(".ide").join("memory.json"), "{}").unwrap();

        assert_eq!(jar.stats().count, 3);
        let r = jar.clear_all().unwrap();
        assert_eq!(r.count, 4); // 3 个 json + 1 个 tmp
        assert!(r.bytes > 0);
        assert_eq!(jar.stats().count, 0);
        assert!(jar.dir().join("keep.txt").exists());
        assert!(ws.join(".ide").join("memory.json").exists());
        assert!(ws.join("a.rs").exists(), "源文件不受影响");
        // 空目录再清也不报错
        assert_eq!(CookieJar::new(&tmp_ws("none")).clear_all().unwrap().count, 0);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn prune_drops_orphans_expired_and_over_budget_oldest() {
        let ws = tmp_ws("prune");
        let jar = CookieJar::new(&ws);
        for n in ["keep.rs", "old.rs", "n1.rs", "n2.rs"] {
            std::fs::write(ws.join(n), "x").unwrap();
        }
        let put = |rel: &str, age: i64| {
            let mut c = FileCookie::new(rel);
            c.intent = "t".into();
            jar.save(&mut c).unwrap();
            // 回写时间戳 (save 会盖成 now, 这里直接改文件)
            let mut c2 = jar.load(rel).unwrap();
            c2.updated_at = now_secs() - age;
            std::fs::write(jar.file_for(rel), serde_json::to_vec(&c2).unwrap()).unwrap();
        };
        put("keep.rs", 10);
        put("old.rs", 40 * 24 * 3600); // 过期
        put("gone.rs", 10); // 孤儿: 源文件不存在
        let r = jar.prune(PrunePolicy::default());
        assert_eq!(r.count, 2);
        assert!(jar.load("keep.rs").is_some());
        assert!(jar.load("old.rs").is_none());

        // 预算: 3 个存活, 预算只容 1 个 => 回收最旧的 2 个
        put("n1.rs", 300);
        put("n2.rs", 200);
        let one = std::fs::metadata(jar.file_for("keep.rs")).unwrap().len() + 2;
        let r = jar.prune(PrunePolicy {
            max_age_secs: i64::MAX / 2,
            max_total_bytes: one,
        });
        assert_eq!(r.count, 2);
        assert!(jar.load("keep.rs").is_some(), "最新的留下");
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn resume_prompt_only_when_there_is_something_to_resume() {
        let mut c = FileCookie::new("a.rs");
        assert!(c.format_resume_prompt().is_none());
        c.state = TaskState::Vetting;
        c.intent = "修复溢出".into();
        c.invariants = vec!["no_heap_alloc".into()];
        let p = c.format_resume_prompt().unwrap();
        assert!(p.contains("修复溢出") && p.contains("no_heap_alloc") && p.contains("Vetting"));
        assert!(TaskState::Exploring.is_active() && !TaskState::Committed.is_active());
    }

    #[test]
    fn cookie_dir_is_git_ignored_by_default() {
        let ws = tmp_ws("git");
        let jar = CookieJar::new(&ws);
        let mut c = FileCookie::new("a.rs");
        c.intent = "x".into();
        jar.save(&mut c).unwrap();
        assert_eq!(
            std::fs::read_to_string(jar.dir().join(".gitignore")).unwrap(),
            "*\n"
        );
        // 清除全部后规则文件还在 (不被算作 Cookie, 也不被删)
        jar.clear_all().unwrap();
        assert!(jar.dir().join(".gitignore").exists());
        assert_eq!(jar.stats().count, 0);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn rel_of_rejects_paths_outside_workspace() {
        let ws = tmp_ws("rel");
        let jar = CookieJar::new(&ws);
        assert_eq!(jar.rel_of(&ws.join("src").join("a.rs")).as_deref(), Some("src/a.rs"));
        assert!(jar.rel_of(&std::env::temp_dir().join("elsewhere.rs")).is_none());
        assert!(jar.rel_of(&ws).is_none());
        let _ = std::fs::remove_dir_all(&ws);
    }
}
