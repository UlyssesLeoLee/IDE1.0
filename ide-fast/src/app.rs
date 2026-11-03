//! 应用外壳: 菜单 / 标签 / 编辑区 / 查找条 / 状态栏。编辑语义都在 `doc.rs` (-> editcore), 这里只管画和收输入。

use crate::doc::{expand_tabs, Doc};
use crate::cmds::{self, Action};
use crate::cmdui::{CmdUi, Item, PluginCmd};
use crate::keymap::{self, App as KApp, Run};
use crate::find::Find;
use crate::grep::{self, Ev as GEv};
use crate::i18n::{lk, tr, trf, trz, Lang};
use crate::plugins;
use crate::prefs::{prefs_page, Prefs, PrefsTab};
use crate::settings::Settings;
use crate::sidebar::{FileOp, Sidebar, Tab};
use crate::tabs::{self, CloseKind, JumpHist};
use crate::terminal::Terminal;
use editcore::document_io::{Charset, Eol};
use crate::Trace;
use eframe::egui::{self, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Vec2};
use editcore::syntax_styling::{Color, Theme};
use editcore::type_config::{TokenKind, TypeConfig, TypeRegistry};
use std::path::PathBuf;

pub struct App {
    docs: Vec<Doc>,
    active: usize,
    next_id: u64,
    untitled_n: usize,
    find: Find,
    dark: bool,
    font_px: f32,
    tab: usize,
    highlight: bool,
    recent: Vec<String>,
    saved: Settings,
    registry: TypeRegistry,
    side: Sidebar,
    term: Terminal,
    goto_open: bool,
    was_focused: bool,
    goto_text: String,
    /// 主题插件: 选中的 id 与换算好的外观
    theme_id: String,
    theme_visuals: Option<egui::Visuals>,
    /// 主题变量 (CSS 变量名 -> 颜色串) 与换算好的语义色; 没选主题插件时用内置深 / 浅色
    theme_vars: Option<std::collections::BTreeMap<String, String>>,
    tc: crate::view::ThemeColors,
    /// 分割窗口: 左右两个编辑区各看一个文档; active 永远是有焦点那一栏的文档
    split_on: bool,
    left_doc: usize,
    right_doc: usize,
    focus_right: bool,
    /// 当前这栏是否接收键盘 (分割时只有焦点栏收)
    pane_keys: bool,
    /// 终端: 选区作标准输入 / 输出替换选区
    term_stdin: bool,
    term_replace: bool,
    md_preview: bool,
    wrap: bool,
    goal_x: Option<f32>,
    inbox: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    /// 设置改了是否落盘
    persist: bool,
    /// 首选项对话框: Some = 打开着, 里面是草稿 (OK 才生效, Cancel 丢弃)
    prefs: Option<Box<Prefs>>,
    /// 首选项里没有对应 App 字段的那些键 (自动缩进 / 编码 / 备份 / 颜色 ...) 和界面状态
    cfg: Settings,
    /// 已经装进 egui 的编辑器字体文件名 (变了才重装)
    applied_font: String,
    /// 最近一次「正常状态」(非最大化 / 最小化 / 全屏) 的窗口位置和大小; 关窗口时落盘
    geom_now: Option<crate::settings::WinGeom>,
    maximized_now: bool,
    /// 自动换行时每个逻辑行的累计高度 (按 文档 / 版本 / 宽度 / 字号 缓存; 行数多时关闭换行)
    wrap_cache: Option<((u64, u64, i32, i32, usize), Vec<f32>)>,
    max_cols: usize,
    trace: Trace,
    first_frame: bool,
    confirm_exit: bool,
    /// 输入法正在组字的文字 (还没提交): 画在光标处, 不进文档
    preedit: String,
    /// 下一次滚到光标时把它放在窗口中央 (从 Grep 结果跳转时)
    center_once: bool,
    /// 正在被 Grep 流式填充的结果文档
    grep_doc: Option<u64>,
    /// 结果文档里请求跳转的 (文档 id, 行)
    jump_req: Option<(u64, usize)>,
    /// 脚本插件: 宿主在第一次跑命令时才创建 (启动路径上不碰 JS 引擎)
    plug: crate::plugin_ui::PlugState,
    /// 编辑区控件的 id (有别的输入框拿着焦点时, 编辑区不抢键盘; 编辑区自己拿着焦点不算)
    editor_ids: Vec<egui::Id>,
    /// 移动履历 (Alt+P / Alt+N)
    jump: JumpHist,
    /// 「文件属性」对话框的内容 (Some = 打开着)
    props: Option<String>,
    /// 书签列表窗口
    bm_open: bool,
    fullscreen: bool,
    topmost: bool,
    recent_dirs: Vec<String>,
    /// 每个文件上次的光标 (路径, 行, 列)
    positions: Vec<(String, usize, usize)>,
    /// 括弧强调的缓存: ((文档 id, 版本, 光标), 结果); 光标没动就不重算
    bracket_cache: Option<((u64, u64, [usize; 2]), Option<([usize; 2], [usize; 2])>)>,
    /// 上一帧的活动标签下标: 变了就把新的活动标签滚进标签栏可见范围
    last_active: usize,
    /// Markdown 预览: 图片纹理缓存与按 (文档, 版本) 缓存的解析结果
    md_cache: crate::md_view::MdCache,
    md_parsed: Option<((u64, u64), Vec<crate::markdown::Parsed>)>,
    /// 预览要滚到的源码行 (大纲点击)
    md_scroll_to: Option<usize>,
    /// 当前的两份文档比较; 差异列表窗口 / 选择对比标签窗口
    diff: Option<crate::diff::DiffView>,
    diff_list_open: bool,
    diff_pick: bool,
    /// 命令面板 / 键位设置 / 键位一览
    cmdui: CmdUi,
    /// 打开命令面板那一刻的脚本插件命令 (打开期间不再反复读盘)
    pal_plugins: Vec<PluginCmd>,
}

impl App {
    pub fn new(args: Vec<String>, inbox: Option<std::sync::mpsc::Receiver<Vec<String>>>, trace: Trace) -> Self {
        Self::build(args, inbox, trace, Settings::load(), true)
    }

    /// `persist` = 设置改了要不要落盘 (测试里关掉, 免得碰用户真实的设置文件)
    fn build(args: Vec<String>, inbox: Option<std::sync::mpsc::Receiver<Vec<String>>>, trace: Trace, st: Settings, persist: bool) -> Self {
        let mut app = App {
            docs: vec![], active: 0, next_id: 1, untitled_n: 0,
            find: Find::new(),
            dark: true, font_px: 15.0, tab: 4, highlight: true, recent: vec![], saved: Settings::default(), registry: TypeRegistry::default_registry(), side: Sidebar::new(), term: Terminal::new(), goto_open: false, was_focused: true, goto_text: String::new(), theme_id: String::new(), theme_visuals: None, theme_vars: None, tc: crate::view::ThemeColors::builtin(true), split_on: false, left_doc: 0, right_doc: 0, focus_right: false, pane_keys: true, term_stdin: false, term_replace: false, md_preview: false, wrap: false, goal_x: None, inbox, prefs: None, persist: true, cfg: Settings::default(), applied_font: String::new(), geom_now: None, maximized_now: false, wrap_cache: None, max_cols: 80, trace, first_frame: true, confirm_exit: false, preedit: String::new(), center_once: false, grep_doc: None, jump_req: None, plug: Default::default(), editor_ids: vec![], jump: JumpHist::default(), props: None, bm_open: false, fullscreen: false, topmost: false, recent_dirs: vec![], positions: vec![], bracket_cache: None, last_active: usize::MAX, md_cache: Default::default(), md_parsed: None, md_scroll_to: None, diff: None, diff_list_open: false, diff_pick: false, cmdui: CmdUi::default(), pal_plugins: vec![],
        };
        app.persist = persist;
        app.apply_settings(&st);
        app.recent = st.recent.clone();
        app.recent_dirs = st.recent_dirs.clone();
        app.positions = st.positions.clone();
        app.side.open = st.view.side_open;
        app.term.open = st.view.term_open;
        app.md_preview = st.view.md_preview;
        app.applied_font = st.font_family.clone();
        app.saved = app.current_settings();
        for a in &args {
            app.open_spec(a);
        }
        if app.docs.is_empty() {
            app.new_doc();
        }
        app
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn new_doc(&mut self) {
        self.untitled_n += 1;
        let (id, n) = (self.id(), self.untitled_n);
        let mut d = Doc::untitled(id, n);
        if let Some(cs) = Charset::from_label(&self.cfg.default_encoding) {
            d.charset = cs;
            d.bom = match cs { Charset::Utf16Le => editcore::document_io::Bom::Utf16Le, Charset::Utf16Be => editcore::document_io::Bom::Utf16Be, _ => editcore::document_io::Bom::None };
        }
        if let Some(e) = Eol::from_label(&self.cfg.default_eol) { d.eol = e; }
        self.docs.push(d);
        self.active = self.docs.len() - 1;
    }

    fn open_path(&mut self, p: &std::path::Path) {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(p)) {
            self.active = i;
            return;
        }
        if std::fs::metadata(p).map(|m| m.len() > self.cfg.max_file_mb as u64 * 1024 * 1024 && m.len() <= crate::doc::HARD_LIMIT).unwrap_or(false) {
            let mb = std::fs::metadata(p).map(|m| m.len() / 1024 / 1024).unwrap_or(0);
            let go = rfd::MessageDialog::new().set_title(tr("dlg.big-title")).set_description(trf("dlg.big-body", &[&p.display().to_string(), &mb.to_string()])).set_buttons(rfd::MessageButtons::YesNo).show();
            if go != rfd::MessageDialogResult::Yes { return; }
        }
        let id = self.id();
        match Doc::open(id, p) {
            Ok(d) => {
                // 还没动过的空白无标题文档被打开的文件顶替
                if self.docs.len() == 1 && self.docs[0].path.is_none() && !self.docs[0].dirty && self.docs[0].lines == [String::new()] {
                    self.docs.clear();
                }
                self.docs.push(d);
                self.active = self.docs.len() - 1;
                if let Some((r, c)) = self.positions.iter().find(|x| std::path::Path::new(&x.0) == p).map(|x| (x.1, x.2)) {
                    self.docs[self.active].goto(r, c); // 回到上次关掉时的光标位置
                }
                let mut st = self.current_settings();
                st.push_recent(p);
                self.recent = st.recent.clone();
                self.saved = st.clone();
                if self.persist { st.save(); }
            }
            Err(e) => {
                rfd::MessageDialog::new().set_title(tr("dlg.open-failed")).set_description(format!("{}\n{e}", p.display())).show();
            }
        }
    }

    fn current_settings(&self) -> Settings {
        let (f, g) = (&self.find, &self.side.grep.o);
        let search = crate::settings::SearchPrefs { find_case: f.opts.case, find_regex: f.opts.regex, find_word: f.opts.word, find_wrap: f.opts.wrap, find_marks: f.marks, grep_case: g.case, grep_regex: g.regex, grep_word: g.word, grep_sub: g.sub, grep_outside: g.outside, grep_files: g.files.clone(), grep_exc_files: g.exc_files.clone(), grep_exc_folders: g.exc_folders.clone() };
        let mut view = self.cfg.view.clone();
        (view.side_open, view.term_open, view.md_preview) = (self.side.open, self.term.open, self.md_preview);
        Settings { dark: self.dark, font_px: self.font_px, tab_width: self.tab, highlight: self.highlight, wrap: self.wrap, theme_plugin: self.theme_id.clone(), recent: self.recent.clone(), recent_dirs: self.recent_dirs.clone(), positions: self.positions.clone(), search, view, ..self.cfg.clone() }
    }

    /// 把一份设置套到运行状态上 (启动、首选项 OK、恢复默认、导入配置都走这里)
    fn apply_settings(&mut self, st: &Settings) {
        self.dark = st.dark;
        if st.theme_plugin != self.theme_id { self.set_theme_plugin(&st.theme_plugin.clone()); }
        self.font_px = st.font_px;
        self.tab = st.tab_width;
        self.highlight = st.highlight;
        self.wrap = st.wrap;
        crate::i18n::set_code(&st.lang);
        self.cfg = st.clone();
        keymap::set_overrides(&st.keys);
        self.apply_search_prefs(st);
        // 最近文件个数变小了要立刻截断
        let n = st.mru_files.max(1);
        self.recent.truncate(n);
    }

    /// 记下窗口位置 / 大小 (只记正常状态; 最大化时保留最近一次正常状态的尺寸, 下次启动先开成这个尺寸再最大化)
    fn track_geometry(&mut self, ctx: &egui::Context) {
        let (outer, inner, max, min, full) = ctx.input(|i| { let v = i.viewport(); (v.outer_rect, v.inner_rect, v.maximized.unwrap_or(false), v.minimized.unwrap_or(false), v.fullscreen.unwrap_or(false)) });
        self.maximized_now = max;
        if max || min || full { return; }
        if let (Some(o), Some(i)) = (outer, inner) {
            let g = crate::settings::WinGeom { x: o.min.x, y: o.min.y, w: i.width(), h: i.height(), maximized: false };
            if g.sane() { self.geom_now = Some(g); }
        }
    }

    /// 每帧把编辑偏好写进各文档 (自动缩进 / 智能缩进 / Tab 插入空格, 后者按扩展名可覆盖); 便宜, 且新建 / 打开的文档不用另外挂钩
    fn sync_doc_prefs(&mut self) {
        let (ai, si, ins) = (self.cfg.auto_indent, self.cfg.smart_indent, self.cfg.insert_spaces);
        for i in 0..self.docs.len() {
            let (_, _, sp) = self.cfg.effective_for(&self.docs[i].name, self.tab, self.wrap, ins);
            let d = &mut self.docs[i];
            (d.auto_indent, d.smart_indent, d.insert_spaces) = (ai, si, sp);
        }
    }

    /// 某个文件名适用的 (Tab 宽度, 自动换行, Tab 插入空格)
    fn eff(&self, name: &str) -> (usize, bool, bool) {
        self.cfg.effective_for(name, self.tab, self.wrap, self.cfg.insert_spaces)
    }

    /// 启动时把上次的查找 / Grep 条件放回去
    fn apply_search_prefs(&mut self, st: &Settings) {
        let p = &st.search;
        self.find.opts = crate::find::FindOpts { case: p.find_case, regex: p.find_regex, word: p.find_word, wrap: p.find_wrap };
        self.find.marks = p.find_marks;
        let g = &mut self.side.grep.o;
        (g.case, g.regex, g.word, g.sub, g.outside) = (p.grep_case, p.grep_regex, p.grep_word, p.grep_sub, p.grep_outside);
        (g.files, g.exc_files, g.exc_folders) = (p.grep_files.clone(), p.grep_exc_files.clone(), p.grep_exc_folders.clone());
    }

    /// 设置变了就落盘 (每帧比较一次, 只有真变了才写)
    fn persist_if_changed(&mut self) {
        let now = self.current_settings();
        if now != self.saved {
            if self.persist { now.save(); }
            self.saved = now;
        }
    }

    /// 命令行 / 别的实例转来的 `路径[:行[:列]]`
    fn open_spec(&mut self, spec: &str) {
        let (path, line, col) = editcore::text_util::split_path_line_col(spec);
        let p = PathBuf::from(&path);
        if !p.is_file() {
            self.cur_or_new().tip = trf("dlg.not-found", &[&path]);
            return;
        }
        self.open_path(&p);
        if line > 0 {
            self.cur().goto(line - 1, col.saturating_sub(1));
        }
    }

    fn cur_or_new(&mut self) -> &mut Doc {
        if self.docs.is_empty() {
            self.new_doc();
        }
        self.cur()
    }

    /// 打开首选项: 草稿 = 当前设置的拷贝
    fn open_prefs(&mut self) {
        if self.prefs.is_none() {
            self.prefs = Some(Box::new(Prefs { draft: self.current_settings(), tab: PrefsTab::General, new_ext: String::new(), fonts: crate::fonts::system_fonts() }));
        }
    }

    /// 把草稿并进当前设置并落盘 (OK / 应用)
    fn prefs_commit(&mut self) {
        let Some(p) = self.prefs.as_ref() else { return };
        let mut next = self.current_settings();
        next.adopt_prefs(&p.draft);
        self.apply_settings(&next);
        self.persist_if_changed();
    }

