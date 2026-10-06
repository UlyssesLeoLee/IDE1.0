//! ide-shell-protocol - headless vim-like state machine.
//!
//! 核心思想: AI agent (IDE 一方) 通过 vim-like RPC 操控 IDE buffer/cursor/mode,
//! 不需要关心 UI 渲染 / mouse / WebView. AI 集成路径: 注册 provider
//! (completion_fn / edit_fn / chat_fn) — 当 buffer/cursor 变化时 server
//! 调用 provider 返回结果给 client.
//!
//! 设计目标 (vim `clientserver` 风格):
// - 多 buffer, 每个 buffer 有 name + lines + cursor (row, col)
//- 三种 mode: normal / insert / command
//- 命令: :w :q :e <path> :buffer <id> 等 vim-like 形式
//- 键位: 字符键 + Ctrl 修饰符, 与 vim 一致
//- undo/redo: 每 buffer 独立, snap 整 buffer
//!
//! 不依赖 ratatui/crossterm/Tauri — 纯 headless, 可被任意 host 集成.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Vim-like mode — 与 ide-shell 一致.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

/// 单个文本 buffer — lines (按 \n 分割, 不含 trailing empty) + cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buffer {
    pub id: u32,
    pub name: String,
    pub path: Option<String>,
    pub lines: Vec<String>,
    pub cursor: Cursor,
    pub mode: Mode,
    pub dirty: bool,
    pub language: String,
    pub undo_stack: Vec<BufferSnapshot>,
    pub redo_stack: Vec<BufferSnapshot>,
    pub cmd_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSnapshot {
    pub lines: Vec<String>,
    pub cursor: Cursor,
}

impl Buffer {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            path: None,
            lines: vec![String::new()],
            cursor: Cursor { row: 0, col: 0 },
            mode: Mode::Normal,
            dirty: false,
            language: "plain".into(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            cmd_text: String::new(),
        }
    }

    /// 当前行内容.
    pub fn current_line(&self) -> &str {
        self.lines
            .get(self.cursor.row as usize)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// 当前光标前的字符 (用于 completion).
    pub fn prefix(&self) -> String {
        let line = self.current_line();
        let col = self.cursor.col as usize;
        line[..col.min(line.len())].to_string()
    }

    /// 整 buffer 文本 — 用 \n 拼接.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// 在光标位置插入字符 (INSERT 模式语义).
    pub fn insert_char(&mut self, c: char) {
        let line_idx = self.cursor.row as usize;
        if line_idx >= self.lines.len() {
            return;
        }
        let line = &mut self.lines[line_idx];
        let col = self.cursor.col as usize;
        if col > line.len() {
            line.push(c);
        } else {
            line.insert(col, c);
        }
        self.cursor.col += 1;
        self.dirty = true;
    }

    /// backspace 删字符.
    pub fn backspace(&mut self) {
        let line_idx = self.cursor.row as usize;
        if line_idx >= self.lines.len() {
            return;
        }
        let col = self.cursor.col as usize;
        if col == 0 {
            if line_idx == 0 {
                return;
            }
            let cur = self.lines.remove(line_idx);
            let prev_len = self.lines[line_idx - 1].len();
            self.lines[line_idx - 1].push_str(&cur);
            self.cursor.row -= 1;
            self.cursor.col = prev_len as u32;
            self.dirty = true;
            return;
        }
        let line = &mut self.lines[line_idx];
        line.remove(col - 1);
        self.cursor.col -= 1;
        self.dirty = true;
    }

    /// Enter 拆行 (INSERT 模式) + 自动缩进复制前一行 indent + 行尾 { [ ( 多缩一级.
    pub fn newline(&mut self) {
        let line_idx = self.cursor.row as usize;
        if line_idx >= self.lines.len() {
            return;
        }
        let col = self.cursor.col as usize;
        let line = self.lines[line_idx].clone();
        let left = line[..col.min(line.len())].to_string();
        let right = line[col.min(line.len())..].to_string();
        let indent = left
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect::<String>();
        let trimmed = left.trim_end();
        let extra = if trimmed.ends_with('{') || trimmed.ends_with('[') || trimmed.ends_with('(') {
            if indent.contains('\t') { "\t".to_string() } else { "  ".to_string() }
        } else {
            String::new()
        };
        self.lines[line_idx] = left;
        self.lines.insert(line_idx + 1, format!("{}{}", indent, extra) + &right);
        self.cursor.row = (line_idx + 1) as u32;
        self.cursor.col = (indent.len() + extra.len()) as u32;
        self.dirty = true;
    }

    /// 移动光标 — vim h/j/k/l.
    pub fn move_cursor(&mut self, d_row: i32, d_col: i32) {
        let max_row = self.lines.len().saturating_sub(1) as i32;
        let new_row = (self.cursor.row as i32 + d_row).clamp(0, max_row) as u32;
        let max_col = self.lines.get(new_row as usize).map(|l| l.len()).unwrap_or(0) as i32;
        let new_col = (self.cursor.col as i32 + d_col).clamp(0, max_col) as u32;
        self.cursor.row = new_row;
        self.cursor.col = new_col;
    }

    /// 切换模式.
    pub fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        if m == Mode::Insert {
            // push undo
            self.push_undo();
        }
    }

    pub fn push_undo(&mut self) {
        if self.undo_stack.len() >= 200 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(BufferSnapshot {
            lines: self.lines.clone(),
            cursor: self.cursor.clone(),
        });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(BufferSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor.clone(),
            });
            self.lines = snap.lines;
            self.cursor = snap.cursor;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(BufferSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor.clone(),
            });
            self.lines = snap.lines;
            self.cursor = snap.cursor;
            self.dirty = true;
            true
        } else {
            false
        }
    }
}

