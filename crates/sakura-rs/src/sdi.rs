// sdi.rs — SDI (文書毎ウィンドウ) (sakura 11.1)
//
// 1 ファイル 1 ウィンドウ. MDI の反対.
// 実装: 各文書を独立した Window として扱うメタデータ管理.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u32);

#[derive(Debug, Clone)]
pub struct SdiWindow {
    pub id: WindowId,
    pub file: Option<PathBuf>,
    pub title: String,
    pub active: bool,
}

/// SDI ウィンドウ管理
#[derive(Debug, Default)]
pub struct SdiManager {
    windows: Vec<SdiWindow>,
    next_id: u32,
}

impl SdiManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新規ウィンドウ
    pub fn open(&mut self, file: Option<PathBuf>) -> WindowId {
        let id = WindowId(self.next_id);
        self.next_id += 1;
        let title = match &file {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled".to_string()),
            None => "untitled".to_string(),
        };
        // 他を inactive に
        for w in &mut self.windows {
            w.active = false;
        }
        self.windows.push(SdiWindow {
            id,
            file,
            title,
            active: true,
        });
        id
    }

    /// アクティブ化
    pub fn activate(&mut self, id: WindowId) {
        for w in &mut self.windows {
            w.active = w.id == id;
        }
    }

    /// 閉じる
    pub fn close(&mut self, id: WindowId) -> bool {
        let pos = self.windows.iter().position(|w| w.id == id);
        if let Some(idx) = pos {
            self.windows.remove(idx);
            // 残りがあれば最後を active に
            if let Some(last) = self.windows.last_mut() {
                last.active = true;
            }
            true
        } else {
            false
        }
    }

    /// アクティブウィンドウ
    pub fn active(&self) -> Option<&SdiWindow> {
        self.windows.iter().find(|w| w.active)
    }

    /// 全ウィンドウ
    pub fn windows(&self) -> &[SdiWindow] {
        &self.windows
    }

    /// 件数
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_window() {
        let mut m = SdiManager::new();
        let _id = m.open(None);
        assert_eq!(m.len(), 1);
        assert!(m.active().unwrap().active);
    }

    #[test]
    fn test_open_with_file() {
        let mut m = SdiManager::new();
        let id = m.open(Some(PathBuf::from("test.txt")));
        let w = m.windows().iter().find(|w| w.id == id).unwrap();
        assert_eq!(w.title, "test.txt");
    }

    #[test]
    fn test_activate() {
        let mut m = SdiManager::new();
        let id1 = m.open(None);
        let id2 = m.open(None);
        assert!(m.windows().iter().find(|w| w.id == id1).unwrap().active == false);
        assert!(m.windows().iter().find(|w| w.id == id2).unwrap().active);
        m.activate(id1);
        assert!(m.windows().iter().find(|w| w.id == id1).unwrap().active);
    }

    #[test]
    fn test_close() {
        let mut m = SdiManager::new();
        let id = m.open(None);
        assert!(m.close(id));
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn test_close_invalid() {
        let mut m = SdiManager::new();
        assert!(!m.close(WindowId(99)));
    }
}
