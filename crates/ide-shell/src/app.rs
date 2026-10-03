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
    /// 编辑 buffer(Insert / Command 模式共用)
    buffer: InputBuffer,
    /// 历史 (已执行的命令输出)
    history: Vec<String>,
    /// AI 桥
    ai: Box<dyn AiBackend>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            mode: Mode::Normal,
            buffer: InputBuffer::new(),
            history: Vec::new(),
            ai: Box::new(PlaceholderAi),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn buffer(&self) -> &InputBuffer {
        &self.buffer
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn kernel_banner(&self) -> &str {
        &self.config.kernel_banner
    }

    /// 处理键盘事件 — 返回 true 表示 app 应该继续,false 表示退出。
    pub fn on_key(&mut self, event: KeyEvent) -> bool {
        // Command mode 下 Enter 把 buffer 当命令执行
        if self.mode == Mode::Command && event.code == crossterm::event::KeyCode::Enter {
            let cmd = self.buffer.as_str().clone();
            self.buffer.clear();
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
            Action::Quit => return false,
            Action::EnterInsert => {
                self.mode = Mode::Insert;
            }
            Action::EnterNormal => {
                self.mode = Mode::Normal;
            }
            Action::EnterCommand => {
                self.mode = Mode::Command;
                self.buffer.clear();
            }
            Action::MoveLeft => self.buffer.move_left(),
            Action::MoveRight => self.buffer.move_right(),
            Action::MoveHome => self.buffer.set_cursor(0),
            Action::MoveEnd => self.buffer.set_cursor(self.buffer.len()),
            Action::Insert(c) => self.buffer.insert_char(c),
            Action::Backspace => {
                self.buffer.backspace();
            }
            Action::Delete => {
                self.buffer.delete();
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
        let cmd = raw.trim();
        if cmd.is_empty() {
            self.history.push("<empty>".into());
            return;
        }
        // 内部命令 (演示 AI native + IDE1.0 kernel 复用)
        match cmd {
            ":q" | "quit" => {
                self.history.push("<quit signal>".into());
                std::process::exit(0);
            }
            ":help" | "help" => {
                self.history.push(self.help_text());
                return;
            }
            ":version" | "version" => {
                self.history.push(format!(
                    "{} | IDE1.0 ide-shell demo",
                    self.config.kernel_banner
                ));
                return;
            }
            ":ai" | "ai" => {
                let sug = self.ai_suggestion();
                self.history.push(if sug.is_empty() {
                    "<no ai suggestion>".into()
                } else {
                    sug
                });
                return;
            }
            ":clear" | "clear" => {
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
        // 切到 Command
        assert!(app.on_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)));
        assert_eq!(app.mode(), Mode::Command);
        // 输入 "version"
        for c in "version".chars() {
            assert!(app.on_key(k(c)));
        }
        assert_eq!(app.buffer().as_str(), "version");
        // Enter 执行
        assert!(app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        // 执行后回 Normal 且 history 至少有一条
        assert_eq!(app.mode(), Mode::Normal);
        assert!(!app.history().is_empty());
    }

    #[test]
    fn test_ai_suggestion_placeholder() {
        let app = App::new(AppConfig::default());
        // 空输入 → 无建议
        assert_eq!(app.ai_suggestion(), "");
    }
}
