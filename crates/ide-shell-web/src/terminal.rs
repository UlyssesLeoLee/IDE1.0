//! ide-shell-web — Terminal panel backend (HTTP polling model).
//!
//! ## Design
//! - Per-session child process with pipe stdin/stdout/stderr
//! - Reader thread accumulates chunks in `Mutex<VecDeque<String>>`
//! - Frontend polls `/api/terminal_output/{id}` (every 100ms) and gets queued chunks
//! - `/api/terminal_input` posts data → stdin
//!
//! ## Trade-off vs desktop
//! - Desktop uses Tauri events (push, low latency); web uses HTTP polling (simpler, no WS upgrade)
//! - For 1-2 terminals polling every 100ms = 10 req/s, well under any limit

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub title: String,
    pub shell: String,
    pub cwd: String,
    pub alive: bool,
}

struct Session {
    id: String,
    shell: String,
    cwd: String,
    stdin: ChildStdin,
    child: Child,
    /// 累积 chunks — drain 时返回 + 清空
    buffer: Mutex<VecDeque<String>>,
    alive: bool,
}

static SESSIONS: once_cell::sync::Lazy<Mutex<HashMap<String, Session>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn create(shell: Option<String>, cwd: Option<String>) -> Option<String> {
    let shell_cmd = resolve_shell(shell.as_deref());
    let cwd_path = cwd.unwrap_or_else(|| {
        std::env::var("IDE_SHELL_WEB_TEST_ROOT").unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    });

    let id_n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id = format!("term-{}", id_n);

    let mut cmd = Command::new(&shell_cmd);
    cmd.current_dir(&cwd_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    // 管道式 shell 的输入输出默认是系统 OEM 代码页 (中文系统 = GBK): 中文报错会变成 U+FFFD, 输入的中文也会乱。
    // 统一切到 UTF-8 (PowerShell 用 [Console]::*Encoding, cmd 用 chcp 65001), 下面按 UTF-8 解码。
    if shell_cmd.contains("powershell") {
        cmd.args(["-NoLogo", "-NoExit", "-Command", "[Console]::InputEncoding = [Console]::OutputEncoding = $OutputEncoding = New-Object System.Text.UTF8Encoding $false"]);
    } else if shell_cmd.to_ascii_lowercase().contains("cmd") {
        cmd.args(["/K", "chcp 65001>nul"]);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[terminal] spawn failed: {}", e);
            return None;
        }
    };
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut sessions = SESSIONS.lock().unwrap();
    sessions.insert(
        id.clone(),
        Session {
            id: id.clone(),
            shell: shell_cmd,
            cwd: cwd_path,
            stdin,
            child,
            buffer: Mutex::new(VecDeque::new()),
            alive: true,
        },
    );

    spawn_reader(id.clone(), Box::new(stdout));
    spawn_reader(id.clone(), Box::new(stderr));
    Some(id)
}

pub fn input(id: &str, data: &str) -> Result<(), String> {
    let mut sessions = SESSIONS.lock().unwrap();
    let session = sessions
        .get_mut(id)
        .ok_or_else(|| format!("session {} not found", id))?;
    session
        .stdin
        .write_all(data.as_bytes())
        .map_err(|e| format!("write: {}", e))?;
    session.stdin.flush().map_err(|e| format!("flush: {}", e))?;
    Ok(())
}

pub fn close(id: &str) {
    let mut sessions = SESSIONS.lock().unwrap();
    if let Some(mut s) = sessions.remove(id) {
        let _ = s.child.kill();
    }
}

pub fn list() -> Vec<TerminalInfo> {
    let sessions = SESSIONS.lock().unwrap();
    sessions
        .values()
        .map(|s| TerminalInfo {
            id: s.id.clone(),
            title: format!("{} ({})", shell_display(&s.shell), s.id),
            shell: s.shell.clone(),
            cwd: s.cwd.clone(),
            alive: s.alive,
        })
        .collect()
}

/// 会话是否还活着 (不存在 / 已退出 = false)
pub fn is_alive(id: &str) -> bool {
    let sessions = SESSIONS.lock().unwrap();
    sessions.get(id).map(|s| s.alive).unwrap_or(false)
}

pub fn drain_output(id: &str) -> Vec<String> {
    let sessions = SESSIONS.lock().unwrap();
    if let Some(s) = sessions.get(id) {
        let mut buf = s.buffer.lock().unwrap();
        buf.drain(..).collect()
    } else {
        Vec::new()
    }
}

pub fn shell_cmd(id: &str) -> String {
    let sessions = SESSIONS.lock().unwrap();
    sessions
        .get(id)
        .map(|s| s.shell.clone())
        .unwrap_or_default()
}

pub fn cwd(id: &str) -> String {
    let sessions = SESSIONS.lock().unwrap();
    sessions.get(id).map(|s| s.cwd.clone()).unwrap_or_default()
}

