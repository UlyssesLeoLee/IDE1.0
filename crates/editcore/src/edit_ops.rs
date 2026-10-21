//! 选区文字变换 / 行操作 (ARCH-01 Phase 1: 编辑语义放 Rust, 前端只负责选区与描画).
//!
//! 语义全部按真实 Sakura Editor 的键盘宏逐条比对确认 (tests/sakura-diff).
//! - [`transform_text`]: 作用于选中文字 (大小写 / 全角半角 / かな / TAB↔空格)
//! - [`transform_lines`]: 作用于选区涉及的整行 (排序 / 去空白 / 去重 / 缩进)

/// 半角カナ (U+FF61..U+FF9D) -> 全角 (濁点 ゛゜ 单独成字符时见 [`half_kana_to_full`])
const HALF_TO_FULL: &[(u32, u32)] = &[(0xFF61, 0x3002), (0xFF62, 0x300C), (0xFF63, 0x300D), (0xFF64, 0x3001), (0xFF65, 0x30FB), (0xFF66, 0x30F2), (0xFF67, 0x30A1), (0xFF68, 0x30A3), (0xFF69, 0x30A5), (0xFF6A, 0x30A7), (0xFF6B, 0x30A9), (0xFF6C, 0x30E3), (0xFF6D, 0x30E5), (0xFF6E, 0x30E7), (0xFF6F, 0x30C3), (0xFF70, 0x30FC), (0xFF71, 0x30A2), (0xFF72, 0x30A4), (0xFF73, 0x30A6), (0xFF74, 0x30A8), (0xFF75, 0x30AA), (0xFF76, 0x30AB), (0xFF77, 0x30AD), (0xFF78, 0x30AF), (0xFF79, 0x30B1), (0xFF7A, 0x30B3), (0xFF7B, 0x30B5), (0xFF7C, 0x30B7), (0xFF7D, 0x30B9), (0xFF7E, 0x30BB), (0xFF7F, 0x30BD), (0xFF80, 0x30BF), (0xFF81, 0x30C1), (0xFF82, 0x30C4), (0xFF83, 0x30C6), (0xFF84, 0x30C8), (0xFF85, 0x30CA), (0xFF86, 0x30CB), (0xFF87, 0x30CC), (0xFF88, 0x30CD), (0xFF89, 0x30CE), (0xFF8A, 0x30CF), (0xFF8B, 0x30D2), (0xFF8C, 0x30D5), (0xFF8D, 0x30D8), (0xFF8E, 0x30DB), (0xFF8F, 0x30DE), (0xFF90, 0x30DF), (0xFF91, 0x30E0), (0xFF92, 0x30E1), (0xFF93, 0x30E2), (0xFF94, 0x30E4), (0xFF95, 0x30E6), (0xFF96, 0x30E8), (0xFF97, 0x30E9), (0xFF98, 0x30EA), (0xFF99, 0x30EB), (0xFF9A, 0x30EC), (0xFF9B, 0x30ED), (0xFF9C, 0x30EF), (0xFF9D, 0x30F3)];

fn half_to_full(c: char) -> Option<char> {
    let u = c as u32;
    if !(0xFF61..=0xFF9D).contains(&u) {
        return None;
    }
    HALF_TO_FULL.iter().find(|(h, _)| *h == u).and_then(|(_, f)| char::from_u32(*f))
}

fn full_to_half(c: char) -> Option<char> {
    let u = c as u32;
    if !(0x3001..=0x30FC).contains(&u) {
        return None;
    }
    HALF_TO_FULL.iter().find(|(_, f)| *f == u).and_then(|(h, _)| char::from_u32(*h))
}

