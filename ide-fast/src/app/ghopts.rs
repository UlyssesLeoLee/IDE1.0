//! 「从 GitHub 安装 / 更新插件」窗口。只有用户点按钮才联网;安装前弹确认框 (仓库 / 版本 / 权限),同意的权限才会写进插件仓库。
//! 联网和校验的实现在 `ghplugin.rs`,这里只管窗口和把结果落到提示 / 输出页。

use super::*;
use crate::ghplugin::{self, Done, Found};
use std::sync::mpsc::{Receiver, TryRecvError};

#[derive(Default)]
pub(super) struct GhState {
    pub open: bool,
    repo: String,
    found: Option<Found>,
    updates: Vec<(String, Result<Found, String>)>,
    status: String,
    busy: Option<Receiver<Done>>,
}

impl App {
    pub(super) fn gh_open(&mut self) {
        self.gh.open = true;
    }

    fn gh_start(
        &mut self,
        job: impl FnOnce(&dyn editcore::plugin_host::store::Http, &std::path::Path) -> Done
            + Send
            + 'static,
        status: String,
    ) {
        if self.gh.busy.is_some() {
            return;
        }
        self.gh.status = status;
        self.gh.busy = Some(ghplugin::spawn(plugins::root(), job));
    }

    /// 每帧:收后台结果 (没有任务时什么都不做)
    fn gh_poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.gh.busy else { return };
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let done = match rx.try_recv() {
            Ok(d) => d,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Done::Installed(Err("后台任务意外结束".into())),
        };
        self.gh.busy = None;
        match done {
            Done::Inspected(Ok(f)) => {
                self.gh.status = if f.installable() {
                    trf("gh.found", &[&f.name, &f.version])
                } else {
                    tr("gh.incompatible").to_string()
                };
                self.gh.found = Some(f);
            }
            Done::Inspected(Err(e)) => {
                self.gh.found = None;
                self.gh.status = e.clone();
                self.log(format!("GitHub: {e}"));
            }
            Done::Installed(Ok(what)) => {
                let msg = trf("gh.installed", &[&what]);
                self.gh.status = msg.clone();
                self.log(msg.clone());
                self.cur().tip = msg;
                self.after_plugins_changed();
                self.gh.found = None;
                self.gh.updates.clear();
            }
            Done::Installed(Err(e)) => {
                self.gh.status = e.clone();
                self.log(format!("GitHub: {e}"));
            }
            Done::Updates(u) => {
                let n = u
                    .iter()
                    .filter(|(_, r)| r.as_ref().is_ok_and(|f| f.update))
                    .count();
                self.gh.status = trf("gh.updates", &[&n.to_string()]);
                self.gh.updates = u;
            }
        }
    }

    /// 装了新插件: 宿主丢掉旧运行环境 (下次调用重新加载新代码),围栏 / 主题缓存重查
    fn after_plugins_changed(&mut self) {
        let ids: Vec<String> = plugins::rows().into_iter().map(|r| r.id).collect();
        for id in ids {
            self.plug.unload(&id);
        }
        self.md10.invalidate();
    }

    /// 确认框 -> 安装 (权限以框里列出的为准)
    fn gh_confirm_install(&mut self, f: &Found) {
        let perms = if f.perms.is_empty() {
            tr("gh.no-perms").to_string()
        } else {
            f.perms.join(", ")
        };
        let body = trf(
            "gh.confirm",
            &[&f.repo, &f.name, &f.version, &f.tag, &perms],
        );
        let ok = rfd::MessageDialog::new()
            .set_title(tr("gh.title"))
            .set_description(body)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes;
        if !ok {
            return;
        }
        let (repo, approved) = (f.repo.clone(), f.perms.clone());
        self.gh_start(
            move |http, root| Done::Installed(ghplugin::install(http, root, &repo, &approved)),
            trf("gh.installing", &[&f.name]),
        );
    }

    pub(super) fn gh_window(&mut self, ctx: &egui::Context) {
        self.gh_poll(ctx);
        if !self.gh.open {
            return;
        }
        let mut open = true;
        let mut check_repo: Option<String> = None;
        let mut check_all = false;
        let mut install: Option<Found> = None;
        egui::Window::new(tr("gh.title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(440.0)
            .default_pos(egui::pos2(24.0, 56.0))
            .show(ctx, |ui| {
                ui.small(tr("gh.note"));
                ui.horizontal(|ui| {
                    ui.label(tr("gh.repo"));
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.gh.repo)
                            .hint_text("owner/name")
                            .desired_width(220.0),
                    );
                    let busy = self.gh.busy.is_some();
                    let go = ui
                        .add_enabled(
                            !busy && !self.gh.repo.trim().is_empty(),
                            egui::Button::new(tr("gh.check")),
                        )
                        .clicked()
                        || (r.lost_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter))
                            && !busy
                            && !self.gh.repo.trim().is_empty());
                    if go {
                        check_repo = Some(self.gh.repo.clone());
                    }
                });
                if let Some(f) = &self.gh.found {
                    ui.separator();
                    ui.strong(format!("{} {}", f.name, f.version));
                    ui.label(format!("{}  ({})", f.repo, f.tag));
                    ui.label(match (&f.installed, f.update) {
                        (Some(v), true) => trf("gh.update-from", &[v]),
                        (Some(v), false) => trf("gh.up-to-date", &[v]),
                        (None, _) => tr("gh.not-installed").to_string(),
                    });
                    ui.label(format!(
                        "{}: {}",
                        tr("gh.perms"),
                        if f.perms.is_empty() {
                            tr("gh.no-perms").to_string()
                        } else {
                            f.perms.join(", ")
                        }
                    ));
                    for p in &f.problems {
                        ui.colored_label(self.tc.red, p);
                    }
                    let label = if f.installed.is_some() {
                        tr("gh.update")
                    } else {
                        tr("gh.install")
                    };
                    if ui
                        .add_enabled(
                            f.installable() && self.gh.busy.is_none(),
                            egui::Button::new(label),
                        )
                        .clicked()
                    {
                        install = Some(f.clone());
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.gh.busy.is_none(),
                        egui::Button::new(tr("gh.check-all")),
                    )
                    .clicked()
                {
                    check_all = true;
                }
                for (id, r) in &self.gh.updates {
                    match r {
                        Ok(f) if f.update => {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{id}: {} → {}",
                                    f.installed.clone().unwrap_or_default(),
                                    f.version
                                ));
                                if ui
                                    .add_enabled(
                                        self.gh.busy.is_none() && f.installable(),
                                        egui::Button::new(tr("gh.update")).small(),
                                    )
                                    .clicked()
                                {
                                    install = Some(f.clone());
                                }
                            });
                        }
                        Ok(f) => {
                            ui.weak(format!("{id}: {}", trf("gh.up-to-date", &[&f.version])));
                        }
                        Err(e) => {
                            ui.weak(format!("{id}: {e}"));
                        }
                    }
                }
                if self.gh.busy.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(&self.gh.status);
                    });
                } else if !self.gh.status.is_empty() {
                    ui.label(&self.gh.status);
                }
            });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.gh.open = false;
        }
        if let Some(repo) = check_repo {
            let st = trf("gh.checking", &[&repo]);
            self.gh_start(
                move |http, root| Done::Inspected(ghplugin::inspect(http, root, &repo)),
                st,
            );
        }
        if check_all {
            self.gh_start(
                |http, root| Done::Updates(ghplugin::check_updates(http, root)),
                tr("gh.checking-all").to_string(),
            );
        }
        if let Some(f) = install {
            self.gh_confirm_install(&f);
        }
    }
}
