//! 「检查更新」窗口 (帮助菜单 / 命令面板 / 可选的每日启动检查)。联网逻辑在 `updnet.rs`, 纯规则在 `update.rs`, 安装在 `updapply.rs`,
//! 这里只管阶段状态机、一次性后台线程 (用完即退, 不常驻、不轮询) 和窗口。流程: 检查 -> (用户点)下载并校验 -> (用户点)安装并重启。
//! 任何一步失败: 当前版本不动, 显示原因和发布页链接。

use super::*;
use crate::app::installer_ui::art::*;
use crate::app::installer_ui::cards::*;
use crate::ui_icons::UiIcon;
use crate::updapply::{self, Apply};
use crate::update::{self, Channel, InstallKind, Source};
use crate::updnet::{self, Candidate, Curl, Outcome, Staged, UpdErr};
use egui::{pos2, vec2, Color32, Rect, Stroke, StrokeKind, Vec2};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Box<Candidate>),
    Downloading(Box<Candidate>),
    Ready(Box<Candidate>, Staged),
    Failed(String),
}

enum Msg {
    Checked(Result<Outcome, UpdErr>),
    Staged(Result<Staged, UpdErr>),
}

pub(super) struct UpdState {
    pub open: bool,
    phase: Phase,
    rx: Option<Receiver<Msg>>,
    /// 这次检查是启动时自动做的: 没有新版本 / 出错就不打扰
    silent: bool,
    auto_done: bool,
    /// 启动时刻记录: 启动后 90 秒内绝对不发起后台更新请求, 确保记事本秒开秒关完全零网络
    start_instant: std::time::Instant,
    /// 失败时给用户的页面 (有候选版本就是它的发布页)
    page: String,
}

