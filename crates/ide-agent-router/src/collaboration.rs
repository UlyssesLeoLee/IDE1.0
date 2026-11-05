//! 多 AI / 多人协同实时编辑引擎 (Multi-Agent Co-Editing Engine)
//!
//! 提供：
//! 1. 逐字击键广播 (Keystroke-by-keystroke Streaming Broadcast)
//! 2. 远程多人/多 AI 光标与选区状态跟踪 (Remote Cursor & Selection Tracking)
//! 3. 实时行级/字符级作者归属矩阵 (Collaborative Attribution Matrix)
//! 4. 并发意图保持位置偏移变换 (Position Mapping & Cursor Shift)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 协作者角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaboratorRole {
    /// 人类用户
    Human,
    /// 核心编写 Agent (业务逻辑、函数实现)
    AiCoder,
    /// 优化重构 Agent (性能优化、规范整洁、算法重构)
    AiOptimizer,
    /// 测试生成 Agent (单元测试、边界测试、Mock)
    AiTester,
    /// 统筹审查验收 Agent / 人类 Reviewer
    Reviewer,
}

impl CollaboratorRole {
    pub fn badge_prefix(&self) -> &'static str {
        match self {
            Self::Human => "👤 人类",
            Self::AiCoder => "🤖 编码",
            Self::AiOptimizer => "⚡ 优化",
            Self::AiTester => "🧪 测试",
            Self::Reviewer => "🛡️ 审查",
        }
    }
}

/// 协作者颜色 (RGBA 与 HEX 表达)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaboratorColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    pub hex: String,
}

impl CollaboratorColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            a: 255,
            hex: format!("#{:02X}{:02X}{:02X}", r, g, b),
        }
    }

    /// 按照 Agent ID 稳定确定性哈希分配色彩池 (避免颜色碰撞)
    pub fn from_id(id: &str) -> Self {
        const PALETTE: &[(u8, u8, u8)] = &[
            (56, 189, 248),  // 天蓝 (Sky Blue)
            (168, 85, 247),  // 紫罗兰 (Violet)
            (52, 211, 153),  // 翡翠绿 (Emerald)
            (251, 146, 60),  // 琥珀橙 (Amber Orange)
            (244, 63, 94),   // 玫瑰红 (Rose Red)
            (234, 179, 8),   // 耀斑金 (Sun Gold)
            (99, 102, 241),  // 靛青 (Indigo)
            (20, 184, 166),  // 绿松石 (Teal)
        ];
        let mut hash = 0u64;
        for b in id.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        let (r, g, b) = PALETTE[(hash as usize) % PALETTE.len()];
        Self::new(r, g, b)
    }
}

/// 远程光标与选区状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteCursor {
    pub agent_id: String,
    pub display_name: String,
    pub role: CollaboratorRole,
    pub color: CollaboratorColor,
    /// 光标所在位置 [行号 (0-based), 列号 (0-based)]
    pub cursor: [usize; 2],
    /// 可选选区 [r0, c0, r1, c1] (排好序：起行, 起列, 止行, 止列)
    pub selection: Option<[usize; 4]>,
    /// 当前正在操作的语义符号 (如 `fn parse_header`)
    pub active_symbol: Option<String>,
    /// 最后击键/移动时间戳
    pub last_active_epoch_ms: u64,
}

/// 逐字击键与细粒度文本操作原子指令
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data")]
pub enum KeystrokeOp {
    /// 逐字插入单字符
    InsertChar {
        row: usize,
        col: usize,
        ch: char,
    },
    /// 插入连续文本片段 (如粘贴或快速补全)
    InsertText {
        row: usize,
        col: usize,
        text: String,
    },
    /// 退格/删除单字符
    DeleteChar {
        row: usize,
        col: usize,
    },
    /// 删除指定范围
    DeleteRange {
        start: [usize; 2],
        end: [usize; 2],
    },
    /// 替换指定范围
    ReplaceRange {
        start: [usize; 2],
        end: [usize; 2],
        text: String,
    },
}

/// 协同广播消息 (Wire Event)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeystrokeBroadcast {
    pub broadcast_id: u64,
    pub file_id: String,
    pub agent_id: String,
    pub display_name: String,
    pub role: CollaboratorRole,
    pub seq: u64,
    pub op: KeystrokeOp,
    pub cursor_after: [usize; 2],
    pub selection_after: Option<[usize; 4]>,
    pub timestamp_epoch_ms: u64,
}

/// 语义指导区锚点 (ZoneAnchor)
/// 记录某个特定区域（如函数、类定义）在初始规划时的基准起始行号，
/// 以及随协同编辑期间上方区域增删行所产生的累积偏移量 `line_delta`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneAnchor {
    pub zone_id: String,
    pub base_start_line: usize,
    pub line_delta: i64,
}

impl ZoneAnchor {
    pub fn new(zone_id: impl Into<String>, base_start_line: usize) -> Self {
        Self {
            zone_id: zone_id.into(),
            base_start_line,
            line_delta: 0,
        }
    }

