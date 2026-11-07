//! IDE1.0 Web — Semantic Agent Router & Commit Coordinator
//!
//! 在保留 IDE1.0 Tauri 桌面版和 Rust 编辑核心的基础上，
//! 为大量 AI Agent 提供：
//! - 阶段 ①：路由拆解 (Router Agent: Task Input -> DAG & File Allocations)
//! - 阶段 ②：.nema 并行协作 (Co-editing Group A & Independent Group B)
//! - 阶段 ③：汇总统筹 & .nema -> Rust 降维转换 (Aggregator: Semantic Merge & SourceMap)
//! - 阶段 ④：文件级 Git 管理与四级门禁验证 (Verify: Build / Test / Quality / Spec & Iteration Feedback Loop)
//! - 异构智能体独立配置模型、Reasoning Effort、Thinking Mode 与自审加固防线

#![forbid(unsafe_code)]

pub mod agent_profile;
pub mod collaboration;
pub mod coordinator;
pub mod identity;
pub mod ledger;
pub mod nema_pipeline;
pub mod outbox;
pub mod parser;
pub mod patch;
pub mod reviewer;
pub mod sandbox;
pub mod tenant;
pub mod verifier;

pub use agent_profile::{
    AgentModelProfile, AgentSlotId, EffectiveAgentConfig, FeedbackDecision, FeedbackLoopGuard,
    GlobalAiBaseline, MultiAgentProfileRegistry, ReasoningEffort, ThinkingMode,
};
pub use collaboration::{
    AnchorOffsetTable, CollaborationHub, CollaborationSession, CollaboratorColor, CollaboratorRole,
    KeystrokeBroadcast, KeystrokeOp, RemoteCursor, ZoneAnchor,
};
pub use coordinator::{
    CoordinatorError, FileCommitCoordinator, OutboxEvent, PatchCommitResponse, TaskState, TaskStatus,
};
pub use identity::{SemanticUnit, SupportedLanguage};
pub use ledger::{
    CommitRecord, LedgerError, LedgerStore, MemoryLedgerStore, OutboxRecord, OutboxStatus,
};
pub use nema_pipeline::{
    CoEditBranchAssignment, CollaborationGroupKind, ConflictResolutionStrategy,
    DependencyEdge, GitBranchOrchestrator, GitBranchRecord, GitBranchState,
    IndependentBranchAssignment, IterationFeedbackTicket, LoweringOutcome,
    MultiAgentPipelineCoordinator, NemaMergeOutcome, NemaRustDriftGuard,
    NemaRustSourceMap, NemaRustSourceMapEntry, NemaSemanticConflict,
    NemaSemanticMerger, NemaToRustLowering, PullRequestRecord, RouterEngine,
    TaskDecompositionPlan, TaskInputKind, VerifyPipeline, VerifyPipelineReport,
};
pub use outbox::{
    BroadcastPubSubAdapter, DispatchBatchReport, DispatchResult, InMemoryPubSubAdapter,
    OutboxDispatchError, OutboxDispatcher, OutboxPubSubAdapter, RetryPolicy,
};
pub use parser::SemanticParser;
pub use patch::{PatchEngine, PatchError, PatchProposal, PatchResult};
pub use reviewer::{
    AcceptanceCertificate, AttributedHunk, GuidanceZone, HintLevel, HunkStatus, ReviewDecision,
    ReviewPhase, ReviewSession, ReviewerHint, UpfrontPlan,
};
pub use sandbox::{SandboxEnvironment, SandboxError, SimulationResult, WorkerSandbox};
pub use tenant::{
    MultiTenantCollaborationHub, MultiTenantSession, TenantContext, TenantError, TenantQuota,
    TenantTier,
};
pub use verifier::{
    IssueSeverity, PatchVerifier, VerificationIssue, VerificationReport, VerificationStatus,
};
