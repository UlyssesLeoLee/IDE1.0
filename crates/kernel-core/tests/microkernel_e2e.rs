//! Integration Tests for `kernel-core` — Microkernel end-to-end public API surface
//!
//! TP ID: IT-CORE-001..006 — DD-11 §3 (Integration Test 観点)
//! 検証対象: Microkernel 集約ハンドル (EventBus + SessionManager + CapabilityRegistry
//!          + CommandBus) を一体として使った E2E シナリオ。
//!
//! 実行方法:
//!   cargo test -p kernel-core --test microkernel_e2e

use kernel_capability_registry::{CapabilityManifest, CommandProvider, Priority, SideEffectLevel};
use kernel_cmd_bus::{CommandBuilder, CommandStatus};
use kernel_core::Microkernel;
use kernel_error::CoreResult;
use kernel_session::{ActorKind, PermissionSet, ResourceBudget};
use std::sync::Arc;

struct FailingProvider;

#[async_trait::async_trait]
impl CommandProvider for FailingProvider {
    fn name(&self) -> &'static str {
        "fail-intentionally"
    }
    async fn execute(&self, _args: serde_json::Value) -> CoreResult<serde_json::Value> {
        Err(kernel_error::CoreError::Internal("forced failure".into()))
    }
}

#[tokio::test]
async fn it_core_001_echo_e2e_happy_path() {
    // IT-CORE-001: Microkernel::new + register_echo_capability + 1 Command 実行で Ok + Event 発行。
    let kernel = Microkernel::new();
    kernel.register_echo_capability().await.unwrap();
    let session = kernel
        .session_manager
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let cmd = CommandBuilder::new("echo", session.id.clone())
        .args(serde_json::json!({"text": "hello"}))
        .build();
    let result = kernel.command_bus.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::Ok);
    assert_eq!(result.data.unwrap()["text"], "hello");

    let replayed = kernel.event_bus.replay(1, None).await.unwrap();
    let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&kernel_eventbus::event_type::COMMAND_STARTED));
    assert!(types.contains(&kernel_eventbus::event_type::COMMAND_COMPLETED));
}

#[tokio::test]
async fn it_core_002_multiple_sessions_isolation() {
    // IT-CORE-002: 複数 Session は互いに独立して動作する。
    let kernel = Microkernel::new();
    kernel.register_echo_capability().await.unwrap();

    let s1 = kernel
        .session_manager
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let s2 = kernel
        .session_manager
        .create(ActorKind::Agent, PermissionSet::agent_default(), ResourceBudget::default())
        .await
        .unwrap();

    let cmd1 = CommandBuilder::new("echo", s1.id.clone())
        .args(serde_json::json!({"who": "s1"}))
        .build();
    let cmd2 = CommandBuilder::new("echo", s2.id.clone())
        .args(serde_json::json!({"who": "s2"}))
        .build();

    let r1 = kernel.command_bus.execute(cmd1).await.unwrap();
    let r2 = kernel.command_bus.execute(cmd2).await.unwrap();
    assert_eq!(r1.status, CommandStatus::Ok);
    assert_eq!(r2.status, CommandStatus::Ok);
    assert_eq!(r1.data.unwrap()["who"], "s1");
    assert_eq!(r2.data.unwrap()["who"], "s2");
}

#[tokio::test]
async fn it_core_003_failing_provider_yields_command_failed_event() {
    // IT-CORE-003: Provider が失敗を返すと command.failed Durable Event が発行される。
    use kernel_capability_registry::CapabilityRegistry;
    let kernel = Microkernel::new();
    // kernel-core の CapabilityRegistry にアクセスして独自 Provider を登録
    let reg: Arc<CapabilityRegistry> = kernel.capability_registry.clone();
    reg.register(
        CapabilityManifest {
            capability_id: "fail-intentionally".into(),
            description: "always fail".into(),
            version: "1.0".into(),
            side_effect_level: SideEffectLevel::ReadOnly,
        },
        Arc::new(FailingProvider),
        Priority::default(),
    )
    .await
    .unwrap();
    let session = kernel
        .session_manager
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let cmd = CommandBuilder::new("fail-intentionally", session.id.clone()).build();
    let result = kernel.command_bus.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::Failed);

    let replayed = kernel.event_bus.replay(1, None).await.unwrap();
    let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&kernel_eventbus::event_type::COMMAND_FAILED));
}

#[tokio::test]
async fn it_core_004_capability_registry_list_reflects_registration() {
    // IT-CORE-004: CapabilityRegistry::list() は登録 Capability を列挙する。
    let kernel = Microkernel::new();
    kernel.register_echo_capability().await.unwrap();
    let listed = kernel.capability_registry.list();
    let names: Vec<&str> = listed.iter().map(|m| m.capability_id.as_str()).collect();
    assert!(names.contains(&"echo"));
}

#[tokio::test]
async fn it_core_005_event_bus_replay_returns_in_order() {
    // IT-CORE-005: Replay は sequence 昇順で取得できる。
    let kernel = Microkernel::new();
    kernel.register_echo_capability().await.unwrap();
    let s = kernel
        .session_manager
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    for i in 0..3 {
        let cmd = CommandBuilder::new("echo", s.id.clone())
            .args(serde_json::json!({"i": i}))
            .build();
        kernel.command_bus.execute(cmd).await.unwrap();
    }
    let replayed = kernel.event_bus.replay(1, None).await.unwrap();
    assert!(!replayed.is_empty());
    for w in replayed.windows(2) {
        assert!(w[0].sequence < w[1].sequence);
    }
}

#[tokio::test]
async fn it_core_006_clone_handle_points_to_same_instance() {
    // IT-CORE-006: Microkernel の Clone は Arc を共有する (内部状態が一致)。
    let kernel = Microkernel::new();
    let cloned = kernel.clone();
    kernel.register_echo_capability().await.unwrap();

    let s = cloned
        .session_manager
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let cmd = CommandBuilder::new("echo", s.id.clone())
        .args(serde_json::json!({"text": "shared"}))
        .build();
    let result = cloned.command_bus.execute(cmd).await.unwrap();
    assert_eq!(result.status, CommandStatus::Ok);
    assert_eq!(result.data.unwrap()["text"], "shared");
}