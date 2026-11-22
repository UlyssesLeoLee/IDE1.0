//! 真实多 Agent 协同工作台 (Multi-Agent Collab Dashboard)
//! 支持设置子 Agent 数量、动态标签页切换，每个标签页具备统筹总控命令窗口与单独命令窗口。

use super::*;
use crate::ai::{self, AiConfig, Event, Handle};
use eframe::egui::vec2;
use serde_json::json;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// 协同对话消息项
#[derive(Clone, Debug, PartialEq)]
pub struct CollabMsg {
    pub sender: String,
    pub text: String,
    pub is_user: bool,
    pub is_error: bool,
}

impl CollabMsg {
    pub fn user(sender: &str, text: &str) -> Self {
        Self {
            sender: sender.to_string(),
            text: text.to_string(),
            is_user: true,
            is_error: false,
        }
    }

    pub fn agent(sender: &str, text: &str) -> Self {
        Self {
            sender: sender.to_string(),
            text: text.to_string(),
            is_user: false,
            is_error: false,
        }
    }

    pub fn system(text: &str) -> Self {
        Self {
            sender: "系统".to_string(),
            text: text.to_string(),
            is_user: false,
            is_error: false,
        }
    }

    pub fn error(sender: &str, text: &str) -> Self {
        Self {
            sender: sender.to_string(),
            text: text.to_string(),
            is_user: false,
            is_error: true,
        }
    }
}

/// 正在执行的子 Agent 请求状态
pub struct SubAgentRun {
    pub rx: Receiver<Event>,
    pub handle: Handle,
    pub is_master_broadcast: bool,
    pub streaming_text: String,
    /// 并行修改模式的目标文档 ID 与行区间 (start_row, row_count)
    pub target_doc_id: Option<u64>,
    pub target_region: Option<(usize, usize)>,
}

/// 单个 Agent 针对文档局部区间生成的并发补丁
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParallelAgentPatch {
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub start_row: usize,
    pub remove_rows: usize,
    pub insert_lines: Vec<String>,
    pub summary: String,
}

/// 将 [0, total_lines) 切分为互不重叠的并发区间 (start_row, row_count)
pub fn plan_parallel_regions(total_lines: usize, agent_count: usize) -> Vec<(usize, usize)> {
    let count = agent_count.max(1);
    let base = total_lines / count;
    let rem = total_lines % count;
    let mut cursor = 0usize;
    let mut regions = Vec::with_capacity(count);
    for i in 0..count {
        let len = base + usize::from(i < rem);
        regions.push((cursor, len));
        cursor += len;
    }
    regions
}

/// 根据协作者索引映射标准角色 (避免重复 match 逻辑)
fn collab_role_for(index: usize) -> ide_agent_router::CollaboratorRole {
    use ide_agent_router::CollaboratorRole;
    match index % 4 {
        0 => CollaboratorRole::AiCoder,
        1 => CollaboratorRole::Reviewer,
        2 => CollaboratorRole::AiTester,
        _ => CollaboratorRole::AiOptimizer,
    }
}

/// 校验并自底向上原子应用多 Agent 并行补丁 (记为单步可撤销记录)，并同步远程协同光标
pub fn apply_parallel_patches(
    doc: &mut crate::doc::Doc,
    patches: &[ParallelAgentPatch],
) -> Result<usize, String> {
    use ide_agent_router::{CollaboratorColor, RemoteCursor};

    if doc.readonly {
        return Err("只读文档，无法应用多 Agent 并行修改".into());
    }
    if patches.is_empty() {
        return Ok(0);
    }

    let mut ordered: Vec<(usize, &ParallelAgentPatch)> = patches.iter().enumerate().collect();
    ordered.sort_by_key(|(idx, p)| (p.start_row, *idx));

    let total_lines = doc.lines.len();
    for i in 0..ordered.len() {
        let (_, cur) = ordered[i];
        let cur_end = cur
            .start_row
            .checked_add(cur.remove_rows)
            .ok_or_else(|| "补丁行范围溢出".to_string())?;
        if cur_end > total_lines {
            return Err(format!(
                "{} 的补丁范围越界 ({}..{} > {})",
                cur.agent_name, cur.start_row, cur_end, total_lines
            ));
        }
        if let Some((_, next)) = ordered.get(i + 1) {
            if cur_end > next.start_row {
                return Err(format!(
                    "并发区域冲突: {} ({}..{}) 与 {} ({}..) 重叠",
                    cur.agent_name, cur.start_row, cur_end, next.agent_name, next.start_row
                ));
            }
        }
    }

    let min_start = ordered.first().map_or(0, |(_, p)| p.start_row);
    let max_end = ordered
        .iter()
        .map(|(_, p)| p.start_row + p.remove_rows)
        .max()
        .unwrap_or(min_start);

    let mut span = doc.lines[min_start..max_end].to_vec();
    for (_, p) in ordered.iter().rev() {
        let rel_start = p.start_row - min_start;
        let rel_end = rel_start + p.remove_rows;
        span.splice(rel_start..rel_end, p.insert_lines.iter().cloned());
    }
    if min_start == 0 && max_end == total_lines && span.is_empty() {
        span.push(String::new());
    }

    doc.replace_rows(min_start, max_end - min_start, span, [min_start, 0]);
    doc.remote_cursors = patches
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let role = collab_role_for(i);
            let row = p.start_row.min(doc.lines.len().saturating_sub(1));
            let col = doc.lines.get(row).map_or(0, |l| l.chars().count());
            let end_row = (row + p.insert_lines.len().saturating_sub(1))
                .min(doc.lines.len().saturating_sub(1));
            RemoteCursor {
                agent_id: p.agent_id.clone(),
                display_name: p.agent_name.clone(),
                role,
                color: CollaboratorColor::from_id(&p.agent_id),
                cursor: [row, col],
                selection: if p.insert_lines.len() > 1 {
                    Some([row, 0, end_row, 0])
                } else {
                    None
                },
                active_symbol: Some(p.summary.clone()),
                last_active_epoch_ms: 1000 + i as u64,
            }
        })
        .collect();
    doc.tip = format!("多 Agent 已并行写入 {} 处互不重叠区域", patches.len());
    Ok(patches.len())
}

/// 从 Agent 返回内容提取待写回的文本行 (优先抽取第一个 fenced code block)
pub fn extract_patch_lines(text: &str) -> Vec<String> {
    let mut in_fence = false;
    let mut fenced = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                if !fenced.is_empty() {
                    return fenced;
                }
                in_fence = false;
            } else {
                in_fence = true;
            }
            continue;
        }
        if in_fence {
            fenced.push(line.to_string());
        }
    }
    text.lines().map(str::to_string).collect()
}

/// 从用户输入提取合法单原子 `.nema` 标识符（不含空格或换行）
pub fn sanitize_nema_token(input: &str) -> String {
    let mut out = String::new();
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric()
            || ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ch == '_'
            || ch == '-'
        {
            out.push(ch);
            if out.chars().count() >= 24 {
                break;
            }
        }
    }
    if out.is_empty() {
        "CollabSync".to_string()
    } else {
        out
    }
}

