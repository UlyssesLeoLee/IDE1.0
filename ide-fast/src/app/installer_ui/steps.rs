//! 美漫安装向导欢迎与选项分镜 (Welcome & Options Step Views)

use super::art::*;
use super::cards::*;
use super::InstallerStep;
use crate::app::App;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind};

impl App {
    /// 渲染右侧多步骤向导分镜
    pub(super) fn render_right_step_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel_rect: Rect,
        standalone: bool,
    ) {
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
}
