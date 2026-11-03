//! `App` 的批次 10 扩展命令: 编码内容转换 / Base64·uudecode / 彩色 HTML / 剪贴板历史 / 控制字符 / 日期·时间·戳记 / 折行列数。
//! 逻辑都在 `textconv` / `tools` (纯函数, 有单测); 这里只是把它们接到当前文档和窗口上。

use super::*;
use crate::textconv::{self, StampKind, StampPos};
use crate::tools::Ev;

/// 彩色 HTML 最多处理的行数 (再多的复制出去也贴不动)
const HTML_MAX_ROWS: usize = 100_000;

impl App {
    /// 复制 / 剪切得到的文字: 放进系统剪贴板, 同时记进剪贴板历史
    pub(super) fn put_clip(&mut self, ctx: &egui::Context, text: String) {
        self.tools.push_clip(&text);
        ctx.copy_text(text);
    }

    /// 编辑区实际用的语法配色: 内置 / 主题插件配色 + 首选项里的词法颜色覆盖
    pub(super) fn syntax_theme_full(&self) -> Theme {
        let mut theme = self.tc.syntax_theme();
        for (kind, c) in [(TokenKind::Keyword, self.cfg.colors.keyword), (TokenKind::String, self.cfg.colors.string), (TokenKind::Comment, self.cfg.colors.comment), (TokenKind::Number, self.cfg.colors.number)] {
            if let Some(c) = c {
                let mut st = theme.get(kind);
                st.fg = Color(c[0], c[1], c[2]);
                theme.styles.insert(kind, st);
            }
        }
        theme
    }

    /// 对选区 (没有 = 整篇) 做编码内容转换, 结果保持选中
    fn code_convert(&mut self, id: &'static str) {
        let d = self.cur();
        let ok = d.convert_selection(|t| textconv::codecnv(id, t));
        if ok {
            d.tip = trf("dlg.codecnv-done", &[id]);
        }
    }

