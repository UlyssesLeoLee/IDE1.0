//! 小工具窗口: 剪贴板历史 / 控制字符表 / 戳记设置 / 折行列数。
//! 状态和绘制放这里, 对文档的修改一律以 `Ev` 交还给 `App` (这里不碰文档, 也不碰 egui 之外的东西)。

use crate::i18n::trz;
use crate::settings::ViewState;
use crate::textconv::{self, StampKind, StampPos};
use eframe::egui::{self, Key};

/// 历史最多留几条 / 每条最多多少字节 (再大的不进历史, 免得一次复制整个大文件吃掉内存)
pub const CLIP_MAX: usize = 20;
pub const CLIP_ITEM_MAX: usize = 1 << 20;

/// 折行列数的上限 (再宽没有意义, 也防止输入把布局撑爆)
pub const WRAP_COL_MAX: usize = 2000;

#[derive(Default)]
pub struct Tools {
    /// 剪贴板历史, 新的在前; 只存本进程里复制 / 剪切过的文字, 不落盘, 不轮询系统剪贴板
    pub clip: Vec<String>,
    pub clip_open: bool,
    pub ctrl_open: bool,
    pub stamp_open: bool,
    pub wrap_open: bool,
    wrap_text: String,
    pub col_open: bool,
    col_text: String,
    /// 通用选择列表 (窗口一览 / 最近的文件 / 字符集 / 帮助检索…); 画窗口和处理选中在 `app/picks.rs`
    pub pick: Option<crate::pick::Pick>,
    /// 关键字帮助自动显示: 上一次提示过的词, 和上一次看过的 (文档, 光标) —— 光标没动就不再查
    pub key_word: String,
    pub key_pos: Option<(u64, [usize; 2])>,
}

/// 窗口里点出来的动作, 由 `App` 去改文档 / 剪贴板
pub enum Ev {
    /// 把这条历史粘贴到光标处
    Paste(String),
    /// 把这条历史重新放回系统剪贴板
    Copy(String),
    /// 插入控制字符
    Ctrl(char),
    /// 折行列数定了 (0 = 按窗口宽度)
    WrapCol(usize),
    /// 列块替换: 把矩形选区换成这段文字 (多行用换行分隔, 一行对应一行)
    ColumnReplace(String),
}

impl Tools {
    /// 记一条复制 / 剪切的内容: 去重后置顶, 空的和太大的不记
    pub fn push_clip(&mut self, text: &str) {
        if text.is_empty() || text.len() > CLIP_ITEM_MAX {
            return;
        }
        self.clip.retain(|c| c != text);
        self.clip.insert(0, text.to_string());
        self.clip.truncate(CLIP_MAX);
    }

    /// 打开「折行列数」输入框 (预填当前值; 没有就 80)
    pub fn open_wrap(&mut self, cur: usize) {
        self.wrap_text = if cur > 0 {
            cur.to_string()
        } else {
            "80".into()
        };
        self.wrap_open = true;
    }

    /// 打开「列块替换」输入框 (预填列块现在的内容)
    pub fn open_col(&mut self, prefill: String) {
        self.col_text = prefill;
        self.col_open = true;
    }

    /// 一行预览: 换行显示成 ↵, 最多 `n` 个字符
    pub fn preview(text: &str, n: usize) -> String {
        let mut s: String = text
            .chars()
            .take(n)
            .map(|c| {
                if c == '\n' {
                    '↵'
                } else if c == '\r' {
                    ' '
                } else if c == '\t' {
                    '→'
                } else {
                    c
                }
            })
            .collect();
        if text.chars().count() > n {
            s.push('…');
        }
        s
    }

