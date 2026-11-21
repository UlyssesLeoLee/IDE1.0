//! 「检查更新」窗口 (帮助菜单 / 命令面板 / 可选的每日启动检查)。联网逻辑在 `updnet.rs`, 纯规则在 `update.rs`, 安装在 `updapply.rs`,
//! 这里只管阶段状态机、一次性后台线程 (用完即退, 不常驻、不轮询) 和窗口。流程: 检查 -> (用户点)下载并校验 -> (用户点)安装并重启。
//! 任何一步失败: 当前版本不动, 显示原因和发布页链接。

use super::*;
use crate::app::installer_ui::art::paint_comic_hard_shadow_rect;
use crate::app::installer_ui::art::paint_step_hero_banner;
use crate::app::installer_ui::cards::*;
use crate::app::installer_ui::frame::ComicFrameSpec;
use crate::ui_icons::UiIcon;
use crate::updapply::{self, Apply};
use crate::update::{self, Channel, InstallKind, Source};
use crate::updnet::{self, Candidate, Curl, Outcome, Staged, UpdErr};
use egui::{vec2, Color32, Rect, Stroke, StrokeKind, Vec2};
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
}

#[derive(Default)]
struct UpdActions {
    check: bool,
    download: Option<Box<Candidate>>,
    install: Option<Staged>,
    page: bool,
    close: bool,
}

impl App {
    fn upd_badge(&self) -> (Color32, &'static str) {
        match &self.upd.phase {
            Phase::Available(_) => (
                Color32::from_rgb(16, 185, 129),
                crate::i18n::trz("发现新版本"),
            ),
            Phase::Downloading(_) => (
                Color32::from_rgb(16, 185, 129),
                crate::i18n::trz("正在下载"),
            ),
            Phase::Ready(_, _) => (
                Color32::from_rgb(16, 185, 129),
                crate::i18n::trz("更新就绪"),
            ),
            Phase::Failed(_) => (Color32::from_rgb(239, 68, 68), crate::i18n::trz("更新异常")),
            _ => (Color32::from_rgb(2, 132, 199), crate::i18n::trz("检查更新")),
        }
    }

