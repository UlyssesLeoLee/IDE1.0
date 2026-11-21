//! 行级 diff: 公共前后缀裁剪 + 有界 Myers O(ND)。
//!
//! 不变量:
//! - 内存有界: 编辑距离超过 [`MAX_EDIT_DISTANCE`] 时退化为"整段替换"(`coarse = true`),
//!   绝不无界分配。
//! - 比较使用**带行尾**的行 (`split_inclusive('\n')`), 所以 `a\n` 与 `a` 视为不同行,
//!   行尾 / 末行换行的改动不会被悄悄吞掉。

/// Myers 允许的最大编辑距离 (峰值内存约 `D^2 * 8` 字节 ≈ 18 MiB)。
pub const MAX_EDIT_DISTANCE: usize = 1500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Equal,
    Delete,
    Insert,
}

/// 编辑脚本中的一步: `a` / `b` 是该步**之前**两侧已消耗的行数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub op: Op,
    pub a: usize,
    pub b: usize,
}

/// 连续变更区: a 的 `[a0, a1)` 被 b 的 `[b0, b1)` 取代 (纯插入 a0 == a1, 纯删除 b0 == b1)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub a0: usize,
    pub a1: usize,
    pub b0: usize,
    pub b1: usize,
}

/// 按行切分并保留行尾。
pub fn split_lines(s: &str) -> Vec<&str> {
    s.split_inclusive('\n').collect()
}

fn step(op: Op, a: usize, b: usize) -> Step {
    Step { op, a, b }
}

/// 计算编辑脚本。返回 `(steps, coarse)`。
pub fn diff_lines(a: &[&str], b: &[&str]) -> (Vec<Step>, bool) {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf]
    {
        suf += 1;
    }
    let am = &a[pre..a.len() - suf];
    let bm = &b[pre..b.len() - suf];

    let mut steps = Vec::with_capacity(a.len().max(b.len()));
    for i in 0..pre {
        steps.push(step(Op::Equal, i, i));
    }
    let mut coarse = false;
    match myers(am, bm) {
        Some(mid) => {
            for s in mid {
                steps.push(step(s.op, s.a + pre, s.b + pre));
            }
        }
        None => {
            coarse = true;
            for i in 0..am.len() {
                steps.push(step(Op::Delete, pre + i, pre));
            }
            for j in 0..bm.len() {
                steps.push(step(Op::Insert, pre + am.len(), pre + j));
            }
        }
    }
    for i in 0..suf {
        steps.push(step(Op::Equal, a.len() - suf + i, b.len() - suf + i));
    }
    (steps, coarse)
}

/// 有界 Myers; 超出 [`MAX_EDIT_DISTANCE`] 返回 `None`。
fn myers(a: &[&str], b: &[&str]) -> Option<Vec<Step>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    if n == 0 && m == 0 {
        return Some(Vec::new());
    }
    if n == 0 {
        return Some((0..m as usize).map(|j| step(Op::Insert, 0, j)).collect());
    }
    if m == 0 {
        return Some((0..n as usize).map(|i| step(Op::Delete, i, 0)).collect());
    }
    let dmax = ((n + m) as usize).min(MAX_EDIT_DISTANCE) as isize;
    let off = dmax + 1;
    let mut v = vec![0isize; (2 * dmax + 3) as usize];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut found: Option<isize> = None;
    'outer: for d in 0..=dmax {
        let mut k = -d;
        while k <= d {
            let ki = (off + k) as usize;
            let mut x = if k == -d || (k != d && v[ki - 1] < v[ki + 1]) {
                v[ki + 1]
            } else {
                v[ki - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[ki] = x;
            if x >= n && y >= m {
                trace.push(v[(off - d) as usize..=(off + d) as usize].to_vec());
                found = Some(d);
                break 'outer;
            }
            k += 2;
        }
        trace.push(v[(off - d) as usize..=(off + d) as usize].to_vec());
    }
    let dfound = found?;

    let mut out: Vec<Step> = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (0..=dfound).rev() {
        if d == 0 {
            while x > 0 && y > 0 {
                x -= 1;
                y -= 1;
                out.push(step(Op::Equal, x as usize, y as usize));
            }
            break;
        }
        let vp = &trace[(d - 1) as usize];
        let at = |k: isize| vp[(k + d - 1) as usize];
        let k = x - y;
        let prev_k = if k == -d || (k != d && at(k - 1) < at(k + 1)) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = at(prev_k);
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            out.push(step(Op::Equal, x as usize, y as usize));
        }
        if x == prev_x {
            y -= 1;
            out.push(step(Op::Insert, x as usize, y as usize));
        } else {
            x -= 1;
            out.push(step(Op::Delete, x as usize, y as usize));
        }
    }
    out.reverse();
    Some(out)
}

