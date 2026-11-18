//! 经典美漫与赛璐珞风格安装向导的绘图图元与交互组件 (Comic Art Primitives & UI Controls)

use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};
use std::f32::consts::PI;

use super::InstallerStep;

/// 绘制向导步骤胶囊指示条 (Step Indicator Capsules)
pub fn paint_step_indicator(p: &egui::Painter, rect: Rect, current: InstallerStep) {
    let steps = [
        (InstallerStep::Welcome, "1. 欢迎"),
        (InstallerStep::Options, "2. 配置"),
        (InstallerStep::Installing, "3. 部署"),
        (InstallerStep::Finished, "4. 完成"),
    ];

    let n = steps.len();
    let step_w = (rect.width() - (n as f32 - 1.0) * 16.0) / n as f32;
    let mut x = rect.left();

    for (i, (step, label)) in steps.into_iter().enumerate() {
        let cap_rect = Rect::from_min_size(pos2(x, rect.top()), vec2(step_w, rect.height()));
        let is_curr = step == current;
        let is_past = (step as usize) < (current as usize);

        let (fill_col, text_col) = if is_curr {
            (Color32::from_rgb(2, 132, 199), Color32::WHITE)
        } else if is_past {
            (Color32::from_rgb(16, 185, 129), Color32::WHITE)
        } else {
            (
                Color32::from_rgb(22, 28, 40),
                Color32::from_rgb(148, 163, 184),
            )
        };

        paint_comic_hard_shadow_rect(p, cap_rect, fill_col, 1.8, vec2(1.5, 1.5));
        p.text(
            cap_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.5),
            text_col,
        );

        if i < n - 1 {
            let line_y = rect.center().y;
            p.line_segment(
                [
                    pos2(cap_rect.right() + 2.0, line_y),
                    pos2(cap_rect.right() + 14.0, line_y),
                ],
                Stroke::new(2.0, Color32::from_rgb(30, 41, 59)),
            );
        }

        x += step_w + 16.0;
    }
}

/// 绘制美漫纯黑硬阴影矩形卡片
pub fn paint_comic_hard_shadow_rect(
    p: &egui::Painter,
    rect: Rect,
    fill_col: Color32,
    border_w: f32,
    shadow_offset: Vec2,
) {
    if shadow_offset.x > 0.0 || shadow_offset.y > 0.0 {
        p.rect_filled(rect.translate(shadow_offset), 0.0, Color32::BLACK);
    }
    p.rect_filled(rect, 0.0, fill_col);
    p.rect_stroke(
        rect,
        0.0,
        Stroke::new(border_w, Color32::BLACK),
        StrokeKind::Inside,
    );
}

/// 绘制波普半色调网点 (Ben-Day Screentone Dots)
pub fn paint_ben_day_dots(
    p: &egui::Painter,
    rect: Rect,
    dot_col: Color32,
    spacing: f32,
    radius: f32,
) {
    let mut y = rect.top();
    let mut row = 0;
    while y < rect.bottom() {
        let x_offset = if row % 2 == 1 { spacing * 0.5 } else { 0.0 };
        let mut x = rect.left() + x_offset;
        while x < rect.right() {
            p.circle_filled(pos2(x, y), radius, dot_col);
            x += spacing;
        }
        y += spacing;
        row += 1;
    }
}

/// 绘制美漫动作爆发星贴纸 (Spiky Action Burst Star)
pub fn paint_comic_burst_star(
    p: &egui::Painter,
    center: egui::Pos2,
    r_outer: f32,
    r_inner: f32,
    points: usize,
    fill_col: Color32,
    shadow_offset: Vec2,
) {
    let angle_step = PI / (points as f32);
    let mut pts = Vec::with_capacity(points * 2);
    for i in 0..(points * 2) {
        let r = if i % 2 == 0 { r_outer } else { r_inner };
        let a = (i as f32) * angle_step - PI * 0.5;
        pts.push(pos2(center.x + r * a.cos(), center.y + r * a.sin()));
    }

    if shadow_offset.x > 0.0 || shadow_offset.y > 0.0 {
        let shadow_pts = pts.iter().map(|pt| *pt + shadow_offset).collect::<Vec<_>>();
        p.add(egui::Shape::convex_polygon(
            shadow_pts,
            Color32::BLACK,
            Stroke::NONE,
        ));
    }

    p.add(egui::Shape::convex_polygon(
        pts,
        fill_col,
        Stroke::new(2.0, Color32::BLACK),
    ));
}

