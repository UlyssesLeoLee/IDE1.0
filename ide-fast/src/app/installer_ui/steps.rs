//! 美漫安装向导欢迎与选项分镜 (Welcome & Options Step Views)
//! 排版优化：消除右下角空旷，字号加粗放大，对比度显著强化，去欢迎页直达核心配置。

use super::art::*;
use super::cards::*;
use super::{InstallerMode, InstallerStep};
use crate::app::App;
use crate::ui_icons::{self, UiIcon};
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind};
use std::sync::OnceLock;

enum EulaLine {
    Heading(&'static str),
    Subheading(&'static str),
    Quote(&'static str),
    Blank,
    Body(&'static str),
}

fn parsed_eula_lines() -> &'static [EulaLine] {
    static LINES: OnceLock<Vec<EulaLine>> = OnceLock::new();
    LINES.get_or_init(|| {
        super::USER_AGREEMENT_TEXT
            .lines()
            .map(|line| {
                if let Some(rest) = line.strip_prefix("# ") {
                    EulaLine::Heading(rest)
                } else if let Some(rest) = line.strip_prefix("## ") {
                    EulaLine::Subheading(rest)
                } else if let Some(rest) = line.strip_prefix("> ") {
                    EulaLine::Quote(rest)
                } else if line.trim().is_empty() {
                    EulaLine::Blank
                } else {
                    EulaLine::Body(line)
                }
            })
            .collect()
    })
}

impl App {
    /// 渲染右侧多步骤向导分镜
    pub(super) fn render_right_step_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel_rect: Rect,
        standalone: bool,
    ) {
        // 预留 3px 阴影空间，确保卡片投影与进度条严丝合缝且绝不超出右侧底板
        let content_rect = Rect::from_min_max(
            panel_rect.min,
            pos2(
                (panel_rect.right() - 3.0).max(panel_rect.left() + 60.0),
                (panel_rect.bottom() - 3.0).max(panel_rect.top() + 60.0),
            ),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 8.0);

            // 顶部 3 步横向阶段指示进度条 (圆角胶囊 + 居中亮色连线 + 纯矢量徽章)
            let (ind_rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 36.0), egui::Sense::hover());
            paint_step_indicator_for_mode(
                ui.painter(),
                ind_rect,
                self.installer.step,
                self.installer.mode,
            );

            match self.installer.step {
                InstallerStep::Agreement => self.render_step_agreement(ui, standalone),
                InstallerStep::Options => match self.installer.mode {
                    InstallerMode::Install => self.render_step_options(ui, standalone),
                    InstallerMode::Maintenance => self.render_step_maintenance(ui, standalone),
                    InstallerMode::Repair => self.render_step_repair_options(ui, standalone),
                    InstallerMode::Uninstall => self.render_step_uninstall_options(ui, standalone),
                },
                InstallerStep::Installing => self.render_step_installing(ui),
                InstallerStep::Finished => self.render_step_finished(ui, standalone),
            }
        });
    }

    /// 共用语言切换栏 (安装 / 维护 / 修复 / 卸载共用，支持实时切换并持久化语种)
    fn render_language_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui_icons::icon_label(
                ui,
                UiIcon::Globe,
                egui::RichText::new(crate::i18n::trz("界面语言 (Language):"))
                    .strong()
                    .size(12.0)
                    .color(Color32::WHITE),
                Color32::from_rgb(56, 189, 248),
            );
            let cur_lang = crate::i18n::cur();
            for l in crate::i18n::Lang::ALL {
                let is_sel = l == cur_lang;
                let bg = if is_sel {
                    Color32::from_rgb(2, 132, 199)
                } else {
                    Color32::from_rgb(30, 41, 59)
                };
                if comic_action_button(ui, l.native(), vec2(80.0, 30.0), is_sel, bg).clicked() {
                    crate::i18n::set(l);
                    self.cfg.lang = l.code().to_string();
                }
            }
        });
    }

    fn close_wizard(&mut self, ui: &mut egui::Ui, standalone: bool) {
        if standalone {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.installer.open = false;
        }
    }

    /// 共用步骤 1 底部双按钮条 (主操作 + 返回维护/退出向导)
    fn render_step1_action_bar(
        &mut self,
        ui: &mut egui::Ui,
        standalone: bool,
        primary_icon: UiIcon,
        primary_label: &str,
        primary_color: Color32,
    ) -> bool {
        let mut primary_clicked = false;
        let mut secondary_clicked = false;
        let has_existing = self.installer.existing.is_some();
        let secondary_label = if has_existing {
            crate::i18n::trz("返回维护")
        } else {
            crate::i18n::trz("退出向导")
        };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let total_w = ui.available_width();
            let cancel_w = 120.0;
            let primary_w = (total_w - cancel_w - 14.0).max(80.0);

            if comic_action_button_with_icon(
                ui,
                Some(primary_icon),
                primary_label,
                vec2(primary_w, 46.0),
                true,
                primary_color,
            )
            .clicked()
            {
                primary_clicked = true;
            }

            if comic_action_button(
                ui,
                secondary_label,
                vec2(cancel_w, 46.0),
                false,
                Color32::from_rgb(30, 41, 59),
            )
            .clicked()
            {
                secondary_clicked = true;
            }
        });

        if secondary_clicked {
            if has_existing {
                self.installer.mode = InstallerMode::Maintenance;
            } else {
                self.close_wizard(ui, standalone);
            }
        }
        primary_clicked
    }

    /// 步骤 0: 最终用户许可与使用协议分镜 (必须拉到底部并点击同意方可继续)
    fn render_step_agreement(&mut self, ui: &mut egui::Ui, standalone: bool) {
        paint_step_hero_banner(
            ui,
            Color32::from_rgb(2, 132, 199),
            crate::i18n::trz("最终用户许可与使用协议 (EULA)"),
            crate::i18n::trz("请通读下方开源许可与使用条款；必须拉到底部并勾选同意后方可继续安装"),
        );

        ui.add_space(2.0);
        self.render_language_bar(ui);
        ui.add_space(2.0);

        // 状态引导提示胶囊 (未到底部显示警告指示，拉到底部显示就绪指示)
        let (tip_icon, tip_text, tip_accent) = if self.installer.agreement_scrolled_to_bottom {
            (
                UiIcon::CheckCircle,
                crate::i18n::trz(
                    "【已阅读至末尾】您已通读协议，请在下方勾选同意条款以解锁继续按钮",
                ),
                Color32::from_rgb(16, 185, 129),
            )
        } else {
            (
                UiIcon::Warning,
                crate::i18n::trz(
                    "【需阅读至底部】请拖动右侧滚动条或使用鼠标滚轮向下阅读至协议全文底部",
                ),
                Color32::from_rgb(245, 158, 11),
            )
        };

        comic_info_card(
            ui,
            crate::i18n::trz("许可协议阅读进度状态"),
            tip_text,
            tip_icon,
            tip_accent,
        );

        ui.add_space(2.0);

        // 协议正文漫画滚动面板 (带深黑实心阴影与纯黑描边墨水框)
        let w = ui.available_width();
        let scroll_card_h = 220.0;
        let (box_rect, _) = ui.allocate_exact_size(vec2(w, scroll_card_h), egui::Sense::hover());
        let p = ui.painter();
        paint_comic_hard_shadow_rect(
            p,
            box_rect,
            Color32::from_rgb(15, 19, 28),
            2.0,
            vec2(3.0, 3.0),
        );
        p.rect_stroke(
            box_rect,
            0.0,
            Stroke::new(1.8, Color32::from_rgb(45, 55, 75)),
            StrokeKind::Inside,
        );

        let inner_rect = box_rect.shrink2(vec2(10.0, 8.0));
        let mut scrolled_to_end = false;

        ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
            let scroll_out = egui::ScrollArea::vertical()
                .id_salt("eula_scroll_view")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    for line in parsed_eula_lines() {
                        match *line {
                            EulaLine::Heading(rest) => {
                                ui.heading(
                                    egui::RichText::new(rest)
                                        .strong()
                                        .color(Color32::WHITE)
                                        .size(13.5),
                                );
                            }
                            EulaLine::Subheading(rest) => {
                                ui.add_space(3.0);
                                ui.label(
                                    egui::RichText::new(rest)
                                        .strong()
                                        .color(Color32::from_rgb(56, 189, 248))
                                        .size(12.0),
                                );
                            }
                            EulaLine::Quote(rest) => {
                                ui.label(
                                    egui::RichText::new(rest)
                                        .color(Color32::from_rgb(251, 191, 36))
                                        .size(10.5),
                                );
                            }
                            EulaLine::Blank => {
                                ui.add_space(2.0);
                            }
                            EulaLine::Body(text) => {
                                ui.label(
                                    egui::RichText::new(text)
                                        .color(Color32::from_rgb(226, 232, 240))
                                        .size(10.5),
                                );
                            }
                        }
                    }

                    // 底部 Sentinel 哨兵检测
                    let (sentinel_rect, _) = ui
                        .allocate_exact_size(vec2(ui.available_width(), 2.0), egui::Sense::hover());
                    ui.is_rect_visible(sentinel_rect)
                });

            let visible_bottom = scroll_out.inner;
            let offset_bottom = scroll_out.state.offset().y + scroll_out.inner_rect.height()
                >= scroll_out.content_size.y - 15.0;
            if visible_bottom || offset_bottom {
                scrolled_to_end = true;
            }
        });

        if scrolled_to_end {
            self.installer.agreement_scrolled_to_bottom = true;
        }

        ui.add_space(4.0);

        // 交互勾选卡片 (必须已拉到底部才可勾选)
        let unlocked = self.installer.agreement_scrolled_to_bottom;
        comic_agreement_card(
            ui,
            &mut self.installer.agreement_accepted,
            unlocked,
            crate::i18n::trz("我已完整阅读并同意《IDE 1.0 最终用户许可与使用协议》"),
            crate::i18n::trz("确认已理解代码执行风险、本地文件修改权限与责任限制条款"),
            Color32::from_rgb(2, 132, 199),
        );

        ui.add_space(6.0);

        // 底部动作条 (同意并继续 / 拒绝并退出)
        let total_w = ui.available_width();
        let cancel_w = 120.0;
        let primary_w = (total_w - cancel_w - 14.0).max(80.0);

        let can_continue = self.installer.agreement_accepted;
        let primary_col = if can_continue {
            Color32::from_rgb(2, 132, 199)
        } else {
            Color32::from_rgb(30, 41, 59)
        };

        let mut continue_clicked = false;
        let mut exit_clicked = false;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;

            if comic_action_button_with_icon(
                ui,
                Some(UiIcon::CheckCircle),
                crate::i18n::trz("同意并继续 (AGREE & NEXT)"),
                vec2(primary_w, 46.0),
                can_continue,
                primary_col,
            )
            .clicked()
            {
                if can_continue {
                    continue_clicked = true;
                }
            }

            if comic_action_button(
                ui,
                crate::i18n::trz("拒绝并退出"),
                vec2(cancel_w, 46.0),
                false,
                Color32::from_rgb(30, 41, 59),
            )
            .clicked()
            {
                exit_clicked = true;
            }
        });

        if continue_clicked {
            self.installer.step = InstallerStep::Options;
        } else if exit_clicked {
            self.close_wizard(ui, standalone);
        }
    }

    /// 维护总览分镜 (已安装状态下启动安装向导时默认呈现：修复 / 卸载 / 更改配置)
    fn render_step_maintenance(&mut self, ui: &mut egui::Ui, standalone: bool) {
        paint_step_hero_banner(
            ui,
            Color32::from_rgb(2, 132, 199),
            crate::i18n::trz("检测到本机已安装 IDE 1.0 (MAINTENANCE)"),
            crate::i18n::trz("选择修复现有安装、卸载移除组件，或更改目标目录与集成选项"),
        );

        ui.add_space(2.0);
        self.render_language_bar(ui);

        let ver = self
            .installer
            .existing
            .as_ref()
            .and_then(|e| e.installed_version.as_deref())
            .unwrap_or(env!("CARGO_PKG_VERSION"));
        let inst_title = format!("{}  (v{})", crate::i18n::trz("当前已安装实例"), ver);
        comic_info_card(
            ui,
            &inst_title,
            &self.installer.target_dir,
            UiIcon::CheckCircle,
            Color32::from_rgb(16, 185, 129),
        );

        if comic_mode_card(
            ui,
            crate::i18n::trz("修复安装 (Repair Installation)"),
            crate::i18n::trz("重新释放主程序并修复快捷方式、PATH 与注册表，完整保留个人配置"),
            crate::i18n::trz("修复"),
            UiIcon::Wrench,
            Color32::from_rgb(20, 184, 166),
        )
        .clicked()
        {
            self.installer.mode = InstallerMode::Repair;
        }

        if comic_mode_card(
            ui,
            crate::i18n::trz("卸载移除 (Uninstall IDE 1.0)"),
            crate::i18n::trz("移除主程序、快捷方式、环境变量与卸载记录，可选清理个人配置"),
            crate::i18n::trz("卸载"),
            UiIcon::Trash,
            Color32::from_rgb(239, 68, 68),
        )
        .clicked()
        {
            self.installer.mode = InstallerMode::Uninstall;
        }

        if comic_mode_card(
            ui,
            crate::i18n::trz("更改配置或安装目录 (Modify / Reinstall)"),
            crate::i18n::trz("修改安装目标路径或重新配置桌面图标、PATH 与常驻托盘选项"),
            crate::i18n::trz("配置"),
            UiIcon::Folder,
            Color32::from_rgb(14, 165, 233),
        )
        .clicked()
        {
            self.installer.mode = InstallerMode::Install;
        }

        ui.add_space(4.0);

        let mut launch_clicked = false;
        let mut exit_clicked = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let total_w = ui.available_width();
            let exit_w = 120.0;
            let launch_w = (total_w - exit_w - 14.0).max(80.0);

            if comic_action_button_with_icon(
                ui,
                Some(UiIcon::Play),
                crate::i18n::trz("直接启动 IDE 1.0"),
                vec2(launch_w, 46.0),
                true,
                Color32::from_rgb(16, 185, 129),
            )
            .clicked()
            {
                launch_clicked = true;
            }

            if comic_action_button(
                ui,
                crate::i18n::trz("退出向导"),
                vec2(exit_w, 46.0),
                false,
                Color32::from_rgb(30, 41, 59),
            )
            .clicked()
            {
                exit_clicked = true;
            }
        });

        if launch_clicked {
            if !self.installer.dry_run {
                super::deploy::launch_installed_binary(&self.installer.target_dir);
            }
            self.close_wizard(ui, standalone);
        } else if exit_clicked {
            self.close_wizard(ui, standalone);
        }
    }

    /// 修复确认分镜 (保留个人配置，修复核心二进制、快捷方式、PATH 与注册表)
    fn render_step_repair_options(&mut self, ui: &mut egui::Ui, standalone: bool) {
        paint_step_hero_banner(
            ui,
            Color32::from_rgb(13, 148, 136),
            crate::i18n::trz("修复现有安装 (REPAIR INSTALLATION)"),
            crate::i18n::trz("保留您的全部偏好与会话，重新部署核心程序并修复系统集成项"),
        );

        ui.add_space(2.0);
        self.render_language_bar(ui);

        comic_info_card(
            ui,
            crate::i18n::trz("目标修复目录"),
            &self.installer.target_dir,
            UiIcon::Folder,
            Color32::from_rgb(20, 184, 166),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.add_path,
            crate::i18n::trz("校验并修复全局环境变量 PATH"),
            crate::i18n::trz("确保终端中键入 IDE1.0 可直接唤起当前安装目录"),
            "01",
            Color32::from_rgb(14, 165, 233),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.desktop_shortcut,
            crate::i18n::trz("重建桌面与开始菜单快捷方式"),
            crate::i18n::trz("修复损坏或丢失的桌面与开始菜单水晶图标入口"),
            "02",
            Color32::from_rgb(168, 85, 247),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.tray_resident,
            crate::i18n::trz("同步系统托盘常驻守护配置"),
            crate::i18n::trz("恢复最小化驻留托盘与毫秒级热唤醒能力"),
            "03",
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(6.0);

        if self.render_step1_action_bar(
            ui,
            standalone,
            UiIcon::Wrench,
            crate::i18n::trz("开始修复 (REPAIR NOW)"),
            Color32::from_rgb(13, 148, 136),
        ) {
            self.installer.install_progress = 0.0;
            self.installer.status_msg = None;
            self.installer.step = InstallerStep::Installing;
        }
    }

    /// 卸载选项分镜 (停止关联实例，移除主程序/快捷方式/PATH/注册表，可选保留或清除用户数据)
    fn render_step_uninstall_options(&mut self, ui: &mut egui::Ui, standalone: bool) {
        paint_step_hero_banner(
            ui,
            Color32::from_rgb(220, 38, 38),
            crate::i18n::trz("卸载移除 IDE 1.0 (UNINSTALL)"),
            crate::i18n::trz("安全停止关联托盘进程并移除程序与集成项，全程无黑框闪烁"),
        );

        ui.add_space(2.0);
        self.render_language_bar(ui);

        comic_info_card(
            ui,
            crate::i18n::trz("待卸载安装目录"),
            &self.installer.target_dir,
            UiIcon::Warning,
            Color32::from_rgb(245, 158, 11),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.remove_shortcuts,
            crate::i18n::trz("移除桌面与开始菜单快捷方式"),
            crate::i18n::trz("清理 Desktop 与 Start Menu 中的 IDE1.0 启动快捷方式"),
            "01",
            Color32::from_rgb(14, 165, 233),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.remove_path_and_reg,
            crate::i18n::trz("清理环境变量 PATH 与卸载注册表项"),
            crate::i18n::trz("从用户 PATH 移除安装目录并注销 Windows 应用列表记录"),
            "02",
            Color32::from_rgb(168, 85, 247),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.remove_user_data,
            crate::i18n::trz("同时删除个人偏好配置与会话缓存"),
            crate::i18n::trz("清空 IDE1.0 Fast 配置目录（默认关闭以保留您的个人配置）"),
            "03",
            Color32::from_rgb(239, 68, 68),
        );

        ui.add_space(6.0);

        if self.render_step1_action_bar(
            ui,
            standalone,
            UiIcon::Trash,
            crate::i18n::trz("确认卸载 (UNINSTALL NOW)"),
            Color32::from_rgb(220, 38, 38),
        ) {
            self.installer.install_progress = 0.0;
            self.installer.status_msg = None;
            self.installer.step = InstallerStep::Installing;
        }
    }

    /// 核心步骤: 选项配置分镜 (排版充盈均整 · 粗体高对比 · 全矢量图标)
    fn render_step_options(&mut self, ui: &mut egui::Ui, standalone: bool) {
        paint_step_hero_banner(
            ui,
            Color32::from_rgb(2, 132, 199),
            crate::i18n::trz("环境配置与安装参数 (CONFIGURATION)"),
            crate::i18n::trz("自定义目标安装路径与环境联动，保障纯净绿色的极速微内核体验"),
        );

        ui.add_space(2.0);
        self.render_language_bar(ui);
        ui.add_space(2.0);

        // 路径设置标题 (矢量文件夹图标)
        ui_icons::icon_label(
            ui,
            UiIcon::Folder,
            egui::RichText::new(crate::i18n::trz(
                "目标安装与工作目录 (Installation Directory):",
            ))
            .strong()
            .size(12.5)
            .color(Color32::WHITE),
            Color32::from_rgb(56, 189, 248),
        );

        let mut browse_clicked = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let btn_w = 124.0;
            let input_w = (ui.available_width() - btn_w - 10.0).max(60.0);
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
                crate::i18n::trz("浏览 (BROWSE)..."),
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
                .set_title(crate::i18n::trz("选择 IDE 1.0 安装或工作目录"))
                .pick_folder()
            {
                self.installer.target_dir = folder.to_string_lossy().to_string();
            }
        }

        ui.label(
            egui::RichText::new(crate::i18n::trz(
                "所需空间: ~85 MB  |  架构: x86_64 Windows (Native)  |  零第三方依赖 · 秒速起航",
            ))
            .monospace()
            .size(10.5)
            .color(Color32::from_rgb(180, 195, 215)),
        );

        ui.add_space(2.0);

        // 3 大复选集成卡片 (矢量图标 + 圆角内嵌复选开关)
        comic_interactive_card(
            ui,
            &mut self.installer.add_path,
            crate::i18n::trz("将 IDE1.0 添加至全局环境变量 PATH"),
            crate::i18n::trz("可在任意终端 (PowerShell / CMD) 键入 IDE1.0 秒级呼出"),
            "01",
            Color32::from_rgb(14, 165, 233),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.desktop_shortcut,
            crate::i18n::trz("创建桌面与开始菜单极速启动快捷方式"),
            crate::i18n::trz("生成正版水晶图标，双击秒开，随时直达工作空间"),
            "02",
            Color32::from_rgb(168, 85, 247),
        );

        comic_interactive_card(
            ui,
            &mut self.installer.tray_resident,
            crate::i18n::trz("启用 Windows 系统托盘常驻守护 (Resident Daemon)"),
            crate::i18n::trz("最小化时驻留托盘，毫秒级热唤醒，防误关守护活跃会话"),
            "03",
            Color32::from_rgb(16, 185, 129),
        );

        ui.add_space(6.0);

        // 底部行动条：严格 14px 水平间距齐边对齐，主按钮带纯矢量闪电图标
        if self.render_step1_action_bar(
            ui,
            standalone,
            UiIcon::Bolt,
            crate::i18n::trz("立即安装 (INSTALL NOW)"),
            Color32::from_rgb(2, 132, 199),
        ) {
            self.installer.install_progress = 0.0;
            self.installer.status_msg = None;
            self.installer.step = InstallerStep::Installing;
        }
    }
}
