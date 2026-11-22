//! 应用生命周期管理：单实例 IPC 唤醒、系统托盘事件处理、窗口关闭与退出确认

use super::*;

impl App {
    /// 同步系统托盘常驻状态但不写盘 (启动与 apply_settings 路径使用: 避免启动写盘与无头测试创建系统托盘)
    pub(super) fn sync_tray_resident_no_save(&mut self, resident: bool) {
        self.cfg.tray_resident = resident;
        if resident {
            if self.persist && self.tray.is_none() {
                self.tray = crate::tray::Tray::build(true, self.topmost);
            }
            if let Some(t) = &self.tray {
                t.set_resident_checked(true);
                t.set_topmost_checked(self.topmost);
            }
        } else {
            self.tray = None;
        }
    }

    /// 统一同步系统托盘常驻状态与托盘图标生命周期
    pub fn set_tray_resident(&mut self, resident: bool) {
        self.cfg.tray_resident = resident;
        if resident {
            if self.tray.is_none() {
                self.tray = crate::tray::Tray::build(true, self.topmost);
            }
            if let Some(t) = &self.tray {
                t.set_resident_checked(true);
                t.set_topmost_checked(self.topmost);
            }
        } else {
            self.tray = None;
        }
        if self.persist {
            self.cfg.save();
        }
    }

    /// 处理来自系统托盘的交互事件
    pub fn handle_tray_event(&mut self, ctx: &egui::Context, ev: crate::tray::TrayEvent) {
        match ev {
            crate::tray::TrayEvent::ShowHide => {
                let is_min = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
                let is_vis = ctx.input(|i| i.viewport().visible().unwrap_or(true));
                if !self.window_hidden && is_vis && !is_min {
                    self.window_hidden = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                } else {
                    self.window_hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::NewFile => {
                self.window_hidden = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.new_doc();
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::OpenFile => {
                self.window_hidden = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.open_dialog();
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::OpenCollab => {
                self.window_hidden = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.ai.open = true;
                self.ai.collab_mode = true;
                self.collab.open = true;
                self.expand_dock(crate::panels::Dock::Right);
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::OpenDiff => {
                self.window_hidden = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.diff_with_file();
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::ToggleTopmost => {
                self.toggle_topmost(ctx);
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::ToggleResident => {
                let next = !self.cfg.tray_resident;
                self.set_tray_resident(next);
                if !next {
                    self.window_hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::DisableResident => {
                self.set_tray_resident(false);
                self.window_hidden = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            }
            crate::tray::TrayEvent::Quit => {
                if self.docs.iter().any(|d| d.dirty) && self.cfg.confirm_close {
                    self.window_hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        egui::WindowLevel::AlwaysOnTop,
                    ));
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Critical,
                    ));
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
            self.window_hidden = false;
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
                self.window_hidden = true;
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
            self.window_hidden = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
            self.pending_confirm_close = true;
            ctx.request_repaint();
            return false;
        }
        if self.pending_confirm_close {
            self.process_pending_confirm_close(ctx);
            return false;
        }
        // 关窗口: 确认退出后清理超过 7 天的历史日志
        if ctx.input(|i| i.viewport().close_requested()) {
            self.start_exit_log_cleanup(ctx);
        }
        false
    }

    fn process_pending_confirm_close(&mut self, ctx: &egui::Context) {
        self.pending_confirm_close = false;
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
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::Normal,
            ));
            crate::updapply::cancel();
        }
    }

    fn start_exit_log_cleanup(&mut self, ctx: &egui::Context) {
        match &self.log_cleanup_state {
            LogCleanupState::NotStarted => {
                let stale = crate::diag_log::scan_stale_logs(crate::diag_log::MAX_LOG_AGE_DAYS);
                if stale.is_empty() {
                    self.log_cleanup_state = LogCleanupState::Finished;
                    return;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                let total = stale.len();
                let rx = crate::diag_log::start_stale_log_cleanup(stale);
                self.log_cleanup_state = LogCleanupState::Cleaning {
                    total,
                    current: 0,
                    file_name: String::new(),
                    rx,
                    started_at: std::time::Instant::now(),
                };
                ctx.request_repaint();
            }
            LogCleanupState::Cleaning { .. } => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            LogCleanupState::Finished => {}
        }
    }

    /// 渲染并驱动日志清理进度模态窗口
    pub(super) fn log_cleanup_window(&mut self, ctx: &egui::Context) {
        let (total, current, file_name, should_close) = match &mut self.log_cleanup_state {
            LogCleanupState::Cleaning {
                total,
                current,
                file_name,
                rx,
                started_at,
            } => {
                let mut done = false;
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        crate::diag_log::LogCleanupMsg::Progress {
                            current: c,
                            file_name: f,
                            ..
                        } => {
                            *current = c;
                            *file_name = f;
                        }
                        crate::diag_log::LogCleanupMsg::Complete { .. } => {
                            done = true;
                        }
                    }
                }
                if started_at.elapsed() >= std::time::Duration::from_millis(1500) {
                    done = true;
                }
                (*total, *current, file_name.clone(), done)
            }
            _ => return,
        };

        if should_close {
            self.log_cleanup_state = LogCleanupState::Finished;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        ctx.request_repaint();
        Self::render_comic_backdrop(ctx, "log_cleanup_comic_modal_backdrop");

        let win_size = egui::vec2(440.0, 160.0);
        egui::Window::new("log_cleanup_modal")
            .id(egui::Id::new("log_cleanup_modal_window"))
            .title_bar(false)
            .frame(egui::Frame::NONE)
            .fixed_size(win_size)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                self.render_log_cleanup_panel(ui, total, current, &file_name);
            });
    }

    fn render_log_cleanup_panel(
        &self,
        ui: &mut egui::Ui,
        total: usize,
        current: usize,
        file_name: &str,
    ) {
        let rect = ui.max_rect();
        let p = ui.painter();
        crate::app::installer_ui::art::paint_comic_hard_shadow_rect(
            p,
            rect,
            Color32::from_rgb(18, 22, 32),
            2.0,
            egui::vec2(4.0, 4.0),
        );
        p.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 4.0)),
            0.0,
            Color32::from_rgb(0, 180, 216),
        );

