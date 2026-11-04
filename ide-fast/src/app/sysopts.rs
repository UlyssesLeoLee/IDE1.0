//! `App` 的批次 10 系统类选项: 自动备份定时器 / 文件锁 / 玻璃窗口的状态,以及底部面板的页签条 (终端 / 输出 / 问题)。
//! 逻辑在 `backup.rs` (纯函数 + 单测); 这里把它们接到文档列表和窗口上。每帧的开销: 选项都关着时只是几个布尔判断。

use super::*;
use crate::backup::{AutoBackup, FileLocks, LockOutcome};
use crate::mdtools::Problem;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum BottomTab {
    #[default]
    Terminal,
    Output,
    Problems,
}

/// 输出页最多留的条数
const LOG_MAX: usize = 300;

#[derive(Default)]
pub(super) struct SysState {
    pub backup: AutoBackup,
    pub locks: FileLocks,
    /// 窗口创建时就是透明的 (启动时的主题是半透明主题); 透明要在建窗口时定, 运行中换主题只能提示重启
    pub glass: bool,
    /// 当前主题是半透明主题 / 已经提示过要重启
    pub theme_glass: bool,
    pub glass_warned: bool,
    /// 「立即重启」对话框开着
    pub restart_ask: bool,
    /// 窗口现在是不是带系统标题栏 (和 `cfg.system_title_bar` 不一致时 `top_bar` 发 Decorations 命令)
    pub decor: bool,
    /// 菜单栏太窄时不画助记字母 (标题栏每帧按宽度定)
    pub mn_hide: bool,
    /// 自绘标题栏里的应用图标 (第一次画时才建纹理)
    pub title_icon: Option<egui::TextureHandle>,
    /// 欢迎页的大徽标 (第一次画欢迎页时才解码建纹理)
    pub logo: Option<egui::TextureHandle>,
    /// 启动时发现的遗留自动备份 (非空 = 弹「恢复」窗口)
    pub restore: Vec<crate::backup::Leftover>,
    pub bottom: BottomTab,
    /// 输出页的日志 (备份 / 锁 / 导出 / 插件安装 ... 的结果)
    pub log: Vec<String>,
    /// 问题页: Markdown 检查结果,以及它属于哪个文档
    pub problems: Vec<Problem>,
    pub problems_doc: Option<u64>,
}

impl App {
    /// 记一条到「输出」页 (时间用单调的序号即可,不引入时区)
    pub(super) fn log(&mut self, msg: impl Into<String>) {
        let l = &mut self.sys.log;
        l.push(msg.into());
        if l.len() > LOG_MAX {
            l.drain(..l.len() - LOG_MAX);
        }
    }

    /// 每帧一次: 自动备份定时器 + 文件锁同步 + 半透明主题要重启的提示
    pub(super) fn sys_frame(&mut self, ctx: &egui::Context) {
        if let Some(after) = self.sys.backup.tick(self.cfg.auto_backup, self.cfg.auto_backup_min, &self.docs, std::time::Instant::now()) {
            ctx.request_repaint_after(after); // 只在选项开着且有未保存的文档时才排
        }
        for (id, outcome) in self.sys.locks.sync(self.cfg.file_lock, &self.docs) {
            let name = self.docs.iter().find(|d| d.id == id).map(|d| d.name.clone()).unwrap_or_default();
            let msg = match outcome {
                LockOutcome::Busy => trf("sys.lock-busy", &[&name]),
                LockOutcome::Unsupported => tr("sys.lock-unsupported").to_string(),
                LockOutcome::Failed(e) => trf("sys.lock-failed", &[&name, &e]),
            };
            self.log(msg.clone());
            if let Some(d) = self.docs.iter_mut().find(|d| d.id == id) {
                d.tip = msg;
            }
        }
        if !self.sys.theme_glass { self.sys.glass_warned = false; }
        if crate::relaunch::needs_restart(self.sys.glass, self.sys.theme_glass) && !self.sys.glass_warned && !self.first_frame {
            self.sys.glass_warned = true;
            self.sys.restart_ask = true;
            let msg = tr("sys.glass-restart").to_string();
            self.log(msg.clone());
            self.cur().tip = msg;
        }
    }

    /// 遗留备份的年龄文字
    pub(super) fn age_text(secs: Option<u64>) -> String {
        match secs {
            None => tr("sys.restore-age-unknown").to_string(),
            Some(s) if s < 3600 => trf("sys.restore-age-min", &[&(s / 60).max(1).to_string()]),
            Some(s) if s < 86400 => trf("sys.restore-age-hour", &[&(s / 3600).to_string()]),
            Some(s) => trf("sys.restore-age-day", &[&(s / 86400).to_string()]),
        }
    }

