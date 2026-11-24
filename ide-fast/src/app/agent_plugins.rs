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
    AgentMemoryEngine, ContextCompactor, CustomHookSpec, CustomHooksConfig, HookInfo, HookPipeline,
    LoopBreaker, McpConfig, McpManager, McpServerConfig, Skill, SkillRegistry,
    WorkspaceRulesEngine,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// 创建或更新自定义 Skill 时的结构化入参（消除零散多参数传递）
#[derive(Clone, Copy, Debug)]
pub struct CustomSkillDraft<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub trigger: &'a str,
    pub description: &'a str,
    pub system_prompt: &'a str,
}

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

    fn ensure_default_hooks<'a>(
        hooks: &'a mut Option<HookPipeline>,
        loaded_root: Option<&Path>,
    ) -> &'a mut HookPipeline {
        if hooks.is_none() {
            let mut pipeline = HookPipeline::default();
            let user_root = plugins::root();
            if let Some(ws) = loaded_root {
                let _ = pipeline.load_workspace_and_user_hooks(ws, Some(&user_root));
            } else {
                let _ = pipeline.load_custom_hooks_from_file(&user_root.join("custom_hooks.json"));
                let _ = pipeline.load_custom_hooks_from_dir(&user_root.join("custom_hooks"));
            }
            *hooks = Some(pipeline);
        }
        hooks.as_mut().expect("hooks initialized")
    }

    fn active_hook_pipeline(&mut self) -> &mut HookPipeline {
        Self::ensure_default_hooks(&mut self.hooks, self.hooks_loaded_root.as_deref())
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

    /// 列出当前工作区与全局已生效的全部 Skills（内建 + 自定义 + 第三方）
    pub fn list_active_skills(&mut self, ws_root: &Path) -> Vec<Skill> {
        self.ensure_skills_config(ws_root);
        self.skills
            .as_ref()
            .map(|r| r.list().into_iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 列出当前工作区与全局已生效的全部 Hooks 名称（内建 + 自定义 + 第三方）
    pub fn list_active_hooks(&mut self, ws_root: &Path) -> Vec<String> {
        self.ensure_hooks_config(ws_root);
        self.hooks
            .as_ref()
            .map(|h| h.hook_names())
            .unwrap_or_default()
    }

    /// 列出当前工作区与全局已生效的全部 Hooks 元数据（含描述与是否内建标记）
    pub fn list_active_hook_infos(&mut self, ws_root: &Path) -> Vec<HookInfo> {
        self.ensure_hooks_config(ws_root);
        self.hooks
            .as_ref()
            .map(|h| h.hook_infos())
            .unwrap_or_default()
    }

    /// 列出当前工作区与全局合并后的全部 MCP 服务器配置
    pub fn list_active_mcp_servers(&mut self, ws_root: &Path) -> Vec<(String, McpServerConfig)> {
        self.ensure_mcp_config(ws_root);
        self.mcp
            .as_ref()
            .map(|m| m.list_server_configs())
            .unwrap_or_default()
    }

    /// 一键添加并持久化一条自定义或第三方 Skill 到 `<ws_root>/.skills/<id>/SKILL.md` 并立即热重载
    pub fn add_custom_skill(
        &mut self,
        ws_root: &Path,
        id: &str,
        name: &str,
        trigger: &str,
        description: &str,
        system_prompt: &str,
    ) -> Result<PathBuf, String> {
        self.add_custom_skill_spec(
            ws_root,
            CustomSkillDraft {
                id,
                name,
                trigger,
                description,
                system_prompt,
            },
        )
    }

    /// 按结构化草稿添加并持久化自定义或第三方 Skill
    pub fn add_custom_skill_spec(
        &mut self,
        ws_root: &Path,
        draft: CustomSkillDraft<'_>,
    ) -> Result<PathBuf, String> {
        let clean_id: String = draft
            .id
            .trim()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let clean_id = clean_id.trim_matches('-').to_string();
        if clean_id.is_empty() {
            return Err("技能 ID 不能为空（请使用字母、数字或连字符，如 my-skill）".into());
        }
        let trig = draft.trigger.trim();
        let final_trig = if trig.is_empty() {
            format!("/{clean_id}")
        } else if trig.starts_with('/') {
            trig.to_string()
        } else {
            format!("/{trig}")
        };
        let final_name = if draft.name.trim().is_empty() {
            clean_id.clone()
        } else {
            draft.name.trim().to_string()
        };
        let final_desc = if draft.description.trim().is_empty() {
            "用户自定义/第三方扩展技能".to_string()
        } else {
            draft.description.trim().to_string()
        };
        let final_prompt = if draft.system_prompt.trim().is_empty() {
            "你是一位资深系统架构师与代码专家，请按高标准完成用户请求。".to_string()
        } else {
            draft.system_prompt.trim().to_string()
        };

        let skill_dir = ws_root.join(".skills").join(&clean_id);
        std::fs::create_dir_all(&skill_dir).map_err(|e| format!("创建技能目录失败: {e}"))?;
        let target = skill_dir.join("SKILL.md");
        let content = format!(
            "---\nname: {final_name}\ndescription: {final_desc}\ntrigger: {final_trig}\ntags: custom, extension\n---\n\n{final_prompt}\n"
        );
        std::fs::write(&target, content).map_err(|e| format!("写入 SKILL.md 失败: {e}"))?;
        self.reload_custom_extensions();
        self.ensure_skills_config(ws_root);
        Ok(target)
    }

    /// 删除一条自定义或第三方 Skill 文件/目录并立即热重载
    pub fn remove_custom_skill(&mut self, ws_root: &Path, id: &str) -> Result<(), String> {
        let user_root = plugins::root();
        let candidates = [
            ws_root.join(".skills").join(id),
            ws_root.join(".skills").join(format!("{id}.md")),
            ws_root.join(".skills").join(format!("{id}.json")),
            ws_root.join(".claude/skills").join(id),
            ws_root.join(".claude/commands").join(format!("{id}.md")),
            ws_root.join(".agents/skills").join(id),
            ws_root.join("skills").join(id),
            user_root.join("custom_skills").join(id),
            user_root.join("custom_skills").join(format!("{id}.md")),
            user_root.join("custom_skills").join(format!("{id}.json")),
        ];
        let mut removed = false;
        for p in candidates {
            if p.is_dir() {
                std::fs::remove_dir_all(&p)
                    .map_err(|e| format!("删除技能目录 {} 失败: {e}", p.display()))?;
                removed = true;
            } else if p.is_file() {
                std::fs::remove_file(&p)
                    .map_err(|e| format!("删除技能文件 {} 失败: {e}", p.display()))?;
                removed = true;
            }
        }
        if !removed {
            return Err(format!("未找到可删除的自定义技能文件: {id}"));
        }
        self.reload_custom_extensions();
        self.ensure_skills_config(ws_root);
        Ok(())
    }

    /// 一键添加并持久化一条自定义或第三方 Hook 规则到 `<ws_root>/.hooks.json` 并立即热重载
    pub fn add_custom_hook(
        &mut self,
        ws_root: &Path,
        spec: CustomHookSpec,
    ) -> Result<PathBuf, String> {
        let name = spec.name.trim().to_string();
        if name.is_empty() {
            return Err("Hook 名称不能为空".into());
        }
        let target = ws_root.join(".hooks.json");
        let mut cfg: CustomHooksConfig = if target.exists() {
            let raw = std::fs::read_to_string(&target)
                .map_err(|e| format!("读取现有配置 {} 失败: {e}", target.display()))?;
            serde_json::from_str(&raw).map_err(|e| {
                format!(
                    "解析现有配置文件 {} 失败，已拒绝覆盖以防数据损坏: {e}",
                    target.display()
                )
            })?
        } else {
            CustomHooksConfig::default()
        };
        cfg.hooks.retain(|h| h.name != name);
        let mut clean_spec = spec;
        clean_spec.name = name;
        clean_spec.enabled = true;
        cfg.hooks.push(clean_spec);
        let pretty =
            serde_json::to_string_pretty(&cfg).map_err(|e| format!("序列化 Hook 失败: {e}"))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录 {} 失败: {e}", parent.display()))?;
        }
        std::fs::write(&target, pretty).map_err(|e| format!("写入 .hooks.json 失败: {e}"))?;
        self.reload_custom_extensions();
        self.ensure_hooks_config(ws_root);
        Ok(target)
    }

    /// 删除指定名称的自定义或第三方 Hook 并立即热重载
    pub fn remove_custom_hook(&mut self, ws_root: &Path, hook_name: &str) -> Result<(), String> {
        let user_root = plugins::root();
        let files = [
            ws_root.join(".hooks.json"),
            ws_root.join("hooks.json"),
            ws_root.join(".ide-hooks.json"),
            user_root.join("custom_hooks.json"),
        ];
        let mut changed = false;
        for f in files {
            if let Ok(raw) = std::fs::read_to_string(&f) {
                if let Ok(mut cfg) = serde_json::from_str::<CustomHooksConfig>(&raw) {
                    let before = cfg.hooks.len();
                    cfg.hooks.retain(|h| h.name != hook_name);
                    if cfg.hooks.len() < before {
                        let pretty = serde_json::to_string_pretty(&cfg)
                            .map_err(|e| format!("序列化 Hook 配置失败: {e}"))?;
                        std::fs::write(&f, pretty)
                            .map_err(|e| format!("更新 {} 失败: {e}", f.display()))?;
                        changed = true;
                    }
                }
            }
        }
        for dir in [
            ws_root.join(".hooks"),
            ws_root.join(".claude/hooks"),
            user_root.join("custom_hooks"),
        ] {
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_file() && p.extension().is_some_and(|ext| ext == "json") {
                        if let Ok(raw) = std::fs::read_to_string(&p) {
                            if let Ok(mut cfg) = serde_json::from_str::<CustomHooksConfig>(&raw) {
                                let before = cfg.hooks.len();
                                cfg.hooks.retain(|h| h.name != hook_name);
                                if cfg.hooks.len() < before {
                                    if cfg.hooks.is_empty() {
                                        std::fs::remove_file(&p).map_err(|e| {
                                            format!("删除 {} 失败: {e}", p.display())
                                        })?;
                                    } else {
                                        let pretty = serde_json::to_string_pretty(&cfg)
                                            .map_err(|e| format!("序列化 Hook 失败: {e}"))?;
                                        std::fs::write(&p, pretty).map_err(|e| {
                                            format!("写入 {} 失败: {e}", p.display())
                                        })?;
                                    }
                                    changed = true;
                                }
                            } else if let Ok(single) = serde_json::from_str::<CustomHookSpec>(&raw)
                            {
                                if single.name == hook_name {
                                    std::fs::remove_file(&p)
                                        .map_err(|e| format!("删除 {} 失败: {e}", p.display()))?;
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        if !changed {
            return Err(format!("未找到可删除的自定义 Hook: {hook_name}"));
        }
        self.reload_custom_extensions();
        self.ensure_hooks_config(ws_root);
        Ok(())
    }

    /// 一键添加并持久化一条自定义或第三方 MCP Server 配置到 `<ws_root>/.mcp.json` 并立即热重载
    pub fn add_custom_mcp_server(
        &mut self,
        ws_root: &Path,
        server_id: &str,
        command: &str,
        args: Vec<String>,
    ) -> Result<PathBuf, String> {
        self.add_custom_mcp_server_with_env(ws_root, server_id, command, args, Default::default())
    }

    /// 一键添加并持久化带有环境变量 (`env`) 的自定义或第三方 MCP Server 配置到 `<ws_root>/.mcp.json` 并立即热重载
    pub fn add_custom_mcp_server_with_env(
        &mut self,
        ws_root: &Path,
        server_id: &str,
        command: &str,
        args: Vec<String>,
        env: std::collections::HashMap<String, String>,
    ) -> Result<PathBuf, String> {
        let id = server_id.trim();
        let cmd = command.trim();
        if id.is_empty() {
            return Err("MCP 服务名称 (Server ID) 不能为空".into());
        }
        if cmd.is_empty() {
            return Err(
                "MCP 启动命令 (Command) 不能为空，例如 npx、uvx、node 或可执行文件路径".into(),
            );
        }
        let target = ws_root.join(".mcp.json");
        let mut cfg = if target.exists() {
            let raw = std::fs::read_to_string(&target)
                .map_err(|e| format!("读取现有配置 {} 失败: {e}", target.display()))?;
            McpConfig::from_json_str(&raw).map_err(|e| {
                format!(
                    "解析现有配置文件 {} 失败，已拒绝覆盖以防数据损坏: {e}",
                    target.display()
                )
            })?
        } else {
            McpConfig::default()
        };
        cfg.mcp_servers.insert(
            id.to_string(),
            McpServerConfig {
                command: cmd.to_string(),
                args,
                env,
                disabled: false,
            },
        );
        let pretty =
            serde_json::to_string_pretty(&cfg).map_err(|e| format!("序列化 MCP 配置失败: {e}"))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建配置目录 {} 失败: {e}", parent.display()))?;
        }
        std::fs::write(&target, pretty).map_err(|e| format!("写入 .mcp.json 失败: {e}"))?;
        self.reload_custom_extensions();
        self.ensure_mcp_config(ws_root);
        Ok(target)
    }

    /// 删除指定 ID 的自定义或第三方 MCP Server 配置并立即热重载
    pub fn remove_custom_mcp_server(
        &mut self,
        ws_root: &Path,
        server_id: &str,
    ) -> Result<(), String> {
        let user_root = plugins::root();
        let files = [
            ws_root.join(".mcp.json"),
            ws_root.join("mcp_servers.json"),
            ws_root.join(".cursor/mcp.json"),
            user_root.join("mcp_servers.json"),
            user_root.join(".mcp.json"),
        ];
        let mut changed = false;
        for f in files {
            if let Some(mut cfg) = McpConfig::load_from_file(&f) {
                if cfg.mcp_servers.remove(server_id).is_some() {
                    let pretty = serde_json::to_string_pretty(&cfg)
                        .map_err(|e| format!("序列化 MCP 配置失败: {e}"))?;
                    std::fs::write(&f, pretty)
                        .map_err(|e| format!("更新 {} 失败: {e}", f.display()))?;
                    changed = true;
                }
            }
        }
        if let Ok(rd) = std::fs::read_dir(user_root.join("custom_mcp")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file() && p.extension().is_some_and(|ext| ext == "json") {
                    if let Some(mut cfg) = McpConfig::load_from_file(&p) {
                        if cfg.mcp_servers.remove(server_id).is_some() {
                            if cfg.mcp_servers.is_empty() {
                                std::fs::remove_file(&p)
                                    .map_err(|e| format!("删除 {} 失败: {e}", p.display()))?;
                            } else {
                                let pretty = serde_json::to_string_pretty(&cfg)
                                    .map_err(|e| format!("序列化 MCP 配置失败: {e}"))?;
                                std::fs::write(&p, pretty)
                                    .map_err(|e| format!("写入 {} 失败: {e}", p.display()))?;
                            }
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            return Err(format!("未找到可删除的 MCP 服务配置: {server_id}"));
        }
        self.reload_custom_extensions();
        self.ensure_mcp_config(ws_root);
        Ok(())
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
        hooks_loaded_root: Option<&Path>,
        breaker: &mut LoopBreaker,
        tool_name: &str,
        args: &Value,
    ) -> Result<(), String> {
        breaker.check_and_record(tool_name, args)?;
        Self::ensure_default_hooks(hooks, hooks_loaded_root).execute_pre_tool_call(tool_name, args)
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
        Self::run_tool_gate(
            &mut self.hooks,
            self.hooks_loaded_root.as_deref(),
            breaker,
            tool_name,
            args,
        )
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
        Self::run_tool_gate(
            &mut self.hooks,
            self.hooks_loaded_root.as_deref(),
            breaker,
            tool_name,
            args,
        )
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
                if let Some((skill, combined)) = registry.match_prompt_with_prefix(raw_prompt) {
                    (Some(skill.clone()), combined)
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
                tools.extend(mcp.aggregate_openai_tools_nonblocking());
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

    #[test]
    fn custom_skills_hooks_and_mcp_crud_work_end_to_end() {
        let guard = PluginDirGuard::new("custom_crud_e2e");
        let ws = guard.dir.join("workspace");
        std::fs::create_dir_all(&ws).unwrap();

        let mut rt = AgentPluginRuntime::default();

        // 1. 添加自定义 Skill -> 列表可见 -> /trigger 真实生效 -> 删除后移除
        let skill_path = rt
            .add_custom_skill(
                &ws,
                "sec-audit",
                "安全审计技能",
                "/sec-audit",
                "自定义安全审计",
                "请严格输出 [SEC_AUDIT_SKILL_PROMPT]",
            )
            .unwrap();
        assert!(skill_path.is_file());
        let skills = rt.list_active_skills(&ws);
        assert!(skills.iter().any(|s| s.id == "sec-audit" && !s.is_builtin));
        let turn = rt.prepare_agent_turn("BASE", "/sec-audit 检查鉴权", "code", &ws, None, &[]);
        assert!(turn.system_prompt.contains("[SEC_AUDIT_SKILL_PROMPT]"));
        assert_eq!(turn.user_prompt, "检查鉴权");
        rt.remove_custom_skill(&ws, "sec-audit").unwrap();
        assert!(!rt
            .list_active_skills(&ws)
            .iter()
            .any(|s| s.id == "sec-audit"));

        // 2. 添加自定义 Hook -> 列表可见 -> PreInput/PreTool/PostResponse 真实生效 -> 删除后移除
        let hook_path = rt
            .add_custom_hook(
                &ws,
                CustomHookSpec {
                    name: "TeamGuardHook".into(),
                    description: "团队安全门禁".into(),
                    enabled: true,
                    pre_input_prefix: Some("[TEAM_GUARD]".into()),
                    pre_input_block_patterns: vec!["BLOCK_ME".into()],
                    blocked_tools: vec!["danger_tool".into()],
                    pre_tool_block_patterns: vec!["rm -rf /".into()],
                    mask_patterns: vec!["teamtok-".into()],
                },
            )
            .unwrap();
        assert!(hook_path.is_file());
        assert!(rt
            .list_active_hooks(&ws)
            .iter()
            .any(|h| h == "TeamGuardHook"));
        let pre = rt.run_pre_input_hook("hello".into()).unwrap();
        assert!(pre.starts_with("[TEAM_GUARD]\nhello"));
        assert!(rt.run_pre_input_hook("please BLOCK_ME".into()).is_err());
        assert!(rt.check_tool_gate("danger_tool", &json!({})).is_err());
        assert!(rt
            .check_tool_gate("exec", &json!({"cmd": "rm -rf /"}))
            .is_err());
        let post = rt.run_post_response_hook("key=teamtok-12345678".into());
        assert!(post.contains("teamtok-****"));
        assert!(!post.contains("12345678"));
        rt.remove_custom_hook(&ws, "TeamGuardHook").unwrap();
        assert!(!rt
            .list_active_hooks(&ws)
            .iter()
            .any(|h| h == "TeamGuardHook"));

        // 3. 添加自定义/第三方 MCP Server -> 列表可见 -> 删除后移除
        let mcp_path = rt
            .add_custom_mcp_server(
                &ws,
                "custom-fs",
                "npx",
                vec![
                    "-y".into(),
                    "@modelcontextprotocol/server-filesystem".into(),
                    ".".into(),
                ],
            )
            .unwrap();
        assert!(mcp_path.is_file());
        let servers = rt.list_active_mcp_servers(&ws);
        assert!(servers
            .iter()
            .any(|(id, cfg)| id == "custom-fs" && cfg.command == "npx" && !cfg.disabled));
        rt.remove_custom_mcp_server(&ws, "custom-fs").unwrap();
        assert!(!rt
            .list_active_mcp_servers(&ws)
            .iter()
            .any(|(id, _)| id == "custom-fs"));

        // 4. 校验 Pre-Input Hook 前置提示词不会破坏后续 /skill 触发匹配，且 HookInfo 保留 description
        let desc_hook = rt
            .add_custom_hook(
                &ws,
                CustomHookSpec {
                    name: "PrefixHook".into(),
                    description: "带有描述的前置注入钩子".into(),
                    enabled: true,
                    pre_input_prefix: Some("[Custom Hook] 注意边界".into()),
                    pre_input_block_patterns: vec![],
                    blocked_tools: vec![],
                    pre_tool_block_patterns: vec![],
                    mask_patterns: vec![],
                },
            )
            .unwrap();
        assert!(desc_hook.is_file());
        let infos = rt.list_active_hook_infos(&ws);
        assert!(infos.iter().any(|h| h.name == "PrefixHook"
            && h.description == "带有描述的前置注入钩子"
            && !h.is_builtin));
        let pre_skill = rt.run_pre_input_hook("/review 检查逻辑".into()).unwrap();
        let turn_after_hook = rt.prepare_agent_turn("BASE", &pre_skill, "code", &ws, None, &[]);
        assert!(
            turn_after_hook.system_prompt.contains("[Active Skill:"),
            "Pre-Input Hook 前置注入后 /review 仍须触发 Skill: {}",
            turn_after_hook.system_prompt
        );
        assert!(turn_after_hook
            .user_prompt
            .contains("[Custom Hook] 注意边界"));
        rt.remove_custom_hook(&ws, "PrefixHook").unwrap();
    }
}
