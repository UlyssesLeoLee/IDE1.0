//! 经典美漫与赛璐珞风格真 3D 渲染微内核 (True 3D Cel-Shaded Engine)
//! 具备完整的 3D 几何管线、透视投影、3D 阶梯式赛璐珞多色阶光照 (Cel-Shading Bands)、
//! 纯黑美漫墨线外轮廓 (Ink Outlines)、画师算法深度排序与高光星芒粒子。

use eframe::egui;
use egui::{pos2, Color32, Painter, Pos2, Rect, Stroke};

pub use super::cel3d_models::*;
pub use super::cel3d_vfx::*;
use super::InstallerStep;

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

/// 赛璐珞材质类型与多色阶量化
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CelMaterial {
    MarbleWhite,
    GoldTrim,
    CrystalGem,
    LaurelLeaf,
    DarkPedestal,
    /// IDE1.0 正版水晶钻石徽标: 上冠摄光天青切面
    CrystalDiamondLight,
    /// IDE1.0 正版水晶钻石徽标: 腰棱皇家宝蓝切面
    CrystalDiamondMid,
    /// IDE1.0 正版水晶钻石徽标: 亭部黑曜钴蓝切面
    CrystalDiamondDark,
    /// IDE1.0 正版水晶核心: 恒光高亮天青 < I > 代码符号
    CrystalCoreGlyph,
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
            Self::CrystalDiamondLight => {
                // 上冠切面: 摄光高透天青/冰晶蓝 (#a5f3fc -> #38bdf8 -> #0284c7)
                if spec > 0.80 {
                    Color32::from_rgb(245, 253, 255)
                } else if dot > 0.40 {
                    Color32::from_rgb(165, 243, 252)
                } else if dot > 0.08 {
                    Color32::from_rgb(56, 189, 248)
                } else {
                    Color32::from_rgb(2, 132, 199)
                }
            }
            Self::CrystalDiamondMid => {
                // 腰棱切面: 皇家宝蓝/深空靛蓝 (#38bdf8 -> #0284c7 -> #0369a1)
                if spec > 0.85 {
                    Color32::from_rgb(224, 242, 254)
                } else if dot > 0.38 {
                    Color32::from_rgb(56, 189, 248)
                } else if dot > 0.02 {
                    Color32::from_rgb(2, 132, 199)
                } else {
                    Color32::from_rgb(3, 105, 161)
                }
            }
            Self::CrystalDiamondDark => {
                // 亭部切面: 深邃黑曜钴蓝 (#0284c7 -> #0369a1 -> #0d2242)
                if spec > 0.88 {
                    Color32::from_rgb(186, 230, 253)
                } else if dot > 0.35 {
                    Color32::from_rgb(2, 132, 199)
                } else if dot > -0.10 {
                    Color32::from_rgb(3, 105, 161)
                } else {
                    Color32::from_rgb(13, 34, 66)
                }
            }
            Self::CrystalCoreGlyph => {
                // 核心 < I > 代码符号: 恒光高亮天青白 (#f0fdfa / #a5f3fc)
                if dot > 0.0 {
                    Color32::from_rgb(245, 253, 255)
                } else {
                    Color32::from_rgb(186, 246, 255)
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

struct RenderPolygon {
    pts: Vec<Pos2>,
    depth: f32,
    color: Color32,
}

fn collect_mesh_polys(
    mesh: &Mesh3D,
    model_mat: Option<&Mat4>,
    cam: &Camera3D,
    view_rot: Mat4,
    center: Pos2,
    out: &mut Vec<RenderPolygon>,
) {
    let light_dir = Vec3::new(0.5, 0.75, -0.6).norm();
    let light_cam = view_rot.transform_dir(light_dir);
    let view_dir = Vec3::new(0.0, 0.0, -1.0);

    let cam_verts: Vec<Vec3> = mesh
        .vertices
        .iter()
        .map(|v| {
            let w = match model_mat {
                Some(m) => m.transform_point(*v),
                None => *v,
            };
            let rot = view_rot.transform_point(w - cam.target);
            Vec3::new(rot.x, rot.y, rot.z + cam.dist)
        })
        .collect();

    for face in &mesh.faces {
        if face.indices.len() < 3 {
            continue;
        }
        let p0 = cam_verts[face.indices[0]];
        let p1 = cam_verts[face.indices[1]];
        let p2 = cam_verts[face.indices[2]];
        let n = (p1 - p0).cross(p2 - p0).norm();
        if n.dot((-p0).norm()) <= 0.01 {
            continue;
        }

        let col = face.material.shade(n, light_cam, view_dir);
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
            out.push(RenderPolygon {
                pts: screen_pts,
                depth,
                color: col,
            });
        }
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
    step: InstallerStep,
) {
    let center = rect.center();
    let rot_x = Mat4::rot_x(cam.pitch);
    let rot_y = Mat4::rot_y(cam.yaw);
    let view_rot = rot_x.mul(&rot_y);

    // 1. 优先绘制深层 3D 华丽背景特效 (放射状光芒、能量双环、星尘粒子、曲速/庆典特效)
    render_3d_vfx(p, cam, view_rot, center, time, step);

    let mut render_list = Vec::with_capacity(300);
    collect_mesh_polys(pedestal, None, cam, view_rot, center, &mut render_list);
    collect_mesh_polys(
        gem,
        Some(gem_model_mat),
        cam,
        view_rot,
        center,
        &mut render_list,
    );
    collect_mesh_polys(
        wreath,
        Some(wreath_model_mat),
        cam,
        view_rot,
        center,
        &mut render_list,
    );

    // 画师算法深度排序
    render_list.sort_by(|a, b| b.depth.total_cmp(&a.depth));

    // 绘制多边形与美漫粗黑外轮廓
    for poly in render_list {
        p.add(egui::Shape::convex_polygon(
            poly.pts,
            poly.color,
            Stroke::new(2.0, Color32::BLACK),
        ));
    }
}

/// 专属渲染 IDE1.0 正版水晶钻石徽标真 3D 赛璐珞场景 (低调奢华 · 重点突出 · 0 冗余干扰)
pub fn render_cel3d_crystal_scene(
    p: &Painter,
    rect: Rect,
    cam: &Camera3D,
    logo_mesh: &Mesh3D,
    logo_model_mat: &Mat4,
    time: f32,
) {
    let center = rect.center();
    let rot_x = Mat4::rot_x(cam.pitch);
    let rot_y = Mat4::rot_y(cam.yaw);
    let view_rot = rot_x.mul(&rot_y);

    // 1. 低调奢华暗场氛围光晕 (Subtle Luxury Ambient Glow) - 柔和天青微光衬托晶钻轮廓
    draw_ambient_luxury_glow(p, center, time);

    // 2. 空间悬浮地面接触投影 (Grounding Contact Floor Shadow)
    draw_contact_floor_shadow(p, cam, view_rot, center, -115.0, time);

    // 3. 收集 3D 钻石网格多边形
    let mut render_list = Vec::with_capacity(120);
    collect_mesh_polys(
        logo_mesh,
        Some(logo_model_mat),
        cam,
        view_rot,
        center,
        &mut render_list,
    );

    // 4. 画师算法深度排序 (Painter's Algorithm Depth Sort)
    render_list.sort_by(|a, b| b.depth.total_cmp(&a.depth));

    // 5. 绘制赛璐珞多边形切面与美漫纯黑粗墨线外边框 (1.8px)
    for poly in render_list {
        p.add(egui::Shape::convex_polygon(
            poly.pts,
            poly.color,
            Stroke::new(1.8, Color32::from_rgb(10, 15, 26)),
        ));
    }
}
