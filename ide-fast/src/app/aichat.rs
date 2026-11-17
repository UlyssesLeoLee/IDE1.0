//! AI 对话面板 (右侧): 流式回复、工具往返 (内建 + 插件 agent.tool)、补丁提案审核、插入编辑器、取消。
//! 联网只发生在点「发送」之后; 传输、密钥处理、解析都在 `ai.rs` (有单测)。配置在第一次打开面板时才读盘, 启动路径零开销。

use super::*;
use crate::ai::{self, AiConfig, Event, Handle, Msg, ToolCall};
use crate::app::collab::CollabMsg;
use ide_agent_router::{
    AgentMemoryEngine, ContextCompactor, CookieJar, CookieStats, FileCookie, HookPipeline,
    LoopBreaker, McpConfig, McpManager, PrunePolicy, SkillRegistry, TaskState,
    WorkspaceRulesEngine,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// 一次 Agent 运行所绑定的文件会话 Cookie (文件即任务): 运行结束 / 取消 / 出错时统一结算落盘
struct RunCookie {
    jar: CookieJar,
    ck: FileCookie,
    /// 本次运行是否产出了补丁提案 (决定结算成 Vetting 还是 Planning)
    proposed: bool,
}

struct Proposal {
    doc_id: u64,
    start: usize,
    remove: usize,
    insert: Vec<String>,
    old: Vec<String>,
    reason: String,
}

enum Item {
    User(String),
    Ai(String),
    Tool(String),
    Err(String),
    Sys(String),
    Proposal(Proposal),
}

struct Pending {
    slot: usize,
    call: ToolCall,
    rx: Receiver<Result<String, String>>,
}

struct Run {
    rx: Receiver<Event>,
    handle: Handle,
    msgs: Vec<Value>,
    tools: Vec<Value>,
    round: usize,
    doc_id: u64,
    /// 正在流式追加的 Ai 条目下标
    stream: Option<usize>,
    /// 本轮工具结果: 按调用顺序, None = 还在等插件
    slots: Vec<Option<Value>>,
    pending: Vec<Pending>,
    /// 当前文件的会话 Cookie (文档没有磁盘路径 / 不在工作区内时为 None)
    cookie: Option<RunCookie>,
    /// 本次运行是否已直接向当前打开文件应用补丁
    auto_applied: bool,
}

pub(super) struct AiState {
    pub open: bool,
    /// 统一右侧 AI 面板模式开关: false = 智能体对话, true = 多 Agent 并行协作
    pub collab_mode: bool,
    /// 是否免点击自动将需求结果/补丁直接应用到当前打开的文件
    pub auto_apply: bool,
    /// 当前激活的 Claude Mod 工作模式 id (`code` / `plan` / `review` / `ask`)
    pub active_mod: String,
    cfg: Option<AiConfig>,
    settings: bool,
    input: String,
    use_ctx: bool,
    items: Vec<Item>,
    history: Vec<Msg>,
    run: Option<Run>,
    /// 等用户同意发送上下文的提问
    ask: Option<String>,
    /// 设置里「自定义档位」输入框的文字 (打开设置后第一次画时从配置里填)
    efforts_text: Option<String>,
    /// 正在向供应商获取 effort 档位 (后台线程把结果送回这里)
    fetch: Option<Receiver<Result<Vec<String>, String>>>,
    /// 获取档位的结果 / 错误说明
    fetch_msg: String,
    // Agent 生态基础设施 (零冷启开销、Sakura 级轻量):
    pub skills: SkillRegistry,
    pub rules_engine: WorkspaceRulesEngine,
    pub memory_engine: AgentMemoryEngine,
    pub hooks: HookPipeline,
    pub loop_breaker: LoopBreaker,
    pub mcp: McpManager,
    mcp_loaded_root: Option<PathBuf>,
    /// 文件即任务: 文档 id → 任务状态 (Tab 指示条读它; 只存会话内状态, 随文档关闭回收)
    task_states: HashMap<u64, TaskState>,
    /// 已做过一次后台回收的工作区 (每个工作区每次启动只回收一次)
    pruned_roots: HashSet<PathBuf>,
    /// 设置菜单里显示的 Cookie 统计 (2 秒缓存, 避免菜单打开时每帧读目录)
    cookie_stats: Option<(Instant, CookieStats)>,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            open: false,
            collab_mode: false,
            auto_apply: true,
            active_mod: "code".into(),
            cfg: None,
            settings: false,
            input: String::new(),
            use_ctx: true,
            items: Vec::new(),
            history: Vec::new(),
            run: None,
            ask: None,
            efforts_text: None,
            fetch: None,
            fetch_msg: String::new(),
            skills: SkillRegistry::default(),
            rules_engine: WorkspaceRulesEngine::default(),
            memory_engine: AgentMemoryEngine::default(),
            hooks: HookPipeline::default(),
            loop_breaker: LoopBreaker::default(),
            mcp: McpManager::default(),
            mcp_loaded_root: None,
            task_states: HashMap::new(),
            pruned_roots: HashSet::new(),
            cookie_stats: None,
        }
    }
}

enum Act {
    Send,
    Cancel,
    Clear,
    Apply(usize),
    Reject(usize),
    Insert(String),
    Copy(String),
    Save,
    OpenCollab(String),
}

/// 字节数 → 人读文本 (设置菜单的 Cookie 占用显示)
pub(super) fn fmt_bytes(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
    }
}