    /// 首选项窗口: 分页 + 草稿, OK 才生效 / 落盘, Cancel 丢弃
    fn prefs_window(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.prefs.take() else { return };
        let (mut ok, mut cancel, mut apply, mut reset) = (false, false, false, false);
        let mut open = true;
        egui::Window::new(tr("pf.title")).open(&mut open).collapsible(false).resizable(false).default_width(460.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (t, key) in [(PrefsTab::General, "pf.tab.general"), (PrefsTab::Edit, "pf.tab.edit"), (PrefsTab::Files, "pf.tab.files"), (PrefsTab::Font, "pf.tab.font"), (PrefsTab::Types, "pf.tab.types"), (PrefsTab::Search, "pf.tab.search")] {
                    ui.selectable_value(&mut p.tab, t, tr(key));
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(340.0).auto_shrink([false, true]).show(ui, |ui| prefs_page(ui, &mut p));
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr("pf.ok")).clicked() { ok = true; }
                if ui.button(tr("pf.cancel")).clicked() { cancel = true; }
                if ui.button(tr("pf.apply")).clicked() { apply = true; }
                if ui.button(lk("c.reset-settings", "")).clicked() { reset = true; }
            });
            ui.small(trf("pf.file", &[&crate::settings::path().map(|p| p.display().to_string()).unwrap_or_else(|| tr("pf.file-off").into())]));
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) { cancel = true; }
        self.prefs = Some(p);
        if ok || apply { self.prefs_commit(); }
        if reset { self.reset_settings(); }
        if ok || cancel || !open { self.prefs = None; }
    }

    /// 偏好恢复默认 (问一下; 最近文件 / 光标位置 / 界面状态保留)
    fn reset_settings(&mut self) {
        if rfd::MessageDialog::new().set_title(tr("dlg.reset-title")).set_description(tr("dlg.reset-body")).set_buttons(rfd::MessageButtons::YesNo).show() != rfd::MessageDialogResult::Yes { return; }
        let mut next = self.current_settings();
        next.reset_prefs();
        self.apply_settings(&next);
        let fresh = self.current_settings();
        if let Some(p) = self.prefs.as_mut() { p.draft = fresh; }
        self.persist_if_changed();
    }

    fn export_settings(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("ide-fast-settings.json").save_file() else { return };
        let tip = match self.current_settings().export_to(&path) { Ok(()) => trf("dlg.settings-exported", &[&path.display().to_string()]), Err(e) => e };
        self.cur_or_new().tip = tip;
    }

    fn import_settings(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() else { return };
        let mut next = self.current_settings();
        match next.import_from(&path) {
            Ok(()) => { self.apply_settings(&next); self.persist_if_changed(); self.cur_or_new().tip = tr("dlg.settings-imported").into(); }
            Err(_) => { rfd::MessageDialog::new().set_title(tr("dlg.cant-import")).set_description(tr("dlg.settings-bad")).show(); }
        }
    }

    fn import_sakura(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("sakura.ini", &["ini"]).pick_file() else { return };
        // Sakura 的 ini 常是 Shift-JIS / UTF-16: 走 editcore 的自动判别读
        let Ok(d) = editcore::document_io::read_doc(&path, None) else { rfd::MessageDialog::new().set_title(tr("dlg.cant-import")).set_description(tr("dlg.settings-bad")).show(); return };
        let mut next = self.current_settings();
        let n = next.import_sakura_ini(&d.content);
        self.apply_settings(&next);
        self.persist_if_changed();
        self.cur_or_new().tip = format!("sakura.ini: {n}");
    }

    fn open_folder_dialog(&mut self) {
        if let Some(d) = rfd::FileDialog::new().pick_folder() {
            self.open_folder(d);
        }
    }

    fn open_folder(&mut self, d: PathBuf) {
        let mut st = self.current_settings();
        st.push_recent_dir(&d);
        self.recent_dirs = st.recent_dirs;
        self.side.set_root(d);
        self.side.scm.loaded = false;
        self.side.open = true;
    }

    /// 打开文件并跳到某行 (Grep 结果 / 命令行 `文件:行`)
    fn open_at(&mut self, p: &std::path::Path, line: usize, col: usize) {
        if p.is_file() { self.open_path(p); }
        // 路径不是磁盘文件 (只搜当前文件、还没存盘的标签): 就在当前标签里跳
        if p.is_file() && !self.docs.iter().any(|d| d.path.as_deref() == Some(p)) { return; }
        self.cur().goto(line.saturating_sub(1), col);
    }

    /// 侧栏内容: 文件树 / 大纲 / Grep / Git
    fn side_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (t, name) in [(Tab::Files, "文件"), (Tab::Outline, "大纲"), (Tab::Search, "搜索"), (Tab::Scm, "Git")] {
                if ui.selectable_label(self.side.tab == t, name).clicked() { self.side.tab = t; }
            }
        });
        ui.separator();
        let mut open: Option<(PathBuf, usize)> = None;
        match self.side.tab {
            Tab::Files => {
                match self.side.root.clone() {
                    None => { if ui.button("打开文件夹…").clicked() { self.open_folder_dialog(); } }
                    Some(root) => {
                        let mut op: Option<FileOp> = None;
                        ui.horizontal(|ui| {
                            let title = ui.strong(root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string()));
                            title.context_menu(|ui| {
                                if ui.button("新建文件…").clicked() { op = Some(FileOp::NewFile(root.clone())); ui.close(); }
                                if ui.button("新建文件夹…").clicked() { op = Some(FileOp::NewDir(root.clone())); ui.close(); }
                            });
                            if ui.small_button("刷新").clicked() { self.side.refresh(); }
                        });
                        self.pending_ui(ui);
                        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| tree(ui, &mut self.side, &root, &mut open, &mut op));
                        if let Some(o) = op { self.begin_file_op(o); }
                    }
                }
            }
            Tab::Outline => {
                let ai = self.active.min(self.docs.len() - 1);
                let d = &self.docs[ai];
                self.side.update_outline(d.id, d.rev, &d.name, &d.lines);
                let mut jump: Option<usize> = None;
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    if self.side.outline.is_empty() { ui.label("(没有可显示的大纲)"); }
                    for e in &self.side.outline {
                        ui.horizontal(|ui| {
                            ui.add_space(e.depth.max(0) as f32 * 12.0);
                            if ui.selectable_label(false, format!("{}  {}", icon_of(&e.kind), e.name)).clicked() { jump = Some(e.line.max(0) as usize); }
                        });
                    }
                });
                if let Some(l) = jump {
                    self.cur().goto(l, 0);
                    if self.md_preview { self.md_scroll_to = Some(l); }
                }
            }
            Tab::Search => {
                let root = self.grep_root().unwrap_or_default();
                let has_cur = self.cur().path.is_some();
                let shown_root = self.side.grep.last.as_ref().map(|l| l.1.clone()).unwrap_or(root);
                let evs = self.side.grep.panel(ui, &shown_root, has_cur);
                for ev in evs { self.grep_event(ev); }
            }
            Tab::Scm => self.scm_ui(ui, &mut open),
        }
        if let Some((p, l)) = open { self.open_at(&p, l, 0); }
    }

    /// 文件树操作的输入 / 确认条
    fn pending_ui(&mut self, ui: &mut egui::Ui) {
        let red = self.tc.red;
        let Some(p) = self.side.pending.as_mut() else { return };
        let name_of = |f: &std::path::Path| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (title, needs_text) = match &p.op {
            FileOp::NewFile(_) => ("新建文件".to_string(), true),
            FileOp::NewDir(_) => ("新建文件夹".to_string(), true),
            FileOp::Rename(f) => (format!("重命名 {}", name_of(f)), true),
            FileOp::Delete(f) => (format!("确定删除 {} ?", name_of(f)), false),
        };
        let mut go = false;
        let mut cancel = false;
        ui.group(|ui| {
            ui.label(title);
            if needs_text {
                let r = ui.add(egui::TextEdit::singleline(&mut p.text).desired_width(f32::INFINITY));
                if p.fresh { r.request_focus(); p.fresh = false; }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) { go = true; }
                if ui.input(|i| i.key_pressed(Key::Escape)) { cancel = true; }
            }
            if let Some(e) = &p.error { ui.colored_label(red, e); }
            ui.horizontal(|ui| {
                if ui.button("确定").clicked() { go = true; }
                if ui.button("取消").clicked() { cancel = true; }
            });
        });
        if cancel { self.side.pending = None; } else if go { self.apply_file_op(); }
    }

    fn begin_file_op(&mut self, op: FileOp) {
        let text = match &op { FileOp::Rename(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), _ => String::new() };
        self.side.pending = Some(crate::sidebar::Pending { op, text, error: None, fresh: true });
    }

    /// 执行文件树上的操作; 失败把原因留在条上, 成功后刷新树并同步已打开的标签
    fn apply_file_op(&mut self) {
        let Some(p) = self.side.pending.as_mut() else { return };
        let mut open_after: Option<PathBuf> = None;
        let mut moved: Option<(PathBuf, PathBuf)> = None;
        let mut deleted: Option<PathBuf> = None;
        let r: Result<(), String> = match &p.op {
            FileOp::NewFile(d) => crate::sidebar::create_file(d, &p.text).map(|f| open_after = Some(f)),
            FileOp::NewDir(d) => crate::sidebar::create_dir(d, &p.text).map(|_| ()),
            FileOp::Rename(f) => crate::sidebar::rename_entry(f, &p.text).map(|to| moved = Some((f.clone(), to))),
            FileOp::Delete(f) => crate::sidebar::delete_entry(f).map(|_| deleted = Some(f.clone())),
        };
        if let Err(e) = r { p.error = Some(e); return; }
        self.side.pending = None;
        if let Some((from, to)) = moved { self.docs_follow_rename(&from, &to); }
        if let Some(f) = deleted {
            for d in self.docs.iter_mut().filter(|d| d.path.as_deref().is_some_and(|p| p.starts_with(&f))) { d.dirty = true; d.tip = "磁盘上的文件已被删除".into(); }
        }
        self.side.refresh();
        if let Some(f) = open_after { self.open_path(&f); }
    }

    /// 文件 / 文件夹改名后, 打开着的标签跟着换路径和标题
    fn docs_follow_rename(&mut self, from: &std::path::Path, to: &std::path::Path) {
        for d in self.docs.iter_mut() {
            let Some(old) = d.path.clone() else { continue };
            if let Ok(rest) = old.strip_prefix(from) {
                let np = if rest.as_os_str().is_empty() { to.to_path_buf() } else { to.join(rest) };
                d.name = np.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                d.path = Some(np);
            }
        }
    }

    /// Git 页: 分支、改动列表, 点文件打开, 「差异」在新标签里看统一差异, 暂存 / 取消暂存
    fn scm_ui(&mut self, ui: &mut egui::Ui, open: &mut Option<(PathBuf, usize)>) {
        let root = self.grep_root().unwrap_or_default();
        if self.side.scm.poll() { ui.ctx().request_repaint(); }
        if !self.side.scm.loaded && !self.side.scm.loading() { self.side.scm.refresh(root.clone()); }
        if self.side.scm.loading() { ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
        ui.horizontal(|ui| {
            let st = &self.side.scm.status;
            ui.strong(if st.branch.is_empty() { "Git".to_string() } else { format!("分支 {}", st.branch) });
            if ui.small_button("刷新").clicked() { self.side.scm.refresh(root.clone()); }
            if self.side.scm.loading() { ui.label("…"); }
        });
        if let Some(e) = self.side.scm.status.error.clone() { ui.colored_label(self.tc.red, e); }
        let Some(top) = self.side.scm.status.top.clone() else { return };
        let changes = self.side.scm.status.changes.clone();
        if changes.is_empty() && self.side.scm.status.error.is_none() { ui.label("没有改动"); }
        let tc = self.tc;
        let mut act: Option<(String, u8)> = None; // 0 = 打开, 1 = 差异, 2 = 暂存, 3 = 取消暂存
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            for (title, staged) in [("已暂存的更改", true), ("更改", false)] {
                let rows: Vec<&crate::scm::Change> = changes.iter().filter(|c| c.staged() == staged).collect();
                if rows.is_empty() { continue; }
                ui.label(egui::RichText::new(format!("{title} ({})", rows.len())).strong());
                for c in rows {
                    ui.horizontal(|ui| {
                        let col = match c.letter() { 'M' => tc.yellow, 'A' | 'U' => tc.green, 'D' => tc.red, _ => tc.blue };
                        ui.label(egui::RichText::new(c.letter().to_string()).monospace().color(col));
                        if ui.small_button("差异").clicked() { act = Some((c.path.clone(), 1)); }
                        if staged { if ui.small_button("-").on_hover_text("取消暂存").clicked() { act = Some((c.path.clone(), 3)); } }
                        else if ui.small_button("+").on_hover_text("暂存").clicked() { act = Some((c.path.clone(), 2)); }
                        if ui.selectable_label(false, &c.path).clicked() { act = Some((c.path.clone(), 0)); }
                    });
                }
            }
        });
        let Some((path, kind)) = act else { return };
        match kind {
            0 => *open = Some((top.join(&path), 1)),
            1 => {
                let staged = changes.iter().any(|c| c.path == path && c.staged() && c.y == ' ');
                match crate::scm::file_diff(&top, &path, staged) {
                    Ok(t) if t.trim().is_empty() => self.cur().tip = format!("{path}: 没有可显示的差异 (未跟踪文件?)"),
                    Ok(t) => self.open_text_doc(&format!("Diff: {path}.diff"), &t),
                    Err(e) => self.cur().tip = e,
                }
            }
            k => {
                let r = if k == 2 { crate::scm::stage(&top, &path) } else { crate::scm::unstage(&top, &path) };
                if let Err(e) = r { self.cur().tip = e; }
                self.side.scm.refresh(root);
            }
        }
    }

    /// 把一段文字开成新的只读标签 (Git 差异 / 文档比较结果)
    fn open_text_doc(&mut self, name: &str, text: &str) {
        let id = self.id();
        let mut d = Doc::untitled(id, 0);
        d.name = name.to_string();
        d.lines = text.trim_end_matches('\n').split('\n').map(str::to_string).collect();
        d.base = d.lines.clone();
        d.readonly = true;
        self.docs.push(d);
        self.active = self.docs.len() - 1;
    }

    /// 比较当前标签 (左) 和第 `other` 个标签 (右)
    fn start_diff(&mut self, other: usize) {
        let cur = self.active.min(self.docs.len() - 1);
        if other == cur || other >= self.docs.len() { return; }
        let (l, r) = (&self.docs[cur], &self.docs[other]);
        let v = crate::diff::DiffView::new(l.id, r.id, &l.lines, &r.lines, (l.rev, r.rev));
        let tip = format!("比较 {} ↔ {}: {}", l.name, r.name, v.summary());
        self.diff = Some(v);
        self.cur().tip = tip;
    }

    /// 选一个文件打开成标签, 再和当前标签比较
    fn diff_with_file(&mut self) {
        let Some(p) = rfd::FileDialog::new().pick_file() else { return };
        let cur_id = self.cur().id;
        self.open_path(&p);
        let other = self.active.min(self.docs.len() - 1);
        if let Some(ci) = self.docs.iter().position(|d| d.id == cur_id) {
            self.active = ci;
            self.start_diff(other);
        }
    }

    /// 下一处 / 上一处差异 (在当前标签所在的那一侧找, 到头回绕)
    fn diff_step(&mut self, forward: bool) {
        let Some(v) = &self.diff else { self.cur().tip = "没有进行中的比较".into(); return };
        let (id, row) = { let d = &self.docs[self.active.min(self.docs.len() - 1)]; (d.id, d.cursor[0]) };
        if id != v.left && id != v.right { self.cur().tip = "当前标签不在比较里".into(); return; }
        match crate::diff::step(&v.hunks, row, forward, id == v.right) {
            Some(r) => self.cur().goto(r, 0),
            None => self.cur().tip = "两份文档完全相同".into(),
        }
    }

    /// 比较的文档变了就重算; 任何一份被关掉就结束比较
    fn diff_refresh(&mut self) {
        let Some(v) = self.diff.as_mut() else { return };
        let (a, b) = (self.docs.iter().find(|d| d.id == v.left), self.docs.iter().find(|d| d.id == v.right));
        match (a, b) {
            (Some(a), Some(b)) => v.refresh(&a.lines, &b.lines, (a.rev, b.rev)),
            _ => { self.diff = None; self.diff_list_open = false; }
        }
    }

    /// 选择对比标签 / 差异列表 两个小窗口
    fn diff_windows(&mut self, ctx: &egui::Context) {
        if self.diff_pick {
            let mut open = true;
            let mut pick: Option<usize> = None;
            let cur = self.active.min(self.docs.len() - 1);
            egui::Window::new("与哪个标签比较").open(&mut open).collapsible(false).show(ctx, |ui| {
                for (i, d) in self.docs.iter().enumerate().filter(|(i, _)| *i != cur) {
                    if ui.selectable_label(false, &d.name).clicked() { pick = Some(i); }
                }
                if self.docs.len() < 2 { ui.label("需要至少两个标签"); }
            });
            if let Some(i) = pick { self.start_diff(i); open = false; }
            self.diff_pick = open;
        }
        if self.diff_list_open {
            let mut open = true;
            let mut jump: Option<usize> = None;
            egui::Window::new("差异列表").open(&mut open).default_width(280.0).show(ctx, |ui| {
                let Some(v) = &self.diff else { ui.label("没有进行中的比较"); return };
                ui.label(v.summary());
                let right = self.docs[self.active.min(self.docs.len() - 1)].id == v.right;
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (n, h) in v.hunks.iter().enumerate() {
                        let (kind, (s, len)) = (if h.a_len == 0 { "新增" } else if h.b_len == 0 { "删除" } else { "修改" }, if right { (h.b_start, h.b_len) } else { (h.a_start, h.a_len) });
                        let label = format!("{}. {kind}  第 {} 行起 ({} 行)", n + 1, s + 1, len);
                        if ui.selectable_label(false, label).clicked() { jump = Some(s); }
                    }
                });
            });
            if let Some(r) = jump { self.cur().goto(r, 0); }
            self.diff_list_open = open;
        }
    }

    /// Grep 的搜索根: 打开的文件夹; 没有就用当前文件所在目录, 再没有就是进程工作目录
    fn grep_root(&mut self) -> Option<PathBuf> {
        self.side.root.clone().or_else(|| self.cur().path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))).or_else(|| std::env::current_dir().ok())
    }

    /// 打开 Grep 页并把焦点放到搜索框; 选区 (单行) 作搜索内容
    fn focus_grep(&mut self) {
        if let Some(t) = self.cur().selected_text_single() { self.side.grep.o.pattern = if self.side.grep.o.regex { crate::find::escape_regex(&t) } else { t }; }
        self.side.open = true;
        self.side.tab = Tab::Search;
        self.side.grep.focus_pattern = true;
    }

    fn grep_event(&mut self, ev: GEv) {
        match ev {
            GEv::Start => self.grep_start(),
            GEv::Stop => { self.side.grep.cancel(); self.side.grep.msg = format!("已停止, {} 处命中", self.side.grep.results.len()); }
            GEv::ResultDoc => self.open_grep_doc(),
            GEv::UseCurDir => {
                if let Some(d) = self.cur().path.as_ref().and_then(|p| p.parent()).map(|d| d.to_string_lossy().into_owned()) { self.side.grep.o.folders = d; }
            }
            GEv::ReplaceAll => self.grep_replace(),
            GEv::Open { path, line, text } => {
                let o = self.side.grep.last.as_ref().map(|l| l.0.clone()).unwrap_or_else(|| self.side.grep.o.clone());
                let col = grep::first_cols(&[text], &o)[0].saturating_sub(1);
                self.center_once = true;
                self.open_at(std::path::Path::new(&path), line, col);
            }
        }
    }

    fn grep_start(&mut self) {
        let Some(root) = self.grep_root() else { self.side.grep.msg = "找不到搜索位置".into(); return };
        if self.side.grep.o.this_file {
            let (label, lines) = { let d = self.cur(); (d.path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| d.name.clone()), d.lines.clone()) };
            self.side.grep.start_text(root.clone(), &label, &lines);
        } else {
            self.side.grep.start(root.clone());
        }
        if let Some(id) = self.grep_doc { self.reset_grep_doc(id); }
    }

    fn grep_replace(&mut self) {
        let Some(root) = self.grep_root() else { return };
        let o = self.side.grep.o.clone();
        if o.pattern.is_empty() { self.side.grep.msg = "请输入要搜索的内容".into(); return; }
        let dirty: Vec<String> = self.docs.iter().filter(|d| d.dirty).filter_map(|d| d.path.as_ref().map(|p| p.to_string_lossy().into_owned())).collect();
        let text = format!("把「{}」置换为「{}」\n范围: {}\n会改写磁盘上所有命中的文件 (原文件另存为 .bak)。继续吗?", o.pattern, o.repl, root.display());
        if rfd::MessageDialog::new().set_title("Grep 置换").set_description(text).set_buttons(rfd::MessageButtons::YesNo).show() != rfd::MessageDialogResult::Yes { return; }
        let (msg, written) = grep::replace(&root, &o, dirty);
        // 已打开且没有未保存修改的标签: 重新读盘, 免得显示旧内容、之后又存回去
        let mut reloaded = 0;
        for d in self.docs.iter_mut().filter(|d| !d.dirty) {
            if d.path.as_ref().map(|p| written.iter().any(|w| grep::same_path(w, &p.to_string_lossy()))).unwrap_or(false) && d.reload().is_ok() { reloaded += 1; }
        }
        self.side.grep.msg = if reloaded > 0 { format!("{msg}; 已重新载入 {reloaded} 个打开的标签") } else { msg };
        self.side.grep.results.clear();
    }

    /// Grep 结果开成(或复用)一个只读标签, 此后新结果继续流进去
    fn open_grep_doc(&mut self) {
        let Some(_) = self.side.grep.last.clone() else { self.side.grep.msg = "还没有搜索结果".into(); return };
        let id = match self.grep_doc.filter(|id| self.docs.iter().any(|d| d.id == *id)) {
            Some(id) => id,
            None => {
                let id = self.id();
                let mut d = Doc::untitled(id, 0);
                d.name = "Grep 结果".into();
                d.readonly = true;
                d.grep_result = true;
                self.docs.push(d);
                id
            }
        };
        self.grep_doc = Some(id);
        if let Some(i) = self.docs.iter().position(|d| d.id == id) { self.active = i; }
        self.reset_grep_doc(id);
        self.side.grep.finished = self.side.grep.end.clone(); // 已经跑完的: 补上尾
        self.side.grep.rendered = 0;
        self.sync_grep_doc();
    }

    fn reset_grep_doc(&mut self, id: u64) {
        let Some((o, root)) = self.side.grep.last.clone() else { return };
        if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
            d.lines = grep::header(&o, &root);
            d.rev += 1;
            d.cursor = [0, 0];
            d.sel = None;
        }
        self.side.grep.rendered = 0;
    }

    /// 每帧: 把 Grep 新来的结果追加到结果文档 (标准格式边搜边追加, 其它格式在结束时一次写出)
    fn sync_grep_doc(&mut self) {
        let Some(id) = self.grep_doc else { return };
        let Some(di) = self.docs.iter().position(|d| d.id == id) else { self.grep_doc = None; return };
        let g = &mut self.side.grep;
        let Some((o, _)) = g.last.clone() else { return };
        let mut add: Vec<String> = Vec::new();
        if o.fmt == grep::Fmt::Normal && g.rendered < g.results.len() {
            add.extend(grep::normal_lines(&g.results[g.rendered..], &o));
            g.rendered = g.results.len();
        }
        if let Some(end) = g.finished.take() {
            if o.fmt != grep::Fmt::Normal { add.extend(grep::summary_lines(&g.results, o.fmt)); }
            add.extend(grep::footer(g.results.len(), &end));
        }
        if add.is_empty() { return; }
        let d = &mut self.docs[di];
        d.lines.extend(add);
        d.rev += 1;
        d.dirty = false;
    }

    fn cur(&mut self) -> &mut Doc {
        let i = self.active.min(self.docs.len() - 1);
        &mut self.docs[i]
    }

    fn open_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new().pick_files() {
            for f in files {
                self.open_path(&f);
            }
        }
    }

    fn save(&mut self, as_new: bool) -> bool {
        let path = if as_new || self.cur().path.is_none() {
            match rfd::FileDialog::new().set_file_name(self.cur().name.clone()).save_file() {
                Some(p) => p,
                None => return false,
            }
        } else {
            self.cur().path.clone().unwrap()
        };
        if self.cfg.save_backup && path.is_file() {
            // 保存前把磁盘上的旧版本备份到 .sakura-backup (编辑器自己的保险, 失败只提示不挡保存)
            if let Err(e) = editcore::auto_backup::backup_before_save(&path, &editcore::auto_backup::SaveBackupConfig::default()) {
                self.cur().tip = format!("backup: {e}");
            }
        }
        match self.cur().save_to(&path) {
            Ok(()) => {
                self.cur().tip = trf("dlg.saved", &[&path.display().to_string()]);
                true
            }
            Err(e) => {
                rfd::MessageDialog::new().set_title(tr("dlg.save-failed")).set_description(e).show();
                false
            }
        }
    }

    /// 关标签: 有未保存修改先问; 返回是否真的关了
    fn close_tab(&mut self, i: usize) -> bool {
        if self.docs[i].dirty {
            let r = rfd::MessageDialog::new().set_title(tr("dlg.unsaved-title")).set_description(trf("dlg.unsaved-body", &[&self.docs[i].name]))
                .set_buttons(rfd::MessageButtons::YesNoCancel).show();
            match r {
                rfd::MessageDialogResult::Yes => {
                    let keep = self.active;
                    self.active = i;
                    let ok = self.save(false);
                    self.active = keep;
                    if !ok {
                        return false;
                    }
                }
                rfd::MessageDialogResult::No => {}
                _ => return false,
            }
        }
        self.remember_pos(i);
        let gone = self.docs.remove(i);
        self.jump.forget(gone.id);
        self.split_on = false;
        if self.docs.is_empty() {
            self.new_doc();
        }
        self.active = self.active.min(self.docs.len() - 1);
        true
    }

    /// 记下第 i 个文档的光标位置 (有路径才记), 下次打开时回到这里
    fn remember_pos(&mut self, i: usize) {
        let d = &self.docs[i];
        let Some(p) = d.path.clone() else { return };
        let mut st = Settings::default();
        st.positions = std::mem::take(&mut self.positions);
        st.remember_pos(&p, d.cursor[0], d.cursor[1]);
        self.positions = st.positions;
    }

    // ---------------------------------------------------------------- 标签 / 窗口命令

    fn select_tab(&mut self, i: usize) {
        if i < self.docs.len() { self.active = i; }
    }

    fn next_tab(&mut self, fwd: bool) {
        self.active = tabs::cycle(self.active.min(self.docs.len() - 1), self.docs.len(), fwd);
    }

    fn move_active_tab(&mut self, left: bool) {
        let a = self.active.min(self.docs.len() - 1);
        self.active = tabs::move_tab(&mut self.docs, a, left);
        self.split_on = false;
    }

    /// 批量关标签 (其它 / 左边 / 右边 / 全部): 逐个关, 有未保存的会问, 选了取消就停在那里
    fn close_many(&mut self, kind: CloseKind) {
        let a = self.active.min(self.docs.len() - 1);
        let keep = self.docs[a].id;
        let ids: Vec<u64> = tabs::close_set(kind, a, self.docs.len()).into_iter().map(|i| self.docs[i].id).collect();
        for id in ids {
            let Some(i) = self.docs.iter().position(|d| d.id == id) else { continue };
            if !self.close_tab(i) { break; }
        }
        if let Some(i) = self.docs.iter().position(|d| d.id == keep) { self.active = i; }
    }

    /// 全部保存: 有路径的直接存, 无标题的逐个弹另存为 (取消就跳过那个)
    fn save_all(&mut self) {
        let keep = self.active;
        let mut n = 0;
        for i in 0..self.docs.len() {
            if !self.docs[i].dirty || self.docs[i].grep_result { continue; }
            self.active = i;
            if self.save(false) { n += 1; }
        }
        self.active = keep.min(self.docs.len() - 1);
        self.cur().tip = format!("已保存 {n} 个文件");
    }

    /// 保存并关闭当前标签
    fn save_close(&mut self) {
        if self.cur().dirty && !self.save(false) { return; }
        let a = self.active;
        self.close_tab(a);
    }

    /// 关闭当前标签, 再选一个文件打开
    fn close_open(&mut self) {
        let a = self.active;
        if self.close_tab(a) { self.open_dialog(); }
    }

    /// 以指定编码重新打开当前文件 (有未保存修改先问)
    fn reopen_as(&mut self, cs: Charset) {
        let (dirty, name, has_path) = { let d = self.cur(); (d.dirty, d.name.clone(), d.path.is_some()) };
        if !has_path { self.cur().tip = "还没有保存过, 不能按别的编码重新打开".into(); return; }
        if dirty && rfd::MessageDialog::new().set_title(tr("dlg.reopen-title")).set_description(trf("dlg.reopen-body", &[&name, cs.label()])).set_buttons(rfd::MessageButtons::YesNo).show() != rfd::MessageDialogResult::Yes { return; }
        if let Err(e) = self.cur().reopen_as(cs) { self.cur().tip = e; }
    }

    /// 用系统默认程序打开当前文件 (html 就是浏览器)
    fn browse_current(&mut self) {
        let Some(p) = self.cur().path.clone() else { self.cur().tip = "还没有保存过".into(); return };
        let r = if cfg!(windows) { std::process::Command::new("explorer").arg(&p).spawn() } else { std::process::Command::new("xdg-open").arg(&p).spawn() };
        if let Err(e) = r { self.cur().tip = format!("打不开: {e}"); }
    }

    /// 新窗口: 起一个独立的新进程 (`--new` 不和已有实例合并)
    fn new_window(&mut self) {
        let r = std::env::current_exe().and_then(|exe| std::process::Command::new(exe).arg("--new").spawn());
        if let Err(e) = r { self.cur().tip = format!("开不了新窗口: {e}"); }
    }

    fn clear_recent(&mut self) {
        self.recent.clear();
        self.recent_dirs.clear();
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn toggle_topmost(&mut self, ctx: &egui::Context) {
        self.topmost = !self.topmost;
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.topmost { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal }));
    }

    // ---------------------------------------------------------------- 移动履历 / 括弧

    /// 当前位置记进移动履历 (会「跳走」的命令之前调)
    fn jump_mark(&mut self) {
        let d = self.cur();
        let here = (d.id, d.cursor[0], d.cursor[1]);
        self.jump.set(here);
    }

    fn jump_go(&mut self, d: i32) {
        let here = { let c = self.cur(); (c.id, c.cursor[0], c.cursor[1]) };
        match self.jump.go(d, here) {
            Some((id, r, c)) => {
                if let Some(i) = self.docs.iter().position(|x| x.id == id) { self.active = i; self.docs[i].goto(r, c); }
            }
            None => self.cur().tip = if d < 0 { "没有更早的移动履历".into() } else { "没有更新的移动履历".into() },
        }
    }

    fn bracket_jump(&mut self) {
        self.jump_mark();
        self.cur().jump_to_pair();
    }

    /// 书签列表: 点一行跳过去; 可以全部清除
    fn bookmark_window(&mut self, ctx: &egui::Context) {
        let mut open = self.bm_open;
        let mut go: Option<usize> = None;
        let mut clear = false;
        let ai = self.active.min(self.docs.len() - 1);
        egui::Window::new(tr("dlg.bm-title")).open(&mut open).collapsible(false).default_width(360.0).show(ctx, |ui| {
            let d = &self.docs[ai];
            if d.bookmarks.is_empty() { ui.label(tr("dlg.bm-empty")); }
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for b in &d.bookmarks {
                    let text = d.lines.get(b.row).map(|l| l.trim()).unwrap_or("");
                    let shown: String = text.chars().take(60).collect();
                    if ui.selectable_label(false, format!("{:>5}  {}", b.row + 1, shown)).clicked() { go = Some(b.row); }
                }
            });
            ui.separator();
            if ui.button(tr("c.bookmark-reset")).clicked() { clear = true; }
        });
        self.bm_open = open;
        if clear { self.cur().bookmarks.clear(); self.cur().tip = tr("dlg.bm-cleared").into(); }
        if let Some(r) = go { self.jump_mark(); self.cur().goto(r, 0); }
    }

    fn props_window(&mut self, ctx: &egui::Context) {
        let Some(text) = self.props.clone() else { return };
        let mut open = true;
        egui::Window::new(tr("dlg.props-title")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&text).monospace()).selectable(true));
            if ui.button("复制").clicked() { ctx.copy_text(text.clone()); }
        });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) { self.props = None; }
    }

    fn menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr("menu.file"), |ui| {
                if ui.button(lk("c.new-file", "Ctrl+N")).clicked() { self.new_doc(); ui.close(); }
                if ui.button(lk("c.open-file", "Ctrl+O")).clicked() { ui.close(); self.open_dialog(); }
                if ui.button(lk("c.open-folder", "")).clicked() { ui.close(); self.open_folder_dialog(); }
                ui.menu_button(tr("sub.recent"), |ui| {
                    if self.recent.is_empty() { ui.label(tr("ui.none")); }
                    let mut pick: Option<String> = None;
                    for r in &self.recent {
                        if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
                    }
                    if let Some(r) = pick { self.open_path(std::path::Path::new(&r)); }
                    ui.separator();
                    if ui.button(lk("c.recent-clear", "")).clicked() { self.clear_recent(); ui.close(); }
                });
                ui.menu_button(tr("sub.recent-dirs"), |ui| {
                    if self.recent_dirs.is_empty() { ui.label(tr("ui.none")); }
                    let mut pick: Option<String> = None;
                    for r in &self.recent_dirs {
                        if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
                    }
                    if let Some(r) = pick { self.open_folder(PathBuf::from(r)); }
                });
                ui.separator();
                if ui.button(lk("c.save", "Ctrl+S")).clicked() { ui.close(); self.save(false); }
                if ui.button(lk("c.save-as", "Ctrl+Shift+S")).clicked() { ui.close(); self.save(true); }
                if ui.button(lk("c.save-all", "Ctrl+Alt+Shift+S")).clicked() { ui.close(); self.save_all(); }
                if ui.button(lk("c.save-close", "")).clicked() { ui.close(); self.save_close(); }
                ui.separator();
                if ui.button(lk("c.close-tab", "Ctrl+W")).clicked() { ui.close(); let a = self.active; self.close_tab(a); }
                if ui.button(lk("c.close-all", "Ctrl+Shift+W")).clicked() { ui.close(); self.close_many(CloseKind::All); }
                if ui.button(lk("c.close-open", "Ctrl+Shift+F4")).clicked() { ui.close(); self.close_open(); }
                ui.separator();
                if ui.button(lk("c.reload", "")).clicked() { ui.close(); self.reload_current(); }
                ui.menu_button(tr("sub.reopen"), |ui| {
                    for cs in Charset::ALL {
                        if ui.button(cs.label()).clicked() { ui.close(); self.reopen_as(cs); }
                    }
                });
                ui.separator();
                if ui.button(lk("c.print", "Ctrl+P")).clicked() { ui.close(); self.print_cur(); }
                if ui.button(lk("c.browse", "")).clicked() { ui.close(); self.browse_current(); }
                if ui.button(lk("c.file-properties", "Alt+Enter")).clicked() { ui.close(); self.props = Some(self.cur().properties()); }
                if ui.button(lk("c.new-window", "")).clicked() { ui.close(); self.new_window(); }
                ui.separator();
                if ui.button(lk("c.exit", "")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
            });
            ui.menu_button(tr("menu.edit"), |ui| {
                if ui.button(lk("c.undo", "Ctrl+Z")).clicked() { self.cur().undo(); ui.close(); }
                if ui.button(lk("c.redo", "Ctrl+Y")).clicked() { self.cur().redo(); ui.close(); }
                ui.separator();
                if ui.button(lk("c.cut", "Ctrl+X")).clicked() { if let Some(c) = self.cur().edit("cut", None) { ctx.copy_text(c); } ui.close(); }
                if ui.button(lk("c.copy", "Ctrl+C")).clicked() { if let Some(c) = self.cur().edit("copy", None) { ctx.copy_text(c); } ui.close(); }
                if ui.button(lk("c.paste", "Ctrl+V")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste); ui.close(); }
                if ui.button(lk("c.select-all", "Ctrl+A")).clicked() { self.cur().select_all(); ui.close(); }
                ui.separator();
                self.cmd_menu(ui, ctx);
            });
            ui.menu_button(tr("menu.search"), |ui| {
                if ui.button(lk("c.find", "Ctrl+F")).clicked() { self.open_find(false); ui.close(); }
                if ui.button(lk("c.replace", "Ctrl+H / Ctrl+R")).clicked() { self.open_find(true); ui.close(); }
                if ui.button(lk("c.grep", "Ctrl+G")).clicked() { self.focus_grep(); ui.close(); }
                if ui.button(lk("c.find-next", "F3")).clicked() { self.find_next(true); ui.close(); }
                if ui.button(lk("c.find-prev", "Shift+F3")).clicked() { self.find_next(false); ui.close(); }
                if ui.button(lk("c.find-word", "Ctrl+F3")).clicked() { let i = self.active.min(self.docs.len() - 1); self.find.word_search(&mut self.docs[i], true); ui.close(); }
                if ui.button(lk("c.replace-all", "Ctrl+Alt+R")).clicked() { let i = self.active.min(self.docs.len() - 1); self.find.replace_all(&mut self.docs[i]); ui.close(); }
                ui.menu_button("比较", |ui| {
                    if ui.button("与其他标签比较…").clicked() { self.diff_pick = true; ui.close(); }
                    if ui.button("与文件比较…").clicked() { self.diff_with_file(); ui.close(); }
                    if ui.button("差异列表").clicked() { self.diff_list_open = true; ui.close(); }
                    if ui.button("下一处差异 (Alt+↓)").clicked() { self.diff_step(true); ui.close(); }
                    if ui.button("上一处差异 (Alt+↑)").clicked() { self.diff_step(false); ui.close(); }
                    if ui.button("清除比较").clicked() { self.diff = None; self.diff_list_open = false; ui.close(); }
                });
            });
            ui.menu_button(tr("menu.fold"), |ui| {
                if ui.button(lk("c.fold-toggle", "Ctrl+Shift+[")).clicked() { let d = self.cur(); let r = d.cursor[0]; d.toggle_fold(r); ui.close(); }
                if ui.button(lk("c.unfold-all", "Ctrl+Shift+]")).clicked() { self.cur().folds.clear(); ui.close(); }
            });
            ui.menu_button(tr("menu.plugin"), |ui| {
                let rows = plugins::rows();
                if ui.radio(self.theme_id.is_empty(), tr("c.builtin-theme")).clicked() { self.set_theme_plugin(""); ui.close(); }
                for r in &rows {
                    ui.horizontal(|ui| {
                        if !r.commands.is_empty() || r.script_only {
                            if ui.checkbox(&mut r.enabled.clone(), "").on_hover_text(tr("ui.plugin-enabled")).clicked() {
                                let _ = plugins::set_enabled(&r.id, !r.enabled);
                                if r.enabled { self.plug.unload(&r.id); }
                            }
                        }
                        if let Some(t) = &r.theme {
                            if ui.radio(self.theme_id == r.id, format!("{}{}", tr("ui.theme-prefix"), t.label)).clicked() { self.set_theme_plugin(&r.id); ui.close(); }
                        } else {
                            ui.label(format!("{} {}{}", r.name, r.version, if r.script_only && r.pele_like() { tr("ui.script-only") } else { "" }));
                        }
                        if ui.small_button(tr("c.delete")).clicked() {
                            let _ = plugins::remove(&r.id);
                            self.plug.unload(&r.id);
                            if self.theme_id == r.id { self.set_theme_plugin(""); }
                            ui.close();
                        }
                    });
                }
                // 脚本插件贡献的命令: 点了才(惰性)启动 JS 宿主
                let cmds: Vec<(String, String, String)> = rows.iter().filter(|r| r.enabled).flat_map(|r| r.commands.iter().map(|(c, t)| (r.id.clone(), c.clone(), t.clone()))).collect();
                if !cmds.is_empty() {
                    ui.separator();
                    for (id, c, t) in cmds {
                        if ui.button(format!("▶ {t}")).clicked() { ui.close(); self.run_plugin_command(&id, &c, &t); }
                    }
                }
                ui.separator();
                if ui.button(tr("c.plugin-import")).clicked() { ui.close(); self.import_plugin_zip(); }
                let gone = plugins::removed_bundled();
                if !gone.is_empty() {
                    ui.menu_button(tr("sub.plugin-restore"), |ui| {
                        for (id, name, _) in gone {
                            if ui.button(name).clicked() { let _ = plugins::restore(&id); ui.close(); }
                        }
                    });
                }
            });
            ui.menu_button(tr("menu.setting"), |ui| {
                if ui.button(lk("c.common-config", "")).clicked() { self.open_prefs(); ui.close(); }
                ui.menu_button(tr("sub.uilang"), |ui| {
                    for l in Lang::ALL {
                        if ui.radio(crate::i18n::cur() == l, l.native()).clicked() {
                            let mut n = self.current_settings();
                            n.lang = l.code().into();
                            self.apply_settings(&n);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button(lk("c.export-config", "")).clicked() { ui.close(); self.export_settings(); }
                if ui.button(lk("c.import-config", "")).clicked() { ui.close(); self.import_settings(); }
                if ui.button(lk("c.import-sakura", "")).clicked() { ui.close(); self.import_sakura(); }
                if ui.button(lk("c.reset-settings", "")).clicked() { ui.close(); self.reset_settings(); }
                ui.separator();
                if ui.button(lk("c.key-config", "")).clicked() { ui.close(); self.cmdui.open_keycfg(); }
            });
            ui.menu_button(tr("menu.bookmark"), |ui| {
                if ui.button(lk("c.bookmark-toggle", "Ctrl+F2")).clicked() { self.cur().bookmark("toggle"); ui.close(); }
                if ui.button(lk("c.bookmark-next", "F2")).clicked() { self.jump_mark(); self.cur().bookmark("next"); ui.close(); }
                if ui.button(lk("c.bookmark-prev", "Shift+F2")).clicked() { self.jump_mark(); self.cur().bookmark("prev"); ui.close(); }
                if ui.button(lk("c.bookmark-view", "Alt+F2")).clicked() { self.bm_open = true; ui.close(); }
                if ui.button(lk("c.bookmark-reset", "Ctrl+Shift+F2")).clicked() { self.cur().bookmarks.clear(); self.cur().tip = tr("dlg.bm-cleared").into(); ui.close(); }
                ui.separator();
                if ui.button(lk("c.bracket-jump", "Ctrl+[")).clicked() { self.bracket_jump(); ui.close(); }
                if ui.button(lk("c.hist-prev", "Alt+P")).clicked() { self.jump_go(-1); ui.close(); }
                if ui.button(lk("c.hist-next", "Alt+N")).clicked() { self.jump_go(1); ui.close(); }
                if ui.button(lk("c.hist-mark", "")).clicked() { self.jump_mark(); self.cur().tip = tr("dlg.hist-marked").into(); ui.close(); }
            });
            ui.menu_button(tr("menu.window"), |ui| {
                if ui.button(lk("c.split", "Ctrl+\\")).clicked() { self.toggle_split(); ui.close(); }
                ui.separator();
                if ui.button(lk("c.next-tab", "Ctrl+Tab")).clicked() { self.next_tab(true); ui.close(); }
                if ui.button(lk("c.prev-tab", "Ctrl+Shift+Tab")).clicked() { self.next_tab(false); ui.close(); }
                ui.menu_button(tr("sub.tabops"), |ui| {
                    if ui.button(lk("c.tab-move-left", "")).clicked() { self.move_active_tab(true); ui.close(); }
                    if ui.button(lk("c.tab-move-right", "")).clicked() { self.move_active_tab(false); ui.close(); }
                    if ui.button(lk("c.close-others", "")).clicked() { self.close_many(CloseKind::Others); ui.close(); }
                    if ui.button(lk("c.close-left", "")).clicked() { self.close_many(CloseKind::Left); ui.close(); }
                    if ui.button(lk("c.close-right", "")).clicked() { self.close_many(CloseKind::Right); ui.close(); }
                });
                ui.menu_button(tr("sub.winlist"), |ui| {
                    let mut pick = None;
                    for (i, d) in self.docs.iter().enumerate() {
                        if ui.selectable_label(i == self.active, format!("{}{}", if d.dirty { "● " } else { "" }, d.name)).clicked() { pick = Some(i); ui.close(); }
                    }
                    if let Some(i) = pick { self.select_tab(i); }
                });
                ui.separator();
                if ui.button(lk("c.minimize-window", "")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); ui.close(); }
                if ui.button(lk("c.maximize", "")).clicked() { let m = ctx.input(|i| i.viewport().maximized.unwrap_or(false)); ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!m)); ui.close(); }
                if ui.checkbox(&mut self.fullscreen.clone(), lk("c.fullscreen", "F11")).clicked() { self.toggle_fullscreen(ctx); ui.close(); }
                if ui.checkbox(&mut self.topmost.clone(), lk("c.topmost", "")).clicked() { self.toggle_topmost(ctx); ui.close(); }
            });
            ui.menu_button(tr("menu.macro"), |ui| {
                let rec = self.cur().rec.is_some();
                if ui.button(lk(if rec { "c.macro-stop" } else { "c.macro-start" }, "Ctrl+Shift+M")).clicked() { self.cur().macro_toggle(); ui.close(); }
                if ui.button(lk("c.macro-play", "Ctrl+Shift+L")).clicked() { self.cur().macro_play(); ui.close(); }
                ui.separator();
                if ui.button(lk("c.macro-run-doc", "")).clicked() { ui.close(); self.run_mac_doc(); }
                if ui.button(lk("c.macro-run-file", "")).clicked() { ui.close(); self.run_mac_file(); }
                if ui.button(lk("c.macro-run-ppa", "")).clicked() { ui.close(); self.run_ppa_doc(); }
            });
            ui.menu_button(tr("menu.view"), |ui| {
                if ui.checkbox(&mut self.split_on.clone(), lk("c.split", "Ctrl+\\")).clicked() { self.toggle_split(); ui.close(); }
                ui.checkbox(&mut self.term.open, lk("c.terminal", "Ctrl+`"));
                ui.checkbox(&mut self.md_preview, lk("c.md-preview", "Ctrl+Shift+V"));
                if ui.checkbox(&mut self.dark, lk("c.dark", "")).clicked() { ui.close(); }
                ui.checkbox(&mut self.highlight, lk("c.highlight", ""));
                ui.checkbox(&mut self.wrap, lk("c.wrap", "Alt+Z"));
                ui.checkbox(&mut self.side.open, lk("c.sidebar", "Ctrl+B"));
                ui.separator();
                ui.checkbox(&mut self.cfg.view.line_numbers, lk("c.line-numbers", ""));
                ui.checkbox(&mut self.cfg.view.status_bar, lk("c.status-bar", ""));
                ui.checkbox(&mut self.cfg.view.whitespace, lk("c.whitespace", ""));
                ui.checkbox(&mut self.cfg.view.ruler, lk("c.ruler", ""));
                ui.checkbox(&mut self.cfg.view.cur_line, lk("c.cur-line", ""));
                ui.checkbox(&mut self.cfg.view.minimap, lk("c.minimap", ""));
                if ui.button(lk("c.minimal-mode", "")).clicked() { self.set_minimal(); ui.close(); }
                ui.separator();
                ui.menu_button(tr("sub.tabwidth"), |ui| {
                    for w in [2usize, 3, 4, 8] {
                        if ui.radio_value(&mut self.tab, w, format!("{w}")).clicked() { ui.close(); }
                    }
                });
                if ui.button(lk("c.zoom-in", "Ctrl+=")).clicked() { self.font_px = (self.font_px + 1.0).min(40.0); }
                if ui.button(lk("c.zoom-out", "Ctrl+-")).clicked() { self.font_px = (self.font_px - 1.0).max(8.0); }
                if ui.button(lk("c.zoom-reset", "Ctrl+0")).clicked() { self.font_px = DEFAULT_FONT_PX; }
            });
            ui.menu_button(tr("menu.help"), |ui| {
                if ui.button(lk("c.palette", "")).clicked() { ui.close(); self.run_app(KApp::Palette, ctx); }
                if ui.button(lk("c.key-list", "")).clicked() { ui.close(); self.cmdui.open_keylist(); }
                if ui.button(lk("c.key-config", "")).clicked() { ui.close(); self.cmdui.open_keycfg(); }
                ui.separator();
                ui.label(tr("ui.about"));
            });
        });
    }

    /// 把当前文档的文本当 Sakura 宏运行 (作用在上一份活动的另一个文档上; 只有一份就作用在自己上)
    fn run_mac_doc(&mut self) {
        let src = self.cur().text_lf();
        let target = if self.docs.len() > 1 { (0..self.docs.len()).rev().find(|&i| i != self.active) } else { Some(self.active) };
        self.run_mac_src(&src, target.unwrap_or(self.active));
    }

    /// 点了插件菜单里的命令: 按权限取文档参数, 交给脚本宿主异步跑 (结果在 `ui()` 里轮询)
    fn run_plugin_command(&mut self, plugin: &str, cmd: &str, title: &str) {
        if plugin == "ppa" && cmd == "run" {
            return self.run_ppa_doc();
        }
        let Some(row) = plugins::rows().into_iter().find(|r| r.id == plugin && r.enabled) else {
            self.cur().tip = trf("dlg.plugin-off", &[plugin]);
            return;
        };
        let a = self.active;
        self.plug.start(plugin, &row.perms, cmd, title, &self.docs[a], None);
        self.docs[a].tip = trf("dlg.plugin-running", &[title]);
    }

    /// PPA: 脚本 = 选区 (没有选区就是当前 .ppa 文档), 作用在最近活动的另一份文档上 (有选区且只有一份文档就作用在自己上)
    fn run_ppa_doc(&mut self) {
        let Some(row) = plugins::rows().into_iter().find(|r| r.id == "ppa" && r.enabled) else {
            self.cur().tip = tr("dlg.ppa-off").into();
            return;
        };
        let sel = crate::plugin_ui::selection_text(self.cur());
        let script = if !sel.is_empty() {
            sel
        } else if self.cur().name.to_lowercase().ends_with(".ppa") {
            self.cur().text_lf()
        } else {
            self.cur().tip = tr("dlg.ppa-hint").into();
            return;
        };
        let other = (0..self.docs.len()).rev().find(|&i| i != self.active);
        let Some(target) = other.or(if script.is_empty() { None } else { Some(self.active) }) else { return };
        if other.is_none() && self.docs[self.active].name.to_lowercase().ends_with(".ppa") {
            self.cur().tip = tr("dlg.ppa-no-target").into();
            return;
        }
        self.plug.start("ppa", &row.perms, "run", "PPA", &self.docs[target], Some(serde_json::json!({ "script": script })));
        self.docs[self.active].tip = trf("dlg.plugin-running", &["PPA"]);
    }

    fn run_mac_file(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("Sakura 宏", &["mac"]).pick_file() else { return };
        match editcore::document_io::read_doc(&p, None) {
            Ok(d) => { let a = self.active; self.run_mac_src(&d.content, a); }
            Err(e) => self.cur().tip = format!("读取宏失败: {e}"),
        }
    }

    fn run_mac_src(&mut self, src: &str, target: usize) {
        let last = self.docs.len() - 1;
        let d = &mut self.docs[target.min(last)];
        d.tip = match crate::macros::run_mac(d, src) { Ok(n) => format!("宏已执行 ({n} 步)"), Err(e) => format!("宏未执行: {e}") };
    }

    /// 终端面板: 输出 + 输入行 (Enter 执行, ↑↓ 翻历史)
    fn term_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("终端");
            if ui.small_button("清屏").clicked() { self.term.clear(); }
            if ui.small_button("重启").on_hover_text("结束当前 shell, 下一条命令重新开一个 (Ctrl+C)").clicked() { self.term.restart(); }
            ui.checkbox(&mut self.term.persistent, "常驻会话");
            if ui.small_button("×").clicked() { self.term.open = false; }
            ui.checkbox(&mut self.term_stdin, "选区作标准输入");
            ui.checkbox(&mut self.term_replace, "输出替换选区");
            if self.term.running() { ui.label("运行中…"); }
        });
        let h = (ui.available_height() - 28.0).max(30.0);
        egui::ScrollArea::vertical().max_height(h).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(self.term.display()).monospace()).selectable(true));
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.term.input).font(egui::TextStyle::Monospace).hint_text("输入命令, Enter 执行").desired_width(f32::INFINITY));
        if r.has_focus() {
            if self.term.input.is_empty() && ui.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::C)) { self.term.restart(); }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) { self.term.history_step(true); }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) { self.term.history_step(false); }
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let cwd = self.side.root.clone().or_else(|| self.docs[self.active.min(self.docs.len() - 1)].path.as_ref().and_then(|p| p.parent().map(|x| x.to_path_buf()))).unwrap_or_else(|| PathBuf::from("."));
            let (want_in, want_rep) = (self.term_stdin, self.term_replace);
            let (stdin, rdoc) = if want_in || want_rep {
                let d = self.cur();
                let sel = d.edit("copy", None).unwrap_or_default();
                (Some(sel), if want_rep { Some(d.id) } else { None })
            } else { (None, None) };
            self.term.submit(cwd, stdin, rdoc);
            r.request_focus();
        }
    }

    /// Markdown 预览: 块按 (文档, 版本) 缓存解析; 点块 = 编辑区跳到那一行, 点链接 = 打开 / 跳到标题
    fn md_ui(&mut self, ui: &mut egui::Ui) {
        let ai = self.active.min(self.docs.len() - 1);
        let key = (self.docs[ai].id, self.docs[ai].rev);
        if self.md_parsed.as_ref().map(|p| p.0) != Some(key) {
            self.md_parsed = Some((key, crate::markdown::parse(&self.docs[ai].lines)));
        }
        let blocks = self.md_parsed.take().expect("刚刚填好");
        let theme = self.tc.syntax_theme();
        let base = self.docs[ai].path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
        let cx = crate::md_view::MdCtx { base: base.as_deref(), registry: &self.registry, theme: &theme, tc: self.tc };
        let ev = crate::md_view::show(ui, &blocks.1, &cx, &mut self.md_cache, self.md_scroll_to.take());
        if let Some(l) = ev.jump { self.docs[ai].goto(l, 0); }
        if let Some(link) = ev.link {
            if let Some(name) = link.strip_prefix("wiki:") {
                self.docs[ai].tip = format!("维基链接 [[{name}]]: 预览里不跳转");
            } else if let Some(anchor) = link.strip_prefix('#') {
                // 页内锚点: 找标题文字相同 (忽略大小写和空白) 的块
                let norm = |s: &str| s.trim().to_lowercase().replace(' ', "-");
                let hit = blocks.1.iter().position(|b| matches!(&b.block, crate::markdown::Block::Heading { text, .. } if norm(text) == norm(anchor)));
                match hit { Some(i) => self.md_scroll_to = Some(blocks.1[i].line), None => self.docs[ai].tip = format!("没有找到标题 #{anchor}") }
            } else if crate::md_view::openable(&link) {
                if let Err(e) = crate::md_view::open_url(&link) { self.docs[ai].tip = e; }
            } else {
                self.docs[ai].tip = format!("不打开此链接: {link}");
            }
        }
        self.md_parsed = Some(blocks);
    }

    /// 自动换行下的 ↑ ↓ Home End: 用当前行 (和相邻行) 的真实排版找「上一 / 下一显示行」同一横坐标处
    fn wrap_nav(&mut self, ctx: &egui::Context, font: &FontId, wrap_w: f32, tab: usize, key: Key) {
        use egui::text::CCursor;
        let ai = self.active.min(self.docs.len() - 1);
        let (r, c) = (self.docs[ai].cursor[0], self.docs[ai].cursor[1]);
        let layout = |ctx: &egui::Context, line: &str| {
            let (disp, map) = expand_tabs(line, tab);
            let mut job = egui::text::LayoutJob::simple(disp, font.clone(), Color32::WHITE, wrap_w);
            job.wrap.break_anywhere = true;
            (ctx.fonts_mut(|f| f.layout_job(job)), map)
        };
        let to_col = |idx: usize, map: &[usize]| -> usize { let mut col = 0; for (k, &m) in map.iter().enumerate() { if m <= idx { col = k; } else { break; } } col.min(map.len().saturating_sub(1)) };
        let (g, map) = layout(ctx, &self.docs[ai].lines[r]);
        let here = g.pos_from_cursor(CCursor::new(map[c.min(map.len() - 1)]));
        let x = self.goal_x.unwrap_or(here.min.x);
        let (nr, ncol, keep_goal) = match key {
            Key::Home => (r, to_col(g.cursor_from_pos(Vec2::new(0.0, here.min.y + 1.0)).index.into(), &map), false),
            Key::End => (r, to_col(g.cursor_from_pos(Vec2::new(f32::MAX / 4.0, here.min.y + 1.0)).index.into(), &map), false),
            Key::ArrowUp => {
                if here.min.y > 1.0 {
                    (r, to_col(g.cursor_from_pos(Vec2::new(x, here.min.y - 1.0)).index.into(), &map), true)
                } else if r > 0 {
                    let (g2, m2) = layout(ctx, &self.docs[ai].lines[r - 1]);
                    (r - 1, to_col(g2.cursor_from_pos(Vec2::new(x, g2.size().y - 1.0)).index.into(), &m2), true)
                } else { (r, c, true) }
            }
            _ => {
                if here.max.y < g.size().y - 1.0 {
                    (r, to_col(g.cursor_from_pos(Vec2::new(x, here.max.y + 1.0)).index.into(), &map), true)
                } else if r + 1 < self.docs[ai].lines.len() {
                    let (g2, m2) = layout(ctx, &self.docs[ai].lines[r + 1]);
                    (r + 1, to_col(g2.cursor_from_pos(Vec2::new(x, 1.0)).index.into(), &m2), true)
                } else { (r, c, true) }
            }
        };
        self.docs[ai].goto(nr, ncol);
        self.goal_x = if keep_goal { Some(x) } else { None };
    }

    fn find_next(&mut self, fwd: bool) {
        let i = self.active.min(self.docs.len() - 1);
        self.find.next(&mut self.docs[i], fwd);
    }

    /// 打开查找栏 (`replace` = 同时显示置换行)
    fn open_find(&mut self, replace: bool) {
        let i = self.active.min(self.docs.len() - 1);
        self.find.open_with(&self.docs[i], replace);
    }

    /// 快捷键分发: 表驱动 (keymap.rs 的注册表 + 用户改过的键), 修饰键多的先消费 (Ctrl+Shift+F3 先于 F3)。
    /// 只有 Esc 的多级取消是固定逻辑。等用户按新键时 (键位设置对话框) 一律不分发。
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.cmdui.capturing() { return; }
        let cmd = |ctx: &egui::Context, m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        for (idx, m, k) in keymap::bound() {
            if keymap::entries()[idx].needs_diff && self.diff.is_none() { continue; }
            if cmd(ctx, m, k) { self.run_entry(idx, ctx); }
        }
        // Esc: 先停掉正在跑的 Grep, 再关查找栏, 最后才是清矩形选区 / 多光标
        if cmd(ctx, Modifiers::NONE, Key::Escape) {
            if self.side.grep.running() { self.side.grep.cancel(); self.side.grep.msg = format!("已停止, {} 处命中", self.side.grep.results.len()); }
            else if self.find.open { self.find.open = false; }
            else { let d = self.cur(); d.box_sel = None; d.extra.clear(); }
        }
    }

    /// 跑注册表里的一条命令 (快捷键、命令面板共用)
    fn run_entry(&mut self, idx: usize, ctx: &egui::Context) {
        match keymap::entries()[idx].run {
            Run::Act(a) => self.run_action(a, ctx),
            Run::App(a) => self.run_app(a, ctx),
        }
    }

    fn run_item(&mut self, it: Item, ctx: &egui::Context) {
        match it {
            Item::Cmd(i) => self.run_entry(i, ctx),
            Item::Plugin(i) => {
                if let Some((id, c, t)) = self.pal_plugins.get(i).cloned() { self.run_plugin_command(&id, &c, &t); }
            }
        }
    }

    /// 命令面板里可选的脚本插件命令 (启用的插件贡献的)
    fn plugin_cmds() -> Vec<PluginCmd> {
        plugins::rows().into_iter().filter(|r| r.enabled).flat_map(|r| r.commands.iter().map(|(c, t)| (r.id.clone(), c.clone(), t.clone())).collect::<Vec<_>>()).collect()
    }

    /// 窗口级动作 (菜单、快捷键、命令面板都走这里)
    fn run_app(&mut self, a: KApp, ctx: &egui::Context) {
        let ai = self.active.min(self.docs.len() - 1);
        match a {
            KApp::NewDoc => self.new_doc(),
            KApp::OpenFile => self.open_dialog(),
            KApp::OpenFolder => self.open_folder_dialog(),
            KApp::Save => { self.save(false); }
            KApp::SaveAs => { self.save(true); }
            KApp::SaveAll => self.save_all(),
            KApp::SaveClose => self.save_close(),
            KApp::CloseTab => { let t = self.active; self.close_tab(t); }
            KApp::CloseAll => self.close_many(CloseKind::All),
            KApp::CloseOpen => self.close_open(),
            KApp::Reload => self.reload_current(),
            KApp::RecentClear => self.clear_recent(),
            KApp::Print => self.print_cur(),
            KApp::Browse => self.browse_current(),
            KApp::Props => self.props = Some(self.cur().properties()),
            KApp::NewWindow => self.new_window(),
            KApp::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            KApp::Undo => self.cur().undo(),
            KApp::Redo => self.cur().redo(),
            KApp::Cut => { if let Some(c) = self.cur().edit("cut", None) { ctx.copy_text(c); } }
            KApp::Copy => { if let Some(c) = self.cur().edit("copy", None) { ctx.copy_text(c); } }
            KApp::Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            KApp::SelectAll => self.cur().select_all(),
            KApp::Find => self.open_find(false),
            KApp::Replace => self.open_find(true),
            KApp::Grep => self.focus_grep(),
            KApp::FindNext => self.find_next(true),
            KApp::FindPrev => self.find_next(false),
            KApp::FindWord => self.find.word_search(&mut self.docs[ai], true),
            KApp::FindWordPrev => self.find.word_search(&mut self.docs[ai], false),
            KApp::ReplaceAll => self.find.replace_all(&mut self.docs[ai]),
            KApp::DiffTab => self.diff_pick = true,
            KApp::DiffFile => self.diff_with_file(),
            KApp::DiffList => self.diff_list_open = true,
            KApp::DiffNext => self.diff_step(true),
            KApp::DiffPrev => self.diff_step(false),
            KApp::DiffClear => { self.diff = None; self.diff_list_open = false; }
            KApp::FoldToggle => { let d = self.cur(); let r = d.cursor[0]; d.toggle_fold(r); }
            KApp::UnfoldAll => self.cur().folds.clear(),
            KApp::BmToggle => self.cur().bookmark("toggle"),
            KApp::BmNext => { self.jump_mark(); self.cur().bookmark("next"); }
            KApp::BmPrev => { self.jump_mark(); self.cur().bookmark("prev"); }
            KApp::BmView => self.bm_open = !self.bm_open,
            KApp::BmReset => { self.cur().bookmarks.clear(); self.cur().tip = tr("dlg.bm-cleared").into(); }
            KApp::BracketJump => self.bracket_jump(),
            KApp::HistPrev => self.jump_go(-1),
            KApp::HistNext => self.jump_go(1),
            KApp::HistMark => { self.jump_mark(); self.cur().tip = tr("dlg.hist-marked").into(); }
            KApp::Split => self.toggle_split(),
            KApp::NextTab => self.next_tab(true),
            KApp::PrevTab => self.next_tab(false),
            KApp::TabLeft => self.move_active_tab(true),
            KApp::TabRight => self.move_active_tab(false),
            KApp::CloseOthers => self.close_many(CloseKind::Others),
            KApp::CloseLeft => self.close_many(CloseKind::Left),
            KApp::CloseRight => self.close_many(CloseKind::Right),
            KApp::Minimize => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            KApp::Maximize => { let m = ctx.input(|i| i.viewport().maximized.unwrap_or(false)); ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!m)); }
            KApp::Fullscreen => self.toggle_fullscreen(ctx),
            KApp::Topmost => self.toggle_topmost(ctx),
            KApp::MacroToggle => self.cur().macro_toggle(),
            KApp::MacroPlay => self.cur().macro_play(),
            KApp::MacroRunDoc => self.run_mac_doc(),
            KApp::MacroRunFile => self.run_mac_file(),
            KApp::MacroRunPpa => self.run_ppa_doc(),
            KApp::Terminal => self.term.open = !self.term.open,
            KApp::MdPreview => self.md_preview = !self.md_preview,
            KApp::Dark => self.dark = !self.dark,
            KApp::Highlight => self.highlight = !self.highlight,
            KApp::Wrap => self.wrap = !self.wrap,
            KApp::Sidebar => self.side.open = !self.side.open,
            KApp::LineNumbers => self.cfg.view.line_numbers = !self.cfg.view.line_numbers,
            KApp::StatusBar => self.cfg.view.status_bar = !self.cfg.view.status_bar,
            KApp::Whitespace => self.cfg.view.whitespace = !self.cfg.view.whitespace,
            KApp::Ruler => self.cfg.view.ruler = !self.cfg.view.ruler,
            KApp::CurLine => self.cfg.view.cur_line = !self.cfg.view.cur_line,
            KApp::Minimap => self.cfg.view.minimap = !self.cfg.view.minimap,
            KApp::MinimalMode => self.set_minimal(),
            KApp::ZoomIn => self.font_px = (self.font_px + 1.0).min(40.0),
            KApp::ZoomOut => self.font_px = (self.font_px - 1.0).max(8.0),
            KApp::ZoomReset => self.font_px = DEFAULT_FONT_PX,
            KApp::Prefs => self.open_prefs(),
            KApp::ExportCfg => self.export_settings(),
            KApp::ImportCfg => self.import_settings(),
            KApp::ImportSakura => self.import_sakura(),
            KApp::ResetCfg => self.reset_settings(),
            KApp::KeyConfig => self.cmdui.open_keycfg(),
            KApp::KeyList => self.cmdui.open_keylist(),
            KApp::Palette => {
                self.cmdui.toggle_palette();
                self.pal_plugins = Self::plugin_cmds();
            }
        }
    }

    /// 命令面板 / 键位设置 / 键位一览三个窗口; 键位改动立刻套用并落盘
    fn cmd_windows(&mut self, ctx: &egui::Context) {
        if let Some(it) = self.cmdui.palette_window(ctx, &self.pal_plugins) { self.run_item(it, ctx); }
        if let Some(ov) = self.cmdui.keycfg_window(ctx, &self.cfg.keys) {
            let mut next = self.current_settings();
            next.keys = ov;
            self.apply_settings(&next);
            self.persist_if_changed();
        }
        if let Some(tip) = self.cmdui.keylist_window(ctx) { self.cur().tip = tip; }
    }

    /// 编辑区: 输入事件 -> doc; 然后只画看得见的行
    /// 一栏编辑区: 临时把 active 换成这栏的文档, 画完换回; 点击这栏 = 它拿到焦点
    fn editor_pane(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, find_has_focus: bool, right: bool) {
        if !self.split_on {
            self.pane_keys = true;
            return self.editor(ui, ctx, find_has_focus);
        }
        let last = self.docs.len() - 1;
        let doc = if right { self.right_doc.min(last) } else { self.left_doc.min(last) };
        let keep = self.active;
        self.active = doc;
        self.pane_keys = self.focus_right == right;
        self.editor(ui, ctx, find_has_focus);
        self.active = keep;
        if ctx.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            self.focus_right = right;
        }
    }

    fn print_cur(&mut self) {
        let (name, lines) = { let d = self.cur(); (d.name.clone(), d.lines.clone()) };
        let pt = (self.font_px * 0.75).clamp(7.0, 14.0);
        let tip = match crate::print::open_for_print(&name, &lines, true, pt) { Ok(_) => "已在浏览器打开打印页".to_string(), Err(e) => e };
        self.cur().tip = tip;
    }

    /// 极简布局 (Sakura 风格的单个文本框): 收起侧栏 / 终端 / 预览 / 分割, 只留编辑区和状态栏
    fn set_minimal(&mut self) {
        self.side.open = false;
        self.term.open = false;
        self.md_preview = false;
        if self.split_on { self.toggle_split(); }
        self.cfg.view.status_bar = true;
        self.cfg.view.minimap = false;
        self.cfg.view.ruler = false;
    }

    /// 选主题插件 (空 id = 回到内置深 / 浅色)
    fn set_theme_plugin(&mut self, id: &str) {
        self.theme_id = id.to_string();
        let spec = plugins::rows().into_iter().find(|r| r.id == id && r.enabled).and_then(|r| r.theme);
        self.theme_visuals = spec.as_ref().map(|t| plugins::visuals(&t.vars));
        self.theme_vars = spec.map(|t| t.vars);
        if self.theme_visuals.is_none() { self.theme_id.clear(); }
    }

    /// 选一个 zip -> 给确认框 (名称 / 版本 / 权限) -> 安装
    fn import_plugin_zip(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("插件包", &["zip"]).pick_file() else { return };
        let bytes = match std::fs::read(&path) { Ok(b) => b, Err(e) => { self.cur().tip = format!("读不了 {}: {e}", path.display()); return; } };
        let msg = match plugins::inspect_zip(&bytes) {
            Err(e) => { rfd::MessageDialog::new().set_title(tr("dlg.cant-import")).set_description(e).show(); return; }
            Ok((note, perms)) => {
                let ok = rfd::MessageDialog::new().set_title(tr("dlg.import-plugin")).set_description(format!("{note}\n\n来源: {} (未验证)\n导入吗?", path.display())).set_buttons(rfd::MessageButtons::YesNo).show() == rfd::MessageDialogResult::Yes;
                if !ok { return; }
                match plugins::install_zip(&bytes, &perms) { Ok(m) => m, Err(e) => e }
            }
        };
        self.cur().tip = msg;
    }

    /// 命令表 -> 菜单: 空 group 的直接放, 其余按 group 分成子菜单 (首次出现的顺序)
    fn cmd_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut groups: Vec<&'static str> = vec![];
        for c in cmds::CMDS {
            if !groups.contains(&c.group) { groups.push(c.group); }
        }
        let label = |c: &cmds::Cmd| match keymap::display(&keymap::action_id(c.action)).filter(|k| !k.is_empty()) { Some(k) => format!("{}  {}", trz(c.label), k), None => trz(c.label).to_string() };
        for g in groups {
            if g.is_empty() {
                for c in cmds::CMDS.iter().filter(|c| c.group.is_empty()) {
                    let checked = match c.action {
                        Action::ToggleOverwrite => Some(self.cur().overwrite),
                        Action::ToggleReadonly => Some(self.cur().readonly),
                        _ => None,
                    };
                    let text = label(c);
                    let clicked = match checked { Some(mut on) => ui.checkbox(&mut on, text).clicked(), None => ui.button(text).clicked() };
                    if clicked { self.run_action(c.action, ctx); ui.close(); }
                }
            } else {
                ui.menu_button(trz(g), |ui| {
                    for c in cmds::CMDS.iter().filter(|c| c.group == g) {
                        if ui.button(label(c)).clicked() { self.run_action(c.action, ctx); ui.close(); }
                    }
                });
            }
        }
    }

    fn run_action(&mut self, a: Action, ctx: &egui::Context) {
        let name = self.cur().name.clone();
        let tab = self.eff(&name).0;
        match a {
            Action::Edit(n) => { if let Some(c) = self.cur().edit(n, None) { ctx.copy_text(c); } }
            Action::Transform(k) => self.cur().transform_sel(k, tab),
            Action::Lines(k) => self.cur().lines_op(k),
            Action::Indent => self.cur().indent(false, tab),
            Action::Outdent => self.cur().indent(true, tab),
            Action::ToggleComment => {
                let name = self.cur().name.clone();
                match self.registry.detect(&name).and_then(|t| t.line_comment.clone()) {
                    Some(mark) => { self.cur().edit("toggle-line-comment", Some(mark)); }
                    None => self.cur().tip = "这个类型没有行注释符".into(),
                }
            }
            Action::GotoLine => { self.goto_open = true; self.goto_text.clear(); }
            Action::ToggleOverwrite => { let d = self.cur(); d.overwrite = !d.overwrite; d.tip = if d.overwrite { "改写模式".into() } else { "插入模式".into() }; }
            Action::ToggleReadonly => { let d = self.cur(); d.readonly = !d.readonly; d.tip = if d.readonly { "已设为只读".into() } else { "已取消只读".into() }; }
        }
    }

    fn goto_window(&mut self, ctx: &egui::Context) {
        let mut go: Option<usize> = None;
        let mut open = self.goto_open;
        egui::Window::new(tr("dlg.goto-title")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            let total = self.docs[self.active.min(self.docs.len() - 1)].lines.len();
            let r = ui.add(egui::TextEdit::singleline(&mut self.goto_text).hint_text(format!("1 – {total}")).desired_width(120.0));
            r.request_focus();
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                go = self.goto_text.trim().parse::<usize>().ok();
                if go.is_none() { self.cur().tip = tr("dlg.goto-bad").into(); }
                self.goto_open = false;
            }
        });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) { self.goto_open = false; }
        if let Some(n) = go { self.jump_mark(); let d = self.cur(); let row = n.saturating_sub(1).min(d.lines.len() - 1); d.goto(row, 0); }
    }

    /// 窗口重新获得焦点时: 看每个文档的文件有没有被外部改过; 没动过就静默重载, 有未保存修改就问
    fn check_external(&mut self) {
        for i in 0..self.docs.len() {
            if !self.docs[i].changed_on_disk() { continue; }
            let name = self.docs[i].name.clone();
            let reload = if !self.docs[i].dirty {
                true
            } else {
                rfd::MessageDialog::new().set_title(tr("dlg.ext-title")).set_description(trf("dlg.ext-body", &[&name])).set_buttons(rfd::MessageButtons::YesNo).show() == rfd::MessageDialogResult::Yes
            };
            if reload {
                if let Err(e) = self.docs[i].reload() { self.docs[i].tip = e; }
                else { self.docs[i].tip = tr("dlg.ext-reloaded").into(); }
            } else {
                self.docs[i].stamp = crate::doc::stamp_of(self.docs[i].path.as_deref().unwrap_or(std::path::Path::new("")));
            }
        }
    }

    fn reload_current(&mut self) {
        let (dirty, name) = { let d = self.cur(); (d.dirty, d.name.clone()) };
        if dirty && rfd::MessageDialog::new().set_title(tr("dlg.reload-title")).set_description(trf("dlg.reload-body", &[&name])).set_buttons(rfd::MessageButtons::YesNo).show() != rfd::MessageDialogResult::Yes { return; }
        if let Err(e) = self.cur().reload() { self.cur().tip = e; }
    }

    fn toggle_split(&mut self) {
        self.split_on = !self.split_on;
        if self.split_on {
            let a = self.active.min(self.docs.len() - 1);
            self.left_doc = a;
            self.right_doc = if self.docs.len() > 1 { (a + 1) % self.docs.len() } else { a };
            self.focus_right = false;
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, find_has_focus: bool) {
        let font = FontId::new(self.font_px, crate::fonts::editor_family());
        let (base_h, char_w) = ctx.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        // 行高倍数: 整行(含自动换行的每个显示行)都按这个高度排, 文字在行内居中
        let row_h = (base_h * self.cfg.line_height).max(4.0);
        let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
        let text_color = self.cfg.colors.text.map(rgb).unwrap_or_else(|| ui.visuals().text_color());
        let weak = ui.visuals().weak_text_color();
        let sel_bg = ui.visuals().selection.bg_fill;
        let max_cols = self.max_cols;
        let ai = self.active.min(self.docs.len() - 1);
        let (tab, wrap_pref, _) = self.eff(&self.docs[ai].name.clone());
        let mut theme = self.tc.syntax_theme();
        // 词法颜色覆盖 (首选项「字体与颜色」)
        for (kind, c) in [(TokenKind::Keyword, self.cfg.colors.keyword), (TokenKind::String, self.cfg.colors.string), (TokenKind::Comment, self.cfg.colors.comment), (TokenKind::Number, self.cfg.colors.number)] {
            if let Some(c) = c {
                let mut st = theme.get(kind);
                st.fg = Color(c[0], c[1], c[2]);
                theme.styles.insert(kind, st);
            }
        }
        let marks = self.find.marks_for(&self.docs[ai]);
        let mark_bg = Color32::from_rgba_unmultiplied(255, 200, 0, 70);
        let bracket_bg = Color32::from_rgba_unmultiplied(110, 190, 255, 95);
        // 括弧强调: 光标在括弧上 (或紧跟其后) 时两个括弧都垫底色; 光标 / 文本没动就用缓存
        let bracket = {
            let d = &self.docs[ai];
            let key = (d.id, d.rev, d.cursor);
            if self.bracket_cache.as_ref().map(|c| c.0) != Some(key) { self.bracket_cache = Some((key, d.bracket_pair())); }
            self.bracket_cache.as_ref().and_then(|c| c.1)
        };
        let cfg: Option<TypeConfig> = if self.highlight { self.registry.detect(&self.docs[ai].name).cloned() } else { None };
        let digits = format!("{}", self.docs[ai].lines.len()).len().max(3);
        let vw = self.cfg.view.clone();
        let tc = self.tc;
        let gutter = if vw.line_numbers { digits as f32 * char_w + 18.0 } else { 18.0 };
        let pad = 6.0;
        // 自动换行: 每个逻辑行按可用宽度折成若干显示行, 行高不等 -> 记累计高度 (太长的文档不换行, 免得每次排版都要扫全篇)
        let wrap = wrap_pref && self.docs[ai].lines.len() <= 20000;
        let wrap_w = if wrap { (ui.available_width() - gutter - pad * 2.0 - 16.0).max(char_w * 8.0) } else { f32::INFINITY };
        let use_offs = wrap || !self.docs[ai].folds.is_empty();
        let offs: Vec<f32> = if use_offs {
            let fsig: usize = self.docs[ai].folds.iter().map(|f| f.0 * 31 + f.1).sum::<usize>() + self.docs[ai].folds.len();
            let key = (self.docs[ai].id, self.docs[ai].rev, wrap_w as i32 + fsig as i32 * 7, (self.font_px * 10.0) as i32 + (self.cfg.line_height * 1000.0) as i32 * 1000, tab + (wrap as usize) * 100);
            if self.wrap_cache.as_ref().map(|c| c.0) != Some(key) {
                let d = &self.docs[ai];
                let mut acc = Vec::with_capacity(d.lines.len() + 1);
                let mut y = 0.0f32;
                for (r, l) in d.lines.iter().enumerate() {
                    acc.push(y);
                    if d.is_hidden(r) { continue; }
                    if wrap {
                        let (disp, _) = expand_tabs(l, tab);
                        let mut job = egui::text::LayoutJob::simple(disp, font.clone(), Color32::WHITE, wrap_w);
                        job.wrap.break_anywhere = true;
                        for sec in &mut job.sections { sec.format.line_height = Some(row_h); }
                        y += ctx.fonts_mut(|f| f.layout_job(job)).size().y.max(row_h);
                    } else {
                        y += row_h;
                    }
                }
                acc.push(y);
                self.wrap_cache = Some((key, acc));
            }
            self.wrap_cache.as_ref().map(|c| c.1.clone()).unwrap_or_default()
        } else { vec![] };
        let top = |row: usize| -> f32 { if use_offs { offs.get(row).copied().unwrap_or(0.0) } else { row as f32 * row_h } };
        let total_h = if use_offs { offs.last().copied().unwrap_or(0.0) + row_h * 2.0 } else { self.docs[ai].lines.len() as f32 * row_h + row_h * 2.0 };
        let page_rows = ((ui.available_height() / row_h) as usize).max(1);

        // ---- 键盘 (查找框有焦点时不抢) ----
        let mut clip_out: Option<String> = None;
        let mut preedit_now = self.preedit.clone();
        let mut jump_row: Option<usize> = None; // Grep 结果文档里 Enter 的那一行
        if !find_has_focus && self.pane_keys {
            let mut events = ctx.input(|i| i.events.clone());
            if wrap {
                // 自动换行下 ↑ ↓ Home End 按显示行走 (按真实排版取位置); 带修饰键的仍交给 editcore
                let mut rest = Vec::with_capacity(events.len());
                for ev in events {
                    if let egui::Event::Key { key, pressed: true, modifiers, .. } = &ev {
                        if !modifiers.ctrl && !modifiers.command && !modifiers.alt && !modifiers.shift && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) {
                            let k = *key;
                            self.wrap_nav(ctx, &font, wrap_w, tab, k);
                            continue;
                        }
                    }
                    rest.push(ev);
                }
                events = rest;
            }
            let d = &mut self.docs[ai];
            for ev in events {
                match ev {
                    egui::Event::Text(t) => { if !t.chars().all(|c| c.is_control()) { d.type_text(&t); } }
                    egui::Event::Ime(egui::ImeEvent::Commit(t)) => { preedit_now.clear(); d.type_text(&t); }
                    egui::Event::Ime(egui::ImeEvent::Preedit { text: t, .. }) => preedit_now = t,
                    egui::Event::Paste(t) => d.type_text(&t),
                    egui::Event::Copy => { if let Some(c) = d.edit("copy", None) { clip_out = Some(c); } }
                    egui::Event::Cut => { if let Some(c) = d.edit("cut", None) { clip_out = Some(c); } }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        let (ctrl, shift) = (modifiers.command || modifiers.ctrl, modifiers.shift);
                        let nav = match key {
                            Key::ArrowLeft => Some("ArrowLeft"), Key::ArrowRight => Some("ArrowRight"), Key::ArrowUp => Some("ArrowUp"), Key::ArrowDown => Some("ArrowDown"),
                            Key::Home => Some("Home"), Key::End => Some("End"), Key::PageUp => Some("PageUp"), Key::PageDown => Some("PageDown"), _ => None,
                        };
                        if modifiers.alt && !ctrl { continue; }
                        if let Some(k) = nav { d.nav(k, ctrl, shift, page_rows); continue; }
                        match key {
                            Key::Enter => { if d.grep_result { jump_row = Some(d.cursor[0]); } else { d.edit("new-line", None); } }
                            Key::Backspace => { d.edit(if ctrl { "delete-word-start" } else { "backspace" }, None); }
                            Key::Delete => { d.edit(if ctrl { "delete-word-end" } else { "delete" }, None); }
                            Key::Tab => { d.indent(false, tab); }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(c) = clip_out { ctx.copy_text(c); }
        if let Some(r) = jump_row { self.jump_req = Some((self.docs[ai].id, r)); }
        self.preedit = preedit_now;
        let preedit = self.preedit.clone();

        // ---- 绘制 ----
        let reveal = std::mem::take(&mut self.docs[ai].reveal);
        let center = reveal && std::mem::take(&mut self.center_once);
        let mut widest = max_cols;
        if !self.docs[ai].grep_result { let _ = self.docs[ai].changed(); }  // 修改行标记: 按 rev 缓存, 绘制时只读
        // 列标尺: 固定在编辑区上方, 横向跟着滚动 (滚动量在 ScrollArea 返回后才知道, 先占位)
        let ruler_rect = if vw.ruler { Some(ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover()).0) } else { None };
        let mut clicked: Option<(Pos2, bool, bool)> = None; // (位置, 是否Shift, 是否拖动)
        let mut multi_click = 0u8; // 2 = 双击选词, 3 = 三击选行
        let mut ed_id: Option<egui::Id> = None;
        let out = egui::ScrollArea::both().auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
            let width = (widest as f32 * char_w + gutter + pad * 2.0 + 40.0).max(ui.available_width());
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, total_h), Sense::click_and_drag());
            ed_id = Some(resp.id);
            let painter = ui.painter_at(rect.intersect(ui.clip_rect()));
            let d = &self.docs[ai];
            let dmarks: Option<&[crate::diff::Mark]> = self.diff.as_ref().and_then(|v| v.marks_of(d.id));
            let (first, last) = if use_offs {
                let f = offs.partition_point(|&o| o <= viewport.min.y).saturating_sub(1).min(d.lines.len().saturating_sub(1));
                let l = (offs.partition_point(|&o| o < viewport.max.y) + 1).min(d.lines.len());
                (f, l)
            } else {
                ((viewport.min.y / row_h).floor().max(0.0) as usize, ((viewport.max.y / row_h).ceil() as usize + 1).min(d.lines.len()))
            };
            let x0 = rect.min.x + gutter + pad;
            let is_diff = crate::view::is_diff_name(&d.name);
            for row in first..last {
                if d.is_hidden(row) { continue; }
                let y = rect.min.y + top(row);
                let (disp, map) = expand_tabs(&d.lines[row], tab);
                widest = widest.max(disp.chars().count());
                let row_c = if is_diff { crate::view::diff_line_color(&d.lines[row], &tc).unwrap_or(text_color) } else { text_color };
                let mut job = highlight_job(&d.lines[row], &disp, &map, if is_diff { None } else { cfg.as_ref() }, &theme, &font, row_c, row_h);
                if wrap { job.wrap.max_width = wrap_w; job.wrap.break_anywhere = true; }
                let galley = painter.layout_job(job);
                // 选区 / 查找命中的底色都走这一个函数: 一段 [c0, c1) 列 (自动换行时逐字符取排版位置, 否则一整块)
                let span = |c0: usize, c1: usize, extend: bool, color: Color32| {
                    if wrap {
                        let (ds, de) = (map[c0.min(map.len() - 1)], map[c1.min(map.len() - 1)]);
                        for k in ds..de {
                            let a = galley.pos_from_cursor(egui::text::CCursor::new(k)).min;
                            let b = galley.pos_from_cursor(egui::text::CCursor::new(k + 1)).min;
                            let xe = if (b.y - a.y).abs() < 0.5 && b.x > a.x { b.x } else { a.x + char_w };
                            painter.rect_filled(Rect::from_min_max(Pos2::new(x0 + a.x, y + a.y), Pos2::new(x0 + xe, y + a.y + row_h)), 0.0, color);
                        }
                    } else {
                        let xa = x_of(&painter, &disp, &map, c0, &font, text_color);
                        let mut xb = x_of(&painter, &disp, &map, c1.min(map.len() - 1), &font, text_color);
                        if extend { xb += char_w * 0.6; } // 跨行的行尾多画一点, 表示换行也被选中
                        painter.rect_filled(Rect::from_min_max(Pos2::new(x0 + xa, y), Pos2::new(x0 + xb, y + row_h)), 0.0, color);
                    }
                };
                if let Some(ms) = &marks {
                    let lo = ms.partition_point(|h| h.row < row);
                    for h in ms[lo..].iter().take_while(|h| h.row == row) { span(h.col, h.col + h.len, false, mark_bg); }
                }
                if let Some((a, b)) = bracket {
                    for p in [a, b] { if p[0] == row { span(p[1], p[1] + 1, false, bracket_bg); } }
                }
                if let Some(s) = d.sel {
                    if row >= s[0] && row <= s[2] {
                        let c0 = if row == s[0] { s[1] } else { 0 };
                        let c1 = if row == s[2] { s[3] } else { d.lines[row].chars().count() };
                        span(c0, c1, row != s[2], sel_bg);
                    }
                }
                if let Some(b) = d.box_sel {
                    let (r0, r1, c0, c1) = (b[0].min(b[2]), b[0].max(b[2]), b[1].min(b[3]), b[1].max(b[3]));
                    if row >= r0 && row <= r1 && c1 > c0 {
                        let tail = map.len().saturating_sub(1);
                        let xx = |c: usize| x_of(&painter, &disp, &map, c.min(tail), &font, text_color) + char_w * c.saturating_sub(tail) as f32;
                        painter.rect_filled(Rect::from_min_max(Pos2::new(x0 + xx(c0), y), Pos2::new(x0 + xx(c1), y + row_h)), 0.0, sel_bg);
                    }
                }
                if vw.cur_line && row == d.cursor[0] && !d.grep_result {
                    let yb = rect.min.y + top(row + 1) - 1.0;
                    painter.line_segment([Pos2::new(rect.min.x + gutter, yb), Pos2::new(rect.max.x, yb)], egui::Stroke::new(1.0, tc.border));
                }
                if vw.whitespace {
                    let faint = Color32::from_rgba_unmultiplied(tc.fg_dim.r(), tc.fg_dim.g(), tc.fg_dim.b(), 110);
                    let at = |col: usize| -> (f32, f32) {
                        if wrap {
                            let p = galley.pos_from_cursor(egui::text::CCursor::new(map[col.min(map.len() - 1)])).min;
                            (p.x, p.y)
                        } else { (x_of(&painter, &disp, &map, col.min(map.len() - 1), &font, text_color), 0.0) }
                    };
                    for (c, ch) in crate::view::whitespace_marks(&d.lines[row]) {
                        let (px, py) = at(c);
                        painter.text(Pos2::new(x0 + px, y + py + (row_h - base_h) / 2.0), egui::Align2::LEFT_TOP, ch, font.clone(), faint);
                    }
                    if row + 1 < d.lines.len() {
                        let (px, py) = at(d.lines[row].chars().count());
                        painter.text(Pos2::new(x0 + px, y + py + (row_h - base_h) / 2.0), egui::Align2::LEFT_TOP, '↵', font.clone(), faint);
                    }
                }
                painter.galley(Pos2::new(x0, y), galley, text_color);
                if d.changed_cache.as_ref().map(|c| c.1.get(row).copied().unwrap_or(false)).unwrap_or(false) {
                    painter.rect_filled(Rect::from_min_size(Pos2::new(rect.min.x + gutter - 5.0, y), Vec2::new(3.0, row_h)), 0.0, tc.yellow);
                }
                if let Some(m) = dmarks.and_then(|m| m.get(row)).copied() {
                    use crate::diff::Mark;
                    let (c, h) = match m { Mark::Added => (tc.green, row_h), Mark::Changed => (tc.orange, row_h), Mark::Removed => (tc.red, row_h), Mark::Gap => (tc.red, 3.0), Mark::Same => (tc.fg, 0.0) };
                    if h > 0.0 { painter.rect_filled(Rect::from_min_size(Pos2::new(rect.min.x + gutter - 9.0, y), Vec2::new(3.0, h)), 0.0, c); }
                }
                if d.folds.iter().any(|f| f.0 == row) {
                    painter.text(Pos2::new(rect.min.x + 2.0, y + (row_h - base_h) / 2.0), egui::Align2::LEFT_TOP, "▸", font.clone(), weak);
                }
                if d.bookmarks.iter().any(|b| b.row == row) {
                    painter.circle_filled(Pos2::new(rect.min.x + 7.0, y + row_h / 2.0), 3.5, Color32::from_rgb(80, 170, 255));
                }
                if vw.line_numbers {
                    let num_c = if row == d.cursor[0] { tc.fg } else { weak };
                    painter.text(Pos2::new(rect.min.x + gutter - 8.0, y + (row_h - base_h) / 2.0), egui::Align2::RIGHT_TOP, format!("{}", row + 1), font.clone(), num_c);
                }
            }
            if vw.minimap && !d.lines.is_empty() {
                // 小地图: 贴在可见区右缘, 每个像素行代表一段源码行, 横条长度 = 该段最长行的长度
                let mw = 80.0f32;
                let vh = viewport.height();
                let mrect = Rect::from_min_size(Pos2::new(rect.min.x + viewport.max.x - mw, rect.min.y + viewport.min.y), Vec2::new(mw, vh));
                let mp = ui.painter_at(mrect.intersect(ui.clip_rect()));
                mp.rect_filled(mrect, 0.0, Color32::from_rgba_unmultiplied(tc.bg_edit.r(), tc.bg_edit.g(), tc.bg_edit.b(), 215));
                let buckets = crate::view::minimap_rows(d.lines.len(), (vh / 2.0) as usize);
                let bh = (vh / buckets.len().max(1) as f32).min(3.0); // 行很少时每行 3 像素, 不拉成大块
                let bar = Color32::from_rgba_unmultiplied(tc.fg_dim.r(), tc.fg_dim.g(), tc.fg_dim.b(), 150);
                for (i, (a, b)) in buckets.iter().enumerate() {
                    let n = if b - a > 8 { d.lines[*a].trim_end().chars().count() } else { d.lines[*a..*b].iter().map(|l| l.trim_end().chars().count()).max().unwrap_or(0) };
                    if n > 0 { mp.rect_filled(Rect::from_min_size(Pos2::new(mrect.min.x + 4.0, mrect.min.y + i as f32 * bh), Vec2::new((n as f32 * 0.7).min(mw - 8.0), (bh - 0.5).max(1.0))), 0.0, bar); }
                }
                let total = d.lines.len() as f32;
                let span_h = bh * buckets.len() as f32;
                let (v0, v1) = ((first as f32 / total).min(1.0), (last as f32 / total).min(1.0));
                mp.rect_filled(Rect::from_min_max(Pos2::new(mrect.min.x, mrect.min.y + v0 * span_h), Pos2::new(mrect.max.x, mrect.min.y + (v1 * span_h).max(v0 * span_h + 4.0))), 0.0, Color32::from_rgba_unmultiplied(tc.blue.r(), tc.blue.g(), tc.blue.b(), 40));
            }
            // 光标
            let [cr, cc] = d.cursor;
            if cr < d.lines.len() {
                let (disp, map) = expand_tabs(&d.lines[cr], tab);
                let (cx, cy) = if wrap {
                    let mut job = highlight_job(&d.lines[cr], &disp, &map, None, &theme, &font, text_color, row_h);
                    job.wrap.max_width = wrap_w; job.wrap.break_anywhere = true;
                    let g = painter.layout_job(job);
                    let p = g.pos_from_cursor(egui::text::CCursor::new(map[cc.min(map.len() - 1)])).min;
                    (x0 + p.x, rect.min.y + top(cr) + p.y)
                } else {
                    (x0 + x_of(&painter, &disp, &map, cc.min(map.len() - 1), &font, text_color), rect.min.y + cr as f32 * row_h)
                };
                let blink = ui.input(|i| (i.time * 1000.0) as u64 % 1100 < 700);
                if !preedit.is_empty() {
                    // 组字中的文字: 画在光标处并加下划线 (后面的文字不被挤开, 提交后才真正插入)
                    let g = painter.layout_no_wrap(preedit.clone(), font.clone(), text_color);
                    let w = g.size().x;
                    painter.rect_filled(Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(w, row_h)), 0.0, sel_bg);
                    painter.galley(Pos2::new(cx, cy), g, text_color);
                    painter.line_segment([Pos2::new(cx, cy + row_h - 1.0), Pos2::new(cx + w, cy + row_h - 1.0)], egui::Stroke::new(1.0, text_color));
                } else if blink {
                    painter.rect_filled(Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(2.0, row_h)), 0.0, text_color);
                }
                if reveal {
                    ui.scroll_to_rect(Rect::from_min_size(Pos2::new(cx - 8.0, cy), Vec2::new(40.0, row_h * 2.0)), if center { Some(egui::Align::Center) } else { None });
                }
                ui.ctx().output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput { purpose: egui::IMEPurpose::Normal, rect, cursor_rect: Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(2.0, row_h)), should_interrupt_composition: false });
                });
            }
            for ec in &d.extra {
                if ec[0] >= first && ec[0] < last {
                    let (disp, map) = expand_tabs(&d.lines[ec[0]], tab);
                    let ex = x0 + x_of(&painter, &disp, &map, ec[1].min(map.len() - 1), &font, text_color);
                    painter.rect_filled(Rect::from_min_size(Pos2::new(ex, rect.min.y + top(ec[0])), Vec2::new(2.0, row_h)), 0.0, text_color);
                }
            }
            if resp.is_pointer_button_down_on() || resp.clicked() || resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    clicked = Some((p, ui.input(|i| i.modifiers.shift), resp.dragged() && !resp.drag_started()));
                }
            }
            multi_click = if resp.triple_clicked() { 3 } else if resp.double_clicked() { 2 } else { 0 };
            if resp.clicked() || resp.drag_started() {
                resp.request_focus();
            }
            (rect, x0)
        });
        let (rect, x0) = out.inner;
        if let Some(rr) = ruler_rect {
            let rp = ui.painter_at(rr);
            rp.rect_filled(rr, 0.0, tc.bg_bar);
            rp.line_segment([rr.left_bottom(), rr.right_bottom()], egui::Stroke::new(1.0, tc.border));
            let small = FontId::new((self.font_px * 0.7).max(8.0), crate::fonts::editor_family());
            let origin = rr.min.x + gutter + pad - out.state.clamped_offset().x;
            let cur_col = { let d = &self.docs[ai]; crate::view::display_col(&d.lines[d.cursor[0].min(d.lines.len() - 1)], d.cursor[1], tab) };
            let cols = ((rr.width() + out.state.clamped_offset().x) / char_w) as usize + 1;
            for cell in editcore::ruler::render(&editcore::ruler::RulerConfig { start: 1, end: cols.max(10), major_tick: 10, minor_tick: 5 }) {
                let cx = origin + (cell.col - 1) as f32 * char_w;
                if cx < rr.min.x + gutter - char_w || cx > rr.max.x { continue; }
                match cell.kind {
                    editcore::ruler::RulerMark::Major => { rp.text(Pos2::new(cx + char_w, rr.center().y), egui::Align2::RIGHT_CENTER, format!("{}", cell.col), small.clone(), tc.fg_dim); }
                    editcore::ruler::RulerMark::Minor => { rp.text(Pos2::new(cx + char_w / 2.0, rr.center().y), egui::Align2::CENTER_CENTER, editcore::ruler::marker_for(cell), small.clone(), tc.fg_dim); }
                    editcore::ruler::RulerMark::Plain => {}
                }
            }
            let cx = origin + (cur_col - 1) as f32 * char_w;
            rp.rect_filled(Rect::from_min_size(Pos2::new(cx, rr.min.y), Vec2::new(char_w, rr.height())), 0.0, Color32::from_rgba_unmultiplied(tc.blue.r(), tc.blue.g(), tc.blue.b(), 70));
        }
        if let Some(id) = ed_id { if !self.editor_ids.contains(&id) { self.editor_ids.push(id); } }
        self.max_cols = widest;
        if let Some((p, shift, dragging)) = clicked {
            let rel_y = (p.y - rect.min.y).max(0.0);
            let d = &mut self.docs[ai];
            let row = if use_offs { offs.partition_point(|&o| o <= rel_y).saturating_sub(1) } else { (rel_y / row_h).floor() as usize };
            let row = row.min(d.lines.len() - 1);
            if p.x < rect.min.x + gutter - 6.0 && ctx.input(|i| i.pointer.any_pressed()) {       // 点行号左侧: 折叠 / 展开这一行
                d.toggle_fold(row);
                return;
            }
            let (disp, map) = expand_tabs(&d.lines[row], tab);
            let want = (p.x - x0).max(0.0);
            let col = col_at(ctx, &disp, &map, want, rel_y - top(row), &font, wrap_w);
            let (alt, ctrl) = ctx.input(|i| (i.modifiers.alt, i.modifiers.command || i.modifiers.ctrl));
            if dragging {
                if d.box_sel.is_some() { d.box_drag(row, col); } else { d.drag_to(row, col); }
            } else if ctx.input(|i| i.pointer.any_pressed()) {
                if alt { d.box_start(row, col); }
                else if ctrl { d.toggle_cursor(row, col); }
                else { d.box_sel = None; d.extra.clear(); d.click(row, col, shift); }
            }
            if multi_click == 2 && !dragging {
                if d.grep_result { d.goto(row, 0); self.jump_req = Some((d.id, row)); } else { d.edit("select-word", None); }
            }
            if multi_click == 3 && !dragging { d.select_line(row); }
        }
        let _ = max_cols;
        if self.first_frame {
            self.first_frame = false;
            self.trace.mark("first frame");
        }
    }
}

