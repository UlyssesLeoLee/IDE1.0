//! kernel-capability-registry — Tier 3: Capability Registry
//!
//! DD-01 §MOD-CR-001 を実装。Plugin が公開する Capability (例: `text.search`、
//! `ai.chat`, `ai.edit`) を登録し、Priority ベースのルーティングで Provider を解決する。
//! MVP-1 は単一 Provider 解決のみ。Hot Swap は P1 (DD-01 §CR-007)。
//!
//! トレース: REQ-001 §FR-004 / §FR-007 / DD-01 §MOD-CR / DD-13 §F-23

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use async_trait::async_trait;
use dashmap::DashMap;
use kernel_error::{CoreError, CoreResult};
use kernel_eventbus::EventBus;
use std::sync::Arc;

/// Capability 名 (例: `"text.search"`, `"ai.chat"`, `"ai.edit"`)。
pub type CapabilityId = String;

/// Provider の優先度。高い値ほど優先される (DD-01 §MOD-CR §Priority Routing)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u32);

impl Default for Priority {
    fn default() -> Self {
        Self(100)
    }
}

/// Provider が公開する Capability メタ情報。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapabilityManifest {
    pub capability_id: CapabilityId,
    pub description: String,
    pub version: String,
    pub side_effect_level: SideEffectLevel,
}

/// Workspace に対する副作用のレベル (DD-01 §CB / DD-02 §7.3)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SideEffectLevel {
    /// 読み取りのみ (e.g. `buffer.read`, `text.search`)
    ReadOnly,
    /// 単一 Buffer / Workspace ファイルへの書き込み (e.g. `file.patch`)
    WorkspaceMutation,
    /// Transaction を跨ぐ変更 (e.g. `txn.commit`)
    Transactional,
}

/// Provider 登録エントリ。
pub struct ProviderEntry {
    pub provider: Arc<dyn CommandProvider>,
    pub manifest: CapabilityManifest,
    pub priority: Priority,
}

impl std::fmt::Debug for ProviderEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderEntry")
            .field("manifest", &self.manifest)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

/// Provider が実行する trait。MVP-1 では JSON 引数 / JSON 結果で統一。
#[async_trait]
pub trait CommandProvider: Send + Sync {
    /// Provider 名 (Plugin 名)。
    fn name(&self) -> &str;

    /// Capability 呼び出しを実行。
    /// # Errors
    /// 業務エラーは [`CoreError`] で返す。
    async fn execute(&self, args: serde_json::Value) -> CoreResult<serde_json::Value>;
}

/// Capability Registry。
pub struct CapabilityRegistry {
    providers: DashMap<CapabilityId, Vec<ProviderEntry>>,
    event_bus: Arc<EventBus>,
}

impl std::fmt::Debug for CapabilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilityRegistry")
            .field("capability_count", &self.providers.len())
            .finish()
    }
}