/// 把编辑脚本折叠成连续变更区。
pub fn regions(steps: &[Step]) -> Vec<Region> {
    let mut out: Vec<Region> = Vec::new();
    let mut cur: Option<Region> = None;
    for s in steps {
        if s.op == Op::Equal {
            if let Some(r) = cur.take() {
                out.push(r);
            }
            continue;
        }
        let r = cur.get_or_insert(Region {
            a0: s.a,
            a1: s.a,
            b0: s.b,
            b1: s.b,
        });
        if s.op == Op::Delete {
            r.a1 = s.a + 1;
        } else {
            r.b1 = s.b + 1;
        }
    }
    if let Some(r) = cur {
        out.push(r);
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub op: Op,
    pub text: String,
    pub a: Option<usize>,
    pub b: Option<usize>,
    pub no_eol: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub a_start: usize,
    pub a_len: usize,
    pub b_start: usize,
    pub b_len: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub hunks: Vec<Hunk>,
    pub added: usize,
    pub removed: usize,
    pub identical: bool,
    pub coarse: bool,
}

fn display(l: &str) -> (String, bool) {
    let no_eol = !l.ends_with('\n');
    (l.trim_end_matches(['\n', '\r']).to_string(), no_eol)
}

/// 文本 diff, `context` = 每个 hunk 两侧保留的上下文行数。
pub fn diff_text(a: &str, b: &str, context: usize) -> Diff {
    let al = split_lines(a);
    let bl = split_lines(b);
    let (steps, coarse) = diff_lines(&al, &bl);
    let changed: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.op != Op::Equal)
        .map(|(i, _)| i)
        .collect();
    if changed.is_empty() {
        return Diff {
            hunks: Vec::new(),
            added: 0,
            removed: 0,
            identical: true,
            coarse,
        };
    }
    let mut windows: Vec<(usize, usize)> = Vec::new();
    for &i in &changed {
        let lo = i.saturating_sub(context);
        let hi = (i + context + 1).min(steps.len());
        match windows.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => windows.push((lo, hi)),
        }
    }
    let (mut added, mut removed) = (0, 0);
    let mut hunks = Vec::with_capacity(windows.len());
    for (lo, hi) in windows {
        let mut lines = Vec::with_capacity(hi - lo);
        let first = steps[lo];
        let (mut a_len, mut b_len) = (0, 0);
        for s in &steps[lo..hi] {
            match s.op {
                Op::Equal => {
                    let (t, ne) = display(al[s.a]);
                    lines.push(DiffLine {
                        op: Op::Equal,
                        text: t,
                        a: Some(s.a),
                        b: Some(s.b),
                        no_eol: ne,
                    });
                    a_len += 1;
                    b_len += 1;
                }
                Op::Delete => {
                    let (t, ne) = display(al[s.a]);
                    lines.push(DiffLine {
                        op: Op::Delete,
                        text: t,
                        a: Some(s.a),
                        b: None,
                        no_eol: ne,
                    });
                    a_len += 1;
                    removed += 1;
                }
                Op::Insert => {
                    let (t, ne) = display(bl[s.b]);
                    lines.push(DiffLine {
                        op: Op::Insert,
                        text: t,
                        a: None,
                        b: Some(s.b),
                        no_eol: ne,
                    });
                    b_len += 1;
                    added += 1;
                }
            }
        }
        hunks.push(Hunk {
            a_start: first.a,
            a_len,
            b_start: first.b,
            b_len,
            lines,
        });
    }
    Diff {
        hunks,
        added,
        removed,
        identical: false,
        coarse,
    }
}

