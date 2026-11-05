//! Transactional Outbox 异步事件派发器 (WBS-05)
//!
//! 提供基于发布订阅适配器模型 (Pub/Sub Adapter) 的可靠事件分发、
//! 指数退避重试策略 (Retry Policy) 以及与 LedgerStore 紧密协同的状态流转标记。

use crate::coordinator::OutboxEvent;
use crate::ledger::{LedgerError, LedgerStore};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OutboxDispatchError {
    #[error("Ledger storage error: {0}")]
    Ledger(#[from] LedgerError),

    #[error("Publish adapter failure: {0}")]
    PublishFailure(String),

    #[error("Internal dispatcher failure: {0}")]
    Internal(String),
}

/// 发布订阅适配器抽象，解耦底层具体传输通道 (如内存广播、NATS、Kafka、Redis 等)
#[async_trait::async_trait]
pub trait OutboxPubSubAdapter: Send + Sync {
    async fn publish(&self, event: &OutboxEvent) -> Result<(), OutboxDispatchError>;
}

/// 内存广播适配器：利用 Tokio Broadcast 频道进行实时多订阅者广播
pub struct BroadcastPubSubAdapter {
    sender: tokio::sync::broadcast::Sender<OutboxEvent>,
}

impl BroadcastPubSubAdapter {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity.max(16));
        Self { sender }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<OutboxEvent> {
        self.sender.subscribe()
    }
}

#[async_trait::async_trait]
impl OutboxPubSubAdapter for BroadcastPubSubAdapter {
    async fn publish(&self, event: &OutboxEvent) -> Result<(), OutboxDispatchError> {
        // 如果当前没有任何活跃接收方，send 会返回 SendError，但在发布/订阅模式下这是正常情况
        let _ = self.sender.send(event.clone());
        Ok(())
    }
}

/// 内存收集与模拟适配器：支持事件收集与错误注入测试
pub struct InMemoryPubSubAdapter {
    events: Arc<RwLock<Vec<OutboxEvent>>>,
    fail_countdown: Arc<RwLock<usize>>,
    fail_message: Arc<RwLock<String>>,
}

impl InMemoryPubSubAdapter {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            fail_countdown: Arc::new(RwLock::new(0)),
            fail_message: Arc::new(RwLock::new(String::new())),
        }
    }

    pub fn published_events(&self) -> Vec<OutboxEvent> {
        self.events.read().unwrap().clone()
    }

    pub fn published_count(&self) -> usize {
        self.events.read().unwrap().len()
    }

    pub fn clear(&self) {
        self.events.write().unwrap().clear();
    }

    pub fn simulate_failure_for_next(&self, count: usize, error_msg: &str) {
        *self.fail_countdown.write().unwrap() = count;
        *self.fail_message.write().unwrap() = error_msg.to_string();
    }
}

impl Default for InMemoryPubSubAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl OutboxPubSubAdapter for InMemoryPubSubAdapter {
    async fn publish(&self, event: &OutboxEvent) -> Result<(), OutboxDispatchError> {
        let mut countdown = self.fail_countdown.write().unwrap();
        if *countdown > 0 {
            *countdown -= 1;
            let msg = self.fail_message.read().unwrap().clone();
            return Err(OutboxDispatchError::PublishFailure(if msg.is_empty() {
                "Simulated publish adapter network failure".to_string()
            } else {
                msg
            }));
        }

        self.events.write().unwrap().push(event.clone());
        Ok(())
    }
}

