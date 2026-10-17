//! ide-shell-web — Search backend: 纯 Rust 内置实现 (对标 sakura-editor 的 Grep).
//!
//! # 为什么不再 shell out 到外部 rg
//!
//! 原实现是 `Command::new("rg")` + 缺 rg 时退到无 glob 支持的 `search_fallback`.
//! 三个问题:
//!   1. 违反 docs/SPA-01_sakura_parity.md §3 原则 3「Rust 优先: 不 shell out 到外部二进制」;
//!   2. Windows 上 rg 不保证在 PATH, 缺了就整体降级, 用户不知道自己拿到的是弱化行为;
//!   3. Sakura 的 Grep 语义 (`!` 排除文件 / `#` 排除目录 / 多分隔符) 与 rg 的 glob
//!      不是一一对应, 靠「前端翻译 + 外部再翻译」两层拼凑必然漂移.
//!
//! 现在 glob 匹配、目录枚举、内容搜索全部内置, 与 Sakura 语义直接对齐.
//!
//! # Sakura 文件模式语义 (docs/SPA-01 G-10 ~ G-15)
//!
//! - `include_globs`: `*` / `?` 通配, 按**文件名**匹配 (非路径). 空 = 不过滤.
//! - `exclude_globs`: 认 `!` 前缀 (排除文件), 也认裸目录名 / `dir/**` (排除整个子树).
//!   前端 `excludeDirGlobs` 把 `#target` 翻译成 `!target` 与 `!target/**` 两条.
//! - 排除优先于包含, 与书写顺序无关 (G-14).
//! - `.` 开头的隐藏文件跳过; `node_modules` / `target` / `.git` / `dist` 等固定跳过.
//!
//! # 正则
//!
//! regex=true 时用内置回溯引擎 `MiniRegex` (支持 `^ $ . * + ? | ( ) [ ] {n,m} \d \w \s \b`
//! 与转义). 不引外部正则 crate —— 本项目要求零新增重型依赖, 且 Grep 常用语法已被覆盖.

#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub pattern: String,
    pub case_sensitive: Option<bool>,
    pub regex: Option<bool>,
    pub include_globs: Option<Vec<String>>,
    pub exclude_globs: Option<Vec<String>>,
    pub max_results: Option<usize>,
}

/// 固定跳过的目录名 (编译产物 / 依赖 / VCS).
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", ".git", "dist", ".svn", "__pycache__", ".idea",
];

// ---------------------------------------------------------------------------
// glob 匹配
// ---------------------------------------------------------------------------

/// glob 转正则源: `*` -> `.*`, `?` -> `.`, 其余按字面量转义. 只按**名字**匹配.
fn glob_to_regex(pat: &str) -> String {
    let mut out = String::from("^");
    for c in pat.chars() {
        match c {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            c => {
                if "\\.+()|[]{}^$".contains(c) {
                    out.push('\\');
                }
                out.push(c);
            }
        }
    }
    out.push('$');
    out
}

/// 用内置正则做一次整串匹配.
fn glob_match(name: &str, pat: &str, case_sensitive: bool) -> bool {
    // 关键: 必须把 glob **转换后**的正则源交给引擎。
    // 曾经这里误传了原始 pattern (`*.rs`), 于是 `*` 被正则引擎当成量词解析 ->
    // MiniRegex::new 失败 -> unwrap_or(false) -> 所有 glob 静默失配 (搜索结果恒为空).
    let re_src = glob_to_regex(pat);
    // case_sensitive=false 时双方小写化后再比 (glob 只关心字符类, 不需 Anchor/WordBoundary)
    let (a, b) = if case_sensitive {
        (name.to_string(), re_src)
    } else {
        (name.to_lowercase(), re_src.to_lowercase())
    };
    MiniRegex::new(&b, true)
        .map(|re| re.full_match(&a))
        .unwrap_or(false)
}

