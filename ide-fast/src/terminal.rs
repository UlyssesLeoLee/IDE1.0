//! 终端面板。两种用法: (1) 常驻 shell 会话 (默认): 一个 shell 进程一直活着, 输入行直接写进它的标准输入, `cd` / 环境变量会保留;
//! 输出经 `LineBuffer` 处理 ANSI 转义 / `\r` 回车 / `\b` 退格后再显示。(2) 一次性执行 (`editcore::exec_cmd::run`, 有超时和输出上限):
//! 选区作标准输入或输出替换选区时用。没有 PTY: 不能跑 vim 这类全屏程序, 需要持续输入的程序请在系统终端里跑。

use editcore::exec_cmd::{self, ExecOut};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// 会话输出最多留这么多字节, 超出从最前面丢 (按行)
const MAX_KEEP: usize = 1 << 20;
/// 「保留行数」设置的范围 (默认 5000); 字节上限 `MAX_KEEP` 仍然同时生效
pub const MIN_SCROLLBACK: usize = 100;
pub const MAX_SCROLLBACK: usize = 100_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Esc {
    None,
    /// 刚收到 ESC
    Got,
    /// CSI: ESC [ ... 终止字节 0x40-0x7E
    Csi,
    /// OSC: ESC ] ... BEL 或 ESC \
    Osc,
    /// OSC 里又收到 ESC
    OscEsc,
}

/// 终端文本缓冲: 已完成的行 + 正在写的当前行 (带光标列)。吃字节流里的控制字符。
pub struct LineBuffer {
    done: String,
    line: Vec<char>,
    col: usize,
    esc: Esc,
    csi: String,
    ver: u64,
    /// `done` 里的行数 / 最多保留多少行
    done_lines: usize,
    max_lines: usize,
}

impl LineBuffer {
    pub fn new() -> Self {
        LineBuffer {
            done: String::new(),
            line: vec![],
            col: 0,
            esc: Esc::None,
            csi: String::new(),
            ver: 0,
            done_lines: 0,
            max_lines: MAX_SCROLLBACK,
        }
    }

    pub fn clear(&mut self) {
        let (v, m) = (self.ver + 1, self.max_lines);
        *self = LineBuffer::new();
        self.ver = v;
        self.max_lines = m;
    }

    /// 最多保留的行数 (夹到合法范围); 变小了立刻裁掉多的
    pub fn set_scrollback(&mut self, lines: usize) {
        let n = lines.clamp(MIN_SCROLLBACK, MAX_SCROLLBACK);
        if n != self.max_lines {
            self.max_lines = n;
            self.trim_lines();
            self.ver += 1;
        }
    }

    /// 超过行数上限就从最前面丢整行
    fn trim_lines(&mut self) {
        if self.done_lines <= self.max_lines {
            return;
        }
        let drop = self.done_lines - self.max_lines;
        let at = self
            .done
            .match_indices('\n')
            .nth(drop - 1)
            .map(|(i, _)| i + 1)
            .unwrap_or(self.done.len());
        self.done.drain(..at);
        self.done_lines = self.max_lines;
    }

    /// 喂入一段输出文本 (转义序列可以被切在两次调用之间)
    pub fn version(&self) -> u64 {
        self.ver
    }

