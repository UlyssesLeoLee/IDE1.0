//! 描边小按钮的样式 (Tauri 的 `.md-btn` / `.bottom-toggle`): 透明底、1px 边、圆角 4, 悬停叠一层 `bg_hover`。
//! 只改传进来的这个 `Ui` 的样式 (用 `ui.scope` 包住), 不影响别处的按钮。

use eframe::egui::{self, Color32, Rect, Stroke, Ui, Vec2};

/// 描边色: 深色 `#454545`, 浅色 `#d0d7de`
pub fn edge(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x45, 0x45, 0x45)
    } else {
        Color32::from_rgb(0xd0, 0xd7, 0xde)
    }
}

/// 查找框输入框底色: 深色 `#3c3c3c`, 浅色白
pub fn input_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x3c, 0x3c, 0x3c)
    } else {
        Color32::WHITE
    }
}

/// 把这个 `Ui` 里的按钮改成描边小按钮
pub fn outline(ui: &mut Ui) {
    let dark = ui.visuals().dark_mode;
    let e = edge(dark);
    let hover = if dark {
        Color32::from_white_alpha(14)
    } else {
        Color32::from_black_alpha(14)
    };
    let v = ui.visuals_mut();
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::new(1.0, e);
        w.corner_radius = 4.0.into();
    }
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.active.weak_bg_fill = hover;
}

/// 为当前 Ui 作用域配置深度契合各个主题外观的交互控件样式 (用于菜单、抽屉与浮层按钮)
pub fn apply_theme_button_style(ui: &mut Ui, tc: &crate::view::ThemeColors) {
    let v = ui.visuals_mut();
    if tc.comic {
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = Stroke::new(1.5, Color32::BLACK);
        v.widgets.inactive.corner_radius = 4.0.into();
        v.widgets.inactive.fg_stroke.color = tc.fg;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(255, 230, 0);
        v.widgets.hovered.bg_stroke = Stroke::new(1.8, Color32::BLACK);
        v.widgets.hovered.corner_radius = 4.0.into();
        v.widgets.hovered.fg_stroke.color = Color32::BLACK;
        v.widgets.active.weak_bg_fill = Color32::from_rgb(255, 204, 0);
        v.widgets.active.bg_stroke = Stroke::new(2.0, Color32::BLACK);
        v.widgets.active.fg_stroke.color = Color32::BLACK;
    } else if tc.retro16 {
        v.widgets.inactive.corner_radius = 0.0.into();
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, tc.border_strong);
        v.widgets.inactive.fg_stroke.color = tc.fg;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(34, 20, 60);
        v.widgets.hovered.corner_radius = 0.0.into();
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0, 240, 255));
        v.widgets.hovered.fg_stroke.color = Color32::from_rgb(255, 215, 0);
        v.widgets.active.weak_bg_fill = Color32::from_rgb(48, 30, 80);
        v.widgets.active.fg_stroke.color = Color32::from_rgb(255, 215, 0);
    } else if tc.crystal || tc.clear {
        v.widgets.inactive.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 18);
        v.widgets.inactive.bg_stroke =
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 45));
        v.widgets.inactive.corner_radius = 6.0.into();
        v.widgets.inactive.fg_stroke.color = tc.fg;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 45);
        v.widgets.hovered.bg_stroke =
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 230, 255, 140));
        v.widgets.hovered.corner_radius = 6.0.into();
        v.widgets.hovered.fg_stroke.color = Color32::WHITE;
    } else {
        let (inact_bg, inact_strk) = if tc.dark {
            (Color32::from_white_alpha(10), Color32::from_white_alpha(22))
        } else {
            (Color32::from_black_alpha(8), Color32::from_black_alpha(24))
        };
        let (hov_bg, hov_strk) = if tc.dark {
            (Color32::from_white_alpha(22), Color32::from_white_alpha(45))
        } else {
            (Color32::from_black_alpha(16), Color32::from_black_alpha(42))
        };
        v.widgets.inactive.weak_bg_fill = inact_bg;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, inact_strk);
        v.widgets.inactive.corner_radius = 5.0.into();
        v.widgets.inactive.fg_stroke.color = tc.fg;
        v.widgets.hovered.weak_bg_fill = hov_bg;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, hov_strk);
        v.widgets.hovered.corner_radius = 5.0.into();
        v.widgets.hovered.fg_stroke.color = if tc.dark {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
    }
}

