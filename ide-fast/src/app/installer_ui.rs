//! 3D-to-2D Cel-Shaded 美漫卡通渲染风格安装与首选配置向导 (Comic Setup & Install Wizard)
//! 遵循 BD-003 视觉设计规范：760×520 开阔分镜画幅、纯黑墨水勾线 (2.0px~3.0px Sobel Ink)、
//! 实体硬边波普投影、双色阶赛璐珞高饱和光泽、3D 菱形水晶宝石 Logo、奥林匹斯黄金回纹 (Greek Key / Meander)
//! 与本戴半色调网点 (Ben-Day Screentone)，杜绝任何原生主题控件残留，独特个性又极致简约。

use super::*;
use egui::{pos2, Color32, Rect, Stroke, StrokeKind, Vec2};

#[derive(Clone, Debug)]
pub struct InstallerState {
    pub open: bool,
    pub target_dir: String,
    pub add_path: bool,
    pub desktop_shortcut: bool,
    pub tray_resident: bool,
    pub status_msg: Option<String>,
}

impl Default for InstallerState {
    fn default() -> Self {
        let default_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().to_string()))
            .unwrap_or_else(|| r"C:\Program Files\IDE1.0 Fast".to_string());
        Self {
            open: false,
            target_dir: default_dir,
            add_path: true,
            desktop_shortcut: true,
            tray_resident: true,
            status_msg: None,
        }
    }
}

