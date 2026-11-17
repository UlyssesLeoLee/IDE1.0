//! 经典美漫与赛璐珞风格真 3D 渲染微内核 (True 3D Cel-Shaded Engine)
//! 具备完整的 3D 几何管线、透视投影、3D 阶梯式赛璐珞多色阶光照 (Cel-Shading Bands)、
//! 纯黑美漫墨线外轮廓 (Ink Outlines)、画师算法深度排序与高光星芒粒子。

use eframe::egui;
use egui::{pos2, Color32, Painter, Pos2, Rect, Stroke};
use std::f32::consts::PI;

/// 3D 三维向量
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        Self {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
        }
    }

    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    pub fn norm(self) -> Self {
        let l = self.length();
        if l > 1e-6 {
            Self {
                x: self.x / l,
                y: self.y / l,
                z: self.z / l,
            }
        } else {
            Self::ZERO
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl std::ops::Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// 4x4 仿射变换矩阵
#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub fn identity() -> Self {
        let mut m = [[0.0; 4]; 4];
        for i in 0..4 {
            m[i][i] = 1.0;
        }
        Self { m }
    }

    pub fn rot_x(rad: f32) -> Self {
        let (s, c) = (rad.sin(), rad.cos());
        let mut m = Self::identity();
        m.m[1][1] = c;
        m.m[1][2] = -s;
        m.m[2][1] = s;
        m.m[2][2] = c;
        m
    }

    pub fn rot_y(rad: f32) -> Self {
        let (s, c) = (rad.sin(), rad.cos());
        let mut m = Self::identity();
        m.m[0][0] = c;
        m.m[0][2] = s;
        m.m[2][0] = -s;
        m.m[2][2] = c;
        m
    }

    pub fn rot_z(rad: f32) -> Self {
        let (s, c) = (rad.sin(), rad.cos());
        let mut m = Self::identity();
        m.m[0][0] = c;
        m.m[0][1] = -s;
        m.m[1][0] = s;
        m.m[1][1] = c;
        m
    }

    pub fn translate(tx: f32, ty: f32, tz: f32) -> Self {
        let mut m = Self::identity();
        m.m[0][3] = tx;
        m.m[1][3] = ty;
        m.m[2][3] = tz;
        m
    }

    pub fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        let mut m = Self::identity();
        m.m[0][0] = sx;
        m.m[1][1] = sy;
        m.m[2][2] = sz;
        m
    }

    pub fn mul(&self, o: &Self) -> Self {
        let mut res = [[0.0; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                res[r][c] = self.m[r][0] * o.m[0][c]
                    + self.m[r][1] * o.m[1][c]
                    + self.m[r][2] * o.m[2][c]
                    + self.m[r][3] * o.m[3][c];
            }
        }
        Self { m: res }
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let x = self.m[0][0] * p.x + self.m[0][1] * p.y + self.m[0][2] * p.z + self.m[0][3];
        let y = self.m[1][0] * p.x + self.m[1][1] * p.y + self.m[1][2] * p.z + self.m[1][3];
        let z = self.m[2][0] * p.x + self.m[2][1] * p.y + self.m[2][2] * p.z + self.m[2][3];
        Vec3::new(x, y, z)
    }

    pub fn transform_dir(&self, d: Vec3) -> Vec3 {
        let x = self.m[0][0] * d.x + self.m[0][1] * d.y + self.m[0][2] * d.z;
        let y = self.m[1][0] * d.x + self.m[1][1] * d.y + self.m[1][2] * d.z;
        let z = self.m[2][0] * d.x + self.m[2][1] * d.y + self.m[2][2] * d.z;
        Vec3::new(x, y, z).norm()
    }
}

/// 赛璐珞材质类型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CelMaterial {
    MarbleWhite,
    GoldTrim,
    CrystalGem,
    LaurelLeaf,
    DarkPedestal,
}

