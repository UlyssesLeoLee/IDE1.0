//! 分布式账本持久化抽象与内存 CAS 实现 (WBS-05)
//!
//! 提供基于单文件 CAS、原子提交历史与 Transactional Outbox 的持久化存储抽象。
//! 确保多 Agent 并发场景下的提交线性一致性与事件异步派发的精确投递保障。

use crate::coordinator::OutboxEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LedgerError {
    #[error("File '{0}' not found in ledger")]
    FileNotFound(String),

    #[error("CAS revision conflict for file '{file_id}': expected previous revision {expected}, but current revision is {actual}")]
    RevisionConflict {
        file_id: String,
        expected: u64,
        actual: u64,
    },

    #[error("Commit with id '{0}' already exists")]
    DuplicateCommit(String),

    #[error("Outbox entry with id {0} not found")]
    OutboxEntryNotFound(u64),

    #[error("Internal ledger store error: {0}")]
    StoreError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxStatus {
    Pending,
    Dispatched,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRecord {
    pub commit_id: String,
    pub file_id: String,
    pub workspace_id: String,
    pub previous_revision: u64,
    pub new_revision: u64,
    pub patch_content: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutboxRecord {
    pub event_id: u64,
    pub event_type: String,
    pub workspace_id: String,
    pub file_id: String,
    pub revision: u64,
    pub payload: serde_json::Value,
    pub status: OutboxStatus,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OutboxRecord {
    pub fn to_outbox_event(&self) -> OutboxEvent {
        OutboxEvent {
            event_id: self.event_id,
            event_type: self.event_type.clone(),
            workspace_id: self.workspace_id.clone(),
            file_id: self.file_id.clone(),
            revision: self.revision,
            payload: self.payload.clone(),
            created_at: self.created_at,
        }
    }
}

/// 分布式账本持久化接口
#[async_trait::async_trait]
pub trait LedgerStore: Send + Sync {
    /// 获取指定文件的当前权威修订版本 (None 表示未初始化)
    async fn get_file_revision(&self, file_id: &str) -> Result<Option<u64>, LedgerError>;

    /// 初始化或注册文件的基线修订版本
    async fn init_file_revision(&self, file_id: &str, revision: u64) -> Result<(), LedgerError>;

    /// 原子记录提交历史并向发件箱 (Outbox) 写入待派发事件
    /// 必须在单一事务/临界区内完成：
    /// 1. CAS 校验 (previous_revision == current_revision)
    /// 2. 更新文件修订版本
    /// 3. 记录 Commit 历史
    /// 4. 插入 Outbox 待派发事件
    async fn record_commit(
        &self,
        commit: CommitRecord,
        event: OutboxEvent,
    ) -> Result<(), LedgerError>;

    /// 轮询待派发的 Outbox 事件 (按 event_id 升序排列)
    async fn poll_pending_outbox(&self, limit: usize) -> Result<Vec<OutboxRecord>, LedgerError>;

    /// 标记指定 Outbox 事件为已成功派发
    async fn mark_outbox_dispatched(&self, event_id: u64) -> Result<(), LedgerError>;

    /// 标记指定 Outbox 事件派发重试 (状态保持 Pending，更新 retry_count 与 last_error)
    async fn mark_outbox_retry(
        &self,
        event_id: u64,
        error_msg: &str,
        retry_count: u32,
    ) -> Result<(), LedgerError>;

    /// 标记指定 Outbox 事件终态失败 (状态变更为 Failed)
    async fn mark_outbox_failed(
        &self,
        event_id: u64,
        error_msg: &str,
        retry_count: u32,
    ) -> Result<(), LedgerError>;

    /// 查询单条 Commit 记录
    async fn get_commit(&self, commit_id: &str) -> Result<Option<CommitRecord>, LedgerError>;

    /// 查询单条 Outbox 记录
    async fn get_outbox_entry(&self, event_id: u64) -> Result<Option<OutboxRecord>, LedgerError>;
}

#[derive(Debug, Default)]
struct MemoryLedgerInner {
    file_revisions: HashMap<String, u64>,
    commits: HashMap<String, CommitRecord>,
    outbox: Vec<OutboxRecord>,
    next_event_id: u64,
}

/// 采用 `Arc<RwLock<...>>` 保证多线程并发安全与 CAS 原子性的内存分布式账本实现
#[derive(Clone, Debug)]
pub struct MemoryLedgerStore {
    inner: Arc<RwLock<MemoryLedgerInner>>,
}

impl MemoryLedgerStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MemoryLedgerInner {
                file_revisions: HashMap::new(),
                commits: HashMap::new(),
                outbox: Vec::new(),
                next_event_id: 1,
            })),
        }
    }

    pub fn with_initial_file(file_id: &str, initial_revision: u64) -> Self {
        let store = Self::new();
        store
            .inner
            .write()
            .unwrap()
            .file_revisions
            .insert(file_id.to_string(), initial_revision);
        store
    }

    pub fn total_commits(&self) -> usize {
        self.inner.read().unwrap().commits.len()
    }

    pub fn total_outbox_entries(&self) -> usize {
        self.inner.read().unwrap().outbox.len()
    }
}

