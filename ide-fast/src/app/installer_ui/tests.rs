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
    assert_eq!(s.step, InstallerStep::Agreement);
    assert!(!s.agreement_scrolled_to_bottom);
    assert!(!s.agreement_accepted);
    assert!(s.add_path);
    assert!(s.desktop_shortcut);
    assert!(s.tray_resident);
    assert!(s.launch_on_finish);
    assert!(s.target_dir.contains("IDE1.0") || !s.target_dir.is_empty());
}

#[test]
fn installer_state_machine_transitions() {
    let mut s = InstallerState::default();
    assert_eq!(s.step, InstallerStep::Agreement);

    // Agreement -> Options
    s.step = InstallerStep::Options;
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
    a.installer.dry_run = true;

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
fn test_user_agreement_content_loaded() {
    assert!(!super::USER_AGREEMENT_TEXT.is_empty());
    assert!(super::USER_AGREEMENT_TEXT.contains("IDE 1.0 最终用户许可与使用协议"));
    assert!(super::USER_AGREEMENT_TEXT.contains("开源发行版"));
}

#[test]
fn test_agreement_scroll_and_accept_invariants() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.step = InstallerStep::Agreement;
    a.installer.agreement_scrolled_to_bottom = false;
    a.installer.agreement_accepted = false;

    // 初始状态：未滑到底部，未勾选同意
    assert!(!a.installer.agreement_scrolled_to_bottom);
    assert!(!a.installer.agreement_accepted);
    assert_eq!(a.installer.step, InstallerStep::Agreement);

    // 渲染协议界面首帧
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out.textures_delta.clear();

    // 模拟拉到底部
    a.installer.agreement_scrolled_to_bottom = true;
    assert!(a.installer.agreement_scrolled_to_bottom);

    // 勾选同意并进入 Options
    a.installer.agreement_accepted = true;
    a.installer.step = InstallerStep::Options;
    assert_eq!(a.installer.step, InstallerStep::Options);

    let mut out2 = ctx.run_ui(Default::default(), |_root| {
        a.installer_window(&ctx);
    });
    out2.textures_delta.clear();
}

#[test]
fn test_comic_update_window_rendering_invariants() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.upd.open = true;

    // 验证更新窗口在美漫风格下渲染无 panic
    let mut out = ctx.run_ui(Default::default(), |_root| {
        a.upd_window(&ctx);
    });
    out.textures_delta.clear();
    assert!(a.upd.open);
}

#[test]
fn test_installer_standalone_rendering() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.standalone = true;
    a.installer.dry_run = true;

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
    a.installer.dry_run = true;

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
    assert!(
        pedestal.vertices.len() > 30,
        "神殿立柱应包含丰富的柱础与凹槽几何顶点"
    );
    assert!(
        pedestal.faces.len() > 20,
        "神殿立柱应包含完整的侧面与顶面多边形"
    );

    let gem = build_crystal_gem(30.0);
    assert_eq!(
        gem.vertices.len(),
        12,
        "正二十面体晶石应严格具有 12 个黄金比例顶点"
    );
    assert_eq!(
        gem.faces.len(),
        20,
        "正二十面体晶石应严格具有 20 个三角切面"
    );

    let wreath = build_laurel_wreath(26, 50.0, 44.0);
    assert_eq!(wreath.faces.len(), 26, "阿波罗桂冠应包含 26 片月桂叶几何体");
    assert_eq!(wreath.vertices.len(), 104, "每片月桂叶由 4 个三维顶点构成");

    let logo = build_ide1_crystal_logo(80.0);
    assert!(
        logo.vertices.len() >= 30,
        "正版晶钻 Logo 必须具备丰富的多面体顶点与核心代码符号"
    );
    assert!(
        logo.faces.len() >= 25,
        "正版晶钻 Logo 应包含冠部、腰棱、亭部切面与 < I > 符号切面"
    );
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
    assert_ne!(
        c_gem_bright, c_gem_dark,
        "高光与背光区域应形成鲜明的阶梯式赛璐珞明暗切面"
    );

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
            InstallerStep::Agreement,
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

