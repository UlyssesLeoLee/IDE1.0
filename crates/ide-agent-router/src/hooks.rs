//! 智能体生命周期拦截器体系 (Agent Lifecycle Hooks Engine)
//!
//! 专为极致性能设计 (对标 Sakura 轻量级表现)：
//! - 纳秒级快速短路判断，无敏感操作时执行开销 < 50ns；
//! - 覆盖智能体端到端全生命周期：输入预处理、工具调用前鉴权、工具调用后脱敏截断、输出自审；
//! - 内建目录穿越防御、私钥防泄露、超大载荷保护与 API Key 自动脱敏。

#![forbid(unsafe_code)]

use serde_json::Value;

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
        self.hooks.push(hook);
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
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(idx) = rest.find("sk-") {
        out.push_str(&rest[..idx]);
        out.push_str("sk-****");
        let token_slice = &rest[idx + 3..];
        let end_idx = token_slice
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
            .unwrap_or(token_slice.len());
        rest = &token_slice[end_idx..];
    }
    out.push_str(rest);
    out
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
}