        let text_p = p.with_clip_rect(rect.shrink(12.0));
        text_p.text(
            egui::pos2(rect.left() + 20.0, rect.top() + 22.0),
            egui::Align2::LEFT_CENTER,
            crate::i18n::trz("日志自动维护 (LOG CLEANUP)"),
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
        text_p.text(
            egui::pos2(rect.left() + 20.0, rect.top() + 44.0),
            egui::Align2::LEFT_CENTER,
            crate::i18n::trz("正在清理超过 7 天的历史诊断日志，请稍候..."),
            egui::FontId::proportional(11.5),
            Color32::from_rgb(148, 163, 184),
        );

        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 20.0, rect.top() + 66.0),
            egui::vec2(rect.width() - 40.0, 22.0),
        );
        Self::render_log_cleanup_bar(p, bar_rect, total, current);

        let display_name = truncate_log_filename(file_name, 40);
        let status_desc = if file_name.is_empty() {
            crate::i18n::trz("正在准备清理任务...").to_string()
        } else {
            format!("{}: {display_name}", crate::i18n::trz("正在清理"))
        };
        text_p.text(
            egui::pos2(rect.left() + 20.0, rect.top() + 106.0),
            egui::Align2::LEFT_CENTER,
            status_desc,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(203, 213, 225),
        );
    }

    fn render_log_cleanup_bar(
        p: &egui::Painter,
        bar_rect: egui::Rect,
        total: usize,
        current: usize,
    ) {
        let ratio = if total > 0 {
            (current as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        p.rect_filled(bar_rect, 0.0, Color32::from_rgb(10, 14, 22));
        p.rect_stroke(
            bar_rect,
            0.0,
            egui::Stroke::new(1.8, Color32::BLACK),
            egui::StrokeKind::Inside,
        );
        if ratio > 0.0 {
            let fill_rect = egui::Rect::from_min_size(
                bar_rect.min,
                egui::vec2(bar_rect.width() * ratio, bar_rect.height()),
            );
            p.rect_filled(fill_rect, 0.0, Color32::from_rgb(0, 180, 216));
        }
        let pct_text = format!(
            "{} / {} ({}%)",
            current,
            total,
            (ratio * 100.0).round() as u32
        );
        p.text(
            bar_rect.center(),
            egui::Align2::CENTER_CENTER,
            pct_text,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
}

/// 按 Unicode 字符数从左侧省略截断超长日志文件名，避免 UTF-8 字节边界切片 panic
fn truncate_log_filename(file_name: &str, max_chars: usize) -> String {
    let char_count = file_name.chars().count();
    if char_count <= max_chars || max_chars <= 3 {
        return file_name.to_string();
    }
    let tail: String = file_name
        .chars()
        .skip(char_count.saturating_sub(max_chars.saturating_sub(3)))
        .collect();
    format!("...{tail}")
}

/// 窗口关闭时历史日志清理状态机
#[derive(Default)]
pub enum LogCleanupState {
    #[default]
    NotStarted,
    Cleaning {
        total: usize,
        current: usize,
        file_name: String,
        rx: std::sync::mpsc::Receiver<crate::diag_log::LogCleanupMsg>,
        started_at: std::time::Instant,
    },
    Finished,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_cleanup_state_transitions() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut state = LogCleanupState::Cleaning {
            total: 3,
            current: 1,
            file_name: "test.log".to_string(),
            rx,
            started_at: std::time::Instant::now(),
        };

        tx.send(crate::diag_log::LogCleanupMsg::Progress {
            current: 2,
            total: 3,
            file_name: "test2.log".to_string(),
        })
        .unwrap();
        tx.send(crate::diag_log::LogCleanupMsg::Complete {
            deleted: 3,
            failed: 0,
        })
        .unwrap();

        if let LogCleanupState::Cleaning {
            current,
            file_name,
            rx,
            ..
        } = &mut state
        {
            let mut completed = false;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    crate::diag_log::LogCleanupMsg::Progress {
                        current: c,
                        file_name: f,
                        ..
                    } => {
                        *current = c;
                        *file_name = f;
                    }
                    crate::diag_log::LogCleanupMsg::Complete { .. } => {
                        completed = true;
                    }
                }
            }
            assert_eq!(*current, 2);
            assert_eq!(file_name, "test2.log");
            assert!(completed);
        }
    }

    #[test]
    fn test_truncate_log_filename_utf8_safe() {
        let short = "ide10_diag_20261121.log";
        assert_eq!(truncate_log_filename(short, 40), short);

        let long_ascii = "ide10_diag_very_long_prefix_20261121_180000_detail.log";
        let out_ascii = truncate_log_filename(long_ascii, 40);
        assert!(out_ascii.starts_with("..."));
        assert_eq!(out_ascii.chars().count(), 40);

        let multibyte =
            "崩溃诊断日志_这是一个包含中文与🚀符号的非常长的日志文件名_20261121_180000.log";
        assert!(multibyte.len() > 40 && multibyte.chars().count() > 40);
        let out_utf8 = truncate_log_filename(multibyte, 40);
        assert!(out_utf8.starts_with("..."));
        assert_eq!(out_utf8.chars().count(), 40);
        assert!(out_utf8.ends_with(".log"));
    }
}
