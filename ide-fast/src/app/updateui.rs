//! 「检查更新」窗口 (帮助菜单 / 命令面板 / 可选的每日启动检查)。联网逻辑在 `updnet.rs`, 纯规则在 `update.rs`, 安装在 `updapply.rs`,
//! 这里只管阶段状态机、一次性后台线程 (用完即退, 不常驻、不轮询) 和窗口。流程: 检查 -> (用户点)下载并校验 -> (用户点)安装并重启。
//! 任何一步失败: 当前版本不动, 显示原因和发布页链接。

use super::*;
use crate::updapply::{self, Apply};
use crate::update::{self, Channel, InstallKind, Source};
use crate::updnet::{self, Candidate, Curl, Outcome, Staged, UpdErr};
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
        let mut open = true;
        let (mut check, mut download, mut install, mut page) = (false, None, None, false);
        let busy = self.upd.rx.is_some();
        let ch = Channel::from_code(&self.cfg.update_channel).unwrap_or_default();
        egui::Window::new(tr("upd.title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(460.0)
            .default_pos(egui::pos2(24.0, 56.0))
            .show(ctx, |ui| {
                ui.small(tr("upd.note"));
                ui.label(trf(
                    "upd.current",
                    &[env!("CARGO_PKG_VERSION"), tr(ch.label_key())],
                ));
                ui.separator();
                match &self.upd.phase {
                    Phase::Idle => {}
                    Phase::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(tr("upd.checking"));
                        });
                    }
                    Phase::UpToDate => {
                        ui.label(trf("upd.uptodate", &[env!("CARGO_PKG_VERSION")]));
                    }
                    Phase::Available(c) | Phase::Downloading(c) | Phase::Ready(c, _) => {
                        ui.strong(trf(
                            "upd.available",
                            &[&c.release.tag, env!("CARGO_PKG_VERSION")],
                        ));
                        let notes =
                            update::truncate_notes(&c.release.notes, update::NOTES_MAX_CHARS);
                        if !notes.is_empty() {
                            ui.label(tr("upd.notes"));
                            egui::ScrollArea::vertical()
                                .max_height(160.0)
                                .id_salt("upd-notes")
                                .show(ui, |ui| {
                                    ui.label(notes);
                                });
                        }
                        let signed = crate::updkey::is_configured(&crate::updkey::UPDATE_PUBKEY);
                        match &c.asset {
                            Some(a) if !signed => {
                                // 公钥还是占位: 不下载、不安装, 只给发布页链接 (安装按钮保留但禁用, 让人看到为什么)
                                ui.label(trf(
                                    "upd.asset",
                                    &[&a.name, &format!("{:.1}", a.size as f64 / 1_048_576.0)],
                                ));
                                ui.colored_label(self.tc.red, tr("upd.nokey"));
                                ui.add_enabled(false, egui::Button::new(tr("upd.install")));
                            }
                            Some(a) => {
                                ui.label(trf(
                                    "upd.asset",
                                    &[&a.name, &format!("{:.1}", a.size as f64 / 1_048_576.0)],
                                ));
                                match &self.upd.phase {
                                    Phase::Available(_) => {
                                        if ui
                                            .add_enabled(
                                                !busy,
                                                egui::Button::new(tr("upd.download")),
                                            )
                                            .clicked()
                                        {
                                            download = Some(c.clone());
                                        }
                                    }
                                    Phase::Downloading(_) => {
                                        ui.horizontal(|ui| {
                                            ui.spinner();
                                            ui.label(tr("upd.downloading"));
                                        });
                                    }
                                    Phase::Ready(_, s) => {
                                        ui.label(trf("upd.verified", &[&s.sha256[..12]]));
                                        if ui
                                            .button(tr("upd.install"))
                                            .tip(tr("upd.install-tip"))
                                            .clicked()
                                        {
                                            install = Some(s.clone());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            None => {
                                ui.colored_label(self.tc.red, tr("upd.no-asset"));
                            }
                        }
                    }
                    Phase::Failed(msg) => {
                        ui.colored_label(self.tc.red, trf("upd.failed", &[msg]));
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new(tr("upd.check")))
                        .clicked()
                    {
                        check = true;
                    }
                    if ui.link(tr("upd.page")).tip(&self.upd.page).clicked() {
                        page = true;
                    }
                });
            });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
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
