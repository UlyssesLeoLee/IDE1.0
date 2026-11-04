//! 欢迎页 (Tauri 的 `#welcome`): 完整界面里, 没有项目 / 文件 (唯一的空白无标题文档) 时盖在编辑区上 ——
//! 大徽标 (96px, 带青色光晕) / 「IDE1.0」字标 (34px, 字距 0.14em) / 「打开文件夹」按钮 / 一行按键提示 (kbd 小方块)。
//! 极简布局 (默认) 不画, 保持单个文本框; 在文档里键入任何字, 欢迎页就让位。

use super::*;

/// 徽标边长 / 字标字号 / 元素间距 (CSS `--fs-display` 34, `--sp-5` 20)
const LOGO: f32 = 96.0;
const H1: f32 = 34.0;
const GAP: f32 = 20.0;
const BTN_PAD: Vec2 = Vec2::new(24.0, 8.0);
const KBD_PAD: f32 = 6.0;

/// 按键提示里的一项: 一个 kbd 小方块, 或一段文字
#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    Key(&'static str),
    Text(&'static str),
}

/// Tauri 欢迎页那一行: `i`/`a` INSERT · `v` VISUAL · `Esc` NORMAL · `Ctrl`+`S` 保存 · `Ctrl`+`` ` `` 终端 · `F1` 帮助
pub fn hints() -> Vec<Hint> {
    use Hint::{Key, Text};
    vec![
        Key("i"), Text("/"), Key("a"), Text("INSERT ·"), Key("v"), Text("VISUAL ·"), Key("Esc"), Text("NORMAL ·"),
        Key("Ctrl"), Text("+"), Key("S"), Text("保存 ·"), Key("Ctrl"), Text("+"), Key("`"), Text("终端 ·"), Key("F1"), Text("帮助"),
    ]
}

/// 把一串项目按宽度排成行 (贪心, 一项都不拆); 返回每行 (起, 止) 下标。`widths[i]` 已含项间距
pub fn wrap_items(widths: &[f32], max_w: f32) -> Vec<(usize, usize)> {
    let mut lines = vec![];
    let (mut start, mut w) = (0, 0.0);
    for (i, iw) in widths.iter().enumerate() {
        if i > start && w + iw > max_w {
            lines.push((start, i));
            start = i;
            w = 0.0;
        }
        w += iw;
    }
    if start < widths.len() { lines.push((start, widths.len())); }
    lines
}

/// 把项目分成不可拆的组: 键 + 连接符 (`+` `/`) + 键 算一组
pub fn unit_ranges(items: &[Hint]) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut i = 0;
    while i < items.len() {
        let mut j = i + 1;
        while j + 1 < items.len() && matches!(items[j], Hint::Text("+" | "/")) && matches!(items[j + 1], Hint::Key(_)) { j += 2; }
        out.push((i, j));
        i = j;
    }
    out
}

/// 欢迎页什么时候出现: 完整界面 (活动栏开着) + 没有分割 + 只有一个文档 + 它没有路径且一个字都没有
pub fn visible(full_ui: bool, split: bool, docs: &[Doc], active: usize) -> bool {
    if !full_ui || split || docs.len() != 1 { return false; }
    let d = &docs[active.min(docs.len() - 1)];
    d.path.is_none() && d.lines.len() == 1 && d.lines[0].is_empty()
}

/// 强调色压暗到 62% (Tauri `--pill-mix`), 悬停 74%
fn pill(blue: Color32, hover: bool) -> Color32 {
    let k = if hover { 0.74 } else { 0.62 };
    Color32::from_rgb((blue.r() as f32 * k) as u8, (blue.g() as f32 * k) as u8, (blue.b() as f32 * k) as u8)
}

impl App {
    pub(super) fn welcome_visible(&self) -> bool {
        visible(self.cfg.view.rail, self.split_on, &self.docs, self.active)
    }

