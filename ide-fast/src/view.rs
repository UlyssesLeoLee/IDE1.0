//! 视图与设计: 主题变量 -> 配色 (`ThemeColors`) 和语法高亮 `Theme`、修改行标记、空白符标记、状态栏的列 / 选区统计。
//! 都是不碰 egui 状态的纯函数, 方便单测; 绘制在 app.rs。主题变量就是 Tauri 版的 CSS 变量 (`--bg-edit` `--fg` `--blue` ...),
//! 所以六套内置主题 (dark / light / hc / clear / crystal / retro16) 在两个前端里取的是同一组颜色。

use crate::plugins::parse_color;
use eframe::egui::{self, Color32};
use editcore::syntax_styling::{Color, Style, Theme, TokenStyle};
use editcore::type_config::TokenKind;
use std::collections::BTreeMap;

/// 编辑区 / 状态栏 / 标签栏用到的几个语义色
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeColors {
    pub dark: bool,
    pub bg_edit: Color32,
    pub bg_status: Color32,
    pub bg_bar: Color32,
    pub fg: Color32,
    pub fg_dim: Color32,
    pub border: Color32,
    pub blue: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub red: Color32,
    pub orange: Color32,
    /// 侧栏 / 面板底色 (Tauri `--bg-side`)
    pub bg_side: Color32,
    /// 活动栏 / 折叠细条底色 (`--bg-rail`)
    pub bg_rail: Color32,
    /// 标签栏底色 (`--bg-chrome`)
    pub bg_chrome: Color32,
    /// 悬停叠色 (`--bg-hover`), 半透明
    pub bg_hover: Color32,
}

fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

/// 半透明色铺到底色上 (原生窗口里没有底图可透)
fn over(c: Color32, base: Color32) -> Color32 {
    if c.a() == 255 {
        return c;
    }
    let a = c.a() as f32 / 255.0;
    // Color32 存的是预乘 alpha 的分量, 前景不用再乘 a
    let mix = |f: u8, b: u8| (f as f32 + b as f32 * (1.0 - a)).round().min(255.0) as u8;
    Color32::from_rgb(mix(c.r(), base.r()), mix(c.g(), base.g()), mix(c.b(), base.b()))
}

impl ThemeColors {
    /// 内置深 / 浅色 (没选主题插件时)
    pub fn builtin(dark: bool) -> Self {
        let g = Color32::from_gray;
        let rgb = Color32::from_rgb;
        // 取值来自 Tauri 版 editor.html 的 :root / THEMES (见 docs/VISUAL-PARITY.md 第 2 节)
        if dark {
            ThemeColors { dark, bg_edit: g(30), bg_status: rgb(0, 122, 204), bg_bar: rgb(37, 37, 38), fg: g(212), fg_dim: g(157), border: g(51), blue: rgb(86, 156, 214), green: rgb(106, 190, 120), yellow: rgb(220, 200, 90), red: rgb(230, 90, 90), orange: rgb(214, 150, 80), bg_side: g(30), bg_rail: g(51), bg_chrome: g(42), bg_hover: Color32::from_white_alpha(20) }
        } else {
            ThemeColors { dark, bg_edit: g(255), bg_status: rgb(0, 95, 184), bg_bar: g(248), fg: g(31), fg_dim: g(95), border: g(229), blue: rgb(0, 95, 184), green: rgb(20, 130, 50), yellow: rgb(150, 110, 0), red: rgb(190, 40, 40), orange: rgb(180, 90, 0), bg_side: g(255), bg_rail: g(240), bg_chrome: g(242), bg_hover: Color32::from_black_alpha(10) }
        }
    }

    /// 状态栏上的字色: 底色深就用白, 浅就用前景 (Tauri 是 `--fg-on-accent` 白字, 主题自定义浅色状态栏时不能白字白底)
    pub fn on_status(&self) -> Color32 {
        if luminance(self.bg_status) < 0.5 { Color32::WHITE } else { self.fg }
    }

