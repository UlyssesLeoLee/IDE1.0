//! 美漫安装向导交互卡片与动作按钮控件 (Comic Action Buttons & Cards)
//! 高对比度、字号加大、粗体强化、全部图标纯矢量绘制，彻底杜绝字符回退空心方块。

use super::art::paint_comic_hard_shadow_rect;
use crate::ui_icons::{self, UiIcon};
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};

/// 美漫风格动作按钮 (不带图标版本，转发至带矢量图标实现)
pub fn comic_action_button(
    ui: &mut egui::Ui,
    text: &str,
    size: Vec2,
    primary: bool,
    btn_color: Color32,
) -> egui::Response {
    comic_action_button_with_icon(ui, None, text, size, primary, btn_color)
}

/// 美漫风格动作按钮 (可选前置纯矢量图标 + 居中整体排版)
pub fn comic_action_button_with_icon(
    ui: &mut egui::Ui,
    icon: Option<UiIcon>,
    text: &str,
    size: Vec2,
    primary: bool,
    btn_color: Color32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    let is_down = resp.is_pointer_button_down_on();
    let hovered = resp.hovered();

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

    // 基于 BT.601 感知亮度模型自适应文字颜色
    let text_col = contrast_text_color(btn_color);

    let font = egui::FontId::proportional(if primary { 14.5 } else { 12.5 });
    if let Some(ic) = icon {
        let galley = p.layout_no_wrap(text.to_string(), font, text_col);
        let icon_sz = if primary { 15.0 } else { 13.0 };
        let gap = 7.0;
        let total_w = icon_sz + gap + galley.size().x;
        let start_x = draw_rect.center().x - total_w * 0.5;
        let ic_rect = Rect::from_min_size(
            pos2(start_x, draw_rect.center().y - icon_sz * 0.5),
            Vec2::splat(icon_sz),
        );
        ui_icons::paint(p, ic_rect, ic, text_col);
        p.galley(
            pos2(
                ic_rect.right() + gap,
                draw_rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            text_col,
        );
    } else {
        p.text(
            draw_rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            font,
            text_col,
        );
    }
    resp
}

fn contrast_text_color(bg: Color32) -> Color32 {
    let lum = 0.299 * (bg.r() as f32) + 0.587 * (bg.g() as f32) + 0.114 * (bg.b() as f32);
    if lum > 165.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// 美漫只读信息卡片 (采用纯矢量图标徽章展示特性与参数)
pub fn comic_info_card(ui: &mut egui::Ui, title: &str, desc: &str, icon: UiIcon, accent: Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 52.0), egui::Sense::hover());
    let p = ui.painter();

    paint_comic_hard_shadow_rect(p, rect, Color32::from_rgb(22, 28, 42), 2.0, vec2(3.0, 3.0));

    // 左侧强调色条
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(5.5, rect.height())),
        0.0,
        accent,
    );

    // 矢量图标徽章 (圆角填充底盘)
    let tag_rect =
        Rect::from_center_size(pos2(rect.left() + 28.0, rect.center().y), vec2(28.0, 28.0));
    p.rect_filled(tag_rect.translate(vec2(1.5, 1.5)), 5.0, Color32::BLACK);
    p.rect_filled(tag_rect, 5.0, accent);
    p.rect_stroke(
        tag_rect,
        5.0,
        Stroke::new(1.8, Color32::BLACK),
        StrokeKind::Inside,
    );
    ui_icons::paint(p, tag_rect.shrink(5.5), icon, Color32::from_rgb(10, 14, 24));

    let text_x = rect.left() + 54.0;
    let text_clip = Rect::from_min_max(
        pos2(text_x, rect.top() + 2.0),
        pos2((rect.right() - 10.0).max(text_x + 4.0), rect.bottom() - 2.0),
    );
    let tp = p.with_clip_rect(text_clip);
    tp.text(
        pos2(text_x, rect.center().y - 8.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.5),
        Color32::WHITE,
    );
    tp.text(
        pos2(text_x, rect.center().y + 9.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(203, 213, 225),
    );
}

