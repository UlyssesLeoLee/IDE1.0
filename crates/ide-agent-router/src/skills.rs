//! 高性能 AI 技能体系 (Skill System)
//!
//! 提供结构化、可复用、可插拔的专业 AI 技能：
//! - 内建 6 大高频研发技能（代码审查、安全重构、单测生成、NEMA 拓扑优化、Git 提交提炼、架构拆解），零 I/O 冷启常驻；
//! - 支持用户 / 工作区自定义技能（SKILL.md / JSON 清单）按需惰性扫描，零帧渲染开销；
//! - 统一支持 Slash Command 快捷触发（如 `/review`、`/test`、`/nema`、`/refactor` 等）；
//! - 自动绑定模型系统提示词与所需工具链（内建工具、插件工具与 MCP 工具）。

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 单个 AI 技能定义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    /// 技能唯一标识符 (例如 "code-review", "nema-optimize")
    pub id: String,
    /// 界面显示名称
    pub name: String,
    /// 技能功能说明
    pub description: String,
    /// 触发命令列表 (以斜杠开头，如 ["/review", "/cr"])
    pub triggers: Vec<String>,
    /// 标签分类
    pub tags: Vec<String>,
    /// 注入至 LLM 的专业系统提示词与角色约束
    pub system_prompt: String,
    /// 提示词格式化模板 (可选，支持 `{input}` 占位符)
    pub prompt_template: Option<String>,
    /// 执行该技能所需的工具列表 (内建 / 插件 / MCP)
    pub required_tools: Vec<String>,
    /// 是否为只读内建技能
    pub is_builtin: bool,
}

impl Skill {
    /// 构建基础自定义技能
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        triggers: Vec<String>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            triggers,
            tags: Vec::new(),
            system_prompt: system_prompt.into(),
            prompt_template: None,
            required_tools: Vec::new(),
            is_builtin: false,
        }
    }

    /// 应用技能提示词生成最终的系统提示词与用户输入
    pub fn apply(&self, user_input: &str, base_system: Option<&str>) -> (String, String) {
        let final_system = match base_system {
            Some(base) if !base.trim().is_empty() => {
                format!(
                    "{}\n\n[Active Skill: {}]\n{}",
                    base.trim(),
                    self.name,
                    self.system_prompt.trim()
                )
            }
            _ => format!(
                "[Active Skill: {}]\n{}",
                self.name,
                self.system_prompt.trim()
            ),
        };

        let final_user = match &self.prompt_template {
            Some(tmpl) if tmpl.contains("{input}") => tmpl.replace("{input}", user_input.trim()),
            Some(tmpl) => format!("{}\n\n{}", tmpl.trim(), user_input.trim()),
            None => user_input.trim().to_string(),
        };

        (final_system, final_user)
    }
}

