//! SSE 流解析: 逐行喂 curl 输出, 累积文字与工具调用; 非流式 / 错误响应兜底。

use serde_json::Value;

/// 一次回复最多读这么多字节
pub const MAX_RESPONSE: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turn {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub http_status: Option<u16>,
    pub error: Option<String>,
    pub done: bool,
}

pub const STATUS_MARK: &str = "IDEFAST_HTTP:";

/// 逐行喂 curl 的输出; 返回这一行新增的可见文字
#[derive(Default)]
pub struct SseParser {
    pub turn: Turn,
    raw: String,
    saw_event: bool,
}

impl SseParser {
    pub fn feed_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim_end_matches(['\r', '\n']);
        if let Some(code) = line.strip_prefix(STATUS_MARK) {
            self.turn.http_status = code.trim().parse().ok();
            return None;
        }
        let Some(data) = line.strip_prefix("data:") else {
            if !line.trim().is_empty() && !line.starts_with(':') && !line.starts_with("event:") && !line.starts_with("id:") && !line.starts_with("retry:") && self.raw.len() < MAX_RESPONSE {
                self.raw.push_str(line);
                self.raw.push('\n');
            }
            return None;
        };
        let data = data.trim();
        if data == "[DONE]" {
            self.turn.done = true;
            return None;
        }
        let v: Value = serde_json::from_str(data).ok()?;
        self.saw_event = true;
        if let Some(e) = v["error"]["message"].as_str() {
            self.turn.error = Some(e.to_string());
            return None;
        }
        let ch = &v["choices"][0];
        let mut shown = None;
        if let Some(t) = ch["delta"]["content"].as_str().filter(|t| !t.is_empty()) {
            self.turn.content.push_str(t);
            shown = Some(t.to_string());
        }
        if let Some(tcs) = ch["delta"]["tool_calls"].as_array() {
            for tc in tcs {
                let ix = tc["index"].as_u64().unwrap_or(0).min(15) as usize;
                if self.turn.tool_calls.len() <= ix {
                    self.turn.tool_calls.resize(ix + 1, ToolCall::default());
                }
                let slot = &mut self.turn.tool_calls[ix];
                if let Some(id) = tc["id"].as_str() {
                    slot.id = id.to_string();
                }
                if let Some(n) = tc["function"]["name"].as_str() {
                    slot.name.push_str(n);
                }
                if let Some(a) = tc["function"]["arguments"].as_str() {
                    if slot.args.len() < 1_000_000 {
                        slot.args.push_str(a);
                    }
                }
            }
        }
        if ch["finish_reason"].is_string() {
            self.turn.done = true;
        }
        shown
    }

    /// 流结束: 没有任何 SSE 事件时, 把累积的原文当整段 JSON 解析 (服务器忽略 stream 或返回错误)
    pub fn finish(mut self) -> Turn {
        if !self.saw_event && !self.raw.trim().is_empty() {
            match serde_json::from_str::<Value>(self.raw.trim()) {
                Ok(v) => {
                    if let Some(e) = v["error"]["message"].as_str().or(v["error"].as_str()) {
                        self.turn.error = Some(e.to_string());
                    } else if let Some(m) = v["choices"][0]["message"].as_object() {
                        self.turn.content = m.get("content").and_then(Value::as_str).unwrap_or("").to_string();
                        for tc in m.get("tool_calls").and_then(Value::as_array).into_iter().flatten() {
                            self.turn.tool_calls.push(ToolCall { id: tc["id"].as_str().unwrap_or("").into(), name: tc["function"]["name"].as_str().unwrap_or("").into(), args: tc["function"]["arguments"].as_str().unwrap_or("{}").into() });
                        }
                        self.turn.done = true;
                    }
                }
                Err(_) => self.turn.error = Some(self.raw.trim().chars().take(300).collect()),
            }
        }
        if self.turn.error.is_none() {
            if let Some(st) = self.turn.http_status.filter(|s| !(200..300).contains(s)) {
                self.turn.error = Some(format!("HTTP {st}"));
            }
        }
        // 没有 id 的工具调用补一个, 回传时要对得上
        for (i, tc) in self.turn.tool_calls.iter_mut().enumerate() {
            if tc.id.is_empty() {
                tc.id = format!("call_{i}");
            }
        }
        self.turn.tool_calls.retain(|t| !t.name.is_empty());
        self.turn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_text_and_tool_call_deltas() {
        let mut p = SseParser::default();
        let lines = [
            r#"data: {"choices":[{"delta":{"content":"你"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"好"}}]}"#,
            r##"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"md_out","arguments":"{\"te"}}]}}]}"##,
            r##"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"line","arguments":"xt\":\"# a\"}"}}]},"finish_reason":"tool_calls"}]}"##,
            "data: [DONE]",
            "IDEFAST_HTTP:200",
        ];
        let shown: Vec<_> = lines.iter().filter_map(|l| p.feed_line(l)).collect();
        assert_eq!(shown, vec!["你", "好"]);
        let t = p.finish();
        assert_eq!(t.content, "你好");
        assert_eq!(t.tool_calls, vec![ToolCall { id: "c1".into(), name: "md_outline".into(), args: "{\"text\":\"# a\"}".into() }]);
        assert!(t.done && t.error.is_none());
    }

    #[test]
    fn non_stream_and_error_bodies() {
        let mut p = SseParser::default();
        p.feed_line(r#"{"error":{"message":"bad key"}}"#);
        p.feed_line("IDEFAST_HTTP:401");
        assert_eq!(p.finish().error.as_deref(), Some("bad key"));
        let mut p = SseParser::default();
        p.feed_line(r#"{"choices":[{"message":{"content":"ok","tool_calls":[{"function":{"name":"plugin_list","arguments":"{}"}}]}}]}"#);
        let t = p.finish();
        assert_eq!((t.content.as_str(), t.tool_calls[0].id.as_str()), ("ok", "call_0"));
        let mut p = SseParser::default();
        p.feed_line("IDEFAST_HTTP:502");
        assert_eq!(p.finish().error.as_deref(), Some("HTTP 502"));
    }
}
