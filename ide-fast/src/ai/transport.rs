//! 传输 (curl): 参数 / 配置构造、流式读取、取消。密钥与正文走 curl 的 `-K -` 标准输入, 不进命令行。

use super::config::{endpoint, redact, AiConfig};
use super::prompt::request_body;
use super::sse::{SseParser, Turn, MAX_RESPONSE, STATUS_MARK};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

const MAX_TIME_SECS: u32 = 300;

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

/// 按行读流。行尾 `\n` 不会出现在多字节 UTF-8 序列中间, 所以跨数据块被切开的字符会先攒齐再解码;
/// 超过 `limit` 字节返回 true 并停止; 只有真正非法的字节才会被替换。
fn pump_lines(mut rd: impl BufRead, limit: usize, mut on_line: impl FnMut(&str)) -> bool {
    let (mut total, mut buf) = (0usize, Vec::new());
    loop {
        buf.clear();
        match rd.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return false,
            Ok(n) => {
                total += n;
                if total > limit {
                    return true;
                }
                match std::str::from_utf8(&buf) {
                    Ok(s) => on_line(s),
                    Err(_) => on_line(&String::from_utf8_lossy(&buf)),
                }
            }
        }
    }
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
    let mut cmd = crate::curlx::curl_command();
    cmd.args(curl_args(&url)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
    let mut too_big = false;
    if let Some(out) = stdout {
        too_big = pump_lines(BufReader::new(out), MAX_RESPONSE, |line| {
            if let Some(t) = p.feed_line(line) {
                let _ = tx.send(Event::Text(t));
                wake();
            }
        });
        if too_big {
            h.cancel();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::*;
    use serde_json::json;
    use std::net::TcpListener;

    /// 每次只吐一个字节的读取器, 模拟多字节字符被网络切开
    struct OneByte<'a>(&'a [u8]);
    impl Read for OneByte<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let n = self.0.len().min(1).min(out.len());
            out[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn multibyte_split_across_chunks_is_kept_intact() {
        let data = "data: 你好😀\nsecond 行\n".as_bytes();
        let mut lines = vec![];
        let big = pump_lines(BufReader::with_capacity(1, OneByte(data)), 1000, |l| lines.push(l.to_string()));
        assert!(!big);
        assert_eq!(lines, vec!["data: 你好😀\n", "second 行\n"]);
        assert!(pump_lines(BufReader::new(&b"aaaa\nbbbb\n"[..]), 6, |_| {}), "超限要停");
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
