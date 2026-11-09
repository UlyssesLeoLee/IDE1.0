//! 水晶透明 (Crystal) 与 透明玻璃 (Clear Glass) 材质渲染器
//! 对齐 Tauri 版 index.html 的光学焦散 (Caustics)、棱镜色散 (Prism Dispersion)、
//! 晶面切面高光 (Facet Specular Borders) 与字符级高对比光晕衬底 (Character-Level Contrast Halo)。

use crate::view::ThemeColors;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke};

/// 绘制水晶 / 玻璃背后的光学焦散层与晶面微光
pub fn paint_caustic_backdrop(p: &Painter, rect: Rect, tc: &ThemeColors, time: f64) {
    if !tc.is_translucent() {
        return;
    }
    let (w, h) = (rect.width(), rect.height());
    if w <= 10.0 || h <= 10.0 {
        return;
    }

    if tc.crystal {
        // --- 水晶主题: 纯净光学晶体折射弧光 + 棱镜色散 + 微细晶芒 ---
        // 1. 左上象限: 棱镜青色大跨度焦散光晕 (15% 10%, 95% 60%)
        let cyan_center = Pos2::new(rect.left() + w * 0.15, rect.top() + h * 0.10);
        let cyan_radius = (w * 0.45).min(h * 0.55);
        paint_radial_bloom(p, cyan_center, cyan_radius, Color32::from_rgba_unmultiplied(0, 240, 255, 18));

        // 2. 右下象限: 棱镜粉色大跨度焦散光晕 (85% 90%, 80% 65%)
        let pink_center = Pos2::new(rect.left() + w * 0.85, rect.top() + h * 0.88);
        let pink_radius = (w * 0.42).min(h * 0.50);
        paint_radial_bloom(p, pink_center, pink_radius, Color32::from_rgba_unmultiplied(255, 105, 180, 15));

        // 3. 晶面大跨度微透反光带 (135° 对角线柔和反光)
        let sheen_color = Color32::from_rgba_unmultiplied(255, 255, 255, 10);
        p.line_segment(
            [
                Pos2::new(rect.left() + w * 0.05, rect.top() + h * 0.25),
                Pos2::new(rect.left() + w * 0.75, rect.top() + h * 0.95),
            ],
            Stroke::new(2.0, sheen_color),
        );

        // 4. 水晶切割星芒微光 (Tauri crystal-shimmer 粒子闪烁)
        let sparkles = [
            (0.20, 0.25, 0.0),
            (0.78, 0.18, 2.1),
            (0.65, 0.82, 4.3),
        ];
        for &(px, py, phase) in &sparkles {
            let cx = rect.left() + w * px;
            let cy = rect.top() + h * py;
            let pulse = ((time * 0.85 + phase).sin() * 0.5 + 0.5) as f32; // 0.0 .. 1.0
            if pulse > 0.08 {
                let alpha = (pulse * 140.0) as u8;
                let cyan_alpha = (pulse * 90.0) as u8;
                let halo_c = Color32::from_rgba_unmultiplied(0, 240, 255, cyan_alpha);
                let core_c = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                p.circle_filled(Pos2::new(cx, cy), 3.0, halo_c);
                p.circle_filled(Pos2::new(cx, cy), 1.2, core_c);
            }
        }
    } else if tc.clear {
        // --- 透明玻璃主题: 亚克力天蓝色散 + 紫罗兰漫反射 ---
        // 1. 顶部居中: 蔚蓝光照层 (50% -12%)
        let sky_center = Pos2::new(rect.center().x, rect.top());
        let sky_radius = (w * 0.55).min(h * 0.45);
        paint_radial_bloom(p, sky_center, sky_radius, Color32::from_rgba_unmultiplied(56, 189, 248, 24));

        // 2. 右下角: 紫罗兰柔光 (100% 100%)
        let purple_center = Pos2::new(rect.right(), rect.bottom());
        let purple_radius = (w * 0.45).min(h * 0.40);
        paint_radial_bloom(p, purple_center, purple_radius, Color32::from_rgba_unmultiplied(168, 85, 247, 18));

        // 3. 玻璃材质通透斜光
        let glass_sheen = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
        p.line_segment(
            [
                Pos2::new(rect.left(), rect.top() + h * 0.15),
                Pos2::new(rect.left() + w * 0.65, rect.top() + h * 0.80),
            ],
            Stroke::new(1.5, glass_sheen),
        );
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
        let a = ((color.a() as f32) * (1.0 - factor * 0.65) / (steps as f32 * 0.7)).round().min(255.0) as u8;
        p.circle_filled(
            center,
            r,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a),
        );
    }
}

