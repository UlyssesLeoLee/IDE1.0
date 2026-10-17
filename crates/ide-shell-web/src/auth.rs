//! Web shell 请求认证与传输层防护.
//!
//! # 背景 (P0 安全)
//!
//! `ide-shell-web` 绑定 127.0.0.1, 但「只绑 loopback」不是访问控制:
//! 用户浏览器里打开的**任何恶意网页**都可以直接 `fetch("http://127.0.0.1:8123/api/...")`.
//! 原实现里:
//!
//! - 响应一律带 `Access-Control-Allow-Origin: *` → 跨站 JS 能**读到**响应;
//! - `/api/terminal_create` 的 `shell` 参数取自请求 JSON, 直接交给 `Command::new`
//!   → 任意可执行文件;
//! - `/api/terminal_input` 往已开的 shell 写命令 → 等价于本机 RCE;
//! - 不校验 `Host` 头 → DNS rebinding 可绕过「同源」直觉.
//!
//! 本模块提供四道防线:
//!
//! 1. **随机 token**: 启动时生成, 必须通过 `X-IDE-Token` 头带上.
//!    自定义头会触发 CORS **预检**, 而预检响应我们拒绝通配 origin
//!    → 恶意网页的预检被拦下, 真实请求根本发不出去.
//! 2. **Origin 白名单**: 只放行本机同源. 不带 Origin 头的请求 (curl / 同源 GET
//!    导航) 不受此项约束; 但 `Origin: null` (沙箱 iframe / `data:` URL / 部分
//!    重定向会发它) **不可信**, 一律拒绝, 也绝不会被回显进 CORS 头.
//! 3. **Host 校验**: 必须是 `127.0.0.1:<port>` / `localhost:<port>`,
//!    防 DNS rebinding.
//! 4. **CORS 收紧**: 删掉 `Allow-Origin: *`, 改为按请求 Origin 回显.
//!
//! # 取 token 的方式
//!
//! - 环境变量 `IDE_SHELL_WEB_TOKEN`(UAT / CI 显式传入);
//! - 否则用 OS CSPRNG 生成随机 32 字节 hex, **打印到 stdout + 写入 `<exe>.token`**,
//!   因为服务没有 UI, 用户需要能拿到它 (Tauri 走 IPC, 不受此影响).

use std::fmt;

/// 单次请求通过全部检查后返回; 失败时给出应回给客户端的 HTTP 状态与说明.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthError {
    pub status: &'static str,
    pub reason: &'static str,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status, self.reason)
    }
}

/// 请求头 (已全部转小写 key).
pub type Headers = Vec<(String, String)>;

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// 服务端持有的认证配置.
#[derive(Debug, Clone)]
pub struct Auth {
    token: String,
    /// 允许的 Host 值 (小写, 含端口).
    allowed_hosts: Vec<String>,
    /// 是否允许无 token 的请求 — 只在显式 UAT 模式下开启.
    pub open: bool,
}

/// 无 token 时是否放行的环境变量开关.
pub const OPEN_ENV: &str = "IDE_SHELL_WEB_NO_AUTH";

impl Auth {
    /// 从环境构造. 返回 (auth, token_for_user).
    ///
    /// `IDE_SHELL_WEB_NO_AUTH=1` 时进入开放模式 (仅供 Playwright UAT 与
    /// 本地一次性调试); 生产/CI 建议始终用 token.
    pub fn from_env(port: u16) -> Self {
        let open = std::env::var(OPEN_ENV).map(|v| v == "1" || v == "true").unwrap_or(false);
        let token = match std::env::var("IDE_SHELL_WEB_TOKEN") {
            Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => random_token(),
        };
        let allowed_hosts = vec![
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            // 无端口形式 (HTTP/1.0 或显式 :80)
            "127.0.0.1".to_string(),
            "localhost".to_string(),
        ];
        Self { token, allowed_hosts, open }
    }

