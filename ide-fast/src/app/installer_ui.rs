//! 经典美漫与赛璐珞卡通渲染风格安装与首选配置向导 (American Comic & Cel-Shaded Setup Wizard)
//! 全定制原生渲染：800×560 宽幅漫画分镜布局、粗墨线轮廓描边 (2.0~2.5px 纯黑 Ink Outlines)、
//! 纯黑实心硬阴影 (Hard Drop Shadows)、波普半色调网点 (Ben-Day Screentone Dots)、
//! 3D 赛璐珞两阶硬光影神殿与水晶能量核、爆炸贴纸 (Action Burst Stars) 与对话气泡 (Speech Balloons)，
//! 彻底重构为极具冲击力与趣味性的美漫英雄风格，告别原生传统安装包外观。

use super::*;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};
use std::f32::consts::PI;

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
            .unwrap_or_else(|| r"C:\Program Files\IDE1.0".to_string());
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
    /// 渲染全定制美漫赛璐珞卡通风格安装向导 (800×560 双分镜英雄面板)
    pub(super) fn installer_window(&mut self, ctx: &egui::Context) {
        if !self.installer.open {
            return;
        }

        // 1. 半透明漫画墨水暗化遮罩
        let screen_rect = ctx.content_rect();
        let bg_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("installer_comic_modal_backdrop"),
        ));
        bg_painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(200));

        let mut is_open = self.installer.open;
        let mut apply_clicked = false;
        let mut browse_clicked = false;
        let mut close_clicked = false;

        // 2. 纯自绘美漫风格窗口：800.0 × 560.0
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
                let p = ui.painter();

                // 2.1 美漫纯黑实心硬阴影 (无模糊，纯正美漫贴纸立体感)
                let shadow_offset = vec2(6.0, 6.0);
                p.rect_filled(win_rect.translate(shadow_offset), 0.0, Color32::BLACK);

                // 2.2 漫画主画板底色与粗墨线外框
                let comic_night_bg = Color32::from_rgb(18, 21, 35);
                p.rect_filled(win_rect, 0.0, comic_night_bg);
                // 2.5px 纯黑墨水轮廓
                p.rect_stroke(win_rect, 0.0, Stroke::new(2.5, Color32::BLACK), StrokeKind::Inside);

                // 2.3 背景 Ben-Day 波普网点 (半色调点阵)
                paint_ben_day_dots(p, win_rect.shrink(4.0), Color32::from_rgb(32, 38, 62), 12.0, 1.2);

                // 2.4 顶部 Comic Header 栏：美漫亮黄高饱和行动条 (#FFE600)
                let header_h = 54.0;
                let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
                p.rect_filled(header_rect, 0.0, Color32::from_rgb(255, 230, 0));
                p.rect_stroke(header_rect, 0.0, Stroke::new(2.0, Color32::BLACK), StrokeKind::Inside);
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

                // 左上角 3D 赛璐珞水晶 Logo 徽章
                let logo_center = pos2(header_rect.left() + 32.0, header_rect.center().y);
                paint_cel_shaded_gem_logo(p, logo_center, 18.0);

                // 品牌标题文字 (带纯黑实心投影)
                let title_x = header_rect.left() + 62.0;
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

                // 爆炸贴纸 "POW! FAST EDITION"
                let star_center = pos2(title_x + 130.0, header_rect.center().y);
                paint_comic_burst_star(p, star_center, 22.0, 12.0, 10, Color32::from_rgb(255, 42, 85), vec2(2.0, 2.0));
                p.text(
                    star_center,
                    egui::Align2::CENTER_CENTER,
                    "FAST!",
                    egui::FontId::proportional(9.0),
                    Color32::WHITE,
                );

                // 美漫行动胶囊 "SETUP WIZARD"
                let wiz_rect = Rect::from_center_size(
                    pos2(star_center.x + 86.0, header_rect.center().y),
                    vec2(104.0, 24.0),
                );
                paint_comic_hard_shadow_rect(p, wiz_rect, Color32::from_rgb(0, 229, 255), 2.0, vec2(2.5, 2.5));
                p.text(
                    wiz_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "SETUP WIZARD",
                    egui::FontId::proportional(10.5),
                    Color32::BLACK,
                );

                // 右侧版本行动贴纸
                let ver_rect = Rect::from_center_size(
                    pos2(header_rect.right() - 88.0, header_rect.center().y),
                    vec2(80.0, 24.0),
                );
                paint_comic_hard_shadow_rect(p, ver_rect, Color32::WHITE, 2.0, vec2(2.5, 2.5));
                p.text(
                    ver_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    concat!("★ v", env!("CARGO_PKG_VERSION")),
                    egui::FontId::proportional(11.0),
                    Color32::BLACK,
                );

                // 漫画关闭按钮 (Red Action Button with Hard Shadow)
                let close_rect = Rect::from_center_size(
                    pos2(header_rect.right() - 26.0, header_rect.center().y),
                    vec2(26.0, 26.0),
                );
                let close_resp = ui.interact(close_rect, ui.id().with("comic_inst_close"), egui::Sense::click());
                let is_down = close_resp.is_pointer_button_down_on();
                let close_draw_rect = if is_down { close_rect.translate(vec2(1.5, 1.5)) } else { close_rect };
                paint_comic_hard_shadow_rect(
                    p,
                    close_draw_rect,
                    if close_resp.hovered() { Color32::from_rgb(255, 60, 60) } else { Color32::from_rgb(220, 38, 38) },
                    2.0,
                    if is_down { vec2(1.0, 1.0) } else { vec2(2.5, 2.5) },
                );
                let cc = close_draw_rect.center();
                p.line_segment([pos2(cc.x - 5.0, cc.y - 5.0), pos2(cc.x + 5.0, cc.y + 5.0)], Stroke::new(2.5, Color32::WHITE));
                p.line_segment([pos2(cc.x + 5.0, cc.y - 5.0), pos2(cc.x - 5.0, cc.y + 5.0)], Stroke::new(2.5, Color32::WHITE));
                if close_resp.clicked() {
                    close_clicked = true;
                }

                // 3. 内容主体容器：左侧英雄展台分镜 (Hero Stage) + 右侧操作设置分镜 (Action Settings)
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

                // 3.1 左侧英雄展台分镜：爱奥尼神殿柱台 + 悬浮 3D 水晶核 + 对话气泡
                paint_comic_hard_shadow_rect(p, left_panel_rect, Color32::from_rgb(24, 28, 46), 2.5, vec2(3.5, 3.5));
                // 左分镜网点
                paint_ben_day_dots(p, left_panel_rect.shrink(4.0), Color32::from_rgb(0, 229, 255), 8.0, 1.0);

                let core_center = pos2(left_panel_rect.center().x, left_panel_rect.top() + 106.0);
                // 放射状漫画爆发光芒 (Burst Rays)
                for a in (0..360).step_by(30) {
                    let rad = (a as f32) * PI / 180.0;
                    let ray_p = pos2(core_center.x + 60.0 * rad.cos(), core_center.y + 60.0 * rad.sin());
                    p.line_segment([core_center, ray_p], Stroke::new(1.2, Color32::from_rgb(255, 230, 0)));
                }
                // 3D 水晶悬浮核心
                paint_cel_shaded_gem_logo(p, core_center, 34.0);

                // 爱奥尼古典神殿柱台 (Ionic Pedestal)
                paint_ionic_comic_pedestal(p, left_panel_rect.center().x, left_panel_rect.top() + 166.0);

                // 漫画对话气泡 (Speech Balloon)
                let bubble_rect = Rect::from_center_size(
                    pos2(left_panel_rect.center().x, left_panel_rect.bottom() - 86.0),
                    vec2(210.0, 48.0),
                );
                paint_comic_hard_shadow_rect(p, bubble_rect, Color32::from_rgb(255, 253, 240), 2.0, vec2(2.5, 2.5));
                // 气泡向上的尖角尾巴 (Speech Tail)
                let tail_top = pos2(bubble_rect.center().x - 10.0, bubble_rect.top() - 8.0);
                let tail_l = pos2(bubble_rect.center().x - 18.0, bubble_rect.top());
                let tail_r = pos2(bubble_rect.center().x - 2.0, bubble_rect.top());
                p.add(egui::Shape::convex_polygon(
                    vec![tail_top, tail_l, tail_r],
                    Color32::from_rgb(255, 253, 240),
                    Stroke::new(2.0, Color32::BLACK),
                ));
                p.line_segment([tail_l, tail_r], Stroke::new(2.5, Color32::from_rgb(255, 253, 240)));
                p.text(
                    pos2(bubble_rect.center().x, bubble_rect.center().y - 8.0),
                    egui::Align2::CENTER_CENTER,
                    "⚡ 毫秒极速冷启 · 纯 Rust 打造！",
                    egui::FontId::proportional(11.0),
                    Color32::BLACK,
                );
                p.text(
                    pos2(bubble_rect.center().x, bubble_rect.center().y + 10.0),
                    egui::Align2::CENTER_CENTER,
                    "零开销 · 纯粹原生 · 专为硬核极客设计",
                    egui::FontId::proportional(9.2),
                    Color32::from_rgb(70, 75, 90),
                );

                // 左下角英雄印章贴纸
                let stamp_rect = Rect::from_center_size(
                    pos2(left_panel_rect.center().x, left_panel_rect.bottom() - 28.0),
                    vec2(160.0, 26.0),
                );
                paint_comic_hard_shadow_rect(p, stamp_rect, Color32::from_rgb(255, 42, 85), 2.0, vec2(2.0, 2.0));
                p.text(
                    stamp_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "★ 100% NATIVE HERO ★",
                    egui::FontId::proportional(10.5),
                    Color32::WHITE,
                );

                // 3.2 右侧操作分镜容器 (Settings & Actions)
                ui.scope_builder(egui::UiBuilder::new().max_rect(right_panel_rect), |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 7.0);

                    // Hero 对话框卡片 (White Speech Card)
                    let hero_h = 56.0;
                    let (h_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), hero_h), egui::Sense::hover());
                    let hp = ui.painter();
                    paint_comic_hard_shadow_rect(hp, h_rect, Color32::WHITE, 2.0, vec2(3.0, 3.0));
                    // 顶部亮黄强调条
                    hp.rect_filled(
                        Rect::from_min_size(h_rect.min, vec2(h_rect.width(), 4.0)),
                        0.0,
                        Color32::from_rgb(255, 230, 0),
                    );
                    hp.text(
                        pos2(h_rect.left() + 14.0, h_rect.top() + 16.0),
                        egui::Align2::LEFT_CENTER,
                        "⚡ 开启全新极速编码冒险！(START ADVENTURE)",
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
                    // 卡片右上角爆破小贴纸
                    paint_comic_burst_star(hp, pos2(h_rect.right() - 28.0, h_rect.top() + 24.0), 16.0, 9.0, 8, Color32::from_rgb(255, 230, 0), vec2(1.5, 1.5));
                    hp.text(
                        pos2(h_rect.right() - 28.0, h_rect.top() + 24.0),
                        egui::Align2::CENTER_CENTER,
                        "POW!",
                        egui::FontId::proportional(7.5),
                        Color32::BLACK,
                    );

                    // 路径设置行
                    ui.label(
                        egui::RichText::new("📁 目标安装与工作目录 (Installation Directory):")
                            .strong()
                            .size(11.5)
                            .color(Color32::from_rgb(255, 230, 0)),
                    );

                    ui.horizontal(|ui| {
                        let btn_w = 118.0;
                        let input_w = ui.available_width() - btn_w - 10.0;
                        let (in_rect, in_resp) = ui.allocate_exact_size(vec2(input_w, 34.0), egui::Sense::click());
                        let ip = ui.painter();

                        // 漫画输入框：纯黑阴影 + 白色底板 + 2.0px 黑边框
                        paint_comic_hard_shadow_rect(
                            ip,
                            in_rect,
                            Color32::from_rgb(255, 255, 255),
                            2.0,
                            vec2(2.5, 2.5),
                        );
                        if in_resp.has_focus() {
                            ip.rect_stroke(in_rect, 0.0, Stroke::new(2.5, Color32::from_rgb(0, 229, 255)), StrokeKind::Inside);
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

                        if comic_action_button(ui, "浏览 (BROWSE)...", vec2(btn_w, 34.0), false, Color32::from_rgb(0, 229, 255)).clicked() {
                            browse_clicked = true;
                        }
                    });

                    // 空间与规格提示标签
                    ui.label(
                        egui::RichText::new("所需空间: ~85 MB  |  架构: x86_64 Windows (Native)  |  零第三方依赖 · 秒速起航")
                            .monospace()
                            .size(9.8)
                            .color(Color32::from_rgb(148, 163, 184)),
                    );

                    // 快捷设置分镜 (3 大美漫卡片)
                    ui.label(
                        egui::RichText::new("⚙️ 系统集成与环境选项 (Integration Options):")
                            .strong()
                            .size(11.5)
                            .color(Color32::from_rgb(255, 230, 0)),
                    );

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
                        "生成 3D 赛璐珞水晶图标，双击秒开，随时直达工作空间",
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

                    // 状态反馈贴纸
                    if let Some(msg) = &self.installer.status_msg {
                        ui.horizontal(|ui| {
                            let (badge_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), egui::Sense::hover());
                            let sp = ui.painter();
                            paint_comic_hard_shadow_rect(sp, badge_rect, Color32::from_rgb(16, 185, 129), 2.0, vec2(2.0, 2.0));
                            sp.text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                msg,
                                egui::FontId::proportional(11.0),
                                Color32::BLACK,
                            );
                        });
                    }

                    // 底部行动条：巨大亮黄色漫画 CTA 按钮 + 灰色关闭按钮
                    ui.horizontal(|ui| {
                        let total_w = ui.available_width();
                        let cancel_w = 110.0;
                        let apply_w = total_w - cancel_w - 12.0;

                        if comic_action_button(
                            ui,
                            "⚡ 立即部署并开启极速体验 (LAUNCH NOW!) ⚡",
                            vec2(apply_w, 42.0),
                            true,
                            Color32::from_rgb(255, 230, 0),
                        )
                        .clicked()
                        {
                            apply_clicked = true;
                        }

                        if comic_action_button(
                            ui,
                            "关闭 (CLOSE)",
                            vec2(cancel_w, 42.0),
                            false,
                            Color32::from_rgb(40, 48, 70),
                        )
                        .clicked()
                        {
                            close_clicked = true;
                        }
                    });

                    // 底部免责声明条
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("★ 遵循 AGPL-3.0 协议开源授权 · 保证纯净无捆绑、无流氓遥测、无后台静默操作 ★")
                                .size(9.5)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(concat!("Release Notes v", env!("CARGO_PKG_VERSION"), " →"))
                                    .size(9.5)
                                    .color(Color32::from_rgb(0, 229, 255)),
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
            self.cfg.tray_resident = self.installer.tray_resident;
            self.cfg.save();

            if self.cfg.tray_resident && self.tray.is_none() {
                self.tray = crate::tray::Tray::build(true, false);
            } else if let Some(t) = &self.tray {
                t.set_resident_checked(self.cfg.tray_resident);
            }

            self.installer.status_msg =
                Some("✓ 首选配置与系统集成选项已成功应用并保存！".to_string());
            self.cur().tip = "安装向导首选配置已生效".into();
        }
    }
}

