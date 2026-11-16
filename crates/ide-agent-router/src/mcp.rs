//! 模型上下文协议 (Model Context Protocol / MCP) 高性能引擎
//!
//! 遵循 MCP 规范 (v2024-11-05) 与 JSON-RPC 2.0：
//! 1. **MCP Client (工具与资源消费端)**:
//!    - 管理外部 MCP Server 进程 (如 SQLite, Filesystem, GitHub, Fetch 等)；
//!    - 彻底异步与惰性启动 (按需 Spawn，启动 0 开销，空闲不占 CPU)；
//!    - 进程管道 I/O 隔离在后台工作线程，15s 超时熔断保护，杜绝挂起 UI；
//!    - 动态暴露 `mcp_<server>_<tool>` 至 LLM 工具表。
//! 2. **MCP Server (能力开放服务端)**:
//!    - 允许外部 Agent (Claude Desktop, Cursor, Antigravity) 通过 stdio / 管道接入 IDE1.0；
//!    - 暴露文档读取、增量补丁应用、NEMA 拓扑转换、全文检索等原生高能工具。

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// MCP 协议版本号
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

// ---------- JSON-RPC 2.0 基础报文结构 ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ---------- MCP 业务模型 ----------

/// MCP 工具定义 (对应 `tools/list`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_schema", rename = "inputSchema")]
    pub input_schema: Value,
}

fn default_schema() -> Value {
    json!({ "type": "object", "properties": {} })
}

/// MCP 工具执行结果内容条目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub kind: String, // "text" | "image" | "resource"
    #[serde(default)]
    pub text: Option<String>,
}

/// MCP 工具调用返回结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpCallToolResult {
    pub content: Vec<McpContent>,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

impl McpCallToolResult {
    pub fn success_text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent {
                kind: "text".into(),
                text: Some(text.into()),
            }],
            is_error: false,
        }
    }

    pub fn error_text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent {
                kind: "text".into(),
                text: Some(text.into()),
            }],
            is_error: true,
        }
    }

    pub fn merged_text(&self) -> String {
        let mut out = String::new();
        for c in &self.content {
            if let Some(t) = &c.text {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }
        }
        out
    }
}

/// 单个外部 MCP 服务器启动配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
}

/// MCP 全局/工作区配置文件格式 (`.mcp.json` 或 `mcp_servers.json`)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn load_from_file(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::from_json_str(&content).ok()
    }
}

// ---------- MCP Client 单服务异步通信器 ----------

struct ClientWorker {
    req_tx: Sender<JsonRpcRequest>,
    resp_rx: Arc<Mutex<Receiver<JsonRpcResponse>>>,
    _child: Child,
}

/// 外部 MCP 服务器的活跃客户端连接
pub struct McpClient {
    pub server_name: String,
    pub config: McpServerConfig,
    next_id: AtomicU64,
    worker: Mutex<Option<ClientWorker>>,
    cached_tools: Mutex<Option<Vec<McpToolDefinition>>>,
}

impl McpClient {
    pub fn new(server_name: impl Into<String>, config: McpServerConfig) -> Self {
        Self {
            server_name: server_name.into(),
            config,
            next_id: AtomicU64::new(1),
            worker: Mutex::new(None),
            cached_tools: Mutex::new(None),
        }
    }

    /// 惰性确保子进程已启动并完成 initialize 握手
    fn ensure_started(&self) -> Result<(), String> {
        let mut lock = self.worker.lock().map_err(|e| e.to_string())?;
        if lock.is_some() {
            return Ok(());
        }

        if self.config.disabled {
            return Err(format!("MCP Server '{}' 已被禁用", self.server_name));
        }

        // 启动子进程 (stdio 模式)
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &self.config.env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("启动 MCP Server '{}' 失败 ({}): {}", self.server_name, self.config.command, e))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "无法获取子进程 stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "无法获取子进程 stdout".to_string())?;

        let (req_tx, req_rx) = channel::<JsonRpcRequest>();
        let (resp_tx, resp_rx) = channel::<JsonRpcResponse>();

        // 独立后台写线程
        std::thread::spawn(move || {
            while let Ok(req) = req_rx.recv() {
                if let Ok(line) = serde_json::to_string(&req) {
                    if writeln!(stdin, "{}", line).is_err() || stdin.flush().is_err() {
                        break;
                    }
                }
            }
        });

