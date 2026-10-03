//! App — 顶层状态 + 事件循环 (keymap → action → state mutation)。

use std::process::Command;

use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};
use crossterm::event::{Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::ai::{AiBackend, PlaceholderAi};
use crate::input::InputBuffer;
use crate::keymap::{map_key, Action};
use crate::mode::Mode;

pub struct AppConfig {
    pub kernel_banner: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            kernel_banner: format!(
                "ide-kernel-core {}/{}",
                ide_kernel_core::version(),
                ide_kernel_core::kernel_status()
            ),
        }
    }
}

pub struct App {
    config: AppConfig,
    mode: Mode,
    /// 编辑 buffer(Insert / Normal 模式)— 用户正在构造的命令文本
    buffer: InputBuffer,
    /// 命令 buffer(Command 模式专用)— `:` 后输入的内部命令名,
    /// 与编辑 buffer 分离,保证 `:ai` 能对编辑 buffer 补全而不被清掉。
    cmd_buffer: InputBuffer,
    /// 历史 (已执行的命令输出)
    history: Vec<String>,
    /// AI 桥
    ai: Box<dyn AiBackend>,
    /// `:q` / `quit` 置位 — caller (TUI binary / web server) 决定如何响应.
    /// TUI: 退出进程; web server: 返回 keep_going=false 给 Playwright, 不杀 server.
    quit_requested: bool,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            mode: Mode::Normal,
            buffer: InputBuffer::new(),
            cmd_buffer: InputBuffer::new(),
            history: Vec::new(),
            ai: Box::new(PlaceholderAi),
            quit_requested: false,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn buffer(&self) -> &InputBuffer {
        &self.buffer
    }

    /// Command mode 专用 cmd buffer 内容。
    pub fn cmd_buffer(&self) -> &InputBuffer {
        &self.cmd_buffer
    }

    /// 当前 mode 的活动 buffer — Command mode 返回 cmd_buffer,其余返回编辑 buffer。
    pub fn active_buffer(&self) -> &InputBuffer {
        match self.mode {
            Mode::Command => &self.cmd_buffer,
            _ => &self.buffer,
        }
    }

    fn active_buffer_mut(&mut self) -> &mut InputBuffer {
        match self.mode {
            Mode::Command => &mut self.cmd_buffer,
            _ => &mut self.buffer,
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn kernel_banner(&self) -> &str {
        &self.config.kernel_banner
    }

    /// 用户是否请求退出 (`:q` / `quit` / Normal mode `q` 键)。
    pub fn should_quit(&self) -> bool {
        self.quit_requested
    }

    /// 重置 quit 标志 (web server 收到后可选 reset 或保持)。
    pub fn clear_quit(&mut self) {
        self.quit_requested = false;
    }

    /// 处理键盘事件 — 返回 true 表示 app 应该继续,false 表示退出。
    pub fn on_key(&mut self, event: KeyEvent) -> bool {
        // Command mode 下 Enter 把 cmd_buffer 当内部命令执行 (编辑 buffer 不动)
        if self.mode == Mode::Command && event.code == crossterm::event::KeyCode::Enter {
            let cmd = self.cmd_buffer.as_str().clone();
            self.cmd_buffer.clear();
            self.mode = Mode::Normal;
            self.execute_command(&cmd);
            return true;
        }

        let action = map_key(event, self.mode);
        self.apply(action)
    }

    /// 处理鼠标事件 — Left click → 切到 Insert 模式 (Stage 3.0 简化:鼠标只切 mode)。
    pub fn on_mouse(&mut self, event: MouseEvent) -> bool {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.mode = Mode::Insert;
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        true
    }

    /// 轮询一次(让 AI 槽位填充,目前是空操作)
    pub fn tick(&mut self) {
        // 后续 brief 接真实 AI 时,这里异步拉补全
    }

    /// 把当前 buffer 文本给 AI,返回补全(用于 UI 灰显 placeholder)
    pub fn ai_suggestion(&self) -> String {
        self.ai
            .complete(&self.buffer.as_str(), "")
            .unwrap_or_default()
    }

    fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => {
                self.quit_requested = true;
                return false;
            }
            Action::EnterInsert => {
                self.mode = Mode::Insert;
            }
            Action::EnterNormal => {
                self.mode = Mode::Normal;
            }
            Action::EnterCommand => {
                self.mode = Mode::Command;
                self.cmd_buffer.clear();
            }
            Action::MoveLeft => self.active_buffer_mut().move_left(),
            Action::MoveRight => self.active_buffer_mut().move_right(),
            Action::MoveHome => self.active_buffer_mut().set_cursor(0),
            Action::MoveEnd => {
                let len = self.active_buffer().len();
                self.active_buffer_mut().set_cursor(len);
            }
            Action::Insert(c) => self.active_buffer_mut().insert_char(c),
            Action::Backspace => {
                self.active_buffer_mut().backspace();
            }
            Action::Delete => {
                self.active_buffer_mut().delete();
            }
            Action::Execute => {
                // Insert 模式回车 = 执行 buffer 作为命令
                let cmd = self.buffer.as_str().clone();
                self.buffer.clear();
                self.mode = Mode::Normal;
                self.execute_command(&cmd);
            }
            Action::ExecuteCommand(s) => {
                self.execute_command(&s);
            }
            Action::Noop => {}
        }
        true
    }

