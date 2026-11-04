//! 侧栏: 文件树 / 大纲 / Grep。大纲来自 editcore (`outline_dispatch`); Grep 的状态和控件在 `grep.rs`, 这里只放文件树和大纲。
//! 文件树按需读目录 (展开才读, 读过缓存)。

use crate::grep::Grep;
use editcore::type_config::OutlineEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Files,
    Outline,
    Search,
    /// 源代码管理 (git 状态)
    Scm,
    /// 扩展 (插件列表)
    Plugins,
}

/// 文件树上待确认 / 待输入的操作
#[derive(Clone, PartialEq, Debug)]
pub enum FileOp {
    NewFile(PathBuf),
    NewDir(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
}

pub struct Pending {
    pub op: FileOp,
    /// 名字输入框
    pub text: String,
    pub error: Option<String>,
    /// 刚弹出: 第一帧把焦点给输入框
    pub fresh: bool,
}

/// 名字合法性: 不能空、不能带路径分隔符、不能是 `.` `..`、不能含系统保留字符
pub fn check_name(name: &str) -> Result<(), String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("名字不能为空".into());
    }
    if n == "." || n == ".." || n.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
        return Err("名字里不能有 / \\ : * ? \" < > |".into());
    }
    if n.ends_with('.') || n.ends_with(' ') {
        return Err("名字不能以点或空格结尾".into());
    }
    Ok(())
}

/// 在 dir 下新建空文件; 已存在就拒绝 (不覆盖)
pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf, String> {
    check_name(name)?;
    let p = dir.join(name.trim());
    std::fs::OpenOptions::new().write(true).create_new(true).open(&p).map_err(|e| format!("新建失败: {e}"))?;
    Ok(p)
}

pub fn create_dir(dir: &Path, name: &str) -> Result<PathBuf, String> {
    check_name(name)?;
    let p = dir.join(name.trim());
    if p.exists() {
        return Err("同名文件或文件夹已存在".into());
    }
    std::fs::create_dir(&p).map_err(|e| format!("新建失败: {e}"))?;
    Ok(p)
}

/// 重命名 (留在原目录); 目标已存在就拒绝
pub fn rename_entry(path: &Path, new_name: &str) -> Result<PathBuf, String> {
    check_name(new_name)?;
    let to = path.with_file_name(new_name.trim());
    if to == path {
        return Ok(to);
    }
    if to.exists() {
        return Err("同名文件或文件夹已存在".into());
    }
    std::fs::rename(path, &to).map_err(|e| format!("重命名失败: {e}"))?;
    Ok(to)
}

/// 删除文件或整个文件夹 (调用前界面已经让用户确认)
pub fn delete_entry(path: &Path) -> Result<(), String> {
    let r = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
    r.map_err(|e| format!("删除失败: {e}"))
}

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub struct Sidebar {
    pub open: bool,
    pub tab: Tab,
    pub root: Option<PathBuf>,
    children: HashMap<PathBuf, Vec<Entry>>,
    pub grep: Grep,
    pub scm: crate::scm::Scm,
    /// 文件树上正在进行的新建 / 重命名 / 删除
    pub pending: Option<Pending>,
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
            grep: Grep::new(),
            scm: crate::scm::Scm::new(),
            pending: None,
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
    fn file_operations_create_rename_delete_and_refuse_bad_names() {
        let d = std::env::temp_dir().join(format!("ide-fast-ops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = create_file(&d, "a.txt").unwrap();
        assert!(f.is_file());
        assert!(create_file(&d, "a.txt").is_err(), "已存在不覆盖");
        for bad in ["", "  ", "a/b", "a\\b", "..", "x:y", "tail."] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        let sub = create_dir(&d, "sub").unwrap();
        assert!(sub.is_dir() && create_dir(&d, "sub").is_err());
        let g = rename_entry(&f, "b.txt").unwrap();
        assert!(g.is_file() && !f.exists());
        std::fs::write(d.join("c.txt"), "x").unwrap();
        assert!(rename_entry(&g, "c.txt").is_err(), "目标已存在");
        std::fs::write(sub.join("in.txt"), "x").unwrap();
        delete_entry(&sub).unwrap();
        assert!(!sub.exists(), "文件夹连内容一起删");
        delete_entry(&g).unwrap();
        assert!(!g.exists());
        assert!(delete_entry(&g).is_err());
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