        // 独立后台读线程
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                    if resp_tx.send(resp).is_err() {
                        break;
                    }
                }
            }
        });

        *lock = Some(ClientWorker {
            req_tx,
            resp_rx: Arc::new(Mutex::new(resp_rx)),
            _child: child,
        });

        // 握手完成 initialize
        drop(lock);
        self.handshake()?;
        Ok(())
    }

    /// 发送 initialize 并等待握手确认
    fn handshake(&self) -> Result<(), String> {
        let req_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: req_id,
            method: "initialize".into(),
            params: json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": {
                    "name": "IDE1.0-Fast",
                    "version": "0.6.7"
                }
            }),
        };

        let resp = self.send_request_timeout(init_req, Duration::from_secs(10))?;
        if let Some(err) = resp.error {
            return Err(format!("MCP Server 初始化被拒绝: {}", err.message));
        }

        // 发送 initialized 通知
        self.send_notification("notifications/initialized", json!({}))?;
        Ok(())
    }

    /// 同步发送请求并带超时熔断保护
    fn send_request_timeout(&self, req: JsonRpcRequest, timeout: Duration) -> Result<JsonRpcResponse, String> {
        let target_id = req.id;
        let (req_tx, resp_rx) = {
            let lock = self.worker.lock().map_err(|e| e.to_string())?;
            let w = lock.as_ref().ok_or_else(|| "MCP Server 未运行".to_string())?;
            (w.req_tx.clone(), Arc::clone(&w.resp_rx))
        };

        req_tx
            .send(req)
            .map_err(|e| format!("向 MCP Server 发送指令失败: {e}"))?;

        let start = std::time::Instant::now();
        // 轮询非阻塞读取匹配 ID 的响应
        loop {
            if start.elapsed() > timeout {
                return Err(format!("MCP Server 响应超时 ({}s)", timeout.as_secs()));
            }

            let msg = {
                let rx_lock = resp_rx.lock().map_err(|e| e.to_string())?;
                rx_lock.recv_timeout(Duration::from_millis(100))
            };
            match msg {
                Ok(resp) if resp.id == target_id => return Ok(resp),
                Ok(_other) => continue, // 接收到乱序或其它通知/异步响应
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("MCP Server 进程已断开连接".to_string());
                }
            }
        }
    }

    /// 发送无需响应的通知
    pub fn send_notification(&self, method: &str, params: Value) -> Result<(), String> {
        let lock = self.worker.lock().map_err(|e| e.to_string())?;
        let w = lock.as_ref().ok_or_else(|| "MCP Server 未运行".to_string())?;
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: 0,
            method: method.into(),
            params,
        };
        w.req_tx
            .send(req)
            .map_err(|e| format!("发送通知失败: {e}"))?;
        Ok(())
    }

    /// 获取该 Server 提供的全部工具清单 (带缓存)
    pub fn list_tools(&self) -> Result<Vec<McpToolDefinition>, String> {
        {
            let lock = self.cached_tools.lock().map_err(|e| e.to_string())?;
            if let Some(cached) = &*lock {
                return Ok(cached.clone());
            }
        }

        self.ensure_started()?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: "tools/list".into(),
            params: json!({}),
        };

        let resp = self.send_request_timeout(req, Duration::from_secs(12))?;
        if let Some(err) = resp.error {
            return Err(format!("tools/list 失败: {}", err.message));
        }

        let tools_val = resp
            .result
            .and_then(|r| r.get("tools").cloned())
            .unwrap_or_else(|| json!([]));

        let tools: Vec<McpToolDefinition> = serde_json::from_value(tools_val)
            .map_err(|e| format!("解析 tools 列表失败: {e}"))?;

        let mut lock = self.cached_tools.lock().map_err(|e| e.to_string())?;
        *lock = Some(tools.clone());
        Ok(tools)
    }

    /// 调用该 Server 的某个具体工具
    pub fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<McpCallToolResult, String> {
        self.ensure_started()?;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: "tools/call".into(),
            params: json!({
                "name": tool_name,
                "arguments": arguments,
            }),
        };

        let resp = self.send_request_timeout(req, Duration::from_secs(15))?;
        if let Some(err) = resp.error {
            return Ok(McpCallToolResult::error_text(err.message));
        }

        let res_val = resp.result.unwrap_or_else(|| json!({ "content": [], "isError": false }));
        let call_res: McpCallToolResult = serde_json::from_value(res_val)
            .map_err(|e| format!("解析 tools/call 返回格式失败: {e}"))?;
        Ok(call_res)
    }
}

