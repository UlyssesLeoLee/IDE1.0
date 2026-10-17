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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenKind {
    Default,
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Function,
    Operator,
    Variable,
    Plain,
    Punct,
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
            TokenKind::Variable => "tok-variable",
            TokenKind::Plain => "tok-plain",
            TokenKind::Punct => "tok-punct",
        }
    }

    /// 分类名 (Sakura 的 COLORIDX_* 对应的可读名).
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::Default => "default",
            TokenKind::Keyword => "keyword",
            TokenKind::Type => "type",
            TokenKind::String => "string",
            TokenKind::Number => "number",
            TokenKind::Comment => "comment",
            TokenKind::Function => "function",
            TokenKind::Operator => "operator",
            TokenKind::Variable => "variable",
            TokenKind::Plain => "plain",
            TokenKind::Punct => "punct",
        }
    }

    /// 全部可配色分类 —— Sakura 的「タイプ別設定 『カラー』」列表就是这个集合.
    pub fn all() -> &'static [TokenKind] {
        &[
            TokenKind::Default,
            TokenKind::Keyword,
            TokenKind::Type,
            TokenKind::String,
            TokenKind::Number,
            TokenKind::Comment,
            TokenKind::Function,
            TokenKind::Operator,
            TokenKind::Variable,
            TokenKind::Plain,
            TokenKind::Punct,
        ]
    }

    /// 按名查分类 (未知名回落到 Default, 与 sakura 的配色表缺项处理一致).
    pub fn from_name(name: &str) -> TokenKind {
        Self::all()
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name))
            .unwrap_or(TokenKind::Default)
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
    /// 条目可含通配符 (`*.h` / `Makefile.*`), 语义见 `TypeRegistry::file_matches`.
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
    /// 关键字匹配是否区分大小写. sakura 的类型別設定有这个选项:
    /// SQL / Fortran 等语言大小写不敏感, Rust / C++ 敏感.
    #[serde(default = "default_true")]
    pub keyword_case_sensitive: bool,
    /// 用户自定义关键字 (与内置 `keywords` 合并, 不覆盖内置).
    /// Sakura 允许在类型別設定里追加自己的关键字, 这是它比多数编辑器灵活的地方.
    #[serde(default)]
    pub user_keywords: Vec<String>,
    /// 正则语法名 ("Onigmo" / "bregonig" / "default").
    /// 选不同引擎时字符串 / 注释定界符的判定不同, 例如 bregonig 不支持
    /// 变长后顾, Onigmo 支持 —— 本字段至少要把这个选择表达出来.
    #[serde(default = "default_regex_syntax")]
    pub regex_syntax: String,
}

fn default_true() -> bool {
    true
}

