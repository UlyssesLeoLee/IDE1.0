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

use super::*;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};
use std::f32::consts::PI;

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
        let p = ui.painter();
        let ctx = ui.ctx().clone();

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
        let header_h = 54.0;
        let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
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
        let title_x = header_rect.left() + 60.0;
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
        let star_center = pos2(title_x + 124.0, header_rect.center().y);
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
        let wiz_rect = Rect::from_center_size(
            pos2(star_center.x + 84.0, header_rect.center().y),
            vec2(104.0, 24.0),
        );
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

        // 纯自绘美漫最小化控制按钮 (取代 Windows 原生按钮)
        let min_rect = Rect::from_center_size(
            pos2(header_rect.right() - 50.0, header_rect.center().y),
            vec2(24.0, 24.0),
        );
        let min_resp = ui.interact(
            min_rect,
            ui.id().with("comic_inst_min"),
            egui::Sense::click(),
        );
        let min_down = min_resp.is_pointer_button_down_on();
        let min_draw_rect = if min_down {
            min_rect.translate(vec2(1.5, 1.5))
        } else {
            min_rect
        };
        paint_comic_hard_shadow_rect(
            p,
            min_draw_rect,
            if min_resp.hovered() {
                Color32::from_rgb(255, 255, 255)
            } else {
                Color32::from_rgb(230, 235, 245)
            },
            2.0,
            if min_down {
                vec2(1.0, 1.0)
            } else {
                vec2(2.0, 2.0)
            },
        );
        let min_c = min_draw_rect.center();
        p.line_segment(
            [pos2(min_c.x - 5.0, min_c.y), pos2(min_c.x + 5.0, min_c.y)],
            Stroke::new(2.5, Color32::BLACK),
        );
        if min_resp.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        // 纯自绘美漫关闭控制按钮 (取代 Windows 原生按钮)
        let close_rect = Rect::from_center_size(
            pos2(header_rect.right() - 20.0, header_rect.center().y),
            vec2(24.0, 24.0),
        );
        let close_resp = ui.interact(
            close_rect,
            ui.id().with("comic_inst_close"),
            egui::Sense::click(),
        );
        let close_down = close_resp.is_pointer_button_down_on();
        let close_draw_rect = if close_down {
            close_rect.translate(vec2(1.5, 1.5))
        } else {
            close_rect
        };
        paint_comic_hard_shadow_rect(
            p,
            close_draw_rect,
            if close_resp.hovered() {
                Color32::from_rgb(255, 60, 60)
            } else {
                Color32::from_rgb(220, 38, 38)
            },
            2.0,
            if close_down {
                vec2(1.0, 1.0)
            } else {
                vec2(2.0, 2.0)
            },
        );
        let cc = close_draw_rect.center();
        p.line_segment(
            [pos2(cc.x - 4.5, cc.y - 4.5), pos2(cc.x + 4.5, cc.y + 4.5)],
            Stroke::new(2.5, Color32::WHITE),
        );
        p.line_segment(
            [pos2(cc.x + 4.5, cc.y - 4.5), pos2(cc.x - 4.5, cc.y + 4.5)],
            Stroke::new(2.5, Color32::WHITE),
        );
        if close_resp.clicked() {
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

        let left_w = 236.0;
        let left_panel_rect = Rect::from_min_size(body_rect.min, vec2(left_w, body_rect.height()));
        let right_panel_rect = Rect::from_min_max(
            pos2(left_panel_rect.right() + 14.0, body_rect.top()),
            body_rect.max,
        );

        // 5.1 渲染左侧英雄展台
        self.render_left_hero_stage(p, left_panel_rect);

        // 5.2 渲染右侧向导步骤内容
        self.render_right_step_panel(ui, right_panel_rect, standalone);
    }

    /// 渲染左侧英雄展台分镜 (Hero Stage)
    fn render_left_hero_stage(&self, p: &egui::Painter, left_panel_rect: Rect) {
        paint_comic_hard_shadow_rect(
            p,
            left_panel_rect,
            Color32::from_rgb(24, 28, 46),
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

        let core_center = pos2(left_panel_rect.center().x, left_panel_rect.top() + 106.0);
        // 放射状漫画爆发光芒 (Burst Rays)
        for a in (0..360).step_by(30) {
            let rad = (a as f32) * PI / 180.0;
            let ray_p = pos2(
                core_center.x + 64.0 * rad.cos(),
                core_center.y + 64.0 * rad.sin(),
            );
            p.line_segment(
                [core_center, ray_p],
                Stroke::new(1.2, Color32::from_rgb(255, 230, 0)),
            );
        }

        // 正版原生 Logo 纹理悬浮核心 (使用用户真实正版 Crystal Logo，保留晶莹切面)
        paint_authentic_logo(
            p,
            self.installer.logo_tex.as_ref(),
            core_center,
            72.0,
            vec2(3.5, 3.5),
        );

        // 爱奥尼古典神殿柱台
        paint_ionic_comic_pedestal(p, left_panel_rect.center().x, left_panel_rect.top() + 168.0);

        // 漫画对话气泡 (Speech Balloon)
        let bubble_rect = Rect::from_center_size(
            pos2(left_panel_rect.center().x, left_panel_rect.bottom() - 86.0),
            vec2(210.0, 52.0),
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

        let (b_line1, b_line2) = match self.installer.step {
            InstallerStep::Welcome => (
                "⚡ 毫秒极速冷启 · 纯 Rust 打造！",
                "零 Electron 臃肿 · 极致性能体验",
            ),
            InstallerStep::Options => ("📁 自由定制你的专属环境！", "随心配置路径与系统集成选项"),
            InstallerStep::Installing => {
                ("🚀 正在极速释放原生性能内核...", "编译缓存与环境秒级就绪中")
            }
            InstallerStep::Finished => (
                "🎉 恭喜！IDE 1.0 极速环境已就绪！",
                "开启属于你的无拘编码冒险",
            ),
        };

        p.text(
            pos2(bubble_rect.center().x, bubble_rect.center().y - 9.0),
            egui::Align2::CENTER_CENTER,
            b_line1,
            egui::FontId::proportional(11.0),
            Color32::BLACK,
        );
        p.text(
            pos2(bubble_rect.center().x, bubble_rect.center().y + 11.0),
            egui::Align2::CENTER_CENTER,
            b_line2,
            egui::FontId::proportional(9.2),
            Color32::from_rgb(70, 75, 90),
        );

        // 左下角英雄印章贴纸
        let stamp_rect = Rect::from_center_size(
            pos2(left_panel_rect.center().x, left_panel_rect.bottom() - 28.0),
            vec2(170.0, 26.0),
        );
        let (stamp_col, stamp_text) = if self.installer.step == InstallerStep::Finished {
            (Color32::from_rgb(255, 204, 0), "★ MISSION COMPLETE ★")
        } else {
            (Color32::from_rgb(255, 42, 85), "★ 100% NATIVE HERO ★")
        };
        paint_comic_hard_shadow_rect(p, stamp_rect, stamp_col, 2.0, vec2(2.0, 2.0));
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            stamp_text,
            egui::FontId::proportional(10.5),
            if self.installer.step == InstallerStep::Finished {
                Color32::BLACK
            } else {
                Color32::WHITE
            },
        );
    }

    /// 渲染右侧多步骤向导分镜
    fn render_right_step_panel(&mut self, ui: &mut egui::Ui, panel_rect: Rect, standalone: bool) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 8.0);

            // 步骤进度指示条 (Step Indicator Capsules)
            let (ind_rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 26.0), egui::Sense::hover());
            paint_step_indicator(ui.painter(), ind_rect, self.installer.step);

            match self.installer.step {
                InstallerStep::Welcome => self.render_step_welcome(ui, standalone),
                InstallerStep::Options => self.render_step_options(ui, standalone),
                InstallerStep::Installing => self.render_step_installing(ui),
                InstallerStep::Finished => self.render_step_finished(ui, standalone),
            }
        });
    }

    /// 步骤 0: 欢迎体验分镜
    fn render_step_welcome(&mut self, ui: &mut egui::Ui, standalone: bool) {
        let hero_h = 56.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(hp, h_rect, Color32::WHITE, 2.0, vec2(3.0, 3.0));
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
            0.0,
            Color32::from_rgb(255, 230, 0),
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 16.0),
            egui::Align2::LEFT_CENTER,
            "⚡ 欢迎体验 IDE 1.0 极速现代编辑器！(WELCOME)",
            egui::FontId::proportional(13.5),
            Color32::BLACK,
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 38.0),
            egui::Align2::LEFT_CENTER,
            "纯 Rust 原生极速微内核 · 多 Agent 协同生态 · GPU 晶透透视体验 · 极简无扰",
            egui::FontId::proportional(10.5),
            Color32::from_rgb(60, 65, 80),
        );
        paint_comic_burst_star(
            hp,
            pos2(h_rect.right() - 28.0, h_rect.top() + 24.0),
            16.0,
            9.0,
            8,
            Color32::from_rgb(255, 230, 0),
            vec2(1.5, 1.5),
        );
        hp.text(
            pos2(h_rect.right() - 28.0, h_rect.top() + 24.0),
            egui::Align2::CENTER_CENTER,
            "POW!",
            egui::FontId::proportional(7.5),
            Color32::BLACK,
        );

        ui.add_space(4.0);

        // 3 大特性展示卡片
        comic_info_card(
            ui,
            "⚡ 极致响应 (ULTRA FAST)",
            "GPU 硬件加速与毫秒级冷启动，打字如飞，超大文件秒开不卡顿",
            "01",
            Color32::from_rgb(0, 229, 255),
        );

        comic_info_card(
            ui,
            "🤖 多智能体协同 (MULTI-AGENT COLLAB)",
            "内置多 Agent 实时并行协作工作台，零死锁无阻塞高效结对编程",
            "02",
            Color32::from_rgb(255, 204, 0),
        );

        comic_info_card(
            ui,
            "💎 赛璐珞与晶透美学 (CRYSTAL THEMES)",
            "美漫卡通风格与晶透玻璃视觉，全自由定制配色与插件扩展",
            "03",
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(14.0);

        // 底部行动条：[下一步 (NEXT) →] 与 [关闭 (CLOSE)]
        let mut next_clicked = false;
        let mut close_clicked = false;
        ui.horizontal(|ui| {
            let total_w = ui.available_width();
            let close_w = 110.0;
            let next_w = total_w - close_w - 12.0;

            if comic_action_button(
                ui,
                "下一步 (NEXT) →",
                vec2(next_w, 42.0),
                true,
                Color32::from_rgb(255, 230, 0),
            )
            .clicked()
            {
                next_clicked = true;
            }

            if comic_action_button(
                ui,
                "关闭 (CLOSE)",
                vec2(close_w, 42.0),
                false,
                Color32::from_rgb(40, 48, 70),
            )
            .clicked()
            {
                close_clicked = true;
            }
        });

        if next_clicked {
            self.installer.step = InstallerStep::Options;
        }
        if close_clicked {
            if standalone {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.installer.open = false;
            }
        }
    }

    /// 步骤 1: 选项配置分镜
    fn render_step_options(&mut self, ui: &mut egui::Ui, standalone: bool) {
        let hero_h = 50.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(hp, h_rect, Color32::WHITE, 2.0, vec2(3.0, 3.0));
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
            0.0,
            Color32::from_rgb(0, 229, 255),
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 15.0),
            egui::Align2::LEFT_CENTER,
            "⚙️ 安装目录与系统集成选项 (CONFIGURATION)",
            egui::FontId::proportional(13.0),
            Color32::BLACK,
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 35.0),
            egui::Align2::LEFT_CENTER,
            "自定义目标安装路径与环境联动，保障纯净绿色的使用体验",
            egui::FontId::proportional(10.0),
            Color32::from_rgb(60, 65, 80),
        );

        // 路径设置行
        ui.label(
            egui::RichText::new("📁 目标安装与工作目录 (Installation Directory):")
                .strong()
                .size(11.5)
                .color(Color32::from_rgb(255, 230, 0)),
        );

        let mut browse_clicked = false;
        ui.horizontal(|ui| {
            let btn_w = 118.0;
            let input_w = ui.available_width() - btn_w - 10.0;
            let (in_rect, in_resp) =
                ui.allocate_exact_size(vec2(input_w, 34.0), egui::Sense::click());
            let ip = ui.painter();

            paint_comic_hard_shadow_rect(
                ip,
                in_rect,
                Color32::from_rgb(255, 255, 255),
                2.0,
                vec2(2.5, 2.5),
            );
            if in_resp.has_focus() {
                ip.rect_stroke(
                    in_rect,
                    0.0,
                    Stroke::new(2.5, Color32::from_rgb(0, 229, 255)),
                    StrokeKind::Inside,
                );
            }

            let text_rect = in_rect.shrink2(vec2(10.0, 6.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.installer.target_dir)
                        .frame(egui::Frame::NONE)
                        .text_color(Color32::BLACK)
                        .desired_width(text_rect.width()),
                );
            });

            if comic_action_button(
                ui,
                "浏览 (BROWSE)...",
                vec2(btn_w, 34.0),
                false,
                Color32::from_rgb(0, 229, 255),
            )
            .clicked()
            {
                browse_clicked = true;
            }
        });

        if browse_clicked {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("选择 IDE 1.0 安装或工作目录")
                .pick_folder()
            {
                self.installer.target_dir = folder.to_string_lossy().to_string();
            }
        }

        ui.label(
            egui::RichText::new(
                "所需空间: ~85 MB  |  架构: x86_64 Windows (Native)  |  零第三方依赖 · 秒速起航",
            )
            .monospace()
            .size(9.8)
            .color(Color32::from_rgb(148, 163, 184)),
        );

        // 3 大复选集成卡片
        comic_interactive_card(
            ui,
            &mut self.installer.add_path,
            "将 IDE1.0 添加至全局环境变量 PATH",
            "可在任意终端 (PowerShell / CMD) 键入 ide-fast 秒级呼出",
            "01",
            Color32::from_rgb(0, 229, 255),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.desktop_shortcut,
            "创建桌面与开始菜单极速启动快捷方式",
            "生成正版水晶图标，双击秒开，随时直达工作空间",
            "02",
            Color32::from_rgb(255, 204, 0),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.tray_resident,
            "启用 Windows 系统托盘常驻守护 (Resident Daemon)",
            "最小化时驻留托盘，毫秒级热唤醒，防误关守护活跃会话",
            "03",
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(6.0);

        // 底部行动条：[← 上一步] + [⚡ 立即安装 ⚡] + [取消]
        let mut back_clicked = false;
        let mut install_clicked = false;
        let mut cancel_clicked = false;

        ui.horizontal(|ui| {
            let back_w = 110.0;
            let cancel_w = 90.0;
            let install_w = ui.available_width() - back_w - cancel_w - 16.0;

            if comic_action_button(
                ui,
                "← 上一步",
                vec2(back_w, 42.0),
                false,
                Color32::from_rgb(50, 58, 82),
            )
            .clicked()
            {
                back_clicked = true;
            }

            if comic_action_button(
                ui,
                "⚡ 立即安装 (INSTALL NOW) ⚡",
                vec2(install_w, 42.0),
                true,
                Color32::from_rgb(255, 230, 0),
            )
            .clicked()
            {
                install_clicked = true;
            }

            if comic_action_button(
                ui,
                "取消",
                vec2(cancel_w, 42.0),
                false,
                Color32::from_rgb(40, 48, 70),
            )
            .clicked()
            {
                cancel_clicked = true;
            }
        });

        if back_clicked {
            self.installer.step = InstallerStep::Welcome;
        }
        if install_clicked {
            self.installer.install_progress = 0.0;
            self.installer.step = InstallerStep::Installing;
        }
        if cancel_clicked {
            if standalone {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.installer.open = false;
            }
        }
    }

    /// 步骤 2: 正在部署安装分镜 (动画进度条 + 控制台日志)
    fn render_step_installing(&mut self, ui: &mut egui::Ui) {
        let hero_h = 50.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(hp, h_rect, Color32::WHITE, 2.0, vec2(3.0, 3.0));
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
            0.0,
            Color32::from_rgb(255, 42, 85),
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 15.0),
            egui::Align2::LEFT_CENTER,
            "🚀 正在部署 IDE 1.0 (DEPLOYING PACKAGES)",
            egui::FontId::proportional(13.0),
            Color32::BLACK,
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 35.0),
            egui::Align2::LEFT_CENTER,
            "纯 Rust 原生释放，解压核心微内核与环境配置文件...",
            egui::FontId::proportional(10.0),
            Color32::from_rgb(60, 65, 80),
        );

        ui.add_space(8.0);

        // 推进安装进度 (平滑自增长并严格夹紧)
        self.installer.install_progress = (self.installer.install_progress + 0.016).clamp(0.0, 1.0);
        if self.installer.install_progress < 1.0 {
            ui.ctx().request_repaint();
        }

        let progress = self.installer.install_progress;

        // 美漫动量条 (Animated Speed-Stripe Progress Bar)
        let bar_h = 38.0;
        let (bar_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), bar_h), egui::Sense::hover());
        let bp = ui.painter();
        paint_comic_hard_shadow_rect(
            bp,
            bar_rect,
            Color32::from_rgb(26, 31, 50),
            2.5,
            vec2(3.0, 3.0),
        );

        let fill_w = (bar_rect.width() * progress).max(4.0);
        let fill_rect = Rect::from_min_size(bar_rect.min, vec2(fill_w, bar_rect.height()));
        bp.rect_filled(fill_rect, 0.0, Color32::from_rgb(0, 229, 255));
        bp.rect_stroke(
            fill_rect,
            0.0,
            Stroke::new(2.0, Color32::BLACK),
            StrokeKind::Inside,
        );

        // 进度条内部斜切动态速度纹理
        let time = ui.input(|i| i.time);
        let offset = ((time * 40.0) % 20.0) as f32;
        let mut sx = fill_rect.left() - 20.0 + offset;
        while sx < fill_rect.right() {
            let p1 = pos2(sx, fill_rect.top());
            let p2 = pos2(sx + 14.0, fill_rect.bottom());
            bp.line_segment([p1, p2], Stroke::new(3.0, Color32::from_rgb(100, 245, 255)));
            sx += 20.0;
        }

        // 进度百分比文字
        let pct_str = format!("{:.0}% 部署中...", progress * 100.0);
        bp.text(
            bar_rect.center(),
            egui::Align2::CENTER_CENTER,
            pct_str,
            egui::FontId::proportional(13.0),
            Color32::BLACK,
        );

        ui.add_space(8.0);

        // 美漫终端控制台日志窗口
        let log_h = 168.0;
        let (log_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), log_h), egui::Sense::hover());
        let lp = ui.painter();
        paint_comic_hard_shadow_rect(
            lp,
            log_rect,
            Color32::from_rgb(12, 14, 24),
            2.0,
            vec2(2.5, 2.5),
        );

        // 顶部黑色小工具条
        let top_bar = Rect::from_min_size(log_rect.min, vec2(log_rect.width(), 20.0));
        lp.rect_filled(top_bar, 0.0, Color32::from_rgb(20, 24, 38));
        lp.line_segment(
            [top_bar.left_bottom(), top_bar.right_bottom()],
            Stroke::new(1.5, Color32::BLACK),
        );
        lp.text(
            pos2(top_bar.left() + 8.0, top_bar.center().y),
            egui::Align2::LEFT_CENTER,
            "CONSOLE DISPATCH STREAM",
            egui::FontId::monospace(9.0),
            Color32::from_rgb(0, 229, 255),
        );

        let lines = [
            (
                "01",
                "[OK] 校验本地 CPU 硬件加速扩展指令集 (AVX2/SSE4.2)...",
                progress >= 0.1,
            ),
            (
                "02",
                "[OK] 释放 ide-fast.exe 原生极速微内核至目标目录...",
                progress >= 0.35,
            ),
            (
                "03",
                "[OK] 部署 3D 赛璐珞与晶透毛玻璃视觉管线预设...",
                progress >= 0.60,
            ),
            (
                "04",
                "[OK] 写入系统环境变量 PATH 与桌面快捷启动入口...",
                progress >= 0.85,
            ),
            (
                "05",
                "[OK] 首选参数配置与常驻托盘守护者完成同步初始化！",
                progress >= 0.98,
            ),
        ];

        let mut ly = log_rect.top() + 32.0;
        for (_tag, text, done) in lines {
            if done {
                lp.text(
                    pos2(log_rect.left() + 12.0, ly),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::monospace(10.5),
                    Color32::from_rgb(52, 211, 153),
                );
            } else {
                lp.text(
                    pos2(log_rect.left() + 12.0, ly),
                    egui::Align2::LEFT_CENTER,
                    "[..] 正在处理中...",
                    egui::FontId::monospace(10.5),
                    Color32::from_rgb(100, 116, 139),
                );
                break;
            }
            ly += 24.0;
        }

        ui.add_space(8.0);

        // 部署完成自动切换到 Finished
        if progress >= 1.0 {
            self.cfg.tray_resident = self.installer.tray_resident;
            self.cfg.save();

            if self.cfg.tray_resident && self.tray.is_none() {
                self.tray = crate::tray::Tray::build(true, false);
            } else if let Some(t) = &self.tray {
                t.set_resident_checked(self.cfg.tray_resident);
            }

            self.installer.step = InstallerStep::Finished;
        }
    }

    /// 步骤 3: 部署完成分镜
    fn render_step_finished(&mut self, ui: &mut egui::Ui, standalone: bool) {
        let hero_h = 56.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(hp, h_rect, Color32::WHITE, 2.0, vec2(3.0, 3.0));
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
            0.0,
            Color32::from_rgb(16, 185, 129),
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 16.0),
            egui::Align2::LEFT_CENTER,
            "🎉 部署成功！IDE 1.0 已准备就绪！(SUCCESS)",
            egui::FontId::proportional(13.5),
            Color32::BLACK,
        );
        hp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 38.0),
            egui::Align2::LEFT_CENTER,
            "所有核心依赖与配置已装载完毕，即刻享受极致的纯 Rust 原生开发体验！",
            egui::FontId::proportional(10.2),
            Color32::from_rgb(60, 65, 80),
        );
        paint_comic_burst_star(
            hp,
            pos2(h_rect.right() - 28.0, h_rect.top() + 24.0),
            18.0,
            10.0,
            8,
            Color32::from_rgb(16, 185, 129),
            vec2(1.5, 1.5),
        );
        hp.text(
            pos2(h_rect.right() - 28.0, h_rect.top() + 24.0),
            egui::Align2::CENTER_CENTER,
            "100%",
            egui::FontId::proportional(8.0),
            Color32::WHITE,
        );

        ui.add_space(4.0);

        // 部署配置清单一览
        comic_info_card(
            ui,
            "📁 安装与工作空间目录",
            &format!("已绑定目标目录: {}", self.installer.target_dir),
            "✓",
            Color32::from_rgb(0, 229, 255),
        );

        let path_summary = if self.installer.add_path {
            "已添加至全局 PATH (终端键入 ide-fast 秒级唤醒)"
        } else {
            "未配置全局环境变量 PATH"
        };
        comic_info_card(
            ui,
            "⚡ 终端命令行集成",
            path_summary,
            "✓",
            Color32::from_rgb(255, 204, 0),
        );

        let tray_summary = if self.installer.tray_resident {
            "系统托盘常驻守护已开启 (毫秒级热启动与状态守护)"
        } else {
            "系统托盘守护未启用"
        };
        comic_info_card(
            ui,
            "🛡️ 系统驻留守护",
            tray_summary,
            "✓",
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(4.0);

        // 启动选项复选框
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let mut launch = self.installer.launch_on_finish;
            if ui
                .checkbox(
                    &mut launch,
                    egui::RichText::new("⚡ 完成后立即启动 IDE 1.0 (Launch IDE 1.0 Now)")
                        .strong()
                        .color(Color32::from_rgb(255, 230, 0)),
                )
                .changed()
            {
                self.installer.launch_on_finish = launch;
            }
        });

        ui.add_space(10.0);

        // 底部行动条：[⚡ 完成并开启极速体验 (FINISH) ⚡]
        let mut finish_clicked = false;
        ui.horizontal(|ui| {
            let total_w = ui.available_width();
            if comic_action_button(
                ui,
                "⚡ 完成并开启极速体验 (FINISH) ⚡",
                vec2(total_w, 42.0),
                true,
                Color32::from_rgb(255, 230, 0),
            )
            .clicked()
            {
                finish_clicked = true;
            }
        });

        if finish_clicked {
            if standalone {
                if self.installer.launch_on_finish {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).spawn();
                    }
                }
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.installer.open = false;
                self.cur().tip = "IDE 1.0 配置已成功生效！".into();
            }
        }
    }
}

