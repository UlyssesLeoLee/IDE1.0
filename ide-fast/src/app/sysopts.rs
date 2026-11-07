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
    CollabReview,
}

/// 输出页最多留的条数
const LOG_MAX: usize = 300;

#[derive(Default)]
pub(super) struct SysState {
    pub backup: AutoBackup,
    /// 状态栏选区统计方式: 0 字符数, 1 字节数 (UTF-8), 2 行数 (同 Tauri `select-count-mode`, 不落盘)
    pub sel_count: u8,
    /// 代码补全浮层 (打开时才有); 光标所在的屏幕位置每帧由编辑区画光标时记下, 浮层贴在它下面
    pub complete: Option<crate::complete::Popup>,
    pub caret_screen: Option<egui::Pos2>,
    pub locks: FileLocks,
    /// 当前主题是半透明主题 (窗口建出来就是透明的, 这时只决定清屏色 / 要不要加模糊)
    pub theme_glass: bool,
    /// 窗口现在是不是带系统标题栏 (和 `cfg.system_title_bar` 不一致时 `top_bar` 发 Decorations 命令)
    pub decor: bool,
    /// 菜单栏太窄时不画助记字母 (标题栏每帧按宽度定)
    pub mn_hide: bool,
    /// 上一帧画出的顶层菜单: (菜单名键, 弹出层 id); Alt+助记字母据此打开菜单
    pub menu_ids: Vec<(&'static str, egui::Id)>,
    /// Alt+助记字母打开菜单后, 还要等几帧再把焦点送进菜单 (0 = 不用)
    #[allow(dead_code)]
    pub mn_focus: u8,
    /// 菜单开着时 raw_input_hook 截下的方向键 (egui 自己的焦点导航看不到它们), 下一次 shortcuts 处理
    pub nav_q: Vec<super::menunav::NavKey>,
    /// Alt+字母附带的 Text 事件还要吃几帧
    pub swallow: Option<(char, u8)>,
    /// 单按 Alt 识别器 (`menunav::AltTap`)
    pub alt_tap: super::menunav::AltTap,
    /// 菜单栏焦点 (单按 Alt 之后): 高亮的顶层菜单下标; None = 焦点不在菜单栏
    pub bar_focus: Option<usize>,
    /// 菜单栏焦点上按了 ↓ / Enter / 空格 / ↑: 下一次 shortcuts 展开该菜单并送入这个方向键
    pub bar_open: Option<(usize, super::menunav::NavKey)>,
    /// 「结构」搜索窗口 (Markdown 结构化搜索)
    pub mdq: super::mdstruct::MdQuery,
    /// 窗口背后现在有没有 DWM 模糊 (玻璃主题 + 「模糊背景」设置)
    pub blur_on: bool,
    /// 列标尺占的高度 (标尺开着时 = 一行高, 否则 0); 欢迎页让出这一条
    pub ruler_h: f32,
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
        if let Some(after) = self.sys.backup.tick(
            self.cfg.auto_backup,
            self.cfg.auto_backup_min,
            &self.docs,
            std::time::Instant::now(),
        ) {
            ctx.request_repaint_after(after); // 只在选项开着且有未保存的文档时才排
        }
        for (id, outcome) in self.sys.locks.sync(self.cfg.file_lock, &self.docs) {
            let name = self
                .docs
                .iter()
                .find(|d| d.id == id)
                .map(|d| d.name.clone())
                .unwrap_or_default();
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
    }

    /// 现在窗口背后要不要 DWM 模糊: 半透明主题 + 「模糊背景」设置
    pub(super) fn glass_wants_blur(&self) -> bool {
        self.sys.theme_glass && self.cfg.glass_blur
    }

    /// 玻璃主题: 窗口一直是透明的, 切到 / 离开半透明主题时只需开 / 关窗口背后的 DWM 模糊 (`window-vibrancy` 的安全接口, 直接作用在现有 HWND 上), 不用重启
    pub(super) fn glass_sync(&mut self, frame: &eframe::Frame) {
        let want = self.glass_wants_blur();
        if want == self.sys.blur_on {
            return;
        }
        self.sys.blur_on = want;
        #[cfg(windows)]
        {
            if want {
                let _ = crate::glassfx::apply(frame);
            } else {
                crate::glassfx::clear(frame);
            }
        }
        #[cfg(not(windows))]
        let _ = frame;
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
        if self.sys.restore.is_empty() {
            return;
        }
        let (mut open_one, mut open_all, mut discard, mut later) = (None, false, false, false);
        egui::Window::new(tr("sys.restore-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(tr("sys.restore-hint"));
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (i, l) in self.sys.restore.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.small_button(tr("sys.restore-open")).clicked() {
                                    open_one = Some(i);
                                }
                                ui.label(format!(
                                    "{}  ({} KB, {})",
                                    l.name,
                                    l.size.div_ceil(1024),
                                    Self::age_text(l.age_secs)
                                ));
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
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            later = true;
        }
        if let Some(i) = open_one {
            let l = self.sys.restore.remove(i);
            self.open_path(&l.path);
        }
        if open_all {
            let n = self.sys.restore.len();
            for l in std::mem::take(&mut self.sys.restore) {
                self.open_path(&l.path);
            }
            let msg = trf("sys.restore-opened", &[&n.to_string()]);
            self.log(msg.clone());
            self.cur().tip = msg;
        }
        if discard {
            let n = self.sys.restore.len();
            for l in std::mem::take(&mut self.sys.restore) {
                let _ = std::fs::remove_file(&l.path);
            }
            let msg = trf("sys.restore-discarded", &[&n.to_string()]);
            self.log(msg.clone());
            self.cur().tip = msg;
        }
        if later {
            self.sys.restore.clear();
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
        // 面板外框没有内边距: 正文自己留 8px 左右边, 免得「终端」二字贴着窗口边
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 0))
            .show(ui, |ui| match self.sys.bottom {
                BottomTab::Terminal => self.term_ui(ui),
                BottomTab::Output => self.output_ui(ui),
                BottomTab::Problems => self.problems_ui(ui),
                BottomTab::CollabReview => self.collab_review_ui(ui),
            });
    }

    fn collab_review_ui(&mut self, ui: &mut egui::Ui) {
        let mut run_pipe = false;
        let mut run_demo = false;
        let mut open_doc = false;
        Self::bottom_toolbar(self.tc.fg_dim, ui, |ui| {
            if ui.button(trz("▶ 运行四阶段完整流水线")).clicked() {
                run_pipe = true;
            }
            if ui.button(trz("▶ 实时协同光标")).clicked() {
                run_demo = true;
            }
            if ui.button(trz("📄 独立标签页报告")).clicked() {
                open_doc = true;
            }
            if ui.button("×").clicked() {
                self.term.open = false;
            }
        });
        let fg = self.tc.fg;
        let dim = self.tc.fg_dim;
        let blue = self.tc.blue;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(trz(
                        "🛡️ 多 AI 协同编辑 & 独立审查平台 (Multi-AI Co-Edit & Review)",
                    ))
                    .strong()
                    .color(fg),
                );
                ui.add_space(4.0);
                for (stage, detail) in [
                    (
                        "阶段 ① 路由智能体 (Router Agent)",
                        "任务分析 → 子任务 DAG 拆解 → 槽位能力画像匹配 → .nema Section 独占/追加锁分配",
                    ),
                    (
                        "阶段 ② .nema 原生并行协作 (Parallel Execution)",
                        "各槽位仅在授权 Section 内并发生成代码，行级 LWW + 锚点偏移自动收敛，零锁竞争",
                    ),
                    (
                        "阶段 ③ 统筹智能体 (Aggregator Agent)",
                        "全局 AST 符号表一致性校验 → 跨模块依赖拓扑排序 → 边界粘合代码合成",
                    ),
                    (
                        "阶段 ④ 文件级 Git 与四级门禁 (4-Gate Verification)",
                        "语法树校验 → 跨文件类型检查 → 沙箱测试套件 → 独立异构 Reviewer (Claude 4.6) 终审",
                    ),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(trz(stage)).strong().color(blue));
                        ui.label(egui::RichText::new(trz(detail)).color(dim));
                    });
                }
            });
        if run_pipe {
            self.run_collab_pipeline();
        }
        if run_demo {
            self.trigger_coedit_demo();
        }
        if open_doc {
            self.open_review_doc();
        }
    }

    fn output_ui(&mut self, ui: &mut egui::Ui) {
        Self::bottom_toolbar(self.tc.fg_dim, ui, |ui| {
            if ui.button(tr("bt.clear")).clicked() {
                self.sys.log.clear();
            }
            if ui.button("×").clicked() {
                self.term.open = false;
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
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
        Self::bottom_toolbar(self.tc.fg_dim, ui, |ui| {
            if ui.button(tr("md.lint")).clicked() {
                self.md_lint_now();
            }
            if ui.button(tr("bt.clear")).clicked() {
                self.sys.problems.clear();
            }
            if ui.button("×").clicked() {
                self.term.open = false;
            }
        });
        let shown = self
            .sys
            .problems_doc
            .is_some_and(|id| self.docs.iter().any(|d| d.id == id));
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !shown || self.sys.problems.is_empty() {
                    ui.weak(tr("bt.problems-empty"));
                    return;
                }
                for p in &self.sys.problems {
                    let text = format!(
                        "{}  {}  {}",
                        trf("md.line", &[&(p.line + 1).to_string()]),
                        p.rule,
                        p.message
                    );
                    if ui
                        .add(egui::Label::new(text).sense(Sense::click()))
                        .clicked()
                    {
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
