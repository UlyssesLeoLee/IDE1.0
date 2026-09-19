//! kernel-session — Tier 2: Session ライフサイクル・権限管理
//!
//! DD-02 §MOD-SM-001 (Session Manager) を実装。Session 作成・取得・削除・権限検査・
//! リソース予算管理・最近開いた Buffer 履歴を提供する。
//!
//! トレース: REQ-001 §FR-005 / §FR-016 / §FR-017 / §FR-020

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use kernel_error::{CoreError, CoreResult};
use kernel_eventbus::EventBus;
use std::sync::Arc;
use uuid::Uuid;

/// Session を識別する ID。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Actor (REQ §FR-005-02) — 6 種を区別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActorKind {
    Human,
    LangGraph,
    Agent,
    Ci,
    Plugin,
    Automation,
}

/// 権限スコープ。MVP-1 では固定の Capability allow-list で表現。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionSet {
    /// 呼び出し可能な Capability 名集合 (e.g. `["text.search", "buffer.read"]`)
    pub capabilities: Vec<String>,
    /// Workspace 変更コマンドを許可するか (e.g. `file.patch`, `txn.commit`)
    pub allow_workspace_mutation: bool,
    /// Remote AI 呼び出しを許可するか (DD-13 §F-11)
    pub allow_remote_ai: bool,
}

impl PermissionSet {
    /// 既定の "Human / Editor" 権限セット。
    #[must_use]
    pub fn human_editor() -> Self {
        Self {
            capabilities: vec![
                "buffer.read".into(),
                "buffer.write".into(),
                "text.search".into(),
                "file.patch".into(),
                "txn.commit".into(),
                "context.build".into(),
                "ai.chat".into(),
                "ai.edit".into(),
                "ai.inline".into(),
                "echo".into(),
            ],
            allow_workspace_mutation: true,
            allow_remote_ai: false,
        }
    }

    /// 制限付きの "Agent" 権限セット (MVP-1 既定)。
    #[must_use]
    pub fn agent_default() -> Self {
        Self {
            capabilities: vec![
                "buffer.read".into(),
                "text.search".into(),
                "symbol.search".into(),
                "context.build".into(),
                "ai.chat".into(),
                "ai.edit".into(),
                "txn.begin".into(),
                "txn.commit".into(),
            ],
            allow_workspace_mutation: true,
            allow_remote_ai: false,
        }
    }
}

/// リソース予算 (REQ §FR-005-04 / DD-02 §6.5)。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceBudget {
    pub max_tokens_per_minute: u32,
    pub max_commands_per_session: u32,
    pub max_concurrent_commands: u32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_tokens_per_minute: 100_000,
            max_commands_per_session: 10_000,
            max_concurrent_commands: 8,
        }
    }
}

/// Session 本体。
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub actor: ActorKind,
    pub permissions: PermissionSet,
    pub budget: ResourceBudget,
    pub created_at: DateTime<Utc>,
    pub last_active_at: parking_lot::Mutex<DateTime<Utc>>,
    pub recent_buffers: parking_lot::Mutex<Vec<String>>, // file paths
    pub commands_executed: u32,
}

impl Session {
    /// 最近開いた Buffer を更新 (DD-02 §6.4)。
    pub fn touch_buffer(&self, path: impl Into<String>) {
        let mut g = self.recent_buffers.lock();
        let p = path.into();
        g.retain(|x| x != &p);
        g.insert(0, p);
        if g.len() > 32 {
            g.truncate(32);
        }
    }

    /// 最近開いた Buffer 履歴を取得。
    #[must_use]
    pub fn recent_buffers(&self) -> Vec<String> {
        self.recent_buffers.lock().clone()
    }

    /// Session の最終アクセス時刻を更新。
    pub fn touch(&self) {
        *self.last_active_at.lock() = Utc::now();
        // commands_executed is owned mutably; skip update here to keep trait simple.
    }
}

/// Session Manager。
///
/// Session を in-memory に保持し、作成・取得・削除・権限検査を提供する。
/// MVP-2 で `rusqlite` ベースの永続化を追加 (DD-02 §6.8)。
pub struct SessionManager {
    sessions: DashMap<SessionId, Arc<Session>>,
    event_bus: Arc<EventBus>,
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("session_count", &self.sessions.len())
            .finish()
    }
}

