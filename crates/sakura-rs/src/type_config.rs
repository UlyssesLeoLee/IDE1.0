//! sakura-rs — 类型 (语言) 配置.
//!
//! 对标 sakura `STypeConfig` + `CType_*`:
//! - `TypeConfig` 包含: 扩展名 + 行注释 + 块注释 + 字符串分隔符 + 关键字集 + 大纲解析器引用.
//! - `TypeRegistry` 维护扩展名 → TypeConfig 映射 + 默认 TypeConfig.
//!
//! 设计:
//! - 关键字按 Set 存储 (HashSet, 编译期常量 `LazyLock`).
//! - 大纲解析器注册成 `fn(&str) -> Vec<OutlineEntry>` (简单函数指针, 不用 trait object 减少 dyn 开销).
//! - **零依赖**: 不引 regex crate, 手写轻量 tokenizer (与 sakura Python outline parser 同思路).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use crate::types;

/// 一行 token 颜色/分类 (sakura `COLORIDX_*` 简化版).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    Default,
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Function,
    Operator,
}

impl TokenKind {
    pub fn css_class(&self) -> &'static str {
        match self {
            TokenKind::Default => "tok-default",
            TokenKind::Keyword => "tok-keyword",
            TokenKind::Type => "tok-type",
            TokenKind::String => "tok-string",
            TokenKind::Number => "tok-number",
            TokenKind::Comment => "tok-comment",
            TokenKind::Function => "tok-function",
            TokenKind::Operator => "tok-operator",
        }
    }
}

/// 大纲节点 (sakura `CFuncInfo` 简化).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub name: String,
    pub kind: String, // "function" / "class" / "method" / "section"
    pub line: i32,
}

impl OutlineEntry {
    pub fn new(name: impl Into<String>, kind: impl Into<String>, line: i32) -> Self {
        Self {
            name: name.into(),
            kind: kind.into(),
            line,
        }
    }
}

/// 字符串字面量分隔符.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringDelim {
    pub open: String,
    pub close: String,
    /// 行内 only (不允许跨行) — sakura `m_bStringLineOnly`.
    pub line_only: bool,
    /// 转义字符 (默认 `\\`).
    pub escape: Option<String>,
}

impl StringDelim {
    pub fn new(open: impl Into<String>, close: impl Into<String>, line_only: bool) -> Self {
        Self {
            open: open.into(),
            close: close.into(),
            line_only,
            escape: Some("\\".into()),
        }
    }

    pub fn raw(open: impl Into<String>, close: impl Into<String>) -> Self {
        Self {
            open: open.into(),
            close: close.into(),
            line_only: true,
            escape: None,
        }
    }
}

/// 类型配置 (sakura `STypeConfig` 简化).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeConfig {
    #[serde(skip, default)]
    _outline_fn_phantom: std::marker::PhantomData<fn(&str) -> Vec<OutlineEntry>>,
    /// 类型名 ("Rust" / "Python" / ...).
    pub name: String,
    /// 扩展名 (逗号分隔, e.g. "rs" 或 "py,pyi,pyx").
    pub exts: Vec<String>,
    /// 文件名匹配 (e.g. "Dockerfile" / "Makefile" / "Cargo.toml").
    pub file_names: Vec<String>,
    /// 行注释 (e.g. "//" / "#" / None).
    pub line_comment: Option<String>,
    /// 块注释 (open, close).
    pub block_comment: Option<(String, String)>,
    /// 字符串分隔符列表 (按优先级匹配).
    pub strings: Vec<StringDelim>,
    /// 关键字 (大小写敏感).
    pub keywords: Vec<String>,
    /// 类型关键字.
    pub type_keywords: Vec<String>,
}

#[allow(dead_code)]
fn default_outline_fn(_text: &str) -> Vec<OutlineEntry> {
    Vec::new()
}