/// 可以带濁点 / 半濁点的全角假名 (基底)
const VOICEABLE: &[u32] = &[
    0x30AB, 0x30AD, 0x30AF, 0x30B1, 0x30B3, 0x30B5, 0x30B7, 0x30B9, 0x30BB, 0x30BD, 0x30BF, 0x30C1, 0x30C4, 0x30C6,
    0x30C8, 0x30CF, 0x30D2, 0x30D5, 0x30D8, 0x30DB,
];
const HA_ROW: &[u32] = &[0x30CF, 0x30D2, 0x30D5, 0x30D8, 0x30DB];

fn voiced_of(base: u32, mark: char) -> Option<char> {
    if mark == '\u{FF9F}' {
        return if HA_ROW.contains(&base) { char::from_u32(base + 2) } else { None };
    }
    if VOICEABLE.contains(&base) {
        return char::from_u32(base + 1);
    }
    match base {
        0x30A6 => char::from_u32(0x30F4),
        0x30EF => char::from_u32(0x30F7),
        0x30F2 => char::from_u32(0x30FA),
        _ => None,
    }
}

/// 半角カナ -> 全角 (ｶﾞ -> ガ, ﾊﾟ -> パ)
pub fn half_kana_to_full(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if let Some(f) = half_to_full(cs[i]) {
            if let Some(&m) = cs.get(i + 1) {
                if m == '\u{FF9E}' || m == '\u{FF9F}' {
                    if let Some(v) = voiced_of(f as u32, m) {
                        out.push(v);
                        i += 2;
                        continue;
                    }
                }
            }
            out.push(f);
        } else if cs[i] == '\u{FF9E}' {
            out.push('\u{309B}');
        } else if cs[i] == '\u{FF9F}' {
            out.push('\u{309C}');
        } else {
            out.push(cs[i]);
        }
        i += 1;
    }
    out
}

/// 全角カナ -> 半角カナ (ガ -> ｶﾞ). 平假名不动 (先 [`hira_to_kata`])
pub fn kata_to_half(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let u = c as u32;
        if let Some(h) = full_to_half(c) {
            out.push(h);
        } else if u == 0x309B {
            out.push('\u{FF9E}');
        } else if u == 0x309C {
            out.push('\u{FF9F}');
        } else if u == 0x30F4 || u == 0x30F7 || u == 0x30FA {
            let base = match u {
                0x30F4 => 0x30A6,
                0x30F7 => 0x30EF,
                _ => 0x30F2,
            };
            out.push(char::from_u32(base).and_then(full_to_half).unwrap_or(c));
            out.push('\u{FF9E}');
        } else if (0x30AC..=0x30DD).contains(&u) {
            // 濁音 / 半濁音: 反查基底
            let hit = VOICEABLE
                .iter()
                .find(|b| u == **b + 1)
                .map(|b| (*b, '\u{FF9E}'))
                .or_else(|| HA_ROW.iter().find(|b| u == **b + 2).map(|b| (*b, '\u{FF9F}')));
            match hit {
                Some((b, m)) => {
                    out.push(char::from_u32(b).and_then(full_to_half).unwrap_or(c));
                    out.push(m);
                }
                None => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn hira_to_kata(s: &str) -> String {
    s.chars()
        .map(|c| if ('\u{3041}'..='\u{3096}').contains(&c) { char::from_u32(c as u32 + 0x60).unwrap_or(c) } else { c })
        .collect()
}

pub fn kata_to_hira(s: &str) -> String {
    s.chars()
        .map(|c| if ('\u{30A1}'..='\u{30F6}').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap_or(c) } else { c })
        .collect()
}

pub fn ascii_to_full(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
            ' ' => '\u{3000}',
            _ => c,
        })
        .collect()
}

pub fn full_to_ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' => ' ',
            _ => c,
        })
        .collect()
}

fn zen_ei(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { char::from_u32(c as u32 + 0xFEE0).unwrap_or(c) } else { c })
        .collect()
}

fn han_ei(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}' | '\u{FF10}'..='\u{FF19}' => {
                char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
            }
            _ => c,
        })
        .collect()
}

fn is_wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6)
}

