//! 矢量图标: 把 Tauri 版 `editor.html` 里的内联 SVG (`viewBox 0 0 24 24`, `stroke-width 1.8`, 圆角端点) 用 egui 画笔原样画出来。
//! 只实现图标用到的路径子集 (M L H V A Z 及其小写相对形式); 圆 / 圆角矩形在表里直接写成弧线路径。
//! 解析和展平是纯函数 (有单测), 绘制只把折线交给 `Painter`。

use eframe::egui::{epaint::PathStroke, pos2, Color32, Painter, Pos2, Rect, Shape, Vec2};

/// 一条路径: SVG `d` 属性 + 是否填充 (填充的小圆点)
pub type PathDef = (&'static str, bool);

/// 活动栏图标 (来自 editor.html `#activity-bar`)
pub const EXPLORER: &[PathDef] = &[("M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", false), ("M3 11H21", false)];
pub const SEARCH: &[PathDef] = &[("M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0z", false), ("M21 21L16.65 16.65", false)];
pub const SCM: &[PathDef] = &[
    ("M15 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0z", false),
    ("M3 6a3 3 0 1 0 6 0a3 3 0 1 0-6 0z", false),
    ("M3 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0z", false),
    ("M18 15V9a3 3 0 0 0-3-3H9", false),
    ("M6 9V15", false),
];
pub const OUTLINE: &[PathDef] = &[
    ("M5 6H19", false), ("M9 12H19", false), ("M9 18H19", false),
    ("M3.5 12a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z", true),
    ("M3.5 18a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z", true),
];
pub const EXTENSIONS: &[PathDef] = &[
    ("M4 3h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z", false),
    ("M15 3h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z", false),
    ("M15 14h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1v-5a1 1 0 0 1 1-1z", false),
    ("M3 14h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H3z", false),
];

/// Markdown 工具条图标 (Tauri 用的彩色 emoji 的矢量替身, 同一个 24x24 / 描边 1.8 风格, 颜色跟主题文字色走)
pub const LINK: &[PathDef] = &[
    ("M10 13a5 5 0 0 0 7.54 0.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71", false),
    ("M14 11a5 5 0 0 0-7.54-0.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71", false),
];
pub const IMAGE: &[PathDef] = &[
    ("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z", false),
    ("M7.5 8.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z", true),
    ("M21 15l-5-5L5 21", false),
];
pub const TASK: &[PathDef] = &[
    ("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z", false),
    ("M8 12l3 3l5-6", false),
];
pub const QUOTE: &[PathDef] = &[
    ("M5 4V20", false), ("M10 8H20", false), ("M10 12H20", false), ("M10 16H16", false),
];
pub const CALLOUT: &[PathDef] = &[
    ("M9 17h6", false), ("M10 21h4", false),
    ("M9 17v-1.5a6 6 0 1 1 6 0V17", false),
];
pub const MERMAID: &[PathDef] = &[("M12 3L21 12L12 21L3 12z", false)];
pub const EXPORT: &[PathDef] = &[("M12 15V4", false), ("M8 8l4-4l4 4", false), ("M5 14v6h14v-6", false)];
pub const LINT: &[PathDef] = &[("M4 12.5l5.5 5.5L20 7", false)];

/// 展平后的一段折线
#[derive(Clone, Debug, PartialEq)]
pub struct Sub {
    pub pts: Vec<Pos2>,
    pub closed: bool,
}

/// 读下一个数 (跳过空白和逗号; `1.5-3` 里的 `-` 开始下一个数)
fn number(b: &[u8], i: &mut usize) -> Option<f32> {
    while *i < b.len() && (b[*i] == b' ' || b[*i] == b',') { *i += 1; }
    let st = *i;
    if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') { *i += 1; }
    let mut dot = false;
    while *i < b.len() && (b[*i].is_ascii_digit() || (b[*i] == b'.' && !dot)) {
        dot |= b[*i] == b'.';
        *i += 1;
    }
    std::str::from_utf8(&b[st..*i]).ok()?.parse().ok()
}

/// 圆弧 (端点式) 展平成折线点, 追加到 `out` (不含起点, 含终点)
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<Pos2>, from: Pos2, rx: f32, ry: f32, phi_deg: f32, large: bool, sweep: bool, to: Pos2) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || from == to {
        out.push(to);
        return;
    }
    let (s, c) = phi_deg.to_radians().sin_cos();
    let (dx, dy) = ((from.x - to.x) / 2.0, (from.y - to.y) / 2.0);
    let (x1p, y1p) = (c * dx + s * dy, -s * dx + c * dy);
    let lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if lambda > 1.0 { let k = lambda.sqrt(); rx *= k; ry *= k; }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let coef = (if large == sweep { -1.0 } else { 1.0 }) * (num / den).max(0.0).sqrt();
    let (cxp, cyp) = (coef * rx * y1p / ry, -coef * ry * x1p / rx);
    let (cx, cy) = (c * cxp - s * cyp + (from.x + to.x) / 2.0, s * cxp + c * cyp + (from.y + to.y) / 2.0);
    let u = ((x1p - cxp) / rx, (y1p - cyp) / ry);
    let v = ((-x1p - cxp) / rx, (-y1p - cyp) / ry);
    let t1 = u.1.atan2(u.0);
    let mut dt = (u.0 * v.1 - u.1 * v.0).atan2(u.0 * v.0 + u.1 * v.1);
    if !sweep && dt > 0.0 { dt -= std::f32::consts::TAU; }
    if sweep && dt < 0.0 { dt += std::f32::consts::TAU; }
    let n = ((dt.abs() / (std::f32::consts::PI / 14.0)).ceil() as usize).max(2);
    for k in 1..n {
        let t = t1 + dt * k as f32 / n as f32;
        let (st, ct) = t.sin_cos();
        out.push(pos2(c * rx * ct - s * ry * st + cx, s * rx * ct + c * ry * st + cy));
    }
    out.push(to);
}

