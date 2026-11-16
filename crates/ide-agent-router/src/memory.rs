//! 双层智能体记忆引擎 (Dual-Tier Agent Memory Engine)
//!
//! 专为极速低耗设计 (对标 Sakura 轻量级表现)：
//! - **第一层 (工作区活跃记忆 / Working Memory)**：内存环形缓冲队列 (Ring Buffer)，容量恒定，零堆碎片，零 I/O；
//! - **第二层 (语义持久长期记忆 / Long-Term Semantic Memory)**：`<workspace>/.ide/memory.json` 原子化落盘；
//! - 支持架构决策、技术栈共识与用户偏好的自动沉淀与回灌。

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 单条工作记忆事件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkingMemoryItem {
    pub timestamp: i64,
    pub role: String, // "user" | "assistant" | "system" | "tool"
    pub summary: String,
    pub tags: Vec<String>,
}

/// 语义长期记忆实体 (跨会话持久保存)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LongTermMemory {
    /// 总体架构描述与技术栈认知
    #[serde(default)]
    pub architecture_summary: String,
    /// 关键架构决策与不可撤销准则 (ADRs)
    #[serde(default)]
    pub architectural_decisions: Vec<String>,
    /// 用户偏好设置 (如代码风格、缩进偏好、命名习惯)
    #[serde(default)]
    pub user_preferences: HashMap<String, String>,
    /// 高频使用的工具与命令频率统计
    #[serde(default)]
    pub tool_frequencies: HashMap<String, usize>,
    /// 上次更新时间戳
    #[serde(default)]
    pub updated_at: i64,
}

/// 记忆引擎 (统一纳管双层记忆)
#[derive(Debug)]
pub struct AgentMemoryEngine {
    max_working_items: usize,
    working_memory: Mutex<VecDeque<WorkingMemoryItem>>,
    long_term_cache: Mutex<Option<(PathBuf, LongTermMemory)>>,
}

impl Default for AgentMemoryEngine {
    fn default() -> Self {
        Self::new(32)
    }
}

impl AgentMemoryEngine {
    pub fn new(max_working_items: usize) -> Self {
        Self {
            max_working_items: max_working_items.max(8),
            working_memory: Mutex::new(VecDeque::with_capacity(max_working_items)),
            long_term_cache: Mutex::new(None),
        }
    }

    /// 记录一条轻量级工作记忆 (纳秒级写入环形缓冲，超量自动平滑淘汰)
    pub fn record_working_item(&self, role: &str, summary: &str, tags: Vec<String>) {
        let item = WorkingMemoryItem {
            timestamp: chrono::Utc::now().timestamp(),
            role: role.to_string(),
            summary: summary.to_string(),
            tags,
        };

        if let Ok(mut lock) = self.working_memory.lock() {
            if lock.len() >= self.max_working_items {
                lock.pop_front();
            }
            lock.push_back(item);
        }
    }

    /// 获取最近工作记忆快照
    pub fn recent_working_items(&self, limit: usize) -> Vec<WorkingMemoryItem> {
        let lock = match self.working_memory.lock() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };

