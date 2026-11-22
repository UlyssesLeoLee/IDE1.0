//! 诊断与性能日志管理器 (零开销、按需/崩溃触发式)
//!
//! # 核心约束
//! - 平时运行零 I/O 损耗，保持极轻量与桌面极速启动。
//! - 仅在以下两种情况下输出日志：
//!   1. 软件发生 panic / 启动失败 (run_native 报错) 时自动输出；
//!   2. 用户手动触发 (命令行 `--log` / `--dump-log` 或界面菜单点击“导出调试日志”) 时输出。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const STARTUP_CAP: usize = 32;
const RECENT_CAP: usize = 256;

struct TraceBuffer {
    startup: Vec<String>,
    recent: VecDeque<String>,
    dropped: usize,
}

impl TraceBuffer {
    const fn new() -> Self {
        Self {
            startup: Vec::new(),
            recent: VecDeque::new(),
            dropped: 0,
        }
    }

    fn push(&mut self, msg: &str) {
        if self.startup.len() < STARTUP_CAP {
            self.startup.push(msg.to_string());
        } else {
            if self.recent.len() == RECENT_CAP {
                self.recent.pop_front();
                self.dropped += 1;
            }
            self.recent.push_back(msg.to_string());
        }
    }

    fn snapshot(&self) -> Vec<String> {
        let extra = usize::from(self.dropped > 0);
        let mut out = Vec::with_capacity(self.startup.len() + self.recent.len() + extra);
        out.extend(self.startup.iter().cloned());
        if self.dropped > 0 {
            out.push(format!(
                "... ({} earlier trace events omitted) ...",
                self.dropped
            ));
        }
        out.extend(self.recent.iter().cloned());
        out
    }
}

static IN_MEMORY_TRACE: Mutex<TraceBuffer> = Mutex::new(TraceBuffer::new());
static CACHED_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 官方 GitHub Issues 反馈页面链接
pub const ISSUES_URL: &str = "https://github.com/UlyssesLeoLee/IDE1.0/issues";

/// 默认日志最大保留天数 (超过 7 天即视为过期日志)
pub const MAX_LOG_AGE_DAYS: u64 = 7;

/// 日志清理进度与完成消息通知
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogCleanupMsg {
    Progress {
        current: usize,
        total: usize,
        file_name: String,
    },
    Complete {
        deleted: usize,
        failed: usize,
    },
}

/// 判定是否应弹出崩溃/错误反馈对话框 (单元测试、无头测试与 CI 环境下静默以防卡死测试)
pub fn should_show_crash_dialog() -> bool {
    if cfg!(test) {
        return false;
    }
    if std::env::var_os("CI").is_some()
        || std::env::var_os("GITHUB_ACTIONS").is_some()
        || std::env::var_os("RUST_TEST_NOCAPTURE").is_some()
        || std::env::var_os("HEADLESS").is_some()
        || std::env::var_os("IDE10_HEADLESS").is_some()
    {
        return false;
    }
    true
}

/// 在崩溃或启动严重异常时提示用户并提供打开 GitHub Issues 反馈按钮
pub fn prompt_crash_feedback(log_path: &std::path::Path, error_type: &str) {
    if !should_show_crash_dialog() {
        return;
    }
    let res = rfd::MessageDialog::new()
        .set_title(format!("IDE1.0 - {error_type}"))
        .set_description(format!(
            "IDE1.0 遇到严重错误并已记录日志。\n\
            日志路径：{}\n\n\
            是否打开 GitHub Issues 页面反馈此问题？",
            log_path.display()
        ))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();

    if res == rfd::MessageDialogResult::Yes {
        let _ = crate::md_view::open_url(ISSUES_URL);
    }
}

/// 扫描指定目录下超过 max_age_days 天的日志文件 (带自定义基准时钟，便于纯函数单测)
pub fn scan_stale_logs_with_clock(
    dir: &std::path::Path,
    max_age_days: u64,
    now: std::time::SystemTime,
) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let cutoff_sec = max_age_days.saturating_mul(86400);
    let mut stale = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "log") {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if let Ok(age) = now.duration_since(mtime) {
                        if age.as_secs() >= cutoff_sec {
                            stale.push(path);
                        }
                    }
                }
            }
        }
    }
    stale
}