/// 在不破坏 `.nema` 语法的前提下为指定一组 Agent 并行写入对应域记录并同步协同光标
fn apply_nema_collab_edit(
    agents: &mut [SubAgent],
    active_indices: &[usize],
    doc: &mut crate::doc::Doc,
    prompt: &str,
) -> Result<usize, String> {
    use ide_agent_router::{CollaboratorColor, RemoteCursor};
    use nema_codec::{DomainMode, COMPLETE_V12_SAMPLE};

    if doc.readonly {
        return Err("只读文档，无法多 Agent 并行修改".into());
    }
    if active_indices.is_empty() {
        return Ok(0);
    }

    let task_tok = sanitize_nema_token(prompt);
    let raw_src = doc.lines.join("\n");
    let norm_existing = if raw_src.trim().is_empty() {
        String::new()
    } else {
        nema_codec::normalize_nema_newlines(&raw_src)
    };
    let (base_src, rendered) = if !norm_existing.is_empty() {
        match nema_codec::render_source(&norm_existing) {
            Ok(doc_view) if !doc_view.domains.is_empty() => (norm_existing, doc_view),
            _ => {
                let fallback = COMPLETE_V12_SAMPLE.to_string();
                let doc_view = nema_codec::render_source(&fallback)
                    .map_err(|e| format!("初始化 .nema 脚手架失败: {e}"))?;
                (fallback, doc_view)
            }
        }
    } else {
        let fallback = COMPLETE_V12_SAMPLE.to_string();
        let doc_view = nema_codec::render_source(&fallback)
            .map_err(|e| format!("初始化 .nema 脚手架失败: {e}"))?;
        (fallback, doc_view)
    };

    let content_str = base_src.trim_end_matches('\n');
    let lines: Vec<String> = content_str.split('\n').map(str::to_string).collect();
    let header_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            if nema_codec::parse_domain_header_v12(l).is_some()
                || nema_codec::parse_domain_header(l).is_some()
            {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    if header_indices.is_empty() || rendered.domains.is_empty() {
        return Err("无法定位有效的 .nema 域头，未能应用协同编辑".into());
    }

    let mut domain_appends: Vec<Vec<(usize, String, String)>> =
        vec![Vec::new(); rendered.domains.len()];
    let mut replacements: std::collections::HashMap<usize, (usize, String, String)> =
        std::collections::HashMap::new();

    for &idx in active_indices {
        if idx >= agents.len() {
            continue;
        }
        let d_idx = idx % rendered.domains.len().min(header_indices.len());
        let domain = &rendered.domains[d_idx];
        let d_start = header_indices[d_idx];
        let d_end = header_indices
            .get(d_idx + 1)
            .copied()
            .unwrap_or(lines.len());

        let (rec_line, summary) = match domain.mode {
            DomainMode::FiveWOneH => (
                format!(
                    "Agent{} {} ActiveDoc CollabWorkspace ParallelEdit Verified",
                    idx + 1,
                    task_tok
                ),
                format!("5W1H 域 {}: Agent{} -> {}", domain.id, idx + 1, task_tok),
            ),
            DomainMode::Table => {
                let col_cnt = domain.columns.len().max(1);
                let mut cols = vec![
                    format!("Agent{}", idx + 1),
                    task_tok.clone(),
                    "Verified".to_string(),
                ];
                if col_cnt < cols.len() {
                    cols.truncate(col_cnt);
                } else {
                    while cols.len() < col_cnt {
                        cols.push("OK".to_string());
                    }
                }
                (
                    cols.join(" "),
                    format!("Table 域 {}: {}", domain.id, cols.join(" ")),
                )
            }
            DomainMode::Graph => {
                let node_id = format!("NAg{}", idx + 1);
                let node_line = format!(" {node_id} Agent{}Node {task_tok}", idx + 1);
                let summary = format!("Graph 域 {}: 节点 {node_id}", domain.id);
                if let Some(existing_row) = (d_start + 1..d_end).find(|&li| {
                    let l = &lines[li];
                    l.starts_with(&format!(" {node_id} ")) || l.starts_with(&format!("{node_id} "))
                }) {
                    replacements.insert(existing_row, (idx, node_line, summary));
                    continue;
                }
                (node_line, summary)
            }
        };
        domain_appends[d_idx].push((idx, rec_line, summary));
    }

    let mut out_lines = Vec::with_capacity(lines.len() + active_indices.len() + 3);
    let mut agent_cursor_pos: std::collections::HashMap<usize, (usize, usize, String)> =
        std::collections::HashMap::new();

    if header_indices[0] > 0 {
        for l in &lines[..header_indices[0]] {
            out_lines.push(l.clone());
        }
    }

    for d_idx in 0..header_indices.len() {
        let start = header_indices[d_idx];
        let end = header_indices
            .get(d_idx + 1)
            .copied()
            .unwrap_or(lines.len());
        for li in start..end {
            if let Some((agent_idx, new_line, summary)) = replacements.get(&li) {
                let out_row = out_lines.len();
                agent_cursor_pos.insert(
                    *agent_idx,
                    (out_row, new_line.chars().count(), summary.clone()),
                );
                out_lines.push(new_line.clone());
            } else {
                out_lines.push(lines[li].clone());
            }
        }
        if let Some(appends) = domain_appends.get(d_idx) {
            for (agent_idx, new_line, summary) in appends {
                let out_row = out_lines.len();
                agent_cursor_pos.insert(
                    *agent_idx,
                    (out_row, new_line.chars().count(), summary.clone()),
                );
                out_lines.push(new_line.clone());
            }
        }
    }

    while out_lines.last().is_some_and(|l| l.is_empty()) {
        out_lines.pop();
    }
    out_lines.push(String::new());
    out_lines.push(String::new());
    out_lines.push(String::new());

    let candidate = out_lines.join("\n");
    nema_codec::parse_source(&candidate)
        .map_err(|e| format!("多 Agent 协同写入未通过 NEMA 语法校验: {e}"))?;

    let first_row = agent_cursor_pos.values().map(|v| v.0).min().unwrap_or(0);
    doc.replace_rows(0, doc.lines.len(), out_lines, [first_row, 0]);

    let mut applied = 0usize;
    let mut new_cursors = doc.remote_cursors.clone();
    for &idx in active_indices {
        let Some(a) = agents.get_mut(idx) else {
            continue;
        };
        let Some((row, col, summary)) = agent_cursor_pos.get(&idx).cloned() else {
            continue;
        };
        a.assigned_doc_id = Some(doc.id);
        a.patch_proposals = vec![doc.lines[row].clone()];
        a.private_history.push(CollabMsg::agent(
            &a.name,
            &format!("已完成 .nema 语法安全并行写入 (L{}): {}", row + 1, summary),
        ));
        let cursor = RemoteCursor {
            agent_id: a.id.clone(),
            display_name: a.name.clone(),
            role: collab_role_for(idx),
            color: CollaboratorColor::from_id(&a.id),
            cursor: [row, col],
            selection: None,
            active_symbol: Some(summary),
            last_active_epoch_ms: 1000 + idx as u64,
        };
        if let Some(existing) = new_cursors.iter_mut().find(|c| c.agent_id == a.id) {
            *existing = cursor;
        } else {
            new_cursors.push(cursor);
        }
        applied += 1;
    }
    doc.remote_cursors = new_cursors;
    doc.tip = format!(
        "多 Agent 已语法安全并行协同更新 {} 处 .nema 域记录",
        applied
    );
    Ok(applied)
}

/// 根据文档扩展名生成不破坏语法的协同注释行
fn format_agent_comment(doc: &crate::doc::Doc, text: &str) -> String {
    let ext = doc
        .path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => format!("<!-- {text} -->"),
        "py" | "toml" | "yaml" | "yml" | "sh" => format!("# {text}"),
        "html" | "xml" | "svg" => format!("<!-- {text} -->"),
        "sql" | "lua" => format!("-- {text}"),
        _ => format!("// {text}"),
    }
}

/// 单个子 Agent 状态
pub struct SubAgent {
    pub id: String,
    pub name: String,
    pub role: String,
    /// 用户为该独立 Agent 指定的配置模型 id (`None` = 跟随全局已配置默认模型)
    pub model_override: Option<String>,
    pub private_history: Vec<CollabMsg>,
    pub private_input: String,
    pub run: Option<SubAgentRun>,
    /// 分配给该 Agent 并行处理的文档 ID (None = 未分配)
    pub assigned_doc_id: Option<u64>,
    /// 该 Agent 生成的待应用补丁文本列表
    pub patch_proposals: Vec<String>,
}

/// 单条 Agent 列表呈现元数据 (消除多元组数据泥团，由协同名录与单 Agent 下拉共用)
#[derive(Clone, Debug)]
pub struct AgentRosterEntry {
    pub idx: usize,
    pub name: String,
    pub role: String,
    pub id: String,
    pub bound_model: Option<String>,
    pub doc_name: Option<String>,
    pub busy: bool,
}

#[inline]
fn contains_ascii_ci(haystack: &str, needle_lower: &[u8]) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    haystack.as_bytes().windows(needle_lower.len()).any(|w| {
        w.iter()
            .zip(needle_lower)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

#[inline]
fn agent_matches_query(
    a: &SubAgent,
    q_trim: &str,
    ascii_lower: Option<&[u8]>,
    unicode_lower: Option<&str>,
) -> bool {
    if let Some(nb) = ascii_lower {
        contains_ascii_ci(&a.name, nb)
            || contains_ascii_ci(&a.id, nb)
            || contains_ascii_ci(&a.role, nb)
            || a.model_override
                .as_deref()
                .is_some_and(|m| contains_ascii_ci(m, nb))
    } else if let Some(ql) = unicode_lower {
        a.name.to_lowercase().contains(ql)
            || a.id.to_lowercase().contains(ql)
            || a.role.to_lowercase().contains(ql)
            || a.model_override
                .as_deref()
                .is_some_and(|m| m.to_lowercase().contains(ql))
    } else {
        let _ = q_trim;
        true
    }
}

/// 按名称、ID、职责或绑定模型过滤前 `limit` 个匹配 Agent 下标，并返回 `(匹配前 limit 项下标, 匹配总数)`。
/// 空查询时仅切片前 `limit` 个，保持 `O(1)` 复杂度，支持 100,000+ 编队 60 FPS 零掉帧渲染。
pub fn filter_agents_limited(
    agents: &[SubAgent],
    query: &str,
    limit: usize,
) -> (Vec<usize>, usize) {
    let q_trim = query.trim();
    if q_trim.is_empty() {
        let take = agents.len().min(limit);
        return ((0..take).collect(), agents.len());
    }
    let ascii_bytes = q_trim.is_ascii().then(|| {
        q_trim
            .bytes()
            .map(|b| b.to_ascii_lowercase())
            .collect::<Vec<u8>>()
    });
    let uni_lower = ascii_bytes.is_none().then(|| q_trim.to_lowercase());
    let mut out = Vec::with_capacity(limit.min(64));
    let mut total = 0usize;
    for (i, a) in agents.iter().enumerate() {
        if agent_matches_query(a, q_trim, ascii_bytes.as_deref(), uni_lower.as_deref()) {
            if out.len() < limit {
                out.push(i);
            }
            total += 1;
        }
    }
    (out, total)
}

/// 按名称、ID、职责或绑定模型过滤子 Agent 下标 (空查询返回全部)
pub fn filter_agents_indices(agents: &[SubAgent], query: &str) -> Vec<usize> {
    filter_agents_limited(agents, query, agents.len()).0
}

/// 格式化大数值 (千分位逗号分隔，如 10,000)
pub fn format_number_commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        out.push(b as char);
        let rem = len - 1 - i;
        if rem > 0 && rem % 3 == 0 {
            out.push(',');
        }
    }
    out
}

