//! `App` 的 Markdown 预览扩展: 就地编辑块 / 任务复选框 / 检查 / 导出 HTML / 插件贡献的围栏 (`markdown.fence`)。
//! 纯逻辑在 `mdtools.rs` (有单测); 这里把预览事件 (`MdEvent`) 接到文档上。围栏只在预览里真的出现该语言的代码块时才去调插件,
//! 插件宿主 (QuickJS) 因此也是用到才创建;结果按 (语言, 源码) 缓存,不重复渲染。

use super::*;
use crate::md_view::{fence_key, FenceRes, MdEdit, MdEvent};
use crate::markdown::Parsed;
use crate::mdtools;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};

/// 一个围栏源码最大字节数 / 缓存条数 / 同时在跑的渲染数
const FENCE_MAX_BYTES: usize = 64 * 1024;
const FENCE_CACHE: usize = 64;
const FENCE_JOBS: usize = 6;

/// 工具条按钮 (editor.html `MD_BTNS`; 分组之间一条竖线): (按钮字, 编辑命令, 提示)
const MD_BAR: &[&[(&str, &str, &str)]] = &[
    &[("B", "md-bold", "粗体 (Ctrl+B)"), ("I", "md-italic", "斜体 (Ctrl+I)"), ("S", "md-strike", "删除线 (Ctrl+Shift+X)"), ("==", "md-highlight", "高亮 ==文字=="), ("`", "md-code", "行内代码 (Ctrl+E)")],
    &[("H1", "md-h1", "标题 1 (Ctrl+Alt+1)"), ("H2", "md-h2", "标题 2 (Ctrl+Alt+2)"), ("H3", "md-h3", "标题 3 (Ctrl+Alt+3)"), ("H4", "md-h4", "标题 4")],
    &[("• 列表", "md-ul", "无序列表"), ("1. 列表", "md-ol", "有序列表"), ("☑ 任务", "md-task", "任务列表"), ("❝ 引用", "md-quote", "引用")],
    &[("链接", "md-link", "链接 (Ctrl+Shift+K)"), ("图片", "md-image", "图片"), ("表格", "md-table", "表格 3×3"), ("{ } 代码块", "md-codeblock", "围栏代码块 (自动选安全围栏)")],
    &[("↑ 章节", "md-section-up", "把光标所在章节(含子章节)上移一格"), ("↓ 章节", "md-section-down", "把光标所在章节(含子章节)下移一格")],
    &[("分隔线", "md-hr", "分隔线"), ("Callout", "md-callout", "Obsidian callout"), ("Mermaid", "md-mermaid", "Mermaid 图 (需要渲染插件才能在预览里出图)")],
];

#[derive(Default)]
pub(super) struct MdState {
    pub fences: HashMap<u64, FenceRes>,
    jobs: Vec<(u64, Receiver<Result<String, String>>)>,
    /// (围栏语言, 插件 id, 方法名);None = 还没查过 / 插件有变动要重查 (查插件仓库要读盘,不能每帧做)
    providers: Option<Vec<(String, String, String)>>,
    /// 渲染结果对应的主题 (深浅 / 主题插件): 变了就全部重渲染 (图的颜色是按主题算的)
    theme_sig: (bool, String),
}

impl MdState {
    /// 插件装 / 删 / 启停之后调用
    pub fn sync_theme(&mut self, dark: bool, id: &str) {
        if self.theme_sig.0 != dark || self.theme_sig.1 != id {
            self.theme_sig = (dark, id.to_string());
            self.fences.clear();
        }
    }

    pub fn invalidate(&mut self) {
        self.providers = None;
        self.fences.retain(|_, v| matches!(v, FenceRes::Pending));
    }

    fn providers(&mut self) -> &[(String, String, String)] {
        self.providers.get_or_insert_with(|| {
            crate::plugins::rows().into_iter().filter(|r| r.enabled).flat_map(|r| r.fences.into_iter().map(move |(lang, m)| (lang, r.id.clone(), m))).collect()
        })
    }

    pub fn langs(&mut self) -> Vec<String> {
        self.providers().iter().map(|p| p.0.clone()).collect()
    }
}

