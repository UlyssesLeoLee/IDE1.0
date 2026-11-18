//! 经典美漫与赛璐珞卡通渲染风格安装与首选配置向导 (American Comic & Cel-Shaded Setup Wizard)
//! 全定制原生渲染：800×560 宽幅漫画分镜布局、粗墨线轮廓描边 (2.0~2.5px 纯黑 Ink Outlines)、
//! 纯黑实心硬阴影 (Hard Drop Shadows)、波普半色调网点 (Ben-Day Screentone Dots)、
//! 真实原生正版 Crystal Logo 纹理渲染、爱奥尼古典神殿柱台、爆炸贴纸 (Action Burst Stars) 与对话气泡 (Speech Balloons)。
//!
//! 状态机模式驱动 4 步向导流程：
//! 1. Welcome (欢迎体验英雄展台) ->
//! 2. Options (安装目录与系统集成选项) ->
//! 3. Installing (部署安装中动态速度条与控制台) ->
//! 4. Finished (部署就绪与启动)
//!
//! 无任何 Windows 原生标题栏与原生按钮，全定制美漫拖动把手、美漫最小化与关闭控制钮。

pub mod art;
pub mod cards;
pub mod cel3d;
pub mod cel3d_models;
pub mod cel3d_vfx;
mod deploy;
mod hero;
mod steps;
#[cfg(test)]
mod tests;

use super::*;
use art::*;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};

/// 向导状态机步骤枚举
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum InstallerStep {
    /// 步骤 0: 欢迎体验英雄展台 (已由新版精简，直接切入核心配置)
    Welcome,
    /// 步骤 1: 安装目录与环境选项 (Directory & Integration Options)
    #[default]
    Options,
    /// 步骤 2: 部署安装中动画 (Deploying & Real-time Progress)
    Installing,
    /// 步骤 3: 部署完成与启动 (Completed & Launch)
    Finished,
}

pub struct InstallerState {
    pub open: bool,
    pub standalone: bool,
    pub step: InstallerStep,
    pub target_dir: String,
    pub add_path: bool,
    pub desktop_shortcut: bool,
    pub tray_resident: bool,
    pub launch_on_finish: bool,
    pub install_progress: f32,
    pub status_msg: Option<String>,
    pub logo_tex: Option<egui::TextureHandle>,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam_dist: f32,
    pub auto_rotate: bool,
    pub last_mouse_pos: Option<egui::Pos2>,
    pub rot_target_yaw: f32,
    pub rot_target_pitch: f32,
}

impl Default for InstallerState {
    fn default() -> Self {
        let default_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().to_string()))
            .unwrap_or_else(|| r"C:\Program Files\IDE1.0".to_string());
        Self {
            open: false,
            standalone: false,
            step: InstallerStep::Options,
            target_dir: default_dir,
            add_path: true,
            desktop_shortcut: true,
            tray_resident: true,
            launch_on_finish: true,
            install_progress: 0.0,
            status_msg: None,
            logo_tex: None,
            cam_yaw: 0.35,
            cam_pitch: 0.18,
            cam_dist: 450.0,
            auto_rotate: true,
            last_mouse_pos: None,
            rot_target_yaw: 0.35,
            rot_target_pitch: 0.18,
        }
    }
}

