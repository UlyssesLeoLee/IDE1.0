//! 给智能功能 (内建对话 / 外部 LangGraph) 留的 API: 一张**静态工具表** + 一个统一入口。
//!
//! 约定 (docs/02_详细设计/CODING-STANDARDS.md §7):
//! - 零成本: 这里只有静态数据和纯函数, 没人调用就不占任何资源, 不起线程、不联网、不加载图运行器。
//! - 工具名 / 描述 / 参数 schema 是对外契约 (OpenAI function-calling 格式, LangGraph 的 tool 直接可用)。
//! - **写类工具只产出补丁提案** (`propose_patch`, 版本化 PatchProposal), 由宿主展示差异、用户接受后才套用 (REQ FR-043 / FR-044)。
//! - 读类工具只读调用方交上来的文本 / 行, 不碰磁盘; 文档数据的外发要用户授权 (NFR-026) —— 那是调用方(对话面板)的责任, 这里不联网。

use serde_json::{json, Value};

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// true = 会改文档 (只产出提案)
    pub write: bool,
    /// 参数 JSON Schema
    pub schema: &'static str,
}

pub const TOOLS: &[Tool] = &[
    Tool {
        name: "md_outline",
        description: "解析 Markdown 文本的标题大纲, 返回 [{level,title,line}] (line 从 1 起)",
        write: false,
        schema: r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
    },
    Tool {
        name: "md_render",
        description: "把 Markdown 渲染成已转义的 HTML (只读)",
        write: false,
        schema: r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
    },
    Tool {
        name: "md_find",
        description: "按节点类型做结构化搜索: kind = heading / link / image / fence / task / todo; query 为空 = 全部 (heading 可写 #2 限定层级, fence 写语言名)。返回 [{line,kind,text,detail}]",
        write: false,
        schema: r#"{"type":"object","properties":{"text":{"type":"string"},"kind":{"type":"string","enum":["heading","link","image","fence","task","todo"]},"query":{"type":"string"}},"required":["text","kind"]}"#,
    },
    Tool {
        name: "md_lint",
        description: "检查 Markdown 文档质量 (标题跳级 / 重复标题 / 围栏未闭合 / 空链接 / 图片缺 alt / 空 mermaid), 返回 [{line,rule,message}]",
        write: false,
        schema: r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
    },
    Tool {
        name: "search_text",
        description: "在给定行里查找: 返回命中数与位置 (op 固定 hits)",
        write: false,
        schema: r#"{"type":"object","properties":{"lines":{"type":"array","items":{"type":"string"}},"needle":{"type":"string"},"use_regex":{"type":"boolean"},"match_case":{"type":"boolean"},"whole_word":{"type":"boolean"}},"required":["lines","needle"]}"#,
    },
    Tool {
        name: "propose_patch",
        description: "提议把文档第 start 行起的 remove 行替换为 insert (行号从 0 起)。只是提案: 不会直接修改文档, 用户在界面里审核差异后才套用",
        write: true,
        schema: r#"{"type":"object","properties":{"start":{"type":"integer","minimum":0},"remove":{"type":"integer","minimum":0},"insert":{"type":"array","items":{"type":"string"}},"total_rows":{"type":"integer","minimum":0},"reason":{"type":"string"}},"required":["start","remove","insert","total_rows"]}"#,
    },
    Tool {
        name: "plugin_list",
        description: "列出已安装的插件 (id / 名称 / 版本 / 是否启用 / 贡献点)",
        write: false,
        schema: r#"{"type":"object","properties":{}}"#,
    },
];

/// 单个提案最多改动的行数 (防止一次提案把整个大文件换掉)
const MAX_PATCH_LINES: usize = 2000;

/// OpenAI `tools` 数组格式
pub fn tools_json() -> Value {
    Value::Array(
        TOOLS
            .iter()
            .map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": serde_json::from_str::<Value>(t.schema).unwrap_or(Value::Null) }, "x-write": t.write }))
            .collect(),
    )
}

/// 一行大纲项
fn outline(text: &str) -> Value {
    let mut in_fence = false;
    let mut out = Vec::new();
    for (i, l) in text.replace("\r\n", "\n").split('\n').enumerate() {
        if l.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let h = l.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&h) && l[h..].starts_with(' ') {
            out.push(json!({ "level": h, "title": l[h + 1..].trim().trim_end_matches('#').trim(), "line": i + 1 }));
        }
    }
    Value::Array(out)
}

fn str_arg<'a>(args: &'a Value, k: &str) -> Result<&'a str, String> {
    args.get(k).and_then(Value::as_str).ok_or_else(|| format!("缺少字符串参数 {k}"))
}
fn usize_arg(args: &Value, k: &str) -> Result<usize, String> {
    args.get(k).and_then(Value::as_u64).map(|v| v as usize).ok_or_else(|| format!("缺少整数参数 {k}"))
}