/// 文件树: 目录用折叠头 (展开才读目录), 文件点击打开
/// 标签分组: 下标 = Doc.group
/// 标签分组名的翻译键 (顺序 = 分组编号)
const GROUP_KEYS: [&str; 6] = ["grp.0", "grp.1", "grp.2", "grp.3", "grp.4", "grp.5"];
const GROUP_COLORS: [Option<Color32>; 6] = [None, Some(Color32::from_rgb(230, 80, 80)), Some(Color32::from_rgb(235, 150, 50)), Some(Color32::from_rgb(80, 190, 110)), Some(Color32::from_rgb(80, 150, 235)), Some(Color32::from_rgb(170, 110, 225))];

/// 状态栏里点出来的命令
enum StatusCmd {
    Goto,
    Charset(Charset),
    Reopen(Charset),
    Bom,
    Eol(Eol),
}

/// 标签栏里点出来的命令 (画完再执行, 免得画的时候改 docs)
enum TabCmd {
    Select(usize),
    Close(usize),
    Many(usize, CloseKind),
    Move(usize, bool),
}

fn tree(ui: &mut egui::Ui, side: &mut Sidebar, dir: &std::path::Path, open: &mut Option<(PathBuf, usize)>, op: &mut Option<FileOp>) {
    for e in side.children_of(dir) {
        if e.is_dir {
            let h = egui::CollapsingHeader::new(&e.name).id_salt(&e.path).show(ui, |ui| tree(ui, side, &e.path, open, op));
            h.header_response.context_menu(|ui| {
                if ui.button("新建文件…").clicked() { *op = Some(FileOp::NewFile(e.path.clone())); ui.close(); }
                if ui.button("新建文件夹…").clicked() { *op = Some(FileOp::NewDir(e.path.clone())); ui.close(); }
                if ui.button("重命名…").clicked() { *op = Some(FileOp::Rename(e.path.clone())); ui.close(); }
                if ui.button("删除…").clicked() { *op = Some(FileOp::Delete(e.path.clone())); ui.close(); }
            });
        } else {
            let r = ui.selectable_label(false, &e.name);
            if r.clicked() { *open = Some((e.path.clone(), 1)); }
            r.context_menu(|ui| {
                if ui.button("重命名…").clicked() { *op = Some(FileOp::Rename(e.path.clone())); ui.close(); }
                if ui.button("删除…").clicked() { *op = Some(FileOp::Delete(e.path.clone())); ui.close(); }
            });
        }
    }
}

