//! 外部命令执行并捕获输出 (WBS-02 4.7, Sakura「外部コマンド実行」): 命令行 + 可选的标准输入 (选区), 有超时和输出上限。
//!
//! - 只在用户明确执行时调用, 不常驻; 命令在项目根目录下用系统 shell 运行 (Windows: `cmd /C`, 其它: `sh -c`)。
//! - 输出按字节收集后用自动判别的编码解码 (Windows 控制台常是 GBK / SJIS), 不会因为不是 UTF-8 而乱码或报错。
//! - 超时就杀掉子进程并标出 `timed_out`; 输出超过上限只保留前面一段并标出 `truncated`。

use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const MAX_TIMEOUT_MS: u64 = 600_000;
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ExecOut {
    /// 进程退出码; 被杀掉 / 没有退出码 = None
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub truncated: bool,
}

fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        // raw_arg: 命令行原样交给 cmd (用 .args 会按 C 运行库规则给引号加反斜杠, cmd 不认, 带引号的命令就坏了)
        c.arg("/C").raw_arg(command);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    }
}

/// 收集一路输出到共享缓冲 (线程读, 主线程随时可以取走已读到的部分, 不必等线程结束)
struct Capture {
    buf: std::sync::Arc<std::sync::Mutex<(Vec<u8>, bool)>>,
    done: std::sync::mpsc::Receiver<()>,
}

fn read_capped(mut r: impl Read + Send + 'static) -> Capture {
    let buf = std::sync::Arc::new(std::sync::Mutex::new((Vec::new(), false)));
    let (tx, rx) = std::sync::mpsc::channel();
    let shared = buf.clone();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match r.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut g = shared.lock().unwrap();
                    if g.0.len() < MAX_OUTPUT_BYTES {
                        let take = n.min(MAX_OUTPUT_BYTES - g.0.len());
                        g.0.extend_from_slice(&chunk[..take]);
                        g.1 |= take < n;
                    } else {
                        g.1 = true; // 继续读掉, 免得子进程因管道写满而卡住
                    }
                }
            }
        }
        let _ = tx.send(());
    });
    Capture { buf, done: rx }
}

impl Capture {
    /// 正常结束: 等读完; 超时杀进程的情况: 最多再等一小会儿 (孙进程可能还攥着管道), 然后取走已读到的
    fn take(self, patient: bool) -> (Vec<u8>, bool) {
        let _ = if patient { self.done.recv().ok() } else { self.done.recv_timeout(Duration::from_millis(400)).ok() };
        let g = self.buf.lock().unwrap();
        (g.0.clone(), g.1)
    }
}

/// 杀掉整棵进程树: shell 的子进程 (ping / sleep 等) 也要结束, 否则它们还握着输出管道
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("pkill").args(["-KILL", "-P", &child.id().to_string()]).status();
    }
    let _ = child.kill();
}

fn decode(bytes: &[u8]) -> String {
    crate::document_io::decode_document(bytes, None).content
}

/// 运行命令。`stdin` = Some(文本) 时写进子进程标准输入后关闭。
pub fn run(cwd: &Path, command: &str, stdin: Option<&str>, timeout_ms: Option<u64>) -> Result<ExecOut, String> {
    if command.trim().is_empty() {
        return Err("命令是空的".into());
    }
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).clamp(100, MAX_TIMEOUT_MS));
    let mut child = shell(command)
        .current_dir(cwd)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动命令: {e}"))?;
    if let (Some(text), Some(mut si)) = (stdin, child.stdin.take()) {
        let data = text.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let out_h = read_capped(child.stdout.take().ok_or("没有标准输出")?);
    let err_h = read_capped(child.stderr.take().ok_or("没有标准错误")?);
    let start = Instant::now();
    let (code, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(st)) => break (st.code(), false),
            Ok(None) if start.elapsed() >= timeout => {
                kill_tree(&mut child);
                let _ = child.wait();
                break (None, true);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(format!("等待命令失败: {e}")),
        }
    };
    let (o, ot) = out_h.take(!timed_out);
    let (e, et) = err_h.take(!timed_out);
    Ok(ExecOut { code, stdout: decode(&o), stderr: decode(&e), timed_out, truncated: ot || et })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> std::path::PathBuf {
        std::env::temp_dir()
    }

    #[test]
    fn captures_stdout_and_exit_code() {
        let r = run(&tmp(), "echo hello", None, None).unwrap();
        assert_eq!(r.code, Some(0));
        assert!(r.stdout.contains("hello") && !r.timed_out && !r.truncated, "{r:?}");
        let r = run(&tmp(), if cfg!(windows) { "exit 3" } else { "exit 3" }, None, None).unwrap();
        assert_eq!(r.code, Some(3));
    }

    #[test]
    fn stdin_is_piped_when_given() {
        let cmd = if cfg!(windows) { "findstr /R \".\"" } else { "cat" };
        let r = run(&tmp(), cmd, Some("line one\nline two\n"), None).unwrap();
        assert!(r.stdout.contains("line one") && r.stdout.contains("line two"), "{r:?}");
    }

    #[test]
    fn timeout_kills_the_child() {
        let cmd = if cfg!(windows) { "ping -n 20 127.0.0.1 > nul" } else { "sleep 20" };
        let t = Instant::now();
        let r = run(&tmp(), cmd, None, Some(300)).unwrap();
        assert!(r.timed_out && r.code.is_none(), "{r:?}");
        assert!(t.elapsed() < Duration::from_secs(10), "超时后应很快返回");
    }

    #[test]
    fn empty_command_is_an_error_and_stderr_is_separate() {
        assert!(run(&tmp(), "   ", None, None).is_err());
        let cmd = if cfg!(windows) { "echo oops 1>&2" } else { "echo oops >&2" };
        let r = run(&tmp(), cmd, None, None).unwrap();
        assert!(r.stderr.contains("oops") && !r.stdout.contains("oops"), "{r:?}");
    }
}
