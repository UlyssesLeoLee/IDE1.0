//! 折叠块的边界 (zc / zM): 只算 [起始行, 结束行], 文字不动, 隐藏由前端负责。
//!
//! 块边界按两种规则找: 行内花括号多于闭括号 = 配到平衡为止; 否则按缩进 (下一个非空行更深才算块)。

use super::ctx::Ctx;
use super::req_res::EditRes;
use crate::text_util::indent_len;

/// 折叠: `all == false` 只折光标行起的那一个块, 否则文档里每个能折的块 (含嵌套)。
/// `req.rows` 要从起点行连续交到文档末尾。
pub(super) fn run(cx: &Ctx, all: bool) -> EditRes {
    let mut rs: Vec<&(usize, String)> = cx.req.rows.iter().collect();
    rs.sort_by_key(|r| r.0);
    let first = rs.first().map(|r| r.0).unwrap_or(0);
    let lines: Vec<Vec<char>> = rs.iter().map(|(_, s)| s.chars().collect()).collect();
    let mut folds: Vec<[usize; 2]> = Vec::new();
    if all {
        for i in 0..lines.len() {
            if let Some(e) = fold_end(&lines, i) {
                folds.push([i + first, e + first]);
            }
        }
    } else if cx.row >= first {
        if let Some(e) = fold_end(&lines, cx.row - first) {
            folds.push([cx.row, e + first]);
        }
    }
    let tip = folds.is_empty().then(|| "这里没有可折叠的块".to_string());
    EditRes { cursor: [cx.row, cx.col], folds: Some(folds), tip, ..Default::default() }
}

/// 一行里 `c` ('{' 或 '}') 的个数, 跳过字符串 "..." / 字符 '{' / 行注释 // 里的 (它们不开块也不收块)
fn count_braces(l: &[char], c: char) -> isize {
    let (mut n, mut i, mut in_str) = (0isize, 0usize, false);
    while i < l.len() {
        let ch = l[i];
        if in_str {
            if ch == '\\' {
                i += 1;
            } else if ch == '"' {
                in_str = false;
            }
        } else if ch == '"' {
            in_str = true;
        } else if ch == '/' && l.get(i + 1) == Some(&'/') {
            break;
        } else if ch == '\'' && l.get(i + 2) == Some(&'\'') {
            i += 2; // 'x' 形式的字符字面量
        } else if ch == c {
            n += 1;
        }
        i += 1;
    }
    n
}

/// 从第 `start` 行起的折叠块结束行 (含); 没有可折叠的块 = None
fn fold_end(lines: &[Vec<char>], start: usize) -> Option<usize> {
    let line = lines.get(start)?;
    let end = if count_braces(line, '{') > count_braces(line, '}') { brace_block_end(lines, start) } else { indent_block_end(lines, start)? };
    (end > start).then_some(end)
}

/// 花括号块: 从 `start` 行数到括号平衡; 一直配不上 = 到文档末尾
fn brace_block_end(lines: &[Vec<char>], start: usize) -> usize {
    let mut depth = 0isize;
    for (r, l) in lines.iter().enumerate().skip(start) {
        depth += count_braces(l, '{') - count_braces(l, '}');
        if r > start && depth <= 0 {
            return r;
        }
    }
    lines.len() - 1
}

/// 缩进块: 下一个非空行比本行深才算块; 块延伸到缩进回到本行或更浅之前的最后一个非空行 (末尾空行不吞)
fn indent_block_end(lines: &[Vec<char>], start: usize) -> Option<usize> {
    let is_blank = |l: &Vec<char>| l.iter().all(|c| c.is_whitespace());
    if is_blank(&lines[start]) {
        return None;
    }
    let base = indent_len(&lines[start]);
    let mut end = None;
    for (r, l) in lines.iter().enumerate().skip(start + 1) {
        if is_blank(l) {
            continue;
        }
        if indent_len(l) <= base {
            break;
        }
        end = Some(r);
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(src: &[&str]) -> Vec<Vec<char>> {
        src.iter().map(|s| s.chars().collect()).collect()
    }

    #[test]
    fn braces_inside_literals_and_comments_do_not_count() {
        let l: Vec<char> = "let s = \"}\"; let c = '{'; // {".chars().collect();
        assert_eq!((count_braces(&l, '{'), count_braces(&l, '}')), (0, 0));
        let l: Vec<char> = "if (a) { b(\"\\\"}\") }".chars().collect();
        assert_eq!((count_braces(&l, '{'), count_braces(&l, '}')), (1, 1), "转义的引号不结束字符串");
    }

    #[test]
    fn brace_block_runs_to_balance_or_to_the_end() {
        let src = lines(&["fn a() {", "    if x {", "    }", "}", "tail"]);
        assert_eq!(fold_end(&src, 0), Some(3));
        assert_eq!(fold_end(&src, 1), Some(2));
        assert_eq!(fold_end(&lines(&["if (a) {", "  b();"]), 0), Some(1), "配不上 = 到文档末尾");
    }

    #[test]
    fn indent_block_needs_a_deeper_next_line_and_keeps_trailing_blanks_out() {
        let src = lines(&["def f():", "    a", "", "    b", "", "x"]);
        assert_eq!(fold_end(&src, 0), Some(3));
        assert_eq!(fold_end(&src, 1), None, "同级的下一行不是块");
        assert_eq!(fold_end(&lines(&["   "]), 0), None, "空白行不能折");
    }
}
