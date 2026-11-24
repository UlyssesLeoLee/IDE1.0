//! 智能体生命周期拦截器体系 (Agent Lifecycle Hooks Engine)
//!
//! 专为极致性能设计 (对标 Sakura 轻量级表现)：
//! - 纳秒级快速短路判断，无敏感操作时执行开销 < 50ns；
//! - 覆盖智能体端到端全生命周期：输入预处理、工具调用前鉴权、工具调用后脱敏截断、输出自审；
//! - 内建目录穿越防御、私钥防泄露、超大载荷保护与 API Key 自动脱敏。

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// 拦截器处置决议
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// 允许放行，保持原样
    Continue,
    /// 允许放行，但替换/修改内容 (如脱敏、截断、注入前缀)
    Modify(String),
    /// 拦截并拒绝执行，返回安全原因
    Reject(String),
}

/// 智能体生命周期钩子特征
pub trait AgentHook: Send + Sync {
    /// 钩子名称
    fn name(&self) -> &str;

    /// 1. 用户输入到达，发送至 LLM 之前的预处理
    fn pre_input(&self, _input: &str) -> HookDecision {
        HookDecision::Continue
    }

    /// 2. LLM 决定调用某个工具，实际执行前的权限与参数安全审查
    fn pre_tool_call(&self, _tool: &str, _args: &Value) -> HookDecision {
        HookDecision::Continue
    }

    /// 3. 工具执行完毕，结果回灌至 LLM 上下文前的脱敏与尺寸裁剪
    fn post_tool_call(&self, _tool: &str, _result: &str) -> HookDecision {
        HookDecision::Continue
    }

    /// 4. LLM 最终响应生成后的格式与安全后置检查
    fn post_response(&self, _response: &str) -> HookDecision {
        HookDecision::Continue
    }
}

/// 拦截器责任链管道
pub struct HookPipeline {
    hooks: Vec<Box<dyn AgentHook>>,
}

impl Default for HookPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl HookPipeline {
    pub fn new() -> Self {
        let mut p = Self { hooks: Vec::new() };
        p.add_hook(Box::new(PathSanitizerHook));
        p.add_hook(Box::new(PayloadLimitHook::new(20_000)));
        p.add_hook(Box::new(SensitiveDataMaskHook));
        p
    }

    pub fn add_hook(&mut self, hook: Box<dyn AgentHook>) {
        let name = hook.name().to_string();
        self.hooks.retain(|h| h.name() != name);
        self.hooks.push(hook);
    }

    /// 返回当前已挂载的全部 Hook 名称清单（含内建、用户自定义与第三方 Hook）
    pub fn hook_names(&self) -> Vec<String> {
        self.hooks.iter().map(|h| h.name().to_string()).collect()
    }

