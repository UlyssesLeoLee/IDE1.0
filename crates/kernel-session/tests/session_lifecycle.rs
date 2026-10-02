//! Integration Tests for `kernel-session` — Session Manager public API surface
//!
//! TP ID: IT-SM-001..010 — DD-11 §3 (Integration Test 観点)
//! 検証対象: 公開 API に対する外部使用者視点での Session ライフサイクル確認
//! (クレート内 `#[cfg(test)]` のホワイトボックステストとは別レイヤ)
//!
//! 実行方法:
//!   cargo test -p kernel-session --test session_lifecycle
//! または scripts/test/it.sh

use kernel_eventbus::{EventBus, InMemoryDurableLog};
use kernel_session::{ActorKind, PermissionSet, ResourceBudget, SessionManager, SessionId};
use std::sync::Arc;

fn make_manager() -> (Arc<EventBus>, SessionManager) {
    let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
    let mgr = SessionManager::new(bus.clone());
    (bus, mgr)
}

#[tokio::test]
async fn it_sm_001_create_session_emits_event() {
    // IT-SM-001: 新規 Session 作成で `session.created` Durable Event が
    // Event Bus 経由で永続化されることを確認する。
    let (bus, mgr) = make_manager();
    let _s = mgr
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .expect("create must succeed");

    let replayed = bus.replay(1, None).await.expect("replay must succeed");
    let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
    assert!(
        types.contains(&kernel_eventbus::event_type::SESSION_CREATED),
        "session.created event must be in replayed log, got: {types:?}"
    );
}

#[tokio::test]
async fn it_sm_002_session_id_is_unique() {
    // IT-SM-002: 連続作成した Session の ID が一意であること。
    let (_bus, mgr) = make_manager();
    let mut ids = Vec::new();
    for _ in 0..10 {
        let s = mgr
            .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
            .await
            .unwrap();
        ids.push(s.id.clone());
    }
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "session ids must be unique");
}

#[tokio::test]
async fn it_sm_003_get_unknown_session_returns_errbiz003() {
    // IT-SM-003: 存在しない Session ID で get した場合 `CoreError::SessionNotFound`
    // (ERR-BIZ:003) を返すことを確認する。
    let (_bus, mgr) = make_manager();
    let unknown = SessionId::new();
    let err = mgr.get(&unknown).expect_err("unknown session must error");
    let msg = format!("{err}");
    assert!(msg.contains("ERR-BIZ:003"), "must surface ERR-BIZ:003, got: {msg}");
}

#[tokio::test]
async fn it_sm_004_delete_emits_session_closed() {
    // IT-SM-004: Session 削除で `session.closed` Durable Event が
    // 発行されることを確認する。
    let (bus, mgr) = make_manager();
    let s = mgr
        .create(ActorKind::Agent, PermissionSet::agent_default(), ResourceBudget::default())
        .await
        .unwrap();
    mgr.delete(&s.id).await.unwrap();
    let replayed = bus.replay(1, None).await.unwrap();
    let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&kernel_eventbus::event_type::SESSION_CLOSED));
}

#[tokio::test]
async fn it_sm_005_permission_check_human_vs_agent() {
    // IT-SM-005: Human Editor は buffer.write 可能 / Agent 既定は buffer.write 不許可。
    let (_bus, mgr) = make_manager();
    let human = mgr
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let agent = mgr
        .create(ActorKind::Agent, PermissionSet::agent_default(), ResourceBudget::default())
        .await
        .unwrap();

    mgr.check_capability(&human.id, "buffer.write")
        .expect("human editor must allow buffer.write");
    assert!(mgr.check_capability(&agent.id, "buffer.write").is_err(),
        "agent_default must not allow buffer.write");
}

#[tokio::test]
async fn it_sm_006_recent_buffers_respects_cap_32() {
    // IT-SM-006: `touch_buffer` の履歴は 32 件で打ち止め (古い側が drop)。
    let (_bus, mgr) = make_manager();
    let s = mgr
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    for i in 0..50 {
        s.touch_buffer(format!("file{i}.rs"));
    }
    let recent = s.recent_buffers();
    assert_eq!(recent.len(), 32);
    // 新しい順 = 直近 touch された file49.rs が先頭
    assert_eq!(recent[0], "file49.rs");
}

#[tokio::test]
async fn it_sm_007_resource_budget_default_is_reasonable() {
    // IT-SM-007: ResourceBudget::default() が MVP-1 で期待される範囲。
    let b = ResourceBudget::default();
    assert!(b.max_tokens_per_minute >= 10_000);
    assert!(b.max_commands_per_session >= 1_000);
    assert!(b.max_concurrent_commands >= 1);
}

#[tokio::test]
async fn it_sm_008_actor_kind_serde_roundtrip() {
    // IT-SM-008: ActorKind の serde round-trip (CLI 経由で JSON 交換されるため必須)。
    for ak in [
        ActorKind::Human,
        ActorKind::LangGraph,
        ActorKind::Agent,
        ActorKind::Ci,
        ActorKind::Plugin,
        ActorKind::Automation,
    ] {
        let json = serde_json::to_string(&ak).unwrap();
        let back: ActorKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ak, "round-trip failed for {ak:?}");
    }
}

#[tokio::test]
async fn it_sm_009_session_count_reflects_create_and_delete() {
    // IT-SM-009: SessionManager の count は作成・削除を反映する。
    let (_bus, mgr) = make_manager();
    assert_eq!(mgr.count(), 0);
    let s1 = mgr
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let _s2 = mgr
        .create(ActorKind::Agent, PermissionSet::agent_default(), ResourceBudget::default())
        .await
        .unwrap();
    assert_eq!(mgr.count(), 2);
    mgr.delete(&s1.id).await.unwrap();
    assert_eq!(mgr.count(), 1);
}

#[tokio::test]
async fn it_sm_010_correlation_id_propagates_to_events() {
    // IT-SM-010: Session ID が Event の correlation_id として伝搬する。
    let (bus, mgr) = make_manager();
    let s = mgr
        .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
        .await
        .unwrap();
    let sid_str = s.id.0.to_string();
    let replayed = bus.replay(1, None).await.unwrap();
    assert!(
        replayed.iter().any(|e| e.correlation_id == sid_str),
        "session id must propagate as correlation_id"
    );
}