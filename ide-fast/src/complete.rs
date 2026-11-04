//! 代码补全 (Ctrl+Space) 和「转到定义 (本文件)」的纯逻辑 —— 不碰界面, 可单测。
//! 语义对齐 Tauri 版 editor.html 的 `completionCandidates` / `applyCompletion` / `gotoDefinitionLocal`:
//! 候选 = 当前语言的关键字 > 内建名 > 当前文件里出现过的标识符 (3 个字符以上), 前缀不分大小写匹配, 最多 40 个;
//! 定义 = 第一行形如 `fn|struct|class|let… 符号名` 的行, 找不到就退化成符号的首次出现。

use std::collections::HashSet;

/// 候选最多这么多个
pub const MAX: usize = 40;

/// 补全浮层的状态 (打开时记下文档的 id / 修订号 / 光标, 之后任何一项变了就关掉它)
pub struct Popup {
    pub items: Vec<String>,
    pub idx: usize,
    pub doc_id: u64,
    pub rev: u64,
    pub cursor: [usize; 2],
    /// 选中项刚换过: 下一帧要把它滚进可视范围
    pub scroll: bool,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// 光标 (`col` 个字符) 左边紧挨着的标识符前缀 (`[A-Za-z_][A-Za-z0-9_]*$`); 没有 = 空串
pub fn prefix_before(line: &str, col: usize) -> String {
    let head: Vec<char> = line.chars().take(col).collect();
    let mut i = head.len();
    while i > 0 && is_ident(head[i - 1]) {
        i -= 1;
    }
    // 标识符不能以数字开头: 行首的数字不算前缀
    while i < head.len() && head[i].is_ascii_digit() {
        i += 1;
    }
    head[i..].iter().collect()
}

/// 一行里所有「3 个字符以上的标识符」(`[A-Za-z_][A-Za-z0-9_]{2,}`)
fn identifiers(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut run = String::new();
    let mut flush = |run: &mut String| {
        let w = run.trim_start_matches(|c: char| c.is_ascii_digit());
        if w.chars().count() >= 3 {
            out.push(w.to_string());
        }
        run.clear();
    };
    for c in line.chars() {
        if is_ident(c) {
            run.push(c);
        } else {
            flush(&mut run);
        }
    }
    flush(&mut run);
    out
}

/// 补全候选: 关键字 (排最前) > 内建名 > 文件里的标识符; 同一档里短的在前, 档内按出现顺序。前缀不分大小写, 和前缀完全相同的不列出
pub fn candidates(keywords: &[String], builtins: &[&str], lines: &[String], prefix: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut all: Vec<(String, u8)> = vec![];
    let mut add = |w: &str, rank: u8| {
        if seen.insert(w.to_string()) {
            all.push((w.to_string(), rank));
        }
    };
    for k in keywords {
        add(k, 0);
    }
    for b in builtins {
        add(b, 1);
    }
    for l in lines {
        for w in identifiers(l) {
            add(&w, 2);
        }
    }
    let p = prefix.to_lowercase();
    all.retain(|(w, _)| w.to_lowercase().starts_with(&p) && w != prefix);
    all.sort_by_key(|(w, r)| (*r, w.chars().count())); // 稳定排序: 同档同长保持出现顺序
    all.into_iter().take(MAX).map(|(w, _)| w).collect()
}

/// 把光标前的前缀换成候选词; 返回 (新的整行, 新的光标列)
pub fn apply(line: &str, col: usize, cand: &str) -> (String, usize) {
    let chars: Vec<char> = line.chars().collect();
    let col = col.min(chars.len());
    let plen = prefix_before(line, col).chars().count();
    let head: String = chars[..col - plen].iter().collect();
    let tail: String = chars[col..].iter().collect();
    (format!("{head}{cand}{tail}"), col - plen + cand.chars().count())
}

/// 各语言的内建名 (Tauri 版 `BUILTINS` 表原样搬来); `type_name` 是编辑器的文件类型名, 不分大小写
pub fn builtins(type_name: &str) -> &'static [&'static str] {
    match type_name.to_lowercase().as_str() {
        "rust" => &["clone", "println", "print", "format", "vec", "String", "Vec", "Option", "Some", "None", "Result", "Ok", "Err", "panic", "assert", "assert_eq", "unwrap", "expect", "iter", "map", "filter", "collect", "push", "len", "is_empty", "to_string", "as_str", "clone_from", "matches", "write", "writeln", "cfg", "derive"],
        "python" => &["print", "len", "range", "enumerate", "zip", "open", "str", "int", "float", "list", "dict", "set", "tuple", "sorted", "sum", "min", "max", "abs", "map", "filter", "lambda", "self", "super", "isinstance"],
        "javascript" => &["console", "Array", "Object", "String", "Number", "Boolean", "Promise", "Map", "Set", "JSON", "Math", "Date", "RegExp", "Error", "document", "window", "fetch", "then", "catch", "push", "length", "forEach"],
        "typescript" => &["string", "number", "boolean", "any", "unknown", "never", "void", "Array", "Promise", "Record", "Partial", "console", "JSON", "Math", "Date", "interface", "type", "enum", "readonly", "public", "private"],
        "go" => &["fmt", "Println", "Printf", "Errorf", "make", "new", "append", "copy", "delete", "len", "cap", "close", "nil", "true", "false"],
        "c" => &["printf", "scanf", "malloc", "free", "memset", "memcpy", "strlen", "strcmp", "strcpy", "fopen", "fclose", "NULL", "size_t"],
        "c++" | "cpp" => &["cout", "cin", "endl", "std", "vector", "string", "map", "set", "nullptr", "printf", "cin", "sort", "unique"],
        "java" => &["System", "String", "Integer", "Double", "Boolean", "Math", "ArrayList", "HashMap", "List", "Map", "println", "out"],
        "c#" | "csharp" => &["Console", "WriteLine", "string", "int", "double", "bool", "var", "List", "Dictionary", "Task", "using", "namespace"],
        "ruby" => &["puts", "print", "require", "attr_accessor", "each", "map", "select", "new", "nil", "true", "false", "def", "end"],
        "bash" | "shell" | "sh" => &["echo", "cd", "ls", "cat", "grep", "sed", "awk", "find", "mkdir", "rm", "cp", "mv", "export", "local", "function", "return"],
        "lua" => &["print", "pairs", "ipairs", "type", "tostring", "tonumber", "pcall", "error", "require", "local", "function", "end"],
        "sql" => &["SELECT", "FROM", "WHERE", "INSERT", "UPDATE", "DELETE", "JOIN", "LEFT", "INNER", "GROUP", "ORDER", "BY", "LIMIT", "AS", "ON", "COUNT", "SUM"],
        "markdown" => &["#", "##", "###", "**bold**", "*italic*", "`code`", "-", "1.", ">[", "```"],
        "html" => &["div", "span", "p", "a", "img", "ul", "li", "table", "tr", "td", "th", "h1", "h2", "h3", "form", "input", "button"],
        "css" => &["color", "background", "margin", "padding", "border", "display", "flex", "grid", "position", "width", "height", "font-size"],
        "json" => &["true", "false", "null"],
        "yaml" => &["true", "false", "null", "yes", "no"],
        "toml" => &["true", "false"],
        _ => &[],
    }
}