impl SessionManager {
    /// 新しい Session Manager を生成。
    #[must_use]
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            sessions: DashMap::new(),
            event_bus,
        }
    }

    /// 新規 Session を作成し、Event Bus に `session.created` を発行。
    /// # Errors
    /// Event Bus への発行失敗時に [`CoreError::Internal`] を返す。
    pub async fn create(
        &self,
        actor: ActorKind,
        permissions: PermissionSet,
        budget: ResourceBudget,
    ) -> CoreResult<Arc<Session>> {
        let session = Arc::new(Session {
            id: SessionId::new(),
            actor,
            permissions,
            budget,
            created_at: Utc::now(),
            last_active_at: parking_lot::Mutex::new(Utc::now()),
            recent_buffers: parking_lot::Mutex::new(Vec::new()),
            commands_executed: 0,
        });
        self.sessions.insert(session.id.clone(), session.clone());

        self.event_bus
            .publish(
                kernel_eventbus::event_type::SESSION_CREATED,
                kernel_eventbus::Durability::Durable,
                session.id.0.to_string(),
                String::new(),
                serde_json::json!({
                    "session_id": session.id.0.to_string(),
                    "actor": format!("{:?}", session.actor),
                }),
            )
            .await?;

        Ok(session)
    }

    /// Session を取得。
    /// # Errors
    /// 該当 Session が存在しない場合に [`CoreError::SessionNotFound`] を返す。
    pub fn get(&self, id: &SessionId) -> CoreResult<Arc<Session>> {
        self.sessions
            .get(id)
            .map(|r| r.value().clone())
            .ok_or_else(|| CoreError::SessionNotFound(id.0.to_string()))
    }

    /// Session を削除し、`session.closed` を発行。
    /// # Errors
    /// Event Bus への発行失敗、または存在しない ID 指定でエラーを返す。
    pub async fn delete(&self, id: &SessionId) -> CoreResult<()> {
        let _session = self
            .sessions
            .remove(id)
            .ok_or_else(|| CoreError::SessionNotFound(id.0.to_string()))?
            .1;
        self.event_bus
            .publish(
                kernel_eventbus::event_type::SESSION_CLOSED,
                kernel_eventbus::Durability::Durable,
                id.0.to_string(),
                String::new(),
                serde_json::json!({ "session_id": id.0.to_string() }),
            )
            .await?;
        Ok(())
    }

    /// 現在の Session 数を返す。
    #[must_use]
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Session が Capability `name` を呼び出せるか検査。
    /// # Errors
    /// 該当 Session が存在しない場合に [`CoreError::SessionNotFound`] を返す。
    pub fn check_capability(&self, session_id: &SessionId, name: &str) -> CoreResult<()> {
        let session = self.get(session_id)?;
        if session.permissions.capabilities.iter().any(|c| c == name) {
            Ok(())
        } else {
            Err(CoreError::AuthzDenied(format!(
                "session {} cannot call capability '{}'",
                session_id.0, name
            )))
        }
    }

    /// Workspace 変更可否を検査。
    /// # Errors
    /// 該当 Session が存在しない、または Workspace 変更禁止時にエラーを返す。
    pub fn check_workspace_mutation(&self, session_id: &SessionId) -> CoreResult<()> {
        let session = self.get(session_id)?;
        if session.permissions.allow_workspace_mutation {
            Ok(())
        } else {
            Err(CoreError::CommandPermissionDenied(format!(
                "session {} cannot mutate workspace",
                session_id.0
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel_eventbus::{EventBus, InMemoryDurableLog};

    fn setup() -> (Arc<EventBus>, SessionManager) {
        let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
        let mgr = SessionManager::new(bus.clone());
        (bus, mgr)
    }

    #[tokio::test]
    async fn create_and_get_session() {
        let (_bus, mgr) = setup();
        let s = mgr
            .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
            .await
            .unwrap();
        let got = mgr.get(&s.id).unwrap();
        assert_eq!(got.id, s.id);
        assert_eq!(got.actor, ActorKind::Human);
    }

    #[tokio::test]
    async fn get_unknown_session_errors() {
        let (_bus, mgr) = setup();
        let err = mgr.get(&SessionId::new()).unwrap_err();
        assert!(matches!(err, CoreError::SessionNotFound(_)));
    }

    #[tokio::test]
    async fn delete_emits_event() {
        let (bus, mgr) = setup();
        let s = mgr
            .create(ActorKind::Agent, PermissionSet::agent_default(), ResourceBudget::default())
            .await
            .unwrap();
        mgr.delete(&s.id).await.unwrap();
        // Replay で session.created + session.closed の 2 件が見える
        let replayed = bus.replay(1, None).await.unwrap();
        let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
        assert!(types.contains(&kernel_eventbus::event_type::SESSION_CREATED));
        assert!(types.contains(&kernel_eventbus::event_type::SESSION_CLOSED));
    }

    #[tokio::test]
    async fn check_capability_enforces_allowlist() {
        let (_bus, mgr) = setup();
        let s = mgr
            .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
            .await
            .unwrap();
        mgr.check_capability(&s.id, "buffer.read").unwrap();
        assert!(mgr.check_capability(&s.id, "buffer.banana").is_err());
    }

    #[tokio::test]
    async fn recent_buffers_is_capped_at_32() {
        let (_bus, mgr) = setup();
        let s = mgr
            .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
            .await
            .unwrap();
        for i in 0..40 {
            s.touch_buffer(format!("file{i}.rs"));
        }
        let recent = s.recent_buffers();
        assert_eq!(recent.len(), 32);
        assert_eq!(recent[0], "file39.rs");
    }
}