/// 技能注册表 (管理内建与工作区自定义技能)
#[derive(Debug, Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    trigger_map: HashMap<String, String>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    /// 创建并预加载内建核心技能 (零磁盘 I/O，启动瞬间就绪)
    pub fn new() -> Self {
        let mut reg = Self {
            skills: HashMap::with_capacity(16),
            trigger_map: HashMap::with_capacity(32),
        };
        reg.register_builtin_skills();
        reg
    }

    /// 注册内置核心研发技能
    fn register_builtin_skills(&mut self) {
        let builtins = vec![
            Skill {
                id: "code-review".into(),
                name: "代码审查 (Code Review)".into(),
                description: "执行高标准的代码质量审查，深入检查逻辑缺陷、安全边界、性能隐患与可维护性".into(),
                triggers: vec!["/review".into(), "/cr".into()],
                tags: vec!["review".into(), "quality".into(), "security".into()],
                system_prompt: "你是一位严谨的 Staff Engineer。针对提交的代码片段进行对抗性审查。\n重点排查：\n1. 不变量是否打破，状态转移是否完备；\n2. 潜在内存泄漏、死锁、并发竞态与异常边界；\n3. 资源是否显式释放；\n4. 输出清晰有理有据的审查意见，给出具体修改建议与代码 diff。".into(),
                prompt_template: Some("请对以下代码进行深度代码审查：\n\n```\n{input}\n```".into()),
                required_tools: vec![],
                is_builtin: true,
            },
            Skill {
                id: "refactor".into(),
                name: "架构重构 (Refactor)".into(),
                description: "在保持既有语义与 AST 不变量的前提下，进行模块化解耦、坏味道清除与架构精简".into(),
                triggers: vec!["/refactor".into(), "/clean".into()],
                tags: vec!["refactor".into(), "clean-code".into(), "architecture".into()],
                system_prompt: "你是一位系统重构专家。遵循 Martin Fowler 重构规范。\n原则：保持外部行为严格不变，最小化侵入性改动，消除重复代码与坏味道，增强类型安全。".into(),
                prompt_template: Some("请在保持功能不变的前提下对以下代码进行优雅重构：\n\n```\n{input}\n```".into()),
                required_tools: vec![],
                is_builtin: true,
            },
            Skill {
                id: "unit-test".into(),
                name: "单测生成 (Unit Test)".into(),
                description: "自动推导边界条件、Happy Path 与异常分支，生成高覆盖率的确定性单元测试".into(),
                triggers: vec!["/test".into(), "/ut".into()],
                tags: vec!["test".into(), "qa".into(), "coverage".into()],
                system_prompt: "你是一位精通 TDD 的自动化测试架构师。针对目标代码编写完备的单元测试用例。\n要求：包含正常输入、极限边界条件、错误与无效输入防御，使用原子断言。".into(),
                prompt_template: Some("请为以下实现编写高覆盖率的单元测试：\n\n```\n{input}\n```".into()),
                required_tools: vec![],
                is_builtin: true,
            },
            Skill {
                id: "nema-optimize".into(),
                name: "NEMA 拓扑优化 (NEMA Token Optimizer)".into(),
                description: "深入分析文档词元与结构重用性，固化高频词典引用，最大化 NEMA Token 压缩效益".into(),
                triggers: vec!["/nema".into(), "/compress".into()],
                tags: vec!["nema".into(), "compression".into(), "tokens".into()],
                system_prompt: "你是 NEMA 拓扑压缩与词典编码算法专家。\n任务：识别文档高频词元序列，构建紧凑拓扑字典，使用 `^N` 引用和制表符意群对齐消除冗余 Token。".into(),
                prompt_template: Some("请分析以下文本/数据，提取高频复用模式并输出优化后的 NEMA 编码方案：\n\n{input}".into()),
                required_tools: vec!["c.nema-convert".into()],
                is_builtin: true,
            },
            Skill {
                id: "git-commit".into(),
                name: "Git 提交提炼 (Git Commit)".into(),
                description: "分析代码变更差异 (Diff)，提炼符合 Conventional Commits 规范的精准提交信息".into(),
                triggers: vec!["/commit".into(), "/gc".into()],
                tags: vec!["git".into(), "scm".into(), "commit".into()],
                system_prompt: "你是 SCM 变更总结专家。遵循 Conventional Commits 规范 (feat, fix, refactor, test, docs, chore)。\n输出格式：一行动作概要（不超过 72 字符）+ 详细点列变更说明。".into(),
                prompt_template: Some("请根据以下 Git Diff 变更提取结构化提交信息：\n\n```diff\n{input}\n```".into()),
                required_tools: vec![],
                is_builtin: true,
            },
            Skill {
                id: "explain".into(),
                name: "架构解说 (Explain)".into(),
                description: "以分布式架构师视角深入解说系统状态流转、并发模型与设计权衡".into(),
                triggers: vec!["/explain".into(), "/doc".into()],
                tags: vec!["explain".into(), "architecture".into(), "learning".into()],
                system_prompt: "你是一位分布式系统架构师。使用由浅入深、图文并茂的结构解说目标代码的内部机制、数据流向、时序状态与设计考量。".into(),
                prompt_template: Some("请详细剖析并解说以下实现的工作原理与设计权衡：\n\n```\n{input}\n```".into()),
                required_tools: vec![],
                is_builtin: true,
            },
        ];

        for s in builtins {
            self.register(s);
        }
    }

    /// 注册一个技能
    pub fn register(&mut self, skill: Skill) {
        for trig in &skill.triggers {
            let key = trig.trim().to_ascii_lowercase();
            if !key.is_empty() {
                self.trigger_map.insert(key, skill.id.clone());
            }
        }
        self.skills.insert(skill.id.clone(), skill);
    }

    /// 按 ID 检索技能
    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id)
    }

    /// 按触发词检索技能 (如 "/review")
    pub fn find_by_trigger(&self, trigger: &str) -> Option<&Skill> {
        let key = trigger.trim().to_ascii_lowercase();
        self.trigger_map
            .get(&key)
            .and_then(|id| self.skills.get(id))
    }

    /// 检查用户输入是否以某个触发词开头 (如 "/review 请看这行代码")
    /// 返回匹配到的技能引用以及剔除触发词后的剩余输入正文
    pub fn match_prompt<'a>(&'a self, prompt: &'a str) -> Option<(&'a Skill, &'a str)> {
        let trimmed = prompt.trim_start();
        if !trimmed.starts_with('/') {
            return None;
        }

        let first_word = trimmed.split_whitespace().next()?;
        if let Some(skill) = self.find_by_trigger(first_word) {
            let rest = trimmed[first_word.len()..].trim_start();
            return Some((skill, rest));
        }
        None
    }

    /// 兼容 Pre-Input Hook 前置提示词注入后的触发词匹配（如 `"[Custom Hook]\n/review check"`）
    /// 返回 `(匹配技能, 保留前置注入并与触发词后正文拼接的新提示词)`
    pub fn match_prompt_with_prefix<'a>(&'a self, prompt: &'a str) -> Option<(&'a Skill, String)> {
        if let Some((skill, rest)) = self.match_prompt(prompt) {
            return Some((skill, rest.to_string()));
        }
        let mut offset = 0usize;
        for line in prompt.split_inclusive('\n') {
            let line_trimmed = line.trim_start();
            if line_trimmed.starts_with('/') {
                if let Some(first_word) = line_trimmed.split_whitespace().next() {
                    if let Some(skill) = self.find_by_trigger(first_word) {
                        let prefix = prompt[..offset].trim_end();
                        let leading_ws = line.len() - line_trimmed.len();
                        let after_trigger =
                            prompt[offset + leading_ws + first_word.len()..].trim_start();
                        let combined = if prefix.is_empty() {
                            after_trigger.to_string()
                        } else if after_trigger.is_empty() {
                            prefix.to_string()
                        } else {
                            format!("{prefix}\n{after_trigger}")
                        };
                        return Some((skill, combined));
                    }
                }
            }
            offset += line.len();
        }
        None
    }

    /// 列出所有技能清单 (内建排在前面，保持确定性排序)
    pub fn list(&self) -> Vec<&Skill> {
        let mut list: Vec<&Skill> = self.skills.values().collect();
        list.sort_by(|a, b| {
            b.is_builtin
                .cmp(&a.is_builtin)
                .then_with(|| a.id.cmp(&b.id))
        });
        list
    }

    /// 模糊/包含查询技能 (用于输入补全与快速检索)
    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return self.list();
        }
        let mut matches: Vec<&Skill> = self
            .skills
            .values()
            .filter(|s| {
                s.id.to_ascii_lowercase().contains(&q)
                    || s.name.to_ascii_lowercase().contains(&q)
                    || s.description.to_ascii_lowercase().contains(&q)
                    || s.triggers
                        .iter()
                        .any(|t| t.to_ascii_lowercase().contains(&q))
                    || s.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q))
            })
            .collect();

        matches.sort_by(|a, b| {
            b.is_builtin
                .cmp(&a.is_builtin)
                .then_with(|| a.id.cmp(&b.id))
        });
        matches
    }

    /// 从单个技能文件加载（支持 `SKILL.md`、任意 `<name>.md` 或 `<name>.json`，兼容自定义与第三方技能格式）
    pub fn load_from_file(&mut self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let Ok(content) = std::fs::read_to_string(path) else {
            return false;
        };
        if ext == "json" {
            if let Ok(mut skill) = serde_json::from_str::<Skill>(&content) {
                skill.is_builtin = false;
                self.register(skill);
                return true;
            }
            return false;
        }
        if ext == "md" {
            let id_source = if path
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("SKILL.md"))
            {
                path.parent().unwrap_or(path)
            } else {
                path
            };
            if let Some(skill) = parse_skill_markdown(id_source, &content) {
                self.register(skill);
                return true;
            }
        }
        false
    }

    /// 从目录中扫描自定义与第三方技能文件 (支持 `<dir>/<id>/SKILL.md`、`<dir>/*.md` 与 `<dir>/*.json`)
    /// 仅在显式调用时运行，绝不在启动主路径上阻塞
    pub fn load_from_dir(&mut self, dir: &Path) {
        if !dir.is_dir() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();

        for path in paths {
            if path.is_file() {
                let _ = self.load_from_file(&path);
            } else if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.is_file() {
                    let _ = self.load_from_file(&skill_md);
                }
            }
        }
    }

    /// 自动扫描工作区与用户全局目录下的所有自定义及第三方技能
    /// 支持目录：
    /// - 用户全局 `<user_dir>/custom_skills`
    /// - 工作区 `.skills`、`.claude/skills`、`.claude/commands`、`.agents/skills`、`skills`
    pub fn load_workspace_and_user_skills(&mut self, ws_root: &Path, user_dir: Option<&Path>) {
        if let Some(udir) = user_dir {
            self.load_from_dir(&udir.join("custom_skills"));
        }
        for rel in [
            ".skills",
            ".claude/skills",
            ".claude/commands",
            ".agents/skills",
            "skills",
        ] {
            self.load_from_dir(&ws_root.join(rel));
        }
    }
}

