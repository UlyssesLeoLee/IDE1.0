//! 常驻便笺信息窗 (Windows Sticky Notes 桌面平替便笺)
//!
//! 特性全对齐 Windows Sticky Notes:
//! - 多便笺管理与便笺列表浏览搜索
//! - 7 种经典便笺纸张色彩 (黄/绿/粉/紫/蓝/灰/炭黑) 与主题自适应
//! - 底部快捷格式化工具栏 (粗体/斜体/下划线/删除线/待办清单/项目符号/时间戳/剪贴板复制/字数统计)
//! - 顶栏快捷操作 (+ 新建、📑 列表、🎨 调色盘、📌 常驻置顶、🗑️ 删除、✕ 隐藏)
//! - 纯自绘多主题画风适配:
//!   - Retro 16-Bit: 街机像素微凹凸双色边框 + 像素字体
//!   - Comic 3渲2: 2.5px 纯黑手绘描边 + 纯黑硬边阴影 + 波普高反差高对比纸张
//!   - Crystal / Glass: 水晶半透明毛玻璃 + 顶沿白色切面光泽 + 极简光晕
//!   - Fluent Dark / Light: 现代微软便笺柔和圆角与质感纸张
//! - 全自动持久化保存 (%APPDATA%\IDE1.0 Fast\sticky_notes.json)，不丢任何字

