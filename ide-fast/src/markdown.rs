//! Markdown 预览 (原生): 直接按文档的行解析成块 (标题 / 代码块 / 提示块 / 引用 / 列表 / 表格 / 分隔线 / 段落) 再用 egui 画出来 (见 `md_view.rs`)。
//! 不经过 HTML: 块结构和 `editcore::md_render` 的规则一致 (围栏 ``` 与 ~~~、`>` 引用与 `> [!NOTE]` 提示块、嵌套 / 任务列表、`|` 表格含对齐、`---` 分隔线),
//! 行内认 **粗体** *斜体* ~~删除线~~ ==高亮== `代码` [链接](url) ![图片](url) [[维基链接]] 和裸 URL。
//! 不含 Mermaid / 插件渲染 (那些只在 Tauri 版): mermaid 围栏当作代码块显示并加说明。每个块带源码起始行, 点击块 = 回到编辑区那一行。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// 嵌套层级 (0 起)
    pub depth: usize,
    pub ordered: bool,
    /// 有序列表的序号 (原文写的数字)
    pub num: usize,
    /// 任务列表: Some(是否已勾选)
    pub task: Option<bool>,
    pub text: String,
    /// 源码行 (0 起): 预览里点任务复选框时据此改写那一行
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: usize,
        text: String,
    },
    Code {
        lang: String,
        text: String,
    },
    /// `> [!NOTE] 标题` 开头的提示块; kind 小写 (note / tip / warning ...)
    Callout {
        kind: String,
        title: String,
        text: String,
    },
    Quote(String),
    List(Vec<Item>),
    Table {
        rows: Vec<Vec<String>>,
        aligns: Vec<Align>,
    },
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
    t.len() >= 3
        && (t.chars().all(|c| c == '-')
            || t.chars().all(|c| c == '*')
            || t.chars().all(|c| c == '_'))
}

/// 列表项: (嵌套层级, 有序?, 序号, 正文); 缩进每 2 个空格一层 (Tab 当 4 个)
fn list_item(l: &str) -> Option<(usize, bool, usize, &str)> {
    let t = l.trim_start();
    let indent: usize = l[..l.len() - t.len()]
        .chars()
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    let depth = indent / 2;
    for p in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(p) {
            return Some((depth, false, 0, r));
        }
    }
    let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if d > 0 && (t[d..].starts_with(". ") || t[d..].starts_with(") ")) {
        return Some((depth, true, t[..d].parse().unwrap_or(1), &t[d + 2..]));
    }
    None
}

