//! 终端面板: 在项目根 (没有就当前目录) 下执行一行命令, 显示输出。
//! 复用 `editcore::exec_cmd::run` (系统 shell、超时、输出上限、自动判别输出编码), 在后台线程里跑, 界面不卡。
//! 这是「命令执行 + 输出」的面板, 不是完整的交互式终端 (没有 PTY: 不能跑 vim / 需要持续输入的程序)。

use editcore::exec_cmd::{self, ExecOut};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

pub struct Terminal {
    pub open: bool,
    pub input: String,
    pub output: String,
    pub history: Vec<String>,
    hist_pos: Option<usize>,
    rx: Option<Receiver<(String, Result<ExecOut, String>)>>,
}

impl Terminal {
    pub fn new() -> Self {
        Terminal { open: false, input: String::new(), output: String::new(), history: vec![], hist_pos: None, rx: None }
    }

    pub fn running(&self) -> bool {
        self.rx.is_some()
    }

    /// 执行输入框里的命令 (`cls` / `clear` 清屏)
    pub fn submit(&mut self, cwd: PathBuf) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() || self.running() {
            return;
        }
        self.input.clear();
        self.hist_pos = None;
        if matches!(cmd.as_str(), "cls" | "clear") {
            self.output.clear();
            return;
        }
        self.history.push(cmd.clone());
        self.output.push_str(&format!("> {cmd}\n"));
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = exec_cmd::run(&cwd, &cmd, None, Some(60_000));
            let _ = tx.send((cmd, r));
        });
        self.rx = Some(rx);
    }

    /// 每帧调用: 取后台输出; 有新输出返回 true
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok((_, Ok(o))) => {
                self.output.push_str(&o.stdout);
                if !o.stderr.is_empty() {
                    self.output.push_str(&o.stderr);
                }
                if o.timed_out {
                    self.output.push_str("[超时, 已结束进程]\n");
                } else if o.code.unwrap_or(0) != 0 {
                    self.output.push_str(&format!("[退出码 {}]\n", o.code.unwrap_or(-1)));
                }
                if !self.output.ends_with('\n') {
                    self.output.push('\n');
                }
                self.rx = None;
                true
            }
            Ok((_, Err(e))) => {
                self.output.push_str(&format!("[执行失败: {e}]\n"));
                self.rx = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(_) => {
                self.rx = None;
                false
            }
        }
    }

    /// ↑ / ↓ 翻历史
    pub fn history_step(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }
        let n = self.history.len();
        let pos = match (self.hist_pos, up) {
            (None, true) => n - 1,
            (Some(p), true) => p.saturating_sub(1),
            (Some(p), false) if p + 1 < n => p + 1,
            _ => {
                self.hist_pos = None;
                self.input.clear();
                return;
            }
        };
        self.hist_pos = Some(pos);
        self.input = self.history[pos].clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_command_in_the_background_and_collects_output() {
        let mut t = Terminal::new();
        t.input = "echo hello-fast".into();
        t.submit(std::env::temp_dir());
        assert!(t.running());
        for _ in 0..300 {
            if t.poll() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(t.output.contains("hello-fast"), "{}", t.output);
        assert!(!t.running());
        t.input = "cls".into();
        t.submit(std::env::temp_dir());
        assert!(t.output.is_empty());
        assert_eq!(t.history, vec!["echo hello-fast"]);
    }
}