/// 目录排除模式: 接受 `d`、`d/`、`d/**`、`!d`、`!d/**`; 命中即跳过整个子树.
/// (rg 只写 `!d` 排不掉子树内容, 这是必须显式支持 `d/**` 的原因)
fn is_dir_exclude(pat: &str, dir_name: &str, dir_rel: &str) -> bool {
    let p = pat.trim();
    let p = p.strip_prefix('!').unwrap_or(p);
    let p = p.strip_suffix("/**").unwrap_or(p);
    let p = p.trim_end_matches('/');
    if p.is_empty() {
        return false;
    }
    if p.contains('/') {
        dir_rel == p || dir_rel.starts_with(&format!("{}/", p))
    } else {
        dir_name == p
    }
}

/// 文件是否通过 include / exclude 过滤. exclude 优先.
fn file_passes(name: &str, inc: &[String], exc: &[String], case_sensitive: bool) -> bool {
    for e in exc {
        let bare = e.trim();
        let bare = bare.strip_prefix('!').unwrap_or(bare).trim();
        if bare.is_empty() {
            continue;
        }
        let hit = if bare.contains('*') || bare.contains('?') {
            glob_match(name, bare, case_sensitive)
        } else {
            name == bare
        };
        if hit {
            return false;
        }
    }
    // include 为空 = 不过滤 (调用方没给 include_globs, 或给了空列表).
    // 这里必须显式判空: `inc.iter().any(...)` 对空列表返回 false, 会把**所有**文件
    // 都排掉 —— 这是第一版把所有搜索都跑出 0 结果的根因.
    if inc.is_empty() {
        return true;
    }
    inc.iter().any(|g| glob_match(name, g.trim(), case_sensitive))
}

// ---------------------------------------------------------------------------
// 目录遍历
// ---------------------------------------------------------------------------

fn walk(_root: &Path, exc: &[String], out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 64 || out.len() > 200_000 {
        return; // 防御: 符号链接环 / 超大目录
    }
    let Ok(rd) = fs::read_dir(_root) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name()); // 稳定输出, 便于测试断言
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue; // 不跟随符号链接, 防环
        }
        if ft.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            let rel = path
                .strip_prefix(_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if exc.iter().any(|p| is_dir_exclude(p, &name, &rel)) {
                continue;
            }
            // 递归必须传 `path`, 不是 `root` —— 传 root 会让每一层都从根重新遍历,
            // 产生指数级重复 (实测 2 层目录扫出 65 个「文件」, 其中大量是同一个 keep.rs).
            walk(&path, exc, out, depth + 1);
        } else if ft.is_file() {
            if name.starts_with('.') {
                continue;
            }
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// 主入口
// ---------------------------------------------------------------------------

pub fn search(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    let case_sensitive = opts.case_sensitive.unwrap_or(false);
    let max = opts.max_results.unwrap_or(usize::MAX);
    let inc: Vec<String> = opts.include_globs.clone().unwrap_or_default();
    let exc: Vec<String> = opts.exclude_globs.clone().unwrap_or_default();

    let mut files = Vec::new();
    walk(cwd, &exc, &mut files, 0);

    let matcher = Matcher::new(&opts.pattern, opts.regex.unwrap_or(false), case_sensitive)?;

    let mut results = Vec::new();
    'outer: for f in files {
        let Some(name) = f.file_name().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if !file_passes(&name, &inc, &exc, case_sensitive) {
            continue;
        }
        let Ok(file) = fs::File::open(&f) else { continue };
        let reader = BufReader::new(file);
        for (i, line) in reader.lines().enumerate() {
            let Ok(text) = line else { continue };
            if matcher.matches(&text) {
                results.push(SearchResult {
                    path: f.to_string_lossy().into_owned(),
                    line: i as u64 + 1,
                    text,
                });
                if results.len() >= max {
                    break 'outer;
                }
            }
        }
    }
    Ok(results)
}

