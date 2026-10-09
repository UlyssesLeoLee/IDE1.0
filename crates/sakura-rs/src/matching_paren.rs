// matching_paren.rs — 对应括弧跳转 (sakura 2.11)
//
// カーソル位置の括弧から対応する括弧へジャンプ.
// 対応: () [] {} <>
// 文字列/コメント内は無視 (将来拡張)

/// 括弧種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceKind {
    Round,   // ()
    Square,  // []
    Curly,   // {}
    Angle,   // <>
}

impl BraceKind {
    fn from_char(c: char) -> Option<(Self, bool)> {
        // (kind, is_open)
        match c {
            '(' => Some((Self::Round, true)),
            ')' => Some((Self::Round, false)),
            '[' => Some((Self::Square, true)),
            ']' => Some((Self::Square, false)),
            '{' => Some((Self::Curly, true)),
            '}' => Some((Self::Curly, false)),
            '<' => Some((Self::Angle, true)),
            '>' => Some((Self::Angle, false)),
            _ => None,
        }
    }

    fn open(self) -> char {
        match self {
            Self::Round => '(',
            Self::Square => '[',
            Self::Curly => '{',
            Self::Angle => '<',
        }
    }

    fn close(self) -> char {
        match self {
            Self::Round => ')',
            Self::Square => ']',
            Self::Curly => '}',
            Self::Angle => '>',
        }
    }
}

/// ジャンプ結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpResult {
    /// ジャンプ先 (絶対位置, 文字オフセット)
    pub target: usize,
}

/// 対応括弧を検索
pub fn find_matching(text: &str, pos: usize) -> Option<JumpResult> {
    let chars: Vec<char> = text.chars().collect();
    if pos >= chars.len() {
        return None;
    }

    let (kind, is_open) = BraceKind::from_char(chars[pos])?;
    find_match_in_chars(&chars, pos, kind, is_open)
}

fn find_match_in_chars(
    chars: &[char],
    start: usize,
    kind: BraceKind,
    is_open: bool,
) -> Option<JumpResult> {
    let open = kind.open();
    let close = kind.close();

    if is_open {
        // 前方向: 同種 + 1, 異種 - 1
        let mut depth = 1i64;
        let mut i = start + 1;
        while i < chars.len() {
            if chars[i] == open {
                depth += 1;
            } else if chars[i] == close {
                depth -= 1;
                if depth == 0 {
                    return Some(JumpResult { target: i });
                }
            }
            i += 1;
        }
        None // 対応なし
    } else {
        // 後方向
        if start == 0 {
            return None;
        }
        let mut depth = 1i64;
        let mut i = start as i64 - 1;
        while i >= 0 {
            let idx = i as usize;
            if chars[idx] == close {
                depth += 1;
            } else if chars[idx] == open {
                depth -= 1;
                if depth == 0 {
                    return Some(JumpResult { target: idx });
                }
            }
            i -= 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(s: &str, sub: &str) -> usize {
        s.find(sub).unwrap()
    }

    #[test]
    fn test_open_to_close() {
        let s = "fn main() { x }";
        let pos = offset(s, "{");
        let r = find_matching(s, pos).unwrap();
        assert_eq!(r.target, offset(s, "}"));
    }

    #[test]
    fn test_close_to_open() {
        let s = "fn main() { x }";
        let pos = offset(s, "}");
        let r = find_matching(s, pos).unwrap();
        assert_eq!(r.target, offset(s, "{"));
    }

    #[test]
    fn test_nested() {
        let s = "{ a { b } c }";
        let pos = offset(s, "{");
        let r = find_matching(s, pos).unwrap();
        assert_eq!(r.target, s.rfind('}').unwrap());
    }

    #[test]
    fn test_no_match() {
        let s = "{ a b c";
        let pos = offset(s, "{");
        assert!(find_matching(s, pos).is_none());
    }

    #[test]
    fn test_brackets() {
        let s = "arr[0][1]";
        let pos = offset(s, "[");
        let r = find_matching(s, pos).unwrap();
        assert_eq!(r.target, s.find(']').unwrap());
    }

    #[test]
    fn test_not_a_brace() {
        let s = "hello";
        assert!(find_matching(s, 0).is_none());
    }
}
