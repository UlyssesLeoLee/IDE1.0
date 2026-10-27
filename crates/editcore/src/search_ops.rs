//! 文档内查找 / 置換 (ARCH-01: 查找语义放 Rust).
//!
//! 对应前端的 findInTab / findStep / replaceAll / replaceOne 里的匹配逻辑。列一律按码点 (`char`) 计。
//! 单词单位 = 命中两侧都不是「字母 / 数字 / 下划线」; 正则用 `regex` crate
//! (不支持 JS 的后顾 / 反向引用, 此类写法按"正则表达式无效"处理)。
//!
//! 语义按真实 Sakura Editor 的宏逐条比对 (tests/sakura-diff 的 SearchNext / SearchPrev / ReplaceAll 用例)。

use crate::word::is_word_char as is_word;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

const MAX_HITS: usize = 5000;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchReq {
    /// "hits" | "step" | "replace_all" | "match_expand" | "validate"
    pub op: String,
    pub needle: String,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default)]
    pub use_regex: bool,
    /// 整篇文档 (hits / step / replace_all)。带 doc_id 时可以留空: 正文用 Rust 那份镜像 (见 doc_mirror)
    #[serde(default)]
    pub lines: Vec<String>,
    /// 文档正文镜像: 文档 id (标签) / 内容指纹 / 行数。lines 非空 = 登记这份正文; lines 为空 = 指纹对得上就用镜像, 对不上返回 need_doc
    #[serde(default)]
    pub doc_id: Option<String>,
    #[serde(default)]
    pub doc_hash: Option<[u32; 2]>,
    #[serde(default)]
    pub doc_len: usize,
    // ---- step ----
    #[serde(default)]
    pub row: usize,
    #[serde(default)]
    pub col: usize,
    #[serde(default)]
    pub forward: bool,
    #[serde(default)]
    pub wrap: bool,
    /// 当前选区的起点 (选区正好从光标处开始 = 当前命中正被选中, 从它的"下一处"找)
    #[serde(default)]
    pub sel_start: Option<[usize; 2]>,
    // ---- replace_all / match_expand ----
    #[serde(default)]
    pub repl: String,
    /// 置換範囲 [r0, c0, r1, c1]; None = 全文
    #[serde(default)]
    pub sel: Option<[usize; 4]>,
    /// match_expand: 被选中的那段文字
    #[serde(default)]
    pub seg: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    pub row: usize,
    pub col: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchRes {
    pub error: Option<String>,
    pub hits: Vec<Hit>,
    pub total: usize,
    /// step: 目标命中 / 它在 hits 里的序号 / 是否回绕了
    pub hit: Option<Hit>,
    pub index: usize,
    pub wrapped: bool,
    /// replace_all: 被改动的行 [行号, 新文本] / 置換数 / 最后一处置換之后的位置
    pub rows: Vec<(usize, String)>,
    pub count: usize,
    /// 镜像里没有这份正文 (指纹对不上): 前端要把整篇 lines 补发过来
    pub need_doc: bool,
    pub last_row: Option<usize>,
    pub last_end: usize,
    /// match_expand: 选中的刚好是一处命中时, 展开后的替换文字
    pub text: Option<String>,
}

fn build(req: &SearchReq) -> Result<Regex, String> {
    if req.needle.is_empty() {
        return Err("empty".into());
    }
    let src = if req.use_regex { req.needle.clone() } else { regex::escape(&req.needle) };
    RegexBuilder::new(&src).case_insensitive(!req.match_case).build().map_err(|e| friendly_error(&e.to_string()))
}

