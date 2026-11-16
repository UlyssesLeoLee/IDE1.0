//! 标签 / 窗口级的纯逻辑 (不碰界面, 可单测): 循环切换、移动、批量关闭的范围、移动履历 (Alt+P / Alt+N)。
//! 语义对齐 Tauri 版 editor.html 的 next-tab / tab-move-* / close-others / close-right / jumpHist*。

/// Ctrl+Tab / Ctrl+Shift+Tab: 在 n 个标签里循环
pub fn cycle(active: usize, n: usize, fwd: bool) -> usize {
    if n == 0 {
        return 0;
    }
    if fwd {
        (active + 1) % n
    } else {
        (active + n - 1) % n
    }
}

/// 标签往左 / 右挪一格, 返回挪动后的下标 (到头了不动)
pub fn move_tab<T>(v: &mut [T], i: usize, left: bool) -> usize {
    if i >= v.len() {
        return i;
    }
    let j = if left {
        i.checked_sub(1)
    } else {
        (i + 1 < v.len()).then_some(i + 1)
    };
    match j {
        Some(j) => {
            v.swap(i, j);
            j
        }
        None => i,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloseKind {
    Others,
    Left,
    Right,
    All,
}

/// 要关的标签下标 (升序)
pub fn close_set(kind: CloseKind, active: usize, n: usize) -> Vec<usize> {
    (0..n)
        .filter(|&i| match kind {
            CloseKind::Others => i != active,
            CloseKind::Left => i < active,
            CloseKind::Right => i > active,
            CloseKind::All => true,
        })
        .collect()
}

/// 移动履历: 记 (文档 id, 行, 列); 回退前先把当前位置记进去, 才能再前进回来 (同 Tauri 版 jumpHistGo)
#[derive(Default, Debug)]
pub struct JumpHist {
    list: Vec<(u64, usize, usize)>,
    idx: Option<usize>,
}

const JUMP_MAX: usize = 100;

impl JumpHist {
    /// 登记当前位置 (和最后一条同文档同行就不重复记); 登记会丢掉「前进」那一半
    pub fn set(&mut self, here: (u64, usize, usize)) {
        if let Some(i) = self.idx {
            if let Some(l) = self.list.get(i) {
                if l.0 == here.0 && l.1 == here.1 {
                    return;
                }
            }
        }
        let keep = self.idx.map_or(0, |i| i + 1);
        self.list.truncate(keep);
        self.list.push(here);
        if self.list.len() > JUMP_MAX {
            self.list.remove(0);
        }
        self.idx = Some(self.list.len() - 1);
    }

    /// d = -1 回退, +1 前进; 到头 / 为空 = None
    pub fn go(&mut self, d: i32, here: (u64, usize, usize)) -> Option<(u64, usize, usize)> {
        if self.list.is_empty() {
            return None;
        }
        if d < 0 && self.idx == Some(self.list.len() - 1) {
            self.set(here);
        }
        let i = self.idx? as i64 + d as i64;
        if i < 0 || i as usize >= self.list.len() {
            return None;
        }
        self.idx = Some(i as usize);
        Some(self.list[i as usize])
    }

    /// 文档关了: 把它的记录清掉
    pub fn forget(&mut self, id: u64) {
        self.list.retain(|h| h.0 != id);
        self.idx = if self.list.is_empty() {
            None
        } else {
            Some(self.idx.unwrap_or(0).min(self.list.len() - 1))
        };
    }
}

/// 现有的分组编号 (升序、去重); 传各标签的分组号
pub fn groups(values: impl IntoIterator<Item = u8>) -> Vec<u8> {
    let mut g: Vec<u8> = values.into_iter().collect();
    g.sort_unstable();
    g.dedup();
    g
}

/// 从分组 `cur` 往后 / 往前走一个, 到头绕回另一端 (同 Tauri `switchGroup` / `tab-joint-*`); 不足两个分组 = None
pub fn neighbour_group(groups: &[u8], cur: u8, fwd: bool) -> Option<u8> {
    if groups.len() < 2 {
        return None;
    }
    let (i, n) = (
        groups.iter().position(|&g| g == cur).unwrap_or(0),
        groups.len(),
    );
    Some(groups[if fwd { (i + 1) % n } else { (i + n - 1) % n }])
}

/// 「移到新分组」用的编号: 比现有最大的大一; 编号 (0 是「无分组」, 1..`limit` 是颜色分组) 用完了就取最小的没人用的; 都有人用 = None
pub fn new_group(groups: &[u8], limit: u8) -> Option<u8> {
    let next = groups.iter().copied().max().unwrap_or(0).saturating_add(1);
    if next < limit {
        return Some(next);
    }
    (1..limit).find(|g| !groups.contains(g))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_wraps_both_ways() {
        assert_eq!(cycle(2, 3, true), 0);
        assert_eq!(cycle(0, 3, false), 2);
        assert_eq!(cycle(1, 3, true), 2);
        assert_eq!(cycle(0, 0, true), 0);
    }

    #[test]
    fn move_tab_stops_at_ends() {
        let mut v = vec!['a', 'b', 'c'];
        assert_eq!(move_tab(&mut v, 1, true), 0);
        assert_eq!(v, ['b', 'a', 'c']);
        assert_eq!(move_tab(&mut v, 0, true), 0);
        assert_eq!(move_tab(&mut v, 2, false), 2);
        assert_eq!(move_tab(&mut v, 0, false), 1);
        assert_eq!(v, ['a', 'b', 'c']);
        assert_eq!(move_tab(&mut v, 9, false), 9);
    }

    #[test]
    fn close_sets() {
        assert_eq!(close_set(CloseKind::Others, 1, 4), vec![0, 2, 3]);
        assert_eq!(close_set(CloseKind::Left, 2, 4), vec![0, 1]);
        assert_eq!(close_set(CloseKind::Right, 1, 4), vec![2, 3]);
        assert_eq!(close_set(CloseKind::All, 1, 3), vec![0, 1, 2]);
        assert!(close_set(CloseKind::Right, 3, 4).is_empty());
    }

    #[test]
    fn jump_history_back_then_forward() {
        let mut h = JumpHist::default();
        assert!(h.go(-1, (1, 0, 0)).is_none(), "空履历回退无处可去");
        h.set((1, 10, 0));
        h.set((1, 10, 5)); // 同行不重复
        h.set((2, 3, 0));
        // 现在在 (2,50): 回退 -> (2,3); 再回退 -> (1,10); 再前进 -> (2,3)
        assert_eq!(h.go(-1, (2, 50, 0)), Some((2, 3, 0)));
        assert_eq!(h.go(-1, (2, 3, 0)), Some((1, 10, 0)));
        assert_eq!(h.go(-1, (1, 10, 0)), None);
        assert_eq!(h.go(1, (1, 10, 0)), Some((2, 3, 0)));
        assert_eq!(
            h.go(1, (2, 3, 0)),
            Some((2, 50, 0)),
            "回退前补记的当前位置能再前进回去"
        );
        assert_eq!(h.go(1, (2, 50, 0)), None);
    }

    #[test]
    fn jump_history_forget_and_cap() {
        let mut h = JumpHist::default();
        for r in 0..150 {
            h.set((7, r, 0));
        }
        assert_eq!(h.list.len(), JUMP_MAX);
        h.forget(7);
        assert!(h.go(-1, (1, 0, 0)).is_none());
    }

    #[test]
    fn group_helpers_wrap_and_pick_free_numbers() {
        assert_eq!(groups([2, 0, 2, 1, 0]), [0, 1, 2]);
        assert_eq!(neighbour_group(&[0, 2, 4], 2, true), Some(4));
        assert_eq!(
            neighbour_group(&[0, 2, 4], 4, true),
            Some(0),
            "最后一个再往后绕回第一个"
        );
        assert_eq!(
            neighbour_group(&[0, 2, 4], 0, false),
            Some(4),
            "第一个再往前绕回最后一个"
        );
        assert_eq!(neighbour_group(&[3], 3, true), None, "只有一个分组");
        assert_eq!(
            neighbour_group(&[0, 2], 7, true),
            Some(2),
            "当前分组不在列表里: 当作第一个"
        );
        assert_eq!(new_group(&[0], 6), Some(1));
        assert_eq!(new_group(&[0, 1, 3], 6), Some(4), "比最大的大一");
        assert_eq!(
            new_group(&[0, 1, 5], 6),
            Some(2),
            "编号用到头: 取最小的没人用的"
        );
        assert_eq!(new_group(&[1, 2, 3, 4, 5], 6), None, "颜色分组全被占用");
        assert_eq!(new_group(&[255], 6), Some(1), "不会溢出");
    }
}