impl Default for UpdState {
    fn default() -> Self {
        Self {
            open: false,
            phase: Phase::Idle,
            rx: None,
            silent: false,
            auto_done: false,
            start_instant: std::time::Instant::now(),
            page: String::new(),
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn install_kind() -> InstallKind {
    std::env::current_exe()
        .map(|e| InstallKind::of_exe(&e))
        .unwrap_or(InstallKind::Portable)
}

impl App {
    /// 自动更新检查延迟 (秒): 启动前 90 秒内绝对不发起任何后台网络请求, 确保记事本秒开秒关完全零网络交互
    const AUTO_UPDATE_DELAY_SECS: u64 = 90;

    /// 菜单 / 命令面板: 打开窗口并立刻检查一次 (已经有结果在手就直接显示)
    pub(super) fn upd_open(&mut self) {
        self.upd.open = true;
        if matches!(
            self.upd.phase,
            Phase::Idle | Phase::UpToDate | Phase::Failed(_)
        ) {
            self.upd_check(false, None);
        }
    }

    fn upd_check(&mut self, silent: bool, ctx: Option<egui::Context>) {
        if self.upd.rx.is_some() {
            return;
        }
        self.upd.silent = silent;
        self.upd.phase = Phase::Checking;
        self.upd.page = Source::github().releases_page();
        self.cfg.update_last_check = unix_now();
        let ch = Channel::from_code(&self.cfg.update_channel).unwrap_or_default();
        let (tx, rx) = channel();
        self.upd.rx = Some(rx);
        let _ = std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let res = updnet::check(
                    &Curl,
                    &Source::github(),
                    ch,
                    &update::current_version(),
                    install_kind(),
                );
                let _ = tx.send(Msg::Checked(res));
                if let Some(c) = ctx {
                    c.request_repaint();
                }
            });
    }

    fn upd_download(&mut self, c: Box<Candidate>, ctx: Option<egui::Context>) {
        if self.upd.rx.is_some() {
            return;
        }
        self.upd.phase = Phase::Downloading(c.clone());
        let (tx, rx) = channel();
        self.upd.rx = Some(rx);
        let _ = std::thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                let r = match std::env::current_exe() {
                    Ok(exe) => {
                        let dir = if c.kind == InstallKind::Msi {
                            std::env::temp_dir()
                        } else {
                            exe.parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(std::env::temp_dir)
                        };
                        updnet::stage(
                            &Curl,
                            &c,
                            &crate::updkey::UPDATE_PUBKEY,
                            &dir,
                            &updapply::new_path_of(&exe),
                        )
                    }
                    Err(e) => Err(UpdErr::Io(e.to_string())),
                };
                let _ = tx.send(Msg::Staged(r));
                if let Some(c) = ctx {
                    c.request_repaint();
                }
            });
    }

    /// 每帧: 启动后的每日检查 (启动后延迟 90 秒才允许执行, 记事本模式零网络) + 收后台结果
    pub(super) fn upd_frame(&mut self, ctx: &egui::Context) {
        if !self.upd.auto_done && self.persist && self.cfg.update_daily {
            // 记事本防护: 启动前 90 秒内绝不发起任何网络检查, 避免弱网或离线环境下抢占启动黄金时间
            if self.upd.start_instant.elapsed().as_secs() >= Self::AUTO_UPDATE_DELAY_SECS {
                self.upd.auto_done = true;
                updapply::cleanup_stale();
                if update::due(unix_now(), self.cfg.update_last_check) {
                    self.upd_check(true, Some(ctx.clone()));
                }
            }
        }
        let Some(rx) = &self.upd.rx else { return };
        // 只有在手动打开更新弹窗且正在检查时才做低频重绘兜底; 静默检查完全由工作线程唤醒, 彻底杜绝 10Hz 轮询重绘风暴
        if self.upd.open && !self.upd.silent {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Msg::Checked(Err(UpdErr::Io("worker ended".into()))),
        };
        self.upd.rx = None;
        let silent = std::mem::take(&mut self.upd.silent);
        match msg {
            Msg::Checked(Ok(Outcome::UpToDate)) => {
                self.upd.phase = if silent { Phase::Idle } else { Phase::UpToDate }
            }
            Msg::Checked(Ok(Outcome::Available(c))) => {
                self.cur().tip = trf("upd.tip-found", &[&c.release.tag]);
                self.upd.page = c.release.page.clone();
                self.upd.phase = Phase::Available(c);
                if silent {
                    self.upd.open = true;
                }
            }
            Msg::Checked(Err(e)) => self.upd_fail(e, silent),
            Msg::Staged(Ok(s)) => {
                if let Phase::Downloading(c) = std::mem::take(&mut self.upd.phase) {
                    self.upd.phase = Phase::Ready(c, s);
                }
            }
            Msg::Staged(Err(e)) => self.upd_fail(e, false),
        }
    }

    fn upd_fail(&mut self, e: UpdErr, silent: bool) {
        let msg = e.describe();
        self.log(format!("update: {msg}"));
        self.upd.phase = if silent {
            Phase::Idle
        } else {
            Phase::Failed(msg)
        };
    }

    pub(super) fn upd_window(&mut self, ctx: &egui::Context) {
        self.upd_frame(ctx);
        if !self.upd.open {
            return;
        }

        self.ensure_installer_logo(ctx);

        // 1. 半透明漫画墨水暗化遮罩
        let screen_rect = ctx.content_rect();
        let bg_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("update_comic_modal_backdrop"),
        ));
        bg_painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(210));

        let (mut check, mut download, mut install, mut page) = (false, None, None, false);
        let busy = self.upd.rx.is_some();
        let ch = Channel::from_code(&self.cfg.update_channel).unwrap_or_default();

        let win_size = Vec2::new(880.0, 580.0);
        let mut close_requested = false;

        egui::Window::new("update_comic_modal")
            .id(egui::Id::new("update_comic_modal_window"))
            .title_bar(false)
            .frame(egui::Frame::NONE)
            .fixed_size(win_size)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let win_rect = ui.max_rect();
                let header_h = 50.0;
                let header_rect =
                    Rect::from_min_size(win_rect.min, Vec2::new(win_rect.width(), header_h));

                let badge_bg = match &self.upd.phase {
                    Phase::Available(_) | Phase::Downloading(_) | Phase::Ready(_, _) => {
                        Color32::from_rgb(16, 185, 129)
                    }
                    Phase::Failed(_) => Color32::from_rgb(239, 68, 68),
                    _ => Color32::from_rgb(2, 132, 199),
                };
                let badge_label = match &self.upd.phase {
                    Phase::Available(_) => crate::i18n::trz("发现新版本"),
                    Phase::Downloading(_) => crate::i18n::trz("正在下载"),
                    Phase::Ready(_, _) => crate::i18n::trz("更新就绪"),
                    Phase::Failed(_) => crate::i18n::trz("更新异常"),
                    _ => crate::i18n::trz("检查更新"),
                };

                // 绘制外层美漫底板与标题栏
                {
                    let p = ui.painter();
                    // 硬阴影
                    p.rect_filled(win_rect.translate(vec2(6.0, 6.0)), 0.0, Color32::BLACK);
                    // 深黑钛金底色
                    p.rect_filled(win_rect, 0.0, Color32::from_rgb(14, 18, 26));
                    p.rect_stroke(
                        win_rect,
                        0.0,
                        Stroke::new(2.0, Color32::from_rgb(30, 41, 59)),
                        StrokeKind::Inside,
                    );

                    // 顶部 Header
                    p.rect_filled(header_rect, 0.0, Color32::from_rgb(20, 25, 36));
                    p.line_segment(
                        [header_rect.left_bottom(), header_rect.right_bottom()],
                        Stroke::new(1.8, badge_bg),
                    );

                    // Logo
                    let logo_center = pos2(header_rect.left() + 28.0, header_rect.center().y);
                    paint_authentic_logo(
                        p,
                        self.installer.logo_tex.as_ref(),
                        logo_center,
                        26.0,
                        vec2(2.0, 2.0),
                    );

                    let title_x = header_rect.left() + 56.0;
                    p.text(
                        pos2(title_x + 1.5, header_rect.center().y + 1.5),
                        egui::Align2::LEFT_CENTER,
                        "IDE 1.0",
                        egui::FontId::proportional(17.0),
                        Color32::BLACK,
                    );
                    p.text(
                        pos2(title_x, header_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "IDE 1.0",
                        egui::FontId::proportional(17.0),
                        Color32::from_rgb(241, 245, 249),
                    );

                    // 模式胶囊
                    let wiz_rect = Rect::from_center_size(
                        pos2(title_x + 122.0, header_rect.center().y),
                        vec2(88.0, 22.0),
                    );
                    paint_comic_hard_shadow_rect(p, wiz_rect, badge_bg, 1.5, vec2(1.5, 1.5));
                    p.with_clip_rect(wiz_rect.shrink2(vec2(3.0, 1.0))).text(
                        wiz_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        badge_label,
                        egui::FontId::proportional(10.5),
                        Color32::WHITE,
                    );

                    // 当前版本胶囊
                    let ver_rect = Rect::from_center_size(
                        pos2(header_rect.right() - 86.0, header_rect.center().y),
                        vec2(76.0, 22.0),
                    );
                    paint_comic_hard_shadow_rect(
                        p,
                        ver_rect,
                        Color32::from_rgb(30, 41, 59),
                        1.5,
                        vec2(1.5, 1.5),
                    );
                    crate::ui_icons::paint(
                        p,
                        Rect::from_center_size(
                            pos2(ver_rect.left() + 14.0, ver_rect.center().y),
                            vec2(10.0, 10.0),
                        ),
                        crate::ui_icons::UiIcon::Spark,
                        Color32::from_rgb(56, 189, 248),
                    );
                    p.text(
                        pos2(ver_rect.center().x + 6.0, ver_rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        concat!("v", env!("CARGO_PKG_VERSION")),
                        egui::FontId::proportional(10.5),
                        Color32::from_rgb(203, 213, 225),
                    );
                }

                // 标题栏拖动
                let drag_rect = Rect::from_min_max(
                    pos2(header_rect.left() + 190.0, header_rect.top()),
                    pos2(header_rect.right() - 60.0, header_rect.bottom()),
                );
                let drag_resp = ui.interact(
                    drag_rect,
                    ui.id().with("update_drag_caption"),
                    egui::Sense::drag(),
                );
                if drag_resp.drag_started() || drag_resp.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                // 纯自绘美漫关闭按钮
                if paint_comic_window_btn(
                    ui,
                    pos2(header_rect.right() - 20.0, header_rect.center().y),
                    "comic_upd_close",
                    true,
                ) {
                    close_requested = true;
                }

                // 内容主体：左侧真 3D 展台 + 右侧更新分镜
                let body_rect = Rect::from_min_max(
                    pos2(win_rect.left() + 16.0, header_rect.bottom() + 14.0),
                    pos2(win_rect.right() - 16.0, win_rect.bottom() - 12.0),
                );

                let left_w = 330.0;
                let left_panel_rect =
                    Rect::from_min_size(body_rect.min, vec2(left_w, body_rect.height()));
                let right_panel_rect = Rect::from_min_max(
                    pos2(left_panel_rect.right() + 14.0, body_rect.top()),
                    body_rect.max,
                );

                // 渲染左侧英雄展台 (真 3D 赛璐珞水晶旋转钻石)
                self.render_left_hero_stage(ui, left_panel_rect);

                // 渲染右侧更新分镜面板
                let content_rect = Rect::from_min_max(
                    right_panel_rect.min,
                    pos2(
                        (right_panel_rect.right() - 3.0).max(right_panel_rect.left() + 60.0),
                        (right_panel_rect.bottom() - 3.0).max(right_panel_rect.top() + 60.0),
                    ),
                );

                ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 8.0);

                    // Hero Banner
                    match &self.upd.phase {
                        Phase::Idle => {
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(2, 132, 199),
                                crate::i18n::trz("版本更新与升级 (UPDATES)"),
                                crate::i18n::trz("检查官方渠道发布的最新稳定版与增量补丁"),
                            );
                        }
                        Phase::Checking => {
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(2, 132, 199),
                                crate::i18n::trz("正在连接校验新版本 (CHECKING)..."),
                                crate::i18n::trz("正在连接官方发行源校验最新版本号与数字签名..."),
                            );
                        }
                        Phase::UpToDate => {
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(16, 185, 129),
                                crate::i18n::trz("当前已是最新版本 (UP TO DATE)"),
                                crate::i18n::trz("您当前运行的是最新版，无需更新；可随时重新检查"),
                            );
                        }
                        Phase::Available(c) => {
                            let title = format!(
                                "{}  (v{})",
                                crate::i18n::trz("发现新版本就绪"),
                                c.release.tag
                            );
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(2, 132, 199),
                                &title,
                                crate::i18n::trz(
                                    "官方已有新版本发布，建议升级以获得最新特性与缺陷修复",
                                ),
                            );
                        }
                        Phase::Downloading(c) => {
                            let title = format!(
                                "{}  (v{})",
                                crate::i18n::trz("正在高速下载更新包"),
                                c.release.tag
                            );
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(14, 165, 233),
                                &title,
                                crate::i18n::trz("正在拉取发行包资产并同步校验哈希完整性..."),
                            );
                        }
                        Phase::Ready(c, _s) => {
                            let title = format!(
                                "{}  (v{})",
                                crate::i18n::trz("更新包就绪，校验全量通过"),
                                c.release.tag
                            );
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(16, 185, 129),
                                &title,
                                crate::i18n::trz(
                                    "签名校验与 SHA-256 验证无误，随时可应用更新并极速重启",
                                ),
                            );
                        }
                        Phase::Failed(msg) => {
                            paint_step_hero_banner(
                                ui,
                                Color32::from_rgb(239, 68, 68),
                                crate::i18n::trz("检查更新异常 (UPDATE ERROR)"),
                                msg,
                            );
                        }
                    }

                    ui.add_space(2.0);

                    // 信息卡片: 当前版本与发布通道
                    let cur_ver_desc = format!(
                        "v{}  |  {}: {}  |  {}",
                        env!("CARGO_PKG_VERSION"),
                        crate::i18n::trz("更新通道"),
                        tr(ch.label_key()),
                        crate::i18n::trz("极速桌面开发微内核")
                    );
                    comic_info_card(
                        ui,
                        crate::i18n::trz("本机当前版本"),
                        &cur_ver_desc,
                        UiIcon::Spark,
                        Color32::from_rgb(56, 189, 248),
                    );

                    // 若发现新版本或正在下载/就绪，展示目标版本信息卡片
                    match &self.upd.phase {
                        Phase::Available(c) | Phase::Downloading(c) | Phase::Ready(c, _) => {
                            let signed =
                                crate::updkey::is_configured(&crate::updkey::UPDATE_PUBKEY);
                            let asset_desc = match &c.asset {
                                Some(a) if signed => {
                                    format!(
                                        "{} ({:.1} MB) · {}",
                                        a.name,
                                        a.size as f64 / 1_048_576.0,
                                        crate::i18n::trz("已签名")
                                    )
                                }
                                Some(a) => {
                                    format!(
                                        "{} ({:.1} MB) · {}",
                                        a.name,
                                        a.size as f64 / 1_048_576.0,
                                        crate::i18n::trz("未配置公钥")
                                    )
                                }
                                None => crate::i18n::trz("无直接可执行资产 (请访问发布主页下载)")
                                    .to_string(),
                            };
                            comic_info_card(
                                ui,
                                crate::i18n::trz("目标发布版本与资产"),
                                &format!("v{} · {}", c.release.tag, asset_desc),
                                UiIcon::CheckCircle,
                                Color32::from_rgb(16, 185, 129),
                            );

                            // 更新日志区域
                            let notes =
                                update::truncate_notes(&c.release.notes, update::NOTES_MAX_CHARS);
                            if !notes.is_empty() {
                                let (notes_rect, _) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 160.0),
                                    egui::Sense::hover(),
                                );
                                paint_comic_hard_shadow_rect(
                                    ui.painter(),
                                    notes_rect,
                                    Color32::from_rgb(18, 22, 34),
                                    2.0,
                                    vec2(2.5, 2.5),
                                );
                                ui.painter().rect_stroke(
                                    notes_rect,
                                    0.0,
                                    Stroke::new(1.8, Color32::from_rgb(45, 55, 75)),
                                    StrokeKind::Inside,
                                );
                                let inner = notes_rect.shrink2(vec2(10.0, 8.0));
                                ui.scope_builder(
                                    egui::UiBuilder::new().max_rect(inner),
                                    |ui| {
                                        egui::ScrollArea::vertical()
                                            .id_salt("upd_notes_comic_scroll")
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                ui.spacing_mut().item_spacing.y = 2.0;
                                                ui.label(
                                                    egui::RichText::new(&notes)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(203, 213, 225)),
                                                );
                                            });
                                    },
                                );
                            }
                        }
                        _ => {
                            // 空白提示或说明卡片
                            comic_info_card(
                                ui,
                                crate::i18n::trz("更新安全策略"),
                                crate::i18n::trz(
                                    "更新只在手动点击或定时非阻塞检查时执行，零后台静默常驻网络消耗",
                                ),
                                UiIcon::Shield,
                                Color32::from_rgb(14, 165, 233),
                            );
                        }
                    }

                    ui.add_space(4.0);

                    // 底部行动条
                    let total_w = ui.available_width();
                    let close_w = 80.0;
                    let page_w = 130.0;
                    let primary_w = (total_w - close_w - page_w - 28.0).max(100.0);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;

                        match &self.upd.phase {
                            Phase::Available(c) => {
                                let signed =
                                    crate::updkey::is_configured(&crate::updkey::UPDATE_PUBKEY);
                                if signed && c.asset.is_some() {
                                    if comic_action_button_with_icon(
                                        ui,
                                        Some(UiIcon::Bolt),
                                        crate::i18n::trz("立即下载更新 (DOWNLOAD NOW)"),
                                        vec2(primary_w, 46.0),
                                        !busy,
                                        Color32::from_rgb(2, 132, 199),
                                    )
                                    .clicked()
                                    {
                                        download = Some(c.clone());
                                    }
                                } else if comic_action_button(
                                    ui,
                                    crate::i18n::trz("前往发布主页手动下载"),
                                    vec2(primary_w, 46.0),
                                    true,
                                    Color32::from_rgb(2, 132, 199),
                                )
                                .clicked()
                                {
                                    page = true;
                                }
                            }
                            Phase::Downloading(_) => {
                                comic_action_button(
                                    ui,
                                    crate::i18n::trz("正在后台高速下载中..."),
                                    vec2(primary_w, 46.0),
                                    false,
                                    Color32::from_rgb(30, 41, 59),
                                );
                            }
                            Phase::Ready(_, s) => {
                                if comic_action_button_with_icon(
                                    ui,
                                    Some(UiIcon::Play),
                                    crate::i18n::trz("立即安装并重启 (INSTALL NOW)"),
                                    vec2(primary_w, 46.0),
                                    true,
                                    Color32::from_rgb(16, 185, 129),
                                )
                                .clicked()
                                {
                                    install = Some(s.clone());
                                }
                            }
                            _ => {
                                if comic_action_button_with_icon(
                                    ui,
                                    Some(UiIcon::Bolt),
                                    crate::i18n::trz("重新检查更新 (CHECK AGAIN)"),
                                    vec2(primary_w, 46.0),
                                    !busy,
                                    Color32::from_rgb(2, 132, 199),
                                )
                                .clicked()
                                {
                                    check = true;
                                }
                            }
                        }

                        if comic_action_button(
                            ui,
                            crate::i18n::trz("发布主页"),
                            vec2(page_w, 46.0),
                            false,
                            Color32::from_rgb(30, 41, 59),
                        )
                        .clicked()
                        {
                            page = true;
                        }

                        if comic_action_button(
                            ui,
                            crate::i18n::trz("关闭"),
                            vec2(close_w, 46.0),
                            false,
                            Color32::from_rgb(20, 26, 38),
                        )
                        .clicked()
                        {
                            close_requested = true;
                        }
                    });
                });
            });

        if close_requested || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.upd.open = false;
        }
        if page {
            if let Err(e) = crate::md_view::open_url(&self.upd.page) {
                self.cur().tip = e;
            }
        }
        if check {
            self.upd_check(false, Some(ctx.clone()));
        }
        if let Some(c) = download {
            self.upd_download(c, Some(ctx.clone()));
        }
        if let Some(s) = install {
            // 和「立即重启」同一套: 登记请求, 走正常关闭流程 (有未保存修改照常先问; 取消关闭 = 请求作废), 关完由 main 执行
            updapply::request(if s.kind == InstallKind::Msi {
                Apply::Msi(s.path)
            } else {
                Apply::Swap(s.path)
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