pub fn counter(id: &str) -> u64 {
    id.strip_prefix("term-")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

pub fn shell_display(shell: &str) -> &'static str {
    if shell.contains("powershell") {
        "PowerShell"
    } else if shell.contains("bash") {
        "bash"
    } else if shell.contains("zsh") {
        "zsh"
    } else if shell.contains("cmd") {
        "cmd"
    } else {
        "Terminal"
    }
}

fn default_shell() -> String {
    #[cfg(windows)]
    {
        "powershell.exe".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

/// 把请求里的 `shell` 参数解析成实际可执行文件.
///
/// # 安全 (P0)
///
/// 原实现 `Command::new(shell_cmd)` 直接用请求 JSON 里的字符串 —— 配合无认证的
/// `/api/terminal_create` 等于**任意可执行文件**都能被拉起。现在只接受白名单里的
/// 逻辑名 (或白名单的绝对路径), 其余一律回落到默认 shell.
pub fn resolve_shell(requested: Option<&str>) -> String {
    let default = default_shell();
    let Some(raw) = requested.map(str::trim).filter(|s| !s.is_empty()) else {
        return default;
    };

    // 带目录前缀一律拒绝 —— 否则 `C:\\...\\cmd.exe` 会借白名单名启动任意路径下的程序,
    // 攻击者只需放一个同名 exe 到自己目录。
    // 只接受**裸逻辑名** (powershell / cmd / bash / ...); 默认 shell 的绝对路径
    // 是唯一例外, 且必须是精确等于 default_shell 的值。
    let default = default_shell();
    if raw.contains('\\') || raw.contains('/') {
        if raw != default {
            eprintln!("[terminal] 拒绝带路径的 shell: {raw:?} — 回落到 {default}");
            return default;
        }
        return default;
    }

    let name = raw.to_ascii_lowercase();
    let matched = match name.as_str() {
        // Windows
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe" => Some("powershell.exe"),
        "cmd" | "cmd.exe" => Some("cmd.exe"),
        // POSIX
        "bash" | "bash.exe" | "/bin/bash" => Some("bash"),
        "sh" | "/bin/sh" => Some("sh"),
        "zsh" | "/bin/zsh" => Some("zsh"),
        "fish" => Some("fish"),

        _ => None,
    };

    match matched {
        Some(exe) => exe.to_string(),
        None => {
            eprintln!("[terminal] 拒绝未知 shell: {raw:?} — 回落到 {default}");
            default
        }
    }
}

fn spawn_reader(id: String, mut reader: Box<dyn Read + Send>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut chunker = editcore::text_stream::Utf8Chunker::new(); // 字符被切在两块之间时不能变成两个 U+FFFD
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = chunker.push(&buf[..n]);
                    if data.is_empty() {
                        continue;
                    }
                    let sessions = SESSIONS.lock().unwrap();
                    if let Some(s) = sessions.get(&id) {
                        s.buffer.lock().unwrap().push_back(data);
                    }
                }
                Err(_) => break,
            }
        }
        let mut sessions = SESSIONS.lock().unwrap();
        if let Some(s) = sessions.get_mut(&id) {
            s.alive = false;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0 安全: `shell` 参数不得变成任意可执行文件。
    #[test]
    fn whitelisted_shell_accepted() {
        for name in ["powershell", "PowerShell.EXE", "pwsh", "cmd", "cmd.exe"] {
            let got = resolve_shell(Some(name));
            assert!(got.ends_with(".exe"), "{name} 应解析为 exe, 得到 {got}");
        }
    }

    #[test]
    fn default_shell_absolute_path_accepted() {
        let default = default_shell();
        assert_eq!(resolve_shell(Some(&default)), default);
    }

    #[test]
    fn arbitrary_executable_path_rejected() {
        let default = default_shell();
        for evil in [
            "C:\\Windows\\System32\\cmd.exe", // 绝对路径绕过
            "/bin/sh",
            "calc.exe",
            "notepad.exe",
            "powershell.exe; calc", // 命令拼接
            "../../evil.exe",
        ] {
            let got = resolve_shell(Some(evil));
            // POSIX 下 /bin/sh 在白名单里, 其余必须回落
            if cfg!(windows) || !evil.starts_with('/') {
                assert_eq!(got, default, "{evil:?} 应被拒绝并回落");
            }
        }
    }

    #[test]
    fn blank_and_none_fall_back_to_default() {
        let default = default_shell();
        assert_eq!(resolve_shell(None), default);
        assert_eq!(resolve_shell(Some("")), default);
        assert_eq!(resolve_shell(Some("   ")), default);
    }

    /// 带目录前缀的白名单名**必须被拒** —— 否则攻击者把同名 exe 放到自己目录
    /// 就能借白名单名启动任意程序。
    #[test]
    fn whitelisted_name_with_dir_prefix_is_rejected() {
        let default = default_shell();
        for pathy in [
            "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            "/usr/local/bin/bash",
            "./powershell.exe",
        ] {
            assert_eq!(resolve_shell(Some(pathy)), default, "{pathy} 带路径应回落");
        }
    }
}
