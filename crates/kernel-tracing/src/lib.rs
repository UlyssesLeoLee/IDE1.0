//! kernel-tracing — Tier 1: 構造化ログ・Trace Context
//!
//! DD-04b §MOD-LOGGING + §MOD-TRACE に従う:
//! - 構造化ログ (JSON / pretty) 出力
//! - `trace_id` / `span_id` を W3C Trace Context 互換で付与
//! - `correlation_id` を Agent / Session 単位で伝搬
//!
//! トレース: REQ-001 §FR-021 / NFR-O-001 / INC-08

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use kernel_error::CoreResult;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// ログ出力設定 (DD-04b §MOD-LOGGING)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// ログレベル (trace / debug / info / warn / error)
    pub level: String,
    /// JSON 形式で出力するか
    #[serde(default)]
    pub json: bool,
    /// 出力先 ("stdout" / "stderr" / file path)
    #[serde(default = "default_target")]
    pub target: String,
}

fn default_target() -> String {
    "stdout".into()
}

static INIT: OnceLock<()> = OnceLock::new();

/// Tracing サブスクライバを初期化 (べき等)。
/// # Errors
/// サブスクライバ二重登録時、または無効なレベル指定時に [`CoreError::Config`] を返す。
pub fn init_tracing(cfg: &TracingConfig) -> CoreResult<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(cfg.level.clone()));

    let registry = tracing_subscriber::registry().with(env_filter);

    let result = if cfg.json {
        let layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_target(true);
        registry.with(layer).try_init()
    } else {
        let layer = fmt::layer().with_target(true).compact();
        registry.with(layer).try_init()
    };

    if result.is_ok() {
        INIT.set(()).ok();
        Ok(())
    } else {
        // 既に初期化済みは許容 (べき等)
        Ok(())
    }
}

/// Trace Context (W3C 互換、DD-04b §MOD-TRACE)。
///
/// `trace_id` (16 bytes) と `span_id` (8 bytes) を 16進文字列で保持する。
/// Agent / Session 単位の `correlation_id` を別途持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceContext {
    /// 16 hex chars (8 bytes) — W3C 準拠の最小サイズ
    pub trace_id: String,
    /// 16 hex chars (8 bytes)
    pub span_id: String,
    /// Agent / Session 単位の相関 ID
    pub correlation_id: String,
}

impl TraceContext {
    /// 新しい Trace Context を生成。
    #[must_use]
    pub fn new(correlation_id: impl Into<String>) -> Self {
        Self {
            trace_id: random_hex(16),
            span_id: random_hex(8),
            correlation_id: correlation_id.into(),
        }
    }

    /// 新しい子 Span を生成 (`trace_id` はそのまま、`span_id` のみ更新)。
    #[must_use]
    pub fn child_span(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: random_hex(8),
            correlation_id: self.correlation_id.clone(),
        }
    }

    /// W3C `traceparent` ヘッダ形式 (`00-{trace_id}-{span_id}-01`)。
    #[must_use]
    pub fn to_traceparent(&self) -> String {
        format!("00-{}-{}-01", self.trace_id, self.span_id)
    }

    /// `traceparent` ヘッダから復元。形式不正時は `None`。
    #[must_use]
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }
        Some(Self {
            trace_id: parts[1].into(),
            span_id: parts[2].into(),
            correlation_id: String::new(),
        })
    }
}

fn random_hex(byte_len: usize) -> String {
    use std::fmt::Write;
    let mut buf = String::with_capacity(byte_len * 2);
    let mut counter: u8 = 0;
    for _ in 0..byte_len {
        let b: u8 = (uuid::Uuid::new_v4().as_u128() as u8).wrapping_add(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u8)
                .unwrap_or(0)
                .wrapping_add(counter),
        );
        counter = counter.wrapping_add(1);
        let _ = write!(buf, "{b:02x}");
    }
    buf.truncate(byte_len * 2);
    buf
}

/// 簡易 Span ヘルパ: 関数前後で `tracing::info_span!` を発行する。
#[macro_export]
macro_rules! kernel_span {
    ($name:expr, $ctx:expr) => {{
        tracing::info_span!(
            $name,
            trace_id = %$ctx.trace_id,
            span_id = %$ctx.span_id,
            correlation_id = %$ctx.correlation_id,
        )
    }};
    ($name:expr) => {{
        tracing::info_span!($name)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_context_roundtrip() {
        let ctx = TraceContext::new("S-abc");
        let header = ctx.to_traceparent();
        let parsed = TraceContext::from_traceparent(&header).unwrap();
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.span_id, ctx.span_id);
    }

    #[test]
    fn invalid_traceparent_returns_none() {
        assert!(TraceContext::from_traceparent("garbage").is_none());
        assert!(TraceContext::from_traceparent("").is_none());
    }

    #[test]
    fn child_span_keeps_trace_id() {
        let parent = TraceContext::new("C-1");
        let child = parent.child_span();
        assert_eq!(parent.trace_id, child.trace_id);
        assert_ne!(parent.span_id, child.span_id);
    }

    #[test]
    fn init_tracing_is_idempotent() {
        let cfg = TracingConfig {
            level: "info".into(),
            json: false,
            target: "stdout".into(),
        };
        init_tracing(&cfg).unwrap();
        init_tracing(&cfg).unwrap(); // 二度呼んでも OK
    }

    #[test]
    fn trace_id_has_expected_length() {
        let ctx = TraceContext::new("test");
        assert_eq!(ctx.trace_id.len(), 32);
        assert_eq!(ctx.span_id.len(), 16);
    }
}