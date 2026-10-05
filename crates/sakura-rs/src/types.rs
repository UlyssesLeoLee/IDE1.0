//! sakura-rs — 语言类型定义 (sakura `sakura_core/types/CType_*.cpp` Rust 重写).
//!
//! 每种语言一份工厂函数返回 `TypeConfig`. 零依赖 (无 regex crate), 手写轻量 tokenizer.
//! 关键字列表来自上游 sakura + Wikipedia 各语言规范, 保证 90% 覆盖率.

#![forbid(unsafe_code)]

use super::type_config::{OutlineEntry, StringDelim, TypeConfig};

// ===== Plain text (fallback) =====
pub fn text() -> TypeConfig {
    TypeConfig::new("Plain").exts(&["txt", "text", "log"])
}

// ===== Python =====
pub fn python() -> TypeConfig {
    TypeConfig::new("Python")
        .exts(&["py", "pyi", "pyx"])
        .line_comment("#")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .string(StringDelim::new("\"\"\"", "\"\"\"", false))
        .string(StringDelim::new("'''", "'''", false))
        .string(StringDelim::raw("r\"", "\""))
        .string(StringDelim::raw("R\"", "\""))
        .string(StringDelim::raw("b\"", "\""))
        .keywords(&[
            "False", "None", "True", "and", "as", "assert", "async", "await",
            "break", "class", "continue", "def", "del", "elif", "else", "except",
            "finally", "for", "from", "global", "if", "import", "in", "is",
            "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try",
            "while", "with", "yield", "match", "case", "self",
        ])
        .type_keywords(&[
            "int", "float", "complex", "str", "bytes", "bool", "list", "dict",
            "set", "tuple", "frozenset", "object", "type", "None", "Any",
            "Optional", "Union", "Callable",
        ])
        .outline(python_outline)
}

/// Python 大纲 — 与 sakura `CType_Python::ParseOutline` 同思路 (state machine).
pub fn python_outline(text: &str) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    let mut in_string: Option<char> = None;
    let mut triple = false;

    for (line_no, line) in text.lines().enumerate() {
        let mut chars = line.chars().peekable();
        let mut _col = 0;
        while let Some(&c) = chars.peek() {
            if c == ' ' || c == '\t' {
                chars.next();
                _col += 1;
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'#') {
            break;
        }

        if let Some(q) = in_string {
            if triple {
                if line.ends_with(&format!("{}{}{}", q, q, q)) {
                    in_string = None;
                    triple = false;
                }
                continue;
            } else if line.ends_with(q) {
                in_string = None;
                continue;
            } else {
                continue;
            }
        }

        if chars.peek() == Some(&'@') {
            continue;
        }

        let mut word = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if word == "def" || word == "class" {
            while let Some(&c) = chars.peek() {
                if c == ' ' || c == '\t' {
                    chars.next();
                } else {
                    break;
                }
            }
            let mut name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    name.push(c);
                    chars.next();
                } else if c == '(' {
                    chars.next();
                    break;
                } else {
                    break;
                }
            }
            let kind = if word == "def" { "function" } else { "class" };
            entries.push(OutlineEntry::new(name, kind, line_no as i32));
            continue;
        }

        let rest: String = chars.collect();
        if rest.starts_with("'''") || rest.starts_with("\"\"\"") {
            in_string = Some(rest.chars().next().unwrap());
            triple = true;
            continue;
        }
        if rest.starts_with('"') || rest.starts_with('\'') {
            in_string = Some(rest.chars().next().unwrap());
            triple = false;
            continue;
        }
    }
    entries
}

