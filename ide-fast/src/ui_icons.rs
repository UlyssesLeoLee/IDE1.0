//! 纯矢量自绘 UI 图标系统 (Pure-Vector Procedural UI Icons)
//!
//! 彻底杜绝缺失字符回退导致的空心方块 (Tofu □) 与表情符号兼容性问题：
//! 所有图标均由 `egui::Painter` 直接以精确几何路径、圆形与多边形矢量绘制，
//! 不依赖任何系统 Emoji 字体或第三方图标字体。

use eframe::egui::{
    self, epaint::PathStroke, pos2, vec2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, WidgetText,
};
use std::f32::consts::TAU;

/// 全应用通用的纯矢量 UI 图标种类
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UiIcon {
    Globe,
    Folder,
    Bolt,
    Shield,
    CheckCircle,
    Search,
    Terminal,
    User,
    Gear,
    Chat,
    Collab,
    Trash,
    Copy,
    Insert,
    Spark,
    Cpu,
    Plug,
    Lock,
    Pin,
    Chart,
    Wizard,
    Star,
    Play,
    ArrowRight,
    Info,
    Warning,
    Error,
    Quote,
    Note,
    Wrench,
}

impl UiIcon {
    pub const ALL: &'static [UiIcon] = &[
        UiIcon::Globe,
        UiIcon::Folder,
        UiIcon::Bolt,
        UiIcon::Shield,
        UiIcon::CheckCircle,
        UiIcon::Search,
        UiIcon::Terminal,
        UiIcon::User,
        UiIcon::Gear,
        UiIcon::Chat,
        UiIcon::Collab,
        UiIcon::Trash,
        UiIcon::Copy,
        UiIcon::Insert,
        UiIcon::Spark,
        UiIcon::Cpu,
        UiIcon::Plug,
        UiIcon::Lock,
        UiIcon::Pin,
        UiIcon::Chart,
        UiIcon::Wizard,
        UiIcon::Star,
        UiIcon::Play,
        UiIcon::ArrowRight,
        UiIcon::Info,
        UiIcon::Warning,
        UiIcon::Error,
        UiIcon::Quote,
        UiIcon::Note,
        UiIcon::Wrench,
    ];
}

/// 在给定矩形内绘制清晰对勾 (供复选框、步骤徽章、菜单勾选项、日志状态使用)
pub fn paint_checkmark(p: &egui::Painter, rect: Rect, stroke_w: f32, color: Color32) {
    let side = rect.width().min(rect.height()).max(4.0);
    let r = Rect::from_center_size(rect.center(), Vec2::splat(side));
    let u = |x: f32, y: f32| pos2(r.left() + x * side, r.top() + y * side);
    let pts = vec![u(0.18, 0.52), u(0.42, 0.76), u(0.84, 0.26)];
    p.add(Shape::line(pts, PathStroke::new(stroke_w, color)));
}

/// 绘制一颗对称四角菱星 / 闪烁晶体
fn paint_four_point_star(p: &egui::Painter, c: Pos2, outer: f32, inner: f32, color: Color32) {
    let top = pos2(c.x, c.y - outer);
    let right = pos2(c.x + outer, c.y);
    let bot = pos2(c.x, c.y + outer);
    let left = pos2(c.x - outer, c.y);
    p.add(Shape::convex_polygon(
        vec![top, pos2(c.x + inner, c.y), bot, pos2(c.x - inner, c.y)],
        color,
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![left, pos2(c.x, c.y - inner), right, pos2(c.x, c.y + inner)],
        color,
        Stroke::NONE,
    ));
}

