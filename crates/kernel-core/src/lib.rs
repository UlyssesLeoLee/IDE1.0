//! kernel-core — Tier 4: Microkernel Core 基盤
//!
//! DD-01 §2.1 に従い、Kernel 全体を束ねる DI コンテナ [`Microkernel`] を提供。
//! EventBus / SessionManager / CapabilityRegistry / CommandBus を 1 つのインスタンスにまとめ、
//! CLI や外部 Adapter から単一ハンドルで利用できるようにする。
//!
//! トレース: REQ-001 §FR-001〜§FR-022 / DD-01 §2 / AD-001 §2

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use kernel_capability_registry::CapabilityRegistry;
use kernel_cmd_bus::CommandBus;
use kernel_error::CoreResult;
use kernel_eventbus::{DurableLog, EventBus, InMemoryDurableLog};
use kernel_session::SessionManager;
use std::sync::Arc;

/// Microkernel Core の集約ハンドル。
#[derive(Clone)]
pub struct Microkernel {
    pub event_bus: Arc<EventBus>,
    pub session_manager: Arc<SessionManager>,
    pub capability_registry: Arc<CapabilityRegistry>,
    pub command_bus: Arc<CommandBus>,
}

impl std::fmt::Debug for Microkernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Microkernel")
            .field("event_bus", &self.event_bus)
            .field("session_manager", &self.session_manager)
            .field("capability_registry", &self.capability_registry)
            .field("command_bus", &self.command_bus)
            .finish()
    }
}

impl Microkernel {
    /// 新規 Microkernel を生成 (in-memory DurableLog)。
    #[must_use]
    pub fn new() -> Self {
        Self::with_durable_log(Arc::new(InMemoryDurableLog::new()))
    }

    /// 任意の DurableLog で初期化。
    #[must_use]
    pub fn with_durable_log(log: Arc<dyn DurableLog>) -> Self {
        let event_bus = Arc::new(EventBus::new(log));
        let session_manager = Arc::new(SessionManager::new(event_bus.clone()));
        let capability_registry = Arc::new(CapabilityRegistry::new(event_bus.clone()));
        let command_bus = Arc::new(CommandBus::new(
            capability_registry.clone(),
            session_manager.clone(),
            event_bus.clone(),
        ));
        Self {
            event_bus,
            session_manager,
            capability_registry,
            command_bus,
        }
    }

    /// 基本的な "echo" Capability を登録 (MVP-1 受入基準: `echo "hello"` 実行可能)。
    /// # Errors
    /// Event Bus への発行失敗時に [`kernel_error::CoreError`] を返す。
    pub async fn register_echo_capability(&self) -> CoreResult<()> {
        use async_trait::async_trait;
        use kernel_capability_registry::{
            CapabilityManifest, CommandProvider, Priority, SideEffectLevel,
        };

        struct EchoProvider;
        #[async_trait]
        impl CommandProvider for EchoProvider {
            fn name(&self) -> &str {
                "echo"
            }
            async fn execute(
                &self,
                args: serde_json::Value,
            ) -> kernel_error::CoreResult<serde_json::Value> {
                Ok(args)
            }
        }

        self.capability_registry
            .register(
                CapabilityManifest {
                    capability_id: "echo".into(),
                    description: "echo arguments back".into(),
                    version: "1.0".into(),
                    side_effect_level: SideEffectLevel::ReadOnly,
                },
                Arc::new(EchoProvider),
                Priority::default(),
            )
            .await
    }
}

impl Default for Microkernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel_cmd_bus::CommandBuilder;
    use kernel_session::{ActorKind, PermissionSet, ResourceBudget};

    #[tokio::test]
    async fn microkernel_end_to_end_echo() {
        let kernel = Microkernel::new();
        kernel.register_echo_capability().await.unwrap();

        let session = kernel
            .session_manager
            .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
            .await
            .unwrap();

        // echo "hello" 相当
        let cmd = CommandBuilder::new("echo", session.id.clone())
            .args(serde_json::json!({"text": "hello"}))
            .build();
        let result = kernel.command_bus.execute(cmd).await.unwrap();
        assert_eq!(result.data.unwrap()["text"], "hello");
    }

    #[tokio::test]
    async fn microkernel_event_flow_emits_started_and_completed() {
        let kernel = Microkernel::new();
        kernel.register_echo_capability().await.unwrap();
        let session = kernel
            .session_manager
            .create(ActorKind::Human, PermissionSet::human_editor(), ResourceBudget::default())
            .await
            .unwrap();

        let cmd = CommandBuilder::new("echo", session.id.clone()).build();
        kernel.command_bus.execute(cmd).await.unwrap();

        // Replay で command.started / command.completed が Durable 配信されている
        let replayed = kernel.event_bus.replay(1, None).await.unwrap();
        let types: Vec<&str> = replayed.iter().map(|e| e.event_type.as_str()).collect();
        assert!(types.contains(&kernel_eventbus::event_type::COMMAND_STARTED));
        assert!(types.contains(&kernel_eventbus::event_type::COMMAND_COMPLETED));
    }
}