/// 光标 (`col`) 处的标识符: 光标在标识符里面或紧挨着它的右端 / 左端都算; 没有 = None
pub fn identifier_at(line: &str, col: usize) -> Option<String> {
    let cs: Vec<char> = line.chars().collect();
    let col = col.min(cs.len());
    let (mut a, mut b) = (col, col);
    while a > 0 && is_ident(cs[a - 1]) {
        a -= 1;
    }
    while b < cs.len() && is_ident(cs[b]) {
        b += 1;
    }
    let w: String = cs[a..b].iter().collect();
    let w = w.trim_start_matches(|c: char| c.is_ascii_digit()).to_string();
    (!w.is_empty()).then_some(w)
}

/// 「转到定义 (本文件)」的结果
#[derive(Debug, PartialEq, Eq)]
pub struct Def {
    pub row: usize,
    pub col: usize,
    /// true = 找到了 `fn|struct… 符号` 这样的定义行; false = 只是符号的首次出现
    pub exact: bool,
}

const DEF_WORDS: &[&str] = &["fn", "struct", "enum", "impl", "trait", "type", "func", "class", "def", "function", "interface", "const", "let"];

/// 一行里所有标识符 token: (起始列, 文字)
fn tokens(cs: &[char]) -> Vec<(usize, String)> {
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        if is_ident(cs[i]) {
            let s = i;
            while i < cs.len() && is_ident(cs[i]) {
                i += 1;
            }
            out.push((s, cs[s..i].iter().collect()));
        } else {
            i += 1;
        }
    }
    out
}

