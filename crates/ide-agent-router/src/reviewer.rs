//! 统筹审查验收与多 AI 协同决策流水线 (Unified Reviewer & Guidance Pipeline)
//!
//! 核心理念 (Core Philosophy):
//! 1. **最初规划，避免冲突 (Upfront Architectural Planning)**:
//!    Reviewer 在多 AI 开始编码前，先行制订架构不变量、接口契约与建议作业区（Guidance Zones），
//!    从源头降低认知发散与冲突概率。
//! 2. **指引而非限制 (Guidance over Restrictions)**:
//!    不采用生硬的全局物理互斥锁束缚 AI 探索；通过实时广播咨询建议（Reviewer Hints），
//!    动态指引各 AI 自主对齐，激发最大并行推导效率。
//! 3. **终局统筹验收 (Holistic Convergence & Acceptance)**:
//!    在协同编辑收敛后，汇聚各 Agent 差异、执行自动化前置验证门禁、行使审查裁决，
//!    并签署不可篡改的验收凭证（Acceptance Certificate）。

use crate::collaboration::CollaboratorRole;
use crate::verifier::{VerificationReport, VerificationStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 审查协同生命周期阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewPhase {
    /// 阶段 1: 最初规划与指引 (Reviewer 制订目标、划定建议作业区与契约)
    UpfrontPlanning,
    /// 阶段 2: 自由推导与实时协同 (多 AI 根据指引并发编辑，Reviewer 动态巡航提示)
    LiveCoEditing,
    /// 阶段 3: 终局统筹审查与验收 (自动化验证、差异裁决与签发凭证)
    HolisticReview,
    /// 已完成验收并合入基准版本
    Accepted,
}

/// 建议作业区 (指引而非限制：给 AI 建议关注点与范围，不锁死文件)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuidanceZone {
    pub zone_id: String,
    pub symbol_or_desc: String,
    pub suggested_line_range: [usize; 2],
    pub recommended_agent: Option<String>,
    pub recommended_role: CollaboratorRole,
    pub intent_summary: String,
}

/// 最初规划方案 (Upfront Architectural Plan)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UpfrontPlan {
    pub task_goal: String,
    pub invariants: Vec<String>,
    pub guidance_zones: Vec<GuidanceZone>,
    pub shared_contracts: Vec<String>,
    pub notes: String,
}

