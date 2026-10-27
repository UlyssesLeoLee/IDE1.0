//! 撤销 / 重做栈 (ARCH-01: 历史放 Rust).
//!
//! 以"补丁"形式保存: 每个补丁 = 把第 `start` 行起的 `removed` 行换成 `inserted` 行 (+ 前后光标)。
//! 前端只保留"上一个边界的行快照"用来算补丁, 栈本体 (含 200 步上限) 在这里, 按标签页 id 分开。
//! 进程内全局登记 (web 版 / 桌面版共用同一份实现)。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

const MAX_UNDO: usize = 200;
const MAX_TABS: usize = 512;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Patch {
    pub start: usize,
    pub removed: Vec<String>,
    pub inserted: Vec<String>,
    pub cursor_before: [usize; 2],
    #[serde(default)]
    pub cursor_after: [usize; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistReq {
    pub tab: String,
    /// "push" | "undo" | "redo" | "clear" | "info"
    pub op: String,
    #[serde(default)]
    pub patch: Option<Patch>,
    /// undo / redo 发起时的光标 (redo 回到这里)
    #[serde(default)]
    pub cursor: Option<[usize; 2]>,
}

/// 要套到文档上的改动
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Apply {
    pub start: usize,
    pub remove: usize,
    pub insert: Vec<String>,
    pub cursor: [usize; 2],
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct HistRes {
    pub apply: Option<Apply>,
    pub undo_len: usize,
    pub redo_len: usize,
}

#[derive(Default)]
struct History {
    undo: Vec<Patch>,
    redo: Vec<Patch>,
    /// 最近一次被用到的时刻 (登记表的递增计数); 登记数到上限时淘汰最久没用的
    used: u64,
}

/// 全部标签页的撤销栈。单独成类型, 淘汰策略才能在私有实例上单测 (不碰进程里共用的那份)
#[derive(Default)]
struct Registry {
    tick: u64,
    tabs: HashMap<String, History>,
}

impl Registry {
    /// 执行一个请求; 无法识别的操作 (或缺必需参数) = None, 且不留下任何登记
    fn run(&mut self, req: &HistReq, max_tabs: usize) -> Option<HistRes> {
        if !matches!(req.op.as_str(), "push" | "undo" | "redo" | "clear" | "info") {
            return None;
        }
        if req.op == "push" && req.patch.is_none() {
            return None;
        }
        self.tick += 1;
        let tick = self.tick;
        // 只有 push 会新建登记; 其它操作碰到没登记过的标签页 = 空栈, 不占位置
        if !self.tabs.contains_key(&req.tab) {
            if req.op != "push" {
                return Some(HistRes::default());
            }
            self.evict_for_new_tab(max_tabs);
        }
        let h = self.tabs.entry(req.tab.clone()).or_default();
        h.used = tick;
        let apply = step(h, req);
        let res = HistRes { apply, undo_len: h.undo.len(), redo_len: h.redo.len() };
        if req.op == "clear" {
            self.tabs.remove(&req.tab);
        }
        Some(res)
    }

    /// 页面重新载入时旧会话的标签不会再来清理, 登记数到上限就丢掉最久没用的, 内存不会无限涨
    fn evict_for_new_tab(&mut self, max_tabs: usize) {
        while self.tabs.len() >= max_tabs {
            match self.tabs.iter().min_by_key(|(_, h)| h.used).map(|(k, _)| k.clone()) {
                Some(oldest) => {
                    self.tabs.remove(&oldest);
                }
                None => break,
            }
        }
    }
}

/// 对一个标签页的栈做一步操作, 返回要套到文档上的改动 (undo / redo 才有)
fn step(h: &mut History, req: &HistReq) -> Option<Apply> {
    match req.op.as_str() {
        "push" => {
            if let Some(p) = req.patch.clone() {
                h.undo.push(p);
                if h.undo.len() > MAX_UNDO {
                    h.undo.remove(0);
                }
                h.redo.clear();
            }
            None
        }
        "undo" => {
            let mut p = h.undo.pop()?;
            let apply = Apply { start: p.start, remove: p.inserted.len(), insert: p.removed.clone(), cursor: p.cursor_before };
            if let Some(c) = req.cursor {
                p.cursor_after = c; // redo 回到 undo 发起时的光标
            }
            h.redo.push(p);
            Some(apply)
        }
        "redo" => {
            let p = h.redo.pop()?;
            let apply = Apply { start: p.start, remove: p.removed.len(), insert: p.inserted.clone(), cursor: p.cursor_after };
            h.undo.push(p);
            Some(apply)
        }
        "clear" => {
            h.undo.clear();
            h.redo.clear();
            None
        }
        _ => None, // "info"
    }
}

fn registry() -> &'static Mutex<Registry> {
    static R: OnceLock<Mutex<Registry>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Registry::default()))
}

