// zenkaku_hankaku.rs — 全角⇔半角 変換 (sakura 10.2)
//
// ASCII 全角/半角 変換, カタカナ 全角/半角 変換.

/// 文字カテゴリ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharCategory {
    /// 変換対象でない
    Other,
    /// ASCII 印字可能文字
    Ascii,
    /// ASCII 全角
    FullWidthAscii,
    /// カタカナ (全角)
    Katakana,
    /// カタカナ (半角)
    HalfWidthKatakana,
}

pub fn categorize(c: char) -> CharCategory {
    if c.is_ascii_graphic() || c == ' ' {
        CharCategory::Ascii
    } else if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
        CharCategory::FullWidthAscii
    } else if ('\u{30A1}'..='\u{30F6}').contains(&c) || c == '\u{30FC}' {
        CharCategory::Katakana
    } else if ('\u{FF65}'..='\u{FF9F}').contains(&c) {
        CharCategory::HalfWidthKatakana
    } else {
        CharCategory::Other
    }
}

/// ASCII → 全角
fn ascii_to_fullwidth(c: char) -> Option<char> {
    if c == ' ' {
        // ASCII space → IDEOGRAPHIC SPACE (U+3000)
        Some('　')
    } else if c.is_ascii_graphic() {
        Some(char::from_u32(c as u32 + 0xFEE0).unwrap_or(c))
    } else {
        None
    }
}

/// 全角 → ASCII
fn fullwidth_to_ascii(c: char) -> Option<char> {
    if c == '　' {
        // IDEOGRAPHIC SPACE → ASCII space
        Some(' ')
    } else if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
        Some(char::from_u32(c as u32 - 0xFEE0).unwrap_or(c))
    } else {
        None
    }
}

/// 全角 → 半角カタカナ
fn katakana_to_half(c: char) -> Option<char> {
    // 主要な対応表 (完全実装は将来)
    let table: &[(char, char)] = &[
        ('ァ', 'ｧ'), ('ィ', 'ｨ'), ('ゥ', 'ｩ'), ('ェ', 'ｪ'), ('ォ', 'ｫ'),
        ('カ', 'ｶ'), ('キ', 'ｷ'), ('ク', 'ｸ'), ('ケ', 'ｹ'), ('コ', 'ｺ'),
        ('サ', 'ｻ'), ('シ', 'ｼ'), ('ス', 'ｽ'), ('セ', 'ｾ'), ('ソ', 'ｿ'),
        ('タ', 'ﾀ'), ('チ', 'ﾁ'), ('ツ', 'ﾂ'), ('テ', 'ﾃ'), ('ト', 'ﾄ'),
        ('ナ', 'ﾅ'), ('ニ', 'ﾆ'), ('ヌ', 'ﾇ'), ('ネ', 'ﾈ'), ('ノ', 'ﾉ'),
        ('ハ', 'ﾊ'), ('ヒ', 'ﾋ'), ('フ', 'ﾌ'), ('ヘ', 'ﾍ'), ('ホ', 'ﾎ'),
        ('マ', 'ﾏ'), ('ミ', 'ﾐ'), ('ム', 'ﾑ'), ('メ', 'ﾒ'), ('モ', 'ﾓ'),
        ('ヤ', 'ﾔ'), ('ユ', 'ﾕ'), ('ヨ', 'ﾖ'),
        ('ラ', 'ﾗ'), ('リ', 'ﾘ'), ('ル', 'ﾙ'), ('レ', 'ﾚ'), ('ロ', 'ﾛ'),
        ('ワ', 'ﾜ'), ('ヲ', 'ｦ'), ('ン', 'ﾝ'),
        ('ー', 'ｰ'),
    ];
    table.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// 半角 → 全角カタカナ
fn half_katakana_to_full(c: char) -> Option<char> {
    let table: &[(char, char)] = &[
        ('ｧ', 'ァ'), ('ｨ', 'ィ'), ('ｩ', 'ゥ'), ('ｪ', 'ェ'), ('ｫ', 'ォ'),
        ('ｶ', 'カ'), ('ｷ', 'キ'), ('ｸ', 'ク'), ('ｹ', 'ケ'), ('ｺ', 'コ'),
        ('ｻ', 'サ'), ('ｼ', 'シ'), ('ｽ', 'ス'), ('ｾ', 'セ'), ('ｿ', 'ソ'),
        ('ﾀ', 'タ'), ('ﾁ', 'チ'), ('ﾂ', 'ツ'), ('ﾃ', 'テ'), ('ﾄ', 'ト'),
        ('ﾅ', 'ナ'), ('ﾆ', 'ニ'), ('ﾇ', 'ヌ'), ('ﾈ', 'ネ'), ('ﾉ', 'ノ'),
        ('ﾊ', 'ハ'), ('ﾋ', 'ヒ'), ('ﾌ', 'フ'), ('ﾍ', 'ヘ'), ('ﾎ', 'ホ'),
        ('ﾏ', 'マ'), ('ﾐ', 'ミ'), ('ﾑ', 'ム'), ('ﾒ', 'メ'), ('ﾓ', 'モ'),
        ('ﾔ', 'ヤ'), ('ﾕ', 'ユ'), ('ﾖ', 'ヨ'),
        ('ﾗ', 'ラ'), ('ﾘ', 'リ'), ('ﾙ', 'ル'), ('ﾚ', 'レ'), ('ﾛ', 'ロ'),
        ('ﾜ', 'ワ'), ('ｦ', 'ヲ'), ('ﾝ', 'ン'),
        ('ｰ', 'ー'),
    ];
    table.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// 全角 → 半角
pub fn to_halfwidth(text: &str) -> String {
    text.chars()
        .map(|c| {
            fullwidth_to_ascii(c)
                .or_else(|| katakana_to_half(c))
                .unwrap_or(c)
        })
        .collect()
}

/// 半角 → 全角
pub fn to_fullwidth(text: &str) -> String {
    text.chars()
        .map(|c| {
            ascii_to_fullwidth(c)
                .or_else(|| half_katakana_to_full(c))
                .unwrap_or(c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_to_fullwidth() {
        assert_eq!(to_fullwidth("ABC 123"), "ＡＢＣ　１２３");
    }

    #[test]
    fn test_fullwidth_to_ascii() {
        assert_eq!(to_halfwidth("ＡＢＣ　１２３"), "ABC 123");
    }

    #[test]
    fn test_katakana_to_half() {
        assert_eq!(to_halfwidth("カタカナ"), "ｶﾀｶﾅ");
    }

    #[test]
    fn test_half_katakana_to_full() {
        assert_eq!(to_fullwidth("ｶﾀｶﾅ"), "カタカナ");
    }

    #[test]
    fn test_unchanged_chars() {
        // ASCII → ASCII fullwidth (always changes); non-ASCII CJK unchanged
        // Halfwidth leaves non-ASCII alone
        let s = "漢字";
        assert_eq!(to_halfwidth(s), s);
        assert_eq!(to_fullwidth(s), s);
    }

    #[test]
    fn test_round_trip() {
        let s = "Hello World 123";
        let fw = to_fullwidth(s);
        let back = to_halfwidth(&fw);
        assert_eq!(back, s);
    }

    #[test]
    fn test_categorize() {
        assert_eq!(categorize('A'), CharCategory::Ascii);
        assert_eq!(categorize('Ａ'), CharCategory::FullWidthAscii);
        assert_eq!(categorize('カ'), CharCategory::Katakana);
        assert_eq!(categorize('ｶ'), CharCategory::HalfWidthKatakana);
        assert_eq!(categorize('漢'), CharCategory::Other);
    }
}