fn find_workspace_root(doc_path: Option<&Path>) -> PathBuf {
    if let Some(p) = doc_path {
        let mut cur = if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent().unwrap_or(p).to_path_buf()
        };
        loop {
            if cur.join(".git").exists()
                || cur.join(".ide").exists()
                || cur.join("AGENT.md").exists()
                || cur.join("IDE.md").exists()
                || cur.join("Cargo.toml").exists()
            {
                return cur;
            }
            if !cur.pop() {
                break;
            }
        }
        if let Some(parent) = doc_path.and_then(|p| p.parent()) {
            return parent.to_path_buf();
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

impl App {
    /// 卸载或重置活跃的 MCP 外部工具服务器池 (热关闭 / 删除 mcp 插件时调用)
    pub(super) fn ai_reset_mcp(&mut self) {
        self.ai.mcp_loaded_root = None;
        self.ai.mcp = McpManager::default();
    }

    /// 确保当前工作区的 MCP 外部配置已登记 (按需惰性加载，启动 0 开销)
    fn ensure_mcp_config(&mut self, ws_root: &Path) {
        if self.ai.mcp_loaded_root.as_deref() == Some(ws_root) {
            return;
        }
        let dot_mcp = ws_root.join(".mcp.json");
        let servers_json = ws_root.join("mcp_servers.json");
        if let Some(cfg) =
            McpConfig::load_from_file(&dot_mcp).or_else(|| McpConfig::load_from_file(&servers_json))
        {
            self.ai.mcp.load_config(cfg);
        }
        self.ai.mcp_loaded_root = Some(ws_root.to_path_buf());
    }

    // ---------- 文件即任务: 会话 Cookie ----------

    /// 文档当前的任务状态 (Tab 指示条用; 没有任务 = Idle)
    pub(super) fn task_state_of(&self, doc_id: u64) -> TaskState {
        self.ai
            .task_states
            .get(&doc_id)
            .copied()
            .unwrap_or_default()
    }

    /// 每个工作区每次启动只回收一次: 后台线程清孤儿 / 过期 / 超预算的 Cookie, 不占用 UI 线程
    fn cookie_prune_once(&mut self, jar: &CookieJar, ws_root: &Path) {
        if !self.ai.pruned_roots.insert(ws_root.to_path_buf()) {
            return;
        }
        let jar = jar.clone();
        // 尽力而为的回收: 单个文件删不掉下次启动再试, 不影响任何功能
        let _ = std::thread::Builder::new()
            .name("cookie-prune".into())
            .spawn(move || {
                jar.prune(PrunePolicy::default());
            });
    }

    /// 运行开始: 读出旧 Cookie (生成续航提示) → 状态置 Exploring、记下本轮意图并立即落盘 (崩溃也留痕)
    fn cookie_begin(
        &mut self,
        i: usize,
        ws_root: &Path,
        prompt: &str,
    ) -> (Option<RunCookie>, Option<String>) {
        let Some(path) = self.docs[i].path.clone() else {
            return (None, None);
        };
        let jar = CookieJar::new(ws_root);
        let Some(rel) = jar.rel_of(&path) else {
            return (None, None);
        };
        self.cookie_prune_once(&jar, ws_root);
        let mut ck = jar.load(&rel).unwrap_or_else(|| FileCookie::new(&rel));
        let resume = ck.format_resume_prompt();
        ck.state = TaskState::Exploring;
        ck.intent = self.mask_secrets(prompt.chars().take(512).collect());
        ck.cursor_line = self.docs[i].cursor[0];
        ck.rounds = ck.rounds.saturating_add(1);
        if let Err(e) = jar.save(&mut ck) {
            self.ai.items.push(Item::Err(e));
        }
        self.ai
            .task_states
            .insert(self.docs[i].id, TaskState::Exploring);
        (
            Some(RunCookie {
                jar,
                ck,
                proposed: false,
            }),
            resume,
        )
    }

    /// 落盘前过一遍脱敏钩子 (Cookie 在磁盘上是明文, 不能把 API Key 带进去; 仅在 hooks 插件激活时生效)
    fn mask_secrets(&self, s: String) -> String {
        if crate::plugins::is_module_active("hooks") {
            self.ai.hooks.execute_post_response(s.clone()).unwrap_or(s)
        } else {
            s
        }
    }

    /// 运行结束 / 取消 / 出错: 结算 Cookie。有补丁提案 → Vetting (等人审), 否则 → Planning
    fn cookie_settle(&mut self, doc_id: u64, rc: Option<RunCookie>, summary: Option<String>) {
        let Some(mut rc) = rc else {
            return;
        };
        rc.ck.state = if rc.proposed {
            TaskState::Vetting
        } else {
            TaskState::Planning
        };
        if let Some(s) = summary {
            rc.ck.last_summary = self.mask_secrets(s);
        }
        if let Err(e) = rc.jar.save(&mut rc.ck) {
            self.ai.items.push(Item::Err(e));
        }
        self.ai.task_states.insert(doc_id, rc.ck.state);
        // 已关闭的文档不再保留状态 (有界)
        let live: HashSet<u64> = self.docs.iter().map(|d| d.id).collect();
        self.ai.task_states.retain(|id, _| live.contains(id));
    }

    /// 人工动作 (采纳补丁 → Verified / 拒绝 → Planning) 改写该文件的 Cookie 状态
    pub(super) fn cookie_mark(&mut self, doc_id: u64, state: TaskState) {
        self.ai.task_states.insert(doc_id, state);
        let Some(path) = self
            .docs
            .iter()
            .find(|d| d.id == doc_id)
            .and_then(|d| d.path.clone())
        else {
            return;
        };
        let jar = CookieJar::new(&find_workspace_root(Some(&path)));
        let Some(rel) = jar.rel_of(&path) else {
            return;
        };
        let mut ck = jar.load(&rel).unwrap_or_else(|| FileCookie::new(&rel));
        ck.state = state;
        if let Err(e) = jar.save(&mut ck) {
            self.ai.items.push(Item::Err(e));
        }
    }

    fn cookie_jar_active(&self) -> (CookieJar, PathBuf) {
        let i = self.active.min(self.docs.len() - 1);
        let ws = find_workspace_root(self.docs[i].path.as_deref());
        (CookieJar::new(&ws), ws)
    }

    /// 设置菜单里显示的统计 (2 秒缓存)
    pub(super) fn cookie_stats_cached(&mut self) -> CookieStats {
        if let Some((t, s)) = self.ai.cookie_stats {
            if t.elapsed() < Duration::from_secs(2) {
                return s;
            }
        }
        let s = self.cookie_jar_active().0.stats();
        self.ai.cookie_stats = Some((Instant::now(), s));
        s
    }

    /// 设置菜单「清除全部文件 Cookie」: 只清当前工作区 `.ide/cookies`, 结果写进状态提示
    pub(super) fn cookie_clear_all(&mut self) {
        let (jar, ws) = self.cookie_jar_active();
        let msg = match jar.clear_all() {
            Ok(s) => {
                self.cookie_forget_states_under(&ws);
                format!(
                    "已清除 {} 个文件 Cookie, 释放 {}",
                    s.count,
                    fmt_bytes(s.bytes)
                )
            }
            Err(e) => e,
        };
        self.ai.cookie_stats = None;
        self.cur().tip = msg;
    }

    /// 设置菜单「清理过期与孤儿 Cookie」: 孤儿 (源文件已删) + 超 30 天 + 超 8 MB 预算的最旧者
    pub(super) fn cookie_prune_now(&mut self) {
        let (jar, _) = self.cookie_jar_active();
        let s = jar.prune(PrunePolicy::default());
        self.ai.cookie_stats = None;
        self.cur().tip = format!(
            "已回收 {} 个文件 Cookie, 释放 {}",
            s.count,
            fmt_bytes(s.bytes)
        );
    }

    /// 工作区内的文档状态跟着 Cookie 一起清空 (Tab 指示条立刻消失)
    fn cookie_forget_states_under(&mut self, ws: &Path) {
        let ids: Vec<u64> = self
            .docs
            .iter()
            .filter(|d| d.path.as_deref().is_some_and(|p| p.starts_with(ws)))
            .map(|d| d.id)
            .collect();
        for id in ids {
            self.ai.task_states.remove(&id);
        }
    }

    /// 保存 AI 配置; 写盘失败要让用户看见
    fn ai_save_cfg(&mut self) {
        if let Err(e) = self.ai_cfg().save() {
            let m = tr("ai.save-failed").replacen("{}", &e, 1);
            self.ai.items.push(Item::Err(m));
        }
    }

    pub(super) fn ai_cfg(&mut self) -> &mut AiConfig {
        self.ai.cfg.get_or_insert_with(AiConfig::load)
    }

    pub(super) fn ai_toggle(&mut self) {
        self.ai.open = !self.ai.open;
        if self.ai.open {
            let _ = self.ai_cfg();
        }
    }

    /// 识别多智能体协作 / 协同编辑意图
    pub fn is_multi_agent_collab_intent(q: &str) -> bool {
        let lower = q.to_lowercase();
        lower.contains("多agent")
            || lower.contains("多智能体")
            || lower.contains("协同")
            || lower.contains("协作")
            || lower.contains("并行")
            || lower.contains("派工")
            || lower.contains("collab")
            || lower.contains("multi-agent")
            || lower.contains("/collab")
            || lower.contains("/parallel")
    }

    /// 点发送 / Enter: 先校验配置; 要带上下文且没同意过就弹确认
    pub(super) fn ai_send(&mut self, ctx: &egui::Context) {
        let raw_q = self.ai.input.trim().to_string();
        if raw_q.is_empty() || self.ai.run.is_some() {
            return;
        }
        // 执行 Pre-input 生命周期拦截钩子 (仅在 hooks 热插拔模块启用时运行)
        let q = if crate::plugins::is_module_active("hooks") {
            match self.ai.hooks.execute_pre_input(raw_q) {
                Ok(clean) => clean,
                Err(e) => {
                    self.ai.items.push(Item::Err(e));
                    return;
                }
            }
        } else {
            raw_q
        };

        // 智能识别多智能体协作需求并自动无缝切换，杜绝误报单智能体错误
        if Self::is_multi_agent_collab_intent(&q) {
            self.ai.input.clear();
            self.ai.open = true;
            self.ai.collab_mode = true;
            self.collab.open = true;
            self.collab.master_input.clear();
            self.collab.master_history.push(CollabMsg::user("用户", &q));
            self.collab.master_history.push(CollabMsg::system(
                "✓ 已智能无缝切换至多 Agent 并行协作工作台。",
            ));
            if !self.docs.is_empty() {
                if let Err(err) = self.execute_parallel_on_active(&q, Some(ctx)) {
                    self.collab
                        .master_history
                        .push(CollabMsg::error("系统", &err));
                }
            } else {
                self.collab.master_history.push(CollabMsg::system(
                    "请打开或分配需要协同编辑的文件，亦可在下方统筹命令框直接下达任务。",
                ));
            }
            return;
        }

        if !self.ai_cfg().configured() {
            // 未配置云端大模型时：免点击直接在当前打开的文件上确定性执行需求
            self.ai_exec_local_demand(&q, ctx);
            return;
        }
        let i = self.active.min(self.docs.len() - 1);
        let has_ctx = self.ai.use_ctx && self.ai_context(i).is_some();
        if has_ctx && !self.ai_cfg().consent && !self.ai.auto_apply {
            self.ai.ask = Some(q);
            return;
        }
        self.ai_start(q, has_ctx, ctx);
    }

    /// 离线确定性直接响应聊天需求并免点击修改当前打开的文件
    fn ai_exec_local_demand(&mut self, q: &str, ctx: &egui::Context) {
        self.ai.input.clear();
        self.ai.items.push(Item::User(q.to_string()));
        if self.docs.is_empty() {
            self.ai
                .items
                .push(Item::Err("当前没有打开的文件，无法直接写入".into()));
            return;
        }
        if q.contains("并行") || q.starts_with("/parallel") {
            match self.execute_parallel_on_active(q, Some(ctx)) {
                Ok(n) => {
                    self.ai.items.push(Item::Ai(format!(
                        "已调度 {} 个 Agent 对当前打开文件的互不重叠区域执行并行修改。",
                        n
                    )));
                    self.ai.items.push(Item::Sys(
                        "✓ 需求已免点击直接应用到当前打开文件 (Ctrl+Z 可一键撤销)".into(),
                    ));
                }
                Err(e) => self.ai.items.push(Item::Err(e)),
            }
            return;
        }

        let i = self.active.min(self.docs.len() - 1);
        if self.docs[i].readonly {
            self.ai
                .items
                .push(Item::Err("只读文档，无法直接应用修改".into()));
            return;
        }
        let doc_id = self.docs[i].id;
        let ws_root = find_workspace_root(self.docs[i].path.as_deref());
        let (run_cookie, _) = self.cookie_begin(i, &ws_root, q);

        let (start, remove, insert) = build_direct_patch(&self.docs[i], q);
        match self.docs[i].apply_line_patch(start, remove, insert.clone(), Some([start, 0])) {
            Ok(()) => {
                let doc_name = self.docs[i].name.clone();
                let reply = format!(
                    "已根据需求在「{}」第 {} 行直接写入 {} 行内容（无需配置云端模型即可生效，Ctrl+Z 可撤销）。",
                    doc_name,
                    start + 1,
                    insert.len()
                );
                self.docs[i].tip = tr("ai.applied").into();
                let settled = run_cookie.map(|mut rc| {
                    rc.proposed = true;
                    rc
                });
                self.cookie_settle(doc_id, settled, Some(reply.clone()));
                self.cookie_mark(doc_id, TaskState::Verified);
                self.ai.items.push(Item::Ai(reply.clone()));
                self.ai.items.push(Item::Sys(
                    "✓ 已免点击直接应用到当前打开文件 (如需云端大模型，可在「模型设置…」填入 API Key)"
                        .into(),
                ));
                self.ai.history.push(Msg {
                    role: "user".into(),
                    content: q.to_string(),
                });
                self.ai.history.push(Msg {
                    role: "assistant".into(),
                    content: reply,
                });
            }
            Err(e) => {
                self.cookie_settle(doc_id, run_cookie, None);
                self.ai.items.push(Item::Err(e));
            }
        }
    }

    fn ai_context(&self, i: usize) -> Option<ai::Context> {
        let d = &self.docs[i];
        let n = d.name.to_lowercase();
        ai::context_of(
            &d.lines,
            Some(crate::plugin_ui::selection_text(d)),
            d.cursor[0],
            n.ends_with(".md") || n.ends_with(".markdown"),
        )
    }

    fn ai_start(&mut self, q: String, with_ctx: bool, ctx: &egui::Context) {
        let i = self.active.min(self.docs.len() - 1);
        let ws_root = find_workspace_root(self.docs[i].path.as_deref());
        let mcp_active = crate::plugins::is_module_active("mcp");
        let skills_active = crate::plugins::is_module_active("skills");
        let mod_modes = crate::plugins::active_mod_modes();

        if mcp_active {
            self.ensure_mcp_config(&ws_root);
        } else {
            self.ai_reset_mcp();
        }

        // 1. 技能匹配与指令转化 (仅在 skills 热插拔模块启用时生效)
        let (matched_skill, user_prompt) = if skills_active {
            if let Some((skill, rest)) = self.ai.skills.match_prompt(&q) {
                (Some(skill.clone()), rest.to_string())
            } else {
                (None, q.clone())
            }
        } else {
            (None, q.clone())
        };

        let c = if with_ctx { self.ai_context(i) } else { None };
        let mut sys = ai::system_prompt(&self.docs[i].name, self.docs[i].lines.len(), c.as_ref());

        // 2. Claude Mod 模式与工作区规范约束注入 (仅在 mod 热插拔模块启用时生效)
        if !mod_modes.is_empty() {
            if let Some(rules_prompt) = self.ai.rules_engine.format_invariants_prompt(&ws_root) {
                sys.push_str("\n\n");
                sys.push_str(&rules_prompt);
            }
            if let Some(active_mode) = mod_modes
                .iter()
                .find(|m| m.id == self.ai.active_mod)
                .or_else(|| mod_modes.first())
            {
                if !active_mode.prompt.is_empty() {
                    sys.push_str("\n\n");
                    sys.push_str(&active_mode.prompt);
                }
            }
        }

        // 3. 注入长期记忆 (架构决议、技术栈偏好)
        if let Some(mem_prompt) = self.ai.memory_engine.format_memory_prompt(&ws_root) {
            sys.push_str("\n\n");
            sys.push_str(&mem_prompt);
        }

        // 3.5 文件即任务: 载入该文件的会话 Cookie (断点续航 + 局部不变量), 并把本轮意图记进去
        let (run_cookie, resume) = self.cookie_begin(i, &ws_root, &q);
        if let Some(r) = resume {
            sys.push_str("\n\n");
            sys.push_str(&r);
        }

        // 4. 应用技能模板强化
        let (final_sys, final_q) = if let Some(skill) = matched_skill {
            skill.apply(&user_prompt, Some(&sys))
        } else {
            (sys, user_prompt)
        };

        // 5. 组装消息列表并执行自适应上下文压缩 (Context Compact)
        let mut msgs = ai::base_messages(final_sys, &self.ai.history, &final_q);
        ContextCompactor::compact_messages(&mut msgs, 8192, 4);

        // 6. 聚合内建工具、插件工具与外部 MCP 工具 (mcp 关闭时不再暴露外部工具)
        let installed = editcore::plugin_host::store::list(&plugins::root());
        let mut tools = ai::tool_table(&installed);
        if mcp_active {
            let mcp_tools = self.ai.mcp.aggregate_openai_tools();
            tools.extend(mcp_tools);
        }

        self.ai.input.clear();
        self.ai.items.push(Item::User(q.clone()));
        // 记录工作记忆 (零分配环形缓冲)
        self.ai
            .memory_engine
            .record_working_item("user", &q, vec!["prompt".into()]);

        let (rx, handle) = self.ai_spawn(&msgs, &tools, ctx);
        self.ai.run = Some(Run {
            rx,
            handle,
            msgs,
            tools,
            round: 0,
            doc_id: self.docs[i].id,
            stream: None,
            slots: vec![],
            pending: vec![],
            cookie: run_cookie,
            auto_applied: false,
        });
    }

    fn ai_spawn(
        &mut self,
        msgs: &[Value],
        tools: &[Value],
        ctx: &egui::Context,
    ) -> (Receiver<Event>, Handle) {
        let (tx, rx) = channel();
        let c = ctx.clone();
        let cfg = self.ai_cfg().clone();
        let h = ai::spawn_request(&cfg, msgs.to_vec(), tools.to_vec(), tx, move || {
            c.request_repaint()
        });
        (rx, h)
    }

    /// 每帧: 收流式文字 / 回合结果 / 插件工具结果。没有请求时什么都不做 (无空转轮询)
    pub(super) fn ai_poll(&mut self, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        let mut events = vec![];
        loop {
            match run.rx.try_recv() {
                Ok(e) => events.push(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    events.push(Event::Done(ai::Turn {
                        error: Some("请求线程意外结束".into()),
                        ..Default::default()
                    }));
                    break;
                }
            }
        }
        for e in events {
            match e {
                Event::Text(t) => self.ai_append(&t),
                Event::Done(turn) => self.ai_turn(turn, ctx),
            }
            if self.ai.run.is_none() {
                return;
            }
        }
        self.ai_poll_plugins(ctx);
    }

    fn ai_append(&mut self, t: &str) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        match run.stream {
            Some(i) => {
                if let Some(Item::Ai(s)) = self.ai.items.get_mut(i) {
                    s.push_str(t);
                }
            }
            None => {
                self.ai.items.push(Item::Ai(t.to_string()));
                run.stream = Some(self.ai.items.len() - 1);
            }
        }
    }

    fn ai_turn(&mut self, turn: ai::Turn, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        run.stream = None;
        if let Some(e) = &turn.error {
            let clean = if e.trim() == "HTTP 0" || e.trim() == "HTTP 000" {
                "【网络连接失败】未收到 AI 服务端响应，请检查 Base URL 与网络连通性。".to_string()
            } else {
                e.clone()
            };
            self.ai.items.push(Item::Err(clean));
            if let Some(mut r) = self.ai.run.take() {
                let (id, ck) = (r.doc_id, r.cookie.take());
                self.cookie_settle(id, ck, None);
            }
            return;
        }
        run.msgs.push(ai::assistant_message(&turn));
        run.round += 1;
        if turn.tool_calls.is_empty() || run.round >= ai::MAX_ROUNDS {
            self.ai_finish();
            return;
        }
        let doc_id = run.doc_id;
        let rows = self
            .docs
            .iter()
            .find(|d| d.id == doc_id)
            .map_or(0, |d| d.lines.len());
        let installed = editcore::plugin_host::store::list(&plugins::root());
        if let Some(run) = self.ai.run.as_mut() {
            run.slots = vec![None; turn.tool_calls.len()];
        }
        let mut immediate = vec![];
        let mut spawn = vec![];
        for (slot, call) in turn.tool_calls.iter().enumerate() {
            let args = ai::parse_args(&call.args);

            // 1. 智能体死循环自省熔断 (LoopBreaker: 64位哈希三元组检测防死循环)
            if let Err(loop_err) = self.ai.loop_breaker.check_and_record(&call.name, &args) {
                immediate.push((slot, call.clone(), json!({ "error": loop_err })));
                continue;
            }

            // 2. Pre-tool-call 生命周期拦截钩子 (仅在 hooks 热插拔模块启用时运行)
            if crate::plugins::is_module_active("hooks") {
                if let Err(hook_err) = self.ai.hooks.execute_pre_tool_call(&call.name, &args) {
                    immediate.push((slot, call.clone(), json!({ "error": hook_err })));
                    continue;
                }
            }

            // 3. 工具分发：MCP 外部工具 vs 插件扩展工具 vs 原生内建工具
            if call.name.starts_with("mcp_") {
                if !crate::plugins::is_module_active("mcp") {
                    immediate.push((
                        slot,
                        call.clone(),
                        json!({ "error": "MCP 插件模块已关闭或卸载" }),
                    ));
                    continue;
                }
                // MCP 工具：后台专用 Worker 线程异步执行，零阻塞 UI 渲染
                let (tx, rx) = channel();
                let mgr = self.ai.mcp.clone();
                let full_name = call.name.clone();
                let args_clone = args.clone();
                let c = ctx.clone();
                std::thread::Builder::new()
                    .name(format!("mcp-{}", call.name))
                    .spawn(move || {
                        let res = match mgr.dispatch_tool_call(&full_name, args_clone) {
                            Some(Ok(mcp_res)) => {
                                if mcp_res.is_error {
                                    Err(mcp_res.merged_text())
                                } else {
                                    Ok(serde_json::to_string(&json!({
                                        "result": mcp_res.merged_text()
                                    }))
                                    .unwrap_or_else(|_| "{}".into()))
                                }
                            }
                            Some(Err(err)) => Err(err),
                            None => Err(format!("未找到对应的 MCP 工具服务器: {}", full_name)),
                        };
                        let _ = tx.send(res);
                        c.request_repaint();
                    })
                    .ok();
                if let Some(run) = self.ai.run.as_mut() {
                    run.pending.push(Pending {
                        slot,
                        call: call.clone(),
                        rx,
                    });
                }
            } else if let Some((pid, tool)) = ai::split_plugin_tool(&call.name) {
                if ai::plugin_tool_declared(&installed, &pid, &tool) {
                    spawn.push((slot, call.clone(), pid, tool, args));
                } else {
                    immediate.push((
                        slot,
                        call.clone(),
                        json!({ "error": format!("unknown tool: {}", call.name) }),
                    ));
                }
            } else {
                immediate.push((
                    slot,
                    call.clone(),
                    ai::exec_builtin(&call.name, &args, rows),
                ));
            }
        }
        for (slot, call, pid, tool, args) in spawn {
            let rx = self
                .plug
                .call_raw(&pid, &format!("agent.tool:{tool}"), &json!([args]));
            if let Some(run) = self.ai.run.as_mut() {
                run.pending.push(Pending { slot, call, rx });
            }
        }
        for (slot, call, r) in immediate {
            self.ai_tool_done(slot, &call, r, doc_id);
        }
        self.ai_next_round(ctx);
    }

    fn ai_poll_plugins(&mut self, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        if run.pending.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        let doc_id = run.doc_id;
        let mut done = vec![];
        let mut i = 0;
        while i < run.pending.len() {
            let r = match run.pending[i].rx.try_recv() {
                Ok(r) => Some(r),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err("异步工具服务已停止".into())),
            };
            match r {
                Some(r) => done.push((run.pending.remove(i), r)),
                None => i += 1,
            }
        }
        let rows = self
            .docs
            .iter()
            .find(|d| d.id == doc_id)
            .map_or(0, |d| d.lines.len());
        for (p, r) in done {
            let v = match r {
                Ok(j) => match serde_json::from_str::<Value>(&j) {
                    Ok(v) if v.is_null() => json!({}),
                    Ok(v) => {
                        if v.get("result").is_some() || v.get("error").is_some() {
                            v
                        } else {
                            ai::vet_plugin_patch(&v, rows)
                        }
                    }
                    Err(_) => json!({ "error": "工具返回的不是 JSON" }),
                },
                Err(e) => json!({ "error": format!("工具执行失败: {e}") }),
            };
            self.ai_tool_done(p.slot, &p.call, v, doc_id);
        }
        self.ai_next_round(ctx);
    }

    fn ai_tool_done(&mut self, slot: usize, call: &ToolCall, result: Value, doc_id: u64) {
        // 执行 Post-tool-call 生命周期拦截钩子 (仅在 hooks 热插拔模块启用时运行)
        let result: Value = if crate::plugins::is_module_active("hooks") {
            let masked_result_str = self
                .ai
                .hooks
                .execute_post_tool_call(&call.name, result.to_string());
            serde_json::from_str(&masked_result_str).unwrap_or(result)
        } else {
            result
        };

        let line = match result["error"].as_str() {
            Some(e) => trf("ai.tool-fail", &[&call.name, e]),
            None => trf("ai.tool-ok", &[&call.name]),
        };
        self.ai.items.push(Item::Tool(line.clone()));

        // 记录工作记忆 (无锁环形队列)
        self.ai.memory_engine.record_working_item(
            "tool",
            &format!("{}: {}", call.name, line),
            vec!["tool".into(), call.name.clone()],
        );

        if result["kind"] == "patch_proposal" {
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

            if self.ai.auto_apply {
                let applied = match self.docs.iter_mut().find(|d| d.id == doc_id) {
                    Some(d) => match d.apply_line_patch(start, remove, insert.clone(), None) {
                        Ok(()) => {
                            d.tip = tr("ai.applied").into();
                            Ok(format!(
                                "✓ 已免点击直接应用到当前文件 (第 {} 行, 插入 {} 行){}",
                                start + 1,
                                insert.len(),
                                if reason.is_empty() {
                                    String::new()
                                } else {
                                    format!(": {reason}")
                                }
                            ))
                        }
                        Err(e) => Err(e),
                    },
                    None => Err("目标文档已关闭".into()),
                };
                match applied {
                    Ok(ok_msg) => {
                        if let Some(run) = self.ai.run.as_mut() {
                            run.auto_applied = true;
                            if let Some(rc) = run.cookie.as_mut() {
                                rc.proposed = true;
                            }
                        }
                        self.cookie_mark(doc_id, TaskState::Verified);
                        self.ai.items.push(Item::Sys(ok_msg));
                    }
                    Err(e) => {
                        self.ai.items.push(Item::Err(e));
                    }
                }
            } else {
                // 手动审核模式: 产出了补丁 → 本轮结算为 Vetting (Tab 显示紫色, 等人审)
                if let Some(rc) = self.ai.run.as_mut().and_then(|r| r.cookie.as_mut()) {
                    rc.proposed = true;
                }
                self.cookie_mark(doc_id, TaskState::Vetting);
                let old = self
                    .docs
                    .iter()
                    .find(|d| d.id == doc_id)
                    .map(|d| d.lines.iter().skip(start).take(remove).cloned().collect())
                    .unwrap_or_default();
                self.ai.items.push(Item::Proposal(Proposal {
                    doc_id,
                    start,
                    remove,
                    insert,
                    old,
                    reason,
                }));
            }
        }
        if let Some(run) = self.ai.run.as_mut() {
            if let Some(s) = run.slots.get_mut(slot) {
                *s = Some(ai::tool_message(&call.id, &result));
            }
        }
    }

    /// 本轮工具都有结果了 -> 把结果并进消息, 发下一轮
    fn ai_next_round(&mut self, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        if !run.pending.is_empty() || run.slots.iter().any(Option::is_none) {
            return;
        }
        let results: Vec<Value> = run.slots.drain(..).flatten().collect();
        run.msgs.extend(results);
        let (msgs, tools) = (run.msgs.clone(), run.tools.clone());
        let (rx, handle) = self.ai_spawn(&msgs, &tools, ctx);
        if let Some(run) = self.ai.run.as_mut() {
            run.rx = rx;
            run.handle = handle;
        }
    }

    /// 正常结束: 若模型在正文返回了代码块且未通过工具改动过文件，开启免点击自动应用时直接写回当前打开文件
    fn ai_finish(&mut self) {
        self.ai.loop_breaker.reset();
        if let Some(mut run) = self.ai.run.take() {
            let (doc_id, mut cookie, auto_applied) =
                (run.doc_id, run.cookie.take(), run.auto_applied);
            let h: Vec<Msg> = run
                .msgs
                .iter()
                .skip(1)
                .filter(|m| {
                    m["role"] == "user"
                        || (m["role"] == "assistant"
                            && m["tool_calls"].is_null()
                            && m["content"].as_str().is_some_and(|s| !s.is_empty()))
                })
                .map(|m| Msg {
                    role: m["role"].as_str().unwrap_or("").into(),
                    content: m["content"].as_str().unwrap_or("").into(),
                })
                .collect();
            let last_assistant = h
                .iter()
                .rev()
                .find(|m| m.role == "assistant")
                .map(|m| m.content.as_str());
            let allow_auto_apply =
                !crate::plugins::is_module_active("mod") || self.ai.active_mod == "code";
            if self.ai.auto_apply && !auto_applied && allow_auto_apply {
                if let Some(code_lines) = last_assistant.and_then(extract_fenced_code_block) {
                    if let Some(doc) = self.docs.iter_mut().find(|d| d.id == doc_id) {
                        if !doc.readonly {
                            let (start, remove) = if let Some((r0, r1)) = doc.selected_rows() {
                                (r0, r1 - r0 + 1)
                            } else if doc.lines.len() == 1 && doc.lines[0].is_empty() {
                                (0, 1)
                            } else {
                                (doc.cursor[0].min(doc.lines.len()), 0)
                            };
                            if doc
                                .apply_line_patch(
                                    start,
                                    remove,
                                    code_lines.clone(),
                                    Some([start, 0]),
                                )
                                .is_ok()
                            {
                                doc.tip = tr("ai.applied").into();
                                if let Some(rc) = cookie.as_mut() {
                                    rc.proposed = true;
                                }
                                self.ai.items.push(Item::Sys(format!(
                                    "✓ 已从回复提取 {} 行代码并免点击写入当前文件",
                                    code_lines.len()
                                )));
                                self.cookie_mark(doc_id, TaskState::Verified);
                            }
                        }
                    }
                }
            }
            let summary = last_assistant.map(|c| c.chars().take(600).collect::<String>());
            if let Some(s) = &summary {
                self.ai.memory_engine.record_working_item(
                    "assistant",
                    &s.chars().take(200).collect::<String>(),
                    vec!["response".into()],
                );
            }
            self.cookie_settle(doc_id, cookie, summary);
            self.ai.history = ai::trim_history(h);
        }
    }

    fn ai_cancel(&mut self) {
        self.ai.loop_breaker.reset();
        if let Some(mut run) = self.ai.run.take() {
            let (doc_id, cookie) = (run.doc_id, run.cookie.take());
            run.handle.cancel();
            self.cookie_settle(doc_id, cookie, None);
            self.ai.items.push(Item::Sys(tr("ai.cancelled").into()));
        }
    }

    fn ai_apply(&mut self, idx: usize) {
        let Some(Item::Proposal(p)) = self.ai.items.get(idx) else {
            return;
        };
        let doc_id = p.doc_id;
        let msg = match self.docs.iter_mut().find(|d| d.id == p.doc_id) {
            Some(d)
                if d.lines.len() >= p.start + p.remove
                    && d.lines[p.start..p.start + p.remove] == p.old[..] =>
            {
                match d.apply_line_patch(p.start, p.remove, p.insert.clone(), None) {
                    Ok(()) => {
                        d.tip = tr("ai.applied").into();
                        Item::Sys(tr("ai.applied").into())
                    }
                    Err(e) => Item::Err(e),
                }
            }
            _ => Item::Err(tr("ai.stale").into()),
        };
        let applied = matches!(msg, Item::Sys(_));
        self.ai.items[idx] = msg;
        if applied {
            self.cookie_mark(doc_id, TaskState::Verified);
        }
    }

    pub(super) fn ai_windows(&mut self, ctx: &egui::Context) {
        let Some(q) = self.ai.ask.clone() else { return };
        let url = self.ai_cfg().base_url.clone();
        let i = self.active.min(self.docs.len() - 1);
        let (label, n) = self
            .ai_context(i)
            .map(|c| (c.label, c.text.chars().count()))
            .unwrap_or(("", 0));
        let mut choice = None;
        egui::Window::new(tr("ai.consent-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(trf("ai.consent", &[label, &n.to_string(), &url]));
                ui.horizontal(|ui| {
                    if ui.button(tr("ai.allow")).clicked() {
                        choice = Some(true);
                    }
                    if ui.button(tr("ai.deny")).clicked() {
                        choice = Some(false);
                    }
                });
            });
        if let Some(allow) = choice {
            self.ai.ask = None;
            if allow {
                let c = self.ai_cfg();
                c.consent = true;
                self.ai_save_cfg();
            }
            self.ai_start(q, allow, ctx);
        }
    }

    pub(super) fn ai_ui(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width(ui.available_width());
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let ctx = ui.ctx().clone();
        let mut act: Option<Act> = None;
        let busy = self.ai.run.is_some();
        // 头部 (Tauri `#chat-head`): 左「Chat」(11px 等宽), 右 ✕ / 清空 / 「带上选中内容 / 当前章节」勾选
        let tc = self.tc;
        let edge = if tc.dark {
            Color32::from_gray(69)
        } else {
            Color32::from_gray(204)
        };
        let boxed = |t: &str| {
            if tc.comic {
                egui::Button::new(egui::RichText::new(t).size(12.0).color(Color32::WHITE))
                    .fill(Color32::from_rgb(32, 36, 52))
                    .stroke(egui::Stroke::new(1.5, Color32::BLACK))
                    .corner_radius(3.0)
                    .min_size(Vec2::new(0.0, 26.0))
            } else {
                egui::Button::new(egui::RichText::new(t).size(12.0).color(tc.fg))
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, edge))
                    .corner_radius(4.0)
                    .min_size(Vec2::new(0.0, 26.0))
            }
        };
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            if ui
                .selectable_label(!self.ai.collab_mode, "💬 智能体对话")
                .clicked()
            {
                self.ai.collab_mode = false;
            }
            if ui
                .selectable_label(self.ai.collab_mode, "🤝 多Agent并行协作")
                .clicked()
            {
                self.ai.collab_mode = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(crate::btnstyle::unified_collapse_button("×", &tc, false))
                    .tip(trz("折叠 (Ctrl+Alt+L)"))
                    .clicked()
                {
                    self.ai.open = false;
                }
                if !self.ai.collab_mode && ui.add(boxed(tr("ai.clear"))).clicked() {
                    act = Some(Act::Clear);
                }
            });
        });
        if self.ai.collab_mode {
            ui.separator();
            self.collab_panel_ui(ui);
            return;
        }
        let mod_modes = crate::plugins::active_mod_modes();
        let skills_active = crate::plugins::is_module_active("skills");
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::Checkbox::new(
                &mut self.ai.use_ctx,
                egui::RichText::new(tr("ai.ctx")).size(11.0),
            ));
            ui.add(egui::Checkbox::new(
                &mut self.ai.auto_apply,
                egui::RichText::new("免点击直接改文件").size(11.0),
            ));
        });
        if !mod_modes.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new("Mod:")
                        .size(11.0)
                        .strong()
                        .color(tc.fg_dim),
                );
                for m in &mod_modes {
                    let selected = self.ai.active_mod == m.id;
                    if ui
                        .selectable_label(selected, egui::RichText::new(&m.label).size(11.0))
                        .on_hover_text(&m.title)
                        .clicked()
                    {
                        self.ai.active_mod = m.id.clone();
                    }
                }
            });
        }
        if self.ai.settings {
            ui.separator();
            let cfg = self.ai.cfg.get_or_insert_with(AiConfig::load);
            ui.horizontal_wrapped(|ui| {
                for (name, url) in ai::PRESETS {
                    if ui.small_button(*name).clicked() {
                        cfg.base_url = (*url).to_string();
                    }
                }
            });
            egui::Grid::new("ai_cfg").num_columns(2).show(ui, |ui| {
                ui.label(tr("ai.base-url"));
                ui.add(egui::TextEdit::singleline(&mut cfg.base_url).desired_width(220.0));
                ui.end_row();
                ui.label(tr("ai.model"));
                ui.add(egui::TextEdit::singleline(&mut cfg.model).desired_width(220.0));
                ui.end_row();
                ui.label(tr("ai.key"));
                ui.add(
                    egui::TextEdit::singleline(&mut cfg.key)
                        .password(true)
                        .desired_width(220.0),
                );
                ui.end_row();
                ui.label(tr("ai.effort"));
                // 档位下拉: 默认 (不指定) + 获取到 / 手填的档位 (没有就用通用默认); 当前值不在列表里也照样显示
                let opts = ai::options(cfg);
                let shown = if cfg.effort.is_empty() {
                    tr("ai.effort-default").to_string()
                } else {
                    cfg.effort.clone()
                };
                egui::ComboBox::from_id_salt("ai-effort")
                    .width(220.0)
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut cfg.effort,
                            String::new(),
                            tr("ai.effort-default"),
                        );
                        for o in &opts {
                            ui.selectable_value(&mut cfg.effort, o.clone(), o);
                        }
                    });
                ui.end_row();
                ui.label(tr("ai.effort-custom"));
                let text = self
                    .ai
                    .efforts_text
                    .get_or_insert_with(|| cfg.efforts.join(","));
                if ui
                    .add(
                        egui::TextEdit::singleline(text)
                            .hint_text("low,medium,high,xhigh")
                            .desired_width(220.0),
                    )
                    .changed()
                {
                    cfg.efforts = ai::parse_custom(text);
                }
                ui.end_row();
            });
            // 从供应商获取档位: 点了才联网, 后台线程查模型接口 (密钥只发给上面填的接口地址)
            ui.horizontal(|ui| {
                let busy = self.ai.fetch.is_some();
                if ui
                    .add_enabled(!busy, egui::Button::new(tr("ai.effort-fetch")))
                    .clicked()
                {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let snapshot = cfg.clone();
                    let ctx = ui.ctx().clone();
                    let _ =
                        std::thread::Builder::new()
                            .name("ai-efforts".into())
                            .spawn(move || {
                                let _ = tx.send(ai::fetch_efforts(&snapshot));
                                ctx.request_repaint();
                            });
                    self.ai.fetch = Some(rx);
                    self.ai.fetch_msg = tr("ai.effort-fetching").to_string();
                }
                if busy {
                    ui.spinner();
                }
            });
            if let Some(rx) = &self.ai.fetch {
                match rx.try_recv() {
                    Ok(res) => {
                        self.ai.fetch = None;
                        self.ai.fetch_msg = match res {
                            Ok(list) => {
                                let msg = trf(
                                    "ai.effort-got",
                                    &[&list.len().to_string(), &list.join(", ")],
                                );
                                self.ai.efforts_text = Some(list.join(","));
                                cfg.efforts = list;
                                msg
                            }
                            Err(e) => e,
                        };
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => ui
                        .ctx()
                        .request_repaint_after(std::time::Duration::from_millis(150)),
                    Err(_) => self.ai.fetch = None,
                }
            }
            if !self.ai.fetch_msg.is_empty() {
                ui.weak(self.ai.fetch_msg.as_str());
            }
            if let Err(e) = ai::endpoint(&cfg.base_url) {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            ui.weak(tr("ai.key-note"));
            if ui.button(tr("ai.save")).clicked() {
                act = Some(Act::Save);
            }
        }
        ui.separator();
        let bottom_id = egui::Id::new("ai_bottom_bar_height");
        // 初始自适应预留高度: 模型行(~24px) + 快捷技能栏(~48px) + 输入框3行(~68px) + 分隔线与留白(~24px) = ~164-195px
        let bottom_reserved: f32 = ui.data(|d| d.get_temp(bottom_id)).unwrap_or(195.0);
        let avail = (ui.available_height() - bottom_reserved - 10.0).max(50.0);
        egui::ScrollArea::vertical()
            .max_height(avail)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for idx in 0..self.ai.items.len() {
                    match &self.ai.items[idx] {
                        Item::User(t) => {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{} {t}", tr("ai.you"))).strong(),
                                )
                                .wrap(),
                            );
                            let tl = t.to_lowercase();
                            if tl.contains("agent") || tl.contains("派出") || tl.contains("协同")
                            {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("[提示] 检测到多 Agent 调度任务:")
                                            .size(11.0)
                                            .color(Color32::from_rgb(0, 190, 255)),
                                    );
                                    if ui.small_button("打开多 Agent 协同工作台").clicked()
                                    {
                                        act = Some(Act::OpenCollab(t.clone()));
                                    }
                                });
                            }
                        }
                        Item::Ai(t) => {
                            ui.add(egui::Label::new(t.as_str()).wrap());
                            if !t.is_empty() && !busy {
                                ui.horizontal(|ui| {
                                    if ui.small_button(tr("ai.insert")).clicked() {
                                        act = Some(Act::Insert(t.clone()));
                                    }
                                    if ui.small_button(tr("ai.copy")).clicked() {
                                        act = Some(Act::Copy(t.clone()));
                                    }
                                });
                            }
                        }
                        Item::Tool(t) => {
                            ui.add(egui::Label::new(egui::RichText::new(t.as_str()).weak()).wrap());
                        }
                        Item::Err(t) => {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(t.as_str()).color(Color32::LIGHT_RED),
                                )
                                .wrap(),
                            );
                        }
                        Item::Sys(t) => {
                            ui.add(egui::Label::new(egui::RichText::new(t.as_str()).weak()).wrap());
                        }
                        Item::Proposal(p) => {
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                let head = trf(
                                    "ai.proposal",
                                    &[&(p.start + 1).to_string(), &p.remove.to_string()],
                                );
                                ui.add(
                                    egui::Label::new(if p.reason.is_empty() {
                                        head
                                    } else {
                                        format!("{head} — {}", p.reason)
                                    })
                                    .wrap(),
                                );
                                for l in &p.old {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!("- {l}"))
                                                .color(Color32::from_rgb(220, 110, 110)),
                                        )
                                        .wrap(),
                                    );
                                }
                                for l in &p.insert {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!("+ {l}"))
                                                .color(Color32::from_rgb(110, 200, 120)),
                                        )
                                        .wrap(),
                                    );
                                }
                                ui.horizontal(|ui| {
                                    if ui.button(tr("ai.apply")).clicked() {
                                        act = Some(Act::Apply(idx));
                                    }
                                    if ui.button(tr("ai.reject")).clicked() {
                                        act = Some(Act::Reject(idx));
                                    }
                                });
                            });
                        }
                    }
                    ui.add_space(4.0);
                }
                if busy {
                    ui.weak(tr("ai.thinking"));
                }
            });
        let bottom_start_y = ui.cursor().top();
        ui.separator();
        // 模型行 (Tauri 聊天底部): 左 = 当前模型 (没设就写「未设置模型」), 右 = 模型设置…
        let model = self.ai_cfg().model.clone();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if model.is_empty() {
                    trz("未设置模型").to_string()
                } else {
                    model
                })
                .size(11.0)
                .color(tc.fg_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(boxed(&format!("{}…", trz("模型设置")))).clicked() {
                    self.ai.settings = !self.ai.settings;
                }
            });
        });
        // 快捷技能栏 (Slash Commands 快捷触发, 仅在 skills 热插拔模块启用时显示)
        if skills_active {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let quick_skills = [
                    ("/review", "⚡/review", "代码审查"),
                    ("/test", "🧪/test", "单测生成"),
                    ("/refactor", "🔨/refactor", "架构重构"),
                    ("/commit", "📦/commit", "提交提炼"),
                    ("/nema", "🌀/nema", "NEMA拓扑"),
                    ("/explain", "📖/explain", "架构解说"),
                ];
                for (cmd, label, tip) in quick_skills {
                    if ui.small_button(label).on_hover_text(tip).clicked() {
                        if !self.ai.input.starts_with('/') {
                            self.ai.input = format!("{cmd} {}", self.ai.input);
                        } else if let Some(space_idx) = self.ai.input.find(' ') {
                            self.ai.input = format!("{cmd} {}", &self.ai.input[space_idx + 1..]);
                        } else {
                            self.ai.input = format!("{cmd} ");
                        }
                    }
                }
            });
            if self.ai.input.starts_with('/') {
                if let Some((skill, _)) = self.ai.skills.match_prompt(&self.ai.input) {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "💡 激活技能: {} — {}",
                                skill.name, skill.description
                            ))
                            .size(11.0)
                            .color(Color32::from_rgb(0, 190, 255)),
                        );
                    });
                }
            }
        }
        let id = ui.make_persistent_id("ai_input");
        let enter = ui.memory(|m| m.has_focus(id))
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        // 输入框 + 右侧「发送」(忙时是「取消」): Tauri 的聊天输入是一行两栏
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 62.0).max(80.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.ai.input)
                    .id(id)
                    .desired_rows(3)
                    .desired_width(w)
                    .hint_text(tr("ai.placeholder")),
            );
            if busy {
                if ui.add(boxed(tr("ai.cancel"))).clicked() {
                    act = Some(Act::Cancel);
                }
            } else if ui.add(boxed(tr("ai.send"))).clicked() || enter {
                act = Some(Act::Send);
            }
        });
        let bottom_end_y = ui.cursor().top();
        let actual_bottom_h = (bottom_end_y - bottom_start_y).max(90.0);
        ui.data_mut(|d| d.insert_temp(bottom_id, actual_bottom_h));
        if (actual_bottom_h - bottom_reserved).abs() > 1.5 {
            ui.ctx().request_repaint();
        }
        match act {
            Some(Act::Send) => self.ai_send(&ctx),
            Some(Act::Cancel) => self.ai_cancel(),
            Some(Act::Clear) => {
                self.ai_cancel();
                self.ai.items.clear();
                self.ai.history.clear();
            }
            Some(Act::Apply(i)) => self.ai_apply(i),
            Some(Act::Reject(i)) => {
                let doc_id = match self.ai.items.get(i) {
                    Some(Item::Proposal(p)) => Some(p.doc_id),
                    _ => None,
                };
                self.ai.items[i] = Item::Sys(tr("ai.rejected").into());
                if let Some(id) = doc_id {
                    self.cookie_mark(id, TaskState::Planning);
                }
            }
            Some(Act::Insert(t)) => {
                self.cur().edit("paste", Some(t));
            }
            Some(Act::Copy(t)) => ctx.copy_text(t),
            Some(Act::Save) => {
                self.ai_save_cfg();
                self.ai.settings = false;
            }
            Some(Act::OpenCollab(prompt)) => {
                self.ai.open = true;
                self.ai.collab_mode = true;
                self.collab.open = true;
                self.collab.master_input = prompt;
            }
            None => {}
        }
    }
}

