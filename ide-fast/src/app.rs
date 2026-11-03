//! 应用外壳: 菜单 / 标签 / 编辑区 / 查找条 / 状态栏。编辑语义都在 `doc.rs` (-> editcore), 这里只管画和收输入。

use crate::doc::{expand_tabs, Doc};
use crate::cmds::{self, Action};
use crate::find::Find;
use crate::grep::{self, Ev as GEv};
use crate::plugins;
use crate::settings::Settings;
use crate::sidebar::{Sidebar, Tab};
use crate::terminal::Terminal;
use crate::Trace;
use eframe::egui::{self, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Vec2};
use editcore::syntax_styling::Theme;
use editcore::type_config::{TypeConfig, TypeRegistry};
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
    prefs_open: bool,
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
}

impl App {
    pub fn new(args: Vec<String>, inbox: Option<std::sync::mpsc::Receiver<Vec<String>>>, trace: Trace) -> Self {
        let mut app = App {
            docs: vec![], active: 0, next_id: 1, untitled_n: 0,
            find: Find::new(),
            dark: true, font_px: 15.0, tab: 4, highlight: true, recent: vec![], saved: Settings::default(), registry: TypeRegistry::default_registry(), side: Sidebar::new(), term: Terminal::new(), goto_open: false, was_focused: true, goto_text: String::new(), theme_id: String::new(), theme_visuals: None, split_on: false, left_doc: 0, right_doc: 0, focus_right: false, pane_keys: true, term_stdin: false, term_replace: false, md_preview: false, wrap: false, goal_x: None, inbox, prefs_open: false, wrap_cache: None, max_cols: 80, trace, first_frame: true, confirm_exit: false, preedit: String::new(), center_once: false, grep_doc: None, jump_req: None, editor_ids: vec![],
        };
        let st = Settings::load();
        app.dark = st.dark;
        if !st.theme_plugin.is_empty() { app.set_theme_plugin(&st.theme_plugin.clone()); }
        app.font_px = st.font_px;
        app.tab = st.tab_width;
        app.highlight = st.highlight;
        app.wrap = st.wrap;
        app.recent = st.recent.clone();
        app.apply_search_prefs(&st);
        app.saved = st;
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
        self.docs.push(Doc::untitled(id, n));
        self.active = self.docs.len() - 1;
    }

    fn open_path(&mut self, p: &std::path::Path) {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(p)) {
            self.active = i;
            return;
        }
        if std::fs::metadata(p).map(|m| m.len() > crate::doc::SOFT_LIMIT && m.len() <= crate::doc::HARD_LIMIT).unwrap_or(false) {
            let mb = std::fs::metadata(p).map(|m| m.len() / 1024 / 1024).unwrap_or(0);
            let go = rfd::MessageDialog::new().set_title("较大的文件").set_description(format!("{}
约 {mb} MB, 打开可能比较慢。继续吗?", p.display())).set_buttons(rfd::MessageButtons::YesNo).show();
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
                let mut st = self.current_settings();
                st.push_recent(p);
                self.recent = st.recent.clone();
                self.saved = st.clone();
                st.save();
            }
            Err(e) => {
                rfd::MessageDialog::new().set_title("打开失败").set_description(format!("{}\n{e}", p.display())).show();
            }
        }
    }