use super::*;
use eframe::egui::{
    self, pos2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use serde_json::{json, Value};
use std::path::PathBuf;

/// 7 种经典便笺纸张色彩名称
pub const COLOR_NAMES: &[&str] = &[
    "经典便笺黄",
    "清新薄荷绿",
    "樱花柔粉",
    "薰衣草紫",
    "天空冰蓝",
    "浅石灰",
    "炭黑夜色",
    "跟随主题",
];

/// 单个便笺数据
#[derive(Clone, Debug, PartialEq)]
pub struct StickyNote {
    pub id: String,
    pub title: String,
    pub content: String,
    pub color_idx: usize,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl StickyNote {
    pub fn new(index: usize) -> Self {
        let now_str = current_timestamp();
        let title = format!("便笺 {}", index);
        let id = format!(
            "note_{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        Self {
            id,
            title,
            content: String::new(),
            color_idx: (index - 1) % 6, // 默认轮换经典明亮色
            pinned: false,
            created_at: now_str.clone(),
            updated_at: now_str,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "title": self.title,
            "content": self.content,
            "color_idx": self.color_idx,
            "pinned": self.pinned,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: v["id"].as_str()?.to_string(),
            title: v["title"].as_str().unwrap_or("").to_string(),
            content: v["content"].as_str().unwrap_or("").to_string(),
            color_idx: v["color_idx"].as_u64().unwrap_or(0) as usize,
            pinned: v["pinned"].as_bool().unwrap_or(false),
            created_at: v["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: v["updated_at"].as_str().unwrap_or("").to_string(),
        })
    }
}

/// 便笺全局状态管理
#[derive(Clone, Debug, PartialEq)]
pub struct StickyNotesState {
    pub open: bool,
    pub active_id: String,
    pub notes: Vec<StickyNote>,
    pub win_pos: Option<[f32; 2]>,
    pub win_size: [f32; 2],
    pub show_list: bool,
    pub search_query: String,
    pub color_picker_open: bool,
    pub dirty: bool,
    pub focus_needed: bool,
    pub has_focus: bool,
    pub toast: Option<String>,
}

impl Default for StickyNotesState {
    fn default() -> Self {
        Self::default_initial()
    }
}

impl StickyNotesState {
    /// 默认初始状态 (含一份友好的快速上手便笺)
    pub fn default_initial() -> Self {
        let now_str = current_timestamp();
        let default_note = StickyNote {
            id: "note_welcome".to_string(),
            title: "欢迎使用便笺 📝".to_string(),
            content: "# 欢迎使用便笺 📝\n\n这是专为快速记录灵感、待办清单、临时代码片段设计的常驻信息窗（平替 Windows 便笺）。\n\n## 常用特性\n- 顶栏 `+` 可随时新增便笺，`📑` 切换便笺列表与检索\n- 顶栏 `🎨` 可切换 7 种便笺色彩，支持深浅色与各大主题\n- 顶栏 `📌` 支持常驻置顶，保持便笺在最前\n- 底栏提供粗体、斜体、删除线、代办清单与时间戳\n- 所有便笺内容实时自动保存，重启永不丢失！\n\n## 今日待办\n- [ ] 尝试切换便笺色彩 🎨\n- [ ] 试用 Markdown 待办清单与格式化\n- [ ] 体验全主题画风（Retro 16-Bit / Comic 美漫 / 水晶透明）\n".to_string(),
            color_idx: 0, // 经典黄
            pinned: false,
            created_at: now_str.clone(),
            updated_at: now_str,
        };
        Self {
            open: false,
            active_id: "note_welcome".to_string(),
            notes: vec![default_note],
            win_pos: None,
            win_size: [320.0, 380.0],
            show_list: false,
            search_query: String::new(),
            color_picker_open: false,
            dirty: false,
            focus_needed: false,
            has_focus: false,
            toast: None,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "open": self.open,
            "active_id": self.active_id,
            "win_pos": self.win_pos,
            "win_size": self.win_size,
            "show_list": self.show_list,
            "notes": self.notes.iter().map(|n| n.to_json()).collect::<Vec<_>>(),
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let mut notes = Vec::new();
        if let Some(arr) = v["notes"].as_array() {
            for item in arr {
                if let Some(n) = StickyNote::from_json(item) {
                    notes.push(n);
                }
            }
        }
        if notes.is_empty() {
            return None;
        }

        let open = v["open"].as_bool().unwrap_or(false);
        let active_id = v["active_id"].as_str().unwrap_or(&notes[0].id).to_string();
        let show_list = v["show_list"].as_bool().unwrap_or(false);

        let win_pos = v["win_pos"].as_array().and_then(|arr| {
            if arr.len() == 2 {
                let x = arr[0].as_f64()? as f32;
                let y = arr[1].as_f64()? as f32;
                Some([x, y])
            } else {
                None
            }
        });

        let win_size = v["win_size"]
            .as_array()
            .and_then(|arr| {
                if arr.len() == 2 {
                    let w = arr[0].as_f64()? as f32;
                    let h = arr[1].as_f64()? as f32;
                    Some([w, h])
                } else {
                    None
                }
            })
            .unwrap_or([320.0, 380.0]);

        Some(Self {
            open,
            active_id,
            notes,
            win_pos,
            win_size,
            show_list,
            search_query: String::new(),
            color_picker_open: false,
            dirty: false,
            focus_needed: false,
            has_focus: false,
            toast: None,
        })
    }

    /// 从磁盘加载便笺数据
    pub fn load() -> Self {
        if let Some(path) = sticky_notes_path() {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
                    if let Some(mut state) = Self::from_json(&v) {
                        if !state.notes.iter().any(|n| n.id == state.active_id) {
                            state.active_id = state.notes[0].id.clone();
                        }
                        state.dirty = false;
                        return state;
                    }
                }
            }
        }
        Self::default_initial()
    }

    /// 自动保存到磁盘
    pub fn save(&mut self) {
        let Some(path) = sticky_notes_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_vec_pretty(&self.to_json()) {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
        self.dirty = false;
    }

    /// 标记数据脏并写入
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.save();
    }

    /// 新建一张便笺
    pub fn new_note(&mut self) {
        let next_idx = self.notes.len() + 1;
        let note = StickyNote::new(next_idx);
        self.active_id = note.id.clone();
        self.notes.push(note);
        self.show_list = false;
        self.focus_needed = true;
        self.mark_dirty();
    }

    /// 删除指定便笺
    pub fn delete_note(&mut self, id: &str) {
        if let Some(pos) = self.notes.iter().position(|n| n.id == id) {
            self.notes.remove(pos);
        }
        if self.notes.is_empty() {
            let note = StickyNote::new(1);
            self.active_id = note.id.clone();
            self.notes.push(note);
        } else if !self.notes.iter().any(|n| n.id == self.active_id) {
            self.active_id = self.notes[0].id.clone();
        }
        self.mark_dirty();
    }

    /// 获取当前活动便笺的可变引用
    pub fn active_note_mut(&mut self) -> Option<&mut StickyNote> {
        let id = self.active_id.clone();
        self.notes.iter_mut().find(|n| n.id == id)
    }

    /// 获取当前活动便笺的只读引用
    pub fn active_note(&self) -> Option<&StickyNote> {
        self.notes.iter().find(|n| n.id == self.active_id)
    }
}

/// 便笺色彩配置
pub struct NotePalette {
    pub bg: Color32,
    pub header_bg: Color32,
    pub border: Color32,
    pub fg: Color32,
    pub toolbar_bg: Color32,
    pub accent: Color32,
}

/// 根据主题与颜色索引生成色彩配置
pub fn get_palette(color_idx: usize, tc: &crate::view::ThemeColors) -> NotePalette {
    if tc.retro16 {
        // Retro 16-Bit 街机微动色彩
        match color_idx {
            0 => NotePalette {
                bg: Color32::from_rgb(46, 40, 20),
                header_bg: Color32::from_rgb(66, 58, 28),
                border: Color32::from_rgb(255, 215, 0),
                fg: Color32::from_rgb(255, 240, 160),
                toolbar_bg: Color32::from_rgb(38, 33, 16),
                accent: Color32::from_rgb(255, 215, 0),
            },
            1 => NotePalette {
                bg: Color32::from_rgb(20, 46, 26),
                header_bg: Color32::from_rgb(28, 66, 36),
                border: Color32::from_rgb(50, 255, 120),
                fg: Color32::from_rgb(180, 255, 200),
                toolbar_bg: Color32::from_rgb(16, 38, 21),
                accent: Color32::from_rgb(50, 255, 120),
            },
            2 => NotePalette {
                bg: Color32::from_rgb(50, 22, 36),
                header_bg: Color32::from_rgb(70, 30, 50),
                border: Color32::from_rgb(255, 105, 180),
                fg: Color32::from_rgb(255, 200, 225),
                toolbar_bg: Color32::from_rgb(40, 18, 29),
                accent: Color32::from_rgb(255, 105, 180),
            },
            3 => NotePalette {
                bg: Color32::from_rgb(38, 24, 56),
                header_bg: Color32::from_rgb(54, 34, 78),
                border: Color32::from_rgb(180, 120, 255),
                fg: Color32::from_rgb(230, 200, 255),
                toolbar_bg: Color32::from_rgb(31, 20, 46),
                accent: Color32::from_rgb(180, 120, 255),
            },
            4 => NotePalette {
                bg: Color32::from_rgb(20, 38, 56),
                header_bg: Color32::from_rgb(28, 54, 78),
                border: Color32::from_rgb(0, 220, 255),
                fg: Color32::from_rgb(190, 240, 255),
                toolbar_bg: Color32::from_rgb(16, 31, 46),
                accent: Color32::from_rgb(0, 220, 255),
            },
            5 => NotePalette {
                bg: Color32::from_rgb(32, 34, 40),
                header_bg: Color32::from_rgb(46, 48, 56),
                border: Color32::from_rgb(180, 185, 195),
                fg: Color32::from_rgb(235, 238, 245),
                toolbar_bg: Color32::from_rgb(26, 27, 32),
                accent: Color32::from_rgb(180, 185, 195),
            },
            6 => NotePalette {
                bg: Color32::from_rgb(18, 19, 24),
                header_bg: Color32::from_rgb(28, 30, 38),
                border: Color32::from_rgb(120, 125, 140),
                fg: Color32::from_rgb(220, 225, 235),
                toolbar_bg: Color32::from_rgb(14, 15, 19),
                accent: Color32::from_rgb(140, 145, 160),
            },
            _ => NotePalette {
                bg: tc.bg_edit,
                header_bg: tc.bg_bar,
                border: tc.border_strong,
                fg: tc.fg,
                toolbar_bg: tc.bg_status,
                accent: tc.yellow,
            },
        }
    } else if tc.comic {
        // Comic 3渲2 美漫赛璐珞高饱和波普风
        match color_idx {
            0 => NotePalette {
                bg: Color32::from_rgb(255, 248, 118),
                header_bg: Color32::from_rgb(255, 235, 75),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(245, 235, 95),
                accent: Color32::BLACK,
            },
            1 => NotePalette {
                bg: Color32::from_rgb(182, 248, 206),
                header_bg: Color32::from_rgb(142, 244, 180),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(165, 238, 192),
                accent: Color32::BLACK,
            },
            2 => NotePalette {
                bg: Color32::from_rgb(255, 180, 200),
                header_bg: Color32::from_rgb(255, 145, 172),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(245, 165, 188),
                accent: Color32::BLACK,
            },
            3 => NotePalette {
                bg: Color32::from_rgb(222, 190, 252),
                header_bg: Color32::from_rgb(202, 160, 248),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(212, 175, 245),
                accent: Color32::BLACK,
            },
            4 => NotePalette {
                bg: Color32::from_rgb(168, 232, 252),
                header_bg: Color32::from_rgb(132, 218, 250),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(150, 222, 245),
                accent: Color32::BLACK,
            },
            5 => NotePalette {
                bg: Color32::from_rgb(240, 242, 245),
                header_bg: Color32::from_rgb(222, 226, 232),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(230, 233, 238),
                accent: Color32::BLACK,
            },
            6 => NotePalette {
                bg: Color32::from_rgb(46, 50, 66),
                header_bg: Color32::from_rgb(58, 64, 84),
                border: Color32::BLACK,
                fg: Color32::from_rgb(245, 248, 252),
                toolbar_bg: Color32::from_rgb(38, 41, 54),
                accent: Color32::from_rgb(255, 204, 0),
            },
            _ => NotePalette {
                bg: Color32::from_rgb(255, 248, 118),
                header_bg: Color32::from_rgb(255, 235, 75),
                border: Color32::BLACK,
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgb(245, 235, 95),
                accent: Color32::BLACK,
            },
        }
    } else if tc.crystal || tc.is_translucent() {
        // Crystal / Glass 水晶透明毛玻璃
        let alpha = 190;
        let h_alpha = 220;
        match color_idx {
            0 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(255, 245, 160, alpha),
                header_bg: Color32::from_rgba_unmultiplied(255, 235, 120, h_alpha),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                fg: if tc.dark { Color32::from_rgb(45, 38, 15) } else { Color32::BLACK },
                toolbar_bg: Color32::from_rgba_unmultiplied(245, 232, 140, alpha),
                accent: Color32::from_rgb(255, 180, 0),
            },
            1 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(180, 255, 210, alpha),
                header_bg: Color32::from_rgba_unmultiplied(150, 255, 190, h_alpha),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                fg: if tc.dark { Color32::from_rgb(18, 42, 25) } else { Color32::BLACK },
                toolbar_bg: Color32::from_rgba_unmultiplied(165, 245, 198, alpha),
                accent: Color32::from_rgb(0, 200, 100),
            },
            2 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(255, 195, 215, alpha),
                header_bg: Color32::from_rgba_unmultiplied(255, 170, 195, h_alpha),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                fg: if tc.dark { Color32::from_rgb(48, 20, 32) } else { Color32::BLACK },
                toolbar_bg: Color32::from_rgba_unmultiplied(248, 185, 205, alpha),
                accent: Color32::from_rgb(255, 105, 180),
            },
            3 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(225, 195, 255, alpha),
                header_bg: Color32::from_rgba_unmultiplied(205, 165, 255, h_alpha),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                fg: if tc.dark { Color32::from_rgb(38, 20, 52) } else { Color32::BLACK },
                toolbar_bg: Color32::from_rgba_unmultiplied(215, 180, 248, alpha),
                accent: Color32::from_rgb(160, 90, 240),
            },
            4 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(180, 225, 255, alpha),
                header_bg: Color32::from_rgba_unmultiplied(150, 205, 255, h_alpha),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                fg: if tc.dark { Color32::from_rgb(18, 36, 52) } else { Color32::BLACK },
                toolbar_bg: Color32::from_rgba_unmultiplied(165, 215, 248, alpha),
                accent: Color32::from_rgb(0, 180, 255),
            },
            5 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(235, 238, 245, alpha),
                header_bg: Color32::from_rgba_unmultiplied(215, 220, 230, h_alpha),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                fg: Color32::BLACK,
                toolbar_bg: Color32::from_rgba_unmultiplied(225, 228, 236, alpha),
                accent: Color32::from_rgb(120, 130, 145),
            },
            6 => NotePalette {
                bg: Color32::from_rgba_unmultiplied(35, 38, 48, 210),
                header_bg: Color32::from_rgba_unmultiplied(48, 52, 65, 235),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 50),
                fg: Color32::from_rgb(240, 245, 255),
                toolbar_bg: Color32::from_rgba_unmultiplied(28, 30, 38, 210),
                accent: Color32::from_rgb(0, 220, 255),
            },
            _ => NotePalette {
                bg: Color32::from_rgba_unmultiplied(35, 38, 48, 210),
                header_bg: Color32::from_rgba_unmultiplied(48, 52, 65, 235),
                border: tc.border,
                fg: tc.fg,
                toolbar_bg: Color32::from_rgba_unmultiplied(28, 30, 38, 210),
                accent: tc.blue,
            },
        }
    } else if tc.dark {
        // Fluent 沉浸深色现代便笺
        match color_idx {
            0 => NotePalette {
                bg: Color32::from_rgb(52, 45, 21),
                header_bg: Color32::from_rgb(64, 55, 26),
                border: Color32::from_rgb(120, 105, 48),
                fg: Color32::from_rgb(255, 242, 170),
                toolbar_bg: Color32::from_rgb(44, 38, 18),
                accent: Color32::from_rgb(255, 215, 80),
            },
            1 => NotePalette {
                bg: Color32::from_rgb(24, 48, 30),
                header_bg: Color32::from_rgb(31, 60, 38),
                border: Color32::from_rgb(50, 120, 70),
                fg: Color32::from_rgb(190, 250, 205),
                toolbar_bg: Color32::from_rgb(20, 40, 25),
                accent: Color32::from_rgb(70, 220, 130),
            },
            2 => NotePalette {
                bg: Color32::from_rgb(56, 26, 38),
                header_bg: Color32::from_rgb(70, 33, 48),
                border: Color32::from_rgb(140, 60, 90),
                fg: Color32::from_rgb(255, 205, 225),
                toolbar_bg: Color32::from_rgb(46, 21, 31),
                accent: Color32::from_rgb(255, 120, 170),
            },
            3 => NotePalette {
                bg: Color32::from_rgb(44, 27, 58),
                header_bg: Color32::from_rgb(56, 34, 74),
                border: Color32::from_rgb(110, 65, 145),
                fg: Color32::from_rgb(235, 210, 255),
                toolbar_bg: Color32::from_rgb(37, 22, 49),
                accent: Color32::from_rgb(180, 120, 255),
            },
            4 => NotePalette {
                bg: Color32::from_rgb(23, 42, 60),
                header_bg: Color32::from_rgb(29, 53, 76),
                border: Color32::from_rgb(55, 105, 150),
                fg: Color32::from_rgb(200, 235, 255),
                toolbar_bg: Color32::from_rgb(19, 35, 50),
                accent: Color32::from_rgb(50, 180, 255),
            },
            5 => NotePalette {
                bg: Color32::from_rgb(36, 38, 44),
                header_bg: Color32::from_rgb(47, 50, 58),
                border: Color32::from_rgb(90, 95, 110),
                fg: Color32::from_rgb(235, 238, 245),
                toolbar_bg: Color32::from_rgb(30, 32, 37),
                accent: Color32::from_rgb(170, 175, 190),
            },
            6 => NotePalette {
                bg: Color32::from_rgb(24, 25, 30),
                header_bg: Color32::from_rgb(33, 35, 42),
                border: Color32::from_rgb(65, 68, 80),
                fg: Color32::from_rgb(220, 225, 235),
                toolbar_bg: Color32::from_rgb(19, 20, 24),
                accent: Color32::from_rgb(130, 135, 150),
            },
            _ => NotePalette {
                bg: tc.bg_edit,
                header_bg: tc.bg_bar,
                border: tc.border,
                fg: tc.fg,
                toolbar_bg: tc.bg_status,
                accent: tc.blue,
            },
        }
    } else {
        // Fluent 微软官方经典便笺浅色
        match color_idx {
            0 => NotePalette {
                bg: Color32::from_rgb(255, 247, 166),
                header_bg: Color32::from_rgb(255, 237, 128),
                border: Color32::from_rgb(230, 212, 110),
                fg: Color32::from_rgb(32, 32, 32),
                toolbar_bg: Color32::from_rgb(250, 241, 150),
                accent: Color32::from_rgb(200, 150, 0),
            },
            1 => NotePalette {
                bg: Color32::from_rgb(226, 248, 218),
                header_bg: Color32::from_rgb(206, 244, 194),
                border: Color32::from_rgb(180, 225, 165),
                fg: Color32::from_rgb(28, 38, 28),
                toolbar_bg: Color32::from_rgb(216, 242, 207),
                accent: Color32::from_rgb(40, 160, 60),
            },
            2 => NotePalette {
                bg: Color32::from_rgb(255, 232, 240),
                header_bg: Color32::from_rgb(255, 212, 224),
                border: Color32::from_rgb(235, 185, 200),
                fg: Color32::from_rgb(42, 28, 34),
                toolbar_bg: Color32::from_rgb(250, 222, 232),
                accent: Color32::from_rgb(220, 70, 130),
            },
            3 => NotePalette {
                bg: Color32::from_rgb(241, 231, 254),
                header_bg: Color32::from_rgb(227, 210, 252),
                border: Color32::from_rgb(205, 180, 240),
                fg: Color32::from_rgb(36, 28, 44),
                toolbar_bg: Color32::from_rgb(234, 221, 250),
                accent: Color32::from_rgb(140, 70, 220),
            },
            4 => NotePalette {
                bg: Color32::from_rgb(228, 243, 254),
                header_bg: Color32::from_rgb(206, 231, 252),
                border: Color32::from_rgb(180, 210, 240),
                fg: Color32::from_rgb(24, 34, 46),
                toolbar_bg: Color32::from_rgb(218, 236, 250),
                accent: Color32::from_rgb(0, 140, 220),
            },
            5 => NotePalette {
                bg: Color32::from_rgb(242, 243, 245),
                header_bg: Color32::from_rgb(228, 230, 234),
                border: Color32::from_rgb(205, 208, 215),
                fg: Color32::from_rgb(32, 34, 38),
                toolbar_bg: Color32::from_rgb(235, 236, 239),
                accent: Color32::from_rgb(100, 105, 115),
            },
            6 => NotePalette {
                bg: Color32::from_rgb(42, 44, 52),
                header_bg: Color32::from_rgb(52, 55, 65),
                border: Color32::from_rgb(80, 85, 98),
                fg: Color32::from_rgb(245, 248, 252),
                toolbar_bg: Color32::from_rgb(35, 37, 44),
                accent: Color32::from_rgb(120, 130, 150),
            },
            _ => NotePalette {
                bg: tc.bg_edit,
                header_bg: tc.bg_bar,
                border: tc.border,
                fg: tc.fg,
                toolbar_bg: tc.bg_status,
                accent: tc.blue,
            },
        }
    }
}