fn icon_of(kind: &str) -> &'static str {
    match kind { "function" | "method" => "ƒ", "class" | "struct" => "C", "enum" => "E", "interface" | "trait" => "I", "section" => "§", _ => "•" }
}

/// 一行的带色排版: 词法由 editcore 的 `TypeConfig::highlight_line` 给 (按字符列), 这里换成显示列 (Tab 已展开) 再上色
fn highlight_job(line: &str, disp: &str, map: &[usize], cfg: Option<&TypeConfig>, theme: &Theme, font: &FontId, default: Color32, line_h: f32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat { font_id: font.clone(), color: c, line_height: Some(line_h), ..Default::default() };
    let Some(cfg) = cfg else {
        job.append(disp, 0.0, fmt(default));
        return job;
    };
    // 显示字符下标 -> 字节偏移
    let mut byte_of: Vec<usize> = disp.char_indices().map(|(b, _)| b).collect();
    byte_of.push(disp.len());
    let dlen = byte_of.len() - 1;
    let mut at = 0usize; // 已经输出到的显示字符下标
    for (s, e, kind) in cfg.highlight_line(line) {
        let (s, e) = (s.max(0) as usize, e.max(0) as usize);
        let (ds, de) = (map.get(s).copied().unwrap_or(dlen).min(dlen), map.get(e).copied().unwrap_or(dlen).min(dlen));
        if ds < at || de <= ds {
            continue;
        }
        if ds > at {
            job.append(&disp[byte_of[at]..byte_of[ds]], 0.0, fmt(default));
        }
        let c = theme.get(kind).fg;
        job.append(&disp[byte_of[ds]..byte_of[de]], 0.0, fmt(Color32::from_rgb(c.0, c.1, c.2)));
        at = de;
    }
    if at < dlen {
        job.append(&disp[byte_of[at]..], 0.0, fmt(default));
    }
    job
}

