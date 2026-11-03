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
    /// `anchor`: 0 = 文字从 x 起, 0.5 = 以 x 居中, 1 = 到 x 结束 (`text-anchor`)
    Text { p: [f32; 2], s: String, size: f32, color: Color32, anchor: f32 },
    /// 闭合图形: 填充 (从第一个点扇形剖分: 凸形 / 饼图扇形 / 箭头这类「从起点看得见全部」的形状正确) 和描边
    Poly { pts: Vec<[f32; 2]>, fill: Option<Color32>, stroke: Option<(Color32, f32)> },
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
            'Q' => {
                let mut v = [0f32; 4];
                for s in &mut v { match n(&mut i) { Some(a) => *s = a, None => return subs } }
                let (p0, c, p1) = ([x, y], [ox + v[0], oy + v[1]], [ox + v[2], oy + v[3]]);
                for k in 1..=16 {
                    let t = k as f32 / 16.0;
                    let u = 1.0 - t;
                    pts.push([u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p1[0], u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p1[1]]);
                }
                x = p1[0]; y = p1[1];
            }
            'A' => {
                let mut v = [0f32; 7];
                for s in &mut v { match n(&mut i) { Some(a) => *s = a, None => return subs } }
                let end = [ox + v[5], oy + v[6]];
                pts.extend(arc_points([x, y], v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, end));
                x = end[0]; y = end[1];
            }
            _ => break, // 其余 (S / T 等) 不支持: 放弃这条 path 的后半
        }
    }
    if pts.len() > 1 { subs.push(pts); }
    subs
}

/// 圆弧 (SVG 端点参数式) -> 折线点 (不含起点, 最后一个点就是终点)
fn arc_points(p0: [f32; 2], rx: f32, ry: f32, phi_deg: f32, large: bool, sweep: bool, p1: [f32; 2]) -> Vec<[f32; 2]> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 || p0 == p1 {
        return vec![p1];
    }
    let tau = std::f32::consts::TAU;
    let (cp, sp) = (phi_deg.to_radians().cos(), phi_deg.to_radians().sin());
    let (dx, dy) = ((p0[0] - p1[0]) / 2.0, (p0[1] - p1[1]) / 2.0);
    let (x1, y1) = (cp * dx + sp * dy, -sp * dx + cp * dy);
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        let s = lam.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = (num / den).max(0.0).sqrt();
    if large == sweep {
        co = -co;
    }
    let (cxp, cyp) = (co * rx * y1 / ry, -co * ry * x1 / rx);
    let cx = cp * cxp - sp * cyp + (p0[0] + p1[0]) / 2.0;
    let cy = sp * cxp + cp * cyp + (p0[1] + p1[1]) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
    if !sweep && dth > 0.0 {
        dth -= tau;
    } else if sweep && dth < 0.0 {
        dth += tau;
    }
    let n = 24;
    let mut v: Vec<[f32; 2]> = (1..=n)
        .map(|k| {
            let t = th1 + dth * k as f32 / n as f32;
            let (ct, st) = (t.cos(), t.sin());
            [cp * rx * ct - sp * ry * st + cx, sp * rx * ct + cp * ry * st + cy]
        })
        .collect();
    if let Some(l) = v.last_mut() {
        *l = p1;
    }
    v
}

type M = [f32; 6];
const ID: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// a · b (先做 b 再做 a)
fn mul(a: &M, b: &M) -> M {
    [a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1], a[0] * b[2] + a[2] * b[3], a[1] * b[2] + a[3] * b[3], a[0] * b[4] + a[2] * b[5] + a[4], a[1] * b[4] + a[3] * b[5] + a[5]]
}

fn ap(m: &M, p: [f32; 2]) -> [f32; 2] {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}

fn scale_of(m: &M) -> f32 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
}

