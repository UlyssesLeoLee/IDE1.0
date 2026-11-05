//! PatchVerifier 流水线 (DD-ROUTER-VERIFY)
//!
//! 在多 Agent 并发写入中，补丁被提交合入前必须通过 Verifier 流水线：
//! 1. 安全拦截：检测文件路径穿透、空字节注入、破坏性脚本载荷；
//! 2. 静态语法与格式校验：针对 Rust, Python, TS/JS, C, PowerShell 等语言进行语法与格式检查；
//! 3. AST 锚点重定位前后语法树完整性比对：验证目标符号重定位合入后，所有兄弟节点 AST 语义不变，目标节点解析有效；
//! 4. 导出标准化 VerificationReport 报告。

use crate::identity::SupportedLanguage;
use crate::parser::SemanticParser;
use crate::patch::{PatchEngine, PatchError, PatchProposal};
use crate::sandbox::{SandboxError, WorkerSandbox};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 验证状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// 补丁合法且完全通过 AST 完整性校验与静态语法检查
    Passed,
    /// 存在并发冲突或符号重定位失败
    Conflict,
    /// 语法与 AST 完整，但存在代码格式或规范警告
    FormatWarning,
    /// 命中安全防御规则（路径穿透、破坏性脚本或非法注入）被拦截
    SecurityBlocked,
}

/// 问题严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

/// 验证问题条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationIssue {
    pub severity: IssueSeverity,
    pub rule: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// 验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub status: VerificationStatus,
    pub is_valid: bool,
    pub file_id: String,
    pub symbol_path: String,
    pub issues: Vec<VerificationIssue>,
    pub ast_integrity_preserved: bool,
    pub relocation_applied: bool,
    pub previous_revision: u64,
    pub new_revision: Option<u64>,
    pub original_fingerprint: Option<String>,
    pub verified_fingerprint: Option<String>,
    pub patched_content: Option<String>,
}

pub struct PatchVerifier;

impl PatchVerifier {
    /// 核心流水线：执行完整的补丁安全、语法、AST 重定位与完整性比对
    pub fn verify(
        workspace_id: &str,
        file_id: &str,
        current_content: &str,
        current_revision: u64,
        language: SupportedLanguage,
        proposal: &PatchProposal,
    ) -> VerificationReport {
        let mut issues = Vec::new();

        // 1. 安全拦截检查
        let security_issues = Self::verify_security(proposal);
        let has_fatal = security_issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Fatal);
        issues.extend(security_issues);

        if has_fatal {
            return VerificationReport {
                status: VerificationStatus::SecurityBlocked,
                is_valid: false,
                file_id: file_id.to_string(),
                symbol_path: proposal.symbol_path.clone(),
                issues,
                ast_integrity_preserved: false,
                relocation_applied: false,
                previous_revision: current_revision,
                new_revision: None,
                original_fingerprint: None,
                verified_fingerprint: None,
                patched_content: None,
            };
        }

        // 2. 静态语法与格式校验（针对 proposal.replacement_text）
        let syntax_issues = Self::verify_syntax(&proposal.replacement_text, language);
        let format_issues = Self::verify_format(&proposal.replacement_text);
        let has_syntax_error = syntax_issues
            .iter()
            .any(|i| i.severity >= IssueSeverity::Error);

        issues.extend(syntax_issues);
        issues.extend(format_issues);

        if has_syntax_error {
            return VerificationReport {
                status: VerificationStatus::Conflict,
                is_valid: false,
                file_id: file_id.to_string(),
                symbol_path: proposal.symbol_path.clone(),
                issues,
                ast_integrity_preserved: false,
                relocation_applied: false,
                previous_revision: current_revision,
                new_revision: None,
                original_fingerprint: None,
                verified_fingerprint: None,
                patched_content: None,
            };
        }

