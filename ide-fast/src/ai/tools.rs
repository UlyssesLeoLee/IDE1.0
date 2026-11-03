//! 工具表与工具往返: 内建 (editcore) + 插件 `agent.tool` 工具、补丁提案闸门、回给模型的 tool 消息。

use super::sse::Turn;
use serde_json::{json, Value};

/// 工具往返的最大轮数 (和 Tauri 版同量级)
pub const MAX_ROUNDS: usize = 6;
/// 单个工具结果回给模型的字符上限
pub const MAX_TOOL_RESULT: usize = 20_000;

// ---------- 工具表 ----------

/// 内建工具 (editcore) + 启用且有 `agent.tool` 权限的插件工具。返回 (OpenAI tools 数组, 名字 -> 是否插件工具)
pub fn tool_table(plugins: &[editcore::plugin_host::store::Installed]) -> Vec<Value> {
    let mut out: Vec<Value> = editcore::agent_api::tools_json().as_array().cloned().unwrap_or_default();
    for t in out.iter_mut() {
        if let Some(o) = t.as_object_mut() {
            o.remove("x-write");
        }
    }
    for p in plugins {
        if !p.enabled || !p.manifest.permissions.iter().any(|x| x == "agent.tool") {
            continue;
        }
        let Some(list) = p.manifest.contributes.get("agent.tool").and_then(Value::as_array) else { continue };
        for t in list {
            let Some(name) = t["name"].as_str().filter(|n| valid_tool_name(n)) else { continue };
            let schema = if t["schema"].is_object() { t["schema"].clone() } else { json!({ "type": "object", "properties": {} }) };
            out.push(json!({ "type": "function", "function": { "name": format!("plugin_{}_{}", p.manifest.id, name), "description": format!("[插件 {}] {}", p.manifest.name, t["description"].as_str().unwrap_or("")), "parameters": schema } }));
        }
    }
    out
}

/// 工具名合法 (和暴露给模型时用的是同一道过滤)
fn valid_tool_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 48 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 模型点名的插件工具是否真的存在: 插件已启用、有 `agent.tool` 权限、清单 `contributes["agent.tool"]` 里声明了这个名字且名字合法。
/// 满足才允许调 `call_raw`, 否则按未知工具处理 (模型不能借此调用插件的任意内部方法)。
pub fn plugin_tool_declared(plugins: &[editcore::plugin_host::store::Installed], id: &str, tool: &str) -> bool {
    valid_tool_name(tool)
        && plugins.iter().any(|p| {
            p.enabled && p.manifest.id == id && p.manifest.permissions.iter().any(|x| x == "agent.tool") && p.manifest.contributes.get("agent.tool").and_then(Value::as_array).is_some_and(|l| l.iter().any(|t| t["name"].as_str() == Some(tool)))
        })
}

/// `plugin_<id>_<tool>` -> (id, tool)
pub fn split_plugin_tool(name: &str) -> Option<(String, String)> {
    let r = name.strip_prefix("plugin_")?;
    let i = r.find('_')?;
    let (id, t) = (&r[..i], &r[i + 1..]);
    // 插件 id 允许含 '-' 不含 '_', 所以第一个 '_' 就是分隔
    if id.is_empty() || t.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') { None } else { Some((id.into(), t.into())) }
}

/// 把插件交来的补丁提案过一遍 editcore 的校验 (范围 / 行数), 和内建工具同一道闸
pub fn vet_plugin_patch(r: &Value, total_rows: usize) -> Value {
    if r["kind"] != "patch_proposal" {
        return r.clone();
    }
    match editcore::agent_api::call("propose_patch", &json!({ "start": r["start"], "remove": r["remove"], "insert": r["insert"], "total_rows": total_rows, "reason": r["reason"] })) {
        Ok(v) => v,
        Err(e) => json!({ "error": format!("插件的补丁提案无效: {e}") }),
    }
}

// ---------- 工具往返 ----------

/// 模型给的 arguments 串 -> JSON 对象; 空 / 非法 = 空对象 (和 Tauri 版一样)
pub fn parse_args(s: &str) -> Value {
    serde_json::from_str::<Value>(s).ok().filter(Value::is_object).unwrap_or_else(|| json!({}))
}

/// 执行内建工具。propose_patch 的 total_rows 一律用真实行数 (不信模型报的)
pub fn exec_builtin(name: &str, args: &Value, total_rows: usize) -> Value {
    let mut a = args.clone();
    if name == "propose_patch" {
        a["total_rows"] = json!(total_rows);
    }
    editcore::agent_api::call(name, &a).unwrap_or_else(|e| json!({ "error": e }))
}