/// 解析带有 YAML frontmatter 的 `SKILL.md` 或独立 `<skill-id>.md`
fn parse_skill_markdown(id_path: &Path, content: &str) -> Option<Skill> {
    let raw_stem = if id_path.is_file() {
        id_path.file_stem()?.to_string_lossy().to_string()
    } else {
        id_path.file_name()?.to_string_lossy().to_string()
    };
    let folder_name = raw_stem.trim().to_string();
    if folder_name.is_empty() || folder_name.eq_ignore_ascii_case("README") {
        return None;
    }
    if !content.starts_with("---") {
        return Some(Skill::new(
            folder_name.clone(),
            folder_name.clone(),
            "Custom / 3rd-Party Skill",
            vec![format!("/{folder_name}")],
            content.to_string(),
        ));
    }

    let rest = &content[3..];
    let end_idx = rest.find("---")?;
    let frontmatter = &rest[..end_idx];
    let body = rest[end_idx + 3..].trim();

    let mut name = folder_name.clone();
    let mut description = "Workspace Skill".to_string();
    let mut triggers = vec![format!("/{}", folder_name)];
    let mut tags = Vec::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            match k.as_str() {
                "name" => name = v.to_string(),
                "description" => description = v.to_string(),
                "trigger" | "command" => {
                    triggers = vec![if v.starts_with('/') {
                        v.to_string()
                    } else {
                        format!("/{v}")
                    }]
                }
                "tags" => {
                    tags = v
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    Some(Skill {
        id: folder_name,
        name,
        description,
        triggers,
        tags,
        system_prompt: body.to_string(),
        prompt_template: None,
        required_tools: Vec::new(),
        is_builtin: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_present() {
        let reg = SkillRegistry::new();
        assert!(reg.get("code-review").is_some());
        assert!(reg.get("refactor").is_some());
        assert!(reg.get("unit-test").is_some());
        assert!(reg.get("nema-optimize").is_some());
        assert!(reg.get("git-commit").is_some());
        assert!(reg.get("explain").is_some());
        assert_eq!(reg.list().len(), 6);
    }

    #[test]
    fn test_trigger_matching() {
        let reg = SkillRegistry::new();

        // 匹配 /review
        let (s1, rest1) = reg
            .match_prompt("/review fn calculate() { return 42; }")
            .unwrap();
        assert_eq!(s1.id, "code-review");
        assert_eq!(rest1, "fn calculate() { return 42; }");

        // 匹配 /cr 别名
        let (s2, rest2) = reg.match_prompt("/cr pub struct Foo;").unwrap();
        assert_eq!(s2.id, "code-review");
        assert_eq!(rest2, "pub struct Foo;");

        // 匹配 /test
        let (s3, rest3) = reg.match_prompt("/test fn parse()").unwrap();
        assert_eq!(s3.id, "unit-test");
        assert_eq!(rest3, "fn parse()");

        // 非触发词输入返回 None
        assert!(reg.match_prompt("普通聊天问题").is_none());
        assert!(reg.match_prompt("/unknown-command test").is_none());
    }

    #[test]
    fn test_apply_skill_formatting() {
        let reg = SkillRegistry::new();
        let s = reg.get("code-review").unwrap();
        let (sys, user) = s.apply("let a = 1;", Some("Base System Instructions"));

        assert!(sys.contains("Base System Instructions"));
        assert!(sys.contains("Active Skill: 代码审查"));
        assert!(sys.contains("对抗性审查"));
        assert!(user.contains("深度代码审查"));
        assert!(user.contains("let a = 1;"));
    }

    #[test]
    fn test_custom_skill_registration() {
        let mut reg = SkillRegistry::new();
        let custom = Skill::new(
            "sql-optimizer",
            "SQL 优化器",
            "分析慢 SQL 执行计划并给出索引建议",
            vec!["/sql".into(), "/explain-sql".into()],
            "你是一位 MySQL / PostgreSQL DBA 专家。",
        );
        reg.register(custom);

        assert!(reg.get("sql-optimizer").is_some());
        let (matched, rest) = reg.match_prompt("/sql SELECT * FROM users").unwrap();
        assert_eq!(matched.id, "sql-optimizer");
        assert_eq!(rest, "SELECT * FROM users");

        // Pre-Input Hook 注入前缀后仍可匹配后续行的 /trigger 并保留前缀
        let (matched2, combined) = reg
            .match_prompt_with_prefix("[Custom Hook] 请注意安全\n/sql SELECT id FROM accounts")
            .unwrap();
        assert_eq!(matched2.id, "sql-optimizer");
        assert_eq!(
            combined,
            "[Custom Hook] 请注意安全\nSELECT id FROM accounts"
        );
    }
}
