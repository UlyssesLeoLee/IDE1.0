//! 对括弧 (S_BracketPair / 括弧强调): 找到光标处括弧的另一半.
//!
//! 无状态: 前端交上光标周围的一段行 (`rows`), 这里在这些行里向匹配方向扫描 (最多 30000 个字符,
//! 与 Sakura 一样不无限扫描); 嵌套按同种括弧计数。语义按真实 Sakura Editor 比对 (tests/sakura-diff 的 BR 用例)。

use serde::{Deserialize, Serialize};

/// 开括弧 -> 闭括弧
const PAIRS: &[(char, char)] = &[
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('<', '>'),
    ('（', '）'),
    ('「', '」'),
    ('『', '』'),
    ('【', '】'),
    ('［', '］'),
    ('｛', '｝'),
    ('〈', '〉'),
    ('《', '》'),
    ('〔', '〕'),
];
const SCAN_LIMIT: usize = 30000;

#[derive(Debug, Clone, Deserialize)]
pub struct BracketReq {
    pub row: usize,
    pub col: usize,
    pub total_rows: usize,
    /// [行号, 文本]; 缺的行视为扫描到头
    pub rows: Vec<(usize, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BracketRes {
    /// 光标处 (或光标前一个字符) 的括弧列
    pub at: usize,
    /// 另一半的位置
    pub pair: [usize; 2],
}

fn find_from(req: &BracketReq, row: usize, col: usize) -> Option<[usize; 2]> {
    let line_of = |r: usize| -> Option<Vec<char>> { req.rows.iter().find(|(i, _)| *i == r).map(|(_, s)| s.chars().collect()) };
    let cur = line_of(row)?;
    let ch = *cur.get(col)?;
    let (open, want) = if let Some((_, c)) = PAIRS.iter().find(|(o, _)| *o == ch) {
        (true, *c)
    } else if let Some((o, _)) = PAIRS.iter().find(|(_, c)| *c == ch) {
        (false, *o)
    } else {
        return None;
    };
    let dir: isize = if open { 1 } else { -1 };
    let (mut depth, mut scanned) = (0i32, 0usize);
    let (mut r, mut c) = (row as isize, col as isize);
    while r >= 0 && (r as usize) < req.total_rows && scanned < SCAN_LIMIT {
        let arr = line_of(r as usize)?;
        while c >= 0 && (c as usize) < arr.len() {
            scanned += 1;
            if arr[c as usize] == ch {
                depth += 1;
            } else if arr[c as usize] == want {
                depth -= 1;
                if depth == 0 {
                    return Some([r as usize, c as usize]);
                }
            }
            c += dir;
        }
        r += dir;
        c = if dir > 0 {
            0
        } else {
            match line_of(r as usize) {
                Some(l) => l.len() as isize - 1,
                None => return None,
            }
        };
    }
    None
}

/// 先看光标所在字符, 再看光标前一个字符 (括弧强调 / 对括弧都是这个顺序)
pub fn find_pair(req: &BracketReq) -> Option<BracketRes> {
    for c in [Some(req.col), req.col.checked_sub(1)].into_iter().flatten() {
        if let Some(pair) = find_from(req, req.row, c) {
            return Some(BracketRes { at: c, pair });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(row: usize, col: usize, lines: &[&str]) -> BracketReq {
        BracketReq { row, col, total_rows: lines.len(), rows: lines.iter().enumerate().map(|(i, s)| (i, s.to_string())).collect() }
    }
    const DOC: &[&str] = &["a(b[c]{d}<e>)f", "x (a", "  [b]", ") y"];

    #[test]
    fn nested_and_directions() {
        assert_eq!(find_pair(&req(0, 1, DOC)).map(|r| r.pair), Some([0, 12])); // ( -> )
        assert_eq!(find_pair(&req(0, 12, DOC)).map(|r| r.pair), Some([0, 1])); // ) -> (
        assert_eq!(find_pair(&req(0, 3, DOC)).map(|r| r.pair), Some([0, 5])); // [ -> ]
        assert_eq!(find_pair(&req(0, 6, DOC)).map(|r| r.pair), Some([0, 8])); // { -> }
        assert_eq!(find_pair(&req(0, 9, DOC)).map(|r| r.pair), Some([0, 11])); // < -> >
    }
    #[test]
    fn char_before_cursor_counts() {
        // Sakura: 光标在 ")" 右侧 (col 13) 也算
        let r = find_pair(&req(0, 13, DOC)).unwrap();
        assert_eq!((r.at, r.pair), (12, [0, 1]));
    }
    #[test]
    fn across_lines() {
        assert_eq!(find_pair(&req(1, 2, DOC)).map(|r| r.pair), Some([3, 0]));
        assert_eq!(find_pair(&req(3, 0, DOC)).map(|r| r.pair), Some([1, 2]));
    }
    #[test]
    fn none_when_not_a_bracket_or_unbalanced() {
        assert!(find_pair(&req(0, 0, DOC)).is_none());
        assert!(find_pair(&req(0, 13, &["(("])).is_none());
        let r = req(0, 0, &["(a"]);
        assert!(find_pair(&r).is_none());
    }
    #[test]
    fn fullwidth_brackets() {
        let d = &["「あ」"];
        assert_eq!(find_pair(&req(0, 0, d)).map(|r| r.pair), Some([0, 2]));
    }
}
