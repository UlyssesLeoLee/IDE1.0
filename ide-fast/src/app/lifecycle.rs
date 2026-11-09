//! 应用生命周期管理：单实例 IPC 唤醒、系统托盘事件处理、窗口关闭与退出确认

use super::*;

impl App {
    /// 统一同步系统托盘常驻状态与托盘图标生命周期
    pub fn set_tray_resident(&mut self, resident: bool) {
        self.cfg.tray_resident = resident;
        if self.persist {
            if resident {
                if self.tray.is_none() {
                    self.tray = crate::tray::Tray::build(true, self.always_on_top);
                }
                if let Some(t) = &self.tray {
                    t.set_resident_checked(true);
                }
            } else {
                self.tray = None;
            }
            self.cfg.save();
        }
    }

    /// 处理来自系统托盘的交互事件
    pub fn handle_tray_event(&mut self, ctx: &egui::Context, ev: crate::tray::TrayEvent) {
        match ev {
            crate::tray::TrayEvent::ShowHide => {
                let is_min = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
                let is_foc = ctx.input(|i| i.viewport().focused.unwrap_or(false));
                let is_vis = ctx.input(|i| i.viewport().visible().unwrap_or(true));
                if is_vis && !is_min && is_foc {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
            crate::tray::TrayEvent::NewFile => {
                self.new_doc();
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            crate::tray::TrayEvent::OpenFile => {
                self.open_dialog();
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            crate::tray::TrayEvent::OpenCollab => {
                self.collab.open = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            crate::tray::TrayEvent::OpenDiff => {
                self.diff_with_file();
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            crate::tray::TrayEvent::ToggleTopmost => {
                self.always_on_top = !self.always_on_top;
                let level = if self.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                if let Some(t) = &self.tray {
                    t.set_topmost_checked(self.always_on_top);
                }
            }
            crate::tray::TrayEvent::ToggleResident => {
                self.cfg.tray_resident = !self.cfg.tray_resident;
                if let Some(t) = &self.tray {
                    t.set_resident_checked(self.cfg.tray_resident);
                }
                if !self.cfg.tray_resident {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                if self.persist {
                    self.cfg.save();
                }
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::DisableResident => {
                self.cfg.tray_resident = false;
                self.tray = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                if self.persist {
                    self.cfg.save();
                }
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::Quit => {
                if self.docs.iter().any(|d| d.dirty) && self.cfg.confirm_close {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                self.force_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx.request_repaint();
            }
        }
    }

    /// 轮询系统托盘事件队列
    pub(super) fn poll_tray(&mut self, ctx: &egui::Context) {
        if let Some(tray) = &self.tray {
            for ev in tray.poll() {
                self.handle_tray_event(ctx, ev);
            }
        }
    }

    /// 处理单实例 IPC 转发过来的路径与拉起唤醒信号
    pub(super) fn handle_forwarded_instances(&mut self, ctx: &egui::Context) {
        let forwarded: Vec<Vec<String>> = self
            .inbox
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        if !forwarded.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();
        }
        for paths in forwarded {
            for a in &paths {
                self.open_spec(a);
            }
        }
    }

    /// 处理窗口关闭事件：常驻托盘拦截、退出会话保存与未保存确认
    /// 返回 true 表示拦截成功并隐藏到托盘，当前帧应提前返回
    pub(super) fn handle_window_close(&mut self, ctx: &egui::Context) -> bool {
        if ctx.input(|i| i.viewport().close_requested()) {
            if !self.force_exit && self.cfg.tray_resident && self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                return true;
            }
            if self.exit_session.is_none() {
                self.exit_session = Some((self.session_paths(), self.session_active()));
            }
            for i in 0..self.docs.len() {
                self.remember_pos(i);
            }
            self.cfg.view.geom = self.geom_now.map(|g| crate::settings::WinGeom {
                maximized: self.maximized_now,
                ..g
            });
        }
        // 关窗口: 有未保存修改先问
        if ctx.input(|i| i.viewport().close_requested())
            && !self.confirm_exit
            && self.cfg.confirm_close
            && self.docs.iter().any(|d| d.dirty)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            let mut i = 0;
            let mut all = true;
            while i < self.docs.len() {
                if self.docs[i].dirty {
                    self.active = i;
                    if !self.close_tab(i) {
                        all = false;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            if all {
                self.confirm_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.exit_session = None;
                self.force_exit = false;
                crate::updapply::cancel();
            }
        }
        false
    }
}