    pub fn feed(&mut self, text: &str) {
        self.ver += 1;
        for c in text.chars() {
            match self.esc {
                Esc::Got => {
                    self.esc = match c {
                        '[' => {
                            self.csi.clear();
                            Esc::Csi
                        }
                        ']' => Esc::Osc,
                        _ => Esc::None, // 其它两字节序列直接丢
                    };
                    continue;
                }
                Esc::Csi => {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        self.csi_done(c);
                        self.esc = Esc::None;
                    } else {
                        self.csi.push(c);
                    }
                    continue;
                }
                Esc::Osc => {
                    self.esc = match c {
                        '\u{7}' => Esc::None,
                        '\u{1b}' => Esc::OscEsc,
                        _ => Esc::Osc,
                    };
                    continue;
                }
                Esc::OscEsc => {
                    self.esc = if c == '\\' { Esc::None } else { Esc::Osc };
                    continue;
                }
                Esc::None => {}
            }
            match c {
                '\u{1b}' => self.esc = Esc::Got,
                '\n' => self.newline(),
                '\r' => self.col = 0,
                '\u{8}' => self.col = self.col.saturating_sub(1),
                '\t' => {
                    let to = (self.col / 8 + 1) * 8;
                    while self.col < to {
                        self.put(' ');
                    }
                }
                c if c.is_control() => {}
                c => self.put(c),
            }
        }
    }

    fn put(&mut self, c: char) {
        while self.line.len() < self.col {
            self.line.push(' ');
        }
        if self.col < self.line.len() {
            self.line[self.col] = c;
        } else {
            self.line.push(c);
        }
        self.col += 1;
    }

    fn newline(&mut self) {
        let l: String = self.line.drain(..).collect();
        self.done.push_str(l.trim_end());
        self.done.push('\n');
        self.done_lines += 1;
        self.col = 0;
        if self.done_lines > self.max_lines + self.max_lines / 4 {
            self.trim_lines(); // 多攒 1/4 再一起丢, 不用每行都搬一次内存
        }
        if self.done.len() > MAX_KEEP {
            let cut = self.done.len() - MAX_KEEP / 2;
            let cut = (cut..self.done.len())
                .find(|&i| self.done.is_char_boundary(i))
                .unwrap_or(self.done.len());
            let at = self.done[cut..]
                .find('\n')
                .map(|i| cut + i + 1)
                .unwrap_or(cut);
            self.done.drain(..at);
            self.done_lines = self.done.matches('\n').count();
        }
    }

    /// 处理 CSI 序列里有显示效果的几个: K 擦行, J 清屏, C/D 光标左右, G 到列; 其余 (颜色等) 忽略
    fn csi_done(&mut self, fin: char) {
        let n: usize = self
            .csi
            .trim_start_matches('?')
            .split(';')
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        match fin {
            'K' => match n {
                0 => self.line.truncate(self.col),
                2 => {
                    self.line.clear();
                    self.col = 0;
                }
                _ => {
                    for c in self.line.iter_mut().take(self.col) {
                        *c = ' ';
                    }
                }
            },
            'J' if n == 2 || n == 3 => {
                self.done.clear();
                self.done_lines = 0;
                self.line.clear();
                self.col = 0;
            }
            'C' => self.col = (self.col + n.max(1)).min(4096),
            'D' => self.col = self.col.saturating_sub(n.max(1)),
            'G' => self.col = (n.max(1) - 1).min(4096),
            _ => {}
        }
    }

    /// 全部可见文本 (已完成的行 + 当前行)
    pub fn text(&self) -> String {
        let mut s = self.done.clone();
        s.extend(self.line.iter());
        s
    }
}

/// 常驻 shell 进程: 两个读线程把 stdout / stderr 的字节送回来
struct Session {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Vec<u8>>,
    /// 上次没解码完的尾巴 (多字节字符被切在两块之间)
    tail: Vec<u8>,
}

fn pump(mut r: impl Read + Send + 'static, tx: std::sync::mpsc::Sender<Vec<u8>>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
}

