//! AI bridge — 抽象 trait + Placeholder 实现 (per ULYS-191 §3 brief).
//!
//! 真实 LLM 接入 (Anthropic / OpenAI / MiniMax) 留到后续 brief。
//! 本骨架用 PlaceholderAi: 把 prompt 前缀 "AI suggestion: " 后回显输入。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("AI backend not configured: {0}")]
    NotConfigured(String),
    #[error("AI request failed: {0}")]
    RequestFailed(String),
}

/// AI bridge trait — ide-shell 调它拿到 inline 补全文本。
///
/// Stage 3.0 brief:仅 trait 定义 + PlaceholderAi,真实实现 Stage 4+。
pub trait AiBackend: Send {
    /// 给定当前输入,返回补全建议文本。若空 = 无建议。
    fn complete(&self, prompt: &str, cursor_context: &str) -> Result<String, AiError>;
}

/// 不连真实网络,直接回显的占位实现。
pub struct PlaceholderAi;

impl AiBackend for PlaceholderAi {
    fn complete(&self, prompt: &str, _cursor_context: &str) -> Result<String, AiError> {
        if prompt.trim().is_empty() {
            return Ok(String::new());
        }
        Ok(format!("AI suggestion: {}", prompt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_empty_input() {
        let ai = PlaceholderAi;
        assert_eq!(ai.complete("", "").unwrap(), "");
        assert_eq!(ai.complete("   ", "").unwrap(), "");
    }

    #[test]
    fn test_placeholder_echoes() {
        let ai = PlaceholderAi;
        assert_eq!(
            ai.complete("Get-Process", "").unwrap(),
            "AI suggestion: Get-Process"
        );
    }
}
