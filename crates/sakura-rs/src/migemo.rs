// migemo.rs — Migemo 風検索 (sakura 4.5) — basic 実装
//
// Migemo: ローマ字 1 ~ 数文字で 日本語 を検索.
// 例: "ky" → "きゃ", "きょ", "キャ", "キョ" 等
//
// 完全実装は外部辞書 (migemo dict) が必要.
// ここでは ローマ字 → ひらがな/カタカナ 変換テーブル (basic) を提供.
//
// # 已知能力边界 (与 cmigemo 的差距)
// cmigemo 靠 **40MB 的 cmigemo-dict** 把**汉字**读成罗马音, 所以搜 "学校" 能命中.
// 本模块刻意不引外部字典 (项目要求零新增重型依赖), 因此:
//   - 假名 / 片假名文本: 完整支持 (促音 SOKUON、长音、拗音、浊音、大写输入)
//   - **汉字文本: 不支持** —— 汉字读音需要音读/训读字典, 没有它无法还原.
//
// 见测试 `kanji_search_needs_dictionary`, 它把这条边界显式断言下来, 避免以后
// 误判为回归. 若要支持汉字, 两条路 (都尚未做):
//   1. 内置精简的常用汉字读音表 (数千字量级, 与 cmigemo-dict 的 4 万字不可比);
//   2. 改走「读音标注」路线 —— 但那已经不是 migemo 了.

