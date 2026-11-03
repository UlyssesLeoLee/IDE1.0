//! AI 对话的与界面无关的部分: 配置与密钥存放、端点校验、请求构造、SSE 流解析、工具表、curl 传输。
//! 对标 Tauri 版 (editor.html 的 aiConfig / chatCallModel / chatInvokeTool), 但这里是流式的。
//! 约束:
//! - 只在用户点「发送」时联网; 端点只许 https (本机回环地址允许 http, 给本地模型用);
//! - 密钥只写进 `ai.json` (和 settings.json 同目录, 不进导出的配置), 不进日志、不进命令行 (走 curl 的 `-K -` 标准输入);
//! - 有连接 / 总时长 / 低速超时和响应字节上限; 取消 = 杀掉 curl 子进程。

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// 一次回复最多读这么多字节
pub const MAX_RESPONSE: usize = 4 * 1024 * 1024;
/// 工具往返的最大轮数 (和 Tauri 版同量级)
pub const MAX_ROUNDS: usize = 6;
/// 单个工具结果回给模型的字符上限
pub const MAX_TOOL_RESULT: usize = 20_000;
/// 下一轮对话保留的文字消息条数
pub const KEEP_MESSAGES: usize = 20;
const MAX_TIME_SECS: u32 = 300;

#[derive(Clone, Default, PartialEq)]
pub struct AiConfig {
    pub base_url: String,
    pub model: String,
    pub key: String,
    pub effort: String,
    /// 用户已同意把选中内容 / 当前章节发给端点 (一次同意, 之后不再问; 和 Tauri 版 CHAT_CONSENT_KEY 一致)
    pub consent: bool,
}

impl std::fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiConfig").field("base_url", &self.base_url).field("model", &self.model).field("key", &"<redacted>").finish()
    }
}

/// 供应商预设 (只填接口地址; 模型和密钥用户自己填)
pub const PRESETS: &[(&str, &str)] = &[
    ("OpenAI", "https://api.openai.com/v1"),
    ("DeepSeek", "https://api.deepseek.com/v1"),
    ("OpenRouter", "https://openrouter.ai/api/v1"),
    ("Ollama (local)", "http://127.0.0.1:11434/v1"),
];

pub fn config_path() -> Option<PathBuf> {
    crate::settings::path().map(|p| p.with_file_name("ai.json"))
}

impl AiConfig {
    pub fn default_cfg() -> Self {
        AiConfig { base_url: PRESETS[0].1.into(), ..Default::default() }
    }
    pub fn from_json(v: &Value) -> Self {
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let mut c = AiConfig { base_url: s("baseUrl"), model: s("model"), key: s("key"), effort: s("effort"), consent: v["consent"].as_bool().unwrap_or(false) };
        if c.base_url.is_empty() {
            c.base_url = PRESETS[0].1.into();
        }
        c
    }
    pub fn to_json(&self) -> Value {
        json!({ "baseUrl": self.base_url, "model": self.model, "key": self.key, "effort": self.effort, "consent": self.consent })
    }
    pub fn load() -> Self {
        config_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|v| Self::from_json(&v)).unwrap_or_else(Self::default_cfg)
    }
    /// 设置关闭 (IDE_FAST_SETTINGS=off) 时不落盘
    pub fn save(&self) {
        let Some(p) = config_path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&self.to_json()).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
    pub fn configured(&self) -> bool {
        !self.key.trim().is_empty() && !self.model.trim().is_empty() && endpoint(&self.base_url).is_ok()
    }
}

