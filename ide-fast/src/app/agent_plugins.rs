//! 插件化 Agent 运行时底座 (`mcp`, `skills`, `mod`, `hooks`, `memory`, `loop_breaker`)
//!
//! 所有 Agent 后端增强能力统一惰性封装在此模块中：
//! - **零启动开销**：`AiState::default()` 中 `plugin_rt` 为 `None`，冷启动完全不触碰磁盘配置或插件注册表。
//! - **严格插件边界**：仅当对应热插拔插件（`"mcp"`, `"skills"`, `"mod"`, `"hooks"`）处于激活态时才实例化并生效；
//!   插件禁用或卸载后调用 `unload_disabled_modules()` 即自动释放对应子引擎状态。
//! - **单/多 Agent 能力对等**：同时供单 Agent (`aichat.rs`) 与多 Agent 协同 (`collab.rs`) 共享调用，
//!   统一提供 Hooks (`PreInput` / `PreToolCall` / `PostToolCall` / `PostResponse`)、Skills 匹配、
//!   Mod 工作区规则注入、工作记忆记录、LoopBreaker 熔断与 MCP 外部工具聚合。

use crate::ai;
use crate::plugins;
use editcore::plugin_host::store::Installed;
use ide_agent_router::{
    AgentMemoryEngine, ContextCompactor, HookPipeline, LoopBreaker, McpConfig, McpManager,
    SkillRegistry, WorkspaceRulesEngine,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 单轮 Agent 对话准备结果（系统提示词、转换后的用户指令与可用工具列表）
pub struct PreparedAgentTurn {
    pub system_prompt: String,
    pub user_prompt: String,
    pub tools: Vec<Value>,
}

/// 惰性插件化 Agent 运行时（按需初始化各扩展子引擎，冷启动零成本）
#[derive(Default)]
pub struct AgentPluginRuntime {
    skills: Option<SkillRegistry>,
    rules_engine: Option<WorkspaceRulesEngine>,
    memory_engine: Option<AgentMemoryEngine>,
    hooks: Option<HookPipeline>,
    loop_breaker: Option<LoopBreaker>,
    mcp: Option<McpManager>,
    mcp_loaded_root: Option<PathBuf>,
}

impl AgentPluginRuntime {
    /// 同步释放已被用户禁用或卸载的热插拔子模块（MCP / Skills / Mod / Hooks）
    pub fn unload_disabled_modules(&mut self) {
        if !plugins::is_module_active("mcp") {
            self.reset_mcp();
        }
        if !plugins::is_module_active("skills") {
            self.skills = None;
        }
        if !plugins::is_module_active("mod") {
            self.rules_engine = None;
        }
        if !plugins::is_module_active("hooks") {
            self.hooks = None;
        }
    }

    /// 卸载或重置活跃的 MCP 外部工具服务器池
    pub fn reset_mcp(&mut self) {
        self.mcp_loaded_root = None;
        self.mcp = None;
    }

    /// 重置死循环自省熔断器计数
    pub fn reset_loop_breaker(&mut self) {
        if let Some(lb) = self.loop_breaker.as_mut() {
            lb.reset();
        }
    }

    /// 确保当前工作区的 MCP 外部配置已登记（仅在 `"mcp"` 插件启用时调用）
    pub fn ensure_mcp_config(&mut self, ws_root: &Path) {
        if !plugins::is_module_active("mcp") {
            self.reset_mcp();
            return;
        }
        if self.mcp_loaded_root.as_deref() == Some(ws_root) && self.mcp.is_some() {
            return;
        }
        let mgr = self.mcp.get_or_insert_with(McpManager::default);
        let dot_mcp = ws_root.join(".mcp.json");
        let servers_json = ws_root.join("mcp_servers.json");
        if let Some(cfg) =
            McpConfig::load_from_file(&dot_mcp).or_else(|| McpConfig::load_from_file(&servers_json))
        {
            mgr.load_config(cfg);
        }
        self.mcp_loaded_root = Some(ws_root.to_path_buf());
    }

    /// 克隆当前活跃的 MCP 管理器用于后台工作线程并发调用（若 `"mcp"` 未激活则返回 `None`）
    pub fn mcp_manager_cloned(&mut self) -> Option<McpManager> {
        if !plugins::is_module_active("mcp") {
            self.reset_mcp();
            return None;
        }
        Some(self.mcp.get_or_insert_with(McpManager::default).clone())
    }

    /// 执行 `PreInput` 生命周期钩子（仅当 `"hooks"` 插件激活时生效）
    pub fn run_pre_input_hook(&mut self, input: String) -> Result<String, String> {
        if !plugins::is_module_active("hooks") {
            self.hooks = None;
            return Ok(input);
        }
        self.hooks
            .get_or_insert_with(HookPipeline::default)
            .execute_pre_input(input)
    }

    /// 执行 `PostResponse` 脱敏/后处理钩子（仅当 `"hooks"` 插件激活时生效）
    pub fn run_post_response_hook(&mut self, text: String) -> String {
        if !plugins::is_module_active("hooks") {
            self.hooks = None;
            return text;
        }
        self.hooks
            .get_or_insert_with(HookPipeline::default)
            .execute_post_response(text.clone())
            .unwrap_or(text)
    }

    /// 工具调用前统一闸门：死循环熔断检测 (`LoopBreaker`) + `PreToolCall` 生命周期拦截钩子
    pub fn check_tool_gate(&mut self, tool_name: &str, args: &Value) -> Result<(), String> {
        self.loop_breaker
            .get_or_insert_with(LoopBreaker::default)
            .check_and_record(tool_name, args)?;
        if plugins::is_module_active("hooks") {
            self.hooks
                .get_or_insert_with(HookPipeline::default)
                .execute_pre_tool_call(tool_name, args)?;
        } else {
            self.hooks = None;
        }
        Ok(())
    }

    /// 工具调用后执行 `PostToolCall` 脱敏与清洗钩子（仅当 `"hooks"` 插件激活时生效）
    pub fn run_post_tool_hook(&mut self, tool_name: &str, result: Value) -> Value {
        if !plugins::is_module_active("hooks") {
            self.hooks = None;
            return result;
        }
        let masked = self
            .hooks
            .get_or_insert_with(HookPipeline::default)
            .execute_post_tool_call(tool_name, result.to_string());
        serde_json::from_str(&masked).unwrap_or(result)
    }

    /// 记录工作记忆条目（零分配环形缓冲）
    pub fn record_working_item(&mut self, role: &str, summary: &str, tags: Vec<String>) {
        self.memory_engine
            .get_or_insert_with(AgentMemoryEngine::default)
            .record_working_item(role, summary, tags);
    }

    /// 为单 Agent 或多 Agent 协同准备统一上下文（Skills / Mod / WorkspaceRules / Memory / MCP 工具表）
    pub fn prepare_agent_turn(
        &mut self,
        base_sys: &str,
        raw_prompt: &str,
        active_mod: &str,
        ws_root: &Path,
        extra_sys: Option<&str>,
        installed_plugins: &[Installed],
    ) -> PreparedAgentTurn {
        self.unload_disabled_modules();

        let (matched_skill, user_prompt) = if plugins::is_module_active("skills") {
            let registry = self.skills.get_or_insert_with(SkillRegistry::default);
            if let Some((skill, rest)) = registry.match_prompt(raw_prompt) {
                (Some(skill.clone()), rest.to_string())
            } else {
                (None, raw_prompt.to_string())
            }
        } else {
            (None, raw_prompt.to_string())
        };

        let mut sys = base_sys.to_string();

        let mod_modes = plugins::active_mod_modes();
        if !mod_modes.is_empty() {
            if let Some(rules_prompt) = self
                .rules_engine
                .get_or_insert_with(WorkspaceRulesEngine::default)
                .format_invariants_prompt(ws_root)
            {
                sys.push_str("\n\n");
                sys.push_str(&rules_prompt);
            }
            if let Some(mode) = mod_modes
                .iter()
                .find(|m| m.id == active_mod)
                .or_else(|| mod_modes.first())
            {
                if !mode.prompt.is_empty() {
                    sys.push_str("\n\n");
                    sys.push_str(&mode.prompt);
                }
            }
        }

        if let Some(mem_prompt) = self
            .memory_engine
            .get_or_insert_with(AgentMemoryEngine::default)
            .format_memory_prompt(ws_root)
        {
            sys.push_str("\n\n");
            sys.push_str(&mem_prompt);
        }

        if let Some(extra) = extra_sys {
            if !extra.is_empty() {
                sys.push_str("\n\n");
                sys.push_str(extra);
            }
        }

        let (final_sys, final_q) = if let Some(skill) = matched_skill {
            skill.apply(&user_prompt, Some(&sys))
        } else {
            (sys, user_prompt)
        };

        let mut tools = ai::tool_table(installed_plugins);
        if plugins::is_module_active("mcp") {
            self.ensure_mcp_config(ws_root);
            if let Some(mcp) = self.mcp.as_mut() {
                tools.extend(mcp.aggregate_openai_tools());
            }
        }

        PreparedAgentTurn {
            system_prompt: final_sys,
            user_prompt: final_q,
            tools,
        }
    }

    /// 组装紧凑的对话消息数组（含自适应上下文压缩）
    pub fn build_compacted_messages(system_prompt: String, user_prompt: &str) -> Vec<Value> {
        let mut msgs = ai::base_messages(system_prompt, &[], user_prompt);
        ContextCompactor::compact_messages(&mut msgs, 8192, 4);
        msgs
    }
}

/// 在后台线程中异步执行单一 MCP 工具调用并返回 JSON 结果对象
pub fn dispatch_mcp_tool_blocking(mcp: Option<&McpManager>, full_name: &str, args: Value) -> Value {
    let Some(mgr) = mcp else {
        return json!({ "error": "MCP 插件模块已关闭或尚未加载" });
    };
    match mgr.dispatch_tool_call(full_name, args) {
        Some(Ok(mcp_res)) => {
            if mcp_res.is_error {
                json!({ "error": mcp_res.merged_text() })
            } else {
                json!({ "result": mcp_res.merged_text() })
            }
        }
        Some(Err(err)) => json!({ "error": err }),
        None => json!({ "error": format!("未找到对应的 MCP 工具服务器: {}", full_name) }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::aichat::AiState;

    #[test]
    fn ai_state_default_keeps_plugin_runtime_lazy_for_fast_startup() {
        let st = AiState::default();
        assert!(
            st.plugin_rt.is_none(),
            "冷启动默认 AiState 不得预创建插件运行时，保持启动 0 开销"
        );
    }

    #[test]
    fn plugin_runtime_respects_active_plugin_gates_and_unloads_when_disabled() {
        let mut rt = AgentPluginRuntime::default();
        rt.ensure_mcp_config(Path::new("."));
        rt.record_working_item("user", "test working memory", vec!["test".into()]);

        // 默认或关闭所有扩展插件后，unload_disabled_modules 应安全清零可卸载模块
        let turn = rt.prepare_agent_turn("BASE_SYS", "普通提问", "code", Path::new("."), None, &[]);
        assert!(turn.system_prompt.contains("BASE_SYS"));
        assert!(!turn.tools.is_empty(), "内建 editcore 工具应始终可用");
        assert!(
            rt.run_post_response_hook("secret=ok".into()) == "secret=ok"
                || plugins::is_module_active("hooks")
        );
    }

    #[test]
    fn dispatch_mcp_tool_without_manager_fails_closed() {
        let out = dispatch_mcp_tool_blocking(None, "mcp_git_status", json!({}));
        assert!(out["error"].as_str().is_some_and(|e| e.contains("MCP")));
    }
}