/// 渲染 unified diff 文本 (`diff -u` 兼容的 hunk 头)。
pub fn unified(d: &Diff, a_name: &str, b_name: &str) -> String {
    if d.identical {
        return String::new();
    }
    let mut out = format!("--- {a_name}\n+++ {b_name}\n");
    for h in &d.hunks {
        let a_s = if h.a_len == 0 {
            h.a_start
        } else {
            h.a_start + 1
        };
        let b_s = if h.b_len == 0 {
            h.b_start
        } else {
            h.b_start + 1
        };
        out.push_str(&format!("@@ -{a_s},{} +{b_s},{} @@\n", h.a_len, h.b_len));
        for l in &h.lines {
            let tag = match l.op {
                Op::Equal => ' ',
                Op::Delete => '-',
                Op::Insert => '+',
            };
            out.push(tag);
            out.push_str(&l.text);
            out.push('\n');
            if l.no_eol {
                out.push_str("\\ No newline at end of file\n");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(a: &str, b: &str) -> String {
        let al = split_lines(a);
        let bl = split_lines(b);
        let (steps, _) = diff_lines(&al, &bl);
        let mut out = String::new();
        for s in steps {
            match s.op {
                Op::Equal => out.push_str(al[s.a]),
                Op::Delete => {}
                Op::Insert => out.push_str(bl[s.b]),
            }
        }
        out
    }

    #[test]
    fn identical_and_empty() {
        assert!(diff_text("", "", 3).identical);
        assert!(diff_text("a\nb\n", "a\nb\n", 3).identical);
        let d = diff_text("", "x\n", 3);
        assert_eq!((d.added, d.removed), (1, 0));
    }

    #[test]
    fn script_rebuilds_target_for_many_shapes() {
        let cases = [
            ("a\nb\nc\n", "a\nX\nc\n"),
            ("a\nb\nc\n", "c\nb\na\n"),
            ("", "a\nb\n"),
            ("a\nb\n", ""),
            ("a\nb", "a\nb\n"),
            ("a\r\nb\r\n", "a\nb\n"),
            ("1\n2\n3\n4\n5\n6\n7\n", "1\n3\n4\n5\n6\n6.5\n7\n"),
        ];
        for (a, b) in cases {
            assert_eq!(apply(a, b), b, "{a:?} -> {b:?}");
        }
    }

    #[test]
    fn line_ending_change_is_not_swallowed() {
        let d = diff_text("a\nb", "a\nb\n", 0);
        assert!(!d.identical);
        assert_eq!((d.added, d.removed), (1, 1));
    }

    #[test]
    fn unified_has_correct_hunk_header_and_context() {
        let d = diff_text("a\nb\nc\nd\ne\n", "a\nb\nX\nd\ne\n", 1);
        let u = unified(&d, "old", "new");
        assert!(u.contains("@@ -2,3 +2,3 @@"), "{u}");
        assert!(u.contains("-c\n+X\n"));
    }

    #[test]
    fn distant_changes_make_separate_hunks() {
        let a: String = (0..40).map(|i| format!("{i}\n")).collect();
        let mut bl: Vec<String> = (0..40).map(|i| format!("{i}\n")).collect();
        bl[2] = "two\n".into();
        bl[35] = "thirty-five\n".into();
        let d = diff_text(&a, &bl.concat(), 2);
        assert_eq!(d.hunks.len(), 2);
    }

    #[test]
    fn huge_distance_falls_back_to_coarse_but_stays_correct() {
        let a: String = (0..3000).map(|i| format!("a{i}\n")).collect();
        let b: String = (0..3000).map(|i| format!("b{i}\n")).collect();
        let d = diff_text(&a, &b, 0);
        assert!(d.coarse);
        assert_eq!((d.added, d.removed), (3000, 3000));
        assert_eq!(apply(&a, &b), b);
    }

    #[test]
    fn regions_fold_runs() {
        let al = split_lines("a\nb\nc\nd\n");
        let bl = split_lines("a\nX\nY\nd\n");
        let (steps, _) = diff_lines(&al, &bl);
        let r = regions(&steps);
        assert_eq!(
            r,
            vec![Region {
                a0: 1,
                a1: 3,
                b0: 1,
                b1: 3
            }]
        );
    }
}
