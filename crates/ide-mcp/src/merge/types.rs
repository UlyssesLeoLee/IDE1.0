//! 协同文档存储核心类型定义 (P0-01, P0-04, P0-05, P1-01)。

use super::algo::ConflictHunk;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

pub const MAX_DOCS: usize = 256;
pub const MAX_REVISIONS_PER_DOC: usize = 128;
pub const MAX_IDEMPOTENCY_KEYS: usize = 512;
pub const MAX_PROPOSALS: usize = 512;
pub const MAX_OUTBOX_EVENTS: usize = 2048;

/// 打开文件模式：禁止重复 open 静默覆盖历史版本 (P0-04)。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenMode {
    #[default]
    CreateIfAbsent,
    OpenExisting,
    ForceOverwrite,
}

/// 协同模式：Git 式行级差分合并 vs NEMA 结构化语法与域级仲裁。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollabMode {
    Git,
    Nema,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevisionRecord {
    pub revision: u64,
    pub content: String,
    pub content_hash: String,
    pub provider: String,
    pub agent_id: String,
    pub summary: String,
    pub auto_merged: bool,
    pub created_at: String,
    #[serde(default)]
    pub touched_range: (usize, usize),
    #[serde(default)]
    pub line_delta: isize,
    #[serde(default)]
    pub op_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdempotencyEntry {
    pub payload_hash: String,
    pub response: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeaseRecord {
    pub agent_id: String,
    pub fencing_token: u64,
    pub expires_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictProposal {
    pub proposal_id: String,
    pub file_id: String,
    pub base_revision: u64,
    pub head_revision: u64,
    pub head_content_hash: String,
    pub agent_id: String,
    pub op_id: Option<String>,
    pub proposed_content: String,
    pub proposed_hash: String,
    pub conflict_preview: String,
    pub conflict_diff: String,
    pub conflicts: Vec<ConflictHunk>,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub event_id: u64,
    pub event_type: String,
    pub file_id: String,
    pub revision: u64,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadlessDoc {
    pub file_id: String,
    pub language: String,
    pub current_revision: u64,
    pub content: String,
    pub content_hash: String,
    pub revisions: VecDeque<RevisionRecord>,
    #[serde(default)]
    pub idempotency: HashMap<String, IdempotencyEntry>,
    #[serde(default)]
    pub idempotency_order: VecDeque<String>,
    #[serde(default)]
    pub lease: Option<LeaseRecord>,
    #[serde(default)]
    pub next_fencing_token: u64,
    #[serde(default)]
    pub disk_path: Option<String>,
    #[serde(default)]
    pub last_disk_hash: Option<String>,
    #[serde(default)]
    pub last_synced_revision: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FullCoEditRequest {
    pub file_id: String,
    pub author: String,
    pub new_text: String,
    pub base_revision: Option<u64>,
    #[serde(default)]
    pub base_content_hash: Option<String>,
    #[serde(default)]
    pub fencing_token: Option<u64>,
    pub op_id: Option<String>,
    pub mode: Option<CollabMode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EditRequest {
    pub file_id: String,
    pub provider: String,
    pub agent_id: String,
    pub base_revision: Option<u64>,
    #[serde(default)]
    pub base_content_hash: Option<String>,
    #[serde(default)]
    pub fencing_token: Option<u64>,
    /// 1-based 起始行 (闭区间)
    pub start_line: usize,
    /// 1-based 结束行 (闭区间; 若 end_line < start_line 则为纯插入在 start_line 之前)
    pub end_line: usize,
    pub replacement: String,
    pub op_id: Option<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReplaceAllRequest {
    pub file_id: String,
    pub provider: String,
    pub agent_id: String,
    pub base_revision: Option<u64>,
    #[serde(default)]
    pub base_content_hash: Option<String>,
    #[serde(default)]
    pub fencing_token: Option<u64>,
    pub find: String,
    pub replace: String,
    pub op_id: Option<String>,
}
