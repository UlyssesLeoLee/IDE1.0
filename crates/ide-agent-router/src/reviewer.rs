//! 统筹审查验收与多 AI 协同决策流水线 (Unified Reviewer & Acceptance Pipeline)
//!
//! 在多 AI 共同编辑同一文件后，由 Reviewer (人类或专门的 Reviewer Agent) 负责：
//! 1. 汇聚与展示多 AI 的并发修改差异 (Multi-Agent Attributed Diff Aggregation)
//! 2. 自动化前置验证门禁 (语法、类型、编译与测试验证)
//! 3. 验收裁决：全量通过 (ApproveAll)、局部采纳 (ApproveHunks)、要求重构 (RequestRework)
//! 4. 签发不可篡改的审查验收凭证 (Acceptance Certificate & Audit Record)

use crate::collaboration::CollaboratorRole;
use crate::verifier::{VerificationReport, VerificationStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 修改块状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HunkStatus {
    Pending,
    Approved,
    Rejected,
    ReworkRequested,
}

/// 归属于特定 Agent 的修改差异块
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttributedHunk {
    pub hunk_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub role: CollaboratorRole,
    pub start_line: usize,
    pub end_line: usize,
    pub original_text: String,
    pub proposed_text: String,
    pub status: HunkStatus,
    pub reviewer_comment: Option<String>,
}

/// 审查验收会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSession {
    pub session_id: String,
    pub file_id: String,
    pub reviewer_id: String,
    pub reviewer_name: String,
    pub base_revision: u64,
    pub target_revision: u64,
    pub hunks: Vec<AttributedHunk>,
    pub verification: Option<VerificationReport>,
    pub created_at: DateTime<Utc>,
    pub finalized_at: Option<DateTime<Utc>>,
}

/// 审查裁决动作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewDecision {
    /// 全量批准合并
    ApproveAll,
    /// 批准特定修改块
    ApproveHunks(Vec<String>),
    /// 驳回特定修改块
    RejectHunks {
        hunk_ids: Vec<String>,
        reason: String,
    },
    /// 要求特定 Agent 针对指定行返工
    RequestRework {
        agent_id: String,
        target_lines: [usize; 2],
        instructions: String,
    },
}

/// 验收报告与不可篡改审计凭证
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptanceCertificate {
    pub certificate_id: String,
    pub session_id: String,
    pub file_id: String,
    pub reviewer_id: String,
    pub approved_agents: Vec<String>,
    pub total_hunks_approved: usize,
    pub total_hunks_rejected: usize,
    pub total_hunks_rework: usize,
    pub verification_passed: bool,
    pub content_sha256: String,
    pub approved_at: DateTime<Utc>,
}

impl ReviewSession {
    pub fn new(
        session_id: String,
        file_id: String,
        reviewer_id: String,
        reviewer_name: String,
        base_revision: u64,
        target_revision: u64,
    ) -> Self {
        Self {
            session_id,
            file_id,
            reviewer_id,
            reviewer_name,
            base_revision,
            target_revision,
            hunks: Vec::new(),
            verification: None,
            created_at: Utc::now(),
            finalized_at: None,
        }
    }

    /// 添加待审查的修改块
    pub fn add_hunk(
        &mut self,
        hunk_id: String,
        agent_id: String,
        agent_name: String,
        role: CollaboratorRole,
        start_line: usize,
        end_line: usize,
        original_text: String,
        proposed_text: String,
    ) {
        self.hunks.push(AttributedHunk {
            hunk_id,
            agent_id,
            agent_name,
            role,
            start_line,
            end_line,
            original_text,
            proposed_text,
            status: HunkStatus::Pending,
            reviewer_comment: None,
        });
    }

    /// 关联自动化验证流水线报告
    pub fn attach_verification(&mut self, report: VerificationReport) {
        self.verification = Some(report);
    }

    /// 应用审查裁决
    pub fn apply_decision(&mut self, decision: ReviewDecision) -> Result<(), String> {
        match decision {
            ReviewDecision::ApproveAll => {
                for h in &mut self.hunks {
                    h.status = HunkStatus::Approved;
                }
            }
            ReviewDecision::ApproveHunks(ids) => {
                for h in &mut self.hunks {
                    if ids.contains(&h.hunk_id) {
                        h.status = HunkStatus::Approved;
                    }
                }
            }
            ReviewDecision::RejectHunks { hunk_ids, reason } => {
                for h in &mut self.hunks {
                    if hunk_ids.contains(&h.hunk_id) {
                        h.status = HunkStatus::Rejected;
                        h.reviewer_comment = Some(reason.clone());
                    }
                }
            }
            ReviewDecision::RequestRework {
                agent_id,
                target_lines,
                instructions,
            } => {
                for h in &mut self.hunks {
                    if h.agent_id == agent_id
                        && h.start_line >= target_lines[0]
                        && h.end_line <= target_lines[1]
                    {
                        h.status = HunkStatus::ReworkRequested;
                        h.reviewer_comment = Some(instructions.clone());
                    }
                }
            }
        }
        Ok(())
    }

