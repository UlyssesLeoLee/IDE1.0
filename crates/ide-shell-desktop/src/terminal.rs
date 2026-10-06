//! ide-shell-desktop — Terminal panel backend (per Cursor / VSCode / JetBrains commercial standard).
//!
//! ## Design
//! - **Real PTY** (simplified) — spawn PowerShell (Windows) / bash (Linux/macOS) via `Command::spawn()` + pipe stdin/stdout.
//! - **Multi-session** — each session has unique ID; tabs are independent.
//! - **Event-driven** — backend reads stdout/stderr in thread, emits `terminal://output` events to frontend.
//! - **Lifecycle** — tab close or process exit → emit `terminal://closed` → frontend cleans up.
//!
//! ## Trade-off
//! - No real ConPTY / winpty (avoids heavy external deps). Trade-off: interactive TUIs (vim-with-tui / htop) won't render correctly — but 99% use cases (cargo build, npm test, pytest, git, ls) work fine.
//! - Upgrade path: swap `std::process::Command` for `portable-pty` crate later.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// Terminal session info (sent to frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub title: String,
    pub shell: String,
    pub cwd: String,
    pub alive: bool,
}

/// Internal terminal session.
struct PtySession {
    id: String,
    title: String,
    shell: String,
    child: Child,
    stdin: ChildStdin,
}

/// Global Terminal state.
#[derive(Default)]
pub struct TerminalState {
    inner: Mutex<TerminalInner>,
}

#[derive(Default)]
struct TerminalInner {
    sessions: HashMap<String, PtySession>,
    counter: u64,
}

impl TerminalState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Create a Terminal session — spawn PowerShell (Windows) / bash (Linux/macOS).
#[tauri::command]
pub fn terminal_create(
    app: AppHandle,
    state: tauri::State<'_, TerminalState>,
    shell: Option<String>,
    cwd: Option<String>,
    title: Option<String>,
) -> Result<TerminalInfo, String> {
    let shell_cmd = shell.unwrap_or_else(default_shell);
    let cwd_path = cwd.unwrap_or_else(|| {
        std::env::var("IDE_SHELL_DESKTOP_TEST_ROOT").unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    });

    let (id, title) = {
        let inner = &mut *state.inner.lock().map_err(|e| e.to_string())?;
        inner.counter += 1;
        let id = format!("term-{}", inner.counter);
        let title =
            title.unwrap_or_else(|| format!("{} #{}", shell_display(&shell_cmd), inner.counter));
        (id, title)
    };

    diag_log(&format!(
        "[terminal_create] id={} shell={} cwd={}",
        id, shell_cmd, cwd_path
    ));

    let mut cmd = Command::new(&shell_cmd);
    cmd.current_dir(&cwd_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW — 不弹黑窗
        cmd.creation_flags(0x08000000);
    }

    if shell_cmd.contains("powershell") {
        cmd.args(["-NoLogo", "-NoExit"]);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("terminal_create: spawn '{}' failed: {}", shell_cmd, e))?;

    let stdin = child.stdin.take().ok_or("stdin missing")?;
    let stdout = child.stdout.take().ok_or("stdout missing")?;
    let stderr = child.stderr.take().ok_or("stderr missing")?;

    {
        let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
        inner.sessions.insert(
            id.clone(),
            PtySession {
                id: id.clone(),
                title: title.clone(),
                shell: shell_cmd.clone(),
                child,
                stdin,
            },
        );
    }

    spawn_reader(app.clone(), id.clone(), "stdout", Box::new(stdout));
    spawn_reader(app.clone(), id.clone(), "stderr", Box::new(stderr));

    Ok(TerminalInfo {
        id,
        title,
        shell: shell_cmd,
        cwd: cwd_path,
        alive: true,
    })
}

/// Send chars to terminal stdin.
#[tauri::command]
pub fn terminal_input(
    state: tauri::State<'_, TerminalState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    let session = inner
        .sessions
        .get_mut(&id)
        .ok_or_else(|| format!("session {} not found", id))?;
    session
        .stdin
        .write_all(data.as_bytes())
        .map_err(|e| format!("write stdin failed: {}", e))?;
    session
        .stdin
        .flush()
        .map_err(|e| format!("flush stdin failed: {}", e))?;
    Ok(())
}

/// Close terminal session.
#[tauri::command]
pub fn terminal_close(state: tauri::State<'_, TerminalState>, id: String) -> Result<(), String> {
    let mut inner = state.inner.lock().map_err(|e| e.to_string())?;
    if let Some(mut session) = inner.sessions.remove(&id) {
        let _ = session.child.kill();
        diag_log(&format!("[terminal_close] killed {}", id));
    }
    Ok(())
}

/// List active terminal sessions.
#[tauri::command]
pub fn terminal_list(state: tauri::State<'_, TerminalState>) -> Result<Vec<TerminalInfo>, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(inner
        .sessions
        .values()
        .map(|s| TerminalInfo {
            id: s.id.clone(),
            title: s.title.clone(),
            shell: s.shell.clone(),
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            alive: true,
        })
        .collect())
}

// ===== internal =====

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

fn shell_display(shell: &str) -> &'static str {
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

/// Spawn reader thread — converts ChildStderr to Box<dyn Read> for shared code path.
fn spawn_reader(
    app: AppHandle,
    id: String,
    stream: &'static str,
    mut reader: Box<dyn Read + Send>,
) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let _ = app.emit(
                        "terminal://output",
                        TerminalOutputEvent {
                            id: id.clone(),
                            stream,
                            data,
                        },
                    );
                }
                Err(e) => {
                    diag_log(&format!("[terminal_read] {} read error: {}", id, e));
                    break;
                }
            }
        }
        let _ = app.emit("terminal://closed", TerminalClosedEvent { id: id.clone() });
        diag_log(&format!("[terminal_read] {} {} EOF", id, stream));

        if let Some(state) = app.try_state::<TerminalState>() {
            if let Ok(mut inner) = state.inner.lock() {
                inner.sessions.remove(&id);
            }
        }
    });
}

#[derive(Serialize, Clone)]
struct TerminalOutputEvent {
    id: String,
    stream: &'static str,
    data: String,
}

#[derive(Serialize, Clone)]
struct TerminalClosedEvent {
    id: String,
}

fn diag_log(msg: &str) {
    use std::io::Write;
    let path = std::env::var("IDE_LOG_FILE").unwrap_or_else(|_| {
        let mut p = std::env::temp_dir();
        p.push("ide-shell-desktop-diag.log");
        p.to_string_lossy().into_owned()
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{}", msg);
    }
}
