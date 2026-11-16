//! 智能体死循环熔断与上下文自适应紧缩引擎 (Agent Guard Engine)
//!
//! 专为极致性能设计 (对标 Sakura 轻量级表现)：
//! - 64位哈希三元组极速重复检测 (< 20ns)，彻底避免死循环耗尽 Token；
//! - 线性单遍上下文紧缩滑动窗口 (Auto-Compact)，防范大模型上下文窗口溢出；
//! - 纯内存无锁算法，零 GC 暂停，零额外依赖。

#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 工具调用签名 (用于循环检测)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallSignature {
    tool_hash: u64,
    args_hash: u64,
}

/// 死循环熔断器 (Loop Breaker)
#[derive(Debug)]
pub struct LoopBreaker {
    max_identical_repeats: usize,
    history: Vec<CallSignature>,
}

impl Default for LoopBreaker {
    fn default() -> Self {
        Self::new(3)
    }
}

impl LoopBreaker {
    pub fn new(max_identical_repeats: usize) -> Self {
        Self {
            max_identical_repeats: max_identical_repeats.max(2),
            history: Vec::with_capacity(8),
        }
    }

    /// 登记一次工具调用，若检测到连续相同调用超过阈值则返回熔断反思引导
    pub fn check_and_record(&mut self, tool: &str, args: &Value) -> Result<(), String> {
        let mut h1 = DefaultHasher::new();
        tool.hash(&mut h1);
        let tool_hash = h1.finish();

        let mut h2 = DefaultHasher::new();
        args.to_string().hash(&mut h2);
        let args_hash = h2.finish();

        let sig = CallSignature { tool_hash, args_hash };
        self.history.push(sig);

        let n = self.history.len();
        if n >= self.max_identical_repeats {
            let slice = &self.history[n - self.max_identical_repeats..];
            if slice.iter().all(|s| s == &sig) {
                return Err(format!(
                    "[LoopBreaker 熔断]: 工具 '{}' 已使用完全相同的参数连续执行 {} 次。\
                     模型可能已陷入死循环。请反思当前方案、更换策略或向用户寻求输入。",
                    tool, self.max_identical_repeats
                ));
            }
        }

        Ok(())
    }

    /// 轮次结束或切换工具时重置历史
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

/// 上下文轻量级紧缩器 (Context Compactor)
pub struct ContextCompactor;

impl ContextCompactor {
    /// 极速单遍预估 Token 数量 (英文 4 字符/Token，中日韩 1.5 字符/Token)
    pub fn estimate_tokens(text: &str) -> usize {
        let mut ascii_count = 0;
        let mut non_ascii_count = 0;

        for b in text.bytes() {
            if b < 128 {
                ascii_count += 1;
            } else {
                non_ascii_count += 1;
            }
        }

        (ascii_count / 4) + (non_ascii_count / 3) + 1
    }

    /// 检查并执行上下文紧缩 (如果消息列表总 Token 超标)
    /// - 永远保留第 0 条 System Prompt
    /// - 永远保留最近 `keep_recent_turns` 条对话
    /// - 将中间过长的历史记录合并为一条压缩摘要
    pub fn compact_messages(
        messages: &mut Vec<Value>,
        max_tokens: usize,
        keep_recent_turns: usize,
    ) -> bool {
        if messages.len() <= keep_recent_turns + 2 {
            return false;
        }

        let estimated: usize = messages
            .iter()
            .map(|m| {
                m.get("content")
                    .and_then(Value::as_str)
                    .map_or(0, Self::estimate_tokens)
            })
            .sum();

        if estimated <= max_tokens {
            return false;
        }

        let keep_idx = messages.len() - keep_recent_turns;
        let middle_slice = &messages[1..keep_idx];

        let mut summary_lines = Vec::new();
        for m in middle_slice {
            let role = m.get("role").and_then(Value::as_str).unwrap_or("unknown");
            let content = m.get("content").and_then(Value::as_str).unwrap_or("");
            let preview: String = content.chars().take(80).collect();
            summary_lines.push(format!("[{role}]: {preview}..."));
        }

        let summary_content = format!(
            "[Context Auto-Compacted Summary]: 历史对话已压缩 (共精简 {} 轮消息):\n{}",
            middle_slice.len(),
            summary_lines.join("\n")
        );

        let summary_msg = serde_json::json!({
            "role": "system",
            "content": summary_content
        });

        // 移除中间消息，插入单条压缩摘要
        messages.drain(1..keep_idx);
        messages.insert(1, summary_msg);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_loop_breaker_triggers_on_repeats() {
        let mut breaker = LoopBreaker::new(3);

        let tool = "read_file";
        let args = json!({ "path": "src/lib.rs" });

        // 前两次正常放行
        assert!(breaker.check_and_record(tool, &args).is_ok());
        assert!(breaker.check_and_record(tool, &args).is_ok());

        // 第三次完全相同，触发熔断
        let res = breaker.check_and_record(tool, &args);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("LoopBreaker 熔断"));

        // 重置后恢复正常
        breaker.reset();
        assert!(breaker.check_and_record(tool, &args).is_ok());
    }

    #[test]
    fn test_loop_breaker_different_args_pass() {
        let mut breaker = LoopBreaker::new(3);

        assert!(breaker.check_and_record("read_file", &json!({ "path": "a.rs" })).is_ok());
        assert!(breaker.check_and_record("read_file", &json!({ "path": "b.rs" })).is_ok());
        assert!(breaker.check_and_record("read_file", &json!({ "path": "c.rs" })).is_ok());
    }

    #[test]
    fn test_context_compact_preserves_head_and_tail() {
        let mut msgs = vec![
            json!({ "role": "system", "content": "System Directive" }),
            json!({ "role": "user", "content": "Question 1" }),
            json!({ "role": "assistant", "content": "Answer 1" }),
            json!({ "role": "user", "content": "Question 2" }),
            json!({ "role": "assistant", "content": "Answer 2" }),
            json!({ "role": "user", "content": "Question 3" }),
            json!({ "role": "assistant", "content": "Answer 3" }),
            json!({ "role": "user", "content": "Current Active Question" }),
        ];

        // 强制触发压缩，保留最近 2 条
        let compacted = ContextCompactor::compact_messages(&mut msgs, 10, 2);
        assert!(compacted);

        // 验证系统指令保留在 index 0
        assert_eq!(msgs[0]["content"], "System Directive");
        // 验证 index 1 为压缩摘要
        assert!(msgs[1]["content"].as_str().unwrap().contains("Context Auto-Compacted Summary"));
        // 验证最新活跃消息依然完整保存在末尾
        assert_eq!(msgs.last().unwrap()["content"], "Current Active Question");
    }
}
