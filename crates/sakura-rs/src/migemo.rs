// migemo.rs — Migemo 風検索 (sakura 4.5) — basic 実装
//
// Migemo: ローマ字 1 ~ 数文字で 日本語 を検索.
// 例: "ky" → "きゃ", "きょ", "キャ", "キョ" 等
//
// 完全実装は外部辞書 (migemo dict) が必要.
// ここでは ローマ字 → ひらがな/カタカナ 変換テーブル (basic) を提供.

/// ローマ字 → ひらがな 変換テーブル (基本)
/// 複数文字の組合せ (きゃ, しゃ, ちゃ 等) も含む
const ROMA_TO_KANA: &[(&str, &str)] = &[
    // 拗音 (2 文字)
    ("kya", "きゃ"), ("kyi", "きぃ"), ("kyu", "きゅ"), ("kye", "きぇ"), ("kyo", "きょ"),
    ("sha", "しゃ"), ("shi", "し"), ("syi", "しぃ"),
    ("shu", "しゅ"), ("she", "しぇ"), ("sho", "しょ"),
    ("cha", "ちゃ"), ("chi", "ち"), ("tyu", "ちゅ"), ("che", "ちぇ"), ("cho", "ちょ"),
    ("nya", "にゃ"), ("nyi", "にぃ"), ("nyu", "にゅ"), ("nye", "にぇ"), ("nyo", "にょ"),
    ("hya", "ひゃ"), ("hyi", "ひぃ"), ("hyu", "ひゅ"), ("hye", "ひぇ"), ("hyo", "ひょ"),
    ("mya", "みゃ"), ("myi", "みぃ"), ("myu", "みゅ"), ("mye", "みぇ"), ("myo", "みょ"),
    ("rya", "りゃ"), ("ryi", "りぃ"), ("ryu", "りゅ"), ("rye", "りぇ"), ("ryo", "りょ"),
    ("gya", "ぎゃ"), ("gyi", "ぎぃ"), ("gyu", "ぎゅ"), ("gye", "ぎぇ"), ("gyo", "ぎょ"),
    ("ja", "じゃ"), ("ji", "じ"), ("jya", "じゃ"), ("jyu", "じゅ"), ("jyo", "じょ"),
    ("bya", "びゃ"), ("byi", "びぃ"), ("byu", "びゅ"), ("bye", "びぇ"), ("byo", "びょ"),
    ("pya", "ぴゃ"), ("pyi", "ぴぃ"), ("pyu", "ぴゅ"), ("pye", "ぴぇ"), ("pyo", "ぴょ"),
    // 基本子音 + 母音
    ("ka", "か"), ("ki", "き"), ("ku", "く"), ("ke", "け"), ("ko", "こ"),
    ("sa", "さ"), ("si", "し"), ("su", "す"), ("se", "せ"), ("so", "そ"),
    ("ta", "た"), ("ti", "ち"), ("tu", "つ"), ("te", "て"), ("to", "と"),
    ("na", "な"), ("ni", "に"), ("nu", "ぬ"), ("ne", "ね"), ("no", "の"),
    ("ha", "は"), ("hi", "ひ"), ("hu", "ふ"), ("he", "へ"), ("ho", "ほ"),
    ("ma", "ま"), ("mi", "み"), ("mu", "む"), ("me", "め"), ("mo", "も"),
    ("ya", "や"), ("yi", "い"), ("yu", "ゆ"), ("ye", "いぇ"), ("yo", "よ"),
    ("ra", "ら"), ("ri", "り"), ("ru", "る"), ("re", "れ"), ("ro", "ろ"),
    ("wa", "わ"), ("wi", "ゐ"), ("we", "ゑ"), ("wo", "を"),
    ("ga", "が"), ("gi", "ぎ"), ("gu", "ぐ"), ("ge", "げ"), ("go", "ご"),
    ("za", "ざ"), ("zi", "じ"), ("zu", "ず"), ("ze", "ぜ"), ("zo", "ぞ"),
    ("da", "だ"), ("di", "ぢ"), ("du", "づ"), ("de", "で"), ("do", "ど"),
    ("ba", "ば"), ("bi", "び"), ("bu", "ぶ"), ("be", "べ"), ("bo", "ぼ"),
    ("pa", "ぱ"), ("pi", "ぴ"), ("pu", "ぷ"), ("pe", "ぺ"), ("po", "ぽ"),
    // 単独
    ("a", "あ"), ("i", "い"), ("u", "う"), ("e", "え"), ("o", "お"),
    ("n", "ん"),
    // 促音・長音
    ("tsu", "つ"),
    ("-", "ー"),
];

