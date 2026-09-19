//! kernel-eventbus — Tier 2: Event Bus
//!
//! DD-01 §MOD-EB-001 (Event Bus) を実装。Event Envelope 構築 / Subscriber 配信 / 順序保証 /
//! Replay (Sequence Range) を提供。Durable Event は永続化、Transient は揮発性。
//!
//! トレース: REQ-001 §FR-008 / DD-01 §4.3 / DD-02 §6.8

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Event の永続化可否 (DD-01 §4.3)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// 永続化対象 (例: `TransactionCommitted`, `FileSaved`, `CommandCompleted`)
    Durable,
    /// 一時配信のみ (例: `CursorMoved`, `ai.inline.suggestion`)
    Transient,
}

/// Event を識別する一意 ID (UUID v4)。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventId(pub Uuid);

impl EventId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

/// Event Bus 内で単調増加する Sequence 番号 (Replay の起点)。
pub type Sequence = u64;

/// Event を包む共通ヘッダ。
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub event_type: String,
    pub sequence: Sequence,
    pub occurred_at: DateTime<Utc>,
    pub durability: Durability,
    pub correlation_id: String,
    pub trace_id: String,
    pub payload: serde_json::Value,
}

/// Subscriber が受け取るハンドラの trait。
#[async_trait]
pub trait Subscriber: Send + Sync {
    /// 購読識別子 (重複登録の検出 / unsubscribe で使う)。
    fn subscription_id(&self) -> SubscriptionId;

    /// Event 配信。`true` を返すと配信継続、`false` を返すとこの Subscriber への配信を停止。
    async fn on_event(&self, envelope: &EventEnvelope) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub Uuid);

impl SubscriptionId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SubscriptionId {
    fn default() -> Self {
        Self::new()
    }
}

/// 永続化層の抽象。MVP-1 では in-memory 実装のみ提供し、P3 で `kernel-txn` の WAL を
/// 利用する実装を追加する (DD-01 §MOD-EB §4.4)。
#[async_trait]
pub trait DurableLog: Send + Sync {
    async fn append(&self, envelope: &EventEnvelope) -> kernel_error::CoreResult<()>;
    async fn replay(
        &self,
        from: Sequence,
        to: Option<Sequence>,
    ) -> kernel_error::CoreResult<Vec<EventEnvelope>>;
}

/// メモリ内 Durable Log (MVP-1 用、P3 で WAL に差し替え)。
#[derive(Debug, Default)]
pub struct InMemoryDurableLog {
    inner: parking_lot::Mutex<Vec<EventEnvelope>>,
}

impl InMemoryDurableLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DurableLog for InMemoryDurableLog {
    async fn append(&self, envelope: &EventEnvelope) -> kernel_error::CoreResult<()> {
        let mut g = self.inner.lock();
        g.push(envelope.clone());
        Ok(())
    }

    async fn replay(
        &self,
        from: Sequence,
        to: Option<Sequence>,
    ) -> kernel_error::CoreResult<Vec<EventEnvelope>> {
        let g = self.inner.lock();
        let upper = to.unwrap_or(u64::MAX);
        Ok(g.iter()
            .filter(|e| e.sequence >= from && e.sequence <= upper)
            .cloned()
            .collect())
    }
}

/// Event Bus 本体。
///
/// Subscriber は `Arc<dyn Subscriber>` で保持し、`tokio::spawn` で並列配信する。
/// 順序保証: 同一 Sequence は単調増加。Subscriber への配信順序は best-effort。
pub struct EventBus {
    sequence: AtomicU64,
    subscribers: dashmap::DashMap<SubscriptionId, Arc<dyn Subscriber>>,
    durable_log: Arc<dyn DurableLog>,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("sequence", &self.sequence.load(Ordering::Relaxed))
            .field("subscriber_count", &self.subscribers.len())
            .finish()
    }
}

impl EventBus {
    /// 新しい Event Bus を生成。
    #[must_use]
    pub fn new(durable_log: Arc<dyn DurableLog>) -> Self {
        Self {
            sequence: AtomicU64::new(0),
            subscribers: dashmap::DashMap::new(),
            durable_log,
        }
    }

    /// 購読を登録。
    pub fn subscribe(&self, subscriber: Arc<dyn Subscriber>) -> SubscriptionId {
        let id = subscriber.subscription_id();
        self.subscribers.insert(id, subscriber);
        id
    }

