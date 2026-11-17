//! 美漫安装向导交互卡片与动作按钮控件 (Comic Action Buttons & Cards)

use super::art::paint_comic_hard_shadow_rect;
use eframe::egui;
use egui::{pos2, vec2, Color32, Rect, Stroke, Vec2};

/// 美漫风格动作按钮 (带有纯黑硬阴影与按压位移反馈)
pub fn comic_action_button(
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

/// 美漫只读信息卡片 (展示特性与参数)
pub fn comic_info_card(ui: &mut egui::Ui, title: &str, desc: &str, tag: &str, accent: Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 40.0), egui::Sense::hover());
    let p = ui.painter();

    paint_comic_hard_shadow_rect(p, rect, Color32::from_rgb(26, 31, 50), 2.0, vec2(3.0, 3.0));

    // 左侧强调色条
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(5.0, rect.height())),
        0.0,
        accent,
    );

    // 漫画序号徽章
    let tag_rect =
        Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), vec2(24.0, 22.0));
    paint_comic_hard_shadow_rect(p, tag_rect, accent, 1.5, vec2(1.5, 1.5));
    p.text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::proportional(11.0),
        Color32::BLACK,
    );

    let text_x = rect.left() + 48.0;
    p.text(
        pos2(text_x, rect.center().y - 6.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
    p.text(
        pos2(text_x, rect.center().y + 8.0),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(9.8),
        Color32::from_rgb(160, 175, 200),
    );
}

/// 美漫交互复选卡片
pub fn comic_interactive_card(
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

    // 左侧强调色条
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

    // 漫画复选框
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
