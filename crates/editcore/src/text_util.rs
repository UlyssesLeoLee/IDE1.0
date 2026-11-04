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

/// `路径` / `路径:行` / `路径:行:列` (行、列从 1 起) 拆开; 单个字母 + 冒号 (Windows 盘符) 不当作行号。无位置时 line = 0
pub fn split_path_line_col(spec: &str) -> (String, usize, usize) {
    let spec = spec.trim();
    let mut parts = spec.rsplitn(3, ':').collect::<Vec<_>>(); // 从右边最多拆两段
    parts.reverse();
    let num = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let as_n = |s: &str| s.parse::<usize>().unwrap_or(0);
    // 先试 路径:行:列, 再试 路径:行
    if parts.len() == 3 && num(parts[1]) && num(parts[2]) && !parts[0].is_empty() && parts[0].chars().count() > 1 {
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
        assert_eq!(sp("C:/x/a.txt"), ("C:/x/a.txt".into(), 0, 0), "盘符不是行号");
        assert_eq!(sp("C:/x/a.txt:3:4"), ("C:/x/a.txt".into(), 3, 4));
        assert_eq!(sp("D:5"), ("D:5".into(), 0, 0), "单字母 + 数字当作路径");
        assert_eq!(sp(":7"), (":7".into(), 0, 0));
    }
}