/// ローマ字を ひらがな/カタカナ パターンに変換
/// 戻り値: 検索パターン (全角カタカナ/ひらがな/元のパターン の組合せ)
pub fn expand_pattern(roma: &str) -> Vec<String> {
    let mut patterns = vec![roma.to_string()];
    let lower = roma.to_lowercase();

    // ローマ字 → ひらがな 変換 (greedy, 累積)
    let bytes = lower.as_bytes();
    let mut i = 0;
    let mut hiragana_acc = String::new();
    while i < bytes.len() {
        // 3 文字, 2 文字, 1 文字 の順で試す
        let mut matched = false;
        for &len in &[3usize, 2, 1] {
            if i + len > bytes.len() {
                continue;
            }
            let substr = &lower[i..i + len];
            if let Some((_, kana)) = ROMA_TO_KANA.iter().find(|(r, _)| *r == substr) {
                hiragana_acc.push_str(kana);
                i += len;
                matched = true;
                break;
            }
        }
        if !matched {
            // マッチしない文字はそのまま
            let ch = lower[i..].chars().next().unwrap();
            hiragana_acc.push(ch);
            i += ch.len_utf8();
        }
    }

    patterns.push(hiragana_acc.clone());

    // カタカナ版
    let katakana: String = hiragana_acc.chars().map(hiragana_to_katakana).collect();
    let half_katakana = katakana_to_halfwidth(&katakana);
    patterns.push(katakana);
    patterns.push(half_katakana);

    patterns
}

fn hiragana_to_katakana(c: char) -> char {
    if ('\u{3041}'..='\u{3096}').contains(&c) {
        char::from_u32(c as u32 + 0x60).unwrap_or(c)
    } else {
        c
    }
}

fn katakana_to_halfwidth(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('\u{30A1}'..='\u{30F6}').contains(&c) {
                char::from_u32(c as u32 - 0x30A1 + 0xFF65 as u32).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// 検索: テキスト内で パターン (複数候補) のいずれかにマッチする位置を返す
pub fn migemo_search(text: &str, roma: &str) -> Vec<usize> {
    let patterns = expand_pattern(roma);
    let mut hits = Vec::new();
    for pat in &patterns {
        if pat.is_empty() {
            continue;
        }
        let mut start = 0;
        while let Some(pos) = text[start..].find(pat.as_str()) {
            hits.push(start + pos);
            // 次の開始位置 (1 文字進める, マルチバイト対応)
            let next = start + pos + pat.len();
            if next >= text.len() {
                break;
            }
            // 次の文字境界に進める
            let mut boundary = next;
            while boundary < text.len() && !text.is_char_boundary(boundary) {
                boundary += 1;
            }
            start = boundary;
        }
    }
    hits.sort();
    hits.dedup();
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_kana() {
        let patterns = expand_pattern("ka");
        assert!(patterns.iter().any(|p| p == "か"));
        assert!(patterns.iter().any(|p| p == "カ"));
    }

    #[test]
    fn test_combination_kyo() {
        let patterns = expand_pattern("kyo");
        assert!(patterns.iter().any(|p| p == "きょ"));
        assert!(patterns.iter().any(|p| p == "キョ"));
    }

    #[test]
    fn test_search_japanese() {
        // "きょう" in text → should match "kyo" pattern
        let text = "きょうは良い天気です";
        let hits = migemo_search(text, "kyo");
        assert!(!hits.is_empty(), "expected hits, got: {:?}", hits);
    }

    #[test]
    fn test_search_with_katakana() {
        // カタカナ "テスト" in text
        let text = "これはテストです";
        let hits = migemo_search(text, "tesuto");
        assert!(!hits.is_empty(), "expected hits, got: {:?}", hits);
    }

    #[test]
    fn test_no_match() {
        let text = "hello world";
        let hits = migemo_search(text, "kyo");
        // ASCII 文字なので ローマ字パターンでは マッチ しない
        // ただし "kyo" 自体は残る
        // → パターン に "kyo" が含まれるので "hello world" 内で "kyo" 検索 → 0
        // 実際は "kyo" 検索 で 0 hits が期待
        assert!(hits.is_empty());
    }

    #[test]
    fn test_expand_includes_original() {
        let patterns = expand_pattern("xyz");
        // 元の文字列も含まれる
        assert!(patterns.iter().any(|p| p == "xyz"));
    }

    #[test]
    fn test_hiragana_to_katakana() {
        assert_eq!(hiragana_to_katakana('あ'), 'ア');
        assert_eq!(hiragana_to_katakana('は'), 'ハ');
        assert_eq!(hiragana_to_katakana('A'), 'A'); // ASCII そのまま
    }
}
