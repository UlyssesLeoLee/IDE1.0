//! Markdown 便捷编辑 (工具条 / 命令面板 / 快捷键共用同一条命令, REQ FR-007 / INV-04)。
//!
//! 全是"区间补丁": 只改选区或光标所在的行, 其余文本原样 (NFR-016 / NFR-017 不做全篇格式化)。
//! 命令名 `md-*`: 行内 `md-bold md-italic md-strike md-code md-highlight`; 行前缀 `md-h1..md-h6 md-quote md-ul md-ol md-task`;
//! 块 `md-link md-image md-codeblock md-table md-hr md-mermaid md-callout`。

use super::ctx::Ctx;
use super::req_res::EditRes;
use crate::text_util::slice;

/// 命令名 -> 处理; 不是 md 命令返回 None
pub(super) fn run(cx: &Ctx) -> Option<EditRes> {
    let cmd = cx.req.cmd.strip_prefix("md-")?;
    Some(match cmd {
        "bold" => inline(cx, "**"),
        "italic" => inline(cx, "*"),
        "strike" => inline(cx, "~~"),
        "code" => inline(cx, "`"),
        "highlight" => inline(cx, "=="),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => heading(cx, cmd[1..].parse().unwrap_or(1)),
        "quote" => prefix_toggle(cx, |_| "> ".to_string(), is_quote),
        "ul" => prefix_toggle(cx, |_| "- ".to_string(), is_ul),
        "ol" => prefix_toggle(cx, |i| format!("{}. ", i + 1), is_ol),
        "task" => prefix_toggle(cx, |_| "- [ ] ".to_string(), is_task),
        "link" => link(cx, false),
        "image" => link(cx, true),
        "codeblock" => codeblock(cx, ""),
        "mermaid" => mermaid(cx),
        "table" => table(cx),
        "hr" => hr(cx),
        "callout" => callout(cx),
        _ => return None,
    })
}

fn sel_or_row(cx: &Ctx) -> (usize, usize) {
    cx.sel.map(|s| (s[0], if s[3] == 0 && s[2] > s[0] { s[2] - 1 } else { s[2] })).unwrap_or((cx.row, cx.row))
}

fn is_word(c: char) -> bool {
    !c.is_whitespace() && !"*~`=[]()<>|".contains(c)
}

// ---------- 行内 ----------

fn inline(cx: &Ctx, mark: &str) -> EditRes {
    let m: Vec<char> = mark.chars().collect();
    let n = m.len();
    // 选区 (跨行时只在首行起点和末行终点放标记) 或光标下的词
    let (r0, c0, r1, c1) = match cx.sel {
        Some(s) => (s[0], s[1], s[2], s[3]),
        None => {
            let line = &cx.line;
            let (mut a, mut b) = (cx.col, cx.col);
            while a > 0 && is_word(line[a - 1]) {
                a -= 1;
            }
            while b < line.len() && is_word(line[b]) {
                b += 1;
            }
            (cx.row, a, cx.row, b)
        }
    };
    let first = cx.line(r0);
    let last = cx.line(r1);
    if r0 == r1 {
        // 已被标记包住 -> 去掉 (标记在选区外, 或选区本身带着标记)
        let outer = c0 >= n && c1 + n <= first.len() && first[c0 - n..c0] == m[..] && first[c1..c1 + n] == m[..];
        let inner = c1 - c0 >= 2 * n && first[c0..c0 + n] == m[..] && first[c1 - n..c1] == m[..];
        let s: String = first.iter().collect();
        if outer {
            let new = format!("{}{}{}", slice(&first, 0, c0 - n), slice(&first, c0, c1), slice(&first, c1 + n, usize::MAX));
            let _ = s;
            return EditRes::replace_line(r0, new, [r0, c1 - n]).tip("已取消格式");
        }
        if inner {
            let new = format!("{}{}{}", slice(&first, 0, c0), slice(&first, c0 + n, c1 - n), slice(&first, c1, usize::MAX));
            return EditRes::replace_line(r0, new, [r0, c1 - 2 * n]).tip("已取消格式");
        }
        if c0 == c1 {
            // 空处: 放一对标记, 光标在中间
            let new = format!("{}{}{}{}", slice(&first, 0, c0), mark, mark, slice(&first, c1, usize::MAX));
            return EditRes::replace_line(r0, new, [r0, c0 + n]);
        }
        let new = format!("{}{}{}{}{}", slice(&first, 0, c0), mark, slice(&first, c0, c1), mark, slice(&first, c1, usize::MAX));
        return EditRes::replace_line(r0, new, [r0, c1 + 2 * n]);
    }
    let a = format!("{}{}{}", slice(&first, 0, c0), mark, slice(&first, c0, usize::MAX));
    let b = format!("{}{}{}", slice(&last, 0, c1), mark, slice(&last, c1, usize::MAX));
    let mut rows = vec![a];
    for r in r0 + 1..r1 {
        rows.push(cx.line(r).iter().collect());
    }
    rows.push(b);
    EditRes::splice(r0, r1 - r0 + 1, rows, [r1, c1 + n])
}