    /// 没选主题插件时的 egui 外观: 在 egui 深 / 浅基底上套 Tauri 的颜色令牌 (面板 / 编辑区 / 控件填充 / 选区 / 圆角)
    pub fn visuals(&self) -> egui::Visuals {
        let mut v = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        let g = Color32::from_gray;
        v.panel_fill = self.bg_side;
        v.window_fill = if self.dark { g(45) } else { Color32::WHITE };
        v.extreme_bg_color = self.bg_edit;
        v.faint_bg_color = if self.dark { g(37) } else { g(246) };
        v.override_text_color = Some(self.fg);
        v.widgets.noninteractive.fg_stroke.color = self.fg_dim;
        v.widgets.noninteractive.bg_stroke.color = self.border;
        v.window_stroke = egui::Stroke::new(1.0, if self.dark { g(69) } else { g(204) });
        let (w0, w1, w2) = if self.dark { (g(50), g(66), g(80)) } else { (g(238), g(228), g(214)) };
        v.widgets.inactive.weak_bg_fill = w0;
        v.widgets.inactive.bg_fill = w0;
        v.widgets.hovered.weak_bg_fill = w1;
        v.widgets.hovered.bg_fill = w1;
        v.widgets.active.weak_bg_fill = w2;
        v.widgets.active.bg_fill = w2;
        v.selection.bg_fill = Color32::from_rgba_unmultiplied(self.blue.r(), self.blue.g(), self.blue.b(), 90);
        v.hyperlink_color = self.blue;
        let r = egui::CornerRadius::same(3);
        v.widgets.noninteractive.corner_radius = r;
        v.widgets.inactive.corner_radius = r;
        v.widgets.hovered.corner_radius = r;
        v.widgets.active.corner_radius = r;
        v.menu_corner_radius = egui::CornerRadius::same(2);
        v.window_corner_radius = egui::CornerRadius::same(8);
        v
    }

    /// 从主题变量取色; 缺的 / 认不出的 (`transparent` `var(..)`) 用深 / 浅基底补, 基底由 `--bg-edit` 的亮度定
    pub fn from_vars(vars: &BTreeMap<String, String>) -> Self {
        let get = |k: &str| vars.get(k).and_then(|v| parse_color(v));
        let dark = get("--bg-edit").map(|c| c.a() == 0 || luminance(c) < 0.5).unwrap_or(true);
        let mut t = Self::builtin(dark);
        if let Some(c) = get("--bg-edit").filter(|c| c.a() > 0) { t.bg_edit = over(c, t.bg_edit); }
        let base = t.bg_edit;
        let pick = |k: &str, slot: &mut Color32| { if let Some(c) = get(k) { *slot = over(c, base); } };
        pick("--bg-status", &mut t.bg_status);
        pick("--bg-bar", &mut t.bg_bar);
        pick("--bg-side", &mut t.bg_side);
        pick("--bg-rail", &mut t.bg_rail);
        pick("--bg-chrome", &mut t.bg_chrome);
        // 主题没给的底色跟着它自己的菜单栏 / 编辑区走, 不混入内置深浅色
        if get("--bg-rail").is_none() { t.bg_rail = t.bg_bar; }
        if get("--bg-chrome").is_none() { t.bg_chrome = t.bg_bar; }
        if get("--bg-side").is_none() { t.bg_side = t.bg_edit; }
        pick("--fg", &mut t.fg);
        pick("--fg-dim", &mut t.fg_dim);
        pick("--border", &mut t.border);
        pick("--blue", &mut t.blue);
        pick("--green", &mut t.green);
        pick("--yellow", &mut t.yellow);
        pick("--red", &mut t.red);
        pick("--orange", &mut t.orange);
        t
    }

