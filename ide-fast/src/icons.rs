//! 工具栏图标: 用 egui 画笔直接画线条 / 矩形 / 圆, 不依赖图标字体 (字体里缺字就会显示成方块)。
//! 每个图标画在一个 16x16 的坐标系里 (`u(x, y)` 换算到按钮矩形), 线条颜色跟主题文字色走。没有图标的命令回退成文字按钮。

use crate::ui_ext::TipExt;
use eframe::egui::{
    epaint::PathStroke, pos2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2,
};

/// 按钮边长 (点)
pub const SIZE: f32 = 22.0;

/// 哪些命令 id 有图标
pub fn has_icon(id: &str) -> bool {
    matches!(
        id,
        "c.new-file"
            | "c.open-file"
            | "c.save"
            | "c.undo"
            | "c.redo"
            | "c.cut"
            | "c.copy"
            | "c.paste"
            | "c.find"
            | "c.replace"
            | "c.grep"
            | "c.split"
            | "c.terminal"
            | "c.palette"
            | "c.nema-convert"
    )
}

/// 在 `rect` 里画命令 `id` 的图标; 返回是否有这个图标
pub fn paint(ui: &Ui, rect: Rect, id: &str, col: Color32) -> bool {
    if !has_icon(id) {
        return false;
    }
    let p = ui.painter();
    let s = rect.shrink2(Vec2::splat((rect.width() - 16.0).max(0.0) / 2.0)); // 居中的 16x16
    let u = |x: f32, y: f32| -> Pos2 { pos2(s.min.x + x, s.min.y + y) };
    let st = Stroke::new(1.3, col);
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], st);
    };
    let poly = |pts: &[Pos2]| {
        p.add(Shape::line(pts.to_vec(), PathStroke::new(1.3, col)));
    };
    let boxr = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_stroke(
            Rect::from_min_max(u(x0, y0), u(x1, y1)),
            1.0,
            st,
            StrokeKind::Middle,
        );
    };
    match id {
        "c.new-file" => {
            poly(&[
                u(3.0, 1.5),
                u(9.5, 1.5),
                u(13.0, 5.0),
                u(13.0, 14.5),
                u(3.0, 14.5),
                u(3.0, 1.5),
            ]);
            poly(&[u(9.5, 1.5), u(9.5, 5.0), u(13.0, 5.0)]);
        }
        "c.open-file" => {
            poly(&[
                u(1.5, 3.5),
                u(6.0, 3.5),
                u(7.5, 5.0),
                u(13.0, 5.0),
                u(13.0, 13.5),
                u(1.5, 13.5),
                u(1.5, 3.5),
            ]);
            poly(&[u(1.5, 13.5), u(3.5, 7.5), u(15.0, 7.5), u(13.0, 13.5)]);
        }
        "c.save" => {
            boxr(2.0, 2.0, 14.0, 14.0);
            boxr(5.0, 2.0, 11.0, 6.0);
            boxr(4.5, 9.0, 11.5, 14.0);
        }
        "c.undo" | "c.redo" => {
            let m = |x: f32| if id == "c.redo" { 16.0 - x } else { x };
            poly(&[u(m(5.0), 3.5), u(m(2.0), 6.5), u(m(5.0), 9.5)]);
            poly(&[
                u(m(2.0), 6.5),
                u(m(9.0), 6.5),
                u(m(13.0), 8.5),
                u(m(13.0), 12.5),
            ]);
        }
        "c.cut" => {
            p.circle_stroke(u(4.5, 12.0), 2.3, st);
            p.circle_stroke(u(11.5, 12.0), 2.3, st);
            line(u(5.5, 10.0), u(11.0, 1.5));
            line(u(10.5, 10.0), u(5.0, 1.5));
        }
        "c.copy" => {
            boxr(5.5, 1.5, 14.0, 11.0);
            poly(&[
                u(3.5, 5.0),
                u(2.0, 5.0),
                u(2.0, 14.5),
                u(10.0, 14.5),
                u(10.0, 13.0),
            ]);
        }
        "c.paste" => {
            boxr(2.5, 3.0, 13.5, 14.5);
            boxr(5.5, 1.0, 10.5, 4.5);
            line(u(5.0, 8.0), u(11.0, 8.0));
            line(u(5.0, 11.0), u(11.0, 11.0));
        }
        "c.find" | "c.grep" => {
            p.circle_stroke(u(6.5, 6.5), 4.2, st);
            line(u(9.7, 9.7), u(14.0, 14.0));
            if id == "c.grep" {
                line(u(1.5, 14.0), u(7.0, 14.0));
                line(u(1.5, 11.5), u(4.0, 11.5));
            }
        }
        "c.replace" => {
            poly(&[u(2.0, 5.0), u(13.0, 5.0)]);
            poly(&[u(10.5, 2.5), u(13.5, 5.0), u(10.5, 7.5)]);
            poly(&[u(14.0, 11.0), u(3.0, 11.0)]);
            poly(&[u(5.5, 8.5), u(2.5, 11.0), u(5.5, 13.5)]);
        }
        "c.split" => {
            boxr(1.5, 2.5, 14.5, 13.5);
            line(u(8.0, 2.5), u(8.0, 13.5));
        }
        "c.terminal" => {
            boxr(1.5, 2.5, 14.5, 13.5);
            poly(&[u(4.0, 5.5), u(7.0, 8.0), u(4.0, 10.5)]);
            line(u(8.5, 10.5), u(12.0, 10.5));
        }
        "c.nema-convert" => {
            boxr(1.5, 2.5, 14.5, 13.5);
            line(u(5.5, 2.5), u(5.5, 13.5));
            line(u(1.5, 6.5), u(14.5, 6.5));
            poly(&[u(8.5, 9.5), u(10.5, 11.5), u(13.0, 8.5)]);
        }
        "c.palette" => {
            boxr(1.5, 3.0, 14.5, 13.0);
            poly(&[u(4.0, 6.0), u(6.5, 8.0), u(4.0, 10.0)]);
            line(u(8.0, 10.0), u(12.0, 10.0));
            line(u(8.0, 6.0), u(10.5, 6.0));
        }
        _ => return false,
    }
    true
}