/// 美漫交互模式选择卡片 (用于维护模式：修复 / 卸载 / 更改配置)
pub fn comic_mode_card(
    ui: &mut egui::Ui,
    title: &str,
    desc: &str,
    badge_text: &str,
    icon: UiIcon,
    accent: Color32,
) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 64.0), egui::Sense::click());
    let p = ui.painter();
    let is_down = resp.is_pointer_button_down_on();
    let hovered = resp.hovered();

    let draw_rect = if is_down {
        rect.translate(vec2(1.5, 1.5))
    } else if hovered {
        rect.translate(vec2(-1.0, -1.0))
    } else {
        rect
    };
    let s_off = if is_down {
        vec2(1.5, 1.5)
    } else if hovered {
        vec2(4.0, 4.0)
    } else {
        vec2(3.0, 3.0)
    };
    let card_bg = if hovered {
        Color32::from_rgb(28, 36, 56)
    } else {
        Color32::from_rgb(20, 26, 40)
    };

    paint_comic_hard_shadow_rect(p, draw_rect, card_bg, 2.0, s_off);
    p.rect_filled(
        Rect::from_min_size(draw_rect.min, vec2(5.5, draw_rect.height())),
        0.0,
        accent,
    );

    let icon_plate = Rect::from_center_size(
        pos2(draw_rect.left() + 31.0, draw_rect.center().y),
        vec2(32.0, 32.0),
    );
    p.rect_filled(icon_plate.translate(vec2(1.5, 1.5)), 6.0, Color32::BLACK);
    p.rect_filled(icon_plate, 6.0, accent);
    p.rect_stroke(
        icon_plate,
        6.0,
        Stroke::new(1.8, Color32::BLACK),
        StrokeKind::Inside,
    );
    ui_icons::paint(
        p,
        icon_plate.shrink(6.5),
        icon,
        Color32::from_rgb(10, 14, 24),
    );

    let pill_rect = Rect::from_center_size(
        pos2(draw_rect.right() - 48.0, draw_rect.center().y),
        vec2(76.0, 24.0),
    );
    let pill_fg = contrast_text_color(accent);
    p.rect_filled(pill_rect.translate(vec2(1.5, 1.5)), 4.0, Color32::BLACK);
    p.rect_filled(pill_rect, 4.0, accent);
    p.rect_stroke(
        pill_rect,
        4.0,
        Stroke::new(1.6, Color32::BLACK),
        StrokeKind::Inside,
    );
    let pill_clip = pill_rect.shrink2(vec2(4.0, 2.0));
    let pill_p = p.with_clip_rect(pill_clip);
    pill_p.text(
        pos2(pill_rect.center().x - 6.0, pill_rect.center().y),
        egui::Align2::CENTER_CENTER,
        badge_text,
        egui::FontId::proportional(10.5),
        pill_fg,
    );
    ui_icons::paint(
        &pill_p,
        Rect::from_center_size(
            pos2(pill_rect.right() - 12.0, pill_rect.center().y),
            vec2(10.0, 10.0),
        ),
        UiIcon::ArrowRight,
        pill_fg,
    );

    let text_x = draw_rect.left() + 58.0;
    let text_clip = Rect::from_min_max(
        pos2(text_x, draw_rect.top() + 2.0),
        pos2(
            (pill_rect.left() - 8.0).max(text_x + 4.0),
            draw_rect.bottom() - 2.0,
        ),
    );
    let tp = p.with_clip_rect(text_clip);
    tp.text(
        pos2(text_x, draw_rect.center().y - 9.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.5),
        Color32::WHITE,
    );
    tp.text(
        pos2(text_x, draw_rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(10.8),
        Color32::from_rgb(203, 213, 225),
    );

    resp
}

