//! 美漫安装向导左侧英雄展台分镜 (Hero Stage View)

use super::art::*;
use super::InstallerStep;
use crate::app::App;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke};
use std::f32::consts::PI;

impl App {
    /// 渲染左侧英雄展台分镜 (Hero Stage)
    pub(super) fn render_left_hero_stage(&self, p: &egui::Painter, left_panel_rect: Rect) {
        paint_comic_hard_shadow_rect(
            p,
            left_panel_rect,
            Color32::from_rgb(24, 28, 46),
            2.5,
            vec2(3.5, 3.5),
        );
        paint_ben_day_dots(
            p,
            left_panel_rect.shrink(4.0),
            Color32::from_rgb(0, 229, 255),
            8.0,
            1.0,
        );

        let core_center = pos2(left_panel_rect.center().x, left_panel_rect.top() + 106.0);
        // 放射状漫画爆发光芒 (Burst Rays)
        for a in (0..360).step_by(30) {
            let rad = (a as f32) * PI / 180.0;
            let ray_p = pos2(
                core_center.x + 64.0 * rad.cos(),
                core_center.y + 64.0 * rad.sin(),
            );
            p.line_segment(
                [core_center, ray_p],
                Stroke::new(1.2, Color32::from_rgb(255, 230, 0)),
            );
        }

        // 正版原生 Logo 纹理悬浮核心 (使用用户真实正版 Crystal Logo，保留晶莹切面)
        paint_authentic_logo(
            p,
            self.installer.logo_tex.as_ref(),
            core_center,
            72.0,
            vec2(3.5, 3.5),
        );

        // 爱奥尼古典神殿柱台
        paint_ionic_comic_pedestal(p, left_panel_rect.center().x, left_panel_rect.top() + 168.0);

        // 漫画对话气泡 (Speech Balloon)
        let bubble_rect = Rect::from_center_size(
            pos2(left_panel_rect.center().x, left_panel_rect.bottom() - 86.0),
            vec2(210.0, 52.0),
        );
        paint_comic_hard_shadow_rect(
            p,
            bubble_rect,
            Color32::from_rgb(255, 253, 240),
            2.0,
            vec2(2.5, 2.5),
        );
        let tail_top = pos2(bubble_rect.center().x - 10.0, bubble_rect.top() - 8.0);
        let tail_l = pos2(bubble_rect.center().x - 18.0, bubble_rect.top());
        let tail_r = pos2(bubble_rect.center().x - 2.0, bubble_rect.top());
        p.add(egui::Shape::convex_polygon(
            vec![tail_top, tail_l, tail_r],
            Color32::from_rgb(255, 253, 240),
            Stroke::new(2.0, Color32::BLACK),
        ));
        p.line_segment(
            [tail_l, tail_r],
            Stroke::new(2.5, Color32::from_rgb(255, 253, 240)),
        );

        let (b_line1, b_line2) = match self.installer.step {
            InstallerStep::Welcome => (
                "⚡ 毫秒极速冷启 · 纯 Rust 打造！",
                "零 Electron 臃肿 · 极致性能体验",
            ),
            InstallerStep::Options => ("📁 自由定制你的专属环境！", "随心配置路径与系统集成选项"),
            InstallerStep::Installing => {
                ("🚀 正在极速释放原生性能内核...", "编译缓存与环境秒级就绪中")
            }
            InstallerStep::Finished => (
                "🎉 恭喜！IDE 1.0 极速环境已就绪！",
                "开启属于你的无拘编码冒险",
            ),
        };

        p.text(
            pos2(bubble_rect.center().x, bubble_rect.center().y - 9.0),
            egui::Align2::CENTER_CENTER,
            b_line1,
            egui::FontId::proportional(11.0),
            Color32::BLACK,
        );
        p.text(
            pos2(bubble_rect.center().x, bubble_rect.center().y + 11.0),
            egui::Align2::CENTER_CENTER,
            b_line2,
            egui::FontId::proportional(9.2),
            Color32::from_rgb(70, 75, 90),
        );

        // 左下角英雄印章贴纸
        let stamp_rect = Rect::from_center_size(
            pos2(left_panel_rect.center().x, left_panel_rect.bottom() - 28.0),
            vec2(170.0, 26.0),
        );
        let (stamp_col, stamp_text) = if self.installer.step == InstallerStep::Finished {
            (Color32::from_rgb(255, 204, 0), "★ MISSION COMPLETE ★")
        } else {
            (Color32::from_rgb(255, 42, 85), "★ 100% NATIVE HERO ★")
        };
        paint_comic_hard_shadow_rect(p, stamp_rect, stamp_col, 2.0, vec2(2.0, 2.0));
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            stamp_text,
            egui::FontId::proportional(10.5),
            if self.installer.step == InstallerStep::Finished {
                Color32::BLACK
            } else {
                Color32::WHITE
            },
        );
    }
}
