//! 首选项对话框的页面: 画的是一份草稿 (`Settings` 的拷贝), 按 OK 才并进运行状态 (见 app.rs 的 `prefs_commit`)。
//! 页签对应 Tauri 版「共通設定」的分组: 常规 / 编辑 / 文件 / 字体与颜色 / 文件类型 / 查找。

use crate::i18n::{tr, Lang};
use crate::settings::{self, Settings, TypePref};
use eframe::egui;
use editcore::document_io::{Charset, Eol};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrefsTab {
    General,
    Edit,
    Files,
    Font,
    Types,
    Search,
}

pub struct Prefs {
    pub draft: Settings,
    pub tab: PrefsTab,
    /// 「文件类型」页里正在输入的新扩展名
    pub new_ext: String,
    /// 系统字体文件名 (打开对话框时扫一次)
    pub fonts: Vec<String>,
}

/// 三态: 沿用全局 / 开 / 关 <-> 下拉框序号
pub fn tri_index(v: Option<bool>) -> usize {
    match v { None => 0, Some(true) => 1, Some(false) => 2 }
}

pub fn tri_from_index(i: usize) -> Option<bool> {
    match i { 1 => Some(true), 2 => Some(false), _ => None }
}

/// 加一条按扩展名的覆盖项: 去掉前导点、转小写; 空的 / 已有的不加。返回是否真的加了
pub fn add_type(d: &mut Settings, ext: &str) -> bool {
    let ext = ext.trim().trim_start_matches('.').to_lowercase();
    if ext.is_empty() || d.file_types.iter().any(|t| t.ext == ext) {
        return false;
    }
    d.file_types.push(TypePref { ext, ..Default::default() });
    true
}

fn tri_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, v: &mut Option<bool>) {
    let names = [tr("pf.inherit"), "✔", "✘"];
    let mut i = tri_index(*v);
    egui::ComboBox::from_id_salt(id).width(64.0).selected_text(names[i]).show_ui(ui, |ui| {
        for (k, n) in names.iter().enumerate() { ui.selectable_value(&mut i, k, *n); }
    });
    *v = tri_from_index(i);
}

fn label_of(file: &str) -> String {
    file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file).to_string()
}

