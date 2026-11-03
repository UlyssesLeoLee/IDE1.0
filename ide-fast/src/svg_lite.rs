//! 够用的 SVG 子集绘制: 插件结果里的 SVG (文档关系图这类 path / circle / rect / line / text 拼出来的图) 用 egui 画笔直接画,
//! 不引入 resvg (省 1-2 MB)。`<g data-line="N">` 分组可点击 = 跳到文档第 N 行 (对应 Tauri 版点节点跳行)。
//! 安全: 只认下面这些图元, `<script>` / `<foreignObject>` / 外部引用一律忽略 (等价于 Tauri 版的白名单净化)。
//! 不支持 (如 mermaid 的复杂图) 的图元被跳过, 整张图一个都画不出来时调用方改显示数据文本。

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line { pts: Vec<[f32; 2]>, color: Color32, w: f32 },
    Circle { c: [f32; 2], r: f32, fill: Option<Color32> },
    Rect { r: [f32; 4], fill: Option<Color32>, stroke: Option<Color32> },
    Text { p: [f32; 2], s: String, size: f32, color: Color32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub shape: Shape,
    /// 所属 `data-line` 分组 (文档行号, 1 起)
    pub line: Option<usize>,
    pub tip: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Svg {
    pub w: f32,
    pub h: f32,
    pub items: Vec<Item>,
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    while let Some(i) = rest.find(name) {
        let before_ok = i == 0 || rest.as_bytes()[i - 1].is_ascii_whitespace();
        let after = &rest[i + name.len()..];
        if before_ok && after.starts_with('=') {
            let q = after.as_bytes().get(1).copied()?;
            if q == b'"' || q == b'\'' {
                let body = &after[2..];
                let end = body.find(q as char)?;
                return Some(unescape(&body[..end]));
            }
        }
        rest = &rest[i + name.len()..];
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

fn num(tag: &str, name: &str) -> f32 {
    attr(tag, name).and_then(|v| v.trim_end_matches("px").parse().ok()).unwrap_or(0.0)
}

fn color(tag: &str, name: &str) -> Option<Color32> {
    let v = attr(tag, name)?;
    if v == "none" {
        return None;
    }
    Some(crate::plugins::parse_color(&v).unwrap_or(Color32::GRAY))
}

/// path 的 d 属性 -> 折线 (曲线按 16 段采样)。支持 M L H V C Z 及其小写 (相对) 形式
fn path_points(d: &str) -> Vec<Vec<[f32; 2]>> {
    let mut toks: Vec<String> = vec![];
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !cur.is_empty() { toks.push(std::mem::take(&mut cur)); }
            toks.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !cur.is_empty() { toks.push(std::mem::take(&mut cur)); }
        } else if ch == '-' && !cur.is_empty() && !cur.ends_with('e') {
            toks.push(std::mem::take(&mut cur));
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() { toks.push(cur); }
    let mut subs: Vec<Vec<[f32; 2]>> = vec![];
    let (mut x, mut y, mut sx, mut sy) = (0f32, 0f32, 0f32, 0f32);
    let mut i = 0;
    let mut cmd = 'M';
    let mut pts: Vec<[f32; 2]> = vec![];
    let n = |i: &mut usize| -> Option<f32> { let v = toks.get(*i)?.parse().ok(); *i += 1; v };
    while i < toks.len() {
        if toks[i].chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            cmd = toks[i].chars().next().unwrap();
            i += 1;
            if cmd == 'Z' || cmd == 'z' {
                if !pts.is_empty() { pts.push([sx, sy]); }
                x = sx;
                y = sy;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(a), Some(b)) = (n(&mut i), n(&mut i)) else { break };
                if pts.len() > 1 { subs.push(std::mem::take(&mut pts)); } else { pts.clear(); }
                x = ox + a; y = oy + b; sx = x; sy = y;
                pts.push([x, y]);
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => { let (Some(a), Some(b)) = (n(&mut i), n(&mut i)) else { break }; x = ox + a; y = oy + b; pts.push([x, y]); }
            'H' => { let Some(a) = n(&mut i) else { break }; x = ox + a; pts.push([x, y]); }
            'V' => { let Some(a) = n(&mut i) else { break }; y = oy + a; pts.push([x, y]); }
            'C' => {
                let mut v = [0f32; 6];
                for s in &mut v { match n(&mut i) { Some(a) => *s = a, None => return subs } }
                let p0 = [x, y];
                let p1 = [ox + v[0], oy + v[1]];
                let p2 = [ox + v[2], oy + v[3]];
                let p3 = [ox + v[4], oy + v[5]];
                for k in 1..=16 {
                    let t = k as f32 / 16.0;
                    let u = 1.0 - t;
                    let f = |a: f32, b: f32, c: f32, d: f32| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
                    pts.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
                }
                x = p3[0]; y = p3[1];
            }
            _ => break, // 圆弧等不支持: 放弃这条 path 的后半
        }
    }
    if pts.len() > 1 { subs.push(pts); }
    subs
}

/// 解析 SVG 文本。不是 `<svg` 开头 / 一个图元都没认出来 = Err (调用方退回显示文本)
pub fn parse(svg: &str) -> Result<Svg, String> {
    let mut out = Svg::default();
    // g 栈: (是否带 data-line 的行号) —— 子元素继承最近的一个
    let mut groups: Vec<Option<usize>> = vec![];
    let mut skip_depth = 0usize; // <script> / <foreignObject> 里面的全部忽略
    let mut rest = svg;
    let mut seen_svg = false;
    // 正在收集文本的 <text> / 挂 tooltip 的 <title>
    let mut text_ix: Option<usize> = None;
    let mut title_for: Option<usize> = None;
    let mut in_title = false;
    while let Some(lt) = rest.find('<') {
        let text_before = &rest[..lt];
        rest = &rest[lt..];
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        if let Some(ix) = text_ix {
            if let Shape::Text { s, .. } = &mut out.items[ix].shape { s.push_str(&unescape(text_before)); }
        } else if in_title {
            if let Some(ix) = title_for { out.items[ix].tip = Some(unescape(text_before)); }
        }
        let closing = tag.starts_with('/');
        let body = tag.trim_start_matches('/');
        let name_end = body.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(body.len());
        let name = &body[..name_end];
        let selfclose = tag.ends_with('/');
        if closing {
            let n = name;
            match n {
                "script" | "foreignObject" => skip_depth = skip_depth.saturating_sub(1),
                "g" => { groups.pop(); }
                "text" => text_ix = None,
                "title" => in_title = false,
                _ => {}
            }
            continue;
        }
        if name.starts_with('!') || name.starts_with('?') { continue; }
        if matches!(name, "script" | "foreignObject") {
            if !selfclose { skip_depth += 1; }
            continue;
        }
        if skip_depth > 0 { continue; }
        let line = groups.iter().rev().find_map(|g| *g);
        let push = |shape: Shape, items: &mut Vec<Item>| -> usize { items.push(Item { shape, line, tip: None }); items.len() - 1 };
        match name {
            "svg" => {
                seen_svg = true;
                if let Some(vb) = attr(tag, "viewBox") {
                    let v: Vec<f32> = vb.split([' ', ',']).filter_map(|s| s.parse().ok()).collect();
                    if v.len() == 4 { out.w = v[2]; out.h = v[3]; }
                }
                if out.w == 0.0 { out.w = num(tag, "width"); out.h = num(tag, "height"); }
            }
            "g" => {
                let l = attr(tag, "data-line").and_then(|v| v.parse().ok());
                if !selfclose { groups.push(l); }
            }
            "path" => {
                let c = color(tag, "stroke").unwrap_or(Color32::GRAY);
                let w = attr(tag, "stroke-width").and_then(|v| v.parse().ok()).unwrap_or(1.0);
                let mut first = None;
                for pts in path_points(&attr(tag, "d").unwrap_or_default()) {
                    let ix = push(Shape::Line { pts, color: c, w }, &mut out.items);
                    first.get_or_insert(ix);
                }
                title_for = first;
            }
            "line" => {
                let pts = vec![[num(tag, "x1"), num(tag, "y1")], [num(tag, "x2"), num(tag, "y2")]];
                title_for = Some(push(Shape::Line { pts, color: color(tag, "stroke").unwrap_or(Color32::GRAY), w: attr(tag, "stroke-width").and_then(|v| v.parse().ok()).unwrap_or(1.0) }, &mut out.items));
            }
            "circle" => {
                title_for = Some(push(Shape::Circle { c: [num(tag, "cx"), num(tag, "cy")], r: num(tag, "r"), fill: color(tag, "fill").or(Some(Color32::BLACK)) }, &mut out.items));
            }
            "rect" => {
                let fill = if attr(tag, "fill").is_some() { color(tag, "fill") } else { Some(Color32::BLACK) };
                title_for = Some(push(Shape::Rect { r: [num(tag, "x"), num(tag, "y"), num(tag, "width"), num(tag, "height")], fill, stroke: color(tag, "stroke") }, &mut out.items));
            }
            "text" => {
                let ix = push(Shape::Text { p: [num(tag, "x"), num(tag, "y")], s: String::new(), size: attr(tag, "font-size").and_then(|v| v.trim_end_matches("px").parse().ok()).unwrap_or(13.0), color: color(tag, "fill").unwrap_or(Color32::BLACK) }, &mut out.items);
                title_for = Some(ix);
                if !selfclose { text_ix = Some(ix); }
            }
            "title" => { if !selfclose { in_title = true; } }
            _ => {}
        }
    }
    if !seen_svg {
        return Err("不是 SVG".into());
    }
    out.items.retain(|it| !matches!(&it.shape, Shape::Text { s, .. } if s.trim().is_empty()));
    if out.items.is_empty() {
        return Err("SVG 里没有可绘制的图元".into());
    }
    if out.w <= 0.0 || out.h <= 0.0 {
        let (mut mx, mut my) = (0f32, 0f32);
        for it in &out.items {
            let (x, y) = bbox_max(&it.shape);
            mx = mx.max(x);
            my = my.max(y);
        }
        out.w = mx + 20.0;
        out.h = my + 20.0;
    }
    Ok(out)
}

fn bbox_max(s: &Shape) -> (f32, f32) {
    match s {
        Shape::Line { pts, .. } => pts.iter().fold((0.0, 0.0), |a, p| (f32::max(a.0, p[0]), f32::max(a.1, p[1]))),
        Shape::Circle { c, r, .. } => (c[0] + r, c[1] + r),
        Shape::Rect { r, .. } => (r[0] + r[2], r[1] + r[3]),
        Shape::Text { p, s, size, .. } => (p[0] + s.chars().count() as f32 * size * 0.6, p[1] + 4.0),
    }
}

/// 把 SVG 画进 `ui` (固定 1:1 大小, 外面套 ScrollArea)。返回被点击的 `data-line` (1 起)
pub fn show(ui: &mut egui::Ui, svg: &Svg, text_override: Option<Color32>) -> Option<usize> {
    let (resp, painter) = ui.allocate_painter(Vec2::new(svg.w.max(1.0), svg.h.max(1.0)), Sense::click());
    let o = resp.rect.min.to_vec2();
    let at = |p: [f32; 2]| Pos2::new(p[0], p[1]) + o;
    // 先算每个分组的包围盒 (用来命中 + 高亮), 同时画出来
    let mut boxes: Vec<(usize, Rect)> = vec![];
    let mut grow = |line: Option<usize>, r: Rect| {
        if let Some(l) = line {
            match boxes.iter_mut().find(|(k, _)| *k == l) { Some((_, b)) => *b = b.union(r), None => boxes.push((l, r)) }
        }
    };
    let hover = ui.input(|i| i.pointer.hover_pos());
    // 悬停的分组: 用上一轮的盒子无法拿到, 这里两遍: 先量盒子再画
    let mut galleys: Vec<Option<std::sync::Arc<egui::Galley>>> = vec![];
    for it in &svg.items {
        match &it.shape {
            Shape::Text { s, size, color, .. } => {
                let g = painter.layout_no_wrap(s.clone(), egui::FontId::proportional(*size), text_override.unwrap_or(*color));
                galleys.push(Some(g));
            }
            _ => galleys.push(None),
        }
    }
    for (it, g) in svg.items.iter().zip(&galleys) {
        let r = match &it.shape {
            Shape::Circle { c, r, .. } => Rect::from_center_size(at(*c), Vec2::splat(r * 2.0)),
            Shape::Rect { r, .. } => Rect::from_min_size(at([r[0], r[1]]), Vec2::new(r[2], r[3])),
            Shape::Text { p, .. } => { let g = g.as_ref().unwrap(); Rect::from_min_size(at([p[0], p[1]]) - Vec2::new(0.0, g.size().y * 0.8), g.size()) }
            Shape::Line { .. } => continue,
        };
        grow(it.line, r.expand(2.0));
    }
    let hot = hover.and_then(|h| boxes.iter().find(|(_, b)| b.contains(h)).map(|(l, _)| *l));
    if let Some(l) = hot {
        if let Some((_, b)) = boxes.iter().find(|(k, _)| *k == l) {
            painter.rect_filled(*b, 3.0, ui.visuals().selection.bg_fill);
        }
    }
    for (it, g) in svg.items.iter().zip(galleys) {
        match &it.shape {
            Shape::Line { pts, color, w } => {
                let ps: Vec<Pos2> = pts.iter().map(|p| at(*p)).collect();
                painter.add(egui::Shape::line(ps, Stroke::new(*w, *color)));
            }
            Shape::Circle { c, r, fill } => { if let Some(f) = fill { painter.circle_filled(at(*c), *r, *f); } }
            Shape::Rect { r, fill, stroke } => {
                let rc = Rect::from_min_size(at([r[0], r[1]]), Vec2::new(r[2], r[3]));
                if let Some(f) = fill { painter.rect_filled(rc, 0.0, *f); }
                if let Some(s) = stroke { painter.rect_stroke(rc, 0.0, Stroke::new(1.0, *s), egui::StrokeKind::Inside); }
            }
            Shape::Text { p, .. } => {
                let g = g.unwrap();
                let top = at([p[0], p[1]]) - Vec2::new(0.0, g.size().y * 0.8);
                painter.galley(top, g, Color32::WHITE);
            }
        }
    }
    if let (Some(l), Some(h)) = (hot, hover) {
        if let Some(tip) = svg.items.iter().find(|it| it.line == Some(l) && it.tip.is_some()).and_then(|it| it.tip.clone()) {
            let _ = h;
            resp.clone().on_hover_text(tip);
        }
    }
    if resp.clicked() { hot } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_doc_graph_style_svg() {
        let s = r##"<svg viewBox="0 0 480 100" width="480" height="100" xmlns="http://www.w3.org/2000/svg"><path d="M 26 26 V 50 H 14" fill="none" stroke="#888" stroke-width="1"/><g data-line="3" class="gnode"><circle cx="20" cy="20" r="6" fill="#58a6ff"/><text x="32" y="24" font-size="13" fill="#c9d1d9">A &amp; B</text><title>第 3 行</title></g></svg>"##;
        let v = parse(s).unwrap();
        assert_eq!((v.w, v.h), (480.0, 100.0));
        assert_eq!(v.items.len(), 3);
        assert!(matches!(&v.items[0].shape, Shape::Line { pts, .. } if pts == &vec![[26.0, 26.0], [26.0, 50.0], [14.0, 50.0]]));
        assert_eq!(v.items[1].line, Some(3));
        assert!(matches!(&v.items[2].shape, Shape::Text { s, .. } if s == "A & B"));
        assert_eq!(v.items[2].tip.as_deref(), Some("第 3 行"));
    }

    #[test]
    fn curves_and_relative_commands_sample_to_polylines() {
        let p = path_points("M 0 0 C 10 0, 10 10, 0 10 l 5 5");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].len(), 1 + 16 + 1);
        assert_eq!(p[0].last().unwrap(), &[5.0, 15.0]);
    }

    #[test]
    fn scripts_and_foreign_objects_are_ignored() {
        let s = r#"<svg width="10" height="10"><script>alert(1)</script><foreignObject><circle cx="1" cy="1" r="1"/></foreignObject><circle cx="5" cy="5" r="2"/></svg>"#;
        let v = parse(s).unwrap();
        assert_eq!(v.items.len(), 1);
        assert!(parse("<p>hi</p>").is_err());
        assert!(parse(r#"<svg width="1" height="1"></svg>"#).is_err());
    }
}