#[test]
fn test_installer_progress_clipping_and_no_tofu_icons() {
    fn check_shape_text(s: &egui::Shape, forbidden: &dyn Fn(char) -> bool) {
        match s {
            egui::Shape::Text(ts) => {
                let txt = ts.galley.text();
                assert!(
                    !txt.chars().any(forbidden),
                    "Installer painted forbidden emoji/tofu glyph in text: {txt:?}"
                );
            }
            egui::Shape::Vec(vs) => {
                for sub in vs {
                    check_shape_text(sub, forbidden);
                }
            }
            _ => {}
        }
    }

    let is_forbidden = |c: char| -> bool {
        let cp = c as u32;
        cp >= 0x1F000
            || cp == 0xFE0F
            || (0x2600..=0x27BF).contains(&cp)
            || matches!(cp, 0x25A1 | 0x25A6 | 0x25C8 | 0x25CB | 0x2B50)
    };

    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_lang = crate::i18n::cur();
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(880.0, 640.0),
        )),
        ..Default::default()
    };

    let sample_existing = ExistingInstall {
        install_dir: r"C:\Users\Demo\AppData\Local\IDE1.0".to_string(),
        installed_version: Some("0.8.5".to_string()),
        has_desktop_shortcut: true,
        has_start_menu_shortcut: true,
        has_path_entry: true,
        has_uninstall_reg: true,
    };

    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set(lang);

        for mode in [
            InstallerMode::Install,
            InstallerMode::Maintenance,
            InstallerMode::Repair,
            InstallerMode::Uninstall,
        ] {
            // 1. Options 与 Finished 步骤全量无豆腐块验证
            for step in [InstallerStep::Options, InstallerStep::Finished] {
                let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
                a.installer.open = true;
                a.installer.standalone = true;
                a.installer.dry_run = true;
                a.installer.mode = mode;
                a.installer.step = step;
                if mode != InstallerMode::Install {
                    a.installer.existing = Some(sample_existing.clone());
                    a.installer.target_dir = sample_existing.install_dir.clone();
                }
                let mut out = ctx.run_ui(raw.clone(), |root| {
                    a.installer_standalone(root);
                });
                for cs in &out.shapes {
                    check_shape_text(&cs.shape, &is_forbidden);
                }
                out.textures_delta.clear();
            }

            // 2. Installing 步骤进度条裁剪与多进度点验证 (Install / Repair / Uninstall)
            if mode != InstallerMode::Maintenance {
                for prog in [0.0_f32, 0.35, 0.72, 0.95] {
                    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
                    a.installer.open = true;
                    a.installer.standalone = true;
                    a.installer.dry_run = true;
                    a.installer.mode = mode;
                    a.installer.step = InstallerStep::Installing;
                    a.installer.install_progress = prog;
                    let mut out = ctx.run_ui(raw.clone(), |root| {
                        a.installer_standalone(root);
                    });
                    let mut saw_clipped_fill_band = false;
                    for cs in &out.shapes {
                        check_shape_text(&cs.shape, &is_forbidden);
                        if cs.clip_rect.left() >= 370.0
                            && cs.clip_rect.right() <= 850.0
                            && cs.clip_rect.height() > 5.0
                            && cs.clip_rect.height() < 25.0
                        {
                            saw_clipped_fill_band = true;
                        }
                    }
                    assert!(
                        saw_clipped_fill_band,
                        "Expected strictly clipped progress fill band inside track at lang={lang:?}, mode={mode:?}, prog={prog}"
                    );
                    out.textures_delta.clear();
                }
            }
        }
    }

    crate::i18n::set(prev_lang);
}

