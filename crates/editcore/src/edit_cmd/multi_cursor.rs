//! 多光标 (Ctrl+点击 / Alt+点击 添加) 的键入与退格。
//!
//! 做法: 光标所在的各行 (退格在行首要并到上一行, 也算) 按**连续的一段**拍平成一串字符 (行间 \n),
//! 在偏移上统一编辑, 再按 \n 切回行、把偏移换回 (行, 列)。偏移是在同一个坐标系里算的, 所以同一行有多个光标、
//! 退格并行都不会互相错位。不连续的几段各出一个补丁 (`patches`, 前端从下往上套), 光标隔多远都不用把中间的行发过来。

use super::ctx::Ctx;
use super::req_res::{EditRes, RowPatch};

/// 对每个光标做的事
#[derive(Clone, Copy)]
pub(super) enum MultiOp<'a> {
    /// 在光标处插入这段文字
    Insert(&'a [char]),
    /// 删掉光标前的一个字符 (行首 = 前一行的换行, 即与上一行合并)
    Backspace,
}

pub(super) fn run(cx: &Ctx, op: MultiOp) -> EditRes {
    if matches!(op, MultiOp::Insert(ins) if ins.is_empty()) {
        return cx.nothing("");
    }
    let at = match normalize_cursors(cx) {
        Ok(v) => v,
        Err(why) => return cx.nothing(why),
    };
    let touched = touched_rows(&at, matches!(op, MultiOp::Backspace));
    if touched.iter().any(|&r| !cx.has_row(r)) {
        return cx.nothing("多光标涉及的行没有交全");
    }

    let mut patches: Vec<RowPatch> = Vec::new();
    let mut cursors: Vec<[usize; 2]> = Vec::new();
    let mut removed = 0isize; // 前面各段因为并行少掉的行数 (插入带换行的文字时是负的)
    for (s, e) in contiguous_segments(&touched) {
        let rows: Vec<Vec<char>> = (s..=e).map(|r| cx.line(r)).collect();
        let (flat, starts) = flatten(&rows);
        let offs: Vec<usize> = at.iter().filter(|c| c[0] >= s && c[0] <= e).map(|c| starts[c[0] - s] + c[1]).collect();
        let (edited, new_offs) = apply_op(&flat, &offs, op);
        let text: String = edited.iter().collect();
        let insert: Vec<String> = text.split('\n').map(String::from).collect();
        let base = (s as isize - removed) as usize;
        cursors.extend(offsets_to_cursors(&insert, &new_offs, base));
        removed += (e - s + 1) as isize - insert.len() as isize;
        patches.push((s, e - s + 1, insert));
    }
    if unchanged(cx, &patches) {
        return cx.nothing(""); // 什么也没变 (例如都在 (0,0) 退格)
    }
    cursors.sort();
    cursors.dedup();
    let tip = match op {
        MultiOp::Backspace => format!("{} 个光标删除", at.len()),
        MultiOp::Insert(ins) => format!("{} 个光标插入 {}", at.len(), ins.iter().collect::<String>()),
    };
    let mut res = EditRes::nothing(cursors[0], &tip);
    res.changed = true;
    res.patches = Some(patches);
    res.cursors = Some(cursors);
    res
}

/// 光标: 行夹进文档, 列夹进行长, 去重、升序。涉及的行没交全或不足两个光标 = Err(给用户的提示, 可为空) —— 什么也不做
fn normalize_cursors(cx: &Ctx) -> Result<Vec<[usize; 2]>, &'static str> {
    let mut at: Vec<[usize; 2]> = Vec::with_capacity(cx.req.cursors.len());
    for c in &cx.req.cursors {
        let r = c[0].min(cx.last_row);
        if !cx.has_row(r) {
            return Err("多光标涉及的行没有交全");
        }
        at.push([r, c[1].min(cx.line(r).len())]);
    }
    at.sort();
    at.dedup();
    if at.len() < 2 {
        return Err("");
    }
    Ok(at)
}

/// 要交上来的行: 光标所在的行; 退格在行首还要上一行。升序、去重
fn touched_rows(at: &[[usize; 2]], backspace: bool) -> Vec<usize> {
    let mut touched = Vec::new();
    for c in at {
        touched.push(c[0]);
        if backspace && c[0] > 0 && c[1] == 0 {
            touched.push(c[0] - 1);
        }
    }
    touched.sort();
    touched.dedup();
    touched
}

/// 升序的行号 -> 连续的几段 (起, 止)
fn contiguous_segments(rows: &[usize]) -> Vec<(usize, usize)> {
    let mut segs: Vec<(usize, usize)> = Vec::new();
    for &r in rows {
        match segs.last_mut() {
            Some(s) if s.1 + 1 == r => s.1 = r,
            _ => segs.push((r, r)),
        }
    }
    segs
}