pub fn run(req: &HistReq) -> Option<HistRes> {
    // 别的线程持锁时 panic 会让锁"中毒"; 栈数据本身还是一致的, 照常用 (原来这里会把它当成"未知操作")
    let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
    reg.run(req, MAX_TABS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(start: usize, removed: &[&str], inserted: &[&str], before: [usize; 2]) -> Patch {
        Patch { start, removed: removed.iter().map(|s| s.to_string()).collect(), inserted: inserted.iter().map(|s| s.to_string()).collect(), cursor_before: before, cursor_after: [0, 0] }
    }
    fn req(tab: &str, op: &str, p: Option<Patch>, cursor: Option<[usize; 2]>) -> HistReq {
        HistReq { tab: tab.into(), op: op.into(), patch: p, cursor }
    }

    #[test]
    fn push_undo_redo_roundtrip() {
        let t = "t-roundtrip";
        run(&req(t, "push", Some(patch(1, &["b"], &["B", "B2"], [1, 1])), None)).unwrap();
        let u = run(&req(t, "undo", None, Some([2, 0]))).unwrap();
        assert_eq!(u.apply, Some(Apply { start: 1, remove: 2, insert: vec!["b".into()], cursor: [1, 1] }));
        assert_eq!((u.undo_len, u.redo_len), (0, 1));
        let r = run(&req(t, "redo", None, None)).unwrap();
        assert_eq!(r.apply, Some(Apply { start: 1, remove: 1, insert: vec!["B".into(), "B2".into()], cursor: [2, 0] })); // 回到 undo 发起时的光标
        assert_eq!((r.undo_len, r.redo_len), (1, 0));
    }
    #[test]
    fn new_edit_clears_redo_and_caps_depth() {
        let t = "t-cap";
        for i in 0..205 {
            run(&req(t, "push", Some(patch(0, &["a"], &["b"], [0, i % 3])), None)).unwrap();
        }
        assert_eq!(run(&req(t, "info", None, None)).unwrap().undo_len, 200);
        run(&req(t, "undo", None, None)).unwrap();
        assert_eq!(run(&req(t, "info", None, None)).unwrap().redo_len, 1);
        run(&req(t, "push", Some(patch(0, &["x"], &["y"], [0, 0])), None)).unwrap();
        assert_eq!(run(&req(t, "info", None, None)).unwrap().redo_len, 0);
    }
    #[test]
    fn tabs_are_independent_and_empty_undo_is_noop() {
        run(&req("t-a", "push", Some(patch(0, &[], &["x"], [0, 0])), None)).unwrap();
        assert_eq!(run(&req("t-b", "undo", None, None)).unwrap().apply, None);
        let c = run(&req("t-a", "clear", None, None)).unwrap();
        assert_eq!((c.undo_len, c.redo_len), (0, 0));
        assert!(run(&req("t-a", "nope", None, None)).is_none());
    }
    #[test]
    fn invalid_requests_leave_no_trace() {
        let mut r = Registry::default();
        assert!(r.run(&req("t-x", "nope", None, None), 8).is_none());
        assert!(r.run(&req("t-x", "push", None, None), 8).is_none(), "push 没带补丁");
        assert!(r.tabs.is_empty());
        // info / undo / redo / clear 碰到没登记的标签页 = 空栈, 不占位置
        for op in ["info", "undo", "redo", "clear"] {
            let res = r.run(&req("t-ghost", op, None, None), 8).unwrap();
            assert_eq!((res.apply, res.undo_len, res.redo_len), (None, 0, 0));
        }
        assert!(r.tabs.is_empty());
    }
    #[test]
    fn the_least_recently_used_tab_is_evicted() {
        let mut r = Registry::default();
        for t in ["a", "b", "c"] {
            r.run(&req(t, "push", Some(patch(0, &[], &["x"], [0, 0])), None), 3).unwrap();
        }
        r.run(&req("a", "info", None, None), 3).unwrap(); // a 刚用过, b 变成最久没用的
        r.run(&req("d", "push", Some(patch(0, &[], &["x"], [0, 0])), None), 3).unwrap();
        let mut keys: Vec<&String> = r.tabs.keys().collect();
        keys.sort();
        assert_eq!(keys, ["a", "c", "d"]);
    }
    #[test]
    fn clearing_a_tab_releases_its_slot() {
        let mut r = Registry::default();
        r.run(&req("a", "push", Some(patch(0, &[], &["x"], [0, 0])), None), 3).unwrap();
        r.run(&req("a", "clear", None, None), 3).unwrap();
        assert!(r.tabs.is_empty());
    }
}
