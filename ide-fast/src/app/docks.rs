//! 可折叠的停靠区 (左侧栏 / 右侧面板 / 底部面板) 接到 `App` 上: 命令、细条、箭头按钮。
//! 状态翻转的规则在 `panels.rs` (纯函数 + 单测); 折叠状态存在 `cfg.view.*_collapsed`, 随设置落盘。
//! 折叠成细条时只画这一条细条, 面板里的内容不布局 (调用方按 `panels::mode` 决定走哪条路)。

use super::sysopts::BottomTab;
use super::*;
use crate::panels::{self, Dock, Mode};

impl App {
    pub(super) fn dock_collapsed(&self, d: Dock) -> bool {
        let v = &self.cfg.view;
        match d {
            Dock::Side => v.side_collapsed,
            Dock::Right => v.right_collapsed,
            Dock::Bottom => v.bottom_collapsed,
        }
    }

    fn set_dock_collapsed(&mut self, d: Dock, on: bool) {
        let v = &mut self.cfg.view;
        match d {
            Dock::Side => v.side_collapsed = on,
            Dock::Right => v.right_collapsed = on,
            Dock::Bottom => v.bottom_collapsed = on,
        }
    }

    /// 展开一个折叠成细条的停靠区 (活动栏点击用)
    pub(super) fn expand_dock(&mut self, d: Dock) {
        self.set_dock_collapsed(d, false);
    }

    /// 这个停靠区「想显示」吗 (右侧: AI 对话开着, 或 Markdown 预览开着, 或 NEMA 预览开着)
    pub(super) fn dock_wanted(&self, d: Dock) -> bool {
        match d {
            Dock::Side => self.side.open,
            Dock::Right => {
                let is_nema = self
                    .docs
                    .get(self.active.min(self.docs.len().saturating_sub(1)))
                    .is_some_and(nemaopts::is_nema_doc);
                self.ai.open
                    || (self.md_preview && !self.cfg.view.md_overlay)
                    || (self.nema_preview && is_nema)
            }
            Dock::Bottom => self.term.open,
        }
    }

    /// 隐藏了的面板不留折叠标记 (再打开应当是展开的); 每帧几个布尔判断
    pub(super) fn normalize_docks(&mut self) {
        for d in [Dock::Side, Dock::Right, Dock::Bottom] {
            let c = panels::normalize(self.dock_wanted(d), self.dock_collapsed(d));
            self.set_dock_collapsed(d, c);
        }
    }

    /// 折叠 / 展开命令 (命令面板、快捷键、菜单、标题栏按钮、箭头按钮都走这里)
    pub(super) fn collapse_dock(&mut self, d: Dock) {
        if d == Dock::Right && !self.dock_wanted(d) {
            self.ai_toggle(); // 右边什么都没开: 折叠命令 = 把 AI 对话展开显示出来
            self.set_dock_collapsed(d, false);
            return;
        }
        let (open, collapsed) =
            panels::toggle_collapse(self.dock_wanted(d), self.dock_collapsed(d));
        if open && !self.dock_wanted(d) {
            match d {
                Dock::Side => self.side.open = true,
                Dock::Bottom => self.term.open = true,
                Dock::Right => {}
            }
        }
        self.set_dock_collapsed(d, collapsed);
    }

    /// 「显示 / 隐藏」命令 (Ctrl+B / Ctrl+`): 细条状态下先展开
    pub(super) fn toggle_dock_open(&mut self, d: Dock) {
        let (open, collapsed) = panels::toggle_open(self.dock_wanted(d), self.dock_collapsed(d));
        match d {
            Dock::Side => self.side.open = open,
            Dock::Bottom => self.term.open = open,
            Dock::Right => {}
        }
        self.set_dock_collapsed(d, collapsed);
    }

