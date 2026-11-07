//! 异构智能体独立模型、推理强度 (`ReasoningEffort`)、思考模式 (`ThinkingMode`) 与闭环熔断守卫
//!
//! 对应设计规范：
//! - `docs/01_基本设计/BD-002_Multi-Agent-NEMA-Collab-Platform.md` §4 & §5
//! - `docs/02_详细设计/DD-002_NEMA-Aggregator-Git-Verify-Pipeline.md` §5 & §6

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// 四阶段多智能体协同流水线标准槽位标识
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AgentSlotId {
    /// ① 路由智能体 (Router: 任务拆解 / 文件分配 / 协作策略 / 依赖分析)
    Router,
    /// ② 协同编辑组 A · Agent A1 (分支: feat/A1, 共同编辑 File A.nema)
    AgentA1,
    /// ② 协同编辑组 A · Agent A2 (分支: feat/A2, 共同编辑 File A.nema)
    AgentA2,
    /// ② 独立编辑组 B · Agent B1 (分支: feat/B1, 独立编辑 File B.nema)
    AgentB1,
    /// ② 独立编辑组 B · Agent B2 (分支: feat/B2, 独立编辑 File C.nema)
    AgentB2,
    /// ③ 统筹智能体 (Aggregator: 汇总多分支变更 / 语义合并 / .nema → Rust 转换)
    Aggregator,
    /// ④ 验证智能体 (Verify: 构建验证 / 测试 / 质量检查 / 规范校验 / 逆向反馈)
    Verify,
    /// 动态扩展智能体槽位
    Custom(String),
}

impl AgentSlotId {
    pub fn as_key(&self) -> &str {
        match self {
            Self::Router => "router",
            Self::AgentA1 => "agent-a1",
            Self::AgentA2 => "agent-a2",
            Self::AgentB1 => "agent-b1",
            Self::AgentB2 => "agent-b2",
            Self::Aggregator => "aggregator",
            Self::Verify => "verify",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key.trim().to_ascii_lowercase().as_str() {
            "router" => Self::Router,
            "agent-a1" | "a1" => Self::AgentA1,
            "agent-a2" | "a2" => Self::AgentA2,
            "agent-b1" | "b1" => Self::AgentB1,
            "agent-b2" | "b2" => Self::AgentB2,
            "aggregator" => Self::Aggregator,
            "verify" | "verifier" => Self::Verify,
            other => Self::Custom(other.to_string()),
        }
    }

    pub fn default_branch(&self) -> Option<&'static str> {
        match self {
            Self::Router => None,
            Self::AgentA1 => Some("feat/A1"),
            Self::AgentA2 => Some("feat/A2"),
            Self::AgentB1 => Some("feat/B1"),
            Self::AgentB2 => Some("feat/B2"),
            Self::Aggregator => Some("agg/main-candidate"),
            Self::Verify => None,
            Self::Custom(_) => None,
        }
    }
}

/// 推理强度档位 (`reasoning_effort`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReasoningEffort {
    /// 继承所属组或全局默认档位
    #[default]
    Inherit,
    /// 不传递 `reasoning_effort`，使用模型自身默认
    ProviderDefault,
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
    /// 供应商自定义档位
    Custom(String),
}

impl ReasoningEffort {
    pub fn as_wire_str(&self) -> Option<&str> {
        match self {
            Self::Inherit | Self::ProviderDefault => None,
            Self::None => Some("none"),
            Self::Minimal => Some("minimal"),
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
            Self::XHigh => Some("xhigh"),
            Self::Max => Some("max"),
            Self::Custom(s) if !s.trim().is_empty() => Some(s.as_str()),
            Self::Custom(_) => None,
        }
    }

    pub fn from_wire_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "inherit" => Self::Inherit,
            "default" | "provider_default" => Self::ProviderDefault,
            "none" => Self::None,
            "minimal" => Self::Minimal,
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "xhigh" => Self::XHigh,
            "max" => Self::Max,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// 思考模式与预算 (`ThinkingMode`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThinkingMode {
    /// 继承全局默认思考模式
    #[default]
    Inherit,
    /// 显式关闭思考链（低延迟直接输出）
    Disabled,
    /// 由模型自适应决定是否开启思考链
    Auto,
    /// 强制开启扩展思考，并限定最大思考 Token 预算
    Enabled { budget_tokens: u32 },
    /// 工具调用交错思考模式 (`interleaved-thinking`)
    Interleaved { budget_tokens: u32 },
}

impl ThinkingMode {
    pub const MIN_BUDGET_TOKENS: u32 = 1024;
    pub const MAX_BUDGET_TOKENS: u32 = 131_072;

