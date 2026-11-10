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
    self, pos2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
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
    /// 渲染常驻便笺独立桌面原生窗口 (Multi-Viewport OS Desktop Window)
    ///
    /// 全对标 Windows 自带便笺 (Microsoft Sticky Notes):
    /// - 独立桌面窗口: 原生平滑拖拽移动、跨屏幕自由悬浮、OS 级窗口置顶 (Always On Top)
    /// - 7 种经典便笺纸张色彩 + 主题全画风自适应 (美漫 3渲2 / 水晶透明 / 复古 16-Bit / 现代 Fluent)
    /// - 便笺列表抽屉检索浏览 (Ctrl+H / Ctrl+F)、新建 (+ / Ctrl+N)、删除 (Ctrl+D)
    /// - 底部富格式化工具栏 (粗体/斜体/下划线/删除线/待办复选框/项目符号/时间戳/剪贴板复制/字数统计)
    /// - 右下角原生缩放手柄与边框缩放、位置与尺寸自动持久化保存
    pub(super) fn sticky_notes_window(&mut self, ctx: &egui::Context) {
        if !self.sticky_notes.open {
            return;
        }

        let viewport_id = egui::ViewportId::from_hash_of("ide_sticky_notes_independent_viewport");
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

        let mut builder = egui::ViewportBuilder::default()
            .with_title("便笺 - Sticky Notes")
            .with_inner_size(self.sticky_notes.win_size)
            .with_min_inner_size([260.0, 220.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true);

        if is_pinned {
            builder = builder.with_always_on_top();
        }
        if let Some(pos) = self.sticky_notes.win_pos {
            builder = builder.with_position(pos);
        }

        let tc = self.tc;
        let show_list = self.sticky_notes.show_list;
        let mut close_requested = false;
        let mut new_note_clicked = false;
        let mut delete_active_clicked = false;
        let mut copy_clicked = false;
        let mut clear_clicked = false;
        let mut toggle_list_clicked = false;
        let mut toggle_pin_clicked = false;
        let mut append_markdown = None;
        let mut activate_note_id = None;
        let mut select_color = None;
        let mut note_content_changed = false;
        let mut search_query_changed = false;
        let mut new_win_pos = None;
        let mut new_win_size = None;
        let mut search_focus_requested = false;

        ctx.show_viewport_immediate(viewport_id, builder, |vp_ctx, _class| {
            // 响应操作系统或独立视窗关闭请求
            if vp_ctx.input(|i| i.viewport().close_requested()) {
                close_requested = true;
            }

            // 实时同步独立视窗在桌面上的实际物理位置与大小
            if let Some(rect) = vp_ctx.input(|i| i.viewport().outer_rect) {
                if rect.min.x > -10000.0 && rect.min.y > -10000.0 {
                    new_win_pos = Some([rect.min.x, rect.min.y]);
                }
            }
            if let Some(rect) = vp_ctx.input(|i| i.viewport().inner_rect) {
                if rect.width() >= 150.0 && rect.height() >= 150.0 {
                    new_win_size = Some([rect.width(), rect.height()]);
                }
            }

            // 独立窗口内快捷键全映射 (对标 Windows 便笺)
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
                new_note_clicked = true;
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
                close_requested = true;
                vp_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::H)) {
                toggle_list_clicked = true;
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
                delete_active_clicked = true;
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
                search_focus_requested = true;
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::B)) {
                append_markdown = Some("**粗体文字**".to_string());
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::I)) {
                append_markdown = Some("*斜体文字*".to_string());
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::U)) {
                append_markdown = Some("<u>下划线</u>".to_string());
            }
            if vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
                append_markdown = Some("~~删除线~~".to_string());
            }
            if vp_ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::L)
            }) {
                append_markdown = Some("\n- [ ] 待办事项".to_string());
            }

            egui::CentralPanel::default().frame(egui::Frame::NONE).show(vp_ctx, |ui| {
                let win_rect = ui.max_rect();
                // 留出 3px 边缘给阴影与描边，防止被视窗物理边缘硬裁切
                let card_rect = win_rect.shrink2(Vec2::new(3.0, 3.0));

                // 1. 自绘窗口阴影与背景
                if tc.retro16 {
                    // Retro 16-Bit 街机微凹凸像素边框
                    ui.painter().rect_filled(card_rect, 0.0, palette.bg);
                    ui.painter().line_segment(
                        [card_rect.left_top(), card_rect.right_top()],
                        Stroke::new(2.0, palette.border),
                    );
                    ui.painter().line_segment(
                        [card_rect.left_top(), card_rect.left_bottom()],
                        Stroke::new(2.0, palette.border),
                    );
                    ui.painter().line_segment(
                        [card_rect.left_bottom(), card_rect.right_bottom()],
                        Stroke::new(2.0, Color32::from_rgb(15, 10, 25)),
                    );
                    ui.painter().line_segment(
                        [card_rect.right_top(), card_rect.right_bottom()],
                        Stroke::new(2.0, Color32::from_rgb(15, 10, 25)),
                    );
                } else if tc.comic {
                    // Comic 3渲2: 纯黑实体硬边剪纸投影 (+3px, +3px) 与 2.5px 粗黑边框
                    ui.painter().rect_filled(
                        card_rect.translate(Vec2::new(3.0, 3.0)),
                        0.0,
                        Color32::from_black_alpha(230),
                    );
                    ui.painter().rect_filled(card_rect, 0.0, palette.bg);
                    ui.painter().rect_stroke(
                        card_rect,
                        0.0,
                        Stroke::new(2.5, Color32::BLACK),
                        StrokeKind::Inside,
                    );
                } else if tc.crystal || tc.is_translucent() {
                    // Crystal 水晶透明毛玻璃
                    ui.painter().rect_filled(card_rect, 8.0, palette.bg);
                    ui.painter().rect_stroke(
                        card_rect,
                        8.0,
                        Stroke::new(1.0, palette.border),
                        StrokeKind::Inside,
                    );
                    // 顶部切面微光反射
                    ui.painter().line_segment(
                        [
                            pos2(card_rect.left() + 10.0, card_rect.top() + 1.0),
                            pos2(card_rect.right() - 10.0, card_rect.top() + 1.0),
                        ],
                        Stroke::new(1.0, Color32::from_white_alpha(180)),
                    );
                } else {
                    // Fluent 现代浅色/深色
                    ui.painter().rect_filled(card_rect, 6.0, palette.bg);
                    ui.painter().rect_stroke(
                        card_rect,
                        6.0,
                        Stroke::new(1.0, palette.border),
                        StrokeKind::Inside,
                    );
                }

                // 2. 顶栏 (Header Bar - 高度 36px，兼具操作系统级原生平滑拖拽把手)
                let header_h = 36.0;
                let header_rect =
                    Rect::from_min_size(card_rect.min, Vec2::new(card_rect.width(), header_h));

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

                // 顶栏拖动把手 (调用 Windows DWM 原生 StartDrag，支持跨多显示器、分屏与无缝移动)
                let drag_resp = ui.interact(
                    header_rect,
                    ui.make_persistent_id("sticky-notes-native-drag-handle"),
                    Sense::drag(),
                );
                if drag_resp.drag_started() || drag_resp.dragged() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
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
                        let new_resp = draw_header_btn(
                            ui,
                            StickyHeaderIcon::New,
                            "新建便笺 (Ctrl+N)",
                            &tc,
                            &palette,
                        );
                        if new_resp.clicked() {
                            new_note_clicked = true;
                        }

                        // 2.2 便笺列表抽屉按钮 (📑 / 📝)
                        let list_icon = if show_list {
                            StickyHeaderIcon::BackToEdit
                        } else {
                            StickyHeaderIcon::List
                        };
                        let list_tip = if show_list {
                            "返回编辑便笺"
                        } else {
                            "便笺列表 (Notes List - Ctrl+H)"
                        };
                        let list_resp = draw_header_btn(ui, list_icon, list_tip, &tc, &palette);
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
                                StickyHeaderIcon::Close,
                                "关闭便笺 (Ctrl+W，内容已自动保存)",
                                &tc,
                                &palette,
                            );
                            if close_resp.clicked() {
                                close_requested = true;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }

                            // 2.5 删除按钮 (🗑️)
                            if !show_list {
                                let del_resp = draw_header_btn(
                                    ui,
                                    StickyHeaderIcon::Delete,
                                    "删除此便笺 (Ctrl+D)",
                                    &tc,
                                    &palette,
                                );
                                if del_resp.clicked() {
                                    delete_active_clicked = true;
                                }
                            }

                            // 2.6 置顶常驻按钮 (📌)
                            let pin_tip = if is_pinned {
                                "取消桌面置顶 (已常驻前端)"
                            } else {
                                "桌面常驻置顶 (始终在最前)"
                            };
                            let pin_resp = draw_header_btn(
                                ui,
                                StickyHeaderIcon::Pin(is_pinned),
                                pin_tip,
                                &tc,
                                &palette,
                            );
                            if pin_resp.clicked() {
                                toggle_pin_clicked = true;
                                let next_pinned = !is_pinned;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                                    if next_pinned {
                                        egui::WindowLevel::AlwaysOnTop
                                    } else {
                                        egui::WindowLevel::Normal
                                    },
                                ));
                            }

                            // 2.7 调色盘按钮 (🎨)
                            let pal_resp = draw_header_btn(
                                ui,
                                StickyHeaderIcon::Palette,
                                "切换便笺纸张色彩",
                                &tc,
                                &palette,
                            );
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
                        pos2(card_rect.right() - 210.0, header_rect.bottom() + 4.0),
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
                    pos2(card_rect.left(), header_rect.bottom()),
                    pos2(
                        card_rect.right(),
                        card_rect.bottom() - if show_list { 0.0 } else { 32.0 },
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
                                        .hint_text("🔍 搜索便笺内容与标题 (Ctrl+F)..."),
                                );
                                if search_focus_requested {
                                    search_resp.request_focus();
                                }
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
                                            pos2(
                                                row_rect.left() + 6.0,
                                                row_rect.center().y - 12.0,
                                            ),
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
                                                ui.make_persistent_id(format!(
                                                    "del_note_{}",
                                                    note.id
                                                )),
                                                Sense::click(),
                                            )
                                            .on_hover_text("删除此便笺");
                                        let (c_del_rect, del_col) = paint_sticky_btn_frame(
                                            row_p,
                                            del_btn_rect,
                                            &tc,
                                            &note_pal,
                                            del_btn_resp.hovered(),
                                            del_btn_resp.is_pointer_button_down_on(),
                                            false,
                                        );
                                        paint_sticky_header_icon(
                                            row_p,
                                            c_del_rect,
                                            StickyHeaderIcon::Delete,
                                            &tc,
                                            del_col,
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
                        pos2(card_rect.left(), card_rect.bottom() - 32.0),
                        card_rect.right_bottom(),
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
                            if draw_tool_btn(ui, StickyToolIcon::Bold, "粗体 (Ctrl+B)", &tc, &palette).clicked() {
                                append_markdown = Some("**粗体文字**".to_string());
                            }
                            // 斜体 (I)
                            if draw_tool_btn(ui, StickyToolIcon::Italic, "斜体 (Ctrl+I)", &tc, &palette).clicked() {
                                append_markdown = Some("*斜体文字*".to_string());
                            }
                            // 下划线 (U)
                            if draw_tool_btn(ui, StickyToolIcon::Underline, "下划线 (Ctrl+U)", &tc, &palette).clicked() {
                                append_markdown = Some("<u>下划线</u>".to_string());
                            }
                            // 删除线 (S)
                            if draw_tool_btn(ui, StickyToolIcon::Strikethrough, "删除线 (Ctrl+T)", &tc, &palette).clicked() {
                                append_markdown = Some("~~删除线~~".to_string());
                            }
                            // 待办复选框 (☑)
                            if draw_tool_btn(ui, StickyToolIcon::Checklist, "待办项 (Ctrl+Shift+L)", &tc, &palette)
                                .clicked()
                            {
                                append_markdown = Some("\n- [ ] 待办事项".to_string());
                            }
                            // 项目符号 (•)
                            if draw_tool_btn(ui, StickyToolIcon::BulletList, "项目符号 (- 列表)", &tc, &palette).clicked()
                            {
                                append_markdown = Some("\n- 列表项".to_string());
                            }
                            // 时间戳 (🕒)
                            if draw_tool_btn(ui, StickyToolIcon::Timestamp, "插入当前时间戳", &tc, &palette).clicked() {
                                append_markdown = Some(format!("\n[{}] ", current_timestamp()));
                            }
                            // 复制全部 (📋)
                            if draw_tool_btn(ui, StickyToolIcon::Copy, "一键复制便笺全文到剪贴板", &tc, &palette)
                                .clicked()
                            {
                                copy_clicked = true;
                            }
                            // 清空 (🧹)
                            if draw_tool_btn(ui, StickyToolIcon::Clear, "清空便笺内容", &tc, &palette).clicked() {
                                clear_clicked = true;
                            }

                            // 右侧字数与行数统计
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(20.0); // 留给右下角缩放手柄空间
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

                    // 6. 右下角原生缩放把手 (Resize Grip Handle)
                    let grip_rect = Rect::from_min_max(
                        pos2(card_rect.right() - 16.0, card_rect.bottom() - 16.0),
                        card_rect.right_bottom(),
                    );
                    let grip_resp = ui.interact(
                        grip_rect,
                        ui.make_persistent_id("sticky-notes-resize-grip"),
                        Sense::drag(),
                    );
                    if grip_resp.hovered() || grip_resp.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                    }
                    if grip_resp.drag_started() || grip_resp.dragged() {
                        ui.ctx().send_viewport_cmd(
                            egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast),
                        );
                    }
                    draw_resize_grip(ui, grip_rect, &tc, &palette);
                }
            });
        });

        // 外部状态副作用处理
        if close_requested {
            self.sticky_notes.open = false;
        }

        if let Some(pos) = new_win_pos {
            if self.sticky_notes.win_pos != Some(pos) {
                self.sticky_notes.win_pos = Some(pos);
                self.sticky_notes.mark_dirty();
            }
        }

        if let Some(size) = new_win_size {
            if (self.sticky_notes.win_size[0] - size[0]).abs() > 1.0
                || (self.sticky_notes.win_size[1] - size[1]).abs() > 1.0
            {
                self.sticky_notes.win_size = size;
                self.sticky_notes.mark_dirty();
            }
        }

        if toggle_pin_clicked {
            if let Some(n) = self.sticky_notes.active_note_mut() {
                n.pinned = !n.pinned;
            }
            self.sticky_notes.mark_dirty();
        }

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

