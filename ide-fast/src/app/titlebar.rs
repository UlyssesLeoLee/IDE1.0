//! 自绘标题栏 (默认): 窗口创建时不要系统标题栏 (`with_decorations(false)`), 由这里画一条栏顶替 ——
//! 图标 + 紧凑菜单 + 快捷按钮 + 最小化 / 最大化 / 关闭。拖空白处移动、双击空白处最大化 / 还原、窗口边缘的缩放把手 (`BeginResize`)。
//! 「使用系统标题栏」是设置项 (`Settings::system_title_bar`), 切换时发 `ViewportCommand::Decorations`, 不用重启。
//! 全屏 (F11) 时标题栏的图标 / 按钮 / 拖动区都不画, 只留一条普通菜单行, 菜单仍然够得着; 最大化 / 全屏时没有缩放把手。
//! 命中判断 (`grip`) 是纯函数, 单测覆盖; 栏本身不增加空闲开销 (只在有输入事件时才重绘)。

use super::*;
use crate::panels::Dock;
use egui::viewport::ResizeDirection as Dir;

/// 标题栏高度 (逻辑点)
const BAR_H: f32 = 30.0;
/// 窗口按钮宽度
const BTN_W: f32 = 44.0;
/// 缩放把手的粗细 / 角上的边长
const GRIP: f32 = 5.0;
const CORNER: f32 = 12.0;
/// 快捷按钮区的宽度, 以及栏至少多宽才画它
const QUICK_W: f32 = 270.0;
const QUICK_MIN_W: f32 = 900.0;

/// 快捷按钮: (命令 id, 文字)。面板开关用画出来的小图标, 不在这里。
const QUICK: &[(&str, &str)] = &[("c.new-file", "新"), ("c.open-file", "开"), ("c.save", "存"), ("c.find", "找"), ("c.palette", ">_"), ("c.split", "||")];

/// 光标在窗口的哪个缩放把手上 (None = 不在边缘)。`edge` = 边的粗细, 角比边更宽
pub fn grip(pos: Pos2, win: Rect, edge: f32) -> Option<Dir> {
    if !win.contains(pos) {
        return None;
    }
    let (l, r) = (pos.x - win.left(), win.right() - pos.x);
    let (t, b) = (pos.y - win.top(), win.bottom() - pos.y);
    let near_x_l = l < CORNER;
    let near_x_r = r < CORNER;
    let near_y_t = t < CORNER;
    let near_y_b = b < CORNER;
    if l < edge && near_y_t || t < edge && near_x_l { return Some(Dir::NorthWest); }
    if r < edge && near_y_t || t < edge && near_x_r { return Some(Dir::NorthEast); }
    if l < edge && near_y_b || b < edge && near_x_l { return Some(Dir::SouthWest); }
    if r < edge && near_y_b || b < edge && near_x_r { return Some(Dir::SouthEast); }
    if t < edge { return Some(Dir::North); }
    if b < edge { return Some(Dir::South); }
    if l < edge { return Some(Dir::West); }
    if r < edge { return Some(Dir::East); }
    None
}

fn cursor_for(d: Dir) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match d {
        Dir::North => C::ResizeNorth,
        Dir::South => C::ResizeSouth,
        Dir::East => C::ResizeEast,
        Dir::West => C::ResizeWest,
        Dir::NorthEast => C::ResizeNorthEast,
        Dir::NorthWest => C::ResizeNorthWest,
        Dir::SouthEast => C::ResizeSouthEast,
        Dir::SouthWest => C::ResizeSouthWest,
    }
}

/// 自绘标题栏要不要画 (系统标题栏 = 不画; 全屏 = 只留菜单行)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarMode {
    System,
    Full,
    MenuOnly,
}

pub fn bar_mode(system_title_bar: bool, fullscreen: bool) -> BarMode {
    if system_title_bar { BarMode::System } else if fullscreen { BarMode::MenuOnly } else { BarMode::Full }
}

/// 缩放把手生效吗: 只有自绘标题栏 + 非最大化 + 非全屏
pub fn grips_active(system_title_bar: bool, maximized: bool, fullscreen: bool) -> bool {
    !system_title_bar && !maximized && !fullscreen
}