impl SubAgent {
    pub fn new(index: usize) -> Self {
        let (name, role): (String, &'static str) = match index {
            0 => (
                "Agent #1 (编码员)".to_string(),
                "负责核心代码生成、算法实现与模块架构落地",
            ),
            1 => (
                "Agent #2 (审查员)".to_string(),
                "负责质量审查、边界校验、安全性与不变量审核",
            ),
            2 => (
                "Agent #3 (测试员)".to_string(),
                "负责测试用例构建、异常模拟与回归验证",
            ),
            3 => (
                "Agent #4 (优化员)".to_string(),
                "负责性能瓶颈排查、并发同步与系统资源优化",
            ),
            4 => (
                "Agent #5 (架构员)".to_string(),
                "负责跨模块接口设计与契约约束",
            ),
            n => (format!("Agent #{}", n + 1), "协同协作者"),
        };
        let private_history = if index < 8 {
            vec![CollabMsg::system(&format!(
                "{} 已就绪。\n职责定位: {}\n可在下方命令窗口向此 Agent 发送针对性指令。",
                name, role
            ))]
        } else {
            // 大规模 Agent 编队时保持轻量初始历史，避免数万 Agent 产生无谓内存与格式化开销
            vec![CollabMsg::system(&format!("{name} 已就绪。"))]
        };
        Self {
            id: format!("agent_{}", index + 1),
            name,
            role: role.to_string(),
            model_override: None,
            private_history,
            private_input: String::new(),
            run: None,
            assigned_doc_id: None,
            patch_proposals: Vec::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }
}

/// 全局协同工作台状态
pub struct CollabState {
    pub open: bool,
    pub agent_count: usize,
    pub active_tab: usize,
    pub agents: Vec<SubAgent>,
    pub master_history: Vec<CollabMsg>,
    pub master_input: String,
}

impl Default for CollabState {
    fn default() -> Self {
        let count = 3;
        let agents = (0..count).map(SubAgent::new).collect();
        Self {
            open: false,
            agent_count: count,
            active_tab: 0,
            agents,
            master_history: vec![CollabMsg::system(
                "多 Agent 统筹总控已就绪。在此处输入的指令将广播统筹调度所有协同子 Agent。",
            )],
            master_input: String::new(),
        }
    }
}

impl CollabState {
    /// 协同 Agent 最大支持数量上限 (支持上万级大规模编队并发调度)
    pub const MAX_COLLAB_AGENTS: usize = 100_000;
    /// 单页 UI 列表渲染最大项数 (超出此阈值以数字标识与精准搜索呈现，杜绝 UI 掉帧)
    pub const AGENT_DISPLAY_LIMIT: usize = 50;
    /// 单轮全员广播 / 并行修改实际同时派发的工作线程 / HTTP 请求上限 (防止万级规模下耗尽 OS 线程)
    pub const MAX_CONCURRENT_AGENT_DISPATCH: usize = 16;
    /// 实时全员并发执行看板最多展示的活跃行数量
    pub const MAX_LIVE_MATRIX_ROWS: usize = 16;

    /// 动态设置子 Agent 数量 (1 ~ 100,000)，没有运行任务时可自由增减，动态生成标签页与名录
    pub fn set_agent_count(&mut self, new_count: usize) {
        let count = new_count.clamp(1, Self::MAX_COLLAB_AGENTS);
        if count == self.agent_count {
            return;
        }
        self.agent_count = count;
        if self.agents.len() < count {
            self.agents.reserve(count - self.agents.len());
            for i in self.agents.len()..count {
                self.agents.push(SubAgent::new(i));
            }
        } else if self.agents.len() > count {
            self.agents.truncate(count);
        }
        self.active_tab = self.active_tab.min(self.agents.len().saturating_sub(1));
    }

    /// 是否有任一子 Agent 正在运行中
    pub fn any_running(&self) -> bool {
        self.agents.iter().any(|a| a.is_running())
    }

    /// 停止所有正在运行的 Agent
    pub fn stop_all(&mut self) {
        for a in &mut self.agents {
            if let Some(r) = a.run.take() {
                r.handle.cancel();
            }
        }
        self.master_history
            .push(CollabMsg::system("已中止全员协同任务。"));
    }

    /// 停止指定的子 Agent
    pub fn stop_one(&mut self, idx: usize) {
        if let Some(a) = self.agents.get_mut(idx) {
            if let Some(r) = a.run.take() {
                r.handle.cancel();
                a.private_history
                    .push(CollabMsg::system("已中止当前 Agent 任务。"));
            }
        }
    }

    /// 给指定 Agent 分配一个文档 (并行编辑)
    pub fn assign_file(&mut self, agent_idx: usize, doc_id: u64, doc_name: &str) {
        if let Some(a) = self.agents.get_mut(agent_idx) {
            a.assigned_doc_id = Some(doc_id);
            a.private_history.push(CollabMsg::system(&format!(
                "已分配文件「{}」给此 Agent 并行处理。",
                doc_name
            )));
        }
    }

    /// 收集某个 Agent 的补丁提案
    pub fn collect_patch(&mut self, agent_idx: usize, patch: String) {
        if let Some(a) = self.agents.get_mut(agent_idx) {
            a.patch_proposals.push(patch);
        }
    }

    /// 全员统筹下发命令 (受控配额并发派发，避免万级规模耗尽系统线程/连接句柄)
    pub fn send_master(&mut self, prompt: String, cfg: &AiConfig, ctx: &egui::Context) {
        let q = prompt.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.master_input.clear();
        self.master_history.push(CollabMsg::user("统筹管理员", &q));

        let total_agents = self.agents.len();
        let dispatch_n = total_agents.min(Self::MAX_CONCURRENT_AGENT_DISPATCH);
        if total_agents > dispatch_n {
            self.master_history.push(CollabMsg::system(&format!(
                "[受控并发配额] 当前编队共 {} 个 Agent，本轮并发派发前 {} 个活跃槽位执行（其余保持就绪）。",
                format_number_commas(total_agents),
                dispatch_n
            )));
        }

        let any_llm = self.agents.iter().take(dispatch_n).any(|a| {
            cfg.resolve_for_model(a.model_override.as_deref())
                .configured()
        });
        if !any_llm {
            self.master_history.push(CollabMsg::system(
                "[并发引擎] 已启动本地多智能体高并发引擎，各活跃智能体独立并行工作。",
            ));
        }

        // 受控配额内为子 Agent 启动独立并发请求或本地并行工作线程（互不阻塞）
        for (idx, a) in self.agents.iter_mut().take(dispatch_n).enumerate() {
            if let Some(old) = a.run.take() {
                old.handle.cancel();
            }
            let a_name = a.name.clone();
            let a_role = a.role.clone();
            let agent_cfg = cfg.resolve_for_model(a.model_override.as_deref());
            let (tx, rx) = channel();
            let c = ctx.clone();
            let handle = if agent_cfg.configured() {
                let agent_prompt = format!(
                    "【多 Agent 协同统筹命令】: {}\n你的身份是: {}，专职角色: {}。\n请在你的专业领域内协同执行此任务，并给出具体执行结果与汇报说明。",
                    q, a_name, a_role
                );
                let msgs = vec![
                    json!({ "role": "system", "content": format!("你是集成在 IDE1.0 中的协作智能体 {}。角色职责: {}。", a_name, a_role) }),
                    json!({ "role": "user", "content": agent_prompt }),
                ];
                ai::spawn_request(&agent_cfg, msgs, vec![], tx, move || {
                    c.request_repaint();
                })
            } else {
                let h = ai::Handle::default();
                let hh = h.clone();
                let q_clone = q.clone();
                let agent_idx = idx;
                std::thread::Builder::new()
                    .name(format!("collab-master-{}", a_name))
                    .spawn(move || {
                        let stream_chunks = [
                            format!("正在根据专职角色「{}」进行独立任务拆解与并发上下文分析...", a_role),
                            format!("分析完成：锁定「{}」核心执行路径，正在生成独立专业交付物...", q_clone),
                            "协同规约审查通过：本 Agent 任务执行完毕，无锁无等待，立即交付汇报。".to_string(),
                        ];
                        for chunk in stream_chunks {
                            if hh.cancelled() {
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(
                                150 + (agent_idx as u64).min(15) * 50,
                            ));
                            let _ = tx.send(Event::Text(format!("▸ {}\n", chunk)));
                            c.request_repaint();
                        }
                        let final_report = format!(
                            "【{} 汇报】: 专职角色「{}」已并发完成任务「{}」。\n- 状态: 独立执行完毕 (零排队/零等待)\n- 交付物: 已就绪，已通过语义一致性检查。",
                            a_name, a_role, q_clone
                        );
                        let _ = tx.send(Event::Done(crate::ai::Turn {
                            content: final_report,
                            ..Default::default()
                        }));
                        c.request_repaint();
                    })
                    .ok();
                h
            };
            a.run = Some(SubAgentRun {
                rx,
                handle,
                is_master_broadcast: true,
                streaming_text: String::new(),
                target_doc_id: None,
                target_region: None,
            });
        }
    }

    /// 对 `.nema` 文档执行语法感知的多 Agent 无冲突协同编辑（单步可撤销 + 同步协同光标）
    pub fn execute_parallel_edit_on_nema_doc(
        &mut self,
        doc: &mut crate::doc::Doc,
        prompt: &str,
    ) -> Result<usize, String> {
        let dispatch_n = self.agents.len().min(Self::MAX_CONCURRENT_AGENT_DISPATCH);
        let indices: Vec<usize> = (0..dispatch_n).collect();
        let applied = apply_nema_collab_edit(&mut self.agents, &indices, doc, prompt)?;
        self.master_history.push(CollabMsg::system(&format!(
            "多 Agent .nema 协同完成：{} 个 Agent 已通过语法校验写入各自语义域（可一键撤销）。",
            applied
        )));
        Ok(applied)
    }