    fn execute_command(&mut self, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            self.history.push("<empty>".into());
            return;
        }
        // 归一化: Command mode 进入时 ':' 键只切 mode 不入 buffer,
        // 所以用户输 ":q" 实际 buffer = "q"; Insert mode 直接输 "quit" 也合法。
        // 统一去掉可选 ':' 前缀后匹配裸命令名。
        let cmd = trimmed.strip_prefix(':').unwrap_or(trimmed);
        // 内部命令 (演示 AI native + IDE1.0 kernel 复用)
        match cmd {
            "q" | "quit" => {
                self.history.push("<quit signal>".into());
                self.quit_requested = true;
                return;
            }
            "help" => {
                self.history.push(self.help_text());
                return;
            }
            "version" => {
                self.history.push(format!(
                    "{} | IDE1.0 ide-shell demo",
                    self.config.kernel_banner
                ));
                return;
            }
            "ai" => {
                let sug = self.ai_suggestion();
                self.history.push(if sug.is_empty() {
                    "<no ai suggestion>".into()
                } else {
                    sug
                });
                return;
            }
            "clear" => {
                self.history.clear();
                return;
            }
            _ => {}
        }

        // 兜底: 真起子进程跑命令 (PowerShell → cmd 兜底 Windows)。
        let output = if cfg!(windows) {
            Command::new("cmd").args(["/C", cmd]).output()
        } else {
            Command::new("sh").args(["-c", cmd]).output()
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim_end().to_string();
                let empty = stdout.is_empty() && stderr.is_empty();
                if !stdout.is_empty() {
                    self.history.push(stdout);
                }
                if !stderr.is_empty() {
                    self.history.push(format!("[stderr] {}", stderr));
                }
                if empty {
                    self.history.push(format!("<exit {}>", out.status));
                }
            }
            Err(e) => {
                self.history.push(format!("<exec error: {}>", e));
            }
        }
    }

    fn help_text(&self) -> String {
        // 演示 AI assertion emit,验证 ide-shell 跟 ACI bridge 接通 (Stage 2 aci-emitter 复用)
        let em = AciEmitter::new(Layer::St);
        let aid = "ide1.0:shell:help-1";
        let assertion = em
            .build(
                aid,
                Scope::new("ide1.0", Some("shell".to_string()), None, None, None, None),
                ExpectActual::new(
                    ExpectValueType::WorkflowCompletes,
                    serde_json::json!(true),
                    "help command shows keymap",
                ),
                ExpectActual::new(
                    ExpectValueType::WorkflowCompletes,
                    serde_json::json!(true),
                    "help command rendered",
                ),
                Status::Pass,
                Severity::Info,
                "ide-shell demo",
            )
            .ok();
        let _ = assertion; // 真实集成留到后续 brief;这里仅确认 bridge 可调用

        "── IDE Shell demo ──  hjkl 移动 | i 进 Insert | Enter 执行 buffer | : 进 Command | :help :q :ai :version :clear | 鼠标左键 = Insert | 右键 = Normal | Esc 回 Normal".into()
    }
}

