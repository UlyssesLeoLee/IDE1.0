//! IDE1.0 Model Context Protocol (MCP) 核心服务库。
//!
//! 提供面向各大 AI 智能体（Claude / GPT / DeepSeek / Gemini / Codex / Cursor 等）的标准化协议：
//! - 单一权威 `DocumentStore`（CAS + 作用域幂等 + Fencing Token + 3-Way 安全自动合并 + 冲突提案）
//! - Blender 风格 `RustConsoleEvaluator` 与 MCP 工具实时共享同一状态
//! - 真实磁盘文件原子落盘、外部修改检测与 Git Diff 集成

#![forbid(unsafe_code)]

pub mod diff;
mod dispatch;
pub mod merge;
pub mod rust_console;
mod tools_schema;

use merge::DocumentStore;
use serde_json::{json, Value};
use std::sync::Arc;

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "ide10-mcp";
pub const SERVER_VERSION: &str = "0.8.9";
pub const DEFAULT_WEB_URL: &str = "https://ide1d0.vercel.app";

pub struct McpServer {
    pub store: Arc<DocumentStore>,
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
            store: Arc::new(DocumentStore::default()),
            web_url: web_url.into(),
        }
    }

    pub fn list_tools(&self) -> Vec<Value> {
        tools_schema::build_tools_catalog()
    }

    pub fn handle_jsonrpc(&mut self, req: &Value) -> Option<Value> {
        self.handle_jsonrpc_concurrent(req)
    }

    pub fn call_tool(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        self.call_tool_concurrent(name, args)
    }

    pub fn call_tool_concurrent(&self, name: &str, args: &Value) -> Result<Value, String> {
        dispatch::dispatch_tool(&self.store, &self.web_url, name, args)
    }

    /// 无锁共享引用的并发 JSON-RPC 2.0 处理器。
    pub fn handle_jsonrpc_concurrent(&self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str)?;
        if method == "notifications/initialized" || method == "cancelled" {
            return None;
        }
        let resp_id = id.unwrap_or(Value::Null);

        match method {
            "ping" => Some(json!({ "jsonrpc": "2.0", "id": resp_id, "result": {} })),
            "initialize" => Some(initialize_response(resp_id)),
            "tools/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": { "tools": self.list_tools() }
            })),
            "tools/call" => Some(self.handle_tools_call(req, resp_id)),
            "resources/list" => Some(self.handle_resources_list(resp_id)),
            "resources/read" => Some(self.handle_resources_read(req, resp_id)),
            "prompts/list" => Some(prompts_list_response(resp_id)),
            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            })),
        }
    }

    fn handle_tools_call(&self, req: &Value, resp_id: Value) -> Value {
        let Some(params) = req.get("params") else {
            return json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": { "code": -32602, "message": "Missing params" }
            });
        };
        let Some(tool_name) = params.get("name").and_then(Value::as_str) else {
            return json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": { "code": -32602, "message": "Missing tool name" }
            });
        };
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        match self.call_tool_concurrent(tool_name, &args) {
            Ok(val) => {
                let text = if let Some(s) = val.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string_pretty(&val).unwrap_or_default()
                };
                json!({
                    "jsonrpc": "2.0",
                    "id": resp_id,
                    "result": {
                        "content": [{ "type": "text", "text": text }],
                        "structured_data": val,
                        "isError": false
                    }
                })
            }
            Err(err) => json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "content": [{ "type": "text", "text": format!("Error: {err}") }],
                    "isError": true
                }
            }),
        }
    }

    fn handle_resources_list(&self, resp_id: Value) -> Value {
        let list = self.store.list_docs();
        let mut resources = vec![json!({
            "uri": "ide10://files",
            "name": "Workspace Open Documents",
            "description": "List of active collaborative files with revision metadata",
            "mimeType": "application/json"
        })];
        if let Some(arr) = list["docs"].as_array() {
            for f in arr {
                if let Some(fid) = f["file_id"].as_str() {
                    resources.push(json!({
                        "uri": format!("ide10://file/{fid}"),
                        "name": fid,
                        "mimeType": "text/plain"
                    }));
                }
            }
        }
        json!({
            "jsonrpc": "2.0",
            "id": resp_id,
            "result": { "resources": resources }
        })
    }

    fn handle_resources_read(&self, req: &Value, resp_id: Value) -> Value {
        let uri = req
            .get("params")
            .and_then(|p| p.get("uri"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if uri == "ide10://files" {
            return json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": serde_json::to_string_pretty(&self.store.list_docs()).unwrap_or_default()
                    }]
                }
            });
        }
        let file_id = uri
            .strip_prefix("ide10://file/")
            .or_else(|| uri.strip_prefix("file:///workspace/"));
        let Some(fid) = file_id else {
            return json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": { "code": -32602, "message": format!("Unknown resource URI: {uri}") }
            });
        };
        match self.store.read_doc(fid, None, None) {
            Ok(doc) => json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "result": {
                    "contents": [{
                        "uri": uri,
                        "mimeType": "text/plain",
                        "text": doc["content"].as_str().unwrap_or("")
                    }]
                }
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": resp_id,
                "error": { "code": -32602, "message": e }
            }),
        }
    }
}