    /// 底部面板头部右端的文字钮 (Tauri 的 `.bottom-toggle`: 展开时写「折叠」, 折叠时写「展开」, 11px, 圆角 5, 1px 边)
    pub(super) fn dock_chevron(&mut self, ui: &mut egui::Ui, d: Dock) {
        let collapsed = self.dock_mode(d, self.dock_wanted(d)) != Mode::Full;
        let word = if collapsed { "展开" } else { "折叠" };
        let txt = egui::RichText::new(trz(word))
            .size(11.0)
            .color(self.tc.fg_dim);
        let edge = if self.tc.dark {
            Color32::from_gray(69)
        } else {
            Color32::from_gray(204)
        };
        let btn = egui::Button::new(txt)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, edge))
            .corner_radius(5.0);
        if ui.add(btn).tip(trz(panels::tip(d, collapsed))).clicked() {
            self.collapse_dock(d);
        }
    }

    /// 细条: 左 / 右是常驻的 16px 竖条 (rail 底色, 顶部一个撑满宽、高 40 的无框 `›` / `‹` 按钮);
    /// 底部是 28px 的面板头 (折叠时保留页签名, 右端文字钮「展开」)
    pub(super) fn dock_strip(&mut self, root: &mut egui::Ui, d: Dock) {
        let collapsed = self.dock_mode(d, self.dock_wanted(d)) != Mode::Full;
        let mut toggle = false;
        let tc = self.tc;
        match d {
            Dock::Side | Dock::Right => {
                let p = if d == Dock::Side {
                    egui::Panel::left("side_strip")
                } else {
                    egui::Panel::right("right_strip")
                };
                p.resizable(false)
                    .exact_size(panels::STRIP)
                    .frame(egui::Frame::NONE.fill(tc.bg_rail))
                    .show(root, |ui| {
                        let w = ui.available_width();
                        let h = ui.available_height();
                        let (rect, resp) = ui
                            .allocate_exact_size(Vec2::new(w, h), Sense::click());
                        let resp = resp
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .tip(trz(panels::tip(d, collapsed)));
                        if resp.hovered() {
                            ui.painter().rect_filled(rect, 0.0, tc.bg_hover);
                        }
                        let col = if resp.hovered() { tc.fg } else { tc.fg_dim };
                        let chevron_pos = Pos2::new(rect.center().x, rect.top() + panels::STRIP_BTN_H * 0.5);
                        ui.painter().text(
                            chevron_pos,
                            egui::Align2::CENTER_CENTER,
                            panels::chevron(d, collapsed),
                            egui::FontId::proportional(16.0),
                            col,
                        );
                        toggle = resp.clicked();
                    });
            }
            Dock::Bottom => {
                let frame = egui::Frame::NONE.fill(tc.bg_bar);
                egui::Panel::bottom("bottom_strip")
                    .resizable(false)
                    .exact_size(panels::BOTTOM_HEAD)
                    .frame(frame)
                    .show(root, |ui| {
                        self.bottom_head(ui);
                    });
            }
        }
        if toggle {
            self.collapse_dock(d);
        }
    }

    /// 底部面板头 (Tauri `#bottom-head`, 28px): 左 = TERMINAL / OUTPUT / PROBLEMS 三个大写页签 (11px 等宽, 当前页 2px 蓝色下划线),
    /// 右 = 「+ 新建终端」和「折叠 / 展开」两个带边的小按钮。展开和折叠两种状态共用这一条
    pub(super) fn bottom_head(&mut self, ui: &mut egui::Ui) {
        let tc = self.tc;
        let edge = if tc.dark {
            Color32::from_gray(69)
        } else {
            Color32::from_gray(204)
        };
        let head = Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(ui.available_width(), panels::BOTTOM_HEAD),
        );
        ui.painter().rect_filled(head, 0.0, tc.bg_bar);
        ui.painter().hline(
            head.x_range(),
            head.top() + 0.5,
            egui::Stroke::new(1.0, tc.border),
        );
        ui.allocate_ui_with_layout(
            head.size(),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (t, name) in [
                    (BottomTab::Terminal, "TERMINAL"),
                    (BottomTab::Output, "OUTPUT"),
                    (BottomTab::Problems, "PROBLEMS"),
                ] {
                    let mut label = name.to_string();
                    if t == BottomTab::Problems && !self.sys.problems.is_empty() {
                        label = format!("{label} ({})", self.sys.problems.len());
                    }
                    let on = self.sys.bottom == t;
                    let g = ui.painter().layout_no_wrap(
                        label,
                        egui::FontId::monospace(11.0),
                        if on { tc.fg } else { tc.fg_dim },
                    );
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(g.size().x + 16.0, panels::BOTTOM_HEAD),
                        Sense::click(),
                    );
                    let col = if on || resp.hovered() {
                        tc.fg
                    } else {
                        tc.fg_dim
                    };
                    ui.painter().galley(
                        egui::pos2(rect.left() + 8.0, rect.center().y - g.size().y / 2.0),
                        g,
                        col,
                    );
                    if on {
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                egui::pos2(rect.left(), rect.bottom() - 2.0),
                                rect.max,
                            ),
                            0.0,
                            tc.blue,
                        );
                    }
                    if resp.clicked() {
                        self.sys.bottom = t;
                        // 头条状态 (折叠 / 完整界面里还没开) 点页签 = 展开面板
                        if self.dock_mode(Dock::Bottom, self.term.open) != Mode::Full {
                            self.term.open = true;
                            self.expand_dock(Dock::Bottom);
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(8.0);
                    self.dock_chevron(ui, Dock::Bottom);
                    let txt = egui::RichText::new(format!("+ {}", trz("新建终端")))
                        .size(11.0)
                        .color(tc.fg_dim);
                    let b = egui::Button::new(txt)
                        .fill(Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(1.0, edge))
                        .corner_radius(5.0);
                    if ui.add(b).tip(trz("新建终端 (Ctrl+`)")).clicked() {
                        self.sys.bottom = BottomTab::Terminal;
                        if self.dock_collapsed(Dock::Bottom) || !self.term.open {
                            self.term.open = true;
                            self.expand_dock(Dock::Bottom);
                        }
                        self.term.restart();
                    }
                });
            },
        );
    }

    /// 底部面板正文顶上的工具行 (终端 / 输出 / 问题共用): 11px、弱前景、描边小按钮, 与头条里「+ 新建终端」同款
    pub(super) fn bottom_toolbar(dim: Color32, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
        ui.scope(|ui| {
            crate::btnstyle::outline(ui);
            let st = ui.style_mut();
            st.text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(11.0));
            st.text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(11.0));
            st.spacing.button_padding = Vec2::new(8.0, 2.0);
            st.visuals.override_text_color = Some(dim);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                add(ui);
            });
            ui.add_space(2.0);
        });
    }

    /// 一个停靠区这一帧怎么画
    pub(super) fn dock_mode(&self, d: Dock, visible: bool) -> Mode {
        if d == Dock::Bottom {
            return panels::bottom_mode(visible, self.dock_collapsed(d), self.cfg.view.rail);
        }
        panels::mode(visible, self.dock_collapsed(d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn app() -> (App, std::sync::MutexGuard<'static, ()>) {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        (
            App::build(vec![], None, Trace::new(), Settings::default(), false),
            g,
        )
    }

    #[test]
    fn full_ui_shows_the_bottom_head_without_opening_the_terminal() {
        let (mut a, _g) = app();
        assert_eq!(
            a.dock_mode(Dock::Bottom, a.term.open),
            Mode::Hidden,
            "极简界面: 不画"
        );
        a.cfg.view.rail = true;
        assert!(!a.term.open);
        assert_eq!(
            a.dock_mode(Dock::Bottom, a.term.open),
            Mode::Strip,
            "完整界面: 只有头条"
        );
        assert!(!a.term.running(), "头条状态不启动 shell");
        a.collapse_dock(Dock::Bottom);
        assert_eq!(
            a.dock_mode(Dock::Bottom, a.term.open),
            Mode::Full,
            "点展开 = 打开面板"
        );
    }

    #[test]
    fn collapse_command_folds_and_unfolds_the_side_bar() {
        let (mut a, _g) = app();
        assert!(a.side.open && !a.cfg.view.side_collapsed);
        a.collapse_dock(Dock::Side);
        assert_eq!((a.side.open, a.cfg.view.side_collapsed), (true, true));
        assert_eq!(a.dock_mode(Dock::Side, a.side.open), Mode::Strip);
        a.collapse_dock(Dock::Side);
        assert_eq!(a.dock_mode(Dock::Side, a.side.open), Mode::Full);
    }

    #[test]
    fn collapse_on_a_hidden_panel_shows_it_expanded() {
        let (mut a, _g) = app();
        a.side.open = false;
        a.collapse_dock(Dock::Side);
        assert_eq!((a.side.open, a.cfg.view.side_collapsed), (true, false));
        assert!(!a.term.open);
        a.collapse_dock(Dock::Bottom);
        assert_eq!((a.term.open, a.cfg.view.bottom_collapsed), (true, false));
    }

    #[test]
    fn toggle_open_on_a_strip_expands_instead_of_hiding() {
        let (mut a, _g) = app();
        a.collapse_dock(Dock::Side);
        a.toggle_dock_open(Dock::Side);
        assert_eq!((a.side.open, a.cfg.view.side_collapsed), (true, false));
        a.toggle_dock_open(Dock::Side);
        assert!(!a.side.open);
    }

    #[test]
    fn hiding_a_collapsed_panel_clears_the_flag() {
        let (mut a, _g) = app();
        a.collapse_dock(Dock::Side);
        a.side.open = false; // 菜单勾选框直接改 open
        a.normalize_docks();
        assert!(!a.cfg.view.side_collapsed);
    }

    #[test]
    fn right_dock_collapse_covers_chat_and_preview_and_opens_chat_when_empty() {
        let (mut a, _g) = app();
        assert!(!a.dock_wanted(Dock::Right));
        a.collapse_dock(Dock::Right);
        assert!(
            a.ai.open && !a.cfg.view.right_collapsed,
            "右侧空着: 折叠命令 = 展开 AI 对话"
        );
        a.collapse_dock(Dock::Right);
        assert!(a.cfg.view.right_collapsed);
        a.ai.open = false;
        a.md_preview = true;
        a.cfg.view.md_overlay = true;
        assert!(!a.dock_wanted(Dock::Right), "覆盖模式的预览不占右侧停靠区");
        a.cfg.view.md_overlay = false;
        a.normalize_docks();
        assert!(
            a.cfg.view.right_collapsed,
            "停靠模式的预览还开着: 折叠标记保留"
        );
    }

    #[test]
    fn collapsed_states_and_sizes_persist_through_current_settings() {
        let (mut a, _g) = app();
        a.collapse_dock(Dock::Side);
        a.term.open = true;
        a.collapse_dock(Dock::Bottom);
        a.cfg.view.ai_w = 300.0;
        let s = a.current_settings();
        assert!(s.view.side_collapsed && s.view.bottom_collapsed && !s.view.right_collapsed);
        let mut back = Settings::default();
        back.apply_json(&s.to_json(), true);
        assert!(back.view.side_collapsed && back.view.bottom_collapsed);
        assert_eq!(back.view.ai_w, 300.0);
    }

    #[test]
    fn glass_theme_only_toggles_the_blur_not_the_window() {
        let (mut a, _g) = app();
        a.cfg.glass_blur = true;
        a.sys.blur_on = false;
        a.sys.theme_glass = true;
        assert!(
            a.glass_wants_blur(),
            "半透明主题 + 模糊开: 要加模糊, 不重启"
        );
        a.cfg.glass_blur = false;
        assert!(!a.glass_wants_blur());
        a.cfg.glass_blur = true;
        a.sys.theme_glass = false;
        assert!(!a.glass_wants_blur(), "换回不透明主题: 去掉模糊");
    }

    #[test]
    fn collapse_commands_are_registered_with_keys_and_run() {
        let (mut a, _g) = app();
        let ctx = egui::Context::default();
        for id in [
            "app.side-collapse",
            "app.right-collapse",
            "app.bottom-collapse",
            "app.system-titlebar",
        ] {
            assert!(
                keymap::entries().iter().any(|e| e.id == id),
                "{id} 不在命令表里"
            );
        }
        a.run_app(KApp::SideCollapse, &ctx);
        assert!(a.cfg.view.side_collapsed);
        a.run_app(KApp::SystemTitleBar, &ctx);
        assert!(a.cfg.system_title_bar);
    }
}