impl CapabilityRegistry {
    /// 新しい Registry を生成。
    #[must_use]
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            providers: DashMap::new(),
            event_bus,
        }
    }

    /// Provider を登録。
    /// # Errors
    /// Event Bus への発行失敗時に [`CoreError::Internal`] を返す。
    pub async fn register(
        &self,
        manifest: CapabilityManifest,
        provider: Arc<dyn CommandProvider>,
        priority: Priority,
    ) -> CoreResult<()> {
        let entry = ProviderEntry {
            provider,
            manifest: manifest.clone(),
            priority,
        };
        self.providers
            .entry(manifest.capability_id.clone())
            .or_default()
            .push(entry);
        // Priority 降順でソート
        if let Some(mut v) = self.providers.get_mut(&manifest.capability_id) {
            v.sort_by(|a, b| b.priority.cmp(&a.priority));
        }

        self.event_bus
            .publish(
                kernel_eventbus::event_type::CAPABILITY_REGISTERED,
                kernel_eventbus::Durability::Durable,
                manifest.capability_id.clone(),
                String::new(),
                serde_json::json!({
                    "capability_id": manifest.capability_id,
                    "version": manifest.version,
                    "priority": priority.0,
                }),
            )
            .await?;
        Ok(())
    }

    /// Provider 登録を解除 (P1 Hot Swap 用、MVP-1 では Plugin unload 時のみ)。
    /// # Errors
    /// 存在しない Capability 指定時に [`CoreError::CapabilityNotFound`] を返す。
    pub async fn unregister(&self, capability_id: &str, provider_name: &str) -> CoreResult<()> {
        let mut entry = self.providers.get_mut(capability_id).ok_or_else(|| {
            CoreError::CapabilityNotFound(capability_id.to_string())
        })?;
        let before = entry.len();
        entry.retain(|e| e.provider.name() != provider_name);
        let removed = before - entry.len();
        drop(entry);

        if removed > 0 {
            self.event_bus
                .publish(
                    kernel_eventbus::event_type::CAPABILITY_UNREGISTERED,
                    kernel_eventbus::Durability::Durable,
                    capability_id.to_string(),
                    String::new(),
                    serde_json::json!({
                        "capability_id": capability_id,
                        "provider": provider_name,
                    }),
                )
                .await?;
        }
        Ok(())
    }

    /// Capability 名から Provider を解決。Priority 降順で先頭を返す。
    /// # Errors
    /// 該当 Capability が存在しない場合に [`CoreError::CapabilityNotFound`] を返す。
    pub fn route(&self, capability_id: &str) -> CoreResult<Arc<ProviderEntry>> {
        let entries = self
            .providers
            .get(capability_id)
            .ok_or_else(|| CoreError::CapabilityNotFound(capability_id.to_string()))?;
        entries
            .first()
            .map(|e| {
                let provider = e.provider.clone();
                let manifest = e.manifest.clone();
                let priority = e.priority;
                Arc::new(ProviderEntry {
                    provider,
                    manifest,
                    priority,
                })
            })
            .ok_or_else(|| CoreError::CapabilityNotFound(capability_id.to_string()))
    }

    /// Capability の Manifest を取得。
    #[must_use]
    pub fn manifest(&self, capability_id: &str) -> Option<CapabilityManifest> {
        self.providers
            .get(capability_id)
            .and_then(|v| v.first().map(|e| e.manifest.clone()))
    }

    /// 登録された Capability 一覧。
    #[must_use]
    pub fn list(&self) -> Vec<CapabilityManifest> {
        self.providers
            .iter()
            .filter_map(|r| r.value().first().map(|e| e.manifest.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel_eventbus::{EventBus, InMemoryDurableLog};

    struct EchoProvider;
    #[async_trait]
    impl CommandProvider for EchoProvider {
        fn name(&self) -> &str {
            "echo"
        }
        async fn execute(&self, args: serde_json::Value) -> CoreResult<serde_json::Value> {
            Ok(args)
        }
    }

    struct DoubleProvider;
    #[async_trait]
    impl CommandProvider for DoubleProvider {
        fn name(&self) -> &str {
            "double"
        }
        async fn execute(&self, args: serde_json::Value) -> CoreResult<serde_json::Value> {
            let n = args["n"].as_i64().unwrap_or(0);
            Ok(serde_json::json!({ "result": n * 2 }))
        }
    }

    fn setup() -> (Arc<EventBus>, CapabilityRegistry) {
        let bus = Arc::new(EventBus::new(Arc::new(InMemoryDurableLog::new())));
        let reg = CapabilityRegistry::new(bus.clone());
        (bus, reg)
    }

    #[tokio::test]
    async fn register_and_route() {
        let (_bus, reg) = setup();
        reg.register(
            CapabilityManifest {
                capability_id: "echo".into(),
                description: "echo args".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(EchoProvider),
            Priority::default(),
        )
        .await
        .unwrap();
        let entry = reg.route("echo").unwrap();
        let out = entry.provider.execute(json!({"a": 1})).await.unwrap();
        assert_eq!(out, json!({"a": 1}));
    }

    #[tokio::test]
    async fn higher_priority_wins() {
        let (_bus, reg) = setup();
        reg.register(
            CapabilityManifest {
                capability_id: "math.double".into(),
                description: "double (low prio)".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(DoubleProvider),
            Priority(10),
        )
        .await
        .unwrap();
        reg.register(
            CapabilityManifest {
                capability_id: "math.double".into(),
                description: "double (high prio)".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(DoubleProvider),
            Priority(999),
        )
        .await
        .unwrap();
        let entry = reg.route("math.double").unwrap();
        assert_eq!(entry.priority, Priority(999));
    }

    #[tokio::test]
    async fn unknown_capability_errors() {
        let (_bus, reg) = setup();
        let err = reg.route("nonexistent").unwrap_err();
        assert!(matches!(err, CoreError::CapabilityNotFound(_)));
    }

    #[tokio::test]
    async fn unregister_removes_provider() {
        let (_bus, reg) = setup();
        reg.register(
            CapabilityManifest {
                capability_id: "echo".into(),
                description: "echo".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(EchoProvider),
            Priority::default(),
        )
        .await
        .unwrap();
        reg.unregister("echo", "echo").await.unwrap();
        assert!(reg.route("echo").is_err());
    }

    #[tokio::test]
    async fn list_returns_unique_capabilities() {
        let (_bus, reg) = setup();
        reg.register(
            CapabilityManifest {
                capability_id: "echo".into(),
                description: "a".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(EchoProvider),
            Priority(1),
        )
        .await
        .unwrap();
        reg.register(
            CapabilityManifest {
                capability_id: "echo".into(),
                description: "b".into(),
                version: "1.0".into(),
                side_effect_level: SideEffectLevel::ReadOnly,
            },
            Arc::new(EchoProvider),
            Priority(99),
        )
        .await
        .unwrap();
        let list = reg.list();
        let echo_entries: Vec<_> = list.iter().filter(|m| m.capability_id == "echo").collect();
        // highest priority の方 (description "b") が代表として返る
        assert_eq!(echo_entries.len(), 1);
        assert_eq!(echo_entries[0].description, "b");
    }
}

pub use serde_json::json;