//! 窗口外围: 工具栏 / 功能键栏 / 活动栏。三者默认都关 (极简布局 = 单个文本框), 在「视图」菜单里打开; 命令都走注册表, 键位改了会跟着变。

use super::*;
use crate::panels::Dock;
use crate::sidebar::Tab;
use egui::Key;

const FKEYS: [Key; 12] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
];

/// 命令 id 在注册表里的下标
pub(super) fn index_of(id: &str) -> Option<usize> {
    keymap::entries().iter().position(|e| e.id == id)
}

/// F1..F12 各自 (不带修饰键) 绑定的命令下标
pub fn function_key_cmds(bound: &[(usize, egui::Modifiers, Key)]) -> [Option<usize>; 12] {
    let mut out = [None; 12];
    for (n, k) in FKEYS.iter().enumerate() {
        out[n] = bound
            .iter()
            .find(|(_, m, key)| key == k && *m == egui::Modifiers::NONE)
            .map(|b| b.0);
    }
    out
}

impl App {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut run: Option<usize> = None;
        let cmds = self.cfg.toolbar_cmds.clone(); // 首选项 > 工具栏里自定义的按钮
        ui.horizontal_wrapped(|ui| {
            for id in &cmds {
                if id.is_empty() {
                    ui.separator();
                    continue;
                }
                let Some(i) = index_of(id) else { continue };
                let e = &keymap::entries()[i];
                let tip = crate::i18n::lk_for(e);
                // 有图标的命令用画笔图标按钮 (不靠图标字体), 没有的仍是文字按钮
                let clicked = if crate::icons::has_icon(id) {
                    crate::icons::icon_button(ui, id.as_str(), tip).clicked()
                } else {
                    ui.small_button(e.label.text()).tip(tip).clicked()
                };
                if clicked {
                    run = Some(i);
                }
            }
        });
        if let Some(i) = run {
            self.run_entry(i, ctx);
        }
    }

    pub(super) fn funckey_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let cmds = function_key_cmds(&keymap::bound());
        let mut run: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (n, c) in cmds.iter().enumerate() {
                let (label, tip) = match c {
                    Some(i) => (
                        keymap::entries()[*i].label.text(),
                        crate::i18n::lk_for(&keymap::entries()[*i]),
                    ),
                    None => ("", String::new()),
                };
                let b = ui.add_enabled(
                    c.is_some(),
                    egui::Button::new(format!("F{} {}", n + 1, label)).small(),
                );
                if c.is_some() && b.tip(tip).clicked() {
                    run = *c;
                }
            }
        });
        if let Some(i) = run {
            self.run_entry(i, ctx);
        }
    }

    /// 活动栏 (Tauri 的 `#activity-bar`): 44 宽 rail 底色, 32x32 的矢量图标钮, 间距 4;
    /// 活动项深色 `rgba(255,255,255,.12)` 底 + 白图标 / 浅色 `#e8edf3` 底 + 蓝图标, 左缘一条 2px 竖线; 悬停叠 `bg_hover`。
    /// 点哪个就在侧栏里显示哪个面板; 再点已经显示的那个 = 收起侧栏
    pub(super) fn activity_rail(&mut self, ui: &mut egui::Ui) {
        let tc = self.tc;
        let shown = self.dock_wanted(Dock::Side) && !self.dock_collapsed(Dock::Side);
        let edge = ui.max_rect().left();
        ui.spacing_mut().item_spacing = Vec2::new(0.0, RAIL_GAP);
        ui.add_space(RAIL_GAP);
        let mut hit: Option<Tab> = None;
        ui.vertical_centered(|ui| {
            for (t, icon, tip) in RAIL {
                let on = shown && self.side.tab == *t;
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(RAIL_BTN), Sense::click());
                let p = ui.painter();
                let (act_bg, act_fg) = rail_active(tc.dark, tc.blue);
                if on {
                    if tc.is_translucent() {
                        let (act_fill, border, glow) = if tc.crystal {
                            (
                                Color32::from_rgba_unmultiplied(255, 255, 255, 52),
                                Color32::from_rgba_unmultiplied(255, 255, 255, 150),
                                Some(Color32::from_rgba_unmultiplied(0, 240, 255, 95)),
                            )
                        } else {
                            (
                                Color32::from_rgba_unmultiplied(255, 255, 255, 42),
                                Color32::from_rgba_unmultiplied(255, 255, 255, 120),
                                Some(Color32::from_rgba_unmultiplied(56, 189, 248, 65)),
                            )
                        };
                        super::glass_render::paint_glowing_pill(p, rect, 6.0, act_fill, border, glow);
                    } else {
                        p.rect_filled(rect, 6.0, act_bg);
                    }
                } else if resp.hovered() {
                    p.rect_filled(rect, 6.0, tc.bg_hover);
                }
                let col = if on {
                    act_fg
                } else if resp.hovered() {
                    tc.fg
                } else {
                    tc.fg_dim
                };
                crate::svgicon::paint(p, rect.center(), 18.0, icon, col, 1.8);
                if on {
                    let bar = if tc.crystal {
                        tc.mat_prism_cyan
                    } else if tc.clear {
                        tc.blue
                    } else if tc.dark {
                        Color32::from_rgb(0x00, 0x7a, 0xcc)
                    } else {
                        Color32::from_rgb(0x00, 0x5f, 0xb8)
                    };
                    if tc.is_translucent() {
                        p.rect_filled(
                            Rect::from_center_size(
                                egui::pos2(edge + 1.5, rect.center().y),
                                Vec2::new(5.0, RAIL_BTN * 0.6 + 2.0),
                            ),
                            2.5,
                            Color32::from_rgba_unmultiplied(bar.r(), bar.g(), bar.b(), 75),
                        );
                    }
                    p.rect_filled(
                        Rect::from_center_size(
                            egui::pos2(edge + 1.5, rect.center().y),
                            Vec2::new(3.0, RAIL_BTN * 0.6),
                        ),
                        2.0,
                        bar,
                    );
                }
                if resp.tip(trz(tip)).clicked() {
                    hit = Some(*t);
                }
            }
        });
        if let Some(t) = hit {
            if shown && self.side.tab == t {
                self.side.open = false;
            } else {
                self.side.open = true;
                self.side.tab = t;
                self.expand_dock(Dock::Side);
            }
        }
    }
}

