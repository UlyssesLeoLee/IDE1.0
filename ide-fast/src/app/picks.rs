//! 选择列表类命令: 窗口一览 / 最近的文件 / 最近的文件夹 / 收藏夹 / 插入最近的路径 / 字符集 / 以字符集重新打开,
//! 以及关键字帮助 (手册、检索、光标处的词、自动显示) 和侧栏切页 (大纲 / 文件树)。
//! 列表本身是 `pick.rs` 的通用窗口; 这里负责准备行、处理选中的那一行。

use super::*;
use crate::help;
use crate::pick::{Kind, Match, Outcome, Pick, Row};

/// 窗口一览的行: `▶` 标当前标签、`●` 标有未保存修改; 右侧是路径; 值是文档 id (选中时按 id 找标签)
pub fn window_rows<'a>(
    docs: impl Iterator<Item = (u64, &'a str, Option<&'a str>, bool)>,
    active: usize,
) -> Vec<Row> {
    docs.enumerate()
        .map(|(i, (id, name, path, dirty))| {
            Row::new(
                format!(
                    "{}{}{}",
                    if i == active { "▶ " } else { "　" },
                    if dirty { "● " } else { "" },
                    name
                ),
                path.unwrap_or(""),
            )
            .with_value(id.to_string())
        })
        .collect()
}

/// 一列路径: 整条路径当文字, 过滤时按路径任意位置匹配
pub fn path_rows(paths: &[String], what: &str) -> Vec<Row> {
    paths.iter().map(|p| Row::new(p.clone(), what)).collect()
}

/// 收藏夹: 最近的文件在前, 最近的文件夹在后; 值带前缀 `f:` (文件) / `d:` (文件夹)
pub fn favorite_rows(files: &[String], dirs: &[String]) -> Vec<Row> {
    let mut rows: Vec<Row> = files
        .iter()
        .map(|p| Row::new(p.clone(), trz("文件")).with_value(format!("f:{p}")))
        .collect();
    rows.extend(
        dirs.iter()
            .map(|p| Row::new(p.clone(), trz("文件夹")).with_value(format!("d:{p}"))),
    );
    rows
}

/// 字符集列表: 当前的字符集右边标「当前」
pub fn charset_rows(cur: Charset) -> Vec<Row> {
    Charset::ALL
        .iter()
        .map(|c| Row::new(c.label(), if *c == cur { trz("当前") } else { "" }))
        .collect()
}

impl App {
    /// 命令 → 打开对应的列表 / 页面 (`run_app` 转到这里)
    pub(super) fn run_pick(&mut self, a: KApp) {
        match a {
            KApp::WindowList => self.open_window_list(),
            KApp::PickRecent => self.open_path_pick(Kind::Recent),
            KApp::PickFolders => self.open_path_pick(Kind::Folders),
            KApp::PickFavorite => self.open_path_pick(Kind::Favorite),
            KApp::InsRecentFile => self.open_path_pick(Kind::InsFile),
            KApp::InsRecentDir => self.open_path_pick(Kind::InsDir),
            KApp::PickCharset => self.open_charset_pick(Kind::Charset),
            KApp::PickReopen => self.open_charset_pick(Kind::Reopen),
            KApp::PickSyntax => self.open_syntax_pick(),
            KApp::Help => self.open_manual(None),
            KApp::HelpSearch => self.open_help_search(String::new()),
            KApp::ExtHelp => {
                let w = self.word_under_cursor();
                if w.is_empty() {
                    self.cur().tip = "光标处没有单词".into();
                } else {
                    self.open_help_search(w);
                }
            }
            KApp::KeySearch => {
                self.cfg.view.key_search = !self.cfg.view.key_search;
                self.tools.key_word.clear();
                self.tools.key_pos = None;
                self.cur().tip = if self.cfg.view.key_search {
                    "关键字帮助自动显示: 开"
                } else {
                    "关键字帮助自动显示: 关"
                }
                .into();
            }
            KApp::PaneOutline => self.show_side(Tab::Outline),
            KApp::PaneFiles => self.show_side(Tab::Files),
            _ => {}
        }
    }

