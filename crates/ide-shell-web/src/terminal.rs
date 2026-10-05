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
    let shell_cmd = shell.unwrap_or_else(default_shell);
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
    if shell_cmd.contains("powershell") {
        cmd.args(["-NoLogo", "-NoExit"]);
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

fn spawn_reader(id: String, mut reader: Box<dyn Read + Send>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).into_owned();
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