/// 渲染结果 JSON -> 结果 (插件返回 `{svg}` / `{error}`)
fn hexc(c: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// 主题色 -> 围栏图 (Pele) 的 CSS 变量: 线 / 字用主题前景, 节点底比编辑区底略亮 / 暗一点
fn fence_vars(tc: &crate::view::ThemeColors) -> Vec<(&'static str, String)> {
    let mix = |a: egui::Color32, b: egui::Color32, t: f32| egui::Color32::from_rgb((a.r() as f32 * (1.0 - t) + b.r() as f32 * t) as u8, (a.g() as f32 * (1.0 - t) + b.g() as f32 * t) as u8, (a.b() as f32 * (1.0 - t) + b.b() as f32 * t) as u8);
    let bg = if tc.bg_edit.a() < 255 { if tc.dark { egui::Color32::from_gray(30) } else { egui::Color32::WHITE } } else { tc.bg_edit };
    vec![("--pele-bg", hexc(bg)), ("--pele-fg", hexc(tc.fg)), ("--pele-muted", hexc(tc.fg_dim)), ("--pele-line", hexc(tc.fg_dim)), ("--pele-surface", hexc(mix(bg, tc.fg, 0.10))), ("--pele-surface-alt", hexc(mix(bg, tc.fg, 0.18))), ("--pele-border", hexc(tc.fg_dim)), ("--pele-accent", hexc(tc.blue))]
}

fn outcome_to_res(r: Result<String, String>, vars: &[(&'static str, String)]) -> FenceRes {
    let json = match r {
        Ok(j) => j,
        Err(e) => return FenceRes::Failed(e),
    };
    let out = crate::plugin_host::parse_outcome(&json);
    if let Some(e) = out.error {
        return FenceRes::Failed(e);
    }
    match out.svg {
        Some(s) => match crate::svg_lite::parse_with(&s, vars) {
            Ok(svg) => FenceRes::Svg(svg),
            Err(e) => FenceRes::Failed(e),
        },
        None => FenceRes::Failed(tr("md.fence-nosvg").to_string()),
    }
}

impl App {
    /// 预览每帧:收围栏渲染结果
    pub(super) fn md_poll_fences(&mut self, ctx: &egui::Context) {
        if self.md10.jobs.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        let vars = fence_vars(&self.tc);
        let mut finished = vec![];
        for (i, (_, rx)) in self.md10.jobs.iter().enumerate() {
            match rx.try_recv() {
                Ok(r) => finished.push((i, outcome_to_res(r, &vars))),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => finished.push((i, FenceRes::Failed("插件宿主已停止".into()))),
            }
        }
        for (i, res) in finished.into_iter().rev() {
            let (key, _) = self.md10.jobs.remove(i);
            self.md10.fences.insert(key, res);
        }
    }

    /// 预览事件落到文档上: 就地编辑 / 任务复选框 / 围栏渲染请求
    pub(super) fn md_handle(&mut self, ai: usize, blocks: &[Parsed], ev: &MdEvent) {
        if let Some(line) = ev.toggle {
            let d = &mut self.docs[ai];
            if let Some(new) = d.lines.get(line).and_then(|l| mdtools::toggle_task(l)) {
                let cur = d.cursor;
                if let Err(e) = d.apply_line_patch(line, 1, vec![new], Some(cur)) {
                    d.tip = e;
                }
            }
        }
        // 点另一个块等于「点框外」: 先应用, 这次点击不再开新编辑 (块的行号已经变了)
        if let Some(n) = ev.edit_req.filter(|_| ev.apply.is_none()) {
            let d = &self.docs[ai];
            if let Some((start, end)) = mdtools::block_span(blocks, n, &d.lines) {
                self.md_cache.edit = Some(MdEdit { start, end, buf: d.lines[start..end].join("\n"), orig: d.lines[start..end].join("\n"), shown: false, doc: d.id, rev: d.rev });
            }
        }
        if let Some((start, end, text)) = &ev.apply {
            let d = &mut self.docs[ai];
            let cur = [(*start).min(d.lines.len().saturating_sub(1)), 0];
            if let Err(e) = d.apply_line_patch(*start, end.saturating_sub(*start), mdtools::text_to_lines(text), Some(cur)) {
                d.tip = e;
            }
        }
        for (lang, text) in &ev.fence_req {
            self.md_start_fence(lang, text);
        }
    }

    fn md_start_fence(&mut self, lang: &str, text: &str) {
        let key = fence_key(lang, text);
        if self.md10.fences.contains_key(&key) {
            return;
        }
        if text.len() > FENCE_MAX_BYTES {
            self.md10.fences.insert(key, FenceRes::Failed(trf("md.fence-big", &[&(FENCE_MAX_BYTES / 1024).to_string()])));
            return;
        }
        if self.md10.jobs.len() >= FENCE_JOBS {
            return; // 下一帧再请求
        }
        let Some((_, plugin, method)) = self.md10.providers().iter().find(|p| p.0 == lang).cloned() else { return };
        if self.md10.fences.len() >= FENCE_CACHE {
            self.md10.fences.retain(|_, v| matches!(v, FenceRes::Pending));
        }
        let rx = self.plug.call_raw(&plugin, &method, &serde_json::json!([text]));
        self.md10.fences.insert(key, FenceRes::Pending);
        self.md10.jobs.push((key, rx));
    }

    /// 检查当前 Markdown: 结果进底部「问题」页
    pub(super) fn md_lint_now(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let problems = mdtools::lint(&self.docs[i].lines);
        let n = problems.len();
        self.sys.problems = problems;
        self.sys.problems_doc = Some(self.docs[i].id);
        self.term.open = true;
        self.sys.bottom = super::sysopts::BottomTab::Problems;
        let msg = if n == 0 { tr("md.lint-ok").to_string() } else { trf("md.lint-n", &[&n.to_string()]) };
        self.log(msg.clone());
        self.docs[i].tip = msg;
    }

    /// 导出同名 .html (文档要先保存过)
    pub(super) fn md_export_now(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let d = &self.docs[i];
        let Some(path) = d.path.clone() else {
            self.docs[i].tip = tr("md.export-save-first").to_string();
            return;
        };
        let html = mdtools::export_html(&d.name, &d.lines, &mdtools::load_template());
        let out = mdtools::export_path(&path);
        let msg = match crate::backup::write_atomic(&out, html.as_bytes()) {
            Ok(()) => trf("md.exported", &[&out.display().to_string()]),
            Err(e) => trf("md.export-failed", &[&e.to_string()]),
        };
        self.log(msg.clone());
        self.docs[i].tip = msg;
    }

    /// 预览顶部的小工具条
    /// Tauri 的 `#md-bar`: md 文档上方一条工具条 (2px 6px 内边距, bg_bar 底, 下边线, 横向可滚), 左边是格式按钮, 右边是 预览 开关;
    /// 按钮 11px、透明底 + 透明边、圆角 3, 悬停 bg_hover + 描边, 开着的预览按钮 bg_active。再加 ide-fast 自己的 检查 / 导出 两个按钮
    pub(super) fn md_bar(&mut self, root: &mut egui::Ui, ctx: &egui::Context, is_md: bool) {
        if !is_md || !self.cfg.view.md_overlay { return; }
        let tc = self.tc;
        let on = self.md_preview;
        let mut cmd: Option<&'static str> = None;
        let mut toggle = false;
        let (mut lint, mut export) = (false, false);
        let frame = egui::Frame::NONE.fill(tc.bg_bar).inner_margin(egui::Margin::symmetric(6, 2)).stroke(egui::Stroke::NONE);
        let r = egui::Panel::top("mdbar").frame(frame).show_separator_line(false).show(root, |ui| {
            let v = ui.visuals_mut();
            for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
                w.weak_bg_fill = egui::Color32::TRANSPARENT;
                w.bg_stroke = egui::Stroke::NONE;
                w.corner_radius = 3.0.into();
                w.fg_stroke.color = tc.fg;
            }
            v.widgets.hovered.weak_bg_fill = tc.bg_hover;
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, tc.border);
            v.widgets.active.weak_bg_fill = tc.bg_active;
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
            ui.spacing_mut().button_padding = egui::vec2(8.0, 1.0);
            let sep = |ui: &mut egui::Ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(9.0, 18.0), egui::Sense::hover());
                ui.painter().vline(r.center().x, r.y_range(), egui::Stroke::new(1.0, tc.border));
            };
            // 预览开关钉在最右 (Tauri 它在条尾, 窄窗口里要横向滚才够得着; 这里不让它滚出去)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut b = egui::Button::new(egui::RichText::new(trz("预览")).size(11.0));
                if on { b = b.fill(tc.bg_active).stroke(egui::Stroke::new(1.0, tc.border)); }
                if ui.add(b).tip(trz("源码 / 渲染预览切换 (Ctrl+Shift+V)")).clicked() { toggle = true; }
                sep(ui);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    egui::ScrollArea::horizontal().id_salt("mdbar-scroll").auto_shrink([false, true]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for g in MD_BAR {
                                for (label, c, tip) in *g {
                                    if ui.add(egui::Button::new(egui::RichText::new(*label).size(11.0))).tip(trz(tip)).clicked() { cmd = Some(*c); }
                                }
                                sep(ui);
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(tr("md.lint")).size(11.0))).tip(tr("md.lint-tip")).clicked() { lint = true; }
                            if ui.add(egui::Button::new(egui::RichText::new(tr("md.export-html")).size(11.0))).tip(tr("md.export-tip")).clicked() { export = true; }
                        });
                    });
                });
            });
        });
        let rc = r.response.rect;
        root.painter().hline(rc.x_range(), rc.bottom() - 0.5, egui::Stroke::new(1.0, tc.border));
        if let Some(c) = cmd {
            if let Some(clip) = self.cur().edit(c, None) { self.put_clip(ctx, clip); }
        }
        if toggle { self.md_preview = !self.md_preview; }
        if lint { self.md_lint_now(); }
        if export { self.md_export_now(); }
    }

    /// Tauri 的 `#md-preview`: 盖在编辑区 (位置 = 中央区) 上的整块预览, 编辑区底色, 正文最宽 860 居中; 点一个块就地编辑。
    /// 画在 `Order::Background` 的独立层里: 盖住编辑区, 但查找对话框 / 菜单这些更高层照常在它上面
    pub(super) fn md_overlay(&mut self, ctx: &egui::Context, area: egui::Rect) {
        let fill = ctx.global_style().visuals.extreme_bg_color;
        egui::Area::new(egui::Id::new("md-overlay")).order(egui::Order::Background).fixed_pos(area.min).constrain(false).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(area.size(), egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, fill);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            child.set_clip_rect(rect);
            self.md_ui(&mut child, true);
        });
    }

    pub(super) fn md_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.small_button(tr("md.lint")).tip(tr("md.lint-tip")).clicked() {
                self.md_lint_now();
            }
            if ui.small_button(tr("md.export-html")).tip(tr("md.export-tip")).clicked() {
                self.md_export_now();
            }
        });
    }
}
