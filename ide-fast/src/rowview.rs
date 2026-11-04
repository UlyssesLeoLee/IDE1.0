//! 一行文字的「显示视图」: 编辑区每帧对每个可见行做的排版入口。
//! 普通行 = 整行 (Tab 展开 + 折行点插入换行); 超长行 (>= `LONG` 字符, 不折行, 没有 Tab) 只取水平方向看得见的那一段排版、绘制,
//! 其余部分按「等宽格子」(字符下标 x 字符格宽) 估位置 —— 100 万字符的单行每帧也只排版 ~1000 个字符。
//! 窗口内位置 = 排版实测 (和普通行一样准), 窗口外只用来滚动 / 光标可见性判断, 窗口一动 (滚动) 就会重新取准。
//! 折行开着时不切片 (折行本来就按整行排版, 而且换行后没有水平滚动, 见 PERF.md 的限制说明)。

use crate::kinsoku::{expand_wrap, Wrap};
use eframe::egui::{self, Color32, FontId, Vec2};

/// 超过这么多字符的行才切片 (再短的行整行排版很便宜)
pub const LONG: usize = 4096;
/// 窗口两侧多取的字符数 (滚动时不露白)
const MARGIN: usize = 256;
/// 窗口起点取整的粒度: 起点稳定, 慢速滚动时同一窗口复用同一份排版缓存
const SNAP: usize = 128;

pub struct RowView {
    /// 显示文字 (整行, 或长行里的窗口切片)
    pub disp: String,
    /// 字符列 (相对 `col0`) -> `disp` 的字符下标; 切片时是恒等映射, 不存 (空)
    pub map: Vec<usize>,
    /// `disp` 第一个字符对应的行内字符列
    pub col0: usize,
    /// 整行的显示字符数 (撑宽滚动区用)
    pub ncols: usize,
    /// `disp` 的字符数
    pub len: usize,
    /// 是不是长行切片
    pub sliced: bool,
    /// 字符格宽 (窗口外估位置用)
    pub cw: f32,
}

/// 视口 x 范围 (行内坐标) -> 要排版的字符列区间 [c0, c1)
pub fn window(nchars: usize, vx0: f32, vx1: f32, cw: f32) -> (usize, usize) {
    let cw = cw.max(1.0);
    let lo = (vx0.max(0.0) / cw) as usize;
    let hi = (vx1.max(0.0) / cw * 1.3) as usize + 1; // 窄字符比格宽窄: 多取 30%
    let c0 = lo.saturating_sub(MARGIN) / SNAP * SNAP;
    let c1 = (hi + MARGIN).div_ceil(SNAP) * SNAP;
    (c0.min(nchars), c1.min(nchars))
}

/// 第 `c` 个字符的字节偏移 (ASCII 行直接是 c)
fn byte_of(line: &str, c: usize, ascii: bool) -> usize {
    if ascii {
        c.min(line.len())
    } else {
        line.char_indices()
            .nth(c)
            .map(|(b, _)| b)
            .unwrap_or(line.len())
    }
}

impl RowView {
    /// `view_x`: 不折行时视口的行内 x 范围 (折行时传 None)。长行 + 没 Tab 才切片, 其余走原来的整行路径
    pub fn build(
        line: &str,
        tab: usize,
        kcols: Wrap,
        view_x: Option<(f32, f32)>,
        cw: f32,
    ) -> RowView {
        if let (Some((vx0, vx1)), true) = (view_x, kcols.cols == 0 && line.len() >= LONG) {
            let ascii = line.is_ascii();
            let nchars = if ascii {
                line.len()
            } else {
                line.chars().count()
            };
            if nchars >= LONG && !line.contains('\t') {
                let (c0, c1) = window(nchars, vx0, vx1, cw);
                let (b0, b1) = (byte_of(line, c0, ascii), byte_of(line, c1, ascii));
                return RowView {
                    disp: line[b0..b1].to_string(),
                    map: vec![],
                    col0: c0,
                    ncols: nchars,
                    len: c1 - c0,
                    sliced: true,
                    cw,
                };
            }
        }
        let (disp, map) = expand_wrap(line, tab, kcols);
        let len = disp.chars().count();
        RowView {
            disp,
            map,
            col0: 0,
            ncols: len,
            len,
            sliced: false,
            cw,
        }
    }