    pub fn clamp_budget(budget: u32) -> u32 {
        budget.clamp(Self::MIN_BUDGET_TOKENS, Self::MAX_BUDGET_TOKENS)
    }

    /// 当供应商不支持高阶思考模式时的确定性降级迁移：`Interleaved -> Enabled -> Auto -> Disabled`
    pub fn downgrade(&self) -> Self {
        match *self {
            Self::Interleaved { budget_tokens } => Self::Enabled { budget_tokens },
            Self::Enabled { .. } => Self::Auto,
            Self::Auto | Self::Inherit => Self::Disabled,
            Self::Disabled => Self::Disabled,
        }
    }
}

/// 全局默认 AI 基线配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalAiBaseline {
    pub base_url: String,
    pub model: String,
    pub secret_ref: Option<String>,
    pub effort: ReasoningEffort,
    pub thinking_mode: ThinkingMode,
    pub temperature: Option<f32>,
    pub max_output_tokens: u32,
}

impl Default for GlobalAiBaseline {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-5".to_string(),
            secret_ref: None,
            effort: ReasoningEffort::Medium,
            thinking_mode: ThinkingMode::Auto,
            temperature: None,
            max_output_tokens: 16_384,
        }
    }
}

/// 单个智能体的独立模型、Effort 与思考模式配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentModelProfile {
    pub slot_id: AgentSlotId,
    pub display_name: String,
    pub git_branch: Option<String>,
    pub base_url_override: Option<String>,
    pub model_override: Option<String>,
    /// 仅存储 SecretRef 引用名，绝不明文序列化密钥 (`INV-07`)
    pub secret_ref: Option<String>,
    pub effort: ReasoningEffort,
    pub thinking_mode: ThinkingMode,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
}

/// 经过三级继承解析并完成边界不变量规整后的最终生效配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveAgentConfig {
    pub slot_id: AgentSlotId,
    pub base_url: String,
    pub model: String,
    pub secret_ref: Option<String>,
    pub effort: ReasoningEffort,
    pub thinking_mode: ThinkingMode,
    pub temperature: Option<f32>,
    pub max_output_tokens: u32,
    pub extra_headers: BTreeMap<String, String>,
}

impl EffectiveAgentConfig {
    /// 生成归一化后的模型请求 JSON 字段（兼容 OpenAI / Anthropic / Gemini / DeepSeek 网关）
    pub fn build_wire_parameters(&self) -> Value {
        let mut map = Map::new();
        map.insert("model".to_string(), Value::String(self.model.clone()));
        map.insert(
            "max_tokens".to_string(),
            Value::Number(self.max_output_tokens.into()),
        );

        if let Some(eff) = self.effort.as_wire_str() {
            map.insert(
                "reasoning_effort".to_string(),
                Value::String(eff.to_string()),
            );
        }

        match self.thinking_mode {
            ThinkingMode::Disabled => {
                map.insert("thinking".to_string(), json!({ "type": "disabled" }));
            }
            ThinkingMode::Auto => {
                map.insert("thinking".to_string(), json!({ "type": "auto" }));
            }
            ThinkingMode::Enabled { budget_tokens }
            | ThinkingMode::Interleaved { budget_tokens } => {
                map.insert(
                    "thinking".to_string(),
                    json!({
                        "type": "enabled",
                        "budget_tokens": ThinkingMode::clamp_budget(budget_tokens),
                    }),
                );
            }
            ThinkingMode::Inherit => {}
        }

        // 仅在未开启强制扩展思考时传递自定义 temperature，防止与扩展思考的固定温度约束冲突
        if !matches!(
            self.thinking_mode,
            ThinkingMode::Enabled { .. } | ThinkingMode::Interleaved { .. }
        ) {
            if let Some(temp) = self.temperature {
                if let Some(n) = serde_json::Number::from_f64(f64::from(temp.clamp(0.0, 2.0))) {
                    map.insert("temperature".to_string(), Value::Number(n));
                }
            }
        }

        Value::Object(map)
    }
}

/// 多智能体模型与思考配置注册表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentProfileRegistry {
    pub global: GlobalAiBaseline,
    pub profiles: BTreeMap<String, AgentModelProfile>,
}

