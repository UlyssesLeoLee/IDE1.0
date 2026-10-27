//! Sakura 的"词"模型: 字符种类 (CharKind) 与基于它的词边界 / 词移动。
//!
//! 光标移动 (`nav`)、选词 / 词删除 (`edit_cmd`) 共用这一份; 查找的「单词单位」边界用 [`is_word_char`]。
//! 列一律按码点 (`char`) 计。语义按真实 Sakura 比对确认 (tests/sakura-diff/cases9.mjs)。

/// 单词字符: 字母 / 数字 / 下划线 (查找的「单词单位」边界用这个)
pub fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c.is_numeric() || c == '_'
}

/// Sakura 的字符种类 (CWordParse::WhatKindOfChar 的简化版; 按真实 Sakura 比对确认, 见 tests/sakura-diff/cases9.mjs)。
/// 单词移动 / 选词 / 词删除都以「同一种类的连续段」为一个词: 漢字 / ひらがな / カタカナ / 英数 / 全角英数 / 記号 分别成词。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharKind {
    Space,
    ZenSpace,
    /// 半角英数 + `_` (以及其它语言的字母)
    Csym,
    /// 全角英数 Ａ-Ｚ ａ-ｚ ０-９ ＿
    ZenCsym,
    Hira,
    ZenKata,
    HanKata,
    /// 长音 ー / 浊点 ゛゜: 跟前后的假名连成一个词
    KanaJoin,
    /// 漢字 (含 々 〆 〇)
    Kanji,
    /// 其它 (记号 / 标点)
    Kigo,
}

pub fn char_kind(c: char) -> CharKind {
    let u = c as u32;
    match u {
        0x20 | 0x09 => CharKind::Space,
        0x3000 => CharKind::ZenSpace,
        0x30FC | 0x309B | 0x309C | 0xFF70 | 0xFF9E | 0xFF9F => CharKind::KanaJoin,
        0x3041..=0x3096 | 0x309D | 0x309E => CharKind::Hira,
        0x30A1..=0x30FA | 0x30FD | 0x30FE => CharKind::ZenKata,
        0xFF66..=0xFF9D => CharKind::HanKata,
        0xFF10..=0xFF19 | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A | 0xFF3F => CharKind::ZenCsym,
        0x3005 | 0x3006 | 0x3007 | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x3FFFF => CharKind::Kanji,
        _ if c == '_' || c.is_alphanumeric() => CharKind::Csym,
        _ if c.is_whitespace() => CharKind::Space,
        _ => CharKind::Kigo,
    }
}

fn is_space_kind(k: CharKind) -> bool {
    matches!(k, CharKind::Space | CharKind::ZenSpace)
}

/// 两个相邻字符是否属于同一个词
fn same_word(a: CharKind, b: CharKind) -> bool {
    use CharKind::*;
    if a == b {
        return true;
    }
    let kana = |k: CharKind| matches!(k, Hira | ZenKata | HanKata);
    (a == KanaJoin && kana(b)) || (b == KanaJoin && kana(a))
}

/// 光标所在字符所属的词 (同一种类的连续段) [i, j)
pub fn word_run(line: &[char], col: usize) -> (usize, usize) {
    if col >= line.len() {
        return (line.len(), line.len());
    }
    let (mut i, mut j) = (col, col + 1);
    while i > 0 && same_word(char_kind(line[i - 1]), char_kind(line[i])) {
        i -= 1;
    }
    while j < line.len() && same_word(char_kind(line[j - 1]), char_kind(line[j])) {
        j += 1;
    }
    (i, j)
}

/// Sakura S_WordRight: 跳过光标所在的词, 再跳过后面的空白, 落在下一个词首 (记号也算一个词)
pub fn word_right(line: &[char], from: usize) -> usize {
    let mut i = from.min(line.len());
    if i >= line.len() {
        return line.len();
    }
    if !is_space_kind(char_kind(line[i])) {
        i = word_run(line, i).1;
    }
    while i < line.len() && is_space_kind(char_kind(line[i])) {
        i += 1;
    }
    i
}

/// Sakura S_WordLeft: 跳过前面的空白, 再回到前一个词的词首
pub fn word_left(line: &[char], from: usize) -> usize {
    let mut i = from.min(line.len());
    while i > 0 && is_space_kind(char_kind(line[i - 1])) {
        i -= 1;
    }
    if i > 0 {
        i = word_run(line, i - 1).0;
    }
    i
}

/// 行首空白之后的第一个字符的列 (下一行的「第一个词首」; 全是空白则 0)
pub fn first_word_col(line: &[char]) -> usize {
    line.iter().position(|c| !is_space_kind(char_kind(*c))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 期望值来自真实 Sakura (tests/sakura-diff/cases9.mjs)
    const JA: &[&str] = &["日本語のテキストabc def、漢字123かな", "カタカナＡＢＣ全角　スペース"];

    #[test]
    fn japanese_word_runs() {
        let l0: Vec<char> = JA[0].chars().collect();
        let l1: Vec<char> = JA[1].chars().collect();
        assert_eq!(word_run(&l0, 1), (0, 3)); // 日本語
        assert_eq!(word_run(&l0, 3), (3, 4)); // の
        assert_eq!(word_run(&l0, 5), (4, 8)); // テキスト
        assert_eq!(word_run(&l0, 18), (18, 21)); // 123
        assert_eq!(word_run(&l1, 4), (4, 7)); // ＡＢＣ
        assert_eq!(word_run(&l1, 7), (7, 9)); // 全角
        let l: Vec<char> = "スーパー".chars().collect();
        assert_eq!(word_run(&l, 0), (0, 4)); // 长音连在假名里
    }
}