/// 图标按钮: 悬停高亮, `tip` 做悬停提示。调用方先用 `has_icon` 判断
/// 为各个主题绘制风格化的快捷按钮底框与边框/投影/高光, 并返回图标应绘制的矩形和前景颜色
pub fn paint_themed_quick_btn(
    painter: &eframe::egui::Painter,
    rect: Rect,
    tc: &crate::view::ThemeColors,
    hovered: bool,
    active: bool,
    highlighted: bool,
) -> (Rect, Color32) {
    if tc.retro16 {
        // Retro 16-Bit: 街机微动开关 3D 斜切按键 + 0px 锐利硬像素边框
        let outer_border = if active {
            Color32::from_rgb(0, 0, 0)
        } else if hovered {
            if highlighted {
                Color32::from_rgb(0, 240, 255)
            } else {
                Color32::from_rgb(255, 215, 0) // 街机金色高光
            }
        } else if highlighted {
            Color32::from_rgb(0, 240, 255) // 霓虹青边框
        } else {
            Color32::from_rgb(45, 35, 84) // --border
        };

        let bg = if active {
            Color32::from_rgb(14, 11, 28) // 按下沉陷深黑
        } else if hovered {
            Color32::from_rgb(39, 30, 77) // 悬停紫
        } else if highlighted {
            Color32::from_rgb(30, 23, 58)
        } else {
            Color32::from_rgb(21, 15, 43)
        };

        // 外层像素底色
        painter.rect_filled(rect, 0.0, bg);
        // 外层像素边框
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, outer_border), StrokeKind::Inside);

        // 3D Bevel 棱角线 (按下去时反向凹陷)
        let (top_left_col, bot_right_col) = if active {
            (Color32::from_rgb(10, 7, 23), Color32::from_rgb(79, 62, 145))
        } else if hovered {
            (Color32::from_rgb(255, 215, 0), Color32::from_rgb(22, 15, 46))
        } else {
            (Color32::from_rgb(79, 62, 145), Color32::from_rgb(10, 7, 23))
        };

        let inner = rect.shrink(1.0);
        // 顶和左棱
        painter.line_segment([inner.left_top(), inner.right_top()], Stroke::new(1.0, top_left_col));
        painter.line_segment([inner.left_top(), inner.left_bottom()], Stroke::new(1.0, top_left_col));
        // 底和右棱
        painter.line_segment([inner.left_bottom(), inner.right_bottom()], Stroke::new(1.0, bot_right_col));
        painter.line_segment([inner.right_top(), inner.right_bottom()], Stroke::new(1.0, bot_right_col));

        // 按下时发生 1px 物理位移
        let content_rect = if active {
            rect.translate(Vec2::new(1.0, 1.0))
        } else {
            rect
        };

        let fg = if active {
            if highlighted { Color32::from_rgb(0, 255, 102) } else { Color32::from_rgb(0, 240, 255) }
        } else if hovered {
            if highlighted { Color32::from_rgb(0, 255, 102) } else { Color32::from_rgb(255, 215, 0) }
        } else if highlighted {
            Color32::from_rgb(0, 240, 255)
        } else {
            tc.fg
        };
        (content_rect, fg)
    } else if tc.comic {
        // Comic 3渲2: 美漫赛璐珞 2px 粗黑边框 + 实体纯黑硬边波普投影
        let shadow_offset = if active {
            Vec2::ZERO
        } else {
            Vec2::new(1.5, 1.5)
        };

        // 纯黑硬边阴影 (未按下时投影)
        if !active {
            painter.rect_filled(rect.translate(shadow_offset), 2.0, Color32::BLACK);
        }

        let actual_rect = if active {
            rect.translate(Vec2::new(1.5, 1.5))
        } else {
            rect
        };

        let bg = if active {
            Color32::from_rgb(255, 170, 0) // 按下深波普橙
        } else if hovered {
            Color32::from_rgb(255, 204, 0) // 悬停经典波普黄
        } else if highlighted {
            Color32::from_rgb(50, 54, 76)
        } else {
            Color32::from_rgb(37, 40, 56)
        };

        let border_col = if highlighted && !hovered && !active {
            Color32::from_rgb(255, 204, 0)
        } else {
            Color32::BLACK
        };

        painter.rect_filled(actual_rect, 2.0, bg);
        painter.rect_stroke(actual_rect, 2.0, Stroke::new(1.5, border_col), StrokeKind::Inside);

        let fg = if active || hovered {
            Color32::BLACK // 黄底黑字/黑图标, 极高可辨识波普反差
        } else if highlighted {
            Color32::from_rgb(255, 204, 0)
        } else {
            tc.fg
        };
        (actual_rect, fg)
    } else if tc.crystal {
        // Crystal 水晶透明: 6px 晶钻胶囊 + 顶沿 1px 晶面白光切面反射 + 棱镜色散光晕
        let round = 6.0;
        let bg = if active {
            Color32::from_rgba_unmultiplied(255, 105, 180, 65) // 按下粉色散
        } else if hovered {
            Color32::from_rgba_unmultiplied(255, 255, 255, 55)
        } else if highlighted {
            Color32::from_rgba_unmultiplied(0, 240, 255, 40)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 16)
        };

        let border_col = if active {
            tc.mat_prism_pink
        } else if hovered {
            tc.mat_prism_cyan // 棱镜青色散高光
        } else if highlighted {
            Color32::from_rgba_unmultiplied(0, 240, 255, 180)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 50)
        };

        painter.rect_filled(rect, round, bg);
        painter.rect_stroke(rect, round, Stroke::new(1.0, border_col), StrokeKind::Inside);

        // 顶沿 1px 晶面白光切面反射
        let facet_col = if hovered || active {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(160)
        };
        let hline_min_x = rect.left() + 3.0;
        let hline_max_x = rect.right() - 3.0;
        if hline_max_x > hline_min_x {
            painter.line_segment(
                [pos2(hline_min_x, rect.top() + 1.0), pos2(hline_max_x, rect.top() + 1.0)],
                Stroke::new(1.0, facet_col),
            );
        }

        let fg = if active || hovered {
            Color32::WHITE
        } else if highlighted {
            Color32::from_rgb(0, 240, 255)
        } else {
            Color32::from_white_alpha(220)
        };
        (rect, fg)
    } else if tc.clear {
        // Clear 玻璃透明: 7px 磨砂圆角胶囊 + 双向柔和半透明
        let round = 7.0;
        let bg = if active {
            Color32::from_rgba_unmultiplied(255, 255, 255, 100)
        } else if hovered {
            Color32::from_rgba_unmultiplied(255, 255, 255, 65)
        } else if highlighted {
            Color32::from_rgba_unmultiplied(255, 255, 255, 45)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 15)
        };

        let border_col = if active {
            Color32::from_rgba_unmultiplied(255, 255, 255, 180)
        } else if hovered {
            Color32::from_rgba_unmultiplied(255, 255, 255, 120)
        } else if highlighted {
            Color32::from_rgba_unmultiplied(255, 255, 255, 95)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 35)
        };

        painter.rect_filled(rect, round, bg);
        painter.rect_stroke(rect, round, Stroke::new(1.0, border_col), StrokeKind::Inside);

        let fg = if active || hovered {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(200)
        };
        (rect, fg)
    } else {
        // Dark / Light / 现代默认: 3px 圆角矩形
        let round = 3.0;
        let bg = if active {
            tc.bg_active
        } else if hovered {
            tc.bg_hover
        } else if highlighted {
            tc.bg_active
        } else {
            Color32::TRANSPARENT
        };

        let border_col = if active {
            tc.border_strong
        } else if hovered {
            tc.border
        } else if highlighted {
            tc.blue
        } else {
            Color32::TRANSPARENT
        };

        if bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, round, bg);
        }
        if border_col != Color32::TRANSPARENT {
            painter.rect_stroke(rect, round, Stroke::new(1.0, border_col), StrokeKind::Inside);
        }

        let fg = if active || hovered {
            tc.fg
        } else if highlighted {
            tc.blue
        } else {
            tc.fg_dim
        };
        (rect, fg)
    }
}

