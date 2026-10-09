// tag_jump.rs — ダイレクトタグジャンプ (sakura 8.4)
//
// 関数/タグ定義位置へジャンプ.
// 対応: `func()`, `def func()`, `function func()`, `class Name`, `struct Name` 等.

use std::path::PathBuf;

/// タグ種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Module,
    Variable,
}

/// タグ情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub kind: TagKind,
    pub file: PathBuf,
    pub line: usize, // 1-indexed
}

/// タグ DB
#[derive(Debug, Default)]
pub struct TagDb {
    tags: Vec<Tag>,
}

impl TagDb {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, tag: Tag) {
        self.tags.push(tag);
    }

    /// 名前でタグ検索
    pub fn find(&self, name: &str) -> Vec<&Tag> {
        self.tags.iter().filter(|t| t.name == name).collect()
    }

    /// 種別でフィルタ
    pub fn find_by_kind(&self, name: &str, kind: TagKind) -> Vec<&Tag> {
        self.tags
            .iter()
            .filter(|t| t.name == name && t.kind == kind)
            .collect()
    }

    /// 件数
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }
}

/// 簡易パーサ — 1 ファイルからタグ抽出
/// 対応パターン (basic):
/// - `fn name(`     (Rust)
/// - `def name(`    (Python)
/// - `function name(` (JS)
/// - `class Name`   (Python, JS)
/// - `struct Name`  (Rust, C)
/// - `enum Name`    (Rust, C)
pub fn extract_tags(content: &str, file: &PathBuf) -> Vec<Tag> {
    let mut tags = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        // Rust: fn name
        if let Some(rest) = strip_prefix(trimmed, "fn ") {
            if let Some(name) = extract_ident_before(rest, "(") {
                tags.push(Tag { name, kind: TagKind::Function, file: file.clone(), line: idx + 1 });
                continue;
            }
        }
        // Python: def name
        if let Some(rest) = strip_prefix(trimmed, "def ") {
            if let Some(name) = extract_ident_before(rest, "(") {
                tags.push(Tag { name, kind: TagKind::Function, file: file.clone(), line: idx + 1 });
                continue;
            }
        }
        // JS: function name
        if let Some(rest) = strip_prefix(trimmed, "function ") {
            if let Some(name) = extract_ident_before(rest, "(") {
                tags.push(Tag { name, kind: TagKind::Function, file: file.clone(), line: idx + 1 });
                continue;
            }
        }
        // class Name
        if let Some(rest) = strip_prefix(trimmed, "class ") {
            if let Some(name) = take_ident(rest) {
                tags.push(Tag { name, kind: TagKind::Class, file: file.clone(), line: idx + 1 });
                continue;
            }
        }
        // struct Name
        if let Some(rest) = strip_prefix(trimmed, "struct ") {
            if let Some(name) = take_ident(rest) {
                tags.push(Tag { name, kind: TagKind::Struct, file: file.clone(), line: idx + 1 });
                continue;
            }
        }
        // enum Name
        if let Some(rest) = strip_prefix(trimmed, "enum ") {
            if let Some(name) = take_ident(rest) {
                tags.push(Tag { name, kind: TagKind::Enum, file: file.clone(), line: idx + 1 });
                continue;
            }
        }
    }
    tags
}

fn strip_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.strip_prefix(prefix)
}

fn take_ident(s: &str) -> Option<String> {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_alphanumeric() || c == '_' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end > 0 {
        Some(s[..end].to_string())
    } else {
        None
    }
}

fn extract_ident_before(s: &str, marker: &str) -> Option<String> {
    let pos = s.find(marker)?;
    take_ident(&s[..pos])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_fn() {
        let code = "fn main() {}\nfn helper() {}";
        let tags = extract_tags(code, &PathBuf::from("test.rs"));
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "main");
        assert_eq!(tags[0].kind, TagKind::Function);
    }

    #[test]
    fn test_python_def() {
        let code = "def foo():\n    pass\ndef bar(x):";
        let tags = extract_tags(code, &PathBuf::from("test.py"));
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "foo");
    }

    #[test]
    fn test_class() {
        let code = "class MyClass:\n    pass";
        let tags = extract_tags(code, &PathBuf::from("test.py"));
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].kind, TagKind::Class);
    }

    #[test]
    fn test_struct_enum() {
        let code = "struct Foo {}\nenum Bar { A, B }";
        let tags = extract_tags(code, &PathBuf::from("test.rs"));
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].kind, TagKind::Struct);
        assert_eq!(tags[1].kind, TagKind::Enum);
    }

    #[test]
    fn test_db_search() {
        let mut db = TagDb::new();
        db.add(Tag { name: "main".into(), kind: TagKind::Function, file: PathBuf::from("a.rs"), line: 1 });
        db.add(Tag { name: "main".into(), kind: TagKind::Function, file: PathBuf::from("b.rs"), line: 5 });
        let found = db.find("main");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_jump() {
        let code = "fn target_function() {}\nfn other() {}";
        let tags = extract_tags(code, &PathBuf::from("test.rs"));
        let target = tags.iter().find(|t| t.name == "target_function").unwrap();
        assert_eq!(target.line, 1);
    }
}
