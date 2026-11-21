//! 诊断与性能日志管理器 (零开销、按需/崩溃触发式)
//!
//! # 核心约束
//! - 平时运行零 I/O 损耗，保持极轻量与桌面极速启动。
//! - 仅在以下两种情况下输出日志：
//!   1. 软件发生 panic / 启动失败 (run_native 报错) 时自动输出；
//!   2. 用户手动触发 (命令行 `--log` / `--dump-log` 或界面菜单点击“导出调试日志”) 时输出。

use std::path::PathBuf;
use std::sync::Mutex;

static IN_MEMORY_TRACE: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// 记录启动或运行关键步骤追踪（仅保存在内存中，发生故障或手动导出时才落盘）
pub fn record_trace_step(msg: &str) {
    if let Ok(mut lock) = IN_MEMORY_TRACE.lock() {
        if lock.len() < 256 {
            lock.push(msg.to_string());
        }
    }
}

/// 获取当前内存中缓存的追踪步骤
pub fn get_recorded_traces() -> Vec<String> {
    IN_MEMORY_TRACE
        .lock()
        .map(|l| l.clone())
        .unwrap_or_default()
}

/// 格式化时间戳 YYYYMMDD_HHMMSS
fn current_timestamp() -> String {
    let now = chrono::Local::now();
    now.format("%Y%m%d_%H%M%S").to_string()
}

/// 获取或解析日志输出目录
/// 优先选择 `<exe_parent_dir>/logs` (如 `C:\Program Files\IDE1.0\logs`)；
/// 若安装路径受系统权限限制无法写入，自动优雅降级到 `%LOCALAPPDATA%\IDE1.0\logs`
pub fn get_log_dir() -> PathBuf {
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

/// 注册全局 panic 钩子 (仅在崩溃/启动失败时触发写入)
pub fn install_crash_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let timestamp = current_timestamp();
        let log_dir = get_log_dir();
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

        default_hook(info);
    }));
}

/// 当 run_native 失败或启动阶段严重错误时记录
pub fn log_startup_error(err: &dyn std::fmt::Display) {
    let timestamp = current_timestamp();
    let log_dir = get_log_dir();
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

    let _ = std::fs::write(&log_file, content);
}

/// 手动触发导出诊断与性能日志 (通过菜单点击或命令行调用)
pub fn dump_diagnostic_log(reason: &str) -> Result<PathBuf, String> {
    let timestamp = current_timestamp();
    let log_dir = get_log_dir();
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
    let dir = get_log_dir();
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
    fn test_get_log_dir_is_usable() {
        let dir = get_log_dir();
        assert!(!dir.as_os_str().is_empty());
        assert!(dir.is_dir());
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
}