    /// 侧栏切到某一页并展开
    pub(super) fn show_side(&mut self, tab: Tab) {
        self.side.open = true;
        self.side.tab = tab;
        self.expand_dock(crate::panels::Dock::Side);
    }

    fn word_under_cursor(&mut self) -> String {
        let d = self.cur();
        help::word_at(
            d.lines.get(d.cursor[0]).map(String::as_str).unwrap_or(""),
            d.cursor[1],
        )
    }

    fn open_window_list(&mut self) {
        let active = self.active.min(self.docs.len().saturating_sub(1));
        let pathbufs: Vec<Option<String>> = self
            .docs
            .iter()
            .map(|d| d.path.as_ref().map(|p| p.display().to_string()))
            .collect();
        let rows = window_rows(
            self.docs
                .iter()
                .zip(&pathbufs)
                .map(|(d, p)| (d.id, d.name.as_str(), p.as_deref(), d.dirty)),
            active,
        );
        let title = format!("{} ({})", trz("窗口一览"), rows.len());
        self.tools.pick =
            Some(Pick::new(Kind::Windows, title, Match::Fuzzy, rows).start_at(active));
    }

    fn open_path_pick(&mut self, kind: Kind) {
        let (title, rows) = match kind {
            Kind::Recent => (trz("最近的文件"), path_rows(&self.recent, "")),
            Kind::Folders => (trz("最近的文件夹"), path_rows(&self.recent_dirs, "")),
            Kind::Favorite => (
                trz("收藏夹"),
                favorite_rows(&self.recent, &self.recent_dirs),
            ),
            Kind::InsFile => (trz("插入最近的文件路径"), path_rows(&self.recent, "")),
            _ => (
                trz("插入最近的文件夹路径"),
                path_rows(&self.recent_dirs, ""),
            ),
        };
        if rows.is_empty() {
            self.cur().tip = tr("ui.none").into();
            return;
        }
        let doc = self.cur().id;
        self.tools.pick = Some(Pick::new(kind, title, Match::Fuzzy, rows).for_doc(doc));
    }

    fn open_charset_pick(&mut self, kind: Kind) {
        let (cur, doc) = {
            let d = self.cur();
            (d.charset, d.id)
        };
        let title = if kind == Kind::Reopen {
            trz("指定字符集重新打开")
        } else {
            trz("指定字符集")
        };
        let at = Charset::ALL.iter().position(|c| *c == cur).unwrap_or(0);
        self.tools.pick = Some(
            Pick::new(kind, title, Match::Fuzzy, charset_rows(cur))
                .start_at(at)
                .for_doc(doc),
        );
    }

    fn open_help_search(&mut self, word: String) {
        let rows = help::rows(&help::manual());
        self.tools.pick = Some(
            Pick::new(Kind::Help, trz("关键字帮助检索"), Match::Words, rows).with_query(&word),
        );
    }

    pub(super) fn open_syntax_pick(&mut self) {
        let (cur, doc_id) = {
            let d = self.cur();
            (d.syntax.clone(), d.id)
        };
        let mut rows = Vec::new();
        let auto_label = trz("自动 (按扩展名)");
        let cur_label = trz("当前");
        rows.push(
            Row::new(
                format!("{} {}", if cur.is_none() { "▶" } else { "　" }, auto_label),
                if cur.is_none() { cur_label } else { "" },
            )
            .with_value(""),
        );
        let mut at = 0;
        for (idx, t) in self.registry.all().iter().enumerate() {
            let is_cur = cur.as_deref() == Some(&t.name);
            if is_cur {
                at = idx + 1;
            }
            let detail = if is_cur {
                format!("{} ({})", cur_label, t.exts.join(", "))
            } else {
                t.exts.join(", ")
            };
            rows.push(
                Row::new(
                    format!("{} {}", if is_cur { "▶" } else { "　" }, &t.name),
                    detail,
                )
                .with_value(t.name.clone()),
            );
        }
        let title = format!("{} (Ctrl+4)", trz("选择语言 (类型一览)"));
        self.tools.pick = Some(
            Pick::new(Kind::Syntax, title, Match::Fuzzy, rows)
                .start_at(at)
                .for_doc(doc_id),
        );
    }

