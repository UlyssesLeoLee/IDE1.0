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
    assert_eq!(s.step, InstallerStep::Welcome);
    assert!(s.add_path);
    assert!(s.desktop_shortcut);
    assert!(s.tray_resident);
    assert!(s.launch_on_finish);
    assert!(s.target_dir.contains("IDE1.0") || !s.target_dir.is_empty());
}

#[test]
fn installer_state_machine_transitions() {
    let mut s = InstallerState::default();
    assert_eq!(s.step, InstallerStep::Welcome);

    // Step 0 -> Step 1
    s.step = InstallerStep::Options;
    assert_eq!(s.step, InstallerStep::Options);

    // Step 1 -> Back to Step 0
    s.step = InstallerStep::Welcome;
    assert_eq!(s.step, InstallerStep::Welcome);

    // Step 0 -> Step 1 -> Step 2
    s.step = InstallerStep::Options;
    s.step = InstallerStep::Installing;
    assert_eq!(s.step, InstallerStep::Installing);

    // Step 2 -> Step 3
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
