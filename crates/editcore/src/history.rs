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
}

fn registry() -> &'static Mutex<HashMap<String, History>> {
    static R: OnceLock<Mutex<HashMap<String, History>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn run(req: &HistReq) -> Option<HistRes> {
    let mut reg = registry().lock().ok()?;
    // 上限: 页面重新载入时旧会话的标签不会再来清理, 登记数到上限就丢掉一个旧的, 内存不会无限涨
    if !reg.contains_key(&req.tab) && reg.len() >= MAX_TABS {
        if let Some(k) = reg.keys().next().cloned() {
            reg.remove(&k);
        }
    }
    let h = reg.entry(req.tab.clone()).or_default();
    let mut apply = None;
    match req.op.as_str() {
        "push" => {
            let p = req.patch.clone()?;
            h.undo.push(p);
            if h.undo.len() > MAX_UNDO {
                h.undo.remove(0);
            }
            h.redo.clear();
        }
        "undo" => {
            if let Some(mut p) = h.undo.pop() {
                apply = Some(Apply { start: p.start, remove: p.inserted.len(), insert: p.removed.clone(), cursor: p.cursor_before });
                if let Some(c) = req.cursor {
                    p.cursor_after = c;
                }
                h.redo.push(p);
            }
        }
        "redo" => {
            if let Some(p) = h.redo.pop() {
                apply = Some(Apply { start: p.start, remove: p.removed.len(), insert: p.inserted.clone(), cursor: p.cursor_after });
                h.undo.push(p);
            }
        }
        "clear" => {
            h.undo.clear();
            h.redo.clear();
        }
        "info" => {}
        _ => return None,
    }
    let res = HistRes { apply, undo_len: h.undo.len(), redo_len: h.redo.len() };
    if req.op == "clear" && res.undo_len == 0 {
        reg.remove(&req.tab);
    }
    Some(res)
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
}
