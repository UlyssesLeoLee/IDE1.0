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
        if let Some(n) = ev.edit_req {
            let d = &self.docs[ai];
            if let Some((start, end)) = mdtools::block_span(blocks, n, &d.lines) {
                self.md_cache.edit = Some(MdEdit { start, end, buf: d.lines[start..end].join("\n"), doc: d.id, rev: d.rev });
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
    pub(super) fn md_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.small_button(tr("md.lint")).on_hover_text(tr("md.lint-tip")).clicked() {
                self.md_lint_now();
            }
            if ui.small_button(tr("md.export-html")).on_hover_text(tr("md.export-tip")).clicked() {
                self.md_export_now();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.dock_chevron(ui, crate::panels::Dock::Right));
        });
    }
}