    /// 行内字符列 -> `disp` 里的字符下标 (夹到窗口内)
    pub fn idx(&self, col: usize) -> usize {
        if self.sliced {
            col.clamp(self.col0, self.col0 + self.len) - self.col0
        } else {
            self.map
                .get(col)
                .copied()
                .unwrap_or_else(|| *self.map.last().unwrap_or(&0))
        }
    }

    /// 行内最后一个字符列 (含行尾位置)
    pub fn tail(&self) -> usize {
        if self.sliced {
            self.ncols
        } else {
            self.map.len().saturating_sub(1)
        }
    }

    /// 列区间 [c0, c1) 和窗口有没有交集 (整行永远有)
    pub fn touches(&self, c0: usize, c1: usize) -> bool {
        !self.sliced || (c1 >= self.col0 && c0 <= self.col0 + self.len)
    }

    /// 窗口起点的 x (整行 = 0)
    pub fn x_off(&self) -> f32 {
        self.col0 as f32 * self.cw
    }

    fn width_of(painter: &egui::Painter, text: String, font: &FontId, color: Color32) -> f32 {
        if text.is_empty() {
            0.0
        } else {
            painter.layout_no_wrap(text, font.clone(), color).size().x
        }
    }

    /// 第 `col` 列前缘的 x (相对行首): 窗口内按真实排版量, 窗口外按等宽格子估
    pub fn x_of(&self, painter: &egui::Painter, col: usize, font: &FontId, color: Color32) -> f32 {
        if !self.sliced {
            let di = self.idx(col);
            if di == 0 {
                return 0.0;
            }
            return Self::width_of(painter, self.disp.chars().take(di).collect(), font, color);
        }
        if col <= self.col0 {
            return col as f32 * self.cw;
        }
        let local = (col - self.col0).min(self.len);
        let w = Self::width_of(
            painter,
            self.disp.chars().take(local).collect(),
            font,
            color,
        );
        self.x_off() + w + (col - self.col0 - local) as f32 * self.cw
    }

    /// x (相对行首) 处最近的字符列; `y` = 折行时行内的纵向位置
    pub fn col_at(&self, ctx: &egui::Context, x: f32, y: f32, font: &FontId, wrap_w: f32) -> usize {
        if !self.sliced {
            return crate::app::col_at(ctx, &self.disp, &self.map, x, y, font, wrap_w);
        }
        let xo = self.x_off();
        if x < xo {
            return ((x / self.cw).round() as usize).min(self.col0);
        }
        let galley = ctx.fonts_mut(|f| {
            f.layout_job(egui::text::LayoutJob::simple_singleline(
                self.disp.clone(),
                font.clone(),
                Color32::WHITE,
            ))
        });
        let idx: usize = galley.cursor_from_pos(Vec2::new(x - xo, 0.0)).index.into();
        if idx >= self.len && x - xo > galley.size().x {
            return (self.col0
                + self.len
                + ((x - xo - galley.size().x) / self.cw).round() as usize)
                .min(self.ncols);
        }
        self.col0 + idx.min(self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> FontId {
        FontId::monospace(14.0)
    }

    /// 在一个无头帧里拿到 painter 跑断言
    fn with_painter(f: impl FnOnce(&egui::Context, &egui::Painter)) {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            if let Some(f) = f.take() {
                f(ui.ctx(), ui.painter());
            }
        });
        out.textures_delta.clear();
    }

    #[test]
    fn window_is_snapped_clamped_and_covers_the_view() {
        let (a, b) = window(1_000_000, 90_000.0, 91_000.0, 9.0);
        assert_eq!(a % SNAP, 0);
        assert!(
            a <= 10_000 && b >= 10_111 + 140,
            "窗口要盖住 [{}, {}] 列",
            10_000,
            10_111
        );
        assert!(b - a < 4000, "窗口只有几千字符, 不是整行");
        assert_eq!(window(300, 0.0, 800.0, 9.0), (0, 300), "夹到行长");
        assert_eq!(window(1_000_000, -5.0, 0.0, 9.0).0, 0, "负的视口起点当 0");
    }