/// 解析并展平 SVG 路径; 遇到不认识的命令就停 (图标表里不会有)
pub fn flatten(d: &str) -> Vec<Sub> {
    let b = d.as_bytes();
    let (mut i, mut cmd) = (0, b'M');
    let (mut cur, mut start) = (Pos2::ZERO, Pos2::ZERO);
    let mut subs: Vec<Sub> = vec![];
    loop {
        while i < b.len() && (b[i] == b' ' || b[i] == b',') { i += 1; }
        if i >= b.len() { break; }
        if b[i].is_ascii_alphabetic() { cmd = b[i]; i += 1; }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cur.x, cur.y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            b'Z' => {
                if let Some(s) = subs.last_mut() { s.closed = true; }
                cur = start;
                cmd = b'M';
                continue;
            }
            b'M' | b'L' => {
                let (Some(x), Some(y)) = (number(b, &mut i), number(b, &mut i)) else { break };
                cur = pos2(ox + x, oy + y);
                if cmd.to_ascii_uppercase() == b'M' {
                    start = cur;
                    subs.push(Sub { pts: vec![cur], closed: false });
                    cmd = if rel { b'l' } else { b'L' }; // M 后面多余的坐标对是 L
                } else if let Some(s) = subs.last_mut() { s.pts.push(cur); }
            }
            b'H' => { let Some(x) = number(b, &mut i) else { break }; cur.x = ox + x; if let Some(s) = subs.last_mut() { s.pts.push(cur); } }
            b'V' => { let Some(y) = number(b, &mut i) else { break }; cur.y = oy + y; if let Some(s) = subs.last_mut() { s.pts.push(cur); } }
            b'A' => {
                let (Some(rx), Some(ry), Some(rot), Some(fa), Some(fs), Some(x), Some(y)) = (number(b, &mut i), number(b, &mut i), number(b, &mut i), number(b, &mut i), number(b, &mut i), number(b, &mut i), number(b, &mut i)) else { break };
                let to = pos2(ox + x, oy + y);
                if let Some(s) = subs.last_mut() { arc(&mut s.pts, cur, rx, ry, rot, fa != 0.0, fs != 0.0, to); }
                cur = to;
            }
            _ => break,
        }
    }
    // 闭合路径的终点与起点重合时去掉重复点 (圆: 第二段弧回到起点)
    for s in &mut subs {
        if s.closed && s.pts.len() > 1 && s.pts.first() == s.pts.last() { s.pts.pop(); }
    }
    subs
}