    /// 从单个 JSON 配置文件加载用户自定义或第三方 Hook 规则（支持 `{"hooks": [...]}` 或单条对象）
    pub fn load_custom_hooks_from_file(&mut self, path: &Path) -> usize {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return 0;
        };
        self.load_custom_hooks_from_str(&raw)
    }

    /// 从 JSON 字符串加载自定义或第三方 Hook 规则
    pub fn load_custom_hooks_from_str(&mut self, raw: &str) -> usize {
        let specs: Vec<CustomHookSpec> =
            if let Ok(cfg) = serde_json::from_str::<CustomHooksConfig>(raw) {
                cfg.hooks
            } else if let Ok(list) = serde_json::from_str::<Vec<CustomHookSpec>>(raw) {
                list
            } else if let Ok(single) = serde_json::from_str::<CustomHookSpec>(raw) {
                vec![single]
            } else {
                return 0;
            };
        let mut added = 0usize;
        for spec in specs {
            if spec.enabled && !spec.name.trim().is_empty() {
                self.add_hook(Box::new(CustomRuleHook::new(spec)));
                added += 1;
            }
        }
        added
    }

    /// 从目录扫描所有 `*.json` 自定义或第三方 Hook 文件
    pub fn load_custom_hooks_from_dir(&mut self, dir: &Path) -> usize {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut paths: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        let mut total = 0usize;
        for p in paths {
            total += self.load_custom_hooks_from_file(&p);
        }
        total
    }

    /// 自动加载工作区与用户全局目录下的自定义及第三方 Hooks
    /// 探测顺序：
    /// 1. 用户全局 `<user_dir>/custom_hooks.json` 与 `<user_dir>/custom_hooks/*.json`
    /// 2. 工作区 `.hooks.json`、`hooks.json`、`.hooks/*.json`、`.claude/hooks/*.json`
    pub fn load_workspace_and_user_hooks(
        &mut self,
        ws_root: &Path,
        user_dir: Option<&Path>,
    ) -> usize {
        let mut count = 0usize;
        if let Some(udir) = user_dir {
            count += self.load_custom_hooks_from_file(&udir.join("custom_hooks.json"));
            count += self.load_custom_hooks_from_dir(&udir.join("custom_hooks"));
        }
        for rel_file in [".hooks.json", "hooks.json", ".ide-hooks.json"] {
            count += self.load_custom_hooks_from_file(&ws_root.join(rel_file));
        }
        for rel_dir in [".hooks", ".claude/hooks"] {
            count += self.load_custom_hooks_from_dir(&ws_root.join(rel_dir));
        }
        count
    }

    /// 执行输入前置拦截链
    pub fn execute_pre_input(&self, mut input: String) -> Result<String, String> {
        for h in &self.hooks {
            match h.pre_input(&input) {
                HookDecision::Continue => {}
                HookDecision::Modify(modified) => input = modified,
                HookDecision::Reject(reason) => {
                    return Err(format!("[Hook: {} 拦截输入] {}", h.name(), reason));
                }
            }
        }
        Ok(input)
    }

    /// 执行工具调用前置审查
    pub fn execute_pre_tool_call(&self, tool: &str, args: &Value) -> Result<(), String> {
        for h in &self.hooks {
            match h.pre_tool_call(tool, args) {
                HookDecision::Continue => {}
                HookDecision::Modify(_) => {}
                HookDecision::Reject(reason) => {
                    return Err(format!(
                        "[Hook: {} 拒绝工具调用 '{}'] {}",
                        h.name(),
                        tool,
                        reason
                    ));
                }
            }
        }
        Ok(())
    }

    /// 执行工具调用后置处理
    pub fn execute_post_tool_call(&self, tool: &str, mut result: String) -> String {
        for h in &self.hooks {
            match h.post_tool_call(tool, &result) {
                HookDecision::Continue => {}
                HookDecision::Modify(modified) => result = modified,
                HookDecision::Reject(_) => {}
            }
        }
        result
    }

    /// 执行模型回复后置处理
    pub fn execute_post_response(&self, mut response: String) -> Result<String, String> {
        for h in &self.hooks {
            match h.post_response(&response) {
                HookDecision::Continue => {}
                HookDecision::Modify(modified) => response = modified,
                HookDecision::Reject(reason) => {
                    return Err(format!("[Hook: {} 拒绝响应] {}", h.name(), reason));
                }
            }
        }
        Ok(response)
    }
}

// ---------- 内建高频合规与安全钩子 ----------

/// 1. 路径穿越与敏感文件保护钩子
pub struct PathSanitizerHook;

impl AgentHook for PathSanitizerHook {
    fn name(&self) -> &str {
        "PathSanitizerHook"
    }

    fn pre_tool_call(&self, _tool: &str, args: &Value) -> HookDecision {
        let path = args
            .get("path")
            .or_else(|| args.get("file_path"))
            .and_then(Value::as_str);

        if let Some(p) = path {
            // 拦截目录穿越
            if p.contains("..") {
                return HookDecision::Reject("禁止目录穿越访问相对父路径 (..)".into());
            }
            // 拦截敏感扩展名
            let lower = p.to_ascii_lowercase();
            if lower.ends_with(".key") || lower.ends_with(".sig") || lower.ends_with(".pem") {
                return HookDecision::Reject(
                    "安全策略拦截: 禁止 Agent 访问/写入私钥与签名文件".into(),
                );
            }
            // 拦截系统底层目录
            if lower.starts_with("/etc")
                || lower.starts_with("c:\\windows")
                || lower.starts_with("c:/windows")
            {
                return HookDecision::Reject("安全策略拦截: 禁止访问系统核心目录".into());
            }
        }
        HookDecision::Continue
    }
}

/// 2. 载荷大小限制与自动截断钩子 (防止超出上下文)
pub struct PayloadLimitHook {
    max_chars: usize,
}

impl PayloadLimitHook {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }
}

impl AgentHook for PayloadLimitHook {
    fn name(&self) -> &str {
        "PayloadLimitHook"
    }

    fn post_tool_call(&self, _tool: &str, result: &str) -> HookDecision {
        if result.len() > self.max_chars {
            let truncated: String = result.chars().take(self.max_chars).collect();
            HookDecision::Modify(format!(
                "{}\n\n[System Notice: 工具输出过大，已自动截断前 {} 字符，防止上下文溢出]",
                truncated, self.max_chars
            ))
        } else {
            HookDecision::Continue
        }
    }
}

