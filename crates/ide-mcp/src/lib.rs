//! IDE1.0 Model Context Protocol (MCP) 核心服务库。
//!
//! 提供面向各大 AI 智能体（Claude / GPT-4 / DeepSeek / Gemini / Antigravity 等）的标准化协议实现：
//! 1. 跨供应商共同编辑同一文件的无头协同编辑器（CAS 版本控制 + 3-Way 自动行合并 + 冲突保护）
//! 2. 基于有界 Myers 算法的高性能文本差异对比（Unified Diff）
//! 3. 对标 Blender Python API 架构的 Rust 自动化操控接口（`ide::ops` / `ide::data` / `ide::context`）
//! 4. 浏览器全功能 Web 版即时唤醒与协同链接构造
//! 5. 纯 Safe Rust 编写并完整对齐 `editcore` 12 大智能分析工具

#![forbid(unsafe_code)]

pub mod diff;
pub mod merge;
pub mod rust_console;

use merge::{CoEditWorkspace, ConcurrentCoEditWorkspace, EditRequest};
use rust_console::RustConsoleEvaluator;
use serde_json::{json, Value};
use std::sync::Arc;

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "ide10-mcp";
pub const SERVER_VERSION: &str = "0.8.7";
pub const DEFAULT_WEB_URL: &str = "https://ide1-0.vercel.app";

pub struct McpServer {
    pub workspace: CoEditWorkspace,
    pub concurrent_workspace: Arc<ConcurrentCoEditWorkspace>,
    pub web_url: String,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new(DEFAULT_WEB_URL)
    }
}

impl McpServer {
    pub fn new(web_url: impl Into<String>) -> Self {
        Self {
            workspace: CoEditWorkspace::default(),
            concurrent_workspace: Arc::new(ConcurrentCoEditWorkspace::default()),
            web_url: web_url.into(),
        }
    }

    /// 处理单条标准 JSON-RPC 2.0 请求
    pub fn handle_jsonrpc(&mut self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str)?;

        // JSON-RPC Notification: 无需回包
        if method == "notifications/initialized" || method == "cancelled" {
            return None;
        }

        let resp_id = id.unwrap_or(Value::Null);

