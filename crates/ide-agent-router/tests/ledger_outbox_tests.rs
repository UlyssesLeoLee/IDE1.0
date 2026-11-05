//! WBS-05 分布式账本与 Transactional Outbox 综合测试
//!
//! 验证：
//! 1. 提交与 Outbox 事件原子性 (CAS 冲突回滚与幂等安全)
//! 2. Outbox 轮询与已派发标记 (包含指数退避重试与终态死信流转)
//! 3. 并发提交下的修订版本一致性 (高并发 CAS 争用下的线性一致性保证)
//! 4. 广播发布订阅适配器的多端接收能力

use chrono::Utc;
use ide_agent_router::{
    BroadcastPubSubAdapter, CommitRecord, InMemoryPubSubAdapter, LedgerError, LedgerStore,
    MemoryLedgerStore, OutboxDispatcher, OutboxEvent, OutboxStatus, RetryPolicy,
};
use std::sync::Arc;
use tokio::task::JoinSet;

#[tokio::test]
async fn test_commit_and_outbox_event_atomicity() {
    let store = Arc::new(MemoryLedgerStore::new());
    let file_id = "ws_test:src/main.rs";

    // 1. 初始化文件基准版本为 1
    store.init_file_revision(file_id, 1).await.unwrap();
    assert_eq!(store.get_file_revision(file_id).await.unwrap(), Some(1));

    // 2. 正常提交：版本 1 -> 2
    let commit1 = CommitRecord {
        commit_id: "commit_001".to_string(),
        file_id: file_id.to_string(),
        workspace_id: "ws_test".to_string(),
        previous_revision: 1,
        new_revision: 2,
        patch_content: Some("pub fn run() {}".to_string()),
        metadata: serde_json::json!({ "worker": "worker_a" }),
        created_at: Utc::now(),
    };
    let event1 = OutboxEvent {
        event_id: 101,
        event_type: "FILE_COMMITTED".to_string(),
        workspace_id: "ws_test".to_string(),
        file_id: file_id.to_string(),
        revision: 2,
        payload: serde_json::json!({ "commit_id": "commit_001" }),
        created_at: Utc::now(),
    };

    store.record_commit(commit1, event1).await.unwrap();

    // 验证更新成功
    assert_eq!(store.get_file_revision(file_id).await.unwrap(), Some(2));
    assert_eq!(store.total_commits(), 1);
    let pending = store.poll_pending_outbox(10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_id, 101);
    assert_eq!(pending[0].status, OutboxStatus::Pending);

    // 3. 冲突提交：由于预期版本仍为 1 (现已为 2)，必须触发 CAS 拒绝
    let commit_conflict = CommitRecord {
        commit_id: "commit_002_stale".to_string(),
        file_id: file_id.to_string(),
        workspace_id: "ws_test".to_string(),
        previous_revision: 1, // Stale!
        new_revision: 2,
        patch_content: Some("conflicting content".to_string()),
        metadata: serde_json::json!({ "worker": "worker_b" }),
        created_at: Utc::now(),
    };
    let event_conflict = OutboxEvent {
        event_id: 102,
        event_type: "FILE_COMMITTED".to_string(),
        workspace_id: "ws_test".to_string(),
        file_id: file_id.to_string(),
        revision: 2,
        payload: serde_json::json!({ "commit_id": "commit_002_stale" }),
        created_at: Utc::now(),
    };

    let err = store
        .record_commit(commit_conflict, event_conflict)
        .await
        .unwrap_err();

    match err {
        LedgerError::RevisionConflict {
            expected, actual, ..
        } => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 2);
        }
        other => panic!("Expected RevisionConflict, got: {:?}", other),
    }

    // 验证原子性：版本仍为 2，Commit 记录未增加，Outbox 待派发事件未增加
    assert_eq!(store.get_file_revision(file_id).await.unwrap(), Some(2));
    assert_eq!(store.total_commits(), 1);
    let pending_after = store.poll_pending_outbox(10).await.unwrap();
    assert_eq!(pending_after.len(), 1);
    assert_eq!(pending_after[0].event_id, 101);

    // 4. 重复 CommitId 冲突校验
    let commit_dup = CommitRecord {
        commit_id: "commit_001".to_string(), // 重复
        file_id: file_id.to_string(),
        workspace_id: "ws_test".to_string(),
        previous_revision: 2,
        new_revision: 3,
        patch_content: None,
        metadata: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    let event_dup = OutboxEvent {
        event_id: 103,
        event_type: "FILE_COMMITTED".to_string(),
        workspace_id: "ws_test".to_string(),
        file_id: file_id.to_string(),
        revision: 3,
        payload: serde_json::Value::Null,
        created_at: Utc::now(),
    };

    let err_dup = store
        .record_commit(commit_dup, event_dup)
        .await
        .unwrap_err();
    assert_eq!(
        err_dup,
        LedgerError::DuplicateCommit("commit_001".to_string())
    );
    assert_eq!(store.get_file_revision(file_id).await.unwrap(), Some(2));
}