    fn current_settings(&self) -> Settings {
        let (f, g) = (&self.find, &self.side.grep.o);
        let search = crate::settings::SearchPrefs { find_case: f.opts.case, find_regex: f.opts.regex, find_word: f.opts.word, find_wrap: f.opts.wrap, find_marks: f.marks, grep_case: g.case, grep_regex: g.regex, grep_word: g.word, grep_sub: g.sub, grep_outside: g.outside, grep_files: g.files.clone(), grep_exc_files: g.exc_files.clone(), grep_exc_folders: g.exc_folders.clone() };
        Settings { dark: self.dark, font_px: self.font_px, tab_width: self.tab, highlight: self.highlight, wrap: self.wrap, theme_plugin: self.theme_id.clone(), recent: self.recent.clone(), search }
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
            now.save();
            self.saved = now;
        }
    }

    /// 命令行 / 别的实例转来的 `路径[:行[:列]]`
    fn open_spec(&mut self, spec: &str) {
        let (path, line, col) = editcore::text_util::split_path_line_col(spec);
        let p = PathBuf::from(&path);
        if !p.is_file() {
            self.cur_or_new().tip = format!("找不到文件: {path}");
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

    /// 首选项窗口: 改了立刻生效、立刻落盘 (persist_if_changed)
    fn prefs_window(&mut self, ctx: &egui::Context) {
        let mut open = self.prefs_open;
        egui::Window::new("首选项").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.checkbox(&mut self.dark, "深色主题");
            ui.checkbox(&mut self.highlight, "语法高亮");
            ui.checkbox(&mut self.wrap, "自动换行");
            ui.horizontal(|ui| { ui.label("字号"); ui.add(egui::Slider::new(&mut self.font_px, 8.0..=40.0)); });
            ui.horizontal(|ui| { ui.label("Tab 宽度"); ui.add(egui::Slider::new(&mut self.tab, 1..=16)); });
            if ui.button("恢复默认").clicked() {
                let d = Settings::default();
                self.dark = d.dark; self.font_px = d.font_px; self.tab = d.tab_width; self.highlight = d.highlight; self.wrap = d.wrap;
            }
            ui.separator();
            ui.small(format!("设置文件: {}", crate::settings::path().map(|p| p.display().to_string()).unwrap_or_else(|| "(未启用落盘)".into())));
        });
        self.prefs_open = open;
    }

    fn open_folder_dialog(&mut self) {
        if let Some(d) = rfd::FileDialog::new().pick_folder() {
            self.side.set_root(d);
        }
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
        match self.cur().save_to(&path) {
            Ok(()) => {
                self.cur().tip = format!("已保存 {}", path.display());
                true
            }
            Err(e) => {
                rfd::MessageDialog::new().set_title("保存失败").set_description(e).show();
                false
            }
        }
    }

    /// 关标签: 有未保存修改先问; 返回是否真的关了
    fn close_tab(&mut self, i: usize) -> bool {
        if self.docs[i].dirty {
            let r = rfd::MessageDialog::new().set_title("未保存的修改").set_description(format!("「{}」有未保存的修改, 保存吗?", self.docs[i].name))
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
        self.docs.remove(i);
        self.split_on = false;
        if self.docs.is_empty() {
            self.new_doc();
        }
        self.active = self.active.min(self.docs.len() - 1);
        true
    }

    fn menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("文件", |ui| {
                if ui.button("新建  Ctrl+N").clicked() { self.new_doc(); ui.close(); }
                if ui.button("打开…  Ctrl+O").clicked() { ui.close(); self.open_dialog(); }
                if ui.button("打开文件夹…").clicked() { ui.close(); self.open_folder_dialog(); }
                ui.menu_button("最近使用", |ui| {
                    if self.recent.is_empty() { ui.label("(没有)"); }
                    let mut pick: Option<String> = None;
                    for r in &self.recent {
                        if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
                    }
                    if let Some(r) = pick { self.open_path(std::path::Path::new(&r)); }
                });
                if ui.button("保存  Ctrl+S").clicked() { ui.close(); self.save(false); }
                if ui.button("另存为…").clicked() { ui.close(); self.save(true); }
                if ui.button("重新载入 (丢弃修改)").clicked() { ui.close(); self.reload_current(); }
                if ui.button("打印 / 打印预览…  Ctrl+P").clicked() { ui.close(); self.print_cur(); }
                if ui.button("关闭标签  Ctrl+W").clicked() { ui.close(); let a = self.active; self.close_tab(a); }
                ui.separator();
                if ui.button("退出").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
            });
            ui.menu_button("编辑", |ui| {
                if ui.button("撤销  Ctrl+Z").clicked() { self.cur().undo(); ui.close(); }
                if ui.button("重做  Ctrl+Y").clicked() { self.cur().redo(); ui.close(); }
                ui.separator();
                if ui.button("剪切  Ctrl+X").clicked() { if let Some(c) = self.cur().edit("cut", None) { ctx.copy_text(c); } ui.close(); }
                if ui.button("复制  Ctrl+C").clicked() { if let Some(c) = self.cur().edit("copy", None) { ctx.copy_text(c); } ui.close(); }
                if ui.button("粘贴  Ctrl+V").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste); ui.close(); }
                if ui.button("全选  Ctrl+A").clicked() { self.cur().select_all(); ui.close(); }
                ui.separator();
                self.cmd_menu(ui, ctx);
            });
            ui.menu_button("查找", |ui| {
                if ui.button("查找…  Ctrl+F").clicked() { self.open_find(false); ui.close(); }
                if ui.button("置换…  Ctrl+H / Ctrl+R").clicked() { self.open_find(true); ui.close(); }
                if ui.button("在文件夹中搜索 (Grep)…  Ctrl+G").clicked() { self.focus_grep(); ui.close(); }
                if ui.button("下一个  F3").clicked() { self.find_next(true); ui.close(); }
                if ui.button("上一个  Shift+F3").clicked() { self.find_next(false); ui.close(); }
                if ui.button("查找光标处的单词  Ctrl+F3").clicked() { let i = self.active.min(self.docs.len() - 1); self.find.word_search(&mut self.docs[i], true); ui.close(); }
                if ui.button("全部置换  Ctrl+Alt+R").clicked() { let i = self.active.min(self.docs.len() - 1); self.find.replace_all(&mut self.docs[i]); ui.close(); }
            });
            ui.menu_button("折叠", |ui| {
                if ui.button("折叠 / 展开当前行  Ctrl+Shift+[").clicked() { let d = self.cur(); let r = d.cursor[0]; d.toggle_fold(r); ui.close(); }
                if ui.button("全部展开  Ctrl+Shift+]").clicked() { self.cur().folds.clear(); ui.close(); }
            });
            ui.menu_button("插件", |ui| {
                let rows = plugins::rows();
                if ui.radio(self.theme_id.is_empty(), "内置主题 (深 / 浅色)").clicked() { self.set_theme_plugin(""); ui.close(); }
                for r in &rows {
                    ui.horizontal(|ui| {
                        if let Some(t) = &r.theme {
                            if ui.radio(self.theme_id == r.id, format!("主题: {}", t.label)).clicked() { self.set_theme_plugin(&r.id); ui.close(); }
                        } else {
                            ui.label(format!("{} {}{}", r.name, r.version, if r.script_only { "  (脚本插件, 高速版不运行)" } else { "" }));
                        }
                        if ui.small_button("删除").clicked() {
                            let _ = plugins::remove(&r.id);
                            if self.theme_id == r.id { self.set_theme_plugin(""); }
                            ui.close();
                        }
                    });
                }
                ui.separator();
                if ui.button("导入插件 (.zip)…").clicked() { ui.close(); self.import_plugin_zip(); }
                let gone = plugins::removed_bundled();
                if !gone.is_empty() {
                    ui.menu_button("恢复内置插件", |ui| {
                        for (id, name, _) in gone {
                            if ui.button(name).clicked() { let _ = plugins::restore(&id); ui.close(); }
                        }
                    });
                }
            });
            ui.menu_button("设置", |ui| {
                if ui.button("首选项…").clicked() { self.prefs_open = true; ui.close(); }
            });
            ui.menu_button("书签", |ui| {
                if ui.button("设置 / 取消  Ctrl+F2").clicked() { self.cur().bookmark("toggle"); ui.close(); }
                if ui.button("下一个  F2").clicked() { self.cur().bookmark("next"); ui.close(); }
                if ui.button("上一个  Shift+F2").clicked() { self.cur().bookmark("prev"); ui.close(); }
            });
            ui.menu_button("宏", |ui| {
                let rec = self.cur().rec.is_some();
                if ui.button(if rec { "结束录制  Ctrl+Shift+M" } else { "开始录制  Ctrl+Shift+M" }).clicked() { self.cur().macro_toggle(); ui.close(); }
                if ui.button("回放  Ctrl+Shift+L").clicked() { self.cur().macro_play(); ui.close(); }
                ui.separator();
                if ui.button("运行当前文档 (Sakura .mac)").clicked() { ui.close(); self.run_mac_doc(); }
                if ui.button("运行 .mac 文件…").clicked() { ui.close(); self.run_mac_file(); }
            });
            ui.menu_button("视图", |ui| {
                if ui.checkbox(&mut self.split_on.clone(), "分割窗口  Ctrl+\\").clicked() { self.toggle_split(); ui.close(); }
                ui.checkbox(&mut self.term.open, "终端  Ctrl+`");
                ui.checkbox(&mut self.md_preview, "Markdown 预览  Ctrl+Shift+V");
                if ui.checkbox(&mut self.dark, "深色主题").clicked() { ui.close(); }
                ui.checkbox(&mut self.highlight, "语法高亮");
                ui.checkbox(&mut self.wrap, "自动换行  Alt+Z");
                ui.checkbox(&mut self.side.open, "侧栏  Ctrl+B");
                ui.menu_button("Tab 宽度", |ui| {
                    for w in [2usize, 3, 4, 8] {
                        if ui.radio_value(&mut self.tab, w, format!("{w}")).clicked() { ui.close(); }
                    }
                });
                if ui.button("放大  Ctrl+=").clicked() { self.font_px = (self.font_px + 1.0).min(40.0); }
                if ui.button("缩小  Ctrl+-").clicked() { self.font_px = (self.font_px - 1.0).max(8.0); }
            });
            ui.menu_button("帮助", |ui| {
                ui.label("IDE1.0 高速版 — 原生 Rust (egui)\n编辑语义复用 editcore (与 Tauri 版相同)");
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
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::S) { self.save(true); } else if cmd(ctx, Modifiers::COMMAND, Key::S) { self.save(false); }
        if cmd(ctx, Modifiers::COMMAND, Key::W) { let a = self.active; self.close_tab(a); }
        // 搜索: Ctrl+F 查找, Ctrl+H / Ctrl+R 置换, Ctrl+Alt+R 全部置换, Ctrl+G / Ctrl+Shift+F Grep (键位同 Tauri 版)
        if cmd(ctx, Modifiers::COMMAND | Modifiers::ALT, Key::R) { let i = self.active.min(self.docs.len() - 1); self.find.replace_all(&mut self.docs[i]); }
        else if cmd(ctx, Modifiers::COMMAND | Modifiers::ALT, Key::G) || cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::F) || cmd(ctx, Modifiers::COMMAND, Key::G) { self.focus_grep(); }
        else if cmd(ctx, Modifiers::COMMAND, Key::H) || cmd(ctx, Modifiers::COMMAND, Key::R) { self.open_find(true); }
        else if cmd(ctx, Modifiers::COMMAND, Key::F) || cmd(ctx, Modifiers::COMMAND, Key::I) { self.open_find(false); }
        if cmd(ctx, Modifiers::COMMAND, Key::B) { self.side.open = !self.side.open; }
        if cmd(ctx, Modifiers::ALT, Key::Z) { self.wrap = !self.wrap; }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::OpenBracket) { let d = self.cur(); let r = d.cursor[0]; d.toggle_fold(r); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::CloseBracket) { self.cur().folds.clear(); }
        if cmd(ctx, Modifiers::COMMAND, Key::P) { self.print_cur(); }
        if cmd(ctx, Modifiers::COMMAND, Key::Backslash) { self.toggle_split(); }
        if cmd(ctx, Modifiers::COMMAND, Key::Backtick) { self.term.open = !self.term.open; }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::V) { self.md_preview = !self.md_preview; }
        if cmd(ctx, Modifiers::COMMAND, Key::F2) { self.cur().bookmark("toggle"); }
        if cmd(ctx, Modifiers::SHIFT, Key::F2) { self.cur().bookmark("prev"); } else if cmd(ctx, Modifiers::NONE, Key::F2) { self.cur().bookmark("next"); }
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
            Err(e) => { rfd::MessageDialog::new().set_title("无法导入").set_description(e).show(); return; }
            Ok((note, perms)) => {
                let ok = rfd::MessageDialog::new().set_title("导入插件").set_description(format!("{note}\n\n来源: {} (未验证)\n导入吗?", path.display())).set_buttons(rfd::MessageButtons::YesNo).show() == rfd::MessageDialogResult::Yes;
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
        let label = |c: &cmds::Cmd| if c.keys.is_empty() || c.label.contains("Ctrl+") { c.label.to_string() } else { format!("{}  {}", c.label, c.keys) };
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
                ui.menu_button(g, |ui| {
                    for c in cmds::CMDS.iter().filter(|c| c.group == g) {
                        if ui.button(label(c)).clicked() { self.run_action(c.action, ctx); ui.close(); }
                    }
                });
            }
        }
    }

    fn run_action(&mut self, a: Action, ctx: &egui::Context) {
        let tab = self.tab;
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
        egui::Window::new("转到行").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            let total = self.docs[self.active.min(self.docs.len() - 1)].lines.len();
            let r = ui.add(egui::TextEdit::singleline(&mut self.goto_text).hint_text(format!("1 – {total}")).desired_width(120.0));
            r.request_focus();
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                go = self.goto_text.trim().parse::<usize>().ok();
                if go.is_none() { self.cur().tip = "请输入行号".into(); }
                self.goto_open = false;
            }
        });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) { self.goto_open = false; }
        if let Some(n) = go { let d = self.cur(); let row = n.saturating_sub(1).min(d.lines.len() - 1); d.goto(row, 0); }
    }

    /// 窗口重新获得焦点时: 看每个文档的文件有没有被外部改过; 没动过就静默重载, 有未保存修改就问
    fn check_external(&mut self) {
        for i in 0..self.docs.len() {
            if !self.docs[i].changed_on_disk() { continue; }
            let name = self.docs[i].name.clone();
            let reload = if !self.docs[i].dirty {
                true
            } else {
                rfd::MessageDialog::new().set_title("文件被外部修改").set_description(format!("「{name}」在磁盘上被修改了, 而你这里也有未保存的修改。
重新载入会丢弃你的修改。重新载入吗?")).set_buttons(rfd::MessageButtons::YesNo).show() == rfd::MessageDialogResult::Yes
            };
            if reload {
                if let Err(e) = self.docs[i].reload() { self.docs[i].tip = e; }
                else { self.docs[i].tip = "文件在外部被修改, 已重新载入".into(); }
            } else {
                self.docs[i].stamp = crate::doc::stamp_of(self.docs[i].path.as_deref().unwrap_or(std::path::Path::new("")));
            }
        }
    }

    fn reload_current(&mut self) {
        let (dirty, name) = { let d = self.cur(); (d.dirty, d.name.clone()) };
        if dirty && rfd::MessageDialog::new().set_title("重新载入").set_description(format!("丢弃「{name}」未保存的修改并从磁盘重新载入吗?")).set_buttons(rfd::MessageButtons::YesNo).show() != rfd::MessageDialogResult::Yes { return; }
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
        let font = FontId::monospace(self.font_px);
        let (row_h, char_w) = ctx.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
        let text_color = ui.visuals().text_color();
        let weak = ui.visuals().weak_text_color();
        let sel_bg = ui.visuals().selection.bg_fill;
        let max_cols = self.max_cols;
        let tab = self.tab;
        let ai = self.active.min(self.docs.len() - 1);
        let theme = if self.dark { Theme::dark() } else { Theme::light() };
        let marks = self.find.marks_for(&self.docs[ai]);
        let mark_bg = Color32::from_rgba_unmultiplied(255, 200, 0, 70);
        let cfg: Option<TypeConfig> = if self.highlight { self.registry.detect(&self.docs[ai].name).cloned() } else { None };
        let digits = format!("{}", self.docs[ai].lines.len()).len().max(3);
        let gutter = digits as f32 * char_w + 18.0;
        let pad = 6.0;
        // 自动换行: 每个逻辑行按可用宽度折成若干显示行, 行高不等 -> 记累计高度 (太长的文档不换行, 免得每次排版都要扫全篇)
        let wrap = self.wrap && self.docs[ai].lines.len() <= 20000;
        let wrap_w = if wrap { (ui.available_width() - gutter - pad * 2.0 - 16.0).max(char_w * 8.0) } else { f32::INFINITY };
        let use_offs = wrap || !self.docs[ai].folds.is_empty();
        let offs: Vec<f32> = if use_offs {
            let fsig: usize = self.docs[ai].folds.iter().map(|f| f.0 * 31 + f.1).sum::<usize>() + self.docs[ai].folds.len();
            let key = (self.docs[ai].id, self.docs[ai].rev, wrap_w as i32 + fsig as i32 * 7, (self.font_px * 10.0) as i32, tab + (wrap as usize) * 100);
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
                let mut job = highlight_job(&d.lines[row], &disp, &map, cfg.as_ref(), &theme, &font, text_color);
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
                    painter.text(Pos2::new(rect.min.x + 2.0, y), egui::Align2::LEFT_TOP, "▸", font.clone(), weak);
                }
                if d.bookmarks.iter().any(|b| b.row == row) {
                    painter.circle_filled(Pos2::new(rect.min.x + 7.0, y + row_h / 2.0), 3.5, Color32::from_rgb(80, 170, 255));
                }
                painter.text(Pos2::new(rect.min.x + gutter - 8.0, y), egui::Align2::RIGHT_TOP, format!("{}", row + 1), font.clone(), weak);
            }
            // 光标
            let [cr, cc] = d.cursor;
            if cr < d.lines.len() {
                let (disp, map) = expand_tabs(&d.lines[cr], tab);
                let (cx, cy) = if wrap {
                    let mut job = highlight_job(&d.lines[cr], &disp, &map, None, &theme, &font, text_color);
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
const GROUP_NAMES: [&str; 6] = ["不分组", "红组", "橙组", "绿组", "蓝组", "紫组"];
const GROUP_COLORS: [Option<Color32>; 6] = [None, Some(Color32::from_rgb(230, 80, 80)), Some(Color32::from_rgb(235, 150, 50)), Some(Color32::from_rgb(80, 190, 110)), Some(Color32::from_rgb(80, 150, 235)), Some(Color32::from_rgb(170, 110, 225))];

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
fn highlight_job(line: &str, disp: &str, map: &[usize], cfg: Option<&TypeConfig>, theme: &Theme, font: &FontId, default: Color32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat { font_id: font.clone(), color: c, ..Default::default() };
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
        ctx.set_visuals(match &self.theme_visuals { Some(v) => v.clone(), None => if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() } });
        // 关窗口: 有未保存修改先问
        if ctx.input(|i| i.viewport().close_requested()) && !self.confirm_exit && self.docs.iter().any(|d| d.dirty) {
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
        if focused && !self.was_focused { self.check_external(); }
        self.was_focused = focused;
        if self.prefs_open { self.prefs_window(ctx); }
        if self.goto_open { self.goto_window(ctx); }
        if self.split_on {
            let last = self.docs.len() - 1;
            self.left_doc = self.left_doc.min(last);
            self.right_doc = self.right_doc.min(last);
            self.active = if self.focus_right { self.right_doc } else { self.left_doc };
        }
        self.shortcuts(ctx);
        egui::Panel::top("menu").show(root, |ui| self.menu(ui, ctx));
        egui::Panel::top("tabs").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut close: Option<usize> = None;
                for i in 0..self.docs.len() {
                    let label = format!("{}{}", if self.docs[i].dirty { "● " } else { "" }, self.docs[i].name);
                    let gcol = GROUP_COLORS.get(self.docs[i].group as usize).copied().flatten();
                    let text = match gcol { Some(c) => egui::RichText::new(format!("● {label}")).color(c), None => egui::RichText::new(label) };
                    let tab_resp = ui.selectable_label(i == self.active, text);
                    if tab_resp.clicked() { self.active = i; }
                    tab_resp.context_menu(|ui| {
                        for (g, name) in GROUP_NAMES.iter().enumerate() {
                            if ui.selectable_label(self.docs[i].group as usize == g, *name).clicked() { self.docs[i].group = g as u8; ui.close(); }
                        }
                    });
                    if ui.small_button("×").clicked() { close = Some(i); }
                    ui.add_space(6.0);
                }
                if let Some(i) = close { self.close_tab(i); }
            });
        });
        if self.split_on {
            if self.focus_right { self.right_doc = self.active; } else { self.left_doc = self.active; }
        }
        if self.side.open {
            egui::Panel::left("side").resizable(true).default_size(250.0).show(root, |ui| self.side_ui(ui));
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
        egui::Panel::bottom("status").show(root, |ui| {
            let d = &self.docs[self.active.min(self.docs.len() - 1)];
            ui.horizontal(|ui| {
                ui.label(format!("Ln {}, Col {}", d.cursor[0] + 1, d.cursor[1] + 1));
                ui.separator();
                ui.label(d.charset.label());
                ui.label(d.eol.label());
                ui.label(format!("共 {} 行", d.lines.len()));
                if d.overwrite { ui.label("改写"); }
                if d.readonly { ui.label("只读"); }
                if d.rec.is_some() { ui.colored_label(Color32::from_rgb(230, 70, 70), "● REC"); }
                if d.box_sel.is_some() { ui.label("[矩形]"); }
                if !d.extra.is_empty() { ui.label(format!("{} 个光标", d.extra.len() + 1)); }
                ui.separator();
                ui.label(&d.tip);
            });
        });
        if self.find.open {
            egui::Panel::bottom("find").show(root, |ui| {
                let i = self.active.min(self.docs.len() - 1);
                self.find.bar(ui, &mut self.docs[i]);
            });
        }
        // 查找栏 / 侧栏输入框 / 对话框正在收键盘时, 编辑区不抢
        let find_focus = ctx.memory(|m| m.focused()).map(|f| !self.editor_ids.contains(&f)).unwrap_or(false);
        if self.term.open {
            egui::Panel::bottom("term").resizable(true).default_size(180.0).show(root, |ui| self.term_ui(ui));
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