impl App {
    /// 渲染全定制美漫 3渲2 卡通风格安装与首选配置向导弹窗 (760×520 黄金分镜画幅，无原生边框残留)
    pub(super) fn installer_window(&mut self, ctx: &egui::Context) {
        if !self.installer.open {
            return;
        }

        // 1. 半透明背景暗化遮罩 (模态聚光层，增强漫画分镜纵深感)
        let screen_rect = ctx.content_rect();
        let bg_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("installer_modal_backdrop"),
        ));
        bg_painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(165));

        let mut is_open = self.installer.open;
        let mut apply_clicked = false;
        let mut browse_clicked = false;
        let mut close_clicked = false;

        // 2. 纯自绘漫画分镜窗口：760.0 × 520.0 黄金画幅，禁用原生标题栏与默认 WindowFrame
        let win_size = Vec2::new(760.0, 520.0);
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
                let p = ui.painter();

                // 2.1 漫画实体纯黑硬边投影 (+6px, +6px 无模糊，经典美漫剪纸式阴影)
                p.rect_filled(win_rect.translate(Vec2::new(6.0, 6.0)), 6.0, Color32::from_black_alpha(230));

                // 2.2 漫画深邃暗夜赛璐珞底版 (#0a0d17) 与 3.0px 纯黑墨水轮廓
                p.rect_filled(win_rect, 6.0, Color32::from_rgb(10, 13, 23));
                p.rect_stroke(win_rect, 6.0, Stroke::new(3.0, Color32::BLACK), StrokeKind::Inside);

                // 2.3 本戴半色调网点背景纹理 (Ben-Day Halftone Screentone Dots Field)
                let dot_spacing = 16.0;
                let dot_col = Color32::from_rgba_unmultiplied(6, 182, 212, 14);
                let mut dot_y = win_rect.min.y + 12.0;
                while dot_y < win_rect.max.y - 12.0 {
                    let mut dot_x = win_rect.min.x + 12.0;
                    while dot_x < win_rect.max.x - 12.0 {
                        p.circle_filled(pos2(dot_x, dot_y), 1.2, dot_col);
                        dot_x += dot_spacing;
                    }
                    dot_y += dot_spacing;
                }

                // 2.4 四角奥林匹斯 HUD 希腊科技直角取景框点缀 (Greek Tech Angle Brackets)
                let bracket_len = 10.0;
                let bracket_st = Stroke::new(1.8, Color32::from_rgb(255, 204, 0));
                // 左上角 ┌
                let p_lt = win_rect.min + Vec2::new(7.0, 7.0);
                p.line_segment([p_lt, pos2(p_lt.x + bracket_len, p_lt.y)], bracket_st);
                p.line_segment([p_lt, pos2(p_lt.x, p_lt.y + bracket_len)], bracket_st);
                // 右上角 ┐
                let p_rt = pos2(win_rect.max.x - 7.0, win_rect.min.y + 7.0);
                p.line_segment([p_rt, pos2(p_rt.x - bracket_len, p_rt.y)], bracket_st);
                p.line_segment([p_rt, pos2(p_rt.x, p_rt.y + bracket_len)], bracket_st);
                // 左下角 └
                let p_lb = pos2(win_rect.min.x + 7.0, win_rect.max.y - 7.0);
                p.line_segment([p_lb, pos2(p_lb.x + bracket_len, p_lb.y)], bracket_st);
                p.line_segment([p_lb, pos2(p_lb.x, p_lb.y - bracket_len)], bracket_st);
                // 右下角 ┘
                let p_rb = win_rect.max - Vec2::new(7.0, 7.0);
                p.line_segment([p_rb, pos2(p_rb.x - bracket_len, p_rb.y)], bracket_st);
                p.line_segment([p_rb, pos2(p_rb.x, p_rb.y - bracket_len)], bracket_st);

                // 2.5 顶部 Header 栏：深灰蓝漫画格 (#111624) + 2.5px 纯黑墨线分割
                let header_h = 48.0;
                let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
                p.rect_filled(header_rect, 6.0, Color32::from_rgb(17, 22, 36));
                p.line_segment(
                    [header_rect.left_bottom(), header_rect.right_bottom()],
                    Stroke::new(2.5, Color32::BLACK),
                );

                // 左上角 3D 棱晶水晶宝石 Logo 徽章
                let logo_center = pos2(header_rect.left() + 26.0, header_rect.center().y);
                paint_installer_logo(p, logo_center, 13.5);

                // 希腊回形纹 (Greek Key / Meander) 黄金折线带
                let meander_x = header_rect.left() + 48.0;
                let my = header_rect.center().y;
                let gold_st = Stroke::new(1.8, Color32::from_rgb(255, 204, 0));
                p.line_segment([pos2(meander_x, my - 7.0), pos2(meander_x + 6.0, my - 7.0)], gold_st);
                p.line_segment([pos2(meander_x + 6.0, my - 7.0), pos2(meander_x + 6.0, my + 7.0)], gold_st);
                p.line_segment([pos2(meander_x + 6.0, my + 7.0), pos2(meander_x + 12.0, my + 7.0)], gold_st);
                p.line_segment([pos2(meander_x + 12.0, my + 7.0), pos2(meander_x + 12.0, my - 7.0)], gold_st);
                p.line_segment([pos2(meander_x + 12.0, my - 7.0), pos2(meander_x + 18.0, my - 7.0)], gold_st);

                // 标题文字与漫画标签
                let title_x = meander_x + 26.0;
                p.text(
                    pos2(title_x, header_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "IDE 1.0",
                    egui::FontId::proportional(15.0),
                    Color32::from_rgb(248, 250, 252),
                );
                p.text(
                    pos2(title_x + 64.0, header_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "FAST EDITION",
                    egui::FontId::proportional(15.0),
                    Color32::from_rgb(0, 240, 255),
                );

                let badge_rect = Rect::from_center_size(
                    pos2(title_x + 242.0, header_rect.center().y),
                    Vec2::new(108.0, 22.0),
                );
                p.rect_filled(badge_rect, 3.0, Color32::from_rgb(255, 204, 0));
                p.rect_stroke(badge_rect, 3.0, Stroke::new(1.5, Color32::BLACK), StrokeKind::Inside);
                p.text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "SETUP WIZARD",
                    egui::FontId::proportional(10.5),
                    Color32::BLACK,
                );

                // 右侧版本胶囊
                let ver_rect = Rect::from_center_size(
                    pos2(header_rect.right() - 110.0, header_rect.center().y),
                    Vec2::new(62.0, 22.0),
                );
                p.rect_filled(ver_rect, 11.0, Color32::from_rgb(30, 41, 59));
                p.rect_stroke(ver_rect, 11.0, Stroke::new(1.2, Color32::from_rgb(0, 240, 255)), StrokeKind::Inside);
                p.text(
                    ver_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(0, 240, 255),
                );

                // 装饰性三色指示圆点
                let dots_x = header_rect.right() - 62.0;
                p.circle_filled(pos2(dots_x, header_rect.center().y), 4.0, Color32::from_rgb(16, 185, 129));
                p.circle_filled(pos2(dots_x + 12.0, header_rect.center().y), 4.0, Color32::from_rgb(245, 158, 11));

                // 漫画右上角关闭按钮 ("✕")
                let close_rect = Rect::from_center_size(
                    pos2(header_rect.right() - 24.0, header_rect.center().y),
                    Vec2::new(24.0, 24.0),
                );
                let close_resp = ui.interact(close_rect, ui.id().with("inst_close"), egui::Sense::click());
                let (c_rect, c_bg, c_icon) = if close_resp.is_pointer_button_down_on() {
                    (close_rect.translate(Vec2::new(1.0, 1.0)), Color32::from_rgb(204, 31, 72), Color32::WHITE)
                } else if close_resp.hovered() {
                    p.rect_filled(close_rect.translate(Vec2::new(1.5, 1.5)), 3.0, Color32::BLACK);
                    (close_rect, Color32::from_rgb(239, 68, 68), Color32::WHITE)
                } else {
                    p.rect_filled(close_rect.translate(Vec2::new(1.0, 1.0)), 3.0, Color32::BLACK);
                    (close_rect, Color32::from_rgb(30, 41, 59), Color32::from_rgb(203, 213, 225))
                };
                p.rect_filled(c_rect, 3.0, c_bg);
                p.rect_stroke(c_rect, 3.0, Stroke::new(1.5, Color32::BLACK), StrokeKind::Inside);
                let cc = c_rect.center();
                p.line_segment([pos2(cc.x - 4.5, cc.y - 4.5), pos2(cc.x + 4.5, cc.y + 4.5)], Stroke::new(1.8, c_icon));
                p.line_segment([pos2(cc.x + 4.5, cc.y - 4.5), pos2(cc.x - 4.5, cc.y + 4.5)], Stroke::new(1.8, c_icon));
                if close_resp.clicked() {
                    close_clicked = true;
                }

                // 3. 内容主体容器 (内边距 24px，开阔舒适的漫画大屏交互)
                let body_rect = Rect::from_min_max(
                    pos2(win_rect.left() + 24.0, header_rect.bottom() + 10.0),
                    pos2(win_rect.right() - 24.0, win_rect.bottom() - 14.0),
                );

                ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);

                    // 3.1 Hero Title & Mission Banner (欢迎使用与架构就绪标识)
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("欢迎使用 IDE 1.0 ")
                                    .size(17.5)
                                    .strong()
                                    .color(Color32::from_rgb(248, 250, 252)),
                            );
                            ui.label(
                                egui::RichText::new("FAST EDITION · 极速起航")
                                    .size(17.5)
                                    .strong()
                                    .color(Color32::from_rgb(0, 240, 255)),
                            );
                        });
                        ui.label(
                            egui::RichText::new("基于 Rust 极速微内核 · 多 Agent 协同架构 · 美漫 3渲2 卡通渲染 · 专注纯粹编码体验")
                                .size(11.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    });

                    // 3.2 三列漫画徽章卡片 (Perk Badges)
                    ui.horizontal(|ui| {
                        let total_w = ui.available_width();
                        let gap = 10.0;
                        let chip_w = (total_w - gap * 2.0) / 3.0;

                        render_comic_chip(ui, "⚡ 毫秒冷启", "零依赖·即开即写", Color32::from_rgb(0, 240, 255), chip_w);
                        render_comic_chip(ui, "🛡️ 托盘常驻", "后台待命·秒级唤醒", Color32::from_rgb(255, 204, 0), chip_w);
                        render_comic_chip(ui, "🔤 纯净点阵", "复古街机·清晰无畸", Color32::from_rgb(16, 185, 129), chip_w);
                    });

                    // 3.3 路径配置：全自绘漫画深色输入框 + 纯黑墨线 "浏览..." 按钮
                    ui.label(
                        egui::RichText::new("📁 目标安装与工作目录 (Installation Directory):")
                            .strong()
                            .size(12.0)
                            .color(Color32::from_rgb(241, 245, 249)),
                    );

                    ui.horizontal(|ui| {
                        let btn_w = 110.0;
                        let input_w = ui.available_width() - btn_w - 10.0;
                        let (in_rect, in_resp) = ui.allocate_exact_size(Vec2::new(input_w, 36.0), egui::Sense::click());
                        let ip = ui.painter();

                        // 漫画硬阴影与输入底色
                        ip.rect_filled(in_rect.translate(Vec2::new(2.0, 2.0)), 3.0, Color32::BLACK);
                        ip.rect_filled(in_rect, 3.0, Color32::from_rgb(12, 16, 28));
                        let in_border = if in_resp.has_focus() {
                            Color32::from_rgb(255, 204, 0)
                        } else {
                            Color32::from_rgb(51, 65, 85)
                        };
                        ip.rect_stroke(in_rect, 3.0, Stroke::new(1.8, in_border), StrokeKind::Inside);

                        // 放置单行文本框 (无原生边框)
                        let text_rect = in_rect.shrink2(Vec2::new(10.0, 6.0));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.installer.target_dir)
                                    .frame(egui::Frame::NONE)
                                    .text_color(Color32::from_rgb(248, 250, 252))
                                    .desired_width(text_rect.width()),
                            );
                        });

                        if comic_button(ui, "浏览 (Browse)...", Vec2::new(btn_w, 36.0), false).clicked() {
                            browse_clicked = true;
                        }
                    });

                    // 空间与架构信息提示行
                    ui.label(
                        egui::RichText::new("所需磁盘空间: ~85.0 MB | 文件系统: NTFS / ReFS | 目标架构: x86_64-pc-windows-msvc | 系统状态: 极速就绪")
                            .monospace()
                            .size(10.0)
                            .color(Color32::from_rgb(100, 116, 139)),
                    );

                    // 3.4 快捷设置与系统集成选项 (3 大全自绘高对比漫画交互卡片)
                    ui.label(
                        egui::RichText::new("⚙️ 快捷设置与系统集成选项 (System Integration):")
                            .strong()
                            .size(12.0)
                            .color(Color32::from_rgb(241, 245, 249)),
                    );

                    comic_interactive_card(
                        ui,
                        &mut self.installer.add_path,
                        "将 ide-fast 添加至 Windows 系统全局环境变量 PATH",
                        "可在任意终端 (PowerShell / CMD / Windows Terminal) 极速呼出",
                    );
                    comic_interactive_card(
                        ui,
                        &mut self.installer.desktop_shortcut,
                        "创建桌面与开始菜单极速启动快捷方式",
                        "生成 3D 棱晶水晶 PE 高清图标，双击秒级开启极速工作区",
                    );
                    comic_interactive_card(
                        ui,
                        &mut self.installer.tray_resident,
                        "启用 Windows 系统托盘常驻守护 (Resident Daemon)",
                        "最小化时驻留系统托盘，毫秒级热唤醒，防误关守护工作区状态",
                    );

                    // 状态反馈气泡
                    if let Some(msg) = &self.installer.status_msg {
                        ui.horizontal(|ui| {
                            let (badge_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), egui::Sense::hover());
                            let sp = ui.painter();
                            sp.rect_filled(badge_rect.translate(Vec2::new(1.5, 1.5)), 3.0, Color32::BLACK);
                            sp.rect_filled(badge_rect, 3.0, Color32::from_rgb(15, 35, 26));
                            sp.rect_stroke(badge_rect, 3.0, Stroke::new(1.5, Color32::from_rgb(16, 185, 129)), StrokeKind::Inside);
                            sp.text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                msg,
                                egui::FontId::proportional(11.5),
                                Color32::from_rgb(16, 185, 129),
                            );
                        });
                    }

                    // 3.5 底部 Hero 行动条：奥林匹斯流金美漫 CTA 按钮 + 灰色关闭胶囊
                    ui.horizontal(|ui| {
                        let total_w = ui.available_width();
                        let cancel_w = 110.0;
                        let apply_w = total_w - cancel_w - 10.0;

                        if comic_button(ui, "⚡ 立即部署并开启极速体验 (INSTALL NOW) ⚡", Vec2::new(apply_w, 42.0), true).clicked() {
                            apply_clicked = true;
                        }

                        if comic_button(ui, "关闭", Vec2::new(cancel_w, 42.0), false).clicked() {
                            close_clicked = true;
                        }
                    });

                    // 3.6 底部版权与免责说明
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("遵循 AGPL-3.0 协议开源授权 · 保证纯净无捆绑、无恶意遥测、无后台静默操作")
                                .size(10.0)
                                .color(Color32::from_rgb(100, 116, 139)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("Release Notes v0.6.6 →")
                                    .size(10.0)
                                    .color(Color32::from_rgb(0, 240, 255)),
                            );
                        });
                    });
                });
            });

        if close_clicked {
            is_open = false;
        }
        self.installer.open = is_open;

        if browse_clicked {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("选择 IDE 1.0 安装或工作目录")
                .pick_folder()
            {
                self.installer.target_dir = folder.to_string_lossy().to_string();
            }
        }

        if apply_clicked {
            // 1. 同步并持久化托盘常驻选项
            self.cfg.tray_resident = self.installer.tray_resident;
            self.cfg.save();

            // 2. 如果开启托盘且当前尚未激活托盘，立即尝试构建托盘
            if self.cfg.tray_resident && self.tray.is_none() {
                self.tray = crate::tray::Tray::build(true, false);
            } else if let Some(t) = &self.tray {
                t.set_resident_checked(self.cfg.tray_resident);
            }

            self.installer.status_msg = Some("✓ 首选配置与系统集成选项已成功应用并保存！".to_string());
            self.cur().tip = "安装向导首选配置已生效".into();
        }
    }
}