impl Session {
    fn start(cwd: &std::path::Path) -> Result<Session, String> {
        #[cfg(windows)]
        let spawn_res = {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new("powershell.exe");
            c.args(["-NoLogo", "-ExecutionPolicy", "Bypass"]);
            c.current_dir(cwd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x0800_0000);
            c.spawn()
        };
        #[cfg(windows)]
        let mut child = match spawn_res {
            Ok(child) => child,
            Err(_) => {
                use std::os::windows::process::CommandExt;
                let mut c = Command::new("cmd.exe");
                c.current_dir(cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(0x0800_0000);
                c.spawn().map_err(|e| format!("无法启动 shell: {e}"))?
            }
        };
        #[cfg(not(windows))]
        let mut child = {
            let mut c = Command::new(std::env::var("SHELL").unwrap_or_else(|_| "sh".into()));
            c.current_dir(cwd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            c.spawn().map_err(|e| format!("无法启动 shell: {e}"))?
        };

        let (tx, rx) = channel();
        pump(child.stdout.take().ok_or("没有标准输出")?, tx.clone());
        pump(child.stderr.take().ok_or("没有标准错误")?, tx);
        let stdin = child.stdin.take().ok_or("没有标准输入")?;
        Ok(Session {
            child,
            stdin,
            rx,
            tail: vec![],
        })
    }

    fn send_line(&mut self, line: &str) -> bool {
        let nl: &[u8] = if cfg!(windows) { b"\r\n" } else { b"\n" };
        let clean = line.replace("\r\n", "\n").replace('\r', "\n");
        for sub in clean.split('\n') {
            if self.stdin.write_all(sub.as_bytes()).is_err() || self.stdin.write_all(nl).is_err() {
                return false;
            }
        }
        self.stdin.flush().is_ok()
    }

    /// 取已到的输出并解码 (UTF-8 被切断的尾巴留到下一块; 其余走 editcore 的自动判别)
    fn drain(&mut self) -> String {
        let mut bytes = std::mem::take(&mut self.tail);
        while let Ok(b) = self.rx.try_recv() {
            bytes.extend(b);
        }
        if bytes.is_empty() {
            return String::new();
        }
        if let Err(e) = std::str::from_utf8(&bytes) {
            if e.error_len().is_none() {
                self.tail = bytes.split_off(e.valid_up_to());
            }
        }
        editcore::document_io::decode_document(&bytes, None).content
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Terminal {
    pub open: bool,
    pub input: String,
    /// 请求下一帧把键盘焦点放进终端输入框 (点「+ 新建终端」等触发)
    pub focus_input: bool,
    /// 一次性命令的输出 (会话输出在 `buf` 里, 显示用 `display()`)
    pub output: String,
    pub history: Vec<String>,
    hist_pos: Option<usize>,
    replace_doc: Option<u64>,
    pub replace_out: Option<(u64, String)>,
    rx: Option<Receiver<(String, Result<ExecOut, String>)>>,
    /// 用常驻 shell 会话 (false = 每条命令一次性执行)
    pub persistent: bool,
    session: Option<Session>,
    buf: LineBuffer,
    /// 最近有输入 / 输出到这个时刻之前, 界面用快节奏刷新; 之后空闲慢刷新
    hot_until: Option<std::time::Instant>,
    shown: std::cell::RefCell<Option<((usize, u64), String)>>,
}

impl Terminal {
    pub fn new() -> Self {
        Terminal {
            open: false,
            input: String::new(),
            focus_input: false,
            output: String::new(),
            history: vec![],
            hist_pos: None,
            replace_doc: None,
            replace_out: None,
            rx: None,
            persistent: true,
            session: None,
            buf: LineBuffer::new(),
            hot_until: None,
            shown: Default::default(),
        }
    }

    /// 一次性命令在跑
    pub fn running(&self) -> bool {
        self.rx.is_some()
    }

    /// 常驻 shell 会话是否已建立
    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }

    /// 下一次界面刷新间隔 (毫秒); None = 不用刷新
    pub fn repaint_ms(&self) -> Option<u64> {
        if self.rx.is_some()
            || self
                .hot_until
                .is_some_and(|t| t > std::time::Instant::now())
        {
            Some(80)
        } else if self.session.is_some() {
            Some(500)
        } else {
            None
        }
    }

    fn heat(&mut self) {
        self.hot_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    }

    /// 需要持续刷新界面 (有会话或有命令在跑)
    #[allow(dead_code)]
    pub fn busy(&self) -> bool {
        self.rx.is_some() || self.session.is_some()
    }

    /// 执行输入框里的命令 (`cls` / `clear` 清屏)。没有选区管道时进常驻会话, 否则一次性执行
    pub fn submit(&mut self, cwd: PathBuf, stdin: Option<String>, replace_doc: Option<u64>) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() || self.running() {
            return;
        }
        self.input.clear();
        self.hist_pos = None;
        if matches!(cmd.as_str(), "cls" | "clear") {
            self.clear();
            return;
        }
        self.history.push(cmd.clone());
        if self.persistent && stdin.is_none() && replace_doc.is_none() {
            self.session_send(&cwd, &cmd);
            return;
        }
        self.output.push_str(&format!(
            "> {cmd}{}\n",
            if stdin.is_some() {
                "   (选区作标准输入)"
            } else {
                ""
            }
        ));
        self.replace_doc = replace_doc;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = exec_cmd::run(&cwd, &cmd, stdin.as_deref(), Some(60_000));
            let _ = tx.send((cmd, r));
        });
        self.rx = Some(rx);
    }

    /// 清屏 (一次性输出和会话输出都清)
    /// 外观设置里的保留行数
    pub fn set_scrollback(&mut self, lines: usize) {
        self.buf.set_scrollback(lines);
    }

    pub fn clear(&mut self) {
        self.output.clear();
        self.buf.clear();
    }

