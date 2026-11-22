//! 水晶透明 (Crystal) 与 透明玻璃 (Clear Glass) 材质渲染器
//! 对齐 Tauri 版 index.html 的光学焦散 (Caustics)、棱镜色散 (Prism Dispersion)、
//! 晶面切面高光 (Facet Specular Borders) 与字符级高对比光晕衬底 (Character-Level Contrast Halo)。

use crate::view::ThemeColors;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke};

/// 绘制次世代水晶透明界面的光学焦散层、双曲色散光晕与晶面微光
pub fn paint_caustic_backdrop(p: &Painter, rect: Rect, tc: &ThemeColors, time: f64) {
    if !tc.is_translucent() {
        return;
    }
    let (w, h) = (rect.width(), rect.height());
    if w <= 10.0 || h <= 10.0 {
        return;
    }

    // 1. 防眩光消光微透吸收底衬 (融合 Clear 玻璃的抗过曝托底): 消除高亮壁纸刺眼，保证代码极度清晰
    let body_base_tint = if tc.dark {
        Color32::from_rgba_unmultiplied(12, 16, 26, 88)
    } else {
        Color32::from_rgba_unmultiplied(240, 244, 252, 98)
    };
    p.rect_filled(rect, 0.0, body_base_tint);

    // 2. 左上象限: 棱镜青色大跨度焦散光晕 (融合 Clear 天蓝与 Crystal 纯青色散)
    let cyan_center = Pos2::new(rect.left() + w * 0.15, rect.top() + h * 0.10);
    let cyan_radius = (w * 0.48).min(h * 0.55);
    paint_radial_bloom(
        p,
        cyan_center,
        cyan_radius,
        Color32::from_rgba_unmultiplied(0, 240, 255, 22),
    );

    // 3. 右下象限: 棱镜粉红/紫罗兰大跨度焦散光晕 (融合 Clear 紫罗兰与 Crystal 洋红色散)
    let pink_center = Pos2::new(rect.left() + w * 0.85, rect.top() + h * 0.88);
    let pink_radius = (w * 0.45).min(h * 0.50);
    paint_radial_bloom(
        p,
        pink_center,
        pink_radius,
        Color32::from_rgba_unmultiplied(255, 105, 180, 18),
    );

    // 4. 晶面大跨度微透反光带 (135° 对角线柔和反光)
    let sheen_color = Color32::from_rgba_unmultiplied(255, 255, 255, 12);
    p.line_segment(
        [
            Pos2::new(rect.left() + w * 0.05, rect.top() + h * 0.20),
            Pos2::new(rect.left() + w * 0.75, rect.top() + h * 0.90),
        ],
        Stroke::new(2.0, sheen_color),
    );

    // 5. 水晶微晶星芒闪烁
    paint_crystal_sparkles(p, rect, w, h, time);
}

fn paint_crystal_sparkles(p: &Painter, rect: Rect, w: f32, h: f32, time: f64) {
    let sparkles = [(0.20, 0.25, 0.0), (0.78, 0.18, 2.1), (0.65, 0.82, 4.3)];
    for &(px, py, phase) in &sparkles {
        let cx = rect.left() + w * px;
        let cy = rect.top() + h * py;
        let pulse = ((time * 0.85 + phase).sin() * 0.5 + 0.5) as f32;
        if pulse > 0.08 {
            let alpha = (pulse * 140.0) as u8;
            let cyan_alpha = (pulse * 90.0) as u8;
            let halo_c = Color32::from_rgba_unmultiplied(0, 240, 255, cyan_alpha);
            let core_c = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
            p.circle_filled(Pos2::new(cx, cy), 3.0, halo_c);
            p.circle_filled(Pos2::new(cx, cy), 1.2, core_c);
        }
    }
}

/// 柔和多圈径向微光晕 (用于模拟 CSS radial-gradient)
fn paint_radial_bloom(p: &Painter, center: Pos2, max_radius: f32, color: Color32) {
    if max_radius <= 2.0 || color.a() == 0 {
        return;
    }
    let steps = 4;
    for i in (1..=steps).rev() {
        let factor = i as f32 / steps as f32;
        let r = max_radius * factor;
        // 外圈 alpha 衰减
        let a = ((color.a() as f32) * (1.0 - factor * 0.65) / (steps as f32 * 0.7))
            .round()
            .min(255.0) as u8;
        p.circle_filled(
            center,
            r,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a),
        );
    }
}