/// 扫描应用日志目录下超过 max_age_days 天的日志文件
pub fn scan_stale_logs(max_age_days: u64) -> Vec<PathBuf> {
    scan_stale_logs_with_clock(&get_log_dir(), max_age_days, std::time::SystemTime::now())
}

/// 启动异步日志清理任务并返回进度通道
pub fn start_stale_log_cleanup(
    stale_files: Vec<PathBuf>,
) -> std::sync::mpsc::Receiver<LogCleanupMsg> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let total = stale_files.len();
        let mut deleted = 0;
        let mut failed = 0;
        for (i, path) in stale_files.into_iter().enumerate() {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let _ = tx.send(LogCleanupMsg::Progress {
                current: i + 1,
                total,
                file_name,
            });
            if std::fs::remove_file(&path).is_ok() {
                deleted += 1;
            } else {
                failed += 1;
            }
            let sleep_ms = if total <= 5 {
                30
            } else if total <= 15 {
                10
            } else {
                0
            };
            if sleep_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            }
        }
        let _ = tx.send(LogCleanupMsg::Complete { deleted, failed });
    });
    rx
}

/// 记录启动或运行关键步骤追踪（仅保存在内存中，保留首段启动轨迹与最近 256 条运行事件）
pub fn record_trace_step(msg: &str) {
    let mut lock = IN_MEMORY_TRACE.lock().unwrap_or_else(|e| e.into_inner());
    lock.push(msg);
}

/// 获取当前内存中缓存的追踪步骤
pub fn get_recorded_traces() -> Vec<String> {
    let lock = IN_MEMORY_TRACE.lock().unwrap_or_else(|e| e.into_inner());
    lock.snapshot()
}

/// 格式化时间戳 YYYYMMDD_HHMMSS
fn current_timestamp() -> String {
    let now = chrono::Local::now();
    now.format("%Y%m%d_%H%M%S").to_string()
}

/// 解析并校验日志输出目录 (仅执行一次磁盘写探测)
/// 优先选择 `<exe_parent_dir>/logs` (如 `C:\Program Files\IDE1.0\logs`)；
/// 若安装路径受系统权限限制无法写入，自动优雅降级到 `%LOCALAPPDATA%\IDE1.0\logs`
fn resolve_log_dir() -> PathBuf {
    // 1. 尝试当前 exe 所在目录下的 logs 文件夹
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("logs");
            if candidate.is_dir() || std::fs::create_dir_all(&candidate).is_ok() {
                // 测试写权限
                let probe = candidate.join(format!(".probe_{}", std::process::id()));
                if std::fs::write(&probe, b"ok").is_ok() {
                    let _ = std::fs::remove_file(&probe);
                    return candidate;
                }
            }
        }
    }

    // 2. 降级为用户本地目录 (保证绝对有写入权限)
    let fallback = if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("IDE1.0").join("logs")
    } else if let Some(app_data) = std::env::var_os("APPDATA") {
        PathBuf::from(app_data).join("IDE1.0 Fast").join("logs")
    } else {
        std::env::temp_dir().join("IDE1.0").join("logs")
    };
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

/// 获取日志输出目录 (进程内通过 OnceLock 缓存，避免重复创建/删除探测文件)
pub fn get_log_dir() -> PathBuf {
    CACHED_LOG_DIR.get_or_init(resolve_log_dir).clone()
}

/// 确保已缓存的日志目录在写入或打开前存在（不重跑写权限探测）
fn ensure_log_dir() -> PathBuf {
    let dir = get_log_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// 注册全局 panic 钩子 (仅在崩溃/启动失败时触发写入)
pub fn install_crash_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let timestamp = current_timestamp();
        let log_dir = ensure_log_dir();
        let log_file = log_dir.join(format!("crash_{timestamp}.log"));

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "unknown panic payload"
        };

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let traces = get_recorded_traces();
        let mut report = format!(
            "=== IDE1.0 CRASH REPORT ===\n\
            Time: {timestamp}\n\
            Version: {ver}\n\
            Location: {location}\n\
            Panic: {payload}\n\
            OS: {os} ({arch})\n\
            Log Directory: {log_dir_str}\n\
            PID: {pid}\n\
            ===========================\n\n\
            --- Recent Lifecycle Trace ---\n",
            ver = env!("CARGO_PKG_VERSION"),
            os = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            log_dir_str = log_dir.to_string_lossy(),
            pid = std::process::id(),
        );

        for t in traces {
            report.push_str(&t);
            report.push('\n');
        }

        let _ = std::fs::write(&log_file, &report);
        eprintln!("{report}");

        prompt_crash_feedback(&log_file, "程序异常崩溃");

        default_hook(info);
    }));
}

