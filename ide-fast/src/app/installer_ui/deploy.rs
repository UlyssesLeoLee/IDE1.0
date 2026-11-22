//! 美漫安装/修复/卸载向导部署与完成分镜 (Deploying, Repairing, Uninstalling & Finished Views)

use super::art::*;
use super::cards::*;
use super::{ExistingInstall, InstallerMode, InstallerState, InstallerStep};
use crate::app::App;
use crate::ui_icons::{self, UiIcon};
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke};
use std::path::{Path, PathBuf};

#[cfg(windows)]
const UNINSTALL_REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\IDE1.0";
#[cfg(windows)]
const SHORTCUT_REG_KEY: &str = r"HKCU\Software\ide1\IDE1.0";

const IDE_BIN_FILENAMES: &[&str] = &[
    "IDE1.0.exe",
    "ide-fast.exe",
    "IDE1.0.exe.new",
    "ide-fast.exe.new",
    "IDE1.0.exe.old",
    "ide-fast.exe.old",
    "ide-fast.pdb",
];

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct UninstallFileReport {
    pub removed_files: usize,
    pub deferred_self: bool,
    pub dir_removed: bool,
    pub permission_denied: bool,
}

impl App {
    /// 步骤 2: 正在执行部署 / 修复 / 卸载分镜 (严丝合缝防溢出进度卡片 + 矢量状态流水日志)
    pub(super) fn render_step_installing(&mut self, ui: &mut egui::Ui) {
        let mode = match self.installer.mode {
            InstallerMode::Repair => InstallerMode::Repair,
            InstallerMode::Uninstall => InstallerMode::Uninstall,
            InstallerMode::Install | InstallerMode::Maintenance => InstallerMode::Install,
        };

        let (
            accent_primary,
            accent_bright,
            accent_glow,
            hero_title,
            hero_sub,
            log_title,
            finalize_label,
            stages,
        ) = match mode {
            InstallerMode::Install | InstallerMode::Maintenance => (
                Color32::from_rgb(2, 132, 199),
                Color32::from_rgb(56, 189, 248),
                Color32::from_rgb(125, 211, 252),
                crate::i18n::trz("正在部署 IDE 1.0 (DEPLOYING PACKAGES)"),
                crate::i18n::trz("纯 Rust 原生释放，解压核心微内核与环境配置文件..."),
                "DEPLOYMENT LOG",
                crate::i18n::trz("即将完成安装..."),
                [
                    (
                        0.10_f32,
                        crate::i18n::trz("校验本地 CPU 硬件加速扩展指令集 (AVX2/SSE4.2)..."),
                    ),
                    (
                        0.35_f32,
                        crate::i18n::trz("释放 IDE1.0.exe 原生极速微内核至目标目录..."),
                    ),
                    (
                        0.60_f32,
                        crate::i18n::trz("部署 3D 赛璐珞与晶透毛玻璃视觉管线预设..."),
                    ),
                    (
                        0.85_f32,
                        crate::i18n::trz("写入系统环境变量 PATH 与桌面快捷启动入口..."),
                    ),
                    (
                        0.98_f32,
                        crate::i18n::trz("首选参数配置与常驻托盘守护者完成同步初始化！"),
                    ),
                ],
            ),
            InstallerMode::Repair => (
                Color32::from_rgb(13, 148, 136),
                Color32::from_rgb(45, 212, 191),
                Color32::from_rgb(153, 246, 228),
                crate::i18n::trz("正在修复 IDE 1.0 (REPAIRING INSTALLATION)"),
                crate::i18n::trz("正在校验核心二进制完整性并重建快捷方式与系统集成..."),
                "REPAIR LOG",
                crate::i18n::trz("即将完成修复..."),
                [
                    (
                        0.12_f32,
                        crate::i18n::trz("检查运行中的 IDE 1.0 实例并准备安全修复..."),
                    ),
                    (
                        0.36_f32,
                        crate::i18n::trz("校验并重新释放 IDE1.0.exe 核心主程序..."),
                    ),
                    (
                        0.60_f32,
                        crate::i18n::trz("修复 3D 赛璐珞与晶透主题运行时资源..."),
                    ),
                    (
                        0.84_f32,
                        crate::i18n::trz("重建桌面/开始菜单快捷方式与环境变量 PATH..."),
                    ),
                    (
                        0.98_f32,
                        crate::i18n::trz("刷新系统卸载注册表与托盘守护配置..."),
                    ),
                ],
            ),
            InstallerMode::Uninstall => (
                Color32::from_rgb(220, 38, 38),
                Color32::from_rgb(248, 113, 113),
                Color32::from_rgb(252, 165, 165),
                crate::i18n::trz("正在卸载 IDE 1.0 (UNINSTALLING)"),
                crate::i18n::trz("正在静默移除已部署的程序文件、快捷方式与注册表记录..."),
                "UNINSTALL LOG",
                crate::i18n::trz("即将完成卸载..."),
                [
                    (
                        0.12_f32,
                        crate::i18n::trz("停止关联的 IDE 1.0 托盘守护与后台实例..."),
                    ),
                    (
                        0.35_f32,
                        crate::i18n::trz("移除桌面与开始菜单快捷方式入口..."),
                    ),
                    (
                        0.60_f32,
                        crate::i18n::trz("从用户环境变量 PATH 移除安装路径..."),
                    ),
                    (
                        0.84_f32,
                        crate::i18n::trz("注销 Windows 应用列表卸载注册表项..."),
                    ),
                    (
                        0.98_f32,
                        crate::i18n::trz("清理目标目录主程序与关联残留文件..."),
                    ),
                ],
            ),
        };

        let hero_h = 54.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(
            hp,
            h_rect,
            Color32::from_rgb(20, 26, 38),
            2.0,
            vec2(3.0, 3.0),
        );
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
            0.0,
            accent_primary,
        );
        let htp = hp.with_clip_rect(h_rect.shrink2(vec2(10.0, 4.0)));
        htp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 18.0),
            egui::Align2::LEFT_CENTER,
            hero_title,
            egui::FontId::proportional(14.0),
            Color32::from_rgb(241, 245, 249),
        );
        htp.text(
            pos2(h_rect.left() + 14.0, h_rect.top() + 38.0),
            egui::Align2::LEFT_CENTER,
            hero_sub,
            egui::FontId::proportional(10.5),
            Color32::from_rgb(148, 163, 184),
        );

        ui.add_space(4.0);

        // 基于帧时间平滑推进进度并严格夹紧在 0.0..=1.0
        let dt = ui.input(|i| i.stable_dt).clamp(0.008, 0.05);
        self.installer.install_progress =
            (self.installer.install_progress + dt * 0.55).clamp(0.0, 1.0);
        if self.installer.install_progress < 1.0 {
            ui.ctx().request_repaint();
        }
        let progress = self.installer.install_progress;
        let time = ui.input(|i| i.time) as f32;

        let done_count = stages.iter().filter(|(th, _)| progress >= *th).count();
        let active_stage_label = stages
            .iter()
            .find(|(th, _)| progress < *th)
            .map(|(_, s)| *s)
            .unwrap_or(finalize_label);

        // 主进度卡片 (脉动状态点 + 裁剪阶段标题 + 右对齐百分比 + 严格裁剪圆角进度轨 + 底行路径)
        let card_h = 96.0;
        let (card_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), card_h), egui::Sense::hover());
        let bp = ui.painter();
        paint_comic_hard_shadow_rect(
            bp,
            card_rect,
            Color32::from_rgb(20, 26, 38),
            2.0,
            vec2(3.0, 3.0),
        );
        bp.rect_filled(
            Rect::from_min_size(card_rect.min, vec2(card_rect.width(), 3.0)),
            0.0,
            accent_bright,
        );

        let pulse_r = 4.2 + 1.0 * (time * 5.0).sin().abs();
        bp.circle_filled(
            pos2(card_rect.left() + 20.0, card_rect.top() + 22.0),
            pulse_r,
            accent_bright,
        );
        let stage_clip = Rect::from_min_max(
            pos2(card_rect.left() + 30.0, card_rect.top() + 8.0),
            pos2(
                (card_rect.right() - 64.0).max(card_rect.left() + 34.0),
                card_rect.top() + 36.0,
            ),
        );
        bp.with_clip_rect(stage_clip).text(
            pos2(card_rect.left() + 32.0, card_rect.top() + 22.0),
            egui::Align2::LEFT_CENTER,
            active_stage_label,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(226, 232, 240),
        );
        bp.text(
            pos2(card_rect.right() - 16.0, card_rect.top() + 22.0),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}%", progress * 100.0),
            egui::FontId::proportional(16.0),
            accent_bright,
        );

        let track_rect = Rect::from_min_max(
            pos2(card_rect.left() + 16.0, card_rect.top() + 41.0),
            pos2(card_rect.right() - 16.0, card_rect.top() + 59.0),
        );
        bp.rect_filled(track_rect, 9.0, Color32::from_rgb(11, 17, 32));
        bp.rect_stroke(
            track_rect,
            9.0,
            Stroke::new(1.5, Color32::from_rgb(38, 49, 68)),
            egui::StrokeKind::Inside,
        );

        for frac in [0.25_f32, 0.50, 0.75] {
            let tx = track_rect.left() + track_rect.width() * frac;
            bp.line_segment(
                [
                    pos2(tx, track_rect.top() + 3.0),
                    pos2(tx, track_rect.bottom() - 3.0),
                ],
                Stroke::new(1.0, Color32::from_rgb(30, 41, 59)),
            );
        }

        let inner = track_rect.shrink(2.0);
        let fill_w = inner.width() * progress;
        if fill_w > 0.5 {
            let fill_rect = Rect::from_min_size(inner.min, vec2(fill_w, inner.height()));
            let fill_round = (inner.height() * 0.5).min(fill_w * 0.5);
            bp.rect_filled(fill_rect, fill_round, accent_primary);

            let clip_p = bp.with_clip_rect(fill_rect);
            clip_p.rect_filled(
                Rect::from_min_size(
                    fill_rect.min,
                    vec2(fill_rect.width(), fill_rect.height() * 0.42),
                ),
                fill_round,
                Color32::from_rgba_unmultiplied(
                    accent_glow.r(),
                    accent_glow.g(),
                    accent_glow.b(),
                    72,
                ),
            );
            let stripe_step = 18.0_f32;
            let stripe_shift = (time * 48.0) % stripe_step;
            let mut sx = fill_rect.left() - fill_rect.height() - stripe_step + stripe_shift;
            while sx < fill_rect.right() + stripe_step {
                clip_p.line_segment(
                    [
                        pos2(sx, fill_rect.bottom() + 2.0),
                        pos2(sx + fill_rect.height() * 0.85, fill_rect.top() - 2.0),
                    ],
                    Stroke::new(
                        3.0,
                        Color32::from_rgba_unmultiplied(
                            accent_glow.r(),
                            accent_glow.g(),
                            accent_glow.b(),
                            70,
                        ),
                    ),
                );
                sx += stripe_step;
            }
            clip_p.line_segment(
                [
                    pos2(fill_rect.right() - 1.0, fill_rect.top()),
                    pos2(fill_rect.right() - 1.0, fill_rect.bottom()),
                ],
                Stroke::new(2.0, accent_glow),
            );
        }

        let dir_icon_rect = Rect::from_center_size(
            pos2(card_rect.left() + 22.0, card_rect.top() + 78.0),
            vec2(12.0, 12.0),
        );
        ui_icons::paint(
            bp,
            dir_icon_rect,
            UiIcon::Folder,
            Color32::from_rgb(148, 163, 184),
        );
        let dir_clip = Rect::from_min_max(
            pos2(card_rect.left() + 32.0, card_rect.top() + 64.0),
            pos2(
                (card_rect.right() - 64.0).max(card_rect.left() + 36.0),
                card_rect.top() + 92.0,
            ),
        );
        bp.with_clip_rect(dir_clip).text(
            pos2(card_rect.left() + 34.0, card_rect.top() + 78.0),
            egui::Align2::LEFT_CENTER,
            &self.installer.target_dir,
            egui::FontId::monospace(10.5),
            Color32::from_rgb(148, 163, 184),
        );
        bp.text(
            pos2(card_rect.right() - 16.0, card_rect.top() + 78.0),
            egui::Align2::RIGHT_CENTER,
            format!("{done_count} / 5"),
            egui::FontId::monospace(11.0),
            Color32::from_rgb(203, 213, 225),
        );

        ui.add_space(4.0);

        let log_h = (ui.available_height() - 4.0).max(210.0);
        let (log_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), log_h), egui::Sense::hover());
        let lp = ui.painter();
        paint_comic_hard_shadow_rect(
            lp,
            log_rect,
            Color32::from_rgb(12, 15, 25),
            2.0,
            vec2(3.0, 3.0),
        );

        let top_bar = Rect::from_min_size(log_rect.min, vec2(log_rect.width(), 24.0));
        lp.rect_filled(top_bar, 0.0, Color32::from_rgb(20, 26, 38));
        lp.line_segment(
            [top_bar.left_bottom(), top_bar.right_bottom()],
            Stroke::new(1.5, Color32::BLACK),
        );
        ui_icons::paint(
            lp,
            Rect::from_center_size(
                pos2(top_bar.left() + 14.0, top_bar.center().y),
                vec2(12.0, 12.0),
            ),
            UiIcon::Terminal,
            accent_bright,
        );
        lp.text(
            pos2(top_bar.left() + 26.0, top_bar.center().y),
            egui::Align2::LEFT_CENTER,
            log_title,
            egui::FontId::monospace(10.0),
            accent_bright,
        );

        let mut ly = log_rect.top() + 46.0;
        let row_step = ((log_rect.height() - 56.0) / 5.0).clamp(26.0, 42.0);
        let row_clip = Rect::from_min_max(
            pos2(log_rect.left() + 34.0, log_rect.top() + 26.0),
            pos2(
                (log_rect.right() - 10.0).max(log_rect.left() + 40.0),
                log_rect.bottom() - 4.0,
            ),
        );
        let rtp = lp.with_clip_rect(row_clip);
        let mut active_shown = false;
        for (thresh, text) in stages {
            let done = progress >= thresh;
            let badge_c = pos2(log_rect.left() + 22.0, ly);
            if done {
                ui_icons::paint(
                    lp,
                    Rect::from_center_size(badge_c, vec2(14.0, 14.0)),
                    UiIcon::CheckCircle,
                    Color32::from_rgb(52, 211, 153),
                );
                rtp.text(
                    pos2(log_rect.left() + 38.0, ly),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(11.5),
                    Color32::from_rgb(209, 250, 229),
                );
            } else if !active_shown {
                active_shown = true;
                lp.circle_stroke(badge_c, 6.0, Stroke::new(1.5, accent_bright));
                lp.circle_filled(badge_c, 2.8, accent_bright);
                rtp.text(
                    pos2(log_rect.left() + 38.0, ly),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(11.5),
                    accent_glow,
                );
            } else {
                lp.circle_filled(badge_c, 3.0, Color32::from_rgb(71, 85, 105));
                rtp.text(
                    pos2(log_rect.left() + 38.0, ly),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(11.5),
                    Color32::from_rgb(100, 116, 139),
                );
            }
            ly += row_step;
        }

        // 进度走满后: dry_run 直接收尾 (零副作用); 真实模式在后台线程执行物理部署，
        // 界面线程仅轮询结果 (不阻塞渲染, 失败信息经 status_msg 显式展示)
        if progress >= 1.0 {
            if self.installer.dry_run {
                self.finish_deploy(mode, DeployOutcome::default());
                return;
            }
            if self.installer.deploy_rx.is_none() {
                let plan = DeployPlan::from_state(&self.installer);
                let (tx, rx) = std::sync::mpsc::channel();
                let ctx = ui.ctx().clone();
                let spawned =
                    std::thread::Builder::new()
                        .name("ide-deploy".into())
                        .spawn(move || {
                            let _ = tx.send(run_deploy_job(mode, &plan));
                            ctx.request_repaint();
                        });
                match spawned {
                    Ok(_) => self.installer.deploy_rx = Some(rx),
                    Err(e) => {
                        self.finish_deploy(
                            mode,
                            DeployOutcome {
                                status_msg: Some(format!(
                                    "{}: {e}",
                                    crate::i18n::trz("无法启动后台部署线程")
                                )),
                                deferred_self_cleanup_dir: None,
                            },
                        );
                        return;
                    }
                }
            }
            let polled = self.installer.deploy_rx.as_ref().map(|rx| rx.try_recv());
            match polled {
                Some(Ok(outcome)) => self.finish_deploy(mode, outcome),
                Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => self.finish_deploy(
                    mode,
                    DeployOutcome {
                        status_msg: Some(
                            crate::i18n::trz("后台部署线程异常退出，请重试").to_string(),
                        ),
                        deferred_self_cleanup_dir: None,
                    },
                ),
                _ => ui
                    .ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50)),
            }
        }
    }

    /// 收尾: 把后台结果落入向导状态，同步托盘并切到完成分镜
    fn finish_deploy(&mut self, mode: InstallerMode, outcome: DeployOutcome) {
        self.installer.deploy_rx = None;
        self.installer.status_msg = outcome.status_msg;
        if outcome.deferred_self_cleanup_dir.is_some() {
            self.installer.deferred_self_cleanup_dir = outcome.deferred_self_cleanup_dir;
        }
        match mode {
            InstallerMode::Install | InstallerMode::Maintenance | InstallerMode::Repair => {
                self.sync_tray_after_install();
            }
            InstallerMode::Uninstall => {
                self.tray = None;
                self.installer.existing = None;
            }
        }
        self.installer.step = InstallerStep::Finished;
    }

    fn sync_tray_after_install(&mut self) {
        self.cfg.tray_resident = self.installer.tray_resident;
        if !self.installer.dry_run {
            self.cfg.save();
            if self.cfg.tray_resident && self.tray.is_none() {
                self.tray = crate::tray::Tray::build(true, false);
            } else if let Some(t) = &self.tray {
                t.set_resident_checked(self.cfg.tray_resident);
            }
        }
    }

    /// 步骤 3: 部署 / 修复 / 卸载完成分镜 (全矢量图标 · 零缺失字符方块)
    pub(super) fn render_step_finished(&mut self, ui: &mut egui::Ui, standalone: bool) {
        match self.installer.mode {
            InstallerMode::Uninstall => self.render_step_finished_uninstall(ui, standalone),
            InstallerMode::Repair => {
                self.render_step_finished_deploy_or_repair(ui, standalone, true)
            }
            InstallerMode::Install | InstallerMode::Maintenance => {
                self.render_step_finished_deploy_or_repair(ui, standalone, false)
            }
        }
    }

    fn render_step_finished_deploy_or_repair(
        &mut self,
        ui: &mut egui::Ui,
        standalone: bool,
        is_repair: bool,
    ) {
        let hero_h = 56.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(
            hp,
            h_rect,
            Color32::from_rgb(20, 26, 38),
            2.0,
            vec2(3.0, 3.0),
        );
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 3.5)),
            0.0,
            if is_repair {
                Color32::from_rgb(20, 184, 166)
            } else {
                Color32::from_rgb(16, 185, 129)
            },
        );
        ui_icons::paint(
            hp,
            Rect::from_center_size(
                pos2(h_rect.left() + 26.0, h_rect.center().y),
                vec2(22.0, 22.0),
            ),
            UiIcon::CheckCircle,
            Color32::from_rgb(52, 211, 153),
        );
        let htp = hp.with_clip_rect(Rect::from_min_max(
            pos2(h_rect.left() + 44.0, h_rect.top() + 2.0),
            pos2(
                (h_rect.right() - 8.0).max(h_rect.left() + 48.0),
                h_rect.bottom() - 2.0,
            ),
        ));
        htp.text(
            pos2(h_rect.left() + 48.0, h_rect.top() + 17.0),
            egui::Align2::LEFT_CENTER,
            if is_repair {
                crate::i18n::trz("修复完成！IDE 1.0 已恢复至最佳状态 (REPAIRED)")
            } else {
                crate::i18n::trz("部署成功！IDE 1.0 已准备就绪！(SUCCESS)")
            },
            egui::FontId::proportional(13.5),
            Color32::from_rgb(241, 245, 249),
        );
        htp.text(
            pos2(h_rect.left() + 48.0, h_rect.top() + 39.0),
            egui::Align2::LEFT_CENTER,
            if is_repair {
                crate::i18n::trz("核心主程序、快捷方式与环境变量均已校验完毕，个人配置完好无损")
            } else {
                crate::i18n::trz(
                    "所有核心依赖与配置已装载完毕，即刻享受极致的纯 Rust 原生开发体验！",
                )
            },
            egui::FontId::proportional(10.2),
            Color32::from_rgb(148, 163, 184),
        );

        ui.add_space(4.0);

        let dir_desc = format!(
            "{}: {}",
            if is_repair {
                crate::i18n::trz("已修复安装目录")
            } else {
                crate::i18n::trz("已绑定目标目录")
            },
            self.installer.target_dir
        );
        comic_info_card(
            ui,
            crate::i18n::trz("安装与工作空间目录"),
            &dir_desc,
            UiIcon::Folder,
            Color32::from_rgb(14, 165, 233),
        );

        // 后台部署若有部分失败 (权限不足 / PATH 未写入等) 必须显式告知，绝不静默吞错
        if let Some(msg) = self.installer.status_msg.clone() {
            comic_info_card(
                ui,
                crate::i18n::trz("部署警告"),
                &msg,
                UiIcon::Warning,
                Color32::from_rgb(245, 158, 11),
            );
        }

        let path_summary = if self.installer.add_path {
            crate::i18n::trz("已添加至全局 PATH (终端键入 IDE1.0 秒级唤醒)")
        } else {
            crate::i18n::trz("未配置全局环境变量 PATH")
        };
        comic_info_card(
            ui,
            crate::i18n::trz("终端命令行集成"),
            path_summary,
            UiIcon::Terminal,
            Color32::from_rgb(168, 85, 247),
        );

        let tray_summary = if self.installer.tray_resident {
            crate::i18n::trz("系统托盘常驻守护已开启 (毫秒级热启动与状态守护)")
        } else {
            crate::i18n::trz("系统托盘守护未启用")
        };
        comic_info_card(
            ui,
            crate::i18n::trz("系统驻留守护"),
            tray_summary,
            UiIcon::Shield,
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let mut launch = self.installer.launch_on_finish;
            if ui
                .checkbox(
                    &mut launch,
                    egui::RichText::new(crate::i18n::trz(
                        "完成后立即启动 IDE 1.0 (Launch IDE 1.0 Now)",
                    ))
                    .strong()
                    .color(Color32::from_rgb(241, 245, 249)),
                )
                .changed()
            {
                self.installer.launch_on_finish = launch;
            }
        });

        ui.add_space(10.0);

        let mut finish_clicked = false;
        ui.horizontal(|ui| {
            let total_w = ui.available_width();
            if comic_action_button_with_icon(
                ui,
                Some(UiIcon::ArrowRight),
                crate::i18n::trz("完成并启动 IDE 1.0 (LAUNCH NOW)"),
                vec2(total_w, 42.0),
                true,
                Color32::from_rgb(16, 185, 129),
            )
            .clicked()
            {
                finish_clicked = true;
            }
        });

        if finish_clicked {
            if standalone {
                if self.installer.launch_on_finish && !self.installer.dry_run {
                    launch_installed_binary(&self.installer.target_dir);
                }
                self.force_exit = true;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.installer.open = false;
                self.cur().tip = "IDE 1.0 配置已成功生效！".into();
            }
        }
    }

    fn render_step_finished_uninstall(&mut self, ui: &mut egui::Ui, standalone: bool) {
        let hero_h = 56.0;
        let (h_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
        let hp = ui.painter();
        paint_comic_hard_shadow_rect(
            hp,
            h_rect,
            Color32::from_rgb(20, 26, 38),
            2.0,
            vec2(3.0, 3.0),
        );
        hp.rect_filled(
            Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 3.5)),
            0.0,
            Color32::from_rgb(16, 185, 129),
        );
        ui_icons::paint(
            hp,
            Rect::from_center_size(
                pos2(h_rect.left() + 26.0, h_rect.center().y),
                vec2(22.0, 22.0),
            ),
            UiIcon::CheckCircle,
            Color32::from_rgb(52, 211, 153),
        );
        let htp = hp.with_clip_rect(Rect::from_min_max(
            pos2(h_rect.left() + 44.0, h_rect.top() + 2.0),
            pos2(
                (h_rect.right() - 8.0).max(h_rect.left() + 48.0),
                h_rect.bottom() - 2.0,
            ),
        ));
        htp.text(
            pos2(h_rect.left() + 48.0, h_rect.top() + 17.0),
            egui::Align2::LEFT_CENTER,
            crate::i18n::trz("卸载完成！IDE 1.0 已从本机移除 (UNINSTALLED)"),
            egui::FontId::proportional(13.5),
            Color32::from_rgb(241, 245, 249),
        );
        htp.text(
            pos2(h_rect.left() + 48.0, h_rect.top() + 39.0),
            egui::Align2::LEFT_CENTER,
            crate::i18n::trz("已安全清理所选程序组件与系统集成项，感谢您的使用"),
            egui::FontId::proportional(10.2),
            Color32::from_rgb(148, 163, 184),
        );

        ui.add_space(4.0);

        let cleanup_desc = if let Some(msg) = &self.installer.status_msg {
            msg.clone()
        } else {
            format!(
                "{}: {}",
                crate::i18n::trz("已清理目标目录"),
                self.installer.target_dir
            )
        };
        comic_info_card(
            ui,
            crate::i18n::trz("程序文件清理"),
            &cleanup_desc,
            UiIcon::Trash,
            Color32::from_rgb(239, 68, 68),
        );

        let integ_desc = if self.installer.remove_shortcuts && self.installer.remove_path_and_reg {
            crate::i18n::trz("已移除所选快捷方式、环境变量与注册表项")
        } else {
            crate::i18n::trz("已按选项保留部分系统集成项")
        };
        comic_info_card(
            ui,
            crate::i18n::trz("快捷方式与环境变量"),
            integ_desc,
            UiIcon::Terminal,
            Color32::from_rgb(168, 85, 247),
        );

        let user_data_desc = if self.installer.remove_user_data {
            crate::i18n::trz("已按选择删除个人配置与会话缓存")
        } else {
            crate::i18n::trz("已为您保留个人偏好设置与会话配置")
        };
        comic_info_card(
            ui,
            crate::i18n::trz("个人配置状态"),
            user_data_desc,
            UiIcon::Shield,
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(10.0);

        let mut finish_clicked = false;
        ui.horizontal(|ui| {
            let total_w = ui.available_width();
            if comic_action_button_with_icon(
                ui,
                Some(UiIcon::CheckCircle),
                crate::i18n::trz("完成并关闭向导 (FINISH)"),
                vec2(total_w, 42.0),
                true,
                Color32::from_rgb(2, 132, 199),
            )
            .clicked()
            {
                finish_clicked = true;
            }
        });

        if finish_clicked {
            let scheduled = trigger_deferred_self_cleanup_if_needed(&mut self.installer);
            if standalone || scheduled {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.installer.open = false;
                self.cur().tip =
                    crate::i18n::trz("卸载完成！IDE 1.0 已从本机移除 (UNINSTALLED)").into();
            }
        }
    }
}

