//! 查找栏: 选项 / 边输入边找 (增量) / 全部强调 / 置换。
//! 查找语义全在 `Doc` (-> `editcore::search_ops`, 和 Tauri 版同一套): F3、增量查找、强调用的是同一个 `step` / `hits`,
//! 所以三者落在同一处。这里只管状态和那两排控件, 不碰文档内部。

use crate::doc::Doc;
use eframe::egui::{self, Key};
use editcore::search_ops::Hit;
use std::collections::HashMap;
use std::sync::Arc;

/// 查找条件 (Sakura 的 检索 对话框那几个勾)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FindOpts {
    pub case: bool,
    pub regex: bool,
    pub word: bool,
    /// 到头后从另一端接着找
    pub wrap: bool,
    /// Migemo: 用罗马字找日文 (kensaku 能找到 検索 / けんさく / ケンサク); 内部换成一个转义过的候选正则
    pub migemo: bool,
}

impl Default for FindOpts {
    fn default() -> Self {
        FindOpts { case: false, regex: false, word: false, wrap: true, migemo: false }
    }
}

/// 罗马字 -> Migemo 候选的正则 (`(?:kensaku|けんさく|ケンサク|...)`, 字面量已转义)。
/// 空串 / 太长 (> 48 字符, 候选会爆炸) / 没有候选 = None, 调用方退回普通查找。
pub fn migemo_regex(roma: &str) -> Option<String> {
    let roma = roma.trim();
    if roma.is_empty() || roma.chars().count() > 48 {
        return None;
    }
    let mut alts: Vec<String> = editcore::migemo::expand_pattern(roma).into_iter().filter(|p| !p.is_empty()).take(64).map(|p| regex_escape(&p)).collect();
    alts.dedup();
    if alts.is_empty() { None } else { Some(format!("(?:{})", alts.join("|"))) }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) { out.push('\\'); }
        out.push(c);
    }
    out
}

impl FindOpts {
    /// 键盘宏里的检索标志 (`S_SearchNext` 等) 换成查找条件
    pub fn from_flags(f: &editcore::macro_sakura::SearchFlags) -> Self {
        FindOpts { case: f.case_sensitive, regex: f.regex, word: f.word, wrap: f.wrap, migemo: false }
    }
}

/// 强调的命中数上限 (editcore 的 hits 到 5001 就停; 超过显示 "5000+")
pub const MARK_CAP: usize = 5000;
/// 行数多到这个程度就不做全部强调 (每次改动都要重扫整篇)
const MARK_MAX_ROWS: usize = 200_000;

/// 一个文档的强调缓存: 版本号 / 条件对得上才算数
struct Marks {
    rev: u64,
    needle: String,
    opts: FindOpts,
    hits: Arc<Vec<Hit>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    None,
    Find,
    Repl,
}

pub struct Find {
    pub open: bool,
    /// 显示置换那一行
    pub replace: bool,
    pub needle: String,
    pub repl: String,
    pub opts: FindOpts,
    /// 全部强调
    pub marks: bool,
    /// 全部置换只作用在选区里
    pub in_sel: bool,
    pub msg: String,
    /// 正则写错了的说明 (边输入边检查)
    pub err: Option<String>,
    /// 独立增量查找条 (isearch) 的方向: Some(true) = 向后, Some(false) = 向前, None = 普通查找栏
    pub isearch: Option<bool>,
    focus: Focus,
    /// 增量查找的起点: 打开查找栏时 (或上一次明确"找下一个"之后) 的位置
    start: [usize; 2],
    /// 上一帧的 (查找串, 条件): 变了才重新增量查找
    last: Option<(String, FindOpts)>,
    cache: HashMap<u64, Marks>,
}

impl Find {
    pub fn new() -> Self {
        Find { open: false, replace: false, needle: String::new(), repl: String::new(), opts: FindOpts::default(), marks: true, in_sel: false, msg: String::new(), err: None, isearch: None, focus: Focus::None, start: [0, 0], last: None, cache: HashMap::new() }
    }

