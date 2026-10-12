//! App — 顶层状态 + 事件循环 (keymap → action → state mutation)。

use std::process::Command;

use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};
use crossterm::event::{Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::ai::{AiBackend, PlaceholderAi};
use crate::input::InputBuffer;
use crate::keymap::{map_key, Action};
use crate::mode::Mode;
use ide_kernel::document::Document;

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
    /// 文件编辑文档 — `:open <file>` 后由 ide-kernel::Document 承载.
    ///
    /// 为何与 `buffer` 并存: 本 shell 原本是 REPL, `buffer` 是单行命令输入框;
    /// `Document` 是真正的多行可编辑文档. 两者语义不同, 不互相替代 ——
    /// 命令行继续用 InputBuffer (够用且无 undo 需求), 文件编辑用内核.
    document: Option<Document>,
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
        document: None,
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
        // 有文档打开时, 编辑类按键走内核 (统一 char 语义 + undo)
        if self.key_routes_to_document(&action) {
            return self.apply_to_document(action);
        }
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

    // ---------- 文件编辑 (ide-kernel 接入) ----------

    /// 打开文件到内核 Document. 成功后进入 Normal 模式.
    ///
    /// 与 `InputBuffer` 并存而非替代: 命令行输入仍走 InputBuffer,
    /// 文件内容走 `ide_kernel::Document` —— 后者才有 char 语义、undo、选区.
    pub fn open_file(&mut self, path: &str) -> Result<(), String> {
        // 4 MB 上限, 与 web/desktop 的 path_safety 一致
        const MAX_BYTES: u64 = 4 * 1024 * 1024;
        let meta = std::fs::metadata(path).map_err(|e| format!("stat 失败: {e}"))?;
        if meta.len() > MAX_BYTES {
            return Err(format!("文件过大: {} 字节 (上限 {} MB)", meta.len(), MAX_BYTES / 1024 / 1024));
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("读取失败 (仅支持 UTF-8): {e}"))?;
        let mut doc = Document::new();
        doc.set_text(&text);
        doc.set_path(path);
        // 光标落在文档末尾 (行首) ——
        // 与 sakura / vim 的 `G` 一致; 若落在行首, 打开文件直接输入会插到最前面.
        let last_row = (doc.line_count() - 1).max(0);
        let last_col = doc.line_text(last_row).chars().count() as i32;
        doc.set_caret(ide_kernel::document::Pos { row: last_row, col: last_col });
        self.document = Some(doc);
        self.mode = Mode::Normal;
        Ok(())
    }

    /// 保存当前文档. 无文档时返回 Err.
    pub fn save_file(&mut self) -> Result<(), String> {
        let doc = self.document.as_ref().ok_or("没有打开的文件")?;
        let path = doc.path().ok_or("文档没有路径")?.to_string();
        let text = doc.export_text();
        std::fs::write(&path, text).map_err(|e| format!("写入失败: {e}"))?;
        // 保存后清 dirty 标记 (保留 undo 历史)
        if let Some(d) = self.document.as_mut() {
            d.mark_saved();
        }
        Ok(())
    }

    pub fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    pub fn document_mut(&mut self) -> Option<&mut Document> {
        self.document.as_mut()
    }

    /// 有文档打开时, Insert/Normal 模式的字符编辑应走内核而非命令行.
    pub fn has_document(&self) -> bool {
        self.document.is_some()
    }

    /// 执行一条内部命令 (`:open` / `:w` / `:u` 等).
    ///
    /// 正常路径由 Command 模式的 Enter 触发; 这个 pub 入口让集成测试能
    /// 直接验证命令行为, 而不必模拟完整的按键序列.
    pub fn run_command(&mut self, raw: &str) {
        self.execute_command(raw);
    }

    /// 按键是否应路由到文档编辑 (而非命令行 buffer).
    ///
    /// Command 模式 (`:`) 与 Normal 模式的命令键仍归命令行 —— 这是 TUI REPL 的
    /// 既有交互, 不因打开文件而改变.
    fn key_routes_to_document(&self, action: &Action) -> bool {
        if self.document.is_none() {
            return false;
        }
        if self.mode == Mode::Command {
            return false;
        }
        matches!(
            action,
            Action::Insert(_)
                | Action::Backspace
                | Action::Delete
                | Action::MoveLeft
                | Action::MoveRight
                | Action::MoveHome
                | Action::MoveEnd
        )
    }

    /// 把 action 应用到内核 Document.
    fn apply_to_document(&mut self, action: Action) -> bool {
        use ide_kernel::document::Pos;
        let Some(doc) = self.document.as_mut() else { return true };
        match action {
            Action::Insert(c) => {
                doc.type_char(c);
            }
            Action::Backspace => {
                doc.backspace();
            }
            Action::Delete => {
                doc.delete_forward();
            }
            Action::MoveLeft => {
                let c = doc.caret();
                let new = if c.col > 0 {
                    Pos { row: c.row, col: c.col - 1 }
                } else if c.row > 0 {
                    let prev = (c.row - 1).max(0);
                    let len = doc.line_text(prev).chars().count() as i32;
                    Pos { row: prev, col: len }
                } else {
                    c
                };
                doc.set_caret(new);
            }
            Action::MoveRight => {
                let c = doc.caret();
                let len = doc.line_text(c.row).chars().count() as i32;
                let new = if c.col < len {
                    Pos { row: c.row, col: c.col + 1 }
                } else if c.row + 1 < doc.line_count() {
                    Pos { row: c.row + 1, col: 0 }
                } else {
                    c
                };
                doc.set_caret(new);
            }
            Action::MoveHome => doc.set_caret(Pos { row: doc.caret().row, col: 0 }),
            Action::MoveEnd => {
                let row = doc.caret().row;
                let len = doc.line_text(row).chars().count() as i32;
                doc.set_caret(Pos { row, col: len });
            }
            _ => {}
        }
        true
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
        // ---- 带参数的文件编辑命令 (必须在精确匹配之前处理) ----
        // `:open <path>` / `:e <path>` — 走 ide-kernel::Document
        // 关键: 命令名必须**完整匹配**, 不能用 `strip_prefix("e")` ——
        // 那会把 `echo hello` / `env` / `exit` 全部劫持成"打开文件",
        // 未知命令的子进程兜底就再也走不到. 这里先按空白切出命令名再比对.
        let (cmd_name, cmd_arg) = match cmd.split_once(char::is_whitespace) {
            Some((n, a)) => (n, a.trim()),
            None => (cmd, ""),
        };
        if matches!(cmd_name, "open" | "e") {
            let arg = cmd_arg;
            if arg.is_empty() {
                self.history.push("<usage: :open <path>>".into());
            } else {
                match self.open_file(arg) {
                    Ok(()) => {
                        let d = self.document.as_ref().expect("open 成功必有文档");
                        self.history.push(format!(
                            "<opened {} — {} 行, 行尾 {}>",
                            arg,
                            d.line_count(),
                            d.line_ending().label()
                        ));
                    }
                    Err(e) => self.history.push(format!("<open 失败: {e}>")),
                }
            }
            return;
        }
        // `:w` / `:write` — 保存 (无参)
        if matches!(cmd, "w" | "write") {
            match self.save_file() {
                Ok(()) => self.history.push("<saved>".into()),
                Err(e) => self.history.push(format!("<save 失败: {e}>")),
            }
            return;
        }
        // `:u` / `:undo` — 内核撤销
        if matches!(cmd, "u" | "undo") {
            let undone = self.document.as_mut().map(|d| d.undo()).unwrap_or(false);
            self.history.push(if undone { "<undo>".to_string() } else { "<nothing to undo>".to_string() });
            return;
        }

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
            // ---- 文件编辑 (ide-kernel 接入) ----
            _ if cmd == "open" || cmd == "e" => {
                // `:open <path>` — 注意此命令带参数, 在上面按整体匹配不到
                let arg = cmd.strip_prefix("open").or_else(|| cmd.strip_prefix('e')).unwrap_or("").trim();
                if arg.is_empty() {
                    self.history.push("<usage: :open <path>>".into());
                } else {
                    match self.open_file(arg) {
                        Ok(()) => {
                            let d = self.document.as_ref().unwrap();
                            self.history.push(format!(
                                "<opened {} — {} 行, 行尾 {}>",
                                arg,
                                d.line_count(),
                                d.line_ending().label()
                            ));
                        }
                        Err(e) => self.history.push(format!("<open 失败: {e}>")),
                    }
                }
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
