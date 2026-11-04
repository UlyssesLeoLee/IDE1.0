//! AI 配置: 接口地址校验、预设、密钥存放 (`ai.json`) 与脱敏。

use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Default, PartialEq)]
pub struct AiConfig {
    pub base_url: String,
    pub model: String,
    pub key: String,
    pub effort: String,
    /// 推理强度可选的档位 (从供应商获取到的, 或自己手填的); 空 = 用通用默认 `efforts::DEFAULT`
    pub efforts: Vec<String>,
    /// 用户已同意把选中内容 / 当前章节发给端点 (一次同意, 之后不再问; 和 Tauri 版 CHAT_CONSENT_KEY 一致)
    pub consent: bool,
}

impl std::fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiConfig")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("key", &"<redacted>")
            .finish()
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
        AiConfig {
            base_url: PRESETS[0].1.into(),
            ..Default::default()
        }
    }
    pub fn from_json(v: &Value) -> Self {
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let mut c = AiConfig {
            base_url: s("baseUrl"),
            model: s("model"),
            key: s("key"),
            effort: s("effort"),
            efforts: super::efforts::clean_list(
                v["efforts"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            consent: v["consent"].as_bool().unwrap_or(false),
        };
        if c.base_url.is_empty() {
            c.base_url = PRESETS[0].1.into();
        }
        c
    }
    pub fn to_json(&self) -> Value {
        json!({ "baseUrl": self.base_url, "model": self.model, "key": self.key, "effort": self.effort, "efforts": self.efforts, "consent": self.consent })
    }
    pub fn load() -> Self {
        config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| Self::from_json(&v))
            .unwrap_or_else(Self::default_cfg)
    }
    /// 设置关闭 (IDE_FAST_SETTINGS=off) 时不落盘
    pub fn save(&self) -> Result<(), String> {
        let Some(p) = config_path() else {
            return Ok(());
        };
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("无法创建配置目录: {e}"))?;
        }
        let bytes = serde_json::to_vec_pretty(&self.to_json()).map_err(|e| e.to_string())?;
        crate::fsx::atomic_write(&p, &bytes, true).map_err(|e| format!("保存 AI 配置失败: {e}"))
    }
    pub fn configured(&self) -> bool {
        !self.key.trim().is_empty()
            && !self.model.trim().is_empty()
            && endpoint(&self.base_url).is_ok()
    }
}

/// 把密钥从任何要显示 / 记录的文字里抹掉
pub fn redact(s: &str, key: &str) -> String {
    if key.len() >= 4 {
        s.replace(key, "***")
    } else {
        s.to_string()
    }
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// 校验接口地址, 返回 chat/completions 的完整 URL
pub fn endpoint(base: &str) -> Result<String, String> {
    Ok(format!("{}/chat/completions", checked_base(base)?))
}

/// 校验接口地址, 返回去掉尾部斜杠的地址。https 限定 (回环地址可 http); 不许内嵌账号口令 / 换行 / UNC 样式
pub fn checked_base(base: &str) -> Result<String, String> {
    let b = base.trim().trim_end_matches('/');
    if b.contains(|c: char| c.is_control() || c.is_whitespace() || c == '\\') {
        return Err("接口地址含非法字符".into());
    }
    let (https, rest) = if let Some(r) = b.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = b.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("接口地址必须以 https:// 开头".into());
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("接口地址不能为空, 也不能内嵌账号口令".into());
    }
    let host = if authority.starts_with('[') {
        authority
            .split(']')
            .next()
            .map(|h| format!("{h}]"))
            .unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or("").to_string()
    };
    if !https && !is_loopback(&host.to_ascii_lowercase()) {
        return Err("只允许 https 接口 (本机 localhost / 127.0.0.1 可用 http)".into());
    }
    if b.contains(['?', '#']) {
        return Err("接口地址不要带查询参数".into());
    }
    Ok(b.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_rules() {
        assert_eq!(
            endpoint("https://api.openai.com/v1/").unwrap(),
            "https://api.openai.com/v1/chat/completions"
        );
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
        let c = AiConfig {
            key: "sk-secret-123".into(),
            ..AiConfig::default_cfg()
        };
        assert!(!format!("{c:?}").contains("sk-secret"));
        assert_eq!(
            redact("bad key sk-secret-123 here", &c.key),
            "bad key *** here"
        );
    }
}