    /// 当前实际计算所得起始行（保证不小于 0）
    pub fn current_start_line(&self) -> usize {
        let line = self.base_start_line as i64 + self.line_delta;
        if line < 0 {
            0
        } else {
            line as usize
        }
    }
}

/// 语义锚点偏移表 (AnchorOffsetTable)
/// 当多 AI 在不同指导区并发增删代码时，各区域的起始行通过本表进行坐标映射，
/// 避免下方 Agent 的编辑位置因为上方 Agent 增加换行而发生坐标失步。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorOffsetTable {
    pub anchors: Vec<ZoneAnchor>,
}

impl AnchorOffsetTable {
    pub fn new() -> Self {
        Self {
            anchors: Vec::new(),
        }
    }

    /// 注册或更新区域基准行
    pub fn register_zone(&mut self, zone_id: &str, start_line: usize) {
        if let Some(anchor) = self.anchors.iter_mut().find(|a| a.zone_id == zone_id) {
            anchor.base_start_line = start_line;
        } else {
            self.anchors.push(ZoneAnchor::new(zone_id, start_line));
        }
        // 保持按基准起始行递增排序
        self.anchors.sort_by_key(|a| a.base_start_line);
    }

    /// 当某个区域发生行数变化时，更新该区域并通知下方所有区域累计平移
    pub fn apply_zone_delta(&mut self, zone_id: &str, delta_lines: i64) {
        if delta_lines == 0 {
            return;
        }
        let target_base = self
            .anchors
            .iter()
            .find(|a| a.zone_id == zone_id)
            .map(|a| a.base_start_line);

        if let Some(target_base) = target_base {
            for anchor in self.anchors.iter_mut() {
                if anchor.base_start_line > target_base {
                    anchor.line_delta += delta_lines;
                }
            }
        }
    }

    /// 将区域内相对行号解析为全局实时绝对行号
    pub fn resolve_line(&self, zone_id: &str, rel_line: usize) -> usize {
        if let Some(anchor) = self.anchors.iter().find(|a| a.zone_id == zone_id) {
            anchor.current_start_line() + rel_line
        } else {
            rel_line
        }
    }
}

/// 协同会话状态
pub struct CollaborationSession {
    pub file_id: String,
    pub current_revision: u64,
    pub collaborators: HashMap<String, RemoteCursor>,
    /// 行归属矩阵: 每行的最后编辑作者与时间戳 (0-based 行号 -> (AgentId, Timestamp))
    pub line_attributions: HashMap<usize, (String, u64)>,
    /// 语义锚点偏移映射表
    pub anchor_table: AnchorOffsetTable,
    next_broadcast_id: u64,
    history: Vec<KeystrokeBroadcast>,
}

impl CollaborationSession {
    pub fn new(file_id: String, base_revision: u64) -> Self {
        Self {
            file_id,
            current_revision: base_revision,
            collaborators: HashMap::new(),
            line_attributions: HashMap::new(),
            anchor_table: AnchorOffsetTable::new(),
            next_broadcast_id: 1,
            history: Vec::new(),
        }
    }

    /// 加入或更新协作者心跳与光标
    pub fn register_or_update_presence(
        &mut self,
        agent_id: String,
        display_name: String,
        role: CollaboratorRole,
        cursor: [usize; 2],
        selection: Option<[usize; 4]>,
        active_symbol: Option<String>,
    ) -> RemoteCursor {
        let now_ms = Utc::now().timestamp_millis() as u64;
        let color = CollaboratorColor::from_id(&agent_id);
        let rc = RemoteCursor {
            agent_id: agent_id.clone(),
            display_name,
            role,
            color,
            cursor,
            selection,
            active_symbol,
            last_active_epoch_ms: now_ms,
        };
        self.collaborators.insert(agent_id, rc.clone());
        rc
    }

    /// 协作者离开
    pub fn remove_collaborator(&mut self, agent_id: &str) -> Option<RemoteCursor> {
        self.collaborators.remove(agent_id)
    }

    /// 记录并广播一次击键操作
    pub fn apply_keystroke(
        &mut self,
        agent_id: &str,
        op: KeystrokeOp,
        cursor_after: [usize; 2],
        selection_after: Option<[usize; 4]>,
    ) -> Result<KeystrokeBroadcast, String> {
        let Some(c) = self.collaborators.get_mut(agent_id) else {
            return Err(format!("Agent '{}' not joined in collaboration session", agent_id));
        };

        c.cursor = cursor_after;
        c.selection = selection_after;
        let now_ms = Utc::now().timestamp_millis() as u64;
        c.last_active_epoch_ms = now_ms;

        // 更新行归属
        match &op {
            KeystrokeOp::InsertChar { row, .. } | KeystrokeOp::DeleteChar { row, .. } => {
                self.line_attributions.insert(*row, (agent_id.to_string(), now_ms));
            }
            KeystrokeOp::InsertText { row, text, .. } => {
                let inserted_newlines = text.chars().filter(|c| *c == '\n').count();
                for r in *row..=(*row + inserted_newlines) {
                    self.line_attributions.insert(r, (agent_id.to_string(), now_ms));
                }
            }
            KeystrokeOp::DeleteRange { start, end } | KeystrokeOp::ReplaceRange { start, end, .. } => {
                for r in start[0]..=end[0] {
                    self.line_attributions.insert(r, (agent_id.to_string(), now_ms));
                }
            }
        }

        let bcast_id = self.next_broadcast_id;
        self.next_broadcast_id += 1;

        let broadcast = KeystrokeBroadcast {
            broadcast_id: bcast_id,
            file_id: self.file_id.clone(),
            agent_id: agent_id.to_string(),
            display_name: c.display_name.clone(),
            role: c.role,
            seq: self.history.len() as u64 + 1,
            op,
            cursor_after,
            selection_after,
            timestamp_epoch_ms: now_ms,
        };

        self.history.push(broadcast.clone());
        Ok(broadcast)
    }

