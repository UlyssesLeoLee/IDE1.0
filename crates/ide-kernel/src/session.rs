//! `Session` — 多标签会话 (对应 Sakura Editor 的「タブモード」).
//!
//! 前端 `editor.html` 里 `TABS` Map + `activeTabId` 的逻辑搬到这里,
//! 但**只有一处实现**: Web / Tauri / TUI 共用.
//!
//! 不含窗口/分屏几何 —— 那是渲染层的事. 这里只管「有哪些文档、当前是哪个」.

use crate::document::{Document, LineEnding};
use std::collections::HashMap;

/// 一个标签页的持久状态 (关闭后重建用).
#[derive(Debug, Clone)]
pub struct TabState {
    pub id: u64,
    pub path: Option<String>,
    pub dirty: bool,
    pub caret_row: i32,
    pub caret_col: i32,
    pub line_ending: LineEnding,
    pub cursor_persist_id: Option<String>,
}

/// 多标签会话.
pub struct Session {
    docs: HashMap<u64, Document>,
    order: Vec<u64>,
    active: Option<u64>,
    next_id: u64,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
            order: Vec::new(),
            active: None,
            next_id: 1,
        }
    }

    /// 新建一个空白文档并激活.
    pub fn new_tab(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.docs.insert(id, Document::new());
        self.order.push(id);
        self.active = Some(id);
        id
    }

    /// 用文本打开一个新标签.
    pub fn open_text(&mut self, path: Option<&str>, text: &str) -> u64 {
        let id = self.new_tab();
        if let Some(doc) = self.docs.get_mut(&id) {
            doc.set_text(text);
            if let Some(p) = path {
                doc.set_path(p);
            }
        }
        id
    }

    /// 切到指定标签 (越界时返回 false).
    pub fn activate(&mut self, id: u64) -> bool {
        if self.docs.contains_key(&id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    /// 切换到下一个标签 (MRU 之外的简单循环 —— Tab 键行为).
    pub fn next_tab(&mut self) -> Option<u64> {
        if self.order.is_empty() {
            return None;
        }
        let cur = self.active?;
        let i = self.order.iter().position(|x| *x == cur)?;
        let n = self.order[(i + 1) % self.order.len()];
        self.active = Some(n);
        Some(n)
    }

    pub fn prev_tab(&mut self) -> Option<u64> {
        if self.order.is_empty() {
            return None;
        }
        let cur = self.active?;
        let i = self.order.iter().position(|x| *x == cur)?;
        let n = self.order[(i + self.order.len() - 1) % self.order.len()];
        self.active = Some(n);
        Some(n)
    }

    /// 关闭标签. 关闭后激活相邻的标签 (Sakura 行为).
    ///
    /// 返回被关闭的标签的最终状态 (便于外壳持久化光标位置).
    pub fn close(&mut self, id: u64) -> Option<TabState> {
        let doc = self.docs.remove(&id)?;
        let idx = self.order.iter().position(|x| *x == id)?;
        self.order.remove(idx);

        let state = TabState {
            id,
            path: doc.path().map(str::to_string),
            dirty: doc.is_dirty(),
            caret_row: doc.caret().row,
            caret_col: doc.caret().col,
            line_ending: doc.line_ending(),
            cursor_persist_id: None,
        };

        if self.active == Some(id) {
            self.active = if self.order.is_empty() {
                None
            } else {
                Some(self.order[idx.min(self.order.len() - 1)])
            };
        }
        Some(state)
    }

    pub fn active_id(&self) -> Option<u64> {
        self.active
    }

    pub fn active(&self) -> Option<&Document> {
        self.active.and_then(|id| self.docs.get(&id))
    }

    pub fn active_mut(&mut self) -> Option<&mut Document> {
        match self.active {
            Some(id) => self.docs.get_mut(&id),
            None => None,
        }
    }

    pub fn get(&self, id: u64) -> Option<&Document> {
        self.docs.get(&id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Document> {
        self.docs.get_mut(&id)
    }

    pub fn tab_ids(&self) -> &[u64] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// 所有未保存的标签 (关闭全部 / 退出前提示).
    pub fn dirty_tabs(&self) -> Vec<u64> {
        self.order
            .iter()
            .copied()
            .filter(|id| self.docs.get(id).map(|d| d.is_dirty()).unwrap_or(false))
            .collect()
    }

    /// 外壳渲染用的标签摘要.
    pub fn tab_states(&self) -> Vec<TabState> {
        self.order
            .iter()
            .filter_map(|id| {
                self.docs.get(id).map(|d| TabState {
                    id: *id,
                    path: d.path().map(str::to_string),
                    dirty: d.is_dirty(),
                    caret_row: d.caret().row,
                    caret_col: d.caret().col,
                    line_ending: d.line_ending(),
                    cursor_persist_id: None,
                })
            })
            .collect()
    }
}
