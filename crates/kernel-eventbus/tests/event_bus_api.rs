//! Integration Tests for `kernel-eventbus` — Event Bus public API surface
//!
//! TP ID: IT-EB-001..008 — DD-11 §3 (Integration Test 観点)
//! 検証対象: 公開 API に対する外部使用者視点での Event 配信 / 永続化 / 購読確認
//!
//! 実行方法:
//!   cargo test -p kernel-eventbus --test event_bus_api

use kernel_eventbus::{
    Durability, EventBus, EventEnvelope, InMemoryDurableLog, SubscriptionId, Subscriber,
};
use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
use std::sync::Arc;
use std::time::Duration;

fn make_bus() -> EventBus {
    EventBus::new(Arc::new(InMemoryDurableLog::new()))
}

struct CountingSub {
    id: SubscriptionId,
    count: Arc<AtomicUsize>,
    accepted_type: &'static str,
}

#[async_trait::async_trait]
impl Subscriber for CountingSub {
    fn subscription_id(&self) -> SubscriptionId {
        self.id
    }
    async fn on_event(&self, envelope: &EventEnvelope) -> bool {
        if envelope.event_type == self.accepted_type {
            self.count.fetch_add(1, AOrdering::SeqCst);
        }
        true
    }
}

#[tokio::test]
async fn it_eb_001_publish_assigns_monotonic_sequence() {
    // IT-EB-001: 連続 publish で sequence が単調増加する。
    let bus = make_bus();
    let e1 = bus
        .publish("evt.a", Durability::Durable, "C", "T", serde_json::json!({}))
        .await
        .unwrap();
    let e2 = bus
        .publish("evt.a", Durability::Durable, "C", "T", serde_json::json!({}))
        .await
        .unwrap();
    assert!(e2.sequence > e1.sequence, "sequence must be monotonic");
}

#[tokio::test]
async fn it_eb_002_durable_event_persists_in_log() {
    // IT-EB-002: Durable Event は Replay で取得できる。
    let bus = make_bus();
    bus.publish("evt.x", Durability::Durable, "C1", "T1", serde_json::json!({"k": "v"}))
        .await
        .unwrap();
    let replayed = bus.replay(1, None).await.unwrap();
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].payload["k"], "v");
}

#[tokio::test]
async fn it_eb_003_transient_event_does_not_persist() {
    // IT-EB-003: Transient Event は Replay に出ない。
    let bus = make_bus();
    bus.publish("evt.t", Durability::Transient, "C1", "T1", serde_json::json!({}))
        .await
        .unwrap();
    let replayed = bus.replay(1, None).await.unwrap();
    assert!(replayed.is_empty());
}

#[tokio::test]
async fn it_eb_004_replay_range_filters_by_sequence() {
    // IT-EB-004: replay(from, to) で sequence 範囲フィルタが効く。
    let bus = make_bus();
    for i in 0..5 {
        bus.publish(
            "evt.x",
            Durability::Durable,
            "C",
            "T",
            serde_json::json!({"i": i}),
        )
        .await
        .unwrap();
    }
    let cur = bus.current_sequence();
    let replayed = bus.replay(2, Some(cur)).await.unwrap();
    // from=2 から cur(=5) まで: 4 件 (seq 2,3,4,5) のはず
    assert_eq!(replayed.len(), 4);
    assert_eq!(replayed[0].sequence, 2);
    assert_eq!(replayed.last().unwrap().sequence, cur);
}

#[tokio::test]
async fn it_eb_005_subscriber_receives_published_event() {
    // IT-EB-005: Subscriber は publish されたイベントを受信する。
    let bus = make_bus();
    let count = Arc::new(AtomicUsize::new(0));
    let sub: Arc<dyn Subscriber> = Arc::new(CountingSub {
        id: SubscriptionId::new(),
        count: count.clone(),
        accepted_type: "evt.z",
    });
    bus.subscribe(sub);
    bus.publish("evt.z", Durability::Durable, "C", "T", serde_json::json!({}))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(count.load(AOrdering::SeqCst), 1);
}

#[tokio::test]
async fn it_eb_006_unsubscribe_stops_delivery() {
    // IT-EB-006: unsubscribe で該当 Subscriber への配信が止まる。
    let bus = make_bus();
    let count = Arc::new(AtomicUsize::new(0));
    let sub: Arc<dyn Subscriber> = Arc::new(CountingSub {
        id: SubscriptionId::new(),
        count: count.clone(),
        accepted_type: "evt.z",
    });
    let id = bus.subscribe(sub);
    bus.publish("evt.z", Durability::Durable, "C", "T", serde_json::json!({}))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(bus.unsubscribe(id));
    bus.publish("evt.z", Durability::Durable, "C", "T", serde_json::json!({}))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    // unsubscribe 後の publish はカウントされない → 1 のまま
    assert_eq!(count.load(AOrdering::SeqCst), 1);
}

#[tokio::test]
async fn it_eb_007_subscriber_count_reflects_state() {
    // IT-EB-007: subscriber_count() は subscribe / unsubscribe を反映する。
    let bus = make_bus();
    assert_eq!(bus.subscriber_count(), 0);
    let count = Arc::new(AtomicUsize::new(0));
    let sub: Arc<dyn Subscriber> = Arc::new(CountingSub {
        id: SubscriptionId::new(),
        count,
        accepted_type: "x",
    });
    let id = bus.subscribe(sub);
    assert_eq!(bus.subscriber_count(), 1);
    bus.unsubscribe(id);
    assert_eq!(bus.subscriber_count(), 0);
}

#[tokio::test]
async fn it_eb_008_event_envelope_carries_correlation_and_trace() {
    // IT-EB-008: publish 時に渡した correlation_id / trace_id が envelope に乗る。
    let bus = make_bus();
    let e = bus
        .publish("evt.t", Durability::Durable, "CORR-1", "TRACE-1", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(e.correlation_id, "CORR-1");
    assert_eq!(e.trace_id, "TRACE-1");
}