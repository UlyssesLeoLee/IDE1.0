//! 真 3D 赛璐珞华丽特效引擎 (3D Cel-Shaded Visual Effects)
//! 包含：3D 粒子流涡旋 (Particle Vortex)、双环能量轨道 (Energy Aura Rings)、
//! 放射状美漫光芒 (Radial Comic Rays)、八芒星光高光 (Diamond Glints)、
//! 超空间曲速流线 (Warp Streaks) 与胜利彩屑礼花 (Celebration Confetti)。

use super::cel3d::{Camera3D, Mat4, Vec3};
use super::InstallerStep;
use eframe::egui;
use egui::{pos2, Color32, Painter, Pos2, Stroke};
use std::f32::consts::PI;

/// 绘制低调奢华暗场氛围光晕 (Subtle Ambient Luxury Glow)
/// 柔和无噪点的天青背景辉光，降低视觉疲劳与认知负荷，重点凸显中央 3D 晶石
pub fn draw_ambient_luxury_glow(p: &Painter, center: Pos2, time: f32) {
    let pulse = (time * 1.2).sin() * 0.15;
    let r1 = 130.0 + pulse * 10.0;
    let r2 = 75.0 + pulse * 6.0;

    // 外圈柔和天青微光
    p.circle_filled(center, r1, Color32::from_rgba_unmultiplied(2, 132, 199, 14));
    // 内圈微光过渡
    p.circle_filled(
        center,
        r2,
        Color32::from_rgba_unmultiplied(14, 165, 233, 22),
    );
}

/// 绘制 3D 悬浮晶石在地面的柔和椭圆接触阴影 (Grounding Contact Shadow)
pub fn draw_contact_floor_shadow(
    p: &Painter,
    cam: &Camera3D,
    view_rot: Mat4,
    center: Pos2,
    floor_y: f32,
    time: f32,
) {
    let rel = Vec3::new(0.0, floor_y, 0.0) - cam.target;
    let rot = view_rot.transform_point(rel);
    let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);
    if p_cam.z > 10.0 {
        let sx = center.x + (p_cam.x * cam.focal) / p_cam.z;
        let sy = center.y - (p_cam.y * cam.focal) / p_cam.z;

        // 根据呼吸悬浮高度微调阴影大小与透明度
        let bob_scale = 1.0 + (time * 1.6).sin() * 0.06;
        let rx = 56.0 * bob_scale;
        let ry = 14.0 * bob_scale;

        let segments = 24;
        let mut pts = Vec::with_capacity(segments);
        for i in 0..segments {
            let a = (i as f32) * 2.0 * PI / (segments as f32);
            pts.push(pos2(sx + a.cos() * rx, sy + a.sin() * ry));
        }

        p.add(egui::Shape::convex_polygon(
            pts,
            Color32::from_black_alpha(110),
            Stroke::NONE,
        ));
    }
}

/// 渲染 3D 华丽特效管线
pub fn render_3d_vfx(
    p: &Painter,
    cam: &Camera3D,
    view_rot: Mat4,
    center: Pos2,
    time: f32,
    step: InstallerStep,
) {
    // 1. 放射状美漫光芒 (Radial Rays) - 位于背景最深处
    draw_radial_comic_rays(p, center, time, step);

    // 2. 双环 3D 能量共振轨道 (Energy Aura Rings)
    draw_energy_aura_rings(p, cam, view_rot, center, time);

    // 3. 3D 粒子流星尘涡旋 (3D Particle Vortex)
    draw_particle_vortex(p, cam, view_rot, center, time);

    // 4. 超空间曲速流线 (Installing 模式专享加速特效)
    if step == InstallerStep::Installing {
        draw_hyperspace_warp_streaks(p, cam, view_rot, center, time);
    }

    // 5. 胜利彩屑与金色礼花 (Finished 模式专享庆典特效)
    if step == InstallerStep::Finished {
        draw_victory_confetti(p, cam, view_rot, center, time);
    }

    // 6. 3D 八芒星光高光 (Diamond Starburst Glints)
    draw_diamond_glints(p, cam, view_rot, center, time);
}

/// 绘制背景深处的旋转放射状美漫光芒
fn draw_radial_comic_rays(p: &Painter, center: Pos2, time: f32, step: InstallerStep) {
    let num_rays = 20;
    let base_speed = if step == InstallerStep::Installing {
        1.8
    } else {
        0.4
    };
    let rot_offset = time * base_speed;
    let r_inner = 30.0;
    let r_outer = 420.0;

    for i in 0..num_rays {
        if i % 2 == 1 {
            continue;
        }
        let a0 = (i as f32) * 2.0 * PI / (num_rays as f32) + rot_offset;
        let a1 = a0 + (PI / (num_rays as f32) * 0.85);

        let p0 = pos2(center.x + a0.cos() * r_inner, center.y + a0.sin() * r_inner);
        let p1 = pos2(center.x + a0.cos() * r_outer, center.y + a0.sin() * r_outer);
        let p2 = pos2(center.x + a1.cos() * r_outer, center.y + a1.sin() * r_outer);
        let p3 = pos2(center.x + a1.cos() * r_inner, center.y + a1.sin() * r_inner);

        let (alpha, col) = if step == InstallerStep::Installing {
            (48, Color32::from_rgb(0, 229, 255))
        } else if step == InstallerStep::Finished {
            (55, Color32::from_rgb(255, 215, 0))
        } else {
            (28, Color32::from_rgb(255, 230, 0))
        };

        p.add(egui::Shape::convex_polygon(
            vec![p0, p1, p2, p3],
            Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha),
            Stroke::NONE,
        ));
    }
}