/// 活动栏按钮边长 / 间距
const RAIL_BTN: f32 = 32.0;
const RAIL_GAP: f32 = 4.0;
/// 活动栏宽
pub const RAIL_W: f32 = 44.0;

/// 活动栏的五个入口, 顺序与 editor.html 一致: 资源管理器 / 搜索 / 源代码管理 / 大纲 / 扩展
const RAIL: &[(Tab, &[crate::svgicon::PathDef], &str)] = &[
    (Tab::Files, crate::svgicon::EXPLORER, "资源管理器"),
    (Tab::Search, crate::svgicon::SEARCH, "搜索"),
    (Tab::Scm, crate::svgicon::SCM, "源代码管理"),
    (Tab::Outline, crate::svgicon::OUTLINE, "大纲"),
    (Tab::Plugins, crate::svgicon::EXTENSIONS, "扩展"),
];

/// 活动项的 (底色, 图标色): 深色白 12% 叠色 + 白图标; 浅色 `#e8edf3` + 强调蓝
pub fn rail_active(dark: bool, blue: Color32) -> (Color32, Color32) {
    if dark {
        (Color32::from_white_alpha(31), Color32::WHITE)
    } else {
        (Color32::from_rgb(0xe8, 0xed, 0xf3), blue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_has_the_five_tauri_entries_in_order() {
        let tabs: Vec<Tab> = RAIL.iter().map(|r| r.0).collect();
        assert!(
            tabs == vec![
                Tab::Files,
                Tab::Search,
                Tab::Scm,
                Tab::Outline,
                Tab::Plugins
            ]
        );
    }

    #[test]
    fn active_colours_follow_dark_and_light() {
        assert_eq!(rail_active(true, Color32::RED).1, Color32::WHITE);
        assert_eq!(rail_active(false, Color32::RED).1, Color32::RED);
    }

    #[test]
    fn toolbar_commands_all_exist_in_the_registry() {
        for id in crate::settings::DEFAULT_TOOLBAR
            .iter()
            .filter(|s| !s.is_empty())
        {
            assert!(index_of(id).is_some(), "工具栏命令 {id} 不在注册表里");
        }
        let d: Vec<String> = crate::settings::DEFAULT_TOOLBAR
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            crate::settings::sanitize_toolbar(d.clone()),
            d,
            "默认工具栏清洗后原样"
        );
    }

    #[test]
    fn function_keys_pick_unmodified_bindings_only() {
        let b = vec![
            (3, egui::Modifiers::NONE, Key::F3),
            (4, egui::Modifiers::CTRL, Key::F4),
            (5, egui::Modifiers::NONE, Key::F5),
        ];
        let m = function_key_cmds(&b);
        assert_eq!((m[2], m[3], m[4], m[0]), (Some(3), None, Some(5), None));
    }
}
