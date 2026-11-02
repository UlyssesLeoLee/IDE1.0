//! 应用外壳: 菜单 / 标签 / 编辑区 / 查找条 / 状态栏。编辑语义都在 `doc.rs` (-> editcore), 这里只管画和收输入。

use crate::doc::{expand_tabs, Doc};
use crate::settings::Settings;
use crate::sidebar::{Sidebar, Tab};
use crate::Trace;
use eframe::egui::{self, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Vec2};
use editcore::syntax_styling::Theme;
use editcore::type_config::{TypeConfig, TypeRegistry};
use std::path::PathBuf;

struct Find {
    open: bool,
    needle: String,
    repl: String,
    case: bool,
    regex: bool,
    msg: String,
    focus: bool,
}

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
    max_cols: usize,
    trace: Trace,
    first_frame: bool,
    confirm_exit: bool,
    /// 输入法正在组字的文字 (还没提交): 画在光标处, 不进文档
    preedit: String,
}

impl App {
    pub fn new(files: Vec<PathBuf>, trace: Trace) -> Self {
        let mut app = App {
            docs: vec![], active: 0, next_id: 1, untitled_n: 0,
            find: Find { open: false, needle: String::new(), repl: String::new(), case: false, regex: false, msg: String::new(), focus: false },
            dark: true, font_px: 15.0, tab: 4, highlight: true, recent: vec![], saved: Settings::default(), registry: TypeRegistry::default_registry(), side: Sidebar::new(), max_cols: 80, trace, first_frame: true, confirm_exit: false, preedit: String::new(),
        };
        let st = Settings::load();
        app.dark = st.dark;
        app.font_px = st.font_px;
        app.tab = st.tab_width;
        app.highlight = st.highlight;
        app.recent = st.recent.clone();
        app.saved = st;
        for f in files {
            app.open_path(&f);
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
        Settings { dark: self.dark, font_px: self.font_px, tab_width: self.tab, highlight: self.highlight, recent: self.recent.clone() }
    }

    /// 设置变了就落盘 (每帧比较一次, 只有真变了才写)
    fn persist_if_changed(&mut self) {
        let now = self.current_settings();
        if now != self.saved {
            now.save();
            self.saved = now;
        }
    }

    fn open_folder_dialog(&mut self) {
        if let Some(d) = rfd::FileDialog::new().pick_folder() {
            self.side.set_root(d);
        }
    }

    /// 打开文件并跳到某行 (Grep 结果 / 命令行 `文件:行`)
    fn open_at(&mut self, p: &std::path::Path, line: usize) {
        self.open_path(p);
        if self.docs.iter().any(|d| d.path.as_deref() == Some(p)) {
            self.cur().goto(line.saturating_sub(1), 0);
        }
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
                if self.side.root.is_none() {
                    ui.label("先在「文件」里打开一个文件夹, 再在它下面搜索。");
                    if ui.button("打开文件夹…").clicked() { self.open_folder_dialog(); }
                } else {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.side.grep.pattern).hint_text("搜索内容").desired_width(f32::INFINITY));
                    ui.add(egui::TextEdit::singleline(&mut self.side.grep.include).hint_text("文件名过滤, 如 *.rs, *.md").desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.side.grep.case, "区分大小写");
                        ui.checkbox(&mut self.side.grep.regex, "正则");
                    });
                    let go = ui.button("搜索").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                    if go { self.side.start_grep(); }
                    ui.label(&self.side.grep.msg);
                    let root = self.side.root.clone().unwrap_or_default();
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        for h in &self.side.grep.results {
                            let rel = std::path::Path::new(&h.path).strip_prefix(&root).map(|p| p.display().to_string()).unwrap_or_else(|_| h.path.clone());
                            let t: String = h.text.trim().chars().take(80).collect();
                            if ui.selectable_label(false, format!("{rel}:{}  {t}", h.line)).clicked() { open = Some((PathBuf::from(&h.path), h.line as usize)); }
                        }
                    });
                }
            }
        }
        if let Some((p, l)) = open { self.open_at(&p, l); }
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
                if ui.button("复制当前行  Ctrl+D").clicked() { self.cur().edit("duplicate-line", None); ui.close(); }
                if ui.button("删除当前行  Ctrl+Shift+K").clicked() { self.cur().edit("delete-line", None); ui.close(); }
            });
            ui.menu_button("查找", |ui| {
                if ui.button("查找 / 置换…  Ctrl+F").clicked() { self.find.open = true; self.find.focus = true; ui.close(); }
                if ui.button("在文件夹中搜索 (Grep)…  Ctrl+Shift+F").clicked() { self.side.open = true; self.side.tab = Tab::Search; ui.close(); }
                if ui.button("下一个  F3").clicked() { self.find_next(true); ui.close(); }
                if ui.button("上一个  Shift+F3").clicked() { self.find_next(false); ui.close(); }
            });
            ui.menu_button("视图", |ui| {
                if ui.checkbox(&mut self.dark, "深色主题").clicked() { ui.close(); }
                ui.checkbox(&mut self.highlight, "语法高亮");
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

    fn find_next(&mut self, fwd: bool) {
        let (n, c, r) = (self.find.needle.clone(), self.find.case, self.find.regex);
        match self.cur().find_step(&n, c, r, fwd) {
            Ok(_) => self.find.msg = self.cur().tip.clone(),
            Err(e) => self.find.msg = e,
        }
    }

    fn find_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("查找");
            let r = ui.add(egui::TextEdit::singleline(&mut self.find.needle).desired_width(200.0));
            if self.find.focus {
                r.request_focus();
                self.find.focus = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let fwd = !ui.input(|i| i.modifiers.shift);
                self.find_next(fwd);
                r.request_focus();
            }
            if ui.button("↓").clicked() { self.find_next(true); }
            if ui.button("↑").clicked() { self.find_next(false); }
            ui.label("置换");
            ui.add(egui::TextEdit::singleline(&mut self.find.repl).desired_width(140.0));
            if ui.button("全部置换").clicked() {
                let (n, rp, c, rg) = (self.find.needle.clone(), self.find.repl.clone(), self.find.case, self.find.regex);
                self.find.msg = match self.cur().replace_all(&n, &rp, c, rg) { Ok(k) => format!("置换了 {k} 处"), Err(e) => e };
            }
            ui.checkbox(&mut self.find.case, "区分大小写");
            ui.checkbox(&mut self.find.regex, "正则");
            ui.label(&self.find.msg);
            if ui.button("×").clicked() { self.find.open = false; }
        });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |ctx: &egui::Context, m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        if cmd(ctx, Modifiers::COMMAND, Key::N) { self.new_doc(); }
        if cmd(ctx, Modifiers::COMMAND, Key::O) { self.open_dialog(); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::S) { self.save(true); } else if cmd(ctx, Modifiers::COMMAND, Key::S) { self.save(false); }
        if cmd(ctx, Modifiers::COMMAND, Key::W) { let a = self.active; self.close_tab(a); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::F) { self.side.open = true; self.side.tab = Tab::Search; } else if cmd(ctx, Modifiers::COMMAND, Key::F) { self.find.open = true; self.find.focus = true; }
        if cmd(ctx, Modifiers::COMMAND, Key::B) { self.side.open = !self.side.open; }
        if cmd(ctx, Modifiers::SHIFT, Key::F3) { self.find_next(false); } else if cmd(ctx, Modifiers::NONE, Key::F3) { self.find_next(true); }
        if cmd(ctx, Modifiers::COMMAND, Key::Equals) { self.font_px = (self.font_px + 1.0).min(40.0); }
        if cmd(ctx, Modifiers::COMMAND, Key::Minus) { self.font_px = (self.font_px - 1.0).max(8.0); }
        if cmd(ctx, Modifiers::COMMAND, Key::Z) { self.cur().undo(); }
        if cmd(ctx, Modifiers::COMMAND, Key::Y) { self.cur().redo(); }
        if cmd(ctx, Modifiers::COMMAND, Key::A) { self.cur().select_all(); }
        if cmd(ctx, Modifiers::COMMAND, Key::D) { self.cur().edit("duplicate-line", None); }
        if cmd(ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::K) { self.cur().edit("delete-line", None); }
        if cmd(ctx, Modifiers::ALT, Key::ArrowUp) { self.cur().edit("move-line-up", None); }
        if cmd(ctx, Modifiers::ALT, Key::ArrowDown) { self.cur().edit("move-line-down", None); }
    }

    /// 编辑区: 输入事件 -> doc; 然后只画看得见的行
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
        let cfg: Option<TypeConfig> = if self.highlight { self.registry.detect(&self.docs[ai].name).cloned() } else { None };
        let digits = format!("{}", self.docs[ai].lines.len()).len().max(3);
        let gutter = digits as f32 * char_w + 18.0;
        let pad = 6.0;
        let total_h = self.docs[ai].lines.len() as f32 * row_h + row_h * 2.0;
        let page_rows = ((ui.available_height() / row_h) as usize).max(1);

        // ---- 键盘 (查找框有焦点时不抢) ----
        let mut clip_out: Option<String> = None;
        let mut preedit_now = self.preedit.clone();
        if !find_has_focus {
            let events = ctx.input(|i| i.events.clone());
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
                            Key::Enter => { d.edit("new-line", None); }
                            Key::Backspace => { d.edit(if ctrl { "delete-word-start" } else { "backspace" }, None); }
                            Key::Delete => { d.edit(if ctrl { "delete-word-end" } else { "delete" }, None); }
                            Key::Tab => { d.type_text("\t"); }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(c) = clip_out { ctx.copy_text(c); }
        self.preedit = preedit_now;
        let preedit = self.preedit.clone();

        // ---- 绘制 ----
        let reveal = std::mem::take(&mut self.docs[ai].reveal);
        let mut widest = max_cols;
        let mut clicked: Option<(Pos2, bool, bool)> = None; // (位置, 是否Shift, 是否拖动)
        let out = egui::ScrollArea::both().auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
            let width = (widest as f32 * char_w + gutter + pad * 2.0 + 40.0).max(ui.available_width());
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, total_h), Sense::click_and_drag());
            let painter = ui.painter_at(rect.intersect(ui.clip_rect()));
            let d = &self.docs[ai];
            let first = (viewport.min.y / row_h).floor().max(0.0) as usize;
            let last = ((viewport.max.y / row_h).ceil() as usize + 1).min(d.lines.len());
            let x0 = rect.min.x + gutter + pad;
            for row in first..last {
                let y = rect.min.y + row as f32 * row_h;
                let (disp, map) = expand_tabs(&d.lines[row], tab);
                widest = widest.max(disp.chars().count());
                let galley = painter.layout_job(highlight_job(&d.lines[row], &disp, &map, cfg.as_ref(), &theme, &font, text_color));
                // 选区底色
                if let Some(s) = d.sel {
                    if row >= s[0] && row <= s[2] {
                        let c0 = if row == s[0] { s[1] } else { 0 };
                        let c1 = if row == s[2] { s[3] } else { d.lines[row].chars().count() };
                        let xa = x_of(&painter, &disp, &map, c0, &font, text_color);
                        let mut xb = x_of(&painter, &disp, &map, c1.min(map.len() - 1), &font, text_color);
                        if row != s[2] { xb += char_w * 0.6; } // 跨行的行尾多画一点, 表示换行也被选中
                        painter.rect_filled(Rect::from_min_max(Pos2::new(x0 + xa, y), Pos2::new(x0 + xb, y + row_h)), 0.0, sel_bg);
                    }
                }
                painter.galley(Pos2::new(x0, y), galley, text_color);
                painter.text(Pos2::new(rect.min.x + gutter - 8.0, y), egui::Align2::RIGHT_TOP, format!("{}", row + 1), font.clone(), weak);
            }
            // 光标
            let [cr, cc] = d.cursor;
            if cr < d.lines.len() {
                let (disp, map) = expand_tabs(&d.lines[cr], tab);
                let cx = x0 + x_of(&painter, &disp, &map, cc.min(map.len() - 1), &font, text_color);
                let cy = rect.min.y + cr as f32 * row_h;
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
                    ui.scroll_to_rect(Rect::from_min_size(Pos2::new(cx - 8.0, cy), Vec2::new(40.0, row_h * 2.0)), None);
                }
                ui.ctx().output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput { purpose: egui::IMEPurpose::Normal, rect, cursor_rect: Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(2.0, row_h)), should_interrupt_composition: false });
                });
            }
            if resp.is_pointer_button_down_on() || resp.clicked() || resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    clicked = Some((p, ui.input(|i| i.modifiers.shift), resp.dragged() && !resp.drag_started()));
                }
            }
            if resp.clicked() || resp.drag_started() {
                resp.request_focus();
            }
            (rect, x0)
        });
        let (rect, x0) = out.inner;
        self.max_cols = widest;
        if let Some((p, shift, dragging)) = clicked {
            let row = (((p.y - rect.min.y) / row_h).floor().max(0.0)) as usize;
            let d = &mut self.docs[ai];
            let row = row.min(d.lines.len() - 1);
            let (disp, map) = expand_tabs(&d.lines[row], tab);
            let want = (p.x - x0).max(0.0);
            let col = col_at(ctx, &disp, &map, want, &font);
            if dragging { d.drag_to(row, col); } else if ctx.input(|i| i.pointer.any_pressed()) { d.click(row, col, shift); }
        }
        let _ = max_cols;
        if self.first_frame {
            self.first_frame = false;
            self.trace.mark("first frame");
        }
    }
}