/// 几行拍平成一串 (行间 \n); 同时给出每行在这一串里的起点偏移
fn flatten(rows: &[Vec<char>]) -> (Vec<char>, Vec<usize>) {
    let mut flat = Vec::new();
    let mut starts = Vec::with_capacity(rows.len());
    for (i, l) in rows.iter().enumerate() {
        if i > 0 {
            flat.push('\n');
        }
        starts.push(flat.len());
        flat.extend_from_slice(l);
    }
    (flat, starts)
}

/// 在偏移 `offs` (升序、不重复) 上编辑, 返回 (编辑后的字符串, 每个光标编辑后的新偏移)
fn apply_op(flat: &[char], offs: &[usize], op: MultiOp) -> (Vec<char>, Vec<usize>) {
    let mut out = Vec::with_capacity(flat.len() + 8);
    let mut new_offs = Vec::with_capacity(offs.len());
    match op {
        MultiOp::Insert(ins) => {
            let mut prev = 0;
            for (i, &o) in offs.iter().enumerate() {
                out.extend_from_slice(&flat[prev..o]);
                out.extend_from_slice(ins);
                new_offs.push(o + ins.len() * (i + 1)); // 前面的光标(含自己)都插了一份
                prev = o;
            }
            out.extend_from_slice(&flat[prev..]);
        }
        MultiOp::Backspace => {
            let dels: Vec<usize> = offs.iter().filter(|&&o| o > 0).map(|&o| o - 1).collect();
            let mut prev = 0;
            for &d in &dels {
                out.extend_from_slice(&flat[prev..d]);
                prev = d + 1;
            }
            out.extend_from_slice(&flat[prev..]);
            new_offs.extend(offs.iter().map(|&o| o - dels.iter().filter(|&&d| d < o).count()));
        }
    }
    (out, new_offs)
}

/// 编辑后的各行 `lines` (这一段从新文档的第 `base` 行开始) 里, 把偏移换回 (行, 列)
fn offsets_to_cursors(lines: &[String], offs: &[usize], base: usize) -> Vec<[usize; 2]> {
    let mut line_starts = Vec::with_capacity(lines.len());
    let mut acc = 0;
    for l in lines {
        line_starts.push(acc);
        acc += l.chars().count() + 1;
    }
    offs.iter()
        .map(|&o| {
            let i = line_starts.partition_point(|&x| x <= o) - 1;
            [base + i, o - line_starts[i]]
        })
        .collect()
}

/// 补丁套上去之后和原来一样吗 (行数相同且每行文字相同)
fn unchanged(cx: &Ctx, patches: &[RowPatch]) -> bool {
    patches.iter().all(|(s, n, ins)| ins.len() == *n && (*s..*s + *n).zip(ins).all(|(r, l)| cx.line(r).iter().collect::<String>() == *l))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn segments_group_consecutive_rows() {
        assert_eq!(contiguous_segments(&[1, 2, 3, 7, 9, 10]), vec![(1, 3), (7, 7), (9, 10)]);
        assert!(contiguous_segments(&[]).is_empty());
    }

    #[test]
    fn touched_rows_add_the_previous_row_only_for_backspace_at_column_zero() {
        assert_eq!(touched_rows(&[[3, 0], [3, 2], [9, 1]], true), vec![2, 3, 9]);
        assert_eq!(touched_rows(&[[3, 0], [9, 1]], false), vec![3, 9]);
        assert_eq!(touched_rows(&[[0, 0]], true), vec![0], "第 0 行没有上一行");
    }

    #[test]
    fn flatten_records_row_starts() {
        let (flat, starts) = flatten(&[cs("ab"), cs(""), cs("cd")]);
        assert_eq!((flat.iter().collect::<String>(), starts), ("ab\n\ncd".to_string(), vec![0, 3, 4]));
    }

    #[test]
    fn insert_shifts_later_cursors_by_every_earlier_insert() {
        let ins = cs("QQ");
        let (out, offs) = apply_op(&cs("abcdef"), &[1, 3], MultiOp::Insert(&ins));
        assert_eq!((out.iter().collect::<String>(), offs), ("aQQbcQQdef".to_string(), vec![3, 7]));
    }

    #[test]
    fn backspace_merges_lines_and_collapses_touching_cursors() {
        let (out, offs) = apply_op(&cs("abc\ndef"), &[3, 4], MultiOp::Backspace);
        assert_eq!((out.iter().collect::<String>(), offs), ("abdef".to_string(), vec![2, 2]), "删 c 与换行后, 两个光标落在同一处");
        let (same, none) = apply_op(&cs("ab"), &[0], MultiOp::Backspace);
        assert_eq!((same.iter().collect::<String>(), none), ("ab".to_string(), vec![0]), "偏移 0 没有可删的");
    }

    #[test]
    fn offsets_map_back_to_rows_even_when_the_text_contains_newlines() {
        let lines: Vec<String> = ["ab", "", "cde"].iter().map(|s| s.to_string()).collect();
        assert_eq!(offsets_to_cursors(&lines, &[0, 2, 3, 4, 7], 10), vec![[10, 0], [10, 2], [11, 0], [12, 0], [12, 3]]);
    }
}
