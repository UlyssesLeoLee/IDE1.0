//! 应用外壳: 菜单 / 标签 / 编辑区 / 查找条 / 状态栏。编辑语义都在 `doc.rs` (-> editcore), 这里只管画和收输入。

use crate::doc::{expand_tabs, Doc};
use crate::cmds::{self, Action};
use crate::find::Find;
use crate::grep::{self, Ev as GEv};
use crate::i18n::{lk, tr, trf, trz, Lang};
use crate::plugins;
use crate::prefs::{prefs_page, Prefs, PrefsTab};
use crate::settings::Settings;
use crate::sidebar::{Sidebar, Tab};
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
            dark: true, font_px: 15.0, tab: 4, highlight: true, recent: vec![], saved: Settings::default(), registry: TypeRegistry::default_registry(), side: Sidebar::new(), term: Terminal::new(), goto_open: false, was_focused: true, goto_text: String::new(), theme_id: String::new(), theme_visuals: None, split_on: false, left_doc: 0, right_doc: 0, focus_right: false, pane_keys: true, term_stdin: false, term_replace: false, md_preview: false, wrap: false, goal_x: None, inbox, prefs: None, persist: true, cfg: Settings::default(), applied_font: String::new(), geom_now: None, maximized_now: false, wrap_cache: None, max_cols: 80, trace, first_frame: true, confirm_exit: false, preedit: String::new(), center_once: false, grep_doc: None, jump_req: None, editor_ids: vec![], jump: JumpHist::default(), props: None, bm_open: false, fullscreen: false, topmost: false, recent_dirs: vec![], positions: vec![], bracket_cache: None, last_active: usize::MAX,
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
                if ui.button(tr("c.reset-settings")).clicked() { reset = true; }
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
        self.side.open = true;
    }

    /// 打开文件并跳到某行 (Grep 结果 / 命令行 `文件:行`)
    fn open_at(&mut self, p: &std::path::Path, line: usize, col: usize) {
        if p.is_file() { self.open_path(p); }
        // 路径不是磁盘文件 (只搜当前文件、还没存盘的标签): 就在当前标签里跳
        if p.is_file() && !self.docs.iter().any(|d| d.path.as_deref() == Some(p)) { return; }
        self.cur().goto(line.saturating_sub(1), col);
    }

    /// 侧栏内容: 文件树 / 大纲 / Grep
    fn side_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (t, name) in [(Tab::Files, "文件"), (Tab::Outline, "大纲"), (Tab::Search, "搜索")] {
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
                        ui.horizontal(|ui| {
                            ui.strong(root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string()));
                            if ui.small_button("刷新").clicked() { self.side.refresh(); }
                        });
                        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| tree(ui, &mut self.side, &root, &mut open));
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
                if let Some(l) = jump { self.cur().goto(l, 0); }
            }
            Tab::Search => {
                let root = self.grep_root().unwrap_or_default();
                let has_cur = self.cur().path.is_some();
                let shown_root = self.side.grep.last.as_ref().map(|l| l.1.clone()).unwrap_or(root);
                let evs = self.side.grep.panel(ui, &shown_root, has_cur);
                for ev in evs { self.grep_event(ev); }
            }
        }
        if let Some((p, l)) = open { self.open_at(&p, l, 0); }
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
                if ui.button(tr("c.open-folder")).clicked() { ui.close(); self.open_folder_dialog(); }
                ui.menu_button(tr("sub.recent"), |ui| {
                    if self.recent.is_empty() { ui.label(tr("ui.none")); }
                    let mut pick: Option<String> = None;
                    for r in &self.recent {
                        if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
                    }
                    if let Some(r) = pick { self.open_path(std::path::Path::new(&r)); }
                    ui.separator();
                    if ui.button(tr("c.recent-clear")).clicked() { self.clear_recent(); ui.close(); }
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
                if ui.button(tr("c.save-close")).clicked() { ui.close(); self.save_close(); }
                ui.separator();
                if ui.button(lk("c.close-tab", "Ctrl+W")).clicked() { ui.close(); let a = self.active; self.close_tab(a); }
                if ui.button(lk("c.close-all", "Ctrl+Shift+W")).clicked() { ui.close(); self.close_many(CloseKind::All); }
                if ui.button(lk("c.close-open", "Ctrl+Shift+F4")).clicked() { ui.close(); self.close_open(); }
                ui.separator();
                if ui.button(tr("c.reload")).clicked() { ui.close(); self.reload_current(); }
                ui.menu_button(tr("sub.reopen"), |ui| {
                    for cs in Charset::ALL {
                        if ui.button(cs.label()).clicked() { ui.close(); self.reopen_as(cs); }
                    }
                });
                ui.separator();
                if ui.button(lk("c.print", "Ctrl+P")).clicked() { ui.close(); self.print_cur(); }
                if ui.button(tr("c.browse")).clicked() { ui.close(); self.browse_current(); }
                if ui.button(lk("c.file-properties", "Alt+Enter")).clicked() { ui.close(); self.props = Some(self.cur().properties()); }
                if ui.button(tr("c.new-window")).clicked() { ui.close(); self.new_window(); }
                ui.separator();
                if ui.button(tr("c.exit")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
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
                        if let Some(t) = &r.theme {
                            if ui.radio(self.theme_id == r.id, format!("{}{}", tr("ui.theme-prefix"), t.label)).clicked() { self.set_theme_plugin(&r.id); ui.close(); }
                        } else {
                            ui.label(format!("{} {}{}", r.name, r.version, if r.script_only { tr("ui.script-only") } else { "" }));
                        }
                        if ui.small_button(tr("c.delete")).clicked() {
                            let _ = plugins::remove(&r.id);
                            if self.theme_id == r.id { self.set_theme_plugin(""); }
                            ui.close();
                        }
                    });
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
                if ui.button(tr("c.common-config")).clicked() { self.open_prefs(); ui.close(); }
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
                if ui.button(tr("c.export-config")).clicked() { ui.close(); self.export_settings(); }
                if ui.button(tr("c.import-config")).clicked() { ui.close(); self.import_settings(); }
                if ui.button(tr("c.import-sakura")).clicked() { ui.close(); self.import_sakura(); }
                if ui.button(tr("c.reset-settings")).clicked() { ui.close(); self.reset_settings(); }
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
                if ui.button(tr("c.hist-mark")).clicked() { self.jump_mark(); self.cur().tip = tr("dlg.hist-marked").into(); ui.close(); }
            });
            ui.menu_button(tr("menu.window"), |ui| {
                if ui.button(lk("c.split", "Ctrl+\\")).clicked() { self.toggle_split(); ui.close(); }
                ui.separator();
                if ui.button(lk("c.next-tab", "Ctrl+Tab")).clicked() { self.next_tab(true); ui.close(); }
                if ui.button(lk("c.prev-tab", "Ctrl+Shift+Tab")).clicked() { self.next_tab(false); ui.close(); }
                ui.menu_button(tr("sub.tabops"), |ui| {
                    if ui.button(tr("c.tab-move-left")).clicked() { self.move_active_tab(true); ui.close(); }
                    if ui.button(tr("c.tab-move-right")).clicked() { self.move_active_tab(false); ui.close(); }
                    if ui.button(tr("c.close-others")).clicked() { self.close_many(CloseKind::Others); ui.close(); }
                    if ui.button(tr("c.close-left")).clicked() { self.close_many(CloseKind::Left); ui.close(); }
                    if ui.button(tr("c.close-right")).clicked() { self.close_many(CloseKind::Right); ui.close(); }
                });
                ui.menu_button(tr("sub.winlist"), |ui| {
                    let mut pick = None;
                    for (i, d) in self.docs.iter().enumerate() {
                        if ui.selectable_label(i == self.active, format!("{}{}", if d.dirty { "● " } else { "" }, d.name)).clicked() { pick = Some(i); ui.close(); }
                    }
                    if let Some(i) = pick { self.select_tab(i); }
                });
                ui.separator();
                if ui.button(tr("c.minimize-window")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); ui.close(); }
                if ui.button(tr("c.maximize")).clicked() { let m = ctx.input(|i| i.viewport().maximized.unwrap_or(false)); ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!m)); ui.close(); }
                if ui.checkbox(&mut self.fullscreen.clone(), lk("c.fullscreen", "F11")).clicked() { self.toggle_fullscreen(ctx); ui.close(); }
                if ui.checkbox(&mut self.topmost.clone(), tr("c.topmost")).clicked() { self.toggle_topmost(ctx); ui.close(); }
            });
            ui.menu_button(tr("menu.macro"), |ui| {
                let rec = self.cur().rec.is_some();
                if ui.button(lk(if rec { "c.macro-stop" } else { "c.macro-start" }, "Ctrl+Shift+M")).clicked() { self.cur().macro_toggle(); ui.close(); }
                if ui.button(lk("c.macro-play", "Ctrl+Shift+L")).clicked() { self.cur().macro_play(); ui.close(); }
                ui.separator();
                if ui.button(tr("c.macro-run-doc")).clicked() { ui.close(); self.run_mac_doc(); }
                if ui.button(tr("c.macro-run-file")).clicked() { ui.close(); self.run_mac_file(); }
            });
            ui.menu_button(tr("menu.view"), |ui| {
                if ui.checkbox(&mut self.split_on.clone(), lk("c.split", "Ctrl+\\")).clicked() { self.toggle_split(); ui.close(); }
                ui.checkbox(&mut self.term.open, lk("c.terminal", "Ctrl+`"));
                ui.checkbox(&mut self.md_preview, lk("c.md-preview", "Ctrl+Shift+V"));
                if ui.checkbox(&mut self.dark, tr("c.dark")).clicked() { ui.close(); }
                ui.checkbox(&mut self.highlight, tr("c.highlight"));
                ui.checkbox(&mut self.wrap, lk("c.wrap", "Alt+Z"));
                ui.checkbox(&mut self.side.open, lk("c.sidebar", "Ctrl+B"));
                ui.menu_button(tr("sub.tabwidth"), |ui| {
                    for w in [2usize, 3, 4, 8] {
                        if ui.radio_value(&mut self.tab, w, format!("{w}")).clicked() { ui.close(); }
                    }
                });
                if ui.button(lk("c.zoom-in", "Ctrl+=")).clicked() { self.font_px = (self.font_px + 1.0).min(40.0); }
                if ui.button(lk("c.zoom-out", "Ctrl+-")).clicked() { self.font_px = (self.font_px - 1.0).max(8.0); }
            });
            ui.menu_button(tr("menu.help"), |ui| {
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
            if ui.small_button("清屏").clicked() { self.term.output.clear(); }
            if ui.small_button("×").clicked() { self.term.open = false; }
            ui.checkbox(&mut self.term_stdin, "选区作标准输入");
            ui.checkbox(&mut self.term_replace, "输出替换选区");
            if self.term.running() { ui.label("运行中…"); }
        });
        let h = (ui.available_height() - 28.0).max(30.0);
        egui::ScrollArea::vertical().max_height(h).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&self.term.output).monospace()).selectable(true));
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.term.input).font(egui::TextStyle::Monospace).hint_text("输入命令, Enter 执行").desired_width(f32::INFINITY));
        if r.has_focus() {
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

    /// Markdown 预览: 块来自文档行的实时解析, 点块 = 编辑区跳到那一行
    fn md_ui(&mut self, ui: &mut egui::Ui) {
        use crate::markdown::{inline, parse, Block};
        use egui::RichText;
        let ai = self.active.min(self.docs.len() - 1);
        let blocks = parse(&self.docs[ai].lines);
        let mut jump: Option<usize> = None;
        let (text_c, strong_c, code_bg) = (ui.visuals().text_color(), ui.visuals().strong_text_color(), ui.visuals().code_bg_color);
        let rich = |s: &str| -> egui::text::LayoutJob {
            let mut job = egui::text::LayoutJob::default();
            for (t, b, i, c) in inline(s) {
                let mut f = egui::TextFormat { font_id: if c { FontId::monospace(13.0) } else { FontId::proportional(14.0) }, color: text_c, italics: i, ..Default::default() };
                if b { f.font_id = FontId::proportional(15.0); f.color = strong_c; }
                if c { f.background = code_bg; }
                job.append(&t, 0.0, f);
            }
            job
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for b in &blocks {
                let r = match &b.block {
                    Block::Heading { level, text } => ui.label(RichText::new(text).strong().size(match level { 1 => 26.0, 2 => 21.0, 3 => 18.0, _ => 15.0 })),
                    Block::Para(t) => ui.label(rich(t)),
                    Block::Quote(t) => ui.label(RichText::new(format!("│ {t}")).italics().weak()),
                    Block::Code { text, .. } => ui.label(RichText::new(text).monospace().background_color(ui.visuals().code_bg_color)),
                    Block::List { ordered, items } => ui.vertical(|ui| { for (k, it) in items.iter().enumerate() { ui.label(rich(&format!("{} {}", if *ordered { format!("{}.", k + 1) } else { "•".into() }, it))); } }).response,
                    Block::Table { rows } => ui.vertical(|ui| { for (k, row) in rows.iter().enumerate() { let t = row.join("   │   "); ui.label(if k == 0 { RichText::new(t).strong() } else { RichText::new(t) }); } }).response,
                    Block::Rule => { ui.separator(); ui.label("") }
                };
                if r.interact(Sense::click()).clicked() { jump = Some(b.line); }
                ui.add_space(6.0);
            }
        });
        if let Some(l) = jump { self.docs[ai].goto(l, 0); }
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

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |ctx: &egui::Context, m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        if cmd(ctx, Modifiers::COMMAND, Key::N) { self.new_doc(); }
        if cmd(ctx, Modifiers::COMMAND, Key::O) { self.open_dialog(); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT, Key::S) { self.save_all(); }
        else if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::S) { self.save(true); } else if cmd(ctx, Modifiers::COMMAND, Key::S) { self.save(false); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::W) { self.close_many(CloseKind::All); }
        else if cmd(ctx, Modifiers::COMMAND, Key::W) { let a = self.active; self.close_tab(a); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::F4) { self.close_open(); }
        // 标签 / 窗口 (键位同 Tauri 版): Ctrl+Tab 切换, F11 全屏, Alt+Enter 文件属性, Alt+P / Alt+N 移动履历, Ctrl+[ / Ctrl+] 对括弧
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab) { self.next_tab(false); } else if cmd(ctx, Modifiers::COMMAND, Key::Tab) { self.next_tab(true); }
        if cmd(ctx, Modifiers::NONE, Key::F11) { self.toggle_fullscreen(ctx); }
        if cmd(ctx, Modifiers::ALT, Key::Enter) { self.props = Some(self.cur().properties()); }
        if cmd(ctx, Modifiers::ALT, Key::P) { self.jump_go(-1); }
        if cmd(ctx, Modifiers::ALT, Key::N) { self.jump_go(1); }
        // 搜索: Ctrl+F 查找, Ctrl+H / Ctrl+R 置换, Ctrl+Alt+R 全部置换, Ctrl+G / Ctrl+Shift+F Grep (键位同 Tauri 版)
        if cmd(ctx, Modifiers::COMMAND | Modifiers::ALT, Key::R) { let i = self.active.min(self.docs.len() - 1); self.find.replace_all(&mut self.docs[i]); }
        else if cmd(ctx, Modifiers::COMMAND | Modifiers::ALT, Key::G) || cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::F) || cmd(ctx, Modifiers::COMMAND, Key::G) { self.focus_grep(); }
        else if cmd(ctx, Modifiers::COMMAND, Key::H) || cmd(ctx, Modifiers::COMMAND, Key::R) { self.open_find(true); }
        else if cmd(ctx, Modifiers::COMMAND, Key::F) || cmd(ctx, Modifiers::COMMAND, Key::I) { self.open_find(false); }
        if cmd(ctx, Modifiers::COMMAND, Key::B) { self.side.open = !self.side.open; }
        if cmd(ctx, Modifiers::ALT, Key::Z) { self.wrap = !self.wrap; }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::OpenBracket) { let d = self.cur(); let r = d.cursor[0]; d.toggle_fold(r); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::CloseBracket) { self.cur().folds.clear(); }
        // 放在折叠 (Ctrl+Shift+[ / ]) 之后: 否则带 Shift 的组合会先被对括弧跳转吃掉
        if cmd(ctx, Modifiers::COMMAND, Key::OpenBracket) || cmd(ctx, Modifiers::COMMAND, Key::CloseBracket) { self.bracket_jump(); }
        if cmd(ctx, Modifiers::COMMAND, Key::P) { self.print_cur(); }
        if cmd(ctx, Modifiers::COMMAND, Key::Backslash) { self.toggle_split(); }
        if cmd(ctx, Modifiers::COMMAND, Key::Backtick) { self.term.open = !self.term.open; }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::V) { self.md_preview = !self.md_preview; }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::F2) { self.cur().bookmarks.clear(); self.cur().tip = "已清除全部书签".into(); }
        else if cmd(ctx, Modifiers::COMMAND, Key::F2) { self.cur().bookmark("toggle"); }
        if cmd(ctx, Modifiers::ALT, Key::F2) { self.bm_open = !self.bm_open; }
        if cmd(ctx, Modifiers::SHIFT, Key::F2) { self.jump_mark(); self.cur().bookmark("prev"); } else if cmd(ctx, Modifiers::NONE, Key::F2) { self.jump_mark(); self.cur().bookmark("next"); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::M) { self.cur().macro_toggle(); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::L) { self.cur().macro_play(); }
        // Esc: 先停掉正在跑的 Grep, 再关查找栏, 最后才是清矩形选区 / 多光标
        if cmd(ctx, Modifiers::NONE, Key::Escape) {
            if self.side.grep.running() { self.side.grep.cancel(); self.side.grep.msg = format!("已停止, {} 处命中", self.side.grep.results.len()); }
            else if self.find.open { self.find.open = false; }
            else { let d = self.cur(); d.box_sel = None; d.extra.clear(); }
        }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::F3) { let i = self.active.min(self.docs.len() - 1); self.find.word_search(&mut self.docs[i], false); }
        else if cmd(ctx, Modifiers::COMMAND, Key::F3) { let i = self.active.min(self.docs.len() - 1); self.find.word_search(&mut self.docs[i], true); }
        else if cmd(ctx, Modifiers::SHIFT, Key::F3) { self.find_next(false); } else if cmd(ctx, Modifiers::NONE, Key::F3) { self.find_next(true); }
        if cmd(ctx, Modifiers::COMMAND, Key::Equals) { self.font_px = (self.font_px + 1.0).min(40.0); }
        if cmd(ctx, Modifiers::COMMAND, Key::Minus) { self.font_px = (self.font_px - 1.0).max(8.0); }
        if cmd(ctx, Modifiers::COMMAND, Key::Z) { self.cur().undo(); }
        if cmd(ctx, Modifiers::COMMAND, Key::Y) { self.cur().redo(); }
        if cmd(ctx, Modifiers::COMMAND, Key::A) { self.cur().select_all(); }
        if cmd(ctx, Modifiers::COMMAND, Key::D) { self.cur().edit("duplicate-line", None); }
        for c in cmds::CMDS {
            if let Some((m, k)) = cmds::parse_keys(c.keys) {
                if cmd(ctx, m, k) { self.run_action(c.action, ctx); }
            }
        }
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

    /// 选主题插件 (空 id = 回到内置深 / 浅色)
    fn set_theme_plugin(&mut self, id: &str) {
        self.theme_id = id.to_string();
        self.theme_visuals = plugins::rows().into_iter().find(|r| r.id == id && r.enabled).and_then(|r| r.theme).map(|t| plugins::visuals(&t.vars));
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
        let label = |c: &cmds::Cmd| if c.keys.is_empty() || c.label.contains("Ctrl+") { trz(c.label).to_string() } else { format!("{}  {}", trz(c.label), c.keys) };
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
        let mut theme = if self.dark { Theme::dark() } else { Theme::light() };
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
        let gutter = digits as f32 * char_w + 18.0;
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
        let mut clicked: Option<(Pos2, bool, bool)> = None; // (位置, 是否Shift, 是否拖动)
        let mut multi_click = 0u8; // 2 = 双击选词, 3 = 三击选行
        let mut ed_id: Option<egui::Id> = None;
        let out = egui::ScrollArea::both().auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
            let width = (widest as f32 * char_w + gutter + pad * 2.0 + 40.0).max(ui.available_width());
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, total_h), Sense::click_and_drag());
            ed_id = Some(resp.id);
            let painter = ui.painter_at(rect.intersect(ui.clip_rect()));
            let d = &self.docs[ai];
            let (first, last) = if use_offs {
                let f = offs.partition_point(|&o| o <= viewport.min.y).saturating_sub(1).min(d.lines.len().saturating_sub(1));
                let l = (offs.partition_point(|&o| o < viewport.max.y) + 1).min(d.lines.len());
                (f, l)
            } else {
                ((viewport.min.y / row_h).floor().max(0.0) as usize, ((viewport.max.y / row_h).ceil() as usize + 1).min(d.lines.len()))
            };
            let x0 = rect.min.x + gutter + pad;
            for row in first..last {
                if d.is_hidden(row) { continue; }
                let y = rect.min.y + top(row);
                let (disp, map) = expand_tabs(&d.lines[row], tab);
                widest = widest.max(disp.chars().count());
                let mut job = highlight_job(&d.lines[row], &disp, &map, cfg.as_ref(), &theme, &font, text_color, row_h);
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
                painter.galley(Pos2::new(x0, y), galley, text_color);
                if d.folds.iter().any(|f| f.0 == row) {
                    painter.text(Pos2::new(rect.min.x + 2.0, y + (row_h - base_h) / 2.0), egui::Align2::LEFT_TOP, "▸", font.clone(), weak);
                }
                if d.bookmarks.iter().any(|b| b.row == row) {
                    painter.circle_filled(Pos2::new(rect.min.x + 7.0, y + row_h / 2.0), 3.5, Color32::from_rgb(80, 170, 255));
                }
                painter.text(Pos2::new(rect.min.x + gutter - 8.0, y + (row_h - base_h) / 2.0), egui::Align2::RIGHT_TOP, format!("{}", row + 1), font.clone(), weak);
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

fn tree(ui: &mut egui::Ui, side: &mut Sidebar, dir: &std::path::Path, open: &mut Option<(PathBuf, usize)>) {
    for e in side.children_of(dir) {
        if e.is_dir {
            egui::CollapsingHeader::new(&e.name).id_salt(&e.path).show(ui, |ui| tree(ui, side, &e.path, open));
        } else if ui.selectable_label(false, &e.name).clicked() {
            *open = Some((e.path.clone(), 1));
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

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
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
        if self.props.is_some() { self.props_window(ctx); }
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
                                if ui.button(tr("c.tab-move-left")).clicked() { tab_cmd = Some(TabCmd::Move(i, true)); ui.close(); }
                                if ui.button(tr("c.tab-move-right")).clicked() { tab_cmd = Some(TabCmd::Move(i, false)); ui.close(); }
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
        let mut st_cmd: Option<StatusCmd> = None;
        egui::Panel::bottom("status").show(root, |ui| {
            let d = &self.docs[self.active.min(self.docs.len() - 1)];
            ui.horizontal(|ui| {
                // 位置: 点一下 = 转到行
                if ui.add(egui::Label::new(format!("Ln {}, Col {}", d.cursor[0] + 1, d.cursor[1] + 1)).sense(Sense::click())).on_hover_text(tr("st.goto-hint")).clicked() { st_cmd = Some(StatusCmd::Goto); }
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
                if d.overwrite { ui.label(tr("st.overwrite")); }
                if d.readonly { ui.label(tr("st.readonly")); }
                if d.rec.is_some() { ui.colored_label(Color32::from_rgb(230, 70, 70), "● REC"); }
                if d.box_sel.is_some() { ui.label(tr("st.box")); }
                if !d.extra.is_empty() { ui.label(trf("st.cursors", &[&(d.extra.len() + 1).to_string()])); }
                ui.separator();
                ui.label(&d.tip);
            });
        });
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
        let find_focus = ctx.memory(|m| m.focused()).map(|f| !self.editor_ids.contains(&f)).unwrap_or(false);
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
        if self.term.running() { ctx.request_repaint_after(std::time::Duration::from_millis(80)); }
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