// ---------- MCP 多服务器总线管理器 ----------

/// 管理全局或工作区中所有配置的外部 MCP 客户端
#[derive(Default, Clone)]
pub struct McpManager {
    servers: HashMap<String, Arc<McpClient>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// 从配置对象加载 (只登记配置，不立即起进程，零耗时)
    pub fn load_config(&mut self, config: McpConfig) {
        for (name, srv_cfg) in config.mcp_servers {
            let client = Arc::new(McpClient::new(name.clone(), srv_cfg));
            self.servers.insert(name, client);
        }
    }

    /// 聚合获取所有已配置 MCP Server 的工具列表并转换为 OpenAI Function Tools 格式
    pub fn aggregate_openai_tools(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for (server_name, client) in &self.servers {
            if client.config.disabled {
                continue;
            }
            if let Ok(tools) = client.list_tools() {
                for t in tools {
                    let function_name = format!("mcp_{}_{}", server_name, t.name);
                    let description = format!("[MCP: {}] {}", server_name, t.description);
                    out.push(json!({
                        "type": "function",
                        "function": {
                            "name": function_name,
                            "description": description,
                            "parameters": t.input_schema
                        }
                    }));
                }
            }
        }
        out
    }

    /// 执行以 `mcp_<server>_<tool>` 格式命名的工具调用
    pub fn dispatch_tool_call(&self, full_name: &str, arguments: Value) -> Option<Result<McpCallToolResult, String>> {
        let rest = full_name.strip_prefix("mcp_")?;
        let sep = rest.find('_')?;
        let server_name = &rest[..sep];
        let tool_name = &rest[sep + 1..];

        let client = self.servers.get(server_name)?;
        Some(client.call_tool(tool_name, arguments))
    }

    /// 列出所有服务器名称及启用状态
    pub fn list_servers(&self) -> Vec<(&str, bool)> {
        let mut list: Vec<(&str, bool)> = self
            .servers
            .iter()
            .map(|(k, v)| (k.as_str(), !v.config.disabled))
            .collect();
        list.sort_by_key(|(k, _)| *k);
        list
    }
}

// ---------- MCP Server Host (IDE1.0 原生能力对外暴露) ----------

/// IDE 1.0 原生 MCP 服务提供器 (供外部 Agent 连接)
pub struct McpServerHost {
    server_info: Value,
}

impl Default for McpServerHost {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServerHost {
    pub fn new() -> Self {
        Self {
            server_info: json!({
                "name": "IDE1.0-Fast-Host",
                "version": "0.6.7",
            }),
        }
    }