/// 把密钥从任何要显示 / 记录的文字里抹掉
pub fn redact(s: &str, key: &str) -> String {
    if key.len() >= 4 { s.replace(key, "***") } else { s.to_string() }
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// 校验接口地址, 返回 chat/completions 的完整 URL。https 限定 (回环地址可 http); 不许内嵌账号口令 / 换行 / UNC 样式
pub fn endpoint(base: &str) -> Result<String, String> {
    let b = base.trim().trim_end_matches('/');
    if b.contains(|c: char| c.is_control() || c.is_whitespace() || c == '\\') {
        return Err("接口地址含非法字符".into());
    }
    let (https, rest) = if let Some(r) = b.strip_prefix("https://") { (true, r) } else if let Some(r) = b.strip_prefix("http://") { (false, r) } else { return Err("接口地址必须以 https:// 开头".into()) };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("接口地址不能为空, 也不能内嵌账号口令".into());
    }
    let host = if authority.starts_with('[') { authority.split(']').next().map(|h| format!("{h}]")).unwrap_or_default() } else { authority.split(':').next().unwrap_or("").to_string() };
    if !https && !is_loopback(&host.to_ascii_lowercase()) {
        return Err("只允许 https 接口 (本机 localhost / 127.0.0.1 可用 http)".into());
    }
    if b.contains(['?', '#']) {
        return Err("接口地址不要带查询参数".into());
    }
    Ok(format!("{b}/chat/completions"))
}

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
            let Some(name) = t["name"].as_str().filter(|n| !n.is_empty() && n.len() <= 48 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) else { continue };
            let schema = if t["schema"].is_object() { t["schema"].clone() } else { json!({ "type": "object", "properties": {} }) };
            out.push(json!({ "type": "function", "function": { "name": format!("plugin_{}_{}", p.manifest.id, name), "description": format!("[插件 {}] {}", p.manifest.name, t["description"].as_str().unwrap_or("")), "parameters": schema } }));
        }
    }
    out
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

// ---------- 上下文 / 提示词 ----------

pub struct Context {
    pub label: &'static str,
    pub rows: (usize, usize),
    pub text: String,
}

/// 选中内容; 没选中且是 Markdown 就取光标所在章节 (标题到下一个同级或更高标题); 否则不带
pub fn context_of(lines: &[String], selection: Option<String>, cursor_row: usize, is_md: bool) -> Option<Context> {
    if let Some(s) = selection.filter(|s| !s.is_empty()) {
        return Some(Context { label: "选中内容", rows: (0, 0), text: s });
    }
    if !is_md || lines.is_empty() {
        return None;
    }
    let heading = |l: &str| -> usize {
        let h = l.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&h) && l[h..].starts_with(' ') { h } else { 0 }
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
    Some(Context { label: "当前章节", rows: (s, e - 1), text: lines[s..e].join("\n") })
}

pub fn system_prompt(doc_name: &str, total_rows: usize, ctx: Option<&Context>) -> String {
    let mut s = format!("你是 IDE1.0 编辑器里的助手。只能通过提供的工具行动; 要改文档只能调用 propose_patch 提出补丁 (行号从 0 起), 由用户审核后套用。当前文档: {doc_name}, 共 {total_rows} 行 (total_rows)。");
    if let Some(c) = ctx {
        s.push_str(&format!("\n用户授权发送的{}{}:\n{}", c.label, if c.rows == (0, 0) { String::new() } else { format!(" (第 {}–{} 行)", c.rows.0 + 1, c.rows.1 + 1) }, c.text));
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

// ---------- SSE 解析 ----------

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

// ---------- 传输 (curl) ----------

fn curl_path() -> PathBuf {
    std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join("curl.exe")).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("curl"))
}

