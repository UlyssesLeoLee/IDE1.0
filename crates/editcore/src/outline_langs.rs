//! 大括号语言 (C/C++、Java、C#、JavaScript/TypeScript、Go) 与 Markdown 的大纲。
//!
//! 不引正则依赖: 逐行把注释和字符串内容抹成空格后, 用括号栈区分「容器」(class/struct/namespace/…)
//! 与函数体; 只有当栈里全是容器时才认函数定义, 所以函数体内的调用语句不会混进大纲。
//! 每个条目带 `depth` (所在容器的层数), 前端据此缩进成层级。

use crate::type_config::OutlineEntry;

const CONTAINERS: &[&str] = &["class", "struct", "namespace", "interface", "enum", "union", "record"];
const NOT_FUNC: &[&str] = &[
    "if", "for", "while", "switch", "return", "catch", "else", "sizeof", "new", "delete", "throw", "do", "typeof", "await", "using",
    "foreach", "lock", "case", "default", "assert", "synchronized", "try", "defined", "function",
];

struct Scope {
    container: bool,
}

/// 抹掉注释和字符串/字符字面量内容 (保持列位置不变)
fn sanitize(line: &str, in_block: &mut bool, hash_comment: bool) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        if *in_block {
            if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                *in_block = false;
                out.push_str("  ");
                i += 2;
            } else {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            *in_block = true;
            out.push_str("  ");
            i += 2;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            break;
        }
        if hash_comment && c == '#' && out.trim().is_empty() {
            break; // C 预处理指令整行忽略
        }
        if c == '"' || c == '\'' || c == '`' {
            let q = c;
            out.push(q);
            i += 1;
            while i < chars.len() && chars[i] != q {
                if chars[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < chars.len() {
                    out.push(' ');
                    i += 1;
                }
            }
            if i < chars.len() {
                out.push(q);
                i += 1;
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// (词, 起点字符下标) — 词 = 标识符 (含 `::` 限定和前导 `~`)
fn words(s: &str) -> Vec<(String, usize)> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if is_ident_char(cs[i]) || cs[i] == '~' {
            let st = i;
            while i < cs.len() {
                if is_ident_char(cs[i]) || cs[i] == '~' {
                    i += 1;
                } else if cs[i] == ':' && cs.get(i + 1) == Some(&':') && cs.get(i + 2).map_or(false, |c| is_ident_char(*c) || *c == '~') {
                    i += 2;
                } else {
                    break;
                }
            }
            out.push((cs[st..i].iter().collect::<String>(), st));
        } else {
            i += 1;
        }
    }
    out
}

pub fn brace_outline(text: &str, lang: &str) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    let mut stack: Vec<Scope> = Vec::new();
    let mut pending = false;
    let c_like = lang == "C++";
    let clean: Vec<String> = {
        let mut b = false;
        text.lines().map(|l| sanitize(l, &mut b, c_like)).collect()
    };
    for (no, s) in clean.iter().enumerate() {
        let trimmed = s.trim();
        let depth = stack.iter().filter(|x| x.container).count() as i32;
        let only_containers = stack.iter().all(|x| x.container);
        let ws = words(s);
        let mut declared = false;

        // 容器: class Foo / struct Foo / namespace a::b / enum class Foo / interface IFoo
        if only_containers {
            for (k, (w, _)) in ws.iter().enumerate() {
                if CONTAINERS.contains(&w.as_str()) {
                    let mut j = k + 1;
                    if matches!(ws.get(j).map(|x| x.0.as_str()), Some("class") | Some("struct")) {
                        j += 1; // enum class X
                    }
                    if let Some((name, st)) = ws.get(j) {
                        let after: String = s.chars().skip(st + name.chars().count()).collect();
                        let fwd = after.trim_start().starts_with(';');
                        let typedef = k > 0 && ws[k - 1].0 == "typedef";
                        if !fwd && !typedef && !NOT_FUNC.contains(&name.as_str()) && name != "final" {
                            entries.push(OutlineEntry { name: name.clone(), kind: w.clone(), line: no as i32, depth });
                            pending = true;
                            declared = true;
                        }
                    }
                    break;
                }
            }
        }

        // 函数 / 方法
        let js_arrow = matches!(lang, "JavaScript" | "TypeScript") && (trimmed.contains("=>") || trimmed.contains("= function"));
        if !declared && only_containers && !trimmed.is_empty() && (!trimmed.ends_with(';') || js_arrow) {
            if let Some(entry) = detect_function(s, &ws, &clean, no, lang, depth, !stack.is_empty()) {
                entries.push(entry);
            }
        }

        // 括号栈
        for ch in s.chars() {
            match ch {
                '{' => {
                    let container = pending;
                    pending = false;
                    stack.push(Scope { container });
                }
                '}' => {
                    stack.pop();
                }
                ';' => {
                    if stack.iter().all(|x| x.container) {
                        pending = false;
                    }
                }
                _ => {}
            }
        }
    }
    entries
}

