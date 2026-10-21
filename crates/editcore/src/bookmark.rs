//! ブックマーク: 切り替え / 次へ / 前へ (ARCH-01: 移动与选择语义放 Rust).
//!
//! 无状态: 前端把当前文档的书签列表与光标交上来, 这里算出新列表 / 目标位置。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Bm {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BmReq {
    /// "toggle" | "next" | "prev"
    pub op: String,
    pub list: Vec<Bm>,
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct BmRes {
    /// toggle 之后的新列表
    pub list: Option<Vec<Bm>>,
    /// toggle: 是否是新增
    pub added: bool,
    /// next / prev: 目标位置与它在排序列表里的序号 (从 0 起) / 总数
    pub target: Option<Bm>,
    pub index: usize,
    pub count: usize,
}

pub fn run(req: &BmReq) -> Option<BmRes> {
    match req.op.as_str() {
        "toggle" => {
            let mut list = req.list.clone();
            let here = Bm { row: req.row, col: req.col };
            let added = match list.iter().position(|b| *b == here) {
                Some(at) => {
                    list.remove(at);
                    false
                }
                None => {
                    list.push(here);
                    true
                }
            };
            let count = list.len();
            Some(BmRes { list: Some(list), added, count, ..Default::default() })
        }
        "next" | "prev" => {
            let delta: isize = if req.op == "next" { 1 } else { -1 };
            let mut list = req.list.clone();
            list.sort_by_key(|b| (b.row, b.col));
            if list.is_empty() {
                return Some(BmRes::default());
            }
            let n = list.len() as isize;
            // 光标正好停在书签上 -> 取相邻的那个; 否则 "之后第一个"(next) / "之前最后一个"(prev), 到头回绕
            let idx = match list.iter().position(|b| b.row == req.row && b.col == req.col) {
                Some(i) => (i as isize + delta).rem_euclid(n),
                None => {
                    let after = list.iter().position(|b| b.row > req.row || (b.row == req.row && b.col > req.col)).unwrap_or(0) as isize;
                    if delta > 0 { after } else { (after - 1).rem_euclid(n) }
                }
            } as usize;
            Some(BmRes { target: Some(list[idx]), index: idx, count: list.len(), ..Default::default() })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(op: &str, list: &[(usize, usize)], row: usize, col: usize) -> BmReq {
        BmReq { op: op.into(), list: list.iter().map(|(r, c)| Bm { row: *r, col: *c }).collect(), row, col }
    }

    #[test]
    fn toggle_adds_and_removes() {
        let r = run(&req("toggle", &[], 2, 0)).unwrap();
        assert_eq!((r.added, r.count), (true, 1));
        let r = run(&req("toggle", &[(2, 0)], 2, 0)).unwrap();
        assert_eq!((r.added, r.count, r.list.unwrap().len()), (false, 0, 0));
    }
    #[test]
    fn next_prev_wrap() {
        let l = [(1, 0), (3, 0)];
        assert_eq!(run(&req("next", &l, 0, 0, )).unwrap().target, Some(Bm { row: 1, col: 0 }));
        assert_eq!(run(&req("next", &l, 1, 0)).unwrap().target, Some(Bm { row: 3, col: 0 }));
        assert_eq!(run(&req("next", &l, 3, 0)).unwrap().target, Some(Bm { row: 1, col: 0 })); // 回绕
        assert_eq!(run(&req("prev", &l, 5, 0)).unwrap().target, Some(Bm { row: 3, col: 0 }));
        assert_eq!(run(&req("prev", &l, 0, 0)).unwrap().target, Some(Bm { row: 3, col: 0 })); // 回绕
        assert_eq!(run(&req("prev", &l, 2, 0)).unwrap().target, Some(Bm { row: 1, col: 0 }));
    }
    #[test]
    fn empty_list_has_no_target() {
        assert_eq!(run(&req("next", &[], 0, 0)).unwrap().target, None);
        assert!(run(&req("nope", &[], 0, 0)).is_none());
    }
}