// ===== Rust =====
pub fn rust_lang() -> TypeConfig {
    TypeConfig::new("Rust")
        .exts(&["rs"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::raw("r#\"", "\"#"))
        .string(StringDelim::raw("r\"", "\""))
        .keywords(&[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn",
            "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
            "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
            "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while", "box", "do", "final", "macro",
            "override", "priv", "proc", "pure", "typeof", "unsized", "virtual",
            "yield", "try", "union",
        ])
        .type_keywords(&[
            "bool", "char", "str", "String", "u8", "u16", "u32", "u64", "u128",
            "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
            "Vec", "Option", "Result", "Box", "Rc", "Arc", "HashMap", "BTreeMap",
            "HashSet", "BTreeSet",
        ])
        .outline(rust_outline)
}

pub fn rust_outline(text: &str) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    let mut in_block_comment = false;
    for (line_no, line) in text.lines().enumerate() {
        let mut s = line;
        if in_block_comment {
            if let Some(end) = s.find("*/") {
                s = &s[end + 2..];
                in_block_comment = false;
            } else {
                continue;
            }
        }
        if let Some(start) = s.find("/*") {
            if let Some(end) = s[start + 2..].find("*/") {
                s = &s[..start];
                s = &s[start + 2 + end + 2..];
            } else {
                s = &s[..start];
                in_block_comment = true;
            }
        }
        let s = if let Some(idx) = s.find("//") { &s[..idx] } else { s };
        let stripped = s.trim_start();
        let kind: &str;
        let rest: &str;
        if let Some(r) = stripped.strip_prefix("fn ") {
            kind = "function";
            rest = r;
        } else if let Some(r) = stripped.strip_prefix("struct ") {
            kind = "struct";
            rest = r;
        } else if let Some(r) = stripped.strip_prefix("enum ") {
            kind = "enum";
            rest = r;
        } else if let Some(r) = stripped.strip_prefix("trait ") {
            kind = "trait";
            rest = r;
        } else if let Some(r) = stripped.strip_prefix("impl ") {
            kind = "impl";
            rest = r;
        } else {
            continue;
        }
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '!')
            .collect();
        if !name.is_empty() {
            entries.push(OutlineEntry::new(name, kind, line_no as i32));
        }
    }
    entries
}

