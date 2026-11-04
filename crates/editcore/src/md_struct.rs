//! Markdown 结构化搜索 (REQ FR-034): 按节点类型 (标题 / 链接 / 图片 / 代码块 / 任务) 和条件过滤, 结果带行号可直接跳转。
//! 纯文本、只读; 围栏代码块里的内容只算"代码块", 不当作标题 / 链接。

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    /// 行号, 从 1 起
    pub line: usize,
    pub kind: &'static str,
    pub text: String,
    /// 标题层级 / 代码块语言 / 任务是否完成 等附加信息
    pub detail: String,
}

/// kind: heading | link | image | fence | task | todo (未完成任务) ; query 为空 = 全部; 非空 = 文字子串 (忽略大小写);
/// heading 的 query 可写 `#2` / `#2 关键字` 限定层级; fence 的 query 是语言名
pub fn find(text: &str, kind: &str, query: &str) -> Result<Vec<Hit>, String> {
    if !["heading", "link", "image", "fence", "task", "todo"].contains(&kind) {
        return Err(format!("未知的节点类型: {kind} (可用 heading / link / image / fence / task / todo)"));
    }
    let q = query.trim();
    let (level, needle) = match q.strip_prefix('#') {
        Some(rest) if kind == "heading" => {
            let d: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            (d.parse::<usize>().ok(), rest[d.len()..].trim().to_lowercase())
        }
        _ => (None, q.to_lowercase()),
    };
    let ok = |s: &str| needle.is_empty() || s.to_lowercase().contains(&needle);
    let mut out = Vec::new();
    let mut fence: Option<(usize, String)> = None;
    for (i, l) in text.replace("\r\n", "\n").split('\n').enumerate() {
        let no = i + 1;
        let t = l.trim_start();
        if t.starts_with("```") {
            match fence.take() {
                Some(_) => {}
                None => {
                    let lang = t.trim_start_matches('`').trim().to_string();
                    if kind == "fence" && ok(&lang) {
                        out.push(Hit { line: no, kind: "fence", text: if lang.is_empty() { "(无语言)".into() } else { lang.clone() }, detail: lang.clone() });
                    }
                    fence = Some((no, lang));
                }
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        match kind {
            "heading" => {
                let h = l.chars().take_while(|&c| c == '#').count();
                if (1..=6).contains(&h) && l[h..].starts_with(' ') && level.is_none_or(|lv| lv == h) {
                    let title = l[h + 1..].trim().trim_end_matches('#').trim();
                    if ok(title) {
                        out.push(Hit { line: no, kind: "heading", text: title.to_string(), detail: format!("H{h}") });
                    }
                }
            }
            "link" | "image" => {
                let mut rest = l;
                while let Some(p) = rest.find("](") {
                    let open = rest[..p].rfind('[');
                    let end = rest[p + 2..].find(')');
                    match (open, end) {
                        (Some(o), Some(e)) => {
                            let is_img = o > 0 && rest.as_bytes()[o - 1] == b'!';
                            let label = &rest[o + 1..p];
                            let url = &rest[p + 2..p + 2 + e];
                            if is_img == (kind == "image") && (ok(label) || ok(url)) {
                                out.push(Hit { line: no, kind: if is_img { "image" } else { "link" }, text: label.to_string(), detail: url.to_string() });
                            }
                            rest = &rest[p + 2 + e + 1..];
                        }
                        _ => break,
                    }
                }
            }
            "task" | "todo" => {
                let b = t;
                let done = b.starts_with("- [x] ") || b.starts_with("- [X] ");
                let open = b.starts_with("- [ ] ");
                if (open || (done && kind == "task")) && ok(&b[6..]) {
                    out.push(Hit { line: no, kind: "task", text: b[6..].trim().to_string(), detail: if done { "done".into() } else { "open".into() } });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Title\n\n## Setup\nsee [docs](a.md) and ![logo](i.png)\n\n- [ ] write tests\n- [x] ship\n\n```rust\n# not heading [x](y)\n```\n\n## Usage\n";

    fn lines(r: Vec<Hit>) -> Vec<usize> {
        r.into_iter().map(|h| h.line).collect()
    }

    #[test]
    fn headings_filter_by_level_and_text() {
        assert_eq!(lines(find(DOC, "heading", "").unwrap()), [1, 3, 13]);
        assert_eq!(lines(find(DOC, "heading", "#2").unwrap()), [3, 13]);
        assert_eq!(lines(find(DOC, "heading", "#2 usa").unwrap()), [13]);
        assert_eq!(lines(find(DOC, "heading", "setup").unwrap()), [3]);
    }

    #[test]
    fn links_images_tasks_and_fences() {
        assert_eq!(find(DOC, "link", "").unwrap()[0].detail, "a.md");
        assert_eq!(find(DOC, "image", "").unwrap()[0].text, "logo");
        assert_eq!(lines(find(DOC, "link", "").unwrap()), [4], "围栏里的链接不算");
        assert_eq!(lines(find(DOC, "todo", "").unwrap()), [6]);
        assert_eq!(lines(find(DOC, "task", "").unwrap()), [6, 7]);
        assert_eq!(find(DOC, "fence", "rust").unwrap()[0].line, 9);
    }

    #[test]
    fn unknown_kind_is_an_error() {
        assert!(find(DOC, "nope", "").is_err());
    }
}