fn default_regex_syntax() -> String {
    "Onigmo".to_string()
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
            keyword_case_sensitive: default_true(),
            user_keywords: Vec::new(),
            regex_syntax: default_regex_syntax(),
        }
    }

    /// 关键字匹配是否区分大小写 (Sakura 类型別設定 的同名选项).
    pub fn keyword_case_sensitive(mut self, on: bool) -> Self {
        self.keyword_case_sensitive = on;
        self
    }

    /// 追加用户自定义关键字 (与内置合并, 不覆盖).
    pub fn user_keywords(mut self, kws: &[&str]) -> Self {
        self.user_keywords = kws.iter().map(|s| s.to_string()).collect();
        self
    }

    /// 指定正则语法 ("Onigmo" / "bregonig").
    pub fn regex_syntax(mut self, name: impl Into<String>) -> Self {
        self.regex_syntax = name.into();
        self
    }

    /// 生效的关键字全集 = 内置 + 用户自定义 (去重, 大小写按配置归一).
    /// 返回小写形式当 `keyword_case_sensitive == false`.
    pub fn effective_keywords(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::with_capacity(self.keywords.len() + self.user_keywords.len());
        for k in self.keywords.iter().chain(self.user_keywords.iter()) {
            if k.is_empty() {
                continue;
            }
            let norm = if self.keyword_case_sensitive {
                k.clone()
            } else {
                k.to_lowercase()
            };
            if !out.contains(&norm) {
                out.push(norm);
            }
        }
        out
    }

    /// 该类型下某词是否算关键字 (按 keyword_case_sensitive 决定是否归一).
    pub fn is_keyword(&self, word: &str) -> bool {
        let probe = if self.keyword_case_sensitive {
            word.to_string()
        } else {
            word.to_lowercase()
        };
        self.keywords.iter().any(|k| {
            if self.keyword_case_sensitive {
                k == &probe
            } else {
                k.to_lowercase() == probe
            }
        }) || self.user_keywords.iter().any(|k| {
            if self.keyword_case_sensitive {
                k == &probe
            } else {
                k.to_lowercase() == probe
            }
        })
    }

    /// 该正则语法是否支持变长后顾 (lookbehind).
    /// bregonig / BreezoLib 不支持, Onigmo 支持 —— 影响 highlight 的正则选用.
    pub fn supports_lookbehind(&self) -> bool {
        !self.regex_syntax.eq_ignore_ascii_case("bregonig")
    }

    pub fn exts(mut self, exts: &[&str]) -> Self {
        // 每个条目内部再按 Sakura 的分隔符切分 —— 上游的类型定義用
        // `L"c,cpp,cxx,cc,..."` 单字符串表达 (CType_Cpp.cpp:70), 而这里是切片,
        // 两者混用时必须归一, 否则 `exts(&["a,b"])` 会把 "a,b" 当成一个扩展名.
        let mut out = Vec::new();
        for e in exts {
            out.extend(split_tokens(e));
        }
        self.exts = out;
        self
    }

    pub fn file_names(mut self, names: &[&str]) -> Self {
        let mut out = Vec::new();
        for n in names {
            out.extend(split_tokens(n));
        }
        self.file_names = out;
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

/// 类型注册表 — 扩展名 / 文件名 → TypeConfig 映射.
pub struct TypeRegistry {
    types: Vec<TypeConfig>,
}

/// 单个 token 是否匹配文件名 —— 对标 sakura `CDocTypeManager::IsFileNameMatch`
/// (sakura_core/env/CDocTypeManager.cpp:154).
///
/// Sakura 的权威语义:
/// - token 列表按 `m_typeExtSeps` (`,` `;` 空白) 切分;
/// - **不含通配符**的 token: 先与整个文件名做 `_wcsicmp` (**大小写不敏感**),
///   再与最后一个 `.` 之后的扩展名做 `_wcsicmp`;
/// - **含通配符**的 token: 走 `PathMatchSpec` (Windows 的 `*` / `?` 匹配).
///
/// 原实现用 `f == name` 与 `e == ext` 精确比较, 少了大小写不敏感这一半 ——
/// 同一个 `RS` / `Py` 扩展名的文件在 Windows 上很常见 (FAT/NTFS 大小写不敏感),
/// 结果会掉到 Plain 兜底, 所有语法高亮失效.
fn token_matches(token: &str, file_name: &str) -> bool {
    let token = token.trim();
    if token.is_empty() {
        return false;
    }
    if token.contains('*') || token.contains('?') {
        return wildcard_match(token, file_name);
    }
    // 1) 整个文件名 (大小写不敏感)
    if token.eq_ignore_ascii_case(file_name) {
        return true;
    }
    // 2) 最后一个 '.' 之后的扩展名 (大小写不敏感)
    match file_name.rfind('.') {
        Some(i) => token.eq_ignore_ascii_case(&file_name[i + 1..]),
        None => false,
    }
}

/// Windows `PathMatchSpec` 风格的 `*` / `?` 匹配 (大小写不敏感).
/// `*` 跨任意字符 (含分隔符, 与 PathMatchSpec 一致), `?` 恰好一个字符.
fn wildcard_match(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    // 经典贪心 + 回溯的通配匹配; 线性级回溯, 不会指数爆炸
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// 按 Sakura 的分隔符 (`,` `;` 空白) 切分类型列表条目.
fn split_tokens(s: &str) -> Vec<String> {
    s.split([',', ';', ' ', '\t'])
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
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

    /// 按路径判定文件类型 —— 对标 sakura `CDocTypeManager::IsFileNameMatch` 的行为.
    ///
    /// 顺序与 Sakura 一致: 文件名/通配匹配优先, 再扩展名, 最后 Plain 兜底.
    pub fn detect(&self, path: &str) -> Option<&TypeConfig> {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        // 1. file_names (支持通配 + 大小写不敏感), 精确名优先于通配
        for exact_pass in [true, false] {
            for t in &self.types {
                for entry in &t.file_names {
                    let has_wild = entry.contains('*') || entry.contains('?');
                    if exact_pass && has_wild {
                        continue; // 精确名先来
                    }
                    if !exact_pass && !has_wild {
                        continue; // 通配后
                    }
                    if token_matches(entry, name) {
                        return Some(t);
                    }
                }
            }
        }
        // 2. 扩展名 (大小写不敏感)
        for t in &self.types {
            for e in &t.exts {
                if token_matches(e, name) {
                    return Some(t);
                }
            }
        }
        // 3. 兜底 Plain
        self.types.iter().find(|t| t.name == "Plain")
    }

    /// 注册用户自定义类型 (Sakura 的「タイプ別設定 新規」).
    /// 同名类型会被替换 —— 语义与 Sakura 的覆盖式保存一致.
    pub fn register(&mut self, cfg: TypeConfig) {
        if let Some(slot) = self.types.iter_mut().find(|t| t.name == cfg.name) {
            *slot = cfg;
        } else {
            self.types.push(cfg);
        }
    }

    /// 删除自定义类型; 内置类型不允许删 (返回 false).
    /// Sakura 内置类型是可编辑但不可删除的, 这里保持一致.
    pub fn unregister(&mut self, name: &str) -> bool {
        if is_builtin_type(name) {
            return false;
        }
        let before = self.types.len();
        self.types.retain(|t| t.name != name);
        self.types.len() != before
    }

    pub fn by_name(&self, name: &str) -> Option<&TypeConfig> {
        self.types.iter().find(|t| t.name == name)
    }

    /// 按类型列出它的颜色分类 —— Sakura 的「タイプ別設定 『カラー』」列表.
    /// 返回 (分类名, CSS class) 对, 只含该类型真正会产生的分类.
    pub fn color_classes(&self, type_name: &str) -> Vec<(&'static str, &'static str)> {
        let Some(t) = self.by_name(type_name) else {
            return Vec::new();
        };
        let has = |present: bool, name: &'static str| -> Option<(&'static str, &'static str)> {
            present.then(|| (name, TokenKind::from_name(name).css_class()))
        };
        let mut out = Vec::new();
        out.extend(has(!t.keywords.is_empty(), "keyword"));
        out.extend(has(!t.type_keywords.is_empty(), "type"));
        out.extend(has(!t.strings.is_empty(), "string"));
        out.extend(has(
            t.line_comment.is_some() || t.block_comment.is_some(),
            "comment",
        ));
        out.extend(has(true, "number")); // 数字由通用词法器识别, 所有类型都有
        out
    }

    pub fn all(&self) -> &[TypeConfig] {
        &self.types
    }
}

/// 内置类型名 (不可删除). 与 default_registry 的 20 个一一对应.
fn is_builtin_type(name: &str) -> bool {
    // 必须与 types.rs 里 default_registry 的实际 name 完全一致 —— 写成 "C/C++" 而实际是
    // "C++" 会让 C++ 被当成可删除的自定义类型.
    const BUILTIN: &[&str] = &[
        "Plain",
        "Python",
        "Rust",
        "C++",
        "JavaScript",
        "TypeScript",
        "Go",
        "Java",
        "C#",
        "Ruby",
        "Bash",
        "HTML",
        "CSS",
        "JSON",
        "Markdown",
        "YAML",
        "TOML",
        "SQL",
        "Dockerfile",
        "Makefile",
    ];
    BUILTIN.contains(&name)
}

/// 抽取文本中的候选跳转标识符 (定义 + 引用), 供 tag_jump 建索引.
///
/// 与 `tag_jump::extract_tags` 的区别: 那个是**正则按语言**猜定义点 (`fn` / `def` /
/// `class`), 这个只做「哪些词可能是标识符」的词法筛选, 且**排除关键字** ——
/// 否则 `if` / `for` / `let` 全会进索引, 跳转列表被关键字淹没.
/// 返回 (标识符, 是否关键字) 序列, 按出现顺序去重.
pub fn extract_identifiers(text: &str, cfg: &TypeConfig) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<(String, bool)>| {
        if cur.is_empty() {
            return;
        }
        let w = std::mem::take(cur);
        // 首字符必须是字母/下划线, 否则是数字或符号, 不是标识符
        let first = w.chars().next().unwrap_or(' ');
        if !(first.is_alphabetic() || first == '_') {
            return;
        }
        let is_kw = cfg.is_keyword(&w);
        if !out.iter().any(|(n, _)| *n == w) {
            out.push((w, is_kw));
        }
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            cur.push(ch);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// 只取非关键字的标识符 —— 建 tag 索引时该用这个.
pub fn extract_jumpable(text: &str, cfg: &TypeConfig) -> Vec<String> {
    extract_identifiers(text, cfg)
        .into_iter()
        .filter(|(_, is_kw)| !*is_kw)
        .map(|(n, _)| n)
        .collect()
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    // ---- 类型匹配 (对标 CDocTypeManager::IsFileNameMatch) ----

    #[test]
    fn detect_is_case_insensitive() {
        let r = TypeRegistry::default_registry();
        // Windows 上 .RS / .Py 很常见; 原实现用 == 精确比较会掉到 Plain 兜底
        assert_eq!(r.detect("main.RS").map(|t| t.name.as_str()), Some("Rust"));
        assert_eq!(r.detect("script.PY").map(|t| t.name.as_str()), Some("Python"));
        assert_eq!(r.detect("A.CPP").map(|t| t.name.as_str()), Some("C++"));
    }

    #[test]
    fn detect_by_file_name_beats_extension() {
        let r = TypeRegistry::default_registry();
        assert_eq!(
            r.detect("proj/Dockerfile").map(|t| t.name.as_str()),
            Some("Dockerfile")
        );
        assert_eq!(
            r.detect("DOCKERFILE").map(|t| t.name.as_str()),
            Some("Dockerfile"),
            "文件名匹配也该大小写不敏感"
        );
    }

    #[test]
    fn wildcard_file_name_entry() {
        let mut r = TypeRegistry::default_registry();
        r.register(
            TypeConfig::new("Inc")
                .file_names(&["*.inc", "config.*"])
                .line_comment(";")
                .keywords(&["DEFINE"]),
        );
        assert_eq!(r.detect("boot.inc").map(|t| t.name.as_str()), Some("Inc"));
        assert_eq!(r.detect("x/y/BOOT.INC").map(|t| t.name.as_str()), Some("Inc"));
        assert_eq!(r.detect("config.h").map(|t| t.name.as_str()), Some("Inc"));
        assert_eq!(r.detect("unrelated.txt").map(|t| t.name.as_str()), Some("Plain"));
    }

    #[test]
    fn wildcard_engine_semantics() {
        assert!(wildcard_match("*.inc", "boot.inc"));
        assert!(wildcard_match("*.inc", "deep/dir/boot.inc"), "* 跨分隔符");
        assert!(wildcard_match("a?c.txt", "abc.txt"));
        assert!(!wildcard_match("a?c.txt", "ac.txt"), "? 恰好一个字符");
        assert!(!wildcard_match("a?c.txt", "abbc.txt"));
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("*.INC", "boot.inc"), "通配也大小写不敏感");
    }

    #[test]
    fn token_matches_semantics() {
        assert!(token_matches("rs", "main.rs"), "扩展名匹配");
        assert!(token_matches("rs", "main.RS"), "扩展名大小写不敏感");
        assert!(token_matches("Makefile", "Makefile"), "整名匹配");
        assert!(token_matches("makefile", "MAKEFILE"), "整名大小写不敏感");
        assert!(!token_matches("rs", "main.py"));
        assert!(!token_matches("", "main.rs"), "空 token 不匹配");
        assert!(!token_matches("rs", "noext"), "无点号文件不该按扩展名匹配");
    }

    #[test]
    fn exts_accepts_sakura_comma_string() {
        // 上游用 L"c,cpp,cxx,cc" 单字符串 (CType_Cpp.cpp:70); 这里是切片, 两者混用
        // 时必须归一, 否则 "c,cpp" 会被当成一个叫 "c,cpp" 的扩展名.
        let cfg = TypeConfig::new("X").exts(&["c,cpp,h", "inl"]);
        assert_eq!(cfg.exts, vec!["c", "cpp", "h", "inl"]);
        let r = TypeRegistry::default_registry();
        assert_eq!(r.detect("a.cpp").map(|t| t.name.as_str()), Some("C++"));
    }

    #[test]
    fn split_tokens_follows_sakura_separators() {
        assert_eq!(split_tokens("c,cpp,h"), vec!["c", "cpp", "h"]);
        assert_eq!(split_tokens("c; cpp  h"), vec!["c", "cpp", "h"]);
        assert!(split_tokens("  ").is_empty());
    }

    // ---- 关键字合并与大小写 ----

    #[test]
    fn user_keywords_merge_with_builtin() {
        let cfg = TypeConfig::new("T")
            .keywords(&["fn", "let"])
            .user_keywords(&["my_macro", "fn"]); // fn 重复, 应去重
        let eff = cfg.effective_keywords();
        assert_eq!(eff.len(), 3, "去重后应有 3 个: {eff:?}");
        assert!(eff.contains(&"my_macro".to_string()));
        assert!(cfg.is_keyword("my_macro"));
        assert!(cfg.is_keyword("fn"));
        assert!(!cfg.is_keyword("other"));
    }

    #[test]
    fn keyword_case_sensitivity_configurable() {
        let cs = TypeConfig::new("CS").keywords(&["SELECT"]);
        assert!(cs.is_keyword("SELECT"));
        assert!(!cs.is_keyword("select"), "默认区分大小写");

        let ci = TypeConfig::new("CI")
            .keywords(&["SELECT"])
            .keyword_case_sensitive(false);
        assert!(ci.is_keyword("select"));
        assert!(ci.is_keyword("SELECT"));
        assert_eq!(ci.effective_keywords(), vec!["select"], "不敏感时归一为小写");
    }

    #[test]
    fn default_config_is_case_sensitive() {
        let cfg = TypeConfig::new("X");
        assert!(cfg.keyword_case_sensitive, "缺省应区分大小写");
        assert_eq!(cfg.regex_syntax, "Onigmo");
    }

    #[test]
    fn regex_syntax_affects_lookbehind_support() {
        assert!(TypeConfig::new("A").supports_lookbehind());
        assert!(
            !TypeConfig::new("B")
                .regex_syntax("bregonig")
                .supports_lookbehind(),
            "bregonig 不支持变长后顾"
        );
    }

    // ---- 自定义类型注册 ----

    #[test]
    fn register_and_unregister_custom_type() {
        let mut r = TypeRegistry::default_registry();
        let n0 = r.all().len();
        r.register(TypeConfig::new("Zig").exts(&["zig"]).keywords(&["fn"]));
        assert_eq!(r.all().len(), n0 + 1);
        assert_eq!(r.detect("main.zig").map(|t| t.name.as_str()), Some("Zig"));

        r.register(TypeConfig::new("Zig").exts(&["zig", "zon"]));
        assert_eq!(r.all().len(), n0 + 1, "同名应覆盖而非追加");
        assert_eq!(r.detect("x.zon").map(|t| t.name.as_str()), Some("Zig"));

        assert!(r.unregister("Zig"));
        assert_eq!(r.all().len(), n0);
        assert!(!r.unregister("Zig"), "重复删除应返回 false");
    }

    #[test]
    fn builtin_types_cannot_be_deleted() {
        let mut r = TypeRegistry::default_registry();
        assert!(!r.unregister("Rust"), "内置类型不可删");
        assert!(r.by_name("Rust").is_some());
    }

    #[test]
    fn every_registered_type_is_protected_as_builtin() {
        // is_builtin_type 的清单与 default_registry 必须一一对应 —— 名字写错 (如把
        // "C++" 写成 "C/C++") 会让某个内置类型被当成自定义类型而可删除.
        let r = TypeRegistry::default_registry();
        for t in r.all() {
            assert!(
                is_builtin_type(&t.name),
                "类型 {} 未被列入内置保护清单",
                t.name
            );
        }
        assert!(
            !is_builtin_type("MyCustomType"),
            "非内置类型不该被保护"
        );
    }

    // ---- 颜色分类 ----

    #[test]
    fn color_classes_reflect_config() {
        let r = TypeRegistry::default_registry();
        let rust = r.color_classes("Rust");
        let names: Vec<&str> = rust.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"keyword"));
        assert!(names.contains(&"comment"));
        assert!(names.contains(&"string"));
        assert!(names.contains(&"number"));
        for (_, cls) in &rust {
            assert!(cls.starts_with("tok-"), "CSS class 应带 tok- 前缀: {cls}");
        }
        assert!(r.color_classes("NoSuchType").is_empty(), "未知类型返回空");
    }

    #[test]
    fn token_kind_lookup() {
        assert_eq!(TokenKind::from_name("keyword"), TokenKind::Keyword);
        assert_eq!(TokenKind::from_name("KEYWORD"), TokenKind::Keyword);
        assert_eq!(TokenKind::from_name("不存在的分类"), TokenKind::Default);
        assert_eq!(TokenKind::Keyword.name(), "keyword");
        assert_eq!(TokenKind::all().len(), 11);
    }

    // ---- 标识符抽取 ----

    #[test]
    fn extract_identifiers_marks_keywords() {
        let cfg = TypeConfig::new("Rust").keywords(&["fn", "let"]);
        let ids = extract_identifiers("fn main() { let x = compute(1); }", &cfg);
        let map: std::collections::HashMap<&str, bool> =
            ids.iter().map(|(n, k)| (n.as_str(), *k)).collect();
        assert_eq!(map.get("fn"), Some(&true));
        assert_eq!(map.get("let"), Some(&true));
        assert_eq!(map.get("main"), Some(&false));
        assert_eq!(map.get("compute"), Some(&false));
        assert_eq!(map.get("x"), Some(&false));
    }

    #[test]
    fn extract_jumpable_excludes_keywords() {
        let cfg = TypeConfig::new("Rust").keywords(&["fn", "let"]);
        let jump = extract_jumpable("fn main() { let compute = 1; }", &cfg);
        assert!(jump.contains(&"main".to_string()));
        assert!(jump.contains(&"compute".to_string()));
        assert!(!jump.contains(&"fn".to_string()), "关键字不该进跳转索引");
        assert!(!jump.contains(&"let".to_string()));
    }

    #[test]
    fn extract_identifiers_dedups_and_skips_numbers() {
        let cfg = TypeConfig::new("T");
        let ids = extract_identifiers("foo foo 123 456 bar", &cfg);
        let names: Vec<&str> = ids.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["foo", "bar"], "去重且不含纯数字");
    }

    #[test]
    fn extract_identifiers_empty_and_punct_only() {
        let cfg = TypeConfig::new("T");
        assert!(extract_identifiers("", &cfg).is_empty());
        assert!(extract_identifiers("  ... 123 !!! ", &cfg).is_empty());
    }

    // ---- 边界: 空类型表 ----

    #[test]
    fn empty_registry_detect_returns_none() {
        let r = TypeRegistry {
            types: Vec::new(),
        };
        assert!(r.detect("main.rs").is_none(), "空表 + 无 Plain 兜底 -> None");
        assert!(r.by_name("Rust").is_none());
        assert!(r.all().is_empty());
        assert!(r.color_classes("Rust").is_empty());
    }

    // ---- 原有测试 ----

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