/// `translate(..) scale(..) rotate(..) matrix(..)` 的组合
fn parse_transform(s: &str) -> M {
    let mut out = ID;
    let mut rest = s;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().trim_start_matches(',').trim();
        let Some(close) = rest[open..].find(')') else { break };
        let nums: Vec<f32> = rest[open + 1..open + close].split([' ', ',']).filter_map(|t| t.trim().parse().ok()).collect();
        rest = &rest[open + close + 1..];
        let f: M = match (name, nums.as_slice()) {
            ("translate", [x]) => [1.0, 0.0, 0.0, 1.0, *x, 0.0],
            ("translate", [x, y, ..]) => [1.0, 0.0, 0.0, 1.0, *x, *y],
            ("scale", [s]) => [*s, 0.0, 0.0, *s, 0.0, 0.0],
            ("scale", [x, y, ..]) => [*x, 0.0, 0.0, *y, 0.0, 0.0],
            ("matrix", [a, b, c, d, e, f]) => [*a, *b, *c, *d, *e, *f],
            ("rotate", [a]) => {
                let (s, c) = a.to_radians().sin_cos();
                [c, s, -s, c, 0.0, 0.0]
            }
            ("rotate", [a, cx, cy]) => {
                let (s, c) = a.to_radians().sin_cos();
                mul(&mul(&[1.0, 0.0, 0.0, 1.0, *cx, *cy], &[c, s, -s, c, 0.0, 0.0]), &[1.0, 0.0, 0.0, 1.0, -*cx, -*cy])
            }
            _ => ID,
        };
        out = mul(&out, &f);
    }
    out
}

/// `var(--名字, 备用值)` 递归展开 (有深度上限); 不是 var() 原样返回
fn resolve(v: &str, vars: &std::collections::HashMap<String, String>, depth: u8) -> String {
    let v = v.trim();
    if let Some(inner) = v.strip_prefix("var(").and_then(|r| r.strip_suffix(')')) {
        if depth > 8 {
            return String::new();
        }
        let (name, fb) = match inner.split_once(',') {
            Some((n, f)) => (n.trim(), Some(f.trim())),
            None => (inner.trim(), None),
        };
        if let Some(x) = vars.get(name) {
            return resolve(x, vars, depth + 1);
        }
        return fb.map(|f| resolve(f, vars, depth + 1)).unwrap_or_default();
    }
    v.to_string()
}

fn style_decl(tag: &str, name: &str) -> Option<String> {
    let st = attr(tag, "style")?;
    st.split(';').find_map(|d| {
        let (k, v) = d.split_once(':')?;
        (k.trim() == name).then(|| v.trim().to_string())
    })
}

/// 属性, 没有就取 style 里的同名声明
fn prop(tag: &str, name: &str) -> Option<String> {
    attr(tag, name).or_else(|| style_decl(tag, name))
}

/// 继承的绘制状态 (g / svg 往下传)
#[derive(Clone)]
struct St {
    fill: Option<Color32>,
    stroke: Option<Color32>,
    sw: f32,
    anchor: f32,
    size: f32,
    color: Color32,
    tf: M,
    line: Option<usize>,
}

fn px(v: &str) -> Option<f32> {
    v.trim().trim_end_matches("px").parse().ok()
}

/// 颜色值: Some(None) = none, Some(Some(c)) = 颜色, None = 认不出 (沿用继承)
fn paint(v: &str, st: &St) -> Option<Option<Color32>> {
    match v.trim() {
        "none" | "transparent" => Some(None),
        "currentColor" => Some(Some(st.color)),
        t => crate::plugins::parse_color(t).map(Some),
    }
}

fn derive(parent: &St, tag: &str, vars: &std::collections::HashMap<String, String>) -> St {
    let mut s = parent.clone();
    let get = |n: &str| prop(tag, n).map(|v| resolve(&v, vars, 0));
    if let Some(c) = get("color").and_then(|v| crate::plugins::parse_color(&v)) {
        s.color = c;
    }
    if let Some(f) = get("fill").and_then(|v| paint(&v, &s)) {
        s.fill = f;
    }
    if let Some(f) = get("stroke").and_then(|v| paint(&v, &s)) {
        s.stroke = f;
    }
    if let Some(w) = get("stroke-width").as_deref().and_then(px) {
        s.sw = w;
    }
    if let Some(a) = get("text-anchor") {
        s.anchor = match a.as_str() { "middle" => 0.5, "end" => 1.0, _ => 0.0 };
    }
    if let Some(z) = get("font-size").as_deref().and_then(px) {
        s.size = z;
    } else if let Some(z) = style_decl(tag, "font").and_then(|f| f.split_whitespace().find_map(|t| t.strip_suffix("px").and_then(|n| n.parse().ok()))) {
        s.size = z;
    }
    if let Some(t) = attr(tag, "transform") {
        s.tf = mul(&s.tf, &parse_transform(&t));
    }
    if let Some(l) = attr(tag, "data-line").and_then(|v| v.parse().ok()) {
        s.line = Some(l);
    }
    s
}