/// 第 col 个字符前缘的 x (按真实排版量, 全角 / 变宽字体都准)
fn x_of(painter: &egui::Painter, disp: &str, map: &[usize], col: usize, font: &FontId, color: Color32) -> f32 {
    let di = map.get(col).copied().unwrap_or_else(|| *map.last().unwrap_or(&0));
    if di == 0 {
        return 0.0;
    }
    let prefix: String = disp.chars().take(di).collect();
    painter.layout_no_wrap(prefix, font.clone(), color).size().x
}

/// 点在 x 处 -> 字符列 (取离得最近的字符边界)
fn col_at(ctx: &egui::Context, disp: &str, map: &[usize], x: f32, y: f32, font: &FontId, wrap_w: f32) -> usize {
    let mut job = egui::text::LayoutJob::simple(disp.to_string(), font.clone(), Color32::WHITE, wrap_w);
    job.wrap.break_anywhere = true;
    let galley = ctx.fonts_mut(|f| f.layout_job(job));
    let idx: usize = galley.cursor_from_pos(Vec2::new(x, y.max(0.0))).index.into(); // 显示字符下标
    // 显示下标 -> 字符列: 取 map[col] <= idx 的最大 col
    let mut col = 0;
    for (c, &m) in map.iter().enumerate() {
        if m <= idx { col = c; } else { break; }
    }
    col.min(map.len().saturating_sub(1))
}