impl CelMaterial {
    pub fn shade(&self, normal: Vec3, light: Vec3, view: Vec3) -> Color32 {
        let dot = normal.dot(-light).clamp(-1.0, 1.0);
        let half = (-light + view).norm();
        let spec = normal.dot(half).max(0.0).powf(16.0);

        match self {
            Self::MarbleWhite => {
                if spec > 0.85 {
                    Color32::WHITE
                } else if dot > 0.40 {
                    Color32::from_rgb(250, 252, 255)
                } else if dot > -0.10 {
                    Color32::from_rgb(218, 226, 240)
                } else {
                    Color32::from_rgb(165, 178, 202)
                }
            }
            Self::GoldTrim => {
                if spec > 0.82 {
                    Color32::WHITE
                } else if dot > 0.45 {
                    Color32::from_rgb(255, 236, 80)
                } else if dot > 0.05 {
                    Color32::from_rgb(245, 185, 20)
                } else {
                    Color32::from_rgb(175, 115, 12)
                }
            }
            Self::CrystalGem => {
                if spec > 0.88 {
                    Color32::WHITE
                } else if dot > 0.55 {
                    Color32::from_rgb(175, 255, 255)
                } else if dot > 0.15 {
                    Color32::from_rgb(0, 225, 255)
                } else if dot > -0.25 {
                    Color32::from_rgb(0, 140, 230)
                } else {
                    Color32::from_rgb(14, 45, 115)
                }
            }
            Self::LaurelLeaf => {
                if dot > 0.45 {
                    Color32::from_rgb(255, 230, 60)
                } else if dot > 0.05 {
                    Color32::from_rgb(235, 175, 20)
                } else {
                    Color32::from_rgb(160, 105, 10)
                }
            }
            Self::DarkPedestal => {
                if dot > 0.30 {
                    Color32::from_rgb(60, 68, 92)
                } else {
                    Color32::from_rgb(28, 32, 48)
                }
            }
        }
    }
}

/// 3D 多边形面
#[derive(Clone, Debug)]
pub struct Face3D {
    pub indices: Vec<usize>,
    pub material: CelMaterial,
}

/// 3D 网格模型
#[derive(Clone, Debug, Default)]
pub struct Mesh3D {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face3D>,
}

impl Mesh3D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, other: Mesh3D) {
        let base_idx = self.vertices.len();
        self.vertices.extend(other.vertices);
        for mut f in other.faces {
            for idx in &mut f.indices {
                *idx += base_idx;
            }
            self.faces.push(f);
        }
    }
}

/// 3D 相机
#[derive(Clone, Copy, Debug)]
pub struct Camera3D {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub focal: f32,
    pub target: Vec3,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            yaw: 0.35,
            pitch: 0.22,
            dist: 450.0,
            focal: 380.0,
            target: Vec3::new(0.0, 20.0, 0.0),
        }
    }
}