    /// Base64 / uudecode 解码选区 (没有 = 整篇): UTF-8 文本开成新标签, 其他内容让用户选路径存成二进制文件
    fn decode_selection(&mut self, uu: bool) {
        let (_, text) = self.cur().selection_or_all();
        let bytes = if uu { textconv::uudecode(&text) } else { textconv::base64_decode(&text) };
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => { self.cur().tip = e; return; }
        };
        match String::from_utf8(bytes) {
            Ok(s) => {
                self.new_doc();
                let s = s.replace("\r\n", "\n").replace('\r', "\n");
                let d = self.cur();
                d.lines = s.split('\n').map(str::to_string).collect();
                d.dirty = true;
                d.tip = trf("dlg.decoded-text", &[&s.chars().count().to_string()]);
            }
            Err(e) => {
                let bytes = e.into_bytes();
                let Some(path) = rfd::FileDialog::new().set_file_name(if uu { "uudecoded.bin" } else { "decoded.bin" }).save_file() else { return };
                let tip = if path.to_string_lossy().starts_with("\\\\") {
                    tr("dlg.no-unc").to_string()
                } else {
                    match editcore::document_io::atomic_write(&path, &bytes, editcore::document_io::AtomicWriteOpts { clear_readonly: false, sync: true }) {
                        Ok(()) => trf("dlg.decoded-saved", &[&bytes.len().to_string(), &path.display().to_string()]),
                        Err(e) => e.to_string(),
                    }
                };
                self.cur().tip = tip;
            }
        }
    }

    /// 把选中的行 (没有选区 = 整篇) 复制成带颜色的 HTML 源码
    fn copy_html(&mut self, ctx: &egui::Context, numbers: bool) {
        let name = self.cur().name.clone();
        let cfg: Option<TypeConfig> = if self.highlight { self.registry.detect(&name).cloned() } else { None };
        let theme = self.syntax_theme_full();
        let fg = self.cfg.colors.text.unwrap_or_else(|| { let c = self.tc.fg; [c.r(), c.g(), c.b()] });
        let d = self.cur();
        let (r0, r1) = d.selected_rows().unwrap_or((0, d.lines.len() - 1));
        let r1 = r1.min(d.lines.len() - 1).min(r0 + HTML_MAX_ROWS - 1);
        let html = textconv::color_html(&d.lines[r0..=r1], r0, numbers, cfg.as_ref(), &theme, fg);
        let n = r1 - r0 + 1;
        self.put_clip(ctx, html);
        self.cur().tip = trf("dlg.html-copied", &[&n.to_string()]);
    }

    /// 在光标处插入一段文字 (替换选区, 一步撤销)
    pub(super) fn insert_text(&mut self, text: &str) {
        self.cur().edit("paste", Some(text.to_string()));
    }

    fn insert_stamp(&mut self) {
        let kind = StampKind::from_u8(self.cfg.view.stamp_kind);
        let pos = StampPos::from_u8(self.cfg.view.stamp_pos);
        let s = kind.text(&textconv::local_now());
        self.cur().insert_stamp(&s, pos);
    }

    /// 折行列数: 0 = 按窗口宽度; 有列数时顺便打开自动换行
    fn set_wrap_col(&mut self, n: usize) {
        self.cfg.view.wrap_col = n;
        self.wrap = true;
        self.cur().tip = if n > 0 { trf("dlg.wrap-col", &[&n.to_string()]) } else { tr("dlg.wrap-window").to_string() };
    }

    /// 小窗口 (剪贴板历史 / 控制字符 / 戳记 / 折行列数) 这一帧的绘制与动作
    pub(super) fn tools_windows(&mut self, ctx: &egui::Context) {
        let mut view = self.cfg.view.clone();
        for ev in self.tools.windows(ctx, &mut view) {
            match ev {
                Ev::Paste(t) => self.insert_text(&t),
                Ev::Copy(t) => { self.put_clip(ctx, t); }
                Ev::Ctrl(c) => self.cur().insert_control(c),
                Ev::WrapCol(n) => self.set_wrap_col(n),
            }
        }
        (self.cfg.view.stamp_kind, self.cfg.view.stamp_pos) = (view.stamp_kind, view.stamp_pos);
    }

    /// 批次 10 新增的窗口级命令 (`run_app` 把它们转到这里)
    pub(super) fn run_extra(&mut self, a: KApp, ctx: &egui::Context) {
        match a {
            KApp::SelMode => self.cur().toggle_sel_mode(),
            KApp::Base64Decode => self.decode_selection(false),
            KApp::UuDecode => self.decode_selection(true),
            KApp::CopyHtml => self.copy_html(ctx, false),
            KApp::CopyHtmlLn => self.copy_html(ctx, true),
            KApp::ClipHistory => self.tools.clip_open = !self.tools.clip_open,
            KApp::CtrlCodes => self.tools.ctrl_open = !self.tools.ctrl_open,
            KApp::InsDate => self.insert_text(&textconv::local_now().date()),
            KApp::InsTime => self.insert_text(&textconv::local_now().time()),
            KApp::InsStamp => self.insert_stamp(),
            KApp::StampCfg => self.tools.stamp_open = true,
            KApp::WrapCol => { let n = self.cfg.view.wrap_col; self.tools.open_wrap(n); }
            KApp::WrapWindow => self.set_wrap_col(0),
            KApp::WrapNone => self.wrap = false,
            KApp::TagJump | KApp::TagBack | KApp::TagKeyword | KApp::TagsMake => self.cur().tip = "标签跳转尚未接上".into(),
            KApp::IsFwd | KApp::IsBack | KApp::IsRegexFwd | KApp::IsRegexBack | KApp::IsMigemoFwd | KApp::IsMigemoBack => self.cur().tip = "增量查找条尚未接上".into(),
            _ => {}
        }
    }

    /// 编辑 → 编码转换执行入口 (`run_action` 用)
    pub(super) fn run_codecnv(&mut self, id: &'static str) {
        self.code_convert(id);
    }

    /// 一个窗口级命令在菜单里的文字: 当前绑定的键附在后面
    pub(super) fn app_label(a: KApp, zh: &'static str) -> String {
        let key = keymap::entries().iter().find(|e| matches!(e.run, Run::App(x) if x == a)).and_then(|e| keymap::display(&e.id)).filter(|k| !k.is_empty());
        match key {
            Some(k) => format!("{}  {}", trz(zh), k),
            None => trz(zh).to_string(),
        }
    }

    /// 编辑 → 插入 子菜单
    pub(super) fn insert_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        for (zh, a) in [("插入日期", KApp::InsDate), ("插入时间", KApp::InsTime), ("插入戳记", KApp::InsStamp), ("戳记设置…", KApp::StampCfg), ("插入控制字符…", KApp::CtrlCodes)] {
            if ui.button(Self::app_label(a, zh)).clicked() { self.run_app(a, ctx); ui.close(); }
        }
        ui.separator();
        let mut pick: Option<String> = None;
        ui.menu_button(trz("最近的文件路径"), |ui| {
            if self.recent.is_empty() { ui.label(tr("ui.none")); }
            for r in &self.recent {
                if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
            }
        });
        ui.menu_button(trz("最近的文件夹路径"), |ui| {
            if self.recent_dirs.is_empty() { ui.label(tr("ui.none")); }
            for r in &self.recent_dirs {
                if ui.button(r).clicked() { pick = Some(r.clone()); ui.close(); }
            }
        });
        if let Some(p) = pick { self.insert_text(&p); }
    }
}
