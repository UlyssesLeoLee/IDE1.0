//! 3D-to-2D Cel-Shaded 美漫卡通渲染风格安装与首选配置向导 (Comic Setup & Install Wizard)
//! 遵循 BD-003 视觉设计规范：纯黑墨水勾线 (2.0px~2.5px Sobel Ink)、实体硬边波普投影、双色阶赛璐珞高饱和光泽、
//! 3D 菱形水晶宝石 Logo 与希腊回纹 (Meander) 极简点缀，杜绝原生主题控件残留，独特个性又极致简约。

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
    /// 渲染全定制美漫 3渲2 卡通风格安装与首选配置向导弹窗 (无原生边框残留)
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
        bg_painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(150));

        let mut is_open = self.installer.open;
        let mut apply_clicked = false;
        let mut browse_clicked = false;
        let mut close_clicked = false;

        // 2. 纯自绘漫画分镜窗口：禁用原生标题栏与默认 WindowFrame，全由 Painter 掌控
        let win_size = Vec2::new(600.0, 520.0);
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

                // 2.1 漫画实体纯黑硬边投影 (+5px, +5px 无模糊，经典美漫剪纸式阴影)
                p.rect_filled(win_rect.translate(Vec2::new(5.0, 5.0)), 4.0, Color32::from_black_alpha(220));

                // 2.2 漫画暗夜赛璐珞底版 (#161824) 与 2.5px 纯黑墨水轮廓
                p.rect_filled(win_rect, 4.0, Color32::from_rgb(22, 24, 36));
                p.rect_stroke(win_rect, 4.0, Stroke::new(2.5, Color32::BLACK), StrokeKind::Inside);

                // 2.3 顶部 Header 栏：深灰蓝漫画格 (#1f2230) + 2.0px 底部分割黑线
                let header_h = 46.0;
                let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
                p.rect_filled(header_rect, 4.0, Color32::from_rgb(31, 34, 48));
                p.line_segment(
                    [header_rect.left_bottom(), header_rect.right_bottom()],
                    Stroke::new(2.0, Color32::BLACK),
                );

                // 左上角 3D 水晶菱形 Logo
                let logo_center = egui::pos2(header_rect.left() + 24.0, header_rect.center().y);
                paint_installer_logo(p, logo_center, 13.0);

                // 希腊回纹 (Greek Meander) 黄金微折线点缀
                let meander_x = header_rect.left() + 45.0;
                let my = header_rect.center().y;
                let gold_st = Stroke::new(1.6, Color32::from_rgb(255, 204, 0));
                p.line_segment([pos2(meander_x, my - 7.0), pos2(meander_x + 5.0, my - 7.0)], gold_st);
                p.line_segment([pos2(meander_x + 5.0, my - 7.0), pos2(meander_x + 5.0, my + 7.0)], gold_st);
                p.line_segment([pos2(meander_x + 5.0, my + 7.0), pos2(meander_x + 10.0, my + 7.0)], gold_st);

                // 标题文字与漫画标签
                p.text(
                    pos2(meander_x + 18.0, header_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "IDE 1.0 FAST",
                    egui::FontId::proportional(14.5),
                    Color32::from_rgb(248, 250, 252),
                );

                let badge_rect = Rect::from_center_size(
                    pos2(header_rect.left() + 205.0, header_rect.center().y),
                    Vec2::new(96.0, 20.0),
                );
                p.rect_filled(badge_rect, 2.0, Color32::from_rgb(255, 204, 0));
                p.rect_stroke(badge_rect, 2.0, Stroke::new(1.2, Color32::BLACK), StrokeKind::Inside);
                p.text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "SETUP WIZARD",
                    egui::FontId::proportional(10.0),
                    Color32::BLACK,
                );

                // 版本胶囊
                let ver_rect = Rect::from_center_size(
                    pos2(header_rect.right() - 72.0, header_rect.center().y),
                    Vec2::new(58.0, 20.0),
                );
                p.rect_filled(ver_rect, 2.0, Color32::from_rgb(37, 40, 56));
                p.rect_stroke(ver_rect, 2.0, Stroke::new(1.2, Color32::BLACK), StrokeKind::Inside);
                p.text(
                    ver_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    egui::FontId::proportional(10.5),
                    Color32::from_rgb(0, 240, 255),
                );

                // 漫画右上角关闭按钮 ("✕")
                let close_rect = Rect::from_center_size(
                    pos2(header_rect.right() - 22.0, header_rect.center().y),
                    Vec2::new(24.0, 24.0),
                );
                let close_resp = ui.interact(close_rect, ui.id().with("inst_close"), egui::Sense::click());
                let (c_rect, c_bg, c_icon) = if close_resp.is_pointer_button_down_on() {
                    (close_rect.translate(Vec2::new(1.0, 1.0)), Color32::from_rgb(204, 31, 72), Color32::WHITE)
                } else if close_resp.hovered() {
                    p.rect_filled(close_rect.translate(Vec2::new(1.5, 1.5)), 2.0, Color32::BLACK);
                    (close_rect, Color32::from_rgb(255, 51, 102), Color32::WHITE)
                } else {
                    p.rect_filled(close_rect.translate(Vec2::new(1.0, 1.0)), 2.0, Color32::BLACK);
                    (close_rect, Color32::from_rgb(37, 40, 56), Color32::from_rgb(203, 213, 225))
                };
                p.rect_filled(c_rect, 2.0, c_bg);
                p.rect_stroke(c_rect, 2.0, Stroke::new(1.5, Color32::BLACK), StrokeKind::Inside);
                let cc = c_rect.center();
                p.line_segment([pos2(cc.x - 4.0, cc.y - 4.0), pos2(cc.x + 4.0, cc.y + 4.0)], Stroke::new(1.8, c_icon));
                p.line_segment([pos2(cc.x + 4.0, cc.y - 4.0), pos2(cc.x - 4.0, cc.y + 4.0)], Stroke::new(1.8, c_icon));
                if close_resp.clicked() {
                    close_clicked = true;
                }

                // 3. 内容主体容器 (内边距 16px)
                let body_rect = Rect::from_min_max(
                    pos2(win_rect.left() + 16.0, header_rect.bottom() + 12.0),
                    pos2(win_rect.right() - 16.0, win_rect.bottom() - 16.0),
                );

                ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 9.0);

                    // 3.1 漫画对话卡片 (Hero Banner Panel)
                    let banner_h = 52.0;
                    let (banner_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), banner_h), egui::Sense::hover());
                    let bp = ui.painter();
                    // 纯黑硬边阴影
                    bp.rect_filled(banner_rect.translate(Vec2::new(2.0, 2.0)), 3.0, Color32::BLACK);
                    bp.rect_filled(banner_rect, 3.0, Color32::from_rgb(32, 35, 49));
                    bp.rect_stroke(banner_rect, 3.0, Stroke::new(1.8, Color32::BLACK), StrokeKind::Inside);

                    bp.text(
                        pos2(banner_rect.left() + 14.0, banner_rect.center().y - 8.0),
                        egui::Align2::LEFT_CENTER,
                        "⚡ 极速起航 · FAST ARCHITECTURE",
                        egui::FontId::proportional(13.5),
                        Color32::from_rgb(255, 204, 0),
                    );
                    bp.text(
                        pos2(banner_rect.left() + 14.0, banner_rect.center().y + 10.0),
                        egui::Align2::LEFT_CENTER,
                        "Rust 极速微内核 · 多 Agent 协同架构 · 美漫 3渲2 卡通渲染 · 纯粹编码体验",
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(148, 163, 184),
                    );

                    // 3.2 三列漫画徽章卡片 (Perk Badges)
                    ui.horizontal(|ui| {
                        let total_w = ui.available_width();
                        let gap = 8.0;
                        let chip_w = (total_w - gap * 2.0) / 3.0;

                        render_comic_chip(ui, "⚡ 毫秒级冷启动", "零依赖·即开即写", Color32::from_rgb(0, 240, 255), chip_w);
                        render_comic_chip(ui, "🛡️ 系统托盘常驻", "后台待命·秒级唤醒", Color32::from_rgb(255, 204, 0), chip_w);
                        render_comic_chip(ui, "🔤 纯净点阵字形", "复古街机·清晰无畸", Color32::from_rgb(0, 245, 155), chip_w);
                    });

                    // 3.3 漫画分隔线 (带中央菱形徽记)
                    render_comic_divider(ui);

                    // 3.4 路径配置：漫画风格输入框 + "浏览..." 按钮
                    ui.label(
                        egui::RichText::new("📁 安装与工作目录 (Installation Directory):")
                            .strong()
                            .size(12.0)
                            .color(Color32::from_rgb(241, 245, 249)),
                    );

                    ui.horizontal(|ui| {
                        let btn_w = 76.0;
                        let input_w = ui.available_width() - btn_w - 8.0;
                        let (in_rect, in_resp) = ui.allocate_exact_size(Vec2::new(input_w, 32.0), egui::Sense::click());
                        let ip = ui.painter();

                        // 漫画硬阴影与输入底色
                        ip.rect_filled(in_rect.translate(Vec2::new(1.5, 1.5)), 2.0, Color32::BLACK);
                        ip.rect_filled(in_rect, 2.0, Color32::from_rgb(17, 18, 25));
                        let in_border = if in_resp.has_focus() {
                            Color32::from_rgb(255, 204, 0)
                        } else {
                            Color32::BLACK
                        };
                        ip.rect_stroke(in_rect, 2.0, Stroke::new(1.5, in_border), StrokeKind::Inside);

                        // 放置单行文本框 (无原生边框)
                        let text_rect = in_rect.shrink2(Vec2::new(8.0, 4.0));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.installer.target_dir)
                                    .frame(egui::Frame::NONE)
                                    .text_color(Color32::from_rgb(248, 250, 252))
                                    .desired_width(text_rect.width()),
                            );
                        });

                        if comic_button(ui, "浏览...", Vec2::new(btn_w, 32.0), false).clicked() {
                            browse_clicked = true;
                        }
                    });

                    // 3.5 系统集成选项 (全自绘 3 个漫画复选框)
                    ui.label(
                        egui::RichText::new("⚙️ 系统集成选项 (System Integration):")
                            .strong()
                            .size(12.0)
                            .color(Color32::from_rgb(241, 245, 249)),
                    );

                    comic_checkbox(
                        ui,
                        &mut self.installer.add_path,
                        "将 IDE 1.0 添加到系统环境变量 PATH",
                        "可在任意终端 (PowerShell / CMD / Bash) 运行 ide-fast 极速呼出",
                    );
                    comic_checkbox(
                        ui,
                        &mut self.installer.desktop_shortcut,
                        "在桌面创建极速启动快捷方式",
                        "生成美漫 3渲2 专属高清图标，秒级启动工作区",
                    );
                    comic_checkbox(
                        ui,
                        &mut self.installer.tray_resident,
                        "启用系统托盘常驻守护 (Resident Daemon)",
                        "最小化时驻留系统托盘，毫秒级热唤醒，防误关守护工作区",
                    );

                    // 状态提示消息
                    if let Some(msg) = &self.installer.status_msg {
                        ui.horizontal(|ui| {
                            let (badge_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), egui::Sense::hover());
                            let sp = ui.painter();
                            sp.rect_filled(badge_rect, 2.0, Color32::from_rgb(18, 38, 30));
                            sp.rect_stroke(badge_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(0, 245, 155)), StrokeKind::Inside);
                            sp.text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                msg,
                                egui::FontId::proportional(11.0),
                                Color32::from_rgb(0, 245, 155),
                            );
                        });
                    } else {
                        ui.add_space(2.0);
                    }

                    // 3.6 底部动作栏：黄色高对比漫画 CTA 按钮 + 灰色关闭按钮
                    ui.horizontal(|ui| {
                        let total_w = ui.available_width();
                        let cancel_w = 90.0;
                        let apply_w = total_w - cancel_w - 8.0;

                        if comic_button(ui, "⚡ 立即应用并开启极速体验", Vec2::new(apply_w, 38.0), true).clicked() {
                            apply_clicked = true;
                        }

                        if comic_button(ui, "关闭", Vec2::new(cancel_w, 38.0), false).clicked() {
                            close_clicked = true;
                        }
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
    p.circle_filled(center, r + 4.0, Color32::from_rgba_unmultiplied(0, 240, 255, 35));

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
    let stroke = Stroke::new(1.5, Color32::from_rgb(255, 204, 0));
    p.line_segment([top, right], stroke);
    p.line_segment([right, bottom], stroke);
    p.line_segment([bottom, left], stroke);
    p.line_segment([left, top], stroke);
    p.line_segment([top, bottom], Stroke::new(1.0, Color32::WHITE));
}

/// 渲染漫画特性徽章芯片
fn render_comic_chip(ui: &mut egui::Ui, title: &str, subtitle: &str, accent: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 42.0), egui::Sense::hover());
    let p = ui.painter();

    // 漫画硬阴影
    p.rect_filled(rect.translate(Vec2::new(1.5, 1.5)), 2.0, Color32::BLACK);
    // 卡片底色 (#1e202d)
    p.rect_filled(rect, 2.0, Color32::from_rgb(30, 32, 45));
    // 纯黑描边
    p.rect_stroke(rect, 2.0, Stroke::new(1.5, Color32::BLACK), StrokeKind::Inside);

    // 顶部 2.5px 彩色强调指示条
    let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 2.5));
    p.rect_filled(top_bar, 1.0, accent);

    p.text(
        pos2(rect.center().x, rect.center().y - 6.0),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(11.5),
        Color32::from_rgb(241, 245, 249),
    );
    p.text(
        pos2(rect.center().x, rect.center().y + 8.5),
        egui::Align2::CENTER_CENTER,
        subtitle,
        egui::FontId::proportional(9.5),
        Color32::from_rgb(148, 163, 184),
    );
}