/// 绘制真实正版原生 Logo (严格保留用户正版 Logo，不改动，带美漫纯黑实心阴影)
pub fn paint_authentic_logo(
    p: &egui::Painter,
    logo_tex: Option<&egui::TextureHandle>,
    center: egui::Pos2,
    size: f32,
    shadow_offset: Vec2,
) {
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    if shadow_offset.x > 0.0 || shadow_offset.y > 0.0 {
        let s_rect = rect.translate(shadow_offset);
        p.rect_filled(s_rect, 4.0, Color32::from_black_alpha(200));
    }

    if let Some(tex) = logo_tex {
        p.image(
            tex.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        // 解码异常兜底
        p.rect_filled(rect, 4.0, Color32::from_rgb(0, 229, 255));
        p.text(
            center,
            egui::Align2::CENTER_CENTER,
            "IDE",
            egui::FontId::proportional(size * 0.4),
            Color32::BLACK,
        );
    }
}

/// 绘制美漫爱奥尼古典柱台 (Ionic Comic Pedestal)
pub fn paint_ionic_comic_pedestal(p: &egui::Painter, cx: f32, top_y: f32) {
    // 1. 柱头 Capital
    let cap_rect = Rect::from_center_size(pos2(cx, top_y + 8.0), vec2(90.0, 16.0));
    paint_comic_hard_shadow_rect(
        p,
        cap_rect,
        Color32::from_rgb(240, 240, 245),
        2.0,
        vec2(0.0, 0.0),
    );
    p.circle_stroke(
        pos2(cap_rect.left() + 10.0, cap_rect.center().y),
        6.0,
        Stroke::new(1.8, Color32::BLACK),
    );
    p.circle_stroke(
        pos2(cap_rect.right() - 10.0, cap_rect.center().y),
        6.0,
        Stroke::new(1.8, Color32::BLACK),
    );

    // 2. 柱身 Shaft
    let shaft_rect = Rect::from_center_size(pos2(cx, top_y + 48.0), vec2(72.0, 64.0));
    paint_comic_hard_shadow_rect(
        p,
        shaft_rect,
        Color32::from_rgb(220, 225, 235),
        2.0,
        vec2(0.0, 0.0),
    );
    for fx in [-24.0, -12.0, 0.0, 12.0, 24.0] {
        let x = cx + fx;
        p.line_segment(
            [
                pos2(x, shaft_rect.top() + 2.0),
                pos2(x, shaft_rect.bottom() - 2.0),
            ],
            Stroke::new(3.0, Color32::from_rgb(170, 180, 200)),
        );
        p.line_segment(
            [
                pos2(x - 1.0, shaft_rect.top() + 2.0),
                pos2(x - 1.0, shaft_rect.bottom() - 2.0),
            ],
            Stroke::new(1.2, Color32::BLACK),
        );
    }

    // 3. 柱础 Base
    let base_rect = Rect::from_center_size(pos2(cx, top_y + 88.0), vec2(96.0, 16.0));
    paint_comic_hard_shadow_rect(
        p,
        base_rect,
        Color32::from_rgb(245, 245, 250),
        2.0,
        vec2(0.0, 0.0),
    );
    let gold_band = Rect::from_center_size(pos2(cx, top_y + 88.0), vec2(88.0, 6.0));
    p.rect_filled(gold_band, 0.0, Color32::from_rgb(255, 204, 0));
    p.rect_stroke(
        gold_band,
        0.0,
        Stroke::new(1.0, Color32::BLACK),
        StrokeKind::Inside,
    );
}

/// 纯自绘美漫窗口控制按钮 (提取公用逻辑，消除重复代码)
pub fn paint_comic_window_btn(
    ui: &mut egui::Ui,
    center: egui::Pos2,
    id_str: &str,
    is_close: bool,
) -> bool {
    let rect = Rect::from_center_size(center, vec2(24.0, 24.0));
    let resp = ui.interact(rect, ui.id().with(id_str), egui::Sense::click());
    let is_down = resp.is_pointer_button_down_on();
    let draw_rect = if is_down {
        rect.translate(vec2(1.5, 1.5))
    } else {
        rect
    };
    let bg_color = if is_close {
        if resp.hovered() {
            Color32::from_rgb(255, 60, 60)
        } else {
            Color32::from_rgb(220, 38, 38)
        }
    } else if resp.hovered() {
        Color32::from_rgb(255, 255, 255)
    } else {
        Color32::from_rgb(230, 235, 245)
    };
    let shadow = if is_down {
        vec2(1.0, 1.0)
    } else {
        vec2(2.0, 2.0)
    };
    let p = ui.painter();
    paint_comic_hard_shadow_rect(p, draw_rect, bg_color, 2.0, shadow);

    let cc = draw_rect.center();
    if is_close {
        p.line_segment(
            [pos2(cc.x - 4.5, cc.y - 4.5), pos2(cc.x + 4.5, cc.y + 4.5)],
            Stroke::new(2.5, Color32::WHITE),
        );
        p.line_segment(
            [pos2(cc.x + 4.5, cc.y - 4.5), pos2(cc.x - 4.5, cc.y + 4.5)],
            Stroke::new(2.5, Color32::WHITE),
        );
    } else {
        p.line_segment(
            [pos2(cc.x - 5.0, cc.y), pos2(cc.x + 5.0, cc.y)],
            Stroke::new(2.5, Color32::BLACK),
        );
    }
    resp.clicked()
}
