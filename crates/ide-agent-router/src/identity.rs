//! 语义单元模型与稳定身份指纹算法 (DD-ROUTER-SEM)
//!
//! 行号和字节偏移在代码被他人修改后会发生漂移，
//! 语义单元使用限定符号路径、语法节点类型、结构签名和内容哈希组合生成稳定的全局身份。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupportedLanguage {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    C,
    PowerShell,
    Fallback,
}

impl SupportedLanguage {
    pub fn from_file_path(path: &str) -> Self {
        if path.ends_with(".rs") {
            Self::Rust
        } else if path.ends_with(".py") {
            Self::Python
        } else if path.ends_with(".ts") || path.ends_with(".tsx") {
            Self::TypeScript
        } else if path.ends_with(".js") || path.ends_with(".jsx") {
            Self::JavaScript
        } else if path.ends_with(".c") || path.ends_with(".h") {
            Self::C
        } else if path.ends_with(".ps1") || path.ends_with(".psm1") {
            Self::PowerShell
        } else {
            Self::Fallback
        }
    }
}

/// 语义单元元数据
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticUnit {
    pub workspace_id: String,
    pub file_id: String,
    pub language: SupportedLanguage,
    pub qualified_symbol_path: String,
    pub node_kind: String,
    pub signature_hash: String,
    pub base_content_hash: String,
    pub parent_fingerprint: String,
    pub node_fingerprint: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

impl SemanticUnit {
    pub fn compute_hash(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn compute_fingerprint(
        workspace_id: &str,
        file_id: &str,
        language: SupportedLanguage,
        qualified_symbol_path: &str,
        node_kind: &str,
        signature_hash: &str,
        base_content_hash: &str,
        parent_fingerprint: &str,
    ) -> String {
        let composite = format!(
            "{}:{}:{:?}:{}:{}:{}:{}:{}",
            workspace_id,
            file_id,
            language,
            qualified_symbol_path,
            node_kind,
            signature_hash,
            base_content_hash,
            parent_fingerprint
        );
        Self::compute_hash(&composite)
    }
}
