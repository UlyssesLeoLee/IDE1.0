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
use crate::plugins::{self, HotModuleKind};
use editcore::plugin_host::store::Installed;
use ide_agent_router::{
    AgentMemoryEngine, ContextCompactor, HookPipeline, LoopBreaker, McpConfig, McpManager,
    SkillRegistry, WorkspaceRulesEngine,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

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
    hooks_loaded_root: Option<PathBuf>,
    skills_loaded_root: Option<PathBuf>,
    loop_breaker: Option<LoopBreaker>,
    mcp: Option<McpManager>,
    mcp_loaded_root: Option<PathBuf>,
    installed_cache: Option<Vec<Installed>>,
}

impl AgentPluginRuntime {
    /// 是否存在任一核心 Agent 扩展插件处于激活态
    pub fn any_agent_plugin_active() -> bool {
        [
            HotModuleKind::Mcp,
            HotModuleKind::Skills,
            HotModuleKind::Mod,
            HotModuleKind::Hooks,
        ]
        .into_iter()
        .any(plugins::is_hot_module_active)
    }

    fn unload_disabled_engines(&mut self) {
        if !plugins::is_hot_module_active(HotModuleKind::Mcp) {
            self.reset_mcp();
        }
        if !plugins::is_hot_module_active(HotModuleKind::Skills) {
            self.skills = None;
            self.skills_loaded_root = None;
        }
        if !plugins::is_hot_module_active(HotModuleKind::Mod) {
            self.rules_engine = None;
        }
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            self.loop_breaker = None;
        }
        if !Self::any_agent_plugin_active() {
            self.memory_engine = None;
        }
    }

    /// 同步释放已被用户禁用或卸载的热插拔子模块与缓存（MCP / Skills / Mod / Hooks / Memory）
    pub fn unload_disabled_modules(&mut self) {
        self.unload_disabled_engines();
        self.installed_cache = None;
    }

    /// 强制热重载所有自定义与第三方 Hooks、Skills、MCP 配置（在用户新建/编辑/导入扩展后调用）
    pub fn reload_custom_extensions(&mut self) {
        self.reset_mcp();
        self.skills = None;
        self.skills_loaded_root = None;
        self.hooks = None;
        self.hooks_loaded_root = None;
        self.installed_cache = None;
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

    /// 刷新并返回已安装插件清单缓存（供单/多 Agent 轮次共享，避免每帧扫盘）
    pub fn refresh_installed_plugins(&mut self) -> &[Installed] {
        let list = editcore::plugin_host::store::list(&plugins::root());
        self.installed_cache.insert(list)
    }

    /// 获取已缓存的插件清单（仅在首次访问或缓存失效后读取磁盘一次）
    pub fn installed_plugins(&mut self) -> &[Installed] {
        self.installed_cache
            .get_or_insert_with(|| editcore::plugin_host::store::list(&plugins::root()))
    }

    /// 确保当前工作区与全局用户目录下的所有自定义及第三方 MCP 服务器配置已合并登记
    pub fn ensure_mcp_config(&mut self, ws_root: &Path) {
        let config_files = plugins::active_mcp_config_files();
        if config_files.is_empty() {
            self.reset_mcp();
            return;
        }
        if self.mcp_loaded_root.as_deref() == Some(ws_root) && self.mcp.is_some() {
            return;
        }
        let mgr = self.mcp.insert(McpManager::default());
        let user_root = plugins::root();
        // 1. 先加载用户全局自定义/第三方 MCP 配置 (<plugins_root>/mcp_servers.json, .mcp.json, custom_mcp/*.json)
        for rel in &config_files {
            let global_cand = user_root.join(rel);
            if let Some(cfg) = McpConfig::load_from_file(&global_cand) {
                mgr.load_config(cfg);
            }
        }
        if let Ok(rd) = std::fs::read_dir(user_root.join("custom_mcp")) {
            let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            files.sort();
            for p in files {
                if p.extension().is_some_and(|ext| ext == "json") {
                    if let Some(cfg) = McpConfig::load_from_file(&p) {
                        mgr.load_config(cfg);
                    }
                }
            }
        }
        // 2. 再合并加载当前工作区下的自定义/第三方 MCP 配置 (.mcp.json, mcp_servers.json, .cursor/mcp.json)
        for rel in &config_files {
            let candidate = ws_root.join(rel);
            if let Some(cfg) = McpConfig::load_from_file(&candidate) {
                mgr.load_config(cfg);
            }
        }
        let cursor_mcp = ws_root.join(".cursor/mcp.json");
        if let Some(cfg) = McpConfig::load_from_file(&cursor_mcp) {
            mgr.load_config(cfg);
        }
        self.mcp_loaded_root = Some(ws_root.to_path_buf());
    }

    /// 确保当前工作区与用户全局目录下的自定义及第三方 Hooks 已挂载
    pub fn ensure_hooks_config(&mut self, ws_root: &Path) {
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            return;
        }
        if self.hooks_loaded_root.as_deref() == Some(ws_root) && self.hooks.is_some() {
            return;
        }
        let mut pipeline = HookPipeline::default();
        let user_root = plugins::root();
        let _ = pipeline.load_workspace_and_user_hooks(ws_root, Some(&user_root));
        self.hooks = Some(pipeline);
        self.hooks_loaded_root = Some(ws_root.to_path_buf());
    }

    fn active_hook_pipeline(&mut self) -> &mut HookPipeline {
        if self.hooks.is_none() {
            let mut pipeline = HookPipeline::default();
            let user_root = plugins::root();
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let _ = pipeline.load_workspace_and_user_hooks(&cwd, Some(&user_root));
            self.hooks = Some(pipeline);
        }
        self.hooks.as_mut().expect("hooks initialized")
    }

    /// 确保当前工作区与用户全局目录下的自定义及第三方 Skills 已扫描加载
    pub fn ensure_skills_config(&mut self, ws_root: &Path) {
        if !plugins::is_hot_module_active(HotModuleKind::Skills) {
            self.skills = None;
            self.skills_loaded_root = None;
            return;
        }
        if self.skills_loaded_root.as_deref() == Some(ws_root) && self.skills.is_some() {
            return;
        }
        let mut reg = SkillRegistry::default();
        let user_root = plugins::root();
        reg.load_workspace_and_user_skills(ws_root, Some(&user_root));
        self.skills = Some(reg);
        self.skills_loaded_root = Some(ws_root.to_path_buf());
    }

    /// 克隆当前活跃的 MCP 管理器用于后台工作线程并发调用（若 `"mcp"` 未激活则返回 `None`）
    pub fn mcp_manager_cloned(&mut self) -> Option<McpManager> {
        if !plugins::is_hot_module_active(HotModuleKind::Mcp) {
            self.reset_mcp();
            return None;
        }
        Some(self.mcp.get_or_insert_with(McpManager::default).clone())
    }

    /// 在后台工作线程中异步派发 MCP 工具调用并统一封装返回值格式（消除单/多 Agent 重复样板）
    pub fn spawn_mcp_tool_call(
        &mut self,
        thread_prefix: &str,
        full_name: String,
        args: Value,
        on_done: impl FnOnce() + Send + 'static,
    ) -> Result<Receiver<Result<String, String>>, String> {
        let Some(mgr) = self.mcp_manager_cloned() else {
            return Err("MCP 插件模块已关闭或卸载".into());
        };
        let (tx, rx) = channel();
        let thread_name = format!("{}-{}", thread_prefix, full_name);
        std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let v = dispatch_mcp_tool_blocking(Some(&mgr), &full_name, args);
                let res = if let Some(err) = v["error"].as_str() {
                    Err(err.to_string())
                } else {
                    Ok(serde_json::to_string(&v).unwrap_or_else(|_| "{}".into()))
                };
                let _ = tx.send(res);
                on_done();
            })
            .map_err(|e| format!("无法启动 MCP 工具工作线程: {e}"))?;
        Ok(rx)
    }

    /// 执行 `PreInput` 生命周期钩子（仅当 `"hooks"` 插件激活时生效）
    pub fn run_pre_input_hook(&mut self, input: String) -> Result<String, String> {
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            return Ok(input);
        }
        self.active_hook_pipeline().execute_pre_input(input)
    }

    /// 执行 `PostResponse` 脱敏/后处理钩子（仅当 `"hooks"` 插件激活时生效）
    pub fn run_post_response_hook(&mut self, text: String) -> String {
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            return text;
        }
        self.active_hook_pipeline()
            .execute_post_response(text.clone())
            .unwrap_or(text)
    }

    fn run_tool_gate(
        hooks: &mut Option<HookPipeline>,
        breaker: &mut LoopBreaker,
        tool_name: &str,
        args: &Value,
    ) -> Result<(), String> {
        breaker.check_and_record(tool_name, args)?;
        if hooks.is_none() {
            let mut pipeline = HookPipeline::default();
            let user_root = plugins::root();
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let _ = pipeline.load_workspace_and_user_hooks(&cwd, Some(&user_root));
            *hooks = Some(pipeline);
        }
        hooks
            .as_ref()
            .expect("hooks initialized")
            .execute_pre_tool_call(tool_name, args)
    }

    /// 工具调用前统一闸门（使用调用方隔离的 `LoopBreaker` + `PreToolCall` 钩子；仅 `"hooks"` 插件激活时生效）
    pub fn check_tool_gate_with_breaker(
        &mut self,
        breaker: &mut LoopBreaker,
        tool_name: &str,
        args: &Value,
    ) -> Result<(), String> {
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            self.loop_breaker = None;
            return Ok(());
        }
        Self::run_tool_gate(&mut self.hooks, breaker, tool_name, args)
    }

    /// 工具调用前统一闸门（单 Agent 共用内部 `LoopBreaker`；仅 `"hooks"` 插件激活时生效）
    pub fn check_tool_gate(&mut self, tool_name: &str, args: &Value) -> Result<(), String> {
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            self.loop_breaker = None;
            return Ok(());
        }
        let breaker = self.loop_breaker.get_or_insert_with(LoopBreaker::default);
        Self::run_tool_gate(&mut self.hooks, breaker, tool_name, args)
    }

    /// 工具调用后执行 `PostToolCall` 脱敏与清洗钩子（仅当 `"hooks"` 插件激活时生效）
    pub fn run_post_tool_hook(&mut self, tool_name: &str, result: Value) -> Value {
        if !plugins::is_hot_module_active(HotModuleKind::Hooks) {
            self.hooks = None;
            self.hooks_loaded_root = None;
            return result;
        }
        let masked = self
            .active_hook_pipeline()
            .execute_post_tool_call(tool_name, result.to_string());
        serde_json::from_str(&masked).unwrap_or(result)
    }

    /// 记录工作记忆条目（仅在至少一个 Agent 扩展插件处于激活态时保留状态）
    pub fn record_working_item(&mut self, role: &str, summary: &str, tags: Vec<String>) {
        if !Self::any_agent_plugin_active() {
            self.memory_engine = None;
            return;
        }
        self.memory_engine
            .get_or_insert_with(AgentMemoryEngine::default)
            .record_working_item(role, summary, tags);
    }

    /// 先对原始用户输入匹配 Slash Skill，再将匹配后的正文交由调用方模板包装，避免模板前缀遮蔽 `/review` 等指令
    pub fn prepare_agent_turn_with<F>(
        &mut self,
        base_sys: &str,
        raw_prompt: &str,
        wrap_prompt: F,
        active_mod: &str,
        ws_root: &Path,
        extra_sys: Option<&str>,
        installed_plugins: &[Installed],
    ) -> PreparedAgentTurn
    where
        F: FnOnce(&str) -> String,
    {
        self.unload_disabled_engines();
        self.ensure_hooks_config(ws_root);

        let (matched_skill, effective_prompt) =
            if plugins::is_hot_module_active(HotModuleKind::Skills) {
                self.ensure_skills_config(ws_root);
                let registry = self.skills.get_or_insert_with(SkillRegistry::default);
                if let Some((skill, rest)) = registry.match_prompt(raw_prompt) {
                    (Some(skill.clone()), rest.to_string())
                } else {
                    (None, raw_prompt.to_string())
                }
            } else {
                (None, raw_prompt.to_string())
            };

        let wrapped_prompt = wrap_prompt(&effective_prompt);

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

        if Self::any_agent_plugin_active() {
            if let Some(mem_prompt) = self
                .memory_engine
                .get_or_insert_with(AgentMemoryEngine::default)
                .format_memory_prompt(ws_root)
            {
                sys.push_str("\n\n");
                sys.push_str(&mem_prompt);
            }
        } else {
            self.memory_engine = None;
        }

        if let Some(extra) = extra_sys {
            if !extra.is_empty() {
                sys.push_str("\n\n");
                sys.push_str(extra);
            }
        }

        let (final_sys, final_q) = if let Some(skill) = matched_skill {
            skill.apply(&wrapped_prompt, Some(&sys))
        } else {
            (sys, wrapped_prompt)
        };

        let mut tools = ai::tool_table(installed_plugins);
        if plugins::is_hot_module_active(HotModuleKind::Mcp) {
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
        self.prepare_agent_turn_with(
            base_sys,
            raw_prompt,
            |p| p.to_string(),
            active_mod,
            ws_root,
            extra_sys,
            installed_plugins,
        )
    }

    /// 组装紧凑的对话消息数组（含自适应上下文压缩）
    pub fn build_compacted_messages(system_prompt: String, user_prompt: &str) -> Vec<Value> {
        let mut msgs = ai::base_messages(system_prompt, &[], user_prompt);
        ContextCompactor::compact_messages(&mut msgs, 8192, 4);
        msgs
    }
}

