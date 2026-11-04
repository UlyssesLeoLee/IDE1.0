//! 标签栏 (完整界面): 照 Tauri 版 `#tabs` / `.tab` 自绘 —— 28 高的 chrome 色条, 每个标签 `padding 0 26 0 12`、最宽 200、右侧 1px 竖线,
//! 活动标签与编辑区同色 + 顶部 2px 强调线, 已修改标签左侧一个黄点, 关闭钮 18x18 只在悬停 / 活动时显示, 标签多了横向滚动。
//! 只有一个文档且没开活动栏 (极简界面) 时整条收起, 与 Tauri 极简布局的 `tabs: auto` 一致。

use super::*;

/// 条高 (不含下边线)
const TAB_H: f32 = 28.0;
/// 标签最宽
const TAB_MAX_W: f32 = 200.0;
/// 文字左 / 右内边距 (右边为关闭钮留位)
const PAD_L: f32 = 12.0;
const PAD_R: f32 = 26.0;
/// 关闭钮边长 / 距右边
const CLOSE: f32 = 18.0;
const CLOSE_GAP: f32 = 4.0;

/// 一个标签的宽度: 文字宽 + 内边距, 封顶 200 (文字超出的部分在绘制时用 `…` 截断)
pub fn tab_width(text_w: f32) -> f32 {
    (text_w + PAD_L + PAD_R).min(TAB_MAX_W)
}

/// 标签栏要不要画: 多个文档, 或者开了活动栏 (完整界面, Tauri 的 `tabs: true`)
pub fn bar_visible(docs: usize, full_ui: bool) -> bool {
    docs > 1 || full_ui
}

impl App {
    pub(super) fn tab_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        let mut tab_cmd: Option<TabCmd> = None;
        if bar_visible(self.docs.len(), self.cfg.view.rail) {
            let tc = self.tc;
            egui::Panel::top("tabs").exact_size(TAB_H + 1.0).frame(egui::Frame::NONE.fill(tc.bg_chrome)).show(root, |ui| {
                let bar = ui.max_rect();
                ui.painter().hline(bar.x_range(), bar.bottom() - 0.5, egui::Stroke::new(1.0, tc.border));
                let scroll_active = self.last_active != self.active;
                let wheel = ui.input(|i| i.smooth_scroll_delta);
                if ui.rect_contains_pointer(bar) && wheel.y != 0.0 && wheel.x == 0.0 { ui.scroll_with_delta(Vec2::new(wheel.y, 0.0)); }
                egui::ScrollArea::horizontal().auto_shrink([false, true]).scroll_source(egui::scroll_area::ScrollSource::ALL).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for i in 0..self.docs.len() { self.tab_item(ui, ctx, i, scroll_active, &mut tab_cmd); }
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
            Some(TabCmd::Group(i, g)) => { if let Some(d) = self.docs.get_mut(i) { d.group = g; } }
            None => {}
        }
        if self.split_on {
            self.panes.docs[self.panes.focus] = self.active;
        }
    }

    fn tab_item(&self, ui: &mut egui::Ui, ctx: &egui::Context, i: usize, scroll_active: bool, cmd: &mut Option<TabCmd>) {
        let tc = self.tc;
        let active = i == self.active;
        let doc = &self.docs[i];
        let gcol = GROUP_COLORS.get(doc.group as usize).copied().flatten();
        let fg = if active { if tc.dark { Color32::WHITE } else { tc.blue } } else { tc.fg_dim };
        let font = FontId::proportional(12.0);
        let mut job = egui::text::LayoutJob::default();
        if let Some(c) = gcol { job.append("● ", 0.0, egui::TextFormat::simple(font.clone(), c)); }
        job.append(&doc.name, 0.0, egui::TextFormat::simple(font.clone(), fg));
        job.wrap = egui::text::TextWrapping { max_width: TAB_MAX_W - PAD_L - PAD_R, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
        let galley = ui.painter().layout_job(job);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(tab_width(galley.size().x), TAB_H), Sense::click());
        let close_rect = Rect::from_center_size(egui::pos2(rect.right() - CLOSE_GAP - CLOSE / 2.0, rect.center().y), Vec2::splat(CLOSE));
        let close = ui.interact(close_rect, ui.make_persistent_id(("tab-x", i)), Sense::click());
        if resp.clicked() && !close.hovered() { *cmd = Some(TabCmd::Select(i)); }
        if close.clicked() || resp.middle_clicked() { *cmd = Some(TabCmd::Close(i)); }
        if scroll_active && active { resp.scroll_to_me(None); }
        let p = ui.painter();
        let hot = resp.hovered() || close.hovered();
        let top_round = egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 };
        if active { p.rect_filled(rect, top_round, tc.bg_edit); } else if hot { p.rect_filled(rect, top_round, tc.bg_hover); }
        if active { p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 2.0)), 0.0, tc.bg_status); }
        p.vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0, tc.border));
        p.galley(egui::pos2(rect.left() + PAD_L, rect.center().y - galley.size().y / 2.0), galley, fg);
        if doc.dirty { p.text(egui::pos2(rect.left() + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, "●", FontId::proportional(8.0), tc.yellow); }
        if active || hot {
            if close.hovered() { p.rect_filled(close_rect, 3.0, tc.bg_active); }
            let c = close_rect.center();
            let st = egui::Stroke::new(1.2, if close.hovered() { tc.fg } else { tc.fg_dim });
            p.line_segment([egui::pos2(c.x - 3.5, c.y - 3.5), egui::pos2(c.x + 3.5, c.y + 3.5)], st);
            p.line_segment([egui::pos2(c.x + 3.5, c.y - 3.5), egui::pos2(c.x - 3.5, c.y + 3.5)], st);
        }
        resp.context_menu(|ui| {
            if ui.button(tr("c.close")).clicked() { *cmd = Some(TabCmd::Close(i)); ui.close(); }
            if ui.button(tr("c.close-others-s")).clicked() { *cmd = Some(TabCmd::Many(i, CloseKind::Others)); ui.close(); }
            if ui.button(tr("c.close-left-s")).clicked() { *cmd = Some(TabCmd::Many(i, CloseKind::Left)); ui.close(); }
            if ui.button(tr("c.close-right-s")).clicked() { *cmd = Some(TabCmd::Many(i, CloseKind::Right)); ui.close(); }
            ui.separator();
            if menuui::mbtn(ui, lk("c.tab-move-left", "")).clicked() { *cmd = Some(TabCmd::Move(i, true)); ui.close(); }
            if menuui::mbtn(ui, lk("c.tab-move-right", "")).clicked() { *cmd = Some(TabCmd::Move(i, false)); ui.close(); }
            if ui.button(tr("c.copy-path")).clicked() { if let Some(p) = &doc.path { ctx.copy_text(p.display().to_string()); } ui.close(); }
            ui.separator();
            for (g, name) in GROUP_KEYS.iter().enumerate() {
                if ui.selectable_label(doc.group as usize == g, tr(name)).clicked() { *cmd = Some(TabCmd::Group(i, g as u8)); ui.close(); }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_width_adds_padding_and_caps_at_200() {
        assert_eq!(tab_width(50.0), 88.0);
        assert_eq!(tab_width(500.0), 200.0);
    }

    #[test]
    fn single_document_hides_the_bar_unless_the_full_ui_is_on() {
        assert!(!bar_visible(1, false));
        assert!(bar_visible(2, false));
        assert!(bar_visible(1, true));
    }
}
