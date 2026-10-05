# ide-shell-protocol — AI agent 集成指南

## 概述

`ide-shell-protocol` 是 IDE1.0 的 **headless vim-like RPC facade**。AI agent 通过 **HTTP + WebSocket + stdio** 三种协议, 操控 IDE 的 buffer/cursor/mode, 注入 **completion / inline edit / chat** 三种 AI provider。

设计目标:
- 对标 **vim `clientserver`** + **neovim `--embed`** — 进程嵌入友好
- 对标 **LSP** — provider 三件套 (completion / edit / chat)
- 轻量高性能 — Rust 实现, headless, 不依赖 TUI/WebView

---

## 启动 server

### 三种模式

```bash
# 1. stdio JSON-RPC (推荐: AI agent 当 subprocess, 最低延迟)
ide-shell-protocol-server stdio

# 2. HTTP + WebSocket (远程集成, AI agent 跑在另一台机器)
ide-shell-protocol-server http --addr 127.0.0.1:8123

# 3. 同时跑 stdio + HTTP (daemon 模式)
ide-shell-protocol-server both --addr 127.0.0.1:8123
```

### 默认注册 mock provider

启动时默认注册 `MockProvider` (echo 回复). 真实部署时, **AI agent 自己用 Rust 写自己的 provider**, 在 `main.rs` 里 `kernel.register_provider("anthropic", provider).await`, 然后 `ide-shell-protocol-server both` 跑.

---

## JSON-RPC 2.0 API (22 个 method)

### Buffer 管理

| method | params | result |
|---|---|---|
| `kernel_info` | — | `{ name, version, mode, api[] }` |
| `open_buffer` | `{ name }` | `{ id }` |
| `close_buffer` | `{ id }` | `{ closed }` |
| `list_buffers` | — | `{ buffers: [id, ...] }` |
| `active_buffer_id` | — | `{ id }` |
| `set_active_buffer` | `{ id }` | `{ active }` |
| `get_buffer` | `{ id }` | full `Buffer` object (id, name, path, lines, cursor, mode, dirty, language, ...) |
| `set_buffer_lines` | `{ id, lines: [str] }` | `{ ok }` |
| `insert_at_cursor` | `{ id, text }` | `{ ok }` |

### Vim-like 编辑

| method | params | result |
|---|---|---|
| `set_cursor` | `{ id, row, col }` | `{ ok }` |
| `key_event` | `{ id, key, ctrl?, shift? }` | `{ ok }` |
| `execute_command` | `{ cmd }` | 由命令决定 (`:w` → `{saved}`, `:q` → `{closed}`, `:buffers` → `{buffers}`, ...) |
| `set_cmd_text` | `{ id, text }` | `{ ok }` (`: ` 命令行累积输入) |
| `get_cmd_text` | `{ id }` | `{ text }` |

### AI Provider 三件套

| method | params | result |
|---|---|---|
| `complete_at_cursor` | `{ id, limit? }` | `{ items: [CompletionItem] }` |
| `edit_via_provider` | `{ id, range, instruction }` | `{ new_text, explanation }` |
| `chat` | `{ messages, tools }` | `{ message, tool_calls, finish_reason }` |

### Provider 管理

| method | params | result |
|---|---|---|
| `register_provider` | `{ name }` | **error: in-process only** — provider 实例必须在 Rust 进程内注册 |
| `unregister_provider` | `{ name }` | `{ ok }` |
| `list_providers` | — | `{ providers: [name, ...] }` |

### 事件订阅

| channel | 用途 |
|---|---|
| WS `/ws` | 单向订阅 — server 推 `Event` (BufferOpened / BufferChanged / ModeChanged / ActiveBufferChanged / ProviderRegistered) |
| WS `/rpcws` | 双向 JSON-RPC + 推事件 |

---

## 三个集成示例

### A. stdio JSON-RPC (Python AI agent)