/// 兼容旧调用点: 现在与 `search` 同一实现 (不再是「rg 缺失时的弱化降级」).
pub fn search_fallback(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    search(cwd, opts)
}

// ---------------------------------------------------------------------------
// 内容匹配 (字面量 / 内置正则)
// ---------------------------------------------------------------------------

enum Matcher {
    Literal {
        needle: String,
        /// case_sensitive=false 时预先小写化; 匹配时把行也小写.
        folded: Option<String>,
    },
    Re(Box<MiniRegex>),
}

impl Matcher {
    fn new(pattern: &str, regex: bool, case_sensitive: bool) -> Result<Self, String> {
        if regex {
            let re = MiniRegex::new(pattern, case_sensitive)
                .map_err(|e| format!("正则表达式无效: {e}"))?;
            Ok(Matcher::Re(Box::new(re)))
        } else if case_sensitive {
            Ok(Matcher::Literal {
                needle: pattern.to_string(),
                folded: None,
            })
        } else {
            Ok(Matcher::Literal {
                needle: pattern.to_string(),
                folded: Some(pattern.to_lowercase()),
            })
        }
    }

    fn matches(&self, line: &str) -> bool {
        match self {
            Matcher::Literal { needle, folded } => match folded {
                None => line.contains(needle.as_str()),
                Some(f) => line.to_lowercase().contains(f.as_str()),
            },
            Matcher::Re(re) => re.is_match(line),
        }
    }
}

// ---------------------------------------------------------------------------
// MiniRegex — 内置回溯正则引擎
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any, // . 默认不匹配 \n (与 rg / Sakura 一致)
    Class(Vec<ClassItem>, bool), // (字符集, 取反)
    Start,
    End,
    Repeat(Box<Node>, usize, Option<usize>, bool), // (子, min, max, greedy)
    Group(Box<Node>),
    Alt(Vec<Node>),   // 或: 分支择一
    Seq(Vec<Node>),   // 串联: 按序全部匹配
    Empty,            // 空分支 (如 "a|")
    WordBoundary(bool),
}

