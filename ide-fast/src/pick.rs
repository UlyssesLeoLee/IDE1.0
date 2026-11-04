//! 通用「选择列表」窗口: 一个过滤框 + 一列可选的行, 方向键 / Enter / Esc / 点击。
//! 窗口一览、最近的文件 / 文件夹、插入最近的路径、字符集、帮助检索都用它, 各自只管准备行和处理选中的那一行。
//! 这里不碰文档, 也不执行任何动作: 选中后只把 (种类, 行号) 还给 `App`。

use crate::keymap;
use eframe::egui::{self, Key, Modifiers};

/// 这个列表是干什么的 (App 据此决定选中后做什么)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// 窗口一览: 行 = 标签, 选中 = 切过去
    Windows,
    /// 最近的文件: 选中 = 打开
    Recent,
    /// 最近的文件夹: 选中 = 作为文件夹打开
    Folders,
    /// 收藏夹: 最近的文件 + 最近的文件夹
    Favorite,
    /// 插入最近的文件路径
    InsFile,
    /// 插入最近的文件夹路径
    InsDir,
    /// 改当前文档的字符集 (下次保存生效)
    Charset,
    /// 用所选字符集重新打开
    Reopen,
    /// 帮助检索: 行 = 手册里的一行
    Help,
}

/// 怎么按查询串过滤
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Match {
    /// 子序列模糊匹配, 分数高的在前 (命令面板同款)
    Fuzzy,
    /// 每个空格分开的词都要出现 (忽略大小写), 保持原来的顺序 (帮助检索: 要按手册顺序读)
    Words,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    /// 右侧的灰字 (路径 / 来源 / 键)
    pub detail: String,
    /// 选中后真正要用的值 (标签 id / 路径 / 手册行号…), 默认就是 `label`。
    /// App 按它找目标, 不按行号回头去查当前的列表 —— 列表开着期间标签 / 最近文件都可能变
    pub value: String,
}

impl Row {
    pub fn new(label: impl Into<String>, detail: impl Into<String>) -> Self {
        let label = label.into();
        Row { value: label.clone(), label, detail: detail.into() }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }
}

/// 列表里最多画这么多行 (帮助手册有几百行, 全画没有意义)
pub const MAX_ROWS: usize = 200;

/// 按查询串过滤: 返回命中的行号。空查询 = 全部 (原顺序)
pub fn filter(rows: &[Row], query: &str, mode: Match) -> Vec<usize> {
    let q = query.trim();
    if q.is_empty() {
        return (0..rows.len()).collect();
    }
    match mode {
        Match::Fuzzy => {
            let mut hits: Vec<(i32, usize)> = rows.iter().enumerate().filter_map(|(i, r)| keymap::fuzzy_score(q, &format!("{} {}", r.label, r.detail)).map(|s| (s, i))).collect();
            hits.sort_by_key(|&(s, i)| (std::cmp::Reverse(s), i));
            hits.into_iter().map(|(_, i)| i).collect()
        }
        Match::Words => {
            let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
            rows.iter().enumerate().filter(|(_, r)| {
                let hay = format!("{} {}", r.label, r.detail).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            }).map(|(i, _)| i).collect()
        }
    }
}

pub struct Pick {
    pub kind: Kind,
    pub title: String,
    pub mode: Match,
    /// 这个列表是替哪个文档开的 (字符集 / 插入路径): 选中时作用在它上面, 不是选中那一刻的活动标签
    pub target: Option<u64>,
    rows: Vec<Row>,
    query: String,
    sel: usize,
    /// 选中行刚被方向键改过: 这一帧要把它滚进可见范围
    moved: bool,
}

/// 一帧里窗口的结果
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// 还开着
    Open,
    Closed,
    /// 选中了第几行 (下标指向 `rows()`, 不是过滤后的位置)
    Picked(usize),
}

impl Pick {
    pub fn new(kind: Kind, title: impl Into<String>, mode: Match, rows: Vec<Row>) -> Self {
        Pick { kind, title: title.into(), mode, target: None, rows, query: String::new(), sel: 0, moved: true }
    }

    pub fn for_doc(mut self, id: u64) -> Self {
        self.target = Some(id);
        self
    }

    /// 打开时高亮这一行 (行号不在范围内就当没设)
    pub fn start_at(mut self, row: usize) -> Self {
        if row < self.rows.len() {
            self.sel = row;
        }
        self
    }