#[tokio::test]
async fn test_outbox_polling_and_dispatched_marking() {
    let store = Arc::new(MemoryLedgerStore::new());
    let pubsub = Arc::new(InMemoryPubSubAdapter::new());
    let file_id = "ws_test:lib.rs";
    store.init_file_revision(file_id, 0).await.unwrap();

    // 1. 批量写入 3 条提交及对应的待派发事件
    for rev in 1..=3 {
        let commit = CommitRecord {
            commit_id: format!("c_{}", rev),
            file_id: file_id.to_string(),
            workspace_id: "ws_test".to_string(),
            previous_revision: rev - 1,
            new_revision: rev,
            patch_content: Some(format!("fn f{}() {{}}", rev)),
            metadata: serde_json::Value::Null,
            created_at: Utc::now(),
        };
        let event = OutboxEvent {
            event_id: rev * 10,
            event_type: "FILE_COMMITTED".to_string(),
            workspace_id: "ws_test".to_string(),
            file_id: file_id.to_string(),
            revision: rev,
            payload: serde_json::json!({ "rev": rev }),
            created_at: Utc::now(),
        };
        store.record_commit(commit, event).await.unwrap();
    }

    // 验证初始状态：3 条待派发
    let pending = store.poll_pending_outbox(10).await.unwrap();
    assert_eq!(pending.len(), 3);

    // 2. 构造 Dispatcher 并执行派发
    let retry_policy = RetryPolicy::default().with_max_retries(3);
    let dispatcher = OutboxDispatcher::new(store.clone(), pubsub.clone(), retry_policy);

    let report = dispatcher.dispatch_once().await.unwrap();
    assert_eq!(report.polled_count, 3);
    assert_eq!(report.dispatched_count, 3);
    assert_eq!(report.failed_count, 0);

    // 验证适配器已接收所有事件
    assert_eq!(pubsub.published_count(), 3);

    // 验证状态标记：账本中所有事件均已置为 Dispatched
    let pending_after = store.poll_pending_outbox(10).await.unwrap();
    assert_eq!(pending_after.len(), 0);

    for rev in 1..=3 {
        let entry = store.get_outbox_entry(rev * 10).await.unwrap().unwrap();
        assert_eq!(entry.status, OutboxStatus::Dispatched);
        assert_eq!(entry.retry_count, 0);
    }

    // 3. 错误重试与恢复测试
    pubsub.simulate_failure_for_next(2, "Temporary connection drop");

    let commit_retry = CommitRecord {
        commit_id: "c_retry".to_string(),
        file_id: file_id.to_string(),
        workspace_id: "ws_test".to_string(),
        previous_revision: 3,
        new_revision: 4,
        patch_content: None,
        metadata: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    let event_retry = OutboxEvent {
        event_id: 40,
        event_type: "FILE_COMMITTED".to_string(),
        workspace_id: "ws_test".to_string(),
        file_id: file_id.to_string(),
        revision: 4,
        payload: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    store.record_commit(commit_retry, event_retry).await.unwrap();

    // 第一次尝试：模拟错误触发
    let report_fail1 = dispatcher.dispatch_once().await.unwrap();
    assert_eq!(report_fail1.polled_count, 1);
    assert_eq!(report_fail1.dispatched_count, 0);
    assert_eq!(report_fail1.failed_count, 1);
    let entry_after_f1 = store.get_outbox_entry(40).await.unwrap().unwrap();
    assert_eq!(entry_after_f1.status, OutboxStatus::Pending); // 仍处在重试中
    assert_eq!(entry_after_f1.retry_count, 1);
    assert!(entry_after_f1.last_error.unwrap().contains("Temporary connection drop"));

    // 第二次尝试：仍然失败
    let report_fail2 = dispatcher.dispatch_once().await.unwrap();
    assert_eq!(report_fail2.failed_count, 1);
    let entry_after_f2 = store.get_outbox_entry(40).await.unwrap().unwrap();
    assert_eq!(entry_after_f2.retry_count, 2);

    // 第三次尝试：错误倒计时结束，派发成功
    let report_succ = dispatcher.dispatch_once().await.unwrap();
    assert_eq!(report_succ.dispatched_count, 1);
    let entry_after_succ = store.get_outbox_entry(40).await.unwrap().unwrap();
    assert_eq!(entry_after_succ.status, OutboxStatus::Dispatched);

    // 4. 重试耗尽转为终态 Failed 测试
    pubsub.simulate_failure_for_next(10, "Persistent channel fatal");
    let commit_dlq = CommitRecord {
        commit_id: "c_dlq".to_string(),
        file_id: file_id.to_string(),
        workspace_id: "ws_test".to_string(),
        previous_revision: 4,
        new_revision: 5,
        patch_content: None,
        metadata: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    let event_dlq = OutboxEvent {
        event_id: 50,
        event_type: "FILE_COMMITTED".to_string(),
        workspace_id: "ws_test".to_string(),
        file_id: file_id.to_string(),
        revision: 5,
        payload: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    store.record_commit(commit_dlq, event_dlq).await.unwrap();

    let strict_dispatcher =
        OutboxDispatcher::new(store.clone(), pubsub.clone(), RetryPolicy::default().with_max_retries(2));

    // attempt 1 (retry_count -> 1, <=2 -> Pending)
    strict_dispatcher.dispatch_once().await.unwrap();
    // attempt 2 (retry_count -> 2, <=2 -> Pending)
    strict_dispatcher.dispatch_once().await.unwrap();
    // attempt 3 (retry_count -> 3, >2 -> Failed)
    strict_dispatcher.dispatch_once().await.unwrap();

    let dead_letter = store.get_outbox_entry(50).await.unwrap().unwrap();
    assert_eq!(dead_letter.status, OutboxStatus::Failed);
    assert_eq!(dead_letter.retry_count, 3);
    assert!(dead_letter.last_error.unwrap().contains("Max retries (2) exceeded"));

    // 随后轮询不再包含该已进入死信态的事件
    let final_pending = store.poll_pending_outbox(10).await.unwrap();
    assert_eq!(final_pending.len(), 0);
}

#[tokio::test]
async fn test_concurrent_commits_revision_consistency() {
    let store = Arc::new(MemoryLedgerStore::new());
    let file_id = "ws_concurrency:counter.rs";
    let initial_revision = 1;
    store.init_file_revision(file_id, initial_revision).await.unwrap();

    let concurrent_workers = 50;
    let mut tasks = JoinSet::new();

    // 启动 50 个并发协程，对同一个文件发起递增提交。
    // 每个 Worker 采用乐观 CAS 循环：读取当前版本 -> 尝试 commit -> 发生冲突则重新读取并重试
    for i in 0..concurrent_workers {
        let store_clone = store.clone();
        let file_key = file_id.to_string();

        tasks.spawn(async move {
            let worker_id = format!("worker_{:02}", i);
            let mut attempts = 0;

            loop {
                attempts += 1;
                let current_rev = store_clone
                    .get_file_revision(&file_key)
                    .await
                    .unwrap()
                    .expect("file must exist");

                let commit = CommitRecord {
                    commit_id: format!("commit_{}_{}", worker_id, attempts),
                    file_id: file_key.clone(),
                    workspace_id: "ws_concurrency".to_string(),
                    previous_revision: current_rev,
                    new_revision: current_rev + 1,
                    patch_content: Some(format!("// patch by {} at attempt {}", worker_id, attempts)),
                    metadata: serde_json::json!({ "worker": worker_id, "attempts": attempts }),
                    created_at: Utc::now(),
                };

                let event = OutboxEvent {
                    event_id: 0, // 委托存储自动分配单调自增 ID
                    event_type: "FILE_COMMITTED".to_string(),
                    workspace_id: "ws_concurrency".to_string(),
                    file_id: file_key.clone(),
                    revision: current_rev + 1,
                    payload: serde_json::json!({ "worker": worker_id }),
                    created_at: Utc::now(),
                };

                match store_clone.record_commit(commit, event).await {
                    Ok(()) => break,
                    Err(LedgerError::RevisionConflict { .. }) => {
                        // CAS 冲突，短暂让出时间片并重试
                        tokio::task::yield_now().await;
                    }
                    Err(other) => panic!("Unexpected error in worker {}: {:?}", worker_id, other),
                }
            }
        });
    }

    // 等待所有 50 个 Worker 成功完成 CAS 提交
    while let Some(res) = tasks.join_next().await {
        res.unwrap();
    }

    // 验证修订版本绝对一致性：1 + 50 = 51
    let final_rev = store.get_file_revision(file_id).await.unwrap().unwrap();
    assert_eq!(
        final_rev,
        initial_revision + concurrent_workers as u64,
        "最终版本号必须严格等于初始版本加上成功提交数"
    );

    assert_eq!(store.total_commits(), concurrent_workers);
    assert_eq!(store.total_outbox_entries(), concurrent_workers);

    // 验证 Outbox 包含全部 50 个待派发事件
    let pending = store.poll_pending_outbox(100).await.unwrap();
    assert_eq!(pending.len(), concurrent_workers);

    // 使用广播适配器批量分发所有事件
    let broadcast_pubsub = Arc::new(BroadcastPubSubAdapter::new(100));
    let mut rx = broadcast_pubsub.subscribe();

    let dispatcher = OutboxDispatcher::new(
        store.clone(),
        broadcast_pubsub.clone(),
        RetryPolicy::default(),
    )
    .with_batch_size(20);

    let batch_report = dispatcher.dispatch_all().await.unwrap();
    assert_eq!(batch_report.dispatched_count, concurrent_workers);

    // 验证订阅者完整无误接收所有事件
    for _ in 0..concurrent_workers {
        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type, "FILE_COMMITTED");
        assert_eq!(received.file_id, file_id);
    }

    // 账本中已无未派发事件
    let remaining_pending = store.poll_pending_outbox(100).await.unwrap();
    assert_eq!(remaining_pending.len(), 0);
}

#[tokio::test]
async fn test_broadcast_pubsub_multiple_subscribers() {
    let pubsub = Arc::new(BroadcastPubSubAdapter::new(32));
    let mut sub1 = pubsub.subscribe();
    let mut sub2 = pubsub.subscribe();

    let store = Arc::new(MemoryLedgerStore::new());
    store.init_file_revision("f1", 0).await.unwrap();

    let commit = CommitRecord {
        commit_id: "c_broadcast".to_string(),
        file_id: "f1".to_string(),
        workspace_id: "ws1".to_string(),
        previous_revision: 0,
        new_revision: 1,
        patch_content: None,
        metadata: serde_json::Value::Null,
        created_at: Utc::now(),
    };
    let event = OutboxEvent {
        event_id: 1,
        event_type: "FILE_COMMITTED".to_string(),
        workspace_id: "ws1".to_string(),
        file_id: "f1".to_string(),
        revision: 1,
        payload: serde_json::json!({ "msg": "hello broadcast" }),
        created_at: Utc::now(),
    };
    store.record_commit(commit, event).await.unwrap();

    let dispatcher = OutboxDispatcher::new(store.clone(), pubsub.clone(), RetryPolicy::no_retry());
    let report = dispatcher.dispatch_once().await.unwrap();
    assert_eq!(report.dispatched_count, 1);

    let msg1 = sub1.recv().await.unwrap();
    let msg2 = sub2.recv().await.unwrap();
    assert_eq!(msg1.event_id, 1);
    assert_eq!(msg2.event_id, 1);
    assert_eq!(msg1.payload["msg"], "hello broadcast");
    assert_eq!(msg2.payload["msg"], "hello broadcast");
}