/// 构建古希腊经典爱奥尼立柱 3D 模型 (Ionic Column Pedestal)
pub fn build_ionic_pedestal() -> Mesh3D {
    let mut mesh = Mesh3D::new();

    // 1. 底层大理石柱础台阶 (Stepped Plinth, 12边形台阶)
    let n = 12;
    let tiers = [
        (-95.0, -82.0, 80.0, 80.0, CelMaterial::MarbleWhite),
        (-82.0, -72.0, 72.0, 68.0, CelMaterial::GoldTrim), // 黄金回形纹饰带
        (-72.0, -62.0, 64.0, 60.0, CelMaterial::MarbleWhite),
    ];
    for (y_bot, y_top, r_bot, r_top, mat) in tiers {
        add_cylinder(&mut mesh, y_bot, y_top, r_bot, r_top, n, mat);
    }

    // 2. 凹槽凹凸柱身 (Fluted Shaft with Entasis, 16 边形起伏凹槽)
    let shaft_segments = 16;
    let shaft_y_bot = -62.0;
    let shaft_y_top = 26.0;
    let mut shaft_bot_pts = Vec::with_capacity(shaft_segments);
    let mut shaft_top_pts = Vec::with_capacity(shaft_segments);

    for i in 0..shaft_segments {
        let a = (i as f32) * 2.0 * PI / (shaft_segments as f32);
        // 凹凸凹槽起伏
        let r_mod = if i % 2 == 0 { 44.0 } else { 40.0 };
        let r_top = if i % 2 == 0 { 39.0 } else { 36.0 };
        shaft_bot_pts.push(Vec3::new(a.cos() * r_mod, shaft_y_bot, a.sin() * r_mod));
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

    // 3. 爱奥尼双向卷轴柱头与顶板 (Ionic Capital & Abacus)
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

    // 左右立体卷涡装饰块
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

    // 侧面
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

    // 顶面
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

/// 渲染 3D 赛璐珞场景到 egui 画布
pub fn render_cel3d_scene(
    p: &Painter,
    rect: Rect,
    cam: &Camera3D,
    pedestal: &Mesh3D,
    gem: &Mesh3D,
    wreath: &Mesh3D,
    gem_model_mat: &Mat4,
    wreath_model_mat: &Mat4,
    time: f32,
    is_warp: bool,
) {
    let center = rect.center();
    let rot_x = Mat4::rot_x(cam.pitch);
    let rot_y = Mat4::rot_y(cam.yaw);
    let view_rot = rot_x.mul(&rot_y);

    let light_dir = Vec3::new(0.5, 0.75, -0.6).norm();
    let view_dir = Vec3::new(0.0, 0.0, -1.0);

    struct RenderPolygon {
        pts: Vec<Pos2>,
        depth: f32,
        color: Color32,
    }

    let mut render_list = Vec::with_capacity(300);

    // 辅助闭包：将 Mesh 变换并投影生成待渲染面列表
    let mut process_mesh = |mesh: &Mesh3D, model_mat: Option<&Mat4>| {
        let world_verts: Vec<Vec3> = if let Some(m) = model_mat {
            mesh.vertices.iter().map(|v| m.transform_point(*v)).collect()
        } else {
            mesh.vertices.clone()
        };

        let cam_verts: Vec<Vec3> = world_verts
            .iter()
            .map(|w| {
                let rel = *w - cam.target;
                let rot = view_rot.transform_point(rel);
                Vec3::new(rot.x, rot.y, rot.z + cam.dist)
            })
            .collect();

        for face in &mesh.faces {
            if face.indices.len() < 3 {
                continue;
            }

            // 计算多边形在相机坐标系下的法线
            let p0 = cam_verts[face.indices[0]];
            let p1 = cam_verts[face.indices[1]];
            let p2 = cam_verts[face.indices[2]];
            let n = (p1 - p0).cross(p2 - p0).norm();

            // 背面剔除 (Backface Culling)
            let view_v = (-p0).norm();
            if n.dot(view_v) <= 0.01 {
                continue;
            }

            let light_cam = view_rot.transform_dir(light_dir);
            let col = face.material.shade(n, light_cam, view_dir);

            // 投影到屏幕 2D 坐标
            let mut screen_pts = Vec::with_capacity(face.indices.len());
            let mut z_sum = 0.0;
            let mut valid = true;

            for &idx in &face.indices {
                let cp = cam_verts[idx];
                if cp.z <= 10.0 {
                    valid = false;
                    break;
                }
                let sx = center.x + (cp.x * cam.focal) / cp.z;
                let sy = center.y - (cp.y * cam.focal) / cp.z;
                screen_pts.push(pos2(sx, sy));
                z_sum += cp.z;
            }

            if valid {
                let depth = z_sum / (face.indices.len() as f32);
                render_list.push(RenderPolygon {
                    pts: screen_pts,
                    depth,
                    color: col,
                });
            }
        }
    };

    // 1. 处理神殿立柱
    process_mesh(pedestal, None);

    // 2. 处理悬浮晶石
    process_mesh(gem, Some(gem_model_mat));

    // 3. 处理环绕桂冠
    process_mesh(wreath, Some(wreath_model_mat));

    // 4. 画师算法深度排序 (由远及近)
    render_list.sort_by(|a, b| {
        b.depth
            .partial_cmp(&a.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 5. 渲染 3D 赛璐珞面并叠加密实粗黑美漫墨线外轮廓
    for poly in render_list {
        p.add(egui::Shape::convex_polygon(
            poly.pts,
            poly.color,
            Stroke::new(2.0, Color32::BLACK),
        ));
    }

    // 6. 3D 星芒粒子闪烁 (Billboard Glints)
    let glint_offsets = [
        (0.0, 85.0, 0.0),
        (-22.0, 95.0, 14.0),
        (22.0, 75.0, -14.0),
    ];
    for (i, &(gx, gy, gz)) in glint_offsets.iter().enumerate() {
        let p_world = Vec3::new(gx, gy, gz);
        let rel = p_world - cam.target;
        let rot = view_rot.transform_point(rel);
        let p_cam = Vec3::new(rot.x, rot.y, rot.z + cam.dist);
        if p_cam.z > 10.0 {
            let sx = center.x + (p_cam.x * cam.focal) / p_cam.z;
            let sy = center.y - (p_cam.y * cam.focal) / p_cam.z;
            let phase = time * 4.0 + (i as f32 * 2.1);
            let size = 6.0 + (phase.sin() * 3.5).abs();
            p.circle_filled(pos2(sx, sy), size * 0.5, Color32::WHITE);
            p.line_segment(
                [pos2(sx - size, sy), pos2(sx + size, sy)],
                Stroke::new(1.8, Color32::WHITE),
            );
            p.line_segment(
                [pos2(sx, sy - size), pos2(sx, sy + size)],
                Stroke::new(1.8, Color32::WHITE),
            );
        }
    }

    // 7. 曲率超速动量粒子线 (Warp Speed Streaks for Step 2)
    if is_warp {
        for i in 0..16 {
            let a = (i as f32) * 2.0 * PI / 16.0;
            let speed_t = (time * 6.0 + (i as f32 * 0.4)) % 1.0;
            let r_start = 40.0 + speed_t * 90.0;
            let r_end = r_start + 18.0;
            let p1 = pos2(center.x + a.cos() * r_start, center.y + a.sin() * r_start);
            let p2 = pos2(center.x + a.cos() * r_end, center.y + a.sin() * r_end);
            p.line_segment(
                [p1, p2],
                Stroke::new(2.5, Color32::from_rgb(0, 240, 255)),
            );
        }
    }
}
