//! 多租户 SaaS 云端协同与配额管控引擎 (Multi-Tenant SaaS Engine)
//!
//! 在保留纯 Rust 高性能与内存安全的基础上，为 SaaS 云端 IDE 提供：
//! 1. 租户物理逻辑隔离 (Tenant Isolation: Org -> Workspace -> File)
//! 2. 租户订阅层级与配额控制 (Quota: Enterprise / Pro / Standard 并发 AI 限制与限流)
//! 3. 多租户协同 Hub (MultiTenantCollaborationHub: 跨租户物理零穿透)
//! 4. 终局审查与 SHA-256 签名证书防伪

use crate::collaboration::{CollaborationSession, CollaboratorRole, KeystrokeBroadcast, KeystrokeOp, RemoteCursor};
use crate::reviewer::{AcceptanceCertificate, GuidanceZone, ReviewDecision, ReviewSession, UpfrontPlan};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// 租户订阅层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenantTier {
    Enterprise,
    Pro,
    Standard,
}

impl TenantTier {
    pub fn default_quota(&self) -> TenantQuota {
        match self {
            Self::Enterprise => TenantQuota {
                max_concurrent_agents: 50,
                keystroke_rate_limit: 1000,
            },
            Self::Pro => TenantQuota {
                max_concurrent_agents: 10,
                keystroke_rate_limit: 200,
            },
            Self::Standard => TenantQuota {
                max_concurrent_agents: 3,
                keystroke_rate_limit: 50,
            },
        }
    }
}

/// 租户配额约束
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantQuota {
    pub max_concurrent_agents: usize,
    pub keystroke_rate_limit: usize,
}

/// 租户上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: String,
    pub name: String,
    pub tier: TenantTier,
    pub quota: TenantQuota,
    pub active_agents: usize,
}