/// 缩放重置 (Ctrl+0) 回到的字号, 也是设置的默认字号
const DEFAULT_FONT_PX: f32 = 15.0;

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        self.tc = match &self.theme_vars { Some(v) if self.theme_visuals.is_some() => crate::view::ThemeColors::from_vars(v), _ => crate::view::ThemeColors::builtin(self.dark) };
        let mut vis = match &self.theme_visuals { Some(v) => v.clone(), None => if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() } };
        if let Some(c) = self.cfg.colors.bg { vis.extreme_bg_color = Color32::from_rgb(c[0], c[1], c[2]); } // 编辑区背景覆盖
        ctx.set_visuals(vis);
        if self.cfg.font_family != self.applied_font {
            crate::fonts::install(ctx, &self.cfg.font_family);
            self.applied_font = self.cfg.font_family.clone();
        }
        self.sync_doc_prefs();
        self.track_geometry(ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            for i in 0..self.docs.len() { self.remember_pos(i); }
            self.cfg.view.geom = self.geom_now.map(|g| crate::settings::WinGeom { maximized: self.maximized_now, ..g });
        }
        // 关窗口: 有未保存修改先问
        if ctx.input(|i| i.viewport().close_requested()) && !self.confirm_exit && self.cfg.confirm_close && self.docs.iter().any(|d| d.dirty) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            let mut i = 0;
            let mut all = true;
            while i < self.docs.len() {
                if self.docs[i].dirty {
                    self.active = i;
                    if !self.close_tab(i) { all = false; break; }
                } else { i += 1; }
            }
            if all { self.confirm_exit = true; ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
        }
        // 拖文件进窗口: 打开
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped { self.open_path(&p); }

        let forwarded: Vec<Vec<String>> = self.inbox.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        for paths in forwarded {
            for a in &paths { self.open_spec(a); }
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused && self.cfg.check_external { self.check_external(); }
        self.was_focused = focused;
        if self.prefs.is_some() { self.prefs_window(ctx); }
        if self.goto_open { self.goto_window(ctx); }
        if self.bm_open { self.bookmark_window(ctx); }
        self.diff_refresh();
        self.diff_windows(ctx);
        if self.props.is_some() { self.props_window(ctx); }
        self.cmd_windows(ctx);
        if self.split_on {
            let last = self.docs.len() - 1;
            self.left_doc = self.left_doc.min(last);
            self.right_doc = self.right_doc.min(last);
            self.active = if self.focus_right { self.right_doc } else { self.left_doc };
        }
        self.shortcuts(ctx);
        egui::Panel::top("menu").show(root, |ui| self.menu(ui, ctx));
        // 标签栏: 只有一个标签时整条收起; 标签多了横向滚动 (滚轮 / Shift+滚轮 / 拖动), 活动标签变了就滚进可见范围
        let mut tab_cmd: Option<TabCmd> = None;
        if self.docs.len() > 1 {
            egui::Panel::top("tabs").show(root, |ui| {
                let scroll_active = self.last_active != self.active;
                let wheel = ui.input(|i| i.smooth_scroll_delta);
                if ui.rect_contains_pointer(ui.max_rect()) && wheel.y != 0.0 && wheel.x == 0.0 { ui.scroll_with_delta(Vec2::new(wheel.y, 0.0)); }
                egui::ScrollArea::horizontal().auto_shrink([false, true]).scroll_source(egui::scroll_area::ScrollSource::ALL).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for i in 0..self.docs.len() {
                            let label = format!("{}{}", if self.docs[i].dirty { "● " } else { "" }, self.docs[i].name);
                            let gcol = GROUP_COLORS.get(self.docs[i].group as usize).copied().flatten();
                            let text = match gcol { Some(c) => egui::RichText::new(format!("● {label}")).color(c), None => egui::RichText::new(label) };
                            let tab_resp = ui.selectable_label(i == self.active, text);
                            if tab_resp.clicked() { tab_cmd = Some(TabCmd::Select(i)); }
                            if tab_resp.middle_clicked() { tab_cmd = Some(TabCmd::Close(i)); }
                            if scroll_active && i == self.active { tab_resp.scroll_to_me(None); }
                            tab_resp.context_menu(|ui| {
                                if ui.button(tr("c.close")).clicked() { tab_cmd = Some(TabCmd::Close(i)); ui.close(); }
                                if ui.button(tr("c.close-others-s")).clicked() { tab_cmd = Some(TabCmd::Many(i, CloseKind::Others)); ui.close(); }
                                if ui.button(tr("c.close-left-s")).clicked() { tab_cmd = Some(TabCmd::Many(i, CloseKind::Left)); ui.close(); }
                                if ui.button(tr("c.close-right-s")).clicked() { tab_cmd = Some(TabCmd::Many(i, CloseKind::Right)); ui.close(); }
                                ui.separator();
                                if ui.button(lk("c.tab-move-left", "")).clicked() { tab_cmd = Some(TabCmd::Move(i, true)); ui.close(); }
                                if ui.button(lk("c.tab-move-right", "")).clicked() { tab_cmd = Some(TabCmd::Move(i, false)); ui.close(); }
                                if ui.button(tr("c.copy-path")).clicked() { if let Some(p) = &self.docs[i].path { ctx.copy_text(p.display().to_string()); } ui.close(); }
                                ui.separator();
                                for (g, name) in GROUP_KEYS.iter().enumerate() {
                                    if ui.selectable_label(self.docs[i].group as usize == g, tr(name)).clicked() { self.docs[i].group = g as u8; ui.close(); }
                                }
                            });
                            if ui.small_button("×").clicked() { tab_cmd = Some(TabCmd::Close(i)); }
                            ui.add_space(6.0);
                        }
                    });
                });
            });
        }
        self.last_active = self.active;
        match tab_cmd {
            Some(TabCmd::Select(i)) => self.select_tab(i),
            Some(TabCmd::Close(i)) => { self.close_tab(i); }
            Some(TabCmd::Many(i, k)) => { self.select_tab(i); self.close_many(k); }
            Some(TabCmd::Move(i, left)) => { self.select_tab(i); self.move_active_tab(left); }
            None => {}
        }
        if self.split_on {
            if self.focus_right { self.right_doc = self.active; } else { self.left_doc = self.active; }
        }
        if self.side.open {
            let r = egui::Panel::left("side").resizable(true).default_size(self.cfg.view.side_w).show(root, |ui| self.side_ui(ui));
            // 拖动期间不记, 松手后取整记一次 (免得每帧都落盘)
            if !ctx.input(|i| i.pointer.any_down()) { self.cfg.view.side_w = r.response.rect.width().round().clamp(120.0, 900.0); }
        }
        self.side.grep.poll();
        self.sync_grep_doc();
        if self.side.grep.running() { ctx.request_repaint_after(std::time::Duration::from_millis(80)); }
        // 结果文档里 Enter / 双击命中行: 打开那个文件并把命中处放到窗口中央
        if let Some((id, row)) = self.jump_req.take() {
            let line = self.docs.iter().find(|d| d.id == id).and_then(|d| d.lines.get(row).cloned()).unwrap_or_default();
            match grep::parse_result_line(&line) {
                Some((p, l, c)) => { self.center_once = true; self.open_at(&p, l, c.saturating_sub(1)); }
                None => self.cur().tip = "这一行不是命中行".into(),
            }
        }
        // 脚本插件: 收结果 (补丁 / 提示 / 结果窗口); 点图里的节点跳到对应行
        self.plug.poll(&mut self.docs, self.active, ctx);
        if self.plug.win.is_some() {
            if let Some((id, row)) = self.plug.window(ctx, self.theme_visuals.as_ref().and_then(|v| v.override_text_color).or(Some(if self.dark { egui::Color32::from_gray(220) } else { egui::Color32::from_gray(30) }))) {
                if let Some(i) = self.docs.iter().position(|d| d.id == id) {
                    self.active = i;
                    self.docs[i].goto(row, 0);
                    self.center_once = true;
                }
            }
        }
        let mut st_cmd: Option<StatusCmd> = None;
        if self.cfg.view.status_bar { egui::Panel::bottom("status").frame(egui::Frame::new().fill(self.tc.bg_status).inner_margin(egui::Margin::symmetric(8, 3)).stroke(egui::Stroke::new(1.0, self.tc.border))).show(root, |ui| {
            let d = &self.docs[self.active.min(self.docs.len() - 1)];
            let lang = self.registry.detect(&d.name).map(|c| c.name.clone());
            let dcol = crate::view::display_col(&d.lines[d.cursor[0].min(d.lines.len() - 1)], d.cursor[1], self.tab);
            ui.horizontal(|ui| {
                // 位置: 点一下 = 转到行
                if ui.add(egui::Label::new(format!("Ln {}, Col {}", d.cursor[0] + 1, dcol)).sense(Sense::click())).on_hover_text(format!("{}
{}", tr("st.goto-hint"), trf("st.char-col", &[&(d.cursor[1] + 1).to_string()]))).clicked() { st_cmd = Some(StatusCmd::Goto); }
                ui.separator();
                // 编码 / 换行符 / BOM: 点开菜单改 (改保存时的格式, 或按别的编码重新打开)
                ui.menu_button(d.charset.label(), |ui| {
                    ui.label(tr("st.charset-save"));
                    for cs in Charset::ALL {
                        if ui.selectable_label(cs == d.charset, cs.label()).clicked() { st_cmd = Some(StatusCmd::Charset(cs)); ui.close(); }
                    }
                    ui.separator();
                    ui.menu_button(tr("st.reopen-as"), |ui| {
                        for cs in Charset::ALL { if ui.button(cs.label()).clicked() { st_cmd = Some(StatusCmd::Reopen(cs)); ui.close(); } }
                    });
                    if ui.button(if d.bom == editcore::document_io::Bom::None { tr("st.add-bom") } else { tr("st.rm-bom") }).clicked() { st_cmd = Some(StatusCmd::Bom); ui.close(); }
                });
                ui.menu_button(d.eol.label(), |ui| {
                    for e in [Eol::Crlf, Eol::Lf, Eol::Cr] {
                        if ui.selectable_label(e == d.eol, e.label()).clicked() { st_cmd = Some(StatusCmd::Eol(e)); ui.close(); }
                    }
                });
                if d.mixed_eol { ui.colored_label(Color32::from_rgb(230, 160, 40), tr("st.mixed-eol")).on_hover_text(tr("st.mixed-eol-tip")); }
                ui.label(trf("st.lines", &[&d.lines.len().to_string()]));
                if let Some(sel) = d.sel {
                    let (n, l) = crate::view::sel_stats(&d.lines, sel);
                    ui.label(trf("st.selected", &[&n.to_string(), &l.to_string()]));
                }
                ui.label(lang.unwrap_or_else(|| tr("st.plain").to_string()));
                ui.label(if d.overwrite { "OVR" } else { "INS" });
                if d.readonly { ui.label(tr("st.readonly")); }
                if d.rec.is_some() { ui.colored_label(Color32::from_rgb(230, 70, 70), "● REC"); }
                if d.box_sel.is_some() { ui.label(tr("st.box")); }
                if !d.extra.is_empty() { ui.label(trf("st.cursors", &[&(d.extra.len() + 1).to_string()])); }
                ui.separator();
                ui.label(&d.tip);
            });
        }); }
        match st_cmd {
            Some(StatusCmd::Goto) => { self.goto_open = true; self.goto_text.clear(); }
            Some(StatusCmd::Charset(cs)) => self.cur().set_charset(cs),
            Some(StatusCmd::Reopen(cs)) => self.reopen_as(cs),
            Some(StatusCmd::Bom) => self.cur().toggle_bom(),
            Some(StatusCmd::Eol(e)) => self.cur().set_eol(e),
            None => {}
        }
        if self.find.open {
            egui::Panel::bottom("find").show(root, |ui| {
                let i = self.active.min(self.docs.len() - 1);
                self.find.bar(ui, &mut self.docs[i]);
            });
        }
        // 查找栏 / 侧栏输入框 / 对话框正在收键盘时, 编辑区不抢
        let find_focus = self.cmdui.capturing() || ctx.memory(|m| m.focused()).map(|f| !self.editor_ids.contains(&f)).unwrap_or(false);
        if self.term.open {
            let r = egui::Panel::bottom("term").resizable(true).default_size(self.cfg.view.term_h).show(root, |ui| self.term_ui(ui));
            if !ctx.input(|i| i.pointer.any_down()) { self.cfg.view.term_h = r.response.rect.height().round().clamp(60.0, 900.0); }
        }
        self.term.poll();
        if let Some((id, out)) = self.term.replace_out.take() {
            if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                d.edit("paste", Some(out.trim_end_matches(['\r', '\n']).to_string()));
            }
        }
        if let Some(ms) = self.term.repaint_ms() { ctx.request_repaint_after(std::time::Duration::from_millis(ms)); }
        let is_md = {
            let n = self.docs[self.active.min(self.docs.len() - 1)].name.to_lowercase();
            n.ends_with(".md") || n.ends_with(".markdown")
        };
        if self.md_preview && is_md {
            egui::Panel::right("mdprev").resizable(true).default_size(420.0).show(root, |ui| self.md_ui(ui));
        }
        if self.split_on {
            egui::Panel::right("split").resizable(true).default_size(root.available_width() * 0.5).frame(egui::Frame::NONE.fill(ctx.global_style().visuals.extreme_bg_color)).show(root, |ui| self.editor_pane(ui, ctx, find_focus, true));
        }
        egui::CentralPanel::default_margins().frame(egui::Frame::NONE.fill(ctx.global_style().visuals.extreme_bg_color)).show(root, |ui| self.editor_pane(ui, ctx, find_focus, false));
        self.persist_if_changed();
        ctx.request_repaint_after(std::time::Duration::from_millis(550)); // 光标闪烁
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::TypePref;

    /// 不落盘的 App; 语言是全局量, 拿锁串行化
    fn app(st: Settings) -> (App, std::sync::MutexGuard<'static, ()>) {
        let g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        (App::build(vec![], None, Trace::new(), st, false), g)
    }

    #[test]
    fn settings_survive_a_trip_through_the_running_app() {
        let mut st = Settings::default();
        st.dark = false; st.font_px = 18.0; st.tab_width = 2; st.lang = "en".into(); st.auto_indent = false; st.save_backup = true;
        st.font_family = "consola.ttf".into(); st.line_height = 1.5; st.colors.keyword = Some([1, 2, 3]);
        st.view.side_w = 333.0; st.view.term_h = 99.0;
        let (a, _g) = app(st.clone());
        let back = a.current_settings();
        assert_eq!((back.dark, back.font_px, back.tab_width, back.lang.as_str(), back.auto_indent, back.save_backup), (false, 18.0, 2, "en", false, true));
        assert_eq!((back.font_family.as_str(), back.line_height, back.colors.keyword), ("consola.ttf", 1.5, Some([1, 2, 3])));
        assert_eq!((back.view.side_w, back.view.term_h), (333.0, 99.0));
        assert_eq!(crate::i18n::cur(), crate::i18n::Lang::En, "语言设置生效到界面");
        crate::i18n::set(crate::i18n::Lang::Zh);
    }

    #[test]
    fn new_documents_use_the_default_encoding_and_eol() {
        let mut st = Settings::default();
        st.default_encoding = "GBK".into(); st.default_eol = "LF".into();
        let (mut a, _g) = app(st);
        a.new_doc();
        let d = a.cur();
        assert_eq!((d.charset, d.eol), (Charset::Gbk, Eol::Lf));
        let mut st = Settings::default();
        st.default_encoding = "Unicode".into();
        let (mut b, _g2) = (App::build(vec![], None, Trace::new(), st, false), ());
        b.new_doc();
        assert_eq!(b.cur().bom, editcore::document_io::Bom::Utf16Le, "UTF-16 的新文档带对应的 BOM");
        assert_eq!(App::build(vec![], None, Trace::new(), Settings { default_encoding: "bogus".into(), ..Settings::default() }, false).docs[0].charset, Charset::Utf8, "认不出的编码 = UTF-8");
    }

    #[test]
    fn doc_prefs_follow_settings_and_extension_overrides() {
        let mut st = Settings::default();
        st.auto_indent = false; st.smart_indent = true; st.insert_spaces = false;
        st.file_types = vec![TypePref { ext: "py".into(), insert_spaces: Some(true), tab_width: Some(2), wrap: Some(true) }];
        let (mut a, _g) = app(st);
        a.docs[0].name = "x.py".into();
        a.new_doc();
        a.docs[1].name = "y.txt".into();
        a.sync_doc_prefs();
        assert_eq!((a.docs[0].auto_indent, a.docs[0].smart_indent, a.docs[0].insert_spaces), (false, true, true), ".py 的 Tab 插入空格被覆盖为开");
        assert_eq!(a.docs[1].insert_spaces, false);
        assert_eq!(a.eff("x.py"), (2, true, true));
        assert_eq!(a.eff("y.txt"), (4, false, false));
    }

    #[test]
    fn prefs_dialog_applies_on_ok_and_discards_on_cancel() {
        let (mut a, _g) = app(Settings::default());
        a.recent = vec!["a.txt".into(), "b.txt".into(), "c.txt".into()];
        a.open_prefs();
        {
            let p = a.prefs.as_mut().unwrap();
            p.draft.tab_width = 8; p.draft.mru_files = 2; p.draft.lang = "ja".into(); p.draft.insert_spaces = true;
        }
        assert_eq!((a.tab, a.cfg.insert_spaces), (4, false), "草稿没提交前不影响运行状态");
        a.recent.push("d.txt".into()); // 对话框开着时又打开了文件: 提交不能把它冲掉
        a.prefs_commit();
        assert_eq!((a.tab, a.cfg.insert_spaces, a.cfg.lang.as_str()), (8, true, "ja"));
        assert_eq!(crate::i18n::cur(), crate::i18n::Lang::Ja);
        assert_eq!(a.recent, vec!["a.txt", "b.txt"], "最近文件个数按新设置截断, 内容用现在的而不是草稿里的旧快照");
        // 取消: 草稿丢弃
        a.prefs = None;
        a.open_prefs();
        a.prefs.as_mut().unwrap().draft.tab_width = 3;
        a.prefs = None;
        assert_eq!(a.tab, 8);
        crate::i18n::set(crate::i18n::Lang::Zh);
    }

    #[test]
    fn reset_keeps_recent_files_but_restores_preferences() {
        let (mut a, _g) = app(Settings { tab_width: 9, font_family: "x.ttf".into(), ..Settings::default() });
        a.recent = vec!["keep.txt".into()];
        let mut next = a.current_settings();
        next.reset_prefs();
        a.apply_settings(&next);
        assert_eq!((a.tab, a.cfg.font_family.as_str(), a.recent.clone()), (4, "", vec!["keep.txt".to_string()]));
    }

    #[test]
    fn view_toggles_persist_and_minimal_layout_hides_panels() {
        let mut st = Settings::default();
        st.view.ruler = true; st.view.whitespace = true; st.view.line_numbers = false;
        let (mut a, _g) = app(st);
        let back = a.current_settings();
        assert_eq!((back.view.ruler, back.view.whitespace, back.view.line_numbers, back.view.status_bar), (true, true, false, true));
        a.side.open = true; a.term.open = true; a.md_preview = true;
        a.set_minimal();
        assert_eq!((a.side.open, a.term.open, a.md_preview, a.cfg.view.ruler), (false, false, false, false));
        assert!(a.cfg.view.status_bar, "极简布局仍留状态栏");
    }

    fn set_lines(d: &mut Doc, text: &str) {
        d.lines = text.split('\n').map(str::to_string).collect();
        d.rev += 1;
    }

    #[test]
    fn comparing_two_tabs_marks_lines_tracks_edits_and_ends_when_a_tab_closes() {
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.new_doc();
        set_lines(&mut a.docs[0], "a\nb\nc");
        set_lines(&mut a.docs[1], "a\nX\nc\nd");
        a.active = 0;
        a.start_diff(1);
        let v = a.diff.as_ref().expect("比较已开始");
        assert_eq!(v.hunks.len(), 2, "第 2 行改了, 末尾多一行");
        let (l, r) = (v.left, v.right);
        assert!(a.diff.as_ref().unwrap().marks_of(l).is_some() && a.diff.as_ref().unwrap().marks_of(r).is_some());
        // 改右边: 版本号变了, 重算
        set_lines(&mut a.docs[1], "a\nb\nc");
        a.diff_refresh();
        assert!(a.diff.as_ref().unwrap().hunks.is_empty());
        // 下一处差异在没有差异时给提示而不是动光标
        a.diff_step(true);
        assert_eq!(a.docs[0].cursor, [0, 0]);
        // 关掉一份: 比较结束
        a.docs.truncate(1);
        a.diff_refresh();
        assert!(a.diff.is_none());
    }

    #[test]
    fn diff_step_walks_hunks_and_wraps() {
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.new_doc();
        set_lines(&mut a.docs[0], "1\n2\n3\n4\n5\n6");
        set_lines(&mut a.docs[1], "1\nX\n3\n4\nY\n6");
        a.active = 1; // 右侧当前
        a.docs[1].cursor = [0, 0];
        a.diff = Some(crate::diff::DiffView::new(a.docs[0].id, a.docs[1].id, &a.docs[0].lines, &a.docs[1].lines, (a.docs[0].rev, a.docs[1].rev)));
        a.diff_step(true);
        assert_eq!(a.docs[1].cursor[0], 1);
        a.diff_step(true);
        assert_eq!(a.docs[1].cursor[0], 4);
        a.diff_step(true);
        assert_eq!(a.docs[1].cursor[0], 1, "到头回绕");
        a.diff_step(false);
        assert_eq!(a.docs[1].cursor[0], 4, "往回也回绕");
    }

    #[test]
    fn renaming_a_file_or_folder_retargets_open_tabs() {
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.docs[0].path = Some(PathBuf::from("/p/old/a.txt"));
        a.docs[0].name = "a.txt".into();
        a.docs_follow_rename(std::path::Path::new("/p/old"), std::path::Path::new("/p/new"));
        assert_eq!(a.docs[0].path.as_deref(), Some(std::path::Path::new("/p/new/a.txt")));
        a.docs_follow_rename(std::path::Path::new("/p/new/a.txt"), std::path::Path::new("/p/new/b.md"));
        assert_eq!((a.docs[0].name.as_str(), a.docs[0].path.as_deref()), ("b.md", Some(std::path::Path::new("/p/new/b.md"))));
    }

    #[test]
    fn opening_a_text_doc_makes_a_read_only_tab() {
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.open_text_doc("Diff: x.diff", "@@ -1 +1 @@\n-a\n+b\n");
        let d = a.cur();
        assert!(d.readonly && !d.dirty && d.name == "Diff: x.diff");
        assert_eq!(d.lines, vec!["@@ -1 +1 @@", "-a", "+b"]);
    }

    #[test]
    fn choosing_a_theme_plugin_switches_colors_and_syntax_theme() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide-fast-app-theme-{}", std::process::id()));
        std::env::set_var("IDE_PLUGIN_DIR", &dir);
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.set_theme_plugin("theme-light");
        assert!(a.theme_vars.is_some());
        let tc = crate::view::ThemeColors::from_vars(a.theme_vars.as_ref().unwrap());
        assert!(!tc.dark);
        a.set_theme_plugin("");
        assert!(a.theme_vars.is_none() && a.theme_id.is_empty());
        std::env::remove_var("IDE_PLUGIN_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn geometry_is_remembered_only_in_the_normal_state() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput {
            viewport_id: egui::ViewportId::ROOT,
            viewports: std::iter::once((egui::ViewportId::ROOT, egui::ViewportInfo {
                outer_rect: Some(Rect::from_min_size(Pos2::new(40.0, 50.0), Vec2::new(900.0, 700.0))),
                inner_rect: Some(Rect::from_min_size(Pos2::new(48.0, 80.0), Vec2::new(884.0, 660.0))),
                ..Default::default()
            })).collect(),
            ..Default::default()
        }, |ui| a.track_geometry(ui.ctx()));
        out.textures_delta.clear();
        let g = a.geom_now.expect("正常状态记下了位置");
        assert_eq!((g.x, g.y, g.w, g.h), (40.0, 50.0, 884.0, 660.0));
        assert!(!a.maximized_now);
    }
}
