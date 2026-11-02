//! Markdown 预览 (原生): 直接按文档的行解析成块 (标题 / 代码块 / 引用 / 列表 / 表格 / 分隔线 / 段落) 再用 egui 画出来。
//! 不经过 HTML: 块结构和 `editcore::md_render` 的规则一致 (围栏、`>` 引用、`-` `*` `1.` 列表、`|` 表格、`---` 分隔线), 行内只认 `**粗体**` `*斜体*` `` `代码` ``。
//! 不含 Mermaid / 插件渲染 (那些只在 Tauri 版)。每个块带源码起始行, 点击块 = 回到编辑区那一行。

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: usize, text: String },
    Code { lang: String, text: String },
    Quote(String),
    List { ordered: bool, items: Vec<String> },
    Table { rows: Vec<Vec<String>> },
    Rule,
    Para(String),
}

pub struct Parsed {
    pub block: Block,
    /// 源码起始行 (0 起)
    pub line: usize,
}

fn is_rule(l: &str) -> bool {
    let t = l.trim();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

fn list_item(l: &str) -> Option<(bool, &str)> {
    let t = l.trim_start();
    for p in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(p) {
            return Some((false, r));
        }
    }
    let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if d > 0 && t[d..].starts_with(". ") {
        return Some((true, &t[d + 2..]));
    }
    None
}

fn cells(l: &str) -> Vec<String> {
    l.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

fn table_sep(l: &str) -> bool {
    let t = l.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

pub fn parse(lines: &[String]) -> Vec<Parsed> {
    let mut out = vec![];
    let mut i = 0;
    // Front Matter
    if lines.first().map(|l| l.trim_end() == "---").unwrap_or(false) {
        if let Some(end) = lines.iter().skip(1).position(|l| l.trim_end() == "---" || l.trim_end() == "...") {
            i = end + 2;
        }
    }
    while i < lines.len() {
        let l = &lines[i];
        let start = i;
        if l.trim().is_empty() {
            i += 1;
        } else if let Some(fence) = l.trim_start().strip_prefix("```") {
            let lang = fence.trim().to_string();
            let mut body = vec![];
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                body.push(lines[i].as_str());
                i += 1;
            }
            i += 1;
            out.push(Parsed { block: Block::Code { lang, text: body.join("\n") }, line: start });
        } else if let Some(h) = Some(l.chars().take_while(|&c| c == '#').count()).filter(|&h| (1..=6).contains(&h) && l[h..].starts_with(' ')) {
            out.push(Parsed { block: Block::Heading { level: h, text: l[h + 1..].trim().trim_end_matches('#').trim().to_string() }, line: start });
            i += 1;
        } else if is_rule(l) {
            out.push(Parsed { block: Block::Rule, line: start });
            i += 1;
        } else if l.starts_with('>') {
            let mut q = vec![];
            while i < lines.len() && lines[i].starts_with('>') {
                q.push(lines[i].trim_start_matches('>').trim_start().to_string());
                i += 1;
            }
            out.push(Parsed { block: Block::Quote(q.join("\n")), line: start });
        } else if i + 1 < lines.len() && l.contains('|') && table_sep(&lines[i + 1]) {
            let mut rows = vec![cells(l)];
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(cells(&lines[i]));
                i += 1;
            }
            out.push(Parsed { block: Block::Table { rows }, line: start });
        } else if let Some((ordered, _)) = list_item(l) {
            let mut items = vec![];
            while i < lines.len() {
                let Some((o, it)) = list_item(&lines[i]) else { break };
                if o != ordered {
                    break;
                }
                items.push(it.to_string());
                i += 1;
            }
            out.push(Parsed { block: Block::List { ordered, items }, line: start });
        } else {
            let mut p = vec![l.trim().to_string()];
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() && !lines[i].starts_with('#') && !lines[i].starts_with('>') && !lines[i].trim_start().starts_with("```") && list_item(&lines[i]).is_none() && !is_rule(&lines[i]) {
                p.push(lines[i].trim().to_string());
                i += 1;
            }
            out.push(Parsed { block: Block::Para(p.join(" ")), line: start });
        }
    }
    out
}

/// 行内片段: (文字, 粗体, 斜体, 代码)
pub fn inline(s: &str) -> Vec<(String, bool, bool, bool)> {
    let cs: Vec<char> = s.chars().collect();
    let mut out: Vec<(String, bool, bool, bool)> = vec![];
    let mut plain = String::new();
    let mut i = 0;
    let flush = |plain: &mut String, out: &mut Vec<(String, bool, bool, bool)>| {
        if !plain.is_empty() {
            out.push((std::mem::take(plain), false, false, false));
        }
    };
    while i < cs.len() {
        let find = |from: usize, pat: &[char]| -> Option<usize> { (from..cs.len().saturating_sub(pat.len() - 1)).find(|&j| cs[j..].starts_with(pat)) };
        if cs[i] == '`' {
            if let Some(e) = find(i + 1, &['`']) {
                flush(&mut plain, &mut out);
                out.push((cs[i + 1..e].iter().collect(), false, false, true));
                i = e + 1;
                continue;
            }
        }
        if cs[i..].starts_with(&['*', '*']) {
            if let Some(e) = find(i + 2, &['*', '*']) {
                flush(&mut plain, &mut out);
                out.push((cs[i + 2..e].iter().collect(), true, false, false));
                i = e + 2;
                continue;
            }
        }
        if cs[i] == '*' {
            if let Some(e) = find(i + 1, &['*']) {
                flush(&mut plain, &mut out);
                out.push((cs[i + 1..e].iter().collect(), false, true, false));
                i = e + 1;
                continue;
            }
        }
        plain.push(cs[i]);
        i += 1;
    }
    flush(&mut plain, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Vec<Parsed> {
        parse(&s.split('\n').map(str::to_string).collect::<Vec<_>>())
    }

    #[test]
    fn blocks_and_their_source_lines() {
        let b = p("# T\n\npara one\ncontinues\n\n- a\n- b\n\n```js\nx\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n> q\n\n---");
        let kinds: Vec<(usize, &str)> = b.iter().map(|x| (x.line, match &x.block {
            Block::Heading { .. } => "h", Block::Para(_) => "p", Block::List { .. } => "l", Block::Code { .. } => "c", Block::Table { .. } => "t", Block::Quote(_) => "q", Block::Rule => "r", }
        )).collect();
        assert_eq!(kinds, vec![(0, "h"), (2, "p"), (5, "l"), (8, "c"), (12, "t"), (16, "q"), (18, "r")]);
        assert_eq!(b[1].block, Block::Para("para one continues".into()));
        assert_eq!(b[2].block, Block::List { ordered: false, items: vec!["a".into(), "b".into()] });
    }

    #[test]
    fn front_matter_is_skipped_and_inline_styles_parse() {
        let b = p("---\ntitle: x\n---\n\n# H");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].line, 4);
        let i = inline("a **b** *c* `d` e");
        assert_eq!(i.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a ", "b", " ", "c", " ", "d", " e"]);
        assert!(i[1].1 && i[3].2 && i[5].3);
    }
}