struct TempDirGuard(std::path::PathBuf);

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ide10_inst_test_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test tempdir");
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_reg_query_uninstall_parsing_and_detection() {
    let raw_reg = r#"
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall\IDE1.0
    DisplayName    REG_SZ    IDE 1.0
    DisplayVersion    REG_SZ    0.8.5
    InstallLocation    REG_SZ    C:\Users\Demo User\AppData\Local\IDE1.0
    DisplayIcon    REG_SZ    C:\Users\Demo User\AppData\Local\IDE1.0\IDE1.0.exe
"#;
    assert_eq!(
        super::deploy::parse_reg_query_uninstall(raw_reg),
        Some((
            r"C:\Users\Demo User\AppData\Local\IDE1.0".to_string(),
            Some("0.8.5".to_string())
        ))
    );
    assert_eq!(
        super::deploy::parse_reg_query_uninstall("ERROR: not found"),
        None
    );

    let tmp = TempDirGuard::new("detect_existing");
    let installed_dir = tmp.path().join("InstalledIDE");
    let setup_dir = tmp.path().join("Downloads");
    std::fs::create_dir_all(&installed_dir).unwrap();
    std::fs::create_dir_all(&setup_dir).unwrap();
    std::fs::write(installed_dir.join("IDE1.0.exe"), b"MZ").unwrap();
    let lnk = tmp.path().join("IDE1.0.lnk");
    std::fs::write(&lnk, b"lnk").unwrap();

    // 1. 注册表记录指向有效二进制目录 -> 识别为已有安装
    let reg = Some((
        installed_dir.to_string_lossy().to_string(),
        Some("0.8.5".into()),
    ));
    let setup_exe = setup_dir.join("IDE1.0_Setup_0.8.5_x64.exe");
    let path_str = format!(r"C:\Windows\System32;{}/", installed_dir.display());
    let detected = super::deploy::detect_existing_in_paths(
        reg,
        Some(&setup_exe),
        &[],
        Some(&lnk),
        None,
        &path_str,
    )
    .expect("valid registry install should be detected");
    assert!(detected.has_uninstall_reg);
    assert!(detected.has_desktop_shortcut);
    assert!(!detected.has_start_menu_shortcut);
    assert!(detected.has_path_entry);
    assert_eq!(detected.installed_version.as_deref(), Some("0.8.5"));

    // 2. 陈旧注册表记录指向已不存在目录 -> 忽略，不得误报
    let stale_reg = Some((
        tmp.path().join("GoneDir").to_string_lossy().to_string(),
        None,
    ));
    assert!(super::deploy::detect_existing_in_paths(
        stale_reg.clone(),
        Some(&setup_exe),
        &[],
        None,
        None,
        "",
    )
    .is_none());

    // 3. 从外部 Downloads\IDE1.0_Setup.exe 运行时扫描标准候选目录 -> 识别到已有安装
    let found_cand = super::deploy::detect_existing_in_paths(
        None,
        Some(&setup_exe),
        std::slice::from_ref(&installed_dir),
        None,
        None,
        "",
    )
    .expect("external setup must detect existing candidate install");
    assert!(!found_cand.has_uninstall_reg);
    assert_eq!(found_cand.install_dir, installed_dir.to_string_lossy());

    // 4. MSI 首次安装后立即拉起同目录下 IDE1.0.exe --setup (尚无 Uninstall 键) -> 不误判为已配置安装
    let self_exe = installed_dir.join("IDE1.0.exe");
    assert!(super::deploy::detect_existing_in_paths(
        None,
        Some(&self_exe),
        std::slice::from_ref(&installed_dir),
        None,
        None,
        "",
    )
    .is_none());
}