        match method {
            "ping" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {}
            })),

            "initialize" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false },
                        "prompts": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION,
                        "description": "IDE1.0 Headless Multi-Provider Co-Editing, Diff & Automation MCP Server"
                    }
                }
            })),

            "tools/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "tools": self.list_tools()
                }
            })),

            "tools/call" => {
                let params = req.get("params")?;
                let tool_name = params.get("name").and_then(Value::as_str)?;
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let result = self.call_tool(tool_name, &args);
                match result {
                    Ok(val) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": if val.is_string() { val.as_str().unwrap().to_string() } else { serde_json::to_string_pretty(&val).unwrap_or_default() }
                                }
                            ],
                            "structured_data": val,
                            "isError": false
                        }
                    })),
                    Err(err) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "result": {
                            "content": [
                                { "type": "text", "text": format!("Error: {err}") }
                            ],
                            "isError": true
                        }
                    })),
                }
            }

            "resources/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "resources": [
                        {
                            "uri": "ide10://files",
                            "name": "Workspace Open Documents",
                            "description": "List of active collaborative files with revision metadata",
                            "mimeType": "application/json"
                        }
                    ]
                }
            })),

            "resources/read" => {
                let params = req.get("params")?;
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                if uri == "ide10://files" {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "result": {
                            "contents": [
                                {
                                    "uri": uri,
                                    "mimeType": "application/json",
                                    "text": serde_json::to_string_pretty(&self.workspace.list_docs()).unwrap_or_default()
                                }
                            ]
                        }
                    }))
                } else if let Some(file_id) = uri.strip_prefix("ide10://file/") {
                    match self.workspace.read_doc(file_id, None, None) {
                        Ok(doc) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": resp_id,
                            "result": {
                                "contents": [
                                    {
                                        "uri": uri,
                                        "mimeType": "text/plain",
                                        "text": doc["content"].as_str().unwrap_or("").to_string()
                                    }
                                ]
                            }
                        })),
                        Err(e) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": resp_id,
                            "error": { "code": -32602, "message": e }
                        })),
                    }
                } else {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "error": { "code": -32602, "message": format!("Unknown resource URI: {uri}") }
                    }))
                }
            }

            "prompts/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "prompts": [
                        {
                            "name": "cross_provider_coedit",
                            "description": "Orchestrate multi-model co-editing on the same codebase using atomic CAS and 3-way merge",
                            "arguments": [
                                { "name": "goal", "description": "Editing goal or task description", "required": true }
                            ]
                        },
                        {
                            "name": "blender_rust_automation",
                            "description": "Control the IDE via Blender-style ide::ops Rust script instructions",
                            "arguments": [
                                { "name": "task", "description": "Automation workflow to execute", "required": true }
                            ]
                        }
                    ]
                }
            })),

            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {method}")
                }
            })),
        }
    }

    pub fn list_tools(&self) -> Vec<Value> {
        let mut tools = vec![
            json!({
                "name": "headless_open",
                "description": "打开或在无头协同编辑器中登记新文件（若已存在则直接返回当前版本与内容哈希）",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": { "type": "string", "description": "文件唯一标识 (如 'src/main.rs')" },
                        "content": { "type": "string", "description": "文件初始内容" },
                        "language": { "type": "string", "description": "文件语言 (如 'rust', 'markdown', 'typescript')" },
                        "provider": { "type": "string", "description": "调用方供应商标识 (如 'anthropic', 'openai', 'deepseek')" },
                        "agent_id": { "type": "string", "description": "操作智能体标识" }
                    },
                    "required": ["file_id", "content"]
                }
            }),
            json!({
                "name": "headless_read",
                "description": "读取无头编辑器中指定文档的正文与版本元数据 (支持行切片)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": { "type": "string" },
                        "start_line": { "type": "integer", "minimum": 1, "description": "1-based 起始行" },
                        "end_line": { "type": "integer", "minimum": 1, "description": "1-based 结束行" }
                    },
                    "required": ["file_id"]
                }
            }),
            json!({
                "name": "headless_coedit",
                "description": "跨供应商多 Agent 无头协同编辑：支持 Git 式 (git) 与 NEMA 结构化语法 (nema) 两种协作模式。具备 CAS 快径、Myers 3-Way 自动合并、NEMA 语法封闭性守护与红绿冲突保护",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": { "type": "string", "description": "目标文件 ID (如 'default:src/main.rs' 或 'docs/spec.nema')" },
                        "author": { "type": "string", "description": "提交 Agent 或供应商标识 (如 'claude-3-7-sonnet', 'gpt-4o')" },
                        "new_text": { "type": "string", "description": "完整的待提交新文本" },
                        "base_revision": { "type": "integer", "description": "编辑所基于的基准版本号" },
                        "op_id": { "type": "string", "description": "可选操作唯一键，支持幂等重放" },
                        "mode": { "type": "string", "enum": ["git", "nema"], "description": "协作模式：'git' 为无类型行级 3-Way 合并与冲突标记；'nema' 为 NEMA v1.5 语法封闭性守护与域级安全合并" }
                    },
                    "required": ["file_id", "author", "new_text"]
                }
            }),
            json!({
                "name": "headless_edit",
                "description": "跨供应商共同编辑同一文件：具备 CAS 保护与 3-Way 自动合并。多 Agent 基于历史版本并发修改时，若改动不冲突将全自动平滑合入并生成新版本，若重叠冲突则拒止并返回差异冲突块",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": { "type": "string", "description": "目标文件 ID" },
                        "provider": { "type": "string", "description": "AI 供应商名称 (如 'anthropic', 'openai')" },
                        "agent_id": { "type": "string", "description": "Agent 名称" },
                        "base_revision": { "type": "integer", "description": "开始编辑时的基线版本号 (CAS 参照系)" },
                        "start_line": { "type": "integer", "minimum": 1, "description": "1-based 替换起始行 (闭区间)" },
                        "end_line": { "type": "integer", "minimum": 1, "description": "1-based 替换结束行 (若小于起始行则为纯插入)" },
                        "replacement": { "type": "string", "description": "新替换文本行" },
                        "op_id": { "type": "string", "description": "可选幂等操作唯一键，防止网络重试造成重复写入" },
                        "summary": { "type": "string", "description": "提交摘要说明" }
                    },
                    "required": ["file_id", "start_line", "end_line", "replacement"]
                }
            }),
            json!({
                "name": "headless_three_way_merge",
                "description": "纯算法行级 3-Way 差异合并：以 base 为基准，结合 ours 与 theirs 的修改，输出合并文本或冲突标记",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "base": { "type": "string" },
                        "ours": { "type": "string" },
                        "theirs": { "type": "string" }
                    },
                    "required": ["base", "ours", "theirs"]
                }
            }),
            json!({
                "name": "headless_history",
                "description": "查询无头编辑器中文档的全量版本历史与各 Agent 贡献归属",
                "inputSchema": {
                    "type": "object",
                    "properties": { "file_id": { "type": "string" } },
                    "required": ["file_id"]
                }
            }),
            json!({
                "name": "headless_list_files",
                "description": "列出无头协同工作区中当前打开的所有文件",
                "inputSchema": { "type": "object", "properties": {} }
            }),
            json!({
                "name": "diff_unified",
                "description": "计算两个文本间的 Unified Diff (含 `@@ -a,b +c,d @@` 补丁头与增减行统计)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "old_text": { "type": "string" },
                        "new_text": { "type": "string" },
                        "context_lines": { "type": "integer", "default": 3 }
                    },
                    "required": ["old_text", "new_text"]
                }
            }),
            json!({
                "name": "diff_revisions",
                "description": "比对无头文档中任意两个版本之间的 Unified Diff",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": { "type": "string" },
                        "from_revision": { "type": "integer" },
                        "to_revision": { "type": "integer" },
                        "context_lines": { "type": "integer", "default": 3 }
                    },
                    "required": ["file_id", "from_revision", "to_revision"]
                }
            }),
            json!({
                "name": "ide_rust_eval",
                "description": "像 Blender 的 Python 控制台一样，通过纯 Rust 代码批量操控 IDE 所有能力（支持 `ide::context::*`, `ide::data::*`, `ide::ops::*`）",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "script": { "type": "string", "description": "Rust 自动化脚本代码（支持以分号或换行分隔多个指令）" }
                    },
                    "required": ["script"]
                }
            }),
            json!({
                "name": "ide_launch_browser",
                "description": "获取或启动 IDE1.0 的云端浏览器旗舰工作台 (Vercel)，支持携带目标文件与任务深层链接",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": { "type": "string", "description": "可选聚焦的文件" },
                        "open_local": { "type": "boolean", "default": false, "description": "是否同时在宿主默认浏览器中拉起该窗口" }
                    }
                }
            }),
        ];

        // 引入 editcore 的 12 项智能工具
        for t in editcore::agent_api::TOOLS {
            let schema: Value =
                serde_json::from_str(t.schema).unwrap_or(json!({ "type": "object" }));
            tools.push(json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": schema
            }));
        }

        tools
    }

    pub fn call_tool(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "headless_open" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                let content = args
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or("Missing content")?;
                let language = args
                    .get("language")
                    .and_then(Value::as_str)
                    .unwrap_or("plain");
                let provider = args
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("agent");
                let agent_id = args
                    .get("agent_id")
                    .and_then(Value::as_str)
                    .unwrap_or("worker");
                self.concurrent_workspace
                    .open_doc(file_id, content, language, provider, agent_id);
                let doc = self
                    .workspace
                    .open_doc(file_id, content, language, provider, agent_id);
                Ok(json!({
                    "status": "OPENED",
                    "file_id": doc.file_id,
                    "language": doc.language,
                    "current_revision": doc.current_revision,
                    "content_hash": doc.content_hash
                }))
            }

            "headless_read" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                let s = args
                    .get("start_line")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                let e = args
                    .get("end_line")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                if let Ok(res) = self.concurrent_workspace.read_doc(file_id, s, e) {
                    Ok(res)
                } else {
                    self.workspace.read_doc(file_id, s, e)
                }
            }

            "headless_coedit" => {
                let req: merge::FullCoEditRequest = serde_json::from_value(args.clone())
                    .map_err(|e| format!("Invalid coedit arguments: {e}"))?;
                self.concurrent_workspace.coedit_concurrent(req)
            }

            "headless_edit" => {
                let req: EditRequest = serde_json::from_value(args.clone())
                    .map_err(|e| format!("Invalid edit arguments: {e}"))?;
                self.concurrent_workspace.edit_doc(req)
            }

            "headless_three_way_merge" => {
                let base = args
                    .get("base")
                    .and_then(Value::as_str)
                    .ok_or("Missing base")?;
                let ours = args
                    .get("ours")
                    .and_then(Value::as_str)
                    .ok_or("Missing ours")?;
                let theirs = args
                    .get("theirs")
                    .and_then(Value::as_str)
                    .ok_or("Missing theirs")?;
                let m = merge::three_way_merge(base, ours, theirs);
                Ok(json!(m))
            }

            "headless_history" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                if let Ok(res) = self.concurrent_workspace.doc_history(file_id) {
                    Ok(res)
                } else {
                    self.workspace.doc_history(file_id)
                }
            }

            "headless_list_files" => Ok(self.concurrent_workspace.list_docs()),

            "diff_unified" => {
                let old_t = args
                    .get("old_text")
                    .and_then(Value::as_str)
                    .ok_or("Missing old_text")?;
                let new_t = args
                    .get("new_text")
                    .and_then(Value::as_str)
                    .ok_or("Missing new_text")?;
                let ctx = args
                    .get("context_lines")
                    .and_then(Value::as_u64)
                    .unwrap_or(3) as usize;
                let d = diff::diff_text(old_t, new_t, ctx);
                let u = diff::unified(&d, "original", "modified");
                Ok(json!({
                    "identical": d.identical,
                    "added": d.added,
                    "removed": d.removed,
                    "coarse": d.coarse,
                    "unified_diff": u
                }))
            }

            "diff_revisions" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                let from_r = args
                    .get("from_revision")
                    .and_then(Value::as_u64)
                    .ok_or("Missing from_revision")?;
                let to_r = args
                    .get("to_revision")
                    .and_then(Value::as_u64)
                    .ok_or("Missing to_revision")?;
                let ctx = args
                    .get("context_lines")
                    .and_then(Value::as_u64)
                    .unwrap_or(3) as usize;
                if let Ok(res) = self
                    .concurrent_workspace
                    .diff_revisions(file_id, from_r, to_r, ctx)
                {
                    Ok(res)
                } else {
                    self.workspace.diff_revisions(file_id, from_r, to_r, ctx)
                }
            }

            "ide_rust_eval" => {
                let script = args
                    .get("script")
                    .and_then(Value::as_str)
                    .ok_or("Missing script")?;
                let res = RustConsoleEvaluator::eval(&mut self.workspace, script);
                let steps_json: Vec<Value> = res
                    .steps
                    .iter()
                    .map(|s| json!({ "expr": s.expr, "ok": s.ok, "output": s.output }))
                    .collect();
                Ok(json!({
                    "ok": res.ok,
                    "summary": res.summary,
                    "steps": steps_json
                }))
            }

            "ide_launch_browser" => {
                let file_id = args.get("file_id").and_then(Value::as_str);
                let open_local = args
                    .get("open_local")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let target_url = if let Some(fid) = file_id {
                    format!("{}/#file={}", self.web_url, url_encode(fid))
                } else {
                    self.web_url.clone()
                };

                if open_local {
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("powershell")
                            .args([
                                "-NoProfile",
                                "-WindowStyle",
                                "Hidden",
                                "-Command",
                                &format!("Start-Process '{}'", target_url),
                            ])
                            .spawn();
                    }
                }

                Ok(json!({
                    "web_url": target_url,
                    "launched_locally": open_local,
                    "status": "READY",
                    "note": "Connect to this URL via browser to inspect and interact with the live workbench UI"
                }))
            }

            // editcore 12 tools
            _ => editcore::agent_api::call(name, args),
        }
    }

    /// 纯只读引用的并发工具调用接口（无需 `&mut self`，成百上千个 Agent 可同时跨线程调用）
    pub fn call_tool_concurrent(&self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "headless_open" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                let content = args
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or("Missing content")?;
                let language = args
                    .get("language")
                    .and_then(Value::as_str)
                    .unwrap_or("plain");
                let provider = args
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("agent");
                let agent_id = args
                    .get("agent_id")
                    .and_then(Value::as_str)
                    .unwrap_or("worker");
                Ok(self
                    .concurrent_workspace
                    .open_doc(file_id, content, language, provider, agent_id))
            }
            "headless_read" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                let s = args
                    .get("start_line")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                let e = args
                    .get("end_line")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                self.concurrent_workspace.read_doc(file_id, s, e)
            }
            "headless_coedit" => {
                let req: merge::FullCoEditRequest = serde_json::from_value(args.clone())
                    .map_err(|e| format!("Invalid coedit arguments: {e}"))?;
                self.concurrent_workspace.coedit_concurrent(req)
            }
            "headless_edit" => {
                let req: EditRequest = serde_json::from_value(args.clone())
                    .map_err(|e| format!("Invalid edit arguments: {e}"))?;
                self.concurrent_workspace.edit_doc(req)
            }
            "headless_three_way_merge" => {
                let base = args
                    .get("base")
                    .and_then(Value::as_str)
                    .ok_or("Missing base")?;
                let ours = args
                    .get("ours")
                    .and_then(Value::as_str)
                    .ok_or("Missing ours")?;
                let theirs = args
                    .get("theirs")
                    .and_then(Value::as_str)
                    .ok_or("Missing theirs")?;
                let m = merge::three_way_merge(base, ours, theirs);
                Ok(json!(m))
            }
            "headless_history" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                self.concurrent_workspace.doc_history(file_id)
            }
            "headless_list_files" => Ok(self.concurrent_workspace.list_docs()),
            "diff_unified" => {
                let old_t = args
                    .get("old_text")
                    .and_then(Value::as_str)
                    .ok_or("Missing old_text")?;
                let new_t = args
                    .get("new_text")
                    .and_then(Value::as_str)
                    .ok_or("Missing new_text")?;
                let ctx = args
                    .get("context_lines")
                    .and_then(Value::as_u64)
                    .unwrap_or(3) as usize;
                let d = diff::diff_text(old_t, new_t, ctx);
                let u = diff::unified(&d, "original", "modified");
                Ok(json!({
                    "identical": d.identical,
                    "added": d.added,
                    "removed": d.removed,
                    "coarse": d.coarse,
                    "unified_diff": u
                }))
            }
            "diff_revisions" => {
                let file_id = args
                    .get("file_id")
                    .and_then(Value::as_str)
                    .ok_or("Missing file_id")?;
                let from_r = args
                    .get("from_revision")
                    .and_then(Value::as_u64)
                    .ok_or("Missing from_revision")?;
                let to_r = args
                    .get("to_revision")
                    .and_then(Value::as_u64)
                    .ok_or("Missing to_revision")?;
                let ctx = args
                    .get("context_lines")
                    .and_then(Value::as_u64)
                    .unwrap_or(3) as usize;
                self.concurrent_workspace
                    .diff_revisions(file_id, from_r, to_r, ctx)
            }
            "ide_launch_browser" => {
                let file_id = args.get("file_id").and_then(Value::as_str);
                let open_local = args
                    .get("open_local")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let target_url = if let Some(fid) = file_id {
                    format!("{}/#file={}", self.web_url, url_encode(fid))
                } else {
                    self.web_url.clone()
                };

                if open_local {
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("powershell")
                            .args([
                                "-NoProfile",
                                "-WindowStyle",
                                "Hidden",
                                "-Command",
                                &format!("Start-Process '{}'", target_url),
                            ])
                            .spawn();
                    }
                }

                Ok(json!({
                    "web_url": target_url,
                    "launched_locally": open_local,
                    "status": "READY",
                    "note": "Connect to this URL via browser to inspect and interact with the live workbench UI"
                }))
            }
            _ => editcore::agent_api::call(name, args),
        }
    }

    /// 纯只读引用的并发 JSON-RPC 2.0 请求处理（成百上千个 Agent 可零锁跨线程并行调用）
    pub fn handle_jsonrpc_concurrent(&self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str)?;

        if method == "notifications/initialized" || method == "cancelled" {
            return None;
        }

        let resp_id = id.unwrap_or(Value::Null);

        match method {
            "ping" => Some(json!({ "jsonrpc": "2.0", "id": resp_id, "result": {} })),
            "initialize" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false },
                        "prompts": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION,
                        "description": "IDE1.0 Concurrent Multi-Agent Co-Editing MCP Server"
                    }
                }
            })),
            "tools/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": { "tools": self.list_tools() }
            })),
            "tools/call" => {
                let params = req.get("params")?;
                let tool_name = params.get("name").and_then(Value::as_str)?;
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let result = self.call_tool_concurrent(tool_name, &args);
                match result {
                    Ok(val) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": if val.is_string() { val.as_str().unwrap().to_string() } else { serde_json::to_string_pretty(&val).unwrap_or_default() }
                            }],
                            "structured_data": val,
                            "isError": false
                        }
                    })),
                    Err(err) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Error: {err}") }],
                            "isError": true
                        }
                    })),
                }
            }
            "resources/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "resources": [{
                        "uri": "ide10://files",
                        "name": "Workspace Open Documents",
                        "description": "List of active collaborative files with revision metadata",
                        "mimeType": "application/json"
                    }]
                }
            })),
            "resources/read" => {
                let params = req.get("params")?;
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                if uri == "ide10://files" {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "result": {
                            "contents": [{
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": serde_json::to_string_pretty(&self.concurrent_workspace.list_docs()).unwrap_or_default()
                            }]
                        }
                    }))
                } else if let Some(file_id) = uri.strip_prefix("ide10://file/") {
                    match self.concurrent_workspace.read_doc(file_id, None, None) {
                        Ok(doc) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": resp_id,
                            "result": {
                                "contents": [{
                                    "uri": uri,
                                    "mimeType": "text/plain",
                                    "text": doc["content"].as_str().unwrap_or("").to_string()
                                }]
                            }
                        })),
                        Err(e) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": resp_id,
                            "error": { "code": -32602, "message": e }
                        })),
                    }
                } else {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": resp_id,
                        "error": { "code": -32602, "message": format!("Unknown resource URI: {uri}") }
                    }))
                }
            }
            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            })),
        }
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let mut server = McpServer::default();

        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        });
        let init_resp = server.handle_jsonrpc(&init_req).unwrap();
        assert_eq!(init_resp["result"]["serverInfo"]["name"], SERVER_NAME);

        let list_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        let list_resp = server.handle_jsonrpc(&list_req).unwrap();
        let tools = list_resp["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 22); // 10 custom + 12 editcore

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"headless_open"));
        assert!(names.contains(&"headless_edit"));
        assert!(names.contains(&"diff_unified"));
        assert!(names.contains(&"diff_revisions"));
        assert!(names.contains(&"ide_rust_eval"));
        assert!(names.contains(&"ide_launch_browser"));
        assert!(names.contains(&"md_outline"));
        assert!(names.contains(&"propose_patch"));
    }

    #[test]
    fn test_mcp_cross_provider_coedit_via_tools_call() {
        let mut server = McpServer::default();

        // 1. Claude 编辑 add 函数
        let req1 = json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "tools/call",
            "params": {
                "name": "headless_edit",
                "arguments": {
                    "file_id": "default:src/math.rs",
                    "provider": "anthropic",
                    "agent_id": "claude",
                    "base_revision": 1,
                    "start_line": 1,
                    "end_line": 3,
                    "replacement": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b + 1\n}"
                }
            }
        });
        let resp1 = server.handle_jsonrpc(&req1).unwrap();
        assert_eq!(resp1["result"]["isError"], false);
        assert_eq!(resp1["result"]["structured_data"]["new_revision"], 2);

        // 2. GPT 并发基于 rev 1 编辑 multiply 函数
        let req2 = json!({
            "jsonrpc": "2.0",
            "id": 102,
            "method": "tools/call",
            "params": {
                "name": "headless_edit",
                "arguments": {
                    "file_id": "default:src/math.rs",
                    "provider": "openai",
                    "agent_id": "gpt-4o",
                    "base_revision": 1,
                    "start_line": 5,
                    "end_line": 7,
                    "replacement": "pub fn multiply(a: i32, b: i32) -> i32 {\n    a * b * 2\n}"
                }
            }
        });
        let resp2 = server.handle_jsonrpc(&req2).unwrap();
        assert_eq!(resp2["result"]["isError"], false);
        assert_eq!(resp2["result"]["structured_data"]["new_revision"], 3);
        assert_eq!(resp2["result"]["structured_data"]["auto_merged"], true);

        // 3. 校验最终内容
        let read_req = json!({
            "jsonrpc": "2.0",
            "id": 103,
            "method": "tools/call",
            "params": {
                "name": "headless_read",
                "arguments": { "file_id": "default:src/math.rs" }
            }
        });
        let read_resp = server.handle_jsonrpc(&read_req).unwrap();
        let content = read_resp["result"]["structured_data"]["content"]
            .as_str()
            .unwrap();
        assert!(content.contains("a + b + 1"));
        assert!(content.contains("a * b * 2"));
    }

    #[test]
    fn test_mcp_ide_rust_eval_via_tools_call() {
        let mut server = McpServer::default();
        let eval_req = json!({
            "jsonrpc": "2.0",
            "id": 201,
            "method": "tools/call",
            "params": {
                "name": "ide_rust_eval",
                "arguments": {
                    "script": "ide::context::status(); ide::ops::search(\"default:src/math.rs\", \"multiply\")"
                }
            }
        });
        let resp = server.handle_jsonrpc(&eval_req).unwrap();
        assert_eq!(resp["result"]["isError"], false);
        assert_eq!(resp["result"]["structured_data"]["ok"], true);
    }

    #[test]
    fn test_mcp_concurrent_multi_thread_coedit_via_immutable_server() {
        let server = Arc::new(McpServer::default());

        let s1 = Arc::clone(&server);
        let s2 = Arc::clone(&server);

        let h1 = std::thread::spawn(move || {
            let req = json!({
                "jsonrpc": "2.0",
                "id": 301,
                "method": "tools/call",
                "params": {
                    "name": "headless_coedit",
                    "arguments": {
                        "file_id": "default:src/math.rs",
                        "author": "claude",
                        "new_text": "// Claude Header\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n",
                        "base_revision": 1
                    }
                }
            });
            s1.handle_jsonrpc_concurrent(&req).unwrap()
        });

        let h2 = std::thread::spawn(move || {
            let req = json!({
                "jsonrpc": "2.0",
                "id": 302,
                "method": "tools/call",
                "params": {
                    "name": "headless_coedit",
                    "arguments": {
                        "file_id": "default:src/math.rs",
                        "author": "gpt",
                        "new_text": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n// GPT Footer\n",
                        "base_revision": 1
                    }
                }
            });
            s2.handle_jsonrpc_concurrent(&req).unwrap()
        });

        let resp1 = h1.join().unwrap();
        let resp2 = h2.join().unwrap();

        assert_eq!(resp1["result"]["isError"], false);
        assert_eq!(resp2["result"]["isError"], false);

        // Read final content
        let read_req = json!({
            "jsonrpc": "2.0",
            "id": 303,
            "method": "tools/call",
            "params": {
                "name": "headless_read",
                "arguments": { "file_id": "default:src/math.rs" }
            }
        });
        let read_resp = server.handle_jsonrpc_concurrent(&read_req).unwrap();
        let content = read_resp["result"]["structured_data"]["content"]
            .as_str()
            .unwrap();
        assert!(
            content.contains("// Claude Header"),
            "Claude header must be retained"
        );
        assert!(
            content.contains("// GPT Footer"),
            "GPT footer must be retained"
        );
    }
}