fn detect_function(s: &str, ws: &[(String, usize)], clean: &[String], no: usize, lang: &str, depth: i32, in_container: bool) -> Option<OutlineEntry> {
    let t = s.trim_start();
    // JS/TS: function name( / const name = (…) => / name = function
    if matches!(lang, "JavaScript" | "TypeScript") {
        let ww: Vec<&str> = ws.iter().map(|x| x.0.as_str()).collect();
        if let Some(p) = ww.iter().position(|w| *w == "function") {
            if let Some(n) = ww.get(p + 1) {
                if s.contains('(') {
                    return Some(OutlineEntry { name: (*n).to_string(), kind: "function".into(), line: no as i32, depth });
                }
            }
        }
        if matches!(ww.first().copied(), Some("const") | Some("let") | Some("var") | Some("export")) && (s.contains("=>") || s.contains("= function")) {
            let at = ww.iter().position(|w| matches!(*w, "const" | "let" | "var")).map(|p| p + 1)?;
            return Some(OutlineEntry { name: ww.get(at)?.to_string(), kind: "function".into(), line: no as i32, depth });
        }
    }
    // Go: func name( / func (r T) name(
    if lang == "Go" {
        if let Some(rest) = t.strip_prefix("func ") {
            let rest = rest.trim_start();
            let rest = if rest.starts_with('(') { rest.find(')').map(|i| rest[i + 1..].trim_start()).unwrap_or("") } else { rest };
            let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
            if !name.is_empty() && rest[name.len()..].trim_start().starts_with('(') {
                let kind = if t.starts_with("func (") { "method" } else { "function" };
                return Some(OutlineEntry { name, kind: kind.into(), line: no as i32, depth });
            }
        }
        return None;
    }
    // C 系: 第一个 '(' 前的标识符是函数名, 前面没有控制关键字, 且有函数体 ('{' 在本行 ')' 之后或下一行开头)
    let open_byte = s.find('(')?;
    let open = s[..open_byte].chars().count();
    let before = &s[..open_byte];
    if before.contains('=') || before.contains('.') || before.contains("->") {
        return None;
    }
    let name_tok = ws.iter().rev().find(|(_, st)| *st < open)?;
    let name = &name_tok.0;
    if NOT_FUNC.contains(&name.as_str()) || name.chars().next().map_or(true, |c| c.is_ascii_digit()) {
        return None;
    }
    if ws.iter().any(|(w, st)| *st < open && NOT_FUNC.contains(&w.as_str()) && w != name) {
        return None;
    }
    // 签名可能跨行: 把后续行并进来直到括号配平
    let mut joined = s.to_string();
    let mut last = no;
    while joined.matches('(').count() > joined.matches(')').count() && last + 1 < clean.len() && last - no < 8 {
        last += 1;
        joined.push(' ');
        joined.push_str(clean[last].trim());
    }
    let close = joined.rfind(')')?;
    let after = joined[close + 1..].trim();
    let has_body = after.contains('{') || {
        let mut j = last + 1;
        while j < clean.len() && clean[j].trim().is_empty() {
            j += 1;
        }
        clean.get(j).map_or(false, |l| l.trim_start().starts_with('{')) && !after.ends_with(',')
    };
    let tail_ok = after.is_empty()
        || after.starts_with('{')
        || after.starts_with(':')
        || after.starts_with("const")
        || after.starts_with("noexcept")
        || after.starts_with("override")
        || after.starts_with("final")
        || after.starts_with("throws")
        || after.starts_with("->")
        || after.starts_with("where");
    if !has_body || !tail_ok {
        return None;
    }
    let kind = if in_container { "method" } else { "function" };
    Some(OutlineEntry { name: name.clone(), kind: kind.into(), line: no as i32, depth })
}

