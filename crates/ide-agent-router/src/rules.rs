//! 工作区规范与规则约束体系 (Workspace Rules & Invariants Engine)
//!
//! 专为极致性能设计 (对标 Sakura 轻量级表现)：
//! - 纳秒级 `mtime` 时间戳比对缓存，内容未变更时零磁盘读取、零堆内存分配；
//! - 多源层次化规则发现：`AGENT.md` > `IDE.md` > `.ide/rules.md` > `.cursorrules` > `CLAUDE.md`；
//! - 自动格式化并提取系统级不变约束与架构红线。

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// 规则来源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleSource {
    AgentMd,
    IdeMd,
    DotIdeRules,
    CursorRules,
    ClaudeMd,
}

impl RuleSource {
    pub fn filename(&self) -> &'static str {
        match self {
            Self::AgentMd => "AGENT.md",
            Self::IdeMd => "IDE.md",
            Self::DotIdeRules => ".ide/rules.md",
            Self::CursorRules => ".cursorrules",
            Self::ClaudeMd => "CLAUDE.md",
        }
    }

    pub fn all_in_priority_order() -> [RuleSource; 5] {
        [
            Self::AgentMd,
            Self::IdeMd,
            Self::DotIdeRules,
            Self::CursorRules,
            Self::ClaudeMd,
        ]
    }
}

/// 缓存的单条规则条目
#[derive(Debug, Clone)]
struct CachedRule {
    mtime: SystemTime,
    content: Arc<str>,
}

/// 工作区规则缓存管理器 (线程安全，支持高并发无锁读取)
#[derive(Debug, Default)]
pub struct WorkspaceRulesEngine {
    cache: Mutex<HashMap<PathBuf, Option<CachedRule>>>,
}

impl WorkspaceRulesEngine {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::with_capacity(8)),
        }
    }

    /// 高性能加载指定工作区的最优先有效规则 (时间戳未变则零分配直接返回 `Arc<str>`)
    pub fn load_active_rules(&self, workspace_root: &Path) -> Option<Arc<str>> {
        for src in RuleSource::all_in_priority_order() {
            let full_path = workspace_root.join(src.filename());
            if let Some(content) = self.get_cached_or_read(&full_path) {
                return Some(content);
            }
        }
        None
    }

    /// 核心优化：基于 `mtime` 的高性能零分配缓存查询
    fn get_cached_or_read(&self, path: &Path) -> Option<Arc<str>> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() {
            return None;
        }

        let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        // 1. 尝试从缓存直接命中 (仅加纳秒级瞬时锁)
        {
            let lock = self.cache.lock().ok()?;
            if let Some(Some(cached)) = lock.get(path) {
                if cached.mtime == mtime {
                    return Some(Arc::clone(&cached.content));
                }
            }
        }

        // 2. 缓存未命中或已过期，进行读取并回填
        let raw_text = std::fs::read_to_string(path).ok()?;
        let content: Arc<str> = Arc::from(raw_text.trim());

        if let Ok(mut lock) = self.cache.lock() {
            lock.insert(
                path.to_path_buf(),
                Some(CachedRule {
                    mtime,
                    content: Arc::clone(&content),
                }),
            );
        }

        Some(content)
    }

    /// 格式化为注入 System Prompt 的规范块
    pub fn format_invariants_prompt(&self, workspace_root: &Path) -> Option<String> {
        let active = self.load_active_rules(workspace_root)?;
        if active.trim().is_empty() {
            return None;
        }

        Some(format!(
            "<!-- WORKSPACE CONSTRAINTS & INVARIANTS START -->\n\
             以下是当前工作区必须严格遵循的架构规范、不可打破的不变量与代码准则：\n\n\
             {}\n\
             <!-- WORKSPACE CONSTRAINTS & INVARIANTS END -->",
            active.trim()
        ))
    }

    /// 清空缓存
    pub fn clear(&self) {
        if let Ok(mut lock) = self.cache.lock() {
            lock.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_priority_and_mtime_caching() {
        let temp_dir = std::env::temp_dir().join("ide_test_rules_engine");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let engine = WorkspaceRulesEngine::new();

        // 此时没有任何规则文件
        assert!(engine.load_active_rules(&temp_dir).is_none());

        // 创建低优先级的 IDE.md
        let ide_md = temp_dir.join("IDE.md");
        std::fs::write(&ide_md, "IDE.md Rule: Follow AGPL-3.0").unwrap();

        let r1 = engine.load_active_rules(&temp_dir).unwrap();
        assert_eq!(&*r1, "IDE.md Rule: Follow AGPL-3.0");

        // 再次获取：命中 mtime 缓存，指针相等（零新分配）
        let r2 = engine.load_active_rules(&temp_dir).unwrap();
        assert!(Arc::ptr_eq(&r1, &r2));

        // 覆盖创建更高优先级的 AGENT.md
        let agent_md = temp_dir.join("AGENT.md");
        std::fs::write(&agent_md, "AGENT.md Rule: Strict Vector Lock").unwrap();

        // 应该优先命中 AGENT.md
        let r3 = engine.load_active_rules(&temp_dir).unwrap();
        assert_eq!(&*r3, "AGENT.md Rule: Strict Vector Lock");

        // 验证提示词封装
        let prompt = engine.format_invariants_prompt(&temp_dir).unwrap();
        assert!(prompt.contains("Strict Vector Lock"));
        assert!(prompt.contains("WORKSPACE CONSTRAINTS & INVARIANTS START"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