        // 3. 执行 AST 锚点重定位与应用
        let patch_result = match PatchEngine::relocate_and_apply(
            workspace_id,
            file_id,
            current_content,
            current_revision,
            language,
            proposal,
        ) {
            Ok(res) => res,
            Err(err) => {
                let rule = match err {
                    PatchError::SymbolNotFound(_) => "symbol_not_found",
                    PatchError::SemanticWriteConflict { .. } => "semantic_write_conflict",
                    PatchError::InvalidRange => "invalid_range",
                };
                issues.push(VerificationIssue {
                    severity: IssueSeverity::Error,
                    rule: rule.to_string(),
                    message: err.to_string(),
                    line: None,
                    column: None,
                });
                return VerificationReport {
                    status: VerificationStatus::Conflict,
                    is_valid: false,
                    file_id: file_id.to_string(),
                    symbol_path: proposal.symbol_path.clone(),
                    issues,
                    ast_integrity_preserved: false,
                    relocation_applied: false,
                    previous_revision: current_revision,
                    new_revision: None,
                    original_fingerprint: None,
                    verified_fingerprint: None,
                    patched_content: None,
                };
            }
        };

        // 4. AST 锚点重定位前后语法树完整性比对
        let old_units = SemanticParser::parse(workspace_id, file_id, current_content, language);
        let new_units =
            SemanticParser::parse(workspace_id, file_id, &patch_result.new_content, language);

        let mut ast_integrity_preserved = true;
        let mut original_target_fp = None;

        // 4.1 检查所有兄弟节点（Sibling Units）的完整性：内容哈希与节点类型不得受损
        for old_u in &old_units {
            if old_u.qualified_symbol_path == proposal.symbol_path {
                original_target_fp = Some(old_u.node_fingerprint.clone());
                continue;
            }

            match new_units
                .iter()
                .find(|u| u.qualified_symbol_path == old_u.qualified_symbol_path)
            {
                Some(new_u) => {
                    if new_u.base_content_hash != old_u.base_content_hash
                        || new_u.node_kind != old_u.node_kind
                    {
                        ast_integrity_preserved = false;
                        issues.push(VerificationIssue {
                            severity: IssueSeverity::Error,
                            rule: "sibling_ast_corruption".to_string(),
                            message: format!(
                                "Sibling symbol '{}' was corrupted by patch application",
                                old_u.qualified_symbol_path
                            ),
                            line: Some(new_u.start_line),
                            column: None,
                        });
                    }
                }
                None => {
                    ast_integrity_preserved = false;
                    issues.push(VerificationIssue {
                        severity: IssueSeverity::Error,
                        rule: "sibling_ast_lost".to_string(),
                        message: format!(
                            "Sibling symbol '{}' disappeared after patch application",
                            old_u.qualified_symbol_path
                        ),
                        line: None,
                        column: None,
                    });
                }
            }
        }

        // 4.2 检查目标节点本身在新 AST 中是否成功被抽取并且语法结构有效
        let new_target = new_units
            .iter()
            .find(|u| u.qualified_symbol_path == proposal.symbol_path);
        if new_target.is_none() {
            ast_integrity_preserved = false;
            issues.push(VerificationIssue {
                severity: IssueSeverity::Error,
                rule: "target_symbol_ast_broken".to_string(),
                message: format!(
                    "Target symbol '{}' failed to be parsed into a valid semantic unit",
                    proposal.symbol_path
                ),
                line: None,
                column: None,
            });
        }

        let verified_fp = new_target.map(|u| u.node_fingerprint.clone());

        // 5. 判定最终验证状态
        let has_errors = issues.iter().any(|i| i.severity >= IssueSeverity::Error);
        let has_warnings = issues.iter().any(|i| i.severity == IssueSeverity::Warning);

        let status = if !ast_integrity_preserved || has_errors {
            VerificationStatus::Conflict
        } else if has_warnings {
            VerificationStatus::FormatWarning
        } else {
            VerificationStatus::Passed
        };

        let is_valid = ast_integrity_preserved && !has_errors;