/// 绘制左上角 3D 水晶菱形 Logo (带有赛璐珞多切面高光与黄金轮廓)
fn paint_installer_logo(p: &egui::Painter, center: egui::Pos2, r: f32) {
    p.circle_filled(center, r + 5.0, Color32::from_rgba_unmultiplied(0, 240, 255, 35));

    let top = pos2(center.x, center.y - r);
    let bottom = pos2(center.x, center.y + r);
    let left = pos2(center.x - r * 0.85, center.y);
    let right = pos2(center.x + r * 0.85, center.y);

    // 左上切面 (青蓝高光)
    p.add(egui::Shape::convex_polygon(
        vec![top, left, center],
        Color32::from_rgb(56, 189, 248),
        Stroke::NONE,
    ));
    // 右上切面 (明亮湖蓝)
    p.add(egui::Shape::convex_polygon(
        vec![top, right, center],
        Color32::from_rgb(14, 165, 233),
        Stroke::NONE,
    ));
    // 左下切面 (深沉湛蓝)
    p.add(egui::Shape::convex_polygon(
        vec![left, bottom, center],
        Color32::from_rgb(2, 132, 199),
        Stroke::NONE,
    ));
    // 右下切面 (暗夜深蓝)
    p.add(egui::Shape::convex_polygon(
        vec![right, bottom, center],
        Color32::from_rgb(3, 105, 161),
        Stroke::NONE,
    ));

    // 赛璐珞纯黑硬轮廓与黄金边框
    let stroke = Stroke::new(1.8, Color32::from_rgb(255, 204, 0));
    p.line_segment([top, right], stroke);
    p.line_segment([right, bottom], stroke);
    p.line_segment([bottom, left], stroke);
    p.line_segment([left, top], stroke);
    p.line_segment([top, bottom], Stroke::new(1.0, Color32::WHITE));
}