    #[test]
    fn short_lines_and_lines_with_tabs_stay_whole() {
        let v = RowView::build("short line", 4, Wrap::default(), Some((0.0, 800.0)), 9.0);
        assert!(!v.sliced && v.ncols == 10 && v.col0 == 0);
        let tabbed = format!("\t{}", "x".repeat(LONG + 10));
        let v = RowView::build(&tabbed, 4, Wrap::default(), Some((0.0, 800.0)), 9.0);
        assert!(!v.sliced, "含 Tab 的长行走整行路径 (列宽依赖前缀)");
        assert_eq!(v.ncols, tabbed.chars().count() + 3);
        let long = "a".repeat(LONG + 10);
        assert!(
            !RowView::build(&long, 4, Wrap::default(), None, 9.0).sliced,
            "折行模式不切片"
        );
        assert!(
            !RowView::build(
                &long,
                4,
                Wrap {
                    cols: 80,
                    hang: false
                },
                Some((0.0, 800.0)),
                9.0
            )
            .sliced,
            "固定列折行不切片"
        );
    }

    #[test]
    fn long_line_slice_is_a_small_window_with_the_right_text() {
        let line: String = (0..200_000)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let v = RowView::build(&line, 4, Wrap::default(), Some((90_000.0, 91_000.0)), 9.0);
        assert!(v.sliced && v.ncols == 200_000);
        assert!(v.len < 4000 && v.disp.chars().count() == v.len);
        let expect: String = line.chars().skip(v.col0).take(v.len).collect();
        assert_eq!(v.disp, expect, "切片就是原行的这一段");
        assert_eq!(v.idx(v.col0 + 5), 5);
        assert_eq!(v.idx(0), 0, "窗口前的列夹到窗口头");
        assert_eq!(v.idx(usize::MAX / 2), v.len, "窗口后的列夹到窗口尾");
        assert!(
            v.touches(v.col0 + 3, v.col0 + 9)
                && !v.touches(0, 10)
                && !v.touches(v.col0 + v.len + 5, v.col0 + v.len + 9)
        );
    }

    #[test]
    fn multibyte_long_line_slices_on_char_boundaries() {
        let line: String = "あいうえお".repeat(2000); // 10000 字符, 30000 字节
        let v = RowView::build(&line, 4, Wrap::default(), Some((5000.0, 5400.0)), 14.0);
        assert!(v.sliced && v.ncols == 10_000);
        let expect: String = line.chars().skip(v.col0).take(v.len).collect();
        assert_eq!(v.disp, expect);
    }

    #[test]
    fn x_positions_are_exact_inside_the_window_and_gridded_outside() {
        with_painter(|ctx, p| {
            let line = "a".repeat(100_000);
            let cw = ctx.fonts_mut(|f| f.glyph_width(&font(), '0'));
            let v = RowView::build(&line, 4, Wrap::default(), Some((50_000.0, 50_800.0)), cw);
            let c = Color32::WHITE;
            let whole = RowView::build(&"a".repeat(200), 4, Wrap::default(), None, cw);
            let unit = whole.x_of(p, 100, &font(), c) / 100.0;
            let col = v.col0 + 40;
            let want = v.x_off() + unit * 40.0;
            assert!(
                (v.x_of(p, col, &font(), c) - want).abs() < 0.5,
                "窗口内 = 偏移 + 实测宽"
            );
            assert_eq!(v.x_of(p, 0, &font(), c), 0.0);
            assert_eq!(v.x_of(p, 100, &font(), c), 100.0 * cw, "窗口之前按格子估");
            let past = v.col0 + v.len + 10;
            assert!(
                (v.x_of(p, past, &font(), c) - (v.x_off() + unit * v.len as f32 + 10.0 * cw)).abs()
                    < 1.0
            );
        });
    }

    #[test]
    fn click_x_maps_back_to_the_column_inside_the_window() {
        with_painter(|ctx, p| {
            let line = "a".repeat(100_000);
            let cw = ctx.fonts_mut(|f| f.glyph_width(&font(), '0'));
            let v = RowView::build(&line, 4, Wrap::default(), Some((50_000.0, 50_800.0)), cw);
            let col = v.col0 + 123;
            let x = v.x_of(p, col, &font(), Color32::WHITE);
            assert_eq!(v.col_at(ctx, x + 0.2, 0.0, &font(), f32::INFINITY), col);
            assert_eq!(
                v.col_at(ctx, 0.0, 0.0, &font(), f32::INFINITY),
                0,
                "点在窗口前: 按格子取列"
            );
            let far = v.col_at(ctx, v.x_off() + 1e6, 0.0, &font(), f32::INFINITY);
            assert!(
                far > v.col0 + v.len && far <= v.ncols,
                "点在窗口后: 估列并夹到行长"
            );
        });
    }
}