    /// 语法高亮用的 Theme: 先取内置深 / 浅色, 再把关键字 / 字符串 / 数字 / 注释 / 函数 / 类型换成主题色;
    /// 和背景对比太弱的颜色 (浅色主题里的亮黄 ...) 保留内置色, 免得字看不见
    pub fn syntax_theme(&self) -> Theme {
        let mut th = if self.dark { Theme::dark() } else { Theme::light() };
        let conv = |c: Color32| Color(c.r(), c.g(), c.b());
        let ok = |c: Color32| (luminance(c) - luminance(self.bg_edit)).abs() >= 0.25;
        let mut set = |kind: TokenKind, c: Color32, style: Style| {
            if ok(c) {
                th.styles.insert(kind, TokenStyle::new(conv(c)).with_style(style));
            }
        };
        set(TokenKind::Keyword, self.blue, Style::bold());
        set(TokenKind::String, self.green, Style::default());
        set(TokenKind::Number, self.orange, Style::default());
        set(TokenKind::Comment, self.fg_dim, Style::italic());
        set(TokenKind::Function, self.yellow, Style::default());
        set(TokenKind::Type, self.red, Style::default());
        for k in [TokenKind::Plain, TokenKind::Operator, TokenKind::Punct, TokenKind::Variable] {
            if ok(self.fg) {
                th.styles.insert(k, TokenStyle::new(conv(self.fg)));
            }
        }
        th
    }
}

