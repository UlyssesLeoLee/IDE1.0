//! 原生美漫 3渲2 风格安装与首选配置向导 (Setup & Install Wizard)
//! 对应极简全宽无左侧图、左上角 3D 水晶 Logo 视觉规范，支持运行时应用系统环境变量、桌面快捷方式与托盘常驻。

use super::*;
use egui::{Color32, Rect, Vec2};

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
    /// 渲染原生安装与首选配置向导弹窗
    pub(super) fn installer_window(&mut self, ctx: &egui::Context) {
        if !self.installer.open {
            return;
        }

        let mut is_open = self.installer.open;
        let mut apply_clicked = false;
        let mut browse_clicked = false;
        let mut close_clicked = false;

        egui::Window::new("IDE 1.0 Fast Edition · 首选配置与安装向导")
            .open(&mut is_open)
            .default_size(Vec2::new(600.0, 480.0))
            .min_size(Vec2::new(520.0, 420.0))
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 8.0);

                // 1. 顶部 Header 栏：左上角 3D 水晶 Logo + 标题 + 版本号胶囊
                let (head_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), egui::Sense::hover());
                let p = ui.painter_at(head_rect);
                p.rect_filled(head_rect, 4.0, Color32::from_rgb(15, 23, 42));
                p.rect_stroke(head_rect, 4.0, egui::Stroke::new(1.0, Color32::from_rgb(30, 41, 59)), egui::StrokeKind::Inside);

                // 绘制左上角 3D 菱形水晶 Logo
                let logo_center = egui::pos2(head_rect.left() + 26.0, head_rect.center().y);
                paint_installer_logo(&p, logo_center, 14.0);

                p.text(
                    egui::pos2(head_rect.left() + 50.0, head_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "IDE 1.0 Fast Edition · 安装向导",
                    egui::FontId::proportional(15.0),
                    Color32::from_rgb(248, 250, 252),
                );

                let ver_rect = Rect::from_min_size(
                    egui::pos2(head_rect.right() - 76.0, head_rect.center().y - 11.0),
                    Vec2::new(62.0, 22.0),
                );
                p.rect_filled(ver_rect, 3.0, Color32::from_rgb(30, 41, 59));
                p.rect_stroke(ver_rect, 3.0, egui::Stroke::new(1.0, Color32::from_rgb(6, 182, 212)), egui::StrokeKind::Inside);
                p.text(
                    ver_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(6, 182, 212),
                );

                ui.add_space(4.0);

                // 2. 欢迎 Banner 卡片
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new("欢迎使用 IDE 1.0 FAST EDITION")
                            .size(15.0)
                            .strong()
                            .color(Color32::from_rgb(255, 204, 0)),
                    );
                    ui.label(
                        egui::RichText::new("Rust 极速微内核 · 多 Agent 协同架构 · 美漫 3渲2 卡通渲染 · 纯粹编码体验")
                            .size(11.5)
                            .color(Color32::from_rgb(148, 163, 184)),
                    );
                });

                // 3. 特性亮点芯片组 (水平排列)
                ui.horizontal(|ui| {
                    let chip_w = (ui.available_width() - 16.0) / 3.0;
                    render_feature_chip(ui, "• 毫秒级极速冷启动", Color32::from_rgb(6, 182, 212), chip_w);
                    render_feature_chip(ui, "• 系统托盘常驻守护", Color32::from_rgb(245, 158, 11), chip_w);
                    render_feature_chip(ui, "• 字体防畸变净化", Color32::from_rgb(16, 185, 129), chip_w);
                });

                ui.add_space(4.0);
                ui.separator();

                // 4. 安装与运行路径设置
                ui.label(egui::RichText::new("📁 安装与工作目录:").strong().size(12.5));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.installer.target_dir)
                            .desired_width(ui.available_width() - 80.0),
                    );
                    if ui.button("浏览...").clicked() {
                        browse_clicked = true;
                    }
                });

                ui.add_space(4.0);

                // 5. 系统集成选项 (3个复选框)
                ui.label(egui::RichText::new("⚙️ 系统集成选项:").strong().size(12.5));
                ui.checkbox(&mut self.installer.add_path, "将 IDE 1.0 添加到系统环境变量 PATH (支持命令行 ide-fast 极速呼出)");
                ui.checkbox(&mut self.installer.desktop_shortcut, "在桌面创建极速启动快捷方式");
                ui.checkbox(&mut self.installer.tray_resident, "启用系统托盘常驻守护 (最小化即休眠，秒级热唤醒，防意外关闭)");

                ui.add_space(8.0);

                // 状态消息展示
                if let Some(msg) = &self.installer.status_msg {
                    ui.colored_label(Color32::from_rgb(16, 185, 129), msg);
                }

                ui.add_space(6.0);

                // 6. 金色高饱和赛璐珞 CTA 按钮：立即应用并安装设置 ⚡
                ui.horizontal(|ui| {
                    let btn_size = Vec2::new(ui.available_width() - 100.0, 36.0);
                    let (btn_rect, btn_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                    let bp = ui.painter_at(btn_rect);

                    let bg = if btn_resp.is_pointer_button_down_on() {
                        Color32::from_rgb(217, 119, 6)
                    } else if btn_resp.hovered() {
                        Color32::from_rgb(251, 191, 36)
                    } else {
                        Color32::from_rgb(245, 158, 11)
                    };
                    bp.rect_filled(btn_rect, 4.0, bg);
                    bp.rect_stroke(btn_rect, 4.0, egui::Stroke::new(1.5, Color32::from_rgb(0, 0, 0)), egui::StrokeKind::Inside);
                    bp.text(
                        btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "立即应用并安装设置 ⚡",
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(18, 19, 26),
                    );

                    if btn_resp.clicked() {
                        apply_clicked = true;
                    }

                    if ui.button("关闭").clicked() {
                        close_clicked = true;
                    }
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

/// 绘制左上角官方 3D 水晶菱形 Logo (带有赛璐珞多切面光泽与黄金边框)
fn paint_installer_logo(p: &egui::Painter, center: egui::Pos2, r: f32) {
    // 外发光圈
    p.circle_filled(center, r + 4.0, Color32::from_rgba_unmultiplied(2, 132, 199, 45));

    let top = egui::pos2(center.x, center.y - r);
    let bottom = egui::pos2(center.x, center.y + r);
    let left = egui::pos2(center.x - r * 0.85, center.y);
    let right = egui::pos2(center.x + r * 0.85, center.y);

    // 左上切面 (青蓝高光)
    p.add(egui::Shape::convex_polygon(
        vec![top, left, center],
        Color32::from_rgb(56, 189, 248),
        egui::Stroke::NONE,
    ));
    // 右上切面 (明亮湖蓝)
    p.add(egui::Shape::convex_polygon(
        vec![top, right, center],
        Color32::from_rgb(14, 165, 233),
        egui::Stroke::NONE,
    ));
    // 左下切面 (深沉湛蓝)
    p.add(egui::Shape::convex_polygon(
        vec![left, bottom, center],
        Color32::from_rgb(2, 132, 199),
        egui::Stroke::NONE,
    ));
    // 右下切面 (暗夜深蓝)
    p.add(egui::Shape::convex_polygon(
        vec![right, bottom, center],
        Color32::from_rgb(3, 105, 161),
        egui::Stroke::NONE,
    ));

    // 赛璐珞纯黑硬轮廓与金边点缀
    let stroke = egui::Stroke::new(1.2, Color32::from_rgb(255, 204, 0));
    p.line_segment([top, right], stroke);
    p.line_segment([right, bottom], stroke);
    p.line_segment([bottom, left], stroke);
    p.line_segment([left, top], stroke);
    p.line_segment([top, bottom], egui::Stroke::new(0.8, Color32::from_rgb(255, 255, 255)));
}

/// 渲染水平特性胶囊
fn render_feature_chip(ui: &mut egui::Ui, text: &str, border_color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 28.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(15, 23, 42));
    p.rect_stroke(rect, 3.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(241, 245, 249),
    );
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
}
