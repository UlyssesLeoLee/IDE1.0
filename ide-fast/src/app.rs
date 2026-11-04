//! 应用外壳: 菜单 / 标签 / 编辑区 / 查找条 / 状态栏。编辑语义都在 `doc.rs` (-> editcore), 这里只管画和收输入。

use crate::ui_ext::TipExt;
use crate::ckbox::CheckExt;
use crate::doc::Doc;
use crate::kinsoku::{expand_wrap, Wrap};
use crate::rowview::RowView;
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

mod chrome;
mod docks;
mod menuui;
mod menunav;
mod tabbar;
mod statusbar;
mod titlebar;
mod welcome;
mod textcmds;
mod picks;
mod ghopts;
mod updateui;
mod aichat;
mod mdopts;
mod mdstruct;
mod printui;
mod sysopts;
mod tagops;
#[cfg(test)]
mod perf_bench;

pub struct App {
    /// 批次 10: 自动备份 / 文件锁 / 玻璃窗口 / 底部页签 (`app/sysopts.rs`)、Markdown 预览扩展 (`app/mdopts.rs`)、GitHub 插件窗口 (`app/ghopts.rs`)
    sys: sysopts::SysState,
    md10: mdopts::MdState,
    gh: ghopts::GhState,
    upd: updateui::UpdState,
    ai: aichat::AiState,
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
    panes: crate::split::Panes,
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
    /// 隐藏创建的窗口是否已经显示 (第一帧画完后显示)
    shown: bool,
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
    /// 「关于」窗口开着
    about_open: bool,
    /// 关窗请求那一刻的会话 (路径, 活动位置): 关窗确认会逐个关掉标签, 之后再算会话就少了刚处理过的文件
    exit_session: Option<(Vec<String>, usize)>,
    /// 打开命令面板那一刻的脚本插件命令 (打开期间不再反复读盘)
    pal_plugins: Vec<PluginCmd>,
    /// 批次 10 的小工具窗口 (剪贴板历史 / 控制字符 / 戳记 / 折行列数)
    tools: crate::tools::Tools,
    /// 标签跳转: 候选列表 / 关键字框 / 返回栈
    tags: crate::tagjump::TagState,
    /// 正在拖放选中文字 (按下时落在选区里才有)
    text_drag: Option<crate::textdrag::TextDrag>,
    print_ui: printui::PrintUi,
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
            dark: true, font_px: 15.0, tab: 4, highlight: true, recent: vec![], saved: Settings::default(), registry: TypeRegistry::default_registry(), side: Sidebar::new(), term: Terminal::new(), goto_open: false, was_focused: true, goto_text: String::new(), theme_id: String::new(), theme_visuals: None, theme_vars: None, tc: crate::view::ThemeColors::builtin(true), split_on: false, panes: Default::default(),   pane_keys: true, term_stdin: false, term_replace: false, md_preview: false, wrap: false, goal_x: None, inbox, prefs: None, persist: true, cfg: Settings::default(), applied_font: String::new(), geom_now: None, maximized_now: false, wrap_cache: None, max_cols: 80, trace, first_frame: true, shown: false, confirm_exit: false, preedit: String::new(), center_once: false, grep_doc: None, jump_req: None, plug: Default::default(), editor_ids: vec![], jump: JumpHist::default(), props: None, bm_open: false, fullscreen: false, topmost: false, recent_dirs: vec![], positions: vec![], bracket_cache: None, last_active: usize::MAX, md_cache: Default::default(), md_parsed: None, md_scroll_to: None, diff: None, diff_list_open: false, diff_pick: false, cmdui: CmdUi::default(), pal_plugins: vec![], about_open: false, exit_session: None, tools: Default::default(), tags: Default::default(), text_drag: None, sys: Default::default(), md10: Default::default(), gh: Default::default(), upd: Default::default(), ai: Default::default(), print_ui: Default::default(),
        };
        app.persist = persist;
        app.sys.decor = st.system_title_bar;
        // 窗口一律透明 (main.rs); 模糊材质由 glass_sync 按主题随时开关
        app.sys.blur_on = plugins::is_translucent(&st.theme_plugin) && st.glass_blur; // main.rs 建窗口时已按同一判断加了模糊
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
        if args.is_empty() && st.restore_session { app.restore_session(&st); }
        if app.docs.is_empty() {
            app.new_doc();
        }
        if persist { app.sys.restore = crate::backup::leftovers(); } // 只扫一次: 上次没正常退出留下的自动备份
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

    /// 现在打开的有路径的文档 (会话恢复用), 最多 20 个
    fn session_paths(&self) -> Vec<String> {
        if let Some((p, _)) = &self.exit_session { return p.clone(); }
        self.docs.iter().filter_map(|d| d.path.as_ref().map(|p| p.display().to_string())).take(20).collect()
    }

    /// 活动文档在 `session_paths` 里的位置 (活动的是无路径文档 = 0)
    fn session_active(&self) -> usize {
        if let Some((_, a)) = &self.exit_session { return *a; }
        let cur = self.docs.get(self.active.min(self.docs.len().saturating_sub(1))).and_then(|d| d.path.as_ref());
        self.docs.iter().filter_map(|d| d.path.as_ref()).take(20).position(|p| Some(p) == cur).unwrap_or(0)
    }

    fn current_settings(&self) -> Settings {
        let (f, g) = (&self.find, &self.side.grep.o);
        let search = crate::settings::SearchPrefs { find_case: f.opts.case, find_regex: f.opts.regex, find_word: f.opts.word, find_wrap: f.opts.wrap, find_marks: f.marks, grep_case: g.case, grep_regex: g.regex, grep_word: g.word, grep_sub: g.sub, grep_outside: g.outside, grep_files: g.files.clone(), grep_exc_files: g.exc_files.clone(), grep_exc_folders: g.exc_folders.clone() };
        let mut view = self.cfg.view.clone();
        (view.side_open, view.term_open, view.md_preview) = (self.side.open, self.term.open, self.md_preview);
        Settings { dark: self.dark, font_px: self.font_px, tab_width: self.tab, highlight: self.highlight, wrap: self.wrap, theme_plugin: self.theme_id.clone(), recent: self.recent.clone(), recent_dirs: self.recent_dirs.clone(), positions: self.positions.clone(), search, view, session: self.session_paths(), session_active: self.session_active(), ..self.cfg.clone() }
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

    /// 文档适用的文件类型: 手动指定的优先 (名字查不到就退回按扩展名判定)
    fn type_of(&self, d: &Doc) -> Option<&TypeConfig> {
        d.syntax.as_deref().and_then(|n| self.registry.by_name(n)).or_else(|| self.registry.detect(&d.name))
    }

    /// 某个文件名适用的 (Tab 宽度, 自动换行, Tab 插入空格)
    fn eff(&self, name: &str) -> (usize, bool, bool) {
        self.cfg.effective_for(name, self.tab, self.wrap, self.cfg.insert_spaces)
    }

    /// 启动时把上次的查找 / Grep 条件放回去
    fn apply_search_prefs(&mut self, st: &Settings) {
        let p = &st.search;
        self.find.opts = crate::find::FindOpts { case: p.find_case, regex: p.find_regex, word: p.find_word, wrap: p.find_wrap, migemo: self.find.opts.migemo };
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
        egui::Window::new(tr("pf.title")).open(&mut open).collapsible(false).resizable(false).default_width(460.0).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (t, key) in [(PrefsTab::General, "pf.tab.general"), (PrefsTab::Edit, "pf.tab.edit"), (PrefsTab::Files, "pf.tab.files"), (PrefsTab::Font, "pf.tab.font"), (PrefsTab::Types, "pf.tab.types"), (PrefsTab::Search, "pf.tab.search"), (PrefsTab::Terminal, "pf.tab.terminal"), (PrefsTab::Toolbar, "pf.tab.toolbar"), (PrefsTab::Macro, "pf.tab.macro")] {
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
                if menuui::mbtn(ui, lk("c.reset-settings", "")).clicked() { reset = true; }
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
        // 开了活动栏就用它切换面板 (Tauri 版侧栏上没有页签条); 没开时(极简界面)才需要这一行页签
        if self.cfg.view.rail {
            // 面板标题 (Tauri `#left-title`): 只有名字, 13px; 文件页没打开项目时写「未打开项目」, 打开后写文件夹名
            let title = match self.side.tab {
                Tab::Files => self.side.root.as_ref().map(|r| r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| r.display().to_string())).unwrap_or_else(|| trz("未打开项目").to_string()),
                Tab::Search => trz("搜索").to_string(),
                Tab::Scm => trz("源代码管理").to_string(),
                Tab::Outline => trz("大纲").to_string(),
                Tab::Plugins => trz("扩展").to_string(),
            };
            ui.add_space(8.0);
            ui.label(egui::RichText::new(title).size(13.0));
            ui.add_space(4.0);
        } else {
            ui.horizontal(|ui| {
                for (t, name) in [(Tab::Files, "文件"), (Tab::Outline, "大纲"), (Tab::Search, "搜索"), (Tab::Scm, "Git"), (Tab::Plugins, "扩展")] {
                    if ui.selectable_label(self.side.tab == t, trz(name)).clicked() { self.side.tab = t; }
                }
            });
            ui.separator();
        }
        let mut open: Option<(PathBuf, usize)> = None;
        match self.side.tab {
            Tab::Files => {
                match self.side.root.clone() {
                    None => {
                        if self.cfg.view.rail { ui.add_space(6.0); ui.label(egui::RichText::new(trz("点工具栏「打开文件夹」导入项目")).size(12.0).color(self.tc.fg_dim)); ui.add_space(6.0); }
                        if ui.button(trz("打开文件夹…")).clicked() { self.open_folder_dialog(); }
                    }
                    Some(root) => {
                        let mut op: Option<FileOp> = None;
                        ui.horizontal(|ui| {
                            let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string());
                            // 活动栏开着时面板标题已经写了文件夹名, 这里只留一块可右键的空位
                            let title = if self.cfg.view.rail { ui.label(egui::RichText::new(root.display().to_string()).size(11.0).color(self.tc.fg_dim)) } else { ui.strong(name) };
                            title.context_menu(|ui| {
                                if ui.button(trz("新建文件…")).clicked() { op = Some(FileOp::NewFile(root.clone())); ui.close(); }
                                if ui.button(trz("新建文件夹…")).clicked() { op = Some(FileOp::NewDir(root.clone())); ui.close(); }
                            });
                            if ui.small_button(trz("刷新")).clicked() { self.side.refresh(); }
                        });
                        self.pending_ui(ui);
                        crate::scrollx::show(ui, egui::ScrollArea::both().auto_shrink([false, false]), |ui| tree(ui, &mut self.side, &root, &mut open, &mut op));
                        if let Some(o) = op { self.begin_file_op(o); }
                    }
                }
            }
            Tab::Outline => {
                let ai = self.active.min(self.docs.len() - 1);
                let d = &self.docs[ai];
                self.side.update_outline(d.id, d.rev, &d.name, &d.lines);
                let mut jump: Option<usize> = None;
                crate::scrollx::show(ui, egui::ScrollArea::both().auto_shrink([false, false]), |ui| {
                    if self.side.outline.is_empty() { ui.label(trz("(没有可显示的大纲)")); }
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
            Tab::Plugins => { crate::scrollx::show(ui, egui::ScrollArea::vertical().auto_shrink([false, false]), |ui| self.plugin_panel(ui)); }
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
            if let Some(e) = &p.error { ui.colored_label(red, crate::msgtr::msg(e).as_ref()); }
            ui.horizontal(|ui| {
                if ui.button(trz("确定")).clicked() { go = true; }
                if ui.button(trz("取消")).clicked() { cancel = true; }
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
            ui.strong(if st.branch.is_empty() { "Git".to_string() } else { crate::msgtr::msg(&format!("分支 {}", st.branch)).into_owned() });
            if ui.small_button(trz("刷新")).clicked() { self.side.scm.refresh(root.clone()); }
            if self.side.scm.loading() { ui.label("…"); }
        });
        if let Some(e) = self.side.scm.status.error.clone() { ui.colored_label(self.tc.red, crate::msgtr::msg(&e).as_ref()); }
        let Some(top) = self.side.scm.status.top.clone() else { return };
        let changes = self.side.scm.status.changes.clone();
        if changes.is_empty() && self.side.scm.status.error.is_none() { ui.label(trz("没有改动")); }
        let tc = self.tc;
        let mut act: Option<(String, u8)> = None; // 0 = 打开, 1 = 差异, 2 = 暂存, 3 = 取消暂存
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            for (title, staged) in [("已暂存的更改", true), ("更改", false)] {
                let rows: Vec<&crate::scm::Change> = changes.iter().filter(|c| c.staged() == staged).collect();
                if rows.is_empty() { continue; }
                ui.label(egui::RichText::new(format!("{} ({})", trz(title), rows.len())).strong());
                for c in rows {
                    ui.horizontal(|ui| {
                        let col = match c.letter() { 'M' => tc.yellow, 'A' | 'U' => tc.green, 'D' => tc.red, _ => tc.blue };
                        ui.label(egui::RichText::new(c.letter().to_string()).monospace().color(col));
                        if ui.small_button(trz("差异")).clicked() { act = Some((c.path.clone(), 1)); }
                        if staged { if ui.small_button("-").tip(trz("取消暂存")).clicked() { act = Some((c.path.clone(), 3)); } }
                        else if ui.small_button("+").tip(trz("暂存")).clicked() { act = Some((c.path.clone(), 2)); }
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

    /// 把一段文字开成新的只读标签 (Git 差异 / 文档比较结果 / 帮助手册)
    fn open_text_doc(&mut self, name: &str, text: &str) {
        let id = self.id();
        self.docs.push(Self::text_doc(id, name, text));
        self.active = self.docs.len() - 1;
    }

    /// 只读的文字文档 (没有路径、没有修改、撤销栈为空)
    fn text_doc(id: u64, name: &str, text: &str) -> Doc {
        let mut d = Doc::untitled(id, 0);
        d.name = name.to_string();
        d.lines = text.trim_end_matches('\n').split('\n').map(str::to_string).collect();
        d.base = d.lines.clone();
        d.readonly = true;
        d
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
            egui::Window::new(trz("与哪个标签比较")).open(&mut open).collapsible(false).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
                for (i, d) in self.docs.iter().enumerate().filter(|(i, _)| *i != cur) {
                    if ui.selectable_label(false, &d.name).clicked() { pick = Some(i); }
                }
                if self.docs.len() < 2 { ui.label(trz("需要至少两个标签")); }
            });
            if let Some(i) = pick { self.start_diff(i); open = false; }
            self.diff_pick = open;
        }
        if self.diff_list_open {
            let mut open = true;
            let mut jump: Option<usize> = None;
            egui::Window::new(trz("差异列表")).open(&mut open).default_width(280.0).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
                let Some(v) = &self.diff else { ui.label(trz("没有进行中的比较")); return };
                ui.label(v.summary());
                let right = self.docs[self.active.min(self.docs.len() - 1)].id == v.right;
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (n, h) in v.hunks.iter().enumerate() {
                        let (kind, (s, len)) = (if h.a_len == 0 { trz("新增") } else if h.b_len == 0 { trz("删除") } else { trz("修改") }, if right { (h.b_start, h.b_len) } else { (h.a_start, h.a_len) });
                        let label = trf("diff.hunk", &[&(n + 1).to_string(), kind, &(s + 1).to_string(), &len.to_string()]);
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
        let (cur_id, old_bk) = { let d = self.cur(); (d.id, crate::backup::file_name(d.path.as_deref(), &d.name, d.id)) };
        self.before_save(cur_id);
        match self.cur().save_to(&path) {
            Ok(()) => {
                let ai = self.active.min(self.docs.len() - 1);
                self.after_save(ai, &old_bk);
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
        self.sys.backup.forget(&gone);
        self.jump.forget(gone.id);
        self.end_split();
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
        self.end_split();
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
        egui::Window::new(tr("dlg.bm-title")).open(&mut open).collapsible(false).default_width(360.0).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
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
        egui::Window::new(tr("dlg.props-title")).open(&mut open).collapsible(false).resizable(false).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&text).monospace()).selectable(true));
            if ui.button(trz("复制")).clicked() { ctx.copy_text(text.clone()); }
        });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) { self.props = None; }
    }

    /// 插件列表: 主题单选 / 脚本插件开关 / 命令 / 导入 / GitHub 安装 (「插件」菜单和侧栏的扩展页共用)
    fn plugin_panel(&mut self, ui: &mut egui::Ui) {
        let rows = plugins::rows();
        if ui.radio(self.theme_id.is_empty(), tr("c.builtin-theme")).clicked() { self.set_theme_plugin(""); ui.close(); }
        for r in &rows {
            ui.horizontal(|ui| {
                if !r.commands.is_empty() || r.script_only {
                    if ui.check(&mut r.enabled.clone(), "").tip(tr("ui.plugin-enabled")).clicked() {
                        let _ = plugins::set_enabled(&r.id, !r.enabled);
                        self.md10.invalidate();
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
                    self.md10.invalidate();
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
        if ui.button(tr("c.plugin-github")).clicked() { ui.close(); self.gh_open(); }
        let gone = plugins::removed_bundled();
        if !gone.is_empty() {
            ui.menu_button(tr("sub.plugin-restore"), |ui| {
                for (id, name, _) in gone {
                    if ui.button(name).clicked() { let _ = plugins::restore(&id); self.md10.invalidate(); ui.close(); }
                }
            });
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use menuui::menu_check;
        let tc = self.tc;
        let hide = self.sys.mn_hide;
        let mn = move |k: &str| if hide { ' ' } else { menuui::mnemonic(k) };
        let mut mids: Vec<(&'static str, egui::Id)> = Vec::new();
        let bf = self.sys.bar_focus;
        ui.data_mut(|d| d.insert_temp(menuui::bar_focus_id(), bf));
        egui::MenuBar::new().style(menuui::bar_style(tc)).config(egui::containers::menu::MenuConfig::new().style(menuui::style(tc))).ui(ui, |ui| {
            menuui::top(ui, &mut mids, "menu.file", menuui::menu_title(&tc, tr("menu.file"), mn("menu.file")), |ui| menuui::scroll(ui, |ui| {
                if menuui::mbtn(ui, lk("c.new-file", "Ctrl+N")).clicked() { self.new_doc(); ui.close(); }
                if menuui::mbtn(ui, lk("c.open-file", "Ctrl+O")).clicked() { ui.close(); self.open_dialog(); }
                if menuui::mbtn(ui, lk("c.open-folder", "")).clicked() { ui.close(); self.open_folder_dialog(); }
                ui.menu_button(tr("sub.recent"), |ui| {
                    if self.recent.is_empty() { ui.label(tr("ui.none")); }
                    let mut pick: Option<String> = None;
                    for r in &self.recent {
                        if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
                    }
                    if let Some(r) = pick { self.open_path(std::path::Path::new(&r)); }
                    ui.separator();
                    if menuui::mbtn(ui, lk("c.recent-clear", "")).clicked() { self.clear_recent(); ui.close(); }
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
                if menuui::mbtn(ui, lk("c.save", "Ctrl+S")).clicked() { ui.close(); self.save(false); }
                if menuui::mbtn(ui, lk("c.save-as", "Ctrl+Shift+S")).clicked() { ui.close(); self.save(true); }
                if menuui::mbtn(ui, lk("c.save-all", "Ctrl+Alt+Shift+S")).clicked() { ui.close(); self.save_all(); }
                if menuui::mbtn(ui, lk("c.save-close", "")).clicked() { ui.close(); self.save_close(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.close-tab", "Ctrl+W")).clicked() { ui.close(); let a = self.active; self.close_tab(a); }
                if menuui::mbtn(ui, lk("c.close-all", "Ctrl+Shift+W")).clicked() { ui.close(); self.close_many(CloseKind::All); }
                if menuui::mbtn(ui, lk("c.close-open", "Ctrl+Shift+F4")).clicked() { ui.close(); self.close_open(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.reload", "")).clicked() { ui.close(); self.reload_current(); }
                ui.menu_button(tr("sub.reopen"), |ui| {
                    for cs in Charset::ALL {
                        if ui.button(cs.label()).clicked() { ui.close(); self.reopen_as(cs); }
                    }
                });
                ui.separator();
                if menuui::mbtn(ui, lk("c.print", "Ctrl+P")).clicked() { ui.close(); self.print_cur(); }
                if menuui::mbtn(ui, lk("c.print-setup", "")).clicked() { ui.close(); self.print_ui.setup_open = true; }
                if menuui::mbtn(ui, lk("c.print-preview", "")).clicked() { ui.close(); self.print_ui.preview = Some(0); }
                if menuui::mbtn(ui, lk("c.browse", "")).clicked() { ui.close(); self.browse_current(); }
                if menuui::mbtn(ui, lk("c.file-properties", "Alt+Enter")).clicked() { ui.close(); self.props = Some(self.cur().properties()); }
                if menuui::mbtn(ui, lk("c.new-window", "")).clicked() { ui.close(); self.new_window(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.exit", "")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
            }));
            menuui::top(ui, &mut mids, "menu.edit", menuui::menu_title(&tc, tr("menu.edit"), mn("menu.edit")), |ui| menuui::scroll(ui, |ui| {
                if menuui::mbtn(ui, lk("c.undo", "Ctrl+Z")).clicked() { self.cur().undo(); ui.close(); }
                if menuui::mbtn(ui, lk("c.redo", "Ctrl+Y")).clicked() { self.cur().redo(); ui.close(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.cut", "Ctrl+X")).clicked() { if let Some(c) = self.cur().edit("cut", None) { self.put_clip(ctx, c); } ui.close(); }
                if menuui::mbtn(ui, lk("c.copy", "Ctrl+C")).clicked() { if let Some(c) = self.cur().edit("copy", None) { self.put_clip(ctx, c); } ui.close(); }
                if menuui::mbtn(ui, lk("c.paste", "Ctrl+V")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste); ui.close(); }
                if menuui::mbtn(ui, lk("c.select-all", "Ctrl+A")).clicked() { self.cur().select_all(); ui.close(); }
                ui.separator();
                self.cmd_menu(ui, ctx);
                ui.separator();
                for (z, a) in [("选到上一段落", KApp::SelParaPrev), ("选到下一段落", KApp::SelParaNext), ("范围选择模式 (开始 / 结束)", KApp::SelMode), ("复制文件名", KApp::CopyFname), ("复制完整路径", KApp::CopyPath),
                               ("代码补全", KApp::Completion), ("SPACE 缩进", KApp::SpaceIndent), ("SPACE 反缩进", KApp::SpaceUnindent), ("光标行居中", KApp::CurCenter), ("切换选区统计方式 (字符 / 字节 / 行)", KApp::SelCountMode), ("复制路径与光标位置", KApp::CopyTag), ("在折行处加换行复制", KApp::CopyWrapped), ("列块删除", KApp::ColDelete), ("列块替换…", KApp::ColReplace), ("复制为彩色 HTML", KApp::CopyHtml), ("复制为彩色 HTML (带行号)", KApp::CopyHtmlLn), ("剪贴板历史…", KApp::ClipHistory)] {
                    if ui.button(Self::app_label(a, z)).clicked() { self.run_app(a, ctx); ui.close(); }
                }
                ui.separator();
                ui.menu_button(trz("插入"), |ui| self.insert_menu(ui, ctx));
                for (z, a) in [("Base64 解码", KApp::Base64Decode), ("uudecode 解码", KApp::UuDecode)] {
                    if ui.button(Self::app_label(a, z)).clicked() { self.run_app(a, ctx); ui.close(); }
                }
            }));
            menuui::top(ui, &mut mids, "menu.search", menuui::menu_title(&tc, tr("menu.search"), mn("menu.search")), |ui| menuui::scroll(ui, |ui| {
                if menuui::mbtn(ui, lk("c.find", "Ctrl+F")).clicked() { self.open_find(false); ui.close(); }
                if menuui::mbtn(ui, lk("c.replace", "Ctrl+H / Ctrl+R")).clicked() { self.open_find(true); ui.close(); }
                if menuui::mbtn(ui, lk("c.grep", "Ctrl+G")).clicked() { self.focus_grep(); ui.close(); }
                if menuui::mbtn(ui, lk("c.md-struct", "")).clicked() { ui.close(); self.md_struct_open(); }
                if menuui::mbtn(ui, lk("c.find-next", "F3")).clicked() { self.find_next(true); ui.close(); }
                if menuui::mbtn(ui, lk("c.find-prev", "Shift+F3")).clicked() { self.find_next(false); ui.close(); }
                if menuui::mbtn(ui, lk("c.find-word", "Ctrl+F3")).clicked() { let i = self.active.min(self.docs.len() - 1); self.find.word_search(&mut self.docs[i], true); ui.close(); }
                if menuui::mbtn(ui, lk("c.replace-all", "Ctrl+Alt+R")).clicked() { let i = self.active.min(self.docs.len() - 1); self.find.replace_all(&mut self.docs[i]); ui.close(); }
                for (z, a) in [("上一处改动", KApp::ModPrev), ("下一处改动", KApp::ModNext), ("上一个函数", KApp::FuncPrev), ("下一个函数", KApp::FuncNext), ("(选择) 上一处改动", KApp::ModPrevSel), ("(选择) 下一处改动", KApp::ModNextSel), ("打开同名 C/C++ 头 / 源文件", KApp::OpenHc), ("切换查找标记 (全部强调)", KApp::FindMarks), ("回到查找起点", KApp::FindStart), ("转到定义 (本文件)", KApp::GotoDef)] {
                    if ui.button(trz(z)).clicked() { self.run_app(a, ctx); ui.close(); }
                }
                ui.menu_button(trz("比较"), |ui| {
                    if ui.button(trz("与其他标签比较…")).clicked() { self.diff_pick = true; ui.close(); }
                    if ui.button(trz("与文件比较…")).clicked() { self.diff_with_file(); ui.close(); }
                    if ui.button(trz("差异列表")).clicked() { self.diff_list_open = true; ui.close(); }
                    if ui.button(trz("下一处差异 (Alt+↓)")).clicked() { self.diff_step(true); ui.close(); }
                    if ui.button(trz("上一处差异 (Alt+↑)")).clicked() { self.diff_step(false); ui.close(); }
                    if ui.button(trz("清除比较")).clicked() { self.diff = None; self.diff_list_open = false; ui.close(); }
                });
            }));
            menuui::top(ui, &mut mids, "menu.fold", menuui::menu_title(&tc, tr("menu.fold"), mn("menu.fold")), |ui| {
                if menuui::mbtn(ui, lk("c.fold-toggle", "Ctrl+Shift+[")).clicked() { let d = self.cur(); let r = d.cursor[0]; d.toggle_fold(r); ui.close(); }
                if menuui::mbtn(ui, lk("c.unfold-all", "Ctrl+Shift+]")).clicked() { self.cur().folds.clear(); ui.close(); }
            });
            menuui::top(ui, &mut mids, "menu.plugin", menuui::menu_title(&tc, tr("menu.plugin"), mn("menu.plugin")), |ui| self.plugin_panel(ui));
            menuui::top(ui, &mut mids, "menu.setting", menuui::menu_title(&tc, tr("menu.setting"), mn("menu.setting")), |ui| menuui::scroll(ui, |ui| {
                if menuui::mbtn(ui, lk("c.common-config", "")).clicked() { self.open_prefs(); ui.close(); }
                ui.menu_button(trz("语法类型"), |ui| self.syntax_items(ui));
                ui.menu_button(tr("sub.uilang"), |ui| {
                    for l in Lang::ALL {
                        if menu_check(ui, &tc, crate::i18n::cur() == l, l.native()).clicked() {
                            let mut n = self.current_settings();
                            n.lang = l.code().into();
                            self.apply_settings(&n);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if menuui::mbtn(ui, lk("c.export-config", "")).clicked() { ui.close(); self.export_settings(); }
                if menuui::mbtn(ui, lk("c.import-config", "")).clicked() { ui.close(); self.import_settings(); }
                if menuui::mbtn(ui, lk("c.import-sakura", "")).clicked() { ui.close(); self.import_sakura(); }
                if menuui::mbtn(ui, lk("c.reset-settings", "")).clicked() { ui.close(); self.reset_settings(); }
                if ui.button(tr("c.open-autosave")).clicked() { ui.close(); self.open_autosave(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.key-config", "")).clicked() { ui.close(); self.cmdui.open_keycfg(); }
            }));
            menuui::top(ui, &mut mids, "menu.bookmark", menuui::menu_title(&tc, tr("menu.bookmark"), mn("menu.bookmark")), |ui| {
                if menuui::mbtn(ui, lk("c.bookmark-toggle", "Ctrl+F2")).clicked() { self.cur().bookmark("toggle"); ui.close(); }
                if menuui::mbtn(ui, lk("c.bookmark-next", "F2")).clicked() { self.jump_mark(); self.cur().bookmark("next"); ui.close(); }
                if menuui::mbtn(ui, lk("c.bookmark-prev", "Shift+F2")).clicked() { self.jump_mark(); self.cur().bookmark("prev"); ui.close(); }
                if menuui::mbtn(ui, lk("c.bookmark-view", "Alt+F2")).clicked() { self.bm_open = true; ui.close(); }
                if menuui::mbtn(ui, lk("c.bookmark-reset", "Ctrl+Shift+F2")).clicked() { self.cur().bookmarks.clear(); self.cur().tip = tr("dlg.bm-cleared").into(); ui.close(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.bracket-jump", "Ctrl+[")).clicked() { self.bracket_jump(); ui.close(); }
                if menuui::mbtn(ui, lk("c.hist-prev", "Alt+P")).clicked() { self.jump_go(-1); ui.close(); }
                if menuui::mbtn(ui, lk("c.hist-next", "Alt+N")).clicked() { self.jump_go(1); ui.close(); }
                if menuui::mbtn(ui, lk("c.hist-mark", "")).clicked() { self.jump_mark(); self.cur().tip = tr("dlg.hist-marked").into(); ui.close(); }
            });
            menuui::top(ui, &mut mids, "menu.window", menuui::menu_title(&tc, tr("menu.window"), mn("menu.window")), |ui| menuui::scroll(ui, |ui| {
                if menuui::mbtn(ui, lk("c.split", "Ctrl+\\")).clicked() { self.toggle_split(); ui.close(); }
                if menuui::mbtn(ui, lk("c.split-rows", "")).clicked() { self.toggle_split_kind(crate::split::SplitKind::Rows); ui.close(); }
                if menuui::mbtn(ui, lk("c.split-quad", "")).clicked() { self.toggle_split_kind(crate::split::SplitKind::Quad); ui.close(); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.next-tab", "Ctrl+Tab")).clicked() { self.next_tab(true); ui.close(); }
                if menuui::mbtn(ui, lk("c.prev-tab", "Ctrl+Shift+Tab")).clicked() { self.next_tab(false); ui.close(); }
                ui.menu_button(tr("sub.tabops"), |ui| {
                    if menuui::mbtn(ui, lk("c.tab-move-left", "")).clicked() { self.move_active_tab(true); ui.close(); }
                    if menuui::mbtn(ui, lk("c.tab-move-right", "")).clicked() { self.move_active_tab(false); ui.close(); }
                    if menuui::mbtn(ui, lk("c.close-others", "")).clicked() { self.close_many(CloseKind::Others); ui.close(); }
                    if menuui::mbtn(ui, lk("c.close-left", "")).clicked() { self.close_many(CloseKind::Left); ui.close(); }
                    if menuui::mbtn(ui, lk("c.close-right", "")).clicked() { self.close_many(CloseKind::Right); ui.close(); }
                });
                ui.menu_button(trz("标签分组"), |ui| {
                    for (z, a) in [("下一个分组", KApp::GroupNext), ("上一个分组", KApp::GroupPrev), ("移到新分组", KApp::TabSeparate), ("移到下一个分组", KApp::TabJointNext), ("移到上一个分组", KApp::TabJointPrev), ("合并为一组", KApp::BindWindow), ("关闭当前分组", KApp::GroupClose)] {
                        if ui.button(Self::app_label(a, z)).clicked() { self.run_app(a, ctx); ui.close(); }
                    }
                });
                if ui.button(Self::app_label(KApp::WindowList, "窗口一览…")).clicked() { self.run_app(KApp::WindowList, ctx); ui.close(); }
                ui.menu_button(tr("sub.winlist"), |ui| {
                    let mut pick = None;
                    for (i, d) in self.docs.iter().enumerate() {
                        if ui.selectable_label(i == self.active, format!("{}{}", if d.dirty { "● " } else { "" }, d.name)).clicked() { pick = Some(i); ui.close(); }
                    }
                    if let Some(i) = pick { self.select_tab(i); }
                });
                ui.separator();
                if menuui::mbtn(ui, lk("c.minimize-window", "")).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); ui.close(); }
                if menuui::mbtn(ui, lk("c.maximize", "")).clicked() { let m = ctx.input(|i| i.viewport().maximized.unwrap_or(false)); ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!m)); ui.close(); }
                if menu_check(ui, &tc, self.fullscreen, lk("c.fullscreen", "F11")).clicked() { self.toggle_fullscreen(ctx); ui.close(); }
                if menu_check(ui, &tc, self.topmost, lk("c.topmost", "")).clicked() { self.toggle_topmost(ctx); ui.close(); }
            }));
            menuui::top(ui, &mut mids, "menu.macro", menuui::menu_title(&tc, tr("menu.macro"), mn("menu.macro")), |ui| self.macro_items(ui));
            menuui::top(ui, &mut mids, "menu.view", menuui::menu_title(&tc, tr("menu.view"), mn("menu.view")), |ui| menuui::scroll(ui, |ui| {
                if menu_check(ui, &tc, self.split_on, lk("c.split", "Ctrl+\\")).clicked() { self.toggle_split(); ui.close(); }
                if menu_check(ui, &tc, self.term.open, lk("c.terminal", "Ctrl+`")).clicked() { self.term.open = !self.term.open; }
                if menu_check(ui, &tc, self.ai.open, lk("c.ai-chat", "Ctrl+Alt+L")).clicked() { self.ai_toggle(); ui.close(); }
                if menu_check(ui, &tc, self.md_preview, lk("c.md-preview", "Ctrl+Shift+V")).clicked() { self.md_preview = !self.md_preview; }
                if menu_check(ui, &tc, self.cfg.view.md_overlay, tr("menu.md-overlay")).clicked() { self.cfg.view.md_overlay = !self.cfg.view.md_overlay; }
                if menu_check(ui, &tc, self.dark, lk("c.dark", "")).clicked() { self.dark = !self.dark; ui.close(); }
                if menu_check(ui, &tc, self.highlight, lk("c.highlight", "")).clicked() { self.highlight = !self.highlight; }
                if menu_check(ui, &tc, self.wrap, lk("c.wrap", "Alt+Z")).clicked() { self.wrap = !self.wrap; }
                ui.menu_button(trz("折行方式"), |ui| {
                    let col = self.cfg.view.wrap_col;
                    if menu_check(ui, &tc, !self.wrap, Self::app_label(KApp::WrapNone, "不折行")).clicked() { self.run_app(KApp::WrapNone, ctx); ui.close(); }
                    if menu_check(ui, &tc, self.wrap && col == 0, Self::app_label(KApp::WrapWindow, "按窗口宽度折行")).clicked() { self.run_app(KApp::WrapWindow, ctx); ui.close(); }
                    let label = if col > 0 { format!("{} ({col})", Self::app_label(KApp::WrapCol, "按指定列数折行…")) } else { Self::app_label(KApp::WrapCol, "按指定列数折行…") };
                    if menu_check(ui, &tc, self.wrap && col > 0, label).clicked() { self.run_app(KApp::WrapCol, ctx); ui.close(); }
                    ui.separator();
                    ui.add_enabled(col > 0, crate::ckbox::Check::new(&mut self.cfg.view.kinsoku, trz("行头 / 行尾禁则 (指定列数折行时)")));
                    ui.add_enabled(col > 0 && self.cfg.view.kinsoku, crate::ckbox::Check::new(&mut self.cfg.view.hang, trz("句读挂在行尾 (ぶら下げ)")));
                });
                if menu_check(ui, &tc, self.side.open, lk("c.sidebar", "Ctrl+B")).clicked() { self.side.open = !self.side.open; }
                for (a, zh, on) in [(KApp::SideCollapse, "折叠左侧栏", self.cfg.view.side_collapsed), (KApp::RightCollapse, "折叠右侧面板", self.cfg.view.right_collapsed), (KApp::BottomCollapse, "折叠底部面板", self.cfg.view.bottom_collapsed)] {
                    if menu_check(ui, &tc, on, Self::app_label(a, zh)).clicked() { self.run_app(a, ctx); ui.close(); }
                }
                if menu_check(ui, &tc, self.cfg.system_title_bar, Self::app_label(KApp::SystemTitleBar, "使用系统标题栏")).clicked() { self.run_app(KApp::SystemTitleBar, ctx); ui.close(); }
                ui.separator();
                if menu_check(ui, &tc, self.cfg.view.line_numbers, lk("c.line-numbers", "")).clicked() { self.cfg.view.line_numbers = !self.cfg.view.line_numbers; }
                if menu_check(ui, &tc, self.cfg.view.status_bar, lk("c.status-bar", "")).clicked() { self.cfg.view.status_bar = !self.cfg.view.status_bar; }
                if menu_check(ui, &tc, self.cfg.view.whitespace, lk("c.whitespace", "")).clicked() { self.cfg.view.whitespace = !self.cfg.view.whitespace; }
                if menu_check(ui, &tc, self.cfg.view.ruler, lk("c.ruler", "")).clicked() { self.cfg.view.ruler = !self.cfg.view.ruler; }
                if menu_check(ui, &tc, self.cfg.view.cur_line, lk("c.cur-line", "")).clicked() { self.cfg.view.cur_line = !self.cfg.view.cur_line; }
                if menu_check(ui, &tc, self.cfg.view.minimap, lk("c.minimap", "")).clicked() { self.cfg.view.minimap = !self.cfg.view.minimap; }
                if menu_check(ui, &tc, self.cfg.view.toolbar, lk("c.toolbar", "")).clicked() { self.cfg.view.toolbar = !self.cfg.view.toolbar; }
                if menu_check(ui, &tc, self.cfg.view.funckey, lk("c.funckey", "")).clicked() { self.cfg.view.funckey = !self.cfg.view.funckey; }
                if menu_check(ui, &tc, self.cfg.view.rail, lk("c.rail", "")).clicked() { self.cfg.view.rail = !self.cfg.view.rail; }
                if menuui::mbtn(ui, lk("c.full-mode", "")).clicked() { self.set_full(); ui.close(); }
                if menuui::mbtn(ui, lk("c.minimal-mode", "")).clicked() { self.set_minimal(); ui.close(); }
                ui.separator();
                ui.menu_button(tr("sub.tabwidth"), |ui| {
                    for w in [2usize, 3, 4, 8] {
                        if menu_check(ui, &tc, self.tab == w, format!("{w}")).clicked() { self.tab = w; ui.close(); }
                    }
                });
                if menuui::mbtn(ui, lk("c.zoom-in", "Ctrl+=")).clicked() { self.font_px = (self.font_px + 1.0).min(40.0); }
                if menuui::mbtn(ui, lk("c.zoom-out", "Ctrl+-")).clicked() { self.font_px = (self.font_px - 1.0).max(8.0); }
                if menuui::mbtn(ui, lk("c.zoom-reset", "Ctrl+0")).clicked() { self.font_px = DEFAULT_FONT_PX; }
            }));
            menuui::top(ui, &mut mids, "menu.tool", menuui::menu_title(&tc, tr("menu.tool"), mn("menu.tool")), |ui| menuui::scroll(ui, |ui| self.tool_items(ui, ctx)));
            menuui::top(ui, &mut mids, "menu.help", menuui::menu_title(&tc, tr("menu.help"), mn("menu.help")), |ui| {
                if menuui::mbtn(ui, lk("c.palette", "")).clicked() { ui.close(); self.run_app(KApp::Palette, ctx); }
                if menuui::mbtn(ui, lk("c.key-list", "")).clicked() { ui.close(); self.cmdui.open_keylist(); }
                if menuui::mbtn(ui, lk("c.key-config", "")).clicked() { ui.close(); self.cmdui.open_keycfg(); }
                ui.separator();
                for (zh, a) in [("帮助手册", KApp::Help), ("关键字帮助检索…", KApp::HelpSearch), ("关键字帮助: 光标处的词", KApp::ExtHelp)] {
                    if ui.button(Self::app_label(a, zh)).clicked() { ui.close(); self.run_app(a, ctx); }
                }
                if menu_check(ui, &tc, self.cfg.view.key_search, Self::app_label(KApp::KeySearch, "关键字帮助自动显示")).clicked() { ui.close(); self.run_app(KApp::KeySearch, ctx); }
                ui.separator();
                if menuui::mbtn(ui, lk("c.check-update", "")).clicked() { ui.close(); self.upd_open(); }
                if menuui::mbtn(ui, lk("c.about", "")).clicked() { ui.close(); self.about_open = true; }
            });
        });
        self.sys.menu_ids = mids;
    }

    /// 宏菜单的内容 (「宏」和「工具 > 宏」共用)
    fn macro_items(&mut self, ui: &mut egui::Ui) {
        let rec = self.cur().rec.is_some();
        if menuui::mbtn(ui, lk(if rec { "c.macro-stop" } else { "c.macro-start" }, "Ctrl+Shift+M")).clicked() { self.cur().macro_toggle(); ui.close(); }
        if menuui::mbtn(ui, lk("c.macro-play", "Ctrl+Shift+L")).clicked() { self.cur().macro_play(); ui.close(); }
        ui.separator();
        if ui.button(trz("保存键盘宏…")).clicked() { ui.close(); self.macro_save(); }
        if ui.button(trz("载入键盘宏…")).clicked() { ui.close(); self.macro_load(); }
        ui.separator();
        let rec_mac = self.cur().rec.is_some();
        if ui.button(trz(if rec_mac { "停止录制并生成 .mac" } else { "宏录制 (.mac)" })).clicked() { ui.close(); self.macro_rec_mac(); }
        if ui.button(trz("把刚录的键盘宏转成 .mac")).clicked() { ui.close(); self.macro_to_mac(); }
        ui.separator();
        let ctx = ui.ctx().clone();
        let mut run_slot: Option<usize> = None;
        ui.menu_button(trz("宏登记"), |ui| {
            let mut any = false;
            for (i, s) in self.cfg.macro_slots.iter().enumerate().filter(|(_, s)| !s.is_empty()) {
                any = true;
                if ui.button(format!("{}: {}", i + 1, if s.name.is_empty() { "(无名)" } else { &s.name })).clicked() { run_slot = Some(i); ui.close(); }
            }
            if !any { ui.label(tr("ui.none")); }
            ui.separator();
            if ui.button(trz("把当前文档 / 选区登记为宏")).clicked() { self.register_macro_doc(); ui.close(); }
        });
        if let Some(i) = run_slot { self.run_macro_slot(i, &ctx); }
        if menuui::mbtn(ui, lk("c.macro-run-doc", "")).clicked() { ui.close(); self.run_mac_doc(&ctx); }
        if menuui::mbtn(ui, lk("c.macro-run-file", "")).clicked() { ui.close(); self.run_mac_file(&ctx); }
        if menuui::mbtn(ui, lk("c.macro-run-ppa", "")).clicked() { ui.close(); self.run_ppa_doc(); }
    }

    /// 语法类型子菜单的内容 (「设置」和「工具」共用)
    fn syntax_items(&mut self, ui: &mut egui::Ui) {
        let tc = self.tc;
        use menuui::menu_check;
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].syntax.clone();
        if menu_check(ui, &tc, cur.is_none(), trz("自动 (按扩展名)")).clicked() { self.docs[i].syntax = None; ui.close(); }
        ui.separator();
        let names: Vec<String> = self.registry.all().iter().map(|t| t.name.clone()).collect();
        for n in names {
            if menu_check(ui, &tc, cur.as_deref() == Some(n.as_str()), &n).clicked() { self.docs[i].syntax = Some(n); ui.close(); }
        }
    }

    /// 「工具」菜单 (Tauri `MENU_DEF` 的 tool): 大纲 / 语法 / 类型别设定 / 设置导出导入 / 自动备份 / 文件加锁 / 宏 / 插件 / 执行外部命令,
    /// 每项接到 ide-fast 已有的功能 (Tauri 的 SQL*Plus 与「历史管理」ide-fast 没有对应功能, 不列)
    fn tool_items(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        if ui.button(tr("sd.outline")).clicked() {
            self.side.open = true;
            self.side.tab = crate::sidebar::Tab::Outline;
            self.expand_dock(crate::panels::Dock::Side);
            ui.close();
        }
        ui.menu_button(trz("语法类型"), |ui| self.syntax_items(ui));
        if menuui::mbtn(ui, lk("c.common-config", "")).clicked() { self.open_prefs(); ui.close(); }
        if menuui::mbtn(ui, lk("c.export-config", "")).clicked() { ui.close(); self.export_settings(); }
        if menuui::mbtn(ui, lk("c.import-config", "")).clicked() { ui.close(); self.import_settings(); }
        ui.separator();
        if ui.button(tr("c.open-autosave")).clicked() { ui.close(); self.open_autosave(); }
        if ui.button(trz("立即备份")).clicked() { ui.close(); self.backup_now(); }
        if ui.button(trz("文件加锁…")).clicked() { self.open_prefs(); ui.close(); }
        ui.separator();
        ui.menu_button(tr("menu.macro"), |ui| self.macro_items(ui));
        ui.menu_button(tr("menu.plugin"), |ui| self.plugin_panel(ui));
        ui.separator();
        if menuui::mbtn(ui, Self::app_label(KApp::ExecCmd, "执行外部命令 (终端)")).clicked() { self.open_exec_terminal(); ui.close(); }
    }

    /// 把当前文档的文本当 Sakura 宏运行 (作用在上一份活动的另一个文档上; 只有一份就作用在自己上)
    fn run_mac_doc(&mut self, ctx: &egui::Context) {
        let src = self.cur().text_lf();
        let target = if self.docs.len() > 1 { (0..self.docs.len()).rev().find(|&i| i != self.active) } else { Some(self.active) };
        self.run_mac_src(&src, target.unwrap_or(self.active), ctx);
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

    /// 运行宏登记槽 `n` (0 起): Sakura 宏作用在活动文档上; PPA 脚本交给插件宿主
    fn run_macro_slot(&mut self, n: usize, ctx: &egui::Context) {
        let slot = self.cfg.macro_slots.get(n).cloned().unwrap_or_default();
        if slot.is_empty() {
            self.cur().tip = format!("宏 {} 是空的 (首选项 > 宏 里登记)", n + 1);
            return;
        }
        if slot.kind == "ppa" {
            let Some(row) = plugins::rows().into_iter().find(|r| r.id == "ppa" && r.enabled) else { self.cur().tip = tr("dlg.ppa-off").into(); return };
            let t = self.active.min(self.docs.len() - 1);
            self.plug.start("ppa", &row.perms, "run", "PPA", &self.docs[t], Some(serde_json::json!({ "script": slot.code })));
            self.docs[t].tip = trf("dlg.plugin-running", &["PPA"]);
        } else {
            let a = self.active;
            self.run_mac_src(&slot.code, a, ctx);
        }
    }

    /// 把当前文档 (有选区就只登记选区) 登记到第一个空宏槽 (同 Tauri `macro-register-doc`); .ppa 文档登记成 PPA, 其它当 Sakura 宏
    fn register_macro_doc(&mut self) {
        let (code, name, lower) = {
            let d = self.cur();
            let sel = crate::plugin_ui::selection_text(d);
            (if sel.is_empty() { d.text_lf() } else { sel }, d.name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&d.name).to_string(), d.name.to_lowercase())
        };
        if code.trim().is_empty() { self.cur().tip = "没有可登记的内容".into(); return; }
        if lower.ends_with(".js") { self.cur().tip = "高速版不运行 JS 宏, 没有登记".into(); return; }
        let kind = if lower.ends_with(".ppa") { "ppa" } else { "mac" };
        self.cur().tip = match crate::settings::register_macro(&mut self.cfg.macro_slots, &name, kind, &code) {
            Some(i) => format!("已登记到宏 {}: {name}", i + 1),
            None => format!("{} 个宏槽都满了 (首选项 > 宏 里清空一个)", crate::settings::MACRO_SLOTS),
        };
    }

    fn run_mac_file(&mut self, ctx: &egui::Context) {
        let Some(p) = rfd::FileDialog::new().add_filter("Sakura 宏", &["mac"]).pick_file() else { return };
        match editcore::document_io::read_doc(&p, None) {
            Ok(d) => { let a = self.active; self.run_mac_src(&d.content, a, ctx); }
            Err(e) => self.cur().tip = format!("读取宏失败: {e}"),
        }
    }

    /// 运行一份 Sakura 宏: 先整体预检 (有一步跑不了就一步都不执行), 再按顺序逐步执行 ——
    /// 只动文档的步骤在目标文档上跑, 窗口 / 文件 / 查找这类步骤按命令表里的命令转交 `run_entry` (作用在宏的目标文档上);
    /// 转交后活动标签可能变了 (新建、切换、关闭), 之后的步骤就跟着活动标签走。宏里的剪贴板最后同步回系统剪贴板
    pub(super) fn run_mac_src(&mut self, src: &str, target: usize, ctx: &egui::Context) {
        let last = self.docs.len() - 1;
        let steps = match crate::macros::plan_checked(src) {
            Ok(s) => s,
            Err(e) => { self.docs[target.min(last)].tip = format!("宏未执行: {e}"); return; }
        };
        let mut tid = self.docs[target.min(last)].id;
        let mut env = crate::macros::MacroEnv::new(self.tab, self.tools.clip.first().cloned().unwrap_or_default());
        let (mut done, mut failure) = (0usize, None::<String>);
        for s in &steps {
            let Some(i) = self.docs.iter().position(|d| d.id == tid) else { failure = Some("宏的目标文档已经关了".into()); break };
            match crate::macros::run_step(&mut self.docs[i], s, &mut env) {
                Ok(None) => done += 1,
                Ok(Some(id)) => {
                    let Some(idx) = chrome::index_of(id) else { failure = Some(format!("命令表里没有 {id}")); break };
                    self.active = i;
                    self.run_entry(idx, ctx);
                    tid = self.docs[self.active.min(self.docs.len() - 1)].id;
                    done += 1;
                }
                Err(e) => { failure = Some(e); break; }
            }
        }
        if env.clip_changed { self.put_clip(ctx, env.clip.clone()); }
        let i = self.docs.iter().position(|d| d.id == tid).unwrap_or(self.active.min(self.docs.len() - 1));
        self.docs[i].tip = match failure {
            None => format!("宏已执行 ({done} 步)"),
            Some(e) => format!("宏在第 {} 步停下 (已执行 {done} 步): {e}", done + 1),
        };
    }

    /// 终端面板: 输出 + 输入行 (Enter 执行, ↑↓ 翻历史)
    fn term_ui(&mut self, ui: &mut egui::Ui) {
        Self::bottom_toolbar(self.tc.fg_dim, ui, |ui| {
            if ui.button(trz("清屏")).clicked() { self.term.clear(); }
            if ui.button(trz("重启")).tip(trz("结束当前 shell, 下一条命令重新开一个 (Ctrl+C)")).clicked() { self.term.restart(); }
            ui.check(&mut self.term.persistent, trz("常驻会话"));
            ui.check(&mut self.term_stdin, trz("选区作标准输入"));
            ui.check(&mut self.term_replace, trz("输出替换选区"));
            if self.term.running() { ui.label(trz("运行中…")); }
            if ui.button("×").clicked() { self.term.open = false; }
        });
        let h = (ui.available_height() - 28.0).max(30.0);
        self.term.set_scrollback(self.cfg.term_scrollback);
        let (tfg, tbg) = (self.cfg.term_fg.map(|c| Color32::from_rgb(c[0], c[1], c[2])), self.cfg.term_bg.map(|c| Color32::from_rgb(c[0], c[1], c[2])));
        let tpx = self.cfg.term_font_px;
        egui::Frame::NONE.fill(tbg.unwrap_or(Color32::TRANSPARENT)).show(ui, |ui| {
            crate::scrollx::show(ui, egui::ScrollArea::vertical().max_height(h).stick_to_bottom(true).auto_shrink([false, false]), |ui| {
                let shown = self.term.display();
                if shown.trim().is_empty() && !self.term.running() {
                    // 没有会话输出时的空状态 (Tauri: No terminal session / 暂无终端会话)
                    let dim = self.tc.fg_dim;
                    ui.add_space((h * 0.3).max(4.0));
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("No terminal session — 暂无终端会话").monospace().size(tpx).color(dim));
                        ui.label(egui::RichText::new(trz("输入命令按 Enter 启动 shell, 或点「+ 新建终端」")).monospace().size(tpx).color(dim));
                    });
                    return;
                }
                let mut t = egui::RichText::new(shown).monospace().size(tpx);
                if let Some(c) = tfg { t = t.color(c); }
                ui.add(egui::Label::new(t).selectable(true));
            });
        });
        let r = ui.add(egui::TextEdit::singleline(&mut self.term.input).font(egui::TextStyle::Monospace).hint_text(trz("输入命令, Enter 执行")).desired_width(f32::INFINITY));
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
    fn md_ui(&mut self, ui: &mut egui::Ui, overlay: bool) {
        let ai = self.active.min(self.docs.len() - 1);
        let key = (self.docs[ai].id, self.docs[ai].rev);
        if self.md_parsed.as_ref().map(|p| p.0) != Some(key) {
            self.md_parsed = Some((key, crate::markdown::parse(&self.docs[ai].lines)));
        }
        let blocks = self.md_parsed.take().expect("刚刚填好");
        if self.md_cache.edit.as_ref().is_some_and(|e| e.doc != self.docs[ai].id || e.rev != self.docs[ai].rev) { self.md_cache.edit = None; } // 文档在别处改过: 放弃就地编辑
        self.md10.sync_theme(self.dark, &self.theme_id);
        if !overlay { self.md_toolbar(ui); }
        self.md_poll_fences(&ui.ctx().clone());
        let fence_langs = self.md10.langs();
        let theme = self.tc.syntax_theme();
        let base = self.docs[ai].path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
        let cx = crate::md_view::MdCtx { base: base.as_deref(), registry: &self.registry, theme: &theme, tc: self.tc, fence_langs: &fence_langs, fences: &self.md10.fences };
        let ev = crate::md_view::show(ui, &blocks.1, &cx, &mut self.md_cache, self.md_scroll_to.take(), if overlay { crate::md_view::MdOpts::OVERLAY } else { crate::md_view::MdOpts::DOCKED });
        if let Some(l) = ev.jump { self.docs[ai].goto(l, 0); }
        self.md_handle(ai, &blocks.1, &ev);
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
    fn wrap_nav(&mut self, ctx: &egui::Context, font: &FontId, wrap_w: f32, tab: usize, kcols: Wrap, key: Key) {
        use egui::text::CCursor;
        let ai = self.active.min(self.docs.len() - 1);
        let (r, c) = (self.docs[ai].cursor[0], self.docs[ai].cursor[1]);
        let layout = |ctx: &egui::Context, line: &str| {
            let (disp, map) = expand_wrap(line, tab, kcols);
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
        self.menu_mnemonics(ctx); // Alt+助记字母先于命令键 (同 Tauri: 在捕获阶段就拦下; Alt+P 开「插件」菜单而不是「上一个位置」)
        let cmd = |ctx: &egui::Context, m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        for (idx, m, k) in keymap::bound() {
            if keymap::entries()[idx].needs_diff && self.diff.is_none() { continue; }
            if cmd(ctx, m, k) { self.run_entry(idx, ctx); }
        }
        // Esc: 先停掉正在跑的 Grep, 再关查找栏, 最后才是清矩形选区 / 多光标
        // 菜单 / 弹出层开着时 Esc 只关它 (这里若先把 Esc 吃掉, 菜单永远收不起来)
        if ctx.input(|i| i.key_pressed(Key::Escape)) && egui::Popup::is_any_open(ctx) {
            egui::Popup::close_all(ctx);
            cmd(ctx, Modifiers::NONE, Key::Escape);
        } else if self.md_cache.edit.is_some() {
            // 预览里就地编辑开着: Esc 留给编辑框 (取消), 这里不吃
        } else if cmd(ctx, Modifiers::NONE, Key::Escape) {
            if self.text_drag.as_ref().is_some_and(|t| t.started && !t.cancelled) { if let Some(t) = self.text_drag.as_mut() { t.cancelled = true; t.drop = None; } self.cur().tip = trz("已取消拖放").into(); }
            else if self.side.grep.running() { self.side.grep.cancel(); self.side.grep.msg = format!("已停止, {} 处命中", self.side.grep.results.len()); }
            else if self.find.open { self.find.open = false; }
            else if self.cur().sel_mode { self.cur().sel_mode = false; self.cur().tip = trz("范围选择模式: 关").into(); }
            else { let d = self.cur(); d.box_sel = None; d.extra.clear(); }
        }
    }

    /// Alt+助记字母: 打开对应的顶层菜单 (和 Tauri 一样先于命令键: 命令绑了同一个 Alt+字母也让给菜单)。另外所有「只按 Alt + 字母」的组合都不让字母漏进文档。
    /// 打开后的上下键 / 回车 / Esc 走 egui 菜单自带的焦点导航和关闭逻辑
    fn menu_mnemonics(&mut self, ctx: &egui::Context) {
        let keys: Vec<&str> = self.sys.menu_ids.iter().map(|m| m.0).collect();
        let hit = ctx.input_mut(|i| {
            let m = i.modifiers;
            let k = menuui::MNEMONIC_KEYS.iter().copied().find(|k| i.key_pressed(*k))?;
            // Alt+字母 winit 还会补一条 Text 事件: 不吃掉的话字母会被打进文档 (命令键 Alt+Z 等同理, 这里统一清)
            if m.alt && !m.ctrl && !m.command {
                i.events.retain(|e| !matches!(e, egui::Event::Text(t) if t.eq_ignore_ascii_case(k.name())));
            }
            let n = menuui::menu_for_key(&keys, m, k)?;
            i.consume_key(m, k);
            Some(n)
        });
        if let Some(n) = hit {
            let id = self.sys.menu_ids[n].1;
            egui::Popup::close_all(ctx);
            egui::Popup::open_id(ctx, id);
            // 编辑区占着焦点而且锁着方向键: 放掉它; 之后 ↑ ↓ ← → 由 raw_input_hook 截下、在 `menu_arrows` 里按状态机处理 (第一次 ↓ 高亮第一项)
            ctx.memory_mut(|m| if let Some(f) = m.focused() { m.surrender_focus(f); });
            self.sys.nav_q.clear();
        }
        if let Some((n, k)) = self.sys.bar_open.take() {
            if let Some(&(_, id)) = self.sys.menu_ids.get(n) {
                egui::Popup::close_all(ctx);
                egui::Popup::open_id(ctx, id);
                ctx.memory_mut(|m| if let Some(f) = m.focused() { m.surrender_focus(f); });
                self.sys.nav_q.clear();
                self.sys.nav_q.push(k);
            }
        }
        self.menu_arrows(ctx);
    }

    /// 单按 Alt = 把焦点放到菜单栏 (照 Tauri `altArmed`): 高亮第一个顶层菜单, ← → 切换, ↓ / Enter / 空格 展开, Esc 或再按 Alt 退出;
    /// 菜单已展开时再单按 Alt 就关菜单。菜单栏焦点开着时这些键在这里吃掉 (编辑区不会同时移动光标); 点鼠标 / 按别的键就退出焦点。
    /// 在 `raw_input_hook` 里调用: 此时拿得到未经 egui 处理的原始事件和修饰键
    fn bar_focus_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput, menu_open: bool) {
        let n = self.sys.menu_ids.len();
        if self.sys.alt_tap.feed(&raw.events) && n > 0 && !self.cmdui.capturing() {
            if menu_open {
                egui::Popup::close_all(ctx);
                self.sys.bar_focus = None;
            } else if self.sys.bar_focus.take().is_none() {
                self.sys.bar_focus = Some(0);
                ctx.memory_mut(|mem| if let Some(f) = mem.focused() { mem.surrender_focus(f); });
            }
            ctx.request_repaint();
            return;
        }
        let Some(cur) = self.sys.bar_focus else { return };
        if menu_open || n == 0 { self.sys.bar_focus = None; return; }
        let cur = cur.min(n - 1);
        let mnemonics: Vec<char> = self.sys.menu_ids.iter().map(|x| menuui::mnemonic(x.0)).collect();
        let mut state = Some(cur);
        let mut open = None;
        let mut eat_text = false;
        raw.events.retain(|e| match e {
            egui::Event::Text(_) if eat_text || state.is_some() => false,
            _ if state.is_none() => true,
            egui::Event::PointerButton { pressed: true, .. } | egui::Event::WindowFocused(false) => { state = None; true }
            egui::Event::Key { key, pressed, modifiers, .. } => {
                if matches!(key, Key::AltLeft | Key::AltRight) { return true; }
                // 菜单栏上直接按助记字母: 打开对应菜单 (Tauri 同)
                if modifiers.is_none() && key.name().len() == 1 {
                    if let Some(i) = mnemonics.iter().position(|c| key.name().eq_ignore_ascii_case(&c.to_string())) {
                        if *pressed { open = Some((i, menunav::NavKey::Down)); state = None; eat_text = true; }
                        return false;
                    }
                }
                match menunav::bar_key(state.unwrap_or(cur), n, *key, *modifiers) {
                    menunav::BarOut::Pass => { if *pressed { state = None; } true }
                    menunav::BarOut::Leave => { if *pressed { state = None; } false }
                    menunav::BarOut::Move(i) => { if *pressed { state = Some(i); } false }
                    menunav::BarOut::Open(i, k) => { if *pressed { open = Some((i, k)); state = None; eat_text = true; } false }
                }
            }
            _ => true,
        });
        self.sys.bar_focus = state;
        if open.is_some() { self.sys.bar_open = open; }
        ctx.request_repaint();
    }

    /// 当前开着的顶层菜单下标
    fn open_top_menu(&self, ctx: &egui::Context) -> Option<usize> {
        self.sys.menu_ids.iter().position(|m| egui::Popup::is_id_open(ctx, m.1))
    }

    /// 菜单开着时的方向键: ← → 切顶层菜单, ↑ ↓ 在项间移动 (焦点即高亮), Enter / Esc 走 egui 自带逻辑
    fn menu_arrows(&mut self, ctx: &egui::Context) {
        let keys = std::mem::take(&mut self.sys.nav_q);
        for k in keys {
            let Some(menu) = self.open_top_menu(ctx) else { break };
            let layer = egui::LayerId::new(egui::Order::Foreground, self.sys.menu_ids[menu].1);
            let mut items: Vec<(egui::Id, egui::Rect)> = ctx.viewport(|v| v.prev_pass.widgets.get_layer(layer).filter(|w| w.enabled && w.sense.is_focusable()).map(|w| (w.id, w.rect)).collect());
            items.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()).then(a.1.left().total_cmp(&b.1.left())));
            let focused = ctx.memory(|m| m.focused());
            let cur = items.iter().position(|i| Some(i.0) == focused);
            let (nm, ni) = menunav::step(menu, cur, self.sys.menu_ids.len(), items.len(), k);
            if nm != menu {
                egui::Popup::open_id(ctx, self.sys.menu_ids[nm].1);
                ctx.memory_mut(|m| if let Some(f) = m.focused() { m.surrender_focus(f); });
            } else if let Some(i) = ni {
                let id = items[i].0;
                ctx.memory_mut(|m| m.request_focus(id));
            }
            ctx.request_repaint();
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
            KApp::PrintSetup => self.print_ui.setup_open = true,
            KApp::PrintPreview => self.print_ui.preview = Some(0),
            KApp::Browse => self.browse_current(),
            KApp::Props => self.props = Some(self.cur().properties()),
            KApp::NewWindow => self.new_window(),
            KApp::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            KApp::Undo => self.cur().undo(),
            KApp::Redo => self.cur().redo(),
            KApp::Cut => { if let Some(c) = self.cur().edit("cut", None) { self.put_clip(ctx, c); } }
            KApp::Copy => { if let Some(c) = self.cur().edit("copy", None) { self.put_clip(ctx, c); } }
            KApp::Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            KApp::SelectAll => self.cur().select_all(),
            KApp::Find => self.open_find(false),
            KApp::Replace => self.open_find(true),
            KApp::Grep => self.focus_grep(),
            KApp::MdStruct => self.md_struct_open(),
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
            KApp::SplitRows => self.toggle_split_kind(crate::split::SplitKind::Rows),
            KApp::SplitQuad => self.toggle_split_kind(crate::split::SplitKind::Quad),
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
            KApp::MacroRunDoc => self.run_mac_doc(ctx),
            KApp::MacroRunFile => self.run_mac_file(ctx),
            KApp::MacroRunPpa => self.run_ppa_doc(),
            KApp::Terminal => self.toggle_dock_open(crate::panels::Dock::Bottom),
            KApp::AiChat => self.ai_toggle(),
            KApp::MdPreview => self.md_preview = !self.md_preview,
            KApp::Dark => self.dark = !self.dark,
            KApp::Highlight => self.highlight = !self.highlight,
            KApp::Wrap => self.wrap = !self.wrap,
            KApp::Sidebar => self.toggle_dock_open(crate::panels::Dock::Side),
            KApp::SideCollapse => self.collapse_dock(crate::panels::Dock::Side),
            KApp::RightCollapse => self.collapse_dock(crate::panels::Dock::Right),
            KApp::BottomCollapse => self.collapse_dock(crate::panels::Dock::Bottom),
            KApp::SystemTitleBar => self.toggle_system_title_bar(),
            KApp::LineNumbers => self.cfg.view.line_numbers = !self.cfg.view.line_numbers,
            KApp::StatusBar => self.cfg.view.status_bar = !self.cfg.view.status_bar,
            KApp::Whitespace => self.cfg.view.whitespace = !self.cfg.view.whitespace,
            KApp::Ruler => self.cfg.view.ruler = !self.cfg.view.ruler,
            KApp::CurLine => self.cfg.view.cur_line = !self.cfg.view.cur_line,
            KApp::Minimap => self.cfg.view.minimap = !self.cfg.view.minimap,
            KApp::ToolbarToggle => self.cfg.view.toolbar = !self.cfg.view.toolbar,
            KApp::FunckeyToggle => self.cfg.view.funckey = !self.cfg.view.funckey,
            KApp::RailToggle => self.cfg.view.rail = !self.cfg.view.rail,
            KApp::MinimalMode => self.set_minimal(),
            KApp::FullMode => self.set_full(),
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
            KApp::CopyFname => self.copy_doc_name(ctx, false),
            KApp::CopyPath => self.copy_doc_name(ctx, true),
            KApp::SelParaPrev => self.cur().select_para(false),
            KApp::SelParaNext => self.cur().select_para(true),
            KApp::ModNext => self.step_modified(true),
            KApp::ModPrev => self.step_modified(false),
            KApp::MacroSave => self.macro_save(),
            KApp::MacroLoad => self.macro_load(),
            KApp::MacroRecMac => self.macro_rec_mac(),
            KApp::FuncNext => self.step_function(true),
            KApp::CopyTag => self.copy_tag(ctx),
            KApp::GroupNext => self.group_switch(true),
            KApp::GroupPrev => self.group_switch(false),
            KApp::TabSeparate => self.group_separate(),
            KApp::TabJointNext => self.group_joint(true),
            KApp::TabJointPrev => self.group_joint(false),
            KApp::BindWindow => self.group_bind_all(),
            KApp::GroupClose => self.group_close(),
            KApp::SpaceIndent => self.space_indent_cmd(false),
            KApp::SpaceUnindent => self.space_indent_cmd(true),
            KApp::CurCenter => self.center_cursor(),
            KApp::BackupNow => self.backup_now(),
            KApp::Completion => self.show_completion(),
            KApp::MacroRegister => self.register_macro_doc(),
            KApp::MacroSlot(n) => self.run_macro_slot(n as usize, ctx),
            KApp::GotoDef => self.goto_definition(),
            KApp::ExecCmd => self.open_exec_terminal(),
            KApp::SelCountMode => self.cycle_sel_count(),
            KApp::FindMarks => self.toggle_find_marks(),
            KApp::FindStart => self.jump_find_start(),
            KApp::CopyWrapped => self.copy_wrapped(ctx),
            KApp::OpenHc => self.open_counterpart(),
            KApp::ModNextSel => self.step_modified_sel(true),
            KApp::ModPrevSel => self.step_modified_sel(false),
            KApp::FuncPrev => self.step_function(false),
            KApp::MacroToMac => self.macro_to_mac(),
            KApp::About => self.about_open = true,
            KApp::CheckUpdate => self.upd_open(),
            KApp::ColDelete | KApp::ColReplace | KApp::SelMode | KApp::TagJump | KApp::TagBack | KApp::TagKeyword | KApp::TagsMake | KApp::IsFwd | KApp::IsBack | KApp::IsRegexFwd | KApp::IsRegexBack | KApp::IsMigemoFwd | KApp::IsMigemoBack
            | KApp::Base64Decode | KApp::UuDecode | KApp::CopyHtml | KApp::CopyHtmlLn | KApp::ClipHistory | KApp::CtrlCodes | KApp::InsDate | KApp::InsTime | KApp::InsStamp | KApp::StampCfg
            | KApp::WrapCol | KApp::WrapWindow | KApp::WrapNone => self.run_extra(a, ctx),
            KApp::WindowList | KApp::PickRecent | KApp::PickFolders | KApp::PickFavorite | KApp::InsRecentFile | KApp::InsRecentDir | KApp::PickCharset | KApp::PickReopen
            | KApp::Help | KApp::HelpSearch | KApp::ExtHelp | KApp::KeySearch | KApp::PaneOutline | KApp::PaneFiles => self.run_pick(a),
            KApp::Palette => {
                self.cmdui.toggle_palette();
                self.pal_plugins = Self::plugin_cmds();
            }
        }
    }

    /// 复制当前文档的文件名 / 完整路径 (没保存过的文档没有路径)
    fn copy_doc_name(&mut self, ctx: &egui::Context, full: bool) {
        let (name, path) = { let d = self.cur(); (d.name.clone(), d.path.clone()) };
        let text = if full { path.map(|p| p.display().to_string()) } else { Some(name) };
        self.cur().tip = match text {
            Some(t) => { self.put_clip(ctx, t.clone()); trf("dlg.copied-name", &[&t]) }
            None => tr("dlg.no-path").into(),
        };
    }

    /// (选择) 下一处 / 上一处改动: 选区从原来的位置拉到改动块首
    fn step_modified_sel(&mut self, fwd: bool) {
        self.jump_mark();
        if !self.cur().step_changed_sel(fwd) { self.cur().tip = tr("dlg.no-more-changes").into(); }
    }

    fn step_modified(&mut self, fwd: bool) {
        self.jump_mark();
        if !self.cur().step_changed(fwd) { self.cur().tip = tr("dlg.no-more-changes").into(); }
    }

    /// 把当前文档最近录下的键盘宏存成 JSON 文件
    fn macro_save(&mut self) {
        let steps = self.cur().last_macro.clone();
        if steps.is_empty() { self.cur().tip = tr("dlg.macro-empty").into(); return; }
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("key-macro.json").save_file() else { return };
        let tip = match std::fs::write(&path, crate::macros::macro_to_json(&steps)) { Ok(()) => trf("dlg.macro-saved", &[&path.display().to_string()]), Err(e) => e.to_string() };
        self.cur().tip = tip;
    }

    /// 宏录制 (.mac), 同 Tauri 版: 没在录就开始录; 正在录就停下, 并把刚录的转成 Sakura `.mac` 开成新标签
    fn macro_rec_mac(&mut self) {
        let recording = self.cur().rec.is_some();
        self.cur().macro_toggle();
        if recording {
            self.macro_to_mac();
        } else {
            self.cur().tip = "● 录制宏 (.mac): 再执行一次「宏录制 (.mac)」停止并生成".into();
        }
    }

    /// 把当前标签最近录的键盘宏 (`last_macro`) 转成 Sakura `.mac`, 开成一个未保存的新标签; 没有对应 `S_` 函数的步骤列在提示里
    fn macro_to_mac(&mut self) {
        let steps = self.cur().last_macro.clone();
        if steps.is_empty() { self.cur().tip = tr("dlg.macro-empty").into(); return; }
        let out = crate::macros::rec_to_mac(&steps);
        let (text, n) = crate::macros::mac_document(&out);
        let skipped = if out.skipped.is_empty() { String::new() } else { format!("; 没有对应 S_ 函数 (未写入): {}", out.skipped.join(", ")) };
        if n == 0 { self.cur().tip = format!("没有可写成 .mac 的操作{skipped}"); return; }
        let id = self.id();
        let mut d = Doc::untitled(id, 0);
        d.name = "录制的宏.mac".into();
        d.lines = text.trim_end_matches('\n').split('\n').map(str::to_string).collect();
        d.dirty = true;
        d.tip = format!("已生成 {n} 条宏语句{skipped} — 保存为 .mac 后可用「运行当前文档 (Sakura .mac)」回放");
        self.docs.push(d);
        self.active = self.docs.len() - 1;
    }

    /// 从 JSON 文件载入键盘宏 (文件大小先封顶, 内容整份校验过才替换)
    fn macro_load(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() else { return };
        let res = match std::fs::metadata(&path) {
            Ok(m) if m.len() > 8 * 1024 * 1024 => Err("文件超过 8 MB".to_string()),
            Err(e) => Err(e.to_string()),
            Ok(_) => std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| crate::macros::macro_from_json(&t)),
        };
        match res {
            Ok(steps) => { let n = steps.len().to_string(); let d = self.cur(); d.last_macro = steps; d.tip = trf("dlg.macro-loaded", &[&n]); }
            Err(e) => self.cur().tip = trf("dlg.macro-bad", &[&e]),
        }
    }

    /// 启动时重新打开上次的文件 (只在没有命令行文件时): 路径过一遍 `session_path_ok`, 不存在 / 超过大文件阈值的跳过 (启动时不弹确认框)
    fn restore_session(&mut self, st: &Settings) {
        let limit = st.max_file_mb as u64 * 1024 * 1024;
        let mut active = None;
        for (i, p) in st.session.iter().enumerate() {
            let path = std::path::Path::new(p);
            if !crate::settings::session_path_ok(p) || !std::fs::metadata(path).map(|m| m.is_file() && m.len() <= limit).unwrap_or(false) { continue; }
            self.open_path(path);
            if i == st.session_active { active = Some(self.active); }
        }
        if let Some(a) = active.filter(|a| *a < self.docs.len()) { self.active = a; }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new(tr("dlg.about-title")).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.strong(format!("IDE1.0 Fast {}", env!("CARGO_PKG_VERSION")));
            ui.label(tr("ui.about"));
        });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) { self.about_open = false; }
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
        if self.about_open { self.about_window(ctx); }
        self.gh_window(ctx);
        self.upd_window(ctx);
        self.ai_windows(ctx);
    }

    /// 编辑区: 输入事件 -> doc; 然后只画看得见的行
    /// 一栏编辑区: 临时把 active 换成这栏的文档, 画完换回; 点击这栏 = 它拿到焦点
    fn editor_pane(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, find_has_focus: bool, pane: usize) {
        if !self.split_on {
            self.pane_keys = true;
            return self.editor(ui, ctx, find_has_focus);
        }
        let doc = self.panes.docs[pane].min(self.docs.len() - 1);
        let keep = self.active;
        self.active = doc;
        self.pane_keys = self.panes.focus == pane;
        // 每个窗格自己的 id 空间 (四分割时同一个 Ui 下并排的窗格否则会撞 id)
        ui.push_id(("pane", pane), |ui| self.editor(ui, ctx, find_has_focus));
        self.active = keep;
        if ctx.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            self.panes.focus = pane;
        }
    }

    /// 完整界面 (Tauri 版的布局): 活动栏 + 左侧栏 + 标签栏 + 状态栏; 折叠状态清掉
    fn set_full(&mut self) {
        self.side.open = true;
        self.cfg.view.side_collapsed = false;
        self.cfg.view.status_bar = true;
        self.cfg.view.rail = true;
        self.cfg.view.ruler = true;
    }

    /// 极简布局 (Sakura 风格的单个文本框): 收起侧栏 / 终端 / 预览 / 分割, 只留编辑区和状态栏
    fn set_minimal(&mut self) {
        self.side.open = false;
        self.term.open = false;
        self.md_preview = false;
        self.end_split();
        self.cfg.view.status_bar = true;
        self.cfg.view.minimap = false;
        self.cfg.view.ruler = false;
        (self.cfg.view.toolbar, self.cfg.view.funckey, self.cfg.view.rail) = (false, false, false);
    }

    /// 选主题插件 (空 id = 回到内置深 / 浅色)
    fn set_theme_plugin(&mut self, id: &str) {
        self.theme_id = id.to_string();
        let spec = plugins::rows().into_iter().find(|r| r.id == id && r.enabled).and_then(|r| r.theme);
        self.sys.theme_glass = spec.as_ref().is_some_and(|t| t.translucent);
        // 玻璃主题声明 luminance=dynamic 时, 文字色按系统「应用主题」明暗取 (一次读取, 不轮询)
        let spec = spec.map(|mut t| {
            if t.translucent { plugins::apply_dynamic_luminance(&mut t.vars, editcore::theme_luminance::detect()); }
            t
        });
        self.theme_visuals = spec.as_ref().map(|t| plugins::visuals_glass(&t.vars, t.translucent));
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
                self.md10.invalidate();
                match plugins::install_zip(&bytes, &perms) { Ok(m) => m, Err(e) => e }
            }
        };
        self.cur().tip = msg;
    }

    /// 命令表 -> 菜单: 空 group 的直接放, 其余按 group 分成子菜单 (首次出现的顺序)
    fn cmd_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let tc = self.tc;
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
                    let clicked = match checked { Some(on) => menuui::menu_check(ui, &tc, on, text).clicked(), None => ui.button(text).clicked() };
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
            Action::Edit(n) => { if let Some(c) = self.cur().edit(n, None) { self.put_clip(ctx, c); } }
            Action::Transform(k) => self.cur().transform_sel(k, tab),
            Action::Lines(k) => self.cur().lines_op(k),
            Action::Indent => self.cur().indent(false, tab),
            Action::Outdent => self.cur().indent(true, tab),
            Action::ToggleComment => {
                let mark = { let i = self.active.min(self.docs.len() - 1); self.type_of(&self.docs[i]).and_then(|t| t.line_comment.clone()) };
                match mark {
                    Some(mark) => { self.cur().edit("toggle-line-comment", Some(mark)); }
                    None => self.cur().tip = "这个类型没有行注释符".into(),
                }
            }
            Action::GotoLine => { self.goto_open = true; self.goto_text.clear(); }
            Action::CodeCnv(id) => self.run_codecnv(id),
            Action::ToggleOverwrite => { let d = self.cur(); d.overwrite = !d.overwrite; d.tip = if d.overwrite { "改写模式".into() } else { "插入模式".into() }; }
            Action::ToggleReadonly => { let d = self.cur(); d.readonly = !d.readonly; d.tip = if d.readonly { "已设为只读".into() } else { "已取消只读".into() }; }
        }
    }

    fn goto_window(&mut self, ctx: &egui::Context) {
        let mut go: Option<usize> = None;
        let mut open = self.goto_open;
        egui::Window::new(tr("dlg.goto-title")).open(&mut open).collapsible(false).resizable(false).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
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

    fn toggle_split(&mut self) { self.toggle_split_kind(crate::split::SplitKind::Cols); }

    fn toggle_split_kind(&mut self, k: crate::split::SplitKind) {
        let (a, n) = (self.active.min(self.docs.len() - 1), self.docs.len());
        self.panes.toggle(k, a, n);
        self.split_on = self.panes.kind != crate::split::SplitKind::Off;
    }

    fn end_split(&mut self) {
        self.panes.kind = crate::split::SplitKind::Off;
        self.split_on = false;
    }

    fn editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, find_has_focus: bool) {
        let font = FontId::new(self.font_px, crate::fonts::editor_family());
        let (base_h, char_w) = ctx.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        // 行高倍数: 整行(含自动换行的每个显示行)都按这个高度排, 文字在行内居中
        let row_h = crate::view::row_height(base_h, self.cfg.line_height, self.font_px);
        let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
        let text_color = self.cfg.colors.text.map(rgb).unwrap_or_else(|| ui.visuals().text_color());
        let weak = self.tc.fg_dim; // 行号 / 折叠标记: --fg-muted
        let sel_bg = ui.visuals().selection.bg_fill;
        let max_cols = self.max_cols;
        let ai = self.active.min(self.docs.len() - 1);
        let (tab, wrap_pref, _) = self.eff(&self.docs[ai].name.clone());
        let theme = self.syntax_theme_full(); // 含首选项「字体与颜色」里的词法颜色覆盖
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
        let cfg: Option<TypeConfig> = if self.highlight { self.type_of(&self.docs[ai]).cloned() } else { None };
        let digits = format!("{}", self.docs[ai].lines.len()).len().max(3);
        let vw = self.cfg.view.clone();
        let tc = self.tc;
        // 行号栏: 12px 数字, 至少 40 宽 (Tauri `#gutter`), 正文左内边距 8 (`#ed-content padding 4px 8px`)
        let num_font = FontId::new((self.font_px * 12.0 / 13.0).max(8.0), crate::fonts::editor_family());
        let num_w = ctx.fonts_mut(|f| f.glyph_width(&num_font, '0'));
        let gutter = if vw.line_numbers { (digits as f32 * num_w + 16.0).max(40.0) } else { 18.0 };
        let pad = 8.0;
        // 自动换行: 每个逻辑行按可用宽度折成若干显示行, 行高不等 -> 记累计高度 (太长的文档不换行, 免得每次排版都要扫全篇)
        let wrap = wrap_pref && self.docs[ai].lines.len() <= 20000;
        let wcol = self.cfg.wrap_col_for(&self.docs[ai].name, self.cfg.view.wrap_col);
        let win_w = (ui.available_width() - gutter - pad * 2.0 - 16.0).max(char_w * 8.0);
        // 指定列数折行: 列宽 = 列数 × 半角字符宽, 窗口更窄时仍以窗口为准
        let wrap_w = if !wrap { f32::INFINITY } else if wcol > 0 { win_w.min((wcol + if self.cfg.view.hang && self.cfg.view.kinsoku { 2 } else { 0 }) as f32 * char_w + 1.0) } else { win_w };
        // 固定列数折行 + 禁则: 折行点自己算 (显示文字里插入换行); 0 = 交给 egui 按宽度折
        let kcols = if wrap && self.cfg.view.kinsoku { Wrap { cols: wcol, hang: self.cfg.view.hang } } else { Wrap::default() };
        let use_offs = wrap || !self.docs[ai].folds.is_empty();
        let offs: Vec<f32> = if use_offs {
            let fsig: usize = self.docs[ai].folds.iter().map(|f| f.0 * 31 + f.1).sum::<usize>() + self.docs[ai].folds.len();
            let key = (self.docs[ai].id, self.docs[ai].rev, wrap_w as i32 + fsig as i32 * 7, (self.font_px * 10.0) as i32 + (self.cfg.line_height * 1000.0) as i32 * 1000, tab + (wrap as usize) * 100 + kcols.cols * 1000 + kcols.hang as usize * 50);
            if self.wrap_cache.as_ref().map(|c| c.0) != Some(key) {
                let d = &self.docs[ai];
                let mut acc = Vec::with_capacity(d.lines.len() + 1);
                let mut y = 0.0f32;
                for (r, l) in d.lines.iter().enumerate() {
                    acc.push(y);
                    if d.is_hidden(r) { continue; }
                    if wrap {
                        let (disp, _) = expand_wrap(l, tab, kcols);
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

        let text_drop: Option<[usize; 2]> = self.text_drag.filter(|t| t.started).and_then(|t| t.drop);
        let mut drag_end: Option<Pos2> = None;
        let mut caret_screen: Option<Pos2> = None;
        // ---- 键盘 (查找框有焦点时不抢) ----
        let mut clip_out: Option<String> = None;
        let mut preedit_now = self.preedit.clone();
        let mut jump_row: Option<usize> = None; // Grep 结果文档里 Enter 的那一行
        let mut accept_completion = false;
        if !find_has_focus && self.pane_keys {
            let mut events = ctx.input(|i| i.events.clone());
            if self.sys.complete.is_some() {
                // 补全浮层开着: ↑ ↓ 换候选, Enter / Tab 接受, Esc 关闭 (这些键不再交给编辑区)
                let mut rest = Vec::with_capacity(events.len());
                for ev in events {
                    if let egui::Event::Key { key, pressed: true, modifiers, .. } = &ev {
                        if !modifiers.ctrl && !modifiers.command && !modifiers.alt && !modifiers.shift {
                            match key {
                                Key::ArrowDown => { self.complete_move(1); continue; }
                                Key::ArrowUp => { self.complete_move(-1); continue; }
                                Key::Enter | Key::Tab => { accept_completion = true; continue; }
                                Key::Escape => { self.sys.complete = None; continue; }
                                _ => {}
                            }
                        }
                    }
                    rest.push(ev);
                }
                events = rest;
            }
            if wrap {
                // 自动换行下 ↑ ↓ Home End 按显示行走 (按真实排版取位置); 带修饰键的仍交给 editcore
                let mut rest = Vec::with_capacity(events.len());
                for ev in events {
                    if let egui::Event::Key { key, pressed: true, modifiers, .. } = &ev {
                        if !modifiers.ctrl && !modifiers.command && !modifiers.alt && !modifiers.shift && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) {
                            let k = *key;
                            self.wrap_nav(ctx, &font, wrap_w, tab, kcols, k);
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
        if accept_completion { self.accept_completion(); }
        if let Some(c) = clip_out { self.put_clip(ctx, c); }
        if let Some(r) = jump_row { self.jump_req = Some((self.docs[ai].id, r)); }
        self.preedit = preedit_now;
        let preedit = self.preedit.clone();

        // ---- 绘制 ----
        let reveal = std::mem::take(&mut self.docs[ai].reveal);
        let center = reveal && std::mem::take(&mut self.center_once);
        let mut widest = max_cols;
        if !self.docs[ai].grep_result { let _ = self.docs[ai].changed(); }  // 修改行标记: 按 rev 缓存, 绘制时只读
        // 列标尺: 固定在编辑区上方, 横向跟着滚动 (滚动量在 ScrollArea 返回后才知道, 先占位)
        self.sys.ruler_h = if vw.ruler { row_h } else { 0.0 };
        let ruler_rect = if vw.ruler { Some(ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover()).0) } else { None };
        let mut clicked: Option<(Pos2, bool, bool)> = None; // (位置, 是否Shift, 是否拖动)
        let mut multi_click = 0u8; // 2 = 双击选词, 3 = 三击选行
        let mut ed_id: Option<egui::Id> = None;
        let mut view_x: Option<(f32, f32)> = None; // 不折行时视口的行内 x 范围 (长行只排版这一段, 点击取列也用同一段)
        let out = egui::ScrollArea::both().auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
            if !wrap { view_x = Some((viewport.min.x - gutter - pad, viewport.max.x - gutter - pad)); }
            let vx = view_x;
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
                let rv = RowView::build(&d.lines[row], tab, kcols, vx, char_w);
                widest = widest.max(rv.ncols);
                let row_c = if is_diff { crate::view::diff_line_color(&d.lines[row], &tc).unwrap_or(text_color) } else { text_color };
                let ident: Vec<usize>;
                let (hl_line, hl_map): (&str, &[usize]) = if rv.sliced { ident = (0..=rv.len).collect(); (&rv.disp, &ident) } else { (&d.lines[row], &rv.map) };
                let mut job = highlight_job(hl_line, &rv.disp, hl_map, if is_diff { None } else { cfg.as_ref() }, &theme, &font, row_c, row_h);
                if wrap { job.wrap.max_width = wrap_w; job.wrap.break_anywhere = true; }
                let galley = painter.layout_job(job);
                // 选区 / 查找命中的底色都走这一个函数: 一段 [c0, c1) 列 (自动换行时逐字符取排版位置, 否则一整块)
                let span = |c0: usize, c1: usize, extend: bool, color: Color32| {
                    if wrap {
                        let (ds, de) = (rv.idx(c0.min(rv.tail())), rv.idx(c1.min(rv.tail())));
                        for k in ds..de {
                            let a = galley.pos_from_cursor(egui::text::CCursor::new(k)).min;
                            let b = galley.pos_from_cursor(egui::text::CCursor::new(k + 1)).min;
                            let xe = if (b.y - a.y).abs() < 0.5 && b.x > a.x { b.x } else { a.x + char_w };
                            painter.rect_filled(Rect::from_min_max(Pos2::new(x0 + a.x, y + a.y), Pos2::new(x0 + xe, y + a.y + row_h)), 0.0, color);
                        }
                    } else {
                        if !rv.touches(c0, c1) { return; }
                        let xa = rv.x_of(&painter, c0, &font, text_color);
                        let mut xb = rv.x_of(&painter, c1.min(rv.tail()), &font, text_color);
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
                        let c1 = if row == s[2] { s[3] } else if rv.sliced { rv.ncols } else { d.lines[row].chars().count() };
                        span(c0, c1, row != s[2], sel_bg);
                    }
                }
                if let Some(b) = d.box_sel {
                    let (r0, r1, c0, c1) = (b[0].min(b[2]), b[0].max(b[2]), b[1].min(b[3]), b[1].max(b[3]));
                    if row >= r0 && row <= r1 && c1 > c0 {
                        let tail = rv.tail();
                        let xx = |c: usize| rv.x_of(&painter, c.min(tail), &font, text_color) + char_w * c.saturating_sub(tail) as f32;
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
                            let p = galley.pos_from_cursor(egui::text::CCursor::new(rv.idx(col.min(rv.tail())))).min;
                            (p.x, p.y)
                        } else { (rv.x_of(&painter, col.min(rv.tail()), &font, text_color), 0.0) }
                    };
                    let marks_src = if rv.sliced { &rv.disp } else { &d.lines[row] };
                    for (c, ch) in crate::view::whitespace_marks(marks_src) {
                        let (px, py) = at(c + rv.col0);
                        painter.text(Pos2::new(x0 + px, y + py + (row_h - base_h) / 2.0), egui::Align2::LEFT_TOP, ch, font.clone(), faint);
                    }
                    if row + 1 < d.lines.len() {
                        let (px, py) = at(if rv.sliced { rv.ncols } else { d.lines[row].chars().count() });
                        painter.text(Pos2::new(x0 + px, y + py + (row_h - base_h) / 2.0), egui::Align2::LEFT_TOP, '↵', font.clone(), faint);
                    }
                }
                painter.galley(Pos2::new(x0 + rv.x_off(), y), galley, text_color);
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
                    painter.text(Pos2::new(rect.min.x + gutter - 8.0, y + row_h / 2.0), egui::Align2::RIGHT_CENTER, format!("{}", row + 1), num_font.clone(), num_c);
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
                let rv = RowView::build(&d.lines[cr], tab, kcols, vx, char_w);
                let (cx, cy) = if wrap {
                    let mut job = highlight_job(&d.lines[cr], &rv.disp, &rv.map, None, &theme, &font, text_color, row_h);
                    job.wrap.max_width = wrap_w; job.wrap.break_anywhere = true;
                    let g = painter.layout_job(job);
                    let p = g.pos_from_cursor(egui::text::CCursor::new(rv.idx(cc.min(rv.tail())))).min;
                    (x0 + p.x, rect.min.y + top(cr) + p.y)
                } else {
                    (x0 + rv.x_of(&painter, cc.min(rv.tail()), &font, text_color), rect.min.y + cr as f32 * row_h)
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
                caret_screen = Some(Pos2::new(cx, cy + row_h)); // 补全浮层贴在光标行下面
                ui.ctx().output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput { purpose: egui::IMEPurpose::Normal, rect, cursor_rect: Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(2.0, row_h)), should_interrupt_composition: false });
                });
            }
            if let Some(dr) = text_drop {
                // 拖放文字的落点: 一根蓝色竖线
                if dr[0] >= first && dr[0] < last && dr[0] < d.lines.len() {
                    let rv = RowView::build(&d.lines[dr[0]], tab, kcols, vx, char_w);
                    let c = dr[1].min(rv.tail());
                    let (dx, dy) = if wrap {
                        let mut job = highlight_job(&d.lines[dr[0]], &rv.disp, &rv.map, None, &theme, &font, text_color, row_h);
                        job.wrap.max_width = wrap_w; job.wrap.break_anywhere = true;
                        let g = painter.layout_job(job);
                        let p = g.pos_from_cursor(egui::text::CCursor::new(rv.idx(c))).min;
                        (x0 + p.x, rect.min.y + top(dr[0]) + p.y)
                    } else {
                        (x0 + rv.x_of(&painter, c, &font, text_color), rect.min.y + dr[0] as f32 * row_h)
                    };
                    painter.rect_filled(Rect::from_min_size(Pos2::new(dx, dy), Vec2::new(2.0, row_h)), 0.0, tc.blue);
                }
            }
            for ec in &d.extra {
                if ec[0] >= first && ec[0] < last {
                    let rv = RowView::build(&d.lines[ec[0]], tab, kcols, vx, char_w);
                    let ex = x0 + rv.x_of(&painter, ec[1].min(rv.tail()), &font, text_color);
                    painter.rect_filled(Rect::from_min_size(Pos2::new(ex, rect.min.y + top(ec[0])), Vec2::new(2.0, row_h)), 0.0, text_color);
                }
            }
            if resp.is_pointer_button_down_on() || resp.clicked() || resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    clicked = Some((p, ui.input(|i| i.modifiers.shift), resp.dragged() && !resp.drag_started()));
                }
            }
            if resp.drag_stopped() {
                drag_end = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.latest_pos()));
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
            let ruler_font = FontId::new(self.font_px, crate::fonts::editor_family());
            let origin = rr.min.x + gutter + pad - out.state.clamped_offset().x;
            let cur_col = { let d = &self.docs[ai]; crate::view::display_col(&d.lines[d.cursor[0].min(d.lines.len() - 1)], d.cursor[1], tab) };
            let cols = ((rr.width() + out.state.clamped_offset().x) / char_w) as usize + 1;
            // Tauri 的标尺是一串字符 `....+....1....+....2`: 每列一个字, 等宽对齐正文
            let first = (((rr.min.x + gutter - char_w - origin) / char_w).floor().max(0.0) as usize) + 1;
            if cols >= first {
                let text = crate::view::ruler_text(first, cols - first + 1);
                rp.text(Pos2::new(origin + (first - 1) as f32 * char_w, rr.center().y), egui::Align2::LEFT_CENTER, text, ruler_font, tc.fg_dim);
            }
            let cx = origin + (cur_col - 1) as f32 * char_w;
            rp.rect_filled(Rect::from_min_size(Pos2::new(cx, rr.min.y), Vec2::new(char_w, rr.height())), 0.0, Color32::from_rgba_unmultiplied(tc.blue.r(), tc.blue.g(), tc.blue.b(), 70));
        }
        if let Some(id) = ed_id { if !self.editor_ids.contains(&id) { self.editor_ids.push(id); } }
        self.max_cols = widest;
        self.sys.caret_screen = caret_screen;
        if let Some((p, shift, dragging, ended)) = drag_end.map(|p| (p, false, true, true)).or(clicked.map(|(p, s, d)| (p, s, d, false))) {
            let rel_y = (p.y - rect.min.y).max(0.0);
            let d = &mut self.docs[ai];
            let row = if use_offs { offs.partition_point(|&o| o <= rel_y).saturating_sub(1) } else { (rel_y / row_h).floor() as usize };
            let row = row.min(d.lines.len() - 1);
            if p.x < rect.min.x + gutter - 6.0 && ctx.input(|i| i.pointer.any_pressed()) {       // 点行号左侧: 折叠 / 展开这一行
                d.toggle_fold(row);
                return;
            }
            let rv = RowView::build(&d.lines[row], tab, kcols, view_x, char_w);
            let want = (p.x - x0).max(0.0);
            let col = rv.col_at(ctx, want, rel_y - top(row), &font, wrap_w);
            let (alt, ctrl) = ctx.input(|i| (i.modifiers.alt, i.modifiers.command || i.modifiers.ctrl));
            let in_sel = d.sel.is_some_and(|s| (row, col) >= (s[0], s[1]) && (row, col) < (s[2], s[3]));
            if ended {
                // 松手: 拖放文字就落下去 (Ctrl = 复制); 否则照旧结束框选
                match self.text_drag.take() {
                    Some(t) if t.started && !t.cancelled => {
                        if d.drop_selection([row, col], ctrl) { d.tip = tr(if ctrl { "dlg.drop-copied" } else { "dlg.drop-moved" }).into(); }
                    }
                    Some(_) => {}
                    None => { if d.box_sel.is_some() { d.box_drag(row, col); } else { d.drag_to(row, col); } }
                }
            } else if dragging {
                if let Some(t) = self.text_drag.as_mut() {
                    if !t.cancelled { t.started = true; t.drop = Some([row, col]); }
                } else if d.box_sel.is_some() { d.box_drag(row, col); } else { d.drag_to(row, col); }
            } else if ctx.input(|i| i.pointer.any_pressed()) {
                self.text_drag = None;
                if alt { d.box_start(row, col); }
                else if ctrl && !in_sel { d.toggle_cursor(row, col); }
                else if !shift && multi_click == 0 && d.box_sel.is_none() && in_sel && !d.grep_result { self.text_drag = Some(crate::textdrag::TextDrag::default()); }
                else { d.box_sel = None; d.extra.clear(); d.click(row, col, shift); }
            } else if ctx.input(|i| i.pointer.any_released()) && self.text_drag.take().is_some() {
                d.click(row, col, false); // 只是点了一下选区里面: 现在才把选区收起来放光标
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

/// 标签栏里点出来的命令 (画完再执行, 免得画的时候改 docs)
enum TabCmd {
    Select(usize),
    Close(usize),
    Many(usize, CloseKind),
    Move(usize, bool),
    Group(usize, u8),
}

/// 右侧停靠区 (Chat / 预览) 的宽度上限: 窄窗口里同时开侧栏 + 预览 + Chat 时, 至少给编辑区留 240px (不然编辑区被挤成 0 宽, 整块消失)
fn right_cap(avail: f32) -> f32 { (avail - 240.0).max(100.0) }
/// 剩余宽度够不够再放一个右侧停靠区 (编辑区 240 + 停靠区最小 160); 不够就不画, 免得压在编辑区上
fn right_room(avail: f32) -> bool { avail - 240.0 >= 160.0 }

fn tree(ui: &mut egui::Ui, side: &mut Sidebar, dir: &std::path::Path, open: &mut Option<(PathBuf, usize)>, op: &mut Option<FileOp>) {
    for e in side.children_of(dir) {
        if e.is_dir {
            let h = egui::CollapsingHeader::new(&e.name).id_salt(&e.path).show(ui, |ui| tree(ui, side, &e.path, open, op));
            h.header_response.context_menu(|ui| {
                if ui.button(trz("新建文件…")).clicked() { *op = Some(FileOp::NewFile(e.path.clone())); ui.close(); }
                if ui.button(trz("新建文件夹…")).clicked() { *op = Some(FileOp::NewDir(e.path.clone())); ui.close(); }
                if ui.button(trz("重命名…")).clicked() { *op = Some(FileOp::Rename(e.path.clone())); ui.close(); }
                if ui.button(trz("删除…")).clicked() { *op = Some(FileOp::Delete(e.path.clone())); ui.close(); }
            });
        } else {
            let r = ui.selectable_label(false, &e.name);
            if r.clicked() { *open = Some((e.path.clone(), 1)); }
            r.context_menu(|ui| {
                if ui.button(trz("重命名…")).clicked() { *op = Some(FileOp::Rename(e.path.clone())); ui.close(); }
                if ui.button(trz("删除…")).clicked() { *op = Some(FileOp::Delete(e.path.clone())); ui.close(); }
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

/// 点在 x 处 -> 字符列 (取离得最近的字符边界)
pub(crate) fn col_at(ctx: &egui::Context, disp: &str, map: &[usize], x: f32, y: f32, font: &FontId, wrap_w: f32) -> usize {
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
    /// 半透明主题: 窗口是透明的,清屏色也要全透明,面板自己的 alpha 才能透出桌面 / 模糊材质
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.sys.theme_glass { [0.0; 4] } else { Color32::from_rgb(12, 12, 12).to_normalized_gamma_f32() }
    }

    /// 菜单开着时截下方向键 (egui 的空间焦点导航会乱跳), 并清洗 Alt+字母 / Esc 同帧的 Text 事件
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let menu = self.open_top_menu(ctx);
        let any_popup = egui::Popup::is_any_open(ctx);
        self.bar_focus_hook(ctx, raw, menu.is_some() || any_popup);
        let menu = self.open_top_menu(ctx);
        menunav::scrub(&mut raw.events, &mut self.sys.swallow, menu.is_some() || egui::Popup::is_any_open(ctx));
        let Some(menu) = menu else { return };
        // 焦点在子菜单里 (不是顶层菜单那一层) 就让 egui 自己处理
        let top = egui::LayerId::new(egui::Order::Foreground, self.sys.menu_ids[menu].1);
        if let Some(f) = ctx.memory(|m| m.focused()) {
            let in_top = ctx.viewport(|v| v.prev_pass.widgets.get_layer(top).any(|w| w.id == f));
            let on_bar = ctx.viewport(|v| v.prev_pass.widgets.get(f).is_some_and(|w| w.layer_id != top && w.layer_id.order != egui::Order::Foreground));
            if !in_top && !on_bar { return; }
            // 焦点在会展开子菜单的项上: → 留给 egui 开子菜单
            if in_top && egui::Popup::is_id_open(ctx, egui::containers::menu::SubMenu::id_from_widget_id(f)) { return; }
        }
        let q = &mut self.sys.nav_q;
        raw.events.retain(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } => match menunav::nav_key(*key, *modifiers) { Some(k) => { q.push(k); false } None => true },
            egui::Event::Key { key, pressed: false, modifiers, .. } => menunav::nav_key(*key, *modifiers).is_none(),
            _ => true,
        });
    }

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.glass_sync(frame);
        // 自绘标题栏 + 最大化: Windows 把无边框的最大化窗口放大出屏幕 8 点 (可调大小的窗口框), 内容缩进这么多才不会被裁掉
        let inset = self.maximized_inset(root.ctx());
        if inset > 0.0 {
            let rect = root.max_rect().shrink(inset);
            let mut child = root.new_child(egui::UiBuilder::new().max_rect(rect));
            self.ui_main(&mut child);
        } else {
            self.ui_main(root);
        }
    }
}

/// 滚动条 (Tauri 全局 `::-webkit-scrollbar`: 12 宽、轨道透明、滑块可见 6 宽圆角、常显)
fn tauri_scroll() -> egui::style::ScrollStyle {
    egui::style::ScrollStyle {
        floating: false, bar_width: 6.0, bar_inner_margin: 3.0, bar_outer_margin: 3.0, handle_min_length: 24.0,
        dormant_background_opacity: 0.0, active_background_opacity: 0.0, interact_background_opacity: 0.0,
        dormant_handle_opacity: 1.0, active_handle_opacity: 1.0, interact_handle_opacity: 1.0,
        ..egui::style::ScrollStyle::solid()
    }
}

/// 状态栏里的控件样式: 蓝底白字 (字色由底色亮度定), 按钮 / 菜单钮不带底色, 悬停叠一层白
fn status_bar_style(ui: &mut egui::Ui, text: Color32) {
    ui.spacing_mut().interact_size.y = 16.0;
    ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
    let v = ui.visuals_mut();
    v.override_text_color = Some(text);
    v.widgets.noninteractive.bg_stroke.color = Color32::from_white_alpha(50);
    for w in [&mut v.widgets.inactive, &mut v.widgets.open] { w.weak_bg_fill = Color32::TRANSPARENT; w.bg_fill = Color32::TRANSPARENT; w.bg_stroke = egui::Stroke::NONE; w.fg_stroke.color = text; }
    v.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(36);
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.fg_stroke.color = text;
}

impl App {
    fn ui_main(&mut self, root: &mut egui::Ui) {
        let ctx = &root.ctx().clone();
        self.tc = match &self.theme_vars { Some(v) if self.theme_visuals.is_some() => crate::view::ThemeColors::from_vars(v), _ => crate::view::ThemeColors::builtin(self.dark) };
        self.tc.retro16 = self.theme_id == "theme-retro16";
        let mut vis = match &self.theme_visuals { Some(v) => v.clone(), None => self.tc.visuals() };
        if let Some(c) = self.cfg.colors.bg { vis.extreme_bg_color = Color32::from_rgb(c[0], c[1], c[2]); } // 编辑区背景覆盖
        ctx.set_visuals(vis);
        if !ctx.data(|d| d.get_temp::<bool>(egui::Id::new("tauri-scroll")).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("tauri-scroll"), true));
            ctx.global_style_mut(|s| s.spacing.scroll = tauri_scroll());
        }
        if self.cfg.font_family != self.applied_font {
            crate::fonts::install(ctx, &self.cfg.font_family);
            self.applied_font = self.cfg.font_family.clone();
        }
        self.sync_doc_prefs();
        self.sys_frame(ctx);
        self.track_geometry(ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.exit_session.is_none() { self.exit_session = Some((self.session_paths(), self.session_active())); }
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
            if all { self.confirm_exit = true; ctx.send_viewport_cmd(egui::ViewportCommand::Close); } else { self.exit_session = None; crate::updapply::cancel(); }
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
        self.completion_popup(ctx);
        if self.sys.mdq.open { self.md_struct_window(ctx); }
        if self.bm_open { self.bookmark_window(ctx); }
        self.diff_refresh();
        self.diff_windows(ctx);
        if self.props.is_some() { self.props_window(ctx); }
        self.cmd_windows(ctx);
        self.tools_windows(ctx);
        self.tag_windows(ctx);
        self.print_windows(ctx);
        if self.split_on {
            self.panes.clamp(self.docs.len());
            self.active = self.panes.focused_doc();
        }
        // 选择列表在这一步之后: 它切过去 / 新开的标签要盖过焦点窗格, 再由标签栏同步回窗格 (放在前面会被上面那句盖回去)
        self.pick_window(ctx);
        self.key_search_tick();
        self.shortcuts(ctx);
        self.top_bar(root, ctx);
        if self.cfg.view.toolbar { egui::Panel::top("toolbar").show(root, |ui| self.toolbar(ui, ctx)); }
        use crate::panels::{Dock, Mode};
        self.normalize_docks();
        self.status_panel(root);
        if self.cfg.view.funckey { egui::Panel::bottom("funckey").show(root, |ui| self.funckey_bar(ui, ctx)); }
        if self.find.open {
            let i = self.active.min(self.docs.len() - 1);
            self.find.window(ctx, &mut self.docs[i]);
        }
        // 查找栏 / 侧栏输入框 / 对话框正在收键盘时, 编辑区不抢
        let find_focus = self.cmdui.capturing() || ctx.memory(|m| m.focused()).map(|f| !self.editor_ids.contains(&f)).unwrap_or(false);
        match self.dock_mode(Dock::Bottom, self.term.open) {
            Mode::Full => {
                let r = egui::Panel::bottom("term").resizable(true).default_size(self.cfg.view.term_h).frame(egui::Frame::NONE.fill(self.tc.bg_edit)).show(root, |ui| self.bottom_ui(ui));
                if !ctx.input(|i| i.pointer.any_down()) { self.cfg.view.term_h = crate::panels::clamp_size(Dock::Bottom, r.response.rect.height().round()); }
            }
            Mode::Strip => self.dock_strip(root, Dock::Bottom),
            Mode::Hidden => {}
        }
        if !self.cfg.view.rail && self.dock_mode(crate::panels::Dock::Side, self.side.open) != crate::panels::Mode::Hidden { self.dock_strip(root, crate::panels::Dock::Side); }
        if self.cfg.view.rail { egui::Panel::left("rail").resizable(false).exact_size(chrome::RAIL_W).frame(egui::Frame::NONE.fill(self.tc.bg_rail)).show(root, |ui| self.activity_rail(ui)); }
        match self.dock_mode(Dock::Side, self.side.open) {
            Mode::Full => {
                let r = egui::Panel::left("side").resizable(true).default_size(self.cfg.view.side_w).show(root, |ui| {
                    // 内容窄 (Git 页 / 空文件页) 时面板不能缩成内容宽: 撑满当前面板宽, 宽度才不会被内容带着越缩越窄
                    ui.set_min_width(ui.available_width());
                    self.side_ui(ui)
                });
                // 拖动期间不记, 松手后取整记一次 (免得每帧都落盘)
                if !ctx.input(|i| i.pointer.any_down()) { self.cfg.view.side_w = crate::panels::clamp_size(Dock::Side, r.response.rect.width().round()); }
            }
            Mode::Strip => {}
            Mode::Hidden => {}
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
        self.ai_poll(ctx);
        self.dock_strip(root, Dock::Right); // 右细条常驻 (最外层), Chat / 预览停靠在它左边
        match self.dock_mode(Dock::Right, crate::panels::right_visible(self.ai.open, self.md_preview, is_md, self.cfg.view.md_overlay)) {
            Mode::Full => {
                let settled = !ctx.input(|i| i.pointer.any_down());
                if self.ai.open && right_room(root.available_width()) {
                    let r = egui::Panel::right("aichat").resizable(true).default_size(self.cfg.view.ai_w).max_size(right_cap(root.available_width())).show(root, |ui| self.ai_ui(ui));
                    if settled { self.cfg.view.ai_w = crate::panels::clamp_size(Dock::Right, r.response.rect.width().round()); }
                }
                if self.md_preview && is_md && !self.cfg.view.md_overlay && right_room(root.available_width()) {
                    let r = egui::Panel::right("mdprev").resizable(true).default_size(self.cfg.view.md_w).max_size(right_cap(root.available_width())).show(root, |ui| self.md_ui(ui, false));
                    if settled { self.cfg.view.md_w = crate::panels::clamp_size(Dock::Right, r.response.rect.width().round()); }
                }
            }
            Mode::Strip => {}
            Mode::Hidden => {}
        }
        self.tab_bar(root, ctx);
        self.md_bar(root, ctx, is_md);
        use crate::split::SplitKind as SK;
        let fill = ctx.global_style().visuals.extreme_bg_color;
        let kind = self.panes.kind;
        if matches!(kind, SK::Rows | SK::Quad) {
            egui::Panel::bottom("split_b").resizable(true).default_size(root.available_height() * 0.5).frame(egui::Frame::NONE.fill(fill)).show(root, |ui| {
                if kind == SK::Rows { self.editor_pane(ui, ctx, find_focus, 1); } else {
                    ui.columns(2, |c| { self.editor_pane(&mut c[0], ctx, find_focus, 2); self.editor_pane(&mut c[1], ctx, find_focus, 3); });
                }
            });
        }
        if kind == SK::Cols {
            egui::Panel::right("split").resizable(true).default_size(root.available_width() * 0.5).frame(egui::Frame::NONE.fill(fill)).show(root, |ui| self.editor_pane(ui, ctx, find_focus, 1));
        }
        egui::CentralPanel::default_margins().frame(egui::Frame::NONE.fill(fill)).show(root, |ui| {
            if kind == SK::Quad { ui.columns(2, |c| { self.editor_pane(&mut c[0], ctx, find_focus, 0); self.editor_pane(&mut c[1], ctx, find_focus, 1); }); }
            else {
                let area = ui.max_rect();
                self.editor_pane(ui, ctx, find_focus, 0);
                if self.md_preview && is_md && self.cfg.view.md_overlay && !self.welcome_visible() { self.md_overlay(ctx, area); }
                if self.welcome_visible() { self.welcome(ui, area); }
            }
        });
        self.restore_window(ctx);
        self.window_grips(ctx);
        self.persist_if_changed();
        ctx.request_repaint_after(std::time::Duration::from_millis(550)); // 光标闪烁
        if !self.shown {
            // 窗口创建时是隐藏的 (见 main.rs): 第一帧画完才显示, 用户永远看不到未绘制的空白 / 白闪窗口
            self.shown = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            self.trace.mark("window shown");
        }
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
    fn markdown_block_click_edits_in_place_and_apply_is_one_undo_step() {
        let (mut a, _g) = app(Settings::default());
        a.new_doc();
        let ai = a.active;
        let orig: Vec<String> = ["# Title", "", "first para", "", "second para"].iter().map(|s| s.to_string()).collect();
        a.docs[ai].lines = orig.clone();
        let blocks = crate::markdown::parse(&a.docs[ai].lines);
        let n = blocks.iter().position(|b| b.line == 2).expect("段落块");
        // 点块 -> 进入就地编辑, 编辑框装着该块原文
        a.md_handle(ai, &blocks, &crate::md_view::MdEvent { edit_req: Some(n), ..Default::default() });
        let e = a.md_cache.edit.as_ref().expect("进入编辑");
        assert_eq!((e.start, e.end, e.buf.as_str()), (2, 3, "first para"));
        // 点框外 (有改动) = 应用: 一个补丁, 撤销一次就回到原文
        let ev = crate::md_view::MdEvent { apply: Some((2, 3, "first
EDITED".into())), ..Default::default() };
        a.md_handle(ai, &blocks, &ev);
        assert_eq!(a.docs[ai].lines[2..4], ["first".to_string(), "EDITED".to_string()]);
        a.docs[ai].undo();
        assert_eq!(a.docs[ai].lines, orig, "一次撤销还原整个就地编辑");
        // 同一帧既应用又点了另一块: 不开新编辑 (行号已变)
        a.md_cache.edit = None;
        let ev = crate::md_view::MdEvent { apply: Some((2, 3, "x".into())), edit_req: Some(0), ..Default::default() };
        a.md_handle(ai, &blocks, &ev);
        assert!(a.md_cache.edit.is_none());
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
    fn manual_syntax_type_overrides_the_extension_and_unknown_names_fall_back() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].name = "x.txt".into();
        assert_eq!(a.type_of(&a.docs[0]).map(|t| t.name.as_str()), Some("Plain"));
        a.docs[0].syntax = Some("Rust".into());
        assert_eq!(a.type_of(&a.docs[0]).map(|t| t.name.as_str()), Some("Rust"));
        a.docs[0].syntax = Some("NoSuchType".into());
        assert_eq!(a.type_of(&a.docs[0]).map(|t| t.name.as_str()), Some("Plain"), "查不到 = 按扩展名");
    }

    #[test]
    fn function_step_jumps_between_outline_entries_and_wraps() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].name = "x.rs".into();
        a.docs[0].lines = ["fn a() {}", "", "fn b() {", "}", "", "fn c() {}"].iter().map(|s| s.to_string()).collect();
        a.docs[0].cursor = [3, 0];
        a.step_function(true);
        assert_eq!(a.docs[0].cursor[0], 5);
        a.step_function(true);
        assert_eq!(a.docs[0].cursor[0], 0, "最后一个函数再往下绕回第一个");
        a.step_function(false);
        assert_eq!(a.docs[0].cursor[0], 5, "第一个再往上绕回最后一个");
        a.docs[0].name = "x.txt".into();
        a.docs[0].lines = vec!["plain".into()];
        a.docs[0].rev += 1; // 内容变了, 大纲缓存按修订号失效
        a.docs[0].cursor = [0, 0];
        a.step_function(true);
        assert_eq!(a.docs[0].cursor, [0, 0], "没有大纲的文件: 不动");
        assert!(a.docs[0].tip.contains("没有函数标记"), "{}", a.docs[0].tip);
    }

    #[test]
    fn copy_commands_put_the_expected_text_on_the_clipboard_history() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.docs[0].name = "n.txt".into();
        a.docs[0].lines = vec!["abcdefghij".into()];
        a.docs[0].cursor = [0, 3];
        a.copy_tag(&ctx);
        assert_eq!(a.tools.clip.first().map(String::as_str), Some("n.txt(1,4)"), "没存盘的文档用文档名, 行列从 1 数");
        a.cfg.view.wrap_col = 4;
        a.copy_wrapped(&ctx);
        assert_eq!(a.tools.clip.first().map(String::as_str), Some("abcd\r\nefgh\r\nij"), "无选区 = 当前行, 每 4 字符切一刀");
        a.docs[0].sel = Some([0, 2, 0, 8]);
        a.copy_wrapped(&ctx);
        assert_eq!(a.tools.clip.first().map(String::as_str), Some("cdef\r\nGH".to_lowercase().as_str()), "有选区 = 只复制选区");
    }

    #[test]
    fn open_counterpart_switches_between_header_and_source() {
        let dir = std::env::temp_dir().join(format!("idefast-hc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (c, h) = (dir.join("foo.c"), dir.join("foo.hpp"));
        std::fs::write(&c, "int main(){}\n").unwrap();
        std::fs::write(&h, "#pragma once\n").unwrap();
        let (mut a, _g) = app(Settings::default());
        a.open_path(&c);
        assert_eq!(a.cur().path.as_deref(), Some(c.as_path()));
        a.open_counterpart();
        assert_eq!(a.cur().path.as_deref(), Some(h.as_path()), "foo.c -> foo.hpp (foo.h 不存在, 按顺序找下一个)");
        a.open_counterpart();
        assert_eq!(a.cur().path.as_deref(), Some(c.as_path()), "再按回到 foo.c, 不重复开标签");
        let before = a.docs.len();
        std::fs::remove_file(&h).unwrap();
        a.open_counterpart();
        assert_eq!((a.docs.len(), a.cur().tip.contains("没有找到")), (before, true), "对应文件不存在: 只提示");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn completion_lists_candidates_cycles_and_accepts_as_one_undo_step() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].name = "x.rs".into();
        a.docs[0].lines = vec!["fn main() {".into(), "    let counter = 1;".into(), "    cou + 1".into()];
        a.docs[0].goto(2, 7);
        a.show_completion();
        let p = a.sys.complete.as_ref().expect("前缀 cou 有候选");
        assert_eq!(p.items, ["counter"], "文件里出现过的标识符; 关键字里没有以 cou 开头的");
        a.docs[0].goto(1, 7);
        a.show_completion();
        assert!(a.sys.complete.is_none(), "前缀就是 let 本身: 没有更长的候选, 不弹浮层");
        // 前缀 "l": 关键字 let / loop 排在文件标识符前面; 上下键到头绕回
        a.docs[0].lines[2] = "    l".into();
        a.docs[0].goto(2, 5);
        a.show_completion();
        let n = a.sys.complete.as_ref().unwrap().items.len();
        assert!(n >= 2 && a.sys.complete.as_ref().unwrap().items[0].starts_with('l'));
        a.complete_move(-1);
        assert_eq!(a.sys.complete.as_ref().unwrap().idx, n - 1, "第一项再往上绕回最后一项");
        a.complete_move(1);
        assert_eq!(a.sys.complete.as_ref().unwrap().idx, 0);
        // 接受第一项: 前缀换成候选, 光标到候选末尾, 一步撤销
        let first = a.sys.complete.as_ref().unwrap().items[0].clone();
        a.accept_completion();
        assert!(a.sys.complete.is_none());
        assert_eq!(a.docs[0].lines[2], format!("    {first}"));
        assert_eq!(a.docs[0].cursor, [2, 4 + first.chars().count()]);
        a.docs[0].undo();
        assert_eq!(a.docs[0].lines[2], "    l", "一步撤销回到补全前");
    }

    #[test]
    fn completion_reports_when_there_is_nothing_and_closes_when_stale() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].lines = vec!["zq".into()];
        a.docs[0].goto(0, 2);
        a.show_completion();
        assert!(a.sys.complete.is_none() && a.docs[0].tip.contains("无补全候选"), "{}", a.docs[0].tip);
        a.docs[0].lines = vec!["alpha_one alpha_two al".into()];
        a.docs[0].goto(0, 22);
        a.show_completion();
        assert!(a.sys.complete.is_some());
        a.docs[0].goto(0, 3); // 光标挪走: 浮层该关
        a.completion_popup(&egui::Context::default());
        assert!(a.sys.complete.is_none(), "光标变了就关掉浮层");
        a.docs[0].goto(0, 22);
        a.show_completion();
        a.docs[0].type_text("x"); // 打字: 修订号变了
        a.completion_popup(&egui::Context::default());
        assert!(a.sys.complete.is_none(), "文档变了就关掉浮层");
    }

    #[test]
    fn goto_definition_jumps_to_the_definition_line_and_reports_misses() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].lines = vec!["main();".into(), "fn helper() {}".into(), "helper();".into(), "  + ".into()];
        a.docs[0].goto(2, 3);
        a.goto_definition();
        assert_eq!(a.docs[0].cursor, [1, 3], "跳到 fn helper 的 helper 上");
        assert!(a.docs[0].tip.contains("定义 @ Ln 2"), "{}", a.docs[0].tip);
        a.docs[0].goto(3, 3);
        a.goto_definition();
        assert!(a.docs[0].tip.contains("光标处无标识符"), "{}", a.docs[0].tip);
        a.docs[0].lines[3] = "unknown_sym".into();
        a.docs[0].goto(3, 4);
        a.goto_definition();
        assert_eq!(a.docs[0].cursor, [3, 0], "只出现这一次: 首次出现就是自己");
    }

    #[test]
    fn macro_runs_document_and_window_steps_in_order_and_follows_the_new_active_tab() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.run_mac_src("S_InsText(\"first\");\nS_FileNew();\nS_InsText(\"second\");", 0, &ctx);
        assert_eq!(a.docs.len(), 2, "S_FileNew 开了新标签");
        assert_eq!(a.docs[0].lines, ["first"], "新建之前的文字在原来的文档里");
        assert_eq!(a.docs[1].lines, ["second"], "新建之后的步骤跟着新的活动标签走");
        assert_eq!(a.active, 1);
        assert!(a.cur().tip.contains("宏已执行 (3 步)"), "{}", a.cur().tip);
    }

    #[test]
    fn macro_with_an_unrunnable_step_does_nothing_and_says_why() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.run_mac_src("S_InsText(\"x\");\nS_Cascade();", 0, &ctx);
        assert_eq!(a.docs[0].lines, [""], "整份宏都没执行");
        assert!(a.docs[0].tip.contains("宏未执行") && a.docs[0].tip.contains("第 2 行") && a.docs[0].tip.contains("层叠窗口"), "{}", a.docs[0].tip);
    }

    #[test]
    fn macro_steps_for_window_list_help_and_panes_run_through_the_app() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.side.open = false;
        a.run_mac_src("S_Outline();\nS_WinList();", 0, &ctx);
        assert!(a.side.open && a.side.tab == Tab::Outline, "S_Outline 切到大纲页");
        assert_eq!(a.tools.pick.as_ref().map(|p| p.kind), Some(crate::pick::Kind::Windows), "S_WinList 打开窗口一览");
        a.tools.pick = None;
        a.run_mac_src("S_FileTree();\nS_ToggleKeySearch();\nS_HelpContents();", 0, &ctx);
        assert!(a.side.tab == Tab::Files && a.cfg.view.key_search, "S_FileTree 切文件页, S_ToggleKeySearch 打开自动显示");
        assert_eq!(a.cur().name, crate::help::MANUAL_NAME, "S_HelpContents 开手册");
        a.run_mac_src("S_ChgCharset();", 0, &ctx);
        assert_eq!(a.tools.pick.as_ref().map(|p| p.kind), Some(crate::pick::Kind::Charset));
        a.run_mac_src("S_FileReopen();", 0, &ctx);
        assert_eq!(a.tools.pick.as_ref().map(|p| p.kind), Some(crate::pick::Kind::Reopen), "后开的列表替换先开的");
        a.run_mac_src("S_ShowTab();", 0, &ctx);
        assert!(a.cur().tip.contains("宏未执行") && a.cur().tip.contains("开关"), "ShowTab 仍然明确拒绝: {}", a.cur().tip);
    }

    #[test]
    fn macro_clipboard_reaches_the_app_clipboard_history() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.docs[0].lines = vec!["alpha".into(), "beta".into()];
        a.docs[0].goto(1, 0);
        a.run_mac_src("S_Copy();\nS_GoFileTop();\nS_Paste();", 0, &ctx);
        assert_eq!(a.tools.clip.first().map(String::as_str), Some("beta\n"), "宏里复制的整行同步进了剪贴板历史");
        assert_eq!(a.docs[0].lines, ["beta", "alpha", "beta"], "宏里粘贴用的是宏自己的剪贴板");
    }

    #[test]
    fn macro_slots_run_register_and_report_empty_or_full() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.run_macro_slot(0, &ctx);
        assert!(a.cur().tip.contains("宏 1 是空的"), "{}", a.cur().tip);
        a.cfg.macro_slots[3] = crate::settings::MacroSlot { name: "加头".into(), kind: "mac".into(), code: "S_InsText(\"> \");".into() };
        a.docs[0].lines = vec!["body".into()];
        a.docs[0].goto(0, 0);
        a.run_macro_slot(3, &ctx);
        assert_eq!(a.docs[0].lines, ["> body"], "槽里的 Sakura 宏作用在活动文档上");
        // 登记: 选区优先, 否则整份文档; .js 不登记
        a.docs[0].name = "tidy.mac".into();
        a.docs[0].lines = vec!["S_Undo();".into()];
        a.register_macro_doc();
        let tip = a.cur().tip.clone();
        assert_eq!((a.cfg.macro_slots[0].name.as_str(), a.cfg.macro_slots[0].kind.as_str(), a.cfg.macro_slots[0].code.trim()), ("tidy", "mac", "S_Undo();"), "{tip}");
        assert!(tip.contains("已登记到宏 1"), "{tip}");
        a.docs[0].name = "x.ppa".into();
        a.register_macro_doc();
        assert_eq!(a.cfg.macro_slots[1].kind, "ppa", ".ppa 文档登记成 PPA");
        a.docs[0].name = "m.js".into();
        a.register_macro_doc();
        assert!(a.cur().tip.contains("不运行 JS 宏") && a.cfg.macro_slots[2].is_empty(), "{}", a.cur().tip);
        a.docs[0].name = "full.mac".into();
        for i in 0..crate::settings::MACRO_SLOTS { if a.cfg.macro_slots[i].is_empty() { a.cfg.macro_slots[i].code = "S_Undo();".into(); } }
        a.register_macro_doc();
        assert!(a.cur().tip.contains("宏槽都满了"), "{}", a.cur().tip);
        a.docs[0].lines = vec!["  ".into()];
        a.register_macro_doc();
        assert!(a.cur().tip.contains("没有可登记的内容") || a.cur().tip.contains("宏槽都满了"), "{}", a.cur().tip);
    }

    #[test]
    fn tab_group_commands_switch_move_merge_and_close() {
        let (mut a, _g) = app(Settings::default());
        a.new_doc();
        a.new_doc();
        a.new_doc();
        for (d, g) in a.docs.iter_mut().zip([0u8, 2, 2, 4]) { d.group = g; }
        a.active = 0;
        a.group_switch(true);
        assert_eq!(a.active, 1, "下一个分组 (2) 里最靠左的标签");
        a.group_switch(true);
        assert_eq!(a.active, 3);
        a.group_switch(true);
        assert_eq!(a.active, 0, "最后一个分组再往后绕回第一个");
        a.group_switch(false);
        assert_eq!(a.active, 3, "往前绕回最后一个");
        a.group_joint(false);
        assert_eq!(a.docs[3].group, 2, "当前标签移到上一个分组");
        a.active = 0;
        a.group_separate();
        assert_eq!(a.docs[0].group, 3, "新分组 = 现有最大编号 + 1");
        a.group_bind_all();
        assert!(a.docs.iter().all(|d| d.group == 0), "合并为一组 = 都回到无分组");
        a.group_switch(true);
        assert!(a.cur().tip.contains("只有一个分组"), "{}", a.cur().tip);
        a.docs[1].group = 2;
        a.docs[2].group = 2;
        a.active = 1;
        a.group_close();
        assert_eq!(a.docs.len(), 2, "当前分组里的两个标签关掉了");
        assert!(a.docs.iter().all(|d| d.group == 0) && a.active < a.docs.len());
    }

    #[test]
    fn small_view_and_search_commands_update_their_state() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].lines = vec!["abc".into(), "def".into()];
        a.center_cursor();
        assert!(a.center_once && a.docs[0].reveal, "光标行居中: 下一帧居中并滚到光标");
        assert_eq!(a.sys.sel_count, 0);
        a.cycle_sel_count();
        a.cycle_sel_count();
        assert_eq!(a.sys.sel_count, 2);
        assert!(a.cur().tip.contains("行数"), "{}", a.cur().tip);
        a.cycle_sel_count();
        assert_eq!(a.sys.sel_count, 0, "字符 -> 字节 -> 行 -> 回到字符");
        let m = a.find.marks;
        a.toggle_find_marks();
        assert_eq!(a.find.marks, !m);
        a.jump_find_start();
        assert!(a.cur().tip.contains("还没有查找起点"), "没查找过: 只提示");
        a.find.needle = "e".into();
        a.docs[0].goto(1, 2);
        a.jump_find_start();
        assert_eq!(a.docs[0].cursor, [0, 0], "回到查找开始时的位置");
        a.docs[0].goto(1, 1);
        a.space_indent_cmd(false);
        assert_eq!(a.docs[0].lines[1], " def", "缩进当前行 1 个空格");
        a.space_indent_cmd(true);
        assert_eq!(a.docs[0].lines[1], "def");
    }

    #[test]
    fn column_delete_needs_a_box_selection_and_removes_the_block() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].lines = vec!["abcdef".into(), "ghijkl".into(), "mnopqr".into()];
        a.column_delete();
        assert_eq!(a.docs[0].lines[0], "abcdef", "没有列块选区: 什么也不改");
        assert!(a.docs[0].tip.contains("Alt+拖选"));
        a.docs[0].box_sel = Some([0, 1, 2, 3]); // 第 0..=2 行, 第 1..3 列
        a.column_delete();
        assert_eq!(a.docs[0].lines, vec!["adef", "gjkl", "mpqr"]);
        assert_eq!(a.docs[0].cursor, [0, 1], "光标回到矩形左上角");
    }

    #[test]
    fn column_replace_opens_with_the_block_text_and_applies_per_row() {
        let (mut a, _g) = app(Settings::default());
        a.docs[0].lines = vec!["abcdef".into(), "ghijkl".into()];
        a.column_replace();
        assert!(!a.tools.col_open, "没有列块选区: 不弹窗");
        a.docs[0].box_sel = Some([0, 1, 1, 3]);
        a.docs[0].cursor = [1, 3];
        a.column_replace();
        assert!(a.tools.col_open);
        assert_eq!((a.docs[0].box_sel, a.docs[0].cursor), (Some([0, 1, 1, 3]), [1, 3]), "取预填文字不能弄丢列块选区和光标");
        assert!(a.docs[0].rec.is_none() && a.docs[0].lines == vec!["abcdef", "ghijkl"]);
        a.docs[0].edit("box-replace", Some("12\nXY".into()));
        assert_eq!(a.docs[0].lines, vec!["a12def", "gXYjkl"], "每一行换成对应的一行文字");
    }

    #[test]
    fn doc_prefs_follow_settings_and_extension_overrides() {
        let mut st = Settings::default();
        st.auto_indent = false; st.smart_indent = true; st.insert_spaces = false;
        st.file_types = vec![TypePref { ext: "py".into(), insert_spaces: Some(true), tab_width: Some(2), wrap: Some(true), wrap_col: None }];
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
    fn right_dock_cap_leaves_editor_room() {
        assert_eq!(right_cap(900.0), 660.0);
        assert_eq!(right_cap(300.0), 100.0, "窗口再窄也不会把上限压到 0");
        assert!(right_room(560.0) && !right_room(399.0));
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
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
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
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
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
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
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
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
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

    #[test]
    fn session_is_restored_only_without_command_line_files() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
        let dir = std::env::temp_dir().join(format!("idefast-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        std::fs::write(&a, "aaa").unwrap();
        std::fs::write(&b, "bbb").unwrap();
        let mut st = Settings::default();
        st.session = vec![a.display().to_string(), dir.join("gone.txt").display().to_string(), b.display().to_string(), "\\\\evil\\share\\x.txt".into()];
        st.session_active = 2;
        let app1 = App::build(vec![], None, Trace::new(), st.clone(), false);
        let names: Vec<_> = app1.docs.iter().map(|d| d.name.clone()).collect();
        assert_eq!(names, vec!["a.txt", "b.txt"], "不存在的 / UNC 的跳过");
        assert_eq!(app1.docs[app1.active].name, "b.txt", "活动标签回到上次的位置");
        // 往回写: 当前打开的文件就是下次的会话
        let now = app1.current_settings();
        assert_eq!(now.session, vec![a.display().to_string(), b.display().to_string()]);
        assert_eq!(now.session_active, 1);
        // 命令行给了文件 = 不恢复; 关掉开关 = 不恢复
        let app2 = App::build(vec![a.display().to_string()], None, Trace::new(), st.clone(), false);
        assert_eq!(app2.docs.len(), 1);
        st.restore_session = false;
        assert_eq!(App::build(vec![], None, Trace::new(), st, false).docs.len(), 1, "只有一份空白文档");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_snapshot_survives_the_close_confirmation_loop() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
        let dir = std::env::temp_dir().join(format!("idefast-exit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let mut st = Settings::default();
        st.session = vec![a.display().to_string(), b.display().to_string()];
        st.session_active = 1;
        let mut app = App::build(vec![], None, Trace::new(), st, false);
        // 关窗请求: 先冻结, 之后确认循环把标签一个个关掉也不影响要写下去的会话
        app.exit_session = Some((app.session_paths(), app.session_active()));
        app.docs.truncate(1);
        let now = app.current_settings();
        assert_eq!(now.session.len(), 2, "两个文件都在会话里");
        assert_eq!(now.session_active, 1);
        // 用户取消关窗: 快照清掉, 会话又跟着现在打开的文件走
        app.exit_session = None;
        assert_eq!(app.current_settings().session.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 100 万字符的单行: 一帧能画完, 光标在行中间 / 选区 / 查找命中都不 panic, 撑宽用整行长度
    #[test]
    fn a_million_char_single_line_renders_with_caret_selection_and_marks() {
        let (mut a, _g) = app(Settings::default());
        let ctx = egui::Context::default();
        a.docs[0].lines = vec!["word ".repeat(200_000)];
        a.docs[0].goto(0, 500_000);
        a.docs[0].sel = Some([0, 499_990, 0, 500_010]);
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| a.ui_main(ui));
            out.textures_delta.clear();
        }
        assert!(a.max_cols >= 1_000_000, "横向滚动区按整行长度撑开, 不是按窗口");
    }
}
