//! 首选项对话框的页面: 画的是一份草稿 (`Settings` 的拷贝), 按 OK 才并进运行状态 (见 app.rs 的 `prefs_commit`)。
//! 页签对应 Tauri 版「共通設定」的分组: 常规 / 编辑 / 文件 / 字体与颜色 / 文件类型 / 查找。

use crate::ui_ext::TipExt;
use crate::ckbox::CheckExt;
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
    Terminal,
    Toolbar,
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

/// 工具栏列表里第 `i` 项往上 / 往下挪一格; 挪成了返回 true
pub fn toolbar_move(list: &mut [String], i: usize, up: bool) -> bool {
    let j = if up { i.checked_sub(1) } else { (i + 1 < list.len()).then_some(i + 1) };
    match j {
        Some(j) if i < list.len() => { list.swap(i, j); true }
        _ => false,
    }
}

/// 往工具栏末尾加一项: 命令 (注册表里有、列表里还没有) 或分隔线 (`id` 为空, 且上一项不是分隔线); 加成了返回 true
pub fn toolbar_add(list: &mut Vec<String>, id: &str) -> bool {
    if list.len() >= settings::TOOLBAR_MAX {
        return false;
    }
    let ok = if id.is_empty() { list.last().is_some_and(|l| !l.is_empty()) } else { !list.iter().any(|x| x == id) && crate::keymap::entry_by_id(id).is_some() };
    if ok { list.push(id.to_string()); }
    ok
}

/// 能放进工具栏的全部命令: (id, 当前语言的名字), 按名字排序
pub fn toolbar_candidates() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = crate::keymap::entries().iter().map(|e| (e.id.clone(), e.label.text().to_string())).collect();
    v.sort_by(|a, b| a.1.cmp(&b.1));
    v
}

