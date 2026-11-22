//! Blender `bpy` 风格的 Rust 自动化控制台脚本引擎。
//!
//! 提供对标 Blender `bpy` 架构的统一三元分层：
//! - `ide::context::*`：当前上下文（活跃文件、工作区统计、环境元数据、系统状态）
//! - `ide::data::*`：数据对象模型（所有缓冲区分区、版本树、文档正文、行度量）
//! - `ide::ops::*`：操作动作执行器（打开文件、按行修改、全文替换、双版本比对、大纲解析）
//!
//! 支持以分号 `;` 或换行分隔的多行 Rust 脚本代码，支持 `//` 注释，专为外部 AI 与本地控制台自动化设计。

use crate::merge::{CoEditWorkspace, EditRequest};
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptStep {
    pub expr: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct ScriptResult {
    pub ok: bool,
    pub summary: String,
    pub steps: Vec<ScriptStep>,
}

pub struct RustConsoleEvaluator;

impl RustConsoleEvaluator {
    /// 执行一段符合 Blender `ide::*` 规范的 Rust 操作代码
    pub fn eval(ws: &mut CoEditWorkspace, script: &str) -> ScriptResult {
        let mut steps = Vec::new();
        let mut all_ok = true;

        for raw_line in script.lines() {
            // 支持单行内以分号分隔多个语句
            for stmt in raw_line.split(';') {
                let s = stmt.trim();
                if s.is_empty() || s.starts_with("//") {
                    continue;
                }
                let (ok, out) = Self::eval_statement(ws, s);
                if !ok {
                    all_ok = false;
                }
                steps.push(ScriptStep {
                    expr: s.to_string(),
                    ok,
                    output: out,
                });
            }
        }

        let summary = if steps.is_empty() {
            "Empty script (no statements executed)".to_string()
        } else {
            let passed = steps.iter().filter(|s| s.ok).count();
            format!(
                "Executed {} statement(s): {} passed, {} failed",
                steps.len(),
                passed,
                steps.len() - passed
            )
        };

        ScriptResult {
            ok: all_ok,
            summary,
            steps,
        }
    }

    fn eval_statement(ws: &mut CoEditWorkspace, stmt: &str) -> (bool, String) {
        let clean = stmt.trim().trim_end_matches(';');
        if clean.eq_ignore_ascii_case("ide::context::help()")
            || clean.eq_ignore_ascii_case("ide::ops::help()")
            || clean.eq_ignore_ascii_case("help()")
        {
            return (
                true,
                "=== IDE1.0 Blender-Style Rust Scripting API ===\n\
                 ide::context::status()                     - 查看工作区状态与打开文档数量\n\
                 ide::context::help()                       - 打印全部控制指令帮助\n\
                 ide::data::files()                         - 列出所有已加载文档及其版本\n\
                 ide::data::read(\"file_id\")                - 读取文档全部正文\n\
                 ide::data::history(\"file_id\")             - 查询文档历史版本与协作作者\n\
                 ide::ops::open(\"file_id\", \"lang\", \"txt\")  - 打开或创建新文档\n\
                 ide::ops::edit(\"file_id\", s, e, \"txt\")   - 按行修改文档 (1-based 行号)\n\
                 ide::ops::replace_all(\"file_id\", \"a\", \"b\")- 全文查找并替换为新版本\n\
                 ide::ops::diff(\"file_id\", rev_a, rev_b)  - 比对文档两版本间的 Unified Diff\n\
                 ide::ops::search(\"file_id\", \"needle\")    - 查找文本行命中\n\
                 ide::ops::outline(\"file_id\")              - 提取文档 Markdown 标题大纲"
                    .into(),
            );
        }

        if clean.eq_ignore_ascii_case("ide::context::status()") {
            let files_count = ws.docs.len();
            let total_revs: usize = ws.docs.values().map(|d| d.revisions.len()).sum();
            return (
                true,
                format!(
                    "ide::context: active_documents={}, total_revisions={}, backend=\"IDE1.0 MCP Engine v0.8.9\"",
                    files_count, total_revs
                ),
            );
        }

        if clean.eq_ignore_ascii_case("ide::data::files()") {
            let docs_json = ws.list_docs();
            let docs = docs_json["docs"].as_array();
            let mut out = String::from("ide::data::files:\n");
            if let Some(list) = docs {
                for d in list {
                    out.push_str(&format!(
                        "  - {} [rev={}, lang={}]\n",
                        d["file_id"].as_str().unwrap_or("?"),
                        d["current_revision"].as_u64().unwrap_or(0),
                        d["language"].as_str().unwrap_or("?")
                    ));
                }
            }
            return (true, out);
        }

        if let Some(args) = Self::strip_call(clean, "ide::data::read") {
            let arg = args.trim().trim_matches('"');
            return match ws.read_doc(arg, None, None) {
                Ok(v) => (true, v["content"].as_str().unwrap_or("").to_string()),
                Err(e) => (false, e),
            };
        }

        if let Some(args) = Self::strip_call(clean, "ide::data::history") {
            let arg = args.trim().trim_matches('"');
            return match ws.doc_history(arg) {
                Ok(v) => (
                    true,
                    serde_json::to_string_pretty(&v["history"]).unwrap_or_default(),
                ),
                Err(e) => (false, e),
            };
        }

        if let Some(args) = Self::strip_call(clean, "ide::ops::open") {
            let parts = Self::parse_args(args);
            if parts.len() < 3 {
                return (
                    false,
                    "ide::ops::open requires 3 args: (file_id, language, content)".into(),
                );
            }
            let doc = ws.open_doc(&parts[0], &parts[2], &parts[1], "rust_script", "console");
            return (
                true,
                format!(
                    "Opened '{}' (rev 1, hash={})",
                    doc.file_id, doc.content_hash
                ),
            );
        }

        if let Some(args) = Self::strip_call(clean, "ide::ops::edit") {
            let parts = Self::parse_args(args);
            if parts.len() < 4 {
                return (
                    false,
                    "ide::ops::edit requires 4 args: (file_id, start_line, end_line, replacement)"
                        .into(),
                );
            }
            let s: usize = parts[1].parse().unwrap_or(1);
            let e: usize = parts[2].parse().unwrap_or(s);
            let req = EditRequest {
                file_id: parts[0].clone(),
                provider: "rust_script".into(),
                agent_id: "console".into(),
                base_revision: None,
                start_line: s,
                end_line: e,
                replacement: parts[3].clone(),
                op_id: None,
                summary: Some(format!("ide::ops::edit L{}..L{}", s, e)),
            };
            return match ws.edit_doc(req) {
                Ok(v) => (
                    true,
                    format!(
                        "Committed rev {} (status={}, hash={})",
                        v["new_revision"],
                        v["status"],
                        v["content_hash"].as_str().unwrap_or("?")
                    ),
                ),
                Err(e) => (false, e),
            };
        }

        if let Some(args) = Self::strip_call(clean, "ide::ops::replace_all") {
            let parts = Self::parse_args(args);
            if parts.len() < 3 {
                return (
                    false,
                    "ide::ops::replace_all requires 3 args: (file_id, from, to)".into(),
                );
            }
            let file_id = &parts[0];
            let from = &parts[1];
            let to = &parts[2];
            let doc = match ws.docs.get(file_id) {
                Some(d) => d.clone(),
                None => return (false, format!("Document not found: {file_id}")),
            };
            let count = doc.content.matches(from).count();
            if count == 0 {
                return (
                    true,
                    format!("No occurrences of '{}' found in '{}'", from, file_id),
                );
            }
            let replaced = doc.content.replace(from, to);
            let lines_count = crate::diff::split_lines(&doc.content).len();
            let req = EditRequest {
                file_id: file_id.clone(),
                provider: "rust_script".into(),
                agent_id: "console".into(),
                base_revision: Some(doc.current_revision),
                start_line: 1,
                end_line: lines_count,
                replacement: replaced,
                op_id: None,
                summary: Some(format!(
                    "ide::ops::replace_all: replaced {} instance(s)",
                    count
                )),
            };
            return match ws.edit_doc(req) {
                Ok(v) => (
                    true,
                    format!(
                        "Replaced {} instance(s), committed rev {}",
                        count, v["new_revision"]
                    ),
                ),
                Err(e) => (false, e),
            };
        }

        if let Some(args) = Self::strip_call(clean, "ide::ops::diff") {
            let parts = Self::parse_args(args);
            if parts.len() < 3 {
                return (
                    false,
                    "ide::ops::diff requires 3 args: (file_id, rev_a, rev_b)".into(),
                );
            }
            let r_a: u64 = parts[1].parse().unwrap_or(1);
            let r_b: u64 = parts[2].parse().unwrap_or(r_a);
            return match ws.diff_revisions(&parts[0], r_a, r_b, 2) {
                Ok(v) => (true, v["unified_diff"].as_str().unwrap_or("").to_string()),
                Err(e) => (false, e),
            };
        }

        if let Some(args) = Self::strip_call(clean, "ide::ops::search") {
            let parts = Self::parse_args(args);
            if parts.len() < 2 {
                return (
                    false,
                    "ide::ops::search requires 2 args: (file_id, needle)".into(),
                );
            }
            let doc = match ws.docs.get(&parts[0]) {
                Some(d) => d,
                None => return (false, format!("Document not found: {}", parts[0])),
            };
            let needle = &parts[1];
            let mut hits = Vec::new();
            for (idx, line) in doc.content.lines().enumerate() {
                if line.contains(needle) {
                    hits.push(format!("  L{}: {}", idx + 1, line.trim()));
                }
            }
            return (
                true,
                format!(
                    "Found {} hit(s) for '{}':\n{}",
                    hits.len(),
                    needle,
                    hits.join("\n")
                ),
            );
        }

        if let Some(args) = Self::strip_call(clean, "ide::ops::outline") {
            let arg = args.trim().trim_matches('"');
            let doc = match ws.docs.get(arg) {
                Some(d) => d,
                None => return (false, format!("Document not found: {arg}")),
            };
            match editcore::agent_api::call("md_outline", &json!({ "text": doc.content })) {
                Ok(v) => (
                    true,
                    serde_json::to_string_pretty(&v["outline"]).unwrap_or_default(),
                ),
                Err(e) => (false, e),
            }
        } else {
            (
                false,
                format!(
                    "Unrecognized statement: '{}'. Type 'ide::context::help()' to view available functions.",
                    stmt
                ),
            )
        }
    }

    fn strip_call<'a>(s: &'a str, fn_name: &str) -> Option<&'a str> {
        let trimmed = s.trim();
        if let Some(rest) = trimmed.strip_prefix(fn_name) {
            let rest = rest.trim();
            if rest.starts_with('(') && rest.ends_with(')') {
                return Some(&rest[1..rest.len() - 1]);
            }
        }
        None
    }

    /// 简易参数切分，支持双引号字符串与整型
    fn parse_args(args_str: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_quote = false;
        let mut cur = String::new();
        let mut escape = false;

        for ch in args_str.chars() {
            if escape {
                cur.push(ch);
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_quote = !in_quote;
                continue;
            }
            if ch == ',' && !in_quote {
                out.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            cur.push(ch);
        }
        if !cur.trim().is_empty() {
            out.push(cur.trim().to_string());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_script_multi_statement_workflow() {
        let mut ws = CoEditWorkspace::default();
        let script = r#"
            // 1. 查看初始环境状态
            ide::context::status();

            // 2. 打开一个新代码文件
            ide::ops::open("src/algo.rs", "rust", "fn solve() -> i32 { 10 }\n");

            // 3. 修改第 1 行
            ide::ops::edit("src/algo.rs", 1, 1, "pub fn solve() -> i32 { 42 }");

            // 4. 比对统一版本差分
            ide::ops::diff("src/algo.rs", 1, 2);

            // 5. 查找文本
            ide::ops::search("src/algo.rs", "solve");
        "#;

        let res = RustConsoleEvaluator::eval(&mut ws, script);
        assert!(res.ok, "Script execution must succeed: {:?}", res.steps);
        assert_eq!(res.steps.len(), 5);

        // 验证文件状态
        let read = ws.read_doc("src/algo.rs", None, None).unwrap();
        assert!(read["content"]
            .as_str()
            .unwrap()
            .contains("pub fn solve() -> i32 { 42 }"));
        assert_eq!(read["current_revision"], 2);

        // 验证 diff 输出
        assert!(res.steps[3].output.contains("@@ -1,1 +1,1 @@"));
        assert!(res.steps[3]
            .output
            .contains("+pub fn solve() -> i32 { 42 }"));
    }

    #[test]
    fn test_rust_script_help_and_unknown() {
        let mut ws = CoEditWorkspace::default();
        let res = RustConsoleEvaluator::eval(&mut ws, "ide::ops::help()");
        assert!(res.ok);
        assert!(res.steps[0].output.contains("ide::context::status()"));

        let fail = RustConsoleEvaluator::eval(&mut ws, "ide::ops::non_existent()");
        assert!(!fail.ok);
        assert!(fail.steps[0].output.contains("Unrecognized statement"));
    }
}