/// 解析工具返回的行级补丁提案 (`kind == "patch_proposal"`)
pub fn parse_patch_proposal(result: &Value) -> Option<(usize, usize, Vec<String>, String)> {
    if result.get("kind").and_then(Value::as_str) != Some("patch_proposal")
        || result["error"].as_str().is_some()
    {
        return None;
    }
    let start = result["start"].as_u64().unwrap_or(0) as usize;
    let remove = result["remove"].as_u64().unwrap_or(0) as usize;
    let insert: Vec<String> = result["insert"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let reason = result["reason"].as_str().unwrap_or("").to_string();
    Some((start, remove, insert, reason))
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

    struct PluginDirGuard {
        dir: PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl PluginDirGuard {
        fn new(tag: &str) -> Self {
            let lock = crate::i18n::TEST_LOCK
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let dir = std::env::temp_dir().join(format!(
                "ide_agent_plugins_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var("IDE_PLUGIN_DIR", &dir);
            Self { dir, _lock: lock }
        }
    }

    impl Drop for PluginDirGuard {
        fn drop(&mut self) {
            std::env::remove_var("IDE_PLUGIN_DIR");
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn dispatch_mcp_tool_without_manager_fails_closed() {
        let out = dispatch_mcp_tool_blocking(None, "mcp_git_status", json!({}));
        assert!(out["error"].as_str().is_some_and(|e| e.contains("MCP")));
    }

    #[test]
    fn plugin_runtime_unloads_all_subengines_when_plugins_disabled() {
        let _guard = PluginDirGuard::new("unload_all");
        let mut rt = AgentPluginRuntime::default();
        let _ = rt.refresh_installed_plugins();
        let _ = rt.prepare_agent_turn("BASE", "/review code", "code", Path::new("."), None, &[]);
        rt.ensure_mcp_config(Path::new("."));
        rt.record_working_item("user", "work mem", vec!["t".into()]);
        let _ = rt.check_tool_gate("read_file", &json!({"start": 0}));

        assert!(rt.skills.is_some());
        assert!(rt.rules_engine.is_some());
        assert!(rt.memory_engine.is_some());
        assert!(rt.hooks.is_some());
        assert!(rt.loop_breaker.is_some());
        assert!(rt.mcp.is_some());
        assert!(rt.mcp_loaded_root.is_some());
        assert!(rt.installed_cache.is_some());
        assert!(!plugins::active_mcp_config_files().is_empty());

        for kind in [
            HotModuleKind::Mcp,
            HotModuleKind::Skills,
            HotModuleKind::Mod,
            HotModuleKind::Hooks,
        ] {
            crate::plugins::set_enabled(kind.plugin_id(), false).unwrap();
        }
        assert!(plugins::active_mcp_config_files().is_empty());

        rt.unload_disabled_modules();
        assert!(rt.skills.is_none());
        assert!(rt.rules_engine.is_none());
        assert!(rt.memory_engine.is_none());
        assert!(rt.hooks.is_none());
        assert!(rt.loop_breaker.is_none());
        assert!(rt.mcp.is_none());
        assert!(rt.mcp_loaded_root.is_none());
        assert!(rt.installed_cache.is_none());
    }

    #[test]
    fn prepare_agent_turn_with_matches_skill_before_wrapping() {
        let _guard = PluginDirGuard::new("skills_wrap");
        let mut rt = AgentPluginRuntime::default();
        let turn = rt.prepare_agent_turn_with(
            "BASE_SYS",
            "/review 检查并发",
            |effective| {
                format!(
                    "【多 Agent 协同统筹命令】: {}\n你的身份是: Agent-1",
                    effective
                )
            },
            "code",
            Path::new("."),
            None,
            &[],
        );
        assert!(
            turn.system_prompt.contains("[Active Skill:"),
            "应在包装模板前缀之前完成 /review 技能匹配: {}",
            turn.system_prompt
        );
        assert!(
            turn.user_prompt
                .contains("【多 Agent 协同统筹命令】: 检查并发"),
            "包装后的用户消息应保留剥离触发词后的真实任务: {}",
            turn.user_prompt
        );
        assert!(
            !turn.user_prompt.contains("/review"),
            "触发词 /review 应已被 Skill 引擎剥离: {}",
            turn.user_prompt
        );
    }
}
