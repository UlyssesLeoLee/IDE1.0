//! SVG path 的 d 属性 -> 折线 (含圆弧换算)。

/// path 的 d 属性 -> 折线 (曲线按 16 段采样)。支持 M L H V C Z 及其小写 (相对) 形式
pub(super) fn path_points(d: &str) -> Vec<Vec<[f32; 2]>> {
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