/// Event 聚合:键盘 + 鼠标统一入口
pub fn handle_event<B: ratatui::backend::Backend>(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(k) => app.on_key(k),
        Event::Mouse(m) => app.on_mouse(m),
        Event::Resize(_, _) => true,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn k(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn test_new_app_in_normal() {
        let app = App::new(AppConfig::default());
        assert_eq!(app.mode(), Mode::Normal);
        assert!(app.buffer().is_empty());
    }

    #[test]
    fn test_quit_returns_false() {
        let mut app = App::new(AppConfig::default());
        assert!(!app.on_key(k('q')));
        assert!(app.should_quit(), "q should set quit_requested flag");
    }

    #[test]
    fn test_quit_command_sets_flag_not_exit() {
        let mut app = App::new(AppConfig::default());
        // : → Command, 输 q, Enter → 执行 ":q" 走 quit 分支
        assert!(app.on_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)));
        assert!(app.on_key(k('q')));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            app.should_quit(),
            ":q should set quit_requested, not exit process"
        );
        app.clear_quit();
        assert!(!app.should_quit(), "clear_quit should reset flag");
    }

    #[test]
    fn test_insert_and_backspace() {
        let mut app = App::new(AppConfig::default());
        assert!(app.on_key(k('i')));
        assert_eq!(app.mode(), Mode::Insert);
        assert!(app.on_key(k('a')));
        assert!(app.on_key(k('b')));
        assert_eq!(app.buffer().as_str(), "ab");
        let back = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        assert!(app.on_key(back));
        assert_eq!(app.buffer().as_str(), "a");
    }

    #[test]
    fn test_mouse_left_click_enters_insert() {
        let mut app = App::new(AppConfig::default());
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        assert!(app.on_mouse(m));
        assert_eq!(app.mode(), Mode::Insert);
    }

    #[test]
    fn test_mouse_right_click_enters_normal() {
        let mut app = App::new(AppConfig::default());
        app.mode = Mode::Insert;
        let m = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        assert!(app.on_mouse(m));
        assert_eq!(app.mode(), Mode::Normal);
    }

    #[test]
    fn test_command_mode_enter_executes() {
        let mut app = App::new(AppConfig::default());
        // 先填一个编辑 buffer (Insert mode), 验证 Command 不动它
        app.on_key(k('i'));
        for c in "draft".chars() {
            app.on_key(k(c));
        }
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.buffer().as_str(), "draft");

        // 切到 Command
        assert!(app.on_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)));
        assert_eq!(app.mode(), Mode::Command);
        // 输入 "version" → 进 cmd_buffer, 编辑 buffer 不动
        for c in "version".chars() {
            assert!(app.on_key(k(c)));
        }
        assert_eq!(app.cmd_buffer().as_str(), "version");
        assert_eq!(
            app.buffer().as_str(),
            "draft",
            "编辑 buffer 不应被 Command mode 污染"
        );
        // Enter 执行
        assert!(app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        // 执行后回 Normal, cmd_buffer 清空, 编辑 buffer 保留, history 至少一条
        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.cmd_buffer().as_str(), "");
        assert_eq!(app.buffer().as_str(), "draft");
        assert!(!app.history().is_empty());
    }

    #[test]
    fn test_ai_suggestion_placeholder() {
        let app = App::new(AppConfig::default());
        // 空输入 → 无建议
        assert_eq!(app.ai_suggestion(), "");
    }
}
