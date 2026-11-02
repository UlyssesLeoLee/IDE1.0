//! 侧栏: 文件树 / 大纲 / Grep。语义都来自 editcore (`outline_dispatch` / `grep_engine::search`), 这里只管列表和点击。
//! 文件树按需读目录 (展开才读, 读过缓存); Grep 在后台线程里跑 (不卡界面), 结果逐条点开跳到行。

use editcore::grep_engine::{self, SearchOptions, SearchResult};
use editcore::type_config::OutlineEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Files,
    Outline,
    Search,
}

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub struct GrepState {
    pub pattern: String,
    pub include: String,
    pub case: bool,
    pub regex: bool,
    pub results: Vec<SearchResult>,
    pub msg: String,
    rx: Option<Receiver<Result<Vec<SearchResult>, String>>>,
}

pub struct Sidebar {
    pub open: bool,
    pub tab: Tab,
    pub root: Option<PathBuf>,
    children: HashMap<PathBuf, Vec<Entry>>,
    pub grep: GrepState,
    /// 大纲缓存: (文档 id, 版本号) -> 条目
    outline_key: Option<(u64, u64)>,
    pub outline: Vec<OutlineEntry>,
}

/// 固定跳过的目录 (和 editcore 的 grep 一致: 编译产物 / 依赖 / VCS)
const SKIP: &[&str] = &[".git", "node_modules", "target", ".hg", ".svn"];

impl Sidebar {
    pub fn new() -> Self {
        Sidebar {
            open: false,
            tab: Tab::Files,
            root: None,
            children: HashMap::new(),
            grep: GrepState { pattern: String::new(), include: String::new(), case: false, regex: false, results: vec![], msg: String::new(), rx: None },
            outline_key: None,
            outline: vec![],
        }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.children.clear();
        self.root = Some(root);
        self.open = true;
    }

    /// 目录的子项 (目录在前, 名字排序; 读过缓存; 隐藏 SKIP 里的目录)
    pub fn children_of(&mut self, dir: &Path) -> Vec<Entry> {
        if let Some(c) = self.children.get(dir) {
            return c.clone();
        }
        let mut v: Vec<Entry> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        if is_dir && SKIP.contains(&name.as_str()) {
                            return None;
                        }
                        Some(Entry { name, path: e.path(), is_dir })
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        self.children.insert(dir.to_path_buf(), v.clone());
        v
    }

    pub fn refresh(&mut self) {
        self.children.clear();
    }

    /// 大纲: 文档没变就用缓存
    pub fn update_outline(&mut self, doc_id: u64, rev: u64, name: &str, lines: &[String]) {
        if self.outline_key == Some((doc_id, rev)) {
            return;
        }
        self.outline_key = Some((doc_id, rev));
        let registry = editcore::type_config::TypeRegistry::default_registry();
        let ty = registry.detect(name).map(|t| t.name.clone()).unwrap_or_else(|| "Plain".into());
        self.outline = editcore::type_config::TypeConfig::outline_dispatch(&ty, &lines.join("\n"));
    }

    /// 在项目根下开始 Grep (后台线程); 上一次没跑完的结果作废
    pub fn start_grep(&mut self) {
        let Some(root) = self.root.clone() else {
            self.grep.msg = "先打开一个文件夹".into();
            return;
        };
        if self.grep.pattern.is_empty() {
            self.grep.msg = "请输入要搜索的内容".into();
            return;
        }
        let include: Vec<String> = self.grep.include.split([',', ';', ' ']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
        let opts = SearchOptions {
            pattern: self.grep.pattern.clone(),
            case_sensitive: Some(self.grep.case),
            regex: Some(self.grep.regex),
            include_files: if include.is_empty() { None } else { Some(include) },
            max_results: Some(2000),
            ..Default::default()
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(grep_engine::search(&root, &opts));
        });
        self.grep.rx = Some(rx);
        self.grep.results.clear();
        self.grep.msg = "搜索中…".into();
    }

    /// 每帧调用: 取后台结果; 返回 true 表示有新结果 (需要重画)
    pub fn poll_grep(&mut self) -> bool {
        let Some(rx) = &self.grep.rx else { return false };
        match rx.try_recv() {
            Ok(Ok(r)) => {
                self.grep.msg = format!("{} 处命中", r.len());
                self.grep.results = r;
                self.grep.rx = None;
                true
            }
            Ok(Err(e)) => {
                self.grep.msg = e;
                self.grep.rx = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(_) => {
                self.grep.rx = None;
                false
            }
        }
    }

    pub fn grep_running(&self) -> bool {
        self.grep.rx.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_lists_dirs_first_and_skips_vcs_dirs() {
        let d = std::env::temp_dir().join(format!("ide-fast-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("b_dir")).unwrap();
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::write(d.join("a.txt"), "x").unwrap();
        let mut s = Sidebar::new();
        s.set_root(d.clone());
        let names: Vec<String> = s.children_of(&d).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["b_dir", "a.txt"], "目录在前, .git 被隐藏");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn grep_runs_in_background_and_returns_hits() {
        let d = std::env::temp_dir().join(format!("ide-fast-grep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.rs"), "fn main() {}\nlet needle = 1;\n").unwrap();
        let mut s = Sidebar::new();
        s.set_root(d.clone());
        s.grep.pattern = "needle".into();
        s.start_grep();
        for _ in 0..200 {
            if s.poll_grep() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(s.grep.results.len(), 1, "{}", s.grep.msg);
        assert_eq!(s.grep.results[0].line, 2);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn outline_is_cached_by_revision() {
        let mut s = Sidebar::new();
        let lines: Vec<String> = ["# T", "", "## S"].iter().map(|x| x.to_string()).collect();
        s.update_outline(1, 1, "a.md", &lines);
        assert!(s.outline.len() >= 2, "{:?}", s.outline.len());
        s.outline.clear();
        s.update_outline(1, 1, "a.md", &lines);
        assert!(s.outline.is_empty(), "同一版本不重算");
        s.update_outline(1, 2, "a.md", &lines);
        assert!(!s.outline.is_empty());
    }
}