/// 便笺顶栏按钮图标类型
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StickyHeaderIcon {
    New,
    List,
    BackToEdit,
    Palette,
    Pin(bool),
    Delete,
    Close,
}

/// 便笺底栏格式化工具图标类型
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StickyToolIcon {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Checklist,
    BulletList,
    Timestamp,
    Copy,
    Clear,
}

/// 绘制便笺按钮的主题底框 (支持美漫波普硬阴影/按压位移、复古街机3D切角、水晶切面白光与Fluent现代圆角)
pub fn paint_sticky_btn_frame(
    p: &egui::Painter,
    rect: Rect,
    tc: &crate::view::ThemeColors,
    palette: &NotePalette,
    hovered: bool,
    active: bool,
    highlighted: bool,
) -> (Rect, Color32) {
    if tc.retro16 {
        // Retro 16-Bit: 街机微动开关 3D 棱角像素按键
        let outer_border = if active {
            Color32::from_rgb(0, 0, 0)
        } else if hovered || highlighted {
            palette.accent
        } else {
            palette.border.gamma_multiply(0.7)
        };

        let bg = if active {
            palette.toolbar_bg
        } else if hovered {
            palette.header_bg
        } else {
            Color32::TRANSPARENT
        };

        p.rect_filled(rect, 0.0, bg);
        p.rect_stroke(rect, 0.0, Stroke::new(1.0, outer_border), StrokeKind::Inside);

        // 3D Bevel 棱角线 (按下去时反向凹陷)
        let (top_left_col, bot_right_col) = if active {
            (Color32::from_rgb(10, 7, 23), palette.border)
        } else if hovered {
            (palette.accent, Color32::from_rgb(15, 10, 25))
        } else {
            (palette.border.gamma_multiply(0.8), Color32::from_rgb(15, 10, 25))
        };

        let inner = rect.shrink(1.0);
        p.line_segment([inner.left_top(), inner.right_top()], Stroke::new(1.0, top_left_col));
        p.line_segment([inner.left_top(), inner.left_bottom()], Stroke::new(1.0, top_left_col));
        p.line_segment([inner.left_bottom(), inner.right_bottom()], Stroke::new(1.0, bot_right_col));
        p.line_segment([inner.right_top(), inner.right_bottom()], Stroke::new(1.0, bot_right_col));

        let content_rect = if active { rect.translate(Vec2::new(1.0, 1.0)) } else { rect };
        let fg = if active || hovered || highlighted { palette.accent } else { palette.fg };
        (content_rect, fg)
    } else if tc.comic {
        // Comic 3渲2: 2.0px 纯黑手绘墨线 + 实体硬波普投影
        let shadow_offset = if active { Vec2::ZERO } else { Vec2::new(1.5, 1.5) };
        if hovered && !active {
            p.rect_filled(rect.translate(shadow_offset), 2.0, Color32::BLACK);
        }

        let actual_rect = if active { rect.translate(Vec2::new(1.2, 1.2)) } else { rect };
        let bg = if active {
            Color32::from_rgb(255, 170, 0) // 按下深波普橙
        } else if hovered {
            Color32::from_rgb(255, 204, 0) // 悬停经典波普黄
        } else if highlighted {
            palette.accent.gamma_multiply(0.4)
        } else {
            Color32::TRANSPARENT
        };

        let border_col = if hovered || active { Color32::BLACK } else { Color32::from_black_alpha(100) };
        if bg != Color32::TRANSPARENT {
            p.rect_filled(actual_rect, 2.0, bg);
        }
        if hovered || active {
            p.rect_stroke(actual_rect, 2.0, Stroke::new(1.5, border_col), StrokeKind::Inside);
        }

        let fg = if active || hovered { Color32::BLACK } else { palette.fg };
        (actual_rect, fg)
    } else if tc.crystal || tc.clear {
        // 水晶 / 玻璃: 晶钻圆角毛玻璃 + 切面反射
        let round = 5.0;
        let bg = if active {
            Color32::from_rgba_unmultiplied(255, 105, 180, 80)
        } else if hovered {
            Color32::from_rgba_unmultiplied(255, 255, 255, 55)
        } else if highlighted {
            Color32::from_rgba_unmultiplied(0, 240, 255, 45)
        } else {
            Color32::TRANSPARENT
        };

        let border_col = if active {
            Color32::from_rgb(255, 105, 180)
        } else if hovered {
            Color32::from_rgba_unmultiplied(0, 240, 255, 200)
        } else if highlighted {
            Color32::from_rgba_unmultiplied(0, 240, 255, 150)
        } else {
            Color32::TRANSPARENT
        };

        if bg != Color32::TRANSPARENT {
            p.rect_filled(rect, round, bg);
        }
        if border_col != Color32::TRANSPARENT {
            p.rect_stroke(rect, round, Stroke::new(1.0, border_col), StrokeKind::Inside);
            // 顶沿微光折射
            p.line_segment(
                [pos2(rect.left() + 2.0, rect.top() + 1.0), pos2(rect.right() - 2.0, rect.top() + 1.0)],
                Stroke::new(1.0, Color32::WHITE),
            );
        }

        let fg = if active || hovered { Color32::WHITE } else { palette.fg };
        (rect, fg)
    } else {
        // Fluent 现代深色/浅色: 柔和圆角
        let round = 3.5;
        let bg = if active {
            palette.accent.gamma_multiply(0.35)
        } else if hovered {
            palette.accent.gamma_multiply(0.18)
        } else {
            Color32::TRANSPARENT
        };

        if bg != Color32::TRANSPARENT {
            p.rect_filled(rect, round, bg);
            p.rect_stroke(rect, round, Stroke::new(1.0, palette.accent.gamma_multiply(0.6)), StrokeKind::Inside);
        }

        let fg = if active || hovered { palette.accent } else { palette.fg };
        (rect, fg)
    }
}