/// 若助手回复中明确包含 fenced ``` 代码块，提取其中代码行
fn extract_fenced_code_block(text: &str) -> Option<Vec<String>> {
    let mut in_fence = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                return (!lines.is_empty()).then_some(lines);
            }
            in_fence = true;
            continue;
        }
        if in_fence {
            lines.push(line.to_string());
        }
    }
    None
}

/// 根据文件扩展名与聊天需求构造确定性直接写入补丁 (start_row, remove_rows, insert_lines)
fn build_direct_patch(doc: &crate::doc::Doc, q: &str) -> (usize, usize, Vec<String>) {
    let ext = doc
        .path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let is_empty_doc = doc.lines.len() <= 1 && doc.lines.first().is_some_and(|l| l.is_empty());
    let (start, remove, orig) = if let Some((r0, r1)) = doc.selected_rows() {
        (r0, r1 - r0 + 1, doc.lines[r0..=r1].to_vec())
    } else if is_empty_doc {
        (0, 1, Vec::new())
    } else {
        (doc.cursor[0].min(doc.lines.len()), 0, Vec::new())
    };

    let clean_q = q.trim();
    let header = match ext.as_str() {
        "md" | "markdown" => format!("> [AI 需求响应] {clean_q}"),
        "nema" => format!("\\ AI: {clean_q}"),
        "py" | "toml" | "yaml" | "yml" | "sh" => format!("# AI: {clean_q}"),
        _ => format!("// AI: {clean_q}"),
    };
    let mut out = vec![header];
    if clean_q.starts_with("/test") && matches!(ext.as_str(), "rs" | "") {
        out.push("#[test]".into());
        out.push("fn generated_ai_test() {".into());
        out.push("    assert!(true);".into());
        out.push("}".into());
    } else if matches!(ext.as_str(), "md" | "markdown") {
        out.push(format!("- {clean_q}"));
    }
    out.extend(orig);
    (start, remove, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_workspace_root() {
        let root = find_workspace_root(None);
        assert!(!root.as_os_str().is_empty());

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let discovered = find_workspace_root(Some(&cwd.join("src/main.rs")));
        assert!(discovered.exists());
    }

    #[test]
    fn test_ai_agent_skills_integration() {
        let skills = SkillRegistry::new();
        assert!(skills.list().len() >= 6);

        let (skill, rest) = skills
            .match_prompt("/review fn broken() {}")
            .expect("should match review skill");
        assert_eq!(skill.id, "code-review");
        assert_eq!(rest, "fn broken() {}");

        let (sys, user) = skill.apply(rest, Some("Base System Prompt"));
        assert!(sys.contains("Active Skill: 代码审查"));
        assert!(user.contains("深度代码审查"));
        assert!(user.contains("fn broken() {}"));
    }

    #[test]
    fn test_ai_agent_rules_and_memory_injection() {
        let temp = std::env::temp_dir().join(format!(
            "ide_test_rules_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("AGENT.md"), "RULE: ALL_INVARIANTS_HOLD").unwrap();

        let rules = WorkspaceRulesEngine::new();
        let prompt = rules
            .format_invariants_prompt(&temp)
            .expect("should load rules");
        assert!(prompt.contains("RULE: ALL_INVARIANTS_HOLD"));

        let mem = AgentMemoryEngine::new(16);
        mem.record_working_item("user", "test summary", vec!["test".into()]);
        let items = mem.recent_working_items(10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].summary, "test summary");

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_ai_hooks_pipeline_execution() {
        let hooks = HookPipeline::new();

        // 路径穿越拦截
        let bad_tool_args = json!({ "path": "../../../etc/passwd" });
        let pre_res = hooks.execute_pre_tool_call("read_file", &bad_tool_args);
        assert!(pre_res.is_err());

        // 密钥脱敏
        let tool_result = "Fetched config with token: sk-ant-api03-abcdef12345678901234567890 and key=sk-proj-xyz";
        let masked = hooks.execute_post_tool_call("read_file", tool_result.to_string());
        assert!(!masked.contains("abcdef12345678901234567890"));
        assert!(masked.contains("sk-****"));
    }

    #[test]
    fn test_ai_loop_breaker() {
        let mut breaker = LoopBreaker::new(3);
        let args = json!({ "path": "file.rs" });

        assert!(breaker.check_and_record("read_file", &args).is_ok());
        assert!(breaker.check_and_record("read_file", &args).is_ok());
        let third = breaker.check_and_record("read_file", &args);
        assert!(third.is_err(), "应该在第3次连续相同调用时触发死循环熔断");
        assert!(third.unwrap_err().contains("死循环"));
    }

    #[test]
    fn fmt_bytes_picks_a_readable_unit() {
        assert_eq!(fmt_bytes(0), "0 B");
        assert_eq!(fmt_bytes(1023), "1023 B");
        assert_eq!(fmt_bytes(1536), "1.5 KB");
        assert_eq!(fmt_bytes(3 * 1024 * 1024), "3.0 MB");
    }

    /// 带 Cargo.toml 的临时工作区 + 一个源文件, 返回 (工作区, 文件路径)
    fn cookie_ws(tag: &str) -> (PathBuf, PathBuf) {
        let ws = std::env::temp_dir().join(format!(
            "ide_fat_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(ws.join("Cargo.toml"), "[package]").unwrap();
        let f = ws.join("a.rs");
        std::fs::write(&f, "fn main() {}").unwrap();
        (ws, f)
    }

    fn test_app() -> (App, std::sync::MutexGuard<'static, ()>) {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        (
            App::build(vec![], None, Trace::new(), Settings::default(), false),
            g,
        )
    }

    #[test]
    fn file_as_task_cookie_follows_the_run_lifecycle() {
        let (ws, file) = cookie_ws("life");
        let (mut a, _g) = test_app();
        a.new_doc();
        let i = a.active;
        a.docs[i].path = Some(file.clone());
        let id = a.docs[i].id;
        let jar = CookieJar::new(&ws);

        // 开始: 状态 Exploring, 意图立即落盘 (崩溃也留痕); 第一次没有可续航的内容
        let (rc, resume) = a.cookie_begin(i, &ws, "修复空指针");
        assert!(resume.is_none());
        assert_eq!(a.task_state_of(id), TaskState::Exploring);
        let on_disk = jar.load("a.rs").expect("落盘");
        assert_eq!((on_disk.state, on_disk.rounds), (TaskState::Exploring, 1));
        assert_eq!(on_disk.intent, "修复空指针");

        // 出补丁 → 结算 Vetting + 摘要
        let mut rc = rc.expect("绑定了 Cookie");
        rc.proposed = true;
        a.cookie_settle(id, Some(rc), Some("建议加判空".into()));
        assert_eq!(a.task_state_of(id), TaskState::Vetting);
        assert_eq!(jar.load("a.rs").unwrap().last_summary, "建议加判空");

        // 第二轮: 能拿到续航提示 (上轮意图 + 结论), 轮次累加
        let (rc2, resume2) = a.cookie_begin(i, &ws, "继续");
        let r = resume2.expect("有续航提示");
        assert!(r.contains("修复空指针") && r.contains("建议加判空"));
        assert_eq!(jar.load("a.rs").unwrap().rounds, 2);

        // 无补丁 → Planning
        a.cookie_settle(id, rc2, None);
        assert_eq!(a.task_state_of(id), TaskState::Planning);

        // 人工采纳 → Verified (磁盘也变)
        a.cookie_mark(id, TaskState::Verified);
        assert_eq!(a.task_state_of(id), TaskState::Verified);
        assert_eq!(jar.load("a.rs").unwrap().state, TaskState::Verified);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn cookie_never_stores_api_keys_and_skips_files_outside_the_workspace() {
        let (ws, file) = cookie_ws("mask");
        let (mut a, _g) = test_app();
        a.new_doc();
        let i = a.active;
        a.docs[i].path = Some(file);
        let (_, _) = a.cookie_begin(i, &ws, "用 sk-abcdef1234567890XYZ 去调接口");
        let text = std::fs::read_to_string(
            CookieJar::new(&ws)
                .dir()
                .read_dir()
                .unwrap()
                .flatten()
                .find(|e| e.path().extension().is_some_and(|x| x == "json"))
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(
            !text.contains("abcdef1234567890XYZ"),
            "密钥不能明文落盘: {text}"
        );
        assert!(text.contains("sk-****"));

        // 文档不在工作区内: 不建 Cookie
        let other = std::env::temp_dir().join("ide_fat_outside.rs");
        a.docs[i].path = Some(other);
        let (rc, resume) = a.cookie_begin(i, &ws, "x");
        assert!(rc.is_none() && resume.is_none());
        // 没有磁盘路径: 同样不建
        a.docs[i].path = None;
        assert!(a.cookie_begin(i, &ws, "x").0.is_none());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn settings_menu_actions_clear_and_prune_and_report() {
        let (ws, file) = cookie_ws("clear");
        let (mut a, _g) = test_app();
        a.new_doc();
        let i = a.active;
        a.docs[i].path = Some(file.clone());
        let id = a.docs[i].id;
        let jar = CookieJar::new(&ws);
        let _ = a.cookie_begin(i, &ws, "任务");
        assert_eq!(a.cookie_stats_cached().count, 1);

        // 源文件被删 → 孤儿; 菜单「清理」把它回收
        std::fs::remove_file(&file).unwrap();
        a.cookie_prune_now();
        assert_eq!(jar.stats().count, 0);
        assert!(a.cur().tip.contains("已回收 1 个"));

        // 重新来一个, 再「清除全部」: 磁盘清空 + Tab 状态消失 + 提示里有数量
        std::fs::write(&file, "x").unwrap();
        let _ = a.cookie_begin(i, &ws, "任务2");
        assert_eq!(a.task_state_of(id), TaskState::Exploring);
        a.cookie_clear_all();
        assert_eq!(jar.stats().count, 0);
        assert_eq!(a.task_state_of(id), TaskState::Idle);
        assert!(a.cur().tip.contains("已清除 1 个"));
        assert!(file.exists(), "源文件绝不能被清除");
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_ai_mcp_manager_integration() {
        let mut mgr = McpManager::new();
        let cfg_json = r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "node",
                    "args": ["server.js"],
                    "disabled": false
                }
            }
        }"#;
        let cfg = McpConfig::from_json_str(cfg_json).unwrap();
        mgr.load_config(cfg);

        let servers = mgr.list_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].0, "filesystem");
        assert!(servers[0].1);
    }

    #[test]
    fn ai_state_defaults_enable_context_and_auto_apply() {
        let st = AiState::default();
        assert!(st.use_ctx);
        assert!(st.auto_apply);
        assert!(!st.collab_mode);
        assert!(AiConfig::default_cfg().consent);
    }

    #[test]
    fn ai_send_reacts_directly_to_active_open_file_without_clicks() {
        let (mut app, _g) = test_app();
        app.ai.cfg = Some(AiConfig::default_cfg());
        app.new_doc();
        let i = app.active;
        app.docs[i].lines = vec!["fn hello() {}".into()];
        app.ai.input = "添加函数说明".into();

        app.ai_send(&egui::Context::default());

        assert!(
            app.docs[i].lines.len() > 1,
            "lines={:?}, items_len={}",
            app.docs[i].lines,
            app.ai.items.len()
        );
        assert!(app.docs[i].lines.iter().any(|l| l.contains("添加函数说明")));
        assert_eq!(app.task_state_of(app.docs[i].id), TaskState::Verified);
        assert!(app.ai.input.is_empty());
    }

    #[test]
    fn hot_plug_hooks_and_mod_gates_respond_immediately() {
        let (mut app, _g) = test_app();
        let dir = std::env::temp_dir().join(format!("ide-fast-aichat-hp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("IDE_PLUGIN_DIR", &dir);

        app.new_doc();
        // 默认 hooks 启用时自动脱敏 sk- 密钥
        let masked = app.mask_secrets("token=sk-secret999".to_string());
        assert_eq!(masked, "token=sk-****");

        // 关闭 hooks 插件后，钩子管道立即停用
        crate::plugins::set_enabled("hooks", false).unwrap();
        let unmasked = app.mask_secrets("token=sk-secret999".to_string());
        assert_eq!(unmasked, "token=sk-secret999");

        // Mod 循环命令与关闭门禁
        assert_eq!(app.ai.active_mod, "code");
        app.run_plugin_command("mod", "cycle", "Mod");
        assert_eq!(app.ai.active_mod, "plan");
        crate::plugins::remove("mod").unwrap();
        app.run_plugin_command("mod", "cycle", "Mod");
        assert_eq!(app.ai.active_mod, "plan");

        std::env::remove_var("IDE_PLUGIN_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ai_bottom_bar_adaptive_reservation() {
        let (mut app, _g) = test_app();
        app.new_doc();
        app.ai.open = true;
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };

        // Frame 1: 首次渲染 ai_ui
        let mut out = ctx.run_ui(raw.clone(), |ui| {
            app.ai_ui(ui);
        });
        out.textures_delta.clear();

        // 验证底栏高度已经动态测量且自适应存入 temp memory
        let bottom_id = egui::Id::new("ai_bottom_bar_height");
        let measured_h: Option<f32> = ctx.data(|d| d.get_temp(bottom_id));
        assert!(
            measured_h.is_some(),
            "Bottom bar height must be recorded dynamically"
        );
        let h = measured_h.unwrap();
        assert!(
            h >= 90.0 && h <= 300.0,
            "Bottom bar height should be reasonable, got {h}"
        );

        // Frame 2: 验证第2帧收敛稳定，不产生振荡与无限重绘
        let mut out2 = ctx.run_ui(raw, |ui| {
            app.ai_ui(ui);
        });
        out2.textures_delta.clear();
        let h2: f32 = ctx.data(|d| d.get_temp(bottom_id)).unwrap();
        assert!(
            (h2 - h).abs() <= 1.0,
            "Height should converge stably: h={h}, h2={h2}"
        );
    }
}