        VerificationReport {
            status,
            is_valid,
            file_id: file_id.to_string(),
            symbol_path: proposal.symbol_path.clone(),
            issues,
            ast_integrity_preserved,
            relocation_applied: patch_result.relocated,
            previous_revision: current_revision,
            new_revision: Some(patch_result.new_revision),
            original_fingerprint: original_target_fp,
            verified_fingerprint: verified_fp,
            patched_content: Some(patch_result.new_content),
        }
    }

    /// 在 WorkerSandbox 中执行沙盒暂存与隔离校验流水线
    pub fn verify_in_sandbox(
        sandbox: &WorkerSandbox,
        workspace_id: &str,
        file_id: &str,
        current_content: &str,
        current_revision: u64,
        language: SupportedLanguage,
        proposal: &PatchProposal,
    ) -> Result<(VerificationReport, Option<PathBuf>), SandboxError> {
        let report = Self::verify(
            workspace_id,
            file_id,
            current_content,
            current_revision,
            language,
            proposal,
        );

        if !report.is_valid {
            return Ok((report, None));
        }

        let mut env = sandbox.create_environment();
        if let Some(patched_text) = &report.patched_content {
            let staged_path = env.stage_file(sandbox, file_id, patched_text.as_bytes())?;
            Ok((report, Some(staged_path)))
        } else {
            Ok((report, None))
        }
    }

    /// 安全规则校验（防止路径穿透与破坏性载荷）
    pub fn verify_security(proposal: &PatchProposal) -> Vec<VerificationIssue> {
        let mut issues = Vec::new();

        // 检查 file_id 是否包含路径穿透
        if proposal.file_id.contains("..")
            || proposal.file_id.contains('\0')
            || proposal.file_id.starts_with('/')
            || proposal.file_id.starts_with('\\')
            || proposal.file_id.contains(':')
        {
            issues.push(VerificationIssue {
                severity: IssueSeverity::Fatal,
                rule: "security_path_traversal".to_string(),
                message: format!(
                    "Path traversal attempt or invalid characters detected in file_id: '{}'",
                    proposal.file_id
                ),
                line: None,
                column: None,
            });
        }

        // 检查 symbol_path
        if proposal.symbol_path.contains('\0') || proposal.symbol_path.contains("..") {
            issues.push(VerificationIssue {
                severity: IssueSeverity::Fatal,
                rule: "security_invalid_symbol".to_string(),
                message: "Symbol path contains illegal control characters or traversal".to_string(),
                line: None,
                column: None,
            });
        }

        // 检查 replacement_text
        if proposal.replacement_text.contains('\0') {
            issues.push(VerificationIssue {
                severity: IssueSeverity::Fatal,
                rule: "security_null_byte_injection".to_string(),
                message: "Null byte detected in patch replacement payload".to_string(),
                line: None,
                column: None,
            });
        }

        // 破坏性脚本载荷检测
        let dangerous_payloads = [
            ":(){ :|:& };:", // Forkbomb
            "rm -rf /",
            "del /f /s /q c:\\",
            "format c:",
        ];
        let lower_replacement = proposal.replacement_text.to_lowercase();
        for payload in dangerous_payloads {
            if lower_replacement.contains(payload) {
                issues.push(VerificationIssue {
                    severity: IssueSeverity::Fatal,
                    rule: "security_destructive_payload".to_string(),
                    message: format!(
                        "Potentially destructive payload pattern detected: '{}'",
                        payload
                    ),
                    line: None,
                    column: None,
                });
            }
        }

        issues
    }

    /// 多语言静态语法校验
    pub fn verify_syntax(code: &str, language: SupportedLanguage) -> Vec<VerificationIssue> {
        let mut issues = Vec::new();

        // 1. 括号与界定符配对校验
        if let Err((msg, line, col)) = check_balanced_delimiters(code, language) {
            issues.push(VerificationIssue {
                severity: IssueSeverity::Error,
                rule: "syntax_unbalanced_delimiter".to_string(),
                message: msg,
                line: Some(line),
                column: Some(col),
            });
        }

        // 2. 特定语言静态语法校验
        match language {
            SupportedLanguage::Rust => {
                check_rust_syntax(code, &mut issues);
            }
            SupportedLanguage::Python => {
                check_python_syntax(code, &mut issues);
            }
            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                check_js_ts_syntax(code, &mut issues);
            }
            SupportedLanguage::PowerShell => {
                check_powershell_syntax(code, &mut issues);
            }
            SupportedLanguage::C | SupportedLanguage::Fallback => {}
        }

        issues
    }

    /// 补丁格式校验（FormatWarning）
    pub fn verify_format(code: &str) -> Vec<VerificationIssue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = code.lines().collect();

        let mut consecutive_blank = 0;
        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;

            // 尾随空白字符检查
            if line.ends_with(' ') || line.ends_with('\t') {
                issues.push(VerificationIssue {
                    severity: IssueSeverity::Warning,
                    rule: "format_trailing_whitespace".to_string(),
                    message: "Line contains trailing whitespace".to_string(),
                    line: Some(line_num),
                    column: Some(line.len()),
                });
            }

            // 单行超长警告 (> 120 字符)
            if line.len() > 120 {
                issues.push(VerificationIssue {
                    severity: IssueSeverity::Warning,
                    rule: "format_line_length_exceeded".to_string(),
                    message: format!("Line length exceeds 120 characters ({})", line.len()),
                    line: Some(line_num),
                    column: Some(120),
                });
            }

            // 连续空行过多检查 (> 2)
            if line.trim().is_empty() {
                consecutive_blank += 1;
                if consecutive_blank > 2 {
                    issues.push(VerificationIssue {
                        severity: IssueSeverity::Warning,
                        rule: "format_consecutive_blank_lines".to_string(),
                        message: "More than 2 consecutive blank lines detected".to_string(),
                        line: Some(line_num),
                        column: Some(1),
                    });
                }
            } else {
                consecutive_blank = 0;
            }
        }

        issues
    }
}