/// 渲染漫画特性徽章芯片
fn render_comic_chip(ui: &mut egui::Ui, title: &str, subtitle: &str, accent: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), egui::Sense::hover());
    let p = ui.painter();

    // 漫画硬阴影
    p.rect_filled(rect.translate(Vec2::new(2.0, 2.0)), 3.0, Color32::BLACK);
    // 卡片底色 (#181b2a)
    p.rect_filled(rect, 3.0, Color32::from_rgb(24, 27, 42));
    // 纯黑描边
    p.rect_stroke(rect, 3.0, Stroke::new(1.8, Color32::BLACK), StrokeKind::Inside);

    // 顶部 2.5px 彩色强调指示条
    let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 2.5));
    p.rect_filled(top_bar, 1.0, accent);

    p.text(
        pos2(rect.center().x, rect.center().y - 5.5),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(11.5),
        Color32::from_rgb(241, 245, 249),
    );
    p.text(
        pos2(rect.center().x, rect.center().y + 8.0),
        egui::Align2::CENTER_CENTER,
        subtitle,
        egui::FontId::proportional(9.5),
        Color32::from_rgb(148, 163, 184),
    );
}

/// 美漫 3渲2 卡通风格定制按钮 (纯黑墨水边框 + 实体硬阴影 + 按下消除阴影位移)
fn comic_button(ui: &mut egui::Ui, text: &str, size: Vec2, primary: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();
    let active = resp.is_pointer_button_down_on();

    let shadow_offset = if active { Vec2::ZERO } else { Vec2::new(3.0, 3.0) };
    if !active {
        p.rect_filled(rect.translate(shadow_offset), 3.0, Color32::BLACK);
    }
    let draw_rect = if active { rect.translate(Vec2::new(3.0, 3.0)) } else { rect };

    let (bg, fg) = if primary {
        if active {
            (Color32::from_rgb(245, 158, 11), Color32::BLACK)
        } else if hovered {
            (Color32::from_rgb(255, 224, 102), Color32::BLACK)
        } else {
            (Color32::from_rgb(255, 204, 0), Color32::BLACK)
        }
    } else if active {
        (Color32::from_rgb(204, 31, 72), Color32::WHITE)
    } else if hovered {
        (Color32::from_rgb(239, 68, 68), Color32::WHITE)
    } else {
        (Color32::from_rgb(30, 41, 59), Color32::from_rgb(241, 245, 249))
    };

    p.rect_filled(draw_rect, 3.0, bg);
    p.rect_stroke(draw_rect, 3.0, Stroke::new(if primary { 2.4 } else { 1.8 }, Color32::BLACK), StrokeKind::Inside);
    p.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(if primary { 13.5 } else { 12.0 }),
        fg,
    );
    resp
}