/// 构造无头外部进程启动命令 (Windows 下强制 CREATE_NO_WINDOW，杜绝弹出终端黑框；
/// stdin 恒为 null，绝不继承父进程句柄 —— 与 ADR-003 无头执行不变量一致)
#[cfg(windows)]
pub(super) fn headless_cmd(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(0x0800_0000)
        .stdin(std::process::Stdio::null());
    cmd
}

fn normalize_win_path_key(s: &str) -> String {
    s.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

pub(super) fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    let ka = normalize_win_path_key(&a.to_string_lossy());
    let kb = normalize_win_path_key(&b.to_string_lossy());
    !ka.is_empty() && ka == kb
}

pub(super) fn dir_has_ide_binary(dir: &Path) -> bool {
    dir.join("IDE1.0.exe").is_file() || dir.join("ide-fast.exe").is_file()
}

pub(super) fn path_contains_dir(path_env: &str, target_dir: &str) -> bool {
    let key = normalize_win_path_key(target_dir);
    if key.is_empty() {
        return false;
    }
    path_env
        .split(';')
        .any(|seg| normalize_win_path_key(seg) == key)
}

pub(super) fn add_dir_to_path_str(path_env: &str, target_dir: &str) -> String {
    let clean = target_dir.trim();
    if clean.is_empty() || path_contains_dir(path_env, clean) {
        return path_env.to_string();
    }
    let mut parts: Vec<String> = path_env
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    parts.push(clean.to_string());
    parts.join(";")
}

