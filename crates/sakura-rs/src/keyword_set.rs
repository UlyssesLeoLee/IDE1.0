// keyword_set.rs — 強調キーワード 10 セット (sakura 6.2)
//
// 1 つの言語に対し 10 セットのキーワード (色違いで強調).
// セット 1 ~ 10: 異なる色/スタイル で同じキーワードを強調可能.
//
// 例: 言語 Rust で
//  - セット 1: キーワード (青)
//  - セット 2: 型名 (緑)
//  - セット 3: マクロ名 (黄)
//  - セット 4: トレイト名 (シアン)
//  - セット 5: 関数名 (赤)
//  ...

use std::collections::HashSet;

/// キーワードセット (1 ~ 10)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeywordSet {
    pub id: u8, // 1-10
    pub name: String,
    pub keywords: HashSet<String>,
    pub case_sensitive: bool,
}

impl KeywordSet {
    pub fn new(id: u8, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            keywords: HashSet::new(),
            case_sensitive: true,
        }
    }

    pub fn with_keywords(mut self, kws: &[&str]) -> Self {
        for k in kws {
            self.keywords.insert((*k).to_string());
        }
        self
    }

    pub fn add(&mut self, keyword: impl Into<String>) {
        self.keywords.insert(keyword.into());
    }

    pub fn contains(&self, word: &str) -> bool {
        if self.case_sensitive {
            self.keywords.contains(word)
        } else {
            self.keywords.iter().any(|k| k.eq_ignore_ascii_case(word))
        }
    }
}

/// キーワードセット管理 (10 セット + 言語別)
#[derive(Debug, Default)]
pub struct KeywordSetRegistry {
    sets: Vec<KeywordSet>,
    max_sets: usize,
}

impl KeywordSetRegistry {
    pub fn new() -> Self {
        Self {
            sets: Vec::new(),
            max_sets: 10,
        }
    }

    /// セット追加 (id は 1-10)
    pub fn add(&mut self, set: KeywordSet) -> Result<(), &'static str> {
        if self.sets.len() >= self.max_sets {
            return Err("Max 10 keyword sets");
        }
        if !(1..=10).contains(&set.id) {
            return Err("Set ID must be 1-10");
        }
        self.sets.push(set);
        Ok(())
    }

    /// ID で取得
    pub fn get(&self, id: u8) -> Option<&KeywordSet> {
        self.sets.iter().find(|s| s.id == id)
    }

    /// 全セット
    pub fn all(&self) -> &[KeywordSet] {
        &self.sets
    }

    /// word が どのセットに含まれるか (複数可)
    pub fn find_sets(&self, word: &str) -> Vec<u8> {
        self.sets
            .iter()
            .filter(|s| s.contains(word))
            .map(|s| s.id)
            .collect()
    }
}

/// 言語別既定キーワードセット (sakura デフォルト)
pub fn rust_default_sets() -> KeywordSetRegistry {
    let mut r = KeywordSetRegistry::new();
    r.add(KeywordSet::new(1, "Keywords").with_keywords(&[
        "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait",
        "if", "else", "match", "for", "while", "loop", "return", "as", "in", "where",
    ])).unwrap();
    r.add(KeywordSet::new(2, "Types").with_keywords(&[
        "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128",
        "f32", "f64", "bool", "char", "str", "String", "Vec", "Option", "Result",
    ])).unwrap();
    r.add(KeywordSet::new(3, "Macros").with_keywords(&[
        "println", "print", "format", "vec", "panic", "assert", "dbg", "todo", "unimplemented",
    ])).unwrap();
    r
}

pub fn python_default_sets() -> KeywordSetRegistry {
    let mut r = KeywordSetRegistry::new();
    r.add(KeywordSet::new(1, "Keywords").with_keywords(&[
        "def", "class", "if", "elif", "else", "for", "while", "import", "from", "as",
        "return", "yield", "with", "try", "except", "finally", "lambda", "pass", "break",
    ])).unwrap();
    r.add(KeywordSet::new(2, "Builtins").with_keywords(&[
        "True", "False", "None", "self", "cls", "print", "len", "range", "list", "dict",
        "tuple", "set", "str", "int", "float", "bool", "open", "isinstance", "type",
    ])).unwrap();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_set() {
        let s = KeywordSet::new(1, "test").with_keywords(&["foo", "bar"]);
        assert!(s.contains("foo"));
        assert!(s.contains("bar"));
        assert!(!s.contains("baz"));
    }

    #[test]
    fn test_case_insensitive() {
        let mut s = KeywordSet::new(1, "test");
        s.case_sensitive = false;
        s.add("Foo");
        assert!(s.contains("foo"));
        assert!(s.contains("FOO"));
    }

    #[test]
    fn test_registry_max() {
        let mut r = KeywordSetRegistry::new();
        for i in 1..=10 {
            r.add(KeywordSet::new(i, format!("set{}", i))).unwrap();
        }
        // 11 個目はエラー
        let r11 = r.add(KeywordSet::new(11, "extra"));
        assert!(r11.is_err());
    }

    #[test]
    fn test_invalid_id() {
        let mut r = KeywordSetRegistry::new();
        assert!(r.add(KeywordSet::new(0, "bad")).is_err());
        assert!(r.add(KeywordSet::new(11, "bad")).is_err());
    }

    #[test]
    fn test_find_sets() {
        let mut r = KeywordSetRegistry::new();
        r.add(KeywordSet::new(1, "kw").with_keywords(&["fn", "let"])).unwrap();
        r.add(KeywordSet::new(2, "types").with_keywords(&["i32", "String"])).unwrap();
        let sets = r.find_sets("fn");
        assert_eq!(sets, vec![1]);
        let sets = r.find_sets("i32");
        assert_eq!(sets, vec![2]);
    }

    #[test]
    fn test_rust_default() {
        let r = rust_default_sets();
        assert!(r.find_sets("fn").contains(&1));
        assert!(r.find_sets("i32").contains(&2));
        assert!(r.find_sets("println").contains(&3));
    }

    #[test]
    fn test_python_default() {
        let r = python_default_sets();
        assert!(r.find_sets("def").contains(&1));
        assert!(r.find_sets("True").contains(&2));
    }
}
