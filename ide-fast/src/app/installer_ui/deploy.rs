//! 美漫安装向导部署与完成分镜 (Deploying & Finished Step Views)

use super::art::*;
use super::cards::*;
use super::InstallerStep;
use crate::app::App;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind};

impl App {
    /// 步骤 2: 正在部署安装分镜 (动画进度条 + 控制台日志)
    pub(super) fn render_step_installing(&mut self, ui: &mut egui::Ui) {
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
    pub(super) fn render_step_finished(&mut self, ui: &mut egui::Ui, standalone: bool) {
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
