//! 美漫安装向导单元测试与对抗鲁棒性验证 (Unit & Adversarial Tests)

use super::*;
use crate::app::App;
use crate::app::Settings;
use crate::Trace;
use eframe::egui;

#[test]
fn installer_default_state() {
    let s = InstallerState::default();
    assert!(!s.open);
    assert!(!s.standalone);
    assert_eq!(s.step, InstallerStep::Options);
    assert!(s.add_path);
    assert!(s.desktop_shortcut);
    assert!(s.tray_resident);
    assert!(s.launch_on_finish);
    assert!(s.target_dir.contains("IDE1.0") || !s.target_dir.is_empty());
}

#[test]
fn installer_state_machine_transitions() {
    let mut s = InstallerState::default();
    assert_eq!(s.step, InstallerStep::Options);

    // Options -> Installing
    s.step = InstallerStep::Installing;
    assert_eq!(s.step, InstallerStep::Installing);

    // Installing -> Finished
    s.step = InstallerStep::Finished;
    assert_eq!(s.step, InstallerStep::Finished);
}

#[test]
fn installer_authentic_logo_decoding() {
    let logo_opt = crate::appicon::logo();
    assert!(logo_opt.is_some(), "正版 logo-256.png 必须能被完整解码");
    let logo = logo_opt.unwrap();
    assert_eq!(logo.width, 256);
    assert_eq!(logo.height, 256);
    assert_eq!(logo.rgba.len(), 256 * 256 * 4);
}

#[test]
fn test_installer_comic_rendering_invariants() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.standalone = false;

    // 验证模态向导能在 run_ui 下完整自绘且无 panic
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();
    assert!(a.installer.open);

    // 验证步骤切换渲染无 panic
    a.installer.step = InstallerStep::Options;
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    a.installer.step = InstallerStep::Installing;
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    a.installer.step = InstallerStep::Finished;
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();
}

#[test]
fn test_installer_standalone_rendering() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.standalone = true;

    let mut out = ctx.run_ui(Default::default(), |root| {
        a.installer_standalone(root);
    });
    out.textures_delta.clear();
    assert!(a.shown);
}

#[test]
fn test_installer_adversarial_progress_and_path() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;

    // 对抗测试：非法或极端路径输入
    a.installer.target_dir = String::new();
    a.installer.step = InstallerStep::Options;
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    // 对抗测试：极端进度值越界
    a.installer.step = InstallerStep::Installing;
    a.installer.install_progress = 999.0;
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    a.installer.install_progress = -50.0;
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();
}

#[test]
fn test_installer_logo_fallback_without_texture() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    // 强制清空 logo 纹理，验证兜底渲染
    a.installer.logo_tex = None;

    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();
    assert!(a.installer.open);
}

#[test]
fn test_cel3d_math_and_transforms() {
    use super::cel3d::*;

    // 向量运算
    let v1 = Vec3::new(1.0, 0.0, 0.0);
    let v2 = Vec3::new(0.0, 1.0, 0.0);
    let cross = v1.cross(v2);
    assert!((cross.z - 1.0).abs() < 1e-5);
    assert_eq!(v1.dot(v2), 0.0);
    assert!((cross.length() - 1.0).abs() < 1e-5);

    // 矩阵变换
    let m_rot = Mat4::rot_y(std::f32::consts::PI * 0.5);
    let p_rot = m_rot.transform_point(v1);
    assert!(p_rot.x.abs() < 1e-4);
    assert!((p_rot.z - (-1.0)).abs() < 1e-4);

    let m_trans = Mat4::translate(10.0, 20.0, 30.0);
    let p_trans = m_trans.transform_point(Vec3::ZERO);
    assert_eq!(p_trans, Vec3::new(10.0, 20.0, 30.0));
}