/// 界定符配对校验器（支持跳过注释与字符串字面量）
fn check_balanced_delimiters(
    code: &str,
    language: SupportedLanguage,
) -> Result<(), (String, usize, usize)> {
    let mut stack = Vec::new();
    let chars: Vec<char> = code.chars().collect();
    let mut idx = 0;
    let mut line = 1;
    let mut col = 0;

    let is_hash_comment = matches!(
        language,
        SupportedLanguage::Python | SupportedLanguage::PowerShell
    );

    while idx < chars.len() {
        let ch = chars[idx];
        col += 1;

        if ch == '\n' {
            line += 1;
            col = 0;
            idx += 1;
            continue;
        }

        // 单行注释检查: //
        if !is_hash_comment && ch == '/' && idx + 1 < chars.len() && chars[idx + 1] == '/' {
            while idx < chars.len() && chars[idx] != '\n' {
                idx += 1;
            }
            continue;
        }

        // 块注释检查: /* ... */
        if !is_hash_comment && ch == '/' && idx + 1 < chars.len() && chars[idx + 1] == '*' {
            let start_line = line;
            let start_col = col;
            idx += 2;
            let mut closed = false;
            while idx + 1 < chars.len() {
                if chars[idx] == '\n' {
                    line += 1;
                    col = 0;
                }
                if chars[idx] == '*' && chars[idx + 1] == '/' {
                    idx += 2;
                    closed = true;
                    break;
                }
                idx += 1;
            }
            if !closed {
                return Err((
                    "Unclosed block comment '/*'".to_string(),
                    start_line,
                    start_col,
                ));
            }
            continue;
        }

        // 单行注释检查: #
        if is_hash_comment && ch == '#' {
            while idx < chars.len() && chars[idx] != '\n' {
                idx += 1;
            }
            continue;
        }

        // 字符串字面量检查: " 或 ' 或 `
        if ch == '"'
            || ch == '`'
            || (ch == '\'' && language != SupportedLanguage::Rust)
        {
            let quote = ch;
            let start_line = line;
            let start_col = col;
            idx += 1;
            let mut closed = false;
            while idx < chars.len() {
                if chars[idx] == '\\' && idx + 1 < chars.len() {
                    idx += 2;
                    continue;
                }
                if chars[idx] == '\n' {
                    line += 1;
                    col = 0;
                    if quote != '`' {
                        // 单引号或双引号通常不允许跨行（除了多行字符串）
                    }
                }
                if chars[idx] == quote {
                    closed = true;
                    idx += 1;
                    break;
                }
                idx += 1;
            }
            if !closed {
                return Err((
                    format!("Unclosed string literal starting with '{}'", quote),
                    start_line,
                    start_col,
                ));
            }
            continue;
        }

        // 括号配对
        match ch {
            '(' | '[' | '{' => {
                stack.push((ch, line, col));
            }
            ')' => {
                if let Some((top, _, _)) = stack.pop() {
                    if top != '(' {
                        return Err((
                            format!("Mismatched delimiter: expected ')', found '{}'", top),
                            line,
                            col,
                        ));
                    }
                } else {
                    return Err(("Unexpected closing parenthesis ')'".to_string(), line, col));
                }
            }
            ']' => {
                if let Some((top, _, _)) = stack.pop() {
                    if top != '[' {
                        return Err((
                            format!("Mismatched delimiter: expected ']', found '{}'", top),
                            line,
                            col,
                        ));
                    }
                } else {
                    return Err(("Unexpected closing bracket ']'".to_string(), line, col));
                }
            }
            '}' => {
                if let Some((top, _, _)) = stack.pop() {
                    if top != '{' {
                        return Err((
                            format!("Mismatched delimiter: expected '}}', found '{}'", top),
                            line,
                            col,
                        ));
                    }
                } else {
                    return Err(("Unexpected closing brace '}}'".to_string(), line, col));
                }
            }
            _ => {}
        }

        idx += 1;
    }

    if let Some((unclosed, l, c)) = stack.pop() {
        return Err((
            format!("Unclosed delimiter '{}'", unclosed),
            l,
            c,
        ));
    }

    Ok(())
}

