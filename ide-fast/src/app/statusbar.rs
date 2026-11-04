//! 状态栏 (Tauri 的 `#status`): 一行、每项 `padding 0 8` 右侧 1px 竖线 (字色 20%), 顺序照 editor.html:
//! [列块 / 矩形] [选区字数] `Ln r, Col c` [● REC] [U+码位] [挿入 / 上書] [编码] [换行符] [文件名] [提示 (撑满, 超出 …)] ... [语法] (靠右)。
//! ide-fast 多出的指示 (换行混用 / 只读 / 范围选择 / 多光标) 只在有效时出现在换行符后面。

use super::*;

/// 状态栏里点出来的命令
pub(super) enum StatusCmd {
    Goto,
    Charset(Charset),
    Reopen(Charset),
    Bom,
    Eol(Eol),
}

/// 光标处字符的码位: 行尾 `EOL`, 文件末尾 `EOF` (Sakura 状态栏的「文字コード」, Tauri `#s-code`)
pub fn code_label(lines: &[String], cursor: [usize; 2]) -> String {
    let row = cursor[0].min(lines.len().saturating_sub(1));
    match lines.get(row).and_then(|l| l.chars().nth(cursor[1])) {
        Some(c) => format!("U+{:04X}", c as u32),
        None if row + 1 >= lines.len() => "EOF".into(),
        None => "EOL".into(),
    }
}

/// `Ln r, Col c`, 显示列与字符列不同 (全角 / Tab) 时再补 ` (表示 n)` (Tauri `updateCaretColumn`)
pub fn pos_label(row: usize, char_col: usize, display_col: usize) -> String {
    let mut s = format!("Ln {}, Col {}", row + 1, char_col + 1);
    if display_col != char_col + 1 { s.push_str(&format!(" (表示 {display_col})")); }
    s
}

/// 一项之后的竖线 (整条栏高, 字色 20%)
fn sep(ui: &mut egui::Ui, text: Color32) {
    let r = ui.max_rect();
    let x = ui.cursor().left() + 0.5;
    ui.painter().vline(x, r.y_range(), egui::Stroke::new(1.0, text.gamma_multiply(0.2)));
    ui.add_space(1.0);
}

/// 一个不可点的文字项: 左右各 8px + 右侧竖线
fn item(ui: &mut egui::Ui, text: Color32, label: egui::RichText) -> egui::Response {
    ui.add_space(8.0);
    let r = ui.add(egui::Label::new(label));
    ui.add_space(8.0);
    sep(ui, text);
    r
}

fn text_w(ui: &egui::Ui, s: &str) -> f32 {
    ui.painter().layout_no_wrap(s.to_string(), egui::TextStyle::Body.resolve(ui.style()), Color32::WHITE).size().x
}