    /// 要显示的全部文本: 一次性命令的输出在前, 会话输出在后
    pub fn display(&self) -> String {
        let key = (self.output.len(), self.buf.version());
        let mut c = self.shown.borrow_mut();
        if c.as_ref().map(|c| c.0) != Some(key) {
            let mut s = self.output.clone();
            s.push_str(&self.buf.text());
            *c = Some((key, s));
        }
        c.as_ref().map(|c| c.1.clone()).unwrap_or_default()
    }

    /// 结束会话 (下一条命令会重新开一个)
    pub fn restart(&mut self) {
        if self.session.take().is_some() {
            self.buf.feed("\n[会话已重启]\n");
        }
    }

    /// 新建或重启常驻 shell 会话并聚焦输入框 (点「+ 新建终端」触发)
    pub fn start_session(&mut self, cwd: &std::path::Path) {
        let had = self.session.take().is_some();
        self.persistent = true;
        self.focus_input = true;
        match Session::start(cwd) {
            Ok(s) => {
                self.session = Some(s);
                if had {
                    self.buf
                        .feed(&format!("\n[已新建终端会话] PS {}>\n", cwd.display()));
                } else {
                    self.buf
                        .feed(&format!("[终端会话已就绪] PS {}>\n", cwd.display()));
                }
                self.heat();
            }
            Err(e) => {
                self.buf.feed(&format!("[{e}]\n"));
            }
        }
    }