/// Rust 语言静态语法特征检查
fn check_rust_syntax(code: &str, issues: &mut Vec<VerificationIssue>) {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return;
    }

    // 检查是否有缺失的大括号或函数签名断裂
    if (trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ")) && !trimmed.contains('{') {
        issues.push(VerificationIssue {
            severity: IssueSeverity::Error,
            rule: "rust_missing_function_body".to_string(),
            message: "Function definition is missing opening brace '{'".to_string(),
            line: Some(1),
            column: None,
        });
    }
}

/// Python 语言静态语法与缩进特征检查
fn check_python_syntax(code: &str, issues: &mut Vec<VerificationIssue>) {
    let lines: Vec<&str> = code.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // 检查同一行前导缩进是否混合使用了空格与 Tab
        let leading_whitespace = &line[..line.len() - line.trim_start().len()];
        if leading_whitespace.contains(' ') && leading_whitespace.contains('\t') {
            issues.push(VerificationIssue {
                severity: IssueSeverity::Error,
                rule: "python_mixed_indentation".to_string(),
                message: "Mixed spaces and tabs in line indentation".to_string(),
                line: Some(line_num),
                column: Some(1),
            });
        }

        // 块声明语句结尾必须包含冒号 ':'
        let is_block_starter = trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("if ")
            || trimmed.starts_with("elif ")
            || trimmed == "else:"
            || trimmed.starts_with("else ")
            || trimmed.starts_with("while ")
            || trimmed.starts_with("for ")
            || trimmed.starts_with("try:")
            || trimmed.starts_with("except ")
            || trimmed == "finally:"
            || trimmed.starts_with("with ");

        if is_block_starter {
            let without_comment = trimmed.split('#').next().unwrap_or("").trim();
            if !without_comment.ends_with(':') {
                issues.push(VerificationIssue {
                    severity: IssueSeverity::Error,
                    rule: "python_missing_colon".to_string(),
                    message: "Python block statement must end with a colon ':'".to_string(),
                    line: Some(line_num),
                    column: Some(without_comment.len()),
                });
            }
        }
    }
}

/// TS/JS 语言语法校验
fn check_js_ts_syntax(code: &str, issues: &mut Vec<VerificationIssue>) {
    let trimmed = code.trim();
    if (trimmed.starts_with("function ") || trimmed.starts_with("export function "))
        && !trimmed.contains('(')
    {
        issues.push(VerificationIssue {
            severity: IssueSeverity::Error,
            rule: "js_missing_parameter_list".to_string(),
            message: "Function declaration is missing parameter list '()'.".to_string(),
            line: Some(1),
            column: None,
        });
    }
}

/// PowerShell 语法校验
fn check_powershell_syntax(code: &str, issues: &mut Vec<VerificationIssue>) {
    let trimmed = code.trim();
    if trimmed.starts_with("function ") && !trimmed.contains('{') {
        issues.push(VerificationIssue {
            severity: IssueSeverity::Error,
            rule: "ps_missing_body".to_string(),
            message: "PowerShell function declaration is missing body '{'".to_string(),
            line: Some(1),
            column: None,
        });
    }
}