/// 审查员实时广播指引 (非阻塞式建议与协同对齐)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewerHint {
    pub hint_id: u64,
    pub timestamp_epoch_ms: u64,
    pub level: HintLevel,
    pub message: String,
    pub target_agent: Option<String>,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HintLevel {
    /// 架构背景与建议
    Advisory,
    /// 接口契约对齐 (如“AI-1 已定义 Result 类型，AI-2 请直接复用”)
    ContractAlignment,
    /// 潜在重叠预警 (如“两 Agent 正在接近同一行，建议分工”)
    PotentialOverlap,
    /// 正向认可
    Encouragement,
}

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
    pub current_phase: ReviewPhase,
    pub plan: UpfrontPlan,
    pub hints: Vec<ReviewerHint>,
    pub hunks: Vec<AttributedHunk>,
    pub verification: Option<VerificationReport>,
    pub created_at: DateTime<Utc>,
    pub finalized_at: Option<DateTime<Utc>>,
    next_hint_id: u64,
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
            current_phase: ReviewPhase::UpfrontPlanning,
            plan: UpfrontPlan::default(),
            hints: Vec::new(),
            hunks: Vec::new(),
            verification: None,
            created_at: Utc::now(),
            finalized_at: None,
            next_hint_id: 1,
        }
    }

    /// 最初规划设定 (发布架构目标与指引作业区，流转至协同推导期)
    pub fn publish_upfront_plan(&mut self, plan: UpfrontPlan) {
        self.plan = plan;
        self.current_phase = ReviewPhase::LiveCoEditing;
    }

    /// 审查员广播动态指引 (指引而非限制)
    pub fn broadcast_hint(
        &mut self,
        level: HintLevel,
        message: String,
        target_agent: Option<String>,
        suggested_action: Option<String>,
    ) -> ReviewerHint {
        let hint = ReviewerHint {
            hint_id: self.next_hint_id,
            timestamp_epoch_ms: Utc::now().timestamp_millis() as u64,
            level,
            message,
            target_agent,
            suggested_action,
        };
        self.next_hint_id += 1;
        self.hints.push(hint.clone());
        hint
    }

    /// 流转生命周期阶段
    pub fn transition_phase(&mut self, next: ReviewPhase) {
        self.current_phase = next;
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
        self.current_phase = ReviewPhase::Accepted;

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
    fn test_upfront_planning_and_live_hints() {
        let mut session = ReviewSession::new(
            "session_plan_001".into(),
            "src/service.rs".into(),
            "reviewer_lead".into(),
            "Architect Lead".into(),
            1,
            2,
        );

        assert_eq!(session.current_phase, ReviewPhase::UpfrontPlanning);

        // 1. 最初规划，划分指引作业区与接口契约
        let plan = UpfrontPlan {
            task_goal: "实现高并发订单处理流水线".into(),
            invariants: vec!["所有状态迁移必须幂等".into(), "订单号必须唯一".into()],
            guidance_zones: vec![
                GuidanceZone {
                    zone_id: "zone_data".into(),
                    symbol_or_desc: "OrderPayload struct & Validation".into(),
                    suggested_line_range: [10, 50],
                    recommended_agent: Some("agent_coder_1".into()),
                    recommended_role: CollaboratorRole::AiCoder,
                    intent_summary: "负责数据校验与模型定义".into(),
                },
                GuidanceZone {
                    zone_id: "zone_exec".into(),
                    symbol_or_desc: "fn process_order execution engine".into(),
                    suggested_line_range: [51, 120],
                    recommended_agent: Some("agent_coder_2".into()),
                    recommended_role: CollaboratorRole::AiOptimizer,
                    intent_summary: "负责并发执行与异步重试".into(),
                },
            ],
            shared_contracts: vec!["pub struct OrderResult { pub order_id: String, pub ok: bool }".into()],
            notes: "非阻断性指引，各 Agent 自主对齐契约".into(),
        };

        session.publish_upfront_plan(plan);
        assert_eq!(session.current_phase, ReviewPhase::LiveCoEditing);
        assert_eq!(session.plan.guidance_zones.len(), 2);

        // 2. 实时指引广播 (指引而非限制)
        let hint = session.broadcast_hint(
            HintLevel::ContractAlignment,
            "AI-1 已定义 OrderResult 契约，AI-2 可直接在 process_order 返回值复用".into(),
            Some("agent_coder_2".into()),
            Some("use OrderResult".into()),
        );
        assert_eq!(hint.hint_id, 1);
        assert_eq!(session.hints.len(), 1);

        // 3. 协同结束，流转到终局审查
        session.transition_phase(ReviewPhase::HolisticReview);
        assert_eq!(session.current_phase, ReviewPhase::HolisticReview);
    }

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

        session.publish_upfront_plan(UpfrontPlan {
            task_goal: "AST 解析性能优化与类型补全".into(),
            ..Default::default()
        });

        // 录入两位 Agent 的并发修改
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

        session.transition_phase(ReviewPhase::HolisticReview);

        // Reviewer 裁决: Approve All
        session.apply_decision(ReviewDecision::ApproveAll).unwrap();
        assert_eq!(session.hunks[0].status, HunkStatus::Approved);
        assert_eq!(session.hunks[1].status, HunkStatus::Approved);

        // 验收签发 Certificate
        let cert = session
            .finalize_acceptance("fn new_ast_parser() {}\nlet x: u64 = 1;\n")
            .expect("finalize should succeed");

        assert_eq!(cert.total_hunks_approved, 2);
        assert_eq!(cert.approved_agents.len(), 2);
        assert!(cert.verification_passed);
        assert_eq!(session.current_phase, ReviewPhase::Accepted);
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