/// 在 `rect` 中居中绘制 `UiIcon` 纯矢量图标 (彻底无字体依赖、无空心方块)
pub fn paint(p: &egui::Painter, rect: Rect, icon: UiIcon, color: Color32) {
    let side = rect.width().min(rect.height()).max(6.0);
    let box_r = Rect::from_center_size(rect.center(), Vec2::splat(side));
    let u = |x: f32, y: f32| -> Pos2 {
        pos2(
            box_r.left() + (x / 16.0) * side,
            box_r.top() + (y / 16.0) * side,
        )
    };
    let s = |v: f32| -> f32 { (v / 16.0) * side };
    let sw = (side * 0.105).clamp(1.25, 2.3);
    let st = Stroke::new(sw, color);
    let soft = color.gamma_multiply(0.24);
    let c = box_r.center();

    match icon {
        UiIcon::Globe => {
            let r = s(6.2);
            p.circle_filled(c, r, color.gamma_multiply(0.14));
            p.circle_stroke(c, r, st);
            p.line_segment([u(1.9, 8.0), u(14.1, 8.0)], st);
            p.line_segment([u(8.0, 1.8), u(8.0, 14.2)], st);
            p.line_segment([u(3.2, 4.7), u(12.8, 4.7)], Stroke::new(sw * 0.85, color));
            p.line_segment([u(3.2, 11.3), u(12.8, 11.3)], Stroke::new(sw * 0.85, color));
            let merid_l = vec![
                u(8.0, 1.8),
                u(5.2, 4.8),
                u(4.8, 8.0),
                u(5.2, 11.2),
                u(8.0, 14.2),
            ];
            let merid_r = vec![
                u(8.0, 1.8),
                u(10.8, 4.8),
                u(11.2, 8.0),
                u(10.8, 11.2),
                u(8.0, 14.2),
            ];
            p.add(Shape::line(merid_l, PathStroke::new(sw * 0.85, color)));
            p.add(Shape::line(merid_r, PathStroke::new(sw * 0.85, color)));
        }
        UiIcon::Folder => {
            // 顶部折耳标签 + 前面板填充，轮廓清晰
            let tab = vec![u(1.6, 3.0), u(6.2, 3.0), u(7.8, 5.0), u(1.6, 5.0)];
            p.add(Shape::convex_polygon(
                tab,
                color.gamma_multiply(0.75),
                Stroke::NONE,
            ));
            let body = Rect::from_min_max(u(1.6, 5.0), u(14.4, 13.0));
            p.rect_filled(body, 2.0, soft);
            p.rect_stroke(body, 2.0, st, StrokeKind::Inside);
            p.add(Shape::line(
                vec![u(1.6, 5.0), u(1.6, 3.0), u(6.2, 3.0), u(7.8, 5.0)],
                PathStroke::new(sw, color),
            ));
            p.line_segment([u(4.0, 7.6), u(12.0, 7.6)], Stroke::new(sw * 0.9, color));
        }
        UiIcon::Bolt => {
            p.add(Shape::convex_polygon(
                vec![u(9.6, 1.2), u(3.6, 9.0), u(8.6, 9.0)],
                color,
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                vec![u(7.2, 7.2), u(12.4, 7.2), u(6.4, 14.8)],
                color,
                Stroke::NONE,
            ));
        }
        UiIcon::Shield => {
            let pts = vec![
                u(8.0, 1.6),
                u(13.6, 3.8),
                u(12.9, 9.5),
                u(8.0, 14.4),
                u(3.1, 9.5),
                u(2.4, 3.8),
            ];
            p.add(Shape::convex_polygon(pts.clone(), soft, st));
            paint_checkmark(p, Rect::from_center_size(c, Vec2::splat(s(7.2))), sw, color);
        }
        UiIcon::CheckCircle => {
            let r = s(6.3);
            p.circle_filled(c, r, soft);
            p.circle_stroke(c, r, st);
            paint_checkmark(
                p,
                Rect::from_center_size(c, Vec2::splat(s(8.4))),
                sw * 1.1,
                color,
            );
        }
        UiIcon::Search => {
            let lc = u(6.6, 6.6);
            let lr = s(4.3);
            p.circle_filled(lc, lr, color.gamma_multiply(0.14));
            p.circle_stroke(lc, lr, st);
            p.line_segment([u(9.8, 9.8), u(13.8, 13.8)], Stroke::new(sw * 1.25, color));
        }
        UiIcon::Terminal => {
            let frame = Rect::from_min_max(u(1.6, 2.4), u(14.4, 13.6));
            p.rect_filled(frame, 2.5, soft);
            p.rect_stroke(frame, 2.5, st, StrokeKind::Inside);
            let bar = Rect::from_min_max(u(1.6, 2.4), u(14.4, 5.2));
            p.rect_filled(bar, 1.5, color.gamma_multiply(0.35));
            p.add(Shape::line(
                vec![u(4.4, 7.2), u(7.2, 9.4), u(4.4, 11.6)],
                PathStroke::new(sw, color),
            ));
            p.line_segment([u(8.2, 11.6), u(11.6, 11.6)], st);
        }
        UiIcon::User => {
            p.circle_filled(u(8.0, 4.8), s(2.7), color);
            p.add(Shape::convex_polygon(
                vec![u(2.8, 13.8), u(4.6, 9.2), u(11.4, 9.2), u(13.2, 13.8)],
                color,
                Stroke::NONE,
            ));
        }
        UiIcon::Gear => {
            p.circle_stroke(c, s(3.8), Stroke::new(sw * 1.2, color));
            p.circle_filled(c, s(1.6), color);
            for i in 0..8 {
                let a = (i as f32) * (TAU / 8.0);
                let p1 = pos2(c.x + a.cos() * s(4.3), c.y + a.sin() * s(4.3));
                let p2 = pos2(c.x + a.cos() * s(6.5), c.y + a.sin() * s(6.5));
                p.line_segment([p1, p2], Stroke::new(sw * 1.3, color));
            }
        }
        UiIcon::Chat => {
            let bub = Rect::from_min_max(u(1.8, 2.4), u(14.2, 11.2));
            p.rect_filled(bub, 3.0, soft);
            p.rect_stroke(bub, 3.0, st, StrokeKind::Inside);
            p.add(Shape::convex_polygon(
                vec![u(4.2, 11.0), u(3.6, 14.0), u(7.6, 11.0)],
                color,
                Stroke::NONE,
            ));
            p.line_segment([u(4.4, 5.6), u(11.6, 5.6)], Stroke::new(sw * 0.9, color));
            p.line_segment([u(4.4, 8.2), u(9.4, 8.2)], Stroke::new(sw * 0.9, color));
        }
        UiIcon::Collab => {
            p.circle_filled(u(5.6, 5.2), s(2.2), color.gamma_multiply(0.7));
            p.add(Shape::convex_polygon(
                vec![u(1.6, 13.0), u(2.8, 9.0), u(8.2, 9.0), u(9.0, 13.0)],
                color.gamma_multiply(0.65),
                Stroke::NONE,
            ));
            p.circle_filled(u(10.6, 5.0), s(2.4), color);
            p.add(Shape::convex_polygon(
                vec![u(6.4, 13.8), u(7.8, 9.4), u(13.2, 9.4), u(14.6, 13.8)],
                color,
                Stroke::NONE,
            ));
        }
        UiIcon::Trash => {
            p.line_segment([u(2.8, 4.4), u(13.2, 4.4)], st);
            p.add(Shape::line(
                vec![u(5.8, 4.4), u(5.8, 2.3), u(10.2, 2.3), u(10.2, 4.4)],
                PathStroke::new(sw, color),
            ));
            p.add(Shape::convex_polygon(
                vec![u(4.0, 4.6), u(12.0, 4.6), u(10.8, 13.6), u(5.2, 13.6)],
                soft,
                st,
            ));
            p.line_segment([u(6.7, 6.6), u(7.0, 11.6)], Stroke::new(sw * 0.85, color));
            p.line_segment([u(9.3, 6.6), u(9.0, 11.6)], Stroke::new(sw * 0.85, color));
        }
        UiIcon::Copy => {
            let back = Rect::from_min_max(u(2.4, 2.0), u(10.4, 11.2));
            p.rect_filled(back, 1.5, color.gamma_multiply(0.14));
            p.rect_stroke(back, 1.5, st, StrokeKind::Inside);
            let front = Rect::from_min_max(u(5.4, 4.8), u(13.6, 14.0));
            p.rect_filled(front, 1.5, soft);
            p.rect_stroke(front, 1.5, st, StrokeKind::Inside);
            p.line_segment([u(7.4, 8.0), u(11.6, 8.0)], Stroke::new(sw * 0.85, color));
            p.line_segment([u(7.4, 10.6), u(11.6, 10.6)], Stroke::new(sw * 0.85, color));
        }
        UiIcon::Insert => {
            p.line_segment([u(8.0, 2.0), u(8.0, 10.2)], st);
            p.add(Shape::line(
                vec![u(4.8, 7.2), u(8.0, 10.5), u(11.2, 7.2)],
                PathStroke::new(sw, color),
            ));
            p.add(Shape::line(
                vec![u(2.6, 10.6), u(2.6, 13.6), u(13.4, 13.6), u(13.4, 10.6)],
                PathStroke::new(sw, color),
            ));
        }
        UiIcon::Spark => {
            paint_four_point_star(p, c, s(6.5), s(2.1), color);
        }
        UiIcon::Cpu => {
            let body = Rect::from_min_max(u(3.8, 3.8), u(12.2, 12.2));
            p.rect_filled(body, 2.0, soft);
            p.rect_stroke(body, 2.0, st, StrokeKind::Inside);
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(s(3.4))), 1.0, color);
            for offset in [5.8_f32, 10.2_f32] {
                p.line_segment([u(offset, 1.6), u(offset, 3.8)], st);
                p.line_segment([u(offset, 12.2), u(offset, 14.4)], st);
                p.line_segment([u(1.6, offset), u(3.8, offset)], st);
                p.line_segment([u(12.2, offset), u(14.4, offset)], st);
            }
        }
        UiIcon::Plug => {
            p.line_segment([u(5.8, 2.0), u(5.8, 5.6)], st);
            p.line_segment([u(10.2, 2.0), u(10.2, 5.6)], st);
            let head = vec![u(3.8, 5.6), u(12.2, 5.6), u(10.6, 10.4), u(5.4, 10.4)];
            p.add(Shape::convex_polygon(head, color, Stroke::NONE));
            p.line_segment([u(8.0, 10.4), u(8.0, 14.2)], Stroke::new(sw * 1.15, color));
        }
        UiIcon::Lock => {
            let shackle = vec![
                u(5.2, 7.2),
                u(5.2, 4.4),
                u(6.4, 2.6),
                u(9.6, 2.6),
                u(10.8, 4.4),
                u(10.8, 7.2),
            ];
            p.add(Shape::line(shackle, PathStroke::new(sw, color)));
            let body = Rect::from_min_max(u(3.2, 7.2), u(12.8, 13.8));
            p.rect_filled(body, 2.2, color);
            p.circle_filled(u(8.0, 10.5), s(1.1), Color32::from_rgb(15, 23, 42));
        }
        UiIcon::Pin => {
            p.add(Shape::convex_polygon(
                vec![u(5.0, 2.4), u(11.0, 2.4), u(10.2, 7.8), u(5.8, 7.8)],
                color,
                Stroke::NONE,
            ));
            p.line_segment([u(3.8, 8.2), u(12.2, 8.2)], Stroke::new(sw * 1.15, color));
            p.line_segment([u(8.0, 8.2), u(8.0, 14.2)], Stroke::new(sw * 1.1, color));
        }
        UiIcon::Chart => {
            p.rect_filled(Rect::from_min_max(u(2.4, 8.6), u(5.0, 13.4)), 1.0, color);
            p.rect_filled(Rect::from_min_max(u(6.4, 5.4), u(9.0, 13.4)), 1.0, color);
            p.rect_filled(Rect::from_min_max(u(10.4, 2.6), u(13.0, 13.4)), 1.0, color);
            p.line_segment([u(1.8, 13.8), u(14.2, 13.8)], Stroke::new(sw * 0.9, color));
        }
        UiIcon::Wizard => {
            p.line_segment([u(2.6, 13.4), u(10.4, 5.6)], Stroke::new(sw * 1.2, color));
            paint_four_point_star(p, u(11.8, 4.2), s(3.0), s(1.0), color);
        }
        UiIcon::Star => {
            let mut pts = Vec::with_capacity(10);
            for i in 0..10 {
                let r = if i % 2 == 0 { s(6.6) } else { s(2.8) };
                let a = (i as f32) * (TAU / 10.0) - TAU * 0.25;
                pts.push(pos2(c.x + r * a.cos(), c.y + s(0.5) + r * a.sin()));
            }
            for i in 0..5 {
                let p_prev = pts[(i * 2 + 9) % 10];
                let p_tip = pts[i * 2];
                let p_next = pts[(i * 2 + 1) % 10];
                p.add(Shape::convex_polygon(
                    vec![c + vec2(0.0, s(0.5)), p_prev, p_tip, p_next],
                    color,
                    Stroke::NONE,
                ));
            }
        }
        UiIcon::Play => {
            p.add(Shape::convex_polygon(
                vec![u(4.2, 2.4), u(13.2, 8.0), u(4.2, 13.6)],
                color,
                Stroke::NONE,
            ));
        }
        UiIcon::ArrowRight => {
            p.line_segment([u(2.2, 8.0), u(12.8, 8.0)], Stroke::new(sw * 1.1, color));
            p.add(Shape::line(
                vec![u(8.8, 3.8), u(13.2, 8.0), u(8.8, 12.2)],
                PathStroke::new(sw * 1.1, color),
            ));
        }
        UiIcon::Info => {
            p.circle_filled(c, s(6.2), soft);
            p.circle_stroke(c, s(6.2), st);
            p.circle_filled(u(8.0, 4.8), s(1.05), color);
            p.line_segment([u(8.0, 7.2), u(8.0, 11.8)], Stroke::new(sw * 1.1, color));
        }
        UiIcon::Warning => {
            let tri = vec![u(8.0, 2.0), u(14.2, 13.4), u(1.8, 13.4)];
            p.add(Shape::convex_polygon(tri, soft, st));
            p.line_segment([u(8.0, 5.8), u(8.0, 9.6)], Stroke::new(sw * 1.1, color));
            p.circle_filled(u(8.0, 11.4), s(0.95), color);
        }
        UiIcon::Error => {
            p.circle_filled(c, s(6.2), soft);
            p.circle_stroke(c, s(6.2), st);
            p.line_segment([u(5.2, 5.2), u(10.8, 10.8)], st);
            p.line_segment([u(10.8, 5.2), u(5.2, 10.8)], st);
        }
        UiIcon::Quote => {
            p.circle_filled(u(5.2, 6.2), s(2.0), color);
            p.add(Shape::convex_polygon(
                vec![u(3.8, 6.6), u(7.0, 6.6), u(4.2, 11.6)],
                color,
                Stroke::NONE,
            ));
            p.circle_filled(u(10.8, 6.2), s(2.0), color);
            p.add(Shape::convex_polygon(
                vec![u(9.4, 6.6), u(12.6, 6.6), u(9.8, 11.6)],
                color,
                Stroke::NONE,
            ));
        }
        UiIcon::Note => {
            let pts = vec![
                u(2.6, 1.8),
                u(10.0, 1.8),
                u(13.4, 5.2),
                u(13.4, 14.2),
                u(2.6, 14.2),
            ];
            p.add(Shape::convex_polygon(pts, soft, st));
            p.add(Shape::line(
                vec![u(10.0, 1.8), u(10.0, 5.2), u(13.4, 5.2)],
                PathStroke::new(sw * 0.9, color),
            ));
            p.line_segment([u(4.8, 8.0), u(11.2, 8.0)], Stroke::new(sw * 0.9, color));
            p.line_segment([u(4.8, 10.8), u(9.6, 10.8)], Stroke::new(sw * 0.9, color));
        }
        UiIcon::Wrench => {
            p.line_segment([u(3.2, 12.8), u(9.6, 6.4)], Stroke::new(sw * 1.35, color));
            p.circle_filled(u(3.8, 12.2), s(1.1), color);
            p.add(Shape::line(
                vec![
                    u(12.8, 2.4),
                    u(10.2, 2.2),
                    u(8.4, 4.0),
                    u(8.6, 6.6),
                    u(11.2, 7.6),
                    u(13.6, 5.8),
                ],
                PathStroke::new(sw * 1.15, color),
            ));
        }
    }
}

