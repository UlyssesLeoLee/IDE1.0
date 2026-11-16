//! Tauri 样式的工具提示 (`#tip`: #333 底 / 白字 / 圆角 3 / 内边距 4x8 / 11px / 最宽 400 / 立即出现 / 贴在控件下方 4px,
//! 放不下就翻到上方)。egui 的 `on_hover_text` 用全局弹出框样式, 只改它会连窗口 / 对话框一起变, 所以这里自己画:
//! 一个 `Order::Tooltip` 的 `Area`, 外框显式指定, 不碰任何别的弹出层。

use eframe::egui::{
    self, epaint::Shadow, Align2, Color32, FontId, Frame, Margin, Pos2, Rect, Response, Vec2,
};

pub const TIP_BG: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
pub const TIP_FONT: f32 = 11.0;
pub const TIP_MAX_W: f32 = 400.0;
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 4.0;
const GAP: f32 = 4.0;

/// 气泡左上角 (Tauri 的 mouseover 处理): 水平贴控件左缘, 不出屏幕 (左 4 / 右 8 边距); 垂直放下方, 下方放不下 (留 2px) 翻到上方 (最少 4)
pub fn tip_pos(target: Rect, size: Vec2, screen: Rect) -> Pos2 {
    let x = target
        .left()
        .min(screen.right() - size.x - 8.0)
        .max(screen.left() + 4.0);
    let below = target.bottom() + GAP;
    let y = if below + size.y > screen.bottom() - 2.0 {
        (target.top() - size.y - GAP).max(screen.top() + 4.0)
    } else {
        below
    };
    Pos2::new(x, y)
}

pub trait TipExt {
    /// 悬停时显示 Tauri 样式的提示 (代替 `on_hover_text`)
    fn tip(self, text: impl Into<String>) -> Self;
}

impl TipExt for Response {
    fn tip(self, text: impl Into<String>) -> Self {
        if !self.hovered() && !self.is_pointer_button_down_on() {
            return self;
        }
        let ctx = self.ctx.clone();
        // 正在拖动 / 按住时不弹 (浏览器里按下鼠标也会收起)
        if ctx.input(|i| i.pointer.any_down() && !self.is_pointer_button_down_on()) {
            return self;
        }
        let text: String = text.into();
        if text.is_empty() {
            return self;
        }
        let galley = ctx.fonts_mut(|f| {
            f.layout(
                text,
                FontId::proportional(TIP_FONT),
                Color32::WHITE,
                TIP_MAX_W,
            )
        });
        let size = galley.size() + Vec2::new(2.0 * PAD_X, 2.0 * PAD_Y);
        let pos = tip_pos(self.rect, size, ctx.content_rect());
        egui::Area::new(self.id.with("tip"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos)
            .pivot(Align2::LEFT_TOP)
            .interactable(false)
            .show(&ctx, |ui| {
                Frame::new()
                    .fill(TIP_BG)
                    .corner_radius(3.0)
                    .shadow(Shadow::NONE)
                    .inner_margin(Margin::symmetric(PAD_X as i8, PAD_Y as i8))
                    .show(ui, |ui| {
                        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                        ui.painter().galley(rect.min, galley, Color32::WHITE);
                    });
            });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    #[test]
    fn tip_sits_below_the_control_left_aligned() {
        let p = tip_pos(
            r(100.0, 50.0, 130.0, 70.0),
            Vec2::new(80.0, 22.0),
            r(0.0, 0.0, 800.0, 600.0),
        );
        assert_eq!(p, Pos2::new(100.0, 74.0));
    }

    #[test]
    fn tip_is_kept_inside_the_window_horizontally() {
        let s = r(0.0, 0.0, 800.0, 600.0);
        assert_eq!(
            tip_pos(r(790.0, 50.0, 800.0, 70.0), Vec2::new(100.0, 22.0), s).x,
            692.0
        );
        assert_eq!(
            tip_pos(r(-20.0, 50.0, 0.0, 70.0), Vec2::new(100.0, 22.0), s).x,
            4.0
        );
    }

    #[test]
    fn tip_flips_above_when_there_is_no_room_below() {
        let p = tip_pos(
            r(100.0, 570.0, 130.0, 590.0),
            Vec2::new(80.0, 22.0),
            r(0.0, 0.0, 800.0, 600.0),
        );
        assert_eq!(p.y, 570.0 - 22.0 - 4.0);
    }
}