    /// 盖在编辑区 `rect` 上画欢迎页; 点「打开文件夹」走 `open_folder_dialog`
    pub(super) fn welcome(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let tc = self.tc;
        let rect = Rect::from_min_max(egui::pos2(rect.left(), rect.top() + self.sys.ruler_h), rect.max);
        if self.sys.logo.is_none() {
            self.sys.logo = crate::appicon::logo().map(|ic| ui.ctx().load_texture("welcome-logo", egui::ColorImage::from_rgba_unmultiplied([ic.width as usize, ic.height as usize], &ic.rgba), egui::TextureOptions::LINEAR));
        }
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, tc.bg_edit);
        // 先量各块的高度, 整体垂直居中
        let h1_font = FontId::monospace(H1);
        let spacing = H1 * 0.14;
        let word = "IDE1.0";
        let glyphs: Vec<_> = word.chars().map(|c| p.layout_no_wrap(c.to_string(), h1_font.clone(), tc.fg.gamma_multiply(0.9))).collect();
        let h1_w: f32 = glyphs.iter().map(|g| g.size().x + spacing).sum::<f32>() - spacing;
        let h1_h = glyphs.iter().map(|g| g.size().y).fold(0.0, f32::max);
        let open_txt = p.layout_no_wrap(trz("打开文件夹").to_string(), FontId::proportional(13.0), Color32::WHITE);
        let btn_size = open_txt.size() + BTN_PAD * 2.0;
        let small = FontId::proportional(11.0);
        let items = hints();
        let galleys: Vec<_> = items.iter().map(|h| match h {
            Hint::Key(k) => p.layout_no_wrap((*k).to_string(), FontId::monospace(11.0), tc.fg),
            Hint::Text(t) => p.layout_no_wrap(trz(t).to_string(), small.clone(), tc.fg_dim),
        }).collect();
        let widths: Vec<f32> = items.iter().zip(&galleys).map(|(h, g)| match h {
            Hint::Key(_) => g.size().x + 2.0 * KBD_PAD + 6.0,
            Hint::Text(_) => g.size().x + 6.0,
        }).collect();
        let max_w = (rect.width() - 48.0).max(120.0);
        // 「Ctrl + S」「i / a」是一组, 换行时不拆开: 先按组排, 再还原成项目下标
        let units = unit_ranges(&items);
        let uw: Vec<f32> = units.iter().map(|&(a, b)| widths[a..b].iter().sum()).collect();
        let lines: Vec<(usize, usize)> = wrap_items(&uw, max_w).into_iter().map(|(a, b)| (units[a].0, units[b - 1].1)).collect();
        let line_h = 11.0 * 1.9 + 2.0;
        let total_h = LOGO + GAP + h1_h + GAP + btn_size.y + GAP + line_h * lines.len() as f32;
        let mut y = rect.center().y - total_h / 2.0;
        y = y.max(rect.top() + 8.0);
        let cx = rect.center().x;
        // 徽标 + 光晕 (rgba(56,189,248,.45), 模糊 16px: 几层渐淡的圆近似)
        let logo_c = egui::pos2(cx, y + LOGO / 2.0);
        for i in 0..8 {
            let r = LOGO * 0.34 + i as f32 * 3.0;
            p.circle_filled(logo_c, r, Color32::from_rgba_unmultiplied(56, 189, 248, (7 - i as u8) * 2 + 1));
        }
        if let Some(t) = &self.sys.logo {
            p.image(t.id(), Rect::from_center_size(logo_c, Vec2::splat(LOGO)), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        y += LOGO + GAP;
        let mut x = cx - h1_w / 2.0;
        for g in &glyphs {
            let w = g.size().x;
            p.galley(egui::pos2(x, y), g.clone(), tc.fg);
            x += w + spacing;
        }
        y += h1_h + GAP;
        let btn = Rect::from_center_size(egui::pos2(cx, y + btn_size.y / 2.0), btn_size);
        let resp = ui.interact(btn, ui.make_persistent_id("welcome-open"), Sense::click());
        if tc.dark {
            p.rect_filled(btn, 6.0, pill(tc.blue, resp.hovered()));
            p.galley(btn.center() - open_txt.size() / 2.0, open_txt, Color32::WHITE);
        } else {
            // 浅色: 白底 + 1px 灰边 + 深色字, 悬停 #f6f8fa (Tauri `.btn` 浅色规则)
            let bg = if resp.hovered() { Color32::from_rgb(0xf6, 0xf8, 0xfa) } else { Color32::WHITE };
            p.rect(btn, 6.0, bg, egui::Stroke::new(1.0, Color32::from_rgb(0xd0, 0xd7, 0xde)), egui::StrokeKind::Inside);
            let txt = p.layout_no_wrap(trz("打开文件夹").to_string(), FontId::proportional(13.0), tc.fg);
            p.galley(btn.center() - txt.size() / 2.0, txt, tc.fg);
        }
        if resp.on_hover_text(trz("打开文件夹 (Ctrl+K O)")).clicked() { self.open_folder_dialog(); }
        y += btn_size.y + GAP;
        let kbd_bg = if tc.dark { Color32::from_gray(51) } else { Color32::from_gray(238) };
        let kbd_edge = if tc.dark { Color32::from_gray(69) } else { Color32::from_gray(204) };
        for (a, b) in lines {
            let w: f32 = widths[a..b].iter().sum();
            let mut x = cx - w / 2.0;
            let cy = y + line_h / 2.0;
            for i in a..b {
                let g = &galleys[i];
                match items[i] {
                    Hint::Key(_) => {
                        let r = Rect::from_min_size(egui::pos2(x + 3.0, cy - (g.size().y + 4.0) / 2.0), Vec2::new(g.size().x + 2.0 * KBD_PAD, g.size().y + 4.0));
                        p.rect(r, 3.0, kbd_bg, egui::Stroke::new(1.0, kbd_edge), egui::StrokeKind::Inside);
                        p.galley(egui::pos2(r.left() + KBD_PAD, r.center().y - g.size().y / 2.0), g.clone(), tc.fg);
                    }
                    Hint::Text(_) => { p.galley(egui::pos2(x + 3.0, cy - g.size().y / 2.0), g.clone(), tc.fg_dim); }
                }
                x += widths[i];
            }
            y += line_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str, path: bool) -> Doc {
        let mut d = Doc::untitled(1, 1);
        d.lines = text.split('\n').map(str::to_string).collect();
        if path { d.path = Some(PathBuf::from("a.txt")); }
        d
    }

    #[test]
    fn welcome_shows_only_for_the_empty_untitled_doc_in_the_full_ui() {
        assert!(visible(true, false, &[doc("", false)], 0));
        assert!(!visible(false, false, &[doc("", false)], 0), "极简布局 (默认) 不画欢迎页");
        assert!(!visible(true, true, &[doc("", false)], 0), "分割时不画");
        assert!(!visible(true, false, &[doc("x", false)], 0), "有字就让位");
        assert!(!visible(true, false, &[doc("a\nb", false)], 0));
        assert!(!visible(true, false, &[doc("", true)], 0), "已有路径 = 打开的文件");
        assert!(!visible(true, false, &[doc("", false), doc("", false)], 0), "多个标签不画");
    }

    #[test]
    fn hint_items_wrap_without_splitting_and_cover_everything() {
        let w = vec![20.0; 10];
        let l = wrap_items(&w, 70.0);
        assert_eq!(l, vec![(0, 3), (3, 6), (6, 9), (9, 10)]);
        assert_eq!(wrap_items(&w, 1000.0), vec![(0, 10)]);
        assert_eq!(wrap_items(&[500.0], 10.0), vec![(0, 1)], "单项比行宽还宽也不丢");
        assert!(wrap_items(&[], 10.0).is_empty());
    }

    #[test]
    fn key_combos_stay_together_when_wrapping() {
        let u = unit_ranges(&hints());
        assert!(u.contains(&(0, 3)), "i / a");
        assert!(u.contains(&(8, 11)), "Ctrl + S");
        assert_eq!(u.iter().map(|r| r.1 - r.0).sum::<usize>(), hints().len());
    }

    #[test]
    fn hints_match_the_tauri_line() {
        let keys: Vec<_> = hints().into_iter().filter_map(|h| if let Hint::Key(k) = h { Some(k) } else { None }).collect();
        assert_eq!(keys, vec!["i", "a", "v", "Esc", "Ctrl", "S", "Ctrl", "`", "F1"]);
    }
}
