//! SVG transform 的 2x3 矩阵运算与解析。

pub(super) type M = [f32; 6];
pub(super) const ID: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// a · b (先做 b 再做 a)
pub(super) fn mul(a: &M, b: &M) -> M {
    [a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1], a[0] * b[2] + a[2] * b[3], a[1] * b[2] + a[3] * b[3], a[0] * b[4] + a[2] * b[5] + a[4], a[1] * b[4] + a[3] * b[5] + a[5]]
}

pub(super) fn ap(m: &M, p: [f32; 2]) -> [f32; 2] {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}

pub(super) fn scale_of(m: &M) -> f32 {
    let k = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
    // 插件给的变换可能是 NaN / 极大: 当作 1, 免得字号 / 线宽爆炸
    if k.is_finite() && k < 1000.0 { k } else { 1.0 }
}

/// `translate(..) scale(..) rotate(..) matrix(..)` 的组合
pub(super) fn parse_transform(s: &str) -> M {
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

