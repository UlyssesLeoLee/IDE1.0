//! 自绘标题栏 (默认): 窗口创建时不要系统标题栏 (`with_decorations(false)`), 由这里画一条栏顶替 ——
//! 图标 + 紧凑菜单 + 快捷按钮 + 最小化 / 最大化 / 关闭。拖空白处移动、双击空白处最大化 / 还原、窗口边缘的缩放把手 (`BeginResize`)。
//! 「使用系统标题栏」是设置项 (`Settings::system_title_bar`), 切换时发 `ViewportCommand::Decorations`, 不用重启。
//! 全屏 (F11) 时标题栏的图标 / 按钮 / 拖动区都不画, 只留一条普通菜单行, 菜单仍然够得着; 最大化 / 全屏时没有缩放把手。
//! 命中判断 (`grip`) 是纯函数, 单测覆盖; 栏本身不增加空闲开销 (只在有输入事件时才重绘)。

use super::*;
use egui::viewport::ResizeDirection as Dir;

/// 标题栏高度 (逻辑点)
const BAR_H: f32 = 30.0;
/// 窗口按钮宽度
const BTN_W: f32 = 44.0;
/// 缩放把手的粗细 / 角上的边长
const GRIP: f32 = 5.0;
const CORNER: f32 = 12.0;
/// 快捷按钮: (命令 id, 文字)。面板开关用画出来的小图标, 不在这里。
const QUICK: &[(&str, &str)] = &[
    ("c.new-file", "新"),
    ("c.open-file", "开"),
    ("c.save", "存"),
    ("c.find", "找"),
    ("c.palette", ">_"),
    ("c.split", "||"),
    ("c.nema-convert", "⇥\\"),
];

/// 标题栏各块的宽度 (逻辑点): 左边距 + 图标 + 间隔、一个快捷图标钮 (含间隔)、面板开关三连 (含分隔线)、终端按钮、菜单与右区之间的余量、窗口标题文字的上限
const LEFT_FIXED: f32 = 6.0 + 16.0 + 6.0;
const QUICK_BTN: f32 = 26.0;
const DIFF_BTN_W: f32 = 30.0;
const GAP: f32 = 10.0;
const TITLE_MAX: f32 = 220.0;

/// 标题栏怎么排: 菜单永远完整显示、绝不和右边的按钮重叠; 空间不够时按优先级一样样撤 ——
/// 先撤快捷图标钮, 再撤 WinMerge 对比入口, 再撤窗口标题文字, 最后撤菜单的助记字母
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BarPlan {
    pub mnemonics: bool,
    pub title: bool,
    pub diff: bool,
    pub quick: usize,
}

impl BarPlan {
    /// 兼容旧版测试与属性访问
    #[inline]
    pub fn docks(&self) -> bool {
        self.diff
    }
}

/// `total` = 栏宽, `menu_mn` / `menu_plain` = 菜单带 / 不带助记字母时的总宽, `title_w` = 标题文字宽
pub fn plan_bar(total: f32, menu_mn: f32, menu_plain: f32, title_w: f32) -> BarPlan {
    let avail = total - BTN_W * 3.0 - LEFT_FIXED - GAP;
    let mnemonics = menu_mn <= avail;
    let mut rem = avail - if mnemonics { menu_mn } else { menu_plain };
    let mut take = |w: f32| {
        if rem >= w {
            rem -= w;
            true
        } else {
            false
        }
    };
    let title = take(title_w.min(TITLE_MAX) + 8.0);
    let diff = take(DIFF_BTN_W);
    let mut quick = 0;
    while quick < QUICK.len() && take(QUICK_BTN) {
        quick += 1;
    }
    BarPlan {
        mnemonics,
        title,
        diff,
        quick,
    }
}

