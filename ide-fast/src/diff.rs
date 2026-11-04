//! 比较两份文档: 按行做 LCS 差异, 产出「差异块」(hunk) 和两侧的逐行标记 (行号槽里画色条)。
//! 纯函数, 不碰界面。太大的文档 (中间不同的部分超过 `MAX_CELLS` 格) 退化成一个整块替换, 保证不卡。

/// 超过这个格数 (中段行数之积) 就不做 LCS, 直接当一整块不同
const MAX_CELLS: usize = 4_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    /// 左文档的起始行 / 行数 (0 起)
    pub a_start: usize,
    pub a_len: usize,
    pub b_start: usize,
    pub b_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mark {
    #[default]
    Same,
    Added,
    Changed,
    /// 这一侧有行、对侧没有 (整行画红条)
    Removed,
    /// 这一侧在此处缺了对侧的行: 在该位置的行上画一小段红色缺口
    Gap,
}

/// 逐行比较, 返回按位置排序的差异块
pub fn diff_lines(a: &[String], b: &[String]) -> Vec<Hunk> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..]
        .iter()
        .rev()
        .zip(b[pre..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    if am.is_empty() && bm.is_empty() {
        return vec![];
    }
    if am.is_empty() || bm.is_empty() || am.len().saturating_mul(bm.len()) > MAX_CELLS {
        return vec![Hunk {
            a_start: pre,
            a_len: am.len(),
            b_start: pre,
            b_len: bm.len(),
        }];
    }
    // lcs[i][j] = am[i..] 与 bm[j..] 的最长公共子序列长度
    let (n, m) = (am.len(), bm.len());
    let w = m + 1;
    let mut lcs = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * w + j] = if am[i] == bm[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }
    let mut out: Vec<Hunk> = vec![];
    let (mut i, mut j) = (0, 0);
    // 当前未闭合的差异块起点
    let mut open: Option<(usize, usize)> = None;
    let close = |open: &mut Option<(usize, usize)>, i: usize, j: usize, out: &mut Vec<Hunk>| {
        if let Some((si, sj)) = open.take() {
            out.push(Hunk {
                a_start: pre + si,
                a_len: i - si,
                b_start: pre + sj,
                b_len: j - sj,
            });
        }
    };
    while i < n || j < m {
        if i < n && j < m && am[i] == bm[j] {
            close(&mut open, i, j, &mut out);
            i += 1;
            j += 1;
            continue;
        }
        open.get_or_insert((i, j));
        if j >= m || (i < n && lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    close(&mut open, i, j, &mut out);
    out
}

/// 两侧的逐行标记: (左, 右)。`Removed` 标本侧多出的行, `Gap` 标本侧缺行的位置
pub fn marks(hunks: &[Hunk], a_len: usize, b_len: usize) -> (Vec<Mark>, Vec<Mark>) {
    let (mut l, mut r) = (vec![Mark::Same; a_len], vec![Mark::Same; b_len]);
    for h in hunks {
        let kind_a = if h.b_len == 0 {
            Mark::Removed
        } else {
            Mark::Changed
        };
        let kind_b = if h.a_len == 0 {
            Mark::Added
        } else {
            Mark::Changed
        };
        for row in h.a_start..h.a_start + h.a_len {
            if let Some(m) = l.get_mut(row) {
                *m = kind_a;
            }
        }
        for row in h.b_start..h.b_start + h.b_len {
            if let Some(m) = r.get_mut(row) {
                *m = kind_b;
            }
        }
        // 这一侧没有行 (纯增 / 纯删): 在该位置的行上标 Gap, 表示「这里少了东西」
        if h.a_len == 0 {
            if let Some(m) = l.get_mut(h.a_start).filter(|_| h.a_start < a_len) {
                if *m == Mark::Same {
                    *m = Mark::Gap;
                }
            }
        }
        if h.b_len == 0 {
            if let Some(m) = r.get_mut(h.b_start).filter(|_| h.b_start < b_len) {
                if *m == Mark::Same {
                    *m = Mark::Gap;
                }
            }
        }
    }
    (l, r)
}

/// 下一个 / 上一个差异块 (按右侧行号, 到头回绕); `row` = 当前行
pub fn step(hunks: &[Hunk], row: usize, forward: bool, right: bool) -> Option<usize> {
    let start = |h: &Hunk| if right { h.b_start } else { h.a_start };
    if hunks.is_empty() {
        return None;
    }
    if forward {
        hunks
            .iter()
            .map(start)
            .find(|&s| s > row)
            .or_else(|| hunks.first().map(start))
    } else {
        hunks
            .iter()
            .map(start)
            .rev()
            .find(|&s| s < row)
            .or_else(|| hunks.last().map(start))
    }
}

/// 当前比较的状态 (放在 App 里): 两份文档的 id、版本号 (变了就重算) 与结果
pub struct DiffView {
    pub left: u64,
    pub right: u64,
    revs: (u64, u64),
    pub hunks: Vec<Hunk>,
    pub left_marks: Vec<Mark>,
    pub right_marks: Vec<Mark>,
}

impl DiffView {
    pub fn new(left: u64, right: u64, a: &[String], b: &[String], revs: (u64, u64)) -> Self {
        let hunks = diff_lines(a, b);
        let (left_marks, right_marks) = marks(&hunks, a.len(), b.len());
        DiffView {
            left,
            right,
            revs,
            hunks,
            left_marks,
            right_marks,
        }
    }

    /// 文档改过 (版本号变了) 就重算
    pub fn refresh(&mut self, a: &[String], b: &[String], revs: (u64, u64)) {
        if self.revs != revs {
            *self = DiffView::new(self.left, self.right, a, b, revs);
        }
    }

    /// 某份文档的逐行标记 (不在比较里 = None)
    pub fn marks_of(&self, doc_id: u64) -> Option<&[Mark]> {
        if doc_id == self.left {
            Some(&self.left_marks)
        } else if doc_id == self.right {
            Some(&self.right_marks)
        } else {
            None
        }
    }

    pub fn summary(&self) -> String {
        if self.hunks.is_empty() {
            "两份文档完全相同".into()
        } else {
            format!("{} 处差异", self.hunks.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        if s.is_empty() {
            vec![]
        } else {
            s.split(',').map(str::to_string).collect()
        }
    }

    #[test]
    fn identical_docs_have_no_hunks() {
        assert!(diff_lines(&v("a,b,c"), &v("a,b,c")).is_empty());
    }

    #[test]
    fn insert_delete_and_change_are_told_apart() {
        // 中间插入 x
        let h = diff_lines(&v("a,b,c"), &v("a,x,b,c"));
        assert_eq!(
            h,
            vec![Hunk {
                a_start: 1,
                a_len: 0,
                b_start: 1,
                b_len: 1
            }]
        );
        // 删除 b
        let h = diff_lines(&v("a,b,c"), &v("a,c"));
        assert_eq!(
            h,
            vec![Hunk {
                a_start: 1,
                a_len: 1,
                b_start: 1,
                b_len: 0
            }]
        );
        // b 改成 y
        let h = diff_lines(&v("a,b,c"), &v("a,y,c"));
        assert_eq!(
            h,
            vec![Hunk {
                a_start: 1,
                a_len: 1,
                b_start: 1,
                b_len: 1
            }]
        );
    }

    #[test]
    fn separate_changes_make_separate_hunks() {
        let h = diff_lines(&v("a,b,c,d,e"), &v("a,B,c,d,E"));
        assert_eq!(h.len(), 2);
        assert_eq!((h[0].a_start, h[1].a_start), (1, 4));
    }

    #[test]
    fn marks_color_both_sides() {
        let (a, b) = (v("a,b,c"), v("a,y,c,d"));
        let hs = diff_lines(&a, &b);
        let (l, r) = marks(&hs, a.len(), b.len());
        assert_eq!(l, vec![Mark::Same, Mark::Changed, Mark::Same]);
        assert_eq!(r, vec![Mark::Same, Mark::Changed, Mark::Same, Mark::Added]);
        // 纯增: 左侧在该位置标 Gap
        let (a, b) = (v("a,c"), v("a,b,c"));
        let hs = diff_lines(&a, &b);
        let (l, r) = marks(&hs, a.len(), b.len());
        assert_eq!(l[1], Mark::Gap);
        assert_eq!(r[1], Mark::Added);
    }

    #[test]
    fn extra_lines_are_removed_rows_on_their_own_side() {
        let (a, b) = (v("a,b,c,"), v("a,X,c,d,"));
        let hs = diff_lines(&a, &b);
        let (l, r) = marks(&hs, a.len(), b.len());
        assert_eq!(l, vec![Mark::Same, Mark::Changed, Mark::Same, Mark::Gap]);
        assert_eq!(r[3], Mark::Added);
        let (l2, _) = marks(&diff_lines(&b, &a), b.len(), a.len());
        assert_eq!(l2[3], Mark::Removed, "多出来的行在自己这一侧整行标红");
    }

    #[test]
    fn empty_sides_and_huge_inputs_do_not_panic() {
        assert_eq!(diff_lines(&[], &v("a,b")).len(), 1);
        assert_eq!(diff_lines(&v("a,b"), &[]).len(), 1);
        let big_a: Vec<String> = (0..5000).map(|i| format!("a{i}")).collect();
        let big_b: Vec<String> = (0..5000).map(|i| format!("b{i}")).collect();
        let h = diff_lines(&big_a, &big_b);
        assert_eq!(h.len(), 1, "超出格数上限时退化成整块");
    }

    #[test]
    fn stepping_wraps_around() {
        let hs = diff_lines(&v("a,b,c,d,e"), &v("a,B,c,d,E"));
        assert_eq!(step(&hs, 0, true, true), Some(1));
        assert_eq!(step(&hs, 1, true, true), Some(4));
        assert_eq!(step(&hs, 4, true, true), Some(1), "到末尾回绕");
        assert_eq!(step(&hs, 4, false, true), Some(1));
        assert_eq!(step(&hs, 1, false, true), Some(4), "到开头回绕");
        assert_eq!(step(&[], 0, true, true), None);
    }

    #[test]
    fn view_recomputes_only_when_revisions_change() {
        let (a, b) = (v("a,b"), v("a,c"));
        let mut d = DiffView::new(1, 2, &a, &b, (0, 0));
        assert_eq!(d.hunks.len(), 1);
        let b2 = v("a,b");
        d.refresh(&a, &b2, (0, 0));
        assert_eq!(d.hunks.len(), 1, "版本号没变不重算");
        d.refresh(&a, &b2, (0, 1));
        assert!(d.hunks.is_empty());
        assert_eq!(d.summary(), "两份文档完全相同");
        assert!(d.marks_of(1).is_some() && d.marks_of(9).is_none());
    }
}