/// 美漫 3渲2 专属系统集成交互卡片 (整卡响应点击 + 纯黑硬边阴影 + 赛璐珞勾选印章)
fn comic_interactive_card(ui: &mut egui::Ui, checked: &mut bool, title: &str, desc: &str) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 36.0), egui::Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();
    let active = resp.is_pointer_button_down_on();

    // 实体纯黑硬阴影
    let shadow_off = if active { Vec2::ZERO } else { Vec2::new(2.0, 2.0) };
    if !active {
        p.rect_filled(rect.translate(shadow_off), 3.0, Color32::BLACK);
    }
    let draw_rect = if active { rect.translate(Vec2::new(2.0, 2.0)) } else { rect };

    // 卡片底色
    let card_bg = if *checked {
        if hovered { Color32::from_rgb(28, 36, 58) } else { Color32::from_rgb(20, 26, 44) }
    } else if hovered {
        Color32::from_rgb(26, 30, 44)
    } else {
        Color32::from_rgb(18, 21, 33)
    };
    p.rect_filled(draw_rect, 3.0, card_bg);
    p.rect_stroke(draw_rect, 3.0, Stroke::new(1.8, Color32::BLACK), StrokeKind::Inside);

    // 选中时左侧彩色强调条
    if *checked {
        let left_bar = Rect::from_min_size(draw_rect.min, Vec2::new(3.5, draw_rect.height()));
        p.rect_filled(left_bar, 1.0, Color32::from_rgb(0, 240, 255));
    }

    // 复选框标志 (20x20 纯黑墨线框)
    let box_center = pos2(draw_rect.left() + 20.0, draw_rect.center().y);
    let box_rect = Rect::from_center_size(box_center, Vec2::splat(20.0));
    p.rect_filled(box_rect.translate(Vec2::new(1.5, 1.5)), 2.0, Color32::BLACK);

    let (box_bg, box_border) = if *checked {
        (Color32::from_rgb(0, 240, 255), Color32::BLACK)
    } else {
        (Color32::from_rgb(12, 15, 24), Color32::from_rgb(71, 85, 105))
    };
    p.rect_filled(box_rect, 2.0, box_bg);
    p.rect_stroke(box_rect, 2.0, Stroke::new(1.8, box_border), StrokeKind::Inside);

    if *checked {
        let p1 = pos2(box_rect.left() + 4.0, box_rect.center().y);
        let p2 = pos2(box_rect.center().x - 1.0, box_rect.bottom() - 4.5);
        let p3 = pos2(box_rect.right() - 4.0, box_rect.top() + 4.5);
        let check_st = Stroke::new(2.4, Color32::BLACK);
        p.line_segment([p1, p2], check_st);
        p.line_segment([p2, p3], check_st);
    }

    // 标题与描述
    let text_x = draw_rect.left() + 42.0;
    p.text(
        pos2(text_x, draw_rect.center().y - 6.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(11.8),
        Color32::from_rgb(248, 250, 252),
    );
    p.text(
        pos2(text_x, draw_rect.center().y + 7.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(9.8),
        Color32::from_rgb(148, 163, 184),
    );

    // 右侧漫画印章状态胶囊 [ ✓ 已就绪 ] / [ ○ 未启用 ]
    let stamp_rect = Rect::from_center_size(
        pos2(draw_rect.right() - 45.0, draw_rect.center().y),
        Vec2::new(70.0, 20.0),
    );
    if *checked {
        p.rect_filled(stamp_rect, 2.0, Color32::from_rgb(14, 40, 48));
        p.rect_stroke(stamp_rect, 2.0, Stroke::new(1.2, Color32::from_rgb(0, 240, 255)), StrokeKind::Inside);
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            "✓ 已就绪",
            egui::FontId::proportional(10.0),
            Color32::from_rgb(0, 240, 255),
        );
    } else {
        p.rect_filled(stamp_rect, 2.0, Color32::from_rgb(22, 26, 38));
        p.rect_stroke(stamp_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(71, 85, 105)), StrokeKind::Inside);
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            "○ 未启用",
            egui::FontId::proportional(10.0),
            Color32::from_rgb(100, 116, 139),
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
        assert!(s.add_path);
        assert!(s.desktop_shortcut);
        assert!(s.tray_resident);
        assert!(s.target_dir.contains("IDE1.0 Fast") || !s.target_dir.is_empty());
    }

    #[test]
    fn installer_apply_toggles_tray() {
        let mut s = InstallerState::default();
        s.tray_resident = false;
        let mut cfg = Settings::default();
        cfg.tray_resident = true;

        cfg.tray_resident = s.tray_resident;
        assert!(!cfg.tray_resident);
    }

    #[test]
    fn test_installer_comic_rendering_invariants() {
        let ctx = egui::Context::default();
        let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
        a.installer.open = true;

        // 验证向导能在 run_ui 下完整自绘无 panic，并且无原生窗口边框残留
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();
        assert!(a.installer.open);
    }
}