/// 绘制次世代晶面切面边框与双内阴影折射高光
pub fn paint_facet_borders(p: &Painter, rect: Rect, tc: &ThemeColors) {
    if !tc.is_translucent() {
        p.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, tc.border),
            egui::StrokeKind::Inside,
        );
        return;
    }

    // 次世代水晶玻璃切面高光:
    // - 顶边: 1.5px 纯净切面强反射 (rgba(255, 255, 255, 0.75))
    // - 左边: 1.0px 侧切面次高光 (rgba(255, 255, 255, 0.52))
    // - 底边: 1.0px 棱镜青折射边缘 (rgba(0, 240, 255, 0.40))
    // - 右边: 1.0px 棱镜粉折射边缘 (rgba(255, 105, 180, 0.35))
    let top_c = Color32::from_rgba_unmultiplied(255, 255, 255, 190);
    let left_c = Color32::from_rgba_unmultiplied(255, 255, 255, 132);
    let bottom_c = Color32::from_rgba_unmultiplied(0, 240, 255, 102);
    let right_c = Color32::from_rgba_unmultiplied(255, 105, 180, 88);

    p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, top_c));
    p.vline(rect.left() + 0.5, rect.y_range(), Stroke::new(1.0, left_c));
    p.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, bottom_c),
    );
    p.vline(
        rect.right() - 0.5,
        rect.y_range(),
        Stroke::new(1.0, right_c),
    );
}

/// 字符级通透光学折射光晕与防眩光对比度托底 (融合玻璃护眼托底与水晶棱镜双色辉光)
pub fn paint_text_halo(
    p: &Painter,
    pos: Pos2,
    galley: std::sync::Arc<egui::Galley>,
    tc: &ThemeColors,
) {
    if !tc.is_translucent() {
        return;
    }
    // 1. 来自玻璃的柔和护眼对比度托底 (消除高噪或高亮壁纸下的文字发虚)
    let contrast_shadow = if tc.dark {
        Color32::from_rgba_unmultiplied(0, 0, 0, 48)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 60)
    };
    p.galley(
        Pos2::new(pos.x + 0.5, pos.y + 0.5),
        galley.clone(),
        contrast_shadow,
    );

    // 2. 来自水晶的纯净棱镜双色辉光 (左青微偏 -0.75px，右粉微偏 +0.75px，形成晶莹微色散)
    let prism_glow = Color32::from_rgba_unmultiplied(0, 240, 255, 32);
    let pink_glow = Color32::from_rgba_unmultiplied(255, 105, 180, 22);
    p.galley(Pos2::new(pos.x - 0.75, pos.y), galley.clone(), prism_glow);
    p.galley(Pos2::new(pos.x + 0.75, pos.y), galley, pink_glow);
}

/// 发光水晶胶囊药丸 (用于状态栏模式、标签页选中项、标题栏按钮)
pub fn paint_glowing_pill(
    p: &Painter,
    rect: Rect,
    round: f32,
    fill: Color32,
    border: Color32,
    glow: Option<Color32>,
) {
    if let Some(g) = glow {
        if g.a() > 0 {
            // 外发光环
            let glow_rect = rect.expand(2.0);
            p.rect_stroke(
                glow_rect,
                round + 1.5,
                Stroke::new(1.0, g),
                egui::StrokeKind::Middle,
            );
        }
    }
    p.rect_filled(rect, round, fill);
    p.rect_stroke(
        rect,
        round,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    // 顶部晶钻高光微线 (1px)
    if rect.width() > 14.0 {
        p.line_segment(
            [
                Pos2::new(rect.left() + 4.0, rect.top() + 1.0),
                Pos2::new(rect.right() - 4.0, rect.top() + 1.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(120)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Vec2;

    #[test]
    fn test_glass_render_does_not_panic() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
            let mut tc = ThemeColors::builtin(true);
            tc.crystal = true;
            paint_caustic_backdrop(ui.painter(), rect, &tc, 1.23);
            paint_facet_borders(ui.painter(), rect, &tc);

            let pill_rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(40.0, 20.0));
            paint_glowing_pill(
                ui.painter(),
                pill_rect,
                4.0,
                Color32::from_white_alpha(30),
                Color32::from_white_alpha(150),
                Some(Color32::from_rgba_unmultiplied(0, 240, 255, 90)),
            );

            tc.crystal = false;
            tc.clear = true;
            paint_caustic_backdrop(ui.painter(), rect, &tc, 2.34);
            paint_facet_borders(ui.painter(), rect, &tc);
        });
        out.textures_delta.clear();
    }
}