    /// 启动后的恢复提示: 列出遗留备份, 可逐个打开 / 全部打开 / 全部丢弃 / 稍后 (稍后 = 文件留着, 下次启动再问)。只在有遗留时才画, 不轮询
    pub(super) fn restore_window(&mut self, ctx: &egui::Context) {
        if self.sys.restore.is_empty() { return; }
        let (mut open_one, mut open_all, mut discard, mut later) = (None, false, false, false);
        egui::Window::new(tr("sys.restore-title")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(tr("sys.restore-hint"));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for (i, l) in self.sys.restore.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.small_button(tr("sys.restore-open")).clicked() { open_one = Some(i); }
                        ui.label(format!("{}  ({} KB, {})", l.name, (l.size + 1023) / 1024, Self::age_text(l.age_secs)));
                    });
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                open_all = ui.button(tr("sys.restore-open-all")).clicked();
                discard = ui.button(tr("sys.restore-discard")).clicked();
                later = ui.button(tr("sys.restore-later")).clicked();
            });
        });
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) { later = true; }
        if let Some(i) = open_one {
            let l = self.sys.restore.remove(i);
            self.open_path(&l.path);
        }
        if open_all {
            let n = self.sys.restore.len();
            for l in std::mem::take(&mut self.sys.restore) { self.open_path(&l.path); }
            let msg = trf("sys.restore-opened", &[&n.to_string()]);
            self.log(msg.clone());
            self.cur().tip = msg;
        }
        if discard {
            let n = self.sys.restore.len();
            for l in std::mem::take(&mut self.sys.restore) { let _ = std::fs::remove_file(&l.path); }
            let msg = trf("sys.restore-discarded", &[&n.to_string()]);
            self.log(msg.clone());
            self.cur().tip = msg;
        }
        if later { self.sys.restore.clear(); }
    }

    /// 半透明主题要重启才能让窗口真正透明: 问一句「立即重启 / 稍后」。立即重启 = 走正常关闭流程 (有未保存的先问), 关完由 main 拉起新进程
    pub(super) fn restart_window(&mut self, ctx: &egui::Context) {
        if !self.sys.restart_ask { return; }
        let mut now = false;
        let mut later = false;
        egui::Window::new(trz("重启以启用半透明窗口")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(trz("窗口是否透明在创建窗口时就定了, 运行中无法切换。重启后会重新打开现在的文件。"));
            ui.horizontal(|ui| {
                now = ui.button(trz("立即重启")).clicked();
                later = ui.button(trz("稍后")).clicked();
            });
        });
        if now {
            self.sys.restart_ask = false;
            let paths: Vec<String> = self.docs.iter().filter_map(|d| d.path.as_ref().map(|p| p.display().to_string())).collect();
            crate::relaunch::request(paths);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if later {
            self.sys.restart_ask = false;
        }
    }

    /// 保存前: 放掉自己持有的锁 (否则自己的写入会被自己挡住);下一帧 `sys_frame` 会重新持有
    pub(super) fn before_save(&mut self, id: u64) {
        self.sys.locks.release(id);
    }

    /// 保存 / 关闭后: 这个文档的自动备份不再需要
    pub(super) fn after_save(&mut self, i: usize, old_backup: &str) {
        if let Some(d) = self.docs.get(i) {
            self.sys.backup.forget(d);
        }
        if let Some(dir) = crate::backup::dir() {
            let _ = std::fs::remove_file(dir.join(old_backup)); // 另存为: 旧名字下的备份也不要了
        }
    }

    /// 设置 > 打开自动备份文件…: 在备份目录里选一个,当作普通文档打开
    pub(super) fn open_autosave(&mut self) {
        let Some(dir) = crate::backup::dir().filter(|d| d.is_dir()) else {
            self.cur().tip = tr("sys.no-autosave").to_string();
            return;
        };
        if let Some(p) = rfd::FileDialog::new().set_directory(dir).pick_file() {
            self.open_path(&p);
        }
    }

    /// 底部面板: 页签条 + 当前页
    pub(super) fn bottom_ui(&mut self, ui: &mut egui::Ui) {
        self.bottom_head(ui);
        match self.sys.bottom {
            BottomTab::Terminal => self.term_ui(ui),
            BottomTab::Output => self.output_ui(ui),
            BottomTab::Problems => self.problems_ui(ui),
        }
    }

    fn output_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.small_button(tr("bt.clear")).clicked() {
                self.sys.log.clear();
            }
            if ui.small_button("×").clicked() {
                self.term.open = false;
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            if self.sys.log.is_empty() {
                ui.weak(tr("bt.output-empty"));
            }
            for l in &self.sys.log {
                ui.add(egui::Label::new(egui::RichText::new(l).monospace()).selectable(true));
            }
        });
    }

    fn problems_ui(&mut self, ui: &mut egui::Ui) {
        let mut jump: Option<usize> = None;
        ui.horizontal(|ui| {
            if ui.small_button(tr("md.lint")).clicked() {
                self.md_lint_now();
            }
            if ui.small_button(tr("bt.clear")).clicked() {
                self.sys.problems.clear();
            }
            if ui.small_button("×").clicked() {
                self.term.open = false;
            }
        });
        let shown = self.sys.problems_doc.is_some_and(|id| self.docs.iter().any(|d| d.id == id));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !shown || self.sys.problems.is_empty() {
                ui.weak(tr("bt.problems-empty"));
                return;
            }
            for p in &self.sys.problems {
                let text = format!("{}  {}  {}", trf("md.line", &[&(p.line + 1).to_string()]), p.rule, p.message);
                if ui.add(egui::Label::new(text).sense(Sense::click())).clicked() {
                    jump = Some(p.line);
                }
            }
        });
        if let (Some(line), Some(id)) = (jump, self.sys.problems_doc) {
            if let Some(i) = self.docs.iter().position(|d| d.id == id) {
                self.active = i;
                self.docs[i].goto(line, 0);
                self.center_once = true;
            }
        }
    }
}