    /// 手册开成(或刷新)只读标签; `line` 从 1 数, 有就跳过去
    pub(super) fn open_manual(&mut self, line: Option<usize>) {
        let text = help::manual();
        let idx = match self
            .docs
            .iter()
            .position(|d| d.name == help::MANUAL_NAME && d.path.is_none())
        {
            Some(i) => {
                // 命令一览跟着此刻的键位和语言走, 所以重开时整份换成新的 (撤销栈 / 修改标记 / 选区都不留, id 不变)
                let (id, rev) = (self.docs[i].id, self.docs[i].rev);
                let mut d = Self::text_doc(id, help::MANUAL_NAME, &text);
                d.rev = rev + 1;
                self.docs[i] = d;
                i
            }
            None => {
                self.open_text_doc(help::MANUAL_NAME, &text);
                self.docs.len() - 1
            }
        };
        self.select_tab(idx);
        if let Some(n) = line {
            self.docs[idx].goto(n.saturating_sub(1), 0);
        }
    }

    /// 画选择列表, 处理选中
    pub(super) fn pick_window(&mut self, ctx: &egui::Context) {
        let Some(p) = self.tools.pick.as_mut() else {
            return;
        };
        match p.window(ctx) {
            Outcome::Open => {}
            Outcome::Closed => self.tools.pick = None,
            Outcome::Picked(row) => {
                let Some(p) = self.tools.pick.take() else {
                    return;
                };
                if let Some(r) = p.rows().get(row) {
                    self.apply_pick(p.kind, r, p.target);
                }
            }
        }
    }

    /// 选中某一行之后做的事。目标按行里的值 (文档 id / 路径 / 行号) 和打开列表时记下的文档 `target` 找,
    /// 不按行号回头查现在的列表 (列表开着期间标签 / 最近文件都可能变); 找不到就什么也不做
    pub(super) fn apply_pick(&mut self, kind: Kind, row: &Row, target: Option<u64>) {
        let doc_of = |a: &App, id: u64| a.docs.iter().position(|d| d.id == id);
        let target_ix = target.and_then(|id| doc_of(self, id));
        let charset = Charset::ALL
            .iter()
            .copied()
            .find(|c| c.label() == row.value);
        match kind {
            Kind::Windows => {
                if let Some(i) = row.value.parse().ok().and_then(|id| doc_of(self, id)) {
                    self.select_tab(i);
                }
            }
            Kind::Recent => self.open_path(std::path::Path::new(&row.value)),
            Kind::Folders => self.open_folder(PathBuf::from(&row.value)),
            Kind::Favorite => {
                if let Some(p) = row.value.strip_prefix("f:") {
                    self.open_path(std::path::Path::new(p));
                } else if let Some(p) = row.value.strip_prefix("d:") {
                    self.open_folder(PathBuf::from(p));
                }
            }
            Kind::InsFile | Kind::InsDir => {
                let Some(i) = target_ix else { return };
                self.select_tab(i);
                self.insert_text(&row.value);
            }
            Kind::Charset => {
                if let (Some(i), Some(cs)) = (target_ix, charset) {
                    self.docs[i].set_charset(cs);
                }
            }
            Kind::Reopen => {
                if let (Some(i), Some(cs)) = (target_ix, charset) {
                    self.select_tab(i);
                    self.reopen_as(cs);
                }
            }
            Kind::Help => self.open_manual(row.value.parse().ok()),
            Kind::Syntax => {
                let i = target_ix.unwrap_or(self.active);
                let lang_name = if row.value.is_empty() {
                    None
                } else {
                    Some(row.value.clone())
                };
                self.docs[i].syntax = lang_name.clone();
                let tip = format!(
                    "{}: {}",
                    trz("语言"),
                    lang_name.as_deref().unwrap_or(trz("自动 (按扩展名)"))
                );
                self.docs[i].tip = tip;
            }
        }
    }