/// 标题栏文字 (Tauri: 无文档「IDE1.0」, 有文档「文件名 - IDE1.0」)
pub fn window_title(doc: Option<&str>) -> String {
    match doc {
        Some(n) if !n.is_empty() => format!("{n} - IDE1.0"),
        _ => "IDE1.0".to_string(),
    }
}

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
    if l < edge && near_y_t || t < edge && near_x_l {
        return Some(Dir::NorthWest);
    }
    if r < edge && near_y_t || t < edge && near_x_r {
        return Some(Dir::NorthEast);
    }
    if l < edge && near_y_b || b < edge && near_x_l {
        return Some(Dir::SouthWest);
    }
    if r < edge && near_y_b || b < edge && near_x_r {
        return Some(Dir::SouthEast);
    }
    if t < edge {
        return Some(Dir::North);
    }
    if b < edge {
        return Some(Dir::South);
    }
    if l < edge {
        return Some(Dir::West);
    }
    if r < edge {
        return Some(Dir::East);
    }
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
    if system_title_bar {
        BarMode::System
    } else if fullscreen {
        BarMode::MenuOnly
    } else {
        BarMode::Full
    }
}

/// 缩放把手生效吗: 只有自绘标题栏 + 非最大化 + 非全屏
pub fn grips_active(system_title_bar: bool, maximized: bool, fullscreen: bool) -> bool {
    !system_title_bar && !maximized && !fullscreen
}

/// Windows 无边框 + 可调大小的窗口最大化后, 实际矩形比工作区每边大出这么多 (逻辑点); 内容要缩进这么多
pub const MAX_OVERSHOOT: f32 = 8.0;

/// 内容缩进: 只有自绘标题栏 + 最大化 (非全屏) 才需要
pub fn max_inset(system_title_bar: bool, maximized: bool, fullscreen: bool) -> f32 {
    if !system_title_bar && maximized && !fullscreen && cfg!(windows) {
        MAX_OVERSHOOT
    } else {
        0.0
    }
}

/// 菜单栏上的菜单 (与 `App::menu` 里的顺序一致, 完全对齐 Tauri 版 8 个菜单)
pub const MENU_KEYS: [&str; 8] = [
    "menu.file",
    "menu.edit",
    "menu.search",
    "menu.view",
    "menu.tool",
    "menu.setting",
    "menu.window",
    "menu.help",
];