/// 正则编译错误的中文说明。Sakura 在「正则」打开时同样把模式交给 Perl 风格的正则库: 单独的 `*` 是量词, 前面没有可重复的内容就是语法错误;
/// 要查找字符 `*` 本身要写 `\*` 或 `[*]` —— 这里把这个提示直接写在报错里。
fn friendly_error(raw: &str) -> String {
    let r = raw.to_lowercase();
    let msg = if r.contains("repetition operator missing expression") || r.contains("nothing to repeat") {
        "量词 (* + ? {n,m}) 前面没有可重复的内容。要查找字符 * + ? 本身, 请写 \\* \\+ \\? 或 [*] [+] [?]"
    } else if r.contains("unclosed group") {
        "缺少右括号 ) —— 要查找字符 ( 本身请写 \\("
    } else if r.contains("unopened group") {
        "多出右括号 ) —— 要查找字符 ) 本身请写 \\)"
    } else if r.contains("unclosed character class") {
        "缺少 ] —— 要查找字符 [ 本身请写 \\["
    } else if r.contains("unclosed counted repetition") || r.contains("repetition quantifier expects a valid decimal") {
        "{} 里的重复次数写法不对 (应为 {n} 或 {n,m}) —— 要查找字符 { 本身请写 \\{"
    } else if r.contains("escape sequence") || r.contains("unrecognized") {
        "含有不支持的转义序列"
    } else if r.contains("look-around") || r.contains("backreference") || r.contains("look-ahead") {
        "不支持反向引用 / 环视断言"
    } else {
        "语法错误"
    };
    msg.to_string()
}

/// 一行里的全部命中: (起始字节, 结束字节, 起始列, 长度(字符数)); 空匹配不算
fn line_matches(re: &Regex, line: &str, whole_word: bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos <= line.len() {
        let Some(m) = re.find_at(line, pos) else { break };
        if m.start() == m.end() {
            pos = next_char(line, m.end());
            continue;
        }
        if whole_word {
            let before = line[..m.start()].chars().next_back();
            let after = line[m.end()..].chars().next();
            if before.map(is_word).unwrap_or(false) || after.map(is_word).unwrap_or(false) {
                pos = next_char(line, m.start());
                continue;
            }
        }
        out.push((m.start(), m.end()));
        pos = m.end();
    }
    out
}

fn next_char(s: &str, i: usize) -> usize {
    s[i..].chars().next().map(|c| i + c.len_utf8()).unwrap_or(s.len() + 1)
}

fn all_hits(re: &Regex, req: &SearchReq, lines: &[String]) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        for (s, e) in line_matches(re, line, req.whole_word) {
            let (col, len) = byte_span_to_cols(line, s, e);
            hits.push(Hit { row, col, len });
            if hits.len() > MAX_HITS {
                return hits;
            }
        }
    }
    hits
}

/// 正则模式下展开 $1 $& \1 (与前端原实现一致); 普通模式原样返回
fn expand(caps: &regex::Captures, repl: &str, use_regex: bool) -> String {
    if !use_regex {
        return repl.to_string();
    }
    let cs: Vec<char> = repl.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '$' && i + 1 < cs.len() && cs[i + 1] == '&' {
            out.push_str(caps.get(0).map(|m| m.as_str()).unwrap_or(""));
            i += 2;
        } else if c == '$' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < cs.len() && cs[j].is_ascii_digit() {
                j += 1;
            }
            let k: usize = cs[i + 1..j].iter().collect::<String>().parse().unwrap_or(usize::MAX);
            out.push_str(caps.get(k).map(|m| m.as_str()).unwrap_or(""));
            i = j;
        } else if c == '\\' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
            let k = cs[i + 1].to_digit(10).unwrap_or(0) as usize;
            out.push_str(caps.get(k).map(|m| m.as_str()).unwrap_or(""));
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

pub fn run(req: &SearchReq) -> Option<SearchRes> {
    // 文档正文: 带 doc_id 的走镜像 (登记 / 取用 / 丢弃), 否则就是请求里的 lines
    let mirrored: Option<std::sync::Arc<Vec<String>>> = match &req.doc_id {
        Some(id) if req.op == "forget" => {
            crate::doc_mirror::forget(id);
            return Some(SearchRes::default());
        }
        Some(id) if !req.lines.is_empty() => Some(crate::doc_mirror::put(id, req.lines.clone())),
        Some(id) => match req.doc_hash {
            Some(fp) => match crate::doc_mirror::get(id, fp, req.doc_len) {
                Some(d) => Some(d),
                None => return Some(SearchRes { need_doc: true, ..Default::default() }),
            },
            None => None,
        },
        None => None,
    };
    let lines: &[String] = mirrored.as_deref().map(|v| v.as_slice()).unwrap_or(&req.lines);
    run_on(req, lines)
}

