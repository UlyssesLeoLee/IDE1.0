//! Markdown 预览的编辑 / 检查 / 导出 (纯函数, 有单测; 界面接线在 `app/mdopts.rs`)。
//! 检查复用 `editcore::md_lint` (和 Tauri 版同一套规则), 导出复用 `editcore::md_render` + 同一份默认模板;
//! 预览里的编辑 = 块的源码行范围 + 任务复选框的一行改写, 都以「行补丁」交给 `Doc::apply_line_patch` (一步撤销)。

use crate::markdown::Parsed;
use std::path::{Path, PathBuf};

/// 导出模板 (`{{title}}` 标题, `{{body}}` 渲染后的正文), 和 Tauri 版 `MD_TEMPLATE_DEFAULT` 一致
pub const TEMPLATE_DEFAULT: &str = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{{title}}</title>\n<style>body{max-width:860px;margin:2em auto;padding:0 1em;font:16px/1.7 sans-serif}pre{background:#f4f4f4;padding:.6em;overflow:auto}code{background:#f4f4f4;padding:0 .25em}table{border-collapse:collapse}td,th{border:1px solid #ccc;padding:3px 10px}blockquote{border-left:3px solid #ccc;margin:0;padding:0 1em;color:#555}.callout{border-left:4px solid #58a6ff;background:#f4f8ff;padding:.4em 1em}</style></head>\n<body>\n{{body}}\n</body></html>\n";

/// 自定义模板文件 (设置目录下的 `md-template.html`); 没有 / 读不了 / 缺 `{{body}}` 就用默认的
pub fn load_template() -> String {
    crate::settings::path()
        .and_then(|p| p.parent().map(|d| d.join("md-template.html")))
        .and_then(|p| std::fs::metadata(&p).ok().filter(|m| m.len() <= 256 * 1024).and_then(|_| std::fs::read_to_string(p).ok()))
        .filter(|t| t.contains("{{body}}"))
        .unwrap_or_else(|| TEMPLATE_DEFAULT.to_string())
}

fn escape_title(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '<' | '>' | '&' | '"')).collect()
}

/// 整篇文档 -> 完整的 HTML 页面
pub fn export_html(name: &str, lines: &[String], template: &str) -> String {
    let title = escape_title(name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name));
    let body = editcore::md_render::render(&lines.join("\n"));
    template.replace("{{title}}", &title).replace("{{body}}", &body)
}

/// 同名 `.html`
pub fn export_path(doc: &Path) -> PathBuf {
    doc.with_extension("html")
}

/// 一条检查结果 (行号 0 起)
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

pub fn lint(lines: &[String]) -> Vec<Problem> {
    editcore::md_lint::lint(&lines.join("\n")).into_iter().map(|i| Problem { line: i.line.saturating_sub(1), rule: i.rule, message: i.message }).collect()
}

/// 块 `i` 在源码里占的行范围 `[起, 止)`: 到下一块之前, 去掉尾部空行 (围栏块自带结束行)
pub fn block_span(blocks: &[Parsed], i: usize, lines: &[String]) -> Option<(usize, usize)> {
    let start = blocks.get(i)?.line;
    let limit = blocks.get(i + 1).map(|b| b.line).unwrap_or(lines.len()).min(lines.len());
    let mut end = limit;
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    (start < lines.len()).then_some((start, end.max(start + 1)))
}

/// 任务行 `- [ ] x` <-> `- [x] x`; 不是任务行 = None
pub fn toggle_task(line: &str) -> Option<String> {
    let at = ["[ ]", "[x]", "[X]"].iter().filter_map(|m| line.find(m)).min()?;
    let head = line[..at].trim();
    if !(head == "-" || head == "*" || head == "+" || (head.ends_with('.') && head[..head.len() - 1].chars().all(|c| c.is_ascii_digit()) && head.len() > 1)) {
        return None;
    }
    let mark = if line[at..].starts_with("[ ]") { "[x]" } else { "[ ]" };
    Some(format!("{}{}{}", &line[..at], mark, &line[at + 3..]))
}