/// 美漫交互复选卡片 (圆角内嵌开关指示器 + 纯矢量勾号徽章，杜绝空心方块与特殊字体符号)
pub fn comic_interactive_card(
    ui: &mut egui::Ui,
    checked: &mut bool,
    title: &str,
    desc: &str,
    tag: &str,
    accent: Color32,
) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 58.0), egui::Sense::click());
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
            Color32::from_rgb(28, 36, 56)
        } else {
            Color32::from_rgb(22, 28, 44)
        }
    } else if hovered {
        Color32::from_rgb(20, 25, 38)
    } else {
        Color32::from_rgb(16, 20, 32)
    };

    paint_comic_hard_shadow_rect(p, draw_rect, card_bg, 2.0, s_off);

    // 左侧强调色条
    p.rect_filled(
        Rect::from_min_size(draw_rect.min, vec2(5.5, draw_rect.height())),
        0.0,
        accent,
    );

    // 漫画序号徽章 (圆角填充底盘)
    let tag_rect = Rect::from_center_size(
        pos2(draw_rect.left() + 26.0, draw_rect.center().y),
        vec2(26.0, 26.0),
    );
    p.rect_filled(tag_rect.translate(vec2(1.5, 1.5)), 5.0, Color32::BLACK);
    p.rect_filled(tag_rect, 5.0, accent);
    p.rect_stroke(
        tag_rect,
        5.0,
        Stroke::new(1.8, Color32::BLACK),
        StrokeKind::Inside,
    );
    p.text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::proportional(12.0),
        Color32::BLACK,
    );

    // 勾选开关指示器 (圆角填充底盘 + 矢量对勾/圆点，绝不呈现裸空心方块)
    let box_center = pos2(draw_rect.left() + 58.0, draw_rect.center().y);
    let box_rect = Rect::from_center_size(box_center, vec2(22.0, 22.0));
    let box_bg = if *checked {
        Color32::from_rgb(2, 132, 199)
    } else {
        Color32::from_rgb(38, 49, 68)
    };
    p.rect_filled(box_rect.translate(vec2(1.5, 1.5)), 4.0, Color32::BLACK);
    p.rect_filled(box_rect, 4.0, box_bg);
    p.rect_stroke(
        box_rect,
        4.0,
        Stroke::new(1.8, Color32::BLACK),
        StrokeKind::Inside,
    );

    if *checked {
        ui_icons::paint_checkmark(p, box_rect.shrink(4.5), 2.8, Color32::WHITE);
    } else {
        p.circle_filled(box_rect.center(), 3.2, Color32::from_rgb(100, 116, 139));
    }

    // 右侧状态徽章 (提前计算以严格裁剪左侧文案区域)
    let stamp_rect = Rect::from_center_size(
        pos2(draw_rect.right() - 48.0, draw_rect.center().y),
        vec2(76.0, 24.0),
    );

    let text_x = draw_rect.left() + 80.0;
    let text_clip = Rect::from_min_max(
        pos2(text_x, draw_rect.top() + 2.0),
        pos2(
            (stamp_rect.left() - 8.0).max(text_x + 4.0),
            draw_rect.bottom() - 2.0,
        ),
    );
    let tp = p.with_clip_rect(text_clip);
    tp.text(
        pos2(text_x, draw_rect.center().y - 8.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.5),
        Color32::WHITE,
    );
    tp.text(
        pos2(text_x, draw_rect.center().y + 10.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(203, 213, 225),
    );

    let stamp_bg = if *checked {
        Color32::from_rgb(16, 185, 129)
    } else {
        Color32::from_rgb(30, 41, 59)
    };
    p.rect_filled(stamp_rect.translate(vec2(1.5, 1.5)), 4.0, Color32::BLACK);
    p.rect_filled(stamp_rect, 4.0, stamp_bg);
    p.rect_stroke(
        stamp_rect,
        4.0,
        Stroke::new(1.6, Color32::BLACK),
        StrokeKind::Inside,
    );
    if *checked {
        let ic_r = Rect::from_center_size(
            pos2(stamp_rect.left() + 12.0, stamp_rect.center().y),
            vec2(11.0, 11.0),
        );
        ui_icons::paint_checkmark(p, ic_r, 2.0, Color32::WHITE);
        p.text(
            pos2(stamp_rect.center().x + 6.0, stamp_rect.center().y),
            egui::Align2::CENTER_CENTER,
            "ENABLED",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );
    } else {
        p.circle_filled(
            pos2(stamp_rect.left() + 14.0, stamp_rect.center().y),
            3.0,
            Color32::from_rgb(148, 163, 184),
        );
        p.text(
            pos2(stamp_rect.center().x + 5.0, stamp_rect.center().y),
            egui::Align2::CENTER_CENTER,
            "OFF",
            egui::FontId::proportional(10.0),
            Color32::from_rgb(148, 163, 184),
        );
    }

    if resp.clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// 美漫用户协议确认卡片 (必须拉到底部解锁，支持纯矢量勾号与锁定状态)
pub fn comic_agreement_card(
    ui: &mut egui::Ui,
    checked: &mut bool,
    unlocked: bool,
    title: &str,
    desc: &str,
    accent: Color32,
) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 52.0), egui::Sense::click());
    let p = ui.painter();
    let is_down = resp.is_pointer_button_down_on() && unlocked;
    let hovered = resp.hovered() && unlocked;

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

    let card_bg = if !unlocked {
        Color32::from_rgb(18, 22, 32)
    } else if *checked {
        if hovered {
            Color32::from_rgb(28, 36, 56)
        } else {
            Color32::from_rgb(22, 28, 44)
        }
    } else if hovered {
        Color32::from_rgb(20, 25, 38)
    } else {
        Color32::from_rgb(16, 20, 32)
    };

    paint_comic_hard_shadow_rect(p, draw_rect, card_bg, 2.0, s_off);

    // 左侧强调色条
    let bar_col = if unlocked {
        accent
    } else {
        Color32::from_rgb(71, 85, 105)
    };
    p.rect_filled(
        Rect::from_min_size(draw_rect.min, vec2(5.5, draw_rect.height())),
        0.0,
        bar_col,
    );

    // 勾选方框
    let box_rect = Rect::from_center_size(
        pos2(draw_rect.left() + 26.0, draw_rect.center().y),
        vec2(22.0, 22.0),
    );
    let box_bg = if !unlocked {
        Color32::from_rgb(30, 41, 59)
    } else if *checked {
        accent
    } else {
        Color32::from_rgb(20, 26, 40)
    };
    p.rect_filled(box_rect.translate(vec2(1.5, 1.5)), 4.0, Color32::BLACK);
    p.rect_filled(box_rect, 4.0, box_bg);
    p.rect_stroke(
        box_rect,
        4.0,
        Stroke::new(1.8, Color32::BLACK),
        StrokeKind::Inside,
    );

    if *checked && unlocked {
        ui_icons::paint_checkmark(p, box_rect.shrink(4.0), 2.8, Color32::WHITE);
    } else if !unlocked {
        ui_icons::paint(
            p,
            box_rect.shrink(4.0),
            UiIcon::Lock,
            Color32::from_rgb(148, 163, 184),
        );
    } else {
        p.circle_filled(box_rect.center(), 3.0, Color32::from_rgb(100, 116, 139));
    }

    // 右侧状态徽章
    let stamp_rect = Rect::from_center_size(
        pos2(draw_rect.right() - 48.0, draw_rect.center().y),
        vec2(76.0, 24.0),
    );
    let (stamp_bg, stamp_label, stamp_icon) = if !unlocked {
        (Color32::from_rgb(51, 65, 85), "LOCKED", Some(UiIcon::Lock))
    } else if *checked {
        (
            Color32::from_rgb(16, 185, 129),
            "AGREED",
            Some(UiIcon::CheckCircle),
        )
    } else {
        (Color32::from_rgb(30, 41, 59), "PENDING", None)
    };

    p.rect_filled(stamp_rect.translate(vec2(1.5, 1.5)), 4.0, Color32::BLACK);
    p.rect_filled(stamp_rect, 4.0, stamp_bg);
    p.rect_stroke(
        stamp_rect,
        4.0,
        Stroke::new(1.6, Color32::BLACK),
        StrokeKind::Inside,
    );
    if let Some(ic) = stamp_icon {
        let ic_r = Rect::from_center_size(
            pos2(stamp_rect.left() + 12.0, stamp_rect.center().y),
            vec2(11.0, 11.0),
        );
        ui_icons::paint(p, ic_r, ic, Color32::WHITE);
    }
    p.text(
        pos2(
            stamp_rect.center().x + if stamp_icon.is_some() { 6.0 } else { 0.0 },
            stamp_rect.center().y,
        ),
        egui::Align2::CENTER_CENTER,
        stamp_label,
        egui::FontId::proportional(10.0),
        Color32::WHITE,
    );

    let text_x = draw_rect.left() + 50.0;
    let text_clip = Rect::from_min_max(
        pos2(text_x, draw_rect.top() + 2.0),
        pos2(
            (stamp_rect.left() - 8.0).max(text_x + 4.0),
            draw_rect.bottom() - 2.0,
        ),
    );
    let tp = p.with_clip_rect(text_clip);
    let title_col = if unlocked {
        Color32::WHITE
    } else {
        Color32::from_rgb(148, 163, 184)
    };
    tp.text(
        pos2(text_x, draw_rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.0),
        title_col,
    );
    let desc_col = if unlocked {
        Color32::from_rgb(203, 213, 225)
    } else {
        Color32::from_rgb(100, 116, 139)
    };
    tp.text(
        pos2(text_x, draw_rect.center().y + 9.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(10.5),
        desc_col,
    );

    if unlocked && resp.clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// 统一绘制美漫三渲二弹窗外壳底板（硬阴影、黑曜钛金底色、粗墨描边与顶部高光装饰条）
pub fn paint_comic_modal_chrome(p: &egui::Painter, rect: Rect, accent: Color32) {
    paint_comic_hard_shadow_rect(p, rect, Color32::from_rgb(18, 22, 32), 2.2, vec2(4.5, 4.5));
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), 4.5)),
        0.0,
        accent,
    );
}