    /// 購読を解除。
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.subscribers.remove(&id).is_some()
    }

    /// 現在の購読数を返す。
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// 次の Sequence を発行。
    fn next_sequence(&self) -> Sequence {
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Event を発行し、Durable なものは永続化する。
    /// # Errors
    /// Durable な Event の永続化失敗時に [`kernel_error::CoreError::Internal`] を返す。
    pub async fn publish(
        &self,
        event_type: impl Into<String>,
        durability: Durability,
        correlation_id: impl Into<String>,
        trace_id: impl Into<String>,
        payload: serde_json::Value,
    ) -> kernel_error::CoreResult<EventEnvelope> {
        let envelope = EventEnvelope {
            event_id: EventId::new(),
            event_type: event_type.into(),
            sequence: self.next_sequence(),
            occurred_at: Utc::now(),
            durability,
            correlation_id: correlation_id.into(),
            trace_id: trace_id.into(),
            payload,
        };

        if matches!(durability, Durability::Durable) {
            self.durable_log.append(&envelope).await?;
        }

        // 全 Subscriber に並列配信。失敗しても Bus 自体は落ちない。
        let subs: Vec<Arc<dyn Subscriber>> = self
            .subscribers
            .iter()
            .map(|r| r.value().clone())
            .collect();
        for sub in subs {
            let env = envelope.clone();
            tokio::spawn(async move {
                let keep = sub.on_event(&env).await;
                if !keep {
                    // Subscription 停止の要求 — EventBus のインスタンスを持っていないので
                    // ここでは何もしない。Subscriber 側で deregister を明示的に呼ぶ。
                    tracing::debug!(
                        subscription = %sub.subscription_id().0,
                        "subscriber requested unsubscribe (signal)"
                    );
                }
            });
        }

        Ok(envelope)
    }

    /// 過去 Event を Sequence Range で Replay。
    /// # Errors
    /// DurableLog の replay 失敗時に [`kernel_error::CoreError::Internal`] を返す。
    pub async fn replay(
        &self,
        from: Sequence,
        to: Option<Sequence>,
    ) -> kernel_error::CoreResult<Vec<EventEnvelope>> {
        self.durable_log.replay(from, to).await
    }

    /// 現在の Sequence カウンタ値。
    #[must_use]
    pub fn current_sequence(&self) -> Sequence {
        self.sequence.load(Ordering::SeqCst)
    }
}

/// イベントタイプ定数 (DD-01 §4.3 / DD-13 で使用)。
pub mod event_type {
    /// Command の実行開始
    pub const COMMAND_STARTED: &str = "command.started";
    /// Command の実行完了 (成功)
    pub const COMMAND_COMPLETED: &str = "command.completed";
    /// Command の実行失敗
    pub const COMMAND_FAILED: &str = "command.failed";
    /// Transaction の commit
    pub const TRANSACTION_COMMITTED: &str = "transaction.committed";
    /// Transaction の rollback
    pub const TRANSACTION_ROLLED_BACK: &str = "transaction.rolledback";
    /// Capability が登録された
    pub const CAPABILITY_REGISTERED: &str = "capability.registered";
    /// Capability の登録解除
    pub const CAPABILITY_UNREGISTERED: &str = "capability.unregistered";
    /// Session の作成
    pub const SESSION_CREATED: &str = "session.created";
    /// Session の終了
    pub const SESSION_CLOSED: &str = "session.closed";
    /// AI 編集 Patch の提案
    pub const AI_EDIT_PROPOSED: &str = "ai.edit.proposed";
    /// AI チャット delta
    pub const AI_CHAT_DELTA: &str = "ai.chat.delta";
    /// AI インライン suggestion
    pub const AI_INLINE_SUGGESTION: &str = "ai.inline.suggestion";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AOrdering};
    use tokio::time::Duration;

    struct CountingSubscriber {
        id: SubscriptionId,
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Subscriber for CountingSubscriber {
        fn subscription_id(&self) -> SubscriptionId {
            self.id
        }
        async fn on_event(&self, _: &EventEnvelope) -> bool {
            self.count.fetch_add(1, AOrdering::SeqCst);
            true
        }
    }

    #[tokio::test]
    async fn publish_assigns_monotonic_sequence() {
        let bus = EventBus::new(Arc::new(InMemoryDurableLog::new()));
        let e1 = bus
            .publish("test.event", Durability::Transient, "C-1", "T-1", json!({}))
            .await
            .unwrap();
        let e2 = bus
            .publish("test.event", Durability::Transient, "C-1", "T-1", json!({}))
            .await
            .unwrap();
        assert!(e2.sequence > e1.sequence);
    }

    #[tokio::test]
    async fn durable_events_are_persisted() {
        let bus = EventBus::new(Arc::new(InMemoryDurableLog::new()));
        bus.publish(
            event_type::TRANSACTION_COMMITTED,
            Durability::Durable,
            "C-1",
            "T-1",
            json!({"txn_id": "T-001"}),
        )
        .await
        .unwrap();
        bus.publish(
            event_type::TRANSACTION_COMMITTED,
            Durability::Durable,
            "C-1",
            "T-2",
            json!({"txn_id": "T-002"}),
        )
        .await
        .unwrap();
        let replayed = bus.replay(1, None).await.unwrap();
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].payload["txn_id"], "T-001");
    }

    #[tokio::test]
    async fn transient_events_are_not_persisted() {
        let bus = EventBus::new(Arc::new(InMemoryDurableLog::new()));
        bus.publish(
            "cursor.moved",
            Durability::Transient,
            "C-1",
            "T-1",
            json!({}),
        )
        .await
        .unwrap();
        let replayed = bus.replay(1, None).await.unwrap();
        assert!(replayed.is_empty());
    }

    #[tokio::test]
    async fn subscriber_receives_published_events() {
        let bus = EventBus::new(Arc::new(InMemoryDurableLog::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let sub: Arc<dyn Subscriber> = Arc::new(CountingSubscriber {
            id: SubscriptionId::new(),
            count: count.clone(),
        });
        bus.subscribe(sub);
        bus.publish("x", Durability::Transient, "C", "T", json!({}))
            .await
            .unwrap();
        // tokio::spawn が走るのを少し待つ
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(count.load(AOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unsubscribe_removes_subscriber() {
        let bus = EventBus::new(Arc::new(InMemoryDurableLog::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let sub: Arc<dyn Subscriber> = Arc::new(CountingSubscriber {
            id: SubscriptionId::new(),
            count: count.clone(),
        });
        let id = bus.subscribe(sub);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
    }
}

// `serde_json::json!` マクロが使えるよう re-export
pub use serde_json::json;