    /// 完成验收并签发审计凭证
    pub fn finalize_acceptance(&mut self, final_content: &str) -> Result<AcceptanceCertificate, String> {
        // 门禁检验: 若存在阻断性验证失败，严禁验收签发
        if let Some(v) = &self.verification {
            if v.status == VerificationStatus::Conflict || v.status == VerificationStatus::SecurityBlocked {
                return Err(format!(
                    "Cannot finalize acceptance: Automated verification failed with blocking status {:?}",
                    v.status
                ));
            }
        }

        let mut approved_agents = Vec::new();
        let mut approved_count = 0;
        let mut rejected_count = 0;
        let mut rework_count = 0;

        for h in &self.hunks {
            match h.status {
                HunkStatus::Approved => {
                    approved_count += 1;
                    if !approved_agents.contains(&h.agent_id) {
                        approved_agents.push(h.agent_id.clone());
                    }
                }
                HunkStatus::Rejected => rejected_count += 1,
                HunkStatus::ReworkRequested => rework_count += 1,
                HunkStatus::Pending => {
                    return Err(format!("Cannot finalize: Hunk '{}' is still Pending", h.hunk_id));
                }
            }
        }

        // 计算最终文本的 SHA-256 哈希
        let mut hasher = Sha256::new();
        hasher.update(final_content.as_bytes());
        let content_sha256 = format!("{:x}", hasher.finalize());

        let now = Utc::now();
        self.finalized_at = Some(now);

        let cert = AcceptanceCertificate {
            certificate_id: format!("CERT-{}-{}", self.session_id, now.timestamp()),
            session_id: self.session_id.clone(),
            file_id: self.file_id.clone(),
            reviewer_id: self.reviewer_id.clone(),
            approved_agents,
            total_hunks_approved: approved_count,
            total_hunks_rejected: rejected_count,
            total_hunks_rework: rework_count,
            verification_passed: self
                .verification
                .as_ref()
                .map(|v| v.status == VerificationStatus::Passed || v.status == VerificationStatus::FormatWarning)
                .unwrap_or(true),
            content_sha256,
            approved_at: now,
        };

        Ok(cert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reviewer_pipeline_and_certificate() {
        let mut session = ReviewSession::new(
            "session_001".into(),
            "src/parser.rs".into(),
            "reviewer_lead".into(),
            "Staff Lead Reviewer".into(),
            10,
            11,
        );

        // 1. 录入两位 Agent 的并发修改
        session.add_hunk(
            "hunk_1".into(),
            "agent_coder".into(),
            "Coder Alpha".into(),
            CollaboratorRole::AiCoder,
            15,
            25,
            "fn old_parser() {}".into(),
            "fn new_ast_parser() { /* optimized */ }".into(),
        );

        session.add_hunk(
            "hunk_2".into(),
            "agent_optimizer".into(),
            "Optimizer Beta".into(),
            CollaboratorRole::AiOptimizer,
            40,
            50,
            "let x = 1;".into(),
            "let x: u64 = 1;".into(),
        );

        // 2. 模拟 Reviewer 裁决: Approve All
        session.apply_decision(ReviewDecision::ApproveAll).unwrap();
        assert_eq!(session.hunks[0].status, HunkStatus::Approved);
        assert_eq!(session.hunks[1].status, HunkStatus::Approved);

        // 3. 验收签发 Certificate
        let cert = session
            .finalize_acceptance("fn new_ast_parser() {}\nlet x: u64 = 1;\n")
            .expect("finalize should succeed");

        assert_eq!(cert.total_hunks_approved, 2);
        assert_eq!(cert.approved_agents.len(), 2);
        assert!(cert.verification_passed);
        assert!(!cert.content_sha256.is_empty());
    }

    #[test]
    fn test_reviewer_partial_rework_decision() {
        let mut session = ReviewSession::new(
            "session_002".into(),
            "src/lib.rs".into(),
            "reviewer_bot".into(),
            "AI Reviewer".into(),
            5,
            6,
        );

        session.add_hunk(
            "hunk_1".into(),
            "agent_coder".into(),
            "Coder Alpha".into(),
            CollaboratorRole::AiCoder,
            10,
            20,
            "old".into(),
            "new".into(),
        );

        // 要求返工
        session
            .apply_decision(ReviewDecision::RequestRework {
                agent_id: "agent_coder".into(),
                target_lines: [10, 20],
                instructions: "Please add comprehensive error handling for EOF".into(),
            })
            .unwrap();

        assert_eq!(session.hunks[0].status, HunkStatus::ReworkRequested);
        assert_eq!(
            session.hunks[0].reviewer_comment.as_deref(),
            Some("Please add comprehensive error handling for EOF")
        );
    }
}