/// 事件派发重试策略配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 10,
            max_backoff_ms: 500,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            initial_backoff_ms: 0,
            max_backoff_ms: 0,
            backoff_multiplier: 1.0,
        }
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_backoff(mut self, initial_ms: u64, max_ms: u64, multiplier: f64) -> Self {
        self.initial_backoff_ms = initial_ms;
        self.max_backoff_ms = max_ms;
        self.backoff_multiplier = multiplier;
        self
    }

    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt <= self.max_retries
    }

    pub fn backoff_duration(&self, attempt: u32) -> Duration {
        if attempt == 0 || self.initial_backoff_ms == 0 {
            return Duration::from_millis(0);
        }
        let mult = self.backoff_multiplier.powi(attempt.saturating_sub(1) as i32);
        let backoff = (self.initial_backoff_ms as f64 * mult) as u64;
        let clamped = backoff.min(self.max_backoff_ms);
        Duration::from_millis(clamped)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    pub event_id: u64,
    pub success: bool,
    pub retry_count: u32,
    pub error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DispatchBatchReport {
    pub polled_count: usize,
    pub dispatched_count: usize,
    pub failed_count: usize,
    pub results: Vec<DispatchResult>,
}

/// Transactional Outbox 异步派发器核心
pub struct OutboxDispatcher<S, P>
where
    S: LedgerStore,
    P: OutboxPubSubAdapter,
{
    ledger: Arc<S>,
    pubsub: Arc<P>,
    retry_policy: RetryPolicy,
    batch_size: usize,
}

impl<S, P> OutboxDispatcher<S, P>
where
    S: LedgerStore,
    P: OutboxPubSubAdapter,
{
    pub fn new(ledger: Arc<S>, pubsub: Arc<P>, retry_policy: RetryPolicy) -> Self {
        Self {
            ledger,
            pubsub,
            retry_policy,
            batch_size: 100,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    pub fn ledger(&self) -> &Arc<S> {
        &self.ledger
    }

    pub fn pubsub(&self) -> &Arc<P> {
        &self.pubsub
    }

    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    /// 执行单批次待派发事件轮询与分发
    pub async fn dispatch_once(&self) -> Result<DispatchBatchReport, OutboxDispatchError> {
        let pending = self.ledger.poll_pending_outbox(self.batch_size).await?;
        let polled_count = pending.len();

        let mut report = DispatchBatchReport {
            polled_count,
            dispatched_count: 0,
            failed_count: 0,
            results: Vec::with_capacity(polled_count),
        };

        for entry in pending {
            let event = entry.to_outbox_event();
            match self.pubsub.publish(&event).await {
                Ok(()) => {
                    self.ledger.mark_outbox_dispatched(entry.event_id).await?;
                    report.dispatched_count += 1;
                    report.results.push(DispatchResult {
                        event_id: entry.event_id,
                        success: true,
                        retry_count: entry.retry_count,
                        error: None,
                    });
                }
                Err(err) => {
                    let next_retry = entry.retry_count + 1;
                    let err_msg = err.to_string();

                    if self.retry_policy.should_retry(next_retry) {
                        // 仍在重试额度内：保留 Pending 状态并累加重试计数与错误原因
                        self.ledger
                            .mark_outbox_retry(entry.event_id, &err_msg, next_retry)
                            .await?;
                    } else {
                        // 重试次数已耗尽：置为终态 Failed
                        let terminal_msg = format!(
                            "Max retries ({}) exceeded: {}",
                            self.retry_policy.max_retries, err_msg
                        );
                        self.ledger
                            .mark_outbox_failed(entry.event_id, &terminal_msg, next_retry)
                            .await?;
                    }

                    report.failed_count += 1;
                    report.results.push(DispatchResult {
                        event_id: entry.event_id,
                        success: false,
                        retry_count: next_retry,
                        error: Some(err_msg),
                    });
                }
            }
        }

        Ok(report)
    }

    /// 循环拉取直到所有当前就绪事件全部处理完毕 (或达到重试耗尽)
    pub async fn dispatch_all(&self) -> Result<DispatchBatchReport, OutboxDispatchError> {
        let mut total_report = DispatchBatchReport::default();

        loop {
            let batch = self.dispatch_once().await?;
            if batch.polled_count == 0 {
                break;
            }

            total_report.polled_count += batch.polled_count;
            total_report.dispatched_count += batch.dispatched_count;
            total_report.failed_count += batch.failed_count;
            total_report.results.extend(batch.results);

            // 若本轮拉取的事件全部派发失败，避免在同一调度点发生死循环空转
            if batch.dispatched_count == 0 && batch.failed_count > 0 {
                break;
            }
        }

        Ok(total_report)
    }

    /// 启动后台常驻派发循环
    pub async fn run_loop(
        &self,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        interval: Duration,
    ) -> Result<usize, OutboxDispatchError> {
        let mut total_dispatched = 0;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(interval) => {
                    let report = self.dispatch_once().await?;
                    total_dispatched += report.dispatched_count;
                }
            }
        }

        Ok(total_dispatched)
    }
}
