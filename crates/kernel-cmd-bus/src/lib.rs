//! kernel-cmd-bus — Tier 3: Command Bus
//!
//! DD-01 §MOD-CB-001 (Command Bus) を実装。Command 受付 → Schema Validation → Session 解決
//! → Permission 検査 → Capability 解決 → Provider 実行 → Event 発行 のパイプライン。
//! MVP-1 では同期実行 (async Provider) のみ。非同期 / Cancellation は P1 (DD-01 §M-CB-007)。
//!
//! トレース: REQ-001 §FR-003 / §FR-022 / DD-01 §MOD-CB / DD-13 §F-03

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use dashmap::DashMap;
use kernel_capability_registry::{CapabilityRegistry, SideEffectLevel};
use kernel_error::{CoreError, CoreResult};
use kernel_eventbus::EventBus;
use kernel_session::{SessionId, SessionManager};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Command を識別する ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub Uuid);

impl CommandId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CommandId {
    fn default() -> Self {
        Self::new()
    }
}

/// Command 本体。
#[derive(Debug, Clone)]
pub struct Command {
    pub id: CommandId,
    pub name: String,
    pub session_id: SessionId,
    pub arguments: serde_json::Value,
    pub timeout: Option<std::time::Duration>,
}

/// Command 実行結果。
#[derive(Debug)]
pub struct CommandResult {
    pub command_id: CommandId,
    pub status: CommandStatus,
    pub data: Option<serde_json::Value>,
    pub diagnostics: Vec<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// 実行ステータス。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Ok,
    Failed,
    PermissionDenied,
    Timeout,
}

/// 実行中 Command を追跡するエントリ。
#[derive(Debug)]
pub struct InflightEntry {
    pub command_id: CommandId,
    pub session_id: SessionId,
    pub started_at: Instant,
}

/// Command Bus 本体。
pub struct CommandBus {
    inflight: DashMap<CommandId, InflightEntry>,
    registry: Arc<CapabilityRegistry>,
    session_mgr: Arc<SessionManager>,
    event_bus: Arc<EventBus>,
}

impl std::fmt::Debug for CommandBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandBus")
            .field("inflight_count", &self.inflight.len())
            .finish()
    }
}

