//! 描边小按钮的样式 (Tauri 的 `.md-btn` / `.bottom-toggle`): 透明底、1px 边、圆角 4, 悬停叠一层 `bg_hover`。
//! 只改传进来的这个 `Ui` 的样式 (用 `ui.scope` 包住), 不影响别处的按钮。

use eframe::egui::{Color32, Stroke, Ui};

/// 描边色: 深色 `#454545`, 浅色 `#d0d7de`
pub fn edge(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0x45, 0x45, 0x45) } else { Color32::from_rgb(0xd0, 0xd7, 0xde) }
}

/// 查找框输入框底色: 深色 `#3c3c3c`, 浅色白
pub fn input_bg(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0x3c, 0x3c, 0x3c) } else { Color32::WHITE }
}

/// 把这个 `Ui` 里的按钮改成描边小按钮
pub fn outline(ui: &mut Ui) {
    let dark = ui.visuals().dark_mode;
    let e = edge(dark);
    let hover = if dark { Color32::from_white_alpha(14) } else { Color32::from_black_alpha(14) };
    let v = ui.visuals_mut();
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::new(1.0, e);
        w.corner_radius = 4.0.into();
    }
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.active.weak_bg_fill = hover;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_and_input_colours_match_tauri() {
        assert_eq!(edge(true), Color32::from_rgb(69, 69, 69));
        assert_eq!(edge(false), Color32::from_rgb(208, 215, 222));
        assert_eq!(input_bg(true), Color32::from_rgb(60, 60, 60));
        assert_eq!(input_bg(false), Color32::WHITE);
    }
}
