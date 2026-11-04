//! 工具栏图标: 用 egui 画笔直接画线条 / 矩形 / 圆, 不依赖图标字体 (字体里缺字就会显示成方块)。
//! 每个图标画在一个 16x16 的坐标系里 (`u(x, y)` 换算到按钮矩形), 线条颜色跟主题文字色走。没有图标的命令回退成文字按钮。

use crate::ui_ext::TipExt;
use eframe::egui::{epaint::PathStroke, pos2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

/// 按钮边长 (点)
pub const SIZE: f32 = 22.0;

/// 哪些命令 id 有图标
pub fn has_icon(id: &str) -> bool {
    matches!(id, "c.new-file" | "c.open-file" | "c.save" | "c.undo" | "c.redo" | "c.cut" | "c.copy" | "c.paste" | "c.find" | "c.replace" | "c.grep" | "c.split" | "c.terminal" | "c.palette")
}

/// 在 `rect` 里画命令 `id` 的图标; 返回是否有这个图标
pub fn paint(ui: &Ui, rect: Rect, id: &str, col: Color32) -> bool {
    if !has_icon(id) { return false; }
    let p = ui.painter();
    let s = rect.shrink2(Vec2::splat((rect.width() - 16.0).max(0.0) / 2.0)); // 居中的 16x16
    let u = |x: f32, y: f32| -> Pos2 { pos2(s.min.x + x, s.min.y + y) };
    let st = Stroke::new(1.3, col);
    let line = |a: Pos2, b: Pos2| { p.line_segment([a, b], st); };
    let poly = |pts: &[Pos2]| { p.add(Shape::line(pts.to_vec(), PathStroke::new(1.3, col))); };
    let boxr = |x0: f32, y0: f32, x1: f32, y1: f32| { p.rect_stroke(Rect::from_min_max(u(x0, y0), u(x1, y1)), 1.0, st, StrokeKind::Middle); };
    match id {
        "c.new-file" => {
            poly(&[u(3.0, 1.5), u(9.5, 1.5), u(13.0, 5.0), u(13.0, 14.5), u(3.0, 14.5), u(3.0, 1.5)]);
            poly(&[u(9.5, 1.5), u(9.5, 5.0), u(13.0, 5.0)]);
        }
        "c.open-file" => {
            poly(&[u(1.5, 3.5), u(6.0, 3.5), u(7.5, 5.0), u(13.0, 5.0), u(13.0, 13.5), u(1.5, 13.5), u(1.5, 3.5)]);
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
            poly(&[u(m(2.0), 6.5), u(m(9.0), 6.5), u(m(13.0), 8.5), u(m(13.0), 12.5)]);
        }
        "c.cut" => {
            p.circle_stroke(u(4.5, 12.0), 2.3, st);
            p.circle_stroke(u(11.5, 12.0), 2.3, st);
            line(u(5.5, 10.0), u(11.0, 1.5));
            line(u(10.5, 10.0), u(5.0, 1.5));
        }
        "c.copy" => {
            boxr(5.5, 1.5, 14.0, 11.0);
            poly(&[u(3.5, 5.0), u(2.0, 5.0), u(2.0, 14.5), u(10.0, 14.5), u(10.0, 13.0)]);
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
pub fn icon_button(ui: &mut Ui, id: &str, tip: String) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let w = if resp.is_pointer_button_down_on() { &v.widgets.active } else if resp.hovered() { &v.widgets.hovered } else { &v.widgets.inactive };
        if resp.hovered() || resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 3.0, w.weak_bg_fill);
        }
        let col = w.fg_stroke.color;
        paint(ui, rect, id, col);
    }
    resp.tip(tip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_toolbar_command_with_an_icon_paints_something() {
        let ctx = eframe::egui::Context::default();
        let mut shapes = 0;
        let mut out = ctx.run_ui(Default::default(), |root| {
            {
                let ui = &mut *root;
                for id in ["c.new-file", "c.open-file", "c.save", "c.undo", "c.redo", "c.cut", "c.copy", "c.paste", "c.find", "c.replace", "c.grep", "c.split", "c.terminal", "c.palette"] {
                    assert!(paint(ui, Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(SIZE)), id, Color32::WHITE), "{id}");
                    shapes += 1;
                }
                assert!(!paint(ui, Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(SIZE)), "c.nope", Color32::WHITE));
            }
        });
        out.textures_delta.clear();
        assert_eq!(shapes, 14);
        assert!(has_icon("c.save") && !has_icon("c.nope"));
    }
}