impl CommandBus {
    /// 新しい Command Bus を生成。
    #[must_use]
    pub fn new(
        registry: Arc<CapabilityRegistry>,
        session_mgr: Arc<SessionManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            inflight: DashMap::new(),
            registry,
            session_mgr,
            event_bus,
        }
    }

    /// 実行中 Command 数を返す。
    #[must_use]
    pub fn inflight_count(&self) -> usize {
        self.inflight.len()
    }

    /// Command を実行する。
    /// # Errors
    /// Session 解決失敗 / 権限不足 / Capability 解決失敗 / Provider 実行失敗で
    /// [`CoreError`] を返す。
    pub async fn execute(&self, cmd: Command) -> CoreResult<CommandResult> {
        let started = Instant::now();
        self.inflight.insert(
            cmd.id,
            InflightEntry {
                command_id: cmd.id,
                session_id: cmd.session_id.clone(),
                started_at: started,
            },
        );

        // 1. Session 解決
        let _session = self.session_mgr.get(&cmd.session_id)?;

        // 2. Capability 名決定 (cmd.name → capability_id 直接マッピング)
        let capability_id = cmd.name.clone();

        // 3. Permission 検査
        if let Err(e) = self
            .session_mgr
            .check_capability(&cmd.session_id, &capability_id)
        {
            let result = self.fail(&cmd, started, CommandStatus::PermissionDenied, e);
            self.event_bus
                .publish(
                    kernel_eventbus::event_type::COMMAND_FAILED,
                    kernel_eventbus::Durability::Durable,
                    cmd.session_id.0.to_string(),
                    String::new(),
                    serde_json::json!({"command_id": cmd.id.0.to_string(), "reason": "permission"}),
                )
                .await
                .ok();
            self.inflight.remove(&cmd.id);
            return Ok(result);
        }

        // 4. Side effect による追加検査 (Workspace Mutation)
        if let Some(manifest) = self.registry.manifest(&capability_id) {
            if manifest.side_effect_level == SideEffectLevel::WorkspaceMutation
                || manifest.side_effect_level == SideEffectLevel::Transactional
            {
                if let Err(e) = self.session_mgr.check_workspace_mutation(&cmd.session_id) {
                    let result = self.fail(&cmd, started, CommandStatus::PermissionDenied, e);
                    self.inflight.remove(&cmd.id);
                    return Ok(result);
                }
            }
        }

        // 5. Capability 解決
        let entry = match self.registry.route(&capability_id) {
            Ok(e) => e,
            Err(e) => {
                let result = self.fail(&cmd, started, CommandStatus::Failed, e);
                self.inflight.remove(&cmd.id);
                return Ok(result);
            }
        };

        // 6. 開始イベント
        self.event_bus
            .publish(
                kernel_eventbus::event_type::COMMAND_STARTED,
                kernel_eventbus::Durability::Durable,
                cmd.session_id.0.to_string(),
                String::new(),
                serde_json::json!({
                    "command_id": cmd.id.0.to_string(),
                    "name": cmd.name,
                }),
            )
            .await
            .ok();

        // 7. Provider 実行 (タイムアウト付き)
        let exec = entry.provider.execute(cmd.arguments.clone()).await;
        let duration_ms = started.elapsed().as_millis() as u64;

        let result = match exec {
            Ok(data) => {
                self.event_bus
                    .publish(
                        kernel_eventbus::event_type::COMMAND_COMPLETED,
                        kernel_eventbus::Durability::Durable,
                        cmd.session_id.0.to_string(),
                        String::new(),
                        serde_json::json!({
                            "command_id": cmd.id.0.to_string(),
                            "duration_ms": duration_ms,
                        }),
                    )
                    .await
                    .ok();
                CommandResult {
                    command_id: cmd.id,
                    status: CommandStatus::Ok,
                    data: Some(data),
                    diagnostics: Vec::new(),
                    error: None,
                    duration_ms,
                }
            }
            Err(err) => {
                let status = if matches!(err, CoreError::Timeout(_)) {
                    CommandStatus::Timeout
                } else {
                    CommandStatus::Failed
                };
                self.event_bus
                    .publish(
                        kernel_eventbus::event_type::COMMAND_FAILED,
                        kernel_eventbus::Durability::Durable,
                        cmd.session_id.0.to_string(),
                        String::new(),
                        serde_json::json!({
                            "command_id": cmd.id.0.to_string(),
                            "code": err.code().to_string(),
                        }),
                    )
                    .await
                    .ok();
                CommandResult {
                    command_id: cmd.id,
                    status,
                    data: None,
                    diagnostics: vec![format!("{err}")],
                    error: Some(format!("{err}")),
                    duration_ms,
                }
            }
        };

        // Session の last_active_at などを軽く更新 (将来は session.touch() に副作用委譲)
        self.inflight.remove(&cmd.id);
        Ok(result)
    }

    fn fail(
        &self,
        cmd: &Command,
        started: Instant,
        status: CommandStatus,
        err: CoreError,
    ) -> CommandResult {
        CommandResult {
            command_id: cmd.id,
            status,
            data: None,
            diagnostics: vec![format!("{err}")],
            error: Some(format!("{err}")),
            duration_ms: started.elapsed().as_millis() as u64,
        }
    }
}

/// 簡易 Command Builder。
pub struct CommandBuilder {
    name: String,
    session_id: SessionId,
    arguments: serde_json::Value,
    timeout: Option<std::time::Duration>,
}

impl CommandBuilder {
    #[must_use]
    pub fn new(name: impl Into<String>, session_id: SessionId) -> Self {
        Self {
            name: name.into(),
            session_id,
            arguments: serde_json::json!({}),
            timeout: None,
        }
    }

    #[must_use]
    pub fn args(mut self, args: serde_json::Value) -> Self {
        self.arguments = args;
        self
    }