```python
import json, subprocess
proc = subprocess.Popen(
    ["ide-shell-protocol-server", "stdio"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1
)

def rpc(method, params=None, id=1):
    req = {"jsonrpc":"2.0","id":id,"method":method,"params":params or {}}
    proc.stdin.write(json.dumps(req) + "\n"); proc.stdin.flush()
    line = proc.stdout.readline()
    return json.loads(line)

# 1. open buffer
bid = rpc("open_buffer", {"name": "main.rs"})["result"]["id"]

# 2. 触发补全
items = rpc("complete_at_cursor", {"id": bid})["result"]["items"]
for item in items:
    print(f"  {item['label']}: {item['insert_text']}")

# 3. inline edit
result = rpc("edit_via_provider", {
    "id": bid,
    "range": {"start_row": 0, "start_col": 0, "end_row": 0, "end_col": 0},
    "instruction": "Add error handling"
})["result"]
print(f"New text: {result['new_text']}")

# 4. chat (通用 agent)
resp = rpc("chat", {
    "messages": [{"role": "user", "content": "Help me refactor this code"}],
    "tools": []
})["result"]
print(f"Assistant: {resp['message']['content']}")
```

### B. HTTP + WebSocket (Node.js / Python)

```python
import websocket, json, requests

# 1. HTTP 单条 RPC
resp = requests.post("http://localhost:8123/rpc", json={
    "jsonrpc":"2.0","id":1,"method":"open_buffer","params":{"name":"hello.ts"}
}).json()
print(resp["result"])  # {"id": 1}

# 2. WebSocket 双向 + 推事件
ws = websocket.create_connection("ws://localhost:8123/rpcws")

# 发 RPC
ws.send(json.dumps({
    "jsonrpc":"2.0","id":2,"method":"open_buffer","params":{"name":"world.ts"}
}))
result = json.loads(ws.recv())
print(result)  # {"jsonrpc":"2.0","id":2,"result":{"id":2}}

# 同时 push 事件
event = json.loads(ws.recv())
print(event)  # {"kind":"buffer_opened","id":2,"name":"world.ts"}
```

### C. 写自己的 provider (Rust)

AI agent 平台可以用 Rust 写自己的 provider (HTTP bridge / Anthropic / OpenAI / Ollama):

```rust
use ide_shell_protocol::{AiProvider, CompletionRequest, CompletionItem, ...};
use std::sync::Arc;

struct AnthropicProvider {
    api_key: String,
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }

    async fn complete(&self, req: CompletionRequest) -> Result<Vec<CompletionItem>, String> {
        let resp = self.client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .json(&serde_json::json!({
                "model": "claude-sonnet-4",
                "messages": [{"role": "user", "content": req.prefix}],
                "max_tokens": 100,
            }))
            .send().await.map_err(|e| e.to_string())?;
        // ... 解析 resp → CompletionItem
        Ok(vec![CompletionItem { /* ... */ }])
    }
    // 类似 edit/chat
}

// 在自己的 server 二进制里:
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let kernel = Arc::new(ide_shell_protocol::Kernel::new());
    kernel.register_provider(
        "anthropic",
        Arc::new(AnthropicProvider { api_key: std::env::var("ANTHROPIC_API_KEY")?, client: Default::default() })
    ).await;
    // 跑 HTTP/stdio server
    ide_shell_protocol_server::run(kernel, "127.0.0.1:8123").await
}
```

---

## 设计哲学

1. **vim-like RPC** — `set_cursor`, `key_event`, `execute_command` 都是 vim 命令行风格, AI agent 学习成本低
2. **Provider 抽象** — completion / inline edit / chat 三件套对齐 LSP, AI agent 可自己实现
3. **事件订阅** — `subscribe()` 拿 mpsc Receiver, kernel 状态变化推 Event, 客户端 (UI / WebSocket) 实时同步
4. **headless** — 不依赖 ratatui/Tauri, 任意 host 都能集成
5. **vim/neovim 兼容** — Mode enum (`Normal`/`Insert`/`Command`)、键位 (h/j/k/l/i/a/o/u/Ctrl+R undo redo) 全部与 vim 一致, AI agent 复用 vim muscle memory

---

## Roadmap

当前 (ULYS-191 §5 v0.1):
- ✅ Headless vim-like state (Buffer, Cursor, Mode, KeyEvent, undo/redo)
- ✅ 22 RPC methods (buffer / cursor / key / command / completion / edit / chat / providers / subscribe)
- ✅ 三协议 server (HTTP + WS + stdio)
- ✅ Mock provider (演示)
- ✅ 20 unit + integration tests

下一阶段 (ULYS-191 §6):
- 🔄 集成 ide-shell TUI (复用 App 共享 state)
- 🔄 集成 ide-shell-web HTTP server (同一 kernel 暴露在 Tauri + Web)
- 🔄 Anthropic / OpenAI / Ollama provider examples
- 🔄 LSP-style JSON Schema for `tools[]` in chat