/// 绘制围绕多面体晶石的 3D 倾斜能量双环轨道
fn draw_energy_aura_rings(p: &Painter, cam: &Camera3D, view_rot: Mat4, center: Pos2, time: f32) {
    let gem_y = 80.0 + (time * 1.5).sin() * 5.0;
    let rings = [
        (65.0, 0.45, 0.35, time * 1.2, Color32::from_rgb(0, 229, 255)),
        (
            78.0,
            -0.55,
            -0.40,
            -time * 0.9,
            Color32::from_rgb(255, 42, 85),
        ),
    ];

    for &(r, tilt_x, tilt_z, rot_y, color) in &rings {
        let mat = Mat4::rot_x(tilt_x)
            .mul(&Mat4::rot_z(tilt_z))
            .mul(&Mat4::rot_y(rot_y));

        let segments = 28;
        let mut pts = Vec::with_capacity(segments);
        for i in 0..=segments {
            let theta = (i as f32) * 2.0 * PI / (segments as f32);
            let local_p = Vec3::new(r * theta.cos(), 0.0, r * theta.sin());
            let world_p = mat.transform_point(local_p) + Vec3::new(0.0, gem_y, 0.0);

            let rot = view_rot.transform_point(world_p - cam.target);
            let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);

            if p_cam.z > 15.0 {
                let sx = center.x + (p_cam.x * cam.focal) / p_cam.z;
                let sy = center.y - (p_cam.y * cam.focal) / p_cam.z;
                pts.push((pos2(sx, sy), p_cam.z));
            }
        }

        if pts.len() > 2 {
            for i in 0..pts.len() - 1 {
                let (p0, z0) = pts[i];
                let (p1, _) = pts[i + 1];
                let thickness = (3.5 * cam.focal / z0).clamp(1.2, 4.0);
                p.line_segment([p0, p1], Stroke::new(thickness, color));
            }
        }
    }
}

/// 绘制 3D 空间中环绕爱奥尼立柱与晶石的粒子涡旋 (48 颗 3D 浮动星尘)
fn draw_particle_vortex(p: &Painter, cam: &Camera3D, view_rot: Mat4, center: Pos2, time: f32) {
    let count = 48;
    for i in 0..count {
        let fi = i as f32;
        let r = 52.0 + (fi * 1.87).sin().abs() * 45.0;
        let theta = (fi * 2.0 * PI / (count as f32)) + (time * 0.85);
        let y_base = -65.0 + (fi * 3.4);
        let y = y_base + (time * 2.2 + fi).sin() * 8.0;

        let world_p = Vec3::new(r * theta.cos(), y, r * theta.sin());
        let rot = view_rot.transform_point(world_p - cam.target);
        let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);

        if p_cam.z > 15.0 {
            let sx = center.x + (p_cam.x * cam.focal) / p_cam.z;
            let sy = center.y - (p_cam.y * cam.focal) / p_cam.z;
            let size = (4.5 * cam.focal / p_cam.z).clamp(1.5, 6.0);

            let col = if i % 3 == 0 {
                Color32::from_rgb(0, 240, 255)
            } else if i % 3 == 1 {
                Color32::from_rgb(255, 230, 0)
            } else {
                Color32::WHITE
            };

            // 赛璐珞美漫粒子：实心高光核心 + 黑色外圈描边
            p.circle_filled(pos2(sx, sy), size, col);
            p.circle_stroke(pos2(sx, sy), size, Stroke::new(1.0, Color32::BLACK));
        }
    }
}