/// Windows 无边框 + 可调大小的窗口最大化后, 实际矩形比工作区每边大出这么多 (逻辑点); 内容要缩进这么多
pub const MAX_OVERSHOOT: f32 = 8.0;

/// 内容缩进: 只有自绘标题栏 + 最大化 (非全屏) 才需要
pub fn max_inset(system_title_bar: bool, maximized: bool, fullscreen: bool) -> f32 {
    if !system_title_bar && maximized && !fullscreen && cfg!(windows) { MAX_OVERSHOOT } else { 0.0 }
}

impl App {
    pub(super) fn maximized_inset(&self, ctx: &egui::Context) -> f32 {
        let (m, f) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
        max_inset(self.cfg.system_title_bar, m, f)
    }

    /// 顶部这一条: 按模式画系统菜单行 / 自绘标题栏 / 全屏时的菜单行
    pub(super) fn top_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        if self.cfg.system_title_bar != self.sys.decor {
            self.sys.decor = self.cfg.system_title_bar;
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(self.sys.decor));
        }
        match bar_mode(self.cfg.system_title_bar, self.fullscreen) {
            BarMode::System | BarMode::MenuOnly => { egui::Panel::top("menu").frame(egui::Frame::new().fill(self.tc.bg_bar).inner_margin(egui::Margin::symmetric(4, 2))).show(root, |ui| self.menu(ui, ctx)); }
            BarMode::Full => self.title_bar(root, ctx),
        }
    }

    fn title_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let fill = self.tc.bg_bar;
        if self.sys.title_icon.is_none() {
            self.sys.title_icon = crate::appicon::window_icon().map(|ic| ctx.load_texture("title-icon", egui::ColorImage::from_rgba_unmultiplied([ic.width as usize, ic.height as usize], &ic.rgba), Default::default()));
        }
        let icon = self.sys.title_icon.clone();
        let mut run: Option<usize> = None;
        egui::Panel::top("titlebar").exact_size(BAR_H).frame(egui::Frame::new().fill(fill)).show(root, |ui| {
            let rect = ui.max_rect();
            ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, self.tc.border));
            // 先登记整条栏的拖动 / 双击区, 后面的按钮压在它上面 (egui 里后加的控件优先)
            let drag = ui.interact(rect, ui.make_persistent_id("titlebar-drag"), Sense::click_and_drag());
            if drag.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            } else if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            // 三块固定位置: 左 = 图标 + 菜单, 中 = 快捷按钮 + 面板开关 (栏够宽才画), 右 = 窗口按钮 (MenuBar 会占满剩余宽度, 所以不能靠「剩多少」来排)
            let quick_w = if rect.width() >= QUICK_MIN_W { QUICK_W } else { 0.0 };
            let right = Rect::from_min_max(egui::pos2(rect.right() - BTN_W * 3.0, rect.top()), rect.max);
            let mid = Rect::from_min_max(egui::pos2(right.left() - quick_w, rect.top()), egui::pos2(right.left(), rect.bottom()));
            let left = Rect::from_min_max(rect.min, egui::pos2(mid.left(), rect.bottom()));
            let lay = egui::Layout::left_to_right(egui::Align::Center);
            ui.scope_builder(egui::UiBuilder::new().max_rect(left).layout(lay), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(6.0);
                if let Some(t) = &icon { ui.image((t.id(), Vec2::splat(16.0))); }
                ui.add_space(2.0);
                ui.spacing_mut().button_padding = Vec2::new(5.0, 2.0);
                self.menu(ui, ctx);
            });
            if quick_w > 0.0 {
                ui.scope_builder(egui::UiBuilder::new().max_rect(mid).layout(lay), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (id, glyph) in QUICK {
                        let Some(i) = chrome::index_of(id) else { continue };
                        let e = &keymap::entries()[i];
                        if ui.add(egui::Button::new(*glyph).small().frame(false).min_size(Vec2::new(20.0, 22.0))).on_hover_text(crate::i18n::lk_for(e)).clicked() { run = Some(i); }
                    }
                    ui.separator();
                    for d in [Dock::Side, Dock::Bottom, Dock::Right] {
                        if self.dock_toggle_icon(ui, d) { self.collapse_dock(d); }
                    }
                });
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(right).layout(lay), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if win_button(ui, WinBtn::Minimize).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); }
                if win_button(ui, if maximized { WinBtn::Restore } else { WinBtn::Maximize }).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)); }
                if win_button(ui, WinBtn::Close).clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
            });
        });
        if let Some(i) = run { self.run_entry(i, ctx); }
    }

    /// 小图标: 一个矩形, 对应的那一块实心 = 该面板现在展开着
    fn dock_toggle_icon(&self, ui: &mut egui::Ui, d: Dock) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 22.0), Sense::click());
        let vis = ui.style().interact(&resp);
        if resp.hovered() { ui.painter().rect_filled(rect.shrink(1.0), 3.0, ui.visuals().widgets.hovered.weak_bg_fill); }
        let r = Rect::from_center_size(rect.center(), Vec2::new(14.0, 11.0));
        let expanded = self.dock_wanted(d) && !self.dock_collapsed(d);
        let part = match d {
            Dock::Side => Rect::from_min_max(r.min, egui::pos2(r.left() + 5.0, r.bottom())),
            Dock::Right => Rect::from_min_max(egui::pos2(r.right() - 5.0, r.top()), r.max),
            Dock::Bottom => Rect::from_min_max(egui::pos2(r.left(), r.bottom() - 4.0), r.max),
        };
        if expanded { ui.painter().rect_filled(part, 1.0, vis.fg_stroke.color); }
        ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, vis.fg_stroke.color), egui::StrokeKind::Inside);
        let tip = trz(match d {
            Dock::Side => "折叠 / 展开左侧栏",
            Dock::Right => "折叠 / 展开右侧面板",
            Dock::Bottom => "折叠 / 展开底部面板",
        });
        resp.on_hover_text(tip).clicked()
    }

    /// 窗口边缘的缩放把手 + 自绘窗口的 1 像素外框; 每帧末尾调一次
    pub(super) fn window_grips(&mut self, ctx: &egui::Context) {
        let (maximized, full) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
        if !grips_active(self.cfg.system_title_bar, maximized, full) { return; }
        let win = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::NULL.with("win-border")));
        painter.rect_stroke(win, 0.0, egui::Stroke::new(1.0, ctx.global_style().visuals.widgets.noninteractive.bg_stroke.color), egui::StrokeKind::Inside);
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        if let Some(d) = grip(pos, win, GRIP) {
            ctx.set_cursor_icon(cursor_for(d));
            if ctx.input(|i| i.pointer.primary_pressed()) { ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d)); }
        }
    }

    /// 命令: 切换系统 / 自绘标题栏 (下一帧 `top_bar` 把变化发给窗口)
    pub(super) fn toggle_system_title_bar(&mut self) {
        self.cfg.system_title_bar = !self.cfg.system_title_bar;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WinBtn {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn win_button(ui: &mut egui::Ui, kind: WinBtn) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(BTN_W, BAR_H), Sense::click());
    if resp.hovered() {
        let c = if kind == WinBtn::Close { Color32::from_rgb(196, 43, 28) } else { ui.visuals().widgets.hovered.weak_bg_fill };
        ui.painter().rect_filled(rect, 0.0, c);
    }
    let col = if resp.hovered() && kind == WinBtn::Close { Color32::WHITE } else { ui.visuals().widgets.inactive.fg_stroke.color };
    let st = egui::Stroke::new(1.2, col);
    let c = rect.center();
    let p = ui.painter();
    match kind {
        WinBtn::Minimize => { p.line_segment([egui::pos2(c.x - 5.0, c.y), egui::pos2(c.x + 5.0, c.y)], st); }
        WinBtn::Maximize => { p.rect_stroke(Rect::from_center_size(c, Vec2::splat(10.0)), 0.0, st, egui::StrokeKind::Middle); }
        WinBtn::Restore => {
            p.rect_stroke(Rect::from_center_size(c + Vec2::new(-1.0, 1.0), Vec2::splat(8.0)), 0.0, st, egui::StrokeKind::Middle);
            p.line_segment([egui::pos2(c.x - 2.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y - 5.0)], st);
            p.line_segment([egui::pos2(c.x + 5.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y + 2.0)], st);
        }
        WinBtn::Close => {
            p.line_segment([egui::pos2(c.x - 5.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y + 5.0)], st);
            p.line_segment([egui::pos2(c.x + 5.0, c.y - 5.0), egui::pos2(c.x - 5.0, c.y + 5.0)], st);
        }
    }
    resp.on_hover_text(trz(match kind {
        WinBtn::Minimize => "最小化",
        WinBtn::Maximize => "最大化",
        WinBtn::Restore => "还原",
        WinBtn::Close => "关闭",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> Rect {
        Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(800.0, 600.0))
    }

    #[test]
    fn edges_and_corners_map_to_resize_directions() {
        let w = win();
        let g = |x: f32, y: f32| grip(egui::pos2(x, y), w, GRIP);
        assert_eq!(g(400.0, 1.0), Some(Dir::North));
        assert_eq!(g(400.0, 599.0), Some(Dir::South));
        assert_eq!(g(1.0, 300.0), Some(Dir::West));
        assert_eq!(g(799.0, 300.0), Some(Dir::East));
        assert_eq!(g(1.0, 1.0), Some(Dir::NorthWest));
        assert_eq!(g(799.0, 1.0), Some(Dir::NorthEast));
        assert_eq!(g(1.0, 599.0), Some(Dir::SouthWest));
        assert_eq!(g(799.0, 599.0), Some(Dir::SouthEast));
        assert_eq!(g(10.0, 1.0), Some(Dir::NorthWest), "角比边宽: 离角 10 点的上边仍算角");
        assert_eq!(g(1.0, 590.0), Some(Dir::SouthWest));
    }

    #[test]
    fn the_inside_and_outside_have_no_grip() {
        let w = win();
        assert_eq!(grip(egui::pos2(400.0, 300.0), w, GRIP), None);
        assert_eq!(grip(egui::pos2(6.0, 300.0), w, GRIP), None, "边的粗细之外");
        assert_eq!(grip(egui::pos2(-1.0, 300.0), w, GRIP), None, "窗口外");
        assert_eq!(grip(egui::pos2(400.0, 700.0), w, GRIP), None);
    }

    #[test]
    fn bar_mode_depends_on_option_and_fullscreen() {
        assert_eq!(bar_mode(false, false), BarMode::Full);
        assert_eq!(bar_mode(false, true), BarMode::MenuOnly, "全屏: 不画标题栏 / 窗口按钮, 留菜单行");
        assert_eq!(bar_mode(true, false), BarMode::System);
        assert_eq!(bar_mode(true, true), BarMode::System);
    }

    #[test]
    fn grips_only_exist_for_the_custom_bar_in_a_normal_window() {
        assert!(grips_active(false, false, false));
        assert!(!grips_active(false, true, false));
        assert!(!grips_active(false, false, true));
        assert!(!grips_active(true, false, false));
    }

    #[test]
    fn only_a_maximized_custom_window_is_inset() {
        assert_eq!(max_inset(false, true, false), if cfg!(windows) { MAX_OVERSHOOT } else { 0.0 });
        assert_eq!(max_inset(true, true, false), 0.0);
        assert_eq!(max_inset(false, false, false), 0.0);
        assert_eq!(max_inset(false, true, true), 0.0);
    }

    #[test]
    fn quick_buttons_all_exist_in_the_registry() {
        for (id, _) in QUICK {
            assert!(chrome::index_of(id).is_some(), "{id}");
        }
    }

    #[test]
    fn toggling_the_option_flips_the_setting() {
        let g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut a = App::build(vec![], None, Trace::new(), crate::settings::Settings::default(), false);
        assert!(!a.cfg.system_title_bar, "默认自绘");
        a.toggle_system_title_bar();
        assert!(a.cfg.system_title_bar);
        drop(g);
    }
}