fn run_on(req: &SearchReq, lines: &[String]) -> Option<SearchRes> {
    let re = match build(req) {
        Ok(r) => r,
        Err(e) => return Some(SearchRes { error: Some(e), ..Default::default() }),
    };
    match req.op.as_str() {
        "validate" => Some(SearchRes::default()),
        "hits" => Some(op_hits(&re, req, lines)),
        "step" => Some(op_step(&re, req, lines)),
        "replace_all" => Some(op_replace_all(&re, req, lines)),
        "match_expand" => op_match_expand(&re, req), // 没匹配上 = None (HTTP 上是 400), 前端据此当作"不是一处命中"
        _ => None,
    }
}

/// 命中处的 (列, 长度), 按码点
fn byte_span_to_cols(line: &str, s: usize, e: usize) -> (usize, usize) {
    (line[..s].chars().count(), line[s..e].chars().count())
}

fn op_hits(re: &Regex, req: &SearchReq, lines: &[String]) -> SearchRes {
    let hits = all_hits(re, req, lines);
    SearchRes { total: hits.len(), hits, ..Default::default() }
}

/// Sakura: 单词单位向后查找时, 光标在词中 -> 从该词词首起找 (命中当前词本身)
fn step_start_col(req: &SearchReq, lines: &[String]) -> usize {
    let (cr, mut cc) = (req.row, req.col);
    if req.forward && req.whole_word && cc > 0 {
        let cs: Vec<char> = lines.get(cr).map(|l| l.chars().collect()).unwrap_or_default();
        if cs.get(cc - 1).copied().map(is_word).unwrap_or(false) && cs.get(cc).copied().map(is_word).unwrap_or(false) {
            while cc > 0 && is_word(cs[cc - 1]) {
                cc -= 1;
            }
        }
    }
    cc
}

/// 从光标处向前 / 向后找下一处命中; 到头按 `wrap` 绕回
fn op_step(re: &Regex, req: &SearchReq, lines: &[String]) -> SearchRes {
    let hits = all_hits(re, req, lines);
    let (cr, cc) = (req.row, step_start_col(req, lines));
    let on_hit = req.sel_start == Some([cr, cc]); // 当前命中正被选中: 从它的"下一处"找
    let mut wrapped = false;
    let found = if req.forward {
        hits.iter().position(|h| h.row > cr || (h.row == cr && if on_hit { h.col > cc } else { h.col >= cc }))
    } else {
        hits.iter().rposition(|h| h.row < cr || (h.row == cr && h.col < cc))
    };
    let idx = match found {
        Some(i) => Some(i),
        None if req.wrap && !hits.is_empty() => {
            wrapped = true;
            Some(if req.forward { 0 } else { hits.len() - 1 })
        }
        None => None,
    };
    SearchRes { total: hits.len(), hit: idx.map(|i| hits[i].clone()), index: idx.unwrap_or(0), wrapped, ..Default::default() }
}

/// 置換全部 (限选区): 返回被改动的行 [行号, 新文本]、置換数和最后一处置換之后的位置
fn op_replace_all(re: &Regex, req: &SearchReq, lines: &[String]) -> SearchRes {
    let mut res = SearchRes::default();
    if lines.is_empty() {
        return res;
    }
    let n_rows = lines.len();
    let (r0, r1, c0, c1) = match req.sel {
        Some(s) => (s[0], s[2].min(n_rows - 1), s[1], s[3]),
        None => (0, n_rows - 1, 0, usize::MAX),
    };
    for (r, line) in lines.iter().enumerate().take(r1 + 1).skip(r0) {
        let from = if r == r0 { c0 } else { 0 };
        let to = if r == r1 { c1 } else { usize::MAX };
        let (mut out, mut at, mut n, mut end) = (String::new(), 0usize, 0usize, 0usize);
        for (s, e) in line_matches(re, line, req.whole_word) {
            let (col, len) = byte_span_to_cols(line, s, e);
            if col < from || col.saturating_add(len) > to {
                continue;
            }
            let caps = re.captures_at(line, s).filter(|c| c.get(0).map(|m| m.start() == s).unwrap_or(false));
            let rep = match caps {
                Some(c) => expand(&c, &req.repl, req.use_regex),
                None => req.repl.clone(),
            };
            out.push_str(&line[at..s]);
            out.push_str(&rep);
            at = e;
            n += 1;
            end = out.chars().count();
        }
        if n > 0 {
            out.push_str(&line[at..]);
            res.rows.push((r, out));
            res.count += n;
            res.last_row = Some(r);
            res.last_end = end;
        }
    }
    res
}