    /// 便于测试: 直接指定 token.
    pub fn with_token(token: &str, port: u16) -> Self {
        Self {
            token: token.to_string(),
            allowed_hosts: vec![
                format!("127.0.0.1:{port}"),
                format!("localhost:{port}"),
            ],
            open: false,
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// 校验一个请求. 通过返回 `Ok(())`.
    ///
    /// 顺序刻意如此: Host → Origin → token。Host 最便宜且能挡掉 rebinding。
    pub fn check(&self, method: &str, headers: &Headers) -> Result<(), AuthError> {
        // --- 3. Host 校验 (防 DNS rebinding) ---
        // HTTP/1.1 必须有 Host; 缺失按非法处理.
        match header(headers, "host") {
            Some(h) if self.host_allowed(h) => {}
            Some(_) => {
                return Err(AuthError { status: "403 Forbidden", reason: "invalid-host" });
            }
            None => {
                return Err(AuthError { status: "400 Bad Request", reason: "missing-host" });
            }
        }

        // --- 2. Origin 校验 ---
        // 只在「带 Origin」时检查: curl / 同源导航不带 Origin.
        // 注意 `Origin: null` 属于「带了 Origin」, 会在 origin_allowed 里被拒.
        if let Some(origin) = header(headers, "origin") {
            if !self.origin_allowed(origin) {
                return Err(AuthError { status: "403 Forbidden", reason: "origin-not-allowed" });
            }
        }

        // --- CORS 预检 ---
        // 自定义头 (X-IDE-Token) 会让浏览器发 OPTIONS 预检。
        // 我们对预检不放行任何通配 origin, 也不回 Access-Control-Allow-Headers,
        // 使恶意站点的真实请求被浏览器拦下。
        if method == "OPTIONS" {
            return Err(AuthError { status: "204 No Content", reason: "preflight-no-cors" });
        }

        // --- 1. token 校验 ---
        if self.open {
            return Ok(());
        }
        match header(headers, "x-ide-token") {
            Some(t) if ct_eq(t.as_bytes(), self.token.as_bytes()) => Ok(()),
            Some(_) => Err(AuthError { status: "403 Forbidden", reason: "bad-token" }),
            None => Err(AuthError { status: "401 Unauthorized", reason: "missing-token" }),
        }
    }

    fn host_allowed(&self, host: &str) -> bool {
        let h = host.trim().to_ascii_lowercase();
        self.allowed_hosts.iter().any(|a| *a == h)
    }

    /// 供响应构造使用: 该 Origin 是否可信 (决定是否回显 CORS 头).
    pub fn origin_is_allowed(&self, origin: &str) -> bool {
        self.origin_allowed(origin)
    }

    fn origin_allowed(&self, origin: &str) -> bool {
        let o = origin.trim();
        // `Origin: null` 是浏览器对「不透明源」发的: 沙箱 iframe、`data:` URL、
        // 部分跨源重定向、file:// 都会是它 —— 任何恶意页面都能构造出来,
        // 所以**不能**当作可信. 它没有 `://`, 下面的规则会自然拒绝它;
        // 这里显式写出, 防止以后有人为了「兼容本地工具」又把它放开.
        if o.eq_ignore_ascii_case("null") {
            return false;
        }
        // 形如 http://127.0.0.1:8123 —— 复用 host 规则 (host:port 部分)
        if let Some(rest) = o.split_once("://") {
            let hostport = rest.1.split('/').next().unwrap_or(rest.1);
            return self.host_allowed(hostport);
        }
        false
    }
}

/// 常量时间字节串比较, 用于 token 校验.
///
/// `==` 在第一个不同字节处提前返回, 本机攻击者可以靠响应耗时逐字节试出 token。
/// 这里对 `max(len)` 个字节全部做 XOR 累积, 耗时不依赖「前缀对了几位」;
/// 长度差异也折进累积值, 因此不存在提前返回分支。
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= usize::from(x ^ y);
    }
    // black_box: 不让优化器把累积循环改写回提前退出的比较.
    std::hint::black_box(diff) == 0
}