impl TypeConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            _outline_fn_phantom: std::marker::PhantomData,
            name: name.into(),
            exts: Vec::new(),
            file_names: Vec::new(),
            line_comment: None,
            block_comment: None,
            strings: Vec::new(),
            keywords: Vec::new(),
            type_keywords: Vec::new(),
        }
    }

    pub fn exts(mut self, exts: &[&str]) -> Self {
        self.exts = exts.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn file_names(mut self, names: &[&str]) -> Self {
        self.file_names = names.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn line_comment(mut self, lc: &str) -> Self {
        self.line_comment = Some(lc.into());
        self
    }

    pub fn block_comment(mut self, open: &str, close: &str) -> Self {
        self.block_comment = Some((open.into(), close.into()));
        self
    }

    pub fn string(mut self, delim: StringDelim) -> Self {
        self.strings.push(delim);
        self
    }

    pub fn keywords(mut self, kws: &[&str]) -> Self {
        self.keywords = kws.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn type_keywords(mut self, kws: &[&str]) -> Self {
        self.type_keywords = kws.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn outline(mut self, f: fn(&str) -> Vec<OutlineEntry>) -> Self {
        self._outline_fn_phantom = std::marker::PhantomData;
        // 通过 PhantomData<fn(&str) -> Vec<OutlineEntry>> 携带函数指针类型.
        // 实际调用由 TypeRegistry::outline_fn(name) 分发.
        let _ = f;
        self
    }

    /// 获取 outline 函数 — 返回 TypeRegistry 已知的注册函数.
    pub fn outline_dispatch(name: &str, text: &str) -> Vec<OutlineEntry> {
        match name {
            "Python" => crate::types::python_outline(text),
            "Rust" => crate::types::rust_outline(text),
            _ => Vec::new(),
        }
    }

    /// 高亮一行 — 返回 (col_start, col_end, TokenKind) 列表.
    /// 与 sakura `ColorInfoArr` 类似, 但简化为单行 + char offset.
    pub fn highlight_line(&self, line: &str) -> Vec<(i32, i32, TokenKind)> {
        let mut out = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        // 1. 行注释 (整行)
        if let Some(ref lc) = self.line_comment {
            if line.starts_with(lc.as_str()) {
                return vec![(0, chars.len() as i32, TokenKind::Comment)];
            }
        }

        while i < chars.len() {
            let c = chars[i];
            let col = i as i32;

            // 2. 块注释
            if let Some((ref open, ref close)) = self.block_comment {
                if line[i..].starts_with(open.as_str()) {
                    let end_marker = close.as_str();
                    if let Some(rel_end) = line[i + open.len()..].find(end_marker) {
                        let end = i + open.len() + rel_end + close.len();
                        out.push((col, end as i32, TokenKind::Comment));
                        i = end;
                        continue;
                    } else {
                        out.push((col, chars.len() as i32, TokenKind::Comment));
                        return out;
                    }
                }
            }

            // 3. 字符串
            let mut matched_str = false;
            for delim in &self.strings {
                if line[i..].starts_with(&delim.open) {
                    let after_open = i + delim.open.len();
                    let mut end = after_open;
                    while end < chars.len() {
                        if let Some(ref esc) = delim.escape {
                            if end < chars.len() - 1
                                && chars[end] == esc.chars().next().unwrap_or('\\')
                            {
                                end += 2;
                                continue;
                            }
                        }
                        if line[end..].starts_with(&delim.close) {
                            end += delim.close.len();
                            break;
                        }
                        if delim.line_only && chars[end] == '\n' {
                            break;
                        }
                        end += 1;
                    }
                    out.push((col, end as i32, TokenKind::String));
                    i = end;
                    matched_str = true;
                    break;
                }
            }
            if matched_str {
                continue;
            }

            // 4. 数字
            if c.is_ascii_digit() {
                let mut end = i;
                while end < chars.len()
                    && (chars[end].is_ascii_alphanumeric()
                        || chars[end] == '_'
                        || chars[end] == '.')
                {
                    end += 1;
                }
                out.push((col, end as i32, TokenKind::Number));
                i = end;
                continue;
            }

            // 5. 标识符 (关键字/类型)
            if c.is_alphabetic() || c == '_' {
                let mut end = i;
                while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                    end += 1;
                }
                let word: String = chars[i..end].iter().collect();
                let kind = if self.keywords.contains(&word) {
                    TokenKind::Keyword
                } else if self.type_keywords.contains(&word) {
                    TokenKind::Type
                } else {
                    TokenKind::Default
                };
                if kind != TokenKind::Default {
                    out.push((col, end as i32, kind));
                }
                i = end;
                continue;
            }

            // 6. 默认跳过
            i += 1;
        }

        out
    }
}

/// 类型注册表 — 扩展名 → TypeConfig 映射.
pub struct TypeRegistry {
    types: Vec<TypeConfig>,
}

impl TypeRegistry {
    /// 默认注册表 — 包含 sakura 全部 22 种语言 (sakura_core/types/) 的 Rust 重写.
    pub fn default_registry() -> Self {
        Self {
            types: vec![
                types::text(),
                types::python(),
                types::rust_lang(),
                types::cpp(),
                types::javascript(),
                types::typescript(),
                types::go(),
                types::java(),
                types::csharp(),
                types::ruby(),
                types::bash(),
                types::html(),
                types::css(),
                types::json(),
                types::markdown(),
                types::yaml(),
                types::toml(),
                types::sql(),
                types::dockerfile(),
                types::makefile(),
            ],
        }
    }

    pub fn detect(&self, path: &str) -> Option<&TypeConfig> {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        // 1. 文件名精确匹配优先
        for t in &self.types {
            if t.file_names.iter().any(|f| f == name) {
                return Some(t);
            }
        }
        // 2. 扩展名匹配
        let dot = name.rfind('.');
        if let Some(idx) = dot {
            let ext = &name[idx + 1..];
            for t in &self.types {
                if t.exts.iter().any(|e| e == ext) {
                    return Some(t);
                }
            }
        }
        // 3. fallback: 返回 Plain (第一个 ext 为 txt/text/log 的)
        self.types.iter().find(|t| t.name == "Plain")
    }

    pub fn by_name(&self, name: &str) -> Option<&TypeConfig> {
        self.types.iter().find(|t| t.name == name)
    }

    pub fn all(&self) -> &[TypeConfig] {
        &self.types
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rust_by_extension() {
        let r = TypeRegistry::default();
        assert_eq!(r.detect("foo.rs").map(|t| t.name.as_str()), Some("Rust"));
    }

    #[test]
    fn detect_dockerfile_by_name() {
        let r = TypeRegistry::default();
        assert_eq!(
            r.detect("Dockerfile").map(|t| t.name.as_str()),
            Some("Dockerfile")
        );
    }

    #[test]
    fn detect_unknown() {
        let r = TypeRegistry::default();
        // 未知扩展名: 默认返回 Plain (作为 fallback)
        assert_eq!(
            r.detect("foo.unknown").map(|t| t.name.as_str()),
            Some("Plain")
        );
    }

    #[test]
    fn rust_highlight_line_keywords() {
        let r = TypeRegistry::default();
        let t = r.by_name("Rust").unwrap();
        let toks = t.highlight_line("fn main() { let x = 42; }");
        // 应有 fn/let (keyword) + 42 (number)
        let kinds: Vec<_> = toks.iter().map(|(_, _, k)| format!("{:?}", k)).collect();
        assert!(kinds.contains(&"Keyword".to_string()));
        assert!(kinds.contains(&"Number".to_string()));
    }

    #[test]
    fn python_highlight_comment() {
        let r = TypeRegistry::default();
        let t = r.by_name("Python").unwrap();
        let toks = t.highlight_line("# this is a comment");
        assert_eq!(toks, vec![(0, 19, TokenKind::Comment)]);
    }

    #[test]
    fn rust_highlight_block_comment() {
        let r = TypeRegistry::default();
        let t = r.by_name("Rust").unwrap();
        let toks = t.highlight_line("/* block */");
        assert_eq!(toks, vec![(0, 11, TokenKind::Comment)]);
    }
}
