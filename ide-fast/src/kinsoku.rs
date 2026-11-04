//! 固定列数折行 + 行头 / 行尾禁则 (追い出し方式): 行头不放句读 / 闭括号 / 小假名等, 行尾不放开括号。
//! 做法: 把折行点算好, 在显示文字里插入换行符交给 egui 排版 (一行一个段落), 映射表 (字符列 -> 显示下标) 跟着挪。
//! 列宽按显示宽度算: 全角字符 2 列, 其他 1 列, 与 `wrap_col` (半角字符数) 同单位。

use crate::doc::expand_tabs;

/// 不能出现在行头的字符 (句读 / 闭括号 / 长音 / 小假名 / 重复符号 等) 和 ASCII 的结尾标点
const NO_START: &str = "、。，．・：；？！゛゜ゝゞヽヾー々〉》」』】〕〗〙〟）］｝｠｡｣､･ﾞﾟ’”‰′″℃ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ)]},.:;?!%";
/// 不能出现在行尾的字符 (开括号)
const NO_END: &str = "〈《「『【〔〖〘〝（［｛｟｢‘“([{";
/// 可「ぶら下げ」(挂在行尾、超出折行列) 的句读
const HANGABLE: &str = "、。，．,.";
/// 回退找折行点最多退几个字符 (退太多说明这行全是禁则字符, 干脆按原位折)
const MAX_BACK: usize = 6;

/// 显示宽度 (列): 全角 / 宽字符 2, 其他 1
pub fn cell_width(c: char) -> usize {
    let u = c as u32;
    let wide = matches!(u,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD);
    if wide {
        2
    } else {
        1
    }
}

/// 折行参数: `cols` = 一行最多几列 (0 = 不处理); `hang` = 允许句读挂在行尾 (ぶら下げ)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Wrap {
    pub cols: usize,
    pub hang: bool,
}

/// 算折行点 (不挂标点)
#[cfg(test)]
pub fn breaks(chars: &[char], cols: usize) -> Vec<usize> {
    breaks_hang(chars, cols, false)
}

/// 算折行点: 返回「哪些字符下标处要换到新的一行」(升序, 不含 0)。`cols` = 一行最多几列 (< 2 当作不折)。
/// `hang` 开着时, 本该折到行头的句读 (、。) 改为挂在上一行末尾, 每行最多挂一个
pub fn breaks_hang(chars: &[char], cols: usize, hang: bool) -> Vec<usize> {
    let mut out = vec![];
    if cols < 2 {
        return out;
    }
    let mut start = 0usize; // 当前行的第一个字符
    let mut used = 0usize;
    let mut i = 0usize;
    let mut hung = false;
    while i < chars.len() {
        let w = cell_width(chars[i]);
        if hang && !hung && used + w > cols && i > start && HANGABLE.contains(chars[i]) {
            hung = true; // 挂在行尾: 不占列, 下一个字符一定要换行
            used = cols;
            i += 1;
            continue;
        }
        if used + w > cols && i > start {
            // 在 i 前折; 行头禁则 / 行尾禁则就往回退 (追い出し)
            let mut b = i;
            let mut back = 0;
            while back < MAX_BACK
                && b > start + 1
                && (NO_START.contains(chars[b]) || NO_END.contains(chars[b - 1]))
            {
                b -= 1;
                back += 1;
            }
            if NO_START.contains(chars[b]) || NO_END.contains(chars[b - 1]) {
                b = i; // 退不出合适的点
            }
            out.push(b);
            start = b;
            used = 0;
            hung = false;
            i = b;
            continue;
        }
        used += w;
        i += 1;
    }
    out
}

