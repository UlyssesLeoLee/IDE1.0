//! 菜单外观 (照 Tauri 版 `.mb-btn` / `.menu-dropdown` / `.menu-row`): 菜单按钮带助记字母, 下拉框底色 / 边 / 阴影 / 最小宽 240,
//! 行内边距 `5px 14px`、12px 字、悬停 `bg_active`, 勾选 / 单选统一成行首 `✓` 文字 (没有复选框方块)。
//! 样式只作用在菜单里 (`MenuConfig::style`), 不影响窗口里的普通控件。

use crate::view::ThemeColors;
use eframe::egui::{self, epaint::Shadow, Align2, Color32, FontId, Response, Stroke, TextFormat, Ui, Vec2};

/// 菜单栏按钮的字号 / 左右内边距 (Tauri `.mb-btn`: 13px, 约 8-12px)
pub const MENU_FONT: f32 = 13.0;
pub const MENU_PAD_X: f32 = 8.0;

/// 菜单名 -> 助记字母 (Tauri `MENU_DEF`: 文件 F / 编辑 E / 搜索 S / 视图 V / 设置 C / 窗口 W / 帮助 H; ide-fast 多出的菜单取各自的首字母)
pub fn mnemonic(menu_key: &str) -> char {
    match menu_key {
        "menu.file" => 'F',
        "menu.edit" => 'E',
        "menu.search" => 'S',
        "menu.fold" => 'O',
        "menu.plugin" => 'P',
        "menu.setting" => 'C',
        "menu.bookmark" => 'B',
        "menu.window" => 'W',
        "menu.macro" => 'M',
        "menu.view" => 'V',
        "menu.tool" => 'T',
        "menu.help" => 'H',
        _ => ' ',
    }
}

/// 菜单按钮文字: 菜单名 (13px, 随悬停变色) + 6px 间隔 + 带下划线的助记字母 (11px, 弱前景; `' '` = 不画, 窄窗口用)
pub fn menu_title(tc: &ThemeColors, label: &str, mn: char) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(label, 0.0, TextFormat { font_id: FontId::proportional(MENU_FONT), color: Color32::PLACEHOLDER, ..Default::default() });
    if mn != ' ' {
        job.append(&mn.to_string(), 6.0, TextFormat { font_id: FontId::proportional(11.0), color: tc.fg_dim, underline: Stroke::new(1.0, tc.fg_dim), ..Default::default() });
    }
    job
}

/// 菜单栏本身 (按钮一字排开) 的样式: 紧凑内边距、按钮之间不留缝, 悬停底色用 `bg_active`; 下拉框用 `style`
pub fn bar_style(tc: ThemeColors) -> impl Fn(&mut egui::Style) + Send + Sync + 'static {
    move |s: &mut egui::Style| {
        s.spacing.button_padding = Vec2::new(MENU_PAD_X, 3.0);
        s.spacing.item_spacing = Vec2::new(0.0, 0.0);
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(MENU_FONT));
        let v = &mut s.visuals;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.hovered.weak_bg_fill = tc.bg_active;
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        v.widgets.hovered.fg_stroke.color = tc.fg;
        v.widgets.inactive.fg_stroke.color = tc.fg;
        v.widgets.open.weak_bg_fill = tc.bg_active;
        v.widgets.open.fg_stroke.color = tc.fg;
    }
}

/// 一个菜单按钮的宽度 (含内边距)
pub fn title_width(ui: &Ui, label: &str, mn: char) -> f32 {
    let f = |t: &str, px: f32| ui.painter().layout_no_wrap(t.to_string(), FontId::proportional(px), Color32::WHITE).size().x;
    f(label, MENU_FONT) + if mn != ' ' { 6.0 + f(&mn.to_string(), 11.0) } else { 0.0 } + 2.0 * MENU_PAD_X
}

/// 菜单里的样式修改器: 下拉框(底色 = 菜单栏色, 1px 边, 直角, 阴影 `0 2px 8px`), 行内边距, 悬停色, 最小宽
pub fn style(tc: ThemeColors) -> impl Fn(&mut egui::Style) + Send + Sync + 'static {
    move |s: &mut egui::Style| {
        egui::containers::menu::menu_style(s);
        s.spacing.button_padding = Vec2::new(14.0, 5.0);
        s.spacing.item_spacing = Vec2::new(8.0, 0.0);
        s.spacing.menu_width = 240.0;
        s.spacing.menu_margin = egui::Margin::symmetric(0, 4);
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(12.0));
        let v = &mut s.visuals;
        v.window_fill = tc.bg_bar;
        v.window_stroke = Stroke::new(1.0, tc.border);
        v.menu_corner_radius = egui::CornerRadius::ZERO;
        v.popup_shadow = Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(128) };
        v.widgets.hovered.weak_bg_fill = tc.bg_active;
        v.widgets.hovered.fg_stroke.color = tc.fg;
        v.widgets.hovered.corner_radius = egui::CornerRadius::ZERO;
        v.widgets.inactive.corner_radius = egui::CornerRadius::ZERO;
        v.widgets.active.corner_radius = egui::CornerRadius::ZERO;
        v.widgets.active.weak_bg_fill = tc.bg_active;
        v.widgets.inactive.fg_stroke.color = tc.fg;
    }
}

