//! 按字符 (码点, 不是字节) 下标处理文本的小工具 —— 编辑命令的列都是字符数, 这里统一"取子串 / 缩进长度"的写法。
//!
//! 注意: `buffer.rs::slice_chars` 面向另一种数据结构 (带行缓冲), 不要和这里的混用。

/// 把一行拆成字符数组 (按字符下标取子串时用)
pub fn chars_of(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// 取 `cs[a..b]` 拼成字符串; 越界的下标夹到长度内 (`b` 可以传 `usize::MAX` 表示到行尾); 起点不在终点之前 = 空串, 不 panic
pub fn slice(cs: &[char], a: usize, b: usize) -> String {
    let (a, b) = (a.min(cs.len()), b.min(cs.len()));
    if a >= b {
        return String::new();
    }
    cs[a..b].iter().collect()
}

/// 行首连续的空格 / TAB 的个数 (自动缩进、折叠都按它算)
pub fn indent_len(cs: &[char]) -> usize {
    cs.iter().take_while(|c| **c == ' ' || **c == '\t').count()
}

/// 东亚宽字符占 2 列 (CJK / 假名 / 全角形), 其余字符占 1 列
pub fn char_display_width(c: char) -> usize {
    let n = c as u32;
    if (0x1100..=0x115F).contains(&n)
        || (0x2E80..=0xA4CF).contains(&n)
        || (0xAC00..=0xD7A3).contains(&n)
        || (0xF900..=0xFAFF).contains(&n)
        || (0xFE30..=0xFE6F).contains(&n)
        || (0xFF00..=0xFF60).contains(&n)
        || (0xFFE0..=0xFFE6).contains(&n)
    {
        2
    } else {
        1
    }
}

/// 计算字符切片 `l` 的总显示列宽（`\t` 按 `tab` 宽对齐）
pub fn chars_display_width(l: &[char], tab: usize) -> usize {
    let tab = tab.max(1);
    let mut x = 0usize;
    for &ch in l {
        if ch == '\t' {
            x += tab - x % tab;
        } else {
            x += char_display_width(ch);
        }
    }
    x
}

/// 0 起算的显示列区间 `[vcol0, vcol1]` -> 字符切片 `l` 对应的字符列区间 `(char_c0, char_c1)`。
/// 若 `extend_past_eol == true`，越过行尾的部分按 1 列/格顺延到 `l.len()` 之后；否则夹在 `l.len()` 内。
pub fn display_range_to_char_cols_slice(
    l: &[char],
    vcol0: usize,
    vcol1: usize,
    tab: usize,
    extend_past_eol: bool,
) -> (usize, usize) {
    let tab = tab.max(1);
    let len = l.len();
    let mut x = 0usize;
    let mut c0 = None;
    for (i, &ch) in l.iter().enumerate() {
        let w = if ch == '\t' {
            tab - x % tab
        } else {
            char_display_width(ch)
        };
        if c0.is_none() && x + w > vcol0 {
            c0 = Some(i);
        }
        x += w;
    }
    let line_disp_w = x;
    let char_c0 = c0.unwrap_or_else(|| {
        if extend_past_eol {
            len + vcol0.saturating_sub(line_disp_w)
        } else {
            len
        }
    });
    if vcol1 <= vcol0 {
        return (char_c0, char_c0);
    }
    let mut x2 = 0usize;
    let mut c1 = None;
    for (i, &ch) in l.iter().enumerate() {
        let w = if ch == '\t' {
            tab - x2 % tab
        } else {
            char_display_width(ch)
        };
        if x2 >= vcol1 {
            c1 = Some(i);
            break;
        }
        if x2 + w >= vcol1 {
            c1 = Some(i + 1);
            break;
        }
        x2 += w;
    }
    let char_c1 = c1.unwrap_or_else(|| {
        if extend_past_eol {
            len + vcol1.saturating_sub(line_disp_w)
        } else {
            len
        }
    });
    (char_c0, char_c1.max(char_c0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_clamps_both_ends() {
        let cs = chars_of("日本語abc");
        assert_eq!(slice(&cs, 0, 2), "日本");
        assert_eq!(slice(&cs, 2, usize::MAX), "語abc");
        assert_eq!(slice(&cs, 99, 100), "");
        assert_eq!(slice(&cs, 3, 1), "", "起点在终点之后 = 空 (不 panic)");
    }

    #[test]
    fn indent_counts_spaces_and_tabs_only() {
        assert_eq!(indent_len(&chars_of(" \t x")), 3);
        assert_eq!(indent_len(&chars_of("x ")), 0);
        assert_eq!(indent_len(&chars_of("\u{3000}x")), 0, "全角空格不算缩进");
    }
}

/// `路径` / `路径:行` / `路径:行:列` (行、列从 1 起) 拆开; 单个字母 + 冒号 (Windows 盘符) 不当作行号。无位置时 line = 0
pub fn split_path_line_col(spec: &str) -> (String, usize, usize) {
    let spec = spec.trim();
    let mut parts = spec.rsplitn(3, ':').collect::<Vec<_>>(); // 从右边最多拆两段
    parts.reverse();
    let num = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let as_n = |s: &str| s.parse::<usize>().unwrap_or(0);
    // 先试 路径:行:列, 再试 路径:行
    if parts.len() == 3
        && num(parts[1])
        && num(parts[2])
        && !parts[0].is_empty()
        && parts[0].chars().count() > 1
    {
        return (parts[0].to_string(), as_n(parts[1]), as_n(parts[2]));
    }
    if let Some((head, last)) = spec.rsplit_once(':') {
        if num(last) && !head.is_empty() && head.chars().count() > 1 {
            return (head.to_string(), as_n(last), 1);
        }
    }
    (spec.to_string(), 0, 0)
}

#[cfg(test)]
mod path_line_col_tests {
    use super::split_path_line_col as sp;

    #[test]
    fn forms() {
        assert_eq!(sp("a.txt"), ("a.txt".into(), 0, 0));
        assert_eq!(sp("src/a.rs:12"), ("src/a.rs".into(), 12, 1));
        assert_eq!(sp("src/a.rs:12:5"), ("src/a.rs".into(), 12, 5));
        assert_eq!(
            sp("C:/x/a.txt"),
            ("C:/x/a.txt".into(), 0, 0),
            "盘符不是行号"
        );
        assert_eq!(sp("C:/x/a.txt:3:4"), ("C:/x/a.txt".into(), 3, 4));
        assert_eq!(sp("D:5"), ("D:5".into(), 0, 0), "单字母 + 数字当作路径");
        assert_eq!(sp(":7"), (":7".into(), 0, 0));
    }
}