/// TAB -> 空格 (对齐到制表位)
pub fn tabs_to_spaces(line: &str, w: usize) -> String {
    let w = w.max(1);
    let mut out = String::new();
    let mut col = 0usize;
    for c in line.chars() {
        if c == '\t' {
            let n = w - (col % w);
            out.extend(std::iter::repeat(' ').take(n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

/// 空格 -> TAB: 每段连续空格里跨过制表位的部分换成 TAB, 零头保留空格
pub fn spaces_to_tabs(line: &str, w: usize) -> String {
    let w = w.max(1);
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut col = 0usize;
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            ' ' => {
                let mut j = i;
                while j < cs.len() && cs[j] == ' ' {
                    j += 1;
                }
                let end = col + (j - i);
                let mut c = col;
                while (c / w) * w + w <= end {
                    out.push('\t');
                    c = (c / w) * w + w;
                }
                out.extend(std::iter::repeat(' ').take(end - c));
                col = end;
                i = j;
            }
            '\t' => {
                out.push('\t');
                col = (col / w) * w + w;
                i += 1;
            }
            c => {
                out.push(c);
                col += if is_wide(c) { 2 } else { 1 };
                i += 1;
            }
        }
    }
    out
}

/// 选中文字的变换. 未知操作返回 None.
pub fn transform_text(op: &str, text: &str, tab_width: usize) -> Option<String> {
    let per_line = |f: &dyn Fn(&str) -> String| text.split('\n').map(f).collect::<Vec<_>>().join("\n");
    Some(match op {
        "to-upper" => text.to_uppercase(),
        "to-lower" => text.to_lowercase(),
        "to-halfwidth" => kata_to_half(&hira_to_kata(&full_to_ascii(text))),
        "to-zenkaku" | "to-fullwidth" => hira_to_kata(&ascii_to_full(&half_kana_to_full(text))),
        "to-zenhira" => kata_to_hira(&ascii_to_full(&half_kana_to_full(text))),
        "to-zenei" => zen_ei(text),
        "to-hanei" => han_ei(text),
        "to-hankata" => kata_to_half(text),
        "hankata-to-zenkata" => half_kana_to_full(text),
        "hankata-to-zenhira" => kata_to_hira(&half_kana_to_full(text)),
        "tab-to-space" => per_line(&|l| tabs_to_spaces(l, tab_width)),
        "space-to-tab" => per_line(&|l| spaces_to_tabs(l, tab_width)),
        _ => return None,
    })
}

fn cmp_utf16(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// 选区涉及的整行的变换. 未知操作返回 None.
pub fn transform_lines(op: &str, lines: &[String]) -> Option<Vec<String>> {
    let map = |f: &dyn Fn(&str) -> String| lines.iter().map(|l| f(l)).collect::<Vec<_>>();
    Some(match op {
        "sort-asc" => {
            let mut v = lines.to_vec();
            v.sort_by(|a, b| cmp_utf16(a, b));
            v
        }
        "sort-desc" => {
            let mut v = lines.to_vec();
            v.sort_by(|a, b| cmp_utf16(b, a));
            v
        }
        "ltrim-lines" => map(&|l| l.trim_start_matches([' ', '\t']).to_string()),
        "rtrim-lines" => map(&|l| l.trim_end_matches([' ', '\t']).to_string()),
        "uniq-consecutive" => {
            let mut v: Vec<String> = Vec::with_capacity(lines.len());
            for l in lines {
                if v.last() != Some(l) {
                    v.push(l.clone());
                }
            }
            v
        }
        "indent-selection" => map(&|l| if l.is_empty() { String::new() } else { format!("\t{l}") }),
        "unindent-selection" => map(&|l| match l.strip_prefix('\t') {
            Some(r) => r.to_string(),
            None => {
                let n = l.chars().take(4).take_while(|c| *c == ' ').count();
                l[n..].to_string()
            }
        }),
        "indent-space" => map(&|l| if l.is_empty() { String::new() } else { format!(" {l}") }),
        "unindent-space" => map(&|l| l.strip_prefix(' ').unwrap_or(l).to_string()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(op: &str, s: &str) -> String {
        transform_text(op, s, 4).unwrap()
    }
    fn l(op: &str, v: &[&str]) -> Vec<String> {
        transform_lines(op, &v.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    // 以下期望值来自与真实 Sakura 的比对 (tests/sakura-diff 对应用例)
    #[test]
    fn halfwidth_conversion() {
        assert_eq!(t("to-halfwidth", "ＡＢＣ１２３　あ"), "ABC123 ｱ");
        assert_eq!(t("to-hankata", "カタカナガパ"), "ｶﾀｶﾅｶﾞﾊﾟ");
    }
    #[test]
    fn zenkaku_conversion() {
        assert_eq!(t("to-zenkaku", "abc ｶﾀｶﾅ あいう"), "ａｂｃ　カタカナ　アイウ");
        assert_eq!(t("to-zenhira", "abc ｶﾀｶﾅ カタカナ"), "ａｂｃ　かたかな　かたかな");
        assert_eq!(t("hankata-to-zenkata", "ｶﾀｶﾅｶﾞﾊﾟ"), "カタカナガパ");
        assert_eq!(t("hankata-to-zenhira", "ｶﾀｶﾅｶﾞﾊﾟ"), "かたかながぱ");
        assert_eq!(t("to-zenei", "ABC123あ"), "ＡＢＣ１２３あ");
        assert_eq!(t("to-hanei", "ＡＢＣ１２３あ"), "ABC123あ");
    }
    #[test]
    fn voiced_roundtrip() {
        assert_eq!(kata_to_half("ヴァバパ"), "ｳﾞｧﾊﾞﾊﾟ");
        assert_eq!(half_kana_to_full("ｳﾞｧﾊﾞﾊﾟ"), "ヴァバパ");
    }
    #[test]
    fn tab_space() {
        assert_eq!(t("tab-to-space", "ab\tc\n\tx"), "ab  c\n    x");
        assert_eq!(t("space-to-tab", "ab      c\n   x"), "ab\t\tc\n   x");
        assert_eq!(t("space-to-tab", "        a"), "\t\ta");
    }
    #[test]
    fn case() {
        assert_eq!(t("to-upper", "abc Def"), "ABC DEF");
        assert_eq!(t("to-lower", "ＡＢC あ"), "ａｂc あ");
    }
    #[test]
    fn sort_is_code_unit_order() {
        assert_eq!(l("sort-asc", &["c", "b", "a", "B"]), ["B", "a", "b", "c"]);
        assert_eq!(l("sort-desc", &["a", "b", "c", "B"]), ["c", "b", "a", "B"]);
        assert_eq!(l("sort-asc", &["b", "", "a", "", "c"]), ["", "", "a", "b", "c"]);
    }
    #[test]
    fn trim_and_uniq() {
        assert_eq!(l("ltrim-lines", &["  a", "\t\tb"]), ["a", "b"]);
        assert_eq!(l("rtrim-lines", &["a  ", "b\t\t"]), ["a", "b"]);
        assert_eq!(l("uniq-consecutive", &["a", "a", "b", "b", "b", "c", "a"]), ["a", "b", "c", "a"]);
    }
    #[test]
    fn indent_rules() {
        assert_eq!(l("indent-selection", &["a", "", "b"]), ["\ta", "", "\tb"]);
        assert_eq!(l("unindent-selection", &["\ta", "  b", "     c", "d"]), ["a", "b", " c", "d"]);
        assert_eq!(l("indent-space", &["a", "", "b"]), [" a", "", " b"]);
        assert_eq!(l("unindent-space", &["    a", " b", "c"]), ["   a", "b", "c"]);
    }
    #[test]
    fn unknown_op() {
        assert!(transform_text("nope", "x", 4).is_none());
        assert!(transform_lines("nope", &[]).is_none());
    }
}