impl Default for MultiAgentProfileRegistry {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        let presets = [
            (
                AgentSlotId::Router,
                "路由智能体 (Router)",
                ReasoningEffort::High,
                ThinkingMode::Enabled {
                    budget_tokens: 16_384,
                },
            ),
            (
                AgentSlotId::AgentA1,
                "协同编辑组 A · Agent A1",
                ReasoningEffort::Medium,
                ThinkingMode::Enabled {
                    budget_tokens: 4_096,
                },
            ),
            (
                AgentSlotId::AgentA2,
                "协同编辑组 A · Agent A2",
                ReasoningEffort::Medium,
                ThinkingMode::Enabled {
                    budget_tokens: 4_096,
                },
            ),
            (
                AgentSlotId::AgentB1,
                "独立编辑组 B · Agent B1",
                ReasoningEffort::Low,
                ThinkingMode::Auto,
            ),
            (
                AgentSlotId::AgentB2,
                "独立编辑组 B · Agent B2",
                ReasoningEffort::Low,
                ThinkingMode::Auto,
            ),
            (
                AgentSlotId::Aggregator,
                "统筹智能体 (Aggregator)",
                ReasoningEffort::XHigh,
                ThinkingMode::Interleaved {
                    budget_tokens: 32_768,
                },
            ),
            (
                AgentSlotId::Verify,
                "验证智能体 (Verify)",
                ReasoningEffort::High,
                ThinkingMode::Interleaved {
                    budget_tokens: 8_192,
                },
            ),
        ];

        for (slot, name, effort, thinking_mode) in presets {
            let key = slot.as_key().to_string();
            let branch = slot.default_branch().map(str::to_string);
            profiles.insert(
                key,
                AgentModelProfile {
                    slot_id: slot,
                    display_name: name.to_string(),
                    git_branch: branch,
                    base_url_override: None,
                    model_override: None,
                    secret_ref: None,
                    effort,
                    thinking_mode,
                    temperature: None,
                    max_output_tokens: None,
                },
            );
        }

        Self {
            global: GlobalAiBaseline::default(),
            profiles,
        }
    }
}

impl MultiAgentProfileRegistry {
    pub fn set_profile(&mut self, profile: AgentModelProfile) {
        self.profiles
            .insert(profile.slot_id.as_key().to_string(), profile);
    }

    /// 解析指定智能体槽位的最终生效配置（含预算与输出上限不变量校验：`max_output_tokens > budget_tokens`）
    pub fn resolve(&self, slot: &AgentSlotId) -> EffectiveAgentConfig {
        let p = self.profiles.get(slot.as_key());

        let base_url = p
            .and_then(|x| x.base_url_override.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.global.base_url)
            .to_string();

        let model = p
            .and_then(|x| x.model_override.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.global.model)
            .to_string();

        let secret_ref = p
            .and_then(|x| x.secret_ref.clone())
            .or_else(|| self.global.secret_ref.clone());

        let effort = match p.map(|x| &x.effort) {
            Some(e) if *e != ReasoningEffort::Inherit => e.clone(),
            _ => self.global.effort.clone(),
        };

        let thinking_mode = match p.map(|x| &x.thinking_mode) {
            Some(m) if *m != ThinkingMode::Inherit => m.clone(),
            _ => self.global.thinking_mode.clone(),
        };

        let temperature = p.and_then(|x| x.temperature).or(self.global.temperature);

        let requested_max = p
            .and_then(|x| x.max_output_tokens)
            .unwrap_or(self.global.max_output_tokens)
            .max(1024);

        // 不变量保证: 当开启扩展思考或交错思考时，max_output_tokens 必须严格大于 budget_tokens
        let max_output_tokens = match thinking_mode {
            ThinkingMode::Enabled { budget_tokens }
            | ThinkingMode::Interleaved { budget_tokens } => {
                let clamped = ThinkingMode::clamp_budget(budget_tokens);
                requested_max.max(clamped.saturating_add(4096))
            }
            _ => requested_max,
        };

        let mut extra_headers = BTreeMap::new();
        if matches!(thinking_mode, ThinkingMode::Interleaved { .. }) {
            extra_headers.insert(
                "anthropic-beta".to_string(),
                "interleaved-thinking-2025-05-14".to_string(),
            );
        }

        EffectiveAgentConfig {
            slot_id: slot.clone(),
            base_url,
            model,
            secret_ref,
            effort,
            thinking_mode,
            temperature,
            max_output_tokens,
            extra_headers,
        }
    }
}

/// 阶段 ④ → 阶段 ① 反馈与迭代闭环熔断守卫 (`FeedbackLoopGuard`)
///
/// 防止 `Verify` 失败后无限回流至 `Router` 造成死循环与 Token 耗尽。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackLoopGuard {
    pub max_rounds: u32,
    pub max_token_budget: u64,
    pub current_round: u32,
    pub consumed_tokens: u64,
    pub seen_error_fingerprints: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackDecision {
    /// 允许进入下一轮自动修复闭环
    ContinueIteration { next_round: u32 },
    /// 触发熔断并升级至人工审查 (Human-in-the-Loop)
    CircuitBreak { reason: String },
}