/// curl 配置里引号字符串的转义 (只有 \ 和 " 需要)
fn cfg_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// 传给 curl 的命令行参数 (不含密钥和正文) —— 拆出来给测试
pub fn curl_args(url: &str) -> Vec<String> {
    let proto = if url.starts_with("https://") { "=https" } else { "=http" };
    ["-sS", "-N", "--proto", proto, "--max-redirs", "0", "--connect-timeout", "15", "--max-time", &MAX_TIME_SECS.to_string(), "--speed-limit", "1", "--speed-time", "60", "-K", "-", "-w", &format!("\\n{STATUS_MARK}%{{http_code}}\\n"), "--", url]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// 密钥和正文走标准输入的 curl 配置
pub fn curl_config(key: &str, body: &str) -> Result<String, String> {
    if key.contains(|c: char| c.is_control()) {
        return Err("密钥含控制字符".into());
    }
    Ok(format!("request = \"POST\"\nheader = {}\nheader = \"Content-Type: application/json\"\nheader = \"Accept: text/event-stream\"\nheader = \"User-Agent: IDE1.0-fast\"\ndata-raw = {}\n", cfg_quote(&format!("Authorization: Bearer {key}")), cfg_quote(body)))
}

pub enum Event {
    Text(String),
    Done(Turn),
}

/// 可取消的请求句柄: Drop 或 cancel 都会杀掉 curl
#[derive(Clone, Default)]
pub struct Handle {
    child: Arc<Mutex<Option<Child>>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl Handle {
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut g) = self.child.lock() {
            if let Some(c) = g.as_mut() {
                let _ = c.kill();
            }
        }
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// 起一条线程发请求, 流式把文字和最终结果送回 `tx` (每个事件后调 `wake` 让界面重绘)
pub fn spawn_request(cfg: &AiConfig, messages: Vec<Value>, tools: Vec<Value>, tx: Sender<Event>, wake: impl Fn() + Send + 'static) -> Handle {
    let h = Handle::default();
    let (hh, cfg) = (h.clone(), cfg.clone());
    let _ = std::thread::Builder::new().name("ai-chat".into()).spawn(move || {
        let turn = run_request(&cfg, &messages, &tools, &hh, &tx, &wake);
        let _ = tx.send(Event::Done(turn));
        wake();
    });
    h
}

fn fail(cfg: &AiConfig, msg: String) -> Turn {
    Turn { error: Some(redact(&msg, &cfg.key)), ..Default::default() }
}

fn run_request(cfg: &AiConfig, messages: &[Value], tools: &[Value], h: &Handle, tx: &Sender<Event>, wake: &dyn Fn()) -> Turn {
    let url = match endpoint(&cfg.base_url) {
        Ok(u) => u,
        Err(e) => return fail(cfg, e),
    };
    let body = request_body(cfg, messages, tools).to_string();
    let conf = match curl_config(&cfg.key, &body) {
        Ok(c) => c,
        Err(e) => return fail(cfg, e),
    };
    let mut cmd = Command::new(curl_path());
    cmd.args(curl_args(&url)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return fail(cfg, format!("无法运行 curl ({e}): 请确认系统自带 curl 可用")),
    };
    let (stdin, stdout, stderr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
    if let Ok(mut g) = h.child.lock() {
        *g = Some(child);
    }
    if h.cancelled() {
        h.cancel();
    }
    // 配置写完就关 stdin (写在线程里, 避免大正文和 curl 输出互相堵死)
    if let Some(mut si) = stdin {
        let _ = std::thread::Builder::new().name("ai-chat-in".into()).spawn(move || {
            let _ = si.write_all(conf.as_bytes());
        });
    }
    let err_thread = stderr.map(|mut e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::take(&mut e, 4096).read_to_string(&mut s);
            s
        })
    });
    let mut p = SseParser::default();
    let mut total = 0usize;
    let mut too_big = false;
    if let Some(out) = stdout {
        let mut rd = BufReader::new(out);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match rd.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    total += n;
                    if total > MAX_RESPONSE {
                        too_big = true;
                        h.cancel();
                        break;
                    }
                    if let Some(t) = p.feed_line(&String::from_utf8_lossy(&buf)) {
                        let _ = tx.send(Event::Text(t));
                        wake();
                    }
                }
            }
        }
    }
    let status = h.child.lock().ok().and_then(|mut g| g.take()).and_then(|mut c| c.wait().ok());
    let stderr_text = err_thread.and_then(|t| t.join().ok()).unwrap_or_default();
    let mut turn = p.finish();
    if h.cancelled() && !too_big {
        turn.error = Some("已取消".into());
    } else if too_big {
        turn.error = Some("响应超过大小上限, 已中止".into());
    } else if turn.error.is_none() && status.is_some_and(|s| !s.success()) {
        turn.error = Some(stderr_text.trim().to_string()).filter(|s| !s.is_empty()).or(Some("请求失败".into()));
    }
    if let Some(e) = turn.error.take() {
        turn.error = Some(redact(&e, &cfg.key));
    }
    turn
}