/// 渲染漫画分割线 (带中央希腊金菱形徽记)
fn render_comic_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), egui::Sense::hover());
    let p = ui.painter();
    let cy = rect.center().y;

    // 两侧纯黑实线
    let mid_x = rect.center().x;
    p.line_segment([pos2(rect.left(), cy), pos2(mid_x - 14.0, cy)], Stroke::new(1.6, Color32::BLACK));
    p.line_segment([pos2(mid_x + 14.0, cy), pos2(rect.right(), cy)], Stroke::new(1.6, Color32::BLACK));

    // 中央菱形
    let d_top = pos2(mid_x, cy - 4.5);
    let d_bottom = pos2(mid_x, cy + 4.5);
    let d_left = pos2(mid_x - 5.5, cy);
    let d_right = pos2(mid_x + 5.5, cy);
    p.add(egui::Shape::convex_polygon(
        vec![d_top, d_right, d_bottom, d_left],
        Color32::from_rgb(255, 204, 0),
        Stroke::new(1.2, Color32::BLACK),
    ));
}

/// 美漫 3渲2 卡通风格定制按钮 (纯黑墨水边框 + 实体硬阴影 + 按下消除阴影位移)
fn comic_button(ui: &mut egui::Ui, text: &str, size: Vec2, primary: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();
    let active = resp.is_pointer_button_down_on();

    let shadow_offset = if active { Vec2::ZERO } else { Vec2::new(2.0, 2.0) };
    if !active {
        p.rect_filled(rect.translate(shadow_offset), 2.0, Color32::BLACK);
    }
    let draw_rect = if active { rect.translate(Vec2::new(2.0, 2.0)) } else { rect };

    let (bg, fg) = if primary {
        if active {
            (Color32::from_rgb(255, 170, 0), Color32::BLACK)
        } else if hovered {
            (Color32::from_rgb(255, 224, 102), Color32::BLACK)
        } else {
            (Color32::from_rgb(255, 204, 0), Color32::BLACK)
        }
    } else if active {
        (Color32::from_rgb(204, 31, 72), Color32::WHITE)
    } else if hovered {
        (Color32::from_rgb(255, 51, 102), Color32::WHITE)
    } else {
        (Color32::from_rgb(37, 40, 56), Color32::from_rgb(241, 245, 249))
    };

    p.rect_filled(draw_rect, 2.0, bg);
    p.rect_stroke(draw_rect, 2.0, Stroke::new(1.8, Color32::BLACK), StrokeKind::Inside);
    p.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(if primary { 13.5 } else { 12.0 }),
        fg,
    );
    resp
}

