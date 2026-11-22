//! Blender `bpy` 风格的 Rust 自动化控制台脚本引擎（与 MCP 共享单一 `DocumentStore` 权威状态）。

use crate::merge::{DocumentStore, EditRequest};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static CONSOLE_OP_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_console_op(tag: &str) -> String {
    format!(
        "console-{tag}-{}",
        CONSOLE_OP_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

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
    /// 在单一权威 `DocumentStore` 上执行一段符合 Blender `ide::*` 规范的脚本。
    pub fn eval(store: &DocumentStore, script: &str) -> ScriptResult {
        let mut steps = Vec::new();
        let mut all_ok = true;

        for raw_line in script.lines() {
            for stmt in raw_line.split(';') {
                let s = stmt.trim();
                if s.is_empty() || s.starts_with("//") {
                    continue;
                }
                let (ok, out) = Self::eval_statement(store, s);
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

    fn eval_statement(store: &DocumentStore, stmt: &str) -> (bool, String) {
        let clean = stmt.trim().trim_end_matches(';');
        if clean.eq_ignore_ascii_case("ide::context::help()")
            || clean.eq_ignore_ascii_case("ide::ops::help()")
            || clean.eq_ignore_ascii_case("help()")
        {
            return (true, Self::help_text().into());
        }

        if clean.eq_ignore_ascii_case("ide::context::status()") {
            let list = store.list_docs();
            let docs = list["docs"].as_array();
            let files_count = docs.map(|d| d.len()).unwrap_or(0);
            let total_revs: u64 = docs
                .map(|arr| {
                    arr.iter()
                        .filter_map(|d| d["revisions_count"].as_u64())
                        .sum()
                })
                .unwrap_or(0);
            return (
                true,
                format!(
                    "ide::context: active_documents={}, total_revisions={}, backend=\"IDE1.0 MCP Engine v{}\"",
                    files_count, total_revs, env!("CARGO_PKG_VERSION")
                ),
            );
        }

        if clean.eq_ignore_ascii_case("ide::data::files()")
            || clean.eq_ignore_ascii_case("ide::data::docs()")
        {
            return (true, Self::format_docs_list(store));
        }

        if let Some(args) = Self::strip_call(clean, "ide::data::read") {
            let arg = args.trim().trim_matches('"');
            return match store.read_doc(arg, None, None) {
                Ok(v) => (true, v["content"].as_str().unwrap_or("").to_string()),
                Err(e) => (false, e),
            };
        }

        if let Some(args) = Self::strip_call(clean, "ide::data::history") {
            let arg = args.trim().trim_matches('"');
            return match store.doc_history(arg) {
                Ok(v) => (
                    true,
                    serde_json::to_string_pretty(&v["history"]).unwrap_or_default(),
                ),
                Err(e) => (false, e),
            };
        }

        Self::eval_ops_statement(store, clean)
    }

    fn eval_ops_statement(store: &DocumentStore, clean: &str) -> (bool, String) {
        if let Some(args) = Self::strip_call(clean, "ide::ops::open") {
            let parts = Self::parse_args(args);
            if parts.len() < 3 {
                return (
                    false,
                    "ide::ops::open requires 3 args: (file_id, language, content)".into(),
                );
            }
            let v = store.open_doc(&parts[0], &parts[2], &parts[1], "rust_script", "console");
            return (
                v["ok"] != false,
                format!(
                    "Opened '{}' (rev {}, status={}, hash={})",
                    parts[0],
                    v["current_revision"],
                    v["status"].as_str().unwrap_or("?"),
                    v["content_hash"].as_str().unwrap_or("?")
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
                base_content_hash: None,
                fencing_token: None,
                start_line: s,
                end_line: e,
                replacement: parts[3].clone(),
                op_id: Some(next_console_op("edit")),
                summary: Some(format!("ide::ops::edit L{s}..L{e}")),
            };
            return match store.edit_doc(req) {
                Ok(v) => (
                    v["ok"] == true,
                    format!(
                        "Committed rev {} (status={}, hash={})",
                        v["revision"],
                        v["status"].as_str().unwrap_or("?"),
                        v["content_hash"].as_str().unwrap_or("?")
                    ),
                ),
                Err(err) => (false, err),
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
            return match store.replace_all_atomic(crate::merge::ReplaceAllRequest {
                file_id: parts[0].clone(),
                provider: "rust_script".into(),
                agent_id: "console".into(),
                base_revision: None,
                base_content_hash: None,
                fencing_token: None,
                find: parts[1].clone(),
                replace: parts[2].clone(),
                op_id: Some(next_console_op("replace")),
            }) {
                Ok(v) => (
                    v["ok"] == true,
                    format!(
                        "Replaced {} instance(s), rev {} ({})",
                        v["replacements"].as_u64().unwrap_or(0),
                        v["revision"],
                        v["status"].as_str().unwrap_or("?")
                    ),
                ),
                Err(err) => (false, err),
            };
        }

        Self::eval_query_statement(store, clean)
    }

    fn eval_query_statement(store: &DocumentStore, clean: &str) -> (bool, String) {
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
            return match store.diff_revisions(&parts[0], r_a, r_b, 2) {
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
            let doc = match store.read_doc(&parts[0], None, None) {
                Ok(d) => d,
                Err(e) => return (false, e),
            };
            let needle = &parts[1];
            let hits: Vec<String> = doc["content"]
                .as_str()
                .unwrap_or("")
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(needle))
                .map(|(idx, line)| format!("  L{}: {}", idx + 1, line.trim()))
                .collect();
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
            let doc = match store.read_doc(arg, None, None) {
                Ok(d) => d,
                Err(e) => return (false, e),
            };
            let content = doc["content"].as_str().unwrap_or("");
            return match editcore::agent_api::call("md_outline", &json!({ "text": content })) {
                Ok(v) => (
                    true,
                    serde_json::to_string_pretty(&v["outline"]).unwrap_or_default(),
                ),
                Err(e) => (false, e),
            };
        }

        (
            false,
            format!(
                "Unrecognized statement: '{}'. Type 'ide::context::help()' to view available functions.",
                clean
            ),
        )
    }

    fn format_docs_list(store: &DocumentStore) -> String {
        let docs_json = store.list_docs();
        let mut out = String::from("ide::data::files:\n");
        if let Some(list) = docs_json["docs"].as_array() {
            for d in list {
                out.push_str(&format!(
                    "  - {} [rev={}, lang={}]\n",
                    d["file_id"].as_str().unwrap_or("?"),
                    d["current_revision"].as_u64().unwrap_or(0),
                    d["language"].as_str().unwrap_or("?")
                ));
            }
        }
        out
    }

    fn help_text() -> &'static str {
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
        let store = DocumentStore::default();
        let script = r#"
            ide::context::status();
            ide::ops::open("src/algo.rs", "rust", "fn solve() -> i32 { 10 }\n");
            ide::ops::edit("src/algo.rs", 1, 1, "pub fn solve() -> i32 { 42 }");
            ide::ops::diff("src/algo.rs", 1, 2);
            ide::ops::search("src/algo.rs", "solve");
        "#;

        let res = RustConsoleEvaluator::eval(&store, script);
        assert!(res.ok, "Script execution must succeed: {:?}", res.steps);
        assert_eq!(res.steps.len(), 5);

        let read = store.read_doc("src/algo.rs", None, None).unwrap();
        assert!(read["content"]
            .as_str()
            .unwrap()
            .contains("pub fn solve() -> i32 { 42 }"));
        assert_eq!(read["current_revision"], 2);
        assert!(res.steps[3].output.contains("@@ -1,1 +1,1 @@"));
        assert!(res.steps[3]
            .output
            .contains("+pub fn solve() -> i32 { 42 }"));
    }

    #[test]
    fn test_rust_script_help_and_unknown() {
        let store = DocumentStore::default();
        let res = RustConsoleEvaluator::eval(&store, "ide::ops::help()");
        assert!(res.ok);
        assert!(res.steps[0].output.contains("ide::context::status()"));

        let fail = RustConsoleEvaluator::eval(&store, "ide::ops::non_existent()");
        assert!(!fail.ok);
        assert!(fail.steps[0].output.contains("Unrecognized statement"));
    }
}