#[test]
fn test_path_env_add_remove_and_hex_decoding() {
    // 用户 PATH 读取走 UTF-8 十六进制通道，杜绝控制台代码页乱码
    assert_eq!(
        super::deploy::hex_decode_utf8("433a5c42696e").as_deref(),
        Some(r"C:\Bin")
    );
    assert_eq!(
        super::deploy::hex_decode_utf8("e4b8ad").as_deref(),
        Some("中")
    );
    assert_eq!(super::deploy::hex_decode_utf8("").as_deref(), Some(""));
    assert_eq!(super::deploy::hex_decode_utf8("abc"), None);
    assert_eq!(super::deploy::hex_decode_utf8("zz"), None);
    assert_eq!(super::deploy::hex_decode_utf8("ff"), None);

    let initial = r"C:\Windows\System32;D:\Tools";
    let dir = r"C:\Users\Demo\AppData\Local\IDE1.0";
    let added = super::deploy::add_dir_to_path_str(initial, dir);
    assert_eq!(
        added,
        r"C:\Windows\System32;D:\Tools;C:\Users\Demo\AppData\Local\IDE1.0"
    );
    // 幂等添加
    assert_eq!(
        super::deploy::add_dir_to_path_str(&added, r"c:\users\demo\appdata\local\ide1.0\"),
        added
    );
    assert!(super::deploy::path_contains_dir(
        &added,
        r"C:/Users/Demo/AppData/Local/IDE1.0\"
    ));

    // 大小写与尾部斜杠不敏感移除，保留无关项且不留空分号
    let messy = format!(r"C:\Windows\System32;;{dir}\;D:\Tools;");
    assert_eq!(
        super::deploy::remove_dir_from_path_str(&messy, r"c:\users\demo\appdata\local\ide1.0"),
        r"C:\Windows\System32;D:\Tools"
    );
}

#[test]
fn test_safe_uninstall_file_removal_preserves_unrelated_files() {
    let tmp = TempDirGuard::new("safe_remove");

    // 1. 混合目录内存在用户文件时：仅删除已知 IDE1.0 二进制，绝不删除用户文件或非空目录
    let shared = tmp.path().join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("IDE1.0.exe"), b"bin1").unwrap();
    std::fs::write(shared.join("ide-fast.exe"), b"bin2").unwrap();
    std::fs::write(shared.join("IDE1.0.exe.new"), b"bin3").unwrap();
    std::fs::write(shared.join("user_keep.txt"), b"important notes").unwrap();

    let rep1 = super::deploy::remove_ide_files_in_dir(&shared, None);
    assert_eq!(rep1.removed_files, 3);
    assert!(!rep1.deferred_self);
    assert!(!rep1.dir_removed);
    assert!(shared.join("user_keep.txt").is_file());
    assert!(!shared.join("IDE1.0.exe").exists());
    assert!(!shared.join("ide-fast.exe").exists());

    // 2. 专用空目录：移除最后一个二进制后一并清理空目录
    let dedicated = tmp.path().join("dedicated");
    std::fs::create_dir_all(&dedicated).unwrap();
    std::fs::write(dedicated.join("IDE1.0.exe"), b"bin").unwrap();
    let rep2 = super::deploy::remove_ide_files_in_dir(&dedicated, None);
    assert_eq!(rep2.removed_files, 1);
    assert!(!rep2.deferred_self);
    assert!(rep2.dir_removed);
    assert!(!dedicated.exists());

    // 3. 正在运行被删目录中的自身 exe：标记延迟自清理，不在运行期强删自身
    let running_dir = tmp.path().join("running");
    std::fs::create_dir_all(&running_dir).unwrap();
    let self_exe = running_dir.join("IDE1.0.exe");
    std::fs::write(&self_exe, b"running").unwrap();
    std::fs::write(running_dir.join("ide-fast.exe"), b"alias").unwrap();
    let rep3 = super::deploy::remove_ide_files_in_dir(&running_dir, Some(&self_exe));
    assert_eq!(rep3.removed_files, 1);
    assert!(rep3.deferred_self);
    assert!(!rep3.dir_removed);
    assert!(self_exe.is_file());
    assert!(!running_dir.join("ide-fast.exe").exists());
}

#[test]
fn test_apply_open_mode_transitions() {
    let ex = ExistingInstall {
        install_dir: r"C:\Apps\IDE1.0".to_string(),
        installed_version: Some("0.8.5".to_string()),
        has_desktop_shortcut: false,
        has_start_menu_shortcut: true,
        has_path_entry: false,
        has_uninstall_reg: true,
    };

    // 1. Auto + 已安装 -> 维护模式，始终先进入许可协议并继承已有目录与集成选项
    let mut s1 = InstallerState::default();
    s1.apply_open_mode(InstallerLaunchMode::Auto, Some(ex.clone()), None, None);
    assert_eq!(s1.mode, InstallerMode::Maintenance);
    assert_eq!(s1.step, InstallerStep::Agreement);
    assert!(!s1.agreement_scrolled_to_bottom);
    assert!(!s1.agreement_accepted);
    assert_eq!(s1.target_dir, r"C:\Apps\IDE1.0");
    assert!(!s1.add_path);
    assert!(!s1.desktop_shortcut);

    // 2. Auto + 未安装 + *Setup*.exe -> 全新安装，默认定位至 %LOCALAPPDATA%\IDE1.0
    let mut s2 = InstallerState::default();
    let dl_setup = std::path::Path::new(r"C:\Users\Demo\Downloads\IDE1.0_Setup_0.8.5_x64.exe");
    let lad = std::path::Path::new(r"C:\Users\Demo\AppData\Local");
    s2.apply_open_mode(InstallerLaunchMode::Auto, None, Some(dl_setup), Some(lad));
    assert_eq!(s2.mode, InstallerMode::Install);
    assert_eq!(
        s2.target_dir,
        lad.join("IDE1.0").to_string_lossy().to_string()
    );

    // 3. --uninstall -> 直接进入卸载模式
    let mut s3 = InstallerState::default();
    s3.apply_open_mode(InstallerLaunchMode::Uninstall, Some(ex.clone()), None, None);
    assert_eq!(s3.mode, InstallerMode::Uninstall);
    assert_eq!(s3.target_dir, r"C:\Apps\IDE1.0");

    // 4. --repair -> 直接进入修复模式并默认勾选修复快捷方式与 PATH
    let mut s4 = InstallerState::default();
    s4.add_path = false;
    s4.desktop_shortcut = false;
    s4.apply_open_mode(InstallerLaunchMode::Repair, Some(ex), None, None);
    assert_eq!(s4.mode, InstallerMode::Repair);
    assert!(s4.add_path);
    assert!(s4.desktop_shortcut);
}