pub(super) fn remove_dir_from_path_str(path_env: &str, target_dir: &str) -> String {
    let key = normalize_win_path_key(target_dir);
    path_env
        .split(';')
        .map(str::trim)
        .filter(|seg| !seg.is_empty() && (key.is_empty() || normalize_win_path_key(seg) != key))
        .collect::<Vec<_>>()
        .join(";")
}

/// 解析 `reg query ...\Uninstall\IDE1.0` 输出，提取 `(InstallLocation, Option<DisplayVersion>)`
pub(super) fn parse_reg_query_uninstall(raw: &str) -> Option<(String, Option<String>)> {
    let mut install_loc: Option<String> = None;
    let mut display_ver: Option<String> = None;
    for line in raw.lines() {
        let t = line.trim();
        if let Some((name, val)) = t.split_once("REG_SZ") {
            let key = name.trim();
            let v = val.trim();
            if key.eq_ignore_ascii_case("InstallLocation") && !v.is_empty() {
                install_loc = Some(v.to_string());
            } else if key.eq_ignore_ascii_case("DisplayVersion") && !v.is_empty() {
                display_ver = Some(v.to_string());
            }
        }
    }
    install_loc.map(|loc| (loc, display_ver))
}

/// 在给定注册表记录、当前进程路径与候选标准安装目录中探测本机已有安装
pub(super) fn detect_existing_in_paths(
    reg_info: Option<(String, Option<String>)>,
    cur_exe: Option<&Path>,
    candidates: &[PathBuf],
    desktop_lnk: Option<&Path>,
    start_lnk: Option<&Path>,
    path_env: &str,
) -> Option<ExistingInstall> {
    let has_desktop_shortcut = desktop_lnk.is_some_and(|p| p.is_file());
    let has_start_menu_shortcut = start_lnk.is_some_and(|p| p.is_file());

    if let Some((loc, ver)) = reg_info {
        if dir_has_ide_binary(Path::new(&loc)) {
            return Some(ExistingInstall {
                install_dir: loc.clone(),
                installed_version: ver,
                has_desktop_shortcut,
                has_start_menu_shortcut,
                has_path_entry: path_contains_dir(path_env, &loc),
                has_uninstall_reg: true,
            });
        }
    }

    let cur_parent = cur_exe.and_then(|p| p.parent());
    for cand in candidates {
        if dir_has_ide_binary(cand)
            && cur_parent.map_or(true, |parent| !paths_equal_ci(parent, cand))
        {
            let loc = cand.to_string_lossy().to_string();
            return Some(ExistingInstall {
                install_dir: loc.clone(),
                installed_version: None,
                has_desktop_shortcut,
                has_start_menu_shortcut,
                has_path_entry: path_contains_dir(path_env, &loc),
                has_uninstall_reg: false,
            });
        }
    }

    None
}