/// ローマ字 → ひらがな 変換テーブル (基本)
/// 複数文字の組合せ (きゃ, しゃ, ちゃ 等) も含む
const ROMA_TO_KANA: &[(&str, &str)] = &[
    // 拗音 (2 文字)
    ("kya", "きゃ"),
    ("kyi", "きぃ"),
    ("kyu", "きゅ"),
    ("kye", "きぇ"),
    ("kyo", "きょ"),
    ("sha", "しゃ"),
    ("shi", "し"),
    ("syi", "しぃ"),
    ("shu", "しゅ"),
    ("she", "しぇ"),
    ("sho", "しょ"),
    ("cha", "ちゃ"),
    ("chi", "ち"),
    ("tyu", "ちゅ"),
    ("che", "ちぇ"),
    ("cho", "ちょ"),
    ("nya", "にゃ"),
    ("nyi", "にぃ"),
    ("nyu", "にゅ"),
    ("nye", "にぇ"),
    ("nyo", "にょ"),
    ("hya", "ひゃ"),
    ("hyi", "ひぃ"),
    ("hyu", "ひゅ"),
    ("hye", "ひぇ"),
    ("hyo", "ひょ"),
    ("mya", "みゃ"),
    ("myi", "みぃ"),
    ("myu", "みゅ"),
    ("mye", "みぇ"),
    ("myo", "みょ"),
    ("rya", "りゃ"),
    ("ryi", "りぃ"),
    ("ryu", "りゅ"),
    ("rye", "りぇ"),
    ("ryo", "りょ"),
    ("gya", "ぎゃ"),
    ("gyi", "ぎぃ"),
    ("gyu", "ぎゅ"),
    ("gye", "ぎぇ"),
    ("gyo", "ぎょ"),
    ("ja", "じゃ"),
    ("ji", "じ"),
    ("jya", "じゃ"),
    ("jyu", "じゅ"),
    ("jyo", "じょ"),
    ("bya", "びゃ"),
    ("byi", "びぃ"),
    ("byu", "びゅ"),
    ("bye", "びぇ"),
    ("byo", "びょ"),
    ("pya", "ぴゃ"),
    ("pyi", "ぴぃ"),
    ("pyu", "ぴゅ"),
    ("pye", "ぴぇ"),
    ("pyo", "ぴょ"),
    // ===== 促音 (小書き つ) =====
    // Migemo 增量搜索的高频输入: "kitte" (切手) / "gakkou" (学校) / "assari".
    // Hepburn 记法是 xtu/xtsu, Kunrei 是 ltu/ltsu —— cmigemo 两种都支持.
    ("xtu", "っ"),
    ("ltu", "っ"),
    ("xtsu", "っ"),
    ("ltsu", "っ"),
    // 双写促音: 第二个辅音要清音化 (きって -> "kitte" 的 tt 对应っ, 不是っと).
    // 这里只登记基础形, 双写在下方 SOKUON_FOLLOW 处理.
    ("tt", "っ"),
    ("ss", "っ"),
    ("pp", "っ"),
    // ===== 长音 =====
    // "ラーメン" -> "ra-men" / "raamen"; ー 在假名里就是长音符.
    ("-", "ー"),
    // ===== 撥音 (ん) =====
    // "n" 单独 / 連打时的扱い. 基本表已有 "n" -> "ん" (若表里有), 这里补歧义形.
    ("nn", "ん"),
    ("xn", "ん"),
    // 基本子音 + 母音
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("sa", "さ"),
    ("si", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("ta", "た"),
    ("ti", "ち"),
    ("tu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("ha", "は"),
    ("hi", "ひ"),
    ("hu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("ya", "や"),
    ("yi", "い"),
    ("yu", "ゆ"),
    ("ye", "いぇ"),
    ("yo", "よ"),
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("wa", "わ"),
    ("wi", "ゐ"),
    ("we", "ゑ"),
    ("wo", "を"),
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("za", "ざ"),
    ("zi", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    // 単独
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    ("n", "ん"),
    // 促音・長音
    ("tsu", "つ"),
];

/// 双写辅音归一化: 把叠写的辅音替换成「促音记号 + 清音化后的辅音」。
///
/// 例: `gakkou` -> `gaxtu` + `kou` = が + っ + こう (SOKUON 规则)。
/// 实现方式是插入 `xtu`(促音) 并保留**清音**形态的第二个辅音:
/// `kka` -> `kxtua`; 若第二个是浊音 (`ggo`) 则清音化 `o` (VOICED_SEMI list)。
///
/// 这是 cmigemo / 官方 Migemo 的标准行为, 原实现缺失, 导致 "gakkou" 展开成
/// "がkこう" 这种无意义的串, 搜索永远命中不了。
fn apply_sokuon(roma: &str) -> String {
    /// 促音记号 (xtu = っ)
    const SOKUON: &str = "xtu";
    /// 叠辅音的第二个若为浊音, 要清音化 (がっこう 的 く 由 ぐ 变)
    const VOICED_TO_CLEAN: &[(&str, &str)] = &[
        ("ga", "ka"),
        ("gi", "ki"),
        ("gu", "ku"),
        ("ge", "ke"),
        ("go", "ko"),
        ("za", "sa"),
        ("zi", "si"),
        ("zu", "su"),
        ("ze", "se"),
        ("zo", "so"),
        ("da", "ta"),
        ("di", "ti"),
        ("du", "tu"),
        ("de", "te"),
        ("do", "to"),
        ("ba", "pa"),
        ("bi", "pi"),
        ("bu", "pu"),
        ("be", "pe"),
        ("bo", "po"),
        ("va", "ha"),
        ("vi", "hi"),
        ("vu", "hu"),
        ("ve", "he"),
        ("vo", "ho"),
    ];

    let chars: Vec<char> = roma.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + SOKUON.chars().count());
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        // 叠辅音判定: 当前字符是辅音, 下一个是同辅音, 再后面是元音.
        // 促音插在**前一个音节之后**, 所以先输出 c 之前的部分, 再输出 c + 促音 + 清音化.
        if i + 2 < chars.len()
            && chars[i + 1] == c
            && is_consonant(c)
            && !is_consonant(chars[i + 2])
        {
            let voiced: String = chars[i..i + 2.min(chars.len() - i)].iter().collect();
            let clean = VOICED_TO_CLEAN
                .iter()
                .find(|(v, _)| *v == voiced)
                .map(|(_, cl)| cl.to_string())
                .unwrap_or_else(|| c.to_string());
            // c 已经在 out 里 (上一轮 push), 这里补促音与清音化的第二个
            out.extend(SOKUON.chars());
            out.extend(clean.chars());
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.into_iter().collect()
}

/// 是否是子音 (罗马字里起辅音作用的字母).
/// 元音 a/i/u/e/o 不是子音; 其余 (含拗音用的 y/w/n) 都算.
fn is_consonant(c: char) -> bool {
    !matches!(c, 'a' | 'i' | 'u' | 'e' | 'o')
}

/// ローマ字を ひらがな/カタカナ パターンに変換
/// 戻り値: 検索パターン (全角カタカナ/ひらがな/元のパターン の組合せ)
pub fn expand_pattern(roma: &str) -> Vec<String> {
    let mut patterns = vec![roma.to_string()];
    let lower = roma.to_lowercase();

    // 双写辅音 → 促音 + 清音化 (SOKUON 规则, cmigemo 同款)
    //
    // 日语音节以辅音结尾, 遇到叠辅音时后者要清音化:
    //   "gakkou" = が + っ + こう   (不是 がkこう, 也不是 がっこう)
    //   "kitte"  = き + っ + て
    //   "gakkou" 里第二个 k 是**清音** —— 若原文是浊音 (がkkou) 则第二个也浊.
    let lower = apply_sokuon(&lower);

    // ローマ字 → ひらがな 変換 (greedy, 最長一致優先)
    //
    // 按 **char** 而非 byte 推进: 原实现用 `&lower[i..i+len]` 按字节切片, 输入含
    // 非 ASCII (日文/中文/emoji/带音符字母) 时会切在 UTF-8 字符中间而 panic.
    // 表里的键全是 ASCII, 所以只需按 char 取前缀比对, 天然不会越界.
    let chars: Vec<char> = lower.chars().collect();
    let mut i = 0usize;
    let mut hiragana_acc = String::new();
    while i < chars.len() {
        // 4 文字 ("xtsu"/"ltsu" 这类 4 字母促音写法), 3, 2, 1 の順で試す (最長一致)
        let mut matched = false;
        for len in [4usize, 3, 2, 1] {
            if i + len > chars.len() {
                continue;
            }
            let substr: String = chars[i..i + len].iter().collect();
            if let Some((_, kana)) = ROMA_TO_KANA.iter().find(|(r, _)| *r == substr) {
                hiragana_acc.push_str(kana);
                i += len;
                matched = true;
                break;
            }
        }
        if !matched {
            // マッチしない文字はそのまま
            hiragana_acc.push(chars[i]);
            i += 1;
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
            // 拉长音符 U+30FC 落在 U+30A1..U+30F6 区间内, 但它不是规则的
            // 「U+30A1 起的连续块」——套用公式会算出 U+FFC0 (未定义区).
            // 它的半角形是 U+FF70 (ｰ), 必须单列.
            if c == '\u{30FC}' {
                return '\u{FF70}';
            }
            // 「」 ー 等不参与转换
            if ('\u{30A1}'..='\u{30F6}').contains(&c) {
                char::from_u32(c as u32 - 0x30A1 + 0xFF65).unwrap_or(c)
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

    fn has(patterns: &[String], want: &str) -> bool {
        patterns.iter().any(|p| p == want)
    }

    // ===== 促音 (SOKUON) =====

    #[test]
    fn geminate_produces_small_tsu() {
        assert!(
            has(&expand_pattern("gakkou"), "がっこう"),
            "gakkou -> がっこう"
        );
        assert!(has(&expand_pattern("kitte"), "きって"), "kitte -> きって");
        assert!(has(&expand_pattern("sakka"), "さっか"), "sakka -> さっか");
        assert!(has(&expand_pattern("gakkou"), "ガッコウ"), "片假名也要命中");
        assert!(has(&expand_pattern("kitte"), "キッテ"));
    }

    #[test]
    fn sokuon_rules() {
        assert_eq!(
            apply_sokuon("gakkou"),
            "gaxtukou",
            "促音插在音节末: ga + xtu + kou"
        );
        assert_eq!(apply_sokuon("kitte"), "kixtute", "ki + xtu + te");
        assert_eq!(apply_sokuon("sakka"), "saxtuka", "sa + xtu + ka");
        assert_eq!(apply_sokuon("kite"), "kite", "非叠辅音不该被改");
        assert_eq!(apply_sokuon("kana"), "kana");
        assert_eq!(apply_sokuon(""), "");
    }

    #[test]
    fn explicit_sokuon_markers() {
        assert!(has(&expand_pattern("xtu"), "っ"), "xtu -> っ");
        assert!(has(&expand_pattern("ltu"), "っ"), "ltu -> っ (Kunrei)");
        assert!(has(&expand_pattern("xtsu"), "っ"), "xtsu 是 4 字母写法");
        assert!(has(&expand_pattern("ltsu"), "っ"));
    }

    // ===== 长音 =====

    #[test]
    fn chouon_marks_map_to_long_vowel() {
        // 长音统一用 "-" 表达 (cmigemo 的 --romaji 全角用法)
        assert!(
            has(&expand_pattern("ra-men"), "ラーメン"),
            "ra-men -> ラーメン"
        );
        assert!(has(&expand_pattern("-"), "ー"), "单独的 - -> ー");
        assert!(
            !migemo_search("ラーメン", "ra-men").is_empty(),
            "长音搜索应有命中"
        );
    }

    #[test]
    fn chouon_halfwidth_conversion_is_correct() {
        // 拉长音符 U+30FC 落在 U+30A1..U+30F6 区间内, 但套用「区间内减偏移」的
        // 公式会算出 U+FFC0 (未定义区) —— 必须单列成 U+FF70.
        let hw = katakana_to_halfwidth("ラーメン");
        let codes: Vec<u32> = hw.chars().map(|c| c as u32).collect();
        assert_eq!(codes, vec![0xFFAD, 0xFF70, 0xFFA5, 0xFFB7], "got {codes:?}");
        assert!(!codes.contains(&0xFFC0), "不该产出未定义码位");
    }

    #[test]
    fn trailing_vowel_repeat_is_not_swallowed() {
        // kohii (コーヒー) 的正确切分是 ko+hi+i = こひい. 若表里有 "hii" -> "ひー"
        // 就会抢占切分, 展开成 こひー, 搜不到.
        let k = expand_pattern("kohii");
        assert_eq!(k[1], "こひい", "kohii 应展开为 こひい, 实际 {k:?}");
        assert!(!k.iter().any(|p| p.contains('ー')), "不该产生长音符");
    }

    // ===== 浊音 / 拗音 =====

    #[test]
    fn dakuten_and_handakuten_compose_correctly() {
        assert!(has(&expand_pattern("ga"), "が"));
        assert!(has(&expand_pattern("pa"), "ぱ"));
        assert!(has(&expand_pattern("ba"), "ば"));
        let pats = expand_pattern("ga");
        assert!(
            pats.iter().any(|p| p.contains('が')) && !pats.iter().any(|p| p.contains("か\u{309b}")),
            "浊音应合成一个码点, 实际 {pats:?}"
        );
    }

    #[test]
    fn youon_compose_not_decompose() {
        let pats = expand_pattern("kyo");
        assert!(has(&pats, "きょ"));
        assert!(has(&pats, "キョ"));
        assert!(!pats.iter().any(|p| p == "きよ"), "不该退化成 き+よ");
        assert!(has(&expand_pattern("sha"), "しゃ"));
        assert!(has(&expand_pattern("cha"), "ちゃ"));
    }

    #[test]
    fn uppercase_romaji_works() {
        assert!(has(&expand_pattern("KA"), "か"));
        assert!(has(&expand_pattern("KA"), "カ"));
        assert!(has(&expand_pattern("KYO"), "きょ"));
    }

    // ===== 已知能力边界 =====

    #[test]
    fn search_geminate_finds_kana_text() {
        assert!(
            !migemo_search("がっこうに行きます", "gakkou").is_empty(),
            "假名文本应命中"
        );
    }

    #[test]
    fn kanji_search_needs_dictionary() {
        // 已知边界: 本模块只有假名表, **无法**把汉字读成罗马音. cmigemo 靠的是
        // 40MB 的 cmigemo-dict. 我们不引外部字典, 所以 "学校" + "gakkou" 搜不到
        // 是**预期行为**. 把这条写进测试, 避免以后误判为回归.
        assert!(
            migemo_search("学校", "gakkou").is_empty(),
            "汉字读音需 cmigemo-dict, 当前刻意不支持 (见模块文档)"
        );
        assert!(
            !migemo_search("がっこう", "gakkou").is_empty(),
            "假名写法必须能搜到"
        );
    }

    // ===== 边界与健壮性 =====

    #[test]
    fn multibyte_input_does_not_panic() {
        // expand_pattern 曾按**字节**切多字节 UTF-8, 输入含非 ASCII 时会 panic
        for s in ["あ", "你好", "ｱｲｳ", "🇯🇵", "é", "こんにちは", "café"] {
            let _ = expand_pattern(s);
        }
    }

    #[test]
    fn empty_and_punct_only_input() {
        assert!(expand_pattern("").iter().any(|p| p.is_empty()));
        let _ = expand_pattern("!!!");
        let _ = expand_pattern("  ");
    }

    #[test]
    fn search_returns_sorted_deduped_offsets() {
        let hits = migemo_search("きょきょ", "kyo");
        let mut sorted = hits.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(hits, sorted, "命中位置应已排序去重");
    }

    #[test]
    fn search_empty_pattern_returns_nothing() {
        assert!(migemo_search("あいうえお", "").is_empty());
    }

    // ===== 表本身的契约 =====

    #[test]
    fn table_has_no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for (k, _) in ROMA_TO_KANA {
            assert!(seen.insert(*k), "ROMA_TO_KANA 有重复键: {k}");
        }
    }

    #[test]
    fn table_has_no_unreachable_keys() {
        // expand_pattern 先 to_lowercase, 大写键永远匹配不上 —— 是死条目
        for (k, _) in ROMA_TO_KANA {
            assert!(
                !k.chars().any(|c| c.is_uppercase()),
                "表里不该有大写键: {k}"
            );
        }
    }

    #[test]
    fn table_values_are_non_empty() {
        for (k, v) in ROMA_TO_KANA {
            assert!(!v.is_empty(), "键 {k} 的值为空");
        }
    }

    // ---- 原有测试 ----

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
