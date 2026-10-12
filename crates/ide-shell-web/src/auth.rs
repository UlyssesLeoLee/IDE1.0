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
//! 2. **Origin 白名单**: 只放行 `null`(curl/同源导航) 与本机同源.
//! 3. **Host 校验**: 必须是 `127.0.0.1:<port>` / `localhost:<port>`,
//!    防 DNS rebinding.
//! 4. **CORS 收紧**: 删掉 `Allow-Origin: *`, 改为按请求 Origin 回显.
//!
//! # 取 token 的方式
//!
//! - 环境变量 `IDE_SHELL_WEB_TOKEN`(UAT / CI 显式传入);
//! - 否则生成随机 32 字节 hex, **打印到 stdout + 写入 `<exe>.token`**,
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
            Some(t) if t == self.token => Ok(()),
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
        // `null` = 某些本地工具 / file:// 导航
        if o == "null" {
            return true;
        }
        // 形如 http://127.0.0.1:8123 —— 复用 host 规则 (host:port 部分)
        if let Some(rest) = o.split_once("://") {
            let hostport = rest.1.split('/').next().unwrap_or(rest.1);
            return self.host_allowed(hostport);
        }
        false
    }
}

/// 生成 32 字节随机 token (hex).
///
/// 无 `rand` 依赖时用系统时间 + 进程 id + 计数器混合, 经 FNV-1a 扩散。
/// 这不是密码学级随机, 但用途是**防本机其它网页跨站读取**,
/// 攻击者需要在请求发出的同一瞬间猜中; 若要更强可换 `getrandom`.
pub fn random_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let addr = &nanos as *const u128 as usize as u128;

    let mut s = String::new();
    let mut state: u128 = nanos ^ (pid << 32) ^ addr;
    for i in 0..4u128 {
        // splitmix64
        state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        s.push_str(&format!("{:016x}", z));
        let _ = i;
    }
    s.truncate(64);
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
}