/// 在 `lines` 里找 `sym` 的定义: 第一行里有「定义关键字 + 空白 + sym (整词)」的; 都没有就找 sym 第一次出现的位置
pub fn find_definition(lines: &[String], sym: &str) -> Option<Def> {
    if sym.is_empty() {
        return None;
    }
    for (row, l) in lines.iter().enumerate() {
        let cs: Vec<char> = l.chars().collect();
        let toks = tokens(&cs);
        for w in toks.windows(2) {
            let (kw, name) = (&w[0], &w[1]);
            let gap = &cs[kw.0 + kw.1.chars().count()..name.0];
            if DEF_WORDS.contains(&kw.1.as_str()) && name.1 == sym && !gap.is_empty() && gap.iter().all(|c| c.is_whitespace()) {
                return Some(Def { row, col: name.0, exact: true });
            }
        }
    }
    lines.iter().enumerate().find_map(|(row, l)| l.find(sym).map(|b| Def { row, col: l[..b].chars().count(), exact: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn prefix_is_the_identifier_just_left_of_the_cursor() {
        assert_eq!(prefix_before("let foo_b", 9), "foo_b");
        assert_eq!(prefix_before("foo(bar", 3), "foo", "光标在中间: 只看左边");
        assert_eq!(prefix_before("x = 9abc", 8), "abc", "标识符不能以数字开头");
        assert_eq!(prefix_before("a-b", 3), "b");
        assert_eq!(prefix_before("日本ab", 4), "ab", "按字符数算列, 不是字节");
        assert_eq!(prefix_before("foo ", 4), "", "空格后没有前缀");
        assert_eq!(prefix_before("", 0), "");
    }

    #[test]
    fn candidates_rank_keywords_then_builtins_then_file_identifiers() {
        let kw = v(&["fn", "for", "struct"]);
        let lines = v(&["let foobar = 1;", "let fo2 = foobar + format_x;", "ab xy"]);
        // 前缀 "f": 关键字 fn for | 内建 format | 文件里 foobar fo2 format_x (fo2 只有 3 个字符也算)
        let got = candidates(&kw, &["format", "println"], &lines, "f");
        assert_eq!(got, ["fn", "for", "format", "fo2", "foobar", "format_x"], "关键字 > 内建 > 标识符; 同档里短的在前");
        assert_eq!(candidates(&kw, &[], &lines, "FO"), ["for", "fo2", "foobar", "format_x"], "前缀不分大小写");
        assert!(!candidates(&kw, &[], &lines, "foobar").contains(&"foobar".to_string()), "和前缀完全相同的不列出");
        assert!(candidates(&kw, &[], &lines, "ab").is_empty(), "2 个字符的标识符不进候选");
        assert_eq!(candidates(&kw, &[], &[], "zz"), Vec::<String>::new());
    }

    #[test]
    fn candidates_are_deduplicated_and_capped() {
        let lines: Vec<String> = (0..100).map(|i| format!("word{i:03}")).collect();
        let got = candidates(&[], &[], &lines, "word");
        assert_eq!(got.len(), MAX);
        assert_eq!(got[0], "word000", "同长同档按出现顺序");
        let dup = candidates(&v(&["print"]), &["print"], &v(&["print print"]), "pr");
        assert_eq!(dup, ["print"], "同一个词只出现一次, 档次取最靠前的");
    }

    #[test]
    fn apply_replaces_only_the_prefix_and_keeps_the_tail() {
        assert_eq!(apply("let fo = 1;", 6, "foobar"), ("let foobar = 1;".to_string(), 10));
        assert_eq!(apply("fo", 2, "foo"), ("foo".to_string(), 3));
        assert_eq!(apply("x.", 2, "name"), ("x.name".to_string(), 6), "没有前缀: 直接插入");
        assert_eq!(apply("日本ab(", 4, "abc"), ("日本abc(".to_string(), 5), "多字节字符");
        assert_eq!(apply("ab", 99, "abc"), ("abc".to_string(), 3), "列超出行尾: 夹到行尾");
    }

    #[test]
    fn builtin_tables_follow_the_file_type_name() {
        assert!(builtins("Rust").contains(&"println"));
        assert!(builtins("C++").contains(&"cout") && builtins("cpp").contains(&"cout"));
        assert!(builtins("C#").contains(&"WriteLine"));
        assert!(builtins("Bash").contains(&"echo"));
        assert!(builtins("Plain").is_empty() && builtins("无此类型").is_empty());
    }

    #[test]
    fn identifier_at_covers_inside_and_both_edges() {
        assert_eq!(identifier_at("let foo = 1", 5).as_deref(), Some("foo"), "在词里");
        assert_eq!(identifier_at("let foo = 1", 7).as_deref(), Some("foo"), "紧挨右端");
        assert_eq!(identifier_at("let foo = 1", 4).as_deref(), Some("foo"), "紧挨左端");
        assert_eq!(identifier_at("a  b", 2), None, "两个词之间的空白");
        assert_eq!(identifier_at("", 0), None);
    }

    #[test]
    fn definition_prefers_a_definition_line_over_the_first_use() {
        let lines = v(&["main();", "    helper(1);", "fn helper(x: i32) {}", "struct Foo;", "let helper_two = 2;", "class Bar:", "const  Z = 1"]);
        assert_eq!(find_definition(&lines, "helper"), Some(Def { row: 2, col: 3, exact: true }), "跳过前面的调用, 找到 fn 行");
        assert_eq!(find_definition(&lines, "Foo"), Some(Def { row: 3, col: 7, exact: true }));
        assert_eq!(find_definition(&lines, "Bar"), Some(Def { row: 5, col: 6, exact: true }));
        assert_eq!(find_definition(&lines, "Z"), Some(Def { row: 6, col: 7, exact: true }), "关键字后可以有多个空白");
        assert_eq!(find_definition(&lines, "helper_two"), Some(Def { row: 4, col: 4, exact: true }), "let 也算定义");
        // 名字只是别的标识符的一部分, 不能算
        assert_eq!(find_definition(&v(&["fn helper_two() {}"]), "helper"), Some(Def { row: 0, col: 3, exact: false }), "整词匹配; 退化成首次出现");
    }

    #[test]
    fn definition_falls_back_to_first_occurrence_and_then_none() {
        let lines = v(&["日本 use_it(x)", "again use_it"]);
        assert_eq!(find_definition(&lines, "use_it"), Some(Def { row: 0, col: 3, exact: false }), "列按字符数");
        assert_eq!(find_definition(&lines, "missing"), None);
        assert_eq!(find_definition(&lines, ""), None);
        assert_eq!(find_definition(&v(&["fnfoo", "fn  foo"]), "foo"), Some(Def { row: 1, col: 4, exact: true }), "fnfoo 不是 fn + foo");
    }
}