use std::io::Read as _;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn endpoint_rules() {
        assert_eq!(endpoint("https://api.openai.com/v1/").unwrap(), "https://api.openai.com/v1/chat/completions");
        assert!(endpoint("http://example.com/v1").is_err());
        assert!(endpoint("http://127.0.0.1:11434/v1").is_ok());
        assert!(endpoint("http://localhost/v1").is_ok());
        assert!(endpoint("ftp://x").is_err());
        assert!(endpoint("https://user:pw@host/v1").is_err());
        assert!(endpoint("https://host/v1\r\nX: y").is_err());
        assert!(endpoint("https://host\\share").is_err());
        assert!(endpoint("https://host/v1?a=b").is_err());
        assert!(endpoint("https://").is_err());
        assert!(endpoint("http://127.0.0.1.evil.com/v1").is_err());
    }

    #[test]
    fn debug_and_errors_never_show_the_key() {
        let c = AiConfig { key: "sk-secret-123".into(), ..AiConfig::default_cfg() };
        assert!(!format!("{c:?}").contains("sk-secret"));
        assert_eq!(redact("bad key sk-secret-123 here", &c.key), "bad key *** here");
        let a = curl_args("https://h/v1/chat/completions").join(" ");
        assert!(!a.contains("sk-secret") && a.contains("--proto =https"));
    }

    #[test]
    fn config_roundtrip_and_quote() {
        let c = AiConfig { base_url: "https://x/v1".into(), model: "m".into(), key: "k".into(), effort: "high".into(), consent: true };
        assert_eq!(AiConfig::from_json(&c.to_json()), c);
        assert_eq!(AiConfig::from_json(&json!({})).base_url, PRESETS[0].1);
        let cfgtxt = curl_config("k\"ey", "{\"a\":\"b\\n\"}").unwrap();
        assert!(cfgtxt.contains("Bearer k\\\"ey") && cfgtxt.contains("data-raw = \"{\\\"a\\\":\\\"b\\\\n\\\"}\""), "{cfgtxt}");
        assert!(curl_config("a\nb", "{}").is_err());
    }

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
    fn context_prefers_selection_then_markdown_section() {
        let l: Vec<String> = ["# A", "x", "## B", "y", "# C", "z"].iter().map(|s| s.to_string()).collect();
        assert_eq!(context_of(&l, Some("sel".into()), 0, true).unwrap().text, "sel");
        let c = context_of(&l, None, 3, true).unwrap();
        assert_eq!((c.text.as_str(), c.rows), ("## B\ny", (2, 3)));
        assert_eq!(context_of(&l, None, 1, true).unwrap().rows, (0, 3));
        assert!(context_of(&l, None, 3, false).is_none());
        assert!(context_of(&["no heading".to_string()], None, 0, true).is_none());
    }

    #[test]
    fn request_body_shape() {
        let c = AiConfig { model: "m".into(), effort: "low".into(), ..AiConfig::default_cfg() };
        let b = request_body(&c, &[json!({ "role": "user", "content": "hi" })], &tool_table(&[]));
        assert_eq!((b["stream"].clone(), b["reasoning_effort"].clone(), b["tool_choice"].clone()), (json!(true), json!("low"), json!("auto")));
        let b = request_body(&AiConfig::default_cfg(), &[], &[]);
        assert!(b.get("tools").is_none() && b.get("reasoning_effort").is_none());
        assert_eq!(trim_history(vec![Msg { role: "user".into(), content: "x".into() }; 30]).len(), KEEP_MESSAGES);
    }

    /// 本机模拟服务: 读完请求, 回 SSE (或慢速流, 测试取消)
    fn mock(reply: &'static str, slow: bool) -> (String, std::thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", l.local_addr().unwrap());
        let t = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap_or(0);
                req.extend_from_slice(&buf[..n]);
                let txt = String::from_utf8_lossy(&req).to_string();
                if let Some(i) = txt.find("\r\n\r\n") {
                    let cl = txt.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if req.len() >= i + 4 + cl {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(reply.as_bytes());
            let _ = s.flush();
            if slow {
                std::thread::sleep(std::time::Duration::from_secs(20));
            }
            String::from_utf8_lossy(&req).to_string()
        });
        (url, t)
    }

    fn run(cfg: &AiConfig) -> (String, Turn) {
        let (tx, rx) = std::sync::mpsc::channel();
        let _h = spawn_request(cfg, vec![json!({ "role": "user", "content": "你好" })], tool_table(&[]), tx, || {});
        let mut text = String::new();
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(30)).expect("超时") {
                Event::Text(t) => text.push_str(&t),
                Event::Done(t) => return (text, t),
            }
        }
    }

    const STREAM: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

    #[test]
    fn streams_from_a_local_mock_server_without_leaking_the_key() {
        let (url, srv) = mock(STREAM, false);
        let cfg = AiConfig { base_url: url, model: "m".into(), key: "sk-test-key-xyz".into(), ..Default::default() };
        let (text, turn) = run(&cfg);
        assert_eq!(text, "Hello");
        assert_eq!((turn.content.as_str(), turn.http_status, turn.error.clone()), ("Hello", Some(200), None), "{turn:?}");
        let req = srv.join().unwrap();
        assert!(req.contains("Authorization: Bearer sk-test-key-xyz"), "{req}");
        assert!(req.starts_with("POST /v1/chat/completions"), "{req}");
        assert!(req.contains("\"stream\":true") && req.contains("你好"));
    }

    #[test]
    fn error_status_is_reported_and_cancel_kills_the_request() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", l.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut b = [0u8; 8192];
            let _ = s.read(&mut b);
            let _ = s.write_all(b"HTTP/1.1 401 Unauthorized\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{\"error\":{\"message\":\"Incorrect API key sk-test-key-xyz\"}}");
        });
        let cfg = AiConfig { base_url: url, model: "m".into(), key: "sk-test-key-xyz".into(), ..Default::default() };
        let (_, t) = run(&cfg);
        let e = t.error.unwrap();
        assert!(e.contains("Incorrect API key") && !e.contains("sk-test-key-xyz"), "{e}");

        let (url, _srv) = mock("data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n", true);
        let cfg = AiConfig { base_url: url, model: "m".into(), key: "k12345".into(), ..Default::default() };
        let (tx, rx) = std::sync::mpsc::channel();
        let h = spawn_request(&cfg, vec![], vec![], tx, || {});
        assert!(matches!(rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap(), Event::Text(_)));
        let t0 = std::time::Instant::now();
        h.cancel();
        loop {
            if let Event::Done(t) = rx.recv_timeout(std::time::Duration::from_secs(10)).expect("取消后应结束") {
                assert_eq!(t.error.as_deref(), Some("已取消"));
                break;
            }
        }
        assert!(t0.elapsed().as_secs() < 8);
    }

    #[test]
    fn refuses_non_https_remote_before_any_network() {
        let cfg = AiConfig { base_url: "http://example.com/v1".into(), model: "m".into(), key: "k12345".into(), ..Default::default() };
        let (_, t) = run(&cfg);
        assert!(t.error.unwrap().contains("https"));
    }
}