/// 菜单正文放进竖向滚动区: 窗口矮于菜单时 (700 高的「视图」) 末尾几项不被裁掉, 改成滚动
pub fn scroll(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let max = menu_max_height(ui.ctx().content_rect().bottom(), ui.cursor().top());
    ui.set_max_height(max); // 弹出层的可用高度默认只有上一帧内容那么高, 先放开到窗口底
    egui::ScrollArea::vertical().max_height(max).auto_shrink([true, true]).show(ui, add);
}

/// 菜单正文最高多少: 从菜单顶到窗口底, 留 12px 边
pub fn menu_max_height(window_bottom: f32, menu_top: f32) -> f32 {
    (window_bottom - menu_top - 12.0).max(120.0)
}

/// 一行带勾选标记的菜单项: 选中时行首(左边距 7px 处)画一个 `✓`, 文字位置不变; 勾选和单选都用它
/// 菜单项文字「名字  键」拆成 (名字, 键): `lk` 用两个空格隔开, 键靠右对齐显示 (Tauri 菜单的样子)
pub fn split_keys(s: &str) -> (&str, &str) {
    s.rsplit_once("  ").unwrap_or((s, ""))
}

/// 菜单里的一项: 名字靠左, 快捷键靠右 (弱色)
pub fn mbtn(ui: &mut Ui, text: impl AsRef<str>) -> Response {
    let (name, keys) = split_keys(text.as_ref());
    let mut b = egui::Button::new(name);
    if !keys.is_empty() { b = b.shortcut_text(keys); }
    ui.add(b)
}

pub fn menu_check(ui: &mut Ui, tc: &ThemeColors, on: bool, label: impl AsRef<str>) -> Response {
    let (name, keys) = split_keys(label.as_ref());
    let mut b = egui::Button::new(name).fill(Color32::TRANSPARENT);
    if !keys.is_empty() { b = b.shortcut_text(keys); }
    let r = ui.add(b);
    if on {
        ui.painter().text(egui::pos2(r.rect.left() + 7.0, r.rect.center().y), Align2::LEFT_CENTER, "✓", FontId::proportional(12.0), tc.fg);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_body_is_capped_to_the_window_but_never_collapses() {
        assert_eq!(menu_max_height(700.0, 40.0), 648.0);
        assert_eq!(menu_max_height(100.0, 90.0), 120.0);
    }

    #[test]
    fn menu_text_splits_name_and_keys() {
        assert_eq!(split_keys("新建  Ctrl+N"), ("新建", "Ctrl+N"));
        assert_eq!(split_keys("打开文件夹"), ("打开文件夹", ""));
    }

    #[test]
    fn every_menu_has_a_distinct_mnemonic() {
        let keys = ["menu.file", "menu.edit", "menu.search", "menu.fold", "menu.plugin", "menu.setting", "menu.bookmark", "menu.window", "menu.macro", "menu.view", "menu.tool", "menu.help"];
        let mut seen = std::collections::HashSet::new();
        for k in keys {
            let m = mnemonic(k);
            assert_ne!(m, ' ', "{k}");
            assert!(seen.insert(m), "助记字母重复: {m}");
        }
    }

    #[test]
    fn tauri_letters_are_kept_for_the_shared_menus() {
        assert_eq!((mnemonic("menu.file"), mnemonic("menu.edit"), mnemonic("menu.search"), mnemonic("menu.view"), mnemonic("menu.setting"), mnemonic("menu.window"), mnemonic("menu.help")), ('F', 'E', 'S', 'V', 'C', 'W', 'H'));
    }

    #[test]
    fn menu_title_has_label_then_underlined_letter() {
        let tc = ThemeColors::builtin(true);
        let j = menu_title(&tc, "文件", 'F');
        assert_eq!(j.sections.len(), 2);
        assert_eq!(j.text, "文件F");
        assert_eq!(j.sections[1].leading_space, 6.0);
        assert_ne!(j.sections[1].format.underline, Stroke::NONE);
    }
}
