//! 提示词与对话历史: 上下文(选中 / 当前章节)、系统提示、请求体、消息裁剪。

use super::config::AiConfig;
use serde_json::{json, Value};

/// 下一轮对话保留的文字消息条数
pub const KEEP_MESSAGES: usize = 20;

// ---------- 上下文 / 提示词 ----------

pub struct Context {
    pub label: &'static str,
    pub rows: (usize, usize),
    pub text: String,
}

/// 选中内容; 没选中且是 Markdown 就取光标所在章节 (标题到下一个同级或更高标题); 否则不带
pub fn context_of(
    lines: &[String],
    selection: Option<String>,
    cursor_row: usize,
    is_md: bool,
) -> Option<Context> {
    if let Some(s) = selection.filter(|s| !s.is_empty()) {
        return Some(Context {
            label: "选中内容",
            rows: (0, 0),
            text: s,
        });
    }
    if !is_md || lines.is_empty() {
        return None;
    }
    let heading = |l: &str| -> usize {
        let h = l.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&h) && l[h..].starts_with(' ') {
            h
        } else {
            0
        }
    };
    let mut s = cursor_row.min(lines.len() - 1);
    while s > 0 && heading(&lines[s]) == 0 {
        s -= 1;
    }
    let lvl = heading(&lines[s]);
    if lvl == 0 {
        return None;
    }
    let mut e = s + 1;
    while e < lines.len() && !(heading(&lines[e]) > 0 && heading(&lines[e]) <= lvl) {
        e += 1;
    }
    Some(Context {
        label: "当前章节",
        rows: (s, e - 1),
        text: lines[s..e].join("\n"),
    })
}

pub fn system_prompt(doc_name: &str, total_rows: usize, ctx: Option<&Context>) -> String {
    let mut s = format!("你是 IDE1.0 编辑器里的助手。只能通过提供的工具行动; 要改文档只能调用 propose_patch 提出补丁 (行号从 0 起), 由用户审核后套用。当前文档: {doc_name}, 共 {total_rows} 行 (total_rows)。");
    if let Some(c) = ctx {
        s.push_str(&format!(
            "\n用户授权发送的{}{}:\n{}",
            c.label,
            if c.rows == (0, 0) {
                String::new()
            } else {
                format!(" (第 {}–{} 行)", c.rows.0 + 1, c.rows.1 + 1)
            },
            c.text
        ));
    }
    s
}

/// 对话消息的最小形态
#[derive(Clone, Debug, PartialEq)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

pub fn request_body(cfg: &AiConfig, messages: &[Value], tools: &[Value]) -> Value {
    let mut b = json!({ "model": cfg.model, "messages": messages, "stream": true });
    if !tools.is_empty() {
        b["tools"] = Value::Array(tools.to_vec());
        b["tool_choice"] = json!("auto");
    }
    if !cfg.effort.is_empty() {
        b["reasoning_effort"] = json!(cfg.effort);
    }
    b
}

/// 下一轮只留来回的文字, 工具往返不进历史
pub fn trim_history(mut h: Vec<Msg>) -> Vec<Msg> {
    if h.len() > KEEP_MESSAGES {
        h.drain(..h.len() - KEEP_MESSAGES);
    }
    h
}

/// 一轮请求的完整消息: 系统提示 + 历史 + 本次提问
pub fn base_messages(system: String, history: &[Msg], q: &str) -> Vec<Value> {
    let mut v = vec![json!({ "role": "system", "content": system })];
    v.extend(
        history
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content })),
    );
    v.push(json!({ "role": "user", "content": q }));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tool_table;

    #[test]
    fn context_prefers_selection_then_markdown_section() {
        let l: Vec<String> = ["# A", "x", "## B", "y", "# C", "z"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            context_of(&l, Some("sel".into()), 0, true).unwrap().text,
            "sel"
        );
        let c = context_of(&l, None, 3, true).unwrap();
        assert_eq!((c.text.as_str(), c.rows), ("## B\ny", (2, 3)));
        assert_eq!(context_of(&l, None, 1, true).unwrap().rows, (0, 3));
        assert!(context_of(&l, None, 3, false).is_none());
        assert!(context_of(&["no heading".to_string()], None, 0, true).is_none());
    }

    #[test]
    fn request_body_shape() {
        let c = AiConfig {
            model: "m".into(),
            effort: "low".into(),
            ..AiConfig::default_cfg()
        };
        let b = request_body(
            &c,
            &[json!({ "role": "user", "content": "hi" })],
            &tool_table(&[]),
        );
        assert_eq!(
            (
                b["stream"].clone(),
                b["reasoning_effort"].clone(),
                b["tool_choice"].clone()
            ),
            (json!(true), json!("low"), json!("auto"))
        );
        let b = request_body(&AiConfig::default_cfg(), &[], &[]);
        assert!(b.get("tools").is_none() && b.get("reasoning_effort").is_none());
        assert_eq!(
            trim_history(vec![
                Msg {
                    role: "user".into(),
                    content: "x".into()
                };
                30
            ])
            .len(),
            KEEP_MESSAGES
        );
    }
}
