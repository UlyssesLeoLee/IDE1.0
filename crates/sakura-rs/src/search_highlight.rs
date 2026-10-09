// search_highlight.rs — 検索文字列強調表示 (sakura 4.11)
//
// マッチ範囲を記録し, フロントエンドに範囲情報を返す.

/// 強調セグメント
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSegment {
    /// 開始位置 (バイトオフセット)
    pub start: usize,
    /// 終了位置 (排他)
    pub end: usize,
}

impl HighlightSegment {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// 強調マネージャー
#[derive(Debug, Default, Clone)]
pub struct HighlightManager {
    segments: Vec<HighlightSegment>,
}

impl HighlightManager {
    pub fn new() -> Self {
        Self { segments: Vec::new() }
    }

    /// 全マッチを検出
    pub fn highlight_all(&mut self, text: &str, pattern: &str) -> usize {
        self.segments.clear();
        if pattern.is_empty() {
            return 0;
        }
        let mut count = 0;
        let mut start = 0;
        while let Some(pos) = text[start..].find(pattern) {
            let abs = start + pos;
            let end = abs + pattern.len();
            self.segments.push(HighlightSegment::new(abs, end));
            start = end;
            count += 1;
        }
        count
    }

    /// 大文字小文字を区別せず全マッチ
    pub fn highlight_all_case_insensitive(&mut self, text: &str, pattern: &str) -> usize {
        self.segments.clear();
        if pattern.is_empty() {
            return 0;
        }
        let text_lower = text.to_lowercase();
        let pat_lower = pattern.to_lowercase();
        let mut count = 0;
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(&pat_lower) {
            let abs = start + pos;
            let end = abs + pattern.len();
            self.segments.push(HighlightSegment::new(abs, end));
            start = end;
            count += 1;
        }
        count
    }

    /// 全セグメント取得
    pub fn segments(&self) -> &[HighlightSegment] {
        &self.segments
    }

    /// クリア
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// マッチ数
    pub fn count(&self) -> usize {
        self.segments.len()
    }

    /// 位置が マッチ範囲内か
    pub fn is_highlighted(&self, pos: usize) -> bool {
        self.segments.iter().any(|s| pos >= s.start && pos < s.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_highlight() {
        let mut m = HighlightManager::new();
        let n = m.highlight_all("hello world hello", "hello");
        assert_eq!(n, 2);
        assert_eq!(m.segments().len(), 2);
        assert_eq!(m.segments()[0], HighlightSegment::new(0, 5));
        assert_eq!(m.segments()[1], HighlightSegment::new(12, 17));
    }

    #[test]
    fn test_no_match() {
        let mut m = HighlightManager::new();
        let n = m.highlight_all("hello", "xyz");
        assert_eq!(n, 0);
        assert!(m.segments().is_empty());
    }

    #[test]
    fn test_empty_pattern() {
        let mut m = HighlightManager::new();
        let n = m.highlight_all("hello", "");
        assert_eq!(n, 0);
    }

    #[test]
    fn test_case_insensitive() {
        let mut m = HighlightManager::new();
        let n = m.highlight_all_case_insensitive("Hello HELLO hello", "hello");
        assert_eq!(n, 3);
    }

    #[test]
    fn test_is_highlighted() {
        let mut m = HighlightManager::new();
        m.highlight_all("hello world", "world");
        assert!(!m.is_highlighted(0));
        assert!(m.is_highlighted(7));
        assert!(m.is_highlighted(10));
        assert!(!m.is_highlighted(12));
    }

    #[test]
    fn test_overlapping_pattern() {
        let mut m = HighlightManager::new();
        let n = m.highlight_all("aaaa", "aa");
        assert_eq!(n, 2);
        // (0,2), (2,4)
        assert_eq!(m.segments()[0].end, 2);
        assert_eq!(m.segments()[1].start, 2);
    }

    #[test]
    fn test_clear() {
        let mut m = HighlightManager::new();
        m.highlight_all("hello", "hello");
        assert_eq!(m.count(), 1);
        m.clear();
        assert_eq!(m.count(), 0);
    }
}