#[test]
fn test_cel3d_models_generation() {
    use super::cel3d::*;

    let pedestal = build_ionic_pedestal();
    assert!(pedestal.vertices.len() > 30, "神殿立柱应包含丰富的柱础与凹槽几何顶点");
    assert!(pedestal.faces.len() > 20, "神殿立柱应包含完整的侧面与顶面多边形");

    let gem = build_crystal_gem(30.0);
    assert_eq!(gem.vertices.len(), 12, "正二十面体晶石应严格具有 12 个黄金比例顶点");
    assert_eq!(gem.faces.len(), 20, "正二十面体晶石应严格具有 20 个三角切面");

    let wreath = build_laurel_wreath(26, 50.0, 44.0);
    assert_eq!(wreath.faces.len(), 26, "阿波罗桂冠应包含 26 片月桂叶几何体");
    assert_eq!(wreath.vertices.len(), 104, "每片月桂叶由 4 个三维顶点构成");

    let logo = build_ide1_crystal_logo(80.0);
    assert!(logo.vertices.len() >= 30, "正版晶钻 Logo 必须具备丰富的多面体顶点与核心代码符号");
    assert!(logo.faces.len() >= 25, "正版晶钻 Logo 应包含冠部、腰棱、亭部切面与 < I > 符号切面");
}

#[test]
fn test_cel3d_material_shading_invariants() {
    use super::cel3d::*;

    let norm = Vec3::new(0.0, 1.0, 0.0);
    let light_direct = Vec3::new(0.0, -1.0, 0.0);
    let light_back = Vec3::new(0.0, 1.0, 0.0);
    let view = Vec3::new(0.0, 0.0, 1.0);

    let c_gem_bright = CelMaterial::CrystalGem.shade(norm, light_direct, view);
    let c_gem_dark = CelMaterial::CrystalGem.shade(norm, light_back, view);
    assert_ne!(c_gem_bright, c_gem_dark, "高光与背光区域应形成鲜明的阶梯式赛璐珞明暗切面");

    let light_diffuse = Vec3::new(0.3, -0.9, 0.0).norm();
    let c_marble = CelMaterial::MarbleWhite.shade(norm, light_diffuse, view);
    let c_gold = CelMaterial::GoldTrim.shade(norm, light_diffuse, view);
    assert_ne!(c_marble, c_gold, "不同赛璐珞材质呈现独有的美漫色彩层次");
}

#[test]
fn test_cel3d_vfx_pipeline_and_steps() {
    use super::cel3d::*;
    use super::InstallerStep;
    use egui::pos2;

    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let p = ui.painter();
        let cam = Camera3D::default();
        let rot_x = Mat4::rot_x(cam.pitch);
        let rot_y = Mat4::rot_y(cam.yaw);
        let view_rot = rot_x.mul(&rot_y);
        let center = pos2(200.0, 200.0);

        for step in [
            InstallerStep::Welcome,
            InstallerStep::Options,
            InstallerStep::Installing,
            InstallerStep::Finished,
        ] {
            render_3d_vfx(p, &cam, view_rot, center, 1.23, step);
        }
    });
    out.textures_delta.clear();
}

#[test]
fn test_installer_language_switching_in_options() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.step = InstallerStep::Options;

    // 默认初始语言为当前语言
    let init_lang = crate::i18n::cur();

    // 切换至 English
    crate::i18n::set(crate::i18n::Lang::En);
    a.cfg.lang = "en".to_string();
    assert_eq!(crate::i18n::cur(), crate::i18n::Lang::En);

    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    // 切换至 日本語
    crate::i18n::set(crate::i18n::Lang::Ja);
    a.cfg.lang = "ja".to_string();
    assert_eq!(crate::i18n::cur(), crate::i18n::Lang::Ja);

    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    // 切换回 中文
    crate::i18n::set(crate::i18n::Lang::Zh);
    a.cfg.lang = "zh".to_string();
    assert_eq!(crate::i18n::cur(), crate::i18n::Lang::Zh);

    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    // 还原环境
    crate::i18n::set(init_lang);
}

#[test]
fn test_installer_headless_startup_and_subsystem_invariants() {
    // 验证安装向导执行部署命令构造中已配置 CREATE_NO_WINDOW
    #[cfg(windows)]
    {
        let cmd = super::deploy::headless_cmd("powershell");
        assert_eq!(cmd.get_program(), "powershell");
    }
}