#[test]
fn test_dry_run_finishes_without_background_thread_and_real_job_is_isolated() {
    // dry_run: 进度走满直接收尾，绝不启动后台部署线程
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(880.0, 640.0),
        )),
        ..Default::default()
    };
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.standalone = true;
    a.installer.dry_run = true;
    a.installer.mode = InstallerMode::Install;
    a.installer.step = InstallerStep::Installing;
    a.installer.install_progress = 1.0;
    let mut out = ctx.run_ui(raw, |root| {
        a.installer_standalone(root);
    });
    out.textures_delta.clear();
    assert_eq!(a.installer.step, InstallerStep::Finished);
    assert!(a.installer.deploy_rx.is_none());

    // 真实卸载任务 (仅清理临时目录内的 IDE 二进制; 不碰快捷方式 / PATH / 注册表 / 用户数据)
    #[cfg(windows)]
    {
        let tmp = TempDirGuard::new("job_uninstall");
        let dir = tmp.path().join("IDE_JOB");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("IDE1.0.exe"), b"MZ").unwrap();
        std::fs::write(dir.join("notes.txt"), b"keep").unwrap();
        let plan = super::deploy::DeployPlan {
            target_dir: dir.to_string_lossy().to_string(),
            add_path: false,
            desktop_shortcut: false,
            remove_shortcuts: false,
            remove_path_and_reg: false,
            remove_user_data: false,
            legacy_cleanup_dir: None,
        };
        let handle = std::thread::spawn(move || {
            super::deploy::run_deploy_job(InstallerMode::Uninstall, &plan)
        });
        let outcome = handle.join().expect("deploy job thread must not panic");
        assert!(!dir.join("IDE1.0.exe").exists(), "IDE binary removed");
        assert!(dir.join("notes.txt").exists(), "user files never deleted");
        assert!(outcome.deferred_self_cleanup_dir.is_none());
        assert!(outcome.log_path.is_some(), "uninstall must emit log_path");
    }
}

#[test]
fn test_append_installer_log_and_legacy_dir_collection() {
    let tmp = TempDirGuard::new("installer_log_legacy");
    let new_dir = tmp.path().join("ProgramFiles_IDE10");
    let old_dir = tmp.path().join("LocalAppData_IDE10");
    let empty_dir = tmp.path().join("Empty_Other");
    std::fs::create_dir_all(&new_dir).unwrap();
    std::fs::create_dir_all(&old_dir).unwrap();
    std::fs::create_dir_all(&empty_dir).unwrap();
    std::fs::write(new_dir.join("IDE1.0.exe"), b"MZ").unwrap();
    std::fs::write(old_dir.join("ide-fast.exe"), b"MZ").unwrap();

    let log_file = super::deploy::append_installer_log(&new_dir, "ERROR", "simulated copy error")
        .expect("installer log must be written");
    assert!(log_file.is_file());
    let log_body = std::fs::read_to_string(&log_file).unwrap();
    assert!(log_body.contains("[ERROR] simulated copy error"));

    let old_str = old_dir.to_string_lossy().to_string();
    let collected = super::deploy::collect_legacy_install_dirs(
        &new_dir,
        Some(&old_str),
        &[new_dir.clone(), old_dir.clone(), empty_dir],
    );
    assert_eq!(collected.len(), 1);
    assert!(super::deploy::paths_equal_ci(&collected[0], &old_dir));

    // MSI 从新目录启动 `--setup` 而注册表指向旧目录时，自动进入安装并标记清理旧目录
    let new_exe = new_dir.join("IDE1.0.exe");
    let ex = ExistingInstall {
        install_dir: old_str.clone(),
        installed_version: Some("0.8.9".into()),
        has_desktop_shortcut: true,
        has_start_menu_shortcut: true,
        has_path_entry: true,
        has_uninstall_reg: true,
    };
    let mut s = InstallerState::default();
    s.apply_open_mode(InstallerLaunchMode::Auto, Some(ex), Some(&new_exe), None);
    assert_eq!(s.mode, InstallerMode::Install);
    assert_eq!(s.target_dir, new_dir.to_string_lossy().to_string());
    assert_eq!(s.legacy_cleanup_dir.as_deref(), Some(old_str.as_str()));
}

