//! 美漫安装向导真 3D 赛璐珞英雄展台分镜 (Hero 3D Stage View)
//! 包含古希腊爱奥尼神殿立柱展台、正二十面体悬浮晶钻多面体、阿波罗黄金月桂冠、
//! 3D 阶梯式光照、粗黑墨线轮廓描边与用户交互式自由 3D 拖拽旋转。

use super::art::*;
use super::cel3d::*;
use super::{InstallerState, InstallerStep};
use crate::app::App;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke};
use std::f32::consts::PI;
use std::sync::OnceLock;

static PEDESTAL_MESH: OnceLock<Mesh3D> = OnceLock::new();
static GEM_MESH: OnceLock<Mesh3D> = OnceLock::new();
static WREATH_MESH: OnceLock<Mesh3D> = OnceLock::new();

fn get_pedestal() -> &'static Mesh3D {
    PEDESTAL_MESH.get_or_init(build_ionic_pedestal)
}

fn get_gem() -> &'static Mesh3D {
    GEM_MESH.get_or_init(|| build_crystal_gem(30.0))
}

fn get_wreath() -> &'static Mesh3D {
    WREATH_MESH.get_or_init(|| build_laurel_wreath(26, 50.0, 44.0))
}

impl App {
    /// 渲染左侧英雄展台分镜 (Hero 3D Cel-Shaded Stage)
    pub(super) fn render_left_hero_stage(&mut self, ui: &mut egui::Ui, left_panel_rect: Rect) {
        let p = ui.painter();
        paint_comic_hard_shadow_rect(
            p,
            left_panel_rect,
            Color32::from_rgb(22, 26, 44),
            2.5,
            vec2(3.5, 3.5),
        );
        paint_ben_day_dots(
            p,
            left_panel_rect.shrink(4.0),
            Color32::from_rgb(0, 229, 255),
            8.0,
            1.0,
        );

        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let time = ui.input(|i| i.time) as f32;

        let stage_h = left_panel_rect.height() - 124.0;
        let stage_rect =
            Rect::from_min_size(left_panel_rect.min, vec2(left_panel_rect.width(), stage_h));

        // 1. 背景放射状漫画爆发光芒 (Burst Rays)
        let core_center = pos2(stage_rect.center().x, stage_rect.top() + 98.0);
        for a in (0..360).step_by(24) {
            let rad = (a as f32) * PI / 180.0;
            let ray_p = pos2(
                core_center.x + 88.0 * rad.cos(),
                core_center.y + 88.0 * rad.sin(),
            );
            p.line_segment(
                [core_center, ray_p],
                Stroke::new(1.0, Color32::from_rgb(255, 230, 0)),
            );
        }

        // 2. 交互式 3D 舞台鼠标拖拽与滚轮缩放处理
        handle_stage_drag_and_zoom(ui, &mut self.installer, stage_rect, dt);

        // 3. 动态计算 3D 变换矩阵与相机视图
        let bob_y = 78.0 + (time * 2.5).sin() * 6.0;
        let gem_spin_speed = if self.installer.step == InstallerStep::Installing {
            4.8
        } else {
            1.2
        };
        let gem_rot = Mat4::rot_y(time * gem_spin_speed)
            .mul(&Mat4::rot_x((time * 0.8).sin() * 0.25));
        let gem_model_mat = Mat4::translate(0.0, bob_y, 0.0).mul(&gem_rot);

        let wreath_rot = Mat4::rot_y(time * 0.5);
        let wreath_model_mat = Mat4::translate(0.0, bob_y, 0.0).mul(&wreath_rot);

        let cam = Camera3D {
            yaw: self.installer.cam_yaw,
            pitch: self.installer.cam_pitch,
            dist: self.installer.cam_dist,
            focal: 380.0,
            target: Vec3::new(0.0, 16.0, 0.0),
        };

        // 4. 渲染真 3D 赛璐珞神殿与晶钻场景 (包含放射光芒、能量双环、星尘涡旋等华丽特效)
        render_cel3d_scene(
            ui.painter(),
            stage_rect,
            &cam,
            get_pedestal(),
            get_gem(),
            get_wreath(),
            &gem_model_mat,
            &wreath_model_mat,
            time,
            self.installer.step,
        );

        // 5. 投影真实正版 Logo 到晶钻核心中心 (严格保留正版 Logo)
        render_projected_logo(
            ui.painter(),
            self.installer.logo_tex.as_ref(),
            &cam,
            bob_y,
            stage_rect,
        );

        // 6. 3D 交互控制悬浮胶囊 HUD
        render_hud_pill(ui, &mut self.installer, stage_rect);

        // 7. 漫画对话气泡与英雄印章
        render_speech_balloon_and_stamp(ui.painter(), left_panel_rect, self.installer.step);

        // 持续以 60 FPS 平滑微帧驱动 3D 赛璐珞涡旋与光影动效
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn handle_stage_drag_and_zoom(
    ui: &mut egui::Ui,
    inst: &mut InstallerState,
    stage_rect: Rect,
    dt: f32,
) {
    let resp = ui.interact(
        stage_rect,
        ui.id().with("installer_3d_stage_interact"),
        egui::Sense::click_and_drag(),
    );

    if resp.dragged() {
        let delta = resp.drag_delta();
        inst.cam_yaw += delta.x * 0.012;
        inst.cam_pitch = (inst.cam_pitch - delta.y * 0.012).clamp(-0.45, 0.85);
        inst.auto_rotate = false;
        ui.ctx().request_repaint();
    } else if inst.auto_rotate {
        let speed = if inst.step == InstallerStep::Installing {
            2.2
        } else {
            0.40
        };
        inst.cam_yaw += dt * speed;
        ui.ctx().request_repaint();
    }

    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.0 {
            inst.cam_dist = (inst.cam_dist - scroll * 0.5).clamp(320.0, 600.0);
            ui.ctx().request_repaint();
        }
    }

    if resp.double_clicked() {
        inst.cam_yaw = 0.35;
        inst.cam_pitch = 0.22;
        inst.cam_dist = 450.0;
        inst.auto_rotate = false;
    }
}

fn render_projected_logo(
    p: &egui::Painter,
    logo_tex: Option<&egui::TextureHandle>,
    cam: &Camera3D,
    bob_y: f32,
    stage_rect: Rect,
) {
    let rot_x = Mat4::rot_x(cam.pitch);
    let rot_y = Mat4::rot_y(cam.yaw);
    let view_rot = rot_x.mul(&rot_y);
    let rel = Vec3::new(0.0, bob_y, 0.0) - cam.target;
    let rot = view_rot.transform_point(rel);
    let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);
    if p_cam.z > 10.0 {
        let center_x = stage_rect.center().x + (p_cam.x * cam.focal) / p_cam.z;
        let center_y = stage_rect.center().y - (p_cam.y * cam.focal) / p_cam.z;
        paint_authentic_logo(
            p,
            logo_tex,
            pos2(center_x, center_y),
            36.0,
            vec2(2.5, 2.5),
        );
    }
}