/// 绘制符合各主题专属画风的便笺顶栏矢量图标
pub fn paint_sticky_header_icon(
    p: &egui::Painter,
    rect: Rect,
    icon: StickyHeaderIcon,
    tc: &crate::view::ThemeColors,
    col: Color32,
) {
    let c = rect.center();
    let stroke_w = if tc.comic { 1.8 } else { 1.2 };
    let st = Stroke::new(stroke_w, col);

    match icon {
        StickyHeaderIcon::New => {
            let r = 4.5;
            p.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], st);
            p.line_segment([pos2(c.x, c.y - r), pos2(c.x, c.y + r)], st);
        }
        StickyHeaderIcon::List => {
            for dy in [-3.8, 0.0, 3.8] {
                let y = c.y + dy;
                if tc.retro16 {
                    p.rect_filled(
                        Rect::from_min_size(pos2(c.x - 5.5, y - 1.0), Vec2::splat(2.0)),
                        0.0,
                        col,
                    );
                } else {
                    p.circle_filled(pos2(c.x - 4.5, y), 1.2, col);
                }
                let rx = if dy > 2.0 { c.x + 3.0 } else { c.x + 5.5 };
                p.line_segment([pos2(c.x - 2.0, y), pos2(rx, y)], st);
            }
        }
        StickyHeaderIcon::BackToEdit => {
            let doc_rect = Rect::from_min_max(pos2(c.x - 5.5, c.y - 5.0), pos2(c.x + 1.5, c.y + 5.0));
            p.rect_stroke(
                doc_rect,
                if tc.retro16 { 0.0 } else { 1.5 },
                st,
                StrokeKind::Inside,
            );
            // 笔杆与笔尖
            p.line_segment(
                [pos2(c.x + 5.5, c.y - 4.0), pos2(c.x, c.y + 2.0)],
                Stroke::new(stroke_w * 1.1, col),
            );
            p.line_segment([pos2(c.x, c.y + 2.0), pos2(c.x - 1.0, c.y + 3.5)], st);
        }
        StickyHeaderIcon::Palette => {
            let pal_rect = Rect::from_center_size(c, Vec2::new(12.0, 10.0));
            if tc.retro16 {
                p.rect_stroke(pal_rect, 0.0, st, StrokeKind::Inside);
                p.rect_filled(Rect::from_min_size(pos2(c.x - 3.0, c.y - 2.5), Vec2::splat(1.5)), 0.0, col);
                p.rect_filled(Rect::from_min_size(pos2(c.x + 1.5, c.y - 2.5), Vec2::splat(1.5)), 0.0, col);
                p.rect_filled(Rect::from_min_size(pos2(c.x + 2.5, c.y + 1.0), Vec2::splat(1.5)), 0.0, col);
                p.rect_stroke(Rect::from_min_size(pos2(c.x - 3.5, c.y + 1.0), Vec2::splat(2.0)), 0.0, st, StrokeKind::Inside);
            } else {
                p.rect_stroke(pal_rect, 4.0, st, StrokeKind::Inside);
                p.circle_filled(pos2(c.x - 2.5, c.y - 2.0), 1.2, col);
                p.circle_filled(pos2(c.x + 1.5, c.y - 2.0), 1.2, col);
                p.circle_filled(pos2(c.x + 2.5, c.y + 1.5), 1.2, col);
                p.circle_stroke(pos2(c.x - 2.2, c.y + 1.5), 1.2, st);
            }
        }
        StickyHeaderIcon::Pin(is_pinned) => {
            if is_pinned {
                // 常驻置顶状态: 垂直钉入的图钉 (带实心高亮钉帽)
                p.line_segment(
                    [pos2(c.x - 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y - 4.5)],
                    Stroke::new(stroke_w * 1.3, col),
                );
                let body_rect = Rect::from_min_max(pos2(c.x - 2.5, c.y - 4.5), pos2(c.x + 2.5, c.y + 0.5));
                p.rect_filled(body_rect, if tc.retro16 { 0.0 } else { 1.5 }, col);
                p.line_segment([pos2(c.x, c.y + 0.5), pos2(c.x, c.y + 5.5)], Stroke::new(1.4, col));
            } else {
                // 未置顶状态: 斜向 45 度的图钉
                p.line_segment(
                    [pos2(c.x - 1.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 1.0)],
                    Stroke::new(stroke_w * 1.3, col),
                );
                p.line_segment(
                    [pos2(c.x + 2.0, c.y - 2.0), pos2(c.x - 2.0, c.y + 2.0)],
                    Stroke::new(stroke_w * 1.1, col),
                );
                p.line_segment([pos2(c.x - 2.0, c.y + 2.0), pos2(c.x - 5.0, c.y + 5.0)], Stroke::new(1.4, col));
            }
        }
        StickyHeaderIcon::Delete => {
            // 垃圾桶盖子与把手
            p.line_segment([pos2(c.x - 5.0, c.y - 3.5), pos2(c.x + 5.0, c.y - 3.5)], st);
            p.line_segment([pos2(c.x - 1.5, c.y - 5.2), pos2(c.x + 1.5, c.y - 5.2)], st);
            // 桶身
            p.line_segment([pos2(c.x - 4.0, c.y - 2.5), pos2(c.x - 3.2, c.y + 5.0)], st);
            p.line_segment([pos2(c.x + 4.0, c.y - 2.5), pos2(c.x + 3.2, c.y + 5.0)], st);
            p.line_segment([pos2(c.x - 3.2, c.y + 5.0), pos2(c.x + 3.2, c.y + 5.0)], st);
            // 桶身垂直内肋条
            p.line_segment(
                [pos2(c.x - 1.2, c.y - 1.5), pos2(c.x - 1.2, c.y + 3.5)],
                Stroke::new(1.0, col.gamma_multiply(0.8)),
            );
            p.line_segment(
                [pos2(c.x + 1.2, c.y - 1.5), pos2(c.x + 1.2, c.y + 3.5)],
                Stroke::new(1.0, col.gamma_multiply(0.8)),
            );
        }
        StickyHeaderIcon::Close => {
            let r = 4.0;
            p.line_segment([pos2(c.x - r, c.y - r), pos2(c.x + r, c.y + r)], st);
            p.line_segment([pos2(c.x + r, c.y - r), pos2(c.x - r, c.y + r)], st);
        }
    }
}