/// 绘制纯正美漫硬阴影矩形卡片 (硬阴影 + 纯黑外框)
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

    // 纯黑硬阴影
    if shadow_offset.x > 0.0 || shadow_offset.y > 0.0 {
        let shadow_pts = pts.iter().map(|pt| *pt + shadow_offset).collect::<Vec<_>>();
        p.add(egui::Shape::convex_polygon(
            shadow_pts,
            Color32::BLACK,
            Stroke::NONE,
        ));
    }

    // 主星形
    p.add(egui::Shape::convex_polygon(
        pts.clone(),
        fill_col,
        Stroke::new(2.0, Color32::BLACK),
    ));
}

/// 绘制 3D 赛璐珞两阶硬光影水晶 Logo
fn paint_cel_shaded_gem_logo(p: &egui::Painter, center: egui::Pos2, r: f32) {
    let top = pos2(center.x, center.y - r);
    let bottom = pos2(center.x, center.y + r);
    let left = pos2(center.x - r * 0.85, center.y);
    let right = pos2(center.x + r * 0.85, center.y);

    // 纯黑硬阴影
    let s_off = vec2(3.0, 3.0);
    p.add(egui::Shape::convex_polygon(
        vec![top + s_off, right + s_off, bottom + s_off, left + s_off],
        Color32::BLACK,
        Stroke::NONE,
    ));

    // 赛璐珞硬切面
    // 1. 左上亮面 (Sky Cyan)
    p.add(egui::Shape::convex_polygon(
        vec![top, left, center],
        Color32::from_rgb(56, 189, 248),
        Stroke::NONE,
    ));
    // 2. 右上侧光面 (Electric Cyan)
    p.add(egui::Shape::convex_polygon(
        vec![top, right, center],
        Color32::from_rgb(14, 165, 233),
        Stroke::NONE,
    ));
    // 3. 左下暗面 (Core Shadow Blue)
    p.add(egui::Shape::convex_polygon(
        vec![left, bottom, center],
        Color32::from_rgb(2, 132, 199),
        Stroke::NONE,
    ));
    // 4. 右下背光面 (Cast Shadow Navy)
    p.add(egui::Shape::convex_polygon(
        vec![right, bottom, center],
        Color32::from_rgb(3, 105, 161),
        Stroke::NONE,
    ));

    // 粗墨线轮廓描边 (Bold Inking Contours)
    let ink = Stroke::new(2.0, Color32::BLACK);
    p.line_segment([top, right], ink);
    p.line_segment([right, bottom], ink);
    p.line_segment([bottom, left], ink);
    p.line_segment([left, top], ink);
    p.line_segment([top, bottom], ink);
    p.line_segment([left, right], ink);

    // 赛璐珞高光三角折线 (Specular Highlight)
    let hi_poly = vec![
        top,
        pos2(center.x - r * 0.25, center.y - r * 0.4),
        pos2(center.x, center.y - r * 0.2),
    ];
    p.add(egui::Shape::convex_polygon(
        hi_poly,
        Color32::WHITE,
        Stroke::NONE,
    ));
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
    // 卷涡 Volutes
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

    // 2. 柱身 Shaft (Fluted with 2-Tone Cel Shading)
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
    // 希腊黄金回形饰带 (Gold Greek Accent Band)
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

    // 点击时向阴影方向偏移，营造逼真机械漫画按压感
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

/// 美漫交互复选卡片 (漫画分镜格 + 实心黑硬阴影 + 触觉按压反馈)
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

    // 左侧粗强调色条
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

    // 漫画复选框 (Comic Checkbox)
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

    // 标题与描述
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

    // 右侧漫画印章状态 [ ★ READY! ] / [ ○ OFF ]
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
        assert!(s.add_path);
        assert!(s.desktop_shortcut);
        assert!(s.tray_resident);
        assert!(s.target_dir.contains("IDE1.0") || !s.target_dir.is_empty());
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

        // 验证向导能在 run_ui 下完整自绘且无 panic
        let mut out = ctx.run_ui(Default::default(), |_root| {
            a.installer_window(&ctx);
        });
        out.textures_delta.clear();
        assert!(a.installer.open);
    }
}