/// 绘制曲速推进 3D 空间流线 (Installing 模式)
fn draw_hyperspace_warp_streaks(
    p: &Painter,
    cam: &Camera3D,
    view_rot: Mat4,
    center: Pos2,
    time: f32,
) {
    let streak_count = 24;
    for i in 0..streak_count {
        let fi = i as f32;
        let angle = fi * 2.0 * PI / (streak_count as f32);
        let r = 70.0 + (fi * 3.1).cos().abs() * 80.0;
        let progress = (time * 3.5 + fi * 0.28) % 1.0;

        let z_start = 280.0 - progress * 240.0;
        let z_end = (z_start - 35.0).max(10.0);

        let p1_w = Vec3::new(r * angle.cos(), 40.0 + (fi * 2.0).sin() * 40.0, z_start);
        let p2_w = Vec3::new(r * angle.cos(), 40.0 + (fi * 2.0).sin() * 40.0, z_end);

        let r1 = view_rot.transform_point(p1_w - cam.target);
        let r2 = view_rot.transform_point(p2_w - cam.target);

        let pc1 = Vec3::new(r1.x, r1.y, r1.z + cam.dist);
        let pc2 = Vec3::new(r2.x, r2.y, r2.z + cam.dist);

        if pc1.z > 15.0 && pc2.z > 15.0 {
            let sx1 = center.x + (pc1.x * cam.focal) / pc1.z;
            let sy1 = center.y - (pc1.y * cam.focal) / pc1.z;
            let sx2 = center.x + (pc2.x * cam.focal) / pc2.z;
            let sy2 = center.y - (pc2.y * cam.focal) / pc2.z;

            let w = (3.0 * cam.focal / pc2.z).clamp(1.2, 4.0);
            p.line_segment(
                [pos2(sx1, sy1), pos2(sx2, sy2)],
                Stroke::new(w, Color32::from_rgb(0, 240, 255)),
            );
        }
    }
}

/// 绘制 Finished 庆典维度的 3D 胜利彩屑礼花
fn draw_victory_confetti(p: &Painter, cam: &Camera3D, view_rot: Mat4, center: Pos2, time: f32) {
    let count = 36;
    for i in 0..count {
        let fi = i as f32;
        let drop_t = (time * 1.2 + fi * 0.35) % 2.5;
        let y = 140.0 - drop_t * 90.0;
        let sway = (time * 3.0 + fi * 1.5).sin() * 18.0;
        let theta = fi * 2.0 * PI / (count as f32);
        let r = 40.0 + (fi * 2.4).sin().abs() * 70.0;

        let world_p = Vec3::new(r * theta.cos() + sway, y, r * theta.sin());
        let rot = view_rot.transform_point(world_p - cam.target);
        let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);

        if p_cam.z > 15.0 {
            let sx = center.x + (p_cam.x * cam.focal) / p_cam.z;
            let sy = center.y - (p_cam.y * cam.focal) / p_cam.z;
            let sz = (6.0 * cam.focal / p_cam.z).clamp(2.5, 9.0);

            let colors = [
                Color32::from_rgb(255, 215, 0),
                Color32::from_rgb(0, 229, 255),
                Color32::from_rgb(255, 42, 85),
                Color32::from_rgb(16, 185, 129),
            ];
            let col = colors[i % colors.len()];

            // 绘制翻滚的菱形纸屑
            let half = sz * 0.5;
            let poly = vec![
                pos2(sx, sy - sz),
                pos2(sx + half, sy),
                pos2(sx, sy + sz),
                pos2(sx - half, sy),
            ];
            p.add(egui::Shape::convex_polygon(
                poly,
                col,
                Stroke::new(1.0, Color32::BLACK),
            ));
        }
    }
}

/// 绘制高光顶点的 3D 空间八芒星光 (Diamond Starburst Glints)
fn draw_diamond_glints(p: &Painter, cam: &Camera3D, view_rot: Mat4, center: Pos2, time: f32) {
    let glint_anchors = [
        (0.0, 95.0, 0.0),
        (-24.0, 80.0, 15.0),
        (24.0, 80.0, -15.0),
        (0.0, 36.0, 48.0),
        (-48.0, 33.0, 0.0),
        (48.0, 33.0, 0.0),
    ];

    for (i, &(gx, gy, gz)) in glint_anchors.iter().enumerate() {
        let p_world = Vec3::new(gx, gy, gz);
        let rot = view_rot.transform_point(p_world - cam.target);
        let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);

        if p_cam.z > 15.0 {
            let sx = center.x + (p_cam.x * cam.focal) / p_cam.z;
            let sy = center.y - (p_cam.y * cam.focal) / p_cam.z;
            let phase = time * 3.5 + (i as f32 * 1.8);
            let pulse = (phase.sin() * 0.5 + 0.5).powf(2.0);
            let size = 5.0 + pulse * 6.5;

            // 白色实心中心
            p.circle_filled(pos2(sx, sy), size * 0.45, Color32::WHITE);
            // 主十字星芒
            p.line_segment(
                [pos2(sx - size, sy), pos2(sx + size, sy)],
                Stroke::new(2.0, Color32::WHITE),
            );
            p.line_segment(
                [pos2(sx, sy - size), pos2(sx, sy + size)],
                Stroke::new(2.0, Color32::WHITE),
            );
            // 次级 45 度斜十字
            let diag = size * 0.65;
            p.line_segment(
                [pos2(sx - diag, sy - diag), pos2(sx + diag, sy + diag)],
                Stroke::new(1.2, Color32::WHITE),
            );
            p.line_segment(
                [pos2(sx - diag, sy + diag), pos2(sx + diag, sy - diag)],
                Stroke::new(1.2, Color32::WHITE),
            );
        }
    }
}