impl App {
    pub(super) fn status_panel(&mut self, root: &mut egui::Ui) {
        if !self.cfg.view.status_bar { return; }
        let mut st_cmd: Option<StatusCmd> = None;
        let on = self.tc.on_status();
        let welcome = self.welcome_visible();
        let frame = egui::Frame::new().fill(self.tc.bg_status).inner_margin(egui::Margin::symmetric(0, 3));
        egui::Panel::bottom("status").frame(frame).show(root, |ui| {
            status_bar_style(ui, on);
            ui.spacing_mut().item_spacing.x = 0.0;
            // Tauri 状态栏是 11px 等宽字
            for ts in [egui::TextStyle::Body, egui::TextStyle::Button] { ui.style_mut().text_styles.insert(ts, FontId::monospace(11.0)); }
            let d = &self.docs[self.active.min(self.docs.len() - 1)];
            let lang = self.type_of(d).map(|c| c.name.clone()).unwrap_or_else(|| "Plain".into());
            let row = d.cursor[0].min(d.lines.len() - 1);
            let dcol = crate::view::display_col(&d.lines[row], d.cursor[1], self.tab);
            let (total_w, right) = (ui.available_width(), ui.max_rect().right());
            ui.horizontal(|ui| {
                if d.box_sel.is_some() { item(ui, on, egui::RichText::new(tr("st.box"))); }
                if let Some(sel) = d.sel {
                    let (n, l) = crate::view::sel_stats(&d.lines, sel);
                    item(ui, on, egui::RichText::new(trf("st.selected", &[&n.to_string(), &l.to_string()])));
                }
                // 位置: 点一下 = 转到行; 悬停给总行数和字符列
                ui.add_space(8.0);
                let tip = format!("{}\n{}\n{}", trf("st.lines", &[&d.lines.len().to_string()]), tr("st.goto-hint"), trf("st.char-col", &[&(d.cursor[1] + 1).to_string()]));
                if ui.add(egui::Label::new(pos_label(row, d.cursor[1], dcol)).sense(Sense::click())).on_hover_text(tip).clicked() { st_cmd = Some(StatusCmd::Goto); }
                ui.add_space(8.0);
                sep(ui, on);
                if d.rec.is_some() { item(ui, on, egui::RichText::new("● REC").color(Color32::from_rgb(255, 120, 120))); }
                if !welcome { item(ui, on, egui::RichText::new(code_label(&d.lines, d.cursor))); }
                item(ui, on, egui::RichText::new(if d.overwrite { "上書" } else { "挿入" }));
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
                sep(ui, on);
                ui.menu_button(d.eol.label(), |ui| {
                    for e in [Eol::Crlf, Eol::Lf, Eol::Cr] {
                        if ui.selectable_label(e == d.eol, e.label()).clicked() { st_cmd = Some(StatusCmd::Eol(e)); ui.close(); }
                    }
                });
                sep(ui, on);
                if d.mixed_eol {
                    ui.add_space(8.0);
                    ui.colored_label(Color32::from_rgb(255, 200, 90), tr("st.mixed-eol")).on_hover_text(tr("st.mixed-eol-tip"));
                    ui.add_space(8.0);
                    sep(ui, on);
                }
                if d.readonly { item(ui, on, egui::RichText::new(tr("st.readonly"))); }
                if d.sel_mode { item(ui, on, egui::RichText::new(trz("范围选择")).color(Color32::from_rgb(255, 200, 90))); }
                if !d.extra.is_empty() { item(ui, on, egui::RichText::new(trf("st.cursors", &[&(d.extra.len() + 1).to_string()]))); }
                // 文件名 (最宽 40%, 超出 …)
                let name = if welcome { "无活动".to_string() } else { format!("{}{}", if d.dirty { "● " } else { "" }, d.name) };
                ui.add_space(8.0);
                let w = (total_w * 0.4).min(text_w(ui, &name) + 1.0);
                ui.allocate_ui_with_layout(Vec2::new(w, ui.available_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(name).truncate());
                });
                ui.add_space(8.0);
                sep(ui, on);
                // 提示撑满剩下的宽, 语法名靠右
                let lang_w = text_w(ui, &lang) + 16.0;
                let tip_w = (right - ui.cursor().left() - lang_w - 1.0).max(0.0);
                ui.allocate_ui_with_layout(Vec2::new(tip_w, ui.available_height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(tip_w);
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(crate::msgtr::msg(&d.tip).as_ref()).truncate());
                });
                sep(ui, on);
                ui.add_space(8.0);
                ui.label(lang);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn code_label_shows_codepoint_eol_or_eof() {
        let l = lines(&["Ab", "c"]);
        assert_eq!(code_label(&l, [0, 0]), "U+0041");
        assert_eq!(code_label(&l, [0, 2]), "EOL");
        assert_eq!(code_label(&l, [1, 1]), "EOF");
        assert_eq!(code_label(&lines(&["字"]), [0, 0]), "U+5B57");
        assert_eq!(code_label(&[], [0, 0]), "EOF");
    }

    #[test]
    fn position_label_adds_display_column_only_when_it_differs() {
        assert_eq!(pos_label(0, 0, 1), "Ln 1, Col 1");
        assert_eq!(pos_label(2, 3, 5), "Ln 3, Col 4 (表示 5)");
    }
}