/// 绘制符合各主题专属画风的便笺底栏格式化工具矢量图标
pub fn paint_sticky_tool_icon(
    p: &egui::Painter,
    rect: Rect,
    icon: StickyToolIcon,
    tc: &crate::view::ThemeColors,
    col: Color32,
) {
    let c = rect.center();
    let stroke_w = if tc.comic { 1.8 } else { 1.2 };
    let st = Stroke::new(stroke_w, col);

    match icon {
        StickyToolIcon::Bold => {
            let stem_w = if tc.comic { 2.4 } else { 1.8 };
            p.line_segment([pos2(c.x - 3.5, c.y - 4.5), pos2(c.x - 3.5, c.y + 4.5)], Stroke::new(stem_w, col));
            p.line_segment([pos2(c.x - 3.5, c.y - 4.5), pos2(c.x + 1.2, c.y - 4.5)], st);
            p.line_segment([pos2(c.x - 3.5, c.y), pos2(c.x + 1.5, c.y)], st);
            p.line_segment([pos2(c.x - 3.5, c.y + 4.5), pos2(c.x + 1.2, c.y + 4.5)], st);
            p.line_segment([pos2(c.x + 1.2, c.y - 4.5), pos2(c.x + 3.2, c.y - 2.2)], st);
            p.line_segment([pos2(c.x + 3.2, c.y - 2.2), pos2(c.x + 1.5, c.y)], st);
            p.line_segment([pos2(c.x + 1.5, c.y), pos2(c.x + 3.4, c.y + 2.2)], st);
            p.line_segment([pos2(c.x + 3.4, c.y + 2.2), pos2(c.x + 1.2, c.y + 4.5)], st);
        }
        StickyToolIcon::Italic => {
            let stem_w = if tc.comic { 2.0 } else { 1.5 };
            p.line_segment([pos2(c.x - 1.0, c.y - 4.5), pos2(c.x + 3.5, c.y - 4.5)], st);
            p.line_segment([pos2(c.x - 3.5, c.y + 4.5), pos2(c.x + 1.0, c.y + 4.5)], st);
            p.line_segment([pos2(c.x + 1.2, c.y - 4.5), pos2(c.x - 1.2, c.y + 4.5)], Stroke::new(stem_w, col));
        }
        StickyToolIcon::Underline => {
            p.line_segment([pos2(c.x - 3.2, c.y - 4.5), pos2(c.x - 3.2, c.y + 0.8)], st);
            p.line_segment([pos2(c.x + 3.2, c.y - 4.5), pos2(c.x + 3.2, c.y + 0.8)], st);
            p.line_segment([pos2(c.x - 3.2, c.y + 0.8), pos2(c.x - 1.5, c.y + 2.2)], st);
            p.line_segment([pos2(c.x - 1.5, c.y + 2.2), pos2(c.x + 1.5, c.y + 2.2)], st);
            p.line_segment([pos2(c.x + 1.5, c.y + 2.2), pos2(c.x + 3.2, c.y + 0.8)], st);
            p.line_segment([pos2(c.x - 4.5, c.y + 4.8), pos2(c.x + 4.5, c.y + 4.8)], Stroke::new(stroke_w * 1.1, col));
        }
        StickyToolIcon::Strikethrough => {
            p.line_segment([pos2(c.x + 2.8, c.y - 3.5), pos2(c.x - 0.5, c.y - 4.5)], st);
            p.line_segment([pos2(c.x - 0.5, c.y - 4.5), pos2(c.x - 3.0, c.y - 2.8)], st);
            p.line_segment([pos2(c.x - 3.0, c.y - 2.8), pos2(c.x + 2.8, c.y + 1.0)], st);
            p.line_segment([pos2(c.x + 2.8, c.y + 1.0), pos2(c.x + 3.0, c.y + 3.0)], st);
            p.line_segment([pos2(c.x + 3.0, c.y + 3.0), pos2(c.x + 0.5, c.y + 4.5)], st);
            p.line_segment([pos2(c.x + 0.5, c.y + 4.5), pos2(c.x - 2.8, c.y + 3.5)], st);
            p.line_segment([pos2(c.x - 5.0, c.y), pos2(c.x + 5.0, c.y)], Stroke::new(stroke_w * 1.2, col));
        }
        StickyToolIcon::Checklist => {
            let box_r = Rect::from_center_size(c, Vec2::splat(9.0));
            p.rect_stroke(box_r, if tc.retro16 { 0.0 } else { 1.5 }, st, StrokeKind::Inside);
            p.line_segment([pos2(c.x - 2.8, c.y - 0.2), pos2(c.x - 0.8, c.y + 2.2)], Stroke::new(stroke_w * 1.1, col));
            p.line_segment([pos2(c.x - 0.8, c.y + 2.2), pos2(c.x + 3.0, c.y - 2.0)], Stroke::new(stroke_w * 1.1, col));
        }
        StickyToolIcon::BulletList => {
            for dy in [-3.5, 0.0, 3.5] {
                let y = c.y + dy;
                if tc.retro16 {
                    p.rect_filled(Rect::from_min_size(pos2(c.x - 4.8, y - 1.0), Vec2::splat(2.0)), 0.0, col);
                } else {
                    p.circle_filled(pos2(c.x - 4.0, y), 1.2, col);
                }
                p.line_segment([pos2(c.x - 1.5, y), pos2(c.x + 4.5, y)], st);
            }
        }
        StickyToolIcon::Timestamp => {
            p.circle_stroke(c, 4.5, st);
            p.line_segment([c, pos2(c.x, c.y - 2.6)], st);
            p.line_segment([c, pos2(c.x + 2.2, c.y)], st);
        }
        StickyToolIcon::Copy => {
            let back_r = Rect::from_min_max(pos2(c.x - 4.0, c.y - 4.5), pos2(c.x + 1.5, c.y + 2.5));
            let front_r = Rect::from_min_max(pos2(c.x - 1.5, c.y - 2.0), pos2(c.x + 4.0, c.y + 5.0));
            let r_round = if tc.retro16 { 0.0 } else { 1.0 };
            p.rect_stroke(back_r, r_round, Stroke::new(1.0, col.gamma_multiply(0.6)), StrokeKind::Inside);
            p.rect_stroke(front_r, r_round, st, StrokeKind::Inside);
        }
        StickyToolIcon::Clear => {
            let p1 = pos2(c.x - 3.5, c.y - 1.0);
            let p2 = pos2(c.x + 1.0, c.y - 4.5);
            let p3 = pos2(c.x + 4.5, c.y - 1.5);
            let p4 = pos2(c.x, c.y + 2.5);
            p.line_segment([p1, p2], st);
            p.line_segment([p2, p3], st);
            p.line_segment([p3, p4], st);
            p.line_segment([p4, p1], st);
            p.line_segment([pos2(c.x - 5.0, c.y + 4.5), pos2(c.x + 4.0, c.y + 4.5)], Stroke::new(1.0, col.gamma_multiply(0.6)));
        }
    }
}

