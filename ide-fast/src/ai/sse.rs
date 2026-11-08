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
            if !line.trim().is_empty()
                && !line.starts_with(':')
                && !line.starts_with("event:")
                && !line.starts_with("id:")
                && !line.starts_with("retry:")
                && self.raw.len() < MAX_RESPONSE
            {
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
        if let Some(e) = extract_error_detail(&v) {
            self.turn.error = Some(e);
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
            let trimmed = self.raw.trim();
            match serde_json::from_str::<Value>(trimmed) {
                Ok(v) => {
                    if let Some(e) = extract_error_detail(&v) {
                        self.turn.error = Some(e);
                    } else if let Some(m) = v["choices"][0]["message"].as_object() {
                        self.turn.content = m
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        for tc in m
                            .get("tool_calls")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                        {
                            self.turn.tool_calls.push(ToolCall {
                                id: tc["id"].as_str().unwrap_or("").into(),
                                name: tc["function"]["name"].as_str().unwrap_or("").into(),
                                args: tc["function"]["arguments"].as_str().unwrap_or("{}").into(),
                            });
                        }
                        self.turn.done = true;
                    }
                }
                Err(_) => {
                    self.turn.error = Some(trimmed.chars().take(300).collect());
                }
            }
        }
        // 如果有非 2xx/3xx 的 HTTP 状态码，或者已经有 error 详情，进行格式化规整
        // 注意：status 为 Some(0) 时先不在 finish 里格式化，留给 transport.rs 结合 stderr 统一格式化
        if let Some(st) = self.turn.http_status.filter(|s| !(200..300).contains(s) && *s != 0) {
            self.turn.error = Some(format_http_error(
                Some(st),
                self.turn.error.as_deref(),
                None,
            ));
        } else if let Some(err) = self.turn.error.take() {
            self.turn.error = Some(format_http_error(
                self.turn.http_status,
                Some(&err),
                None,
            ));
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

/// 从服务端响应 JSON 中提取错误说明 (兼容 OpenAI、Claude、DeepSeek、OneAPI 等主流格式)
pub fn extract_error_detail(v: &Value) -> Option<String> {
    if let Some(err_obj) = v.get("error") {
        if let Some(msg) = err_obj.get("message").and_then(Value::as_str) {
            let code = err_obj.get("code").and_then(Value::as_str);
            let typ = err_obj.get("type").and_then(Value::as_str);
            let mut s = msg.trim().to_string();
            if let Some(c) = code.filter(|c| !c.is_empty() && !s.contains(*c)) {
                s.push_str(&format!(" (code: {c})"));
            } else if let Some(t) = typ.filter(|t| !t.is_empty() && !s.contains(*t)) {
                s.push_str(&format!(" (type: {t})"));
            }
            return Some(s);
        }
        if let Some(s) = err_obj.as_str() {
            return Some(s.trim().to_string());
        }
        if let Some(msg) = err_obj.get("code").and_then(Value::as_str) {
            return Some(msg.trim().to_string());
        }
        return Some(err_obj.to_string());
    }
    if let Some(msg) = v.get("message").and_then(Value::as_str) {
        return Some(msg.trim().to_string());
    }
    if let Some(detail) = v.get("detail").and_then(Value::as_str) {
        return Some(detail.trim().to_string());
    }
    if let Some(msg) = v.get("msg").and_then(Value::as_str) {
        return Some(msg.trim().to_string());
    }
    None
}

/// 格式化 HTTP 错误与网络诊断，特别是将额度耗尽、欠费、超额、连接中断转化为明确友好的中文说明，绝不裸露无意义的 "HTTP 0"
pub fn format_http_error(
    status: Option<u16>,
    detail: Option<&str>,
    stderr: Option<&str>,
) -> String {
    let clean_detail = detail
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let clean_err = stderr
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // 检查是否命中额度耗尽 / 账户欠费 / 限额
    let is_quota_issue = {
        let text = format!(
            "{} {}",
            clean_detail.as_deref().unwrap_or(""),
            clean_err.as_deref().unwrap_or("")
        )
        .to_ascii_lowercase();
        text.contains("insufficient_quota")
            || text.contains("quota_exceeded")
            || text.contains("exceeded your current quota")
            || text.contains("out of credits")
            || text.contains("insufficient credit")
            || text.contains("balance insufficient")
            || text.contains("billing")
            || text.contains("额度不足")
            || text.contains("余额不足")
            || text.contains("欠费")
            || text.contains("额度已耗尽")
            || text.contains("无可用额度")
            || status == Some(402)
    };

    if is_quota_issue {
        let msg = clean_detail
            .or(clean_err)
            .unwrap_or_else(|| "账户额度已耗尽或余额不足".to_string());
        return format!(
            "【API 额度耗尽 / 账户欠费】服务商返回: {msg}。请前往 API 提供商后台检查账单与额度充值，或在设置中更换有效的 API Key。"
        );
    }

    // 状态码 0 或无状态码: 网络连接异常 / curl 退出
    if status == Some(0) || status.is_none() {
        let err_hint = if let Some(e) = clean_err {
            let el = e.to_ascii_lowercase();
            if el.contains("could not resolve host") {
                format!("{e} (域名无法解析，请检查 Base URL 是否正确或网络 DNS)")
            } else if el.contains("connection refused") || el.contains("failed to connect") {
                format!("{e} (服务连接被拒绝，目标地址可能未启动或端口不通)")
            } else if el.contains("timed out") || el.contains("timeout") {
                format!("{e} (网络连接超时，请检查网络通畅度或系统代理)")
            } else if el.contains("ssl") || el.contains("certificate") || el.contains("schannel") {
                format!("{e} (SSL 证书握手失败，请检查 HTTPS 证书或代理配置)")
            } else {
                e
            }
        } else if let Some(d) = clean_detail {
            d
        } else {
            "网络连接失败或超时，未能与 AI 服务器建立连接".to_string()
        };
        return format!(
            "【网络连接失败】{err_hint}。请检查 Base URL 设置、网络连通性或代理配置。"
        );
    }

    let st = status.unwrap_or(0);
    match st {
        401 | 403 => {
            let msg = clean_detail
                .or(clean_err)
                .unwrap_or_else(|| "身份验证失败，请检查 API Key".to_string());
            format!("【API 鉴权失败 (HTTP {st})】服务商返回: {msg}。请检查输入的 API Key 是否有效。")
        }
        404 => {
            let msg = clean_detail
                .or(clean_err)
                .unwrap_or_else(|| "请求的接口路径或模型未找到".to_string());
            format!("【模型或接口不存在 (HTTP 404)】服务商返回: {msg}。请检查配置中的 Model 名称与 Base URL。")
        }
        429 => {
            let msg = clean_detail
                .or(clean_err)
                .unwrap_or_else(|| "请求速率已达上限".to_string());
            format!("【API 请求过于频繁 (HTTP 429)】服务商返回: {msg}。请稍等片刻后重试。")
        }
        500..=599 => {
            let msg = clean_detail
                .or(clean_err)
                .unwrap_or_else(|| "服务器内部错误".to_string());
            format!("【AI 服务端内部异常 (HTTP {st})】服务商返回: {msg}。服务商暂时不可用，请稍后重试。")
        }
        _ => {
            if st == 0 {
                return "【网络连接失败】网络连接失败或超时，未能与 AI 服务器建立连接。请检查 Base URL 设置或网络连通性。".to_string();
            }
            if let Some(msg) = clean_detail.or(clean_err) {
                let msg_clean = if msg.trim() == "HTTP 0" || msg.trim() == "0" || msg.trim() == "000" {
                    "网络连接中断，未收到服务商有效响应".to_string()
                } else {
                    msg
                };
                format!("HTTP {st}: {msg_clean}")
            } else {
                format!("HTTP {st}")
            }
        }
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
        assert_eq!(
            t.tool_calls,
            vec![ToolCall {
                id: "c1".into(),
                name: "md_outline".into(),
                args: "{\"text\":\"# a\"}".into()
            }]
        );
        assert!(t.done && t.error.is_none());
    }

    #[test]
    fn non_stream_and_error_bodies() {
        let mut p = SseParser::default();
        p.feed_line(r#"{"error":{"message":"bad key"}}"#);
        p.feed_line("IDEFAST_HTTP:401");
        let err = p.finish().error.unwrap();
        assert!(err.contains("【API 鉴权失败 (HTTP 401)】"));
        assert!(err.contains("bad key"));

        let mut p = SseParser::default();
        p.feed_line(r#"{"choices":[{"message":{"content":"ok","tool_calls":[{"function":{"name":"plugin_list","arguments":"{}"}}]}}]}"#);
        let t = p.finish();
        assert_eq!(
            (t.content.as_str(), t.tool_calls[0].id.as_str()),
            ("ok", "call_0")
        );
        let mut p = SseParser::default();
        p.feed_line("IDEFAST_HTTP:502");
        let err = p.finish().error.unwrap();
        assert!(err.contains("【AI 服务端内部异常 (HTTP 502)】"));
    }

    #[test]
    fn quota_insufficient_and_http_zero_diagnostics() {
        // OpenAI / DeepSeek 额度耗尽格式 (HTTP 429)
        let mut p = SseParser::default();
        p.feed_line(r#"{"error":{"message":"You exceeded your current quota, please check your plan and billing details.","type":"insufficient_quota","code":"insufficient_quota"}}"#);
        p.feed_line("IDEFAST_HTTP:429");
        let err = p.finish().error.unwrap();
        assert!(err.contains("【API 额度耗尽 / 账户欠费】"));
        assert!(err.contains("insufficient_quota"));

        // 402 Payment Required 余额不足
        let mut p = SseParser::default();
        p.feed_line(r#"{"message":"账户余额不足，请充值后使用"}"#);
        p.feed_line("IDEFAST_HTTP:402");
        let err = p.finish().error.unwrap();
        assert!(err.contains("【API 额度耗尽 / 账户欠费】"));
        assert!(err.contains("账户余额不足"));

        // 状态码 0 的网络诊断格式化
        let net_err = format_http_error(
            Some(0),
            None,
            Some("curl: (6) Could not resolve host: api.deepseek.com"),
        );
        assert!(net_err.contains("【网络连接失败】"));
        assert!(net_err.contains("域名无法解析"));
        assert!(!net_err.contains("HTTP 0"));

        // 无任何输出且状态码为 0 的兜底提示
        let fallback_err = format_http_error(Some(0), None, None);
        assert!(fallback_err.contains("【网络连接失败】"));
        assert!(fallback_err.contains("网络连接失败或超时"));
        assert!(!fallback_err.contains("HTTP 0"));
    }
}