    /// 向活动终端会话发送脚本代码并立即执行 (如 PowerShell 选区一键执行)
    pub fn execute_script(&mut self, cwd: &std::path::Path, code: &str) {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return;
        }
        self.session_send(cwd, trimmed);
    }

    fn session_send(&mut self, cwd: &std::path::Path, cmd: &str) {
        if !self.session.as_mut().map(|s| s.alive()).unwrap_or(false) {
            match Session::start(cwd) {
                Ok(s) => self.session = Some(s),
                Err(e) => {
                    self.buf.feed(&format!("[{e}]\n"));
                    return;
                }
            }
        }
        // 管道里 shell 不会回显输入, 本地补一份, 看起来像在终端里敲的
        self.buf.feed(&format!("{cmd}\n"));
        self.heat();
        if let Some(s) = self.session.as_mut() {
            if !s.send_line(cmd) {
                self.buf.feed("[shell 已退出]\n");
                self.session = None;
            }
        }
    }

    /// 每帧调用: 取会话输出和后台命令输出; 有新内容返回 true
    pub fn poll(&mut self) -> bool {
        let mut fresh = false;
        if let Some(s) = self.session.as_mut() {
            let t = s.drain();
            if !t.is_empty() {
                self.buf.feed(&t);
                fresh = true;
                self.hot_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            }
            if !s.alive() {
                let rest = s.drain();
                self.buf.feed(&rest);
                self.buf.feed("\n[shell 已退出]\n");
                self.session = None;
                fresh = true;
            }
        }
        self.poll_exec() || fresh
    }

    /// 取一次性命令的输出
    fn poll_exec(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok((_, Ok(o))) => {
                if let Some(id) = self.replace_doc.take() {
                    if o.code.unwrap_or(0) == 0 && !o.timed_out {
                        self.replace_out = Some((id, o.stdout.clone()));
                    }
                }
                self.output.push_str(&o.stdout);
                if !o.stderr.is_empty() {
                    self.output.push_str(&o.stderr);
                }
                if o.timed_out {
                    self.output.push_str("[超时, 已结束进程]\n");
                } else if o.code.unwrap_or(0) != 0 {
                    self.output
                        .push_str(&format!("[退出码 {}]\n", o.code.unwrap_or(-1)));
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

    fn wait(t: &mut Terminal, until: impl Fn(&Terminal) -> bool) {
        for _ in 0..400 {
            t.poll();
            if until(t) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn runs_a_command_in_the_background_and_collects_output() {
        let mut t = Terminal::new();
        t.persistent = false;
        t.input = "echo hello-fast".into();
        t.submit(std::env::temp_dir(), None, None);
        assert!(t.running());
        wait(&mut t, |t| !t.running());
        assert!(t.output.contains("hello-fast"), "{}", t.output);
        assert!(!t.running());
        t.input = "cls".into();
        t.submit(std::env::temp_dir(), None, None);
        assert!(t.output.is_empty());
        assert_eq!(t.history, vec!["echo hello-fast"]);
    }

    #[test]
    fn selection_goes_to_stdin_and_output_comes_back_for_replacement() {
        let mut t = Terminal::new();
        t.input = if cfg!(windows) {
            "findstr \"^\"".into()
        } else {
            "cat".into()
        };
        t.submit(std::env::temp_dir(), Some("piped-text".into()), Some(7));
        wait(&mut t, |t| !t.running());
        let (id, out) = t.replace_out.take().expect("成功的命令把输出交回来");
        assert_eq!(id, 7);
        assert!(out.contains("piped-text"), "{out}");
    }

    #[test]
    fn persistent_shell_keeps_state_between_commands() {
        let mut t = Terminal::new();
        let d = std::env::temp_dir();
        t.input = if cfg!(windows) {
            "set IDEFAST_X=kept-value".into()
        } else {
            "IDEFAST_X=kept-value".into()
        };
        t.submit(d.clone(), None, None);
        t.input = if cfg!(windows) {
            "echo %IDEFAST_X%".into()
        } else {
            "echo $IDEFAST_X".into()
        };
        t.submit(d, None, None);
        assert!(t.busy(), "会话在跑");
        wait(&mut t, |t| t.display().matches("kept-value").count() >= 2);
        // 第一次出现在本地回显的 set 命令里, 第二次才是 echo 打出来的值: 至少 2 次, 说明变量跨命令保留了
        assert!(
            t.display().matches("kept-value").count() >= 2,
            "{}",
            t.display()
        );
        t.restart();
        assert!(!t.busy());
        assert!(t.display().contains("会话已重启"));
    }

    #[test]
    fn line_buffer_handles_cr_backspace_and_ansi() {
        let mut b = LineBuffer::new();
        b.feed("abc\rXY\n");
        assert_eq!(b.text(), "XYc\n", "回车后覆盖写");
        b.feed("ab\u{8}Z\n");
        assert_eq!(b.text(), "XYc\naZ\n", "退格后覆盖");
        b.feed("\u{1b}[31mred\u{1b}[0m \u{1b}]0;title\u{7}ok\n");
        assert_eq!(b.text(), "XYc\naZ\nred ok\n", "颜色和标题序列被吃掉");
        b.feed("progress 10%\r\u{1b}[Kdone");
        assert!(b.text().ends_with("done"), "{}", b.text());
        assert!(!b.text().contains("progress"), "擦行后旧内容消失");
    }

    #[test]
    fn escape_sequences_may_be_split_across_chunks() {
        let mut b = LineBuffer::new();
        b.feed("a\u{1b}");
        b.feed("[3");
        b.feed("1mb\n");
        assert_eq!(b.text(), "ab\n");
        b.feed("x\u{1b}[2Jy");
        assert_eq!(b.text(), "y", "2J 清屏");
    }

    #[test]
    fn scrollback_setting_keeps_only_the_newest_lines() {
        let mut b = LineBuffer::new();
        b.set_scrollback(MIN_SCROLLBACK);
        for i in 0..1000 {
            b.feed(&format!("line {i}\n"));
        }
        let t = b.text();
        let n = t.lines().count();
        assert!(
            (MIN_SCROLLBACK..=MIN_SCROLLBACK + MIN_SCROLLBACK / 4 + 1).contains(&n),
            "{n}"
        );
        assert!(t.ends_with("line 999\n") && !t.contains("line 0\n"));
        b.set_scrollback(1000);
        b.set_scrollback(1); // 夹到下限 (不会变成 0 行),并且立刻裁到这个行数
        assert_eq!(b.text().lines().count(), MIN_SCROLLBACK);
        b.clear();
        for _ in 0..500 {
            b.feed("y\n");
        }
        assert!(
            b.text().lines().count() <= MIN_SCROLLBACK + MIN_SCROLLBACK / 4 + 1,
            "clear 之后设置还在"
        );
    }

    #[test]
    fn line_buffer_trims_old_output_when_huge() {
        let mut b = LineBuffer::new();
        let line = "x".repeat(999) + "\n";
        for _ in 0..1200 {
            b.feed(&line);
        }
        assert!(b.text().len() <= MAX_KEEP, "{}", b.text().len());
        assert!(b.text().ends_with(&line));
    }

    #[test]
    fn test_execute_script_sends_to_terminal() {
        let mut t = Terminal::new();
        t.execute_script(&PathBuf::from("."), "Write-Host 'hello from powershell'");
        assert!(t.busy());
        assert!(t.display().contains("Write-Host 'hello from powershell'"));
    }
}
