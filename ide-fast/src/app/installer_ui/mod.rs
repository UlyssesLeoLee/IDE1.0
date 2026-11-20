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
    /// 步骤 1: 安装目录与环境选项 / 维护模式选择 (Directory & Integration Options)
    #[default]
    Options,
    /// 步骤 2: 部署安装 / 修复 / 卸载执行中动画 (Deploying & Real-time Progress)
    Installing,
    /// 步骤 3: 部署 / 修复 / 卸载完成 (Completed & Launch)
    Finished,
}

/// 启动安装/维护向导时的外部入口模式
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum InstallerLaunchMode {
    /// 自动探测本机是否已安装 IDE 1.0：已安装则进入维护向导，未安装则进入全新安装
    #[default]
    Auto,
    /// 直接进入卸载向导 (例如 --uninstall 或 Windows 应用与功能卸载入口)
    Uninstall,
    /// 直接进入修复向导 (--repair)
    Repair,
}

/// 当前向导所处的运行模式
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum InstallerMode {
    /// 全新安装或更改安装目录/集成配置
    #[default]
    Install,
    /// 已检测到本机安装：维护总览页 (修复 / 卸载 / 更改配置)
    Maintenance,
    /// 修复已有安装 (保留用户配置并重新部署主程序、快捷方式、PATH 与注册表)
    Repair,
    /// 卸载移除程序与系统集成
    Uninstall,
}

/// 本机已探测到的 IDE 1.0 安装信息
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingInstall {
    pub install_dir: String,
    pub installed_version: Option<String>,
    pub has_desktop_shortcut: bool,
    pub has_start_menu_shortcut: bool,
    pub has_path_entry: bool,
    pub has_uninstall_reg: bool,
}

pub struct InstallerState {
    pub open: bool,
    pub standalone: bool,
    pub step: InstallerStep,
    pub mode: InstallerMode,
    pub existing: Option<ExistingInstall>,
    pub target_dir: String,
    pub add_path: bool,
    pub desktop_shortcut: bool,
    pub tray_resident: bool,
    pub launch_on_finish: bool,
    pub remove_shortcuts: bool,
    pub remove_path_and_reg: bool,
    pub remove_user_data: bool,
    pub deferred_self_cleanup_dir: Option<String>,
    /// 测试/预览干跑标记：开启时绝不读写注册表、快捷方式、PATH 或删除主机文件
    pub dry_run: bool,
    pub install_progress: f32,
    pub status_msg: Option<String>,
    /// 后台部署线程结果通道 (真实模式; `None` 表示尚未启动或已收尾)
    pub deploy_rx: Option<std::sync::mpsc::Receiver<deploy::DeployOutcome>>,
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
            mode: InstallerMode::Install,
            existing: None,
            target_dir: default_dir,
            add_path: true,
            desktop_shortcut: true,
            tray_resident: true,
            launch_on_finish: true,
            remove_shortcuts: true,
            remove_path_and_reg: true,
            remove_user_data: false,
            deferred_self_cleanup_dir: None,
            dry_run: false,
            install_progress: 0.0,
            status_msg: None,
            deploy_rx: None,
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

impl InstallerState {
    /// 根据外部入口模式与本机探测结果纯函数化初始化向导状态（便于单测完整覆盖）
    pub fn apply_open_mode(
        &mut self,
        launch_mode: InstallerLaunchMode,
        existing: Option<ExistingInstall>,
        cur_exe: Option<&std::path::Path>,
        local_app_data: Option<&std::path::Path>,
    ) {
        self.step = InstallerStep::Options;
        self.install_progress = 0.0;
        self.status_msg = None;
        self.deploy_rx = None;
        self.deferred_self_cleanup_dir = None;
        self.existing = existing.clone();

        match launch_mode {
            InstallerLaunchMode::Uninstall => {
                self.mode = InstallerMode::Uninstall;
                if let Some(ex) = &existing {
                    self.target_dir = ex.install_dir.clone();
                } else if let Some(parent) = cur_exe.and_then(|p| p.parent()) {
                    if !parent.as_os_str().is_empty() {
                        self.target_dir = parent.to_string_lossy().to_string();
                    }
                }
            }
            InstallerLaunchMode::Repair => {
                self.mode = InstallerMode::Repair;
                if let Some(ex) = &existing {
                    self.target_dir = ex.install_dir.clone();
                }
                // 修复模式默认勾选重建 PATH 与快捷方式以保证完整修复体验
                self.add_path = true;
                self.desktop_shortcut = true;
            }
            InstallerLaunchMode::Auto => {
                if let Some(ex) = &existing {
                    self.mode = InstallerMode::Maintenance;
                    self.target_dir = ex.install_dir.clone();
                    self.add_path = if ex.has_uninstall_reg {
                        ex.has_path_entry
                    } else {
                        true
                    };
                    self.desktop_shortcut = if ex.has_uninstall_reg {
                        ex.has_desktop_shortcut
                    } else {
                        true
                    };
                } else {
                    self.mode = InstallerMode::Install;
                    let is_setup_exe = cur_exe
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
                        .map(|n| n.contains("setup") || n.contains("installer"))
                        .unwrap_or(false);
                    if is_setup_exe {
                        if let Some(lad) = local_app_data {
                            self.target_dir = lad.join("IDE1.0").to_string_lossy().to_string();
                        }
                    }
                }
            }
        }
    }
}

impl App {
    /// 打开安装 / 维护 / 修复 / 卸载向导（自动探测本机已有安装并切换对应模式）
    pub fn open_installer_wizard(&mut self, launch_mode: InstallerLaunchMode, standalone: bool) {
        self.installer.open = true;
        self.installer.standalone = standalone;
        let existing = if self.installer.dry_run {
            self.installer.existing.clone()
        } else {
            deploy::detect_existing_installation()
        };
        let cur_exe = std::env::current_exe().ok();
        let local_app_data = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
        self.installer.apply_open_mode(
            launch_mode,
            existing,
            cur_exe.as_deref(),
            local_app_data.as_deref(),
        );
    }

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
            pos2(title_x + 122.0, header_rect.center().y),
            vec2(88.0, 22.0),
        );
        let (badge_bg, badge_label) = match self.installer.mode {
            InstallerMode::Install => {
                (Color32::from_rgb(2, 132, 199), crate::i18n::trz("安装向导"))
            }
            InstallerMode::Maintenance => (
                Color32::from_rgb(14, 116, 144),
                crate::i18n::trz("维护向导"),
            ),
            InstallerMode::Repair => (
                Color32::from_rgb(13, 148, 136),
                crate::i18n::trz("修复向导"),
            ),
            InstallerMode::Uninstall => {
                (Color32::from_rgb(220, 38, 38), crate::i18n::trz("卸载向导"))
            }
        };

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

