//! 美漫安装向导真 3D 赛璐珞水晶徽标展台 (Authentic 3D Cel-Shaded Logo Stage)
//! 专注于 IDE1.0 正版水晶钻石徽标的真 3D 赛璐珞卡通渲染：
//! 纯粹展示核心 Logo，彻底去除希腊立柱、桂冠与光芒粒子等冗余元素，
//! 采用低调奢华暗色调、色彩心理学宝蓝/天青分级与舒适视觉层次，降低认知负荷，重点突出。

use super::art::*;
use super::cel3d::*;
use super::{InstallerState, InstallerStep};
use crate::app::App;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect};
use std::sync::OnceLock;

static LOGO_MESH: OnceLock<Mesh3D> = OnceLock::new();

fn get_logo() -> &'static Mesh3D {
    LOGO_MESH.get_or_init(|| build_ide1_crystal_logo(82.0))
}

impl App {
    /// 渲染左侧英雄展台分镜 (仅突出正版 3D 水晶钻石徽标，低调奢华 · 0 冗余干扰)
    pub(super) fn render_left_hero_stage(&mut self, ui: &mut egui::Ui, left_panel_rect: Rect) {
        let p = ui.painter();

        // 1. 低调奢华暗黑曜石展台底板 (Obsidian Stage Panel)
        paint_comic_hard_shadow_rect(
            p,
            left_panel_rect,
            Color32::from_rgb(15, 19, 28),
            2.0,
            vec2(3.5, 3.5),
        );

        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let time = ui.input(|i| i.time) as f32;

        let stage_rect = left_panel_rect.shrink(6.0);

        // 2. 交互式 3D 舞台鼠标拖拽与滚轮缩放处理
        handle_stage_drag_and_zoom(ui, &mut self.installer, stage_rect, dt);

        // 3. 动态计算 3D 变换矩阵与相机视图
        // 柔和优雅的呼吸漂浮与旋转
        let bob_y = (time * 1.5).sin() * 7.0;
        let spin_speed = if self.installer.step == InstallerStep::Installing {
            2.0
        } else {
            0.65
        };

        let logo_rot = Mat4::rot_y(time * spin_speed)
            .mul(&Mat4::rot_x((time * 0.5).sin() * 0.12));
        let logo_model_mat = Mat4::translate(0.0, bob_y, 0.0).mul(&logo_rot);

        let cam = Camera3D {
            yaw: self.installer.cam_yaw,
            pitch: self.installer.cam_pitch,
            dist: self.installer.cam_dist,
            focal: 380.0,
            target: Vec3::new(0.0, 0.0, 0.0),
        };

        // 4. 专属渲染 IDE1.0 水晶徽标真 3D 赛璐珞场景 (低调奢华氛围光晕 + 地面阴影 + 赛璐珞分色晶体)
        render_cel3d_crystal_scene(
            ui.painter(),
            stage_rect,
            &cam,
            get_logo(),
            &logo_model_mat,
            time,
        );

        // 5. 底部低调优雅 HUD 控制胶囊 (360° 交互提示)
        render_luxury_hud_pill(ui, &mut self.installer, stage_rect);

        // 持续以 60 FPS 平滑微帧驱动 3D 旋转与微光渲染
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
            1.8
        } else {
            0.35
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
        inst.auto_rotate = true;
    }
}

fn render_luxury_hud_pill(ui: &mut egui::Ui, inst: &mut InstallerState, stage_rect: Rect) {
    let hud_rect = Rect::from_center_size(
        pos2(stage_rect.center().x, stage_rect.bottom() - 20.0),
        vec2(220.0, 24.0),
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

    let (hud_bg, hud_text, text_col) = if hud_resp.hovered() {
        (
            Color32::from_rgb(30, 41, 59),
            if inst.auto_rotate {
                "⏸ 点击暂停自动旋转"
            } else {
                "▶ 点击开启自动旋转"
            },
            Color32::from_rgb(56, 189, 248),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(20, 26, 38, 220),
            if inst.auto_rotate {
                "🔄 360° 拖拽观察 · 双击复位"
            } else {
                "🔄 360° 拖拽观察 · 点击自转"
            },
            Color32::from_rgb(148, 163, 184),
        )
    };

    paint_comic_hard_shadow_rect(ui.painter(), hud_rect, hud_bg, 1.5, vec2(1.5, 1.5));
    ui.painter().text(
        hud_rect.center(),
        egui::Align2::CENTER_CENTER,
        hud_text,
        egui::FontId::proportional(10.0),
        text_col,
    );
}
