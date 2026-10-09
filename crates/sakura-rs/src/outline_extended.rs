// outline_extended.rs — 行頭数字/記号 ツリー (sakura 6.6)
//
// テキストの各行から, 行頭の数字 / 記号 / 見出し を抽出してツリー構造化.
// 例:
//   1. Introduction
//   1.1 Background
//   2. Method
//   2.1 Data
//   # Header
//   ## Sub-header

/// アウトライン種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineKind {
    /// Markdown の `#` 見出し
    MarkdownHeader,
    /// "1.", "1.1", "2.3.4" 形式
    NumberedSection,
    /// 箇条書き "-", "*", "+"
    BulletPoint,
    /// `// SECTION` 形式
    SectionComment,
}

/// アウトライン項目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineNode {
    pub kind: OutlineKind,
    pub level: u8,  // 階層 (0 が最上位)
    pub label: String,
    pub line_no: usize, // 0-indexed
    pub children: Vec<OutlineNode>,
}

impl OutlineNode {
    pub fn new(kind: OutlineKind, level: u8, label: String, line_no: usize) -> Self {
        Self {
            kind,
            level,
            label,
            line_no,
            children: Vec::new(),
        }
    }
}

/// パース結果 (フラットリスト)
#[derive(Debug, Default, Clone)]
pub struct OutlineTree {
    pub nodes: Vec<OutlineNode>,
}

impl OutlineTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// フラットリスト
    pub fn flat(&self) -> &[OutlineNode] {
        &self.nodes
    }

    /// 件数
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// パーサ
pub fn parse(text: &str) -> OutlineTree {
    let mut tree = OutlineTree::new();

    for (i, line) in text.lines().enumerate() {
        if let Some(node) = parse_line(line, i) {
            tree.nodes.push(node);
        }
    }

    tree
}

fn parse_line(line: &str, line_no: usize) -> Option<OutlineNode> {
    let trimmed = line.trim_start();

    // Markdown ヘッダ
    if trimmed.starts_with('#') {
        let mut level = 0u8;
        let mut idx = 0;
        while idx < trimmed.len() && trimmed.as_bytes()[idx] == b'#' && level < 6 {
            level += 1;
            idx += 1;
        }
        if level > 0 && idx < trimmed.len() && trimmed.as_bytes()[idx] == b' ' {
            let label = trimmed[idx + 1..].trim().to_string();
            if !label.is_empty() {
                return Some(OutlineNode::new(
                    OutlineKind::MarkdownHeader,
                    level,
                    label,
                    line_no,
                ));
            }
        }
    }

    // 番号付き "1.", "1.1", "1.2.3"
    let bytes = trimmed.as_bytes();
    let mut level = 0u8;
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits_end = i;
    while i < bytes.len() && bytes[i] == b'.' {
        level += 1;
        i += 1;
        if i < bytes.len() && bytes[i].is_ascii_digit() {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            digits_end = i;
        }
    }
    if level > 0 && i < bytes.len() && bytes[i] == b' ' && digits_end > 0 {
        let label = trimmed[i + 1..].trim().to_string();
        if !label.is_empty() {
            return Some(OutlineNode::new(
                OutlineKind::NumberedSection,
                level,
                label,
                line_no,
            ));
        }
    }

    // 箇条書き
    if let Some(stripped) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return Some(OutlineNode::new(
            OutlineKind::BulletPoint,
            0,
            stripped.to_string(),
            line_no,
        ));
    }

    // SECTION コメント
    if let Some(stripped) = trimmed
        .strip_prefix("// SECTION:")
        .or_else(|| trimmed.strip_prefix("// ==="))
    {
        return Some(OutlineNode::new(
            OutlineKind::SectionComment,
            0,
            stripped.to_string(),
            line_no,
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_headers() {
        let text = "# Top\n## Sub\n# Another\n";
        let tree = parse(text);
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[0].label, "Top");
        assert_eq!(tree.nodes[0].level, 1);
    }

    #[test]
    fn test_numbered_sections() {
        let text = "1. First\n1.1 Sub\n2. Second\n";
        let tree = parse(text);
        assert_eq!(tree.nodes.len(), 3);
    }

    #[test]
    fn test_bullets() {
        let text = "- item one\n- item two\n";
        let tree = parse(text);
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].kind, OutlineKind::BulletPoint);
    }

    #[test]
    fn test_section_comments() {
        let text = "// SECTION: intro\nfn main() {}\n";
        let tree = parse(text);
        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(tree.nodes[0].kind, OutlineKind::SectionComment);
    }

    #[test]
    fn test_no_match() {
        let text = "regular line\nanother\n";
        let tree = parse(text);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_mixed() {
        let text = "# Title\n1. Intro\n- item\n";
        let tree = parse(text);
        assert!(tree.nodes.len() >= 3);
    }
}