/// 统一缩进 / 折叠按钮配色三元组: `(填充底色, 边框, 前景文字色, 圆角)`
pub fn unified_collapse_palette(
    tc: &crate::view::ThemeColors,
    active: bool,
    hovered: bool,
    pressed: bool,
) -> (Color32, Stroke, Color32, f32) {
    if tc.comic {
        let hot = hovered || pressed || active;
        let (bg, fg) = if hot {
            (Color32::from_rgb(255, 204, 0), Color32::BLACK)
        } else {
            (Color32::from_rgb(32, 36, 52), Color32::WHITE)
        };
        (bg, Stroke::new(1.5, Color32::BLACK), fg, 4.0)
    } else if tc.retro16 {
        let hot = hovered || pressed || active;
        let (bg, fg) = if hot {
            (Color32::from_rgb(0, 255, 136), Color32::BLACK)
        } else {
            (
                Color32::from_rgb(21, 15, 43),
                Color32::from_rgb(255, 215, 0),
            )
        };
        (
            bg,
            Stroke::new(1.0, Color32::from_rgb(255, 215, 0)),
            fg,
            2.0,
        )
    } else if tc.crystal || tc.clear {
        let alpha = if hovered || active { 56 } else { 28 };
        (
            Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 72)),
            Color32::WHITE,
            4.0,
        )
    } else {
        let bg = if active || pressed {
            tc.bg_active
        } else if hovered {
            tc.bg_hover
        } else {
            tc.bg_bar
        };
        let fg = if active || hovered { tc.fg } else { tc.fg_dim };
        (bg, Stroke::new(1.0, edge(tc.dark)), fg, 4.0)
    }
}

/// 构造统一样式的 Widget 缩进 / 折叠按钮
pub fn unified_collapse_button<'a>(
    label: &'a str,
    tc: &crate::view::ThemeColors,
    active: bool,
) -> egui::Button<'a> {
    let (bg, stroke, fg, radius) = unified_collapse_palette(tc, active, false, false);
    egui::Button::new(egui::RichText::new(label).size(11.0).color(fg))
        .fill(bg)
        .stroke(stroke)
        .corner_radius(radius)
        .min_size(Vec2::new(22.0, 20.0))
}

/// 使用 `Painter` 绘制与 `unified_collapse_button` 完全一致的统一缩进 / 折叠按钮
pub fn paint_unified_collapse_btn(
    ui: &Ui,
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    tc: &crate::view::ThemeColors,
    active: bool,
    hovered: bool,
    pressed: bool,
) {
    let (bg, stroke, fg, radius) = unified_collapse_palette(tc, active, hovered, pressed);
    let draw_rect = if tc.comic {
        if !pressed {
            painter.rect_filled(rect.translate(Vec2::new(1.5, 1.5)), radius, Color32::BLACK);
            rect
        } else {
            rect.translate(Vec2::new(1.5, 1.5))
        }
    } else {
        rect
    };
    painter.rect_filled(draw_rect, radius, bg);
    painter.rect_stroke(draw_rect, radius, stroke, egui::StrokeKind::Inside);
    painter.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        fg,
    );
    let _ = ui;
}

/// AI Agent 界面专用交互命令按钮角色
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AgentBtnRole {
    /// 核心主操作：发送、应用、允许等 (高对比高能量高光)
    Primary,
    /// 危险或破坏性操作：取消、拒绝、清空等 (警示色)
    Danger,
    /// 辅助设置：模型设置、配置参数等 (精致功能钮)
    Settings,
    /// 快捷技能胶囊：/review, /test 等 (波普网点与胶囊徽章)
    Skill,
}