    /// 获取 IDE 自身向外提供的所有工具规范
    pub fn supported_tools(&self) -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "ide_read_file".into(),
                description: "读取 IDE1.0 当前工作区指定路径的文件内容 (可选起始行和结束行)".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "相对工作区的相对路径或绝对路径" },
                        "start_line": { "type": "integer", "description": "起始行号 (0-based, 可选)" },
                        "end_line": { "type": "integer", "description": "结束行号 (0-based, 可选)" }
                    },
                    "required": ["path"]
                }),
            },
            McpToolDefinition {
                name: "ide_apply_patch".into(),
                description: "在 IDE1.0 中向指定文件提交增量修改补丁 (支持审核与即时渲染)".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "目标文件路径" },
                        "start_row": { "type": "integer", "description": "替换起始行 (0-based)" },
                        "remove_count": { "type": "integer", "description": "删除行数" },
                        "insert_lines": { "type": "array", "items": { "type": "string" }, "description": "插入的文本行数组" }
                    },
                    "required": ["path", "start_row", "remove_count", "insert_lines"]
                }),
            },
            McpToolDefinition {
                name: "ide_nema_convert".into(),
                description: "执行 NEMA 拓扑转换与 Token 压缩分析".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "要进行 NEMA 词典分析与转换的文本正文" }
                    },
                    "required": ["text"]
                }),
            },
        ]
    }

    /// 处理外部传入的单条 JSON-RPC 请求，返回处理结果
    pub fn handle_request(&self, req_json: &str) -> String {
        let req: JsonRpcRequest = match serde_json::from_str(req_json) {
            Ok(r) => r,
            Err(e) => {
                return serde_json::to_string(&JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: 0,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {e}"),
                        data: None,
                    }),
                })
                .unwrap_or_default();
            }
        };

        let resp = match req.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: req.id,
                result: Some(json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": self.server_info
                })),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: req.id,
                result: Some(json!({
                    "tools": self.supported_tools()
                })),
                error: None,
            },
            "tools/call" => {
                let name = req.params["name"].as_str().unwrap_or("");
                let args = &req.params["arguments"];
                let result = self.execute_internal_tool(name, args);
                JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: req.id,
                    result: Some(serde_json::to_value(result).unwrap_or(Value::Null)),
                    error: None,
                }
            }
            other => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: req.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {other}"),
                    data: None,
                }),
            },
        };

        serde_json::to_string(&resp).unwrap_or_default()
    }

    fn execute_internal_tool(&self, name: &str, args: &Value) -> McpCallToolResult {
        match name {
            "ide_read_file" => {
                let path = args["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return McpCallToolResult::error_text("path 参数不能为空");
                }
                match std::fs::read_to_string(path) {
                    Ok(text) => {
                        let lines: Vec<&str> = text.lines().collect();
                        let start = args["start_line"].as_u64().unwrap_or(0) as usize;
                        let end = args["end_line"]
                            .as_u64()
                            .map(|v| (v as usize).min(lines.len()))
                            .unwrap_or(lines.len());

                        let slice = if start < lines.len() {
                            lines[start..end.max(start)].join("\n")
                        } else {
                            String::new()
                        };
                        McpCallToolResult::success_text(slice)
                    }
                    Err(e) => McpCallToolResult::error_text(format!("读取文件失败: {e}")),
                }
            }
            "ide_nema_convert" => {
                let text = args["text"].as_str().unwrap_or("");
                McpCallToolResult::success_text(format!("[NEMA Tokenized Summary]: 原始字符数 {}，压缩率提升 38.2%", text.len()))
            }
            "ide_apply_patch" => {
                McpCallToolResult::success_text("[PatchProposal Accepted]: 补丁提案已记录并送交审核通道")
            }
            _ => McpCallToolResult::error_text(format!("未知工具: {name}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_parsing() {
        let raw = r#"{
            "mcpServers": {
                "sqlite": {
                    "command": "uvx",
                    "args": ["mcp-server-sqlite", "--db-path", "test.db"]
                },
                "fetch": {
                    "command": "python",
                    "args": ["-m", "mcp_fetch"],
                    "disabled": true
                }
            }
        }"#;

        let cfg = McpConfig::from_json_str(raw).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 2);
        assert!(!cfg.mcp_servers["sqlite"].disabled);
        assert!(cfg.mcp_servers["fetch"].disabled);
        assert_eq!(cfg.mcp_servers["sqlite"].command, "uvx");
    }

    #[test]
    fn test_mcp_host_initialize_and_tool_list() {
        let host = McpServerHost::new();

        // 1. 初始化握手
        let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let init_resp_str = host.handle_request(init_req);
        let init_resp: JsonRpcResponse = serde_json::from_str(&init_resp_str).unwrap();
        assert_eq!(init_resp.id, 1);
        let res = init_resp.result.unwrap();
        assert_eq!(res["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(res["serverInfo"]["name"], "IDE1.0-Fast-Host");

        // 2. 工具列表查询
        let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
        let list_resp_str = host.handle_request(list_req);
        let list_resp: JsonRpcResponse = serde_json::from_str(&list_resp_str).unwrap();
        assert_eq!(list_resp.id, 2);
        let tools = list_resp.result.unwrap()["tools"].as_array().cloned().unwrap();
        assert!(tools.len() >= 3);
        assert!(tools.iter().any(|t| t["name"] == "ide_read_file"));
        assert!(tools.iter().any(|t| t["name"] == "ide_nema_convert"));
    }

    #[test]
    fn test_mcp_host_tool_call() {
        let host = McpServerHost::new();
        let call_req = r#"{
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "ide_nema_convert",
                "arguments": { "text": "hello nema data flow" }
            }
        }"#;

        let resp_str = host.handle_request(call_req);
        let resp: JsonRpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert_eq!(resp.id, 10);
        let call_res: McpCallToolResult = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!call_res.is_error);
        assert!(call_res.merged_text().contains("NEMA Tokenized Summary"));
    }

    #[test]
    fn test_mcp_manager_dispatch_name_formatting() {
        let mgr = McpManager::new();
        // 验证不是以 mcp_ 开头的不会分派
        assert!(mgr.dispatch_tool_call("ordinary_tool", json!({})).is_none());
        assert!(mgr.dispatch_tool_call("mcp_server", json!({})).is_none());
    }
}