fn cells(l: &str) -> Vec<String> {
    l.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn table_sep(l: &str) -> bool {
    let t = l.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn aligns(sep: &str) -> Vec<Align> {
    cells(sep)
        .iter()
        .map(|c| match (c.starts_with(':'), c.ends_with(':')) {
            (true, true) => Align::Center,
            (false, true) => Align::Right,
            _ => Align::Left,
        })
        .collect()
}

/// 围栏起始行: (围栏字符, 长度, 语言)
fn fence_open(l: &str) -> Option<(char, usize, String)> {
    let t = l.trim_start();
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|&x| x == c).count();
    (n >= 3).then(|| {
        (
            c,
            n,
            t[n..].split_whitespace().next().unwrap_or("").to_string(),
        )
    })
}

pub fn parse(lines: &[String]) -> Vec<Parsed> {
    let mut out = vec![];
    let mut i = 0;
    // Front Matter
    if lines
        .first()
        .map(|l| l.trim_end() == "---")
        .unwrap_or(false)
    {
        if let Some(end) = lines
            .iter()
            .skip(1)
            .position(|l| l.trim_end() == "---" || l.trim_end() == "...")
        {
            i = end + 2;
        }
    }
    while i < lines.len() {
        let l = &lines[i];
        let start = i;
        if l.trim().is_empty() {
            i += 1;
        } else if let Some((fc, fnum, lang)) = fence_open(l) {
            let mut body = vec![];
            i += 1;
            while i < lines.len()
                && !(lines[i]
                    .trim_start()
                    .starts_with(&fc.to_string().repeat(fnum))
                    && lines[i].trim().chars().all(|c| c == fc))
            {
                body.push(lines[i].as_str());
                i += 1;
            }
            i += 1;
            out.push(Parsed {
                block: Block::Code {
                    lang,
                    text: body.join("\n"),
                },
                line: start,
            });
        } else if let Some(h) = Some(l.chars().take_while(|&c| c == '#').count())
            .filter(|&h| (1..=6).contains(&h) && l[h..].starts_with(' '))
        {
            out.push(Parsed {
                block: Block::Heading {
                    level: h,
                    text: l[h + 1..].trim().trim_end_matches('#').trim().to_string(),
                },
                line: start,
            });
            i += 1;
        } else if is_rule(l) && list_item(l).is_none() {
            out.push(Parsed {
                block: Block::Rule,
                line: start,
            });
            i += 1;
        } else if l.starts_with('>') {
            let mut q = vec![];
            while i < lines.len() && lines[i].starts_with('>') {
                q.push(lines[i].trim_start_matches('>').trim_start().to_string());
                i += 1;
            }
            out.push(Parsed {
                block: quote_block(&q),
                line: start,
            });
        } else if i + 1 < lines.len() && l.contains('|') && table_sep(&lines[i + 1]) {
            let al = aligns(&lines[i + 1]);
            let mut rows = vec![cells(l)];
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(cells(&lines[i]));
                i += 1;
            }
            out.push(Parsed {
                block: Block::Table { rows, aligns: al },
                line: start,
            });
        } else if list_item(l).is_some() {
            let mut items: Vec<Item> = vec![];
            while i < lines.len() {
                if let Some((depth, ordered, num, t)) = list_item(&lines[i]) {
                    let (task, text) = match t.get(..4) {
                        Some("[ ] ") => (Some(false), &t[4..]),
                        Some("[x] ") | Some("[X] ") => (Some(true), &t[4..]),
                        _ => (None, t),
                    };
                    items.push(Item {
                        depth,
                        ordered,
                        num,
                        task,
                        text: text.trim().to_string(),
                        line: i,
                    });
                    i += 1;
                } else if lines[i].trim().is_empty()
                    && lines
                        .get(i + 1)
                        .map(|n| list_item(n).is_some())
                        .unwrap_or(false)
                {
                    i += 1; // 松散列表: 项之间的单个空行
                } else if lines[i].starts_with(char::is_whitespace)
                    && !lines[i].trim().is_empty()
                    && !items.is_empty()
                {
                    // 缩进的续行并进上一项
                    let last = items.last_mut().unwrap();
                    last.text.push(' ');
                    last.text.push_str(lines[i].trim());
                    i += 1;
                } else {
                    break;
                }
            }
            out.push(Parsed {
                block: Block::List(items),
                line: start,
            });
        } else {
            let mut p = vec![l.trim().to_string()];
            i += 1;
            while i < lines.len()
                && !lines[i].trim().is_empty()
                && !lines[i].starts_with('#')
                && !lines[i].starts_with('>')
                && fence_open(&lines[i]).is_none()
                && list_item(&lines[i]).is_none()
                && !is_rule(&lines[i])
            {
                p.push(lines[i].trim().to_string());
                i += 1;
            }
            out.push(Parsed {
                block: Block::Para(p.join(" ")),
                line: start,
            });
        }
    }
    out
}

/// `> [!KIND] 标题` 开头 = 提示块, 否则普通引用
fn quote_block(q: &[String]) -> Block {
    if let Some(first) = q.first() {
        if let Some(rest) = first.strip_prefix("[!") {
            if let Some(end) = rest.find(']') {
                let kind = rest[..end].trim().to_lowercase();
                if !kind.is_empty() && kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                    return Block::Callout {
                        title: rest[end + 1..].trim().to_string(),
                        kind,
                        text: q[1..].join("\n"),
                    };
                }
            }
        }
    }
    Block::Quote(q.join("\n"))
}