fn push_item(out: &mut Svg, shape: Shape, line: Option<usize>) -> usize {
    out.items.push(Item { shape, line, tip: None });
    out.items.len() - 1
}

fn points_attr(tag: &str) -> Vec<[f32; 2]> {
    let v: Vec<f32> = attr(tag, "points").unwrap_or_default().split([' ', ',']).filter_map(|t| t.trim().parse().ok()).collect();
    v.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
}

/// 解析 SVG 文本。不是 `<svg` 开头 / 一个图元都没认出来 = Err (调用方退回显示文本)
pub fn parse(svg: &str) -> Result<Svg, String> {
    parse_with(svg, &[])
}

/// 同 [`parse`]; `overrides` = 外部给的 CSS 变量 (如 `--pele-fg`), 优先于 SVG 自己的声明和 `var()` 的备用值
pub fn parse_with(svg: &str, overrides: &[(&str, String)]) -> Result<Svg, String> {
    let mut out = Svg::default();
    let mut vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let root = St { fill: Some(Color32::BLACK), stroke: None, sw: 1.0, anchor: 0.0, size: 13.0, color: Color32::BLACK, tf: ID, line: None };
    // 绘制状态栈: 栈底是 <svg>, 每个 <g> 压一层, 子元素继承栈顶
    let mut stack: Vec<St> = vec![root];
    let mut skip_depth = 0usize; // <script> / <foreignObject> / <defs> ... 里面的全部忽略
    let mut rest = svg;
    let mut seen_svg = false;
    let mut text_ix: Option<usize> = None;
    let mut title_for: Option<usize> = None;
    let mut in_title = false;
    const SKIP: [&str; 8] = ["script", "foreignObject", "defs", "marker", "clipPath", "mask", "symbol", "style"];
    while let Some(lt) = rest.find('<') {
        let text_before = &rest[..lt];
        rest = &rest[lt..];
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        if skip_depth == 0 {
            if let Some(ix) = text_ix {
                if let Shape::Text { s, .. } = &mut out.items[ix].shape {
                    s.push_str(&unescape(text_before));
                }
            } else if in_title {
                if let Some(ix) = title_for {
                    out.items[ix].tip = Some(unescape(text_before));
                }
            }
        }
        let closing = tag.starts_with('/');
        let body = tag.trim_start_matches('/');
        let name_end = body.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(body.len());
        let name = &body[..name_end];
        let selfclose = tag.ends_with('/');
        if closing {
            match name {
                n if SKIP.contains(&n) => skip_depth = skip_depth.saturating_sub(1),
                "g" => {
                    if stack.len() > 1 {
                        stack.pop();
                    }
                }
                "text" => text_ix = None,
                "title" => in_title = false,
                _ => {}
            }
            continue;
        }
        if name.starts_with('!') || name.starts_with('?') {
            continue;
        }
        if SKIP.contains(&name) {
            if !selfclose {
                skip_depth += 1;
            }
            continue;
        }
        if skip_depth > 0 {
            continue;
        }
        let top = stack.last().cloned().unwrap_or_else(|| stack[0].clone());
        match name {
            "svg" => {
                seen_svg = true;
                if let Some(st) = attr(tag, "style") {
                    for d in st.split(';') {
                        if let Some((k, v)) = d.split_once(':') {
                            if k.trim().starts_with("--") {
                                vars.insert(k.trim().to_string(), v.trim().to_string());
                            }
                        }
                    }
                }
                for (k, v) in overrides {
                    vars.insert((*k).to_string(), v.clone());
                }
                stack[0] = derive(&stack[0], tag, &vars);
                if let Some(vb) = attr(tag, "viewBox") {
                    let v: Vec<f32> = vb.split([' ', ',']).filter_map(|s| s.parse().ok()).collect();
                    if v.len() == 4 {
                        out.w = v[2];
                        out.h = v[3];
                    }
                }
                if out.w == 0.0 {
                    out.w = num(tag, "width");
                    out.h = num(tag, "height");
                }
            }
            "g" => {
                if !selfclose {
                    stack.push(derive(&top, tag, &vars));
                }
            }
            "path" | "line" | "polyline" | "polygon" | "circle" | "ellipse" | "rect" | "text" => {
                let st = derive(&top, tag, &vars);
                let sc = scale_of(&st.tf);
                let stroke = st.stroke.map(|c| (c, st.sw * sc));
                let first = match name {
                    "path" => {
                        let mut first = None;
                        for pts in path_points(&attr(tag, "d").unwrap_or_default()) {
                            let closed = pts.len() > 2 && pts.first() == pts.last();
                            let pts: Vec<[f32; 2]> = pts.iter().map(|p| ap(&st.tf, *p)).collect();
                            let shape = if closed && st.fill.is_some() {
                                Shape::Poly { pts, fill: st.fill, stroke }
                            } else if let Some((color, w)) = stroke {
                                Shape::Line { pts, color, w }
                            } else {
                                continue;
                            };
                            let ix = push_item(&mut out, shape, st.line);
                            first.get_or_insert(ix);
                        }
                        first
                    }
                    "line" => stroke.map(|(color, w)| {
                        let pts = vec![ap(&st.tf, [num(tag, "x1"), num(tag, "y1")]), ap(&st.tf, [num(tag, "x2"), num(tag, "y2")])];
                        push_item(&mut out, Shape::Line { pts, color, w }, st.line)
                    }),
                    "polyline" | "polygon" => {
                        let mut pts: Vec<[f32; 2]> = points_attr(tag).iter().map(|p| ap(&st.tf, *p)).collect();
                        if pts.len() < 2 {
                            None
                        } else if name == "polygon" {
                            Some(push_item(&mut out, Shape::Poly { pts, fill: st.fill, stroke }, st.line))
                        } else {
                            stroke.map(|(color, w)| {
                                pts.shrink_to_fit();
                                push_item(&mut out, Shape::Line { pts, color, w }, st.line)
                            })
                        }
                    }
                    "circle" => Some(push_item(&mut out, Shape::Circle { c: ap(&st.tf, [num(tag, "cx"), num(tag, "cy")]), r: num(tag, "r") * sc, fill: st.fill }, st.line)),
                    "ellipse" => Some(push_item(&mut out, Shape::Circle { c: ap(&st.tf, [num(tag, "cx"), num(tag, "cy")]), r: (num(tag, "rx") + num(tag, "ry")) / 2.0 * sc, fill: st.fill }, st.line)),
                    "rect" => {
                        let (x, y, w, h) = (num(tag, "x"), num(tag, "y"), num(tag, "width"), num(tag, "height"));
                        let shape = if st.tf[1].abs() < 1e-6 && st.tf[2].abs() < 1e-6 {
                            let (a, b) = (ap(&st.tf, [x, y]), ap(&st.tf, [x + w, y + h]));
                            Shape::Rect { r: [a[0].min(b[0]), a[1].min(b[1]), (b[0] - a[0]).abs(), (b[1] - a[1]).abs()], fill: st.fill, stroke: st.stroke }
                        } else {
                            Shape::Poly { pts: [[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]].iter().map(|p| ap(&st.tf, *p)).collect(), fill: st.fill, stroke }
                        };
                        Some(push_item(&mut out, shape, st.line))
                    }
                    _ => {
                        // text
                        match st.fill {
                            Some(color) => {
                                let ix = push_item(&mut out, Shape::Text { p: ap(&st.tf, [num(tag, "x"), num(tag, "y")]), s: String::new(), size: st.size * sc, color, anchor: st.anchor }, st.line);
                                if !selfclose {
                                    text_ix = Some(ix);
                                }
                                Some(ix)
                            }
                            None => None,
                        }
                    }
                };
                title_for = first;
            }
            "title" => {
                if !selfclose {
                    in_title = true;
                }
            }
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
        Shape::Poly { pts, .. } => pts.iter().fold((0.0, 0.0), |a, p| (f32::max(a.0, p[0]), f32::max(a.1, p[1]))),
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
            Shape::Text { p, anchor, .. } => { let g = g.as_ref().unwrap(); Rect::from_min_size(at([p[0], p[1]]) - Vec2::new(anchor * g.size().x, g.size().y * 0.8), g.size()) }
            Shape::Poly { pts, .. } => Rect::from_points(&pts.iter().map(|p| at(*p)).collect::<Vec<_>>()),
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
            Shape::Poly { pts, fill, stroke } => {
                let ps: Vec<Pos2> = pts.iter().map(|p| at(*p)).collect();
                if let (Some(f), Some(first)) = (fill, ps.first()) {
                    for w in ps[1..].windows(2) {
                        painter.add(egui::Shape::convex_polygon(vec![*first, w[0], w[1]], *f, Stroke::NONE));
                    }
                }
                if let Some((c, w)) = stroke { painter.add(egui::Shape::line(ps, Stroke::new(*w, *c))); }
            }
            Shape::Text { p, anchor, .. } => {
                let g = g.unwrap();
                let top = at([p[0], p[1]]) - Vec2::new(anchor * g.size().x, g.size().y * 0.8);
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
    fn pele_style_svg_resolves_css_vars_transforms_and_inherited_paint() {
        let s = r#"<svg viewBox="0 0 86 160" width="86" height="160" style="--_s:var(--pele-surface,#f3f3f3);--_l:var(--pele-line,#111111);font:16px sans-serif" fill="var(--_l)"><g fill="none" stroke="var(--_l)"><path d="M42.9,56L42.9,97"/><path d="M42.9,104L39.5,96L46.3,96Z" fill="var(--_l)" stroke="none"/></g><g transform="translate(42.9,32)"><rect x="-34" y="-24" width="68" height="48" fill="var(--_s)" stroke="var(--pele-border,#8a8a8a)"/><text x="0" y="5" text-anchor="middle">Start</text></g></svg>"#;
        let v = parse_with(s, &[("--pele-surface", "#102030".into())]).unwrap();
        assert!(matches!(&v.items[0].shape, Shape::Line { color, .. } if *color == Color32::from_rgb(0x11, 0x11, 0x11)), "继承 stroke + var 备用值: {:?}", v.items[0]);
        assert!(matches!(&v.items[1].shape, Shape::Poly { fill: Some(_), stroke: None, .. }), "闭合 + fill = 填充多边形");
        assert!(matches!(&v.items[2].shape, Shape::Rect { r, fill, .. } if (r[0] - 8.9).abs() < 1e-3 && r[1..] == [8.0, 68.0, 48.0] && *fill == Some(Color32::from_rgb(0x10, 0x20, 0x30))), "translate + 外部变量: {:?}", v.items[2]);
        assert!(matches!(&v.items[3].shape, Shape::Text { p, size, anchor, s, .. } if *p == [42.9, 37.0] && *size == 16.0 && *anchor == 0.5 && s == "Start"));
    }

    #[test]
    fn arcs_close_on_their_end_point_and_defs_are_skipped() {
        let p = path_points("M0,0L0,-10A10,10 0 0 1 10,0");
        assert_eq!(p[0].last().unwrap(), &[10.0, 0.0]);
        assert!(p[0].len() > 10);
        let v = parse(r#"<svg width="9" height="9"><defs><circle cx="1" cy="1" r="1"/></defs><circle cx="5" cy="5" r="2"/></svg>"#).unwrap();
        assert_eq!(v.items.len(), 1);
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