/// 生成 32 字节随机 token (64 个 hex 字符).
///
/// 熵源是 OS CSPRNG (`getrandom`: Windows `ProcessPrng` / Linux `getrandom(2)` /
/// macOS `getentropy`)。之前用时间 + pid + 栈地址做种子, 本机攻击者能猜到
/// 绝大部分输入, 不是合格的密钥。
///
/// # Panics
///
/// OS 随机源不可用时直接 panic —— token 是唯一的访问控制凭据,
/// 退化成可预测值比启动失败更危险, 所以**不做降级**。
pub fn random_token() -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf)
        .expect("OS CSPRNG 不可用, 拒绝用可预测的 token 启动 ide-shell-web");
    let mut s = String::with_capacity(buf.len() * 2);
    for b in buf {
        s.push(HEX[usize::from(b >> 4)] as char);
        s.push(HEX[usize::from(b & 0x0f)] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> Headers {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn valid_host_and_token_pass() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "127.0.0.1:8123"), ("x-ide-token", "secret")]);
        assert!(a.check("POST", &hdrs).is_ok());
    }

    #[test]
    fn localhost_equivalent_to_127_0_0_1() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "localhost:8123"), ("x-ide-token", "secret")]);
        assert!(a.check("GET", &hdrs).is_ok());
    }

    #[test]
    fn missing_token_rejected() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "127.0.0.1:8123")]);
        let e = a.check("POST", &hdrs).unwrap_err();
        assert_eq!(e.status, "401 Unauthorized");
        assert_eq!(e.reason, "missing-token");
    }

    #[test]
    fn wrong_token_rejected() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "127.0.0.1:8123"), ("x-ide-token", "guess")]);
        let e = a.check("POST", &hdrs).unwrap_err();
        assert_eq!(e.reason, "bad-token");
    }

    #[test]
    fn foreign_host_for_rebinding_rejected() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "evil.example.com"), ("x-ide-token", "secret")]);
        let e = a.check("POST", &hdrs).unwrap_err();
        assert_eq!(e.reason, "invalid-host");
    }

    #[test]
    fn missing_host_rejected() {
        let a = Auth::with_token("secret", 8123);
        let e = a.check("POST", &h(&[("x-ide-token", "secret")])).unwrap_err();
        assert_eq!(e.reason, "missing-host");
    }

    #[test]
    fn cross_origin_rejected() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[
            ("host", "127.0.0.1:8123"),
            ("origin", "https://evil.example.com"),
            ("x-ide-token", "secret"),
        ]);
        let e = a.check("POST", &hdrs).unwrap_err();
        assert_eq!(e.reason, "origin-not-allowed");
    }

    #[test]
    fn same_origin_allowed() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[
            ("host", "127.0.0.1:8123"),
            ("origin", "http://127.0.0.1:8123"),
            ("x-ide-token", "secret"),
        ]);
        assert!(a.check("POST", &hdrs).is_ok());
    }

    #[test]
    fn cross_origin_preflight_not_granted() {
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "127.0.0.1:8123"), ("origin", "https://evil.com")]);
        let e = a.check("OPTIONS", &hdrs).unwrap_err();
        assert_eq!(e.reason, "origin-not-allowed");
    }

    #[test]
    fn open_mode_skips_token() {
        let mut a = Auth::with_token("secret", 8123);
        a.open = true;
        let hdrs = h(&[("host", "127.0.0.1:8123")]);
        assert!(a.check("POST", &hdrs).is_ok());
        // 但 Host 校验依然生效
        let bad = h(&[("host", "evil.com")]);
        assert!(a.check("POST", &bad).is_err());
    }

    #[test]
    fn random_token_unique_and_long_enough() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b, "token 不应可预测");
        assert!(a.len() >= 32, "token 太短: {}", a.len());
    }

    #[test]
    fn random_token_is_64_lowercase_hex() {
        let t = random_token();
        assert_eq!(t.len(), 64, "32 字节 → 64 个 hex 字符");
        assert!(
            t.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "非小写 hex: {t}"
        );
        // 一批 token 不应有重复 (64 个字符 = 256 bit, 碰撞即说明熵源坏了)
        let mut seen = std::collections::HashSet::new();
        for _ in 0..64 {
            assert!(seen.insert(random_token()), "token 重复");
        }
    }

    // ---- Origin: null ----

    #[test]
    fn null_origin_rejected() {
        let a = Auth::with_token("secret", 8123);
        // 即使 Host、token 都对, 带 `Origin: null` 也必须被拒.
        let hdrs = h(&[
            ("host", "127.0.0.1:8123"),
            ("origin", "null"),
            ("x-ide-token", "secret"),
        ]);
        let e = a.check("POST", &hdrs).unwrap_err();
        assert_eq!(e.status, "403 Forbidden");
        assert_eq!(e.reason, "origin-not-allowed");
    }

    #[test]
    fn null_origin_never_trusted_or_echoed() {
        let a = Auth::with_token("secret", 8123);
        // lib.rs 的 echo_origin 靠 origin_is_allowed 过滤 —— 它为 false 就不会回显.
        for o in ["null", "NULL", "Null", " null ", ""] {
            assert!(!a.origin_is_allowed(o), "不应信任 Origin {o:?}");
        }
    }

    #[test]
    fn null_origin_rejected_in_open_mode_too() {
        // UAT 开放模式只跳过 token, Origin 校验照常.
        let mut a = Auth::with_token("secret", 8123);
        a.open = true;
        let hdrs = h(&[("host", "127.0.0.1:8123"), ("origin", "null")]);
        assert_eq!(a.check("GET", &hdrs).unwrap_err().reason, "origin-not-allowed");
    }

    #[test]
    fn no_origin_header_still_allowed() {
        // curl / 同源 GET 导航不带 Origin, 必须继续可用.
        let a = Auth::with_token("secret", 8123);
        let hdrs = h(&[("host", "127.0.0.1:8123"), ("x-ide-token", "secret")]);
        assert!(a.check("GET", &hdrs).is_ok());
        // 开放模式 (Playwright UAT) 下无 Origin、无 token 也能过.
        let mut open = Auth::with_token("secret", 8123);
        open.open = true;
        assert!(open.check("GET", &h(&[("host", "127.0.0.1:8123")])).is_ok());
    }

    #[test]
    fn loopback_origins_still_allowed() {
        let a = Auth::with_token("secret", 8123);
        assert!(a.origin_is_allowed("http://127.0.0.1:8123"));
        assert!(a.origin_is_allowed("http://localhost:8123"));
        // 端口不对 / 异主机 / 其它不透明源 都不行
        assert!(!a.origin_is_allowed("http://127.0.0.1:9999"));
        assert!(!a.origin_is_allowed("http://127.0.0.1:8123@evil.com"));
        assert!(!a.origin_is_allowed("https://evil.example.com"));
        assert!(!a.origin_is_allowed("file://"));
    }

    // ---- token 比较 ----

    #[test]
    fn short_or_prefix_token_rejected() {
        let a = Auth::with_token("secret", 8123);
        for bad in ["", "s", "secre", "secret!", "secret\0", "SECRET", "xecret"] {
            let hdrs = h(&[("host", "127.0.0.1:8123"), ("x-ide-token", bad)]);
            let e = a.check("POST", &hdrs).unwrap_err();
            assert_eq!(e.reason, "bad-token", "token {bad:?} 不应通过");
        }
    }

    #[test]
    fn generated_token_authenticates_and_one_char_off_does_not() {
        let tok = random_token();
        let a = Auth::with_token(&tok, 8123);
        let ok = h(&[("host", "127.0.0.1:8123"), ("x-ide-token", &tok)]);
        assert!(a.check("POST", &ok).is_ok());

        // 改最后一位 (比较必须看完整长度, 不能只比前缀)
        let mut off = tok.clone();
        let last = off.pop().unwrap();
        off.push(if last == '0' { '1' } else { '0' });
        let bad = h(&[("host", "127.0.0.1:8123"), ("x-ide-token", &off)]);
        assert_eq!(a.check("POST", &bad).unwrap_err().reason, "bad-token");
    }

    #[test]
    fn ct_eq_semantics() {
        assert!(ct_eq(b"", b""));
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(!ct_eq(b"abcd", b"abc"));
        assert!(!ct_eq(b"", b"a"));
        // 长度不同但较短者是较长者前缀 / 补零 —— 不能被零填充骗过
        assert!(!ct_eq(b"abc", b"abc\0"));
        assert!(!ct_eq(b"abc\0", b"abc"));
    }
}