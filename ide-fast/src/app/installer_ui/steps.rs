//! 美漫安装向导欢迎与选项分镜 (Welcome & Options Step Views)
//! 排版优化：消除右下角空旷，字号加粗放大，对比度显著强化，去欢迎页直达核心配置。

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
            ui.spacing_mut().item_spacing = vec2(0.0, 10.0);

            // 步骤进度指示条 (Step Indicator Capsules - 3步直达)
            let (ind_rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 28.0), egui::Sense::hover());
            paint_step_indicator(ui.painter(), ind_rect, self.installer.step);

            match self.installer.step {
                InstallerStep::Welcome | InstallerStep::Options => {
                    self.render_step_options(ui, standalone)
                }
                InstallerStep::Installing => self.render_step_installing(ui),
                InstallerStep::Finished => self.render_step_finished(ui, standalone),
            }
        });
    }

    /// 核心步骤: 选项配置分镜 (排版充盈均整 · 粗体高对比 · 无右下角空旷)
    fn render_step_options(&mut self, ui: &mut egui::Ui, standalone: bool) {
        let hero_h = 56.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(hp, h_rect, Color32::from_rgb(20, 26, 38), 2.0, vec2(3.0, 3.0));
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
            0.0,
            Color32::from_rgb(2, 132, 199),
        );
        hp.text(
            pos2(h_rect.left() + 16.0, h_rect.top() + 18.0),
            egui::Align2::LEFT_CENTER,
            "环境配置与安装参数 (CONFIGURATION)",
            egui::FontId::proportional(14.5),
            Color32::WHITE,
        );
        hp.text(
            pos2(h_rect.left() + 16.0, h_rect.top() + 40.0),
            egui::Align2::LEFT_CENTER,
            "自定义目标安装路径与环境联动，保障纯净绿色的极速微内核体验",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(203, 213, 225),
        );

        ui.add_space(2.0);

        // 路径设置标题
        ui.label(
            egui::RichText::new("📁 目标安装与工作目录 (Installation Directory):")
                .strong()
                .size(12.5)
                .color(Color32::WHITE),
        );

        let mut browse_clicked = false;
        ui.horizontal(|ui| {
            let btn_w = 124.0;
            let input_w = ui.available_width() - btn_w - 10.0;
            let (in_rect, in_resp) =
                ui.allocate_exact_size(vec2(input_w, 38.0), egui::Sense::click());
            let ip = ui.painter();

            paint_comic_hard_shadow_rect(
                ip,
                in_rect,
                Color32::from_rgb(20, 24, 40),
                2.0,
                vec2(2.5, 2.5),
            );
            if in_resp.has_focus() {
                ip.rect_stroke(
                    in_rect,
                    0.0,
                    Stroke::new(2.5, Color32::from_rgb(2, 132, 199)),
                    StrokeKind::Inside,
                );
            }

            let text_rect = in_rect.shrink2(vec2(10.0, 6.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.installer.target_dir)
                        .frame(egui::Frame::NONE)
                        .text_color(Color32::WHITE)
                        .font(egui::FontId::proportional(13.0))
                        .desired_width(text_rect.width()),
                );
            });

            if comic_action_button(
                ui,
                "浏览 (BROWSE)...",
                vec2(btn_w, 38.0),
                false,
                Color32::from_rgb(30, 41, 59),
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
            .size(10.5)
            .color(Color32::from_rgb(180, 195, 215)),
        );

        ui.add_space(2.0);

        // 3 大复选集成卡片 (色彩心理学: 天青/紫罗兰/翡翠绿分级，高度 58px，粗体醒目)
        comic_interactive_card(
            ui,
            &mut self.installer.add_path,
            "将 IDE1.0 添加至全局环境变量 PATH",
            "可在任意终端 (PowerShell / CMD) 键入 ide-fast 秒级呼出",
            "01",
            Color32::from_rgb(14, 165, 233),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.desktop_shortcut,
            "创建桌面与开始菜单极速启动快捷方式",
            "生成正版水晶图标，双击秒开，随时直达工作空间",
            "02",
            Color32::from_rgb(168, 85, 247),
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

        // 底部行动条：铺满宽度，主操作突出，彻底告别右下角空旷
        let mut install_clicked = false;
        let mut cancel_clicked = false;

        ui.horizontal(|ui| {
            let total_w = ui.available_width();
            let cancel_w = 120.0;
            let install_w = total_w - cancel_w - 14.0;

            if comic_action_button(
                ui,
                "立即安装 (INSTALL NOW) ⚡",
                vec2(install_w, 46.0),
                true,
                Color32::from_rgb(2, 132, 199),
            )
            .clicked()
            {
                install_clicked = true;
            }

            if comic_action_button(
                ui,
                "退出向导",
                vec2(cancel_w, 46.0),
                false,
                Color32::from_rgb(30, 41, 59),
            )
            .clicked()
            {
                cancel_clicked = true;
            }
        });

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
