//! 文件级一致性协调器与 Fencing 租约机 (DD-ROUTER-DB)
//!
//! 维护文件的权威版本，以单文件 CAS、分布式递增 Fencing Token 租约、
//! AST-aware 重定位和 Transactional Outbox 确保高并发多 Agent 的原子安全。

use crate::identity::{SemanticUnit, SupportedLanguage};
use crate::parser::SemanticParser;
use crate::patch::{PatchEngine, PatchError, PatchProposal};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoordinatorError {
    #[error("File '{0}' not found")]
    FileNotFound(String),

    #[error("Task '{0}' not found")]
    TaskNotFound(String),

    #[error("Fencing token stale (current: {current}, provided: {provided})")]
    FencingTokenStale { current: u64, provided: u64 },

    #[error("Task lease has expired")]
    LeaseExpired,

    #[error("Patch application failed: {0}")]
    PatchFailure(#[from] PatchError),

    #[error("Duplicate idempotency key '{0}' with conflicting payload")]
    IdempotencyConflict(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub file_id: String,
    pub workspace_id: String,
    pub path: String,
    pub language: SupportedLanguage,
    pub current_revision: u64,
    pub content: String,
    pub content_hash: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Ready,
    Claimed,
    Committed,
    Conflict,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub task_id: String,
    pub job_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub symbol_path: String,
    pub base_revision: u64,
    pub status: TaskStatus,
    pub worker_id: Option<String>,
    pub fencing_token: u64,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub expected_fingerprint: Option<String>,
    pub priority: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub event_id: u64,
    pub event_type: String,
    pub workspace_id: String,
    pub file_id: String,
    pub revision: u64,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchCommitResponse {
    pub commit_id: String,
    pub file_id: String,
    pub previous_revision: u64,
    pub new_revision: u64,
    pub relocated: bool,
    pub status: String,
}

pub struct FileCommitCoordinator {
    files: RwLock<HashMap<String, FileState>>,
    tasks: RwLock<HashMap<String, TaskState>>,
    commits: RwLock<HashMap<String, PatchCommitResponse>>, // Idempotency-Key -> cached response
    outbox: RwLock<Vec<OutboxEvent>>,
    next_event_id: RwLock<u64>,
}

impl FileCommitCoordinator {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            tasks: RwLock::new(HashMap::new()),
            commits: RwLock::new(HashMap::new()),
            outbox: RwLock::new(Vec::new()),
            next_event_id: RwLock::new(1),
        }
    }

    /// 注册/初始化权威源文件
    pub fn register_file(
        &self,
        workspace_id: &str,
        path: &str,
        content: &str,
        language: SupportedLanguage,
    ) -> FileState {
        let file_id = format!("{}:{}", workspace_id, path);
        let hash = SemanticUnit::compute_hash(content);
        let state = FileState {
            file_id: file_id.clone(),
            workspace_id: workspace_id.to_string(),
            path: path.to_string(),
            language,
            current_revision: 1,
            content: content.to_string(),
            content_hash: hash,
            updated_at: Utc::now(),
        };

        self.files.write().unwrap().insert(file_id, state.clone());
        state
    }

    /// 查询文件的最新解析单元
    pub fn get_semantic_units(
        &self,
        file_id: &str,
    ) -> Result<Vec<SemanticUnit>, CoordinatorError> {
        let files = self.files.read().unwrap();
        let file = files
            .get(file_id)
            .ok_or_else(|| CoordinatorError::FileNotFound(file_id.to_string()))?;
        Ok(SemanticParser::parse(
            &file.workspace_id,
            &file.file_id,
            &file.content,
            file.language,
        ))
    }

    /// 将高层目标拆分为针对语义单元的独立可执行任务
    pub fn dispatch_job(
        &self,
        workspace_id: &str,
        job_id: &str,
        target_files: &[String],
        priority: u32,
    ) -> Vec<String> {
        let mut created_task_ids = Vec::new();
        let files = self.files.read().unwrap();
        let mut tasks = self.tasks.write().unwrap();

        for file_key in target_files {
            if let Some(file) = files.get(file_key) {
                let units = SemanticParser::parse(
                    workspace_id,
                    &file.file_id,
                    &file.content,
                    file.language,
                );

                for unit in units {
                    let task_id = format!("{}:{}", job_id, unit.qualified_symbol_path);
                    let task = TaskState {
                        task_id: task_id.clone(),
                        job_id: job_id.to_string(),
                        workspace_id: workspace_id.to_string(),
                        file_id: file.file_id.clone(),
                        symbol_path: unit.qualified_symbol_path,
                        base_revision: file.current_revision,
                        status: TaskStatus::Ready,
                        worker_id: None,
                        fencing_token: 0,
                        lease_expires_at: None,
                        expected_fingerprint: Some(unit.node_fingerprint),
                        priority,
                        created_at: Utc::now(),
                    };
                    tasks.insert(task_id.clone(), task);
                    created_task_ids.push(task_id);
                }
            }
        }

        created_task_ids
    }

    /// Worker 认领就绪任务：分配单调递增的 Fencing Token 与 60s 租约
    pub fn claim_task(
        &self,
        worker_id: &str,
        workspace_id: &str,
    ) -> Option<TaskState> {
        let mut tasks = self.tasks.write().unwrap();
        for task in tasks.values_mut() {
            if task.status == TaskStatus::Ready && task.workspace_id == workspace_id {
                task.status = TaskStatus::Claimed;
                task.worker_id = Some(worker_id.to_string());
                task.fencing_token += 1;
                task.lease_expires_at = Some(Utc::now() + Duration::seconds(60));
                return Some(task.clone());
            }
        }
        None
    }

    /// 心跳续约：校验 Fencing Token
    pub fn heartbeat(
        &self,
        task_id: &str,
        worker_id: &str,
        fencing_token: u64,
    ) -> Result<DateTime<Utc>, CoordinatorError> {
        let mut tasks = self.tasks.write().unwrap();
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| CoordinatorError::TaskNotFound(task_id.to_string()))?;

        if task.worker_id.as_deref() != Some(worker_id) || task.fencing_token != fencing_token {
            return Err(CoordinatorError::FencingTokenStale {
                current: task.fencing_token,
                provided: fencing_token,
            });
        }

        let new_expires = Utc::now() + Duration::seconds(60);
        task.lease_expires_at = Some(new_expires);
        Ok(new_expires)
    }

    /// 核心 CAS 提交：严格验证 Fencing Token、幂等性、版本重定位与发件箱持久化
    pub fn commit_patch(
        &self,
        proposal: PatchProposal,
        idempotency_key: &str,
    ) -> Result<PatchCommitResponse, CoordinatorError> {
        // 1. 检查幂等性缓存
        {
            let commits = self.commits.read().unwrap();
            if let Some(cached) = commits.get(idempotency_key) {
                return Ok(cached.clone());
            }
        }

        // 2. 校验任务状态与 Fencing Token
        {
            let tasks = self.tasks.read().unwrap();
            let task = tasks
                .get(&proposal.task_id)
                .ok_or_else(|| CoordinatorError::TaskNotFound(proposal.task_id.clone()))?;

            if task.fencing_token != proposal.fencing_token {
                return Err(CoordinatorError::FencingTokenStale {
                    current: task.fencing_token,
                    provided: proposal.fencing_token,
                });
            }

            if let Some(expires_at) = task.lease_expires_at {
                if Utc::now() > expires_at {
                    return Err(CoordinatorError::LeaseExpired);
                }
            }
        }

        // 3. 锁定文件并执行 CAS + AST-aware 重定位
        let mut files = self.files.write().unwrap();
        let file = files
            .get_mut(&proposal.file_id)
            .ok_or_else(|| CoordinatorError::FileNotFound(proposal.file_id.clone()))?;

        let patch_result = PatchEngine::relocate_and_apply(
            &file.workspace_id,
            &file.file_id,
            &file.content,
            file.current_revision,
            file.language,
            &proposal,
        )?;

        // 4. 更新文件状态
        file.current_revision = patch_result.new_revision;
        file.content = patch_result.new_content.clone();
        file.content_hash = SemanticUnit::compute_hash(&file.content);
        file.updated_at = Utc::now();

        // 5. 更新任务状态
        {
            let mut tasks = self.tasks.write().unwrap();
            if let Some(task) = tasks.get_mut(&proposal.task_id) {
                task.status = TaskStatus::Committed;
            }
        }

        // 6. 生成响应并写入幂等记录
        let commit_id = format!("commit-{}-rev{}", proposal.task_id, patch_result.new_revision);
        let response = PatchCommitResponse {
            commit_id: commit_id.clone(),
            file_id: proposal.file_id.clone(),
            previous_revision: patch_result.previous_revision,
            new_revision: patch_result.new_revision,
            relocated: patch_result.relocated,
            status: "COMMITTED".to_string(),
        };

        self.commits
            .write()
            .unwrap()
            .insert(idempotency_key.to_string(), response.clone());

        // 7. 发件箱事件记录
        {
            let mut event_id_lock = self.next_event_id.write().unwrap();
            let event_id = *event_id_lock;
            *event_id_lock += 1;

            let mut outbox = self.outbox.write().unwrap();
            outbox.push(OutboxEvent {
                event_id,
                event_type: "FILE_COMMITTED".to_string(),
                workspace_id: file.workspace_id.clone(),
                file_id: file.file_id.clone(),
                revision: file.current_revision,
                payload: serde_json::json!({
                    "commit_id": commit_id,
                    "task_id": proposal.task_id,
                    "worker_id": proposal.worker_id,
                    "symbol_path": proposal.symbol_path,
                    "relocated": patch_result.relocated,
                }),
                created_at: Utc::now(),
            });
        }

        Ok(response)
    }

    /// 回收过期租约，将任务重入队
    pub fn reap_expired_leases(&self) -> usize {
        let mut count = 0;
        let mut tasks = self.tasks.write().unwrap();
        let now = Utc::now();

        for task in tasks.values_mut() {
            if task.status == TaskStatus::Claimed {
                if let Some(exp) = task.lease_expires_at {
                    if now > exp {
                        task.status = TaskStatus::Ready;
                        task.worker_id = None;
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// 读取发件箱事件
    pub fn get_events(&self, after_cursor: u64) -> Vec<OutboxEvent> {
        let outbox = self.outbox.read().unwrap();
        outbox
            .iter()
            .filter(|e| e.event_id > after_cursor)
            .cloned()
            .collect()
    }
}

impl Default for FileCommitCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
