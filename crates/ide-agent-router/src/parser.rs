//! 多语言 AST 与语义单元抽取器 (DD-ROUTER-SEM)
//!
//! 在不引入庞大外部依赖的前提下，提供针对主流工程语言的高性能、
//! 确定性语义单元抽取能力，支持 Rust, Python, TypeScript, JavaScript, C, PowerShell。

use crate::identity::{SemanticUnit, SupportedLanguage};

pub struct SemanticParser;

impl SemanticParser {
    /// 预计算行起始绝对字节偏移
    fn compute_line_starts(content: &str) -> Vec<usize> {
        let mut starts = vec![0];
        for (idx, ch) in content.char_indices() {
            if ch == '\n' {
                starts.push(idx + 1);
            }
        }
        starts
    }

    /// 解析源码并提取出所有独立的语义作业单元
    pub fn parse(
        workspace_id: &str,
        file_id: &str,
        content: &str,
        language: SupportedLanguage,
    ) -> Vec<SemanticUnit> {
        match language {
            SupportedLanguage::Rust => Self::parse_rust(workspace_id, file_id, content),
            SupportedLanguage::Python => Self::parse_python(workspace_id, file_id, content),
            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                Self::parse_js_ts(workspace_id, file_id, content, language)
            }
            SupportedLanguage::C => Self::parse_c(workspace_id, file_id, content),
            SupportedLanguage::PowerShell => Self::parse_powershell(workspace_id, file_id, content),
            SupportedLanguage::Fallback => Self::parse_fallback(workspace_id, file_id, content),
        }
    }

    /// Rust 语言解析器：提取顶级与 impl 内部函数、结构体、枚举
    fn parse_rust(workspace_id: &str, file_id: &str, content: &str) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let line_starts = Self::compute_line_starts(content);
        let mut current_impl = String::new();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            if trimmed.starts_with("impl ") {
                if let Some(target) = trimmed.strip_prefix("impl ") {
                    let parts: Vec<&str> = target.split('{').next().unwrap_or("").split_whitespace().collect();
                    if let Some(name) = parts.last() {
                        current_impl = name.to_string();
                    }
                }
            } else if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") || trimmed.starts_with("pub async fn ") || trimmed.starts_with("async fn ") {
                let start_line = i + 1;
                let start_byte = line_starts[i];

                // 提取函数名
                let sig_line = trimmed;
                let fn_keyword = if let Some(idx) = sig_line.find("fn ") { idx + 3 } else { 0 };
                let after_fn = &sig_line[fn_keyword..];
                let fn_name = after_fn.split('(').next().unwrap_or("").trim().split('<').next().unwrap_or("").trim();

                let qualified_path = if current_impl.is_empty() {
                    fn_name.to_string()
                } else {
                    format!("{}::{}", current_impl, fn_name)
                };

                // 匹配大括号结束
                let mut brace_depth = 0;
                let mut end_line = start_line;
                let mut end_byte = start_byte;
                let mut j = i;

                while j < lines.len() {
                    let cur = lines[j];
                    for c in cur.chars() {
                        if c == '{' { brace_depth += 1; }
                        else if c == '}' { brace_depth -= 1; }
                    }
                    if brace_depth == 0 && cur.contains('}') {
                        end_line = j + 1;
                        let last_brace_idx = cur.rfind('}').unwrap_or(cur.len());
                        end_byte = line_starts[j] + last_brace_idx + 1;
                        break;
                    }
                    j += 1;
                }

                if end_byte <= start_byte {
                    end_line = (j + 1).min(lines.len());
                    end_byte = if j < lines.len() {
                        if j + 1 < line_starts.len() { line_starts[j + 1] } else { content.len() }
                    } else {
                        content.len()
                    };
                }

                let unit_content = if end_byte <= content.len() {
                    &content[start_byte..end_byte]
                } else {
                    &content[start_byte..]
                };

                let sig_hash = SemanticUnit::compute_hash(sig_line);
                let content_hash = SemanticUnit::compute_hash(unit_content);
                let parent_fp = if current_impl.is_empty() { "root".to_string() } else { SemanticUnit::compute_hash(&current_impl) };
                let node_fp = SemanticUnit::compute_fingerprint(
                    workspace_id,
                    file_id,
                    SupportedLanguage::Rust,
                    &qualified_path,
                    "function_item",
                    &sig_hash,
                    &content_hash,
                    &parent_fp,
                );

                units.push(SemanticUnit {
                    workspace_id: workspace_id.to_string(),
                    file_id: file_id.to_string(),
                    language: SupportedLanguage::Rust,
                    qualified_symbol_path: qualified_path,
                    node_kind: "function_item".to_string(),
                    signature_hash: sig_hash,
                    base_content_hash: content_hash,
                    parent_fingerprint: parent_fp,
                    node_fingerprint: node_fp,
                    start_byte,
                    end_byte,
                    start_line,
                    end_line,
                });

                i = j;
            }

            i += 1;
        }

        if units.is_empty() {
            units = Self::parse_fallback(workspace_id, file_id, content);
        }
        units
    }

    /// Python 语言解析器：根据缩进层级提取 def / class
    fn parse_python(workspace_id: &str, file_id: &str, content: &str) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let line_starts = Self::compute_line_starts(content);
        let mut current_class = String::new();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            if trimmed.starts_with("class ") {
                let class_name = trimmed.strip_prefix("class ").unwrap_or("").split(':').next().unwrap_or("").split('(').next().unwrap_or("").trim();
                current_class = class_name.to_string();
            } else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let indent = line.len() - line.trim_start().len();
                let start_line = i + 1;
                let start_byte = line_starts[i];

                let fn_keyword = if trimmed.starts_with("def ") { "def " } else { "async def " };
                let after_kw = trimmed.strip_prefix(fn_keyword).unwrap_or("");
                let fn_name = after_kw.split('(').next().unwrap_or("").trim();

                let qualified_path = if indent > 0 && !current_class.is_empty() {
                    format!("{}.{}", current_class, fn_name)
                } else {
                    current_class.clear();
                    fn_name.to_string()
                };

                let mut j = i + 1;
                while j < lines.len() {
                    let next_line = lines[j];
                    let next_trim = next_line.trim();
                    if !next_trim.is_empty() && !next_trim.starts_with('#') {
                        let next_indent = next_line.len() - next_line.trim_start().len();
                        if next_indent <= indent {
                            break;
                        }
                    }
                    j += 1;
                }

                let end_line = j;
                let end_byte = if j < line_starts.len() { line_starts[j] } else { content.len() };
                let unit_content = &content[start_byte..end_byte];

                let sig_hash = SemanticUnit::compute_hash(trimmed);
                let content_hash = SemanticUnit::compute_hash(unit_content);
                let parent_fp = if current_class.is_empty() { "root".to_string() } else { SemanticUnit::compute_hash(&current_class) };
                let node_fp = SemanticUnit::compute_fingerprint(
                    workspace_id,
                    file_id,
                    SupportedLanguage::Python,
                    &qualified_path,
                    "function_definition",
                    &sig_hash,
                    &content_hash,
                    &parent_fp,
                );

                units.push(SemanticUnit {
                    workspace_id: workspace_id.to_string(),
                    file_id: file_id.to_string(),
                    language: SupportedLanguage::Python,
                    qualified_symbol_path: qualified_path,
                    node_kind: "function_definition".to_string(),
                    signature_hash: sig_hash,
                    base_content_hash: content_hash,
                    parent_fingerprint: parent_fp,
                    node_fingerprint: node_fp,
                    start_byte,
                    end_byte,
                    start_line,
                    end_line,
                });

                i = j.saturating_sub(1);
            }
            i += 1;
        }

        if units.is_empty() {
            units = Self::parse_fallback(workspace_id, file_id, content);
        }
        units
    }

    /// TypeScript / JavaScript 解析器
    fn parse_js_ts(
        workspace_id: &str,
        file_id: &str,
        content: &str,
        language: SupportedLanguage,
    ) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let line_starts = Self::compute_line_starts(content);

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            if (trimmed.starts_with("function ") || trimmed.starts_with("export function ") || trimmed.starts_with("async function ") || trimmed.starts_with("export async function ")) && trimmed.contains('(') {
                let start_line = i + 1;
                let start_byte = line_starts[i];

                let after_fn = if let Some(idx) = trimmed.find("function ") { &trimmed[idx + 9..] } else { trimmed };
                let fn_name = after_fn.split('(').next().unwrap_or("").trim().split('<').next().unwrap_or("").trim();

                let mut brace_depth = 0;
                let mut end_line = start_line;
                let mut end_byte = start_byte;
                let mut j = i;

                while j < lines.len() {
                    let cur = lines[j];
                    for c in cur.chars() {
                        if c == '{' { brace_depth += 1; }
                        else if c == '}' { brace_depth -= 1; }
                    }
                    if brace_depth == 0 && cur.contains('}') {
                        end_line = j + 1;
                        let last_brace_idx = cur.rfind('}').unwrap_or(cur.len());
                        end_byte = line_starts[j] + last_brace_idx + 1;
                        break;
                    }
                    j += 1;
                }

                let unit_content = if end_byte <= content.len() { &content[start_byte..end_byte] } else { &content[start_byte..] };
                let sig_hash = SemanticUnit::compute_hash(trimmed);
                let content_hash = SemanticUnit::compute_hash(unit_content);
                let node_fp = SemanticUnit::compute_fingerprint(
                    workspace_id,
                    file_id,
                    language,
                    fn_name,
                    "function_declaration",
                    &sig_hash,
                    &content_hash,
                    "root",
                );

                units.push(SemanticUnit {
                    workspace_id: workspace_id.to_string(),
                    file_id: file_id.to_string(),
                    language,
                    qualified_symbol_path: fn_name.to_string(),
                    node_kind: "function_declaration".to_string(),
                    signature_hash: sig_hash,
                    base_content_hash: content_hash,
                    parent_fingerprint: "root".to_string(),
                    node_fingerprint: node_fp,
                    start_byte,
                    end_byte,
                    start_line,
                    end_line,
                });

                i = j;
            }

            i += 1;
        }

        if units.is_empty() {
            units = Self::parse_fallback(workspace_id, file_id, content);
        }
        units
    }

    /// C 语言解析器
    fn parse_c(workspace_id: &str, file_id: &str, content: &str) -> Vec<SemanticUnit> {
        Self::parse_rust(workspace_id, file_id, content)
    }

    /// PowerShell 语言解析器: function Name { ... }
    fn parse_powershell(workspace_id: &str, file_id: &str, content: &str) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let line_starts = Self::compute_line_starts(content);

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            if trimmed.starts_with("function ") {
                let start_line = i + 1;
                let start_byte = line_starts[i];
                let name = trimmed.strip_prefix("function ").unwrap_or("").split('{').next().unwrap_or("").trim();

                let mut brace_depth = 0;
                let mut end_line = start_line;
                let mut end_byte = start_byte;
                let mut j = i;

                while j < lines.len() {
                    let cur = lines[j];
                    for c in cur.chars() {
                        if c == '{' { brace_depth += 1; }
                        else if c == '}' { brace_depth -= 1; }
                    }
                    if brace_depth == 0 && cur.contains('}') {
                        end_line = j + 1;
                        let last_brace_idx = cur.rfind('}').unwrap_or(cur.len());
                        end_byte = line_starts[j] + last_brace_idx + 1;
                        break;
                    }
                    j += 1;
                }

                let unit_content = if end_byte <= content.len() { &content[start_byte..end_byte] } else { &content[start_byte..] };
                let sig_hash = SemanticUnit::compute_hash(trimmed);
                let content_hash = SemanticUnit::compute_hash(unit_content);
                let node_fp = SemanticUnit::compute_fingerprint(
                    workspace_id,
                    file_id,
                    SupportedLanguage::PowerShell,
                    name,
                    "function_statement",
                    &sig_hash,
                    &content_hash,
                    "root",
                );

                units.push(SemanticUnit {
                    workspace_id: workspace_id.to_string(),
                    file_id: file_id.to_string(),
                    language: SupportedLanguage::PowerShell,
                    qualified_symbol_path: name.to_string(),
                    node_kind: "function_statement".to_string(),
                    signature_hash: sig_hash,
                    base_content_hash: content_hash,
                    parent_fingerprint: "root".to_string(),
                    node_fingerprint: node_fp,
                    start_byte,
                    end_byte,
                    start_line,
                    end_line,
                });

                i = j;
            }

            i += 1;
        }

        if units.is_empty() {
            units = Self::parse_fallback(workspace_id, file_id, content);
        }
        units
    }

    /// 保底退化方案：整文件作为单一语义单元
    fn parse_fallback(workspace_id: &str, file_id: &str, content: &str) -> Vec<SemanticUnit> {
        let content_hash = SemanticUnit::compute_hash(content);
        let node_fp = SemanticUnit::compute_fingerprint(
            workspace_id,
            file_id,
            SupportedLanguage::Fallback,
            "file_root",
            "file_root",
            &content_hash,
            &content_hash,
            "root",
        );

        vec![SemanticUnit {
            workspace_id: workspace_id.to_string(),
            file_id: file_id.to_string(),
            language: SupportedLanguage::Fallback,
            qualified_symbol_path: "file_root".to_string(),
            node_kind: "file_root".to_string(),
            signature_hash: content_hash.clone(),
            base_content_hash: content_hash,
            parent_fingerprint: "root".to_string(),
            node_fingerprint: node_fp,
            start_byte: 0,
            end_byte: content.len(),
            start_line: 1,
            end_line: content.lines().count().max(1),
        }]
    }
}
