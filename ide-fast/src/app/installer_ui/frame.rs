//! 统一美漫三渲二模态窗口外壳 (Unified Cel-Shaded Comic Modal Shell)
//!
//! 为安装 / 维护 / 修复 / 卸载向导与检查更新 / 新版本通知弹窗提供完全一致的
//! 美漫三渲二外壳（墨水暗化遮罩、硬阴影、黑曜钛金底板、正版 Crystal Logo 标题栏、
//! 模式胶囊、版本火花徽章与左侧真 3D 赛璐珞水晶展台）。

use super::art::*;
use crate::app::App;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};

/// 美漫外壳标题栏配置参数
pub(crate) struct ComicFrameSpec<'a> {
    pub badge_label: &'a str,
    pub badge_bg: Color32,
    pub drag_id: &'a str,
    pub min_btn_id: Option<&'a str>,
    pub close_btn_id: &'a str,
}

/// 美漫外壳顶栏控制按钮交互结果
pub(crate) struct ComicFrameResult {
    pub minimize_clicked: bool,
    pub close_clicked: bool,
}

impl App {
    /// 绘制模态半透明漫画墨水暗化背景遮罩
    pub(crate) fn render_comic_backdrop(ctx: &egui::Context, backdrop_id: &str) {
        let screen_rect = ctx.content_rect();
        let bg_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new(backdrop_id),
        ));
        bg_painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(210));
    }

    /// 渲染统一的美漫三渲二双栏外壳（左侧 3D 赛璐珞水晶展台 + 右侧自定义分镜内容）
    pub(crate) fn render_comic_shell(
        &mut self,
        ui: &mut egui::Ui,
        win_rect: Rect,
        spec: ComicFrameSpec<'_>,
        render_right: impl FnOnce(&mut Self, &mut egui::Ui, Rect),
    ) -> ComicFrameResult {
        let ctx = ui.ctx().clone();
        let header_h = 50.0;
        let header_rect = Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));
        let title_x = header_rect.left() + 56.0;
        let wiz_rect = Rect::from_center_size(
            pos2(title_x + 122.0, header_rect.center().y),
            vec2(88.0, 22.0),
        );

        // 1-4. 绘制底板与标题栏静态图元 (黑曜钛金底板 + 纯黑硬阴影 + 粗墨线边框)
        {
            let p = ui.painter();

            // 美漫纯黑实心硬阴影
            p.rect_filled(win_rect.translate(vec2(6.0, 6.0)), 0.0, Color32::BLACK);

            // 主画板底色与外框
            p.rect_filled(win_rect, 0.0, Color32::from_rgb(14, 18, 26));
            p.rect_stroke(
                win_rect,
                0.0,
                Stroke::new(2.0, Color32::from_rgb(30, 41, 59)),
                StrokeKind::Inside,
            );

            // 顶部 Header 栏与主题强调分割线
            p.rect_filled(header_rect, 0.0, Color32::from_rgb(20, 25, 36));
            p.line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                Stroke::new(1.8, spec.badge_bg),
            );

            // 左上角正版原生 Crystal Logo 纹理徽章
            let logo_center = pos2(header_rect.left() + 28.0, header_rect.center().y);
            paint_authentic_logo(
                p,
                self.installer.logo_tex.as_ref(),
                logo_center,
                26.0,
                vec2(2.0, 2.0),
            );

            // 品牌标题文字 "IDE 1.0"
            p.text(
                pos2(title_x + 1.5, header_rect.center().y + 1.5),
                egui::Align2::LEFT_CENTER,
                "IDE 1.0",
                egui::FontId::proportional(17.0),
                Color32::BLACK,
            );
            p.text(
                pos2(title_x, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "IDE 1.0",
                egui::FontId::proportional(17.0),
                Color32::from_rgb(241, 245, 249),
            );

            // 模式胶囊
            paint_comic_hard_shadow_rect(p, wiz_rect, spec.badge_bg, 1.5, vec2(1.5, 1.5));
            p.with_clip_rect(wiz_rect.shrink2(vec2(3.0, 1.0))).text(
                wiz_rect.center(),
                egui::Align2::CENTER_CENTER,
                spec.badge_label,
                egui::FontId::proportional(10.5),
                Color32::WHITE,
            );

            // 右侧版本胶囊 (纯矢量菱形火花 + ASCII 版本号)
            let ver_x = if spec.min_btn_id.is_some() {
                header_rect.right() - 86.0
            } else {
                header_rect.right() - 78.0
            };
            let ver_rect =
                Rect::from_center_size(pos2(ver_x, header_rect.center().y), vec2(76.0, 22.0));
            paint_comic_hard_shadow_rect(
                p,
                ver_rect,
                Color32::from_rgb(30, 41, 59),
                1.5,
                vec2(1.5, 1.5),
            );
            crate::ui_icons::paint(
                p,
                Rect::from_center_size(
                    pos2(ver_rect.left() + 14.0, ver_rect.center().y),
                    vec2(10.0, 10.0),
                ),
                crate::ui_icons::UiIcon::Spark,
                Color32::from_rgb(56, 189, 248),
            );
            p.text(
                pos2(ver_rect.center().x + 6.0, ver_rect.center().y),
                egui::Align2::CENTER_CENTER,
                concat!("v", env!("CARGO_PKG_VERSION")),
                egui::FontId::proportional(10.5),
                Color32::from_rgb(203, 213, 225),
            );
        }

        // 顶部窗口拖动把手
        let drag_right = header_rect.right()
            - if spec.min_btn_id.is_some() {
                126.0
            } else {
                118.0
            };
        let drag_rect = Rect::from_min_max(
            pos2(wiz_rect.right() + 10.0, header_rect.top()),
            pos2(
                drag_right.max(wiz_rect.right() + 20.0),
                header_rect.bottom(),
            ),
        );
        let drag_resp = ui.interact(drag_rect, ui.id().with(spec.drag_id), egui::Sense::drag());
        if drag_resp.drag_started() || drag_resp.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // 可选美漫最小化控制按钮
        let minimize_clicked = if let Some(min_id) = spec.min_btn_id {
            paint_comic_window_btn(
                ui,
                pos2(header_rect.right() - 50.0, header_rect.center().y),
                min_id,
                false,
            )
        } else {
            false
        };

        // 美漫关闭控制按钮
        let close_clicked = paint_comic_window_btn(
            ui,
            pos2(header_rect.right() - 20.0, header_rect.center().y),
            spec.close_btn_id,
            true,
        );

        // 内容主体分镜：左侧 3D 赛璐珞水晶展台 + 右侧内容面板
        let body_rect = Rect::from_min_max(
            pos2(win_rect.left() + 16.0, header_rect.bottom() + 14.0),
            pos2(win_rect.right() - 16.0, win_rect.bottom() - 12.0),
        );

        let left_w = 330.0;
        let left_panel_rect = Rect::from_min_size(body_rect.min, vec2(left_w, body_rect.height()));
        let right_panel_rect = Rect::from_min_max(
            pos2(left_panel_rect.right() + 14.0, body_rect.top()),
            body_rect.max,
        );

        self.render_left_hero_stage(ui, left_panel_rect);
        render_right(self, ui, right_panel_rect);

        ComicFrameResult {
            minimize_clicked,
            close_clicked,
        }
    }
}