/// 便笺存储路径: `%APPDATA%\IDE1.0 Fast\sticky_notes.json`
pub fn sticky_notes_path() -> Option<PathBuf> {
    if let Some(p) = crate::settings::path() {
        return p.parent().map(|dir| dir.join("sticky_notes.json"));
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("IDE1.0 Fast").join("sticky_notes.json"))
}

/// 格式化当前时间
pub fn current_timestamp() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

impl App {
    /// 渲染常驻便笺浮动窗口
    pub(super) fn sticky_notes_window(&mut self, ctx: &egui::Context) {
        if !self.sticky_notes.open {
            return;
        }

        let mut is_open = self.sticky_notes.open;
        let mut new_note_clicked = false;
        let mut delete_active_clicked = false;
        let mut copy_clicked = false;
        let mut clear_clicked = false;
        let mut toggle_list_clicked = false;
        let mut append_markdown = None;
        let mut activate_note_id = None;
        let mut select_color = None;

        let cur_color_idx = self
            .sticky_notes
            .active_note()
            .map(|n| n.color_idx)
            .unwrap_or(0);
        let palette = get_palette(cur_color_idx, &self.tc);
        let is_pinned = self
            .sticky_notes
            .active_note()
            .map(|n| n.pinned)
            .unwrap_or(false);

        let default_size = Vec2::from(self.sticky_notes.win_size);

        let mut win = egui::Window::new("sticky_notes_floating_window")
            .id(egui::Id::new("sticky_notes_window_id"))
            .title_bar(false)
            .frame(egui::Frame::NONE)
            .resizable(true)
            .min_size(Vec2::new(260.0, 220.0))
            .default_size(default_size);

        if let Some(pos) = self.sticky_notes.win_pos {
            win = win.default_pos(Pos2::from(pos));
        }

        // 常驻顶层控制
        if is_pinned {
            win = win.order(egui::Order::Foreground);
        }

        let tc = self.tc;
        let show_list = self.sticky_notes.show_list;
        let mut note_content_changed = false;
        let mut search_query_changed = false;

        win.show(ctx, |ui| {
            let win_rect = ui.max_rect();

            // 1. 自绘窗口阴影与背景
            if tc.retro16 {
                // Retro 16-Bit 街机微凹凸像素边框
                ui.painter().rect_filled(win_rect, 0.0, palette.bg);
                // 顶部/左侧高反差像素光边缘
                ui.painter().line_segment(
                    [win_rect.left_top(), win_rect.right_top()],
                    Stroke::new(2.0, palette.border),
                );
                ui.painter().line_segment(
                    [win_rect.left_top(), win_rect.left_bottom()],
                    Stroke::new(2.0, palette.border),
                );
                // 底部/右侧阴影边
                ui.painter().line_segment(
                    [win_rect.left_bottom(), win_rect.right_bottom()],
                    Stroke::new(2.0, Color32::from_rgb(15, 10, 25)),
                );
                ui.painter().line_segment(
                    [win_rect.right_top(), win_rect.right_bottom()],
                    Stroke::new(2.0, Color32::from_rgb(15, 10, 25)),
                );
            } else if tc.comic {
                // Comic 3渲2: 纯黑实体硬边剪纸投影 (+4px, +4px) 与 2.5px 粗黑边框
                ui.painter().rect_filled(
                    win_rect.translate(Vec2::new(4.0, 4.0)),
                    0.0,
                    Color32::from_black_alpha(240),
                );
                ui.painter().rect_filled(win_rect, 0.0, palette.bg);
                ui.painter().rect_stroke(
                    win_rect,
                    0.0,
                    Stroke::new(2.5, Color32::BLACK),
                    StrokeKind::Inside,
                );
            } else if tc.crystal || tc.is_translucent() {
                // Crystal 水晶透明毛玻璃
                ui.painter().rect_filled(win_rect, 8.0, palette.bg);
                ui.painter().rect_stroke(
                    win_rect,
                    8.0,
                    Stroke::new(1.0, palette.border),
                    StrokeKind::Inside,
                );
                // 顶部切面微光反射
                ui.painter().line_segment(
                    [
                        pos2(win_rect.left() + 10.0, win_rect.top() + 1.0),
                        pos2(win_rect.right() - 10.0, win_rect.top() + 1.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(180)),
                );
            } else {
                // Fluent 现代浅色/深色
                ui.painter().rect_filled(win_rect, 6.0, palette.bg);
                ui.painter().rect_stroke(
                    win_rect,
                    6.0,
                    Stroke::new(1.0, palette.border),
                    StrokeKind::Inside,
                );
            }

            // 2. 顶栏 (Header Bar - 高度 34px，兼具整体拖拽把手)
            let header_h = 34.0;
            let header_rect =
                Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));

            if tc.retro16 {
                ui.painter().rect_filled(header_rect, 0.0, palette.header_bg);
                ui.painter().line_segment(
                    [header_rect.left_bottom(), header_rect.right_bottom()],
                    Stroke::new(1.0, palette.border),
                );
            } else if tc.comic {
                ui.painter().rect_filled(header_rect, 0.0, palette.header_bg);
                ui.painter().line_segment(
                    [header_rect.left_bottom(), header_rect.right_bottom()],
                    Stroke::new(2.0, Color32::BLACK),
                );
            } else if tc.crystal || tc.is_translucent() {
                ui.painter().rect_filled(header_rect, 8.0, palette.header_bg);
                ui.painter().line_segment(
                    [header_rect.left_bottom(), header_rect.right_bottom()],
                    Stroke::new(1.0, Color32::from_white_alpha(60)),
                );
            } else {
                ui.painter().rect_filled(header_rect, 6.0, palette.header_bg);
                ui.painter().line_segment(
                    [header_rect.left_bottom(), header_rect.right_bottom()],
                    Stroke::new(1.0, palette.border),
                );
            }

            // 顶栏拖动把手 (双击或按住任意空余区域拖移窗口)
            let drag_resp = ui.interact(
                header_rect,
                ui.make_persistent_id("sticky-notes-drag-area"),
                Sense::drag(),
            );
            if drag_resp.dragged() {
                let delta = drag_resp.drag_delta();
                let cur = self
                    .sticky_notes
                    .win_pos
                    .unwrap_or([win_rect.min.x, win_rect.min.y]);
                self.sticky_notes.win_pos = Some([cur[0] + delta.x, cur[1] + delta.y]);
                self.sticky_notes.mark_dirty();
            }

            // 顶栏内容排版
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(header_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.add_space(6.0);

                    // 2.1 新建便笺按钮 (+)
                    let new_resp = draw_header_btn(ui, "+", "新建便笺 (Ctrl+N)", &tc, &palette);
                    if new_resp.clicked() {
                        new_note_clicked = true;
                    }

                    // 2.2 便笺列表抽屉按钮 (📑)
                    let list_resp = draw_header_btn(
                        ui,
                        if show_list { "📝" } else { "📑" },
                        if show_list {
                            "返回编辑"
                        } else {
                            "便笺列表 (Notes List)"
                        },
                        &tc,
                        &palette,
                    );
                    if list_resp.clicked() {
                        toggle_list_clicked = true;
                    }

                    // 2.3 标题显示 (自适应显示当前便笺标题或便笺列表)
                    let title_text = if show_list {
                        format!("便笺列表 ({})", self.sticky_notes.notes.len())
                    } else if let Some(n) = self.sticky_notes.active_note() {
                        if n.title.is_empty() {
                            "新建便笺".to_string()
                        } else {
                            n.title.chars().take(14).collect::<String>()
                        }
                    } else {
                        "便笺".to_string()
                    };

                    let font = if tc.retro16 {
                        FontId::monospace(12.0)
                    } else {
                        FontId::proportional(12.0)
                    };
                    ui.label(
                        egui::RichText::new(title_text)
                            .font(font)
                            .color(palette.fg.gamma_multiply(0.85))
                            .strong(),
                    );

                    // 弹性撑开至右侧
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);

                        // 2.4 关闭按钮 (✕)
                        let close_resp = draw_header_btn(
                            ui,
                            "✕",
                            "关闭便笺 (隐藏常驻窗，内容已自动保存)",
                            &tc,
                            &palette,
                        );
                        if close_resp.clicked() {
                            is_open = false;
                        }

                        // 2.5 删除按钮 (🗑️)
                        if !show_list {
                            let del_resp =
                                draw_header_btn(ui, "🗑️", "删除此便笺 (Delete)", &tc, &palette);
                            if del_resp.clicked() {
                                delete_active_clicked = true;
                            }
                        }

                        // 2.6 置顶常驻按钮 (📌)
                        let pin_label = if is_pinned { "📍" } else { "📌" };
                        let pin_tip = if is_pinned {
                            "取消常驻置顶"
                        } else {
                            "常驻置顶 (始终在最前)"
                        };
                        let pin_resp = draw_header_btn(ui, pin_label, pin_tip, &tc, &palette);
                        if pin_resp.clicked() {
                            if let Some(n) = self.sticky_notes.active_note_mut() {
                                n.pinned = !n.pinned;
                            }
                            self.sticky_notes.mark_dirty();
                        }

                        // 2.7 调色盘按钮 (🎨)
                        let pal_resp = draw_header_btn(ui, "🎨", "切换便笺纸张色彩", &tc, &palette);
                        if pal_resp.clicked() {
                            self.sticky_notes.color_picker_open =
                                !self.sticky_notes.color_picker_open;
                        }
                    });
                },
            );

            // 3. 颜色选择悬浮下拉面板 (如果展开)
            if self.sticky_notes.color_picker_open {
                let picker_rect = Rect::from_min_size(
                    pos2(win_rect.right() - 210.0, header_rect.bottom() + 4.0),
                    Vec2::new(200.0, 32.0),
                );
                ui.painter().rect_filled(
                    picker_rect,
                    4.0,
                    if tc.dark {
                        Color32::from_rgb(30, 32, 40)
                    } else {
                        Color32::WHITE
                    },
                );
                ui.painter().rect_stroke(
                    picker_rect,
                    4.0,
                    Stroke::new(1.0, palette.border),
                    StrokeKind::Inside,
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(picker_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        ui.add_space(4.0);
                        for idx in 0..7 {
                            let chip_pal = get_palette(idx, &tc);
                            let (c_rect, c_resp) =
                                ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                            let tip = COLOR_NAMES[idx];
                            let c_resp = c_resp.on_hover_text(tip);
                            let chip_p = ui.painter();
                            let is_curr = cur_color_idx == idx;
                            chip_p.rect_filled(c_rect, 3.0, chip_pal.bg);
                            let st = if is_curr {
                                Stroke::new(2.0, palette.accent)
                            } else {
                                Stroke::new(1.0, Color32::from_black_alpha(100))
                            };
                            chip_p.rect_stroke(c_rect, 3.0, st, StrokeKind::Inside);
                            if c_resp.clicked() {
                                select_color = Some(idx);
                            }
                        }
                    },
                );
            }

            // 4. 内容主体排版
            let body_rect = Rect::from_min_max(
                pos2(win_rect.left(), header_rect.bottom()),
                pos2(
                    win_rect.right(),
                    win_rect.bottom() - if show_list { 0.0 } else { 32.0 },
                ),
            );

            if show_list {
                // ================= 便笺列表模式 (Notes List) =================
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(body_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.add_space(6.0);
                        // 搜索输入栏
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            let search_w = ui.available_width() - 16.0;
                            let search_resp = ui.add_sized(
                                [search_w, 24.0],
                                TextEdit::singleline(&mut self.sticky_notes.search_query)
                                    .hint_text("🔍 搜索便笺内容与标题..."),
                            );
                            if search_resp.changed() {
                                search_query_changed = true;
                            }
                        });

                        ui.add_space(6.0);
                        ui.separator();

                        // 滚动列表项
                        let q = self.sticky_notes.search_query.to_lowercase();
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                                let mut to_delete: Option<String> = None;
                                let mut to_select = None;

                                for note in &self.sticky_notes.notes {
                                    if !q.is_empty()
                                        && !note.title.to_lowercase().contains(&q)
                                        && !note.content.to_lowercase().contains(&q)
                                    {
                                        continue;
                                    }

                                    let is_active = note.id == self.sticky_notes.active_id;
                                    let (row_rect, row_resp) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width() - 12.0, 48.0),
                                        Sense::click(),
                                    );
                                    let row_p = ui.painter();

                                    let note_pal = get_palette(note.color_idx, &tc);
                                    let row_bg = if is_active {
                                        note_pal.bg
                                    } else if row_resp.hovered() {
                                        palette.header_bg
                                    } else {
                                        Color32::from_black_alpha(15)
                                    };

                                    row_p.rect_filled(row_rect, 4.0, row_bg);
                                    row_p.rect_stroke(
                                        row_rect,
                                        4.0,
                                        Stroke::new(
                                            1.0,
                                            if is_active {
                                                note_pal.accent
                                            } else {
                                                palette.border.gamma_multiply(0.5)
                                            },
                                        ),
                                        StrokeKind::Inside,
                                    );

                                    // 左侧色彩标识色块
                                    let badge_rect = Rect::from_min_size(
                                        pos2(row_rect.left() + 6.0, row_rect.center().y - 12.0),
                                        Vec2::new(6.0, 24.0),
                                    );
                                    row_p.rect_filled(badge_rect, 2.0, note_pal.accent);

                                    // 标题
                                    row_p.text(
                                        pos2(row_rect.left() + 18.0, row_rect.top() + 14.0),
                                        egui::Align2::LEFT_CENTER,
                                        note.title.chars().take(22).collect::<String>(),
                                        FontId::proportional(12.5),
                                        palette.fg,
                                    );

                                    // 日期时间与简要片段
                                    let snippet = note
                                        .content
                                        .lines()
                                        .find(|l| !l.is_empty())
                                        .unwrap_or("")
                                        .chars()
                                        .take(24)
                                        .collect::<String>();
                                    let sub_text = format!("{} · {}", note.updated_at, snippet);
                                    row_p.text(
                                        pos2(row_rect.left() + 18.0, row_rect.top() + 32.0),
                                        egui::Align2::LEFT_CENTER,
                                        sub_text,
                                        FontId::proportional(10.0),
                                        palette.fg.gamma_multiply(0.65),
                                    );

                                    let del_btn_rect = Rect::from_center_size(
                                        pos2(row_rect.right() - 16.0, row_rect.center().y),
                                        Vec2::splat(18.0),
                                    );
                                    let del_btn_resp = ui
                                        .interact(
                                            del_btn_rect,
                                            ui.make_persistent_id(format!("del_note_{}", note.id)),
                                            Sense::click(),
                                        )
                                        .on_hover_text("删除此便笺");
                                    row_p.text(
                                        del_btn_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "🗑️",
                                        FontId::proportional(11.0),
                                        palette.fg,
                                    );
                                    if del_btn_resp.clicked() {
                                        to_delete = Some(note.id.clone());
                                    } else if row_resp.clicked() {
                                        to_select = Some(note.id.clone());
                                    }
                                }

                                if let Some(id) = to_select {
                                    activate_note_id = Some(id);
                                }
                                if let Some(id) = to_delete {
                                    self.sticky_notes.delete_note(&id);
                                }
                            });
                    },
                );
            } else {
                // ================= 主便笺编辑模式 (Main Note Editor) =================
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(body_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        let active_id = self.sticky_notes.active_id.clone();
                        if let Some(note) =
                            self.sticky_notes.notes.iter_mut().find(|n| n.id == active_id)
                        {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;

                            let font_id = if tc.retro16 {
                                FontId::monospace(13.0)
                            } else {
                                FontId::proportional(13.0)
                            };

                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add_space(4.0);
                                    let edit_resp = ui.add(
                                        TextEdit::multiline(&mut note.content)
                                            .font(font_id)
                                            .text_color(palette.fg)
                                            .desired_width(f32::INFINITY)
                                            .frame(egui::Frame::NONE)
                                            .hint_text("记下一些文字、备忘或 Markdown 待办..."),
                                    );

                                    // 单次聚焦逻辑 (避免 IME 闪烁)
                                    if self.sticky_notes.focus_needed {
                                        edit_resp.request_focus();
                                        self.sticky_notes.focus_needed = false;
                                    }
                                    self.sticky_notes.has_focus = edit_resp.has_focus();

                                    if edit_resp.changed() {
                                        note_content_changed = true;
                                        note.updated_at = current_timestamp();
                                        // 智能依据首行提炼标题
                                        let first_line = note
                                            .content
                                            .lines()
                                            .map(|l| l.trim().trim_start_matches('#').trim())
                                            .find(|l| !l.is_empty());
                                        if let Some(fl) = first_line {
                                            note.title = fl.chars().take(24).collect();
                                        }
                                    }
                                });
                        }
                    },
                );

                // 5. 底部快捷格式化工具栏 (Formatting Toolbar - 高度 32px，与 Windows 便笺完全一致)
                let toolbar_rect = Rect::from_min_max(
                    pos2(win_rect.left(), win_rect.bottom() - 32.0),
                    win_rect.right_bottom(),
                );

                ui.painter().rect_filled(toolbar_rect, 0.0, palette.toolbar_bg);
                ui.painter().line_segment(
                    [toolbar_rect.left_top(), toolbar_rect.right_top()],
                    Stroke::new(1.0, palette.border.gamma_multiply(0.6)),
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(toolbar_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                        ui.add_space(4.0);

                        // 粗体 (B)
                        if draw_tool_btn(ui, "B", "粗体 (**文字**)", &tc, &palette).clicked() {
                            append_markdown = Some("**粗体文字**".to_string());
                        }
                        // 斜体 (I)
                        if draw_tool_btn(ui, "I", "斜体 (*文字*)", &tc, &palette).clicked() {
                            append_markdown = Some("*斜体文字*".to_string());
                        }
                        // 下划线 (U)
                        if draw_tool_btn(ui, "U", "下划线 (<u>文字</u>)", &tc, &palette).clicked() {
                            append_markdown = Some("<u>下划线</u>".to_string());
                        }
                        // 删除线 (S)
                        if draw_tool_btn(ui, "S", "删除线 (~~文字~~)", &tc, &palette).clicked() {
                            append_markdown = Some("~~删除线~~".to_string());
                        }
                        // 代办复选框 (☑)
                        if draw_tool_btn(ui, "☑", "待办项 (- [ ])", &tc, &palette).clicked() {
                            append_markdown = Some("\n- [ ] 待办事项".to_string());
                        }
                        // 项目符号 (•)
                        if draw_tool_btn(ui, "•", "项目符号 (- 列表)", &tc, &palette).clicked() {
                            append_markdown = Some("\n- 列表项".to_string());
                        }
                        // 时间戳 (🕒)
                        if draw_tool_btn(ui, "🕒", "插入当前时间戳", &tc, &palette).clicked() {
                            append_markdown = Some(format!("\n[{}] ", current_timestamp()));
                        }
                        // 复制全部 (📋)
                        if draw_tool_btn(ui, "📋", "一键复制便笺全文到剪贴板", &tc, &palette).clicked()
                        {
                            copy_clicked = true;
                        }
                        // 清空 (🧹)
                        if draw_tool_btn(ui, "🧹", "清空便笺内容", &tc, &palette).clicked() {
                            clear_clicked = true;
                        }

                        // 右侧字数与行数统计
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            if let Some(n) = self.sticky_notes.active_note() {
                                let chars_cnt = n.content.chars().count();
                                let lines_cnt = n.content.lines().count().max(1);
                                let stats_text = format!("{} 字 · {} 行", chars_cnt, lines_cnt);
                                ui.label(
                                    egui::RichText::new(stats_text)
                                        .font(FontId::proportional(10.0))
                                        .color(palette.fg.gamma_multiply(0.6)),
                                );
                            }
                        });
                    },
                );
            }
        });

        // 外部状态副作用处理
        self.sticky_notes.open = is_open;

        if let Some(color_idx) = select_color {
            if let Some(n) = self.sticky_notes.active_note_mut() {
                n.color_idx = color_idx;
            }
            self.sticky_notes.color_picker_open = false;
            self.sticky_notes.mark_dirty();
        }

        if new_note_clicked {
            self.sticky_notes.new_note();
        }

        if toggle_list_clicked {
            self.sticky_notes.show_list = !self.sticky_notes.show_list;
        }

        if delete_active_clicked {
            let active = self.sticky_notes.active_id.clone();
            self.sticky_notes.delete_note(&active);
        }

        if let Some(id) = activate_note_id {
            self.sticky_notes.active_id = id;
            self.sticky_notes.show_list = false;
            self.sticky_notes.focus_needed = true;
            self.sticky_notes.mark_dirty();
        }

        if let Some(extra) = append_markdown {
            if let Some(n) = self.sticky_notes.active_note_mut() {
                n.content.push_str(&extra);
                n.updated_at = current_timestamp();
            }
            self.sticky_notes.mark_dirty();
        }

        if copy_clicked {
            if let Some(n) = self.sticky_notes.active_note() {
                ctx.copy_text(n.content.clone());
            }
        }

        if clear_clicked {
            if let Some(n) = self.sticky_notes.active_note_mut() {
                n.content.clear();
                n.title = "新建便笺".to_string();
                n.updated_at = current_timestamp();
            }
            self.sticky_notes.mark_dirty();
        }

        if note_content_changed || search_query_changed {
            self.sticky_notes.mark_dirty();
        }
    }
}