impl Default for MemoryLedgerStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LedgerStore for MemoryLedgerStore {
    async fn get_file_revision(&self, file_id: &str) -> Result<Option<u64>, LedgerError> {
        let inner = self.inner.read().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        Ok(inner.file_revisions.get(file_id).copied())
    }

    async fn init_file_revision(&self, file_id: &str, revision: u64) -> Result<(), LedgerError> {
        let mut inner = self.inner.write().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        inner.file_revisions.insert(file_id.to_string(), revision);
        Ok(())
    }

    async fn record_commit(
        &self,
        commit: CommitRecord,
        event: OutboxEvent,
    ) -> Result<(), LedgerError> {
        let mut inner = self.inner.write().map_err(|e| LedgerError::StoreError(e.to_string()))?;

        // 1. 检查 CommitId 幂等性与重复性
        if inner.commits.contains_key(&commit.commit_id) {
            return Err(LedgerError::DuplicateCommit(commit.commit_id));
        }

        // 2. CAS 校验
        let current_revision = inner.file_revisions.get(&commit.file_id).copied();
        match current_revision {
            Some(curr) => {
                if curr != commit.previous_revision {
                    return Err(LedgerError::RevisionConflict {
                        file_id: commit.file_id,
                        expected: commit.previous_revision,
                        actual: curr,
                    });
                }
            }
            None => {
                // 如果文件尚未注册，但预期修订版本不为 0，则说明文件缺失或版本错误
                if commit.previous_revision != 0 {
                    return Err(LedgerError::FileNotFound(commit.file_id));
                }
            }
        }

        // 3. 更新版本号
        inner
            .file_revisions
            .insert(commit.file_id.clone(), commit.new_revision);

        // 4. 记录提交
        inner.commits.insert(commit.commit_id.clone(), commit);

        // 5. 确定 Event ID
        let event_id = if event.event_id == 0 {
            let id = inner.next_event_id;
            inner.next_event_id += 1;
            id
        } else {
            if event.event_id >= inner.next_event_id {
                inner.next_event_id = event.event_id + 1;
            }
            event.event_id
        };

        // 6. 写入 Outbox 事件 (原子事务边界)
        let now = Utc::now();
        let record = OutboxRecord {
            event_id,
            event_type: event.event_type,
            workspace_id: event.workspace_id,
            file_id: event.file_id,
            revision: event.revision,
            payload: event.payload,
            status: OutboxStatus::Pending,
            retry_count: 0,
            last_error: None,
            created_at: event.created_at,
            updated_at: now,
        };
        inner.outbox.push(record);

        Ok(())
    }

    async fn poll_pending_outbox(&self, limit: usize) -> Result<Vec<OutboxRecord>, LedgerError> {
        let inner = self.inner.read().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        let records = inner
            .outbox
            .iter()
            .filter(|e| e.status == OutboxStatus::Pending)
            .take(limit)
            .cloned()
            .collect();
        Ok(records)
    }

    async fn mark_outbox_dispatched(&self, event_id: u64) -> Result<(), LedgerError> {
        let mut inner = self.inner.write().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        let entry = inner
            .outbox
            .iter_mut()
            .find(|e| e.event_id == event_id)
            .ok_or(LedgerError::OutboxEntryNotFound(event_id))?;

        entry.status = OutboxStatus::Dispatched;
        entry.updated_at = Utc::now();
        Ok(())
    }

    async fn mark_outbox_retry(
        &self,
        event_id: u64,
        error_msg: &str,
        retry_count: u32,
    ) -> Result<(), LedgerError> {
        let mut inner = self.inner.write().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        let entry = inner
            .outbox
            .iter_mut()
            .find(|e| e.event_id == event_id)
            .ok_or(LedgerError::OutboxEntryNotFound(event_id))?;

        entry.status = OutboxStatus::Pending;
        entry.retry_count = retry_count;
        entry.last_error = Some(error_msg.to_string());
        entry.updated_at = Utc::now();
        Ok(())
    }

    async fn mark_outbox_failed(
        &self,
        event_id: u64,
        error_msg: &str,
        retry_count: u32,
    ) -> Result<(), LedgerError> {
        let mut inner = self.inner.write().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        let entry = inner
            .outbox
            .iter_mut()
            .find(|e| e.event_id == event_id)
            .ok_or(LedgerError::OutboxEntryNotFound(event_id))?;

        entry.status = OutboxStatus::Failed;
        entry.retry_count = retry_count;
        entry.last_error = Some(error_msg.to_string());
        entry.updated_at = Utc::now();
        Ok(())
    }

    async fn get_commit(&self, commit_id: &str) -> Result<Option<CommitRecord>, LedgerError> {
        let inner = self.inner.read().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        Ok(inner.commits.get(commit_id).cloned())
    }

    async fn get_outbox_entry(&self, event_id: u64) -> Result<Option<OutboxRecord>, LedgerError> {
        let inner = self.inner.read().map_err(|e| LedgerError::StoreError(e.to_string()))?;
        Ok(inner.outbox.iter().find(|e| e.event_id == event_id).cloned())
    }
}