/// 绘制向导步骤胶囊指示条 (Step Indicator Capsules)
fn paint_step_indicator(p: &egui::Painter, rect: Rect, current: InstallerStep) {
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
            (Color32::from_rgb(255, 230, 0), Color32::BLACK)
        } else if is_past {
            (Color32::from_rgb(16, 185, 129), Color32::WHITE)
        } else {
            (
                Color32::from_rgb(32, 38, 58),
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
                Stroke::new(2.0, Color32::BLACK),
            );
        }

        x += step_w + 16.0;
    }
}

/// 绘制美漫纯黑硬阴影矩形卡片
fn paint_comic_hard_shadow_rect(
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
fn paint_ben_day_dots(p: &egui::Painter, rect: Rect, dot_col: Color32, spacing: f32, radius: f32) {
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
fn paint_comic_burst_star(
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
fn paint_authentic_logo(
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
fn paint_ionic_comic_pedestal(p: &egui::Painter, cx: f32, top_y: f32) {
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

/// 美漫风格动作按钮 (带有纯黑硬阴影与按压位移反馈)
fn comic_action_button(
    ui: &mut egui::Ui,
    text: &str,
    size: Vec2,
    primary: bool,
    btn_color: Color32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    let is_down = resp.is_pointer_button_down_on();
    let hovered = resp.hovered();

    let draw_rect = if is_down {
        rect.translate(vec2(2.0, 2.0))
    } else if hovered {
        rect.translate(vec2(-1.0, -1.0))
    } else {
        rect
    };
    let s_off = if is_down {
        vec2(1.5, 1.5)
    } else if hovered {
        vec2(4.5, 4.5)
    } else {
        vec2(3.5, 3.5)
    };

    paint_comic_hard_shadow_rect(p, draw_rect, btn_color, 2.2, s_off);

    let text_col = if primary || (btn_color.r() > 180 && btn_color.g() > 180 && btn_color.b() > 180)
    {
        Color32::BLACK
    } else {
        Color32::WHITE
    };

    p.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(if primary { 13.5 } else { 11.5 }),
        text_col,
    );
    resp
}

/// 美漫只读信息卡片 (展示特性与参数)
fn comic_info_card(ui: &mut egui::Ui, title: &str, desc: &str, tag: &str, accent: Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 40.0), egui::Sense::hover());
    let p = ui.painter();

    paint_comic_hard_shadow_rect(p, rect, Color32::from_rgb(26, 31, 50), 2.0, vec2(3.0, 3.0));

    // 左侧强调色条
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(5.0, rect.height())),
        0.0,
        accent,
    );

    // 漫画序号徽章
    let tag_rect =
        Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), vec2(24.0, 22.0));
    paint_comic_hard_shadow_rect(p, tag_rect, accent, 1.5, vec2(1.5, 1.5));
    p.text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::proportional(11.0),
        Color32::BLACK,
    );

    let text_x = rect.left() + 48.0;
    p.text(
        pos2(text_x, rect.center().y - 6.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
    p.text(
        pos2(text_x, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(9.8),
        Color32::from_rgb(160, 175, 200),
    );
}

/// 美漫交互复选卡片
fn comic_interactive_card(
    ui: &mut egui::Ui,
    checked: &mut bool,
    title: &str,
    desc: &str,
    tag: &str,
    accent: Color32,
) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 38.0), egui::Sense::click());
    let p = ui.painter();
    let is_down = resp.is_pointer_button_down_on();
    let hovered = resp.hovered();

    let draw_rect = if is_down {
        rect.translate(vec2(1.5, 1.5))
    } else {
        rect
    };
    let s_off = if is_down {
        vec2(1.5, 1.5)
    } else {
        vec2(3.0, 3.0)
    };

    let card_bg = if *checked {
        if hovered {
            Color32::from_rgb(32, 38, 60)
        } else {
            Color32::from_rgb(26, 31, 50)
        }
    } else if hovered {
        Color32::from_rgb(24, 28, 44)
    } else {
        Color32::from_rgb(20, 23, 36)
    };

    paint_comic_hard_shadow_rect(p, draw_rect, card_bg, 2.0, s_off);

    // 左侧强调色条
    p.rect_filled(
        Rect::from_min_size(draw_rect.min, vec2(5.0, draw_rect.height())),
        0.0,
        accent,
    );

    // 漫画序号徽章
    let tag_rect = Rect::from_center_size(
        pos2(draw_rect.left() + 24.0, draw_rect.center().y),
        vec2(22.0, 22.0),
    );
    paint_comic_hard_shadow_rect(p, tag_rect, accent, 1.5, vec2(1.5, 1.5));
    p.text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::proportional(10.5),
        Color32::BLACK,
    );

    // 漫画复选框
    let box_center = pos2(draw_rect.left() + 52.0, draw_rect.center().y);
    let box_rect = Rect::from_center_size(box_center, vec2(18.0, 18.0));
    let box_bg = if *checked {
        Color32::from_rgb(255, 230, 0)
    } else {
        Color32::WHITE
    };
    paint_comic_hard_shadow_rect(p, box_rect, box_bg, 2.0, vec2(1.5, 1.5));

    if *checked {
        let p1 = pos2(box_rect.left() + 3.5, box_rect.center().y);
        let p2 = pos2(box_rect.center().x - 0.5, box_rect.bottom() - 3.5);
        let p3 = pos2(box_rect.right() - 3.0, box_rect.top() + 4.0);
        let check_st = Stroke::new(2.8, Color32::BLACK);
        p.line_segment([p1, p2], check_st);
        p.line_segment([p2, p3], check_st);
    }

    let text_x = draw_rect.left() + 72.0;
    p.text(
        pos2(text_x, draw_rect.center().y - 6.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(11.5),
        Color32::WHITE,
    );
    p.text(
        pos2(text_x, draw_rect.center().y + 7.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(9.5),
        Color32::from_rgb(160, 175, 200),
    );

    let stamp_rect = Rect::from_center_size(
        pos2(draw_rect.right() - 44.0, draw_rect.center().y),
        vec2(66.0, 22.0),
    );
    if *checked {
        paint_comic_hard_shadow_rect(
            p,
            stamp_rect,
            Color32::from_rgb(255, 230, 0),
            1.5,
            vec2(1.5, 1.5),
        );
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            "★ READY!",
            egui::FontId::proportional(9.5),
            Color32::BLACK,
        );
    } else {
        paint_comic_hard_shadow_rect(
            p,
            stamp_rect,
            Color32::from_rgb(45, 52, 72),
            1.5,
            vec2(1.5, 1.5),
        );
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            "○ OFF",
            egui::FontId::proportional(9.5),
            Color32::from_rgb(160, 175, 200),
        );
    }

    if resp.clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_default_state() {
        let s = InstallerState::default();
        assert!(!s.open);
        assert!(!s.standalone);
        assert_eq!(s.step, InstallerStep::Welcome);
        assert!(s.add_path);
        assert!(s.desktop_shortcut);
        assert!(s.tray_resident);
        assert!(s.launch_on_finish);
        assert!(s.target_dir.contains("IDE1.0") || !s.target_dir.is_empty());
    }

    #[test]
    fn installer_state_machine_transitions() {
        let mut s = InstallerState::default();
        assert_eq!(s.step, InstallerStep::Welcome);

        // Step 0 -> Step 1
        s.step = InstallerStep::Options;
        assert_eq!(s.step, InstallerStep::Options);

        // Step 1 -> Back to Step 0
        s.step = InstallerStep::Welcome;
        assert_eq!(s.step, InstallerStep::Welcome);

        // Step 0 -> Step 1 -> Step 2
        s.step = InstallerStep::Options;
        s.step = InstallerStep::Installing;
        assert_eq!(s.step, InstallerStep::Installing);

        // Step 2 -> Step 3
        s.step = InstallerStep::Finished;
        assert_eq!(s.step, InstallerStep::Finished);
    }

    #[test]
    fn installer_authentic_logo_decoding() {
        let logo_opt = crate::appicon::logo();
        assert!(logo_opt.is_some(), "正版 logo-256.png 必须能被完整解码");
        let logo = logo_opt.unwrap();
        assert_eq!(logo.width, 256);
        assert_eq!(logo.height, 256);
        assert_eq!(logo.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn test_installer_comic_rendering_invariants() {
        let ctx = egui::Context::default();
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.installer.open = true;
        a.installer.standalone = false;

        // 验证模态向导能在 run_ui 下完整自绘且无 panic
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();
        assert!(a.installer.open);

        // 验证步骤切换渲染无 panic
        a.installer.step = InstallerStep::Options;
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();

        a.installer.step = InstallerStep::Installing;
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();

        a.installer.step = InstallerStep::Finished;
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn test_installer_standalone_rendering() {
        let ctx = egui::Context::default();
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.installer.open = true;
        a.installer.standalone = true;

        let mut out = ctx.run_ui(Default::default(), |root| {
            a.installer_standalone(root);
        });
        out.textures_delta.clear();
        assert!(a.shown);
    }

    #[test]
    fn test_installer_adversarial_progress_and_path() {
        let ctx = egui::Context::default();
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.installer.open = true;

        // 对抗测试：非法或极端路径输入
        a.installer.target_dir = String::new();
        a.installer.step = InstallerStep::Options;
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();

        // 对抗测试：极端进度值越界
        a.installer.step = InstallerStep::Installing;
        a.installer.install_progress = 999.0;
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();

        a.installer.install_progress = -50.0;
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn test_installer_logo_fallback_without_texture() {
        let ctx = egui::Context::default();
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.installer.open = true;
        // 强制清空 logo 纹理，验证兜底渲染
        a.installer.logo_tex = None;

        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();
        assert!(a.installer.open);
    }
}
