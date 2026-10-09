// incremental_search.rs — インクリメンタルサーチ (sakura 4.4)
//
// 1 文字入力ごとに検索し, マッチ位置を返す.
// 大文字小文字区別/区別なし 切替対応.

/// 検索オプション
#[derive(Debug, Clone, Copy)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub wrap: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            wrap: true,
        }
    }
}

/// 検索結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchHit {
    /// マッチ開始位置 (文字オフセット)
    pub start: usize,
    /// マッチ終了位置 (排他的)
    pub end: usize,
}

impl SearchHit {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// インクリメンタル検索
/// pattern が空文字なら None (検索しない)
pub fn incremental_search(
    text: &str,
    pattern: &str,
    from: usize,
    opts: SearchOptions,
) -> Option<SearchHit> {
    if pattern.is_empty() {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let pat_chars: Vec<char> = pattern.chars().collect();

    if pat_chars.len() > chars.len() {
        return None;
    }

    let (haystack, needle) = if opts.case_sensitive {
        (chars.clone(), pat_chars.clone())
    } else {
        (
            chars.iter().map(|c| c.to_ascii_lowercase()).collect(),
            pat_chars.iter().map(|c| c.to_ascii_lowercase()).collect(),
        )
    };

    // from から検索, 見つからなければ wrap
    let start_pos = from.min(haystack.len());
    let mut found = scan(&haystack, &needle, start_pos, haystack.len());

    if found.is_none() && opts.wrap {
        found = scan(&haystack, &needle, 0, start_pos);
    }

    found.map(|(s, e)| SearchHit { start: s, end: e })
}

fn scan(haystack: &[char], needle: &[char], from: usize, to: usize) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return Some((from, from));
    }
    if from >= to || needle.len() > (to - from) {
        return None;
    }
    let end_max = to - needle.len();
    let mut i = from;
    while i <= end_max {
        if &haystack[i..i + needle.len()] == needle {
            return Some((i, i + needle.len()));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_match() {
        let s = "hello world";
        let h = incremental_search(s, "world", 0, SearchOptions::default()).unwrap();
        assert_eq!(h.start, 6);
        assert_eq!(h.end, 11);
    }

    #[test]
    fn test_case_insensitive_default() {
        let s = "Hello World";
        let h = incremental_search(s, "hello", 0, SearchOptions::default()).unwrap();
        assert_eq!(h.start, 0);
    }

    #[test]
    fn test_case_sensitive() {
        let s = "Hello World";
        let h = incremental_search(
            s,
            "hello",
            0,
            SearchOptions { case_sensitive: true, wrap: false },
        );
        assert!(h.is_none());
    }

    #[test]
    fn test_no_match() {
        let s = "hello";
        let h = incremental_search(s, "xyz", 0, SearchOptions::default());
        assert!(h.is_none());
    }

    #[test]
    fn test_from_position() {
        let s = "abcabc";
        let h = incremental_search(s, "abc", 1, SearchOptions::default()).unwrap();
        assert_eq!(h.start, 3);
    }

    #[test]
    fn test_wrap() {
        let s = "abc xyz";
        let h = incremental_search(s, "abc", 4, SearchOptions::default()).unwrap();
        assert_eq!(h.start, 0);
    }

    #[test]
    fn test_no_wrap() {
        let s = "abc xyz";
        let h = incremental_search(
            s,
            "abc",
            4,
            SearchOptions { case_sensitive: false, wrap: false },
        );
        assert!(h.is_none());
    }

    #[test]
    fn test_empty_pattern() {
        let h = incremental_search("hello", "", 0, SearchOptions::default());
        assert!(h.is_none());
    }

    #[test]
    fn test_unicode_basic() {
        // 中文 / 日本語 は char 単位処理なので OK
        let s = "你好世界";
        let h = incremental_search(s, "世界", 0, SearchOptions::default()).unwrap();
        assert_eq!(h.start, 2);
        assert_eq!(h.end, 4);
    }
}