/// 探测本机是否已安装 IDE 1.0 (非 Windows 平台返回 None；Windows 下无头单次查询)
pub(super) fn detect_existing_installation() -> Option<ExistingInstall> {
    #[cfg(windows)]
    {
        let reg_info = headless_cmd("reg")
            .args(["query", UNINSTALL_REG_KEY])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| parse_reg_query_uninstall(&String::from_utf8_lossy(&o.stdout)));

        let mut candidates = Vec::new();
        if let Some(lad) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(lad).join("IDE1.0"));
        }
        candidates.push(PathBuf::from(r"C:\Program Files\IDE1.0"));

        let desktop_lnk = std::env::var_os("USERPROFILE")
            .map(|p| PathBuf::from(p).join("Desktop").join("IDE1.0.lnk"));
        let start_lnk = std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs\IDE1.0.lnk"));
        let path_env = std::env::var("Path").unwrap_or_default();
        let cur_exe = std::env::current_exe().ok();

        detect_existing_in_paths(
            reg_info,
            cur_exe.as_deref(),
            &candidates,
            desktop_lnk.as_deref(),
            start_lnk.as_deref(),
            &path_env,
        )
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// 仅安全清理目标目录中已知的 IDE 1.0 程序与临时更新文件，绝不递归误删用户其它文件
pub(super) fn remove_ide_files_in_dir(dir: &Path, cur_exe: Option<&Path>) -> UninstallFileReport {
    let mut report = UninstallFileReport::default();
    if !dir.is_dir() {
        return report;
    }

    for name in IDE_BIN_FILENAMES {
        let p = dir.join(name);
        if !p.exists() {
            continue;
        }
        if cur_exe.is_some_and(|cur| paths_equal_ci(cur, &p)) {
            report.deferred_self = true;
            continue;
        }
        match std::fs::remove_file(&p) {
            Ok(()) => report.removed_files += 1,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                report.permission_denied = true;
            }
            Err(_) => {}
        }
    }

    if !report.deferred_self {
        if std::fs::remove_dir(dir).is_ok() {
            report.dir_removed = true;
        }
    }

    report
}