impl App {
    pub(super) fn maximized_inset(&self, ctx: &egui::Context) -> f32 {
        let (m, f) = ctx.input(|i| {
            (
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });
        max_inset(self.cfg.system_title_bar, m, f)
    }

    /// 顶部这一条: 按模式画系统菜单行 / 自绘标题栏 / 全屏时的菜单行
    pub(super) fn top_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        if self.cfg.system_title_bar != self.sys.decor {
            self.sys.decor = self.cfg.system_title_bar;
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(self.sys.decor));
        }
        match bar_mode(self.cfg.system_title_bar, self.fullscreen) {
            BarMode::System | BarMode::MenuOnly => {
                egui::Panel::top("menu")
                    .frame(
                        egui::Frame::new()
                            .fill(self.tc.bg_bar)
                            .inner_margin(egui::Margin::symmetric(4, 2)),
                    )
                    .show(root, |ui| self.menu(ui, ctx));
            }
            BarMode::Full => self.title_bar(root, ctx),
        }
    }

    fn title_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let fill = self.tc.bg_bar;
        if self.sys.title_icon.is_none() {
            self.sys.title_icon = crate::appicon::window_icon().map(|ic| {
                ctx.load_texture(
                    "title-icon",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [ic.width as usize, ic.height as usize],
                        &ic.rgba,
                    ),
                    Default::default(),
                )
            });
        }
        let icon = self.sys.title_icon.clone();
        let mut run: Option<usize> = None;
        let mut diff_action: Option<DiffAction> = None;
        egui::Panel::top("titlebar")
            .exact_size(BAR_H)
            .frame(egui::Frame::new().fill(fill))
            .show(root, |ui| {
                let rect = ui.max_rect();
                if self.tc.is_translucent() {
                    super::glass_render::paint_facet_borders(ui.painter(), rect, &self.tc);
                } else {
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom() - 0.5,
                        egui::Stroke::new(1.0, self.tc.border),
                    );
                }
                // 先登记整条栏的拖动 / 双击区, 后面的按钮压在它上面 (egui 里后加的控件优先)
                let drag = ui.interact(
                    rect,
                    ui.make_persistent_id("titlebar-drag"),
                    Sense::click_and_drag(),
                );
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                } else if drag.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                // 三块固定位置: 左 = 图标 + 标题 + 菜单, 中 = 快捷按钮 + 面板开关 + 终端钮 (按宽度逐样撤, 见 `plan_bar`), 右 = 窗口按钮
                let title = window_title({
                    let d = &self.docs[self.active.min(self.docs.len() - 1)];
                    d.path.is_some().then_some(d.name.as_str())
                });
                let tc = self.tc;
                let (mw, mp, tw) = {
                    let keys = MENU_KEYS;
                    let (mut w1, mut w0) = (0.0, 0.0);
                    for k in keys {
                        let l = tr(k);
                        w1 += menuui::title_width(ui, l, menuui::mnemonic(k));
                        w0 += menuui::title_width(ui, l, ' ');
                    }
                    (
                        w1,
                        w0,
                        ui.painter()
                            .layout_no_wrap(title.clone(), egui::FontId::proportional(12.0), tc.fg)
                            .size()
                            .x,
                    )
                };
                let plan = plan_bar(rect.width(), mw, mp, tw);
                self.sys.mn_hide = !plan.mnemonics;
                let mid_w = plan.quick as f32 * QUICK_BTN + if plan.diff { DIFF_BTN_W } else { 0.0 };
                let right = Rect::from_min_max(
                    egui::pos2(rect.right() - BTN_W * 3.0, rect.top()),
                    rect.max,
                );
                let mid = Rect::from_min_max(
                    egui::pos2(right.left() - mid_w, rect.top()),
                    egui::pos2(right.left(), rect.bottom()),
                );
                let left = Rect::from_min_max(rect.min, egui::pos2(mid.left(), rect.bottom()));
                let lay = egui::Layout::left_to_right(egui::Align::Center);
                ui.scope_builder(egui::UiBuilder::new().max_rect(left).layout(lay), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(6.0);
                    if let Some(t) = &icon {
                        ui.image((t.id(), Vec2::splat(16.0)));
                    }
                    if plan.title {
                        ui.add_space(2.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(title.as_str()).size(12.0).strong(),
                            )
                            .truncate(),
                        );
                        ui.add_space(6.0);
                    } else {
                        ui.add_space(4.0);
                    }
                    self.menu(ui, ctx);
                });
                if mid_w > 0.0 {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(mid).layout(lay), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (id, _) in QUICK.iter().take(plan.quick) {
                            let Some(i) = chrome::index_of(id) else {
                                continue;
                            };
                            let e = &keymap::entries()[i];
                            let tip = crate::i18n::lk_for(e);
                            if crate::icons::icon_button(ui, id, tip).clicked() {
                                run = Some(i);
                            }
                        }
                        if plan.diff {
                            if let Some(act) = self.winmerge_diff_button(ui) {
                                diff_action = Some(act);
                            }
                        }
                    });
                }
                ui.scope_builder(egui::UiBuilder::new().max_rect(right).layout(lay), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if win_button(ui, WinBtn::Minimize).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    if win_button(
                        ui,
                        if maximized {
                            WinBtn::Restore
                        } else {
                            WinBtn::Maximize
                        },
                    )
                    .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if win_button(ui, WinBtn::Close).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        if let Some(i) = run {
            self.run_entry(i, ctx);
        }
        if let Some(act) = diff_action {
            match act {
                DiffAction::WithFile => self.diff_with_file(),
                DiffAction::WithTab(i) => self.start_diff(i),
                DiffAction::Next => self.diff_step(true),
                DiffAction::Prev => self.diff_step(false),
                DiffAction::List => self.diff_list_open = true,
                DiffAction::Clear => {
                    self.diff = None;
                    self.diff_list_open = false;
                }
            }
        }
    }

    /// WinMerge 对比快捷入口按钮与弹出菜单
    fn winmerge_diff_button(&self, ui: &mut egui::Ui) -> Option<DiffAction> {
        let is_active = self.diff.is_some();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 22.0), Sense::click());
        let mut action = None;

        let tip = if let Some(v) = &self.diff {
            format!("WinMerge 对比中: {} (点击展开对比菜单)", v.summary())
        } else {
            "WinMerge 对比 / 文件差异比对 (Diff)".to_string()
        };
        let resp = resp.on_hover_text(tip);

        let popup_id = egui::Popup::default_response_id(&resp);
        let is_menu_open = egui::Popup::is_id_open(ui.ctx(), popup_id);

        if resp.hovered() || is_active || is_menu_open {
            let bg = if is_active {
                Color32::from_rgba_unmultiplied(0, 160, 255, 45)
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            };
            ui.painter().rect_filled(rect.shrink(1.0), 3.0, bg);
            if is_active {
                ui.painter().rect_stroke(
                    rect.shrink(1.0),
                    3.0,
                    egui::Stroke::new(1.0, Color32::from_rgb(0, 170, 255)),
                    egui::StrokeKind::Inside,
                );
            }
        }

        paint_winmerge_icon(ui.painter(), rect, is_active, self.tc.fg);

        // 点击展开 WinMerge 风格差异比对菜单
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(210.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(egui::RichText::new("🔍 WinMerge 对比 (Diff)").strong());
            ui.separator();

            if let Some(v) = &self.diff {
                ui.label(egui::RichText::new(v.summary()).color(Color32::from_rgb(0, 190, 255)));
                if ui.button("⏭ 下一处差异 (F7)").clicked() {
                    action = Some(DiffAction::Next);
                    egui::Popup::close_all(ui.ctx());
                }
                if ui.button("⏮ 上一处差异 (Shift+F7)").clicked() {
                    action = Some(DiffAction::Prev);
                    egui::Popup::close_all(ui.ctx());
                }
                if ui.button("📜 查看差异列表...").clicked() {
                    action = Some(DiffAction::List);
                    egui::Popup::close_all(ui.ctx());
                }
                if ui.button("❌ 退出对比 (清除差异)").clicked() {
                    action = Some(DiffAction::Clear);
                    egui::Popup::close_all(ui.ctx());
                }
                ui.separator();
            }

            if ui.button("📂 与本地文件比较 (WinMerge)...").clicked() {
                action = Some(DiffAction::WithFile);
                egui::Popup::close_all(ui.ctx());
            }

            let cur = self.active.min(self.docs.len() - 1);
            let other_docs: Vec<_> = self
                .docs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != cur)
                .collect();

            if !other_docs.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("📑 与已打开标签页比较:").size(11.0));
                for (i, d) in other_docs {
                    let name = if d.name.is_empty() { "未命名" } else { &d.name };
                    if ui.button(format!("  ↔ {name}")).clicked() {
                        action = Some(DiffAction::WithTab(i));
                        egui::Popup::close_all(ui.ctx());
                    }
                }
            }
        });

        action
    }

    /// 窗口边缘的缩放把手 + 自绘窗口的 1 像素外框; 每帧末尾调一次
    pub(super) fn window_grips(&mut self, ctx: &egui::Context) {
        let (maximized, full) = ctx.input(|i| {
            (
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });
        if !grips_active(self.cfg.system_title_bar, maximized, full) {
            return;
        }
        let win = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::NULL.with("win-border"),
        ));
        painter.rect_stroke(
            win,
            0.0,
            egui::Stroke::new(
                1.0,
                ctx.global_style()
                    .visuals
                    .widgets
                    .noninteractive
                    .bg_stroke
                    .color,
            ),
            egui::StrokeKind::Inside,
        );
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if let Some(d) = grip(pos, win, GRIP) {
            ctx.set_cursor_icon(cursor_for(d));
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
            }
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
        let c = if kind == WinBtn::Close {
            Color32::from_rgb(196, 43, 28)
        } else {
            ui.visuals().widgets.hovered.weak_bg_fill
        };
        ui.painter().rect_filled(rect, 0.0, c);
    }
    let col = if resp.hovered() && kind == WinBtn::Close {
        Color32::WHITE
    } else {
        ui.visuals().widgets.inactive.fg_stroke.color
    };
    let st = egui::Stroke::new(1.2, col);
    let c = rect.center();
    let p = ui.painter();
    match kind {
        WinBtn::Minimize => {
            p.line_segment([egui::pos2(c.x - 5.0, c.y), egui::pos2(c.x + 5.0, c.y)], st);
        }
        WinBtn::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(10.0)),
                0.0,
                st,
                egui::StrokeKind::Middle,
            );
        }
        WinBtn::Restore => {
            p.rect_stroke(
                Rect::from_center_size(c + Vec2::new(-1.0, 1.0), Vec2::splat(8.0)),
                0.0,
                st,
                egui::StrokeKind::Middle,
            );
            p.line_segment(
                [
                    egui::pos2(c.x - 2.0, c.y - 5.0),
                    egui::pos2(c.x + 5.0, c.y - 5.0),
                ],
                st,
            );
            p.line_segment(
                [
                    egui::pos2(c.x + 5.0, c.y - 5.0),
                    egui::pos2(c.x + 5.0, c.y + 2.0),
                ],
                st,
            );
        }
        WinBtn::Close => {
            p.line_segment(
                [
                    egui::pos2(c.x - 5.0, c.y - 5.0),
                    egui::pos2(c.x + 5.0, c.y + 5.0),
                ],
                st,
            );
            p.line_segment(
                [
                    egui::pos2(c.x + 5.0, c.y - 5.0),
                    egui::pos2(c.x - 5.0, c.y + 5.0),
                ],
                st,
            );
        }
    }
    resp.tip(trz(match kind {
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
        assert_eq!(
            g(10.0, 1.0),
            Some(Dir::NorthWest),
            "角比边宽: 离角 10 点的上边仍算角"
        );
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
        assert_eq!(
            bar_mode(false, true),
            BarMode::MenuOnly,
            "全屏: 不画标题栏 / 窗口按钮, 留菜单行"
        );
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
        assert_eq!(
            max_inset(false, true, false),
            if cfg!(windows) { MAX_OVERSHOOT } else { 0.0 }
        );
        assert_eq!(max_inset(true, true, false), 0.0);
        assert_eq!(max_inset(false, false, false), 0.0);
        assert_eq!(max_inset(false, true, true), 0.0);
    }

    #[test]
    fn title_text_follows_tauri() {
        assert_eq!(window_title(None), "IDE1.0");
        assert_eq!(window_title(Some("a.rs")), "a.rs - IDE1.0");
    }

    #[test]
    fn quick_buttons_all_exist_in_the_registry() {
        for (id, _) in QUICK {
            assert!(chrome::index_of(id).is_some(), "{id}");
        }
    }

    /// 菜单宽度取真实值的量级: 8 个菜单, 带助记字母约 55 / 个, 不带约 42 / 个
    const MW: f32 = 8.0 * 55.0;
    const MP: f32 = 8.0 * 42.0;

    #[test]
    fn the_plan_never_lets_the_menu_overflow_at_any_width() {
        for w in [580.0, 700.0, 760.0, 900.0, 1100.0, 1280.0, 1600.0, 2200.0] {
            let p = plan_bar(w, MW, MP, 50.0);
            let menu = if p.mnemonics { MW } else { MP };
            let mid = p.quick as f32 * QUICK_BTN + if p.diff { DIFF_BTN_W } else { 0.0 };
            let title = if p.title { 58.0 } else { 0.0 };
            assert!(
                LEFT_FIXED + title + menu + mid + BTN_W * 3.0 <= w,
                "宽 {w}: 排不下 {p:?}"
            );
        }
    }

    #[test]
    fn the_plan_drops_things_progressively_as_the_window_narrows() {
        let wide = plan_bar(1600.0, MW, MP, 50.0);
        assert!(
            wide.mnemonics && wide.title && wide.quick == QUICK.len()
        );
        let mid = plan_bar(700.0, MW, MP, 50.0);
        assert!(
            mid.mnemonics,
            "700: 菜单 (带助记) 在"
        );
        assert!(
            mid.quick < QUICK.len(),
            "700: 快捷钮已被撤掉几个, 不压菜单"
        );
        let narrow = plan_bar(580.0, MW, MP, 50.0);
        assert!(!narrow.mnemonics && narrow.quick == 0);
    }

    #[test]
    fn toggling_the_option_flips_the_setting() {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut a = App::build(
            vec![],
            None,
            Trace::new(),
            crate::settings::Settings::default(),
            false,
        );
        assert!(!a.cfg.system_title_bar, "默认自绘");
        a.toggle_system_title_bar();
        assert!(a.cfg.system_title_bar);
        drop(g);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffAction {
    WithFile,
    WithTab(usize),
    Next,
    Prev,
    List,
    Clear,
}

/// 绘制 WinMerge 经典风格的“双文档差异对比”图标 (左右并排小文档，带标志性差异色条)
fn paint_winmerge_icon(painter: &egui::Painter, rect: Rect, is_active: bool, theme_fg: Color32) {
    let center = rect.center();
    let page_w = 7.0;
    let page_h = 11.0;
    let page_gap = 3.0;

    let left_min = egui::pos2(center.x - page_w - page_gap * 0.5, center.y - page_h * 0.5);
    let left_rect = Rect::from_min_size(left_min, Vec2::new(page_w, page_h));

    let right_min = egui::pos2(center.x + page_gap * 0.5, center.y - page_h * 0.5);
    let right_rect = Rect::from_min_size(right_min, Vec2::new(page_w, page_h));

    let stroke_color = if is_active {
        Color32::from_rgb(0, 190, 255)
    } else {
        theme_fg.gamma_multiply(0.85)
    };
    let doc_stroke = egui::Stroke::new(1.0, stroke_color);

    // 绘制左右两个文档背景底板与边框
    painter.rect_filled(left_rect, 1.0, Color32::from_black_alpha(40));
    painter.rect_stroke(left_rect, 1.0, doc_stroke, egui::StrokeKind::Inside);

    painter.rect_filled(right_rect, 1.0, Color32::from_black_alpha(40));
    painter.rect_stroke(right_rect, 1.0, doc_stroke, egui::StrokeKind::Inside);

    // 文档内的代码行
    let y1 = left_rect.top() + 3.0;
    let y2 = left_rect.top() + 5.5;
    let y3 = left_rect.top() + 8.5;

    // 普通代码行 (低透明度)
    let normal_stroke = egui::Stroke::new(1.0, stroke_color.gamma_multiply(0.35));
    painter.hline(left_rect.left() + 1.5..=left_rect.right() - 1.5, y1, normal_stroke);
    painter.hline(right_rect.left() + 1.5..=right_rect.right() - 1.5, y1, normal_stroke);

    // WinMerge 标志性高亮差异行 (左暖黄，右亮绿/鲜红)
    let left_diff = Color32::from_rgb(255, 185, 30);
    let right_diff = if is_active {
        Color32::from_rgb(40, 220, 110)
    } else {
        Color32::from_rgb(255, 90, 90)
    };

    painter.hline(left_rect.left() + 1.5..=left_rect.right() - 1.5, y2, egui::Stroke::new(1.2, left_diff));
    painter.hline(right_rect.left() + 1.5..=right_rect.right() - 1.5, y2, egui::Stroke::new(1.2, right_diff));

    painter.hline(left_rect.left() + 1.5..=left_rect.right() - 1.5, y3, normal_stroke);
    painter.hline(right_rect.left() + 1.5..=right_rect.right() - 1.5, y3, normal_stroke);

    // 活跃状态下，在两页之间绘制高亮对比连接点
    if is_active {
        painter.circle_filled(center, 1.5, Color32::from_rgb(0, 200, 255));
    }
}
