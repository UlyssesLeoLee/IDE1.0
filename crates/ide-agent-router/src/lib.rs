//! IDE1.0 Web — Semantic Agent Router & Commit Coordinator
//!
//! 在保留 IDE1.0 Tauri 桌面版和 Rust 编辑核心的基础上，
//! 为大量 AI Agent 提供：
//! - 文件级路由 (File-level Routing)
//! - 文件内部语义单元级并行作业 (Semantic Unit Parallel Tasks)
//! - Patch 级提交 (Structured Node Patch)
//! - 文件级一致性协调 (CAS + Fencing + Outbox Event Ledger)

#![forbid(unsafe_code)]

pub mod coordinator;
pub mod identity;
pub mod ledger;
pub mod outbox;
pub mod parser;
pub mod patch;
pub mod sandbox;
pub mod verifier;

pub use coordinator::{
    CoordinatorError, FileCommitCoordinator, OutboxEvent, PatchCommitResponse, TaskState, TaskStatus,
};
pub use identity::{SemanticUnit, SupportedLanguage};
pub use ledger::{
    CommitRecord, LedgerError, LedgerStore, MemoryLedgerStore, OutboxRecord, OutboxStatus,
};
pub use outbox::{
    BroadcastPubSubAdapter, DispatchBatchReport, DispatchResult, InMemoryPubSubAdapter,
    OutboxDispatchError, OutboxDispatcher, OutboxPubSubAdapter, RetryPolicy,
};
pub use parser::SemanticParser;
pub use patch::{PatchEngine, PatchError, PatchProposal, PatchResult};
pub use sandbox::{SandboxEnvironment, SandboxError, SimulationResult, WorkerSandbox};
pub use verifier::{
    IssueSeverity, PatchVerifier, VerificationIssue, VerificationReport, VerificationStatus,
};