/// 调用工具。未知工具 / 参数不对 = Err (调用方把错误文字回给模型)
pub fn call(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "md_outline" => Ok(json!({ "outline": outline(str_arg(args, "text")?) })),
        "md_find" => Ok(json!({ "hits": crate::md_struct::find(str_arg(args, "text")?, str_arg(args, "kind")?, args.get("query").and_then(Value::as_str).unwrap_or(""))? })),
        "md_lint" => Ok(json!({ "issues": crate::md_lint::lint(str_arg(args, "text")?) })),
        "md_render" => Ok(json!({ "html": crate::md_render::render(str_arg(args, "text")?) })),
        "search_text" => {
            let mut a = args.clone();
            a["op"] = json!("hits");
            let req: crate::search_ops::SearchReq = serde_json::from_value(a).map_err(|e| format!("search_text 参数无效: {e}"))?;
            crate::pure_api::search_text(&req)
        }
        "propose_patch" => {
            let (start, remove, total) = (usize_arg(args, "start")?, usize_arg(args, "remove")?, usize_arg(args, "total_rows")?);
            let insert: Vec<String> = args
                .get("insert")
                .and_then(Value::as_array)
                .ok_or("缺少数组参数 insert")?
                .iter()
                .map(|v| v.as_str().map(String::from).ok_or_else(|| "insert 的元素必须是字符串".to_string()))
                .collect::<Result<_, _>>()?;
            if start > total || start.checked_add(remove).is_none_or(|e| e > total) {
                return Err(format!("补丁范围越界: start={start} remove={remove} 文档共 {total} 行"));
            }
            if insert.len().max(remove) > MAX_PATCH_LINES {
                return Err(format!("单个提案最多改动 {MAX_PATCH_LINES} 行, 请拆小"));
            }
            if insert.iter().any(|l| l.contains('\n') || l.contains('\r')) {
                return Err("insert 的每个元素必须是一行 (不含换行符)".into());
            }
            Ok(json!({ "kind": "patch_proposal", "version": 1, "start": start, "remove": remove, "insert": insert, "reason": args.get("reason").and_then(Value::as_str).unwrap_or("") }))
        }
        "plugin_list" => {
            let root = crate::plugin_host::store::default_root();
            let list: Vec<Value> = crate::plugin_host::store::list(&root)
                .into_iter()
                .map(|p| json!({ "id": p.manifest.id, "name": p.manifest.name, "version": p.manifest.version, "enabled": p.enabled, "contributes": p.manifest.contributes.keys().collect::<Vec<_>>() }))
                .collect();
            Ok(json!({ "plugins": list }))
        }
        _ => Err(format!("未知工具: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_valid_schema_and_dispatches() {
        for t in TOOLS {
            assert!(serde_json::from_str::<Value>(t.schema).is_ok(), "{} 的 schema 不是合法 JSON", t.name);
        }
        let names: Vec<_> = TOOLS.iter().map(|t| t.name).collect();
        let mut dedup = names.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), names.len(), "工具名不能重复");
        assert!(!call("nope", &json!({})).is_ok_and(|_| true));
        assert!(tools_json().as_array().unwrap().iter().all(|t| t["function"]["parameters"]["type"] == "object"));
    }

    #[test]
    fn outline_skips_fences_and_reports_lines() {
        let r = call("md_outline", &json!({ "text": "# A\n```\n# not heading\n```\n## B ##\ntext" })).unwrap();
        assert_eq!(r["outline"], json!([{ "level": 1, "title": "A", "line": 1 }, { "level": 2, "title": "B", "line": 5 }]));
    }

    #[test]
    fn search_text_counts_hits() {
        let r = call("search_text", &json!({ "lines": ["foo bar", "foo"], "needle": "foo" })).unwrap();
        assert_eq!(r["total"], 2, "{r}");
    }

    #[test]
    fn write_tool_only_produces_a_validated_proposal() {
        assert!(TOOLS.iter().find(|t| t.name == "propose_patch").unwrap().write);
        let ok = call("propose_patch", &json!({ "start": 1, "remove": 1, "insert": ["x"], "total_rows": 3, "reason": "r" })).unwrap();
        assert_eq!(ok["kind"], "patch_proposal");
        assert!(call("propose_patch", &json!({ "start": 3, "remove": 1, "insert": [], "total_rows": 3 })).is_err(), "越界");
        assert!(call("propose_patch", &json!({ "start": 0, "remove": 0, "insert": ["a\nb"], "total_rows": 1 })).is_err(), "一元素一行");
        assert!(call("propose_patch", &json!({ "start": 0, "remove": 0, "insert": vec!["x"; 2001], "total_rows": 1 })).is_err(), "上限");
        assert!(call("propose_patch", &json!({ "start": usize::MAX, "remove": 1, "insert": [], "total_rows": 1 })).is_err(), "溢出不 panic");
    }
}