#[derive(Debug, Clone)]
enum ClassItem {
    Ch(char),
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match self {
            ClassItem::Ch(x) => *x == c,
            ClassItem::Range(a, b) => *a <= c && c <= *b,
            ClassItem::Digit(pos) => c.is_ascii_digit() == *pos,
            ClassItem::Word(pos) => (c.is_alphanumeric() || c == '_') == *pos,
            ClassItem::Space(pos) => c.is_whitespace() == *pos,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MiniRegex {
    root: Node,
    case_sensitive: bool,
}

struct ParseCtx {
    chars: Vec<char>,
    i: usize,
}

impl ParseCtx {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }
    fn next(&mut self) -> Option<char> {
        let c = self.chars.get(self.i).copied();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn err(&self, msg: &str) -> String {
        let near: String = self.chars[self.i.min(self.chars.len())..]
            .iter()
            .take(6)
            .collect();
        format!("{msg} (位置 {} 附近: \"{}\")", self.i, near)
    }
}

impl MiniRegex {
    pub fn new(pattern: &str, case_sensitive: bool) -> Result<Self, String> {
        if pattern.is_empty() {
            return Ok(MiniRegex {
                root: Node::Empty,
                case_sensitive,
            });
        }
        let mut ctx = ParseCtx {
            chars: pattern.chars().collect(),
            i: 0,
        };
        let root = parse_alt(&mut ctx)?;
        if ctx.i < ctx.chars.len() {
            return Err(ctx.err("多余的字符"));
        }
        Ok(MiniRegex {
            root,
            case_sensitive,
        })
    }

    /// 任意位置出现即算匹配 (rg 语义).
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = if self.case_sensitive {
            text.chars().collect()
        } else {
            text.to_lowercase().chars().collect()
        };
        for start in 0..=chars.len() {
            if match_node(&self.root, &chars, start, &mut |_| true) {
                return true;
            }
        }
        false
    }

    /// 整串匹配 (glob 用). 只从位置 0 起匹配**一次**, 不像 is_match 那样遍历所有起点 ——
    /// 遍历起点会让 `a?.rs` 匹配到 "ab.rs" 里的子串, 这是第一版 glob 全错的根因之一.
    /// 锚点由 glob_to_regex 生成的 ^$ 承担, 这里不重复判断.
    pub fn full_match(&self, text: &str) -> bool {
        let chars: Vec<char> = if self.case_sensitive {
            text.chars().collect()
        } else {
            text.to_lowercase().chars().collect()
        };
        let n = chars.len();
        match_node(&self.root, &chars, 0, &mut |p| p == n)
    }
}

fn parse_alt(ctx: &mut ParseCtx) -> Result<Node, String> {
    let mut branches = vec![parse_seq(ctx)?];
    while ctx.eat('|') {
        branches.push(parse_seq(ctx)?);
    }
    Ok(if branches.len() == 1 {
        branches.pop().expect("len>=1")
    } else {
        Node::Alt(branches)
    })
}

fn parse_seq(ctx: &mut ParseCtx) -> Result<Node, String> {
    let mut items: Vec<Node> = Vec::new();
    while let Some(c) = ctx.peek() {
        if c == '|' || c == ')' {
            break;
        }
        items.push(parse_repeat(ctx)?);
    }
    // 串联必须用 Seq: 不能复用 Alt —— Alt 是「择一」, 复用会让 "abc" 变成
    // 匹配 a/b/c 之一, 而不是依次匹配 abc (这是第一版的真实 bug).
    Ok(match items.len() {
        0 => Node::Empty,
        1 => items.pop().expect("len==1"),
        _ => Node::Seq(items),
    })
}

fn parse_repeat(ctx: &mut ParseCtx) -> Result<Node, String> {
    let atom = parse_atom(ctx)?;
    let Some(c) = ctx.peek() else { return Ok(atom) };
    let (min, max) = match c {
        '*' => {
            ctx.i += 1;
            (0, None)
        }
        '+' => {
            ctx.i += 1;
            (1, None)
        }
        '?' => {
            ctx.i += 1;
            (0, Some(1))
        }
        '{' => {
            let save = ctx.i;
            ctx.i += 1;
            let mut lo = String::new();
            while ctx.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                lo.push(ctx.next().expect("digit"));
            }
            if lo.is_empty() {
                ctx.i = save;
                return Ok(atom);
            }
            let lo_n: usize = lo.parse().map_err(|_| "量词过大".to_string())?;
            let hi = if ctx.eat(',') {
                let mut h = String::new();
                while ctx.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    h.push(ctx.next().expect("digit"));
                }
                if h.is_empty() {
                    None
                } else {
                    Some(h.parse::<usize>().map_err(|_| "量词过大".to_string())?)
                }
            } else {
                Some(lo_n)
            };
            if !ctx.eat('}') {
                ctx.i = save;
                return Ok(atom);
            }
            (lo_n, hi)
        }
        _ => return Ok(atom),
    };
    let greedy = !ctx.eat('?'); // ? 修饰 = 非贪婪
    Ok(Node::Repeat(Box::new(atom), min, max, greedy))
}

