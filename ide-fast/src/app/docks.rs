//! 可折叠的停靠区 (左侧栏 / 右侧面板 / 底部面板) 接到 `App` 上: 命令、细条、箭头按钮。
//! 状态翻转的规则在 `panels.rs` (纯函数 + 单测); 折叠状态存在 `cfg.view.*_collapsed`, 随设置落盘。
//! 折叠成细条时只画这一条细条, 面板里的内容不布局 (调用方按 `panels::mode` 决定走哪条路)。

use super::*;
use super::sysopts::BottomTab;
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

    /// 这个停靠区「想显示」吗 (右侧: AI 对话开着, 或 Markdown 预览开着)
    pub(super) fn dock_wanted(&self, d: Dock) -> bool {
        match d {
            Dock::Side => self.side.open,
            Dock::Right => self.ai.open || self.md_preview,
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
        let (open, collapsed) = panels::toggle_collapse(self.dock_wanted(d), self.dock_collapsed(d));
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
        let collapsed = self.dock_collapsed(d);
        let word = if collapsed { "展开" } else { "折叠" };
        let txt = egui::RichText::new(trz(word)).size(11.0).color(self.tc.fg_dim);
        let edge = if self.tc.dark { Color32::from_gray(69) } else { Color32::from_gray(204) };
        let btn = egui::Button::new(txt).fill(Color32::TRANSPARENT).stroke(egui::Stroke::new(1.0, edge)).corner_radius(5.0);
        if ui.add(btn).on_hover_text(trz(panels::tip(d, collapsed))).clicked() {
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
                let p = if d == Dock::Side { egui::Panel::left("side_strip") } else { egui::Panel::right("right_strip") };
                p.resizable(false).exact_size(panels::STRIP).frame(egui::Frame::NONE.fill(tc.bg_rail)).show(root, |ui| {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, panels::STRIP_BTN_H), Sense::click());
                    let resp = resp.on_hover_text(trz(panels::tip(d, collapsed)));
                    if resp.hovered() { ui.painter().rect_filled(rect, 0.0, tc.bg_hover); }
                    let col = if resp.hovered() { tc.fg } else { tc.fg_dim };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, panels::chevron(d, collapsed), egui::FontId::proportional(16.0), col);
                    toggle = resp.clicked();
                });
            }
            Dock::Bottom => {
                let frame = egui::Frame::NONE.fill(tc.bg_bar).inner_margin(egui::Margin::symmetric(8, 2));
                egui::Panel::bottom("bottom_strip").resizable(false).exact_size(panels::BOTTOM_HEAD).frame(frame).show(root, |ui| {
                    ui.horizontal_centered(|ui| {
                        for (t, key) in [(BottomTab::Terminal, "bt.terminal"), (BottomTab::Output, "bt.output"), (BottomTab::Problems, "bt.problems")] {
                            if ui.selectable_label(self.sys.bottom == t, tr(key)).clicked() { self.sys.bottom = t; }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.dock_chevron(ui, Dock::Bottom));
                    });
                });
            }
        }
        if toggle {
            self.collapse_dock(d);
        }
    }

    /// 一个停靠区这一帧怎么画
    pub(super) fn dock_mode(&self, d: Dock, visible: bool) -> Mode {
        panels::mode(visible, self.dock_collapsed(d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn app() -> (App, std::sync::MutexGuard<'static, ()>) {
        let g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        (App::build(vec![], None, Trace::new(), Settings::default(), false), g)
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
        assert!(a.ai.open && !a.cfg.view.right_collapsed, "右侧空着: 折叠命令 = 展开 AI 对话");
        a.collapse_dock(Dock::Right);
        assert!(a.cfg.view.right_collapsed);
        a.ai.open = false;
        a.md_preview = true;
        a.normalize_docks();
        assert!(a.cfg.view.right_collapsed, "预览还开着: 折叠标记保留");
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
    fn a_glass_theme_in_an_opaque_window_asks_for_a_restart_once() {
        let (mut a, _g) = app();
        let ctx = egui::Context::default();
        a.first_frame = false;
        a.sys.glass = false;
        a.sys.theme_glass = true;
        a.sys_frame(&ctx);
        assert!(a.sys.restart_ask, "要重启才能透明: 弹对话框");
        a.sys.restart_ask = false;
        a.sys_frame(&ctx);
        assert!(!a.sys.restart_ask, "同一个主题只问一次");
        a.sys.theme_glass = false;
        a.sys_frame(&ctx);
        a.sys.theme_glass = true;
        a.sys_frame(&ctx);
        assert!(a.sys.restart_ask, "换走再换回来又是一次新的选择");
    }

    #[test]
    fn collapse_commands_are_registered_with_keys_and_run() {
        let (mut a, _g) = app();
        let ctx = egui::Context::default();
        for id in ["app.side-collapse", "app.right-collapse", "app.bottom-collapse", "app.system-titlebar"] {
            assert!(keymap::entries().iter().any(|e| e.id == id), "{id} 不在命令表里");
        }
        a.run_app(KApp::SideCollapse, &ctx);
        assert!(a.cfg.view.side_collapsed);
        a.run_app(KApp::SystemTitleBar, &ctx);
        assert!(a.cfg.system_title_bar);
    }
}
