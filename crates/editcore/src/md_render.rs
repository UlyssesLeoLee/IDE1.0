//! Markdown -> HTML 预览渲染 (REQ FR-025 / FR-016 / FR-022; NFR-024 / NFR-025)。
//!
//! - 只读: 预览不写回文档 (INV-02), 输入一份文本, 输出一段 HTML。
//! - **安全**: 所有文字一律转义, 不透传用户写的 HTML; 链接 / 图片只放行 http(s) / mailto / 相对路径 / 锚点。
//! - Obsidian 风格语法: Front Matter、callout `> [!note]`、`==高亮==`、`%%注释%%`、`[[wikilink]]` / `![[embed]]`、`#tag`、任务列表、表格、删除线。
//! - 围栏代码块按语言交给插件 (`markdown.fence`): 这里只输出带 `data-lang` 的 `<pre class="md-fence">`, 渲染器 (如 Pele) 不在核心里。

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn safe_url(u: &str) -> bool {
    let l = u.trim().to_ascii_lowercase();
    if l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:") || l.starts_with('#') {
        return true;
    }
    // 其余带 scheme (javascript: data: file: …) 一律拒绝; 相对路径放行
    !l.split(['/', '?', '#']).next().unwrap_or("").contains(':')
}

fn slug(s: &str) -> String {
    let mut o = String::new();
    for c in s.trim().chars() {
        if c.is_alphanumeric() {
            o.extend(c.to_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !o.ends_with('-') {
            o.push('-');
        }
    }
    o.trim_matches('-').to_string()
}

/// 找 `pat` 之后的下一个 `end`, 返回 (内部, 之后的下标)
fn take_until<'a>(s: &'a str, from: usize, end: &str) -> Option<(&'a str, usize)> {
    s[from..].find(end).filter(|&p| p > 0).map(|p| (&s[from..from + p], from + p + end.len()))
}

/// 嵌套深度上限: 超过就把剩下的当普通文字 (防止恶意文档靠无限嵌套的 `>` / `**` 把栈撑爆)
const MAX_DEPTH: usize = 32;
/// 预览输入上限 (超过的部分不渲染)
const MAX_INPUT: usize = 4 * 1024 * 1024;

fn inline(src: &str, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return esc(src);
    }
    let mut out = String::new();
    let mut i = 0;
    let b = src.as_bytes();
    while i < src.len() {
        let rest = &src[i..];
        let two = |m: &str, tag: &str, out: &mut String, i: &mut usize| -> bool {
            if rest.starts_with(m) {
                if let Some((inner, next)) = take_until(src, *i + m.len(), m) {
                    out.push_str(&format!("<{tag}>{}</{tag}>", inline(inner, depth + 1)));
                    *i = next;
                    return true;
                }
            }
            false
        };
        if rest.starts_with('`') {
            if let Some((inner, next)) = take_until(src, i + 1, "`") {
                out.push_str(&format!("<code>{}</code>", esc(inner)));
                i = next;
                continue;
            }
        }
        if rest.starts_with("%%") {
            if let Some((_, next)) = take_until(src, i + 2, "%%") {
                i = next;
                continue; // 注释不显示
            }
        }
        if two("**", "strong", &mut out, &mut i) || two("~~", "del", &mut out, &mut i) || two("==", "mark", &mut out, &mut i) {
            continue;
        }
        if rest.starts_with('*') && !rest.starts_with("**") {
            if let Some((inner, next)) = take_until(src, i + 1, "*") {
                out.push_str(&format!("<em>{}</em>", inline(inner, depth + 1)));
                i = next;
                continue;
            }
        }
        // ![[embed]] / [[wikilink|别名]]
        let (is_embed, wl) = (rest.starts_with("![["), rest.starts_with("[["));
        if is_embed || wl {
            if let Some((inner, next)) = take_until(src, i + if is_embed { 3 } else { 2 }, "]]") {
                let (target, alias) = inner.split_once('|').unwrap_or((inner, inner));
                out.push_str(&format!("<a class=\"wikilink\" data-target=\"{}\" href=\"#\">{}{}</a>", esc(target), if is_embed { "⧉ " } else { "" }, esc(alias)));
                i = next;
                continue;
            }
        }
        // [text](url) / ![alt](url)
        let img = rest.starts_with("![");
        if img || rest.starts_with('[') {
            let open = i + if img { 2 } else { 1 };
            if let Some((label, after)) = take_until(src, open, "](") {
                if let Some((url, next)) = take_until(src, after, ")") {
                    let url = url.split_whitespace().next().unwrap_or("");
                    if safe_url(url) {
                        if img {
                            out.push_str(&format!("<img alt=\"{}\" src=\"{}\" loading=\"lazy\">", esc(label), esc(url)));
                        } else {
                            out.push_str(&format!("<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>", esc(url), inline(label, depth + 1)));
                        }
                    } else {
                        out.push_str(&esc(&src[i..next])); // 不安全的链接按普通文字显示
                    }
                    i = next;
                    continue;
                }
            }
        }
        // #tag (行首或空白之后)
        if rest.starts_with('#') && (i == 0 || b[i - 1] == b' ') {
            let t: String = rest[1..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '/').collect();
            if !t.is_empty() && !t.chars().all(|c| c.is_ascii_digit()) {
                out.push_str(&format!("<span class=\"tag\">#{}</span>", esc(&t)));
                i += 1 + t.len();
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push_str(&esc(&c.to_string()));
        i += c.len_utf8();
    }
    out
}

fn is_hr(l: &str) -> bool {
    let t = l.trim();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}
fn table_sep(l: &str) -> bool {
    let t = l.trim();
    t.contains('-') && t.contains('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}
fn cells(l: &str) -> Vec<String> {
    let t = l.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}
fn list_item(l: &str) -> Option<(usize, bool, &str)> {
    let ind = l.len() - l.trim_start().len();
    let b = l.trim_start();
    if let Some(r) = b.strip_prefix("- ").or_else(|| b.strip_prefix("* ")).or_else(|| b.strip_prefix("+ ")) {
        return Some((ind, false, r));
    }
    let d = b.chars().take_while(|c| c.is_ascii_digit()).count();
    if d > 0 && b[d..].starts_with(". ") {
        return Some((ind, true, &b[d + 2..]));
    }
    None
}

/// 渲染整篇文档
pub fn render(src: &str) -> String {
    render_depth(src, 0)
}

fn render_depth(src: &str, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("<p>{}</p>", esc(src));
    }
    let src = if src.len() > MAX_INPUT { &src[..src.floor_char_boundary(MAX_INPUT)] } else { src };
    let text = src.replace("\r\n", "\n");
    let mut lines: Vec<&str> = text.split('\n').collect();
    let mut out = String::new();
    // Front Matter: 折叠显示, 不解析
    if lines.first().map(|l| l.trim_end()) == Some("---") {
        if let Some(end) = lines.iter().skip(1).position(|l| l.trim_end() == "---" || l.trim_end() == "...") {
            out.push_str(&format!("<details class=\"frontmatter\"><summary>Front Matter</summary><pre>{}</pre></details>", esc(&lines[1..=end].join("\n"))));
            lines.drain(..end + 2);
        }
    }
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
        } else if let Some(fence) = l.trim_start().strip_prefix("```").map(|_| l.trim_start().chars().take_while(|&c| c == '`').count()) {
            let marks = "`".repeat(fence);
            let lang = l.trim_start()[fence..].trim().split_whitespace().next().unwrap_or("");
            let mut body = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(&marks) {
                body.push(lines[i]);
                i += 1;
            }
            i += 1;
            out.push_str(&format!("<pre class=\"md-fence\" data-lang=\"{}\"><code>{}</code></pre>", esc(lang), esc(&body.join("\n"))));
        } else if let Some(h) = Some(l.chars().take_while(|&c| c == '#').count()).filter(|&h| (1..=6).contains(&h) && l[h..].starts_with(' ')) {
            let t = l[h + 1..].trim().trim_end_matches('#').trim();
            out.push_str(&format!("<h{h} id=\"{}\">{}</h{h}>", esc(&slug(t)), inline(t, depth)));
            i += 1;
        } else if is_hr(l) {
            out.push_str("<hr>");
            i += 1;
        } else if l.starts_with('>') {
            let mut q = Vec::new();
            while i < lines.len() && lines[i].starts_with('>') {
                q.push(lines[i].trim_start_matches('>').strip_prefix(' ').unwrap_or(lines[i].trim_start_matches('>')));
                i += 1;
            }
            // callout: > [!type] 标题
            let first = q[0].trim();
            if let Some(r) = first.strip_prefix("[!").and_then(|r| r.split_once(']')) {
                let kind: String = r.0.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
                let title = if r.1.trim().is_empty() { r.0.to_string() } else { r.1.trim().to_string() };
                out.push_str(&format!("<div class=\"callout callout-{}\"><div class=\"callout-title\">{}</div>{}</div>", esc(&kind.to_lowercase()), inline(&title, depth), render(&q[1..].join("\n"))));
            } else {
                out.push_str(&format!("<blockquote>{}</blockquote>", render(&q.join("\n"))));
            }
        } else if i + 1 < lines.len() && l.contains('|') && table_sep(lines[i + 1]) {
            let head = cells(l);
            let mut t = String::from("<table><thead><tr>");
            for c in &head {
                t.push_str(&format!("<th>{}</th>", inline(c, depth)));
            }
            t.push_str("</tr></thead><tbody>");
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                t.push_str("<tr>");
                for c in cells(lines[i]) {
                    t.push_str(&format!("<td>{}</td>", inline(&c, depth)));
                }
                t.push_str("</tr>");
                i += 1;
            }
            out.push_str(&t);
            out.push_str("</tbody></table>");
        } else if let Some((ind0, ordered, _)) = list_item(l) {
            let tag = if ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>"));
            while i < lines.len() {
                let Some((ind, ord, item)) = list_item(lines[i]) else { break };
                if ind < ind0 || ord != ordered {
                    break;
                }
                let (cls, body) = match (item.strip_prefix("[ ] "), item.strip_prefix("[x] ").or_else(|| item.strip_prefix("[X] "))) {
                    (Some(r), _) => (" class=\"task\"", format!("<input type=\"checkbox\" disabled> {}", inline(r, depth))),
                    (_, Some(r)) => (" class=\"task\"", format!("<input type=\"checkbox\" checked disabled> {}", inline(r, depth))),
                    _ => ("", inline(item, depth)),
                };
                out.push_str(&format!("<li{cls}>{body}</li>"));
                i += 1;
            }
            out.push_str(&format!("</{tag}>"));
        } else {
            let mut p = vec![l.trim()];
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() && !lines[i].starts_with('>') && !lines[i].trim_start().starts_with("```") && list_item(lines[i]).is_none() && !is_hr(lines[i]) && !lines[i].starts_with('#') {
                p.push(lines[i].trim());
                i += 1;
            }
            let html = inline(&p.join("\n"), depth);
            if !html.trim().is_empty() {
                out.push_str(&format!("<p>{}</p>", html.replace('\n', "<br>")));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn user_html_is_always_escaped() {
        let h = render("<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>");
        assert!(!h.contains("<script"), "{h}");
        assert!(!h.contains("<img src=x"), "{h}");
        assert!(h.contains("&lt;script&gt;"));
    }

    #[test]
    fn unsafe_urls_are_not_linked() {
        let h = render("[x](javascript:alert(1)) [y](data:text/html,hi) ![z](file:///c:/a.png) [ok](https://a.b/c) [rel](docs/a.md#h) [m](mailto:a@b.c)");
        assert!(!h.contains("href=\"javascript"), "{h}");
        assert!(!h.contains("src=\"file"), "{h}");
        assert!(!h.contains("href=\"data"), "{h}");
        assert!(h.contains("href=\"https://a.b/c\""));
        assert!(h.contains("href=\"docs/a.md#h\""));
        assert!(h.contains("href=\"mailto:a@b.c\""));
    }

    #[test]
    fn basic_blocks() {
        let h = render("# Title\n\npara **b** *i* ~~s~~ ==m== `c<d>`\n\n- a\n- [x] done\n\n1. one\n2. two\n\n---\n\n| A | B |\n| --- | --- |\n| 1 | 2 |");
        for want in ["<h1 id=\"title\">Title</h1>", "<strong>b</strong>", "<em>i</em>", "<del>s</del>", "<mark>m</mark>", "<code>c&lt;d&gt;</code>", "<ul>", "checked disabled", "<ol>", "<hr>", "<th>A</th>", "<td>2</td>"] {
            assert!(h.contains(want), "缺 {want}: {h}");
        }
    }

    #[test]
    fn deep_nesting_does_not_overflow_the_stack() {
        let quotes = format!("{} x", ">".repeat(5000));
        assert!(render(&quotes).contains('x'));
        let em = format!("{}x{}", "**".repeat(5000), "**".repeat(5000));
        assert!(render(&em).contains('x'));
    }

    #[test]
    fn obsidian_syntax() {
        let h = render("---\ntitle: x\n---\n\n> [!warning] 注意\n> 内容\n\n[[Note|别名]] #tag %%隐藏%% 文字");
        assert!(h.contains("<details class=\"frontmatter\">"), "{h}");
        assert!(h.contains("callout callout-warning") && h.contains("注意"), "{h}");
        assert!(h.contains("data-target=\"Note\"") && h.contains("别名"));
        assert!(h.contains("<span class=\"tag\">#tag</span>"));
        assert!(!h.contains("隐藏"), "注释不显示");
    }

    #[test]
    fn fences_are_left_to_plugins_and_stay_verbatim() {
        let h = render("```mermaid\ngraph TD\n  A-->B\n```\n\n````\n```inner```\n````");
        assert!(h.contains("data-lang=\"mermaid\"") && h.contains("A--&gt;B"), "{h}");
        assert!(h.contains("```inner```"), "长围栏里的短围栏原样: {h}");
    }
}