/// 相对基线 (打开 / 上次保存时的内容) 哪些行被改过 / 新加: 去掉相同的头尾后对中间段做 LCS, 不在 LCS 里的行记为 true。
/// 中间段太大 (积 > 1M) 就整段记为改动, 不为一个小标记拖慢编辑。
pub fn changed_lines(base: &[String], cur: &[String]) -> Vec<bool> {
    let mut out = vec![false; cur.len()];
    let mut head = 0;
    while head < base.len() && head < cur.len() && base[head] == cur[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < base.len() - head && tail < cur.len() - head && base[base.len() - 1 - tail] == cur[cur.len() - 1 - tail] {
        tail += 1;
    }
    let (a, b) = (&base[head..base.len() - tail], &cur[head..cur.len() - tail]);
    if b.is_empty() {
        return out;
    }
    if a.is_empty() || a.len() * b.len() > 1_000_000 {
        out[head..cur.len() - tail].iter_mut().for_each(|x| *x = true);
        return out;
    }
    // dp[i][j] = a[i..] 与 b[j..] 的 LCS 长度
    let w = b.len() + 1;
    let mut dp = vec![0u16; (a.len() + 1) * w];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i * w + j] = if a[i] == b[j] { dp[(i + 1) * w + j + 1].saturating_add(1) } else { dp[(i + 1) * w + j].max(dp[i * w + j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    while j < b.len() {
        if i < a.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if i < a.len() && dp[(i + 1) * w + j] >= dp[i * w + j + 1] {
            i += 1;
        } else {
            out[head + j] = true;
            j += 1;
        }
    }
    out
}

/// 空白符标记: 返回 (字符列, 标记字符)。空格 `·`, Tab `→`, 全角空格 `□`; 行尾的换行 `↵` 由调用方另画
pub fn whitespace_marks(line: &str) -> Vec<(usize, char)> {
    line.chars().enumerate().filter_map(|(i, c)| match c {
        ' ' => Some((i, '·')),
        '\t' => Some((i, '→')),
        '\u{3000}' => Some((i, '□')),
        _ => None,
    }).collect()
}

/// 字符列 -> 显示列 (Tab 按 tab 宽对齐, 全角字符占 2 列), 1 起算 —— 状态栏的「桁」
pub fn display_col(line: &str, col: usize, tab: usize) -> usize {
    let tab = tab.max(1);
    let mut x = 0;
    for c in line.chars().take(col) {
        x += if c == '\t' { tab - x % tab } else { wide(c) };
    }
    x + 1
}

/// 东亚宽字符占 2 列 (够状态栏用的近似: CJK / 假名 / 全角形)
pub fn wide(c: char) -> usize {
    let n = c as u32;
    if (0x1100..=0x115F).contains(&n) || (0x2E80..=0xA4CF).contains(&n) || (0xAC00..=0xD7A3).contains(&n) || (0xF900..=0xFAFF).contains(&n) || (0xFE30..=0xFE6F).contains(&n) || (0xFF00..=0xFF60).contains(&n) || (0xFFE0..=0xFFE6).contains(&n) {
        2
    } else {
        1
    }
}

/// 选区统计: (字符数, 行数); 换行不计入字符数
pub fn sel_stats(lines: &[String], sel: [usize; 4]) -> (usize, usize) {
    let [r0, c0, r1, c1] = sel;
    if r0 == r1 {
        return (c1.saturating_sub(c0), 1);
    }
    let mut n = lines.get(r0).map(|l| l.chars().count().saturating_sub(c0)).unwrap_or(0);
    for r in r0 + 1..r1 {
        n += lines.get(r).map(|l| l.chars().count()).unwrap_or(0);
    }
    (n + c1, r1 - r0 + 1)
}

/// 小地图: 把 `n` 行压到 `h` 个像素行里, 返回每个像素行代表的 (起始行, 结束行)
pub fn minimap_rows(n: usize, h: usize) -> Vec<(usize, usize)> {
    if n == 0 || h == 0 {
        return vec![];
    }
    let rows = h.min(n);
    (0..rows).map(|i| (i * n / rows, ((i + 1) * n / rows).max(i * n / rows + 1).min(n))).collect()
}

/// 统一差异 (`.diff` / `.patch`) 一行的着色: 文件头 / 块头蓝, 增绿, 删红; 其它行 None (用默认色)
pub fn diff_line_color(line: &str, tc: &ThemeColors) -> Option<Color32> {
    if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") || line.starts_with("index ") || line.starts_with("@@") {
        Some(tc.blue)
    } else if line.starts_with('+') {
        Some(tc.green)
    } else if line.starts_with('-') {
        Some(tc.red)
    } else {
        None
    }
}

/// 文件名是不是差异文件 (自己着色, editcore 没有这个语言)
pub fn is_diff_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(".diff") || n.ends_with(".patch")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diff_lines_get_colours_by_their_first_characters() {
        let tc = ThemeColors::builtin(true);
        assert_eq!(diff_line_color("+added", &tc), Some(tc.green));
        assert_eq!(diff_line_color("-gone", &tc), Some(tc.red));
        assert_eq!(diff_line_color("@@ -1 +1 @@", &tc), Some(tc.blue));
        assert_eq!(diff_line_color("+++ b/x", &tc), Some(tc.blue), "文件头不当成增行");
        assert_eq!(diff_line_color("--- a/x", &tc), Some(tc.blue));
        assert_eq!(diff_line_color(" context", &tc), None);
        assert!(is_diff_name("Diff: a.rs.diff") && is_diff_name("x.PATCH") && !is_diff_name("diff.rs"));
    }


    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect()
    }

    #[test]
    fn unchanged_text_has_no_marks() {
        assert_eq!(changed_lines(&v(&["a", "b"]), &v(&["a", "b"])), vec![false, false]);
    }

    #[test]
    fn edits_inserts_and_far_apart_changes_mark_only_their_lines() {
        let base = v(&["a", "b", "c", "d", "e"]);
        assert_eq!(changed_lines(&base, &v(&["a", "B", "c", "d", "e"])), vec![false, true, false, false, false]);
        assert_eq!(changed_lines(&base, &v(&["a", "b", "x", "c", "d", "e"])), vec![false, false, true, false, false, false]);
        // 两处相隔很远的改动: 中间没动的行不被连带标记
        assert_eq!(changed_lines(&base, &v(&["A", "b", "c", "d", "E"])), vec![true, false, false, false, true]);
        // 删行: 剩下的行都没变
        assert_eq!(changed_lines(&base, &v(&["a", "c", "e"])), vec![false, false, false]);
    }

    #[test]
    fn everything_is_new_against_an_empty_baseline() {
        assert_eq!(changed_lines(&v(&[""]), &v(&["x", "y"])), vec![true, true]);
        assert_eq!(changed_lines(&[], &v(&["x"])), vec![true]);
    }

    #[test]
    fn whitespace_marks_cover_space_tab_and_ideographic_space() {
        assert_eq!(whitespace_marks("a b\t\u{3000}"), vec![(1, '·'), (3, '→'), (4, '□')]);
        assert!(whitespace_marks("abc").is_empty());
    }

    #[test]
    fn display_column_counts_tabs_and_wide_chars() {
        assert_eq!(display_col("abc", 2, 4), 3);
        assert_eq!(display_col("\tx", 1, 4), 5, "Tab 对齐到下一个 tab 位");
        assert_eq!(display_col("a\tx", 2, 4), 5);
        assert_eq!(display_col("日本語x", 3, 4), 7, "全角字符各占两列");
    }

    #[test]
    fn selection_stats_count_chars_and_lines() {
        let l = v(&["abcd", "ef", "ghij"]);
        assert_eq!(sel_stats(&l, [0, 1, 0, 3]), (2, 1));
        assert_eq!(sel_stats(&l, [0, 2, 2, 1]), (2 + 2 + 1, 3));
    }

    #[test]
    fn minimap_rows_cover_all_lines_exactly_once() {
        let r = minimap_rows(1000, 100);
        assert_eq!((r.len(), r[0], r[99]), (100, (0, 10), (990, 1000)));
        assert_eq!(minimap_rows(3, 100), vec![(0, 1), (1, 2), (2, 3)], "行比像素少: 一行一格");
        assert!(minimap_rows(0, 10).is_empty());
    }

    #[test]
    fn theme_vars_drive_colors_and_fall_back_when_unparsable() {
        let t = ThemeColors::from_vars(&vars(&[("--bg-edit", "#ffffff"), ("--fg", "#101010"), ("--blue", "#0000ff")]));
        assert!(!t.dark);
        assert_eq!((t.bg_edit, t.fg, t.blue), (Color32::WHITE, Color32::from_rgb(16, 16, 16), Color32::from_rgb(0, 0, 255)));
        // 透明背景 (crystal) 不是「亮」: 当深色基底处理, 半透明的状态栏铺到基底上
        let c = ThemeColors::from_vars(&vars(&[("--bg-edit", "transparent"), ("--bg-status", "rgba(255,255,255,0.12)")]));
        assert!(c.dark);
        assert!(c.bg_status.r() > c.bg_edit.r() && c.bg_status.a() == 255);
    }

    #[test]
    fn syntax_theme_takes_theme_colors_but_keeps_readable_defaults() {
        let t = ThemeColors::from_vars(&vars(&[("--bg-edit", "#000000"), ("--fg", "#ffffff"), ("--blue", "#00f0ff"), ("--green", "#7ee2b8")]));
        let th = t.syntax_theme();
        assert_eq!(th.get(TokenKind::Keyword).fg, Color(0, 240, 255));
        assert_eq!(th.get(TokenKind::String).fg, Color(126, 226, 184));
        // 浅色背景 + 亮黄函数色: 对比太弱, 保留内置浅色主题的函数色
        let l = ThemeColors::from_vars(&vars(&[("--bg-edit", "#ffffff"), ("--yellow", "#ffff00")]));
        assert_eq!(l.syntax_theme().get(TokenKind::Function).fg, Theme::light().get(TokenKind::Function).fg);
    }

    #[test]
    fn all_six_bundled_themes_give_a_distinct_readable_palette() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide-fast-view-{}", std::process::id()));
        std::env::set_var("IDE_PLUGIN_DIR", &dir);
        let rows = crate::plugins::rows();
        let mut seen = std::collections::HashSet::new();
        for id in ["theme-dark", "theme-light", "theme-hc", "theme-clear", "theme-crystal", "theme-retro16"] {
            let spec = rows.iter().find(|r| r.id == id).and_then(|r| r.theme.clone()).unwrap_or_else(|| panic!("缺内置主题 {id}"));
            let t = ThemeColors::from_vars(&spec.vars);
            let th = t.syntax_theme();
            let kw = th.get(TokenKind::Keyword).fg;
            assert!(luminance(Color32::from_rgb(kw.0, kw.1, kw.2)) != luminance(t.bg_edit), "{id} 关键字色和背景相同");
            seen.insert((t.bg_edit, t.blue));
        }
        assert!(seen.len() >= 5, "六套主题的配色不该几乎一样");
        std::env::remove_var("IDE_PLUGIN_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