fn render_hud_pill(ui: &mut egui::Ui, inst: &mut InstallerState, stage_rect: Rect) {
    let hud_rect = Rect::from_center_size(
        pos2(stage_rect.center().x, stage_rect.top() + 18.0),
        vec2(210.0, 24.0),
    );
    let hud_resp = ui.interact(
        hud_rect,
        ui.id().with("installer_3d_hud_btn"),
        egui::Sense::click(),
    );
    if hud_resp.clicked() {
        inst.auto_rotate = !inst.auto_rotate;
        if inst.auto_rotate {
            ui.ctx().request_repaint();
        }
    }

    let (hud_bg, hud_text) = if hud_resp.hovered() {
        (
            Color32::from_rgb(255, 230, 0),
            if inst.auto_rotate {
                "⟳ PAUSE 3D ORBIT"
            } else {
                "⟳ 3D VIEW · DRAG TO ROTATE"
            },
        )
    } else {
        (
            Color32::from_rgb(32, 38, 62),
            if inst.auto_rotate {
                "⟳ 3D ORBIT ACTIVE"
            } else {
                "⟳ 3D VIEW · DRAG TO ROTATE"
            },
        )
    };
    paint_comic_hard_shadow_rect(ui.painter(), hud_rect, hud_bg, 1.8, vec2(1.5, 1.5));
    ui.painter().text(
        hud_rect.center(),
        egui::Align2::CENTER_CENTER,
        hud_text,
        egui::FontId::proportional(10.0),
        if hud_resp.hovered() {
            Color32::BLACK
        } else {
            Color32::from_rgb(0, 229, 255)
        },
    );
}

