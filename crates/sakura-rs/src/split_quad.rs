// split_quad.rs — 縦横分割 (四方) (sakura 11.5)
//
// 4 分割レイアウト管理.
// 1 つの文書を 4 ペインに分割表示 (各ペイン独立スクロール).

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pane {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Pane {
    pub const ALL: [Pane; 4] = [Self::TopLeft, Self::TopRight, Self::BottomLeft, Self::BottomRight];
}

#[derive(Debug, Clone)]
pub struct QuadView {
    pub file: PathBuf,
    pub active_pane: Pane,
    pub scroll: [usize; 4],
    pub cursor: [(usize, usize); 4],
}

impl QuadView {
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            active_pane: Pane::TopLeft,
            scroll: [0; 4],
            cursor: [(0, 0); 4],
        }
    }

    /// アクティブペイン変更
    pub fn set_active(&mut self, pane: Pane) {
        self.active_pane = pane;
    }

    /// アクティブペインのスクロール位置
    pub fn scroll(&self) -> usize {
        let idx = pane_index(self.active_pane);
        self.scroll[idx]
    }

    pub fn set_scroll(&mut self, pos: usize) {
        let idx = pane_index(self.active_pane);
        self.scroll[idx] = pos;
    }

    /// アクティブペインのカーソル
    pub fn cursor(&self) -> (usize, usize) {
        let idx = pane_index(self.active_pane);
        self.cursor[idx]
    }

    pub fn set_cursor(&mut self, row: usize, col: usize) {
        let idx = pane_index(self.active_pane);
        self.cursor[idx] = (row, col);
    }
}

fn pane_index(p: Pane) -> usize {
    match p {
        Pane::TopLeft => 0,
        Pane::TopRight => 1,
        Pane::BottomLeft => 2,
        Pane::BottomRight => 3,
    }
}

/// 4 分割管理 (複数の QuadView)
#[derive(Debug, Default)]
pub struct SplitQuadManager {
    views: Vec<QuadView>,
}

impl SplitQuadManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, view: QuadView) {
        self.views.push(view);
    }

    pub fn views(&self) -> &[QuadView] {
        &self.views
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }

    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_all() {
        assert_eq!(Pane::ALL.len(), 4);
    }

    #[test]
    fn test_quad_view_new() {
        let v = QuadView::new(PathBuf::from("test.txt"));
        assert_eq!(v.active_pane, Pane::TopLeft);
        assert_eq!(v.scroll(), 0);
    }

    #[test]
    fn test_set_active() {
        let mut v = QuadView::new(PathBuf::from("test.txt"));
        v.set_active(Pane::BottomRight);
        assert_eq!(v.active_pane, Pane::BottomRight);
    }

    #[test]
    fn test_per_pane_scroll() {
        let mut v = QuadView::new(PathBuf::from("test.txt"));
        v.set_active(Pane::TopLeft);
        v.set_scroll(100);
        v.set_active(Pane::TopRight);
        v.set_scroll(200);
        v.set_active(Pane::TopLeft);
        assert_eq!(v.scroll(), 100);
        v.set_active(Pane::TopRight);
        assert_eq!(v.scroll(), 200);
    }

    #[test]
    fn test_per_pane_cursor() {
        let mut v = QuadView::new(PathBuf::from("test.txt"));
        v.set_active(Pane::BottomLeft);
        v.set_cursor(5, 10);
        v.set_active(Pane::TopRight);
        v.set_cursor(20, 30);
        v.set_active(Pane::BottomLeft);
        assert_eq!(v.cursor(), (5, 10));
    }

    #[test]
    fn test_manager() {
        let mut m = SplitQuadManager::new();
        m.add(QuadView::new(PathBuf::from("a.txt")));
        m.add(QuadView::new(PathBuf::from("b.txt")));
        assert_eq!(m.len(), 2);
    }
}