impl App {
    /// 确保已解码并装载正版原生 Logo 纹理 (严格保留用户原始正版 Logo，不改动)
    fn ensure_installer_logo(&mut self, ctx: &egui::Context) {
        if self.installer.logo_tex.is_none() {
            if let Some(ic) = crate::appicon::logo() {
                let img = egui::ColorImage::from_rgba_unmultiplied(
                    [ic.width as usize, ic.height as usize],
                    &ic.rgba,
                );
                self.installer.logo_tex = Some(ctx.load_texture(
                    "installer_authentic_logo",
                    img,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }

    /// 独立安装向导模式：直接铺满 800×560 视口，完全自绘美漫风格无原生边框窗口
    pub(super) fn installer_standalone(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        self.ensure_installer_logo(&ctx);

        if !self.shown {
            self.shown = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }

        let total_rect = root.max_rect();
        let win_size = Vec2::new(880.0, 600.0);
        let win_rect = Rect::from_center_size(total_rect.center(), win_size);

        self.render_comic_installer_frame(root, win_rect, true);
    }

    /// 应用内嵌入模态模式：暗化遮罩 + 880×600 美漫对话面板
    pub(super) fn installer_window(&mut self, ctx: &egui::Context) {
        if !self.installer.open || self.installer.standalone {
            return;
        }

        self.ensure_installer_logo(ctx);

        // 半透明漫画墨水暗化遮罩
        let screen_rect = ctx.content_rect();
        let bg_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("installer_comic_modal_backdrop"),
        ));
        bg_painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(210));

        let win_size = Vec2::new(880.0, 600.0);
        egui::Window::new("installer_comic_modal")
            .id(egui::Id::new("installer_comic_modal_window"))
            .title_bar(false)
            .frame(egui::Frame::NONE)
            .fixed_size(win_size)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let win_rect = ui.max_rect();
                self.render_comic_installer_frame(ui, win_rect, false);
            });
    }

    /// 渲染美漫向导的核心外观与分镜内容
    fn render_comic_installer_frame(
        &mut self,
        ui: &mut egui::Ui,
        win_rect: Rect,
        standalone: bool,
    ) {
        let ctx = ui.ctx().clone();
        let header_h = 50.0;
        let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
        let title_x = header_rect.left() + 56.0;
        let wiz_rect = Rect::from_center_size(
            pos2(title_x + 116.0, header_rect.center().y),
            vec2(76.0, 22.0),
        );

        // 1-4. 绘制底板与标题栏静态图元 (低调奢华暗色系 · 色彩心理学精准契合)
        {
            let p = ui.painter();

            // 1. 美漫纯黑实心硬阴影 (贴纸级立体感)
            let shadow_offset = vec2(6.0, 6.0);
            p.rect_filled(win_rect.translate(shadow_offset), 0.0, Color32::BLACK);

            // 2. 漫画主画板底色与粗墨线外框 (深邃黑曜钛金底板，低眩光极度舒适)
            let comic_night_bg = Color32::from_rgb(14, 18, 26);
            p.rect_filled(win_rect, 0.0, comic_night_bg);
            p.rect_stroke(
                win_rect,
                0.0,
                Stroke::new(2.0, Color32::from_rgb(30, 41, 59)),
                StrokeKind::Inside,
            );

            // 3. 顶部 Header 栏：精工黑曜钛金栏 (#141924) + 皇家宝蓝分割线 (#0284c7)
            let header_bg = Color32::from_rgb(20, 25, 36);
            p.rect_filled(header_rect, 0.0, header_bg);
            p.line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                Stroke::new(1.8, Color32::from_rgb(2, 132, 199)),
            );

            // 左上角正版原生 Logo 纹理徽章 (严格保留正版 Crystal Logo，26px 紧凑精致)
            let logo_center = pos2(header_rect.left() + 28.0, header_rect.center().y);
            paint_authentic_logo(
                p,
                self.installer.logo_tex.as_ref(),
                logo_center,
                26.0,
                vec2(2.0, 2.0),
            );

            // 品牌标题文字 "IDE 1.0"
            p.text(
                pos2(title_x + 1.5, header_rect.center().y + 1.5),
                egui::Align2::LEFT_CENTER,
                "IDE 1.0",
                egui::FontId::proportional(17.0),
                Color32::BLACK,
            );
            p.text(
                pos2(title_x, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "IDE 1.0",
                egui::FontId::proportional(17.0),
                Color32::from_rgb(241, 245, 249),
            );

            // 专属低调奢华安装向导胶囊 "安装向导"
            paint_comic_hard_shadow_rect(
                p,
                wiz_rect,
                Color32::from_rgb(2, 132, 199),
                1.5,
                vec2(1.5, 1.5),
            );
            p.text(
                wiz_rect.center(),
                egui::Align2::CENTER_CENTER,
                "安装向导",
                egui::FontId::proportional(10.5),
                Color32::WHITE,
            );

            // 右侧版本胶囊
            let ver_rect = Rect::from_center_size(
                pos2(header_rect.right() - 86.0, header_rect.center().y),
                vec2(74.0, 22.0),
            );
            paint_comic_hard_shadow_rect(p, ver_rect, Color32::from_rgb(30, 41, 59), 1.5, vec2(1.5, 1.5));
            p.text(
                ver_rect.center(),
                egui::Align2::CENTER_CENTER,
                concat!("★ v", env!("CARGO_PKG_VERSION")),
                egui::FontId::proportional(10.5),
                Color32::from_rgb(148, 163, 184),
            );
        }

        // 顶部原生窗口拖动把手 (Caption Drag Handle)
        let drag_rect = Rect::from_min_max(
            pos2(wiz_rect.right() + 10.0, header_rect.top()),
            pos2(header_rect.right() - 120.0, header_rect.bottom()),
        );
        let drag_resp = ui.interact(
            drag_rect,
            ui.id().with("installer_drag_caption"),
            egui::Sense::drag(),
        );
        if drag_resp.drag_started() || drag_resp.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // 纯自绘美漫最小化控制按钮 (取代 Windows 原生按钮，复用统一图元)
        if paint_comic_window_btn(
            ui,
            pos2(header_rect.right() - 50.0, header_rect.center().y),
            "comic_inst_min",
            false,
        ) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        // 纯自绘美漫关闭控制按钮 (取代 Windows 原生按钮，复用统一图元)
        if paint_comic_window_btn(
            ui,
            pos2(header_rect.right() - 20.0, header_rect.center().y),
            "comic_inst_close",
            true,
        ) {
            if standalone {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.installer.open = false;
            }
        }

        // 5. 内容主体分镜：左侧英雄展台 (Hero Stage) + 右侧多步操作向导 (Wizard Content)
        let body_rect = Rect::from_min_max(
            pos2(win_rect.left() + 16.0, header_rect.bottom() + 14.0),
            pos2(win_rect.right() - 16.0, win_rect.bottom() - 12.0),
        );

        let left_w = 330.0;
        let left_panel_rect = Rect::from_min_size(body_rect.min, vec2(left_w, body_rect.height()));
        let right_panel_rect = Rect::from_min_max(
            pos2(left_panel_rect.right() + 14.0, body_rect.top()),
            body_rect.max,
        );

        // 5.1 渲染左侧英雄展台 (真 3D 赛璐珞互动舞台)
        self.render_left_hero_stage(ui, left_panel_rect);

        // 5.2 渲染右侧向导步骤内容
        self.render_right_step_panel(ui, right_panel_rect, standalone);
    }
}