    /// 打开查找栏: 单行选区的文字作查找串, 记下增量查找的起点; `replace` = 同时显示置换行并把焦点放在它上面
    pub fn open_with(&mut self, d: &Doc, replace: bool) {
        if let Some(t) = d.selected_text_single() {
            self.needle = if self.opts.regex { escape_regex(&t) } else { t };
        }
        self.start = d.sel.map(|s| [s[0], s[1]]).unwrap_or(d.cursor);
        self.open = true;
        self.isearch = None;
        if replace {
            self.replace = true;
        }
        // 开着置换且查找串已有内容 (多半是刚从选区取来的) = 直接去填置换串
        self.focus = if replace && !self.needle.is_empty() { Focus::Repl } else { Focus::Find };
        self.err = d.find_error(&self.needle, self.opts);
        self.last = Some((self.needle.clone(), self.opts)); // 打开时不动选区
    }

    /// 打开独立的增量查找条 (Sakura 的 isearch): 查找串从空开始, 边输入边从光标处往 `fwd` 方向找; 正则 / Migemo 由命令决定
    pub fn open_isearch(&mut self, d: &Doc, fwd: bool, regex: bool, migemo: bool) {
        self.opts.regex = regex;
        self.opts.migemo = migemo;
        self.needle.clear();
        self.replace = false;
        self.isearch = Some(fwd);
        self.start = d.sel.map(|s| [s[0], s[1]]).unwrap_or(d.cursor);
        self.open = true;
        self.focus = Focus::Find;
        self.err = None;
        self.msg.clear();
        self.last = Some((String::new(), self.opts));
    }

    /// 找下一处 / 上一处 (F3 / Shift+F3); 查找串是空的就拿选区或光标下的单词
    pub fn next(&mut self, d: &mut Doc, fwd: bool) {
        if self.needle.is_empty() && !self.seed_from_doc(d) {
            self.msg = "没有可找的内容".into();
            return;
        }
        self.step(d, fwd);
    }

    /// Ctrl+F3: 用选区 / 光标下的单词作查找串, 然后找下一处
    pub fn word_search(&mut self, d: &mut Doc, fwd: bool) {
        if !self.seed_from_doc(d) {
            return;
        }
        self.step(d, fwd);
    }

    fn seed_from_doc(&mut self, d: &Doc) -> bool {
        let Some(t) = d.selected_text_single().or_else(|| d.word_at_cursor()) else { return false };
        self.needle = if self.opts.regex { escape_regex(&t) } else { t };
        true
    }

    fn step(&mut self, d: &mut Doc, fwd: bool) {
        self.err = d.find_error(&self.needle, self.opts);
        if self.err.is_some() {
            return;
        }
        match d.find_step(&self.needle, self.opts, fwd) {
            Ok(found) => {
                self.msg = d.tip.clone();
                if found {
                    self.start = d.sel.map(|s| [s[0], s[1]]).unwrap_or(d.cursor); // 之后再改查找串, 从这里接着找
                }
            }
            Err(e) => self.err = Some(e),
        }
        self.last = Some((self.needle.clone(), self.opts));
    }

    /// 置换当前命中并跳到下一处 ($1 \1 $& 在正则下展开)
    pub fn replace_one(&mut self, d: &mut Doc) {
        self.err = d.find_error(&self.needle, self.opts);
        if self.err.is_some() {
            return;
        }
        match d.replace_one(&self.needle, &self.repl, self.opts) {
            Ok(_) => {
                self.msg = d.tip.clone();
                self.start = d.sel.map(|s| [s[0], s[1]]).unwrap_or(d.cursor);
            }
            Err(e) => self.err = Some(e),
        }
        self.last = Some((self.needle.clone(), self.opts));
    }

    pub fn replace_all(&mut self, d: &mut Doc) {
        self.err = d.find_error(&self.needle, self.opts);
        if self.err.is_some() {
            return;
        }
        match d.replace_all(&self.needle, &self.repl, self.opts, self.in_sel) {
            Ok(_) => self.msg = d.tip.clone(),
            Err(e) => self.err = Some(e),
        }
        self.last = Some((self.needle.clone(), self.opts));
    }

    /// 条件变了: 查正则、从起点找第一处 (边输入边找)
    fn changed(&mut self, d: &mut Doc) {
        self.err = d.find_error(&self.needle, self.opts);
        if self.needle.is_empty() || self.err.is_some() {
            self.msg.clear();
            return;
        }
        let r = match self.isearch {
            Some(false) => d.find_from_dir(&self.needle, self.opts, false, self.start),
            _ => d.find_from(&self.needle, self.opts, self.start),
        };
        match r {
            Ok(_) => self.msg = d.tip.clone(),
            Err(e) => self.err = Some(e),
        }
    }