fn parse_atom(ctx: &mut ParseCtx) -> Result<Node, String> {
    let Some(c) = ctx.next() else {
        return Err(ctx.err("模式意外结束"));
    };
    Ok(match c {
        '^' => Node::Start,
        '$' => Node::End,
        '.' => Node::Any,
        '(' => {
            if ctx.peek() == Some('?') {
                ctx.i += 1;
                if !ctx.eat(':') {
                    return Err(ctx.err("暂不支持的分组修饰"));
                }
            }
            let inner = parse_alt(ctx)?;
            if !ctx.eat(')') {
                return Err(ctx.err("缺少右括号 )"));
            }
            Node::Group(Box::new(inner))
        }
        '[' => parse_class(ctx)?,
        '\\' => {
            let Some(e) = ctx.next() else {
                return Err(ctx.err("反斜杠后缺字符"));
            };
            match e {
                'd' => Node::Class(vec![ClassItem::Digit(true)], false),
                'D' => Node::Class(vec![ClassItem::Digit(false)], false),
                'w' => Node::Class(vec![ClassItem::Word(true)], false),
                'W' => Node::Class(vec![ClassItem::Word(false)], false),
                's' => Node::Class(vec![ClassItem::Space(true)], false),
                'S' => Node::Class(vec![ClassItem::Space(false)], false),
                'b' => Node::WordBoundary(true),
                'B' => Node::WordBoundary(false),
                'n' => Node::Char('\n'),
                't' => Node::Char('\t'),
                'r' => Node::Char('\r'),
                other => Node::Char(other),
            }
        }
        ')' => return Err(ctx.err("多余的右括号 )")),
        '*' | '+' | '?' => return Err(ctx.err("量词前面没有可重复的内容")),
        other => Node::Char(other),
    })
}