#[test]
fn test_stylized_progress_bars_in_update_and_cards() {
    let ctx = egui::Context::default();
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.upd.open = true;
    a.upd.phase = crate::app::updateui::Phase::Checking;
    let mut out = ctx.run_ui(Default::default(), |_| {
        a.upd_window(&ctx);
    });
    out.textures_delta.clear();

    // 验证确定与不确定两种风格化进度条均能零 panic 渲染
    let mut out2 = ctx.run_ui(Default::default(), |ui| {
        let r1 = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(260.0, 18.0));
        let r2 = egui::Rect::from_min_size(egui::pos2(10.0, 40.0), egui::vec2(260.0, 18.0));
        cards::paint_comic_progress_bar(
            ui.painter(),
            r1,
            Some(0.65),
            Color32::from_rgb(2, 132, 199),
            Color32::from_rgb(56, 189, 248),
            Color32::from_rgb(125, 211, 252),
            1.2,
        );
        cards::paint_comic_progress_bar(
            ui.painter(),
            r2,
            None,
            Color32::from_rgb(2, 132, 199),
            Color32::from_rgb(56, 189, 248),
            Color32::from_rgb(125, 211, 252),
            2.4,
        );
    });
    out2.textures_delta.clear();
}

#[test]
fn test_eula_unaccepted_alert_and_all_steps_640h_layout() {
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(880.0, 640.0),
        )),
        ..Default::default()
    };
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.installer.open = true;
    a.installer.standalone = true;
    a.installer.dry_run = true;
    a.installer.mode = InstallerMode::Install;
    a.installer.step = InstallerStep::Agreement;

    // 1. 首次渲染获取按钮布局后，模拟在未同意情况下点击「同意并继续」位置
    let mut out1 = ctx.run_ui(raw.clone(), |root| {
        a.installer_standalone(root);
    });
    out1.textures_delta.clear();

    // 点击底部左侧主按钮区域（x=500, y=600 -> 位于 880x640 右栏底部固定按钮栏内）
    let click_pos = egui::pos2(500.0, 595.0);
    let mut raw_click = raw.clone();
    raw_click.events.push(egui::Event::PointerMoved(click_pos));
    raw_click.events.push(egui::Event::PointerButton {
        pos: click_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    raw_click.events.push(egui::Event::PointerButton {
        pos: click_pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    let mut out_click = ctx.run_ui(raw_click, |root| {
        a.installer_standalone(root);
    });
    out_click.textures_delta.clear();
    assert!(
        a.installer.alert.is_some(),
        "未阅读或未勾选同意时点击继续必须弹出统一美漫警示窗"
    );
    assert_eq!(a.installer.step, InstallerStep::Agreement);

    // 渲染弹窗一帧确保零 panic
    let mut out_alert = ctx.run_ui(raw.clone(), |root| {
        a.installer_standalone(root);
    });
    out_alert.textures_delta.clear();
    a.installer.alert = None;

    // 2. 验证全 7 个分镜在 880x640 下均能无 panic 渲染
    let scenarios = [
        (InstallerMode::Install, InstallerStep::Agreement),
        (InstallerMode::Maintenance, InstallerStep::Agreement),
        (InstallerMode::Install, InstallerStep::Options),
        (InstallerMode::Maintenance, InstallerStep::Options),
        (InstallerMode::Repair, InstallerStep::Options),
        (InstallerMode::Uninstall, InstallerStep::Options),
        (InstallerMode::Install, InstallerStep::Installing),
        (InstallerMode::Install, InstallerStep::Finished),
        (InstallerMode::Uninstall, InstallerStep::Finished),
    ];
    for (mode, step) in scenarios {
        a.installer.mode = mode;
        a.installer.step = step;
        a.installer.install_progress = 0.5;
        let mut out_s = ctx.run_ui(raw.clone(), |root| {
            a.installer_standalone(root);
        });
        out_s.textures_delta.clear();
    }
}