/// 绘制左图标 + 右标签的紧凑水平组合
pub fn icon_label(
    ui: &mut Ui,
    icon: UiIcon,
    text: impl Into<WidgetText>,
    icon_col: Color32,
) -> Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        paint(ui.painter(), ir, icon, icon_col);
        ui.label(text)
    })
    .response
}

/// 绘制带矢量图标的原生自适应按钮 (带触感微高光与平滑响应)
pub fn icon_button(ui: &mut Ui, icon: UiIcon, text: &str, icon_col: Color32) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text_col = ui.visuals().widgets.inactive.fg_stroke.color;
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, text_col);
    let pad = vec2(8.0, 4.0);
    let icon_sz = 13.0;
    let gap = 5.0;
    let total = vec2(
        pad.x * 2.0 + icon_sz + gap + galley.size().x,
        (galley.size().y.max(icon_sz) + pad.y * 2.0).max(22.0),
    );
    let (rect, resp) = ui.allocate_exact_size(total, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let vis = ui.style().interact(&resp);
        let dark = ui.visuals().dark_mode;
        let p = ui.painter();
        let pressed = resp.is_pointer_button_down_on();
        let hovered = resp.hovered();

        let (fill, stroke) = if pressed {
            (
                if dark {
                    Color32::from_white_alpha(32)
                } else {
                    Color32::from_black_alpha(28)
                },
                Stroke::new(
                    1.0,
                    if dark {
                        Color32::from_white_alpha(50)
                    } else {
                        Color32::from_black_alpha(45)
                    },
                ),
            )
        } else if hovered {
            (
                if dark {
                    Color32::from_white_alpha(22)
                } else {
                    Color32::from_black_alpha(16)
                },
                Stroke::new(
                    1.0,
                    if dark {
                        Color32::from_white_alpha(42)
                    } else {
                        Color32::from_black_alpha(38)
                    },
                ),
            )
        } else {
            (
                if dark {
                    Color32::from_white_alpha(8)
                } else {
                    Color32::from_black_alpha(6)
                },
                Stroke::new(
                    1.0,
                    if dark {
                        Color32::from_white_alpha(18)
                    } else {
                        Color32::from_black_alpha(20)
                    },
                ),
            )
        };

        let radius = 5.0;
        p.rect_filled(rect, radius, fill);
        p.rect_stroke(rect, radius, stroke, StrokeKind::Inside);

        // 顶部微高光反光 (Top Micro Specular Glint)
        if rect.width() > 14.0 && !pressed {
            let glint_alpha = if dark { 40 } else { 85 };
            p.line_segment(
                [
                    pos2(rect.left() + 3.0, rect.top() + 1.0),
                    pos2(rect.right() - 3.0, rect.top() + 1.0),
                ],
                Stroke::new(1.0, Color32::from_white_alpha(glint_alpha)),
            );
        }

        let ir = Rect::from_min_size(
            pos2(rect.left() + pad.x, rect.center().y - icon_sz * 0.5),
            Vec2::splat(icon_sz),
        );
        paint(p, ir, icon, if hovered { Color32::WHITE } else { icon_col });
        p.galley(
            pos2(ir.right() + gap, rect.center().y - galley.size().y * 0.5),
            galley,
            vis.text_color(),
        );
    }
    resp
}

