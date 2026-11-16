//! 描边小按钮的样式 (Tauri 的 `.md-btn` / `.bottom-toggle`): 透明底、1px 边、圆角 4, 悬停叠一层 `bg_hover`。
//! 只改传进来的这个 `Ui` 的样式 (用 `ui.scope` 包住), 不影响别处的按钮。

use eframe::egui::{self, Color32, Rect, Stroke, Ui, Vec2};

/// 描边色: 深色 `#454545`, 浅色 `#d0d7de`
pub fn edge(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x45, 0x45, 0x45)
    } else {
        Color32::from_rgb(0xd0, 0xd7, 0xde)
    }
}

/// 查找框输入框底色: 深色 `#3c3c3c`, 浅色白
pub fn input_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x3c, 0x3c, 0x3c)
    } else {
        Color32::WHITE
    }
}

/// 把这个 `Ui` 里的按钮改成描边小按钮
pub fn outline(ui: &mut Ui) {
    let dark = ui.visuals().dark_mode;
    let e = edge(dark);
    let hover = if dark {
        Color32::from_white_alpha(14)
    } else {
        Color32::from_black_alpha(14)
    };
    let v = ui.visuals_mut();
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::new(1.0, e);
        w.corner_radius = 4.0.into();
    }
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.active.weak_bg_fill = hover;
}

/// 统一缩进 / 折叠按钮配色三元组: `(填充底色, 边框, 前景文字色, 圆角)`
pub fn unified_collapse_palette(
    tc: &crate::view::ThemeColors,
    active: bool,
    hovered: bool,
    pressed: bool,
) -> (Color32, Stroke, Color32, f32) {
    if tc.comic {
        let hot = hovered || pressed || active;
        let (bg, fg) = if hot {
            (Color32::from_rgb(255, 204, 0), Color32::BLACK)
        } else {
            (Color32::from_rgb(32, 36, 52), Color32::WHITE)
        };
        (bg, Stroke::new(1.5, Color32::BLACK), fg, 4.0)
    } else if tc.retro16 {
        let hot = hovered || pressed || active;
        let (bg, fg) = if hot {
            (Color32::from_rgb(0, 255, 136), Color32::BLACK)
        } else {
            (Color32::from_rgb(21, 15, 43), Color32::from_rgb(255, 215, 0))
        };
        (bg, Stroke::new(1.0, Color32::from_rgb(255, 215, 0)), fg, 2.0)
    } else if tc.crystal || tc.clear {
        let alpha = if hovered || active { 56 } else { 28 };
        (
            Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 72)),
            Color32::WHITE,
            4.0,
        )
    } else {
        let bg = if active || pressed {
            tc.bg_active
        } else if hovered {
            tc.bg_hover
        } else {
            tc.bg_bar
        };
        let fg = if active || hovered { tc.fg } else { tc.fg_dim };
        (bg, Stroke::new(1.0, edge(tc.dark)), fg, 4.0)
    }
}

/// 构造统一样式的 Widget 缩进 / 折叠按钮
pub fn unified_collapse_button<'a>(
    label: &'a str,
    tc: &crate::view::ThemeColors,
    active: bool,
) -> egui::Button<'a> {
    let (bg, stroke, fg, radius) = unified_collapse_palette(tc, active, false, false);
    egui::Button::new(egui::RichText::new(label).size(11.0).color(fg))
        .fill(bg)
        .stroke(stroke)
        .corner_radius(radius)
        .min_size(Vec2::new(22.0, 20.0))
}

/// 使用 `Painter` 绘制与 `unified_collapse_button` 完全一致的统一缩进 / 折叠按钮
pub fn paint_unified_collapse_btn(
    ui: &Ui,
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    tc: &crate::view::ThemeColors,
    active: bool,
    hovered: bool,
    pressed: bool,
) {
    let (bg, stroke, fg, radius) = unified_collapse_palette(tc, active, hovered, pressed);
    let draw_rect = if tc.comic {
        if !pressed {
            painter.rect_filled(rect.translate(Vec2::new(1.5, 1.5)), radius, Color32::BLACK);
            rect
        } else {
            rect.translate(Vec2::new(1.5, 1.5))
        }
    } else {
        rect
    };
    painter.rect_filled(draw_rect, radius, bg);
    painter.rect_stroke(draw_rect, radius, stroke, egui::StrokeKind::Inside);
    painter.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        fg,
    );
    let _ = ui;
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