            // 3. 顶部 Header 栏：精工黑曜钛金栏 (#141924) + 主题强调分割线
            let header_bg = Color32::from_rgb(20, 25, 36);
            p.rect_filled(header_rect, 0.0, header_bg);
            p.line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                Stroke::new(1.8, badge_bg),
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

            // 专属低调奢华模式胶囊 ("安装向导" / "维护向导" / "修复向导" / "卸载向导", 支持 i18n)
            paint_comic_hard_shadow_rect(p, wiz_rect, badge_bg, 1.5, vec2(1.5, 1.5));
            p.with_clip_rect(wiz_rect.shrink2(vec2(3.0, 1.0))).text(
                wiz_rect.center(),
                egui::Align2::CENTER_CENTER,
                badge_label,
                egui::FontId::proportional(10.5),
                Color32::WHITE,
            );

            // 右侧版本胶囊 (纯矢量菱形火花 + ASCII 版本号, 杜绝字符星号回退)
            let ver_rect = Rect::from_center_size(
                pos2(header_rect.right() - 86.0, header_rect.center().y),
                vec2(76.0, 22.0),
            );
            paint_comic_hard_shadow_rect(
                p,
                ver_rect,
                Color32::from_rgb(30, 41, 59),
                1.5,
                vec2(1.5, 1.5),
            );
            crate::ui_icons::paint(
                p,
                Rect::from_center_size(
                    pos2(ver_rect.left() + 14.0, ver_rect.center().y),
                    vec2(10.0, 10.0),
                ),
                crate::ui_icons::UiIcon::Spark,
                Color32::from_rgb(56, 189, 248),
            );
            p.text(
                pos2(ver_rect.center().x + 6.0, ver_rect.center().y),
                egui::Align2::CENTER_CENTER,
                concat!("v", env!("CARGO_PKG_VERSION")),
                egui::FontId::proportional(10.5),
                Color32::from_rgb(203, 213, 225),
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
            if self.installer.mode == InstallerMode::Uninstall
                && self.installer.step == InstallerStep::Finished
            {
                let scheduled =
                    deploy::trigger_deferred_self_cleanup_if_needed(&mut self.installer);
                if scheduled || standalone {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.installer.open = false;
                }
            } else if standalone {
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