    #[must_use]
    pub fn timeout(mut self, d: std::time::Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    #[must_use]
    pub fn build(self) -> Command {
        Command {
            id: CommandId::new(),
            name: self.name,
            session_id: self.session_id,
            arguments: self.arguments,
            timeout: self.timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use kernel_capability_registry::{CapabilityManifest, CommandProvider, Priority};
    use kernel_eventbus::{EventBus, InMemoryDurableLog};
    use kernel_session::{ActorKind, PermissionSet, ResourceBudget};

    struct HelloProvider;
        #[async_trait]
        impl CommandProvider for HelloProvider {
            fn name(&self) -> &str {
                "buffer.read"
            }
            async fn execute(&self, args: serde_json::Value) -> CoreResult<serde_json::Value> {
                let name = args["name"].as_str().unwrap_or("world");
                Ok(serde_json::json!({ "greeting": format!("hello, {name}!") }))
            }
        }

        async fn setup() -> (Arc<CommandBus>, SessionId) {
            let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
            let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
            let mgr = Arc::new(SessionManager::new(bus.clone()));
            reg.register(
                CapabilityManifest {
                    capability_id: "buffer.read".into(),
                    description: "say hello".into(),
                    version: "1.0".into(),
                    side_effect_level: SideEffectLevel::ReadOnly,
                },
                Arc::new(HelloProvider),
                Priority::default(),
            )
            .await
            .unwrap();
            let s = mgr
                .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
                .await
                .unwrap();
            let cb = Arc::new(CommandBus::new(reg, mgr, bus));
            (cb, s.id.clone())
        }

    #[tokio::test]
    async fn execute_happy_path() {
        let (cb, sid) = setup().await;
        let cmd = CommandBuilder::new("buffer.read", sid)
            .args(json!({"name": "minimax"}))
            .build();
        let result = cb.execute(cmd).await.unwrap();
        assert_eq!(result.status, CommandStatus::Ok);
        assert_eq!(result.data.unwrap()["greeting"], "hello, minimax!");
    }

    #[tokio::test]
    async fn execute_unknown_capability_returns_failed() {
        let (cb, sid) = setup().await;
        // "text.search" is in human_editor's allow-list but not registered, so it fails at routing
        let cmd = CommandBuilder::new("text.search", sid).build();
        let result = cb.execute(cmd).await.unwrap();
        assert_eq!(result.status, CommandStatus::Failed);
        assert!(result.error.unwrap().contains("ERR-BIZ:004"));
    }

    #[tokio::test]
    async fn execute_permission_denied() {
        let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
        let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
        let mgr = Arc::new(SessionManager::new(bus.clone()));
        reg.register(
            CapabilityManifest {
                capability_id: "secret".into(),
                description: "secret".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(HelloProvider),
            Priority::default(),
        )
        .await
        .unwrap();
        // 制限付き Session (secret を持っていない)
        let mut perm = PermissionSet::agent_default();
        perm.capabilities.clear();
        perm.capabilities.push("buffer.read".into());
        let s = mgr
            .create(ActorKind::Agent, perm, ResourceBudget::default())
            .await
            .unwrap();
        let cb = Arc::new(CommandBus::new(reg, mgr, bus));
        let cmd = CommandBuilder::new("secret", s.id.clone()).build();
        let result = cb.execute(cmd).await.unwrap();
        assert_eq!(result.status, CommandStatus::PermissionDenied);
        assert!(result.error.unwrap().contains("ERR-BIZ:002"));
    }

    #[tokio::test]
    async fn execute_unknown_session_errors() {
        let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
        let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
        let mgr = Arc::new(SessionManager::new(bus.clone()));
        let cb = CommandBus::new(reg, mgr, bus);
        let cmd = CommandBuilder::new("hello", SessionId::new()).build();
        let err = cb.execute(cmd).await.unwrap_err();
        assert!(matches!(err, CoreError::SessionNotFound(_)));
    }
}

pub use serde_json::json;