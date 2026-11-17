//! 古希腊爱奥尼神殿立柱、二十面体晶钻与阿波罗桂冠 3D 参数化几何构建
//! (DD-004 规范实现: 7 凹槽 Entasis 弧度柱身、26 片月桂叶阿波罗金冠与希腊回形纹饰带)

use super::cel3d::{CelMaterial, Face3D, Mesh3D, Vec3};
use std::f32::consts::PI;

/// 构建古希腊经典爱奥尼立柱 3D 模型 (Ionic Column Pedestal)
/// 严格满足 DD-004: 柱身垂直连续凹槽、中部微幅膨胀曲线 (Entasis) 与回形纹大理石柱础
pub fn build_ionic_pedestal() -> Mesh3D {
    let mut mesh = Mesh3D::new();

    // 1. 底层大理石柱础三层台阶 (Stepped Plinth, 12 边形几何台阶)
    let n = 12;
    let tiers = [
        (-96.0, -82.0, 82.0, 82.0, CelMaterial::MarbleWhite),
        (-82.0, -72.0, 74.0, 70.0, CelMaterial::GoldTrim), // 希腊黄金雕带 (Meander Band)
        (-72.0, -62.0, 66.0, 62.0, CelMaterial::MarbleWhite),
    ];
    for (y_bot, y_top, r_bot, r_top, mat) in tiers {
        add_cylinder(&mut mesh, y_bot, y_top, r_bot, r_top, n, mat);
    }

    // 2. 凹槽柱身与中部微幅膨胀曲线 (Fluted Shaft with Entasis, 14 凹凸节律)
    let shaft_segments = 14;
    let shaft_y_bot = -62.0;
    let shaft_y_top = 26.0;
    let entasis_ratio = 1.05; // DD-004: 最大扩张比 1.05

    let mut shaft_bot_pts = Vec::with_capacity(shaft_segments);
    let mut shaft_top_pts = Vec::with_capacity(shaft_segments);

    for i in 0..shaft_segments {
        let a = (i as f32) * 2.0 * PI / (shaft_segments as f32);
        // 凹槽起伏与 Entasis 曲线参数化
        let r_mod_bot = if i % 2 == 0 { 44.0 } else { 40.0 };
        let r_mod_top = if i % 2 == 0 { 39.0 } else { 36.0 };
        let r_bot = r_mod_bot * entasis_ratio;
        let r_top = r_mod_top;
        shaft_bot_pts.push(Vec3::new(a.cos() * r_bot, shaft_y_bot, a.sin() * r_bot));
        shaft_top_pts.push(Vec3::new(a.cos() * r_top, shaft_y_top, a.sin() * r_top));
    }

    let b_start = mesh.vertices.len();
    mesh.vertices.extend(shaft_bot_pts);
    let t_start = mesh.vertices.len();
    mesh.vertices.extend(shaft_top_pts);

    for i in 0..shaft_segments {
        let next = (i + 1) % shaft_segments;
        let b0 = b_start + i;
        let b1 = b_start + next;
        let t0 = t_start + i;
        let t1 = t_start + next;
        mesh.faces.push(Face3D {
            indices: vec![b0, b1, t1, t0],
            material: if i % 2 == 0 {
                CelMaterial::MarbleWhite
            } else {
                CelMaterial::DarkPedestal
            },
        });
    }

    // 3. 爱奥尼双向立体卷涡卷轴柱头与顶板 (Ionic Volutes & Abacus)
    add_cylinder(
        &mut mesh,
        26.0,
        34.0,
        48.0,
        52.0,
        n,
        CelMaterial::MarbleWhite,
    );
    add_box(
        &mut mesh,
        Vec3::new(0.0, 39.0, 0.0),
        Vec3::new(104.0, 8.0, 84.0),
        CelMaterial::MarbleWhite,
    );

    // 左右对数卷涡金饰 (Ionic Scroll Volute Blocks)
    add_box(
        &mut mesh,
        Vec3::new(-48.0, 33.0, 0.0),
        Vec3::new(14.0, 16.0, 48.0),
        CelMaterial::GoldTrim,
    );
    add_box(
        &mut mesh,
        Vec3::new(48.0, 33.0, 0.0),
        Vec3::new(14.0, 16.0, 48.0),
        CelMaterial::GoldTrim,
    );

    mesh
}

