//! AST-aware Patch 应用与锚点重定位引擎 (DD-ROUTER-SEM)
//!
//! 在多 Agent 并行开发中，当两个 Agent 分别修改同一个文件的不同函数时，
//! 后提交的 Agent 基础 revision 虽落后，但目标函数的写足迹未被改动。
//! PatchEngine 重新解析最新 AST，精确定位目标函数新偏移并安全合并。

use crate::identity::{SemanticUnit, SupportedLanguage};
use crate::parser::SemanticParser;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PatchError {
    #[error("Target symbol '{0}' not found in current source")]
    SymbolNotFound(String),

    #[error("Semantic write conflict on symbol '{symbol}': {reason}")]
    SemanticWriteConflict { symbol: String, reason: String },

    #[error("Invalid target byte range")]
    InvalidRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchProposal {
    pub task_id: String,
    pub worker_id: String,
    pub fencing_token: u64,
    pub file_id: String,
    pub base_revision: u64,
    pub symbol_path: String,
    pub expected_fingerprint: Option<String>,
    pub replacement_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchResult {
    pub new_content: String,
    pub previous_revision: u64,
    pub new_revision: u64,
    pub relocated: bool,
    pub updated_unit: SemanticUnit,
}

pub struct PatchEngine;

impl PatchEngine {
    /// 校验是否属于无并发提交的快速路径
    pub fn can_fast_forward(current_revision: u64, base_revision: u64) -> bool {
        current_revision == base_revision
    }

    /// 应用补丁：支持快速路径和并发 AST-aware 自动重定位
    pub fn relocate_and_apply(
        workspace_id: &str,
        file_id: &str,
        current_content: &str,
        current_revision: u64,
        language: SupportedLanguage,
        proposal: &PatchProposal,
    ) -> Result<PatchResult, PatchError> {
        let is_fast_path = Self::can_fast_forward(current_revision, proposal.base_revision);

        // 重新解析最新代码的语义单元
        let units = SemanticParser::parse(workspace_id, file_id, current_content, language);

        // 查找目标符号
        let target_unit = units
            .iter()
            .find(|u| u.qualified_symbol_path == proposal.symbol_path)
            .ok_or_else(|| PatchError::SymbolNotFound(proposal.symbol_path.clone()))?;

        // 若不是快速路径（说明有其他 Worker 在并发中合入了其他修改）
        let relocated = if !is_fast_path {
            // 校验目标函数的指纹是否已被并发修改改动
            if let Some(expected_fp) = &proposal.expected_fingerprint {
                if &target_unit.node_fingerprint != expected_fp {
                    return Err(PatchError::SemanticWriteConflict {
                        symbol: proposal.symbol_path.clone(),
                        reason: "Target symbol body was concurrently modified by another commit".to_string(),
                    });
                }
            }
            true
        } else {
            false
        };

        let start = target_unit.start_byte;
        let end = target_unit.end_byte;

        if start > end || end > current_content.len() {
            return Err(PatchError::InvalidRange);
        }

        // 构建合并后的新文本
        let mut new_content = String::with_capacity(
            current_content.len() + proposal.replacement_text.len() - (end - start),
        );
        new_content.push_str(&current_content[..start]);
        new_content.push_str(&proposal.replacement_text);
        new_content.push_str(&current_content[end..]);

        // 计算更新后的单元信息
        let updated_units = SemanticParser::parse(workspace_id, file_id, &new_content, language);
        let updated_unit = updated_units
            .into_iter()
            .find(|u| u.qualified_symbol_path == proposal.symbol_path)
            .unwrap_or_else(|| target_unit.clone());

        Ok(PatchResult {
            new_content,
            previous_revision: current_revision,
            new_revision: current_revision + 1,
            relocated,
            updated_unit,
        })
    }
}
