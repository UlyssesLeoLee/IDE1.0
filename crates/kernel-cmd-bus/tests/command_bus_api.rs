//! Integration Tests for `kernel-cmd-bus` — Command Bus public API surface
//!
//! TP ID: IT-CB-001..008 — DD-11 §3 (Integration Test 観点)
//! 検証対象: Command 受付 → Schema Validation → Session 解決 → Permission 検査
//!          → Capability 解決 → Provider 実行 → Event 発行 のパイプライン全体を
//!          公開 API 経由で検証する。
//!
//! 実行方法:
//!   cargo test -p kernel-cmd-bus --test command_bus_api

use async_trait::async_trait;
use kernel_capability_registry::{
    CapabilityManifest, CapabilityRegistry, CommandProvider, Priority, SideEffectLevel,
};
use kernel_cmd_bus::{CommandBuilder, CommandBus, CommandStatus};
use kernel_error::CoreResult;
use kernel_eventbus::{EventBus, InMemoryDurableLog};
use kernel_session::{ActorKind, PermissionSet, ResourceBudget, SessionManager, SessionId};
use std::sync::Arc;

struct EchoProvider;

#[async_trait]
impl CommandProvider for EchoProvider {
    fn name(&self) -> &'static str {
        "echo-it"
    }
    async fn execute(&self, args: serde_json::Value) -> CoreResult<serde_json::Value> {
        Ok(args)
    }
}

struct GreetProvider;

#[async_trait]
impl CommandProvider for GreetProvider {
    fn name(&self) -> &'static str {
        "greet-it"
    }
    async fn execute(&self, args: serde_json::Value) -> CoreResult<serde_json::Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("world")
            .to_string();
        Ok(serde_json::json!({"greeting": format!("hello, {name}!")}))
    }
}

async fn setup() -> (Arc<CommandBus>, SessionId) {
    let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
    let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
    let mgr = Arc::new(SessionManager::new(bus.clone()));

    reg.register(
        CapabilityManifest {
            capability_id: "echo-it".into(),
            description: "echo for IT".into(),
            version: "1.0".into(),
            side_effect_level: SideEffectLevel::ReadOnly,
        },
        Arc::new(EchoProvider),
        Priority::default(),
    )
    .await
    .unwrap();

    reg.register(
        CapabilityManifest {
            capability_id: "greet-it".into(),
            description: "greet for IT".into(),
            version: "1.0".into(),
            side_effect_level: SideEffectLevel::ReadOnly,
        },
        Arc::new(GreetProvider),
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
async fn it_cb_001_execute_returns_ok_for_registered_capability() {
    // IT-CB-001: 登録済み Capability の実行は Ok を返す。
    let (cb, sid) = setup().await;
    let cmd = CommandBuilder::new("echo-it", sid)
        .args(serde_json::json!({"text": "hi"}))
        .build();
    let result = cb.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::Ok);
    assert_eq!(result.data.unwrap()["text"], "hi");
}

#[tokio::test]
async fn it_cb_002_execute_unknown_capability_returns_failed() {
    // IT-CB-002: 未登録 Capability の実行は Failed + ERR-BIZ:004。
    let (cb, sid) = setup().await;
    let cmd = CommandBuilder::new("does.not.exist", sid).build();
    let result = cb.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::Failed);
    assert!(result
        .error
        .unwrap()
        .contains("ERR-BIZ:004"));
}

#[tokio::test]
async fn it_cb_003_execute_unknown_session_returns_core_error() {
    // IT-CB-003: 未知の Session ID での実行は CoreError::SessionNotFound を返す。
    let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
    let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
    let mgr = Arc::new(SessionManager::new(bus.clone()));
    let cb = CommandBus::new(reg, mgr, bus);
    let cmd = CommandBuilder::new("echo-it", SessionId::new()).build();
    let err = cb.execute(cmd).await.expect_err("must error");
    assert!(matches!(err, kernel_error::CoreError::SessionNotFound(_)));
}

#[tokio::test]
async fn it_cb_004_permission_denied_yields_permission_denied_status() {
    // IT-CB-004: Capability が権限外 (allow-list 外) の Session では PermissionDenied。
    let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
    let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
    let mgr = Arc::new(SessionManager::new(bus.clone()));
    reg.register(
        CapabilityManifest {
            capability_id: "secret-it".into(),
            description: "secret".into(),
            version: "1.0".into(),
            side_effect_level: SideEffectLevel::ReadOnly,
        },
        Arc::new(EchoProvider),
        Priority::default(),
    )
    .await
    .unwrap();

    // 空の allow-list を持つ Session を作る
    let mut perm = PermissionSet::agent_default();
    perm.capabilities.clear();
    perm.capabilities.push("greet-it".into()); // secret-it を持たない
    let s = mgr
        .create(ActorKind::Agent, perm, ResourceBudget::default())
        .await
        .unwrap();
    let cb = CommandBus::new(reg, mgr, bus);
    let cmd = CommandBuilder::new("secret-it", s.id.clone()).build();
    let result = cb.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::PermissionDenied);
}

#[tokio::test]
async fn it_cb_005_greet_provider_receives_arguments() {
    // IT-CB-005: Provider が引数を正しく受け取れる。
    let (cb, sid) = setup().await;
    let cmd = CommandBuilder::new("greet-it", sid)
        .args(serde_json::json!({"name": "minimax"}))
        .build();
    let result = cb.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::Ok);
    assert_eq!(result.data.unwrap()["greeting"], "hello, minimax!");
}

#[tokio::test]
async fn it_cb_006_command_id_is_unique_per_execution() {
    // IT-CB-006: 連続実行の各 Command は異なる ID を持つ。
    let (cb, sid) = setup().await;
    let mut ids = Vec::new();
    for _ in 0..5 {
        let cmd = CommandBuilder::new("echo-it", sid.clone()).build();
        ids.push(cmd.id);
    }
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "command ids must be unique");
}

#[tokio::test]
async fn it_cb_007_inflight_count_zero_between_executions() {
    // IT-CB-007: 同期実行が完走した直後、inflight は 0。
    let (cb, sid) = setup().await;
    let cmd = CommandBuilder::new("echo-it", sid).build();
    cb.execute(cmd).await.unwrap();
    assert_eq!(cb.inflight_count(), 0);
}

#[tokio::test]
async fn it_cb_008_command_completed_event_is_published() {
    // IT-CB-008: 成功実行後、command.completed Durable Event が発行される。
    let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
    let reg = Arc::new(CapabilityRegistry::new(bus.clone()));
    let mgr = Arc::new(SessionManager::new(bus.clone()));
    reg.register(
        CapabilityManifest {
            capability_id: "echo-it".into(),
            description: "echo".into(),
            version: "1.0".into(),
            side_effect_level: SideEffectLevel::ReadOnly,
        },
        Arc::new(EchoProvider),
        Priority::default(),
    )
    .await
    .unwrap();
    let s = mgr
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let cb = CommandBus::new(reg, mgr, bus.clone());
    let cmd = CommandBuilder::new("echo-it", s.id.clone()).build();
    cb.execute(cmd).await.unwrap();

    let replayed = bus.replay(1, None).await.unwrap();
    let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&kernel_eventbus::event_type::COMMAND_COMPLETED));
    assert!(types.contains(&kernel_eventbus::event_type::COMMAND_STARTED));
}