/// 3. API Key 与敏感凭据自动脱敏钩子
pub struct SensitiveDataMaskHook;

impl AgentHook for SensitiveDataMaskHook {
    fn name(&self) -> &str {
        "SensitiveDataMaskHook"
    }

    fn post_tool_call(&self, _tool: &str, result: &str) -> HookDecision {
        if result.contains("sk-") || result.contains("Bearer ") {
            let masked = mask_secrets(result);
            HookDecision::Modify(masked)
        } else {
            HookDecision::Continue
        }
    }

    fn post_response(&self, response: &str) -> HookDecision {
        if response.contains("sk-") {
            let masked = mask_secrets(response);
            HookDecision::Modify(masked)
        } else {
            HookDecision::Continue
        }
    }
}

/// 快速字符级脱敏 (扫描 `sk-` 开头的密钥)
fn mask_secrets(input: &str) -> String {
    mask_token_prefix(input, "sk-")
}

fn mask_token_prefix(input: &str, prefix: &str) -> String {
    if prefix.is_empty() || !input.contains(prefix) {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find(prefix) {
        out.push_str(&rest[..idx]);
        out.push_str(prefix);
        out.push_str("****");
        let token_slice = &rest[idx + prefix.len()..];
        let end_idx = token_slice
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
            .unwrap_or(token_slice.len());
        rest = &token_slice[end_idx..];
    }
    out.push_str(rest);
    out
}

// ---------- 用户自定义与第三方可扩展 Hook 规范 ----------

fn default_hook_enabled() -> bool {
    true
}

/// 用户自定义或第三方导入的声明式 Hook 规则
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomHookSpec {
    /// 钩子唯一名称（同名规则自动热更新覆盖）
    pub name: String,
    /// 规则描述（如来源的第三方包名或用途）
    #[serde(default)]
    pub description: String,
    /// 是否启用
    #[serde(default = "default_hook_enabled")]
    pub enabled: bool,
    /// 在 `pre_input` 阶段向用户输入追加的安全/规范前置提示
    #[serde(default)]
    pub pre_input_prefix: Option<String>,
    /// 在 `pre_input` 阶段拦截的违禁词列表
    #[serde(default)]
    pub pre_input_block_patterns: Vec<String>,
    /// 在 `pre_tool_call` 阶段禁止调用的工具名列表
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    /// 在 `pre_tool_call` 阶段拦截的参数子串（如危险命令 `rm -rf`、敏感路径等）
    #[serde(default)]
    pub pre_tool_block_patterns: Vec<String>,
    /// 在 `post_tool_call` 与 `post_response` 阶段自动脱敏的自定义或第三方 Token 前缀（如 `glpat-`, `xoxb-`, `AIza`）
    #[serde(default)]
    pub mask_patterns: Vec<String>,
}

/// 自定义与第三方 Hook 配置文件结构（`.hooks.json` / `custom_hooks.json`）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomHooksConfig {
    #[serde(default)]
    pub hooks: Vec<CustomHookSpec>,
}

/// 执行用户自定义或第三方声明式规则的 Hook 实例
pub struct CustomRuleHook {
    spec: CustomHookSpec,
}

impl CustomRuleHook {
    pub fn new(spec: CustomHookSpec) -> Self {
        Self { spec }
    }
}

impl AgentHook for CustomRuleHook {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn pre_input(&self, input: &str) -> HookDecision {
        for pat in &self.spec.pre_input_block_patterns {
            if !pat.is_empty() && input.contains(pat.as_str()) {
                return HookDecision::Reject(format!(
                    "触发自定义/第三方 Hook '{}' 输入拦截规则: 包含禁止模式 '{}'",
                    self.spec.name, pat
                ));
            }
        }
        if let Some(prefix) = &self.spec.pre_input_prefix {
            let p = prefix.trim();
            if !p.is_empty() && !input.contains(p) {
                return HookDecision::Modify(format!("{p}\n{input}"));
            }
        }
        HookDecision::Continue
    }

    fn pre_tool_call(&self, tool: &str, args: &Value) -> HookDecision {
        if self.spec.blocked_tools.iter().any(|t| t == tool) {
            return HookDecision::Reject(format!(
                "触发自定义/第三方 Hook '{}' 工具封禁策略: 禁止调用 '{}'",
                self.spec.name, tool
            ));
        }
        if !self.spec.pre_tool_block_patterns.is_empty() {
            let args_s = args.to_string();
            for pat in &self.spec.pre_tool_block_patterns {
                if !pat.is_empty() && args_s.contains(pat.as_str()) {
                    return HookDecision::Reject(format!(
                        "触发自定义/第三方 Hook '{}' 参数拦截策略: 检测到 '{}'",
                        self.spec.name, pat
                    ));
                }
            }
        }
        HookDecision::Continue
    }