/// 文件树: 目录用折叠头 (展开才读目录), 文件点击打开
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
fn col_at(ctx: &egui::Context, disp: &str, map: &[usize], x: f32, font: &FontId) -> usize {
    let galley = ctx.fonts_mut(|f| f.layout_no_wrap(disp.to_string(), font.clone(), Color32::WHITE));
    let idx: usize = galley.cursor_from_pos(Vec2::new(x, 0.0)).index.into(); // 显示字符下标
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
        ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
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

        self.shortcuts(ctx);
        egui::Panel::top("menu").show(root, |ui| self.menu(ui, ctx));
        egui::Panel::top("tabs").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut close: Option<usize> = None;
                for i in 0..self.docs.len() {
                    let label = format!("{}{}", if self.docs[i].dirty { "● " } else { "" }, self.docs[i].name);
                    if ui.selectable_label(i == self.active, label).clicked() { self.active = i; }
                    if ui.small_button("×").clicked() { close = Some(i); }
                    ui.add_space(6.0);
                }
                if let Some(i) = close { self.close_tab(i); }
            });
        });
        if self.side.open {
            egui::Panel::left("side").resizable(true).default_size(250.0).show(root, |ui| self.side_ui(ui));
        }
        self.side.poll_grep();
        if self.side.grep_running() { ctx.request_repaint_after(std::time::Duration::from_millis(80)); }
        egui::Panel::bottom("status").show(root, |ui| {
            let d = &self.docs[self.active.min(self.docs.len() - 1)];
            ui.horizontal(|ui| {
                ui.label(format!("Ln {}, Col {}", d.cursor[0] + 1, d.cursor[1] + 1));
                ui.separator();
                ui.label(d.charset.label());
                ui.label(d.eol.label());
                ui.label(format!("共 {} 行", d.lines.len()));
                ui.separator();
                ui.label(&d.tip);
            });
        });
        let mut find_focus = false;
        if self.find.open {
            egui::Panel::bottom("find").show(root, |ui| {
                self.find_bar(ui);
                find_focus = ctx.memory(|m| m.focused().is_some());
            });
        }
        egui::CentralPanel::default_margins().frame(egui::Frame::NONE).show(root, |ui| self.editor(ui, ctx, find_focus));
        self.persist_if_changed();
        ctx.request_repaint_after(std::time::Duration::from_millis(550)); // 光标闪烁
    }
}