    /// 当前文档的全部命中 (供编辑区画底色); 查找栏没开 / 没勾强调 / 查找串空 = None。同一 (版本, 查找串, 条件) 只算一次
    pub fn marks_for(&mut self, d: &Doc) -> Option<Arc<Vec<Hit>>> {
        if !self.open || !self.marks || self.needle.is_empty() || self.err.is_some() || d.lines.len() > MARK_MAX_ROWS {
            self.cache.clear();
            return None;
        }
        if let Some(m) = self.cache.get(&d.id) {
            if m.rev == d.rev && m.needle == self.needle && m.opts == self.opts {
                return Some(m.hits.clone());
            }
        }
        if self.cache.len() > 4 {
            self.cache.clear();
        }
        let hits = Arc::new(d.find_hits(&self.needle, self.opts).unwrap_or_default());
        self.cache.insert(d.id, Marks { rev: d.rev, needle: self.needle.clone(), opts: self.opts, hits: hits.clone() });
        Some(hits)
    }

    /// 命中数文字: 超过上限写 "5000+"
    pub fn count_label(&mut self, d: &Doc) -> String {
        match self.marks_for(d) {
            Some(h) if h.len() > MARK_CAP => format!("{MARK_CAP}+ 处"),
            Some(h) => format!("{} 处", h.len()),
            None => String::new(),
        }
    }

    /// 查找栏 (底部面板里画): 第一行查找串 + 选项, 第二行 (开着置换时) 置换串 + 按钮
    pub fn bar(&mut self, ui: &mut egui::Ui, d: &mut Doc) {
        if self.isearch.is_some() {
            self.isearch_bar(ui, d);
        } else {
            self.full_bar(ui, d);
        }
        // 查找串 / 条件变了就边输入边找 (打开栏、F3、置换之后已经同步过 last, 不会误跳)
        let now = (self.needle.clone(), self.opts);
        if self.last.as_ref() != Some(&now) {
            self.last = Some(now);
            self.changed(d);
        }
    }

    /// 增量查找条: 一行 — 方向 / 查找串 / 正则 / Migemo / 区分大小写; Enter 往同方向找下一处, Esc 关
    fn isearch_bar(&mut self, ui: &mut egui::Ui, d: &mut Doc) {
        let fwd = self.isearch.unwrap_or(true);
        ui.horizontal_wrapped(|ui| {
            ui.strong(crate::i18n::trz("增量查找"));
            if ui.selectable_label(fwd, "↓").on_hover_text(crate::i18n::trz("向后")).clicked() && !fwd { self.isearch = Some(true); self.last = None; }
            if ui.selectable_label(!fwd, "↑").on_hover_text(crate::i18n::trz("向前")).clicked() && fwd { self.isearch = Some(false); self.last = None; }
            let r = ui.add(egui::TextEdit::singleline(&mut self.needle).desired_width(260.0));
            if self.focus == Focus::Find {
                r.request_focus();
                self.focus = Focus::None;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let go = self.isearch.unwrap_or(true) != ui.input(|i| i.modifiers.shift);
                self.step(d, go);
                r.request_focus();
            }
            ui.checkbox(&mut self.opts.regex, crate::i18n::trz("正则"));
            ui.checkbox(&mut self.opts.migemo, "Migemo");
            ui.checkbox(&mut self.opts.case, crate::i18n::trz("区分大小写"));
            if let Some(e) = &self.err {
                ui.colored_label(egui::Color32::from_rgb(230, 80, 80), format!("{}: {e}", crate::i18n::trz("正则无效")));
            } else {
                ui.label(&self.msg);
            }
            if ui.button("×").clicked() { self.open = false; self.isearch = None; }
        });
    }

