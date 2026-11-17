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
    /// 步骤 0: 欢迎体验英雄展台 (Welcome Hero Stage)
    #[default]
    Welcome,
    /// 步骤 1: 安装目录与环境选项 (Directory & Integration Options)
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
            step: InstallerStep::Welcome,
            target_dir: default_dir,
            add_path: true,
            desktop_shortcut: true,
            tray_resident: true,
            launch_on_finish: true,
            install_progress: 0.0,
            status_msg: None,
            logo_tex: None,
            cam_yaw: 0.35,
            cam_pitch: 0.22,
            cam_dist: 450.0,
            auto_rotate: false,
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
        let win_size = Vec2::new(800.0, 560.0);
        let win_rect = Rect::from_center_size(total_rect.center(), win_size);

        self.render_comic_installer_frame(root, win_rect, true);
    }

    /// 应用内嵌入模态模式：暗化遮罩 + 800×560 美漫对话面板
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

        let win_size = Vec2::new(800.0, 560.0);
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
        let header_h = 54.0;
        let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
        let title_x = header_rect.left() + 60.0;
        let star_center = pos2(title_x + 124.0, header_rect.center().y);
        let wiz_rect = Rect::from_center_size(
            pos2(star_center.x + 84.0, header_rect.center().y),
            vec2(104.0, 24.0),
        );

        // 1-4. 绘制底板与标题栏静态图元
        {
            let p = ui.painter();

            // 1. 美漫纯黑实心硬阴影 (贴纸级立体感)
            let shadow_offset = vec2(6.0, 6.0);
            p.rect_filled(win_rect.translate(shadow_offset), 0.0, Color32::BLACK);

            // 2. 漫画主画板底色与粗墨线外框
            let comic_night_bg = Color32::from_rgb(18, 21, 35);
            p.rect_filled(win_rect, 0.0, comic_night_bg);
            p.rect_stroke(
                win_rect,
                0.0,
                Stroke::new(2.5, Color32::BLACK),
                StrokeKind::Inside,
            );

            // 3. 背景 Ben-Day 波普网点
            paint_ben_day_dots(
                p,
                win_rect.shrink(4.0),
                Color32::from_rgb(32, 38, 62),
                12.0,
                1.2,
            );

            // 4. 顶部 Comic Header 栏：美漫高饱和行动条 (#FFE600)，彻底移除 Windows 原生标题栏
            p.rect_filled(header_rect, 0.0, Color32::from_rgb(255, 230, 0));
            p.rect_stroke(
                header_rect,
                0.0,
                Stroke::new(2.0, Color32::BLACK),
                StrokeKind::Inside,
            );
            p.line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                Stroke::new(2.5, Color32::BLACK),
            );

            // 头部速度动量斜切线 (Speed Lines)
            for sx in (18..200).step_by(22) {
                let p1 = pos2(header_rect.left() + sx as f32, header_rect.top());
                let p2 = pos2(header_rect.left() + sx as f32 + 24.0, header_rect.bottom());
                p.line_segment([p1, p2], Stroke::new(4.0, Color32::from_rgb(240, 210, 0)));
                p.line_segment([p1, p2], Stroke::new(1.0, Color32::BLACK));
            }

            // 左上角正版原生 Logo 纹理徽章 (使用原始正版 Crystal Logo，保留完整品牌辨识度)
            let logo_center = pos2(header_rect.left() + 32.0, header_rect.center().y);
            paint_authentic_logo(
                p,
                self.installer.logo_tex.as_ref(),
                logo_center,
                28.0,
                vec2(2.0, 2.0),
            );

            // 品牌标题文字 (带纯黑实心投影)
            p.text(
                pos2(title_x + 2.0, header_rect.center().y + 2.0),
                egui::Align2::LEFT_CENTER,
                "IDE 1.0",
                egui::FontId::proportional(18.0),
                Color32::BLACK,
            );
            p.text(
                pos2(title_x, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "IDE 1.0",
                egui::FontId::proportional(18.0),
                Color32::from_rgb(10, 10, 15),
            );

            // 爆炸贴纸 "POW! FAST"
            paint_comic_burst_star(
                p,
                star_center,
                20.0,
                11.0,
                10,
                Color32::from_rgb(255, 42, 85),
                vec2(2.0, 2.0),
            );
            p.text(
                star_center,
                egui::Align2::CENTER_CENTER,
                "FAST!",
                egui::FontId::proportional(8.5),
                Color32::WHITE,
            );

            // 美漫行动胶囊 "SETUP WIZARD"
            paint_comic_hard_shadow_rect(
                p,
                wiz_rect,
                Color32::from_rgb(0, 229, 255),
                2.0,
                vec2(2.5, 2.5),
            );
            p.text(
                wiz_rect.center(),
                egui::Align2::CENTER_CENTER,
                "SETUP WIZARD",
                egui::FontId::proportional(10.5),
                Color32::BLACK,
            );

            // 右侧版本胶囊
            let ver_rect = Rect::from_center_size(
                pos2(header_rect.right() - 86.0, header_rect.center().y),
                vec2(76.0, 24.0),
            );
            paint_comic_hard_shadow_rect(p, ver_rect, Color32::WHITE, 2.0, vec2(2.0, 2.0));
            p.text(
                ver_rect.center(),
                egui::Align2::CENTER_CENTER,
                concat!("★ v", env!("CARGO_PKG_VERSION")),
                egui::FontId::proportional(11.0),
                Color32::BLACK,
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

        let left_w = 310.0;
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