// ---------- 行前缀 ----------

fn is_quote(l: &str) -> bool {
    l.starts_with("> ") || l == ">"
}
fn is_task(l: &str) -> bool {
    ["- [ ] ", "- [x] ", "- [X] "].iter().any(|p| l.starts_with(p))
}
fn is_ul(l: &str) -> bool {
    !is_task(l) && (l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ "))
}
fn is_ol(l: &str) -> bool {
    let d = l.chars().take_while(|c| c.is_ascii_digit()).count();
    d > 0 && l[d..].starts_with(". ")
}
fn heading_level(l: &str) -> usize {
    let h = l.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&h) && l[h..].starts_with(' ') { h } else { 0 }
}
/// 去掉行首的块前缀 (标题 / 引用 / 列表 / 任务), 返回 (缩进, 余下文本)
fn strip_block(l: &str) -> (&str, &str) {
    let ind = l.len() - l.trim_start().len();
    let (indent, body) = l.split_at(ind);
    let rest = if heading_level(body) > 0 {
        body.trim_start_matches('#').trim_start()
    } else if is_task(body) {
        &body[6..]
    } else if is_quote(body) {
        body.strip_prefix("> ").unwrap_or("")
    } else if is_ul(body) {
        &body[2..]
    } else if is_ol(body) {
        let d = body.chars().take_while(|c| c.is_ascii_digit()).count();
        &body[d + 2..]
    } else {
        body
    };
    (indent, rest)
}

/// 选中的每一行加 / 去前缀: 全部已有该类前缀 -> 去掉; 否则换成该类前缀
fn prefix_toggle(cx: &Ctx, make: impl Fn(usize) -> String, has: impl Fn(&str) -> bool) -> EditRes {
    let (r0, r1) = sel_or_row(cx);
    let lines: Vec<String> = (r0..=r1).map(|r| cx.line(r).iter().collect()).collect();
    let all = lines.iter().filter(|l| !l.trim().is_empty()).all(|l| has(l.trim_start())) && lines.iter().any(|l| !l.trim().is_empty());
    let mut idx = 0;
    let out: Vec<String> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                return l.clone();
            }
            let (ind, rest) = strip_block(l);
            if all {
                format!("{ind}{rest}")
            } else {
                let p = make(idx);
                idx += 1;
                format!("{ind}{p}{rest}")
            }
        })
        .collect();
    let last = out.last().map(|s| s.chars().count()).unwrap_or(0);
    EditRes::splice(r0, r1 - r0 + 1, out, [r1, last])
}

fn heading(cx: &Ctx, level: usize) -> EditRes {
    let (r0, r1) = sel_or_row(cx);
    let lines: Vec<String> = (r0..=r1).map(|r| cx.line(r).iter().collect()).collect();
    let all_same = lines.iter().filter(|l| !l.trim().is_empty()).all(|l| heading_level(l) == level);
    let out: Vec<String> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                return l.clone();
            }
            let (_, rest) = strip_block(l);
            if all_same { rest.to_string() } else { format!("{} {}", "#".repeat(level), rest) }
        })
        .collect();
    let last = out.last().map(|s| s.chars().count()).unwrap_or(0);
    EditRes::splice(r0, r1 - r0 + 1, out, [r1, last])
}

