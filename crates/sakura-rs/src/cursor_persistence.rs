// cursor_persistence.rs — カーソル位置保持 (sakura 7.8)
//
// ファイル再オープン時に最終カーソル位置を復元.
// プロジェクトルートに `.sakura-cursor/<file-hash>.json` で保存.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// カーソル位置 (0-indexed row, col)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CursorPos {
    pub row: usize,
    pub col: usize,
}

impl Default for CursorPos {
    fn default() -> Self {
        Self { row: 0, col: 0 }
    }
}

/// ファイル毎のカーソル状態
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CursorState {
    pub pos: CursorPos,
    pub scroll_row: usize,
    pub selection: Option<(CursorPos, CursorPos)>,
    pub last_modified_unix: u64,
}

/// 永続化管理
#[derive(Debug)]
pub struct CursorPersistence {
    storage_dir: PathBuf,
    states: HashMap<String, CursorState>,
}

impl CursorPersistence {
    pub fn new(project_root: &Path) -> Result<Self, std::io::Error> {
        let storage_dir = project_root.join(".sakura-cursor");
        fs::create_dir_all(&storage_dir)?;
        let states = HashMap::new();
        Ok(Self { storage_dir, states })
    }

    /// ファイルパスをハッシュ化 (キー)
    fn key_for(file: &Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        file.to_string_lossy().hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// 状態保存
    pub fn save(&mut self, file: &Path, state: CursorState) -> Result<(), std::io::Error> {
        let key = Self::key_for(file);
        let path = self.storage_dir.join(format!("{}.json", key));
        let json = serde_json::to_string_pretty(&state)?;
        fs::write(&path, json)?;
        self.states.insert(key, state);
        Ok(())
    }

    /// 状態読み込み (未保存なら None)
    pub fn load(&self, file: &Path) -> Result<Option<CursorState>, std::io::Error> {
        let key = Self::key_for(file);
        let path = self.storage_dir.join(format!("{}.json", key));
        if !path.exists() {
            return Ok(None);
        }
        let json = fs::read_to_string(&path)?;
        let state: CursorState = serde_json::from_str(&json)?;
        Ok(Some(state))
    }

    /// 状態削除
    pub fn clear(&self, file: &Path) -> Result<(), std::io::Error> {
        let key = Self::key_for(file);
        let path = self.storage_dir.join(format!("{}.json", key));
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// 全状態削除
    pub fn clear_all(&self) -> Result<(), std::io::Error> {
        fs::remove_dir_all(&self.storage_dir)?;
        fs::create_dir_all(&self.storage_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn make_project_root() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("sakura-rs-cursor-{}-{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_save_and_load() {
        let root = make_project_root();
        let file = root.join("test.txt");
        fs::write(&file, "content").unwrap();
        let mut cp = CursorPersistence::new(&root).unwrap();
        let state = CursorState {
            pos: CursorPos { row: 5, col: 12 },
            scroll_row: 3,
            selection: None,
            last_modified_unix: 1234,
        };
        cp.save(&file, state.clone()).unwrap();
        let loaded = cp.load(&file).unwrap().unwrap();
        assert_eq!(loaded.pos, state.pos);
        assert_eq!(loaded.scroll_row, state.scroll_row);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_load_missing() {
        let root = make_project_root();
        let cp = CursorPersistence::new(&root).unwrap();
        let result = cp.load(&root.join("nonexistent.txt")).unwrap();
        assert!(result.is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_clear() {
        let root = make_project_root();
        let file = root.join("test.txt");
        fs::write(&file, "x").unwrap();
        let mut cp = CursorPersistence::new(&root).unwrap();
        cp.save(&file, CursorState::default()).unwrap();
        cp.clear(&file).unwrap();
        assert!(cp.load(&file).unwrap().is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_clear_all() {
        let root = make_project_root();
        let mut cp = CursorPersistence::new(&root).unwrap();
        cp.save(&root.join("a.txt"), CursorState::default()).unwrap();
        cp.save(&root.join("b.txt"), CursorState::default()).unwrap();
        cp.clear_all().unwrap();
        assert!(cp.load(&root.join("a.txt")).unwrap().is_none());
        assert!(cp.load(&root.join("b.txt")).unwrap().is_none());
        let _ = fs::remove_dir_all(&root);
    }
}