    /// 画所有开着的小窗口; 返回这一帧里点出来的动作
    pub fn windows(&mut self, ctx: &egui::Context, view: &mut ViewState) -> Vec<Ev> {
        let mut evs = vec![];
        if self.clip_open {
            let mut open = true;
            egui::Window::new(trz("剪贴板历史"))
                .open(&mut open)
                .collapsible(false)
                .default_width(420.0)
                .default_pos(egui::pos2(24.0, 56.0))
                .show(ctx, |ui| {
                    if self.clip.is_empty() {
                        ui.label(trz("还没有复制过内容 (只记录本程序里的复制 / 剪切)"));
                    }
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for c in &self.clip {
                                ui.horizontal(|ui| {
                                    if ui.small_button(trz("粘贴")).clicked() {
                                        evs.push(Ev::Paste(c.clone()));
                                    }
                                    if ui.small_button(trz("复制")).clicked() {
                                        evs.push(Ev::Copy(c.clone()));
                                    }
                                    ui.label(format!(
                                        "{}  ({} {})",
                                        Self::preview(c, 60),
                                        c.chars().count(),
                                        trz("字符")
                                    ));
                                });
                            }
                        });
                    ui.separator();
                    if ui.button(trz("清空")).clicked() {
                        self.clip.clear();
                    }
                });
            self.clip_open = open;
        }
        if self.ctrl_open {
            let mut open = true;
            egui::Window::new(trz("控制字符"))
                .open(&mut open)
                .collapsible(false)
                .default_width(420.0)
                .default_pos(egui::pos2(24.0, 56.0))
                .show(ctx, |ui| {
                    ui.label(trz("点要插入的控制字符"));
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in textconv::CTRL_NAMES.iter().enumerate() {
                            let Some(c) = textconv::ctrl_char(i) else {
                                continue;
                            };
                            if ui.button(format!("{name} (0x{:02X})", c as u32)).clicked() {
                                evs.push(Ev::Ctrl(c));
                            }
                        }
                    });
                });
            self.ctrl_open = open;
        }
        if self.stamp_open {
            let mut open = true;
            egui::Window::new(trz("戳记设置"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_pos(egui::pos2(24.0, 56.0))
                .show(ctx, |ui| {
                    let mut kind = StampKind::from_u8(view.stamp_kind);
                    let mut pos = StampPos::from_u8(view.stamp_pos);
                    ui.radio_value(&mut kind, StampKind::Date, trz("日期 (YYYY/MM/DD)"));
                    ui.radio_value(&mut kind, StampKind::Time, trz("时间 (HH:MM:SS)"));
                    ui.radio_value(&mut kind, StampKind::DateTime, trz("日期 + 时间"));
                    ui.separator();
                    ui.radio_value(
                        &mut pos,
                        StampPos::LineStart,
                        trz("行首 (缩进之后, 写成 [戳记] )"),
                    );
                    ui.radio_value(&mut pos, StampPos::AtCursor, trz("光标处"));
                    ui.separator();
                    ui.label(format!(
                        "{} {}",
                        trz("预览:"),
                        kind.text(&textconv::local_now())
                    ));
                    (view.stamp_kind, view.stamp_pos) = (kind.to_u8(), pos.to_u8());
                });
            self.stamp_open = open;
        }
        if self.wrap_open {
            let mut open = true;
            egui::Window::new(trz("按指定列数折行"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_pos(egui::pos2(24.0, 56.0))
                .show(ctx, |ui| {
                    ui.label(trz("列数 (半角字符数; 0 = 按窗口宽度)"));
                    let r =
                        ui.add(egui::TextEdit::singleline(&mut self.wrap_text).desired_width(80.0));
                    r.request_focus();
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        evs.push(Ev::WrapCol(parse_wrap_col(&self.wrap_text)));
                        self.wrap_open = false;
                    }
                });
            if !open || ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.wrap_open = false;
            }
        }
        if self.col_open {
            let mut open = true;
            let mut apply = false;
            egui::Window::new(trz("列块替换"))
                .open(&mut open)
                .collapsible(false)
                .default_width(360.0)
                .default_pos(egui::pos2(24.0, 56.0))
                .show(ctx, |ui| {
                    ui.label(trz("列块替换为 (多行用换行分隔)"));
                    ui.add(
                        egui::TextEdit::multiline(&mut self.col_text)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        if ui.button(trz("应用")).clicked() {
                            apply = true;
                        }
                        if ui.button(trz("取消")).clicked() {
                            self.col_open = false;
                        }
                    });
                });
            if apply {
                evs.push(Ev::ColumnReplace(std::mem::take(&mut self.col_text)));
                self.col_open = false;
            }
            if !open {
                self.col_open = false;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.col_open = false;
            self.clip_open = false;
            self.ctrl_open = false;
            self.stamp_open = false;
        }
        evs
    }
}

/// 折行列数输入: 非数字 = 0 (窗口宽度); 过大夹到上限; 过小 (< 10) 夹到 10 免得一字一行
pub fn parse_wrap_col(s: &str) -> usize {
    match s.trim().parse::<usize>() {
        Ok(0) | Err(_) => 0,
        Ok(n) => n.clamp(10, WRAP_COL_MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_history_dedupes_caps_and_skips_huge_or_empty() {
        let mut t = Tools::default();
        t.push_clip("a");
        t.push_clip("b");
        t.push_clip("a");
        assert_eq!(t.clip, vec!["a", "b"], "重复的置顶");
        t.push_clip("");
        t.push_clip(&"x".repeat(CLIP_ITEM_MAX + 1));
        assert_eq!(t.clip.len(), 2);
        for n in 0..50 {
            t.push_clip(&format!("item{n}"));
        }
        assert_eq!(t.clip.len(), CLIP_MAX);
        assert_eq!(t.clip[0], "item49");
    }

    #[test]
    fn preview_shows_newlines_and_truncates() {
        assert_eq!(Tools::preview("a\nb\tc", 10), "a↵b→c");
        assert_eq!(Tools::preview("abcdef", 3), "abc…");
        assert_eq!(Tools::preview("日本語", 3), "日本語");
    }

    #[test]
    fn wrap_column_input_is_clamped() {
        assert_eq!(parse_wrap_col("80"), 80);
        assert_eq!(parse_wrap_col(" 0 "), 0);
        assert_eq!(parse_wrap_col("abc"), 0);
        assert_eq!(parse_wrap_col("-5"), 0);
        assert_eq!(parse_wrap_col("3"), 10);
        assert_eq!(parse_wrap_col("99999999"), WRAP_COL_MAX);
        assert_eq!(
            parse_wrap_col("99999999999999999999999999"),
            0,
            "溢出 = 解析失败"
        );
    }
}