/// 绘制仅包含矢量图标的紧凑方/圆角按钮 (如设置齿轮，带微高光与平滑响应)
pub fn icon_only_button(ui: &mut Ui, icon: UiIcon, size: Vec2, icon_col: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let vis = ui.style().interact(&resp);
        let dark = ui.visuals().dark_mode;
        let p = ui.painter();
        let pressed = resp.is_pointer_button_down_on();
        let hovered = resp.hovered();

        let (fill, stroke) = if pressed {
            (
                if dark {
                    Color32::from_white_alpha(35)
                } else {
                    Color32::from_black_alpha(30)
                },
                Stroke::new(
                    1.0,
                    if dark {
                        Color32::from_white_alpha(55)
                    } else {
                        Color32::from_black_alpha(48)
                    },
                ),
            )
        } else if hovered {
            (
                if dark {
                    Color32::from_white_alpha(24)
                } else {
                    Color32::from_black_alpha(18)
                },
                Stroke::new(
                    1.0,
                    if dark {
                        Color32::from_white_alpha(45)
                    } else {
                        Color32::from_black_alpha(40)
                    },
                ),
            )
        } else {
            (Color32::TRANSPARENT, Stroke::NONE)
        };

        let radius = 4.0;
        if fill != Color32::TRANSPARENT {
            p.rect_filled(rect, radius, fill);
        }
        if stroke != Stroke::NONE {
            p.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
        }

        let col = if hovered { vis.text_color() } else { icon_col };
        paint(p, rect.shrink(3.5), icon, col);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ui_icons_paint_non_empty_shapes() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let p = ui.painter();
            for (idx, &icon) in UiIcon::ALL.iter().enumerate() {
                let r = Rect::from_min_size(pos2((idx as f32) * 20.0, 0.0), vec2(16.0, 16.0));
                paint(p, r, icon, Color32::from_rgb(56, 189, 248));
            }
            paint_checkmark(
                p,
                Rect::from_min_size(pos2(0.0, 24.0), vec2(14.0, 14.0)),
                2.0,
                Color32::WHITE,
            );
            let _ = icon_label(ui, UiIcon::Globe, "Language", Color32::WHITE);
            let _ = icon_button(ui, UiIcon::Terminal, "Terminal", Color32::WHITE);
            let _ = icon_only_button(ui, UiIcon::Gear, vec2(24.0, 24.0), Color32::WHITE);
        });
        assert!(
            !out.shapes.is_empty(),
            "All pure-vector UI icons must generate visible geometry shapes"
        );
        out.textures_delta.clear();
    }
}