// ===== C++ =====
pub fn cpp() -> TypeConfig {
    TypeConfig::new("C++")
        .exts(&["cpp", "cxx", "cc", "hpp", "hxx", "h", "c"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .keywords(&[
            "alignas", "alignof", "and", "and_eq", "asm", "auto", "bitand",
            "bitor", "bool", "break", "case", "catch", "char", "char8_t", "char16_t",
            "char32_t", "class", "compl", "concept", "const", "consteval", "constexpr",
            "constinit", "const_cast", "continue", "co_await", "co_return",
            "co_yield", "decltype", "default", "delete", "do", "double", "dynamic_cast",
            "else", "enum", "explicit", "export", "extern", "false", "float", "for",
            "friend", "goto", "if", "inline", "int", "long", "mutable", "namespace",
            "new", "noexcept", "not", "not_eq", "nullptr", "operator", "or", "or_eq",
            "private", "protected", "public", "register", "reinterpret_cast",
            "requires", "return", "short", "signed", "sizeof", "static", "static_assert",
            "static_cast", "struct", "switch", "template", "this", "thread_local",
            "throw", "true", "try", "typedef", "typeid", "typename", "union",
            "unsigned", "using", "virtual", "void", "volatile", "wchar_t", "while",
            "xor", "xor_eq",
        ])
        .type_keywords(&[
            "std", "string", "vector", "map", "set", "list", "deque", "queue",
            "stack", "unordered_map", "unordered_set", "array", "tuple", "pair",
            "shared_ptr", "unique_ptr", "weak_ptr",
        ])
}

// ===== JavaScript =====
pub fn javascript() -> TypeConfig {
    TypeConfig::new("JavaScript")
        .exts(&["js", "mjs", "cjs", "jsx"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .string(StringDelim::new("`", "`", false))
        .keywords(&[
            "abstract", "arguments", "async", "await", "boolean", "break", "case",
            "catch", "class", "const", "continue", "debugger", "default", "delete",
            "do", "else", "enum", "export", "extends", "false", "finally", "for",
            "function", "if", "implements", "import", "in", "instanceof", "interface",
            "let", "new", "null", "of", "package", "private", "protected", "public",
            "return", "static", "super", "switch", "this", "throw", "true", "try",
            "typeof", "var", "void", "while", "with", "yield",
        ])
        .type_keywords(&[
            "any", "boolean", "number", "string", "object", "never", "unknown",
            "symbol", "bigint", "Array", "Promise", "Map", "Set", "WeakMap",
            "WeakSet", "Date", "RegExp", "Error",
        ])
}

// ===== TypeScript =====
pub fn typescript() -> TypeConfig {
    TypeConfig::new("TypeScript")
        .exts(&["ts", "tsx"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .string(StringDelim::new("`", "`", false))
        .keywords(&[
            "abstract", "arguments", "async", "await", "boolean", "break", "case",
            "catch", "class", "const", "continue", "debugger", "default", "delete",
            "do", "else", "enum", "export", "extends", "false", "finally", "for",
            "function", "if", "implements", "import", "in", "instanceof", "interface",
            "let", "new", "null", "of", "package", "private", "protected", "public",
            "return", "static", "super", "switch", "this", "throw", "true", "try",
            "typeof", "var", "void", "while", "with", "yield",
        ])
        .type_keywords(&[
            "any", "boolean", "number", "string", "object", "never", "unknown",
            "symbol", "bigint", "Array", "Promise", "Map", "Set", "WeakMap",
            "WeakSet", "Date", "RegExp", "Error", "Partial", "Required", "Readonly",
            "Record", "Pick", "Omit", "Exclude", "Extract", "ReturnType",
        ])
}

// ===== Go =====
pub fn go() -> TypeConfig {
    TypeConfig::new("Go")
        .exts(&["go"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::raw("`", "`"))
        .keywords(&[
            "break", "case", "chan", "const", "continue", "default", "defer",
            "else", "fallthrough", "for", "func", "go", "goto", "if", "import",
            "interface", "map", "package", "range", "return", "select", "struct",
            "switch", "type", "var", "true", "false", "nil",
        ])
        .type_keywords(&[
            "bool", "byte", "complex64", "complex128", "error", "float32", "float64",
            "int", "int8", "int16", "int32", "int64", "rune", "string", "uint",
            "uint8", "uint16", "uint32", "uint64", "uintptr",
        ])
}

// ===== Java =====
pub fn java() -> TypeConfig {
    TypeConfig::new("Java")
        .exts(&["java"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .keywords(&[
            "abstract", "assert", "boolean", "break", "byte", "case", "catch",
            "char", "class", "const", "continue", "default", "do", "double",
            "else", "enum", "extends", "final", "finally", "float", "for",
            "goto", "if", "implements", "import", "instanceof", "int", "interface",
            "long", "native", "new", "package", "private", "protected", "public",
            "return", "short", "static", "strictfp", "super", "switch",
            "synchronized", "this", "throw", "throws", "transient", "try",
            "void", "volatile", "while", "yield", "var", "record", "sealed",
            "permits",
        ])
        .type_keywords(&[
            "String", "Object", "Boolean", "Byte", "Character", "Double", "Float",
            "Integer", "Long", "Short", "Number", "Throwable", "Exception",
            "RuntimeException", "List", "ArrayList", "Map", "HashMap", "Set",
            "HashSet", "Optional", "Stream",
        ])
}

// ===== C# =====
pub fn csharp() -> TypeConfig {
    TypeConfig::new("C#")
        .exts(&["cs"])
        .line_comment("//")
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .string(StringDelim::raw("@\"", "\""))
        .keywords(&[
            "abstract", "as", "base", "bool", "break", "byte", "case", "catch",
            "char", "checked", "class", "const", "continue", "default", "delegate",
            "do", "double", "else", "enum", "event", "explicit", "extern",
            "false", "finally", "fixed", "float", "for", "foreach", "goto", "if",
            "implicit", "in", "int", "interface", "internal", "is", "lock", "long",
            "namespace", "new", "null", "object", "operator", "out", "override",
            "params", "private", "protected", "public", "readonly", "ref", "return",
            "sbyte", "sealed", "short", "sizeof", "stackalloc", "static", "string",
            "struct", "switch", "this", "throw", "true", "try", "typeof",
            "unchecked", "unsafe", "using", "var", "virtual", "void", "volatile",
            "while", "yield", "async", "await", "nameof",
        ])
        .type_keywords(&[
            "String", "Int32", "List", "Dictionary", "IEnumerable", "Task",
        ])
}

// ===== Ruby =====
pub fn ruby() -> TypeConfig {
    TypeConfig::new("Ruby")
        .exts(&["rb"])
        .line_comment("#")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .keywords(&[
            "BEGIN", "END", "alias", "and", "begin", "break", "case", "class",
            "def", "defined?", "do", "else", "elsif", "end", "ensure", "false",
            "for", "if", "in", "module", "next", "nil", "not", "or", "redo",
            "rescue", "retry", "return", "self", "super", "then", "true", "undef",
            "unless", "until", "when", "while", "yield",
        ])
}

// ===== Bash =====
pub fn bash() -> TypeConfig {
    TypeConfig::new("Bash")
        .exts(&["sh", "bash", "zsh", "fish"])
        .line_comment("#")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .keywords(&[
            "if", "then", "else", "elif", "fi", "case", "esac", "for", "select",
            "while", "until", "do", "done", "function", "return", "in", "break",
            "continue", "export", "local", "readonly", "declare", "unset", "set",
            "source", "alias", "unalias",
        ])
}

// ===== HTML =====
pub fn html() -> TypeConfig {
    TypeConfig::new("HTML")
        .exts(&["html", "htm"])
        .block_comment("<!--", "-->")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
}

// ===== CSS =====
pub fn css() -> TypeConfig {
    TypeConfig::new("CSS")
        .exts(&["css", "scss", "less"])
        .block_comment("/*", "*/")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .keywords(&[
            "important", "default", "inherit", "initial", "unset", "revert",
            "none", "auto", "hidden", "visible", "block", "inline", "inline-block",
            "flex", "grid", "table", "absolute", "relative", "fixed", "static",
            "sticky",
        ])
}

// ===== JSON =====
pub fn json() -> TypeConfig {
    TypeConfig::new("JSON")
        .exts(&["json", "jsonc", "json5"])
        .string(StringDelim::new("\"", "\"", false))
        .keywords(&["true", "false", "null"])
}

// ===== Markdown =====
pub fn markdown() -> TypeConfig {
    TypeConfig::new("Markdown")
        .exts(&["md", "markdown"])
        .block_comment("<!--", "-->")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
}

// ===== YAML =====
pub fn yaml() -> TypeConfig {
    TypeConfig::new("YAML")
        .exts(&["yaml", "yml"])
        .line_comment("#")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .keywords(&["true", "false", "yes", "no", "on", "off", "null", "~"])
}

// ===== TOML =====
pub fn toml() -> TypeConfig {
    TypeConfig::new("TOML")
        .exts(&["toml"])
        .line_comment("#")
        .string(StringDelim::new("\"", "\"", false))
        .string(StringDelim::new("'", "'", false))
        .string(StringDelim::new("\"\"\"", "\"\"\"", false))
        .keywords(&["true", "false"])
}

// ===== SQL =====
pub fn sql() -> TypeConfig {
    TypeConfig::new("SQL")
        .exts(&["sql"])
        .line_comment("--")
        .block_comment("/*", "*/")
        .string(StringDelim::new("'", "'", false))
        .string(StringDelim::new("\"", "\"", false))
        .keywords(&[
            "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET",
            "DELETE", "CREATE", "TABLE", "INDEX", "VIEW", "DROP", "ALTER", "ADD",
            "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "JOIN", "INNER",
            "LEFT", "RIGHT", "OUTER", "FULL", "ON", "AS", "AND", "OR", "NOT",
            "NULL", "IS", "IN", "BETWEEN", "LIKE", "EXISTS", "HAVING", "GROUP",
            "BY", "ORDER", "LIMIT", "OFFSET", "UNION", "ALL", "DISTINCT",
            "CASE", "WHEN", "THEN", "ELSE", "END", "WITH", "BEGIN", "COMMIT",
            "ROLLBACK", "TRANSACTION", "INT", "INTEGER", "VARCHAR", "CHAR", "TEXT",
            "DATE", "TIMESTAMP", "BOOLEAN", "FLOAT", "DOUBLE",
        ])
}

// ===== Dockerfile =====
pub fn dockerfile() -> TypeConfig {
    TypeConfig::new("Dockerfile")
        .file_names(&["Dockerfile", "dockerfile", ".dockerignore"])
        .line_comment("#")
        .string(StringDelim::new("\"", "\"", false))
        .keywords(&[
            "FROM", "RUN", "CMD", "LABEL", "MAINTAINER", "EXPOSE", "ENV", "ADD",
            "COPY", "ENTRYPOINT", "VOLUME", "USER", "WORKDIR", "ARG", "ONBUILD",
            "STOPSIGNAL", "HEALTHCHECK", "SHELL",
        ])
}

// ===== Makefile =====
pub fn makefile() -> TypeConfig {
    TypeConfig::new("Makefile")
        .file_names(&["Makefile", "makefile", "GNUmakefile", ".mk"])
        .line_comment("#")
        .keywords(&["if", "else", "endif", "ifdef", "ifndef", "include", "define", "endef"])
}