/// 绘制晶面切面边框与双内阴影折射高光 (对齐 Tauri 的 `--glass-specular` 与 facet border)
pub fn paint_facet_borders(p: &Painter, rect: Rect, tc: &ThemeColors) {
    if !tc.is_translucent() {
        p.rect_stroke(rect, 0.0, Stroke::new(1.0, tc.border), egui::StrokeKind::Inside);
        return;
    }

    if tc.crystal {
        // 水晶切面高光:
        // - 顶边: 1.5px 纯净切面强反射 (rgba(255, 255, 255, 0.68))
        // - 左边: 1.0px 侧切面次高光 (rgba(255, 255, 255, 0.48))
        // - 底边: 1.0px 棱镜青折射边缘 (rgba(0, 240, 255, 0.35))
        // - 右边: 1.0px 棱镜粉折射边缘 (rgba(255, 105, 180, 0.30))
        let top_c = Color32::from_rgba_unmultiplied(255, 255, 255, 175);
        let left_c = Color32::from_rgba_unmultiplied(255, 255, 255, 125);
        let bottom_c = Color32::from_rgba_unmultiplied(0, 240, 255, 90);
        let right_c = Color32::from_rgba_unmultiplied(255, 105, 180, 75);

        p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, top_c));
        p.vline(rect.left() + 0.5, rect.y_range(), Stroke::new(1.0, left_c));
        p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, bottom_c));
        p.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, right_c));
    } else if tc.clear {
        // 经典玻璃微反光:
        // - 顶边: 1.5px 镜面反光 (rgba(255, 255, 255, 0.85))
        // - 左边: 1.0px 棱镜淡青色散 (rgba(56, 189, 248, 0.38))
        // - 底边: 1.0px 玻璃幽暗收口 (rgba(255, 255, 255, 0.22))
        // - 右边: 1.0px 棱镜淡粉色散 (rgba(216, 180, 254, 0.32))
        let top_c = Color32::from_rgba_unmultiplied(255, 255, 255, 195);
        let left_c = Color32::from_rgba_unmultiplied(56, 189, 248, 95);
        let bottom_c = Color32::from_rgba_unmultiplied(255, 255, 255, 55);
        let right_c = Color32::from_rgba_unmultiplied(216, 180, 254, 80);

        p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, top_c));
        p.vline(rect.left() + 0.5, rect.y_range(), Stroke::new(1.0, left_c));
        p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, bottom_c));
        p.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, right_c));
    }
}

/// 字符级通透光学折射光晕 (取代粗暴的大黑底涂抹，保持通透见底)
pub fn paint_text_halo(
    p: &Painter,
    pos: Pos2,
    galley: std::sync::Arc<egui::Galley>,
    tc: &ThemeColors,
) {
    if !tc.is_translucent() {
        return;
    }
    // 水晶主题: 纯净折射色散微辉光，字形晶莹剔透，严禁黑块涂抹
    if tc.crystal {
        let prism_glow = Color32::from_rgba_unmultiplied(0, 240, 255, 30);
        let pink_glow = Color32::from_rgba_unmultiplied(255, 105, 180, 20);
        p.galley(Pos2::new(pos.x - 0.75, pos.y), galley.clone(), prism_glow);
        p.galley(Pos2::new(pos.x + 0.75, pos.y), galley.clone(), pink_glow);
    } else if tc.clear {
        // 玻璃主题: 极细微高透环境阴影 (仅 14% 极低透明度)，柔和托底而不遮挡后方桌面
        let soft_subtle = if tc.dark {
            Color32::from_rgba_unmultiplied(0, 0, 0, 36)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 45)
        };
        p.galley(Pos2::new(pos.x + 0.5, pos.y + 0.5), galley, soft_subtle);
    }
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
            p.rect_stroke(glow_rect, round + 1.5, Stroke::new(1.0, g), egui::StrokeKind::Middle);
        }
    }
    p.rect_filled(rect, round, fill);
    p.rect_stroke(rect, round, Stroke::new(1.0, border), egui::StrokeKind::Inside);
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
