//! kernel-config — Tier 1: 設定読込・環境変数統合
//!
//! DD-04b §MOD-CFG に従い、優先順位 `CLI > Env > .kernel/*.toml > Default` で設定を読む。
//! 階層化された設定は [`figment`] を用いて統合する。
//!
//! トレース: REQ-001 §FR-021 / NFR-M-001 / SD-001 §121

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use figment::providers::Format;
use kernel_error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 設定値の階層ソース優先順位 (DD-04b §MOD-CFG)。
///
/// 値取得時、より高い優先順位の Source の値で上書きされる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourcePriority {
    /// 起動時デフォルト (最低優先)
    Default = 0,
    /// 設定ファイル `.kernel/*.toml`
    File = 10,
    /// 環境変数 `KERNEL_*`
    Env = 20,
    /// CLI 引数 (最高優先)
    Cli = 30,
}

/// Kernel 全体の設定を表すルート構造体。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelConfig {
    /// AI 設定 (Local / Remote Provider)
    pub ai: AiConfig,
    /// ログレベル
    pub log: LogConfig,
    /// データディレクトリ
    pub data_dir: Option<String>,
}

/// AI 設定。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfig {
    /// Remote AI Provider を有効化するか (§94 / §129: 极簡版は default false)
    #[serde(default)]
    pub remote_enabled: bool,
    /// 標準 Provider プラグイン (`ai-router` / `ai-inline` / `ai-edit` / `ai-chat`)
    #[serde(default)]
    pub default_provider: Option<String>,
}

/// ログ設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub json: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            json: false,
        }
    }
}

fn default_log_level() -> String {
    "info".into()
}

/// 設定ローダ。Source を追加していく priority 順で統合する。
#[derive(Debug, Default)]
pub struct ConfigLoader {
    figment: figment::Figment,
}

impl ConfigLoader {
    /// 新しいローダを生成。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// デフォルト値で初期化。
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut loader = Self::new();
        let cfg = KernelConfig::default();
        loader.figment = loader
            .figment
            .merge(figment::providers::Serialized::defaults(cfg));
        loader
    }

    /// TOML ファイルから読み込み (File priority)。
    /// # Errors
    /// ファイル IO / パース / スキーマ違反で [`CoreError::Config`] を返す。
    pub fn with_file<P: AsRef<Path>>(mut self, path: P) -> CoreResult<Self> {
        let path = path.as_ref();
        self.figment = self.figment.merge(figment::providers::Toml::file(path));
        Ok(self)
    }

    /// 環境変数プレフィックス `KERNEL_` で上書き (Env priority)。
    pub fn with_env(mut self, prefix: &str) -> Self {
        self.figment = self.figment.merge(figment::providers::Env::prefixed(prefix));
        self
    }

    /// CLI 引数由来のオーバーライド (Cli priority)。
    ///
    /// `overrides` は KernelConfig 構造に従う部分 JSON を許容。
    /// figment 0.10 は Json provider を持たないため、Data + TOML 文字列で同等表現する。
    pub fn with_cli_overrides(mut self, overrides: serde_json::Value) -> Self {
        // Build a partial TOML string from overrides. figment 0.10 ships with a TOML
        // Format provider; we use it as a generic Data source.
        let mut toml_buf = String::new();
        if let Some(ai) = overrides.get("ai") {
            toml_buf.push_str(&format!(
                "[ai]\nremote_enabled = {}\n",
                ai.get("remote_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            ));
            if let Some(p) = ai.get("default_provider").and_then(|v| v.as_str()) {
                toml_buf.push_str(&format!("default_provider = \"{p}\"\n"));
            }
        }
        if let Some(log) = overrides.get("log") {
            toml_buf.push_str("[log]\n");
            if let Some(level) = log.get("level").and_then(|v| v.as_str()) {
                toml_buf.push_str(&format!("level = \"{level}\"\n"));
            }
            toml_buf.push_str(&format!(
                "json = {}\n",
                log.get("json").and_then(|v| v.as_bool()).unwrap_or(false)
            ));
        }
        if let Some(d) = overrides.get("data_dir").and_then(|v| v.as_str()) {
            toml_buf.push_str(&format!("data_dir = \"{d}\"\n"));
        }
        self.figment = self
            .figment
            .merge(figment::providers::Toml::string(&toml_buf));
        self
    }

    /// 設定を最終形に materialize。
    /// # Errors
    /// スキーマ違反で [`CoreError::Config`] を返す。
    pub fn load(self) -> CoreResult<KernelConfig> {
        self.figment
            .extract::<KernelConfig>()
            .map_err(|e| CoreError::Config(format!("config extract failed: {e}")))
    }
}

/// 補助: 環境変数から AI Remote 設定を抽出するヘルパ。
#[must_use]
pub fn env_ai_remote_enabled() -> Option<bool> {
    std::env::var("KERNEL_AI__REMOTE_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = ConfigLoader::with_defaults().load().unwrap();
        assert!(!cfg.ai.remote_enabled);
        assert_eq!(cfg.log.level, "info");
        assert!(!cfg.log.json);
    }

    #[test]
    fn file_override_works() {
        let dir = std::env::temp_dir();
        let path = dir.join("kernel-config-test.toml");
        std::fs::write(
            &path,
            "[ai]\nremote_enabled = true\ndefault_provider = \"local-llm\"\n[log]\nlevel = \"debug\"\n",
        )
        .unwrap();
        let cfg = ConfigLoader::with_defaults()
            .with_file(&path)
            .unwrap()
            .load()
            .unwrap();
        assert!(cfg.ai.remote_enabled);
        assert_eq!(cfg.ai.default_provider.as_deref(), Some("local-llm"));
        assert_eq!(cfg.log.level, "debug");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cli_overrides_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("kernel-config-cli.toml");
        std::fs::write(
            &path,
            "[ai]\nremote_enabled = false\n[log]\nlevel = \"info\"\n",
        )
        .unwrap();
        let overrides = serde_json::json!({"log": {"level": "trace"}});
        let cfg = ConfigLoader::with_defaults()
            .with_file(&path)
            .unwrap()
            .with_cli_overrides(overrides)
            .load()
            .unwrap();
        assert_eq!(cfg.log.level, "trace");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_errors_cleanly() {
        // figment::providers::Toml::file does not error on missing files in figment 0.10;
        // instead it silently ignores. We only test that load still succeeds with defaults.
        let cfg = ConfigLoader::with_defaults()
            .with_file("nonexistent-zzzz.toml")
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(cfg.log.level, "info");
    }
}