    fn full_bar(&mut self, ui: &mut egui::Ui, d: &mut Doc) {
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.replace, "置换").on_hover_text("显示 / 隐藏置换行 (Ctrl+H)").clicked() {
                self.replace = !self.replace;
            }
            ui.label("查找");
            let r = ui.add(egui::TextEdit::singleline(&mut self.needle).desired_width(220.0));
            if self.focus == Focus::Find {
                r.request_focus();
                self.focus = Focus::None;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let fwd = !ui.input(|i| i.modifiers.shift);
                self.next(d, fwd);
                r.request_focus();
            }
            if ui.button("↓").on_hover_text("下一个 F3").clicked() { self.next(d, true); }
            if ui.button("↑").on_hover_text("上一个 Shift+F3").clicked() { self.next(d, false); }
            ui.checkbox(&mut self.opts.case, "区分大小写");
            ui.checkbox(&mut self.opts.word, "单词单位");
            ui.checkbox(&mut self.opts.regex, "正则");
            ui.checkbox(&mut self.opts.wrap, "折返");
            ui.checkbox(&mut self.opts.migemo, "Migemo").on_hover_text("用罗马字找日文: kensaku 也能找到 検索 / けんさく / ケンサク");
            ui.checkbox(&mut self.marks, "全部强调");
            if let Some(e) = &self.err {
                ui.colored_label(egui::Color32::from_rgb(230, 80, 80), format!("正则无效: {e}"));
            } else {
                let n = self.count_label(d);
                if !n.is_empty() { ui.weak(n); }
                ui.label(&self.msg);
            }
            if ui.button("×").on_hover_text("关闭 Esc").clicked() { self.open = false; }
        });
        if self.replace {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(22.0);
                ui.label("置换");
                let r = ui.add(egui::TextEdit::singleline(&mut self.repl).desired_width(220.0));
                if self.focus == Focus::Repl {
                    r.request_focus();
                    self.focus = Focus::None;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    self.replace_one(d);
                    r.request_focus();
                }
                if ui.button("置换").on_hover_text("置换当前命中并找下一个").clicked() { self.replace_one(d); }
                if ui.button("全部置换").clicked() { self.replace_all(d); }
                ui.checkbox(&mut self.in_sel, "仅选区");
            });
        }
    }
}