/// 绘制高设计感、深度贴合各个主题风格的 AI Agent 命令按钮
pub fn agent_command_button(
    ui: &mut Ui,
    text: &str,
    role: AgentBtnRole,
    tc: &crate::view::ThemeColors,
) -> egui::Response {
    let padding = match role {
        AgentBtnRole::Skill => Vec2::new(8.0, 3.0),
        AgentBtnRole::Settings => Vec2::new(10.0, 5.0),
        _ => Vec2::new(14.0, 6.0),
    };
    let font_size = match role {
        AgentBtnRole::Skill => 11.0,
        _ => 12.0,
    };
    let font_id = egui::FontId::proportional(font_size);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE);
    let desired_size = Vec2::new(
        galley.size().x + padding.x * 2.0,
        (galley.size().y + padding.y * 2.0).max(26.0),
    );
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let hovered = resp.hovered();
        let pressed = resp.is_pointer_button_down_on();

        if tc.comic {
            // === 经典美漫与波普卡通渲染风格 (Comic Cel-Shaded) ===
            // 粗墨线 2.0px 纯黑外框 + 纯黑硬折阴影 ([2.5, 2.5]) + 悬浮上浮/按压下陷
            let radius = 4.0;
            let shadow_offset = if pressed {
                Vec2::new(0.5, 0.5)
            } else if hovered {
                Vec2::new(3.5, 3.5)
            } else {
                Vec2::new(2.5, 2.5)
            };
            let btn_offset = if pressed {
                Vec2::new(2.0, 2.0)
            } else if hovered {
                Vec2::new(-0.5, -0.5)
            } else {
                Vec2::ZERO
            };
            let draw_rect = rect.translate(btn_offset);

            // 纯黑实心硬阴影
            p.rect_filled(draw_rect.translate(shadow_offset), radius, Color32::BLACK);

            let (fill, fg) = match role {
                AgentBtnRole::Primary => {
                    let c = if hovered {
                        Color32::from_rgb(255, 240, 50)
                    } else {
                        Color32::from_rgb(255, 230, 0)
                    };
                    (c, Color32::BLACK)
                }
                AgentBtnRole::Danger => {
                    let c = if hovered {
                        Color32::from_rgb(255, 70, 110)
                    } else {
                        Color32::from_rgb(240, 40, 80)
                    };
                    (c, Color32::WHITE)
                }
                AgentBtnRole::Skill => {
                    let c = if hovered {
                        Color32::from_rgb(80, 240, 255)
                    } else {
                        Color32::from_rgb(0, 220, 255)
                    };
                    (c, Color32::BLACK)
                }
                AgentBtnRole::Settings => {
                    let c = if hovered {
                        Color32::from_rgb(45, 52, 78)
                    } else {
                        Color32::from_rgb(32, 38, 58)
                    };
                    (c, Color32::WHITE)
                }
            };
            p.rect_filled(draw_rect, radius, fill);
            p.rect_stroke(
                draw_rect,
                radius,
                Stroke::new(2.0, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
            // 顶部高光内描边 (Comic Cel-Shaded Glint)
            if draw_rect.width() > 14.0 && !pressed {
                p.line_segment(
                    [
                        draw_rect.left_top() + Vec2::new(3.0, 2.0),
                        draw_rect.right_top() + Vec2::new(-3.0, 2.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(110)),
                );
            }
            p.text(
                draw_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font_id,
                fg,
            );
        } else if tc.retro16 {
            // === 16-Bit 复古街机像素风格 (Retro 16-Bit Arcade) ===
            // 纯直角 (radius 0px) + 双色立体浮雕光影边框 (斜面高光与投影)
            let draw_rect = if pressed {
                rect.translate(Vec2::new(1.5, 1.5))
            } else if hovered {
                rect.translate(Vec2::new(-0.5, -0.5))
            } else {
                rect
            };
            let (bg, fg, hi_color, lo_color) = match role {
                AgentBtnRole::Primary => (
                    if hovered {
                        Color32::from_rgb(0, 255, 140)
                    } else {
                        Color32::from_rgb(0, 204, 102)
                    },
                    Color32::from_rgb(10, 15, 25),
                    Color32::from_rgb(180, 255, 210),
                    Color32::from_rgb(0, 90, 45),
                ),
                AgentBtnRole::Danger => (
                    if hovered {
                        Color32::from_rgb(255, 75, 95)
                    } else {
                        Color32::from_rgb(210, 35, 55)
                    },
                    Color32::WHITE,
                    Color32::from_rgb(255, 170, 180),
                    Color32::from_rgb(110, 12, 22),
                ),
                AgentBtnRole::Skill => (
                    if hovered {
                        Color32::from_rgb(52, 34, 90)
                    } else {
                        Color32::from_rgb(34, 20, 60)
                    },
                    Color32::from_rgb(0, 240, 255),
                    Color32::from_rgb(0, 255, 255),
                    Color32::from_rgb(15, 8, 30),
                ),
                AgentBtnRole::Settings => (
                    if hovered {
                        Color32::from_rgb(50, 42, 80)
                    } else {
                        Color32::from_rgb(30, 24, 50)
                    },
                    Color32::from_rgb(255, 215, 0),
                    Color32::from_rgb(255, 235, 110),
                    Color32::from_rgb(15, 10, 25),
                ),
            };
            p.rect_filled(draw_rect, 0.0, bg);
            let (top_left_col, bot_right_col) = if pressed {
                (lo_color, hi_color)
            } else {
                (hi_color, lo_color)
            };
            p.line_segment(
                [draw_rect.left_top(), draw_rect.right_top()],
                Stroke::new(1.5, top_left_col),
            );
            p.line_segment(
                [draw_rect.left_top(), draw_rect.left_bottom()],
                Stroke::new(1.5, top_left_col),
            );
            p.line_segment(
                [draw_rect.left_bottom(), draw_rect.right_bottom()],
                Stroke::new(1.5, bot_right_col),
            );
            p.line_segment(
                [draw_rect.right_top(), draw_rect.right_bottom()],
                Stroke::new(1.5, bot_right_col),
            );
            p.text(
                draw_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font_id,
                fg,
            );
        } else if tc.crystal || tc.clear {
            // === 晶莹剔透玻璃拟物风格 (Crystal / Clear Glassmorphism) ===
            // 胶囊大圆角 + 棱镜边缘微光 + 磨砂通透质感
            let radius = if role == AgentBtnRole::Skill {
                12.0
            } else {
                7.0
            };
            let (bg, border, fg) = match role {
                AgentBtnRole::Primary => {
                    let a = if pressed {
                        130
                    } else if hovered {
                        95
                    } else {
                        60
                    };
                    (
                        Color32::from_rgba_unmultiplied(0, 180, 255, a),
                        Stroke::new(1.2, Color32::from_rgba_unmultiplied(120, 235, 255, 210)),
                        Color32::WHITE,
                    )
                }
                AgentBtnRole::Danger => {
                    let a = if pressed {
                        120
                    } else if hovered {
                        85
                    } else {
                        50
                    };
                    (
                        Color32::from_rgba_unmultiplied(255, 50, 90, a),
                        Stroke::new(1.2, Color32::from_rgba_unmultiplied(255, 130, 160, 200)),
                        Color32::WHITE,
                    )
                }
                AgentBtnRole::Skill => {
                    let a = if pressed {
                        85
                    } else if hovered {
                        55
                    } else {
                        32
                    };
                    (
                        Color32::from_rgba_unmultiplied(255, 255, 255, a),
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 240, 255, 150)),
                        Color32::from_rgb(180, 240, 255),
                    )
                }
                AgentBtnRole::Settings => {
                    let a = if pressed {
                        65
                    } else if hovered {
                        45
                    } else {
                        22
                    };
                    (
                        Color32::from_rgba_unmultiplied(255, 255, 255, a),
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 90)),
                        Color32::from_white_alpha(225),
                    )
                }
            };
            p.rect_filled(rect, radius, bg);
            p.rect_stroke(rect, radius, border, egui::StrokeKind::Inside);

            // 水晶棱镜微光 (Prismatic specular highlight)
            if rect.width() > 14.0 && !pressed {
                let shine_start = egui::pos2(rect.left() + 5.0, rect.top() + 1.2);
                let shine_end = egui::pos2(rect.right() - 5.0, rect.top() + 1.2);
                p.line_segment(
                    [shine_start, shine_end],
                    Stroke::new(
                        1.0,
                        Color32::from_white_alpha(if hovered { 130 } else { 85 }),
                    ),
                );
                // 悬停时在底部棱边叠加淡粉色散反射 (Prism Pink Dispersion)
                if hovered {
                    p.line_segment(
                        [
                            egui::pos2(rect.left() + 6.0, rect.bottom() - 1.2),
                            egui::pos2(rect.right() - 6.0, rect.bottom() - 1.2),
                        ],
                        Stroke::new(1.0, tc.mat_prism_pink.gamma_multiply(0.4)),
                    );
                }
            }
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font_id,
                fg,
            );
        } else {
            // === 现代极简扁平与微质感风格 (Fluent Modern Dark / Light) ===
            let radius = if role == AgentBtnRole::Skill {
                11.0
            } else {
                5.5
            };
            let (bg, border, fg) = match role {
                AgentBtnRole::Primary => {
                    let (bg_c, border_c) = if tc.dark {
                        if pressed {
                            (
                                Color32::from_rgb(24, 90, 185),
                                Color32::from_rgb(18, 75, 155),
                            )
                        } else if hovered {
                            (
                                Color32::from_rgb(45, 130, 240),
                                Color32::from_rgb(60, 145, 255),
                            )
                        } else {
                            (
                                Color32::from_rgb(31, 111, 235),
                                Color32::from_rgb(40, 120, 245),
                            )
                        }
                    } else {
                        if pressed {
                            (Color32::from_rgb(6, 80, 175), Color32::from_rgb(4, 65, 145))
                        } else if hovered {
                            (
                                Color32::from_rgb(18, 120, 235),
                                Color32::from_rgb(25, 130, 245),
                            )
                        } else {
                            (
                                Color32::from_rgb(9, 105, 218),
                                Color32::from_rgb(15, 115, 230),
                            )
                        }
                    };
                    (bg_c, Stroke::new(1.0, border_c), Color32::WHITE)
                }
                AgentBtnRole::Danger => {
                    let (bg_c, border_c) = if tc.dark {
                        if pressed {
                            (
                                Color32::from_rgb(175, 30, 30),
                                Color32::from_rgb(150, 22, 22),
                            )
                        } else if hovered {
                            (
                                Color32::from_rgb(235, 60, 60),
                                Color32::from_rgb(250, 80, 80),
                            )
                        } else {
                            (
                                Color32::from_rgb(218, 54, 51),
                                Color32::from_rgb(230, 65, 62),
                            )
                        }
                    } else {
                        if pressed {
                            (
                                Color32::from_rgb(155, 18, 28),
                                Color32::from_rgb(130, 12, 20),
                            )
                        } else if hovered {
                            (
                                Color32::from_rgb(225, 42, 52),
                                Color32::from_rgb(240, 55, 65),
                            )
                        } else {
                            (
                                Color32::from_rgb(207, 34, 46),
                                Color32::from_rgb(220, 45, 58),
                            )
                        }
                    };
                    (bg_c, Stroke::new(1.0, border_c), Color32::WHITE)
                }
                AgentBtnRole::Skill => {
                    let (fill, stroke_c, text_c) = if tc.dark {
                        let f = if pressed {
                            Color32::from_gray(55)
                        } else if hovered {
                            Color32::from_gray(45)
                        } else {
                            Color32::from_gray(35)
                        };
                        let st = if hovered {
                            Color32::from_rgb(56, 189, 248)
                        } else {
                            Color32::from_gray(75)
                        };
                        (f, st, Color32::from_rgb(125, 211, 252))
                    } else {
                        let f = if pressed {
                            Color32::from_gray(225)
                        } else if hovered {
                            Color32::from_gray(238)
                        } else {
                            Color32::from_gray(246)
                        };
                        let st = if hovered {
                            Color32::from_rgb(9, 105, 218)
                        } else {
                            Color32::from_gray(210)
                        };
                        (f, st, Color32::from_rgb(9, 105, 218))
                    };
                    (fill, Stroke::new(1.0, stroke_c), text_c)
                }
                AgentBtnRole::Settings => {
                    let (fill, stroke_c, text_c) = if tc.dark {
                        let f = if pressed {
                            Color32::from_white_alpha(32)
                        } else if hovered {
                            Color32::from_white_alpha(22)
                        } else {
                            Color32::from_white_alpha(10)
                        };
                        let st = if hovered {
                            Color32::from_white_alpha(55)
                        } else {
                            Color32::from_white_alpha(28)
                        };
                        (f, st, tc.fg)
                    } else {
                        let f = if pressed {
                            Color32::from_black_alpha(28)
                        } else if hovered {
                            Color32::from_black_alpha(18)
                        } else {
                            Color32::from_black_alpha(8)
                        };
                        let st = if hovered {
                            Color32::from_black_alpha(50)
                        } else {
                            Color32::from_black_alpha(24)
                        };
                        (f, st, tc.fg)
                    };
                    (fill, Stroke::new(1.0, stroke_c), text_c)
                }
            };
            p.rect_filled(rect, radius, bg);
            if border != Stroke::NONE {
                p.rect_stroke(rect, radius, border, egui::StrokeKind::Inside);
            }
            // 顶部高光微反光，赋予原生操作系统材质物理厚度 (Tactile Top Micro-Highlight)
            if rect.width() > 12.0 && !pressed {
                let glint_alpha = if tc.dark { 48 } else { 90 };
                p.line_segment(
                    [
                        egui::pos2(rect.left() + 3.0, rect.top() + 1.0),
                        egui::pos2(rect.right() - 3.0, rect.top() + 1.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(glint_alpha)),
                );
            }
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font_id,
                fg,
            );
        }
    }
    resp
}