/// 回给模型的 tool 消息: 补丁提案只回「已提交审核」, 其余结果按字符数截断
pub fn tool_message(id: &str, result: &Value) -> Value {
    let body = if result["kind"] == "patch_proposal" { json!({ "status": "已提交给用户审核, 尚未套用" }).to_string() } else { result.to_string() };
    json!({ "role": "tool", "tool_call_id": id, "content": body.chars().take(MAX_TOOL_RESULT).collect::<String>() })
}

/// 带工具调用的 assistant 消息 (OpenAI 格式)
pub fn assistant_message(t: &Turn) -> Value {
    let mut m = json!({ "role": "assistant", "content": t.content });
    if !t.tool_calls.is_empty() {
        m["tool_calls"] = Value::Array(t.tool_calls.iter().map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.args } })).collect());
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ToolCall;

    #[test]
    fn tool_table_has_builtin_and_permitted_plugin_tools_only() {
        let m = |perm: bool, en: bool| editcore::plugin_host::store::Installed {
            manifest: serde_json::from_value(json!({ "id": "doc-graph", "name": "G", "version": "1.0.0", "apiVersion": 1, "entry": "main.js", "permissions": if perm { json!(["agent.tool"]) } else { json!([]) }, "contributes": { "agent.tool": [{ "name": "stats", "description": "d", "schema": { "type": "object", "properties": {} } }, { "name": "bad name!" }] } })).unwrap(),
            enabled: en,
            source: "bundled",
            sha256: Default::default(),
        };
        let base = tool_table(&[]).len();
        assert!(tool_table(&[]).iter().all(|t| t.get("x-write").is_none()));
        assert_eq!(tool_table(&[m(false, true)]).len(), base);
        assert_eq!(tool_table(&[m(true, false)]).len(), base);
        let t = tool_table(&[m(true, true)]);
        assert_eq!(t.len(), base + 1);
        assert_eq!(t.last().unwrap()["function"]["name"], "plugin_doc-graph_stats");
        assert_eq!(split_plugin_tool("plugin_doc-graph_stats"), Some(("doc-graph".into(), "stats".into())));
        assert_eq!(split_plugin_tool("md_outline"), None);
        // 只有清单里声明过的工具才放行
        let ps = [m(true, true)];
        assert!(plugin_tool_declared(&ps, "doc-graph", "stats"));
        assert!(!plugin_tool_declared(&ps, "doc-graph", "secret_internal"), "未声明的方法");
        assert!(!plugin_tool_declared(&ps, "doc-graph", "bad name!"), "名字不合法");
        assert!(!plugin_tool_declared(&ps, "other", "stats"));
        assert!(!plugin_tool_declared(&[m(true, false)], "doc-graph", "stats"));
        assert!(!plugin_tool_declared(&[m(false, true)], "doc-graph", "stats"));
    }

    #[test]
    fn plugin_patch_goes_through_the_same_gate() {
        let ok = vet_plugin_patch(&json!({ "kind": "patch_proposal", "start": 0, "remove": 1, "insert": ["x"] }), 3);
        assert_eq!(ok["kind"], "patch_proposal");
        let bad = vet_plugin_patch(&json!({ "kind": "patch_proposal", "start": 5, "remove": 1, "insert": [] }), 3);
        assert!(bad["error"].as_str().unwrap().contains("越界"));
        assert_eq!(vet_plugin_patch(&json!({ "a": 1 }), 3), json!({ "a": 1 }));
    }

    #[test]
    fn tool_round_helpers() {
        assert_eq!(parse_args("not json"), json!({}));
        assert_eq!(parse_args("[1]"), json!({}));
        assert_eq!(parse_args("{\"a\":1}")["a"], 1);
        let r = exec_builtin("propose_patch", &json!({ "start": 0, "remove": 1, "insert": ["x"], "total_rows": 999 }), 3);
        assert_eq!(r["kind"], "patch_proposal");
        assert!(exec_builtin("propose_patch", &json!({ "start": 5, "remove": 1, "insert": [] }), 3)["error"].is_string());
        assert!(exec_builtin("nope", &json!({}), 1)["error"].is_string());
        assert!(tool_message("c", &r)["content"].as_str().unwrap().contains("尚未套用"));
        let big = json!({ "t": "字".repeat(30_000) });
        assert_eq!(tool_message("c", &big)["content"].as_str().unwrap().chars().count(), MAX_TOOL_RESULT);
        let turn = Turn { content: "hi".into(), tool_calls: vec![ToolCall { id: "i".into(), name: "n".into(), args: "{}".into() }], ..Default::default() };
        assert_eq!(assistant_message(&turn)["tool_calls"][0]["function"]["name"], "n");
        assert!(assistant_message(&Turn::default()).get("tool_calls").is_none());
        let m = crate::ai::base_messages("s".into(), &[crate::ai::Msg { role: "user".into(), content: "a".into() }], "q");
        assert_eq!((m.len(), m[2]["content"].clone()), (3, json!("q")));
    }
}