/// 把文字里的正则元字符转义 (正则开着时拿选区 / 单词作查找串用)
pub fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(lines: &[&str]) -> Doc {
        let mut d = Doc::untitled(1, 1);
        d.lines = lines.iter().map(|s| s.to_string()).collect();
        d
    }

    #[test]
    fn open_seeds_needle_from_a_single_line_selection_and_remembers_the_start() {
        let mut d = doc(&["alpha beta", "gamma"]);
        d.sel = Some([0, 6, 0, 10]);
        d.cursor = [0, 10];
        let mut f = Find::new();
        f.open_with(&d, false);
        assert_eq!((f.needle.as_str(), f.start, f.open), ("beta", [0, 6], true));
        // 跨行选区不当作查找串
        let mut d2 = doc(&["a", "b"]);
        d2.sel = Some([0, 0, 1, 1]);
        let mut f2 = Find::new();
        f2.open_with(&d2, false);
        assert_eq!(f2.needle, "");
        // 正则开着: 元字符被转义
        let mut d3 = doc(&["a.b"]);
        d3.sel = Some([0, 0, 0, 3]);
        let mut f3 = Find::new();
        f3.opts.regex = true;
        f3.open_with(&d3, false);
        assert_eq!(f3.needle, "a\\.b");
    }

    #[test]
    fn typing_finds_incrementally_from_the_start_position() {
        let mut d = doc(&["xx foo", "bar", "foo foobar"]);
        d.cursor = [1, 0];
        let mut f = Find::new();
        f.open_with(&d, false);
        f.needle = "fo".into();
        f.changed(&mut d); // 从 (1,0) 起第一处 = 第 3 行
        assert_eq!(d.sel, Some([2, 0, 2, 2]));
        f.needle = "foob".into();
        f.changed(&mut d); // 起点不动: 仍从 (1,0) 起, 命中 foobar
        assert_eq!(d.sel, Some([2, 4, 2, 8]));
        f.needle = "nothing".into();
        f.changed(&mut d);
        assert!(f.msg.is_empty() || f.msg.contains("没有"), "{}", f.msg);
    }

    #[test]
    fn bad_regex_is_reported_and_does_not_move_the_selection() {
        let mut d = doc(&["a(b"]);
        let mut f = Find::new();
        f.opts.regex = true;
        f.open_with(&d, false);
        f.needle = "(".into();
        f.changed(&mut d);
        assert!(f.err.as_deref().unwrap_or("").contains("右括号"), "{:?}", f.err);
        assert_eq!(d.sel, None);
        f.opts.regex = false; // 不当正则就是普通字符
        f.changed(&mut d);
        assert!(f.err.is_none());
        assert_eq!(d.sel, Some([0, 1, 0, 2]));
    }

    #[test]
    fn marks_are_cached_per_doc_revision_and_conditions() {
        let mut d = doc(&["foo foo", "foo"]);
        let mut f = Find::new();
        f.open_with(&d, false);
        f.needle = "foo".into();
        let a = f.marks_for(&d).unwrap();
        assert_eq!(a.len(), 3);
        let b = f.marks_for(&d).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "条件和版本没变 = 同一份缓存");
        d.rev += 1;
        d.lines[1] = "bar".into();
        let c = f.marks_for(&d).unwrap();
        assert_eq!(c.len(), 2, "版本变了重算");
        f.opts.case = true;
        f.needle = "FOO".into();
        assert_eq!(f.marks_for(&d).unwrap().len(), 0, "条件变了重算");
        f.marks = false;
        assert!(f.marks_for(&d).is_none());
        f.marks = true;
        f.open = false;
        assert!(f.marks_for(&d).is_none(), "栏关了不算");
    }

    #[test]
    fn next_uses_the_word_under_the_cursor_when_nothing_was_searched() {
        let mut d = doc(&["let alpha = 1;", "alpha + 2"]);
        d.cursor = [0, 6];
        let mut f = Find::new();
        f.next(&mut d, true);
        assert_eq!(f.needle, "alpha");
        assert_eq!(d.sel, Some([1, 0, 1, 5]), "从光标往后找到第二处");
        f.word_search(&mut d, true);
        assert_eq!(d.sel, Some([0, 4, 0, 9]), "折返回第一处");
    }

    #[test]
    fn replace_one_expands_groups_then_moves_on() {
        let mut d = doc(&["ab12 cd34"]);
        let mut f = Find::new();
        f.opts.regex = true;
        f.open_with(&d, true);
        f.needle = r"([a-z]+)(\d+)".into();
        f.repl = "$2-$1".into();
        f.step(&mut d, true); // 选中 ab12
        f.replace_one(&mut d);
        assert_eq!(d.lines[0], "12-ab cd34");
        assert_eq!(d.sel, Some([0, 6, 0, 10]), "置换后选中下一处");
        f.replace_one(&mut d);
        assert_eq!(d.lines[0], "12-ab 34-cd");
        d.undo();
        assert_eq!(d.lines[0], "12-ab cd34", "一次置换 = 一步撤销");
    }

    #[test]
    fn migemo_pattern_covers_kana_and_escapes() {
        let p = migemo_regex("kensaku").expect("有候选");
        assert!(p.starts_with("(?:") && p.ends_with(')'));
        assert!(p.contains("kensaku"), "原文也在候选里");
        assert!(p.contains("けんさく") || p.contains("ケンサク"), "平假名 / 片假名候选: {p}");
        // 正则元字符被转义, 不会让用户输入变成正则
        let q = migemo_regex("a.b*").unwrap();
        assert!(q.contains("a\\.b\\*"), "{q}");
        assert!(migemo_regex("").is_none());
        assert!(migemo_regex(&"a".repeat(60)).is_none(), "太长就退回普通查找");
    }

    #[test]
    fn migemo_search_finds_japanese_with_roman_letters() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["abc".into(), "これは検索のテスト, けんさく, ケンサク".into()];
        let o = FindOpts { migemo: true, ..FindOpts::default() };
        assert!(d.find_step("kensaku", o, true).unwrap());
        assert_eq!(d.cursor[0], 1, "第二行才有日文");
        let hits = d.find_hits("kensaku", o).unwrap();
        assert!(hits.len() >= 2, "平假名和片假名都应命中, 实际 {}", hits.len());
        d.goto(0, 0);
        assert!(!matches!(d.find_step("kensaku", FindOpts::default(), true), Ok(true)), "关掉 Migemo 就是字面查找");
    }
}
