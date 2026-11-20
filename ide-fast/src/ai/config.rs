//! AI 配置: 接口地址校验、预设、密钥存放 (`ai.json`) 与脱敏。

use serde_json::{json, Value};
use std::path::PathBuf;

/// 用户已配置并保存的单个模型档案
#[derive(Clone, Default, PartialEq)]
pub struct ModelProfile {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub key: String,
    pub effort: String,
    pub efforts: Vec<String>,
}

impl std::fmt::Debug for ModelProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelProfile")
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("key", &"<redacted>")
            .finish()
    }
}

impl ModelProfile {
    pub fn display_label(&self) -> &str {
        let n = self.name.trim();
        if n.is_empty() {
            self.model.trim()
        } else {
            n
        }
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
        let model = s("model");
        if model.is_empty() {
            return None;
        }
        let mut base_url = s("baseUrl");
        if base_url.is_empty() {
            base_url = PRESETS[0].1.into();
        }
        let efforts = super::efforts::clean_list(
            v["efforts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        );
        Some(ModelProfile {
            name: s("name"),
            base_url,
            model,
            key: s("key"),
            effort: s("effort"),
            efforts,
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "baseUrl": self.base_url,
            "model": self.model,
            "key": self.key,
            "effort": self.effort,
            "efforts": self.efforts,
        })
    }
}

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
    /// 用户主动配置并保存的模型列表 (仅这里及已配置的当前模型会出现在切换清单)
    pub saved_models: Vec<ModelProfile>,
}

impl std::fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiConfig")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("key", &"<redacted>")
            .field("saved_models_count", &self.saved_models.len())
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
            consent: true,
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
            consent: v["consent"].as_bool().unwrap_or(true),
            saved_models: Vec::new(),
        };
        if c.base_url.is_empty() {
            c.base_url = PRESETS[0].1.into();
        }
        if let Some(arr) = v.get("savedModels").and_then(Value::as_array) {
            let mut dedup: Vec<ModelProfile> = Vec::with_capacity(arr.len());
            for item in arr {
                if let Some(p) = ModelProfile::from_json(item) {
                    if !dedup
                        .iter()
                        .any(|e| e.model == p.model && e.base_url == p.base_url)
                    {
                        dedup.push(p);
                    }
                }
            }
            c.saved_models = dedup;
        } else if !c.model.trim().is_empty() {
            // 兼容旧版 ai.json：自动将已配置过的单模型迁移至 saved_models
            c.upsert_current_model(None);
        }
        c
    }
    pub fn to_json(&self) -> Value {
        json!({
            "baseUrl": self.base_url,
            "model": self.model,
            "key": self.key,
            "effort": self.effort,
            "efforts": self.efforts,
            "consent": self.consent,
            "savedModels": self.saved_models.iter().map(ModelProfile::to_json).collect::<Vec<_>>(),
        })
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

    /// 仅返回用户主动配置/保存过的模型；未配置时恒为空切片，绝不暴露硬编码默认模型
    pub fn configured_models(&self) -> Vec<ModelProfile> {
        let mut out: Vec<ModelProfile> = self
            .saved_models
            .iter()
            .filter(|p| !p.model.trim().is_empty())
            .cloned()
            .collect();
        let cur = self.model.trim();
        if !cur.is_empty() && !out.iter().any(|p| p.model == cur) {
            out.push(ModelProfile {
                name: String::new(),
                base_url: self.base_url.trim().to_string(),
                model: cur.to_string(),
                key: self.key.trim().to_string(),
                effort: self.effort.trim().to_string(),
                efforts: self.efforts.clone(),
            });
        }
        out
    }

    /// 将当前填写的模型与接口参数写入/更新到 `saved_models`
    pub fn upsert_current_model(&mut self, custom_name: Option<&str>) {
        self.base_url = self.base_url.trim().to_string();
        if self.base_url.is_empty() {
            self.base_url = PRESETS[0].1.into();
        }
        self.model = self.model.trim().to_string();
        self.key = self.key.trim().to_string();
        self.effort = self.effort.trim().to_string();
        if self.model.is_empty() {
            return;
        }
        let trimmed_name = custom_name.map(str::trim).unwrap_or("");
        if let Some(existing) = self.saved_models.iter_mut().find(|p| p.model == self.model) {
            if !trimmed_name.is_empty() {
                existing.name = trimmed_name.to_string();
            }
            existing.base_url = self.base_url.clone();
            existing.key = self.key.clone();
            existing.effort = self.effort.clone();
            existing.efforts = self.efforts.clone();
        } else {
            self.saved_models.push(ModelProfile {
                name: trimmed_name.to_string(),
                base_url: self.base_url.clone(),
                model: self.model.clone(),
                key: self.key.clone(),
                effort: self.effort.clone(),
                efforts: self.efforts.clone(),
            });
        }
    }

    /// 从用户已保存档案切换为当前激活模型
    pub fn select_saved_model(&mut self, model: &str) -> bool {
        let Some(p) = self.saved_models.iter().find(|p| p.model == model).cloned() else {
            return false;
        };
        self.base_url = p.base_url;
        self.model = p.model;
        self.key = p.key;
        self.effort = p.effort;
        self.efforts = p.efforts;
        true
    }

    /// 移除指定已配置模型；若正好是当前模型则回落至第一个剩余模型或清空
    pub fn remove_saved_model(&mut self, model: &str) -> bool {
        let before = self.saved_models.len();
        self.saved_models.retain(|p| p.model != model);
        let removed = self.saved_models.len() != before;
        if removed && self.model == model {
            if let Some(first) = self.saved_models.first().cloned() {
                self.base_url = first.base_url;
                self.model = first.model;
                self.key = first.key;
                self.effort = first.effort;
                self.efforts = first.efforts;
            } else {
                self.model.clear();
                self.effort.clear();
                self.efforts.clear();
            }
        }
        removed
    }

    /// 按单 Agent 指定的模型覆写解析最终请求配置 (未指定或未命中时回落全局默认)
    pub fn resolve_for_model(&self, model_override: Option<&str>) -> AiConfig {
        let Some(m) = model_override.map(str::trim).filter(|s| !s.is_empty()) else {
            return self.clone();
        };
        if let Some(p) = self.saved_models.iter().find(|p| p.model == m) {
            let mut out = self.clone();
            out.base_url = p.base_url.clone();
            out.model = p.model.clone();
            out.key = p.key.clone();
            out.effort = p.effort.clone();
            out.efforts = p.efforts.clone();
            return out;
        }
        self.clone()
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

    #[test]
    fn configured_models_excludes_unconfigured_and_roundtrips_saved_profiles() {
        let empty = AiConfig::default_cfg();
        assert!(
            empty.configured_models().is_empty(),
            "默认未配置时绝不能出现任何内置/未配置模型"
        );

        let mut cfg = AiConfig {
            base_url: "https://api.deepseek.com/v1".into(),
            model: "my-deepseek-v3".into(),
            key: "sk-ds-123456".into(),
            effort: "high".into(),
            efforts: vec!["low".into(), "high".into()],
            consent: true,
            saved_models: vec![],
        };
        cfg.upsert_current_model(Some("My DeepSeek"));
        assert_eq!(cfg.configured_models().len(), 1);
        assert_eq!(cfg.configured_models()[0].display_label(), "My DeepSeek");

        cfg.base_url = "https://api.openai.com/v1".into();
        cfg.model = "my-gpt-model".into();
        cfg.key = "sk-oai-999999".into();
        cfg.effort = "medium".into();
        cfg.upsert_current_model(None);
        assert_eq!(cfg.configured_models().len(), 2);
        assert_eq!(cfg.configured_models()[1].display_label(), "my-gpt-model");

        // JSON 持久化往返（保存后下次启动完整还原，且 Debug 不泄漏密钥）
        let back = AiConfig::from_json(&cfg.to_json());
        assert_eq!(back, cfg);
        assert!(!format!("{:?}", back.saved_models).contains("sk-ds-123456"));

        // 旧版 ai.json（无 savedModels 字段）自动迁移已配置模型
        let legacy = json!({
            "baseUrl": "https://api.deepseek.com/v1",
            "model": "legacy-user-model",
            "key": "sk-legacy",
        });
        let migrated = AiConfig::from_json(&legacy);
        assert_eq!(migrated.configured_models().len(), 1);
        assert_eq!(migrated.configured_models()[0].model, "legacy-user-model");

        // 验证若存在已保存模型同时存在未入库的新模型，configured_models 保持合流且去重
        let mut un_upserted = migrated.clone();
        un_upserted.model = "new-active-model".into();
        let joined = un_upserted.configured_models();
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].model, "legacy-user-model");
        assert_eq!(joined[1].model, "new-active-model");

        // 删除当前选中模型时自动回退至剩余模型或清空
        let mut cur = back;
        assert!(cur.remove_saved_model("my-gpt-model"));
        assert_eq!(cur.model, "my-deepseek-v3");
        assert!(cur.remove_saved_model("my-deepseek-v3"));
        assert!(cur.model.is_empty());
        assert!(cur.configured_models().is_empty());
    }

    #[test]
    fn resolve_for_model_falls_back_cleanly() {
        let mut cfg = AiConfig::default_cfg();
        cfg.base_url = "https://api.openai.com/v1".into();
        cfg.model = "global-model".into();
        cfg.key = "sk-global".into();
        cfg.upsert_current_model(Some("Global"));

        cfg.base_url = "https://api.deepseek.com/v1".into();
        cfg.model = "worker-model".into();
        cfg.key = "sk-worker".into();
        cfg.upsert_current_model(Some("Worker"));
        assert!(cfg.select_saved_model("global-model"));
        assert_eq!(cfg.model, "global-model");

        let resolved_worker = cfg.resolve_for_model(Some("worker-model"));
        assert_eq!(resolved_worker.model, "worker-model");
        assert_eq!(resolved_worker.key, "sk-worker");
        assert_eq!(resolved_worker.base_url, "https://api.deepseek.com/v1");

        // 未命中（例如已被删除的模型）时回落全局默认配置
        let fallback = cfg.resolve_for_model(Some("deleted-model"));
        assert_eq!(fallback.model, "global-model");
        assert_eq!(fallback.key, "sk-global");
    }
}