/// 无头拉起目标目录中已安装的 IDE 1.0 主程序
pub(super) fn launch_installed_binary(target_dir: &str) {
    let dest_ide10 = Path::new(target_dir).join("IDE1.0.exe");
    let dest_fast = Path::new(target_dir).join("ide-fast.exe");
    let run_exe = if dest_ide10.exists() {
        dest_ide10
    } else if dest_fast.exists() {
        dest_fast
    } else if let Ok(exe) = std::env::current_exe() {
        exe
    } else {
        dest_ide10
    };
    #[cfg(windows)]
    let _ = headless_cmd(run_exe.to_str().unwrap_or("IDE1.0.exe"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new(run_exe).spawn();
}

#[cfg(windows)]
fn register_uninstall_entry(target_str: &str, dest_str: &str) {
    let uninstall_cmd = format!("\"{dest_str}\" --uninstall");
    let modify_cmd = format!("\"{dest_str}\" --setup");
    for reg_args in [
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "DisplayName",
            "/t",
            "REG_SZ",
            "/d",
            "IDE 1.0",
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "DisplayVersion",
            "/t",
            "REG_SZ",
            "/d",
            env!("CARGO_PKG_VERSION"),
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "Publisher",
            "/t",
            "REG_SZ",
            "/d",
            "IDE1.0 Community",
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "InstallLocation",
            "/t",
            "REG_SZ",
            "/d",
            target_str,
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "DisplayIcon",
            "/t",
            "REG_SZ",
            "/d",
            dest_str,
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "UninstallString",
            "/t",
            "REG_SZ",
            "/d",
            &uninstall_cmd,
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "ModifyPath",
            "/t",
            "REG_SZ",
            "/d",
            &modify_cmd,
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "NoModify",
            "/t",
            "REG_DWORD",
            "/d",
            "0",
            "/f",
        ][..],
        &[
            "add",
            UNINSTALL_REG_KEY,
            "/v",
            "NoRepair",
            "/t",
            "REG_DWORD",
            "/d",
            "0",
            "/f",
        ][..],
    ] {
        let _ = headless_cmd("reg").args(reg_args).output();
    }
}

/// 部署任务快照: 后台线程只持有纯数据，绝不触碰 UI 状态 (界面线程零阻塞)
#[derive(Clone, Debug)]
pub(super) struct DeployPlan {
    pub target_dir: String,
    pub add_path: bool,
    pub desktop_shortcut: bool,
    pub remove_shortcuts: bool,
    pub remove_path_and_reg: bool,
    pub remove_user_data: bool,
}

impl DeployPlan {
    pub(super) fn from_state(inst: &InstallerState) -> Self {
        Self {
            target_dir: inst.target_dir.clone(),
            add_path: inst.add_path,
            desktop_shortcut: inst.desktop_shortcut,
            remove_shortcuts: inst.remove_shortcuts,
            remove_path_and_reg: inst.remove_path_and_reg,
            remove_user_data: inst.remove_user_data,
        }
    }
}

/// 后台部署结果: 由界面线程在收到后落入 `InstallerState`
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeployOutcome {
    pub status_msg: Option<String>,
    pub deferred_self_cleanup_dir: Option<String>,
}

/// 在后台线程执行真实系统部署 / 修复 / 卸载 (调用方保证非 dry_run)
pub(super) fn run_deploy_job(mode: InstallerMode, plan: &DeployPlan) -> DeployOutcome {
    let mut out = DeployOutcome::default();
    match mode {
        InstallerMode::Install | InstallerMode::Maintenance => {
            execute_physical_deployment(plan, &mut out)
        }
        InstallerMode::Repair => execute_physical_repair(plan, &mut out),
        InstallerMode::Uninstall => execute_physical_uninstall(plan, &mut out),
    }
    out
}

/// 静态 PowerShell 脚本: 所有可变数据 (路径 / PID) 一律经 `IDE10_*` 环境变量传入，
/// 脚本文本本身不含任何运行时拼接，从根上消除引号转义与注入面。
#[cfg(windows)]
const PS_STOP_INSTANCES: &str = r#"$t=$env:IDE10_TARGET_KEY;$me=[int]$env:IDE10_SELF_PID;Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Id -ne $me -and $_.Path -and ((Split-Path $_.Path -Leaf) -iin @('IDE1.0.exe','ide-fast.exe')) -and ((Split-Path $_.Path -Parent).TrimEnd('\').ToLowerInvariant() -eq $t) } | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }; Start-Sleep -Milliseconds 250"#;
#[cfg(windows)]
const PS_CREATE_SHORTCUT: &str = r#"$s=(New-Object -COM WScript.Shell).CreateShortcut($env:IDE10_LNK);$s.TargetPath=$env:IDE10_DEST;$s.WorkingDirectory=$env:IDE10_DIR;$s.Description='IDE1.0 - High Performance Editor';$s.Save()"#;
/// 以 UTF-8 十六进制输出当前用户 PATH (避开控制台代码页乱码)
#[cfg(windows)]
const PS_READ_USER_PATH_HEX: &str = r#"$b=[Text.Encoding]::UTF8.GetBytes([string][Environment]::GetEnvironmentVariable('Path','User'));($b | ForEach-Object { $_.ToString('x2') }) -join ''"#;
#[cfg(windows)]
const PS_WRITE_USER_PATH: &str =
    r#"[Environment]::SetEnvironmentVariable('Path',$env:IDE10_NEW_PATH,'User')"#;
#[cfg(windows)]
const PS_DEFERRED_CLEANUP: &str = r#"$d=$env:IDE10_CLEAN_DIR;Wait-Process -Id ([int]$env:IDE10_SELF_PID) -Timeout 10 -ErrorAction SilentlyContinue;Start-Sleep -Milliseconds 200;foreach($n in 'IDE1.0.exe','ide-fast.exe','IDE1.0.exe.new','ide-fast.exe.new','IDE1.0.exe.old','ide-fast.exe.old','ide-fast.pdb'){Remove-Item -LiteralPath (Join-Path $d $n) -Force -ErrorAction SilentlyContinue};if((Test-Path -LiteralPath $d) -and ((Get-ChildItem -LiteralPath $d -Force -ErrorAction SilentlyContinue|Measure-Object).Count -eq 0)){Remove-Item -LiteralPath $d -Force -ErrorAction SilentlyContinue}"#;

/// 解码 UTF-8 十六进制文本 (`PS_READ_USER_PATH_HEX` 输出)
pub(super) fn hex_decode_utf8(hex: &str) -> Option<String> {
    let h = hex.trim();
    if h.len() % 2 != 0 || !h.is_ascii() {
        return None;
    }
    let mut bytes = Vec::with_capacity(h.len() / 2);
    for i in (0..h.len()).step_by(2) {
        bytes.push(u8::from_str_radix(&h[i..i + 2], 16).ok()?);
    }
    String::from_utf8(bytes).ok()
}

/// 运行静态 PowerShell 脚本 (环境变量传参)；成功返回 stdout，失败返回 None
#[cfg(windows)]
fn run_ps_static(script: &'static str, envs: &[(&str, &str)]) -> Option<String> {
    let mut cmd = headless_cmd("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        script,
    ]);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 读-改-写当前用户 PATH: 读取失败时绝不写入，避免清空用户已有 PATH
#[cfg(windows)]
fn update_user_path(edit: impl FnOnce(&str) -> String) -> bool {
    let Some(hex) = run_ps_static(PS_READ_USER_PATH_HEX, &[]) else {
        return false;
    };
    let Some(cur) = hex_decode_utf8(&hex) else {
        return false;
    };
    let new = edit(&cur);
    if new == cur {
        return true;
    }
    run_ps_static(PS_WRITE_USER_PATH, &[("IDE10_NEW_PATH", &new)]).is_some()
}

#[cfg(windows)]
fn stop_other_ide_instances_in_dir(target_dir: &str) {
    let key = normalize_win_path_key(target_dir);
    let pid = std::process::id().to_string();
    let _ = run_ps_static(
        PS_STOP_INSTANCES,
        &[("IDE10_TARGET_KEY", &key), ("IDE10_SELF_PID", &pid)],
    );
}

#[cfg(windows)]
fn create_shortcut(lnk: &Path, dest: &str, dir: &str) {
    let _ = run_ps_static(
        PS_CREATE_SHORTCUT,
        &[
            ("IDE10_LNK", &lnk.to_string_lossy()),
            ("IDE10_DEST", dest),
            ("IDE10_DIR", dir),
        ],
    );
}

#[cfg(windows)]
fn grant_logs_dir_permissions(dir: &Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("icacls")
        .arg(dir.as_os_str())
        .arg("/grant")
        .arg("*S-1-5-32-545:(OI)(CI)M")
        .arg("/T")
        .arg("/Q")
        .creation_flags(0x08000000)
        .output();
}

/// 执行真正的系统物理部署与快捷方式/PATH/注册表配置 (install 与 repair 共用)
fn execute_physical_deployment(plan: &DeployPlan, out: &mut DeployOutcome) {
    #[cfg(windows)]
    stop_other_ide_instances_in_dir(&plan.target_dir);

    let target = Path::new(&plan.target_dir);
    if let Err(e) = std::fs::create_dir_all(target) {
        out.status_msg = Some(format!(
            "{}: {e}",
            crate::i18n::trz("无法创建安装目录（可能需要管理员权限）")
        ));
        return;
    }

    let logs_dir = target.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    #[cfg(windows)]
    {
        grant_logs_dir_permissions(&logs_dir);
    }

    let dest_exe = target.join("IDE1.0.exe");
    if let Ok(cur_exe) = std::env::current_exe() {
        if !paths_equal_ci(&cur_exe, &dest_exe) {
            if let Err(e) = std::fs::copy(&cur_exe, &dest_exe) {
                let hint = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    crate::i18n::trz("无法写入目标程序文件（可能需要管理员权限或文件正被占用）")
                } else {
                    crate::i18n::trz("复制主程序到目标目录失败")
                };
                out.status_msg = Some(format!("{hint}: {e}"));
            }
            let _ = std::fs::copy(&cur_exe, target.join("ide-fast.exe"));
        }
    }
    #[cfg(windows)]
    {
        let dest_str = dest_exe.to_string_lossy().to_string();
        let target_str = target.to_string_lossy().to_string();

        if plan.desktop_shortcut {
            if let Some(user_prof) = std::env::var_os("USERPROFILE") {
                let desk = PathBuf::from(user_prof).join("Desktop").join("IDE1.0.lnk");
                create_shortcut(&desk, &dest_str, &target_str);
            }
        }

        if let Some(app_data) = std::env::var_os("APPDATA") {
            let start_m =
                PathBuf::from(app_data).join(r"Microsoft\Windows\Start Menu\Programs\IDE1.0.lnk");
            create_shortcut(&start_m, &dest_str, &target_str);
        }

        if plan.add_path {
            let ok = update_user_path(|cur| add_dir_to_path_str(cur, &target_str));
            if !ok && out.status_msg.is_none() {
                out.status_msg =
                    Some(crate::i18n::trz("未能更新用户 PATH，请手动添加安装目录").to_string());
            }
        }

        register_uninstall_entry(&target_str, &dest_str);
    }
}