/// 渲染统一的美漫三渲二前景提示弹窗（半透明墨水遮罩 + 居中漫画卡片 + 矢量图标徽章 + 确认按钮）
/// 返回 `true` 表示用户已点击确认或按下 `Enter` / `Escape` 关闭弹窗。
pub fn render_comic_alert_modal(
    ctx: &egui::Context,
    id_salt: &str,
    title: &str,
    message: &str,
    accent: Color32,
    icon: UiIcon,
    confirm_label: &str,
) -> bool {
    let mut dismissed = false;
    egui::Window::new(id_salt)
        .id(egui::Id::new(format!("{id_salt}_window")))
        .order(egui::Order::Tooltip)
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(vec2(450.0, 204.0))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let p = ui.painter();
            paint_comic_modal_chrome(p, rect, accent);

            // 顶栏图标徽章与标题
            let header_y = rect.top() + 34.0;
            let badge_rect =
                Rect::from_center_size(pos2(rect.left() + 34.0, header_y), vec2(30.0, 30.0));
            p.rect_filled(badge_rect.translate(vec2(1.5, 1.5)), 6.0, Color32::BLACK);
            p.rect_filled(badge_rect, 6.0, accent);
            p.rect_stroke(
                badge_rect,
                6.0,
                Stroke::new(1.8, Color32::BLACK),
                StrokeKind::Inside,
            );
            ui_icons::paint(
                p,
                badge_rect.shrink(6.0),
                icon,
                Color32::from_rgb(10, 14, 24),
            );

            let title_clip = Rect::from_min_max(
                pos2(rect.left() + 58.0, rect.top() + 12.0),
                pos2(
                    (rect.right() - 18.0).max(rect.left() + 62.0),
                    rect.top() + 54.0,
                ),
            );
            p.with_clip_rect(title_clip).text(
                pos2(rect.left() + 58.0, header_y),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::proportional(15.0),
                Color32::WHITE,
            );

            // 分割细线
            p.line_segment(
                [
                    pos2(rect.left() + 18.0, rect.top() + 58.0),
                    pos2(rect.right() - 18.0, rect.top() + 58.0),
                ],
                Stroke::new(1.2, Color32::from_rgb(38, 49, 68)),
            );

            // 正文说明区与底部按钮区
            let content_inner = Rect::from_min_max(
                pos2(rect.left() + 20.0, rect.top() + 68.0),
                pos2(rect.right() - 20.0, rect.bottom() - 18.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(content_inner), |ui| {
                let btn_h = 40.0;
                let msg_rect = Rect::from_min_max(
                    content_inner.min,
                    pos2(
                        content_inner.right(),
                        (content_inner.bottom() - btn_h - 10.0).max(content_inner.top() + 24.0),
                    ),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(msg_rect), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(message)
                                .size(12.2)
                                .color(Color32::from_rgb(203, 213, 225)),
                        )
                        .wrap(),
                    );
                });

                let btn_rect = Rect::from_min_max(
                    pos2(content_inner.left(), content_inner.bottom() - btn_h),
                    pos2(content_inner.right() - 2.0, content_inner.bottom()),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(btn_rect), |ui| {
                    if comic_action_button_with_icon(
                        ui,
                        Some(UiIcon::CheckCircle),
                        confirm_label,
                        vec2(btn_rect.width(), btn_h - 2.0),
                        true,
                        accent,
                    )
                    .clicked()
                    {
                        dismissed = true;
                    }
                });
            });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)) {
        dismissed = true;
    }
    dismissed
}