impl Default for FeedbackLoopGuard {
    fn default() -> Self {
        Self {
            max_rounds: 3,
            max_token_budget: 250_000,
            current_round: 0,
            consumed_tokens: 0,
            seen_error_fingerprints: BTreeSet::new(),
        }
    }
}

impl FeedbackLoopGuard {
    pub fn record_and_check(
        &mut self,
        round_tokens: u64,
        error_fingerprint: &str,
    ) -> FeedbackDecision {
        self.current_round = self.current_round.saturating_add(1);
        self.consumed_tokens = self.consumed_tokens.saturating_add(round_tokens);

        if self.current_round > self.max_rounds {
            return FeedbackDecision::CircuitBreak {
                reason: format!(
                    "已达到最大自动反馈迭代轮数上限 ({}/{})",
                    self.current_round, self.max_rounds
                ),
            };
        }
        if self.consumed_tokens > self.max_token_budget {
            return FeedbackDecision::CircuitBreak {
                reason: format!(
                    "已超出闭环最大 Token 预算 ({}/{})",
                    self.consumed_tokens, self.max_token_budget
                ),
            };
        }
        if !error_fingerprint.trim().is_empty()
            && !self
                .seen_error_fingerprints
                .insert(error_fingerprint.to_string())
        {
            return FeedbackDecision::CircuitBreak {
                reason: format!(
                    "检测到连续重复的编译/测试错误指纹 ({error_fingerprint})，已熔断以防死循环"
                ),
            };
        }

        FeedbackDecision::ContinueIteration {
            next_round: self.current_round,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_assigns_distinct_profiles_and_enforces_budget_invariants() {
        let mut reg = MultiAgentProfileRegistry::default();
        let router_cfg = reg.resolve(&AgentSlotId::Router);
        assert_eq!(router_cfg.effort, ReasoningEffort::High);
        assert_eq!(
            router_cfg.thinking_mode,
            ThinkingMode::Enabled {
                budget_tokens: 16_384
            }
        );
        // max_output_tokens must strictly exceed budget_tokens (16384 + 4096 = 20480)
        assert!(router_cfg.max_output_tokens >= 20_480);

        let agg_cfg = reg.resolve(&AgentSlotId::Aggregator);
        assert_eq!(agg_cfg.effort, ReasoningEffort::XHigh);
        assert_eq!(
            agg_cfg.extra_headers.get("anthropic-beta").map(String::as_str),
            Some("interleaved-thinking-2025-05-14")
        );
        assert!(agg_cfg.max_output_tokens >= 32_768 + 4096);

        // Override Agent A1 with a custom model and custom effort
        reg.set_profile(AgentModelProfile {
            slot_id: AgentSlotId::AgentA1,
            display_name: "Custom A1".to_string(),
            git_branch: Some("feat/A1".to_string()),
            base_url_override: Some("https://api.anthropic.com/v1".to_string()),
            model_override: Some("claude-sonnet-4-6".to_string()),
            secret_ref: Some("SECRET_ANTHROPIC_A1".to_string()),
            effort: ReasoningEffort::Low,
            thinking_mode: ThinkingMode::Disabled,
            temperature: Some(0.2),
            max_output_tokens: Some(4096),
        });
        let a1 = reg.resolve(&AgentSlotId::AgentA1);
        assert_eq!(a1.base_url, "https://api.anthropic.com/v1");
        assert_eq!(a1.model, "claude-sonnet-4-6");
        assert_eq!(a1.effort, ReasoningEffort::Low);
        assert_eq!(a1.thinking_mode, ThinkingMode::Disabled);
        let wire = a1.build_wire_parameters();
        assert_eq!(wire["reasoning_effort"], "low");
        assert_eq!(wire["thinking"]["type"], "disabled");
    }

    #[test]
    fn feedback_loop_guard_trips_on_duplicate_error_or_budget_exhaustion() {
        let mut guard = FeedbackLoopGuard::default();
        assert_eq!(
            guard.record_and_check(10_000, "E0308:src/a.rs:42"),
            FeedbackDecision::ContinueIteration { next_round: 1 }
        );
        // Same error fingerprint on round 2 must immediately trip circuit breaker
        match guard.record_and_check(10_000, "E0308:src/a.rs:42") {
            FeedbackDecision::CircuitBreak { reason } => {
                assert!(reason.contains("E0308:src/a.rs:42"));
            }
            other => panic!("expected CircuitBreak, got {other:?}"),
        }
    }
}