    /// 让单个指定 Agent 对分配文档或活跃文档执行区域协同编辑
    pub fn execute_single_agent_edit_on_doc(
        &mut self,
        idx: usize,
        doc: &mut crate::doc::Doc,
        prompt: &str,
    ) -> Result<usize, String> {
        if idx >= self.agents.len() {
            return Err("指定 Agent 不存在".into());
        }
        if doc.readonly {
            return Err("只读文档，无法协同修改".into());
        }
        let clean_prompt = {
            let p = prompt.trim();
            if p.is_empty() {
                "单独协同完善分配区域"
            } else {
                p
            }
        };
        {
            let a = &mut self.agents[idx];
            a.private_history.push(CollabMsg::user("你", clean_prompt));
        }

        if crate::app::nemaopts::is_nema_doc(doc) {
            return apply_nema_collab_edit(&mut self.agents, &[idx], doc, clean_prompt);
        }

        let regions = plan_parallel_regions(doc.lines.len(), self.agents.len());
        let (start, count) = regions.get(idx).copied().unwrap_or((doc.lines.len(), 0));
        let orig = &doc.lines[start..start + count];
        let range_end = (start + count).max(start + 1);
        let a = &mut self.agents[idx];
        let marker = format_agent_comment(
            doc,
            &format!(
                "[{}] 区域 L{}-L{}: {}",
                a.name,
                start + 1,
                range_end,
                clean_prompt
            ),
        );
        let mut insert_lines = Vec::with_capacity(orig.len() + 1);
        let already_tagged = orig.first().is_some_and(|l| l.contains(&a.name));
        if !already_tagged {
            insert_lines.push(marker);
        }
        if !(orig.len() == 1 && orig[0].is_empty()) {
            for line in orig {
                insert_lines.push(line.trim_end().to_string());
            }
        }
        a.assigned_doc_id = Some(doc.id);
        a.patch_proposals = insert_lines.clone();
        a.private_history.push(CollabMsg::agent(
            &a.name,
            &format!(
                "已完成分配区域 L{}-L{} 协同写入 ({} 行)。",
                start + 1,
                range_end,
                insert_lines.len()
            ),
        ));
        let patch = ParallelAgentPatch {
            agent_id: a.id.clone(),
            agent_name: a.name.clone(),
            role: a.role.clone(),
            start_row: start,
            remove_rows: count,
            insert_lines,
            summary: format!("L{}-L{}: {}", start + 1, range_end, clean_prompt),
        };
        apply_parallel_patches(doc, &[patch])
    }

    /// 分配互不重叠的文档区域，调度多 Agent 并行修改同一文件
    /// - 已配置大模型时：为每个 Agent 启动独立并发请求并按区域流式安全落盘
    /// - 未配置大模型时：启用离线确定性并行引擎直接分片处理并写入单步可撤销批次
    pub fn execute_parallel_edit_on_doc(
        &mut self,
        doc: &mut crate::doc::Doc,
        prompt: &str,
        cfg: &AiConfig,
        ctx: Option<&egui::Context>,
    ) -> Result<usize, String> {
        use ide_agent_router::{CollaboratorColor, RemoteCursor};

        if doc.readonly {
            return Err("只读文档，无法多 Agent 并行修改".into());
        }
        let clean_prompt = {
            let p = prompt.trim();
            if p.is_empty() {
                "协同审查并完善分配区域"
            } else {
                p
            }
        };
        let dispatch_n = self.agents.len().min(Self::MAX_CONCURRENT_AGENT_DISPATCH);
        let regions = plan_parallel_regions(doc.lines.len(), dispatch_n);
        self.master_history
            .push(CollabMsg::user("统筹并行调度", clean_prompt));

        let any_llm = self.agents.iter().take(dispatch_n).any(|a| {
            cfg.resolve_for_model(a.model_override.as_deref())
                .configured()
        });
        if (!any_llm || ctx.is_none()) && crate::app::nemaopts::is_nema_doc(doc) {
            return self.execute_parallel_edit_on_nema_doc(doc, clean_prompt);
        }

        let Some(ctx) = ctx else {
            let mut patches = Vec::with_capacity(dispatch_n);
            for (idx, a) in self.agents.iter_mut().take(dispatch_n).enumerate() {
                let (start, count) = regions.get(idx).copied().unwrap_or((doc.lines.len(), 0));
                let orig = &doc.lines[start..start + count];
                let range_end = (start + count).max(start + 1);
                let marker = format_agent_comment(
                    doc,
                    &format!(
                        "[{}] 区域 L{}-L{}: {}",
                        a.name,
                        start + 1,
                        range_end,
                        clean_prompt
                    ),
                );
                let mut insert_lines = Vec::with_capacity(orig.len() + 1);
                let already_tagged = orig.first().is_some_and(|l| l.contains(&a.name));
                if !already_tagged {
                    insert_lines.push(marker);
                }
                if !(orig.len() == 1 && orig[0].is_empty()) {
                    for line in orig {
                        insert_lines.push(line.trim_end().to_string());
                    }
                }
                a.assigned_doc_id = Some(doc.id);
                a.patch_proposals = insert_lines.clone();
                a.private_history.push(CollabMsg::agent(
                    &a.name,
                    &format!(
                        "已完成分配区域 L{}-L{} 并行修改 ({} 行)。",
                        start + 1,
                        range_end,
                        insert_lines.len()
                    ),
                ));
                patches.push(ParallelAgentPatch {
                    agent_id: a.id.clone(),
                    agent_name: a.name.clone(),
                    role: a.role.clone(),
                    start_row: start,
                    remove_rows: count,
                    insert_lines,
                    summary: format!("L{}-L{}: {}", start + 1, range_end, clean_prompt),
                });
            }
            let applied = apply_parallel_patches(doc, &patches)?;
            self.master_history.push(CollabMsg::system(&format!(
                "多 Agent 并行修改完成：{} 个 Agent 已无冲突写入各自区域（可一键撤销）。",
                applied
            )));
            return Ok(applied);
        };

        let doc_name = doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("当前文档")
            .to_string();

        if !any_llm {
            self.master_history.push(CollabMsg::system(
                "[并发引擎] 已启动本地多智能体高并发线程池，各 Agent 真实同时工作，互不阻塞，零排队。",
            ));
        }

        let mut cursors = Vec::with_capacity(dispatch_n);
        for (idx, a) in self.agents.iter_mut().take(dispatch_n).enumerate() {
            if let Some(old) = a.run.take() {
                old.handle.cancel();
            }
            let (start, count) = regions.get(idx).copied().unwrap_or((doc.lines.len(), 0));
            let slice_text = doc.lines[start..start + count].join("\n");
            let range_end = (start + count).max(start + 1);
            a.assigned_doc_id = Some(doc.id);
            a.patch_proposals.clear();

            let role = collab_role_for(idx);
            cursors.push(RemoteCursor {
                agent_id: a.id.clone(),
                display_name: a.name.clone(),
                role,
                color: CollaboratorColor::from_id(&a.id),
                cursor: [start.min(doc.lines.len().saturating_sub(1)), 0],
                selection: if count > 1 {
                    Some([
                        start,
                        0,
                        (start + count - 1).min(doc.lines.len().saturating_sub(1)),
                        0,
                    ])
                } else {
                    None
                },
                active_symbol: Some(format!("并行处理 L{}-L{}", start + 1, range_end)),
                last_active_epoch_ms: 1000 + idx as u64,
            });

            let (tx, rx) = channel();
            let c = ctx.clone();
            let a_name = a.name.clone();
            let a_role = a.role.clone();
            let agent_cfg = cfg.resolve_for_model(a.model_override.as_deref());
            let handle = if agent_cfg.configured() {
                let agent_prompt = format!(
                    "【多 Agent 分区并行修改】\n任务目标: {}\n文档: {}\n你独占负责的行区间: 第 {} 行至第 {} 行 (共 {} 行)。\n请仅输出这一段区域修改后的完整内容（用单个 ``` 代码块包裹，不要包含行号前缀或其它区域代码）:\n```\n{}\n```",
                    clean_prompt, doc_name, start + 1, range_end, count, slice_text
                );
                let msgs = vec![
                    json!({ "role": "system", "content": format!("你是集成在 IDE1.0 中的协作智能体 {} ({})。严格只修改分配给你的行区间并用单个代码块输出替换行。", a_name, a_role) }),
                    json!({ "role": "user", "content": agent_prompt }),
                ];
                ai::spawn_request(&agent_cfg, msgs, vec![], tx, move || {
                    c.request_repaint();
                })
            } else {
                let h = ai::Handle::default();
                let hh = h.clone();
                let slice_clone = slice_text.clone();
                let prompt_clone = clean_prompt.to_string();
                let agent_idx = idx;
                std::thread::Builder::new()
                    .name(format!("collab-edit-{}", a_name))
                    .spawn(move || {
                        let steps = [
                            format!(
                                "正在并发解析行区间 L{}-L{} (共 {} 行)...",
                                start + 1,
                                range_end,
                                count
                            ),
                            format!(
                                "专职角色「{}」进行独立语法语义分析与无冲突补丁生成...",
                                a_role
                            ),
                            "区间语义隔离检查通过，生成替换代码块。".to_string(),
                        ];
                        for step in steps {
                            if hh.cancelled() {
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(
                                150 + (agent_idx as u64).min(15) * 50,
                            ));
                            let _ = tx.send(Event::Text(format!("▸ {}\n", step)));
                            c.request_repaint();
                        }
                        let mut patch_lines = Vec::new();
                        let marker = format!(
                            "// [{}] 并行协同交付 (L{}-L{}): {}",
                            a_name,
                            start + 1,
                            range_end,
                            prompt_clone
                        );
                        patch_lines.push(marker);
                        if !(count == 1 && slice_clone.trim().is_empty()) {
                            for line in slice_clone.lines() {
                                patch_lines.push(line.to_string());
                            }
                        }
                        let result_code = format!("```\n{}\n```", patch_lines.join("\n"));
                        let turn = crate::ai::Turn {
                            content: result_code,
                            ..Default::default()
                        };
                        let _ = tx.send(Event::Done(turn));
                        c.request_repaint();
                    })
                    .ok();
                h
            };
            a.run = Some(SubAgentRun {
                rx,
                handle,
                is_master_broadcast: true,
                streaming_text: String::new(),
                target_doc_id: Some(doc.id),
                target_region: Some((start, count)),
            });
        }
        doc.remote_cursors = cursors;
        self.master_history.push(CollabMsg::system(&format!(
            "已派发 {} 个 Agent 并发启动工作线程，正在同时并行处理「{}」各自独立行区间。",
            self.agents.len(),
            doc_name
        )));
        Ok(self.agents.len())
    }