fn initialize_response(resp_id: Value) -> Value {
    json!({
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
                "description": "IDE1.0 Hardened Multi-Agent Co-Editing MCP Server"
            }
        }
    })
}

fn prompts_list_response(resp_id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": resp_id,
        "result": {
            "prompts": [
                {
                    "name": "cross_provider_coedit",
                    "description": "Orchestrate multi-model co-editing using atomic CAS and 3-way merge",
                    "arguments": [{ "name": "goal", "description": "Editing goal", "required": true }]
                },
                {
                    "name": "blender_rust_automation",
                    "description": "Control IDE1.0 via ide::ops Rust Console script instructions",
                    "arguments": [{ "name": "task", "description": "Automation workflow", "required": true }]
                }
            ]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let mut server = McpServer::default();
        let init = server
            .handle_jsonrpc(
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
            )
            .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], SERVER_NAME);

        let list = server
            .handle_jsonrpc(
                &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
            )
            .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for expected in [
            "headless_open",
            "ide_open_file",
            "headless_edit",
            "ide_edit_lines",
            "ide_replace_all",
            "headless_list_proposals",
            "headless_resolve_proposal",
            "diff_unified",
            "diff_revisions",
            "ide_rust_eval",
            "ide_sync_to_disk",
            "ide_git_diff",
        ] {
            assert!(names.contains(&expected), "Missing tool {expected}");
        }
    }

    #[test]
    fn test_mcp_cross_provider_coedit_and_console_shared_state() {
        let mut server = McpServer::default();
        let r1 = server
            .handle_jsonrpc(&json!({
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
                        "op_id": "mcp-claude-1",
                        "start_line": 1,
                        "end_line": 3,
                        "replacement": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b + 1\n}"
                    }
                }
            }))
            .unwrap();
        assert_eq!(r1["result"]["isError"], false);
        assert_eq!(r1["result"]["structured_data"]["new_revision"], 2);

        // Verify Rust Console shares the EXACT same DocumentStore state
        let eval_r = server
            .call_tool(
                "ide_rust_eval",
                &json!({ "script": "ide::data::read(\"default:src/math.rs\")" }),
            )
            .unwrap();
        assert!(eval_r["steps"][0]["output"]
            .as_str()
            .unwrap()
            .contains("a + b + 1"));
    }

    #[test]
    fn test_mcp_rejects_missing_cas_fields() {
        let mut server = McpServer::default();
        let missing_op = server
            .handle_jsonrpc(&json!({
                "jsonrpc": "2.0",
                "id": 401,
                "method": "tools/call",
                "params": {
                    "name": "headless_coedit",
                    "arguments": {
                        "file_id": "default:src/math.rs",
                        "author": "claude",
                        "base_revision": 1,
                        "new_text": "// missing op_id\n"
                    }
                }
            }))
            .unwrap();
        assert_eq!(missing_op["result"]["isError"], true);
        assert!(missing_op["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("MISSING_REQUIRED_CAS_FIELD"));
    }

    #[test]
    fn test_mcp_concurrent_multi_thread_coedit_via_immutable_server() {
        let server = Arc::new(McpServer::default());
        let (s1, s2) = (Arc::clone(&server), Arc::clone(&server));

        let h1 = std::thread::spawn(move || {
            s1.handle_jsonrpc_concurrent(&json!({
                "jsonrpc": "2.0",
                "id": 301,
                "method": "tools/call",
                "params": {
                    "name": "headless_coedit",
                    "arguments": {
                        "file_id": "default:src/math.rs",
                        "author": "claude",
                        "base_revision": 1,
                        "op_id": "t-claude-1",
                        "new_text": "// Claude Header\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n"
                    }
                }
            })).unwrap()
        });

        let h2 = std::thread::spawn(move || {
            s2.handle_jsonrpc_concurrent(&json!({
                "jsonrpc": "2.0",
                "id": 302,
                "method": "tools/call",
                "params": {
                    "name": "headless_coedit",
                    "arguments": {
                        "file_id": "default:src/math.rs",
                        "author": "gpt",
                        "base_revision": 1,
                        "op_id": "t-gpt-1",
                        "new_text": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n// GPT Footer\n"
                    }
                }
            })).unwrap()
        });

        assert_eq!(h1.join().unwrap()["result"]["isError"], false);
        assert_eq!(h2.join().unwrap()["result"]["isError"], false);

        let read = server
            .call_tool_concurrent(
                "headless_read",
                &json!({ "file_id": "default:src/math.rs" }),
            )
            .unwrap();
        let content = read["content"].as_str().unwrap();
        assert!(content.contains("// Claude Header"));
        assert!(content.contains("// GPT Footer"));
    }
}