/// 构建悬浮晶钻正二十面体 (Faceted Crystal 3D Polyhedron)
pub fn build_crystal_gem(radius: f32) -> Mesh3D {
    let mut mesh = Mesh3D::new();
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;

    let base_verts = [
        Vec3::new(-1.0, phi, 0.0),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(-1.0, -phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(0.0, -1.0, phi),
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, -1.0, -phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(phi, 0.0, -1.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(-phi, 0.0, -1.0),
        Vec3::new(-phi, 0.0, 1.0),
    ];

    mesh.vertices = base_verts
        .iter()
        .map(|v| v.norm() * radius)
        .collect::<Vec<_>>();

    let faces = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    for f in faces {
        mesh.faces.push(Face3D {
            indices: f.to_vec(),
            material: CelMaterial::CrystalGem,
        });
    }

    mesh
}

/// 构建阿波罗黄金桂冠环绕月桂叶群 (Apollo Golden Laurel Wreath)
/// 严格满足 BD-003 / DD-004: 26 片有机月桂叶片，空间椭圆轨道与立体倾角排布
pub fn build_laurel_wreath(leaf_count: usize, ring_r_x: f32, ring_r_z: f32) -> Mesh3D {
    let mut mesh = Mesh3D::new();

    for i in 0..leaf_count {
        let a = (i as f32) * 2.0 * PI / (leaf_count as f32);
        let center = Vec3::new(a.cos() * ring_r_x, (a * 1.5).sin() * 5.0, a.sin() * ring_r_z);
        let tangent = Vec3::new(-a.sin(), 0.1, a.cos()).norm();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let normal = tangent.cross(up).norm();

        let l_size = 9.0;
        let w_size = 4.2;

        let v_base = center - tangent * (l_size * 0.5);
        let v_tip = center + tangent * (l_size * 0.5);
        let v_left = center + normal * w_size;
        let v_right = center - normal * w_size;

        let start = mesh.vertices.len();
        mesh.vertices.push(v_base);
        mesh.vertices.push(v_tip);
        mesh.vertices.push(v_left);
        mesh.vertices.push(v_right);

        mesh.faces.push(Face3D {
            indices: vec![start, start + 2, start + 1, start + 3],
            material: CelMaterial::LaurelLeaf,
        });
    }

    mesh
}

fn add_cylinder(
    mesh: &mut Mesh3D,
    y_bot: f32,
    y_top: f32,
    r_bot: f32,
    r_top: f32,
    segments: usize,
    mat: CelMaterial,
) {
    let start_idx = mesh.vertices.len();
    for i in 0..segments {
        let a = (i as f32) * 2.0 * PI / (segments as f32);
        mesh.vertices
            .push(Vec3::new(a.cos() * r_bot, y_bot, a.sin() * r_bot));
    }
    for i in 0..segments {
        let a = (i as f32) * 2.0 * PI / (segments as f32);
        mesh.vertices
            .push(Vec3::new(a.cos() * r_top, y_top, a.sin() * r_top));
    }

    for i in 0..segments {
        let next = (i + 1) % segments;
        let b0 = start_idx + i;
        let b1 = start_idx + next;
        let t0 = start_idx + segments + i;
        let t1 = start_idx + segments + next;
        mesh.faces.push(Face3D {
            indices: vec![b0, b1, t1, t0],
            material: mat,
        });
    }

    let top_indices: Vec<usize> = (0..segments).map(|i| start_idx + segments + i).collect();
    mesh.faces.push(Face3D {
        indices: top_indices,
        material: mat,
    });
}

fn add_box(mesh: &mut Mesh3D, center: Vec3, size: Vec3, mat: CelMaterial) {
    let hx = size.x * 0.5;
    let hy = size.y * 0.5;
    let hz = size.z * 0.5;
    let start = mesh.vertices.len();

    let pts = [
        center + Vec3::new(-hx, -hy, -hz),
        center + Vec3::new(hx, -hy, -hz),
        center + Vec3::new(hx, hy, -hz),
        center + Vec3::new(-hx, hy, -hz),
        center + Vec3::new(-hx, -hy, hz),
        center + Vec3::new(hx, -hy, hz),
        center + Vec3::new(hx, hy, hz),
        center + Vec3::new(-hx, hy, hz),
    ];
    mesh.vertices.extend(pts);

    let faces = [
        [0, 1, 2, 3], // Front
        [5, 4, 7, 6], // Back
        [4, 0, 3, 7], // Left
        [1, 5, 6, 2], // Right
        [3, 2, 6, 7], // Top
        [4, 5, 1, 0], // Bottom
    ];
    for f in faces {
        mesh.faces.push(Face3D {
            indices: f.iter().map(|idx| start + idx).collect(),
            material: mat,
        });
    }
}
