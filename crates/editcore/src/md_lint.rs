//! Markdown 文档质量检查 (REQ FR-033): 纯文本规则, 按需调用, 报告「行 / 规则 ID / 说明」。
//! 不读磁盘 (链接目标是否存在不在这里查), 不改文档; 围栏代码块里的内容一律不检查。

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Issue {
    /// 行号, 从 1 起
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

fn issue(line: usize, rule: &'static str, message: impl Into<String>) -> Issue {
    Issue { line, rule, message: message.into() }
}

/// 规则: MD-H1 标题层级跳级 / MD-DUP 同级重复标题 / MD-FENCE 围栏未闭合 / MD-LINK 链接目标为空 / MD-IMG 图片缺 alt / MD-EMPTY-FENCE 空的 mermaid 围栏
pub fn lint(text: &str) -> Vec<Issue> {
    let mut out = Vec::new();
    let mut fence: Option<(usize, String, usize)> = None; // (起始行, 语言, 起始后的非空行数)
    let mut prev_level = 0usize;
    let mut seen: Vec<(usize, String)> = Vec::new();
    for (i, l) in text.replace("\r\n", "\n").split('\n').enumerate() {
        let no = i + 1;
        let t = l.trim_start();
        if t.starts_with("```") {
            match fence.take() {
                Some((start, lang, body)) => {
                    if lang == "mermaid" && body == 0 {
                        out.push(issue(start, "MD-EMPTY-FENCE", "mermaid 代码块是空的"));
                    }
                }
                None => fence = Some((no, t.trim_start_matches('`').trim().to_string(), 0)),
            }
            continue;
        }
        if let Some(f) = fence.as_mut() {
            if !l.trim().is_empty() {
                f.2 += 1;
            }
            continue;
        }
        let h = l.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&h) && l[h..].starts_with(' ') {
            if prev_level > 0 && h > prev_level + 1 {
                out.push(issue(no, "MD-H1", format!("标题从 {} 级直接跳到 {} 级", prev_level, h)));
            }
            prev_level = h;
            let title = l[h + 1..].trim().to_string();
            if seen.iter().any(|(lv, t)| *lv == h && *t == title) {
                out.push(issue(no, "MD-DUP", format!("同级标题重复: {title}")));
            } else {
                seen.push((h, title));
            }
        }
        if l.contains("]()") {
            out.push(issue(no, "MD-LINK", "链接目标为空 ](…)"));
        }
        if l.contains("![](") {
            out.push(issue(no, "MD-IMG", "图片缺少 alt 文字"));
        }
    }
    if let Some((start, _, _)) = fence {
        out.push(issue(start, "MD-FENCE", "代码围栏没有闭合"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(t: &str) -> Vec<(usize, &'static str)> {
        lint(t).into_iter().map(|i| (i.line, i.rule)).collect()
    }

    #[test]
    fn clean_document_has_no_issues() {
        assert!(lint("# A\n\n## B\n\ntext [x](a.md) ![alt](i.png)\n\n```rust\nlet x = 1;\n```\n").is_empty());
    }

    #[test]
    fn reports_each_rule_with_line_numbers() {
        assert_eq!(rules("# A\n### C"), [(2, "MD-H1")]);
        assert_eq!(rules("# A\n## B\n## B"), [(3, "MD-DUP")]);
        assert_eq!(rules("a\n```\nx"), [(2, "MD-FENCE")]);
        assert_eq!(rules("[x]()\n![](a.png)"), [(1, "MD-LINK"), (2, "MD-IMG")]);
        assert_eq!(rules("```mermaid\n\n```"), [(1, "MD-EMPTY-FENCE")]);
    }

    #[test]
    fn fenced_code_is_not_checked() {
        assert!(lint("```\n# A\n#### C\n[x]()\n```").is_empty());
    }
}