/// 修复安装：先安全停止旧进程，再重部署主程序并重建快捷方式、PATH 与注册表
fn execute_physical_repair(plan: &DeployPlan, out: &mut DeployOutcome) {
    #[cfg(windows)]
    stop_other_ide_instances_in_dir(&plan.target_dir);
    execute_physical_deployment(plan, out);
}

/// 卸载安装：停止其它实例、移除快捷方式、PATH、注册表项与程序二进制（支持可选删配置与自删除延迟清理）
fn execute_physical_uninstall(plan: &DeployPlan, out: &mut DeployOutcome) {
    #[cfg(windows)]
    {
        stop_other_ide_instances_in_dir(&plan.target_dir);

        if plan.remove_shortcuts {
            if let Some(user_prof) = std::env::var_os("USERPROFILE") {
                let desk = PathBuf::from(user_prof).join("Desktop").join("IDE1.0.lnk");
                let _ = std::fs::remove_file(desk);
            }
            if let Some(app_data) = std::env::var_os("APPDATA") {
                let start_m = PathBuf::from(app_data)
                    .join(r"Microsoft\Windows\Start Menu\Programs\IDE1.0.lnk");
                let _ = std::fs::remove_file(start_m);
            }
        }

        if plan.remove_path_and_reg {
            let target = plan.target_dir.clone();
            let ok = update_user_path(|cur| {
                if path_contains_dir(cur, &target) {
                    remove_dir_from_path_str(cur, &target)
                } else {
                    cur.to_string()
                }
            });
            if !ok && out.status_msg.is_none() {
                out.status_msg =
                    Some(crate::i18n::trz("未能更新用户 PATH，请手动移除安装目录").to_string());
            }

            for reg_key in [UNINSTALL_REG_KEY, SHORTCUT_REG_KEY] {
                let _ = headless_cmd("reg").args(["delete", reg_key, "/f"]).output();
            }
        }

        let cur_exe = std::env::current_exe().ok();
        let report = remove_ide_files_in_dir(Path::new(&plan.target_dir), cur_exe.as_deref());
        if report.deferred_self {
            out.deferred_self_cleanup_dir = Some(plan.target_dir.clone());
        }
        if report.permission_denied {
            out.status_msg =
                Some(crate::i18n::trz("部分受保护程序文件需要管理员权限才能删除").to_string());
        }

        if plan.remove_user_data {
            if let Some(cfg_dir) =
                crate::settings::path().and_then(|p| p.parent().map(|d| d.to_path_buf()))
            {
                if cfg_dir.file_name().is_some_and(|n| n == "IDE1.0 Fast") {
                    let _ = std::fs::remove_dir_all(cfg_dir);
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (plan, out);
    }
}

/// 若卸载过程中当前正在运行被删目录内的 exe，在向导关闭退出后无头执行延迟自清理
pub(super) fn trigger_deferred_self_cleanup_if_needed(inst: &mut InstallerState) -> bool {
    if inst.dry_run {
        inst.deferred_self_cleanup_dir = None;
        return false;
    }
    let Some(dir) = inst.deferred_self_cleanup_dir.take() else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::process::Stdio;
        let pid = std::process::id().to_string();
        headless_cmd("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                PS_DEFERRED_CLEANUP,
            ])
            .env("IDE10_CLEAN_DIR", &dir)
            .env("IDE10_SELF_PID", &pid)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        false
    }
}