fn render_speech_balloon_and_stamp(p: &egui::Painter, panel_rect: Rect, step: InstallerStep) {
    let bubble_rect = Rect::from_center_size(
        pos2(panel_rect.center().x, panel_rect.bottom() - 78.0),
        vec2(panel_rect.width() - 24.0, 48.0),
    );
    paint_comic_hard_shadow_rect(
        p,
        bubble_rect,
        Color32::from_rgb(255, 253, 240),
        2.0,
        vec2(2.5, 2.5),
    );
    let tail_top = pos2(bubble_rect.center().x - 10.0, bubble_rect.top() - 8.0);
    let tail_l = pos2(bubble_rect.center().x - 18.0, bubble_rect.top());
    let tail_r = pos2(bubble_rect.center().x - 2.0, bubble_rect.top());
    p.add(egui::Shape::convex_polygon(
        vec![tail_top, tail_l, tail_r],
        Color32::from_rgb(255, 253, 240),
        Stroke::new(2.0, Color32::BLACK),
    ));
    p.line_segment(
        [tail_l, tail_r],
        Stroke::new(2.5, Color32::from_rgb(255, 253, 240)),
    );

    let (b_line1, b_line2) = match step {
        InstallerStep::Welcome => (
            "⚡ 真 3D 赛璐珞 · 纯 Rust 极速打造！",
            "全景自由视角 · 极致轻量性能体验",
        ),
        InstallerStep::Options => ("📁 自由定制你的专属环境！", "随心配置路径与系统集成选项"),
        InstallerStep::Installing => {
            ("🚀 3D 超速引擎正释放微内核...", "编译缓存与环境秒级就绪中")
        }
        InstallerStep::Finished => (
            "🎉 恭喜！IDE 1.0 极速环境已就绪！",
            "开启属于你的无拘编码冒险",
        ),
    };

    p.text(
        pos2(bubble_rect.center().x, bubble_rect.center().y - 8.0),
        egui::Align2::CENTER_CENTER,
        b_line1,
        egui::FontId::proportional(11.0),
        Color32::BLACK,
    );
    p.text(
        pos2(bubble_rect.center().x, bubble_rect.center().y + 10.0),
        egui::Align2::CENTER_CENTER,
        b_line2,
        egui::FontId::proportional(9.2),
        Color32::from_rgb(70, 75, 90),
    );

    let stamp_rect = Rect::from_center_size(
        pos2(panel_rect.center().x, panel_rect.bottom() - 24.0),
        vec2(panel_rect.width() - 44.0, 24.0),
    );
    let (stamp_col, stamp_text) = if step == InstallerStep::Finished {
        (Color32::from_rgb(255, 204, 0), "★ MISSION COMPLETE ★")
    } else {
        (Color32::from_rgb(255, 42, 85), "★ 100% NATIVE 3D HERO ★")
    };
    paint_comic_hard_shadow_rect(p, stamp_rect, stamp_col, 2.0, vec2(2.0, 2.0));
    p.text(
        stamp_rect.center(),
        egui::Align2::CENTER_CENTER,
        stamp_text,
        egui::FontId::proportional(10.0),
        if step == InstallerStep::Finished {
            Color32::BLACK
        } else {
            Color32::WHITE
        },
    );
}
