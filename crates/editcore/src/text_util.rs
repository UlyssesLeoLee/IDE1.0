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