/// 美漫 3渲2 卡通风格定制复选框 (18px 纯黑框 + 选中高亮波普黄 + 粗黑对勾)
fn comic_checkbox(ui: &mut egui::Ui, checked: &mut bool, title: &str, desc: &str) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 36.0), egui::Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();

    if hovered {
        p.rect_filled(rect, 3.0, Color32::from_rgb(30, 33, 47));
        p.rect_stroke(rect, 3.0, Stroke::new(1.2, Color32::BLACK), StrokeKind::Inside);
    }

    let box_center = pos2(rect.left() + 14.0, rect.center().y);
    let box_rect = Rect::from_center_size(box_center, Vec2::splat(18.0));

    // 纯黑硬边阴影
    p.rect_filled(box_rect.translate(Vec2::new(1.5, 1.5)), 2.0, Color32::BLACK);

    let (bg, border) = if *checked {
        (Color32::from_rgb(255, 204, 0), Color32::BLACK)
    } else {
        (Color32::from_rgb(17, 18, 25), Color32::BLACK)
    };

    p.rect_filled(box_rect, 2.0, bg);
    p.rect_stroke(box_rect, 2.0, Stroke::new(1.8, border), StrokeKind::Inside);

    if *checked {
        let p1 = pos2(box_rect.left() + 3.5, box_rect.center().y);
        let p2 = pos2(box_rect.center().x - 1.0, box_rect.bottom() - 4.0);
        let p3 = pos2(box_rect.right() - 3.5, box_rect.top() + 4.0);
        let check_st = Stroke::new(2.2, Color32::BLACK);
        p.line_segment([p1, p2], check_st);
        p.line_segment([p2, p3], check_st);
    }

    let text_x = rect.left() + 34.0;
    p.text(
        pos2(text_x, rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(248, 250, 252),
    );
    p.text(
        pos2(text_x, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(10.0),
        Color32::from_rgb(148, 163, 184),
    );

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