fn parse_class(ctx: &mut ParseCtx) -> Result<Node, String> {
    let neg = ctx.eat('^');
    let mut items = Vec::new();
    let mut first = true;
    loop {
        let Some(c) = ctx.next() else {
            return Err(ctx.err("字符集缺少右方括号 ]"));
        };
        if c == ']' && !first {
            break;
        }
        first = false;
        if c == '[' && ctx.peek() == Some(':') {
            // POSIX 子集 [:alpha:]
            let mut name = String::new();
            ctx.i += 1;
            while let Some(x) = ctx.peek() {
                if x == ':' {
                    break;
                }
                name.push(x);
                ctx.i += 1;
            }
            ctx.eat(':');
            ctx.eat(']');
            match name.as_str() {
                "alpha" | "alnum" => {
                    items.push(ClassItem::Range('a', 'z'));
                    items.push(ClassItem::Range('A', 'Z'));
                    if name == "alnum" {
                        items.push(ClassItem::Digit(true));
                    }
                }
                "digit" => items.push(ClassItem::Digit(true)),
                "space" => items.push(ClassItem::Space(true)),
                _ => return Err(ctx.err(&format!("暂不支持的字符集子集 [:{name}:]"))),
            }
            continue;
        }
        let lo = if c == '\\' {
            let Some(e) = ctx.next() else {
                return Err(ctx.err("字符集内反斜杠后缺字符"));
            };
            match e {
                'd' => {
                    items.push(ClassItem::Digit(true));
                    continue;
                }
                'D' => {
                    items.push(ClassItem::Digit(false));
                    continue;
                }
                'w' => {
                    items.push(ClassItem::Word(true));
                    continue;
                }
                'W' => {
                    items.push(ClassItem::Word(false));
                    continue;
                }
                's' => {
                    items.push(ClassItem::Space(true));
                    continue;
                }
                'S' => {
                    items.push(ClassItem::Space(false));
                    continue;
                }
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                o => o,
            }
        } else {
            c
        };
        if ctx.peek() == Some('-') && ctx.chars.get(ctx.i + 1).copied() != Some(']') {
            ctx.i += 1;
            let hi = ctx.next().ok_or_else(|| ctx.err("区间缺上界"))?;
            items.push(ClassItem::Range(lo, hi));
        } else {
            items.push(ClassItem::Ch(lo));
        }
    }
    Ok(Node::Class(items, neg))
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 串联匹配: items[0..] 依次匹配, 末项接调用方 k.
fn match_seq(items: &[Node], chars: &[char], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match items.split_first() {
        None => k(pos),
        Some((head, rest)) => match_node(head, chars, pos, &mut |p| {
            // 尾项直接把调用方的 k 接上; 非尾项继续递归下一项
            if rest.is_empty() {
                k(p)
            } else {
                match_seq(rest, chars, p, k)
            }
        }),
    }
}

/// 回溯匹配: 在 chars[pos..] 上匹配 node, 成功则用 k 续跑后续部分.
fn match_node(node: &Node, chars: &[char], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match node {
        Node::Char(c) => pos < chars.len() && chars[pos] == *c && k(pos + 1),
        Node::Any => pos < chars.len() && chars[pos] != '\n' && k(pos + 1),
        Node::Class(items, neg) => {
            if pos >= chars.len() {
                return false;
            }
            let c = chars[pos];
            (items.iter().any(|it| it.matches(c)) != *neg) && k(pos + 1)
        }
        Node::Start => pos == 0 && k(pos),
        Node::End => pos == chars.len() && k(pos),
        Node::WordBoundary(want) => {
            let before = pos > 0 && is_word(chars[pos - 1]);
            let after = pos < chars.len() && is_word(chars[pos]);
            ((before != after) == *want) && k(pos)
        }
        Node::Group(inner) => match_node(inner, chars, pos, k),
        Node::Empty => k(pos),
        Node::Alt(branches) => branches.iter().any(|b| match_node(b, chars, pos, k)),
        // 串联: 逐项接续, 最后一项用调用方的 k 收尾 (回溯在每一项内自然发生)
        Node::Seq(items) => match_seq(items, chars, pos, k),
        Node::Repeat(inner, min, max, greedy) => {
            match_repeat(inner, chars, pos, *min, *max, *greedy, 0, k)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn match_repeat(
    inner: &Node,
    chars: &[char],
    pos: usize,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    done: usize,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let can_more = max.map(|m| done < m).unwrap_or(true);
    let try_more = |k: &mut dyn FnMut(usize) -> bool| -> bool {
        can_more
            && match_node(inner, chars, pos, &mut |np| {
                // 零宽保护: 子模式匹配空串且已满足 min 时停止, 否则死循环
                if np == pos && done >= min {
                    return false;
                }
                match_repeat(inner, chars, np, min, max, greedy, done + 1, k)
            })
    };
    if done < min {
        return try_more(k);
    }
    if greedy {
        if try_more(k) {
            return true;
        }
        k(pos)
    } else {
        if k(pos) {
            return true;
        }
        try_more(k)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 临时目录 helper — 不引外部 dep, 每实例唯一.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let mut p = std::env::temp_dir();
            p.push(format!("ide10-search-{tag}-{}-{}", std::process::id(), nanos));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn opts(pattern: &str) -> SearchOptions {
        SearchOptions {
            pattern: pattern.into(),
            case_sensitive: Some(true),
            regex: Some(false),
            include_globs: None,
            exclude_globs: None,
            max_results: None,
        }
    }

    fn fname(p: &str) -> String {
        Path::new(p)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn setup(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("keep.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
        fs::write(dir.join("skip.rs"), "fn alpha() {}\n").unwrap();
        fs::write(dir.join("skip.txt"), "alpha beta\n").unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/deep.rs"), "fn alpha() {}\n").unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("target/gen.rs"), "fn alpha() {}\n").unwrap();
    }

    // --- 原有三个测试, 去掉 have_rg() 跳过 (现在纯 Rust, 不依赖外部 rg) ---

    #[test]
    fn exclude_glob_excludes_files_not_inverts_matches() {
        let dir = TempDir::new("exclude");
        setup(dir.path());
        let mut o = opts("alpha");
        o.include_globs = Some(vec!["*.rs".into()]);
        o.exclude_globs = Some(vec!["skip*".into()]);
        let hits = search(dir.path(), &o).expect("search 应成功");
        assert!(hits.iter().all(|h| !h.path.contains("skip")), "exclude 应排除文件");
        assert!(hits.iter().any(|h| h.path.contains("keep")), "keep.rs 应有命中");
        assert!(hits.iter().any(|h| h.path.contains("sub")), "子目录也应被搜到");
    }

    #[test]
    fn dash_leading_pattern_not_treated_as_option() {
        let dir = TempDir::new("dash");
        setup(dir.path());
        // 原来靠 rg 的 `--` 终止符防参数注入; 现在没有外部进程, 整串按字面量处理.
        let mut o = opts("--pre=echo PWNED");
        o.include_globs = Some(vec!["*.rs".into()]);
        let r = search(dir.path(), &o);
        assert!(r.is_ok(), "带 -- 的 pattern 应被当字面量");
        assert!(r.unwrap().is_empty(), "字面量不该命中");
    }

    #[test]
    fn already_negated_exclude_not_double_negated() {
        let dir = TempDir::new("excl2");
        setup(dir.path());
        let mut o = opts("alpha");
        o.include_globs = Some(vec!["*.rs".into()]);
        o.exclude_globs = Some(vec!["!skip*".into()]); // 前端已加 !
        let hits = search(dir.path(), &o).expect("search 应成功");
        assert!(hits.iter().all(|h| !h.path.contains("skip")));
    }

    // --- Sakura 语义 ---

    #[test]
    fn dir_exclude_prunes_whole_subtree() {
        // rg 的 `--glob '!target'` 只排掉名为 target 的**文件**, 排不掉其内容;
        // Sakura 的 `#target` 必须排掉整个子树 -> 依赖 `!target/**` 这一条.
        let dir = TempDir::new("direxcl");
        setup(dir.path());
        let mut o = opts("alpha");
        o.exclude_globs = Some(vec!["!target".into(), "!target/**".into()]);
        let hits = search(dir.path(), &o).unwrap();
        assert!(hits.iter().all(|h| !h.path.contains("target")), "target 子树应被整体排除");
    }

    #[test]
    fn built_in_skip_dirs_never_searched() {
        let dir = TempDir::new("skipdirs");
        setup(dir.path());
        let hits = search(dir.path(), &opts("alpha")).unwrap();
        assert!(
            hits.iter().all(|h| !h.path.contains("target")),
            "target 是固定跳过目录, 未经配置也不该出现"
        );
    }

    #[test]
    fn max_results_truncates() {
        let dir = TempDir::new("maxres");
        setup(dir.path());
        let mut o = opts("alpha");
        o.max_results = Some(2);
        assert_eq!(search(dir.path(), &o).unwrap().len(), 2, "应被 max_results 截断");
    }

    #[test]
    fn case_insensitive_when_flag_off() {
        let dir = TempDir::new("ci");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), "Hello World\n").unwrap();
        let mut o = opts("hello");
        o.case_sensitive = Some(false);
        assert_eq!(search(dir.path(), &o).unwrap().len(), 1, "忽略大小写应命中 Hello");
        let mut o2 = opts("hello");
        o2.case_sensitive = Some(true);
        assert!(search(dir.path(), &o2).unwrap().is_empty(), "区分大小写不该命中");
    }

    #[test]
    fn include_glob_wildcards() {
        let dir = TempDir::new("wild");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.rs"), "hit\n").unwrap();
        fs::write(dir.path().join("ab.rs"), "hit\n").unwrap();
        fs::write(dir.path().join("abc.rs"), "hit\n").unwrap();

        // `?` 恰好一个字符: 只命中 ab.rs (a.rs 少一字符, abc.rs 多一字符)
        let mut q = opts("hit");
        q.include_globs = Some(vec!["a?.rs".into()]);
        let hits = search(dir.path(), &q).unwrap();
        assert_eq!(hits.len(), 1, "? 恰好一个字符, 只该命中 ab.rs");
        assert_eq!(fname(&hits[0].path), "ab.rs");

        // `*` 跨任意长度: 三个都命中
        let mut s = opts("hit");
        s.include_globs = Some(vec!["a*.rs".into()]);
        assert_eq!(search(dir.path(), &s).unwrap().len(), 3, "a*.rs 应命中三个");
    }

    #[test]
    fn regex_mode_uses_builtin_engine() {
        let dir = TempDir::new("re");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), "foo123\nbar\nfoo456\n").unwrap();
        let mut o = opts(r"foo\d+");
        o.regex = Some(true);
        assert_eq!(search(dir.path(), &o).unwrap().len(), 2, r"foo\d+ 应命中两行");
    }

    #[test]
    fn literal_mode_does_not_treat_pattern_as_regex() {
        let dir = TempDir::new("lit");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), "a.c\nabc\n").unwrap();
        let mut o = opts("a.c");
        o.regex = Some(false);
        assert_eq!(search(dir.path(), &o).unwrap().len(), 1, "字面量模式只有第 1 行含 a.c");
        let mut r = opts("a.c");
        r.regex = Some(true);
        assert_eq!(search(dir.path(), &r).unwrap().len(), 2, "正则模式下 '.' 是通配");
    }

    // --- MiniRegex 引擎 ---

    fn m(p: &str) -> MiniRegex {
        MiniRegex::new(p, true).unwrap()
    }

    #[test]
    fn regex_anchors() {
        assert!(m(r"^abc").is_match("abcdef"));
        assert!(!m(r"^abc").is_match("zabcdef"));
        assert!(m(r"abc$").is_match("xxabc"));
        assert!(m(r"^abc$").is_match("abc"));
        assert!(!m(r"^abc$").is_match("abcd"));
    }

    #[test]
    fn regex_quantifiers() {
        assert!(m(r"ab*c").is_match("ac"));
        assert!(m(r"ab+c").is_match("abbc"));
        assert!(!m(r"ab+c").is_match("ac"));
        assert!(m(r"a{2,3}").is_match("aa"));
        assert!(m(r"a{2,3}").is_match("aaa"));
        assert!(!m(r"a{2,3}").is_match("a"));
    }

    #[test]
    fn regex_classes_and_escapes() {
        assert!(m(r"\d+").is_match("abc123"));
        assert!(m(r"[a-c]+").is_match("bbb"));
        assert!(!m(r"[^a-c]").is_match("bbb"));
        assert!(m(r"[^a-c]").is_match("zzz"));
        assert!(m(r"\bword\b").is_match("a word here"));
        assert!(!m(r"\bword\b").is_match("wordy"));
        assert!(m(r"a\.b").is_match("a.b"));
        assert!(!m(r"a\.b").is_match("axb"));
    }

    #[test]
    fn regex_alternation_and_groups() {
        assert!(m(r"cat|dog").is_match("hotdog"));
        assert!(m(r"(ab)+c").is_match("ababc"));
        assert!(m(r"(?:xy)z").is_match("xyz"));
    }

    #[test]
    fn regex_case_insensitive() {
        assert!(MiniRegex::new("hello", false).unwrap().is_match("HELLO World"));
        assert!(!MiniRegex::new("hello", true).unwrap().is_match("HELLO"));
    }

    #[test]
    fn regex_invalid_pattern_errors() {
        assert!(MiniRegex::new("(unclosed", true).is_err());
        assert!(MiniRegex::new("[unclosed", true).is_err());
        assert!(MiniRegex::new("*bad", true).is_err());
        assert!(MiniRegex::new("a)", true).is_err());
    }

    #[test]
    fn regex_zero_width_repeat_terminates() {
        // (a*)* 在纯 a 上若无零宽保护会无限递归.
        let re = m(r"^(a*)*b");
        assert!(!re.is_match(&"a".repeat(12)), "不该匹配无 b 的串");
        assert!(re.is_match("aab"), "应以 b 收尾");
        assert!(re.is_match("b"), "空重复也能到 b");
    }

    #[test]
    fn dot_does_not_cross_newline() {
        let re = m(r"a.b");
        assert!(re.is_match("axb"));
        assert!(!re.is_match("a\nb"), ". 默认不该匹配换行");
    }
}