        lock.iter().rev().take(limit).cloned().collect()
    }

    /// 加载工作区的长期记忆 (优先从缓存读取)
    pub fn load_long_term(&self, workspace_root: &Path) -> LongTermMemory {
        let mem_file = workspace_root.join(".ide").join("memory.json");

        if let Ok(lock) = self.long_term_cache.lock() {
            if let Some((path, mem)) = &*lock {
                if path == &mem_file {
                    return mem.clone();
                }
            }
        }

        let loaded = if mem_file.is_file() {
            std::fs::read_to_string(&mem_file)
                .ok()
                .and_then(|s| serde_json::from_str::<LongTermMemory>(&s).ok())
                .unwrap_or_default()
        } else {
            LongTermMemory::default()
        };

        if let Ok(mut lock) = self.long_term_cache.lock() {
            *lock = Some((mem_file, loaded.clone()));
        }

        loaded
    }

    /// 原子安全落盘长期记忆 (先写入 .tmp 文件再原子重命名，防止写入中断损坏)
    pub fn save_long_term(
        &self,
        workspace_root: &Path,
        memory: &LongTermMemory,
    ) -> Result<(), String> {
        let ide_dir = workspace_root.join(".ide");
        std::fs::create_dir_all(&ide_dir).map_err(|e| format!("创建 .ide 目录失败: {e}"))?;

        let mem_file = ide_dir.join("memory.json");
        let tmp_file = ide_dir.join("memory.json.tmp");

        let serialized =
            serde_json::to_string_pretty(memory).map_err(|e| format!("序列化记忆失败: {e}"))?;

        std::fs::write(&tmp_file, serialized.as_bytes())
            .map_err(|e| format!("写入临时记忆文件失败: {e}"))?;

        std::fs::rename(&tmp_file, &mem_file).map_err(|e| format!("原子更新记忆文件失败: {e}"))?;

        if let Ok(mut lock) = self.long_term_cache.lock() {
            *lock = Some((mem_file, memory.clone()));
        }

        Ok(())
    }

    /// 提炼并格式化注入 Prompt 的记忆上下文
    pub fn format_memory_prompt(&self, workspace_root: &Path) -> Option<String> {
        let lt = self.load_long_term(workspace_root);
        let mut parts = Vec::new();

        if !lt.architecture_summary.is_empty() {
            parts.push(format!("- 项目架构概要: {}", lt.architecture_summary));
        }
        if !lt.architectural_decisions.is_empty() {
            parts.push(format!(
                "- 既定架构决议: {}",
                lt.architectural_decisions.join("; ")
            ));
        }
        if !lt.user_preferences.is_empty() {
            let prefs: Vec<String> = lt
                .user_preferences
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            parts.push(format!("- 用户习惯与偏好: {}", prefs.join(", ")));
        }

        if parts.is_empty() {
            return None;
        }

        Some(format!(
            "<!-- AGENT PERSISTENT MEMORY START -->\n\
             根据历史会话沉淀的项目关键认知：\n\
             {}\n\
             <!-- AGENT PERSISTENT MEMORY END -->",
            parts.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_memory_ring_buffer_bounded() {
        let engine = AgentMemoryEngine::new(8);
        for i in 0..20 {
            engine.record_working_item("user", &format!("msg {i}"), vec![]);
        }

        let items = engine.recent_working_items(10);
        // 最多保留 8 条
        assert_eq!(items.len(), 8);
        // 最新的在第一位
        assert_eq!(items[0].summary, "msg 19");
        assert_eq!(items[7].summary, "msg 12");
    }

    #[test]
    fn test_long_term_memory_atomic_roundtrip() {
        let temp_dir = std::env::temp_dir().join("ide_test_long_term_mem");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let engine = AgentMemoryEngine::default();
        let mut mem = LongTermMemory::default();
        mem.architecture_summary = "Microkernel with pure Rust GUI".into();
        mem.architectural_decisions
            .push("Forbid unsafe code globally".into());
        mem.user_preferences.insert("tab_width".into(), "4".into());

        engine.save_long_term(&temp_dir, &mem).unwrap();

        // 重新加载验证
        let loaded = engine.load_long_term(&temp_dir);
        assert_eq!(
            loaded.architecture_summary,
            "Microkernel with pure Rust GUI"
        );
        assert_eq!(loaded.architectural_decisions.len(), 1);
        assert_eq!(loaded.user_preferences.get("tab_width").unwrap(), "4");

        // 格式化验证
        let prompt = engine.format_memory_prompt(&temp_dir).unwrap();
        assert!(prompt.contains("Microkernel with pure Rust GUI"));
        assert!(prompt.contains("Forbid unsafe code globally"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