// ---------- 块 ----------

fn link(cx: &Ctx, image: bool) -> EditRes {
    let bang = if image { "!" } else { "" };
    let (row, c0, c1) = match cx.sel {
        Some(s) if s[0] == s[2] => (s[0], s[1], s[3]),
        _ => (cx.row, cx.col, cx.col),
    };
    let line = cx.line(row);
    let text = slice(&line, c0, c1);
    let label = if text.is_empty() { if image { "alt" } else { "text" } } else { text.as_str() };
    let ins = format!("{bang}[{label}](url)");
    let new = format!("{}{}{}", slice(&line, 0, c0), ins, slice(&line, c1, usize::MAX));
    // 光标选中占位 "url" 的末尾 (用户直接粘贴 / 输入地址)
    EditRes::replace_line(row, new, [row, c0 + ins.chars().count() - 1])
}

fn fence_for(body: &str) -> String {
    let mut run = 0;
    let mut max = 0;
    for c in body.chars() {
        if c == '`' {
            run += 1;
            max = max.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((max + 1).max(3))
}

fn codeblock(cx: &Ctx, lang: &str) -> EditRes {
    match cx.sel {
        Some(_) => {
            let (r0, r1) = sel_or_row(cx);
            let body: Vec<String> = (r0..=r1).map(|r| cx.line(r).iter().collect()).collect();
            let f = fence_for(&body.join("\n"));
            let mut out = vec![format!("{f}{lang}")];
            out.extend(body);
            out.push(f);
            let end = out.len() - 1;
            EditRes::splice(r0, r1 - r0 + 1, out, [r0 + end, 3])
        }
        None => {
            let f = "```".to_string();
            EditRes::splice(cx.row + 1, 0, vec![format!("{f}{lang}"), String::new(), f], [cx.row + 2, 0])
        }
    }
}

fn mermaid(cx: &Ctx) -> EditRes {
    if cx.sel.is_some() {
        return codeblock(cx, "mermaid");
    }
    EditRes::splice(cx.row + 1, 0, vec!["```mermaid".into(), "graph TD".into(), "  A[开始] --> B[结束]".into(), "```".into()], [cx.row + 3, 0])
}

fn table(cx: &Ctx) -> EditRes {
    // text = "行x列" (含表头, 默认 3x3)
    let spec = cx.req.text.as_deref().unwrap_or("3x3");
    let (r, c) = spec.split_once(['x', 'X']).and_then(|(a, b)| Some((a.trim().parse::<usize>().ok()?, b.trim().parse::<usize>().ok()?))).unwrap_or((3, 3));
    let (r, c) = (r.clamp(2, 50), c.clamp(1, 20));
    let row = |f: &dyn Fn(usize) -> String| format!("| {} |", (0..c).map(f).collect::<Vec<_>>().join(" | "));
    let mut out = vec![String::new(), row(&|i| format!("列{}", i + 1)), row(&|_| "---".into())];
    for _ in 0..r - 1 {
        out.push(row(&|_| "   ".into()));
    }
    out.push(String::new());
    EditRes::splice(cx.row + 1, 0, out, [cx.row + 2, 2])
}

fn hr(cx: &Ctx) -> EditRes {
    EditRes::splice(cx.row + 1, 0, vec![String::new(), "---".into(), String::new()], [cx.row + 3, 0])
}

fn callout(cx: &Ctx) -> EditRes {
    let (r0, r1) = sel_or_row(cx);
    let has_sel = cx.sel.is_some();
    let mut out = vec!["> [!note] 标题".to_string()];
    if has_sel {
        out.extend((r0..=r1).map(|r| format!("> {}", cx.line(r).iter().collect::<String>())));
        let last = out.last().map(|s| s.chars().count()).unwrap_or(0);
        EditRes::splice(r0, r1 - r0 + 1, out.clone(), [r0 + out.len() - 1, last])
    } else {
        out.push("> ".to_string());
        EditRes::splice(cx.row + 1, 0, out, [cx.row + 2, 2])
    }
}

#[cfg(test)]
mod tests {
    use super::super::{run as run_cmd, EditReq};

    fn go(cmd: &str, lines: &[&str], row: usize, col: usize, sel: Option<[usize; 4]>, text: Option<&str>) -> (Vec<String>, [usize; 2]) {
        let req = EditReq {
            cmd: cmd.into(),
            row,
            col,
            sel,
            total_rows: lines.len(),
            text: text.map(String::from),
            rows: lines.iter().enumerate().map(|(i, l)| (i, l.to_string())).collect(),
            ..Default::default()
        };
        let res = run_cmd(&req).expect("命令存在");
        let mut doc: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        doc.splice(res.start..res.start + res.remove, res.insert);
        (doc, res.cursor)
    }

    #[test]
    fn bold_wraps_selection_and_toggles_off() {
        let (d, _) = go("md-bold", &["hello world"], 0, 5, Some([0, 0, 0, 5]), None);
        assert_eq!(d, ["**hello** world"]);
        let (d, _) = go("md-bold", &["**hello** world"], 0, 7, Some([0, 2, 0, 7]), None);
        assert_eq!(d, ["hello world"], "标记在选区外 -> 取消");
        let (d, _) = go("md-bold", &["**hello** world"], 0, 9, Some([0, 0, 0, 9]), None);
        assert_eq!(d, ["hello world"], "选区本身带标记 -> 取消");
    }

    #[test]
    fn no_selection_uses_word_or_inserts_pair() {
        let (d, _) = go("md-italic", &["foo bar"], 0, 5, None, None);
        assert_eq!(d, ["foo *bar*"]);
        let (d, cur) = go("md-code", &["foo "], 0, 4, None, None);
        assert_eq!(d, ["foo ``"]);
        assert_eq!(cur, [0, 5], "光标在一对标记中间");
    }

    #[test]
    fn heading_toggles_and_replaces_level() {
        assert_eq!(go("md-h2", &["title"], 0, 0, None, None).0, ["## title"]);
        assert_eq!(go("md-h2", &["## title"], 0, 0, None, None).0, ["title"]);
        assert_eq!(go("md-h1", &["### title"], 0, 0, None, None).0, ["# title"]);
    }

    #[test]
    fn list_prefixes_apply_per_row_and_toggle() {
        let (d, _) = go("md-ol", &["a", "b", "c"], 0, 0, Some([0, 0, 2, 1]), None);
        assert_eq!(d, ["1. a", "2. b", "3. c"]);
        let (d, _) = go("md-ol", &["1. a", "2. b"], 0, 0, Some([0, 0, 1, 1]), None);
        assert_eq!(d, ["a", "b"]);
        assert_eq!(go("md-task", &["  x"], 0, 0, None, None).0, ["  - [ ] x"], "保留缩进");
        assert_eq!(go("md-quote", &["- a"], 0, 0, None, None).0, ["> a"], "换成另一类前缀");
    }

    #[test]
    fn codeblock_uses_safe_fence() {
        let (d, _) = go("md-codeblock", &["a ``` b"], 0, 0, Some([0, 0, 0, 7]), None);
        assert_eq!(d, ["````", "a ``` b", "````"]);
        let (d, cur) = go("md-codeblock", &["x"], 0, 0, None, None);
        assert_eq!(d, ["x", "```", "", "```"]);
        assert_eq!(cur, [2, 0]);
    }

    #[test]
    fn table_and_link_and_hr() {
        let (d, _) = go("md-table", &["x"], 0, 0, None, Some("3x2"));
        assert_eq!(d, ["x", "", "| 列1 | 列2 |", "| --- | --- |", "|     |     |", "|     |     |", ""]);
        assert_eq!(go("md-link", &["see docs"], 0, 8, Some([0, 4, 0, 8]), None).0, ["see [docs](url)"]);
        assert_eq!(go("md-image", &["a"], 0, 1, None, None).0, ["a![alt](url)"]);
        assert_eq!(go("md-hr", &["x"], 0, 0, None, None).0, ["x", "", "---", ""]);
    }

    #[test]
    fn unknown_md_command_is_none() {
        let req = EditReq { cmd: "md-nope".into(), total_rows: 1, rows: vec![(0, String::new())], ..Default::default() };
        assert!(run_cmd(&req).is_none());
    }
}