/// 在 `center` 为中心、边长 `size` 的方块里画图标 (坐标按 24x24 缩放); `width` 是 SVG 坐标系里的线宽 (Tauri 为 1.8)
pub fn paint(p: &Painter, center: Pos2, size: f32, paths: &[PathDef], color: Color32, width: f32) {
    let k = size / 24.0;
    let o = center - Vec2::splat(size / 2.0);
    let map = |q: Pos2| pos2(o.x + q.x * k, o.y + q.y * k);
    let stroke = PathStroke::new(width * k, color);
    for (d, fill) in paths {
        for sub in flatten(d) {
            let pts: Vec<Pos2> = sub.pts.iter().map(|&q| map(q)).collect();
            if *fill {
                p.add(Shape::convex_polygon(pts, color, eframe::egui::Stroke::NONE));
            } else if sub.closed {
                p.add(Shape::closed_line(pts, stroke.clone()));
            } else {
                p.add(Shape::line(pts, stroke.clone()));
            }
        }
    }
}

/// 图标占据的外接矩形 (测试用)
pub fn bounds(paths: &[PathDef]) -> Rect {
    let mut r = Rect::NOTHING;
    for (d, _) in paths { for s in flatten(d) { for q in s.pts { r.extend_with(q); } } }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_relative_moves_flatten_to_points() {
        let s = flatten("M3 11H21");
        assert_eq!(s, vec![Sub { pts: vec![pos2(3.0, 11.0), pos2(21.0, 11.0)], closed: false }]);
        let s = flatten("M1 1l2 0v3z");
        assert_eq!(s[0].pts, vec![pos2(1.0, 1.0), pos2(3.0, 1.0), pos2(3.0, 4.0)]);
        assert!(s[0].closed);
    }

    #[test]
    fn numbers_may_be_glued_by_minus_signs() {
        let s = flatten("M0 0l1.5-3");
        assert_eq!(s[0].pts[1], pos2(1.5, -3.0));
    }

    #[test]
    fn a_circle_path_stays_on_its_circle() {
        let s = flatten("M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0z");
        assert!(s[0].closed && s[0].pts.len() > 12);
        for q in &s[0].pts {
            let d = ((q.x - 11.0).powi(2) + (q.y - 11.0).powi(2)).sqrt();
            assert!((d - 7.0).abs() < 0.05, "{q:?} 离圆心 {d}");
        }
    }

    #[test]
    fn a_rounded_corner_arc_bulges_the_right_way() {
        // 左上角圆角: 从 (3,7) 到 (5,5), 圆心 (5,7); 中间点应在圆心的左上方
        let s = flatten("M3 7a2 2 0 0 1 2-2");
        let mid = s[0].pts[s[0].pts.len() / 2];
        assert!(mid.x < 4.0 && mid.y < 6.0 + 0.01 && (mid.x - 5.0).hypot(mid.y - 7.0) - 2.0 < 0.05, "{mid:?}");
    }

    #[test]
    fn every_activity_icon_fits_the_24_grid() {
        for icon in [EXPLORER, SEARCH, SCM, OUTLINE, EXTENSIONS] {
            let b = bounds(icon);
            assert!(b.min.x >= 2.5 && b.min.y >= 2.5 && b.max.x <= 21.5 && b.max.y <= 21.5, "{b:?}");
            assert!(b.width() > 10.0 && b.height() > 10.0, "{b:?}");
        }
    }

    #[test]
    fn markdown_bar_icons_fit_the_grid_and_are_not_empty() {
        for (n, icon) in [("link", LINK), ("image", IMAGE), ("task", TASK), ("quote", QUOTE), ("callout", CALLOUT), ("mermaid", MERMAID), ("export", EXPORT), ("lint", LINT)] {
            let b = bounds(icon);
            assert!(b.min.x >= 1.5 && b.min.y >= 1.5 && b.max.x <= 22.5 && b.max.y <= 22.5, "{n}: {b:?}");
            assert!(b.width() > 8.0 && b.height() > 8.0, "{n}: {b:?}");
        }
    }
}
