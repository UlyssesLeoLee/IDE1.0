//! 测试共用的请求构造器和"把结果套回文本"(与前端一样), 仅测试编译。

use super::req_res::{EditReq, EditRes};
use super::run;

/// 只带命令和光标的最小请求; 其余用 `with_*` 链式补
pub fn req(cmd: &str, row: usize, col: usize) -> EditReq {
    EditReq { cmd: cmd.into(), row, col, ..Default::default() }
}

impl EditReq {
    pub fn with_text(mut self, text: &str) -> Self {
        self.text = Some(text.into());
        self
    }
    pub fn with_sel(mut self, sel: Option<[usize; 4]>) -> Self {
        self.sel = sel;
        self
    }
    pub fn with_box(mut self, b: [usize; 4]) -> Self {
        self.box_sel = Some(b);
        self
    }
    pub fn with_cursors(mut self, c: &[[usize; 2]]) -> Self {
        self.cursors = c.to_vec();
        self
    }
    /// 把整篇文档都交上去
    pub fn with_doc(mut self, lines: &[&str]) -> Self {
        self.total_rows = lines.len();
        self.rows = lines.iter().enumerate().map(|(i, s)| (i, s.to_string())).collect();
        self
    }
}

/// 整篇文档都交上去, 自动缩进 / 智能缩进打开 (回车类用例)
pub fn run_on(cmd: &str, row: usize, col: usize, sel: Option<[usize; 4]>, lines: &[&str]) -> EditRes {
    let mut r = req(cmd, row, col).with_sel(sel).with_doc(lines);
    r.auto_indent = true;
    r.smart_indent = true;
    run(&r).unwrap()
}

/// 带文字 (粘贴 / 键入) 的请求
pub fn run_text(cmd: &str, row: usize, col: usize, sel: Option<[usize; 4]>, text: &str, line_mode: bool, lines: &[&str]) -> EditRes {
    let mut r = req(cmd, row, col).with_sel(sel).with_text(text).with_doc(lines);
    r.line_mode = line_mode;
    run(&r).unwrap()
}

/// 矩形命令
pub fn run_box(cmd: &str, row: usize, col: usize, b: [usize; 4], text: &str, lines: &[&str]) -> EditRes {
    run(&req(cmd, row, col).with_text(text).with_box(b).with_doc(lines)).unwrap()
}

/// 多光标命令: 只交光标所在的行和它上面一行 (和前端一样)
pub fn run_multi(cmd: &str, cursors: &[[usize; 2]], text: &str, lines: &[&str]) -> EditRes {
    let mut touched: Vec<usize> = cursors.iter().flat_map(|c| [c[0].saturating_sub(1), c[0]]).collect();
    touched.sort();
    touched.dedup();
    let mut r = req(cmd, cursors[0][0], cursors[0][1]).with_text(text).with_cursors(cursors);
    r.total_rows = lines.len();
    r.rows = touched.into_iter().filter(|&i| i < lines.len()).map(|i| (i, lines[i].to_string())).collect();
    run(&r).unwrap()
}

/// 把结果套回文本 (和前端一样)
pub fn apply(lines: &[&str], r: &EditRes) -> Vec<String> {
    let mut v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    v.splice(r.start..r.start + r.remove, r.insert.clone());
    v
}

/// 把多光标的补丁套回文本 (前端从下往上套)
pub fn apply_multi(lines: &[&str], r: &EditRes) -> Vec<String> {
    let mut v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mut ps = r.patches.clone().unwrap();
    ps.sort_by_key(|p| std::cmp::Reverse(p.0));
    for (s, n, ins) in ps {
        v.splice(s..s + n, ins);
    }
    v
}