    /// 获取活跃的所有协作者光标
    pub fn active_cursors(&self, idle_timeout_ms: u64) -> Vec<RemoteCursor> {
        let now_ms = Utc::now().timestamp_millis() as u64;
        self.collaborators
            .values()
            .filter(|c| now_ms.saturating_sub(c.last_active_epoch_ms) <= idle_timeout_ms)
            .cloned()
            .collect()
    }

    /// 查询某行的贡献作者
    pub fn attribution_for_line(&self, row: usize) -> Option<(&str, u64)> {
        self.line_attributions
            .get(&row)
            .map(|(id, ts)| (id.as_str(), *ts))
    }
}

/// 协同中继路由器 (全局多文件协同管理器)
pub struct CollaborationHub {
    sessions: RwLock<HashMap<String, Arc<RwLock<CollaborationSession>>>>,
}

impl CollaborationHub {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// 获取或创建文件的协同会话
    pub fn get_or_create_session(&self, file_id: &str, base_rev: u64) -> Arc<RwLock<CollaborationSession>> {
        let mut guard = self.sessions.write().unwrap();
        guard
            .entry(file_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(CollaborationSession::new(file_id.to_string(), base_rev))))
            .clone()
    }

    /// 移除会话
    pub fn close_session(&self, file_id: &str) -> bool {
        let mut guard = self.sessions.write().unwrap();
        guard.remove(file_id).is_some()
    }
}

impl Default for CollaborationHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collaborator_color_deterministic_hashing() {
        let c1 = CollaboratorColor::from_id("agent_alpha");
        let c2 = CollaboratorColor::from_id("agent_alpha");
        let c3 = CollaboratorColor::from_id("agent_beta");
        assert_eq!(c1, c2, "相同 ID 必须分配相同色彩");
        assert!(c1.hex.starts_with('#'));
        let _ = c3;
    }

    #[test]
    fn test_keystroke_broadcast_and_attribution() {
        let hub = CollaborationHub::new();
        let session_ref = hub.get_or_create_session("src/main.rs", 100);
        let mut session = session_ref.write().unwrap();

        // 1. Agent 注册
        session.register_or_update_presence(
            "coder_1".into(),
            "Coder Alpha".into(),
            CollaboratorRole::AiCoder,
            [10, 0],
            None,
            Some("fn parse_token".into()),
        );

        session.register_or_update_presence(
            "reviewer_1".into(),
            "Senior Reviewer".into(),
            CollaboratorRole::Reviewer,
            [50, 0],
            Some([50, 0, 52, 10]),
            None,
        );

        let active = session.active_cursors(5000);
        assert_eq!(active.len(), 2);

        // 2. 逐字击键插入字符
        let bcast = session
            .apply_keystroke(
                "coder_1",
                KeystrokeOp::InsertChar {
                    row: 10,
                    col: 5,
                    ch: 'x',
                },
                [10, 6],
                None,
            )
            .expect("apply_keystroke should succeed");

        assert_eq!(bcast.broadcast_id, 1);
        assert_eq!(bcast.agent_id, "coder_1");
        assert_eq!(bcast.cursor_after, [10, 6]);

        // 3. 验证行归属矩阵
        let attr = session.attribution_for_line(10);
        assert!(attr.is_some());
        assert_eq!(attr.unwrap().0, "coder_1");
    }

    #[test]
    fn test_anchor_offset_table_shifting() {
        let mut table = AnchorOffsetTable::new();
        table.register_zone("zone_header", 0);
        table.register_zone("zone_parse_fn", 20);
        table.register_zone("zone_tests", 80);

        assert_eq!(table.resolve_line("zone_header", 5), 5);
        assert_eq!(table.resolve_line("zone_parse_fn", 3), 23);
        assert_eq!(table.resolve_line("zone_tests", 2), 82);

        // 在 zone_parse_fn 中插入了 15 行新代码
        table.apply_zone_delta("zone_parse_fn", 15);

        // zone_header 位于上方，不受影响
        assert_eq!(table.resolve_line("zone_header", 5), 5);
        // zone_parse_fn 自身基准未变
        assert_eq!(table.resolve_line("zone_parse_fn", 3), 23);
        // zone_tests 位于下方，自动下移 15 行！
        assert_eq!(table.resolve_line("zone_tests", 2), 97);
    }
}
