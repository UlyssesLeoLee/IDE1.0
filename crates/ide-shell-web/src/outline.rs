//! Outline service — read file + parse with sakura-rs TypeRegistry.
//!
//! 用 sakura-rs 的 `outline_fn` 字段做 outline (Python/Rust 已实现).
//! 其他语言 fallback 返回空 (后续可补 JS/Go/...).
//!
//! Output: Vec<OutlineEntry> { name, kind, line }

#![allow(dead_code)]

use sakura_rs::type_config::{OutlineEntry, TypeRegistry};
use serde::{Deserialize, Serialize};

/// 单文件 outline 结果.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineResult {
    pub path: String,
    pub language: String,
    pub entries: Vec<OutlineEntry>,
    pub error: Option<String>,
}

/// 解析一个文件的 outline.
pub fn outline_file(path: &str, content: &str) -> OutlineResult {
    let registry = TypeRegistry::default();
    let lang = match registry.detect(path) {
        Some(t) => t,
        None => {
            return OutlineResult {
                path: path.to_string(),
                language: "Plain".to_string(),
                entries: Vec::new(),
                error: None,
            };
        }
    };
    let entries = sakura_rs::type_config::TypeConfig::outline_dispatch(&lang.name, content);
    OutlineResult {
        path: path.to_string(),
        language: lang.name.to_string(),
        entries,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_rust_file() {
        let src = r#"
            fn main() { println!("hi"); }
            struct Foo { x: i32 }
            impl Foo {
                fn bar(&self) -> i32 { self.x }
            }
            trait Bar { fn baz(); }
        "#;
        let r = outline_file("test.rs", src);
        assert_eq!(r.language, "Rust");
        // 4 entries: main (fn), Foo (struct), bar (fn impl), Bar (trait)
        assert!(
            r.entries.len() >= 3,
            "expected >= 3 entries, got {:?}",
            r.entries
        );
        let names: Vec<&str> = r.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"Foo"));
    }

    #[test]
    fn outline_python_file() {
        let src = r#"
            def foo():
                pass

            class Baz:
                def method(self):
                    return 1
        "#;
        let r = outline_file("test.py", src);
        assert_eq!(r.language, "Python");
        let names: Vec<&str> = r.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"foo"), "expected foo in {:?}", names);
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn outline_unknown_ext_returns_empty() {
        let r = outline_file("foo.xyz", "anything");
        assert_eq!(r.language, "Plain");
        assert!(r.entries.is_empty());
    }
}