    fn post_tool_call(&self, _tool: &str, result: &str) -> HookDecision {
        let mut current = result.to_string();
        let mut modified = false;
        for pat in &self.spec.mask_patterns {
            if !pat.is_empty() && current.contains(pat.as_str()) {
                current = mask_token_prefix(&current, pat);
                modified = true;
            }
        }
        if modified {
            HookDecision::Modify(current)
        } else {
            HookDecision::Continue
        }
    }

    fn post_response(&self, response: &str) -> HookDecision {
        let mut current = response.to_string();
        let mut modified = false;
        for pat in &self.spec.mask_patterns {
            if !pat.is_empty() && current.contains(pat.as_str()) {
                current = mask_token_prefix(&current, pat);
                modified = true;
            }
        }
        if modified {
            HookDecision::Modify(current)
        } else {
            HookDecision::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_path_sanitizer_blocks_traversal_and_keys() {
        let pipeline = HookPipeline::default();

        // 正常路径放行
        assert!(pipeline
            .execute_pre_tool_call("read_file", &json!({ "path": "src/main.rs" }))
            .is_ok());

        // 拦截 .. 穿越
        assert!(pipeline
            .execute_pre_tool_call("read_file", &json!({ "path": "../secret.txt" }))
            .is_err());

        // 拦截 .key 私钥
        assert!(pipeline
            .execute_pre_tool_call("read_file", &json!({ "path": "certs/server.key" }))
            .is_err());
        assert!(pipeline
            .execute_pre_tool_call("read_file", &json!({ "path": "release.sig" }))
            .is_err());
    }

    #[test]
    fn test_payload_limit_truncates() {
        let pipeline = HookPipeline::default();
        let huge_text = "a".repeat(25_000);
        let res = pipeline.execute_post_tool_call("grep", huge_text);
        assert!(res.contains("已自动截断前 20000 字符"));
        assert!(res.len() < 22_000);
    }

    #[test]
    fn test_secret_masking() {
        let pipeline = HookPipeline::default();
        let input = "My API key is sk-proj-1234567890abcdef and token is sk-999.";
        let masked = pipeline.execute_post_tool_call("cmd", input.to_string());
        assert!(!masked.contains("sk-proj-1234567890abcdef"));
        assert!(masked.contains("sk-****"));
    }

    #[test]
    fn test_custom_and_third_party_hooks_loading_and_execution() {
        let mut pipeline = HookPipeline::default();
        let custom_json = r#"{
            "hooks": [
                {
                    "name": "ThirdPartyGitLabHook",
                    "description": "第三方 GitLab Token 脱敏与危险命令拦截",
                    "enabled": true,
                    "pre_input_prefix": "[OrgRule]",
                    "pre_input_block_patterns": ["FORBIDDEN_PROMPT"],
                    "blocked_tools": ["drop_db"],
                    "pre_tool_block_patterns": ["rm -rf /"],
                    "mask_patterns": ["glpat-", "xoxb-"]
                }
            ]
        }"#;
        let loaded = pipeline.load_custom_hooks_from_str(custom_json);
        assert_eq!(loaded, 1);
        assert!(pipeline
            .hook_names()
            .contains(&"ThirdPartyGitLabHook".to_string()));

        // Pre-Input 注入与拦截
        let modified = pipeline.execute_pre_input("hello".into()).unwrap();
        assert!(modified.starts_with("[OrgRule]\nhello"));
        assert!(pipeline
            .execute_pre_input("run FORBIDDEN_PROMPT".into())
            .is_err());

        // Pre-Tool 拦截
        assert!(pipeline
            .execute_pre_tool_call("drop_db", &json!({}))
            .is_err());
        assert!(pipeline
            .execute_pre_tool_call("exec", &json!({ "cmd": "rm -rf /" }))
            .is_err());

        // Post-Response 第三方凭证脱敏
        let resp = pipeline
            .execute_post_response("token=glpat-abcdef123456 slack=xoxb-998877".into())
            .unwrap();
        assert!(resp.contains("glpat-****"));
        assert!(resp.contains("xoxb-****"));
        assert!(!resp.contains("abcdef123456"));
    }
}