/// 绘制便笺顶栏小按钮
fn draw_header_btn(
    ui: &mut Ui,
    icon: &str,
    tip: &str,
    tc: &crate::view::ThemeColors,
    palette: &NotePalette,
) -> egui::Response {
    let size = Vec2::splat(22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_text(tip);
    let p = ui.painter();

    let is_hovered = resp.hovered();
    let is_active = resp.is_pointer_button_down_on();

    let bg = if is_active {
        palette.accent.gamma_multiply(0.4)
    } else if is_hovered {
        palette.accent.gamma_multiply(0.2)
    } else {
        Color32::TRANSPARENT
    };

    let round = if tc.retro16 { 0.0 } else { 3.0 };
    p.rect_filled(rect, round, bg);

    if is_hovered || is_active {
        p.rect_stroke(
            rect,
            round,
            Stroke::new(1.0, palette.accent.gamma_multiply(0.8)),
            StrokeKind::Inside,
        );
    }

    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        FontId::proportional(11.5),
        palette.fg,
    );

    resp
}

/// 绘制便笺底栏格式化工具按钮
fn draw_tool_btn(
    ui: &mut Ui,
    text: &str,
    tip: &str,
    tc: &crate::view::ThemeColors,
    palette: &NotePalette,
) -> egui::Response {
    let size = Vec2::new(20.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_text(tip);
    let p = ui.painter();

    let is_hovered = resp.hovered();
    let is_active = resp.is_pointer_button_down_on();

    let bg = if is_active {
        palette.accent.gamma_multiply(0.35)
    } else if is_hovered {
        palette.accent.gamma_multiply(0.18)
    } else {
        Color32::TRANSPARENT
    };

    let round = if tc.retro16 { 0.0 } else { 2.5 };
    p.rect_filled(rect, round, bg);

    if is_hovered {
        p.rect_stroke(
            rect,
            round,
            Stroke::new(1.0, palette.accent.gamma_multiply(0.6)),
            StrokeKind::Inside,
        );
    }

    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.0),
        palette.fg,
    );

    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sticky_notes_initial_invariants() {
        let state = StickyNotesState::default_initial();
        assert!(!state.open);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.active_id, "note_welcome");
        assert!(state.notes[0].content.contains("欢迎使用便笺"));
    }

    #[test]
    fn test_sticky_notes_crud_operations() {
        let mut state = StickyNotesState::default_initial();
        state.new_note();
        assert_eq!(state.notes.len(), 2);
        let new_id = state.active_id.clone();
        assert_ne!(new_id, "note_welcome");

        // 删除新建的便笺
        state.delete_note(&new_id);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.active_id, "note_welcome");

        // 删除最后一个便笺会自动新建一份空便笺
        state.delete_note("note_welcome");
        assert_eq!(state.notes.len(), 1);
        assert_ne!(state.active_id, "note_welcome");
    }

    #[test]
    fn test_sticky_notes_json_roundtrip() {
        let state = StickyNotesState::default_initial();
        let val = state.to_json();
        let parsed = StickyNotesState::from_json(&val).expect("roundtrip should succeed");
        assert_eq!(parsed.notes.len(), state.notes.len());
        assert_eq!(parsed.active_id, state.active_id);
    }

    #[test]
    fn test_palette_generation_all_themes() {
        let mut tc = crate::view::ThemeColors::builtin(false);
        for idx in 0..8 {
            let p = get_palette(idx, &tc);
            assert_ne!(p.bg, Color32::TRANSPARENT);
        }

        tc.dark = true;
        for idx in 0..8 {
            let p = get_palette(idx, &tc);
            assert_ne!(p.bg, Color32::TRANSPARENT);
        }

        tc.comic = true;
        for idx in 0..8 {
            let p = get_palette(idx, &tc);
            assert_eq!(p.border, Color32::BLACK);
        }

        tc.comic = false;
        tc.retro16 = true;
        for idx in 0..8 {
            let p = get_palette(idx, &tc);
            assert_ne!(p.bg, Color32::TRANSPARENT);
        }
    }
}