/// RPC 请求 — JSON-RPC 2.0 风格 (用于 stdio + HTTP/WS 共用).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,  // 总是 "2.0"
    pub id: Option<serde_json::Value>,  // null = notification (no reply)
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// RPC 响应.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcResponse {
    pub fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }
    pub fn err(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError { code, message: message.into(), data: None }),
        }
    }
}

/// RPC 错误码 (JSON-RPC 2.0 兼容).
pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    // 自定义: -32000 ~ -32099
    pub const BUFFER_NOT_FOUND: i32 = -32001;
    pub const READ_ONLY: i32 = -32002;
}

/// Provider trait — AI agent 实现三方法注入到 IDE kernel.
///
/// ## 三件套
/// - `complete(prefix, context)` — 文本补全 (LSP-like)
/// - `edit(range, instruction)` — inline edit (选中范围 + 指令 → 替换文本)
/// - `chat(messages, tools)` — 通用 agent chat (返回 plan/commands)
///
/// 所有方法 async, 由 server 在合适时机调用并把结果推给 client.
#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(&self, req: CompletionRequest) -> Result<Vec<CompletionItem>, String>;
    async fn edit(&self, req: EditRequest) -> Result<EditResponse, String>;
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, String>;
}

/// Provider request/response 数据结构.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub buffer_id: u32,
    pub prefix: String,
    pub line_text: String,
    pub cursor: Cursor,
    pub language: String,
    pub limit: u32,  // 默认 10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub insert_text: String,
    pub kind: String,  // function / variable / class / keyword / snippet
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditRequest {
    pub buffer_id: u32,
    pub range: Range,
    pub instruction: String,  // 自然语言指令, e.g. "add error handling"
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResponse {
    pub new_text: String,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub buffer_id: Option<u32>,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,  // system / user / assistant / tool
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub finish_reason: Option<String>,  // stop / tool_calls / length
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Kernel — 多 buffer 容器 + provider registry + 当前活跃 buffer.
///
/// AI client 通过 RPC 调用 kernel 方法, kernel 调用 provider (如有),
/// 结果回 client. 整个设计无 UI 依赖, 可被任意 host (HTTP server / Tauri /
/// CLI / stdio subprocess) 包装.
#[derive(Clone)]
pub struct Kernel {
    inner: Arc<KernelInner>,
}

struct KernelInner {
    buffers: RwLock<HashMap<u32, Buffer>>,
    active: RwLock<u32>,
    next_id: RwLock<u32>,
    providers: RwLock<HashMap<String, Arc<dyn AiProvider>>>,
    /// 事件订阅 — client 通过 subscribe() 拿到 Rx, kernel 状态变化时
    /// 推送事件 (buffer_changed / mode_changed / buffer_opened 等).
    subscribers: RwLock<Vec<tokio::sync::mpsc::UnboundedSender<Event>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    BufferOpened { id: u32, name: String },
    BufferChanged { id: u32, lines: Vec<String>, cursor: Cursor },
    BufferClosed { id: u32 },
    ModeChanged { id: u32, mode: Mode },
    ActiveBufferChanged { id: u32 },
    ProviderRegistered { name: String },
    ProviderUnregistered { name: String },
}

impl Kernel {
    pub fn new() -> Self {
        let mut buffers = HashMap::new();
        let id = 0;
        buffers.insert(id, Buffer::new(id, "[scratch]"));
        Self {
            inner: Arc::new(KernelInner {
                buffers: RwLock::new(buffers),
                active: RwLock::new(id),
                next_id: RwLock::new(id + 1),
                providers: RwLock::new(HashMap::new()),
                subscribers: RwLock::new(Vec::new()),
            }),
        }
    }

    /// 订阅事件流 — 返回 Receiver. client 可以 listen 然后 push 给 WebSocket 等.
    pub async fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<Event> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.inner.subscribers.write().await.push(tx);
        rx
    }

    async fn emit(&self, ev: Event) {
        let mut subs = self.inner.subscribers.write().await;
        subs.retain(|tx| tx.send(ev.clone()).is_ok());
    }

    // ----- Buffer 管理 -----

    pub async fn open_buffer(&self, name: impl Into<String>) -> u32 {
        let mut next = self.inner.next_id.write().await;
        let id = *next;
        *next += 1;
        let buf = Buffer::new(id, name);
        self.inner.buffers.write().await.insert(id, buf);
        *self.inner.active.write().await = id;
        self.emit(Event::BufferOpened { id, name: self.inner.buffers.read().await.get(&id).unwrap().name.clone() }).await;
        self.emit(Event::ActiveBufferChanged { id }).await;
        id
    }

    pub async fn close_buffer(&self, id: u32) -> Result<(), String> {
        let mut buffers = self.inner.buffers.write().await;
        if buffers.remove(&id).is_none() {
            return Err(format!("buffer {} not found", id));
        }
        drop(buffers);
        // 切换 active 到下一个 buffer
        let buffers = self.inner.buffers.read().await;
        if let Some(&next_id) = buffers.keys().next() {
            *self.inner.active.write().await = next_id;
            self.emit(Event::ActiveBufferChanged { id: next_id }).await;
        }
        self.emit(Event::BufferClosed { id }).await;
        Ok(())
    }

    pub async fn list_buffers(&self) -> Vec<u32> {
        let buffers = self.inner.buffers.read().await;
        let mut ids: Vec<u32> = buffers.keys().copied().collect();
        ids.sort();
        ids
    }

    pub async fn active_buffer_id(&self) -> u32 {
        *self.inner.active.read().await
    }

    pub async fn set_active_buffer(&self, id: u32) -> Result<(), String> {
        let buffers = self.inner.buffers.read().await;
        if !buffers.contains_key(&id) {
            return Err(format!("buffer {} not found", id));
        }
        drop(buffers);
        *self.inner.active.write().await = id;
        self.emit(Event::ActiveBufferChanged { id }).await;
        Ok(())
    }

    pub async fn get_buffer(&self, id: u32) -> Option<Buffer> {
        self.inner.buffers.read().await.get(&id).cloned()
    }

    pub async fn set_buffer_lines(&self, id: u32, lines: Vec<String>) -> Result<(), String> {
        let mut buffers = self.inner.buffers.write().await;
        let buf = buffers.get_mut(&id).ok_or_else(|| format!("buffer {} not found", id))?;
        buf.lines = lines;
        buf.cursor = Cursor { row: 0, col: 0 };
        buf.dirty = true;
        self.emit(Event::BufferChanged {
            id,
            lines: buf.lines.clone(),
            cursor: buf.cursor.clone(),
        }).await;
        Ok(())
    }

    pub async fn insert_at_cursor(&self, id: u32, text: &str) -> Result<(), String> {
        let mut buffers = self.inner.buffers.write().await;
        let buf = buffers.get_mut(&id).ok_or_else(|| format!("buffer {} not found", id))?;
        // 在 cursor 位置插入 text (支持多行)
        for c in text.chars() {
            if c == '\n' {
                buf.newline();
            } else {
                buf.insert_char(c);
            }
        }
        self.emit(Event::BufferChanged {
            id,
            lines: buf.lines.clone(),
            cursor: buf.cursor.clone(),
        }).await;
        Ok(())
    }

    pub async fn key_event(&self, id: u32, key: &str, ctrl: bool, #[allow(unused_variables)] _shift: bool) -> Result<(), String> {
        let mut buffers = self.inner.buffers.write().await;
        let buf = buffers.get_mut(&id).ok_or_else(|| format!("buffer {} not found", id))?;
        match buf.mode {
            Mode::Insert => {
                if ctrl {
                    if key == "s" {
                        // save 留待 server 处理
                    }
                } else {
                    match key {
                        "Escape" => buf.set_mode(Mode::Normal),
                        "Backspace" => buf.backspace(),
                        "Enter" => buf.newline(),
                        "Tab" => {
                            buf.insert_char(' ');
                            buf.insert_char(' ');
                        }
                        "ArrowLeft" => buf.move_cursor(0, -1),
                        "ArrowRight" => buf.move_cursor(0, 1),
                        "ArrowUp" => buf.move_cursor(-1, 0),
                        "ArrowDown" => buf.move_cursor(1, 0),
                        "End" => {
                            let max_col = buf.lines.get(buf.cursor.row as usize).map(|l| l.len()).unwrap_or(0) as u32;
                            buf.cursor.col = max_col;
                        }
                        "Home" => buf.cursor.col = 0,
                        k if k.len() == 1 => {
                            if let Some(c) = k.chars().next() {
                                buf.insert_char(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Mode::Normal => {
                match key {
                    "i" => buf.set_mode(Mode::Insert),
                    "Escape" => {}
                    "h" | "ArrowLeft" => buf.move_cursor(0, -1),
                    "j" | "ArrowDown" => buf.move_cursor(1, 0),
                    "k" | "ArrowUp" => buf.move_cursor(-1, 0),
                    "l" | "ArrowRight" => buf.move_cursor(0, 1),
                    "0" => buf.cursor.col = 0,
                    "$" | "End" => {
                        let max_col = buf.lines.get(buf.cursor.row as usize).map(|l| l.len()).unwrap_or(0) as u32;
                        buf.cursor.col = max_col;
                    }
                    "u" if ctrl => { buf.undo(); }
                    "r" if ctrl => { buf.redo(); }
                    _ => {}
                }
            }
            Mode::Command => {
                // 命令行通过 cmd_text 累积, Enter 触发 execute_command
                if key == "Enter" {
                    // server 处理命令
                } else if key == "Escape" {
                    buf.set_mode(Mode::Normal);
                } else if key == "Backspace" {
                    // 删 cmd_text 末尾
                    buf.cmd_text.pop();
                } else if key.len() == 1 {
                    if let Some(c) = key.chars().next() {
                        buf.cmd_text.push(c);
                    }
                }
            }
        }
        let id_emit = id;
        let lines_emit = buf.lines.clone();
        let cursor_emit = buf.cursor.clone();
        let mode_emit = buf.mode;
        drop(buffers);
        self.emit(Event::BufferChanged {
            id: id_emit,
            lines: lines_emit,
            cursor: cursor_emit,
        }).await;
        self.emit(Event::ModeChanged { id: id_emit, mode: mode_emit }).await;
        Ok(())
    }

    pub async fn set_cursor(&self, id: u32, row: u32, col: u32) -> Result<(), String> {
        let mut buffers = self.inner.buffers.write().await;
        let buf = buffers.get_mut(&id).ok_or_else(|| format!("buffer {} not found", id))?;
        buf.set_cursor(row, col);
        let lines_emit = buf.lines.clone();
        let cursor_emit = buf.cursor.clone();
        drop(buffers);
        self.emit(Event::BufferChanged { id, lines: lines_emit, cursor: cursor_emit }).await;
        Ok(())
    }

    pub async fn execute_command(&self, cmd: &str) -> Result<serde_json::Value, String> {
        let cmd = cmd.trim();
        let active = *self.inner.active.read().await;
        let mut parts = cmd.splitn(2, ' ');
        let head = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("");
        match head {
            "w" | "write" => Ok(serde_json::json!({"saved": true})),
            "q" | "quit" => {
                if active != 0 {
                    self.close_buffer(active).await?;
                }
                Ok(serde_json::json!({"closed": active}))
            }
            "e" | "edit" => {
                let id = self.open_buffer(arg.to_string()).await;
                Ok(serde_json::json!({"opened": id}))
            }
            "buffer" | "b" => {
                let id: u32 = arg.trim().parse().map_err(|_| "expected buffer id")?;
                self.set_active_buffer(id).await?;
                Ok(serde_json::json!({"active": id}))
            }
            "buffers" | "ls" => {
                let ids = self.list_buffers().await;
                Ok(serde_json::json!({"buffers": ids}))
            }
            "providers" => {
                let providers = self.inner.providers.read().await;
                Ok(serde_json::json!({"providers": providers.keys().collect::<Vec<_>>()}))
            }
            "complete" => {
                // :complete — 在光标位置触发补全 (异步返回 provider 结果)
                self.complete_at_cursor(active).await
            }
            _ => Err(format!("unknown command: {}", head)),
        }
    }

    pub async fn set_cmd_text(&self, id: u32, text: &str) -> Result<(), String> {
        let mut buffers = self.inner.buffers.write().await;
        let buf = buffers.get_mut(&id).ok_or_else(|| format!("buffer {} not found", id))?;
        buf.cmd_text = text.to_string();
        Ok(())
    }

    pub async fn get_cmd_text(&self, id: u32) -> Result<String, String> {
        let buffers = self.inner.buffers.read().await;
        let buf = buffers.get(&id).ok_or_else(|| format!("buffer {} not found", id))?;
        Ok(buf.cmd_text.clone())
    }

    /// 触发补全 — 调用当前 active buffer 的 prefix + 第一个注册 provider.
    pub async fn complete_at_cursor(&self, id: u32) -> Result<serde_json::Value, String> {
        let buf = self.get_buffer(id).await.ok_or_else(|| format!("buffer {} not found", id))?;
        let providers = self.inner.providers.read().await;
        if providers.is_empty() {
            return Ok(serde_json::json!({"items": [], "note": "no provider registered"}));
        }
        let req = CompletionRequest {
            buffer_id: id,
            prefix: buf.prefix(),
            line_text: buf.current_line().to_string(),
            cursor: buf.cursor.clone(),
            language: buf.language.clone(),
            limit: 10,
        };
        // 取第一个 provider
        if let Some((_name, provider)) = providers.iter().next() {
            match provider.complete(req).await {
                Ok(items) => Ok(serde_json::json!({"items": items})),
                Err(e) => Err(format!("provider error: {}", e)),
            }
        } else {
            Ok(serde_json::json!({"items": []}))
        }
    }

    pub async fn edit_via_provider(
        &self,
        id: u32,
        range: Range,
        instruction: &str,
    ) -> Result<EditResponse, String> {
        let buf = self.get_buffer(id).await.ok_or_else(|| format!("buffer {} not found", id))?;
        let providers = self.inner.providers.read().await;
        if providers.is_empty() {
            return Err("no provider registered".into());
        }
        let (start_row, _start_col) = (range.start_row as usize, range.start_col as usize);
        let (end_row, _end_col) = (range.end_row as usize, range.end_col as usize);
        let context_before = buf.lines.iter().take(start_row).cloned().collect::<Vec<_>>().join("\n");
        let context_after_lines = buf.lines.iter().skip(end_row).cloned().collect::<Vec<_>>();
        let _ = context_after_lines; // 保留最后一段, 简化处理
        let req = EditRequest {
            buffer_id: id,
            range,
            instruction: instruction.into(),
            context_before: Some(context_before),
            context_after: None,
        };
        if let Some((_name, provider)) = providers.iter().next() {
            provider.edit(req).await
        } else {
            Err("no provider".into())
        }
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, String> {
        let providers = self.inner.providers.read().await;
        if providers.is_empty() {
            return Err("no provider registered".into());
        }
        if let Some((_name, provider)) = providers.iter().next() {
            provider.chat(req).await
        } else {
            Err("no provider".into())
        }
    }

    // ----- Provider registry -----

    pub async fn register_provider(&self, name: impl Into<String>, provider: Arc<dyn AiProvider>) {
        let name = name.into();
        let mut providers = self.inner.providers.write().await;
        providers.insert(name.clone(), provider);
        drop(providers);
        self.emit(Event::ProviderRegistered { name }).await;
    }

    pub async fn unregister_provider(&self, name: &str) {
        let mut providers = self.inner.providers.write().await;
        providers.remove(name);
        drop(providers);
        self.emit(Event::ProviderUnregistered { name: name.into() }).await;
    }

    pub async fn list_providers(&self) -> Vec<String> {
        self.inner.providers.read().await.keys().cloned().collect()
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

/// Idle 0.1 KernelInfo — 服务端启动时打印给 client 看的 banner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub mode: &'static str,
    pub api: Vec<&'static str>,
}

impl KernelInfo {
    pub fn current() -> Self {
        Self {
            name: "ide-shell-protocol",
            version: env!("CARGO_PKG_VERSION"),
            mode: "headless vim-like RPC",
            api: vec![
                "open_buffer", "close_buffer", "list_buffers", "set_active_buffer",
                "active_buffer_id", "get_buffer", "set_buffer_lines", "insert_at_cursor",
                "set_cursor", "key_event", "execute_command", "set_cmd_text", "get_cmd_text",
                "complete_at_cursor", "edit_via_provider", "chat",
                "register_provider", "unregister_provider", "list_providers",
                "subscribe", "kernel_info",
            ],
        }
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

impl Kernel {
    /// 单条 RPC 请求处理 — stdio / HTTP / WS 共用.
    pub async fn handle_rpc(&self, raw: &str) -> Option<String> {
        let req: RpcRequest = match serde_json::from_str(raw) {
            Ok(r) => r,
            Err(e) => {
                return Some(serde_json::to_string(&RpcResponse::err(
                    serde_json::Value::Null,
                    codes::PARSE_ERROR,
                    format!("parse error: {e}"),
                )).ok()?);
            }
        };
        let id = req.id.clone().unwrap_or(serde_json::Value::Null);
        if req.jsonrpc != "2.0" {
            return Some(serde_json::to_string(&RpcResponse::err(
                id,
                codes::INVALID_REQUEST,
                format!("jsonrpc must be 2.0, got {}", req.jsonrpc),
            )).ok()?);
        }
        // notification (id == null) → 不回 response
        let is_notification = req.id.is_none();
        let result = self.dispatch(&req.method, req.params).await;
        if is_notification {
            return None;
        }
        match result {
            Ok(value) => Some(serde_json::to_string(&RpcResponse::ok(id, value)).ok()?),
            Err(e) => {
                let code = if e.contains("not found") {
                    codes::BUFFER_NOT_FOUND
                } else if e.contains("method") {
                    codes::METHOD_NOT_FOUND
                } else {
                    codes::INVALID_PARAMS
                };
                Some(serde_json::to_string(&RpcResponse::err(id, code, e)).ok()?)
            }
        }
    }

    async fn dispatch(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        match method {
            "kernel_info" => Ok(serde_json::to_value(KernelInfo::current()).unwrap()),
            "open_buffer" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("[unnamed]");
                let id = self.open_buffer(name).await;
                Ok(serde_json::json!({"id": id}))
            }
            "close_buffer" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                self.close_buffer(id).await?;
                Ok(serde_json::json!({"closed": id}))
            }
            "list_buffers" => {
                Ok(serde_json::json!({"buffers": self.list_buffers().await}))
            }
            "set_active_buffer" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                self.set_active_buffer(id).await?;
                Ok(serde_json::json!({"active": id}))
            }
            "active_buffer_id" => {
                Ok(serde_json::json!({"id": self.active_buffer_id().await}))
            }
            "get_buffer" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let buf = self.get_buffer(id).await.ok_or("buffer not found")?;
                Ok(serde_json::to_value(buf).unwrap())
            }
            "set_buffer_lines" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let lines_v = params.get("lines").and_then(|v| v.as_array()).ok_or("missing lines")?;
                let lines: Vec<String> = lines_v.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                self.set_buffer_lines(id, lines).await?;
                Ok(serde_json::json!({"ok": true}))
            }
            "insert_at_cursor" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let text = params.get("text").and_then(|v| v.as_str()).ok_or("missing text")?;
                self.insert_at_cursor(id, text).await?;
                Ok(serde_json::json!({"ok": true}))
            }
            "key_event" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let key = params.get("key").and_then(|v| v.as_str()).ok_or("missing key")?;
                let ctrl = params.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false);
                let shift = params.get("shift").and_then(|v| v.as_bool()).unwrap_or(false);
                self.key_event(id, key, ctrl, shift).await?;
                Ok(serde_json::json!({"ok": true}))
            }
            "set_cursor" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let row = params.get("row").and_then(|v| v.as_u64()).ok_or("missing row")? as u32;
                let col = params.get("col").and_then(|v| v.as_u64()).ok_or("missing col")? as u32;
                self.set_cursor(id, row, col).await?;
                Ok(serde_json::json!({"ok": true}))
            }
            "execute_command" => {
                let cmd = params.get("cmd").and_then(|v| v.as_str()).ok_or("missing cmd")?;
                self.execute_command(cmd).await
            }
            "set_cmd_text" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let text = params.get("text").and_then(|v| v.as_str()).ok_or("missing text")?;
                self.set_cmd_text(id, text).await?;
                Ok(serde_json::json!({"ok": true}))
            }
            "get_cmd_text" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                Ok(serde_json::json!({"text": self.get_cmd_text(id).await?}))
            }
            "complete_at_cursor" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                self.complete_at_cursor(id).await
            }
            "edit_via_provider" => {
                let id = params.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as u32;
                let range_v = params.get("range").ok_or("missing range")?;
                let range: Range = serde_json::from_value(range_v.clone()).map_err(|e| e.to_string())?;
                let instruction = params.get("instruction").and_then(|v| v.as_str()).ok_or("missing instruction")?;
                let resp = self.edit_via_provider(id, range, instruction).await?;
                Ok(serde_json::to_value(resp).unwrap())
            }
            "chat" => {
                let req: ChatRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
                let resp = self.chat(req).await?;
                Ok(serde_json::to_value(resp).unwrap())
            }
            "register_provider" => {
                // register_provider 需要 AI agent 通过 HTTP / WS 触发注册, 但 provider
                // 实例必须由 Rust 进程持有 — 所以这里只返回 "provider must be registered in-process"
                Err("register_provider is in-process only. Use setup() to bind a provider at startup.".into())
            }
            "unregister_provider" => {
                let name = params.get("name").and_then(|v| v.as_str()).ok_or("missing name")?;
                self.unregister_provider(name).await;
                Ok(serde_json::json!({"ok": true}))
            }
            "list_providers" => {
                Ok(serde_json::json!({"providers": self.list_providers().await}))
            }
            _ => Err(format!("unknown method: {method}")),
        }
    }
}

/// 在 Buffer 上加 cmd_text 字段 — 命令行累积输入.
impl Buffer {
    pub fn new_with_cmd_text(id: u32, name: impl Into<String>, cmd_text: impl Into<String>) -> Self {
        Self {
            cmd_text: cmd_text.into(),
            ..Self::new(id, name)
        }
    }

    /// 直接设 cursor — 用于测试 / RPC.
    pub fn set_cursor(&mut self, row: u32, col: u32) {
        let max_row = self.lines.len().saturating_sub(1) as u32;
        self.cursor.row = row.min(max_row);
        let max_col = self.lines.get(self.cursor.row as usize).map(|l| l.len()).unwrap_or(0) as u32;
        self.cursor.col = col.min(max_col);
    }
}

#[cfg(test)]
mod tests;