pub fn prefs_page(ui: &mut egui::Ui, p: &mut Prefs) {
    let d = &mut p.draft;
    match p.tab {
        PrefsTab::General => {
            ui.horizontal(|ui| {
                ui.label(tr("pf.language"));
                let cur = Lang::from_code(&d.lang).unwrap_or(Lang::Zh);
                egui::ComboBox::from_id_salt("pf-lang").selected_text(cur.native()).show_ui(ui, |ui| {
                    for l in Lang::ALL { if ui.selectable_label(l == cur, l.native()).clicked() { d.lang = l.code().into(); } }
                });
            });
            ui.checkbox(&mut d.dark, tr("pf.dark"));
            ui.checkbox(&mut d.highlight, tr("pf.highlight"));
            ui.checkbox(&mut d.wrap, tr("pf.wrap"));
            ui.horizontal(|ui| { ui.label(tr("pf.font-size")); ui.add(egui::Slider::new(&mut d.font_px, 8.0..=40.0)); });
            ui.horizontal(|ui| { ui.label(tr("pf.tab-width")); ui.add(egui::Slider::new(&mut d.tab_width, 1..=16)); });
        }
        PrefsTab::Edit => {
            ui.checkbox(&mut d.auto_indent, tr("pf.auto-indent"));
            ui.checkbox(&mut d.smart_indent, tr("pf.smart-indent"));
            ui.checkbox(&mut d.insert_spaces, tr("pf.insert-spaces"));
            ui.horizontal(|ui| {
                ui.label(tr("pf.default-enc"));
                egui::ComboBox::from_id_salt("pf-enc").selected_text(d.default_encoding.clone()).show_ui(ui, |ui| {
                    for cs in Charset::ALL { if ui.selectable_label(cs.label() == d.default_encoding, cs.label()).clicked() { d.default_encoding = cs.label().into(); } }
                });
            });
            ui.horizontal(|ui| {
                ui.label(tr("pf.default-eol"));
                egui::ComboBox::from_id_salt("pf-eol").selected_text(d.default_eol.clone()).show_ui(ui, |ui| {
                    for e in [Eol::Crlf, Eol::Lf, Eol::Cr] { if ui.selectable_label(e.label() == d.default_eol, e.label()).clicked() { d.default_eol = e.label().into(); } }
                });
            });
        }
        PrefsTab::Files => {
            ui.checkbox(&mut d.save_backup, tr("pf.save-backup"));
            ui.checkbox(&mut d.check_external, tr("pf.check-external"));
            ui.checkbox(&mut d.restore_session, tr("pf.restore-session"));
            ui.checkbox(&mut d.confirm_close, tr("pf.confirm-close"));
            ui.horizontal(|ui| { ui.label(tr("pf.max-file")); ui.add(egui::Slider::new(&mut d.max_file_mb, 1..=32)); });
            ui.horizontal(|ui| { ui.label(tr("pf.mru")); ui.add(egui::Slider::new(&mut d.mru_files, 1..=36)); });
        }
        PrefsTab::Font => {
            ui.horizontal(|ui| {
                ui.label(tr("pf.font-family"));
                let shown = if d.font_family.is_empty() { tr("pf.font-default").to_string() } else { label_of(&d.font_family) };
                egui::ComboBox::from_id_salt("pf-font").selected_text(shown).width(220.0).show_ui(ui, |ui| {
                    if ui.selectable_label(d.font_family.is_empty(), tr("pf.font-default")).clicked() { d.font_family.clear(); }
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for f in &p.fonts { if ui.selectable_label(&d.font_family == f, label_of(f)).clicked() { d.font_family = f.clone(); } }
                    });
                });
            });
            ui.horizontal(|ui| { ui.label(tr("pf.line-height")); ui.add(egui::Slider::new(&mut d.line_height, 0.8..=2.5).step_by(0.05)); });
            ui.small(tr("pf.restart-note"));
            ui.separator();
            ui.label(tr("pf.colors"));
            let names = ["pf.col.text", "pf.col.bg", "pf.col.keyword", "pf.col.string", "pf.col.comment", "pf.col.number"];
            // 没设时「设置」给一个看得过去的起点: 取当前主题的对应色太绕, 用中性灰
            for (name, slot) in names.iter().zip(d.colors.slots()) {
                ui.horizontal(|ui| {
                    ui.add_sized([70.0, 18.0], egui::Label::new(tr(name)));
                    match slot {
                        Some(c) => {
                            ui.color_edit_button_srgb(c);
                            ui.label(settings::hex(*c));
                            if ui.small_button(tr("pf.col.clear")).clicked() { *slot = None; }
                        }
                        None => { if ui.small_button(tr("pf.col.set")).clicked() { *slot = Some([128, 128, 128]); } }
                    }
                });
            }
        }
        PrefsTab::Types => {
            ui.small(tr("pf.types-hint"));
            let mut remove: Option<usize> = None;
            egui::Grid::new("pf-types").num_columns(5).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label(tr("pf.type-ext")); ui.label(tr("pf.tab-width")); ui.label(tr("pf.wrap")); ui.label(tr("pf.insert-spaces")); ui.label("");
                ui.end_row();
                for (i, t) in d.file_types.iter_mut().enumerate() {
                    ui.label(format!(".{}", t.ext));
                    let mut on = t.tab_width.is_some();
                    let mut w = t.tab_width.unwrap_or(4);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut on, "").changed() { t.tab_width = if on { Some(w) } else { None }; }
                        if on && ui.add(egui::DragValue::new(&mut w).range(1..=16)).changed() { t.tab_width = Some(w); }
                    });
                    tri_combo(ui, ("pf-wrap", i), &mut t.wrap);
                    tri_combo(ui, ("pf-sp", i), &mut t.insert_spaces);
                    if ui.small_button(tr("pf.type-remove")).clicked() { remove = Some(i); }
                    ui.end_row();
                }
            });
            if let Some(i) = remove { d.file_types.remove(i); }
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut p.new_ext).desired_width(80.0).hint_text("md"));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button(tr("pf.type-add")).clicked() || enter) && add_type(d, &p.new_ext) { p.new_ext.clear(); }
            });
        }
        PrefsTab::Search => {
            ui.checkbox(&mut d.search.find_case, tr("pf.find-case"));
            ui.checkbox(&mut d.search.find_regex, tr("pf.find-regex"));
            ui.checkbox(&mut d.search.find_word, tr("pf.find-word"));
            ui.checkbox(&mut d.search.find_wrap, tr("pf.find-wrap"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_state_maps_both_ways() {
        for v in [None, Some(true), Some(false)] {
            assert_eq!(tri_from_index(tri_index(v)), v);
        }
        assert_eq!(tri_from_index(9), None);
    }

    #[test]
    fn add_type_normalizes_and_rejects_duplicates() {
        let mut d = Settings::default();
        assert!(add_type(&mut d, " .MD "));
        assert!(!add_type(&mut d, "md"), "重复的不加");
        assert!(!add_type(&mut d, "  "), "空的不加");
        assert!(add_type(&mut d, "rs"));
        assert_eq!(d.file_types.iter().map(|t| t.ext.as_str()).collect::<Vec<_>>(), vec!["md", "rs"]);
    }

    #[test]
    fn font_labels_drop_the_extension() {
        assert_eq!(label_of("consola.ttf"), "consola");
        assert_eq!(label_of("noext"), "noext");
    }
}