/// 绘制深度契合各个主题风格的下拉触发按钮 (支持 Comic/Retro16/Crystal/Fluent 风格自适应)
pub fn dropdown_button(
    ui: &mut Ui,
    text: &str,
    max_w: f32,
    tc: &crate::view::ThemeColors,
) -> egui::Response {
    let font = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let arrow_space = 16.0;
    let pad_x = 8.0;
    let desired_w = (galley.size().x + pad_x * 2.0 + arrow_space).clamp(72.0, max_w);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, 24.0), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let hovered = resp.hovered();
        let pressed = resp.is_pointer_button_down_on();

        let (draw_rect, arrow_col, text_col) = if tc.comic {
            let radius = 4.0;
            let shadow_offset = if pressed {
                Vec2::new(0.5, 0.5)
            } else if hovered {
                Vec2::new(2.5, 2.5)
            } else {
                Vec2::new(1.5, 1.5)
            };
            let btn_offset = if pressed {
                Vec2::new(1.0, 1.0)
            } else if hovered {
                Vec2::new(-0.5, -0.5)
            } else {
                Vec2::ZERO
            };
            let d_rect = rect.translate(btn_offset);
            p.rect_filled(d_rect.translate(shadow_offset), radius, Color32::BLACK);
            let fill = if hovered {
                Color32::from_rgb(255, 240, 60)
            } else {
                tc.bg_bar
            };
            let fg = if hovered { Color32::BLACK } else { tc.fg };
            p.rect_filled(d_rect, radius, fill);
            p.rect_stroke(
                d_rect,
                radius,
                Stroke::new(1.8, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
            (d_rect, fg, fg)
        } else if tc.retro16 {
            let d_rect = if pressed {
                rect.translate(Vec2::new(1.0, 1.0))
            } else {
                rect
            };
            let bg = if hovered {
                Color32::from_rgb(34, 20, 60)
            } else {
                tc.bg_bar
            };
            p.rect_filled(d_rect, 0.0, bg);
            let (tl, br) = if pressed {
                (Color32::from_rgb(15, 8, 30), Color32::from_rgb(0, 240, 255))
            } else if hovered {
                (Color32::from_rgb(0, 240, 255), Color32::from_rgb(15, 8, 30))
            } else {
                (tc.border_strong, tc.border)
            };
            p.line_segment(
                [d_rect.left_top(), d_rect.right_top()],
                Stroke::new(1.0, tl),
            );
            p.line_segment(
                [d_rect.left_top(), d_rect.left_bottom()],
                Stroke::new(1.0, tl),
            );
            p.line_segment(
                [d_rect.left_bottom(), d_rect.right_bottom()],
                Stroke::new(1.0, br),
            );
            p.line_segment(
                [d_rect.right_top(), d_rect.right_bottom()],
                Stroke::new(1.0, br),
            );
            let fg = if hovered {
                Color32::from_rgb(255, 215, 0)
            } else {
                tc.fg
            };
            (d_rect, fg, fg)
        } else if tc.crystal || tc.clear {
            let radius = 6.0;
            let fill = if pressed {
                Color32::from_rgba_unmultiplied(255, 255, 255, 65)
            } else if hovered {
                Color32::from_rgba_unmultiplied(255, 255, 255, 45)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 22)
            };
            let stroke = if hovered {
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 230, 255, 160))
            } else {
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 65))
            };
            p.rect_filled(rect, radius, fill);
            p.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
            if rect.width() > 14.0 && !pressed {
                p.line_segment(
                    [
                        egui::pos2(rect.left() + 4.0, rect.top() + 1.0),
                        egui::pos2(rect.right() - 4.0, rect.top() + 1.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(100)),
                );
            }
            let fg = if hovered { Color32::WHITE } else { tc.fg };
            (rect, fg, fg)
        } else {
            let radius = 5.0;
            let (fill, stroke_c) = if tc.dark {
                if pressed {
                    (Color32::from_white_alpha(32), Color32::from_white_alpha(45))
                } else if hovered {
                    (Color32::from_white_alpha(24), Color32::from_white_alpha(50))
                } else {
                    (Color32::from_white_alpha(12), Color32::from_white_alpha(26))
                }
            } else {
                if pressed {
                    (Color32::from_black_alpha(26), Color32::from_black_alpha(40))
                } else if hovered {
                    (Color32::from_black_alpha(18), Color32::from_black_alpha(45))
                } else {
                    (Color32::from_black_alpha(8), Color32::from_black_alpha(25))
                }
            };
            p.rect_filled(rect, radius, fill);
            p.rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, stroke_c),
                egui::StrokeKind::Inside,
            );
            if rect.width() > 12.0 && !pressed {
                let glint_alpha = if tc.dark { 40 } else { 85 };
                p.line_segment(
                    [
                        egui::pos2(rect.left() + 3.0, rect.top() + 1.0),
                        egui::pos2(rect.right() - 3.0, rect.top() + 1.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(glint_alpha)),
                );
            }
            let fg = if hovered {
                if tc.dark {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                }
            } else {
                tc.fg_dim
            };
            (rect, fg, fg)
        };

        let text_clip = Rect::from_min_max(
            egui::pos2(draw_rect.left() + pad_x, draw_rect.top() + 1.0),
            egui::pos2(
                (draw_rect.right() - arrow_space).max(draw_rect.left() + pad_x + 4.0),
                draw_rect.bottom() - 1.0,
            ),
        );
        p.with_clip_rect(text_clip).galley(
            egui::pos2(
                text_clip.left(),
                draw_rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            text_col,
        );
        let cx = draw_rect.right() - 8.5;
        let cy = draw_rect.center().y + 0.5;
        p.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(cx - 3.2, cy - 2.0),
                egui::pos2(cx + 3.2, cy - 2.0),
                egui::pos2(cx, cy + 2.4),
            ],
            arrow_col,
            egui::Stroke::NONE,
        ));
    }
    resp
}