/// 行内片段
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub mark: bool,
    pub code: bool,
    /// 链接目标; 维基链接写成 `wiki:名字`
    pub link: Option<String>,
    /// 图片: text = alt, link = 路径
    pub image: bool,
}

pub fn inline(s: &str) -> Vec<Span> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    parse_inline(&cs, &Span::default(), &mut out);
    out
}

fn find(cs: &[char], from: usize, pat: &[char]) -> Option<usize> {
    if cs.len() < pat.len() {
        return None;
    }
    let mut j = from;
    while j + pat.len() <= cs.len() {
        if cs[j] == '\\' {
            j += 2;
            continue;
        }
        if cs[j..].starts_with(pat) {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn parse_inline(cs: &[char], st: &Span, out: &mut Vec<Span>) {
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span {
                text: std::mem::take(plain),
                ..st.clone()
            });
        }
    };
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '\\' && i + 1 < cs.len() && cs[i + 1].is_ascii_punctuation() {
            plain.push(cs[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(e) = find(cs, i + 1, &['`']) {
                flush(&mut plain, out);
                out.push(Span {
                    text: cs[i + 1..e].iter().collect(),
                    code: true,
                    ..st.clone()
                });
                i = e + 1;
                continue;
            }
        }
        // 图片 / 链接
        if c == '!' && cs.get(i + 1) == Some(&'[') || c == '[' {
            let image = c == '!';
            let o = if image { i + 1 } else { i };
            if !image && cs.get(i + 1) == Some(&'[') {
                if let Some(e) = find(cs, i + 2, &[']', ']']) {
                    flush(&mut plain, out);
                    let inner: String = cs[i + 2..e].iter().collect();
                    let (target, label) = match inner.split_once('|') {
                        Some((t, l)) => (t.to_string(), l.to_string()),
                        None => (inner.clone(), inner),
                    };
                    out.push(Span {
                        text: label,
                        link: Some(format!("wiki:{}", target.trim())),
                        ..st.clone()
                    });
                    i = e + 2;
                    continue;
                }
            }
            if let Some(close) = find(cs, o + 1, &[']']) {
                if cs.get(close + 1) == Some(&'(') {
                    if let Some(pe) = find(cs, close + 2, &[')']) {
                        flush(&mut plain, out);
                        let label = &cs[o + 1..close];
                        let url: String = cs[close + 2..pe].iter().collect();
                        let url = url
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .trim_matches(|c| c == '<' || c == '>')
                            .to_string();
                        if image {
                            out.push(Span {
                                text: label.iter().collect(),
                                link: Some(url),
                                image: true,
                                ..st.clone()
                            });
                        } else {
                            parse_inline(
                                label,
                                &Span {
                                    link: Some(url),
                                    ..st.clone()
                                },
                                out,
                            );
                        }
                        i = pe + 1;
                        continue;
                    }
                }
            }
        }
        // 自动链接 <http://...> 与裸 URL
        if c == '<' {
            if let Some(e) = find(cs, i + 1, &['>']) {
                let inner: String = cs[i + 1..e].iter().collect();
                if inner.starts_with("http://") || inner.starts_with("https://") {
                    flush(&mut plain, out);
                    out.push(Span {
                        text: inner.clone(),
                        link: Some(inner),
                        ..st.clone()
                    });
                    i = e + 1;
                    continue;
                }
            }
        }
        if (c == 'h')
            && st.link.is_none()
            && (cs[i..].starts_with(&['h', 't', 't', 'p', ':', '/', '/'])
                || cs[i..].starts_with(&['h', 't', 't', 'p', 's', ':', '/', '/']))
            && (i == 0 || !cs[i - 1].is_alphanumeric())
        {
            let e = cs[i..]
                .iter()
                .position(|c| c.is_whitespace() || matches!(c, '<' | ')' | '，' | '。'))
                .map(|p| i + p)
                .unwrap_or(cs.len());
            let url: String = cs[i..e]
                .iter()
                .collect::<String>()
                .trim_end_matches(['.', ',', ';', ':'])
                .to_string();
            flush(&mut plain, out);
            let n = url.chars().count();
            out.push(Span {
                text: url.clone(),
                link: Some(url),
                ..st.clone()
            });
            i += n;
            continue;
        }
        // 成对的样式: 记号 -> 改哪个标志
        let pairs: [(&[char], fn(&mut Span)); 4] = [
            (&['*', '*'], |s| s.bold = true),
            (&['~', '~'], |s| s.strike = true),
            (&['=', '='], |s| s.mark = true),
            (&['*'], |s| s.italic = true),
        ];
        let mut hit = false;
        for (pat, apply) in pairs {
            if cs[i..].starts_with(pat)
                && cs
                    .get(i + pat.len())
                    .map(|n| !n.is_whitespace())
                    .unwrap_or(false)
            {
                if let Some(mut e) = find(cs, i + pat.len(), pat) {
                    // `***x***` 这类连续星号: 取最后两个当收尾, 前面的留给内层
                    while pat == ['*', '*'] && cs.get(e + 2) == Some(&'*') {
                        e += 1;
                    }
                    if e > i + pat.len() {
                        flush(&mut plain, out);
                        let mut inner = st.clone();
                        apply(&mut inner);
                        parse_inline(&cs[i + pat.len()..e], &inner, out);
                        i = e + pat.len();
                        hit = true;
                        break;
                    }
                }
            }
        }
        if hit {
            continue;
        }
        plain.push(c);
        i += 1;
    }
    flush(&mut plain, out);
}

/// 提示块种类 -> (图标, 中文标题)
pub fn callout_label(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "note" => ("ℹ", "注意"),
        "tip" | "hint" => ("★", "提示"),
        "important" => ("!", "重要"),
        "warning" | "caution" => ("⚠", "警告"),
        "danger" | "error" | "bug" => ("✖", "危险"),
        "success" | "check" | "done" => ("✔", "成功"),
        "question" | "help" | "faq" => ("?", "疑问"),
        "quote" | "cite" => ("❝", "引用"),
        _ => ("•", "提示"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Vec<Parsed> {
        parse(&s.split('\n').map(str::to_string).collect::<Vec<_>>())
    }

    fn texts(s: &str) -> Vec<String> {
        inline(s).into_iter().map(|x| x.text).collect()
    }

    #[test]
    fn blocks_and_their_source_lines() {
        let b = p("# T\n\npara one\ncontinues\n\n- a\n- b\n\n```js\nx\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n> q\n\n---");
        let kinds: Vec<(usize, &str)> = b
            .iter()
            .map(|x| {
                (
                    x.line,
                    match &x.block {
                        Block::Heading { .. } => "h",
                        Block::Para(_) => "p",
                        Block::List(_) => "l",
                        Block::Code { .. } => "c",
                        Block::Table { .. } => "t",
                        Block::Quote(_) => "q",
                        Block::Rule => "r",
                        Block::Callout { .. } => "k",
                    },
                )
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (0, "h"),
                (2, "p"),
                (5, "l"),
                (8, "c"),
                (12, "t"),
                (16, "q"),
                (18, "r")
            ]
        );
        assert_eq!(b[1].block, Block::Para("para one continues".into()));
        let Block::List(items) = &b[2].block else {
            panic!()
        };
        assert_eq!(
            items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn front_matter_is_skipped() {
        let b = p("---\ntitle: x\n---\n\n# H");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].line, 4);
    }

    #[test]
    fn nested_and_task_lists() {
        let b = p("- a\n  - b\n    - c\n- [ ] todo\n- [x] done\n1. one\n2. two");
        let Block::List(it) = &b[0].block else {
            panic!()
        };
        assert_eq!(
            it.iter().map(|i| i.depth).collect::<Vec<_>>(),
            vec![0, 1, 2, 0, 0, 0, 0]
        );
        assert_eq!(
            (it[3].task, it[4].task, it[0].task),
            (Some(false), Some(true), None)
        );
        assert_eq!(it[3].text, "todo");
        assert!(it[5].ordered && it[6].num == 2);
    }

    #[test]
    fn list_continuation_and_loose_items() {
        let b = p("- a\n  more\n\n- b");
        let Block::List(it) = &b[0].block else {
            panic!()
        };
        assert_eq!(it.len(), 2);
        assert_eq!(it[0].text, "a more");
    }

    #[test]
    fn callouts_and_plain_quotes() {
        let b = p("> [!WARNING] Careful\n> be **safe**\n\n> just a quote");
        assert_eq!(
            b[0].block,
            Block::Callout {
                kind: "warning".into(),
                title: "Careful".into(),
                text: "be **safe**".into()
            }
        );
        assert_eq!(b[1].block, Block::Quote("just a quote".into()));
        assert_eq!(callout_label("warning").0, "⚠");
    }

    #[test]
    fn table_alignment_and_tilde_fences() {
        let b = p("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n\n~~~mermaid\ngraph TD\n~~~");
        let Block::Table { rows, aligns } = &b[0].block else {
            panic!()
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(aligns, &vec![Align::Left, Align::Center, Align::Right]);
        assert_eq!(
            b[1].block,
            Block::Code {
                lang: "mermaid".into(),
                text: "graph TD".into()
            }
        );
    }

    #[test]
    fn fence_needs_matching_closer_and_keeps_inner_backticks() {
        let b = p("````md\n```js\nx\n```\n````\n\nafter");
        assert_eq!(b.len(), 2);
        assert_eq!(
            b[0].block,
            Block::Code {
                lang: "md".into(),
                text: "```js\nx\n```".into()
            }
        );
    }

    #[test]
    fn inline_styles_nest() {
        let i = inline("a **b *c*** ~~d~~ ==e== `f` g");
        assert_eq!(
            i.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(),
            vec!["a ", "b ", "c", " ", "d", " ", "e", " ", "f", " g"]
        );
        assert!(i[1].bold && !i[1].italic);
        assert!(i[2].bold && i[2].italic, "粗体里嵌套斜体");
        assert!(i[4].strike && i[6].mark && i[8].code);
    }

    #[test]
    fn links_images_wikilinks_and_bare_urls() {
        let i = inline("see [the **doc**](http://x.io/a) and ![logo](img/a.png) and [[Page|alias]] and https://b.org/p.");
        let link = i.iter().find(|s| s.text == "doc").unwrap();
        assert!(link.bold && link.link.as_deref() == Some("http://x.io/a"));
        let img = i.iter().find(|s| s.image).unwrap();
        assert_eq!(
            (img.text.as_str(), img.link.as_deref()),
            ("logo", Some("img/a.png"))
        );
        let wiki = i.iter().find(|s| s.text == "alias").unwrap();
        assert_eq!(wiki.link.as_deref(), Some("wiki:Page"));
        let bare = i.iter().find(|s| s.text.starts_with("https://b")).unwrap();
        assert_eq!(
            (bare.text.as_str(), bare.link.as_deref()),
            ("https://b.org/p", Some("https://b.org/p")),
            "裸 URL 不吃句末的点"
        );
    }

    #[test]
    fn escapes_and_unmatched_markers_stay_literal() {
        assert_eq!(texts(r"a \*b\* c"), vec!["a *b* c"]);
        assert_eq!(
            texts("2 * 3 * 4"),
            vec!["2 * 3 * 4"],
            "星号后是空格不当强调"
        );
        assert_eq!(texts("lonely ** end"), vec!["lonely ** end"]);
        assert_eq!(texts("`a*b*c`"), vec!["a*b*c"], "代码里不再解析");
    }
}
