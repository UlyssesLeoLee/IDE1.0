//! 拖放选中文字: 把选区拖到别处 = 移动, 按住 Ctrl 拖 = 复制。
//! 编辑本身是一个纯函数 (`drop_lines`, 有单测), `Doc::drop_selection` 把结果作为一步撤销套到文档上。

use crate::doc::Doc;

/// 鼠标拖放的进行状态 (界面用): `started` = 已经真的拖动了 (只是点一下选区里面不算), `drop` = 现在的落点
#[derive(Default, Clone, Copy, Debug)]
pub struct TextDrag {
    pub started: bool,
    pub drop: Option<[usize; 2]>,
    /// 拖到一半按了 Esc: 松手时什么也不做
    pub cancelled: bool,
}

/// 行 / 列 (字符下标) -> 把 `lines` 用 `\n` 连起来之后的字符偏移
fn offset(lines: &[String], pos: [usize; 2]) -> usize {
    let mut off = 0;
    for (i, l) in lines.iter().enumerate() {
        if i == pos[0] {
            return off + pos[1].min(l.chars().count());
        }
        off += l.chars().count() + 1;
    }
    off
}

/// 字符偏移 -> (行, 列)
fn position(lines: &[String], mut off: usize) -> [usize; 2] {
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        if off <= n {
            return [i, off];
        }
        off -= n + 1;
    }
    let last = lines.len().saturating_sub(1);
    [last, lines.last().map(|l| l.chars().count()).unwrap_or(0)]
}

/// 在 `lines` (从文档第 `base` 行起的一段, 必须覆盖选区和落点) 里把选区 `src` 移到 / 复制到 `drop`。
/// 返回 (新的这一段行, 新文字所在的范围 [r1,c1,r2,c2], 行号相对这一段)。
/// 移动时落点在选区内部 (含两端) = None (什么也不做); 选区为空 = None。
pub fn drop_lines(
    lines: &[String],
    src: [usize; 4],
    drop: [usize; 2],
    copy: bool,
) -> Option<(Vec<String>, [usize; 4])> {
    let s = offset(lines, [src[0], src[1]]);
    let e = offset(lines, [src[2], src[3]]);
    let d = offset(lines, drop);
    if e <= s {
        return None;
    }
    if !copy && d >= s && d <= e {
        return None;
    }
    let chars: Vec<char> = lines.join("\n").chars().collect();
    if e > chars.len() {
        return None;
    }
    let text = &chars[s..e];
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + if copy { text.len() } else { 0 });
    let new_start;
    if copy {
        out.extend_from_slice(&chars[..d]);
        new_start = d;
        out.extend_from_slice(text);
        out.extend_from_slice(&chars[d..]);
    } else if d >= e {
        out.extend_from_slice(&chars[..s]);
        out.extend_from_slice(&chars[e..d]);
        new_start = out.len();
        out.extend_from_slice(text);
        out.extend_from_slice(&chars[d..]);
    } else {
        out.extend_from_slice(&chars[..d]);
        new_start = out.len();
        out.extend_from_slice(text);
        out.extend_from_slice(&chars[d..s]);
        out.extend_from_slice(&chars[e..]);
    }
    let joined: String = out.iter().collect();
    let new_lines: Vec<String> = joined.split('\n').map(str::to_string).collect();
    let a = position(&new_lines, new_start);
    let b = position(&new_lines, new_start + text.len());
    Some((new_lines, [a[0], a[1], b[0], b[1]]))
}

impl Doc {
    /// 把当前选区拖放到 `drop`。成功 = true; 只读 / 没有选区 / 落点在选区里 = false
    pub fn drop_selection(&mut self, drop: [usize; 2], copy: bool) -> bool {
        if self.readonly {
            self.tip = "只读: 不能修改 (编辑 → 只读 可以关掉)".into();
            return false;
        }
        let Some(src) = self.sel else { return false };
        let last = self.lines.len() - 1;
        let drop = [
            drop[0].min(last),
            drop[1].min(self.lines[drop[0].min(last)].chars().count()),
        ];
        let lo = src[0].min(drop[0]);
        let hi = src[2].max(drop[0]).min(last);
        let slice = self.lines[lo..=hi].to_vec();
        let rel = |p: [usize; 2]| [p[0] - lo, p[1]];
        let Some((new_lines, range)) = drop_lines(
            &slice,
            [src[0] - lo, src[1], src[2] - lo, src[3]],
            rel(drop),
            copy,
        ) else {
            return false;
        };
        let (a, b) = ([range[0] + lo, range[1]], [range[2] + lo, range[3]]);
        self.box_sel = None;
        self.extra.clear();
        self.replace_rows(lo, hi - lo + 1, new_lines, b);
        self.sel = Some([a[0], a[1], b[0], b[1]]);
        self.anchor = Some(a);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn move_forward_on_one_line() {
        // "abcdef": 把 "bc" (1..3) 拖到 5 (e 之后)
        let (l, r) = drop_lines(&v(&["abcdef"]), [0, 1, 0, 3], [0, 5], false).unwrap();
        assert_eq!(l, v(&["adebcf"]));
        assert_eq!(r, [0, 3, 0, 5]);
    }

    #[test]
    fn move_backward_on_one_line() {
        let (l, r) = drop_lines(&v(&["abcdef"]), [0, 3, 0, 5], [0, 1], false).unwrap();
        assert_eq!(l, v(&["adebcf"]));
        assert_eq!(r, [0, 1, 0, 3]);
    }

    #[test]
    fn copy_keeps_original() {
        let (l, r) = drop_lines(&v(&["abcdef"]), [0, 0, 0, 2], [0, 6], true).unwrap();
        assert_eq!(l, v(&["abcdefab"]));
        assert_eq!(r, [0, 6, 0, 8]);
    }

    #[test]
    fn move_inside_selection_is_a_noop_but_copy_is_not() {
        assert!(drop_lines(&v(&["abcdef"]), [0, 1, 0, 4], [0, 2], false).is_none());
        assert!(drop_lines(&v(&["abcdef"]), [0, 1, 0, 4], [0, 1], false).is_none());
        assert!(drop_lines(&v(&["abcdef"]), [0, 1, 0, 4], [0, 4], false).is_none());
        assert!(drop_lines(&v(&["abcdef"]), [0, 1, 0, 4], [0, 2], true).is_some());
        assert!(drop_lines(&v(&["abcdef"]), [0, 2, 0, 2], [0, 4], false).is_none());
    }

    #[test]
    fn multi_line_selection_moves_across_lines() {
        let lines = v(&["ab", "cd", "ef"]);
        // 选 b..c (含换行) 拖到末尾
        let (l, r) = drop_lines(&lines, [0, 1, 1, 1], [2, 2], false).unwrap();
        assert_eq!(l, v(&["ad", "efb", "c"]));
        assert_eq!(r, [1, 2, 2, 1]);
    }

    #[test]
    fn doc_drop_is_one_undo_step() {
        let mut d = Doc::untitled(1, 1);
        d.lines = v(&["hello world", "second"]);
        d.sel = Some([0, 0, 0, 5]);
        d.cursor = [0, 5];
        assert!(d.drop_selection([1, 6], false));
        assert_eq!(d.lines, v(&[" world", "secondhello"]));
        assert_eq!(d.sel, Some([1, 6, 1, 11]));
        d.undo();
        assert_eq!(d.lines, v(&["hello world", "second"]));
        d.readonly = true;
        d.sel = Some([0, 0, 0, 5]);
        assert!(!d.drop_selection([1, 2], false));
    }
}