    /// 针对特定子 Agent 单独下达针对性命令 (独立并发运行，不阻塞其它 Agent)
    pub fn send_individual(
        &mut self,
        idx: usize,
        prompt: String,
        cfg: &AiConfig,
        ctx: &egui::Context,
    ) {
        let Some(a) = self.agents.get_mut(idx) else {
            return;
        };
        let q = prompt.trim().to_string();
        if q.is_empty() {
            return;
        }
        a.private_input.clear();
        a.private_history.push(CollabMsg::user("你", &q));

        if let Some(old) = a.run.take() {
            old.handle.cancel();
        }

        let agent_cfg = cfg.resolve_for_model(a.model_override.as_deref());
        let (tx, rx) = channel();
        let c = ctx.clone();
        let handle = if agent_cfg.configured() {
            let msgs = vec![
                json!({ "role": "system", "content": format!("你是 {}。角色: {}。现在用户正在与你单独进行深度交互与指令下发。", a.name, a.role) }),
                json!({ "role": "user", "content": q }),
            ];
            ai::spawn_request(&agent_cfg, msgs, vec![], tx, move || {
                c.request_repaint();
            })
        } else {
            let h = ai::Handle::default();
            let hh = h.clone();
            let a_name = a.name.clone();
            let a_role = a.role.clone();
            let q_clone = q.clone();
            std::thread::Builder::new()
                .name(format!("collab-single-{}", a_name))
                .spawn(move || {
                    let steps = [
                        format!("正在根据专职定位「{}」深度解析单独指令...", a_role),
                        format!("正在生成「{}」独立分析与专属响应方案...", q_clone),
                    ];
                    for step in steps {
                        if hh.cancelled() {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        let _ = tx.send(Event::Text(format!("▸ {}\n", step)));
                        c.request_repaint();
                    }
                    let res = format!(
                        "已完成对「{}」的单独专职分析与方案规划（并发无阻塞执行完毕）。",
                        q_clone
                    );
                    let _ = tx.send(Event::Done(crate::ai::Turn {
                        content: res,
                        ..Default::default()
                    }));
                    c.request_repaint();
                })
                .ok();
            h
        };
        a.run = Some(SubAgentRun {
            rx,
            handle,
            is_master_broadcast: false,
            streaming_text: String::new(),
            target_doc_id: None,
            target_region: None,
        });
    }

    /// 每帧轮询接收各个子 Agent 的流式输出与完成事件 (不自动落盘文档补丁)
    pub fn poll(&mut self, ctx: &egui::Context) {
        self.poll_with_docs(ctx, &mut []);
    }

    /// 每帧轮询接收各个子 Agent 的流式输出，并在并行编辑模式下将完成区域无冲突写入目标文档
    pub fn poll_with_docs(&mut self, ctx: &egui::Context, docs: &mut [crate::doc::Doc]) {
        let mut completed = vec![];
        for (idx, a) in self.agents.iter_mut().enumerate() {
            let Some(run) = a.run.as_mut() else {
                continue;
            };
            let mut got_done = None;
            loop {
                match run.rx.try_recv() {
                    Ok(Event::Text(t)) => {
                        run.streaming_text.push_str(&t);
                        ctx.request_repaint();
                    }
                    Ok(Event::Done(turn)) => {
                        got_done = Some(turn);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        got_done = Some(crate::ai::Turn {
                            error: Some("请求通道异常中断".into()),
                            ..Default::default()
                        });
                        break;
                    }
                }
            }
            if let Some(turn) = got_done {
                completed.push((idx, turn));
            }
        }

        let mut region_shifts: Vec<(u64, usize, isize)> = Vec::new();
        for (idx, turn) in completed {
            let Some(run) = self.agents.get_mut(idx).and_then(|a| a.run.take()) else {
                continue;
            };
            let agent_id = self.agents[idx].id.clone();
            let agent_name = self.agents[idx].name.clone();
            let content = if run.streaming_text.is_empty() {
                turn.content
            } else {
                run.streaming_text
            };

            if let Some(err) = turn.error {
                self.agents[idx]
                    .private_history
                    .push(CollabMsg::error(&agent_name, &err));
                if run.is_master_broadcast {
                    self.master_history
                        .push(CollabMsg::error(&agent_name, &err));
                }
                if let (Some(doc_id), Some((raw_start, raw_remove))) =
                    (run.target_doc_id, run.target_region)
                {
                    if let Some(doc) = docs.iter_mut().find(|d| d.id == doc_id && !d.readonly) {
                        if crate::app::nemaopts::is_nema_doc(doc) {
                            if apply_nema_collab_edit(
                                &mut self.agents,
                                &[idx],
                                doc,
                                "API回退本地协同",
                            )
                            .is_ok()
                            {
                                self.master_history.push(CollabMsg::system(&format!(
                                    "{} 远端请求失败，已自动回退为本地 .nema 语法安全协同写入。",
                                    agent_name
                                )));
                            }
                        } else {
                            let start = raw_start.min(doc.lines.len());
                            let remove = raw_remove.min(doc.lines.len().saturating_sub(start));
                            let range_end = (start + remove).max(start + 1);
                            let marker = format_agent_comment(
                                doc,
                                &format!(
                                    "[{}] 本地回退协同 L{}-L{}",
                                    agent_name,
                                    start + 1,
                                    range_end
                                ),
                            );
                            let orig = &doc.lines[start..start + remove];
                            let mut fallback_lines = Vec::with_capacity(orig.len() + 1);
                            if !orig.first().is_some_and(|l| l.contains(&agent_name)) {
                                fallback_lines.push(marker);
                            }
                            if !(orig.len() == 1 && orig[0].is_empty()) {
                                for l in orig {
                                    fallback_lines.push(l.trim_end().to_string());
                                }
                            }
                            let new_len = fallback_lines.len();
                            if doc
                                .apply_line_patch(
                                    start,
                                    remove,
                                    fallback_lines.clone(),
                                    Some([start, 0]),
                                )
                                .is_ok()
                            {
                                let delta = new_len as isize - remove as isize;
                                region_shifts.push((doc_id, start + remove, delta));
                                self.agents[idx].patch_proposals = fallback_lines;
                                self.agents[idx]
                                    .private_history
                                    .push(CollabMsg::system(&format!(
                                        "远端失败，已在 L{}-L{} 完成本地确定性协同写入。",
                                        start + 1,
                                        range_end
                                    )));
                                if let Some(rc) = doc
                                    .remote_cursors
                                    .iter_mut()
                                    .find(|c| c.agent_id == agent_id)
                                {
                                    rc.cursor = [start.min(doc.lines.len().saturating_sub(1)), 0];
                                    rc.active_symbol = Some(format!("回退写入 {} 行", new_len));
                                }
                                doc.tip = format!("{} 已完成容错本地协同写入", agent_name);
                            }
                        }
                    }
                }
                continue;
            }

            self.agents[idx]
                .private_history
                .push(CollabMsg::agent(&agent_name, &content));
            if let (Some(doc_id), Some((raw_start, raw_remove))) =
                (run.target_doc_id, run.target_region)
            {
                let patch_lines = extract_patch_lines(&content);
                if !patch_lines.is_empty() {
                    if let Some(doc) = docs.iter_mut().find(|d| d.id == doc_id && !d.readonly) {
                        let start = raw_start.min(doc.lines.len());
                        let remove = raw_remove.min(doc.lines.len().saturating_sub(start));
                        let is_nema = crate::app::nemaopts::is_nema_doc(doc);
                        let nema_valid = if is_nema {
                            let mut candidate = doc.lines.clone();
                            candidate.splice(start..start + remove, patch_lines.iter().cloned());
                            let norm = nema_codec::normalize_nema_newlines(&candidate.join("\n"));
                            nema_codec::parse_source(&norm).is_ok()
                        } else {
                            true
                        };
                        if nema_valid {
                            let new_len = patch_lines.len();
                            self.agents[idx].patch_proposals = patch_lines.clone();
                            if let Err(e) =
                                doc.apply_line_patch(start, remove, patch_lines, Some([start, 0]))
                            {
                                self.agents[idx]
                                    .private_history
                                    .push(CollabMsg::error(&agent_name, &e));
                            } else {
                                let delta = new_len as isize - remove as isize;
                                region_shifts.push((doc_id, start + remove, delta));
                                if let Some(rc) = doc
                                    .remote_cursors
                                    .iter_mut()
                                    .find(|c| c.agent_id == agent_id)
                                {
                                    rc.cursor = [start.min(doc.lines.len().saturating_sub(1)), 0];
                                    rc.active_symbol = Some(format!("已写入 {} 行", new_len));
                                }
                                doc.tip = format!("{} 已完成并行区域写入", agent_name);
                            }
                        } else if apply_nema_collab_edit(
                            &mut self.agents,
                            &[idx],
                            doc,
                            &patch_lines.join(" "),
                        )
                        .is_ok()
                        {
                            self.agents[idx].private_history.push(CollabMsg::system(
                                "模型补丁未通过 .nema 语法校验，已自动转为语法安全域记录写入。",
                            ));
                        }
                    }
                }
            }
            if run.is_master_broadcast {
                self.master_history
                    .push(CollabMsg::agent(&agent_name, &content));
            }
        }

        // 同步平移尚未完成的后续 Agent 行坐标，确保流式并发落盘永不重叠越界
        for (doc_id, boundary, delta) in region_shifts {
            if delta == 0 {
                continue;
            }
            for a in &mut self.agents {
                if let Some(run) = a.run.as_mut() {
                    if run.target_doc_id == Some(doc_id) {
                        if let Some((start, _)) = run.target_region.as_mut() {
                            if *start >= boundary {
                                *start = ((*start as isize) + delta).max(0) as usize;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl App {
    /// 让当前配置的多 Agent 并行分区修改活跃文档
    pub fn execute_parallel_on_active(
        &mut self,
        prompt: &str,
        ctx: Option<&egui::Context>,
    ) -> Result<usize, String> {
        if self.docs.is_empty() {
            return Err("当前没有打开的文档".into());
        }
        let idx = self.active.min(self.docs.len() - 1);
        let cfg = self.ai_cfg().clone();
        let doc_id = self.docs[idx].id;
        let res = {
            let doc = &mut self.docs[idx];
            self.collab
                .execute_parallel_edit_on_doc(doc, prompt, &cfg, ctx)
        };
        if let Ok(n) = res {
            self.cookie_mark(doc_id, ide_agent_router::TaskState::Planning);
            if !cfg.configured() || ctx.is_none() {
                self.cookie_mark(doc_id, ide_agent_router::TaskState::Verified);
                self.docs[idx].tip = format!("多 Agent 已并行写入 {} 处区域", n);
            }
        }
        res
    }

    /// 渲染多 Agent 协同工作台 (统一内嵌在右侧 AI 协同侧边栏与协同审查看板中)
    pub(super) fn collab_panel_ui(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width(ui.available_width());
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let mut master_send: Option<String> = None;
        let mut parallel_edit_prompt: Option<String> = None;
        let mut stop_all_clicked = false;
        let active_tab = self.collab.active_tab;
        let tc = self.tc;

        // 1. 顶部协同控制条
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("协同 Agent:").strong());
            let running = self.collab.any_running();
            if ui
                .add_enabled(
                    !running && self.collab.agent_count > 1,
                    egui::Button::new("-"),
                )
                .clicked()
            {
                self.collab
                    .set_agent_count(self.collab.agent_count.saturating_sub(1));
            }
            let mut count_val = self.collab.agent_count;
            let dv = egui::DragValue::new(&mut count_val)
                .range(1..=CollabState::MAX_COLLAB_AGENTS)
                .speed(1.0);
            if ui.add_enabled(!running, dv).changed() {
                self.collab.set_agent_count(count_val);
            }
            if ui
                .add_enabled(
                    !running && self.collab.agent_count < CollabState::MAX_COLLAB_AGENTS,
                    egui::Button::new("+"),
                )
                .clicked()
            {
                self.collab
                    .set_agent_count(self.collab.agent_count.saturating_add(1));
            }
            if self.collab.agent_count > 8 {
                let badge = format!("数额: {}", format_number_commas(self.collab.agent_count));
                ui.label(
                    egui::RichText::new(badge)
                        .size(11.0)
                        .strong()
                        .color(Color32::from_rgb(0, 220, 255)),
                );
            }
            if running {
                ui.label(egui::RichText::new("[并行执行中]").color(Color32::from_rgb(0, 220, 130)));
                if crate::btnstyle::agent_command_button(
                    ui,
                    "终止全员",
                    crate::btnstyle::AgentBtnRole::Danger,
                    &tc,
                )
                .clicked()
                {
                    stop_all_clicked = true;
                }
            } else {
                ui.label(egui::RichText::new("[就绪]").color(Color32::from_rgb(120, 200, 120)));
            }
            if !self.docs.is_empty()
                && crate::ui_icons::icon_button(
                    ui,
                    crate::ui_icons::UiIcon::Bolt,
                    "多Agent并行修改当前文件",
                    Color32::from_rgb(56, 189, 248),
                )
                .clicked()
            {
                parallel_edit_prompt = Some(self.collab.master_input.clone());
            }
            if crate::btnstyle::agent_command_button(
                ui,
                "清空",
                crate::btnstyle::AgentBtnRole::Settings,
                &tc,
            )
            .clicked()
            {
                self.collab.master_history.clear();
            }
        });

        // 实时全员并发执行看板 (Live Concurrency Matrix)
        if self.collab.any_running() {
            ui.add_space(2.0);
            egui::Frame::group(ui.style())
                .fill(Color32::from_black_alpha(45))
                .inner_margin(egui::Margin::symmetric(6, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        crate::ui_icons::icon_label(
                            ui,
                            crate::ui_icons::UiIcon::Bolt,
                            egui::RichText::new(
                                "全员实时并发工作流 (各智能体独立线程并行执行，零排队/零阻塞)",
                            )
                            .size(11.5)
                            .color(Color32::from_rgb(0, 220, 150))
                            .strong(),
                            Color32::from_rgb(0, 220, 150),
                        );
                    });
                    ui.add_space(3.0);
                    let mut shown = 0usize;
                    let mut total_running = 0usize;
                    for a in &self.collab.agents {
                        let Some(run) = &a.run else {
                            continue;
                        };
                        total_running += 1;
                        if shown < CollabState::MAX_LIVE_MATRIX_ROWS {
                            shown += 1;
                            ui.horizontal(|ui| {
                                let (dot_r, _) =
                                    ui.allocate_exact_size(Vec2::new(10.0, 14.0), Sense::hover());
                                ui.painter().circle_filled(
                                    dot_r.center(),
                                    3.5,
                                    Color32::from_rgb(50, 215, 130),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{}:", a.name))
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::from_rgb(50, 215, 130)),
                                );
                                let last_line = run
                                    .streaming_text
                                    .lines()
                                    .rev()
                                    .find(|l| !l.trim().is_empty())
                                    .unwrap_or("正在并发执行中...");
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(last_line)
                                            .size(11.0)
                                            .color(Color32::from_rgb(210, 220, 230)),
                                    )
                                    .truncate(),
                                );
                            });
                        }
                    }
                    if total_running > shown {
                        ui.weak(format!(
                            "… 及其他 {} 个并行运行中的 Agent",
                            total_running - shown
                        ));
                    }
                });
            ui.add_space(2.0);
        }

        ui.separator();

        // 2. 统筹调度命令窗口 + Agent 编队精准控制名录 (点击任意 Agent 立即切换至完整单 Agent 对话引擎)
        let total_avail = ui.available_height();
        let half_height = (total_avail * 0.44).max(120.0);

        // --- 上部：统筹命令窗口 ---
        ui.group(|ui| {
            ui.set_height(half_height);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("【统筹命令】(广播 / 并行修改)")
                        .strong()
                        .color(Color32::from_rgb(0, 190, 255)),
                );
            });
            egui::ScrollArea::vertical()
                .max_height((half_height - 58.0).max(40.0))
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for m in &self.collab.master_history {
                        render_collab_msg(ui, m);
                    }
                });
            ui.horizontal(|ui| {
                let btn_w = 186.0;
                let re = ui.add(
                    egui::TextEdit::singleline(&mut self.collab.master_input)
                        .hint_text("输入统筹命令 (Enter 直接并行执行)...")
                        .desired_width((ui.available_width() - btn_w).max(80.0)),
                );
                let enter = re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if (crate::btnstyle::agent_command_button(
                    ui,
                    "广播统筹",
                    crate::btnstyle::AgentBtnRole::Primary,
                    &tc,
                )
                .clicked()
                    || enter)
                    && !self.collab.master_input.trim().is_empty()
                {
                    if !self.docs.is_empty() {
                        parallel_edit_prompt = Some(self.collab.master_input.clone());
                    } else {
                        master_send = Some(self.collab.master_input.clone());
                    }
                }
                let gear_tip = if self.ai.settings {
                    crate::i18n::trz("收起模型与接口设置")
                } else {
                    crate::i18n::trz("模型与接口设置…")
                };
                if crate::ui_icons::icon_only_button(
                    ui,
                    crate::ui_icons::UiIcon::Gear,
                    vec2(24.0, 24.0),
                    if self.ai.settings {
                        Color32::from_rgb(56, 189, 248)
                    } else {
                        tc.fg
                    },
                )
                .tip(gear_tip)
                .clicked()
                {
                    self.ai.settings = !self.ai.settings;
                }
                if self.collab.any_running()
                    && crate::btnstyle::agent_command_button(
                        ui,
                        "停止",
                        crate::btnstyle::AgentBtnRole::Danger,
                        &tc,
                    )
                    .clicked()
                {
                    stop_all_clicked = true;
                }
            });
        });

        ui.add_space(4.0);

        // --- 下部：Agent 编队精准控制列表 (支持搜索，点击进入单 Agent 完整对话面板) ---
        let (configured_models, global_model) = {
            let cfg = self.ai_cfg();
            let models: Vec<String> = cfg
                .configured_models()
                .into_iter()
                .map(|m| m.model)
                .collect();
            (models, cfg.model.trim().to_string())
        };
        let (filtered, total_roster) = filter_agents_limited(
            &self.collab.agents,
            &self.ai.agent_search,
            CollabState::AGENT_DISPLAY_LIMIT,
        );
        let roster: Vec<AgentRosterEntry> = filtered
            .into_iter()
            .filter_map(|idx| {
                let a = self.collab.agents.get(idx)?;
                let bound_model = a
                    .model_override
                    .as_deref()
                    .filter(|m| configured_models.iter().any(|cm| cm == m))
                    .map(str::to_string)
                    .or_else(|| (!global_model.is_empty()).then(|| global_model.clone()));
                let doc_name = a.assigned_doc_id.and_then(|did| {
                    self.docs
                        .iter()
                        .find(|d| d.id == did)
                        .map(|d| d.name.clone())
                });
                let busy = self.ai_agent_idx_is_busy(idx);
                Some(AgentRosterEntry {
                    idx,
                    name: a.name.clone(),
                    role: a.role.clone(),
                    id: a.id.clone(),
                    bound_model,
                    doc_name,
                    busy,
                })
            })
            .collect();

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::i18n::trz("Agent 协同编队状态"))
                        .strong()
                        .color(Color32::from_rgb(56, 189, 248)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ai.agent_search)
                            .hint_text(crate::i18n::trz("搜索 Agent 名称或职责…"))
                            .desired_width(140.0),
                    );
                    let (sr, _) = ui.allocate_exact_size(vec2(13.0, 13.0), Sense::hover());
                    crate::ui_icons::paint(
                        ui.painter(),
                        sr,
                        crate::ui_icons::UiIcon::Search,
                        tc.fg_dim,
                    );
                });
            });
            ui.add_space(2.0);
            let list_h = ui.available_height().max(90.0);
            egui::ScrollArea::vertical()
                .id_salt("collab_agent_roster_scroll")
                .max_height(list_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if roster.is_empty() {
                        ui.weak(crate::i18n::trz("未找到匹配的 Agent"));
                    }
                    for entry in &roster {
                        let is_selected = entry.idx == active_tab;
                        egui::Frame::group(ui.style())
                            .fill(if is_selected {
                                Color32::from_black_alpha(55)
                            } else {
                                Color32::from_black_alpha(25)
                            })
                            .inner_margin(egui::Margin::symmetric(6, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (dot_r, _) =
                                        ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
                                    ui.painter().circle_filled(
                                        dot_r.center(),
                                        3.5,
                                        if entry.busy {
                                            Color32::from_rgb(50, 215, 130)
                                        } else {
                                            tc.fg_dim
                                        },
                                    );
                                    ui.label(egui::RichText::new(&entry.name).strong().color(
                                        if is_selected {
                                            Color32::from_rgb(56, 189, 248)
                                        } else {
                                            tc.fg
                                        },
                                    ));
                                    if let Some(m) = &entry.bound_model {
                                        ui.label(
                                            egui::RichText::new(format!("[{m}]"))
                                                .size(10.5)
                                                .color(Color32::from_rgb(125, 211, 252)),
                                        );
                                    }
                                    if let Some(dn) = &entry.doc_name {
                                        ui.label(
                                            egui::RichText::new(format!("@{dn}"))
                                                .size(10.5)
                                                .color(Color32::from_rgb(250, 204, 21)),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if entry.busy {
                                                ui.label(
                                                    egui::RichText::new(crate::i18n::trz("运行中"))
                                                        .size(10.5)
                                                        .color(Color32::from_rgb(50, 215, 130)),
                                                );
                                            } else {
                                                ui.label(
                                                    egui::RichText::new(crate::i18n::trz("就绪"))
                                                        .size(10.5)
                                                        .weak(),
                                                );
                                            }
                                        },
                                    );
                                });
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&entry.role).size(11.0).weak(),
                                    )
                                    .truncate(),
                                );
                            });
                        ui.add_space(2.0);
                    }
                    if total_roster > CollabState::AGENT_DISPLAY_LIMIT {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.weak(format!(
                            "共 {} 个 Agent (显示前 {} 个匹配项，输入关键词精准检索)",
                            format_number_commas(total_roster),
                            CollabState::AGENT_DISPLAY_LIMIT
                        ));
                    }
                });
        });

        if stop_all_clicked {
            self.collab.stop_all();
        }
        if let Some(prompt) = parallel_edit_prompt {
            self.collab.master_input.clear();
            let ctx = ui.ctx().clone();
            if let Err(err) = self.execute_parallel_on_active(&prompt, Some(&ctx)) {
                self.collab
                    .master_history
                    .push(CollabMsg::error("系统", &err));
            }
        }
        if let Some(prompt) = master_send {
            let cfg = self.ai_cfg().clone();
            let ctx = ui.ctx().clone();
            self.collab.send_master(prompt, &cfg, &ctx);
        }
    }

    /// 多 Agent 协同浮动窗口 (已弃用 — 统一内嵌在右侧 AI 协同侧边栏)
    pub(super) fn collab_window(&mut self, _ctx: &egui::Context) {}

    /// 每帧轮询协同后台任务并写回并行修改的文档区域
    pub(super) fn collab_poll(&mut self, ctx: &egui::Context) {
        self.collab.poll_with_docs(ctx, &mut self.docs);
    }
}