    /// 预填过滤框 (帮助检索: 光标下的词)
    pub fn with_query(mut self, q: &str) -> Self {
        self.query = q.to_string();
        self.sel = 0;
        self
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    #[cfg(test)]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// 当前过滤结果 (行号)
    pub fn visible(&self) -> Vec<usize> {
        filter(&self.rows, &self.query, self.mode)
    }

    /// 方向键 / Enter / Esc 的纯逻辑 (窗口只负责把按键事件翻成这些参数): 返回选中或关闭
    #[cfg(test)]
    pub fn key(&mut self, down: bool, up: bool, enter: bool, esc: bool) -> Option<Outcome> {
        let vis = self.visible();
        self.step(&vis, down, up, enter, esc)
    }

    /// `key` 的本体, 过滤结果由调用方给 (窗口一帧只过滤一次)。选中行只在画出来的前 `MAX_ROWS` 行里移动
    fn step(&mut self, vis: &[usize], down: bool, up: bool, enter: bool, esc: bool) -> Option<Outcome> {
        if esc {
            return Some(Outcome::Closed);
        }
        let last = vis.len().min(MAX_ROWS).saturating_sub(1);
        if down { self.sel = (self.sel + 1).min(last); self.moved = true; }
        if up { self.sel = self.sel.saturating_sub(1); self.moved = true; }
        self.sel = self.sel.min(last);
        if enter {
            return vis.get(self.sel).map(|&r| Outcome::Picked(r));
        }
        None
    }

    /// 画窗口 (过滤框拿着焦点, 编辑区在这期间不吃键盘)
    pub fn window(&mut self, ctx: &egui::Context) -> Outcome {
        let (down, up, enter, esc) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape)));
        let vis = self.visible();
        if let Some(o) = self.step(&vis, down, up, enter, esc) {
            return o;
        }
        let mut picked: Option<usize> = None;
        let mut open = true;
        egui::Window::new(self.title.clone()).open(&mut open).collapsible(false).resizable(true).default_width(560.0).default_height(340.0).anchor(egui::Align2::CENTER_TOP, [0.0, 56.0]).show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text(crate::i18n::trz("输入以过滤")).desired_width(f32::INFINITY));
            r.request_focus();
            if r.changed() { self.sel = 0; self.moved = true; }
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if vis.is_empty() { ui.weak(crate::i18n::trz("没有匹配的项")); }
                for (n, &ri) in vis.iter().take(MAX_ROWS).enumerate() {
                    let row = &self.rows[ri];
                    ui.horizontal(|ui| {
                        let resp = ui.selectable_label(n == self.sel, &row.label);
                        if resp.clicked() { picked = Some(ri); }
                        if n == self.sel && self.moved { resp.scroll_to_me(None); }
                        if !row.detail.is_empty() { ui.weak(&row.detail); }
                    });
                }
                if vis.len() > MAX_ROWS { ui.weak(format!("… {}", vis.len() - MAX_ROWS)); }
            });
            self.moved = false;
        });
        match picked {
            Some(r) => Outcome::Picked(r),
            None if !open => Outcome::Closed,
            None => Outcome::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        vec![Row::new("main.rs", "src/main.rs"), Row::new("notes.txt", "C:/notes.txt"), Row::new("README.md", "README.md"), Row::new("Cargo.toml", "")]
    }

    #[test]
    fn empty_query_keeps_every_row_in_order() {
        assert_eq!(filter(&rows(), "", Match::Fuzzy), vec![0, 1, 2, 3]);
        assert_eq!(filter(&rows(), "  ", Match::Words), vec![0, 1, 2, 3]);
    }

    #[test]
    fn fuzzy_matches_subsequences_and_ranks_tighter_first() {
        let hits = filter(&rows(), "mrs", Match::Fuzzy);
        assert_eq!(hits.first(), Some(&0), "main.rs 最贴近 mrs");
        assert!(!hits.contains(&3), "Cargo.toml 不含 m-r-s 子序列");
        assert!(filter(&rows(), "zzz", Match::Fuzzy).is_empty());
    }

    #[test]
    fn words_need_every_word_and_ignore_case_but_keep_order() {
        let r = vec![Row::new("保存 Save the file", "Ctrl+S"), Row::new("另存为 SAVE AS", ""), Row::new("打开", "Ctrl+O")];
        assert_eq!(filter(&r, "save", Match::Words), vec![0, 1], "不分大小写, 顺序不变");
        assert_eq!(filter(&r, "save ctrl", Match::Words), vec![0], "每个词都得出现 (细节列也算)");
        assert!(filter(&r, "save open", Match::Words).is_empty());
    }

    #[test]
    fn keys_move_clamp_and_pick_through_the_filter() {
        let mut p = Pick::new(Kind::Windows, "t", Match::Fuzzy, rows());
        assert_eq!(p.key(false, true, false, false), None, "第一行再往上还是第一行");
        assert_eq!(p.key(true, false, false, false), None);
        assert_eq!(p.key(false, false, true, false), Some(Outcome::Picked(1)));
        for _ in 0..9 { p.key(true, false, false, false); }
        assert_eq!(p.key(false, false, true, false), Some(Outcome::Picked(3)), "往下到底不越界");
        assert_eq!(p.key(false, false, false, true), Some(Outcome::Closed));
    }

    #[test]
    fn picked_index_points_into_the_unfiltered_rows() {
        let mut p = Pick::new(Kind::Recent, "t", Match::Fuzzy, rows()).with_query("readme");
        assert_eq!(p.visible(), vec![2]);
        assert_eq!(p.key(false, false, true, false), Some(Outcome::Picked(2)), "选中的是原列表里的第 2 行, 不是过滤后的第 0 行");
        let mut none = Pick::new(Kind::Recent, "t", Match::Fuzzy, rows()).with_query("qqqq");
        assert_eq!(none.key(false, false, true, false), None, "没有匹配时 Enter 什么也不选");
    }

    #[test]
    fn selection_stays_within_the_rows_that_are_drawn() {
        let many: Vec<Row> = (0..MAX_ROWS + 50).map(|i| Row::new(format!("r{i}"), "")).collect();
        let mut p = Pick::new(Kind::Help, "t", Match::Words, many);
        for _ in 0..MAX_ROWS + 40 { p.key(true, false, false, false); }
        assert_eq!(p.key(false, false, true, false), Some(Outcome::Picked(MAX_ROWS - 1)), "最多到画出来的最后一行");
    }

    #[test]
    fn value_defaults_to_the_label_and_can_be_replaced() {
        assert_eq!(Row::new("C:/a.txt", "").value, "C:/a.txt");
        assert_eq!(Row::new("▶ a.txt", "").with_value("7").value, "7");
        assert_eq!(Pick::new(Kind::Charset, "t", Match::Fuzzy, rows()).for_doc(3).target, Some(3));
    }

    #[test]
    fn start_row_is_highlighted_when_in_range() {
        let mut p = Pick::new(Kind::Windows, "t", Match::Fuzzy, rows()).start_at(2);
        assert_eq!(p.key(false, false, true, false), Some(Outcome::Picked(2)));
        let mut q = Pick::new(Kind::Windows, "t", Match::Fuzzy, rows()).start_at(99);
        assert_eq!(q.key(false, false, true, false), Some(Outcome::Picked(0)), "越界的起点忽略");
    }

    fn key_event(key: Key) -> egui::Event {
        egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: Modifiers::NONE }
    }

    /// 真的跑一帧: 窗口画得出来 (不 panic), 没有按键时一直开着; 方向键 + Enter / Esc 经过 egui 的按键消费走到结果
    #[test]
    fn the_window_draws_and_reads_keys_through_egui() {
        let ctx = egui::Context::default();
        let mut p = Pick::new(Kind::Windows, "窗口一览", Match::Fuzzy, rows());
        let frame = |p: &mut Pick, events: Vec<egui::Event>| {
            let mut out = Outcome::Open;
            let mut full = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| { out = p.window(ui.ctx()); });
            full.textures_delta.clear();
            out
        };
        assert_eq!(frame(&mut p, vec![]), Outcome::Open);
        assert_eq!(frame(&mut p, vec![key_event(Key::ArrowDown)]), Outcome::Open);
        assert_eq!(frame(&mut p, vec![key_event(Key::Enter)]), Outcome::Picked(1), "下移一行后回车 = 第 1 行");
        assert_eq!(frame(&mut p, vec![key_event(Key::Escape)]), Outcome::Closed);
    }
}
