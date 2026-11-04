//! 窗口分割: 无 / 左右 / 上下 / 四分。每个窗格各看一个文档, 焦点窗格决定谁收键盘。
//! 纯逻辑 (不碰 egui), 便于单测。

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SplitKind {
    #[default]
    Off,
    /// 左右两栏
    Cols,
    /// 上下两栏
    Rows,
    /// 四个窗格 (2x2)
    Quad,
}

impl SplitKind {
    pub fn panes(self) -> usize {
        match self {
            SplitKind::Off => 1,
            SplitKind::Cols | SplitKind::Rows => 2,
            SplitKind::Quad => 4,
        }
    }
}

/// 窗格 -> 文档下标。`docs` 是文档总数 (>= 1)。
#[derive(Clone, Debug, Default)]
pub struct Panes {
    pub kind: SplitKind,
    pub docs: [usize; 4],
    pub focus: usize,
}

impl Panes {
    /// 切到 `kind`: 窗格 0 看 `active`, 其余窗格依次看后面的文档 (文档不够就重复)
    pub fn set(&mut self, kind: SplitKind, active: usize, ndocs: usize) {
        self.kind = kind;
        self.focus = 0;
        let n = ndocs.max(1);
        let a = active.min(n - 1);
        for (i, d) in self.docs.iter_mut().enumerate() {
            *d = if n > 1 { (a + i) % n } else { a };
        }
    }

    /// 同一个种类再按一次 = 取消分割
    pub fn toggle(&mut self, kind: SplitKind, active: usize, ndocs: usize) {
        if self.kind == kind { self.kind = SplitKind::Off; } else { self.set(kind, active, ndocs); }
    }

    /// 文档数变了 / 关了标签: 把下标夹回范围, 焦点夹回窗格数
    pub fn clamp(&mut self, ndocs: usize) {
        let last = ndocs.max(1) - 1;
        for d in self.docs.iter_mut() { *d = (*d).min(last); }
        self.focus = self.focus.min(self.kind.panes() - 1);
    }

    pub fn focused_doc(&self) -> usize { self.docs[self.focus] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_spreads_docs_over_panes() {
        let mut p = Panes::default();
        p.set(SplitKind::Quad, 1, 3);
        assert_eq!(p.docs, [1, 2, 0, 1]);
        assert_eq!(p.focus, 0);
        p.set(SplitKind::Rows, 0, 1);
        assert_eq!(p.docs, [0, 0, 0, 0]);
    }

    #[test]
    fn toggle_same_kind_turns_off_other_kind_switches() {
        let mut p = Panes::default();
        p.toggle(SplitKind::Rows, 0, 2);
        assert_eq!(p.kind, SplitKind::Rows);
        p.toggle(SplitKind::Quad, 0, 2);
        assert_eq!(p.kind, SplitKind::Quad);
        p.toggle(SplitKind::Quad, 0, 2);
        assert_eq!(p.kind, SplitKind::Off);
    }

    #[test]
    fn clamp_keeps_indices_and_focus_valid() {
        let mut p = Panes::default();
        p.set(SplitKind::Quad, 3, 5);
        p.focus = 3;
        p.clamp(2);
        assert!(p.docs.iter().all(|&d| d <= 1));
        p.kind = SplitKind::Cols;
        p.clamp(2);
        assert_eq!(p.focus, 1);
    }
}