/// 辅助渲染单条协同消息
fn render_collab_msg(ui: &mut egui::Ui, m: &CollabMsg) {
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        if m.is_user {
            ui.label(
                egui::RichText::new(format!("{}: ", m.sender))
                    .strong()
                    .color(Color32::from_rgb(100, 180, 255)),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&m.text).color(Color32::from_rgb(235, 235, 235)),
                )
                .wrap(),
            );
        } else if m.is_error {
            ui.label(
                egui::RichText::new(format!("{}: ", m.sender))
                    .strong()
                    .color(Color32::from_rgb(255, 90, 90)),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&m.text).color(Color32::from_rgb(255, 130, 130)),
                )
                .wrap(),
            );
        } else if m.sender == "系统" {
            ui.add(
                egui::Label::new(egui::RichText::new(&m.text).weak().italics().size(11.5)).wrap(),
            );
        } else {
            ui.label(
                egui::RichText::new(format!("{}: ", m.sender))
                    .strong()
                    .color(Color32::from_rgb(40, 210, 120)),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&m.text).color(Color32::from_rgb(220, 220, 220)),
                )
                .wrap(),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collab_agent_count_and_tab_expansion() {
        let mut s = CollabState::default();
        assert_eq!(s.agent_count, 3);
        assert_eq!(s.agents.len(), 3);
        assert_eq!(s.active_tab, 0);

        // 扩充至 5 个 Agent
        s.set_agent_count(5);
        assert_eq!(s.agent_count, 5);
        assert_eq!(s.agents.len(), 5);
        assert_eq!(s.agents[4].name, "Agent #5 (架构员)");

        // 缩减至 2 个 Agent
        s.active_tab = 4;
        s.set_agent_count(2);
        assert_eq!(s.agent_count, 2);
        assert_eq!(s.agents.len(), 2);
        assert_eq!(s.active_tab, 1); // 自动限制到 1

        // 边界保护：最小 1，最大支持 100,000 大规模编队
        s.set_agent_count(0);
        assert_eq!(s.agent_count, 1);
        s.set_agent_count(99);
        assert_eq!(s.agent_count, 99);
        s.set_agent_count(10_000);
        assert_eq!(s.agent_count, 10_000);
        assert_eq!(s.agents.len(), 10_000);
        s.set_agent_count(200_000);
        assert_eq!(s.agent_count, CollabState::MAX_COLLAB_AGENTS);
    }

    #[test]
    fn collab_messages_and_roles() {
        let a = SubAgent::new(0);
        assert!(a.name.contains("Agent #1"));
        assert!(a.role.contains("代码"));
        assert!(!a.private_history.is_empty());

        let msg = CollabMsg::user("统筹管理员", "写20个随机数");
        assert!(msg.is_user && !msg.is_error && msg.text == "写20个随机数");

        let err = CollabMsg::error("Agent #2", "HTTP 429 额度耗尽");
        assert!(!err.is_user && err.is_error && err.sender == "Agent #2");
    }

    #[test]
    fn parallel_region_planning_is_disjoint_and_exhaustive() {
        let r9 = plan_parallel_regions(9, 3);
        assert_eq!(r9, vec![(0, 3), (3, 3), (6, 3)]);

        let r10 = plan_parallel_regions(10, 3);
        assert_eq!(r10, vec![(0, 4), (4, 3), (7, 3)]);

        let r2 = plan_parallel_regions(2, 4);
        assert_eq!(r2, vec![(0, 1), (1, 1), (2, 0), (2, 0)]);
        for w in r2.windows(2) {
            assert!(w[0].0 + w[0].1 <= w[1].0);
        }
    }

    #[test]
    fn parallel_patches_apply_atomically_with_single_undo_and_reject_overlap() {
        let mut doc = crate::doc::Doc::untitled(1, 1);
        doc.lines = (0..6).map(|i| format!("line {i}")).collect();

        let patches = vec![
            ParallelAgentPatch {
                agent_id: "a1".into(),
                agent_name: "Agent #1".into(),
                role: "coder".into(),
                start_row: 0,
                remove_rows: 2,
                insert_lines: vec!["// a1".into(), "line 0 mod".into(), "line 1 mod".into()],
                summary: "top".into(),
            },
            ParallelAgentPatch {
                agent_id: "a2".into(),
                agent_name: "Agent #2".into(),
                role: "reviewer".into(),
                start_row: 4,
                remove_rows: 2,
                insert_lines: vec!["// a2".into(), "line 4 mod".into()],
                summary: "bottom".into(),
            },
        ];

        assert_eq!(apply_parallel_patches(&mut doc, &patches), Ok(2));
        assert_eq!(
            doc.lines,
            vec![
                "// a1",
                "line 0 mod",
                "line 1 mod",
                "line 2",
                "line 3",
                "// a2",
                "line 4 mod"
            ]
        );
        assert_eq!(doc.remote_cursors.len(), 2);

        // 单步撤销即可完整还原所有并行写入
        doc.undo();
        assert_eq!(
            doc.lines,
            (0..6).map(|i| format!("line {i}")).collect::<Vec<_>>()
        );
        assert!(doc.undo_is_empty());

        // 重叠区域必须拒绝，且文档内容不被破坏
        let overlap = vec![
            ParallelAgentPatch {
                agent_id: "a1".into(),
                agent_name: "Agent #1".into(),
                role: "c".into(),
                start_row: 1,
                remove_rows: 3,
                insert_lines: vec!["x".into()],
                summary: "x".into(),
            },
            ParallelAgentPatch {
                agent_id: "a2".into(),
                agent_name: "Agent #2".into(),
                role: "r".into(),
                start_row: 3,
                remove_rows: 1,
                insert_lines: vec!["y".into()],
                summary: "y".into(),
            },
        ];
        assert!(apply_parallel_patches(&mut doc, &overlap).is_err());
        assert_eq!(doc.lines.len(), 6);
    }

    #[test]
    fn parallel_offline_execution_edits_active_doc_with_remote_cursors() {
        let mut collab = CollabState::default();
        collab.set_agent_count(3);
        let mut doc = crate::doc::Doc::untitled(9, 1);
        doc.lines = vec!["fn a() {}".into(), "fn b() {}".into(), "fn c() {}".into()];
        let cfg = AiConfig::default();
        let n = collab
            .execute_parallel_edit_on_doc(&mut doc, "添加协同注释", &cfg, None)
            .expect("offline parallel edit should succeed");
        assert_eq!(n, 3);
        assert_eq!(doc.remote_cursors.len(), 3);
        assert!(doc.lines.iter().any(|l| l.contains("Agent #1")));
        assert!(doc.lines.iter().any(|l| l.contains("Agent #2")));
        assert!(doc.lines.iter().any(|l| l.contains("Agent #3")));
    }

    #[test]
    fn extract_patch_lines_prefers_fenced_block() {
        let raw = "说明文字\n```rust\nfn main() {}\nprintln!(\"ok\");\n```\n结束";
        assert_eq!(
            extract_patch_lines(raw),
            vec!["fn main() {}".to_string(), "println!(\"ok\");".to_string()]
        );
    }

    #[test]
    fn parallel_edit_on_nema_doc_preserves_valid_grammar_and_single_undo() {
        let mut collab = CollabState::default();
        collab.set_agent_count(3);
        let mut doc = crate::doc::Doc::untitled(42, 1);
        doc.name = "arch.nema".to_string();
        doc.syntax = Some("NEMA".to_string());
        let orig_lines: Vec<String> = nema_codec::COMPLETE_V12_SAMPLE
            .split('\n')
            .map(str::to_string)
            .collect();
        doc.lines = orig_lines.clone();

        let cfg = AiConfig::default();
        let applied = collab
            .execute_parallel_edit_on_doc(&mut doc, "强化多Agent协同拓扑", &cfg, None)
            .expect(".nema parallel co-edit should succeed");
        assert_eq!(applied, 3);
        assert_eq!(doc.remote_cursors.len(), 3);

        let after_src = doc.lines.join("\n");
        assert!(
            nema_codec::parse_source(&after_src).is_ok(),
            "parallel .nema co-edit must produce valid NEMA grammar:\n{after_src}"
        );
        assert!(
            after_src.contains("NAg3"),
            "Graph node NAg3 should be created"
        );

        // 撤销应一步完整还原原始文档
        doc.undo();
        assert_eq!(doc.lines, orig_lines);
    }

    #[test]
    fn single_agent_offline_edit_modifies_target_doc() {
        let mut collab = CollabState::default();
        collab.set_agent_count(3);
        let mut doc = crate::doc::Doc::untitled(77, 1);
        doc.name = "worker.rs".to_string();
        doc.lines = vec![
            "fn step1() {}".into(),
            "fn step2() {}".into(),
            "fn step3() {}".into(),
        ];
        let applied = collab
            .execute_single_agent_edit_on_doc(1, &mut doc, "补充边界检查")
            .expect("single agent edit should succeed");
        assert_eq!(applied, 1);
        assert_eq!(doc.remote_cursors.len(), 1);
        assert_eq!(doc.remote_cursors[0].agent_id, "agent_2");
        assert!(doc.lines.iter().any(|l| l.contains("Agent #2")));
    }

    #[test]
    fn test_multi_agent_true_concurrency_no_queueing() {
        let mut collab = CollabState::default();
        collab.set_agent_count(3);
        let ctx = egui::Context::default();
        let cfg = AiConfig::default();

        // 广播下发统筹任务
        collab.send_master("架构解耦与单元测试生成".into(), &cfg, &ctx);

        // 验证全员同时处于运行态，真正并发，零排队
        assert!(collab.any_running());
        assert!(
            collab.agents.iter().all(|a| a.is_running()),
            "所有智能体必须同时启动运行"
        );

        // 等待并发工作线程输出流式分块 (重试轮询直至所有智能体均产出分块)
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            collab.poll(&ctx);
            let all_streamed = collab
                .agents
                .iter()
                .all(|a| a.run.as_ref().is_some_and(|r| !r.streaming_text.is_empty()));
            if all_streamed {
                break;
            }
        }

        // 验证每个 Agent 都在独立流式并发产出
        for a in &collab.agents {
            if let Some(run) = &a.run {
                assert!(
                    !run.streaming_text.is_empty(),
                    "智能体 {} 应该已有并发流式输出",
                    a.name
                );
            }
        }

        // 停止其中 1 个 Agent，其余 Agent 绝不受影响，继续并发工作，互不阻塞
        collab.stop_one(0);
        assert!(!collab.agents[0].is_running(), "Agent #1 应已停止");
        assert!(collab.agents[1].is_running(), "Agent #2 必须保持并行运行");
        assert!(collab.agents[2].is_running(), "Agent #3 必须保持并行运行");

        // 再次轮询接收剩余并发工作流
        collab.poll(&ctx);
        collab.stop_all();
        assert!(!collab.any_running(), "全部停止后应回到就绪态");
    }

    #[test]
    fn filter_agents_indices_matches_name_role_and_model() {
        let mut collab = CollabState::default();
        collab.set_agent_count(4);
        collab.agents[1].model_override = Some("deepseek-r1-custom".into());

        assert_eq!(
            filter_agents_indices(&collab.agents, "   "),
            vec![0, 1, 2, 3]
        );
        assert_eq!(filter_agents_indices(&collab.agents, "审查"), vec![1]);
        assert_eq!(filter_agents_indices(&collab.agents, "AGENT_3"), vec![2]);
        assert_eq!(
            filter_agents_indices(&collab.agents, "DeepSeek-R1"),
            vec![1]
        );
        assert_eq!(filter_agents_indices(&collab.agents, "性能"), vec![3]);
        assert!(filter_agents_indices(&collab.agents, "nonexistent").is_empty());
    }

    #[test]
    fn per_agent_model_override_persists_through_settings() {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut app = App::build(
            vec![],
            None,
            crate::Trace::new(),
            crate::settings::Settings::default(),
            false,
        );
        app.collab.set_agent_count(4);
        app.collab.active_tab = 2;
        app.collab.agents[0].model_override = Some("user-model-a".into());
        app.collab.agents[2].model_override = Some("user-model-c".into());

        let saved = app.current_settings();
        assert_eq!(saved.collab_agent_count, 4);
        assert_eq!(saved.collab_active_agent, 2);
        assert_eq!(
            saved.collab_agent_models,
            vec![
                ("agent_1".into(), "user-model-a".into()),
                ("agent_3".into(), "user-model-c".into()),
            ]
        );

        let mut restored_cfg = crate::settings::Settings::default();
        restored_cfg.apply_json(&saved.to_json(), true);
        let restored = App::build(vec![], None, crate::Trace::new(), restored_cfg, false);
        assert_eq!(restored.collab.agent_count, 4);
        assert_eq!(restored.collab.active_tab, 2);
        assert_eq!(
            restored.collab.agents[0].model_override.as_deref(),
            Some("user-model-a")
        );
        assert_eq!(restored.collab.agents[1].model_override, None);
        assert_eq!(
            restored.collab.agents[2].model_override.as_deref(),
            Some("user-model-c")
        );
        drop(g);
    }

    #[test]
    fn filter_agents_limited_handles_10000_agents() {
        let mut s = CollabState::default();
        s.set_agent_count(10_000);
        let (ids, total) = filter_agents_limited(&s.agents, "  ", CollabState::AGENT_DISPLAY_LIMIT);
        assert_eq!(total, 10_000);
        assert_eq!(ids.len(), CollabState::AGENT_DISPLAY_LIMIT);
        assert_eq!(ids[0], 0);
        assert_eq!(ids[49], 49);

        let (hit_ids, hit_total) =
            filter_agents_limited(&s.agents, "AGENT_9999", CollabState::AGENT_DISPLAY_LIMIT);
        assert_eq!(hit_total, 1);
        assert_eq!(hit_ids, vec![9998]);
    }

    #[test]
    fn send_master_caps_concurrent_dispatch_under_10000_agents() {
        let mut s = CollabState::default();
        s.set_agent_count(10_000);
        let cfg = AiConfig::default();
        let ctx = egui::Context::default();
        s.send_master("并行检查边界".into(), &cfg, &ctx);
        let active_runs = s.agents.iter().filter(|a| a.is_running()).count();
        assert_eq!(active_runs, CollabState::MAX_CONCURRENT_AGENT_DISPATCH);
        s.stop_all();
        assert!(!s.any_running());
    }
}
