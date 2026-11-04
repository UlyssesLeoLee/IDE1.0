//! 自绘复选框 (Tauri 里是浏览器原生复选框: 13x13, 未选 = 白底 + `#767676` 1px 边, 选中 = `#0075ff` 底 + 白色对勾)。
//! egui 自带的复选框和滚动条滑块共用 `widgets.inactive.bg_fill`, 单改会连滚动条一起变, 所以菜单 / 对话框里的勾选框统一走这里。
//! 是普通的 `egui::Widget`: `ui.add(Check::new(&mut v, "文字"))`, `ui.add_enabled(..)` 照常可用, 点击时 `Response::changed()` 为真。

use eframe::egui::{self, Color32, Pos2, Response, Sense, Shape, Stroke, TextStyle, Ui, Vec2, Widget};

/// 方框边长 (Chromium 默认 13px)
pub const BOX: f32 = 13.0;
/// 方框与文字的间距
const GAP: f32 = 6.0;

/// 方框的 (底色, 边色): 未选白底灰边, 悬停边变深, 选中蓝底蓝边; 禁用时整体变淡
pub fn box_colors(checked: bool, hovered: bool, enabled: bool) -> (Color32, Color32) {
    let (fill, edge) = match (checked, hovered) {
        (true, false) => (Color32::from_rgb(0x00, 0x75, 0xff), Color32::from_rgb(0x00, 0x75, 0xff)),
        (true, true) => (Color32::from_rgb(0x00, 0x60, 0xdf), Color32::from_rgb(0x00, 0x60, 0xdf)),
        (false, false) => (Color32::WHITE, Color32::from_rgb(0x76, 0x76, 0x76)),
        (false, true) => (Color32::WHITE, Color32::from_rgb(0x4f, 0x4f, 0x4f)),
    };
    if enabled { (fill, edge) } else { (fill.gamma_multiply(0.45), edge.gamma_multiply(0.45)) }
}

pub struct Check<'a> {
    on: &'a mut bool,
    text: String,
}

impl<'a> Check<'a> {
    pub fn new(on: &'a mut bool, text: impl Into<String>) -> Self {
        Check { on, text: text.into() }
    }
}

/// 对勾的三个折点 (相对方框左上角, 13x13 里)
pub fn tick_points() -> [Vec2; 3] {
    [Vec2::new(3.0, 6.8), Vec2::new(5.6, 9.4), Vec2::new(10.0, 3.8)]
}

impl Widget for Check<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = ui.is_enabled();
        let font = TextStyle::Body.resolve(ui.style());
        let text_col = ui.visuals().text_color();
        let galley = (!self.text.is_empty()).then(|| ui.painter().layout_no_wrap(self.text.clone(), font, text_col));
        let tw = galley.as_ref().map_or(0.0, |g| g.size().x + GAP);
        let th = galley.as_ref().map_or(0.0, |g| g.size().y);
        let size = Vec2::new(BOX + tw, th.max(BOX).max(ui.spacing().interact_size.y.min(18.0)));
        let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
        if resp.clicked() {
            *self.on = !*self.on;
            resp.mark_changed();
        }
        let on = *self.on;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, self.text.clone()));
        if ui.is_rect_visible(rect) {
            let (fill, edge) = box_colors(on, resp.hovered() && enabled, enabled);
            let b = egui::Rect::from_min_size(Pos2::new(rect.left(), rect.center().y - BOX / 2.0), Vec2::splat(BOX));
            let p = ui.painter();
            p.rect(b, 2.0, fill, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
            if on {
                let pts: Vec<Pos2> = tick_points().iter().map(|v| b.min + *v).collect();
                p.add(Shape::line(pts, Stroke::new(1.8, Color32::WHITE.gamma_multiply(if enabled { 1.0 } else { 0.7 }))));
            }
            if let Some(g) = galley {
                let col = if enabled { text_col } else { ui.visuals().weak_text_color() };
                p.galley(Pos2::new(b.right() + GAP, rect.center().y - g.size().y / 2.0), g, col);
            }
        }
        resp
    }
}

/// `ui.check(&mut v, "文字")`: 代替 `ui.checkbox`, 调用处只换方法名
pub trait CheckExt {
    fn check(&mut self, on: &mut bool, text: impl Into<String>) -> Response;
}

impl CheckExt for Ui {
    fn check(&mut self, on: &mut bool, text: impl Into<String>) -> Response {
        self.add(Check::new(on, text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchecked_is_white_with_grey_border_and_checked_is_blue() {
        let (f, e) = box_colors(false, false, true);
        assert_eq!((f, e), (Color32::WHITE, Color32::from_rgb(0x76, 0x76, 0x76)));
        let (f, e) = box_colors(true, false, true);
        assert_eq!((f, e), (Color32::from_rgb(0, 0x75, 0xff), Color32::from_rgb(0, 0x75, 0xff)));
    }

    #[test]
    fn hover_darkens_the_border_and_disabled_fades() {
        assert_ne!(box_colors(false, true, true).1, box_colors(false, false, true).1);
        assert!(box_colors(false, false, false).0.a() < 255);
    }

    #[test]
    fn tick_stays_inside_the_box() {
        for p in tick_points() {
            assert!((0.0..=BOX).contains(&p.x) && (0.0..=BOX).contains(&p.y));
        }
    }

    #[test]
    fn clicking_toggles_and_reports_changed() {
        let ctx = egui::Context::default();
        let mut on = false;
        let mut ev = egui::RawInput::default();
        ev.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 100.0)));
        ctx.run_ui(ev.clone(), |ui| { ui.add(Check::new(&mut on, "x")); }).textures_delta.clear();
        ev.events.push(egui::Event::PointerMoved(Pos2::new(8.0, 8.0)));
        ev.events.push(egui::Event::PointerButton { pos: Pos2::new(8.0, 8.0), button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
        ev.events.push(egui::Event::PointerButton { pos: Pos2::new(8.0, 8.0), button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
        let mut changed = false;
        ctx.run_ui(ev, |ui| { changed = ui.add(Check::new(&mut on, "x")).changed(); }).textures_delta.clear();
        assert!(on && changed);
    }
}