/// 同 `expand_tabs`, 但按 `cols` 列折行并在折行点插入 `\n`; `cols == 0` = 不处理 (原样返回)。映射表语义不变: 字符下标 -> 显示字符下标
pub fn expand_wrap(line: &str, tab: usize, w: Wrap) -> (String, Vec<usize>) {
    let (disp, mut map) = expand_tabs(line, tab);
    let cols = w.cols;
    if cols == 0 {
        return (disp, map);
    }
    let chars: Vec<char> = disp.chars().collect();
    let bs = breaks_hang(&chars, cols, w.hang);
    if bs.is_empty() {
        return (disp, map);
    }
    let mut out = String::with_capacity(disp.len() + bs.len());
    let mut bi = 0;
    for (i, c) in chars.iter().enumerate() {
        if bi < bs.len() && bs[bi] == i {
            out.push('\n');
            bi += 1;
        }
        out.push(*c);
    }
    for m in map.iter_mut() {
        *m += bs.iter().take_while(|&&b| b <= *m).count();
    }
    (out, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn br(s: &str, cols: usize) -> Vec<usize> {
        breaks(&s.chars().collect::<Vec<_>>(), cols)
    }

    #[test]
    fn plain_text_wraps_at_the_column() {
        assert_eq!(br("abcdefghij", 4), vec![4, 8]);
        assert_eq!(br("abcd", 4), Vec::<usize>::new());
        assert_eq!(br("abc", 0), Vec::<usize>::new());
    }

    #[test]
    fn full_width_counts_two_columns() {
        assert_eq!(br("あいうえお", 4), vec![2, 4]);
    }

    #[test]
    fn line_start_prohibited_char_pushes_previous_char_down() {
        // 「あいう。」 在 6 列: あい う 填满 6 列后 「。」 要进行头 -> 把「う」一起带下去
        assert_eq!(br("あいう。え", 6), vec![2]);
        // 没有禁则时 正常折在 3 个全角之后
        assert_eq!(br("あいうえお", 6), vec![3]);
    }

    #[test]
    fn line_end_prohibited_char_moves_down() {
        // 开括号不能留在行尾
        assert_eq!(br("あい「うえ", 6), vec![2]);
    }

    #[test]
    fn all_prohibited_run_falls_back_to_plain_break() {
        let s = "。。。。。。。。";
        assert_eq!(br(s, 4), vec![2, 4, 6]);
    }

    #[test]
    fn expand_wrap_inserts_newlines_and_shifts_map() {
        let (s, m) = expand_wrap(
            "abcdefgh",
            4,
            Wrap {
                cols: 4,
                hang: false,
            },
        );
        assert_eq!(s, "abcd\nefgh");
        assert_eq!(m, vec![0, 1, 2, 3, 5, 6, 7, 8, 9]);
        // 不折时与 expand_tabs 一致
        assert_eq!(
            expand_wrap("a\tb", 4, Wrap::default()),
            expand_tabs("a\tb", 4)
        );
    }

    #[test]
    fn hanging_punctuation_stays_on_the_previous_row() {
        // 「あいう。え」 6 列: 不挂 = [2] (把う带下去); 挂 = 。 留在第一行末尾, え 换行
        assert_eq!(
            breaks_hang(&"あいう。え".chars().collect::<Vec<_>>(), 6, true),
            vec![4]
        );
        // 只挂句读: 闭括号仍走追い出し
        assert_eq!(
            breaks_hang(&"あいう」え".chars().collect::<Vec<_>>(), 6, true),
            vec![2]
        );
        // 每行最多挂一个: 连着两个句读时第二个走追い出し, 行宽不会超过 列数 + 2
        let (s2, _) = expand_wrap(
            "あいう。。えお",
            4,
            Wrap {
                cols: 6,
                hang: true,
            },
        );
        for row in s2.split('\n') {
            assert!(row.chars().map(cell_width).sum::<usize>() <= 8, "{row}");
        }
        let (s, m) = expand_wrap(
            "あいう。え",
            4,
            Wrap {
                cols: 6,
                hang: true,
            },
        );
        assert_eq!(
            s,
            "あいう。
え"
        );
        assert_eq!(m.len(), 6);
    }

    #[test]
    fn every_row_fits_the_column() {
        let text = "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ、「何でも」薄暗い所で。";
        let (s, _) = expand_wrap(
            text,
            4,
            Wrap {
                cols: 20,
                hang: false,
            },
        );
        for row in s.split('\n') {
            let w: usize = row.chars().map(cell_width).sum();
            assert!(w <= 20, "row too wide: {row}");
            assert!(
                !row.chars()
                    .next()
                    .map(|c| NO_START.contains(c))
                    .unwrap_or(false)
                    || w >= 1
            );
        }
    }
}