/// 支持主题专属风格的图标按钮: 悬停高亮,  tip 做悬停提示。调用方先用 has_icon 判断
pub fn icon_button_themed(
    ui: &mut Ui,
    id: &str,
    tip: String,
    tc: &crate::view::ThemeColors,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::click());
    if ui.is_rect_visible(rect) {
        let is_hovered = resp.hovered();
        let is_active = resp.is_pointer_button_down_on();
        let (content_rect, col) = paint_themed_quick_btn(
            ui.painter(),
            rect.shrink(1.0),
            tc,
            is_hovered,
            is_active,
            false,
        );
        paint(ui, content_rect, id, col);
    }
    resp.tip(tip)
}

/// 图标按钮: 悬停高亮,  tip 做悬停提示。回退至内置默认样式
pub fn icon_button(ui: &mut Ui, id: &str, tip: String) -> Response {
    let tc = crate::view::ThemeColors::default();
    icon_button_themed(ui, id, tip, &tc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_toolbar_command_with_an_icon_paints_something() {
        let ctx = eframe::egui::Context::default();
        let mut shapes = 0;
        let mut out = ctx.run_ui(Default::default(), |root| {
            let ui = &mut *root;
            for id in [
                "c.new-file",
                "c.open-file",
                "c.save",
                "c.undo",
                "c.redo",
                "c.cut",
                "c.copy",
                "c.paste",
                "c.find",
                "c.replace",
                "c.grep",
                "c.split",
                "c.terminal",
                "c.palette",
                "c.nema-convert",
            ] {
                assert!(
                    paint(
                        ui,
                        Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(SIZE)),
                        id,
                        Color32::WHITE
                    ),
                    "{id}"
                );
                shapes += 1;
            }
            assert!(!paint(
                ui,
                Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(SIZE)),
                "c.nope",
                Color32::WHITE
            ));
        });
        out.textures_delta.clear();
        assert_eq!(shapes, 15);
        assert!(has_icon("c.save") && !has_icon("c.nope"));
    }
}