/// 编辑框里的文字 -> 行 (去掉 `\r`; 空 = 删掉整块)
pub fn text_to_lines(s: &str) -> Vec<String> {
    let t = s.replace("\r\n", "\n").replace('\r', "\n");
    let t = t.trim_end_matches('\n');
    if t.is_empty() { vec![] } else { t.split('\n').map(str::to_string).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse;

    fn ls(s: &str) -> Vec<String> {
        s.split('\n').map(str::to_string).collect()
    }

    #[test]
    fn export_wraps_the_rendered_body_in_the_template_and_strips_markup_from_the_title() {
        let html = export_html("a<b>.md", &ls("# Hi\n\ntext"), TEMPLATE_DEFAULT);
        assert!(html.starts_with("<!doctype html>") && html.contains("<title>ab</title>"), "{html}");
        assert!(html.contains("<h1") && html.contains("text"));
        assert!(!html.contains("{{"), "占位符都被换掉");
        let custom = export_html("x.md", &ls("$& $1"), "<main>{{body}}</main>");
        assert!(custom.contains("$&amp;") || custom.contains("$&"), "正文里的 $ 记号原样保留: {custom}");
        assert_eq!(export_path(Path::new("C:/d/x.md")), PathBuf::from("C:/d/x.html"));
    }

    #[test]
    fn script_in_the_document_is_escaped_in_the_export() {
        let html = export_html("x.md", &ls("<script>alert(1)</script>"), TEMPLATE_DEFAULT);
        assert!(!html.contains("<script>alert"), "{html}");
    }

    #[test]
    fn lint_reports_zero_based_lines_and_ignores_fenced_code() {
        let p = lint(&ls("# A\n\n### C\n\n```\n### D\n```\n[x]()"));
        assert!(p.iter().any(|x| x.rule == "MD-H1" && x.line == 2), "{p:?}");
        assert!(p.iter().any(|x| x.rule == "MD-LINK" && x.line == 7));
        assert!(!p.iter().any(|x| x.line == 5), "围栏里不检查");
        assert!(lint(&ls("# ok\n\ntext")).is_empty());
    }

    #[test]
    fn block_spans_drop_trailing_blank_lines_and_cover_fences() {
        let lines = ls("# T\n\npara a\npara b\n\n```rs\nfn x(){}\n```\n\n- [ ] t\n\n");
        let b = parse(&lines);
        assert_eq!(b.len(), 4);
        assert_eq!(block_span(&b, 0, &lines), Some((0, 1)));
        assert_eq!(block_span(&b, 1, &lines), Some((2, 4)));
        assert_eq!(block_span(&b, 2, &lines), Some((5, 8)), "围栏含结束行");
        assert_eq!(block_span(&b, 3, &lines), Some((9, 10)), "最后一块去掉尾部空行");
        assert_eq!(block_span(&b, 9, &lines), None);
    }

    #[test]
    fn task_lines_toggle_both_ways_and_other_lines_are_left_alone() {
        assert_eq!(toggle_task("- [ ] a").as_deref(), Some("- [x] a"));
        assert_eq!(toggle_task("  * [X] a [ ] b").as_deref(), Some("  * [ ] a [ ] b"), "只动行首那个");
        assert_eq!(toggle_task("1. [ ] n").as_deref(), Some("1. [x] n"));
        assert_eq!(toggle_task("text [ ] not a task"), None);
        assert_eq!(toggle_task("- plain"), None);
    }

    #[test]
    fn edited_text_splits_into_lines_and_empty_means_delete() {
        assert_eq!(text_to_lines("a\r\nb\n\n"), vec!["a", "b"]);
        assert!(text_to_lines("\n").is_empty());
        assert_eq!(text_to_lines("x"), vec!["x"]);
    }
}