/// 当 run_native 失败或启动阶段严重错误时记录
pub fn log_startup_error(err: &dyn std::fmt::Display) {
    let timestamp = current_timestamp();
    let log_dir = ensure_log_dir();
    let log_file = log_dir.join(format!("startup_error_{timestamp}.log"));
    let traces = get_recorded_traces();

    let mut content = format!(
        "=== IDE1.0 STARTUP ERROR ===\n\
        Time: {timestamp}\n\
        Version: {}\n\
        Error: {err}\n\
        OS: {} ({})\n\
        Log Directory: {}\n\
        ============================\n\n\
        --- Trace Events ---\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        log_dir.display(),
    );

    for t in traces {
        content.push_str(&t);
        content.push('\n');
    }

    let _ = std::fs::write(&log_file, &content);
    eprintln!("IDE1.0 startup error: {err}");

    prompt_crash_feedback(&log_file, "启动异常");
}

/// 手动触发导出诊断与性能日志 (通过菜单点击或命令行调用)
pub fn dump_diagnostic_log(reason: &str) -> Result<PathBuf, String> {
    let timestamp = current_timestamp();
    let log_dir = ensure_log_dir();
    let log_file = log_dir.join(format!("ide10_diag_{timestamp}.log"));

    let mut body = String::new();
    body.push_str("=== IDE1.0 DIAGNOSTIC & PERFORMANCE LOG ===\n");
    body.push_str(&format!("Generated: {timestamp}\n"));
    body.push_str(&format!("Reason: {reason}\n"));
    body.push_str(&format!("Version: {}\n", env!("CARGO_PKG_VERSION")));
    body.push_str(&format!("PID: {}\n", std::process::id()));
    body.push_str(&format!(
        "Executable: {}\n",
        std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    body.push_str(&format!("Log Directory: {}\n", log_dir.display()));
    body.push_str(&format!(
        "OS: {} ({})\n\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));

    body.push_str("--- LIFECYCLE & PERFORMANCE TRACE ---\n");
    let traces = get_recorded_traces();
    if traces.is_empty() {
        body.push_str("(No trace events recorded)\n");
    } else {
        for line in traces {
            body.push_str(&line);
            body.push('\n');
        }
    }

    std::fs::write(&log_file, body).map_err(|e| e.to_string())?;
    Ok(log_file)
}

/// 在系统文件资源管理器中打开日志目录
pub fn open_log_dir_in_explorer() {
    let dir = ensure_log_dir();
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_get_traces() {
        record_trace_step("test_step_1");
        record_trace_step("test_step_2");
        let traces = get_recorded_traces();
        assert!(traces.iter().any(|s| s == "test_step_1"));
        assert!(traces.iter().any(|s| s == "test_step_2"));
    }

    #[test]
    fn test_dump_diagnostic_log_creates_file() {
        record_trace_step("diag_log_test_marker");
        let path = dump_diagnostic_log("unit_test").expect("dump log failed");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("read dump log");
        assert!(content.contains("=== IDE1.0 DIAGNOSTIC & PERFORMANCE LOG ==="));
        assert!(content.contains("unit_test"));
        assert!(content.contains("diag_log_test_marker"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_log_startup_error_writes_file() {
        log_startup_error(&"simulated_startup_failure");
        let dir = get_log_dir();
        let entries = std::fs::read_dir(&dir).expect("read log dir");
        let mut found = false;
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with("startup_error_") && fname.ends_with(".log") {
                if let Ok(c) = std::fs::read_to_string(entry.path()) {
                    if c.contains("simulated_startup_failure") {
                        found = true;
                        let _ = std::fs::remove_file(entry.path());
                        break;
                    }
                }
            }
        }
        assert!(found, "startup_error log file should have been written");
    }

    #[test]
    fn test_trace_ring_buffer_preserves_startup_and_latest() {
        let mut buf = TraceBuffer::new();
        // 未超出 STARTUP_CAP + RECENT_CAP (288) 时应完整保留，不产生省略标记
        for i in 0..(STARTUP_CAP + RECENT_CAP) {
            buf.push(&format!("step_{i}"));
        }
        let snap_full = buf.snapshot();
        assert_eq!(snap_full.len(), STARTUP_CAP + RECENT_CAP);
        assert_eq!(snap_full[0], "step_0");
        assert_eq!(snap_full[STARTUP_CAP + RECENT_CAP - 1], "step_287");
        assert!(!snap_full.iter().any(|s| s.contains("omitted")));

        // 超出容量后再推入 32 条：保留前 32 条启动事件 + 省略标记 + 最新 256 条（step_64..=step_319）
        for i in (STARTUP_CAP + RECENT_CAP)..320 {
            buf.push(&format!("step_{i}"));
        }
        let snap = buf.snapshot();
        assert_eq!(snap[0], "step_0");
        assert_eq!(snap[STARTUP_CAP - 1], format!("step_{}", STARTUP_CAP - 1));
        assert!(snap[STARTUP_CAP].contains("32 earlier trace events omitted"));
        assert_eq!(snap[STARTUP_CAP + 1], "step_64");
        assert_eq!(snap.last().map(String::as_str), Some("step_319"));
        assert_eq!(snap.len(), STARTUP_CAP + 1 + RECENT_CAP);
    }

    #[test]
    fn test_get_log_dir_is_cached_and_usable() {
        let d1 = get_log_dir();
        let d2 = get_log_dir();
        assert!(!d1.as_os_str().is_empty());
        assert!(d1.is_dir());
        assert_eq!(d1, d2);
        assert_eq!(CACHED_LOG_DIR.get(), Some(&d1));
    }

    #[test]
    fn test_crash_feedback_contract() {
        assert_eq!(ISSUES_URL, "https://github.com/UlyssesLeoLee/IDE1.0/issues");
        assert!(!should_show_crash_dialog());
    }

    #[test]
    fn test_scan_and_cleanup_stale_logs() {
        let temp_dir = std::env::temp_dir().join(format!("ide10_test_logs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let log_old = temp_dir.join("test_old.log");
        let log_recent = temp_dir.join("test_recent.log");
        let non_log = temp_dir.join("test_other.txt");

        std::fs::write(&log_old, b"old log content").unwrap();
        std::fs::write(&log_recent, b"recent log content").unwrap();
        std::fs::write(&non_log, b"other content").unwrap();

        // 模拟 1: 基准时钟为文件创建时刻 -> 没有任何日志超过 7 天
        let now_fresh = std::time::SystemTime::now();
        let stale_zero = scan_stale_logs_with_clock(&temp_dir, 7, now_fresh);
        assert!(stale_zero.is_empty(), "新建日志不应判定为过期");

        // 模拟 2: 基准时钟快进 8 天 -> 两份 log 均判定为过期，txt 非 log 文件不包含
        let now_later = now_fresh + std::time::Duration::from_secs(8 * 86400);
        let stale_found = scan_stale_logs_with_clock(&temp_dir, 7, now_later);
        assert_eq!(stale_found.len(), 2, "应扫描出 2 份过期日志文件");
        assert!(stale_found.contains(&log_old));
        assert!(stale_found.contains(&log_recent));
        assert!(!stale_found.contains(&non_log));

        // 运行清理任务
        let rx = start_stale_log_cleanup(stale_found);
        let mut completed = false;
        let mut last_progress = 0;
        while let Ok(msg) = rx.recv_timeout(std::time::Duration::from_secs(3)) {
            match msg {
                LogCleanupMsg::Progress { current, total, .. } => {
                    assert_eq!(total, 2);
                    last_progress = current;
                }
                LogCleanupMsg::Complete { deleted, failed } => {
                    assert_eq!(deleted, 2);
                    assert_eq!(failed, 0);
                    completed = true;
                    break;
                }
            }
        }
        assert!(completed, "清理任务应正常发送完成通知");
        assert_eq!(last_progress, 2);
        assert!(!log_old.exists(), "过期日志 old 应当已被物理删除");
        assert!(!log_recent.exists(), "过期日志 recent 应当已被物理删除");
        assert!(non_log.exists(), "非 log 文件必须完整保留");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