/// 快捷技能栏徽章药丸胶囊
pub fn agent_skill_pill(
    ui: &mut Ui,
    label: &str,
    tip: &str,
    tc: &crate::view::ThemeColors,
) -> egui::Response {
    agent_command_button(ui, label, AgentBtnRole::Skill, tc).on_hover_text(tip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_and_input_colours_match_tauri() {
        assert_eq!(edge(true), Color32::from_rgb(69, 69, 69));
        assert_eq!(edge(false), Color32::from_rgb(208, 215, 222));
        assert_eq!(input_bg(true), Color32::from_rgb(60, 60, 60));
        assert_eq!(input_bg(false), Color32::WHITE);
    }

    #[test]
    fn test_agent_command_button_rendering_across_themes() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            // 1. Comic 风格
            let mut tc_comic = crate::view::ThemeColors::builtin(true);
            tc_comic.comic = true;
            let r1 = agent_command_button(ui, "发送", AgentBtnRole::Primary, &tc_comic);
            assert!(r1.rect.width() > 0.0);
            let r2 = agent_command_button(ui, "取消", AgentBtnRole::Danger, &tc_comic);
            assert!(r2.rect.width() > 0.0);
            let r3 = agent_skill_pill(ui, "/review", "代码审查", &tc_comic);
            assert!(r3.rect.width() > 0.0);

            // 2. Retro16 风格
            let mut tc_retro = crate::view::ThemeColors::builtin(true);
            tc_retro.retro16 = true;
            let r4 = agent_command_button(ui, "发送", AgentBtnRole::Primary, &tc_retro);
            assert!(r4.rect.width() > 0.0);
            let r5 = agent_command_button(ui, "模型设置…", AgentBtnRole::Settings, &tc_retro);
            assert!(r5.rect.width() > 0.0);

            // 3. Crystal 风格
            let mut tc_crystal = crate::view::ThemeColors::builtin(true);
            tc_crystal.crystal = true;
            let r6 = agent_command_button(ui, "发送", AgentBtnRole::Primary, &tc_crystal);
            assert!(r6.rect.width() > 0.0);

            // 4. Fluent 现代风格
            let tc_dark = crate::view::ThemeColors::builtin(true);
            let r7 = agent_command_button(ui, "发送", AgentBtnRole::Primary, &tc_dark);
            assert!(r7.rect.width() > 0.0);

            // 5. 测试多主题下拉菜单按钮渲染
            let d1 = dropdown_button(ui, "Agent #1", 160.0, &tc_comic);
            assert!(d1.rect.width() > 0.0);
            let d2 = dropdown_button(ui, "全员协同调度", 160.0, &tc_retro);
            assert!(d2.rect.width() > 0.0);
            let d3 = dropdown_button(ui, "Crystal Dropdown", 160.0, &tc_crystal);
            assert!(d3.rect.width() > 0.0);
            let d4 = dropdown_button(ui, "Fluent Dropdown", 160.0, &tc_dark);
            assert!(d4.rect.width() > 0.0);

            // 6. 测试主题作用域应用
            apply_theme_button_style(ui, &tc_comic);
            apply_theme_button_style(ui, &tc_crystal);
            apply_theme_button_style(ui, &tc_retro);
            apply_theme_button_style(ui, &tc_dark);
        });
        out.textures_delta.clear();
    }
}
