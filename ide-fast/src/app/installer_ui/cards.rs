//! 美漫安装向导交互卡片与动作按钮控件 (Comic Action Buttons & Cards)
//! 高对比度、字号加大、粗体强化、色彩心理学优化 (宝蓝/天青/黑曜钛金)

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

    // 基于 BT.601 感知亮度模型自适应文字颜色 (亮色底黑字，暗色与宝蓝底白字)
    let lum = 0.299 * (btn_color.r() as f32)
        + 0.587 * (btn_color.g() as f32)
        + 0.114 * (btn_color.b() as f32);
    let text_col = if lum > 165.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };

    p.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(if primary { 14.5 } else { 12.5 }),
        text_col,
    );
    resp
}

/// 美漫只读信息卡片 (展示特性与参数)
pub fn comic_info_card(ui: &mut egui::Ui, title: &str, desc: &str, tag: &str, accent: Color32) {
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

    // 漫画序号徽章
    let tag_rect =
        Rect::from_center_size(pos2(rect.left() + 26.0, rect.center().y), vec2(26.0, 26.0));
    paint_comic_hard_shadow_rect(p, tag_rect, accent, 1.8, vec2(1.5, 1.5));
    p.text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::proportional(12.0),
        Color32::BLACK,
    );

    let text_x = rect.left() + 52.0;
    p.text(
        pos2(text_x, rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.5),
        Color32::WHITE,
    );
    p.text(
        pos2(text_x, rect.center().y + 9.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(203, 213, 225),
    );
}

/// 美漫交互复选卡片 (高度自适应加大至 58.0px，文字清晰加粗，对比度强化)
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

    // 漫画序号徽章
    let tag_rect = Rect::from_center_size(
        pos2(draw_rect.left() + 26.0, draw_rect.center().y),
        vec2(26.0, 26.0),
    );
    paint_comic_hard_shadow_rect(p, tag_rect, accent, 1.8, vec2(1.5, 1.5));
    p.text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::proportional(12.0),
        Color32::BLACK,
    );

    // 漫画复选框 (尺寸加至 22×22，对比强烈，勾选白线清晰)
    let box_center = pos2(draw_rect.left() + 58.0, draw_rect.center().y);
    let box_rect = Rect::from_center_size(box_center, vec2(22.0, 22.0));
    let box_bg = if *checked {
        Color32::from_rgb(2, 132, 199)
    } else {
        Color32::from_rgb(30, 41, 59)
    };
    paint_comic_hard_shadow_rect(p, box_rect, box_bg, 2.0, vec2(1.5, 1.5));

    if *checked {
        let p1 = pos2(box_rect.left() + 4.5, box_rect.center().y);
        let p2 = pos2(box_rect.center().x - 0.5, box_rect.bottom() - 4.5);
        let p3 = pos2(box_rect.right() - 4.0, box_rect.top() + 5.0);
        let check_st = Stroke::new(3.0, Color32::WHITE);
        p.line_segment([p1, p2], check_st);
        p.line_segment([p2, p3], check_st);
    }

    let text_x = draw_rect.left() + 80.0;
    p.text(
        pos2(text_x, draw_rect.center().y - 8.5),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.5),
        Color32::WHITE,
    );
    p.text(
        pos2(text_x, draw_rect.center().y + 10.5),
        egui::Align2::LEFT_CENTER,
        desc,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(203, 213, 225),
    );

    let stamp_rect = Rect::from_center_size(
        pos2(draw_rect.right() - 48.0, draw_rect.center().y),
        vec2(74.0, 24.0),
    );
    if *checked {
        paint_comic_hard_shadow_rect(
            p,
            stamp_rect,
            Color32::from_rgb(16, 185, 129),
            1.8,
            vec2(1.5, 1.5),
        );
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            "★ ENABLED",
            egui::FontId::proportional(10.5),
            Color32::WHITE,
        );
    } else {
        paint_comic_hard_shadow_rect(
            p,
            stamp_rect,
            Color32::from_rgb(30, 41, 59),
            1.8,
            vec2(1.5, 1.5),
        );
        p.text(
            stamp_rect.center(),
            egui::Align2::CENTER_CENTER,
            "○ OFF",
            egui::FontId::proportional(10.5),
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