/// 美漫赛璐珞风格进度条（支持确定进度 `Some(0.0..=1.0)` 与不确定脉冲进度 `None`）
pub fn paint_comic_progress_bar(
    p: &egui::Painter,
    track_rect: Rect,
    progress: Option<f32>,
    accent_primary: Color32,
    accent_bright: Color32,
    accent_glow: Color32,
    time: f32,
) {
    let radius = (track_rect.height() * 0.5).min(10.0);
    p.rect_filled(track_rect.translate(vec2(1.5, 1.5)), radius, Color32::BLACK);
    p.rect_filled(track_rect, radius, Color32::from_rgb(11, 17, 32));
    p.rect_stroke(
        track_rect,
        radius,
        Stroke::new(1.8, Color32::BLACK),
        StrokeKind::Inside,
    );

    for frac in [0.25_f32, 0.50, 0.75] {
        let tx = track_rect.left() + track_rect.width() * frac;
        p.line_segment(
            [
                pos2(tx, track_rect.top() + 3.0),
                pos2(tx, track_rect.bottom() - 3.0),
            ],
            Stroke::new(1.0, Color32::from_rgb(38, 49, 68)),
        );
    }

    let inner = track_rect.shrink(2.0);
    if inner.width() <= 1.0 || inner.height() <= 1.0 {
        return;
    }

    let fill_rect = match progress {
        Some(v) => {
            let clamped = v.clamp(0.0, 1.0);
            let fill_w = inner.width() * clamped;
            if fill_w <= 0.5 {
                return;
            }
            Rect::from_min_size(inner.min, vec2(fill_w, inner.height()))
        }
        None => {
            let shuttle_w = (inner.width() * 0.30).clamp(24.0, inner.width());
            let travel = (inner.width() - shuttle_w).max(0.0);
            let phase = (time * 1.15).sin() * 0.5 + 0.5;
            Rect::from_min_size(
                pos2(inner.left() + travel * phase, inner.top()),
                vec2(shuttle_w, inner.height()),
            )
        }
    };

    let fill_round = (inner.height() * 0.5).min(fill_rect.width() * 0.5);
    p.rect_filled(fill_rect, fill_round, accent_primary);

    let clip_p = p.with_clip_rect(fill_rect);
    clip_p.rect_filled(
        Rect::from_min_size(
            fill_rect.min,
            vec2(fill_rect.width(), fill_rect.height() * 0.42),
        ),
        fill_round,
        Color32::from_rgba_unmultiplied(accent_glow.r(), accent_glow.g(), accent_glow.b(), 72),
    );
    let stripe_step = 18.0_f32;
    let stripe_shift = (time * 48.0) % stripe_step;
    let mut sx = fill_rect.left() - fill_rect.height() - stripe_step + stripe_shift;
    while sx < fill_rect.right() + stripe_step {
        clip_p.line_segment(
            [
                pos2(sx, fill_rect.bottom() + 2.0),
                pos2(sx + fill_rect.height() * 0.85, fill_rect.top() - 2.0),
            ],
            Stroke::new(
                3.0,
                Color32::from_rgba_unmultiplied(
                    accent_glow.r(),
                    accent_glow.g(),
                    accent_glow.b(),
                    72,
                ),
            ),
        );
        sx += stripe_step;
    }
    if progress.is_some() {
        clip_p.line_segment(
            [
                pos2(fill_rect.right() - 1.0, fill_rect.top()),
                pos2(fill_rect.right() - 1.0, fill_rect.bottom()),
            ],
            Stroke::new(2.0, accent_glow),
        );
    } else {
        p.rect_stroke(
            fill_rect,
            fill_round,
            Stroke::new(1.2, accent_bright),
            StrokeKind::Inside,
        );
    }
}
