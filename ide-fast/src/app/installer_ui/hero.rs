//! 美漫安装向导真 3D 赛璐珞水晶徽标展台 (Authentic 3D Cel-Shaded Logo Stage)
//! 专注于 IDE1.0 正版水晶钻石徽标的真 3D 赛璐珞卡通渲染：
//! 纯粹展示核心 Logo，彻底去除希腊立柱、桂冠与光芒粒子等冗余元素，
//! 采用低调奢华暗色调、色彩心理学宝蓝/天青分级与舒适视觉层次，降低认知负荷，重点突出。
//! 响应式微交互：移动鼠标或移经按钮时触发平滑随机的缓慢宝石旋转，高阻尼优雅过渡。

use super::art::*;
use super::cel3d::*;
use super::InstallerStep;
use crate::app::App;
use eframe::egui;
use egui::{vec2, Color32, Rect};
use std::sync::OnceLock;

static LOGO_MESH: OnceLock<Mesh3D> = OnceLock::new();

fn get_logo() -> &'static Mesh3D {
    LOGO_MESH.get_or_init(|| build_ide1_crystal_logo(96.0))
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

        // 2. 鼠标移动/悬停交互感知：移动或悬停按钮时，触发微小随机且极为平缓的旋转扰动
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        if let Some(curr_pos) = hover_pos {
            let moved = if let Some(last) = self.installer.last_mouse_pos {
                (curr_pos - last).length() > 2.5
            } else {
                true
            };
            if moved {
                self.installer.last_mouse_pos = Some(curr_pos);
                // 空间坐标与时间驱动确定性哈希，生成微幅随机冲量 (绝不激进跳跃)
                let hash_val = (curr_pos.x * 12.9898 + curr_pos.y * 78.233 + time * 2.17).sin() * 43758.5453;
                let delta_yaw = (hash_val.fract() - 0.5) * 0.35;
                let hash_val2 = (curr_pos.y * 23.456 + curr_pos.x * 67.891 + time * 1.63).cos() * 24634.6345;
                let delta_pitch = (hash_val2.fract() - 0.5) * 0.18;

                self.installer.rot_target_yaw += delta_yaw;
                self.installer.rot_target_pitch =
                    (self.installer.rot_target_pitch + delta_pitch).clamp(-0.25, 0.40);
            }
        }

        // 恒常极其微缓的自转 (安装中加速，配置中极其沉静)
        let idle_speed = if self.installer.step == InstallerStep::Installing {
            1.5
        } else {
            0.18
        };
        self.installer.rot_target_yaw += dt * idle_speed;

        // 平滑阻尼插值趋向目标角度，带来如同真实珠宝悬浮于流体中的奢华厚重感
        let damping = (dt * 1.8).clamp(0.02, 0.18);
        self.installer.cam_yaw += (self.installer.rot_target_yaw - self.installer.cam_yaw) * damping;
        self.installer.cam_pitch += (self.installer.rot_target_pitch - self.installer.cam_pitch) * damping;

        // 3. 动态计算 3D 变换矩阵与相机视图
        // 柔和优雅的呼吸漂浮
        let bob_y = (time * 1.4).sin() * 8.0;

        let logo_rot = Mat4::rot_y(self.installer.cam_yaw)
            .mul(&Mat4::rot_x(self.installer.cam_pitch));
        let logo_model_mat = Mat4::translate(0.0, bob_y, 0.0).mul(&logo_rot);

        let cam = Camera3D {
            yaw: 0.0,
            pitch: 0.0,
            dist: 450.0,
            focal: 390.0,
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

        // 持续以 60 FPS 平滑微帧驱动 3D 缓慢旋转与微光渲染
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
    }
}