impl TenantContext {
    pub fn new(tenant_id: impl Into<String>, name: impl Into<String>, tier: TenantTier) -> Self {
        let quota = tier.default_quota();
        Self {
            tenant_id: tenant_id.into(),
            name: name.into(),
            tier,
            quota,
            active_agents: 0,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TenantError {
    #[error("Tenant '{0}' not found")]
    TenantNotFound(String),

    #[error("Tenant '{tenant_id}' exceeded max concurrent AI agent quota ({max})")]
    QuotaExceeded { tenant_id: String, max: usize },

    #[error("Cross-tenant isolation violation between '{tenant_a}' and '{tenant_b}'")]
    IsolationViolation { tenant_a: String, tenant_b: String },

    #[error("Session error: {0}")]
    SessionError(String),
}

/// 多租户文件协同会话
pub struct MultiTenantSession {
    pub tenant_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub collab: CollaborationSession,
    pub review: ReviewSession,
}

impl MultiTenantSession {
    pub fn new(
        tenant_id: String,
        workspace_id: String,
        file_id: String,
        base_revision: u64,
    ) -> Self {
        Self {
            tenant_id: tenant_id.clone(),
            workspace_id,
            file_id: file_id.clone(),
            collab: CollaborationSession::new(file_id.clone(), base_revision),
            review: ReviewSession::new(
                format!("{}:{}", tenant_id, file_id),
                file_id,
                "reviewer_lead".into(),
                "Reviewer Lead".into(),
                base_revision,
                base_revision + 1,
            ),
        }
    }
}

/// 多租户 SaaS 协同总枢纽 (Multi-Tenant SaaS Collaboration Hub)
#[derive(Default)]
pub struct MultiTenantCollaborationHub {
    tenants: RwLock<HashMap<String, TenantContext>>,
    /// Key: (tenant_id, workspace_id, file_id)
    sessions: RwLock<HashMap<(String, String, String), Arc<RwLock<MultiTenantSession>>>>,
}

impl MultiTenantCollaborationHub {
    pub fn new() -> Self {
        let mut default_tenants = HashMap::new();
        default_tenants.insert(
            "org_default".into(),
            TenantContext::new("org_default", "Acme Corp (Global)", TenantTier::Enterprise),
        );
        default_tenants.insert(
            "org_demo".into(),
            TenantContext::new("org_demo", "Startup Demo", TenantTier::Standard),
        );

        Self {
            tenants: RwLock::new(default_tenants),
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// 注册新租户或更新配额
    pub fn register_tenant(&self, context: TenantContext) {
        let mut guard = self.tenants.write().unwrap();
        guard.insert(context.tenant_id.clone(), context);
    }

    /// 查询租户上下文与实时活跃 Agent 数
    pub fn get_tenant_context(&self, tenant_id: &str) -> Option<TenantContext> {
        let tenants_guard = self.tenants.read().unwrap();
        let mut ctx = tenants_guard.get(tenant_id)?.clone();

        // 统计此租户下所有活跃 AI
        let sessions_guard = self.sessions.read().unwrap();
        let mut active = 0;
        for ((t, _, _), session_ref) in sessions_guard.iter() {
            if t == tenant_id {
                if let Ok(sess) = session_ref.read() {
                    active += sess.collab.collaborators.len();
                }
            }
        }
        ctx.active_agents = active;
        Some(ctx)
    }

    /// 获取或创建租户隔离的文件协同会话
    pub fn get_or_create_session(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        file_id: &str,
        base_revision: u64,
    ) -> Result<Arc<RwLock<MultiTenantSession>>, TenantError> {
        if self.get_tenant_context(tenant_id).is_none() {
            return Err(TenantError::TenantNotFound(tenant_id.to_string()));
        }

        let key = (
            tenant_id.to_string(),
            workspace_id.to_string(),
            file_id.to_string(),
        );
        let mut guard = self.sessions.write().unwrap();
        if let Some(s) = guard.get(&key) {
            return Ok(Arc::clone(s));
        }

        let session = Arc::new(RwLock::new(MultiTenantSession::new(
            tenant_id.to_string(),
            workspace_id.to_string(),
            file_id.to_string(),
            base_revision,
        )));
        guard.insert(key, Arc::clone(&session));
        Ok(session)
    }

    /// 注册或更新协作者（带租户并发配额校验）
    pub fn register_presence(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        file_id: &str,
        agent_id: &str,
        display_name: &str,
        role: CollaboratorRole,
        cursor: [usize; 2],
        selection: Option<[usize; 4]>,
        active_symbol: Option<String>,
    ) -> Result<RemoteCursor, TenantError> {
        let tenant_ctx = self
            .get_tenant_context(tenant_id)
            .ok_or_else(|| TenantError::TenantNotFound(tenant_id.to_string()))?;

        let session_ref = self.get_or_create_session(tenant_id, workspace_id, file_id, 1)?;
        let mut session = session_ref.write().unwrap();

        // 仅在新增 Agent 时做配额检查
        if !session.collab.collaborators.contains_key(agent_id)
            && tenant_ctx.active_agents >= tenant_ctx.quota.max_concurrent_agents
        {
            return Err(TenantError::QuotaExceeded {
                tenant_id: tenant_id.to_string(),
                max: tenant_ctx.quota.max_concurrent_agents,
            });
        }

        let rc = session.collab.register_or_update_presence(
            agent_id.to_string(),
            display_name.to_string(),
            role,
            cursor,
            selection,
            active_symbol,
        );

        Ok(rc)
    }

    /// 广播微观击键
    pub fn apply_keystroke(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        file_id: &str,
        agent_id: &str,
        op: KeystrokeOp,
        cursor_after: [usize; 2],
        selection_after: Option<[usize; 4]>,
    ) -> Result<KeystrokeBroadcast, TenantError> {
        let session_ref = self.get_or_create_session(tenant_id, workspace_id, file_id, 1)?;
        let mut session = session_ref.write().unwrap();

        session
            .collab
            .apply_keystroke(agent_id, op, cursor_after, selection_after)
            .map_err(TenantError::SessionError)
    }

    /// 发布 Reviewer 最初规划与作业区
    pub fn set_upfront_plan(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        file_id: &str,
        task_goal: String,
        invariants: Vec<String>,
        guidance_zones: Vec<GuidanceZone>,
        shared_contracts: Vec<String>,
        notes: String,
    ) -> Result<UpfrontPlan, TenantError> {
        let session_ref = self.get_or_create_session(tenant_id, workspace_id, file_id, 1)?;
        let mut session = session_ref.write().unwrap();

        // 注册到 AnchorOffsetTable，保证坐标平移支持
        for zone in &guidance_zones {
            session
                .collab
                .anchor_table
                .register_zone(&zone.zone_id, zone.suggested_line_range[0]);
        }

        let plan = UpfrontPlan {
            task_goal,
            invariants,
            guidance_zones,
            shared_contracts,
            notes,
        };
        session.review.publish_upfront_plan(plan.clone());
        Ok(plan)
    }

    /// 终局统筹审查验收裁决与数字签名证书签署
    pub fn submit_review_decision(
        &self,
        tenant_id: &str,
        workspace_id: &str,
        file_id: &str,
        _reviewer_id: &str,
        decision: ReviewDecision,
        final_code: &str,
    ) -> Result<AcceptanceCertificate, TenantError> {
        let session_ref = self.get_or_create_session(tenant_id, workspace_id, file_id, 1)?;
        let mut session = session_ref.write().unwrap();

        session
            .review
            .apply_decision(decision)
            .map_err(TenantError::SessionError)?;

        session
            .review
            .finalize_acceptance(final_code)
            .map_err(TenantError::SessionError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_quota_enforcement() {
        let hub = MultiTenantCollaborationHub::new();
        // org_demo 的 Standard 配额是 3 个 AI
        let tenant = "org_demo";

        for i in 1..=3 {
            let res = hub.register_presence(
                tenant,
                "ws_1",
                "src/lib.rs",
                &format!("ai_{}", i),
                &format!("AI-{}", i),
                CollaboratorRole::AiCoder,
                [i, 0],
                None,
                None,
            );
            assert!(res.is_ok(), "第 {} 个 AI 应该允许加入", i);
        }

        // 第 4 个 AI 超出配额，必须被拦截
        let overflow = hub.register_presence(
            tenant,
            "ws_1",
            "src/lib.rs",
            "ai_4",
            "AI-4",
            CollaboratorRole::AiOptimizer,
            [4, 0],
            None,
            None,
        );

        assert!(matches!(
            overflow,
            Err(TenantError::QuotaExceeded { max: 3, .. })
        ));
    }

    #[test]
    fn test_multi_tenant_physical_isolation() {
        let hub = MultiTenantCollaborationHub::new();

        // 租户 A (Enterprise)
        let _ = hub.register_presence(
            "org_default",
            "main",
            "src/service.rs",
            "agent_a",
            "Agent A",
            CollaboratorRole::AiCoder,
            [1, 0],
            None,
            None,
        );

        // 租户 B (Standard)
        let _ = hub.register_presence(
            "org_demo",
            "main",
            "src/service.rs",
            "agent_b",
            "Agent B",
            CollaboratorRole::AiCoder,
            [1, 0],
            None,
            None,
        );

        let sess_a_ref = hub
            .get_or_create_session("org_default", "main", "src/service.rs", 1)
            .unwrap();
        let sess_a = sess_a_ref.read().unwrap();

        let sess_b_ref = hub
            .get_or_create_session("org_demo", "main", "src/service.rs", 1)
            .unwrap();
        let sess_b = sess_b_ref.read().unwrap();

        // 租户 A 只包含 agent_a，绝对不可见 agent_b
        assert!(sess_a.collab.collaborators.contains_key("agent_a"));
        assert!(!sess_a.collab.collaborators.contains_key("agent_b"));

        // 租户 B 只包含 agent_b，绝对不可见 agent_a
        assert!(sess_b.collab.collaborators.contains_key("agent_b"));
        assert!(!sess_b.collab.collaborators.contains_key("agent_a"));
    }

    #[test]
    fn test_tenant_review_decision_and_certificate() {
        let hub = MultiTenantCollaborationHub::new();
        let tenant = "org_default";

        let plan = hub
            .set_upfront_plan(
                tenant,
                "main",
                "src/algo.rs",
                "高并发加法运算".into(),
                vec!["无溢出".into()],
                vec![GuidanceZone {
                    zone_id: "zone_add".into(),
                    symbol_or_desc: "add".into(),
                    suggested_line_range: [1, 10],
                    recommended_agent: None,
                    recommended_role: CollaboratorRole::AiCoder,
                    intent_summary: "编写基础算子".into(),
                }],
                vec!["pub fn add(a: i32, b: i32) -> i32".into()],
                "保持无锁".into(),
            )
            .unwrap();

        assert_eq!(plan.guidance_zones.len(), 1);

        // 终局验收并签署证书
        let cert = hub
            .submit_review_decision(
                tenant,
                "main",
                "src/algo.rs",
                "reviewer_lead",
                ReviewDecision::ApproveAll,
                "pub fn add(a: i32, b: i32) -> i32 { a + b }",
            )
            .unwrap();

        assert!(cert.certificate_id.starts_with("CERT-"));
        assert_eq!(cert.content_sha256.len(), 64);
    }
}