fn toolbar_name(id: &str) -> String {
    if id.is_empty() { return tr("pf.toolbar-sepline").to_string(); }
    crate::keymap::entry_by_id(id).map(|e| e.label.text().to_string()).unwrap_or_else(|| id.to_string())
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
            ui.check(&mut d.dark, tr("pf.dark"));
            ui.check(&mut d.highlight, tr("pf.highlight"));
            ui.check(&mut d.wrap, tr("pf.wrap"));
            ui.check(&mut d.glass_blur, tr("pf.glass-blur")).tip(tr("pf.glass-note"));
            ui.horizontal(|ui| { ui.label(tr("pf.font-size")); ui.add(egui::Slider::new(&mut d.font_px, 8.0..=40.0)); });
            ui.horizontal(|ui| { ui.label(tr("pf.tab-width")); ui.add(egui::Slider::new(&mut d.tab_width, 1..=16)); });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(tr("pf.update-channel"));
                let cur = crate::update::Channel::from_code(&d.update_channel).unwrap_or_default();
                egui::ComboBox::from_id_salt("pf-upd-ch").selected_text(tr(cur.label_key())).show_ui(ui, |ui| {
                    for c in crate::update::Channel::ALL { if ui.selectable_label(c == cur, tr(c.label_key())).clicked() { d.update_channel = c.code().into(); } }
                });
            });
            ui.check(&mut d.update_daily, tr("pf.update-daily")).tip(tr("pf.update-daily-tip"));
        }
        PrefsTab::Edit => {
            ui.check(&mut d.auto_indent, tr("pf.auto-indent"));
            ui.check(&mut d.smart_indent, tr("pf.smart-indent"));
            ui.check(&mut d.insert_spaces, tr("pf.insert-spaces"));
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
            ui.check(&mut d.save_backup, tr("pf.save-backup"));
            ui.horizontal(|ui| {
                ui.check(&mut d.auto_backup, tr("pf.auto-backup"));
                ui.add_enabled(d.auto_backup, egui::DragValue::new(&mut d.auto_backup_min).range(crate::backup::MIN_INTERVAL..=crate::backup::MAX_INTERVAL).suffix(tr("pf.minutes")));
            });
            ui.check(&mut d.file_lock, tr("pf.file-lock")).tip(tr("pf.file-lock-tip"));
            ui.check(&mut d.check_external, tr("pf.check-external"));
            ui.check(&mut d.restore_session, tr("pf.restore-session"));
            ui.check(&mut d.confirm_close, tr("pf.confirm-close"));
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
        PrefsTab::Terminal => {
            ui.horizontal(|ui| { ui.label(tr("pf.term-font")); ui.add(egui::Slider::new(&mut d.term_font_px, 8.0..=32.0)); });
            ui.horizontal(|ui| {
                ui.label(tr("pf.term-scrollback"));
                ui.add(egui::DragValue::new(&mut d.term_scrollback).range(crate::terminal::MIN_SCROLLBACK..=crate::terminal::MAX_SCROLLBACK).speed(50.0));
            });
            for (name, slot) in [("pf.term-fg", &mut d.term_fg), ("pf.term-bg", &mut d.term_bg)] {
                ui.horizontal(|ui| {
                    ui.add_sized([70.0, 18.0], egui::Label::new(tr(name)));
                    match slot {
                        Some(c) => {
                            ui.color_edit_button_srgb(c);
                            ui.label(settings::hex(*c));
                            if ui.small_button(tr("pf.col.clear")).clicked() { *slot = None; }
                        }
                        None => { if ui.small_button(tr("pf.col.set")).clicked() { *slot = Some(if name == "pf.term-bg" { [24, 24, 24] } else { [220, 220, 220] }); } }
                    }
                });
            }
        }
        PrefsTab::Types => {
            ui.small(tr("pf.types-hint"));
            let mut remove: Option<usize> = None;
            egui::Grid::new("pf-types").num_columns(6).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label(tr("pf.type-ext")); ui.label(tr("pf.tab-width")); ui.label(tr("pf.wrap")); ui.label(tr("pf.insert-spaces")); ui.label(tr("pf.wrap-col")); ui.label("");
                ui.end_row();
                for (i, t) in d.file_types.iter_mut().enumerate() {
                    ui.label(format!(".{}", t.ext));
                    let mut on = t.tab_width.is_some();
                    let mut w = t.tab_width.unwrap_or(4);
                    ui.horizontal(|ui| {
                        if ui.check(&mut on, "").changed() { t.tab_width = if on { Some(w) } else { None }; }
                        if on && ui.add(egui::DragValue::new(&mut w).range(1..=16)).changed() { t.tab_width = Some(w); }
                    });
                    tri_combo(ui, ("pf-wrap", i), &mut t.wrap);
                    tri_combo(ui, ("pf-sp", i), &mut t.insert_spaces);
                    let mut won = t.wrap_col.is_some();
                    let mut wc = t.wrap_col.filter(|c| *c > 0).unwrap_or(80);
                    ui.horizontal(|ui| {
                        if ui.check(&mut won, "").tip(tr("pf.wrap-col-tip")).changed() { t.wrap_col = if won { Some(wc) } else { None }; }
                        if won && ui.add(egui::DragValue::new(&mut wc).range(10..=crate::tools::WRAP_COL_MAX)).changed() { t.wrap_col = Some(wc); }
                    });
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
        PrefsTab::Toolbar => {
            ui.small(tr("pf.toolbar-hint"));
            ui.add_space(4.0);
            let mut act: Option<(usize, u8)> = None; // (下标, 0 上移 / 1 下移 / 2 删除)
            for (i, id) in d.toolbar_cmds.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(toolbar_name(id));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() { act = Some((i, 2)); }
                        if ui.small_button("▼").clicked() { act = Some((i, 1)); }
                        if ui.small_button("▲").clicked() { act = Some((i, 0)); }
                    });
                });
            }
            match act {
                Some((i, 0)) => { toolbar_move(&mut d.toolbar_cmds, i, true); }
                Some((i, 1)) => { toolbar_move(&mut d.toolbar_cmds, i, false); }
                Some((i, _)) => { d.toolbar_cmds.remove(i); }
                None => {}
            }
            ui.separator();
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("pf-toolbar-add").selected_text(tr("pf.toolbar-add")).height(280.0).show_ui(ui, |ui| {
                    for (id, name) in toolbar_candidates() {
                        let used = d.toolbar_cmds.contains(&id);
                        if ui.add_enabled(!used, egui::Button::selectable(false, name)).clicked() { toolbar_add(&mut d.toolbar_cmds, &id); }
                    }
                });
                if ui.button(tr("pf.toolbar-sep")).clicked() { toolbar_add(&mut d.toolbar_cmds, ""); }
                if ui.button(tr("pf.toolbar-reset")).clicked() { d.toolbar_cmds = settings::DEFAULT_TOOLBAR.iter().map(|s| s.to_string()).collect(); }
            });
        }
        PrefsTab::Search => {
            ui.check(&mut d.search.find_case, tr("pf.find-case"));
            ui.check(&mut d.search.find_regex, tr("pf.find-regex"));
            ui.check(&mut d.search.find_word, tr("pf.find-word"));
            ui.check(&mut d.search.find_wrap, tr("pf.find-wrap"));
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

    #[test]
    fn toolbar_editing_helpers() {
        let mut l: Vec<String> = ["c.save", "c.find", "c.palette"].iter().map(|s| s.to_string()).collect();
        assert!(toolbar_move(&mut l, 1, true) && l == ["c.find", "c.save", "c.palette"]);
        assert!(toolbar_move(&mut l, 1, false) && l == ["c.find", "c.palette", "c.save"]);
        assert!(!toolbar_move(&mut l, 0, true), "第一项不能再往上");
        assert!(!toolbar_move(&mut l, 2, false), "最后一项不能再往下");
        assert!(!toolbar_move(&mut l, 9, true), "下标越界: 什么也不做");
        assert!(!toolbar_add(&mut l, "c.save"), "已经在列表里的命令不重复加");
        assert!(!toolbar_add(&mut l, "no.such.command"), "注册表里没有的命令不加");
        assert!(toolbar_add(&mut l, "c.undo") && l.last().map(String::as_str) == Some("c.undo"));
        assert!(toolbar_add(&mut l, "") && !toolbar_add(&mut l, ""), "分隔线不连着加两条");
        let mut e: Vec<String> = vec![];
        assert!(!toolbar_add(&mut e, ""), "空列表开头不加分隔线");
        let mut full: Vec<String> = (0..settings::TOOLBAR_MAX).map(|_| String::new()).collect();
        assert!(!toolbar_add(&mut full, "c.save"), "封顶");
    }

    #[test]
    fn toolbar_candidates_are_registry_commands_sorted_by_name() {
        let c = toolbar_candidates();
        assert!(c.iter().any(|(id, _)| id == "c.save") && c.iter().any(|(id, _)| id == "app.completion"));
        assert!(c.windows(2).all(|w| w[0].1 <= w[1].1), "按名字排序");
        for (id, _) in &c { assert!(crate::keymap::entry_by_id(id).is_some()); }
        assert_eq!(toolbar_name(""), tr("pf.toolbar-sepline"));
        assert_eq!(toolbar_name("zzz"), "zzz", "未知命令: 原样显示 id");
    }
}