/// 绘制便笺顶栏小按钮 (支持各主题自适应底框与自绘矢量图标)
fn draw_header_btn(
    ui: &mut Ui,
    icon: StickyHeaderIcon,
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

    let (content_rect, icon_col) =
        paint_sticky_btn_frame(p, rect, tc, palette, is_hovered, is_active, false);
    paint_sticky_header_icon(p, content_rect, icon, tc, icon_col);

    resp
}

/// 绘制便笺底栏格式化工具按钮 (支持各主题自适应底框与自绘矢量图标)
fn draw_tool_btn(
    ui: &mut Ui,
    icon: StickyToolIcon,
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

    let (content_rect, icon_col) =
        paint_sticky_btn_frame(p, rect, tc, palette, is_hovered, is_active, false);
    paint_sticky_tool_icon(p, content_rect, icon, tc, icon_col);

    resp
}

/// 绘制便笺右下角原生缩放把手 (Resize Grip Handle)
fn draw_resize_grip(
    ui: &mut Ui,
    rect: Rect,
    tc: &crate::view::ThemeColors,
    palette: &NotePalette,
) {
    let p = ui.painter();
    let col = palette.fg.gamma_multiply(0.35);
    let r = rect.right() - 4.0;
    let b = rect.bottom() - 4.0;

    if tc.retro16 {
        // Retro 16-Bit: 街机像素阶梯方块
        let pts = [(2.0, 2.0), (6.0, 2.0), (2.0, 6.0), (10.0, 2.0), (6.0, 6.0), (2.0, 10.0)];
        for (dx, dy) in pts {
            p.rect_filled(
                Rect::from_min_size(pos2(r - dx, b - dy), Vec2::splat(2.0)),
                0.0,
                col,
            );
        }
    } else if tc.comic {
        // Comic 3渲2: 纯黑实体斜向刻度划线
        p.line_segment([pos2(r - 3.0, b), pos2(r, b - 3.0)], Stroke::new(1.8, Color32::BLACK));
        p.line_segment([pos2(r - 7.0, b), pos2(r, b - 7.0)], Stroke::new(1.8, Color32::BLACK));
        p.line_segment([pos2(r - 11.0, b), pos2(r, b - 11.0)], Stroke::new(1.8, Color32::BLACK));
    } else {
        // 现代 Fluent / Crystal 柔和圆点阵列
        let pts = [(3.0, 3.0), (7.0, 3.0), (3.0, 7.0), (11.0, 3.0), (7.0, 7.0), (3.0, 11.0)];
        for (dx, dy) in pts {
            p.circle_filled(pos2(r - dx, b - dy), 1.2, col);
        }
    }
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

    #[test]
    fn test_sticky_notes_independent_viewport_window_rendering() {
        let mut app = App::build(
            vec![],
            None,
            Trace::new(),
            crate::settings::Settings::default(),
            false,
        );
        app.sticky_notes.open = true;
        let ctx = egui::Context::default();

        // 1. 默认现代主题下独立视窗运行
        let mut out = ctx.run_ui(Default::default(), |_ui_ctx| {
            app.sticky_notes_window(&ctx);
        });
        out.textures_delta.clear();
        assert!(app.sticky_notes.open);

        // 2. Comic 美漫 3渲2 主题下独立视窗渲染
        app.tc.comic = true;
        let mut out = ctx.run_ui(Default::default(), |_ui_ctx| {
            app.sticky_notes_window(&ctx);
        });
        out.textures_delta.clear();

        // 3. Crystal 水晶毛玻璃主题下独立视窗渲染
        app.tc.comic = false;
        app.tc.crystal = true;
        let mut out = ctx.run_ui(Default::default(), |_ui_ctx| {
            app.sticky_notes_window(&ctx);
        });
        out.textures_delta.clear();

        // 4. Retro 16-Bit 街机主题下独立视窗渲染
        app.tc.crystal = false;
        app.tc.retro16 = true;
        let mut out = ctx.run_ui(Default::default(), |_ui_ctx| {
            app.sticky_notes_window(&ctx);
        });
        out.textures_delta.clear();

        // 5. 切换到列表模式并渲染
        app.sticky_notes.show_list = true;
        let mut out = ctx.run_ui(Default::default(), |_ui_ctx| {
            app.sticky_notes_window(&ctx);
        });
        out.textures_delta.clear();
        assert!(app.sticky_notes.show_list);
    }

    #[test]
    fn test_all_sticky_notes_themed_icons_and_frames() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |root| {
            let p = root.painter();
            let rect = Rect::from_min_size(pos2(10.0, 10.0), Vec2::splat(22.0));

            let mut tc_list = vec![
                crate::view::ThemeColors::builtin(false),
                crate::view::ThemeColors::builtin(true),
            ];

            let mut tc_comic = crate::view::ThemeColors::builtin(false);
            tc_comic.comic = true;
            tc_list.push(tc_comic);

            let mut tc_retro = crate::view::ThemeColors::builtin(true);
            tc_retro.retro16 = true;
            tc_list.push(tc_retro);

            let mut tc_crystal = crate::view::ThemeColors::builtin(false);
            tc_crystal.crystal = true;
            tc_list.push(tc_crystal);

            let header_icons = [
                StickyHeaderIcon::New,
                StickyHeaderIcon::List,
                StickyHeaderIcon::BackToEdit,
                StickyHeaderIcon::Palette,
                StickyHeaderIcon::Pin(false),
                StickyHeaderIcon::Pin(true),
                StickyHeaderIcon::Delete,
                StickyHeaderIcon::Close,
            ];

            let tool_icons = [
                StickyToolIcon::Bold,
                StickyToolIcon::Italic,
                StickyToolIcon::Underline,
                StickyToolIcon::Strikethrough,
                StickyToolIcon::Checklist,
                StickyToolIcon::BulletList,
                StickyToolIcon::Timestamp,
                StickyToolIcon::Copy,
                StickyToolIcon::Clear,
            ];

            for tc in &tc_list {
                let pal = get_palette(0, tc);
                for hovered in [false, true] {
                    for active in [false, true] {
                        let (c_rect, col) = paint_sticky_btn_frame(p, rect, tc, &pal, hovered, active, false);
                        assert!(c_rect.width() > 0.0);
                        assert_ne!(col, Color32::TRANSPARENT);

                        for icon in &header_icons {
                            paint_sticky_header_icon(p, c_rect, *icon, tc, col);
                        }
                        for icon in &tool_icons {
                            paint_sticky_tool_icon(p, c_rect, *icon, tc, col);
                        }
                    }
                }
            }
        });
        out.textures_delta.clear();
    }
}