pub fn markdown_outline(text: &str) -> Vec<OutlineEntry> {
    let mut out = Vec::new();
    let mut fence = false;
    for (no, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            let name = t[hashes..].trim().trim_end_matches('#').trim().to_string();
            if !name.is_empty() {
                out.push(OutlineEntry { name, kind: "section".into(), line: no as i32, depth: hashes as i32 - 1 });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[OutlineEntry]) -> Vec<(String, String, i32)> {
        v.iter().map(|e| (e.name.clone(), e.kind.clone(), e.depth)).collect()
    }
    fn t(n: &str, k: &str, d: i32) -> (String, String, i32) {
        (n.into(), k.into(), d)
    }

    #[test]
    fn cpp_classes_methods_and_namespaces_form_a_hierarchy() {
        let src = r#"
#include <x>
namespace app {
class Widget : public Base {
public:
    Widget(int a) : x(a) {}
    void draw() const {
        if (x) { helper(); }
        for (int i = 0; i < 3; i++) { call(i); }
    }
    int size();
private:
    int x;
};
static int free_fn(int a,
                   int b)
{
    return a + b; // not_a_fn(1)
}
struct Fwd;
}
int main(int argc, char** argv) { return 0; }
"#;
        let v = names(&brace_outline(src, "C++"));
        assert_eq!(
            v,
            vec![
                t("app", "namespace", 0),
                t("Widget", "class", 1),
                t("Widget", "method", 2),
                t("draw", "method", 2),
                t("free_fn", "method", 1),
                t("main", "function", 0),
            ],
            "got {v:?}"
        );
    }

    #[test]
    fn js_functions_classes_and_arrows() {
        let src = "function top(a) { return 1; }\nconst arrow = (x) => x + 1;\nclass Foo {\n  bar(y) {\n    if (y) { baz(); }\n  }\n}\n// function commented() {}\nexport default function main() {}\n";
        let v = names(&brace_outline(src, "JavaScript"));
        assert_eq!(
            v,
            vec![t("top", "function", 0), t("arrow", "function", 0), t("Foo", "class", 0), t("bar", "method", 1), t("main", "function", 0)],
            "got {v:?}"
        );
    }

    #[test]
    fn go_funcs_and_methods() {
        let src = "package p\ntype T struct { a int }\nfunc Plain(a int) int {\n\treturn a\n}\nfunc (t *T) Method() {\n}\n";
        let v = names(&brace_outline(src, "Go"));
        assert!(v.contains(&t("Plain", "function", 0)), "{v:?}");
        assert!(v.contains(&t("Method", "method", 0)), "{v:?}");
    }

    #[test]
    fn java_class_with_methods() {
        let src = "public class A {\n  private int f;\n  public void run(String s) {\n    System.out.println(s);\n  }\n  static class Inner {\n    int get() { return 1; }\n  }\n}\n";
        let v = names(&brace_outline(src, "Java"));
        assert_eq!(v, vec![t("A", "class", 0), t("run", "method", 1), t("Inner", "class", 1), t("get", "method", 2)], "got {v:?}");
    }

    #[test]
    fn markdown_headings_skip_code_fences() {
        let src = "# Title\ntext\n## Sub\n```\n# not heading\n```\n### Deep ###\n";
        let v = names(&markdown_outline(src));
        assert_eq!(v, vec![t("Title", "section", 0), t("Sub", "section", 1), t("Deep", "section", 2)]);
    }
}
