//! 页面设置对话框与应用内打印预览 (分页逻辑在 `print`; 真正的打印交给系统浏览器的打印对话框)。

use super::*;
use crate::print::{self, Paper};

#[derive(Default)]
pub struct PrintUi {
    pub setup_open: bool,
    /// Some(页下标) = 预览窗口开着
    pub preview: Option<usize>,
}

/// 预览窗口里一页最多画的字符数 (防超长行把界面拖死)
const PREVIEW_LINE_MAX: usize = 400;

impl App {
    pub(super) fn print_cur(&mut self) {
        let ps = self.cfg.view.print.clone();
        let (name, lines) = {
            let d = self.cur();
            (d.name.clone(), d.lines.clone())
        };
        let tip = match print::open_for_print(&name, &lines, &ps) {
            Ok(_) => "已在浏览器打开打印页".to_string(),
            Err(e) => e,
        };
        self.cur().tip = tip;
    }

    pub(super) fn print_windows(&mut self, ctx: &egui::Context) {
        if self.print_ui.setup_open { self.page_setup_window(ctx); }
        if self.print_ui.preview.is_some() { self.print_preview_window(ctx); }
    }

    fn page_setup_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new(trz("页面设置")).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            let ps = &mut self.cfg.view.print;
            egui::ComboBox::from_label(trz("纸张")).selected_text(ps.paper.code()).show_ui(ui, |ui| {
                for p in [Paper::A4, Paper::Letter, Paper::A3] { ui.selectable_value(&mut ps.paper, p, p.code()); }
            });
            ui.checkbox(&mut ps.landscape, trz("横向"));
            ui.horizontal(|ui| {
                ui.label(trz("字号 (pt)"));
                ui.add(egui::DragValue::new(&mut ps.font_pt).range(6.0..=24.0).speed(0.2));
                ui.label(trz("页边距 (mm)"));
                ui.add(egui::DragValue::new(&mut ps.margin_mm).range(5.0..=40.0).speed(0.2));
            });
            ui.checkbox(&mut ps.line_numbers, trz("打印行号"));
            ui.checkbox(&mut ps.header, trz("页眉 (文件名)"));
            ui.checkbox(&mut ps.footer, trz("页脚 (页码)"));
            if ui.button(trz("打印预览…")).clicked() { self.print_ui.preview = Some(0); }
        });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) { self.print_ui.setup_open = false; }
    }

    fn print_preview_window(&mut self, ctx: &egui::Context) {
        let ps = self.cfg.view.print.clone();
        let (rows, cols) = ps.capacity();
        let (name, pages, lines) = {
            let d = self.cur();
            let pages = print::paginate(&d.lines, rows, cols);
            (d.name.clone(), pages, d.lines.clone())
        };
        let Some(mut pg) = self.print_ui.preview else { return };
        pg = pg.min(pages.len() - 1);
        let (mut open, mut do_print) = (true, false);
        egui::Window::new(trz("打印预览")).open(&mut open).default_size([520.0, 640.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(pg > 0, egui::Button::new("<")).clicked() { pg -= 1; }
                ui.label(format!("{} / {}", pg + 1, pages.len()));
                if ui.add_enabled(pg + 1 < pages.len(), egui::Button::new(">")).clicked() { pg += 1; }
                if ui.button(trz("页面设置…")).clicked() { self.print_ui.setup_open = true; }
                if ui.button(trz("打印…")).clicked() { do_print = true; }
            });
            ui.separator();
            let paper = egui::Frame::NONE.fill(Color32::WHITE).inner_margin(egui::Margin::same(10));
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                paper.show(ui, |ui| {
                    let ink = Color32::from_gray(30);
                    if ps.header { ui.label(egui::RichText::new(&name).strong().color(ink)); ui.separator(); }
                    let mut text = String::new();
                    let w = lines.len().to_string().len();
                    for i in pages[pg].clone() {
                        if ps.line_numbers { text.push_str(&format!("{:>w$} ", i + 1)); }
                        text.extend(lines[i].chars().take(PREVIEW_LINE_MAX));
                        text.push('\n');
                    }
                    ui.label(egui::RichText::new(text).monospace().size(ps.font_pt * 1.2).color(ink));
                    if ps.footer { ui.separator(); ui.label(egui::RichText::new(format!("{} / {}", pg + 1, pages.len())).color(ink)); }
                });
            });
        });
        self.print_ui.preview = if open { Some(pg) } else { None };
        if do_print { self.print_cur(); }
    }
}
