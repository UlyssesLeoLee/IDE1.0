//! kernel-error — Tier 0: 統一エラー型・コード体系
//!
//! DD-01 §3.1 / INC-07 error_mapping に従い、Kernel 全体で使う `CoreError` を定義する。
//! 各下位 crate は自前の `*Error` を定義し、`From` で `CoreError` に変換する。
//! `kernel-cmd-bus` / `kernel-session` 等の上位 crate は `CoreError` を再エクスポートする。
//!
//! トレース: REQ-001 §FR-022 / NFR-R-003 / SD-001 §7 / INC-07

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// すべての Kernel 内エラーを表現する統一エラー型。
///
/// カテゴリ (`ERR-BIZ` / `ERR-EXT` / `ERR-SYS`) と個別コードを `:` 区切りで持つ。
/// 例: `ERR-BIZ:004` (Capability Not Found)、`ERR-EXT:001` (External Provider Error)。
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("ERR-BIZ:001 validation failed: {0}")]
    Validation(String),

    #[error("ERR-BIZ:002 authz denied: {0}")]
    AuthzDenied(String),

    #[error("ERR-BIZ:003 session not found: {0}")]
    SessionNotFound(String),

    #[error("ERR-BIZ:004 capability not found: {0}")]
    CapabilityNotFound(String),

    #[error("ERR-BIZ:005 command not found: {0}")]
    CommandNotFound(String),

    #[error("ERR-BIZ:006 permission denied for command: {0}")]
    CommandPermissionDenied(String),

    #[error("ERR-BIZ:010 invalid argument: {0}")]
    InvalidArgument(String),

    #[error("ERR-BIZ:011 conflict: {0}")]
    Conflict(String),

    #[error("ERR-BIZ:012 transaction failed: {0}")]
    Transaction(String),

    #[error("ERR-EXT:001 provider error: {0}")]
    Provider(String),

    #[error("ERR-EXT:002 network error: {0}")]
    Network(String),

    #[error("ERR-EXT:003 serialization error: {0}")]
    Serialization(String),

    #[error("ERR-SYS:001 internal: {0}")]
    Internal(String),

    #[error("ERR-SYS:002 not implemented: {0}")]
    NotImplemented(String),

    #[error("ERR-SYS:003 timeout: {0}")]
    Timeout(String),

    #[error("ERR-SYS:004 cancelled")]
    Cancelled,

    #[error("ERR-SYS:005 io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ERR-SYS:006 plugin crashed: {0}")]
    PluginCrashed(String),

    #[error("ERR-SYS:007 configuration invalid: {0}")]
    Config(String),
}

/// エラーカテゴリ。INC-07 error_mapping に従う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// 業務ロジック起因 (ERR-BIZ-XXX)
    Biz,
    /// 外部システム起因 (ERR-EXT-XXX)
    Ext,
    /// システム / 内部起因 (ERR-SYS-XXX)
    Sys,
}

impl ErrorCategory {
    /// HTTP ステータスコードへの簡易マッピング (Adapter 層で使用)。
    #[must_use]
    pub const fn into_http_status(self) -> u16 {
        match self {
            Self::Biz => 400,
            Self::Ext => 502,
            Self::Sys => 500,
        }
    }
}

/// 機械可読エラーコード (例: `ERR-BIZ-004`)。`code()` で取得。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorCode(String);

impl ErrorCode {
    /// カテゴリを取り出す。
    #[must_use]
    pub fn category(&self) -> Option<ErrorCategory> {
        if self.0.starts_with("ERR-BIZ") {
            Some(ErrorCategory::Biz)
        } else if self.0.starts_with("ERR-EXT") {
            Some(ErrorCategory::Ext)
        } else if self.0.starts_with("ERR-SYS") {
            Some(ErrorCategory::Sys)
        } else {
            None
        }
    }

    /// リトライ可能かどうかのヒント。
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        // Provider/Network/Timeout は再試行候補、Validation/Authz は再試行不可
        let s: &str = &self.0;
        s.starts_with("ERR-EXT") || s.starts_with("ERR-SYS:003")
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl CoreError {
    /// 機械可読エラーコード (例: `ERR-BIZ:004`)。HTTP Adapter などで利用。
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        let raw = match self {
            Self::Validation(_) => "ERR-BIZ:001",
            Self::AuthzDenied(_) => "ERR-BIZ:002",
            Self::SessionNotFound(_) => "ERR-BIZ:003",
            Self::CapabilityNotFound(_) => "ERR-BIZ:004",
            Self::CommandNotFound(_) => "ERR-BIZ:005",
            Self::CommandPermissionDenied(_) => "ERR-BIZ:006",
            Self::InvalidArgument(_) => "ERR-BIZ:010",
            Self::Conflict(_) => "ERR-BIZ:011",
            Self::Transaction(_) => "ERR-BIZ:012",
            Self::Provider(_) => "ERR-EXT:001",
            Self::Network(_) => "ERR-EXT:002",
            Self::Serialization(_) => "ERR-EXT:003",
            Self::Internal(_) => "ERR-SYS:001",
            Self::NotImplemented(_) => "ERR-SYS:002",
            Self::Timeout(_) => "ERR-SYS:003",
            Self::Cancelled => "ERR-SYS:004",
            Self::Io(_) => "ERR-SYS:005",
            Self::PluginCrashed(_) => "ERR-SYS:006",
            Self::Config(_) => "ERR-SYS:007",
        };
        ErrorCode(raw.into())
    }

    /// リトライ可能か。
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.code().is_retryable()
    }

    /// HTTP ステータスコード。
    #[must_use]
    pub fn into_http_status(&self) -> u16 {
        self.code()
            .category()
            .map(ErrorCategory::into_http_status)
            .unwrap_or(500)
    }
}

/// `Result<T, CoreError>` のエイリアス。Kernel 全体の public API で使う。
pub type CoreResult<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_has_biz_code() {
        let e = CoreError::Validation("bad input".into());
        assert_eq!(e.code().to_string(), "ERR-BIZ:001");
        assert_eq!(e.code().category(), Some(ErrorCategory::Biz));
        assert_eq!(e.into_http_status(), 400);
        assert!(!e.is_retryable());
    }

    #[test]
    fn capability_not_found_maps_to_404() {
        let e = CoreError::CapabilityNotFound("text.search".into());
        assert_eq!(e.code().to_string(), "ERR-BIZ:004");
        assert_eq!(e.into_http_status(), 400);
    }

    #[test]
    fn network_error_is_retryable() {
        let e = CoreError::Network("timeout".into());
        assert!(e.is_retryable());
        assert_eq!(e.into_http_status(), 502);
    }

    #[test]
    fn io_error_conversion_works() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e: CoreError = io.into();
        assert_eq!(e.code().to_string(), "ERR-SYS:005");
    }

    #[test]
    fn cancelled_has_no_payload() {
        let e = CoreError::Cancelled;
        assert_eq!(e.code().to_string(), "ERR-SYS:004");
    }

    #[test]
    fn display_includes_code_and_payload() {
        let e = CoreError::SessionNotFound("S-abc".into());
        let s = format!("{e}");
        assert!(s.contains("ERR-BIZ:003"));
        assert!(s.contains("S-abc"));
    }
}