    pub(super) fn upd_window(&mut self, ctx: &egui::Context) {
        self.upd_frame(ctx);
        if !self.upd.open {
            return;
        }

        self.ensure_installer_logo(ctx);
        Self::render_comic_backdrop(ctx, "update_comic_modal_backdrop");

        let busy = self.upd.rx.is_some();
        let ch = Channel::from_code(&self.cfg.update_channel).unwrap_or_default();
        let mut actions = UpdActions::default();
        let (badge_bg, badge_label) = self.upd_badge();

        egui::Window::new("update_comic_modal")
            .id(egui::Id::new("update_comic_modal_window"))
            .title_bar(false)
            .frame(egui::Frame::NONE)
            .fixed_size(Vec2::new(880.0, 580.0))
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let win_rect = ui.max_rect();
                let res = self.render_comic_shell(
                    ui,
                    win_rect,
                    ComicFrameSpec {
                        badge_label,
                        badge_bg,
                        drag_id: "update_drag_caption",
                        min_btn_id: None,
                        close_btn_id: "comic_upd_close",
                    },
                    |app, ui, right_panel_rect| {
                        let content_rect = Rect::from_min_max(
                            right_panel_rect.min,
                            egui::pos2(
                                (right_panel_rect.right() - 3.0)
                                    .max(right_panel_rect.left() + 60.0),
                                (right_panel_rect.bottom() - 3.0)
                                    .max(right_panel_rect.top() + 60.0),
                            ),
                        );

                        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                            actions = app.render_upd_right_content(ui, ch, busy);
                        });
                    },
                );
                if res.close_clicked {
                    actions.close = true;
                }
            });

        if actions.close || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.upd.open = false;
        }
        if actions.page {
            if let Err(e) = crate::md_view::open_url(&self.upd.page) {
                self.cur().tip = e;
            }
        }
        if actions.check {
            self.upd_check(false, Some(ctx.clone()));
        }
        if let Some(c) = actions.download {
            self.upd_download(c, Some(ctx.clone()));
        }
        if let Some(s) = actions.install {
            // 和「立即重启」同一套: 登记请求, 走正常关闭流程 (有未保存修改照常先问; 取消关闭 = 请求作废), 关完由 main 执行
            updapply::request(if s.kind == InstallKind::Msi {
                Apply::Msi(s.path)
            } else {
                Apply::Swap(s.path)
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn render_upd_right_content(
        &mut self,
        ui: &mut egui::Ui,
        ch: Channel,
        busy: bool,
    ) -> UpdActions {
        ui.spacing_mut().item_spacing = vec2(0.0, 8.0);
        self.render_upd_hero_banner(ui);
        ui.add_space(2.0);
        self.render_upd_body_cards(ui, ch);
        ui.add_space(4.0);
        self.render_upd_action_bar(ui, busy)
    }

    fn render_upd_hero_banner(&self, ui: &mut egui::Ui) {
        let tag_title = |prefix: &'static str, tag: &str| format!("{prefix}  (v{tag})");
        let (accent, owned_title, subtitle) = match &self.upd.phase {
            Phase::Idle => (
                Color32::from_rgb(2, 132, 199),
                crate::i18n::trz("版本更新与升级 (UPDATES)").to_string(),
                crate::i18n::trz("检查官方渠道发布的最新稳定版与增量补丁"),
            ),
            Phase::Checking => (
                Color32::from_rgb(2, 132, 199),
                crate::i18n::trz("正在连接校验新版本 (CHECKING)...").to_string(),
                crate::i18n::trz("正在连接官方发行源校验最新版本号与数字签名..."),
            ),
            Phase::UpToDate => (
                Color32::from_rgb(16, 185, 129),
                crate::i18n::trz("当前已是最新版本 (UP TO DATE)").to_string(),
                crate::i18n::trz("您当前运行的是最新版，无需更新；可随时重新检查"),
            ),
            Phase::Available(c) => (
                Color32::from_rgb(2, 132, 199),
                tag_title(crate::i18n::trz("发现新版本就绪"), &c.release.tag),
                crate::i18n::trz("官方已有新版本发布，建议升级以获得最新特性与缺陷修复"),
            ),
            Phase::Downloading(c) => (
                Color32::from_rgb(14, 165, 233),
                tag_title(crate::i18n::trz("正在高速下载更新包"), &c.release.tag),
                crate::i18n::trz("正在拉取发行包资产并同步校验哈希完整性..."),
            ),
            Phase::Ready(c, _s) => (
                Color32::from_rgb(16, 185, 129),
                tag_title(crate::i18n::trz("更新包就绪，校验全量通过"), &c.release.tag),
                crate::i18n::trz("签名校验与 SHA-256 验证无误，随时可应用更新并极速重启"),
            ),
            Phase::Failed(msg) => (
                Color32::from_rgb(239, 68, 68),
                crate::i18n::trz("检查更新异常 (UPDATE ERROR)").to_string(),
                msg.as_str(),
            ),
        };
        paint_step_hero_banner(ui, accent, &owned_title, subtitle);
    }

    fn render_upd_body_cards(&self, ui: &mut egui::Ui, ch: Channel) {
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

        match &self.upd.phase {
            Phase::Available(c) | Phase::Downloading(c) | Phase::Ready(c, _) => {
                let signed = crate::updkey::is_configured(&crate::updkey::UPDATE_PUBKEY);
                let asset_desc = match &c.asset {
                    Some(a) => format!(
                        "{} ({:.1} MB) · {}",
                        a.name,
                        a.size as f64 / 1_048_576.0,
                        if signed {
                            crate::i18n::trz("已签名")
                        } else {
                            crate::i18n::trz("未配置公钥")
                        }
                    ),
                    None => crate::i18n::trz("无直接可执行资产 (请访问发布主页下载)").to_string(),
                };
                comic_info_card(
                    ui,
                    crate::i18n::trz("目标发布版本与资产"),
                    &format!("v{} · {}", c.release.tag, asset_desc),
                    UiIcon::CheckCircle,
                    Color32::from_rgb(16, 185, 129),
                );
                let notes = update::truncate_notes(&c.release.notes, update::NOTES_MAX_CHARS);
                if !notes.is_empty() {
                    Self::render_upd_notes_box(ui, &notes);
                }
            }
            _ => {
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
    }

    fn render_upd_notes_box(ui: &mut egui::Ui, notes: &str) {
        let (notes_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), 160.0), egui::Sense::hover());
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
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("upd_notes_comic_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        egui::RichText::new(notes)
                            .size(11.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                });
        });
    }

    fn render_upd_primary_action(
        &self,
        ui: &mut egui::Ui,
        primary_w: f32,
        busy: bool,
        actions: &mut UpdActions,
    ) {
        match &self.upd.phase {
            Phase::Available(c) => {
                let signed = crate::updkey::is_configured(&crate::updkey::UPDATE_PUBKEY);
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
                        actions.download = Some(c.clone());
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
                    actions.page = true;
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
                    actions.install = Some(s.clone());
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
                    actions.check = true;
                }
            }
        }
    }

    fn render_upd_action_bar(&self, ui: &mut egui::Ui, busy: bool) -> UpdActions {
        let mut actions = UpdActions::default();
        let total_w = ui.available_width();
        let close_w = 80.0;
        let page_w = 130.0;
        let primary_w = (total_w - close_w - page_w - 28.0).max(100.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            self.render_upd_primary_action(ui, primary_w, busy, &mut actions);

            if comic_action_button(
                ui,
                crate::i18n::trz("发布主页"),
                vec2(page_w, 46.0),
                false,
                Color32::from_rgb(30, 41, 59),
            )
            .clicked()
            {
                actions.page = true;
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
                actions.close = true;
            }
        });
        actions
    }
}