    /// 关键字帮助自动显示: 光标换到新的单词上时, 把手册里第一处提到它的行放进状态栏提示
    pub(super) fn key_search_tick(&mut self) {
        if !self.cfg.view.key_search {
            return;
        }
        let (id, cur) = {
            let d = self.cur();
            (d.id, d.cursor)
        };
        if self.tools.key_pos == Some((id, cur)) {
            return;
        }
        self.tools.key_pos = Some((id, cur));
        let word = self.word_under_cursor();
        if word == self.tools.key_word {
            return;
        }
        self.tools.key_word = word.clone();
        if let Some(t) = help::tip_for(&word) {
            self.cur().tip = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Key, Modifiers};

    #[test]
    fn window_rows_mark_the_active_tab_and_unsaved_ones() {
        let rows = window_rows(
            [
                (7, "a.txt", Some("C:/a.txt"), false),
                (8, "b.txt", None, true),
                (9, "c.txt", Some("D:/c.txt"), false),
            ]
            .into_iter(),
            1,
        );
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].label, "　a.txt");
        assert_eq!(rows[1].label, "▶ ● b.txt", "当前标签 + 未保存");
        assert_eq!(
            (rows[1].detail.as_str(), rows[2].detail.as_str()),
            ("", "D:/c.txt")
        );
        assert_eq!(rows[2].value, "9", "值是文档 id");
    }

    #[test]
    fn favorites_list_files_first_and_tag_folders() {
        let rows = favorite_rows(&["a".into(), "b".into()], &["d".into()]);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            (
                rows[0].value.as_str(),
                rows[2].label.as_str(),
                rows[2].value.as_str()
            ),
            ("f:a", "d", "d:d")
        );
    }

    #[test]
    fn charset_rows_mark_only_the_current_one() {
        let rows = charset_rows(Charset::ShiftJis);
        assert_eq!(rows.len(), Charset::ALL.len());
        assert_eq!(rows.iter().filter(|r| !r.detail.is_empty()).count(), 1);
        assert_eq!(rows[1].label, "SJIS");
        assert!(!rows[1].detail.is_empty());
    }

    fn app_with_docs(names: &[&str]) -> (App, std::sync::MutexGuard<'static, ()>) {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut a = App::build(
            vec![],
            None,
            crate::Trace::new(),
            crate::settings::Settings::default(),
            false,
        );
        for n in names {
            a.new_doc();
            a.cur().name = (*n).to_string();
        }
        (a, g)
    }

    #[test]
    fn window_list_picks_the_tab_by_row() {
        let (mut a, _g) = app_with_docs(&["one", "two", "three"]);
        a.run_pick(KApp::WindowList);
        let p = a.tools.pick.as_ref().expect("窗口一览打开了");
        assert_eq!(p.kind, Kind::Windows);
        assert_eq!(p.rows().len(), a.docs.len());
        let rows = p.rows().to_vec();
        a.apply_pick(Kind::Windows, &rows[0], None);
        assert_eq!(a.active, 0);
        // 列表开着期间关掉了第 1 个标签: 选 "two" 那一行仍然切到 "two", 不是按行号落到 "three"
        a.close_tab(1);
        a.apply_pick(Kind::Windows, &rows[2], None);
        assert_eq!(a.cur().name, "two");
        a.apply_pick(Kind::Windows, &rows[1], None);
        assert_eq!(a.cur().name, "two", "已经关掉的标签什么也不做");
    }

    #[test]
    fn recent_lists_insert_paths_and_ignore_stale_rows() {
        let (mut a, _g) = app_with_docs(&["x"]);
        a.recent = vec!["C:/a.txt".into(), "C:/b.txt".into()];
        a.recent_dirs = vec!["C:/proj".into()];
        let x = a.cur().id;
        a.run_pick(KApp::InsRecentFile);
        let p = a.tools.pick.take().expect("插入最近的文件路径");
        assert_eq!(p.target, Some(x));
        a.select_tab(0);
        a.apply_pick(Kind::InsFile, &p.rows()[1], p.target);
        a.apply_pick(Kind::InsDir, &Row::new("C:/proj", ""), p.target);
        assert_eq!(a.cur().id, x, "插到打开列表时的那个文档");
        assert!(
            a.cur().lines.join("\n").contains("C:/b.txt")
                && a.cur().lines.join("\n").contains("C:/proj")
        );
        let before = a.cur().lines.clone();
        a.apply_pick(Kind::InsFile, &Row::new("C:/z", ""), Some(9999));
        assert_eq!(a.cur().lines, before, "目标文档不在了就不插入");
        a.recent.clear();
        a.recent_dirs.clear();
        a.run_pick(KApp::PickRecent);
        assert!(a.tools.pick.is_none(), "没有最近的文件就不开空列表");
    }

    #[test]
    fn charset_pick_changes_the_saved_charset_of_the_current_doc() {
        let (mut a, _g) = app_with_docs(&["x", "y"]);
        a.select_tab(1);
        a.run_pick(KApp::PickCharset);
        let p = a.tools.pick.take().expect("字符集列表");
        a.select_tab(2); // 列表开着期间换了标签
        a.apply_pick(Kind::Charset, &p.rows()[1], p.target);
        assert_eq!(
            (a.docs[1].charset, a.docs[2].charset),
            (Charset::ShiftJis, Charset::Utf8),
            "改的是打开列表时的文档"
        );
        a.apply_pick(Kind::Charset, &Row::new("NO-SUCH", ""), p.target);
        assert_eq!(a.docs[1].charset, Charset::ShiftJis);
    }

    #[test]
    fn manual_opens_once_as_a_read_only_tab_and_jumps_to_the_line() {
        let (mut a, _g) = app_with_docs(&["x"]);
        let n = a.docs.len();
        a.open_manual(Some(5));
        assert_eq!(a.docs.len(), n + 1);
        assert!(a.cur().readonly && a.cur().name == help::MANUAL_NAME);
        assert_eq!(a.cur().cursor[0], 4, "第 5 行 = 下标 4");
        let m = a.active;
        let id = a.docs[m].id;
        a.docs[m].readonly = false;
        a.docs[m].type_text("x");
        assert!(a.docs[m].dirty);
        a.select_tab(0);
        a.open_manual(None);
        assert_eq!(a.docs.len(), n + 1, "再开不会多出一个标签");
        assert_eq!(a.active, m);
        a.docs[m].undo(); // 撤销栈清空了: 旧的记录不会套到新正文上
        let d = &a.docs[m];
        assert_eq!(d.name, help::MANUAL_NAME);
        assert!(
            d.id == id && d.readonly && !d.dirty,
            "刷新后是干净的只读手册, id 不变"
        );
        assert_eq!(d.lines.join("\n"), help::manual().trim_end_matches('\n'));
    }

    #[test]
    fn help_search_starts_with_the_word_under_the_cursor() {
        let (mut a, _g) = app_with_docs(&["x"]);
        a.cur().lines = vec!["宏登记 here".into()];
        a.cur().cursor = [0, 1];
        a.run_pick(KApp::ExtHelp);
        let p = a.tools.pick.as_ref().expect("检索打开了");
        assert_eq!((p.kind, p.query()), (Kind::Help, "宏登记"));
        assert!(!p.visible().is_empty(), "手册里讲到宏登记");
        a.tools.pick = None;
        a.cur().lines = vec!["   ".into()];
        a.cur().cursor = [0, 1];
        a.run_pick(KApp::ExtHelp);
        assert!(a.tools.pick.is_none(), "光标处没有单词就只提示");
    }

    #[test]
    fn key_search_shows_a_tip_only_when_on_and_only_for_a_new_word() {
        let (mut a, _g) = app_with_docs(&["x"]);
        a.cur().lines = vec!["窗口一览 xyzzyplugh".into()];
        a.cur().cursor = [0, 1];
        a.cur().tip.clear();
        a.key_search_tick();
        assert_eq!(a.cur().tip, "", "没开不提示");
        a.run_pick(KApp::KeySearch);
        assert!(a.cfg.view.key_search);
        a.cur().tip.clear();
        a.key_search_tick();
        assert!(a.cur().tip.starts_with("帮助: "), "实际 {:?}", a.cur().tip);
        a.cur().tip.clear();
        a.key_search_tick();
        assert_eq!(a.cur().tip, "", "光标没动不重复");
        a.cur().cursor = [0, 6];
        a.key_search_tick();
        assert_eq!(a.cur().tip, "", "手册里没有的词不提示");
        a.run_pick(KApp::KeySearch);
        assert!(!a.cfg.view.key_search);
    }

    #[test]
    fn pane_commands_open_the_side_bar_on_that_page() {
        let (mut a, _g) = app_with_docs(&["x"]);
        a.side.open = false;
        a.run_pick(KApp::PaneOutline);
        assert!(a.side.open && a.side.tab == Tab::Outline);
        a.run_pick(KApp::PaneFiles);
        assert!(a.side.tab == Tab::Files);
    }

    /// 整个界面跑几帧: 窗口一览画得出来; 过滤框拿着焦点, 方向键 / 回车不会漏给编辑区
    #[test]
    fn the_pick_window_works_inside_a_full_frame_and_enter_switches_the_tab() {
        let (mut a, _g) = app_with_docs(&["one", "two"]);
        let ctx = egui::Context::default();
        let key = |k: Key| egui::Event::Key {
            key: k,
            physical_key: Some(k),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let frame = |a: &mut App, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| a.ui_main(ui));
            out.textures_delta.clear();
        };
        let (active, lines) = (a.active, a.docs[a.active].lines.clone());
        assert!(active >= 1);
        a.run_pick(KApp::WindowList);
        frame(&mut a, vec![]);
        frame(&mut a, vec![]);
        assert!(a.tools.pick.is_some(), "没按键窗口一直开着");
        frame(&mut a, vec![key(Key::ArrowUp)]);
        frame(&mut a, vec![key(Key::Enter)]);
        assert!(a.tools.pick.is_none(), "回车后窗口关了");
        assert_eq!(a.active, active - 1, "选中的是当前标签的上一个");
        assert_eq!(a.docs[active].lines, lines, "回车没有漏进编辑区");
        a.run_pick(KApp::WindowList);
        frame(&mut a, vec![]);
        frame(&mut a, vec![key(Key::Escape)]);
        assert!(a.tools.pick.is_none(), "Esc 关闭");
    }

    /// 分割视图里: 窗口一览选中的标签要留在焦点窗格里 (不能被每帧的「活动标签 = 焦点窗格的文档」盖回去)
    #[test]
    fn window_list_selection_survives_split_view() {
        let (mut a, _g) = app_with_docs(&["one", "two", "three"]);
        let ctx = egui::Context::default();
        let frame = |a: &mut App, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| a.ui_main(ui));
            out.textures_delta.clear();
        };
        a.select_tab(1);
        a.toggle_split();
        assert!(a.split_on);
        frame(&mut a, vec![]);
        let want = a.docs.iter().position(|d| d.name == "three").unwrap();
        let rows = window_rows(
            a.docs.iter().map(|d| (d.id, d.name.as_str(), None, false)),
            a.active,
        );
        a.tools.pick = Some(Pick::new(Kind::Windows, "t", Match::Fuzzy, rows).start_at(want));
        frame(&mut a, vec![]);
        frame(
            &mut a,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(a.tools.pick.is_none());
        frame(&mut a, vec![]);
        assert_eq!(a.active, want, "下一帧仍然是选中的标签");
        assert_eq!(a.panes.focused_doc(), want, "焦点窗格换成了它");
    }
}