/// 选中的整段刚好是一处命中: 返回展开后的替换文字 ($1  之类)
fn op_match_expand(re: &Regex, req: &SearchReq) -> Option<SearchRes> {
    let seg = &req.seg;
    let m = re.captures(seg)?;
    let whole = m.get(0)?;
    if whole.as_str() == seg && whole.start() == 0 {
        Some(SearchRes { text: Some(expand(&m, &req.repl, req.use_regex)), ..Default::default() })
    } else {
        Some(SearchRes::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &[&str] = &["foo Foo foobar", "bar foo FOO", "foo.bar foo(1)", "x (a [b] {c}) y", "last foo"];

    fn req(op: &str, needle: &str, flags: &str) -> SearchReq {
        SearchReq {
            op: op.into(),
            needle: needle.into(),
            whole_word: flags.contains('w'),
            match_case: flags.contains('c'),
            use_regex: flags.contains('r'),
            lines: DOC.iter().map(|s| s.to_string()).collect(),
            doc_id: None,
            doc_hash: None,
            doc_len: 0,
            row: 0,
            col: 0,
            forward: true,
            wrap: flags.contains('p'),
            sel_start: None,
            repl: String::new(),
            sel: None,
            seg: String::new(),
        }
    }

    #[test]
    fn mirrored_document_is_used_only_when_the_fingerprint_matches() {
        let lines: Vec<String> = DOC.iter().map(|s| s.to_string()).collect();
        let fp = crate::doc_mirror::fingerprint(&lines);
        let by_ref = |id: &str, hash: [u32; 2], n: usize| {
            let mut r = req("hits", "foo", "");
            r.lines = vec![];
            r.doc_id = Some(id.into());
            r.doc_hash = Some(hash);
            r.doc_len = n;
            r
        };
        // 没登记过: 要整篇
        assert!(run(&by_ref("t-search-a", fp, lines.len())).unwrap().need_doc);
        // 带整篇登记 (和不带 doc_id 的结果一样)
        let mut put = req("hits", "foo", "");
        put.doc_id = Some("t-search-a".into());
        let direct = run(&req("hits", "foo", "")).unwrap();
        assert_eq!(run(&put).unwrap().hits, direct.hits);
        // 之后只交指纹就够, 结果一样; 指纹 / 行数对不上 = 前端改过, 不能用旧正文
        let via = run(&by_ref("t-search-a", fp, lines.len())).unwrap();
        assert_eq!((via.need_doc, via.hits), (false, direct.hits));
        assert!(run(&by_ref("t-search-a", [fp[0] ^ 1, fp[1]], lines.len())).unwrap().need_doc);
        assert!(run(&by_ref("t-search-a", fp, lines.len() + 1)).unwrap().need_doc);
        // 置換全部也用镜像里的正文
        let mut rep = by_ref("t-search-a", fp, lines.len());
        rep.op = "replace_all".into();
        rep.repl = "Z".into();
        assert_eq!(run(&rep).unwrap().rows[0], (0, "Z Z Zbar".to_string()));
        // 关闭标签: 丢弃
        let mut forget = by_ref("t-search-a", fp, 0);
        forget.op = "forget".into();
        run(&forget);
        assert!(run(&by_ref("t-search-a", fp, lines.len())).unwrap().need_doc);
    }
    #[test]
    fn regex_errors_explain_how_to_search_a_literal_star() {
        // 与 Sakura 一致: 「正则」打开时单独的 * 是语法错误; \* 与 [*] 才是字面的星号
        let e = run(&req("hits", "*", "r")).unwrap().error.unwrap();
        assert!(e.contains("[*]") && e.contains("量词"), "{e}");
        let e = run(&req("hits", "(", "r")).unwrap().error.unwrap();
        assert!(e.contains("右括号"), "{e}");
        let mut r = req("hits", "x*y", "r");
        r.lines = vec!["a*b".into(), "x*y".into(), "xxy".into()];
        assert_eq!(run(&r).unwrap().total, 2); // x*y = 零个或多个 x 再接 y: 命中 "x*y" 里的 y 和 "xxy" (不是字面的 x*y)
        let mut lit = req("hits", "[*]", "r");
        lit.lines = vec!["a*b".into(), "ab".into()];
        assert_eq!(run(&lit).unwrap().hits, vec![Hit { row: 0, col: 1, len: 1 }]);
        let mut esc = req("hits", "a\\*b", "r");
        esc.lines = vec!["a*b".into(), "aab".into()];
        assert_eq!(run(&esc).unwrap().hits, vec![Hit { row: 0, col: 0, len: 3 }]);
        // 「正则」关闭时 * 就是字面字符
        let mut plain = req("hits", "*", "");
        plain.lines = vec!["a*b".into()];
        assert_eq!(run(&plain).unwrap().total, 1);
    }
    #[test]
    fn hits_flags() {
        assert_eq!(run(&req("hits", "foo", "")).unwrap().total, 8); // 忽略大小写 8 处
        assert_eq!(run(&req("hits", "foo", "c")).unwrap().total, 6);
        assert_eq!(run(&req("hits", "foo", "w")).unwrap().total, 7); // 单词单位: 去掉 foobar × 1
        let h = run(&req("hits", "fo+\\w*", "r")).unwrap();
        assert_eq!(h.hits[0], Hit { row: 0, col: 0, len: 3 });
        assert!(run(&req("hits", "(", "r")).unwrap().error.is_some());
    }
    #[test]
    fn step_forward_backward_wrap() {
        let mut r = req("step", "foo", "");
        r.row = 0;
        r.col = 1;
        assert_eq!(run(&r).unwrap().hit, Some(Hit { row: 0, col: 4, len: 3 }));
        r.row = 4;
        r.col = 5;
        assert_eq!(run(&r).unwrap().hit, Some(Hit { row: 4, col: 5, len: 3 })); // 光标停在命中上也算
        r.col = 6;
        assert_eq!(run(&r).unwrap().hit, None); // 不回绕
        r.wrap = true;
        let s = run(&r).unwrap();
        assert_eq!((s.hit, s.wrapped), (Some(Hit { row: 0, col: 0, len: 3 }), true));
        r.forward = false;
        r.row = 0;
        r.col = 0;
        assert_eq!(run(&r).unwrap().hit, Some(Hit { row: 4, col: 5, len: 3 })); // 向前回绕
    }
    #[test]
    fn replace_all_and_groups() {
        let mut r = req("replace_all", "foo", "");
        r.repl = "X".into();
        let s = run(&r).unwrap();
        assert_eq!((s.count, s.rows[0].1.as_str(), s.last_row), (8, "X X Xbar", Some(4)));
        let mut r = req("replace_all", "(f)(o+)", "r");
        r.repl = "[$2$1]".into();
        let s = run(&r).unwrap();
        assert_eq!(s.rows[0].1, "[oof] [ooF] [oof]bar");
        let mut r = req("replace_all", "^foo", "r");
        let s = run(&r).unwrap();
        assert_eq!((s.count, s.rows[0].1.as_str()), (2, " Foo foobar"));
        r.sel = None;
    }
    #[test]
    fn replace_in_selection_only() {
        let mut r = req("replace_all", "foo", "");
        r.repl = "X".into();
        r.sel = Some([1, 0, 2, 3]); // 第 2 行全部 + 第 3 行前 3 列
        let s = run(&r).unwrap();
        assert_eq!(s.rows.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), vec!["bar X X", "X.bar foo(1)"]);
    }
    #[test]
    fn match_expand_selected_hit() {
        let mut r = req("match_expand", "(f)(o+)", "r");
        r.repl = "$2-$1".into();
        r.seg = "foo".into();
        assert_eq!(run(&r).unwrap().text.as_deref(), Some("oo-f"));
        r.seg = "foox".into();
        assert!(run(&r).unwrap().text.is_none());
    }
}
