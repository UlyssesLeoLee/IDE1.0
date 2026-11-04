//! Markdown 预览的画法: 把 `markdown.rs` 解析出的块用 egui 画出来 (表格对齐、提示块、嵌套 / 任务列表、
//! 围栏代码块带语法着色、本地 PNG 图片、链接可点)。解析在 `markdown.rs`, 这里只管样式和交互。

use crate::ui_ext::TipExt;
use crate::markdown::{callout_label, inline, Align, Block, Item, Parsed};
use crate::view::ThemeColors;
use eframe::egui::{self, Color32, FontId, RichText, Sense};
use editcore::syntax_styling::Theme;
use editcore::type_config::TypeRegistry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 预览里的交互结果
#[derive(Default)]
pub struct MdEvent {
    /// 点了某个块: 编辑区跳到这一行 (0 起)
    pub jump: Option<usize>,
    /// 点了链接 (原样的目标; `wiki:名字` 表示维基链接)
    pub link: Option<String>,
    /// 点了块右上角的 ✎: 块的序号 (宿主据此算出源码行范围, 见 `mdtools::block_span`)
    pub edit_req: Option<usize>,
    /// 就地编辑框按了「应用」: (起, 止, 新文字)
    pub apply: Option<(usize, usize, String)>,
    /// 点了任务复选框: 源码行
    pub toggle: Option<usize>,
    /// 需要渲染的围栏块 (语言, 源码): 宿主按插件贡献去渲染, 结果下一帧经 `MdCtx::fences` 回来
    pub fence_req: Vec<(String, String)>,
}

/// 预览里正在就地编辑的块
pub struct MdEdit {
    pub start: usize,
    pub end: usize,
    pub buf: String,
    /// 开始编辑时的文档 id / 修订号: 之后文档在别处被改过就放弃这次编辑 (行范围可能已经不对)
    pub doc: u64,
    pub rev: u64,
}

/// 围栏块的渲染结果 (插件贡献的 `markdown.fence`)
pub enum FenceRes {
    Pending,
    Svg(crate::svg_lite::Svg),
    Failed(String),
}

/// 围栏结果的键: 语言 + 源码
pub fn fence_key(lang: &str, text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in lang.bytes().chain(std::iter::once(0)).chain(text.bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// 已加载的图片纹理 (路径 -> 纹理; 加载失败记 None, 不反复重试)
#[derive(Default)]
pub struct MdCache {
    images: HashMap<PathBuf, Option<egui::TextureHandle>>,
    /// 每块上次量到的高度 (含块间距); 视口外的块只占位不排版, 每帧开销与可见块数成正比
    heights: Vec<f32>,
    width: f32,
    /// 正在就地编辑的块 (同时只有一个)
    pub edit: Option<MdEdit>,
}

pub struct MdCtx<'a> {
    /// 当前文档所在目录 (相对路径的图片 / 链接从这里算)
    pub base: Option<&'a Path>,
    pub registry: &'a TypeRegistry,
    pub theme: &'a Theme,
    pub tc: ThemeColors,
    /// 有插件认领的围栏语言 -> 已有的渲染结果 (键见 `fence_key`)
    pub fence_langs: &'a [String],
    pub fences: &'a HashMap<u64, FenceRes>,
}

/// 只认本地 PNG (不引入网络和别的解码器); 太大的不读
pub fn load_png(path: &Path) -> Option<egui::ColorImage> {
    if std::fs::metadata(path).ok()?.len() > 16 * 1024 * 1024 {
        return None;
    }
    let mut dec = png::Decoder::new(std::fs::File::open(path).ok()?);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut rd = dec.read_info().ok()?;
    let mut buf = vec![0; rd.output_buffer_size()];
    let info = rd.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let px = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == w * h * 4).then(|| egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

/// 图片目标 -> 本地文件路径 (网络地址 = None)
pub fn resolve_local(base: Option<&Path>, target: &str) -> Option<PathBuf> {
    if target.contains("://") || target.starts_with("data:") {
        return None;
    }
    // UNC / 设备路径会触发网络认证 (NTLM 泄露), 一律不解析
    if target.starts_with("\\\\") || target.starts_with("//") || target.starts_with("file://") && !target.starts_with("file:///") {
        return None;
    }
    let t = target.strip_prefix("file:///").unwrap_or(target);
    let p = PathBuf::from(t);
    if p.components().next().is_some_and(|c| matches!(c, std::path::Component::Prefix(x) if !matches!(x.kind(), std::path::Prefix::Disk(_)))) {
        return None;
    }
    Some(if p.is_absolute() { p } else { base?.join(p) })
}

fn code_job(text: &str, lang: &str, cx: &MdCtx, color: Color32) -> egui::text::LayoutJob {
    let font = FontId::monospace(13.0);
    let mut job = egui::text::LayoutJob::default();
    let fmt = |c: Color32| egui::TextFormat { font_id: font.clone(), color: c, ..Default::default() };
    let ext = match lang {
        "javascript" => "js",
        "typescript" => "ts",
        "python" => "py",
        "rust" => "rs",
        "shell" | "bash" | "sh" => "sh",
        "markdown" => "md",
        "yaml" => "yml",
        l => l,
    };
    let cfg = if ext.is_empty() { None } else { cx.registry.detect(&format!("x.{ext}")) };
    for (n, raw) in text.split('\n').enumerate() {
        if n > 0 {
            job.append("\n", 0.0, fmt(color));
        }
        let line = raw.replace('\t', "    ");
        let chars: Vec<char> = line.chars().collect();
        let Some(cfg) = cfg else {
            job.append(&line, 0.0, fmt(color));
            continue;
        };
        let sub = |s: usize, e: usize| -> String { chars[s.min(chars.len())..e.min(chars.len())].iter().collect() };
        let mut at = 0;
        for (s, e, kind) in cfg.highlight_line(&line) {
            let (s, e) = (s.max(0) as usize, (e.max(0) as usize).min(chars.len()));
            if s < at || e <= s {
                continue;
            }
            if s > at {
                job.append(&sub(at, s), 0.0, fmt(color));
            }
            let c = cx.theme.get(kind).fg;
            job.append(&sub(s, e), 0.0, fmt(Color32::from_rgb(c.0, c.1, c.2)));
            at = e;
        }
        if at < chars.len() {
            job.append(&sub(at, chars.len()), 0.0, fmt(color));
        }
    }
    job
}

/// 行内文字 -> 排版 (链接着蓝色加下划线; 图片在 `inline_ui` 里单独处理)
fn rich(spans: &[crate::markdown::Span], cx: &MdCtx, size: f32, base_color: Color32) -> egui::text::LayoutJob {
    let ui_code_bg = if cx.tc.dark { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(18) };
    let mut job = egui::text::LayoutJob::default();
    for s in spans {
        let mut f = egui::TextFormat { font_id: if s.code { FontId::monospace(size - 1.0) } else { FontId::proportional(size) }, color: base_color, italics: s.italic, ..Default::default() };
        if s.bold {
            f.color = if cx.tc.dark { Color32::WHITE } else { Color32::BLACK };
        }
        if s.code {
            f.background = ui_code_bg;
        }
        if s.mark {
            f.background = Color32::from_rgba_unmultiplied(255, 213, 0, 90);
        }
        if s.strike {
            f.strikethrough = egui::Stroke::new(1.0, base_color);
        }
        if s.link.is_some() {
            f.color = cx.tc.blue;
            f.underline = egui::Stroke::new(1.0, cx.tc.blue);
        }
        job.append(&s.text, 0.0, f);
    }
    job
}

/// 一段行内内容: 文字用换行排版, 图片单独占一行; 返回被点击的链接
fn inline_ui(ui: &mut egui::Ui, text: &str, cx: &MdCtx, cache: &mut MdCache, size: f32, color: Color32, ev: &mut MdEvent) -> egui::Response {
    let spans = inline(text);
    ui.vertical(|ui| {
        let mut run: Vec<crate::markdown::Span> = vec![];
        let flush = |ui: &mut egui::Ui, run: &mut Vec<crate::markdown::Span>, ev: &mut MdEvent| {
            if run.is_empty() {
                return;
            }
            let r = ui.add(egui::Label::new(rich(run, cx, size, color)).sense(Sense::click()));
            // 整段里只要有链接, 点击落在哪个链接上很难精确算, 取该段的第一个链接 (一般一段只有一个)
            if r.clicked() {
                if let Some(l) = run.iter().find_map(|s| s.link.clone()) {
                    ev.link = Some(l);
                }
            }
            if r.hovered() && run.iter().any(|s| s.link.is_some()) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            run.clear();
        };
        for s in spans {
            if s.image {
                flush(ui, &mut run, ev);
                image_ui(ui, &s.text, s.link.as_deref().unwrap_or(""), cx, cache);
            } else {
                run.push(s);
            }
        }
        flush(ui, &mut run, ev);
    })
    .response
}

fn image_ui(ui: &mut egui::Ui, alt: &str, target: &str, cx: &MdCtx, cache: &mut MdCache) {
    let Some(path) = resolve_local(cx.base, target) else {
        ui.weak(format!("[图片 {alt}: {target}]"));
        return;
    };
    let ctx = ui.ctx().clone();
    let tex = cache.images.entry(path.clone()).or_insert_with(|| load_png(&path).map(|img| ctx.load_texture(path.to_string_lossy(), img, Default::default())));
    match tex {
        Some(t) => {
            let max_w = ui.available_width().max(40.0);
            let sz = t.size_vec2();
            let k = (max_w / sz.x).min(1.0);
            ui.add(egui::Image::new((t.id(), sz * k)));
        }
        None => {
            ui.weak(format!("[图片 {alt}: {} (只支持本地 PNG)]", path.display()));
        }
    }
}

fn list_ui(ui: &mut egui::Ui, items: &[Item], cx: &MdCtx, cache: &mut MdCache, ev: &mut MdEvent) {
    // 同层有序列表自己数序号 (从原文第一项的数字起), 子层变化后重新计
    let mut counters: Vec<usize> = vec![];
    for it in items {
        counters.truncate(it.depth + 1);
        while counters.len() <= it.depth {
            counters.push(it.num.max(1) - 1);
        }
        let marker = if it.task.is_some() {
            String::new() // 任务项画只读复选框 (字体里没有 ☑ ☐)
        } else if it.ordered {
            counters[it.depth] += 1;
            format!("{}.", counters[it.depth])
        } else {
            ["•", "◦", "▪"][it.depth.min(2)].to_string()
        };
        ui.horizontal_top(|ui| {
            ui.add_space(it.depth as f32 * 18.0 + 4.0);
            if let Some(mut done) = it.task {
                // 点一下 = 改写源码那一行的 [ ] / [x] (宿主套补丁, 一步撤销); 画面等文档变了再更新
                if ui.add(egui::Checkbox::without_text(&mut done)).changed() { ev.toggle = Some(it.line); }
            } else { ui.label(RichText::new(marker).color(cx.tc.fg_dim)); }
            inline_ui(ui, &it.text, cx, cache, 14.0, cx.tc.fg, ev);
        });
    }
}

fn table_ui(ui: &mut egui::Ui, id: usize, rows: &[Vec<String>], aligns: &[Align], cx: &MdCtx, cache: &mut MdCache, ev: &mut MdEvent) {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, cx.tc.border)).show(ui, |ui| {
        egui::Grid::new(("md-table", id)).striped(true).spacing([20.0, 6.0]).show(ui, |ui| {
            for (k, row) in rows.iter().enumerate() {
                for c in 0..cols {
                    let text = row.get(c).map(String::as_str).unwrap_or("");
                    let al = aligns.get(c).copied().unwrap_or(Align::Left);
                    let layout = match al {
                        Align::Left => egui::Layout::top_down(egui::Align::Min),
                        Align::Center => egui::Layout::top_down(egui::Align::Center),
                        Align::Right => egui::Layout::top_down(egui::Align::Max),
                    };
                    ui.with_layout(layout, |ui| {
                        if k == 0 {
                            ui.label(RichText::new(text).strong());
                        } else {
                            inline_ui(ui, text, cx, cache, 13.0, cx.tc.fg, ev);
                        }
                    });
                }
                ui.end_row();
            }
        });
    });
}

/// 源码第 `line` 行 (0 起) 落在哪个块: 起始行不超过它的最后一个块
/// 就地编辑框: 多行文本 + 应用 / 取消
fn edit_ui(ui: &mut egui::Ui, cache: &mut MdCache, ev: &mut MdEvent) {
    let Some(e) = cache.edit.as_mut() else { return };
    let rows = e.buf.lines().count().clamp(3, 14);
    ui.add(egui::TextEdit::multiline(&mut e.buf).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(rows).code_editor());
    let (mut apply, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        apply = ui.button(crate::i18n::tr("md.apply")).clicked();
        cancel = ui.button(crate::i18n::tr("md.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
    });
    if apply {
        ev.apply = Some((e.start, e.end, e.buf.clone()));
        cache.edit = None;
    } else if cancel {
        cache.edit = None;
    }
}

/// 插件贡献的围栏 (如 Pele 的 mermaid): 有结果画图, 还没有就请宿主去渲染, 失败显示原因和源码
fn fence_ui(ui: &mut egui::Ui, lang: &str, text: &str, cx: &MdCtx, ev: &mut MdEvent) -> egui::Response {
    let key = fence_key(lang, text);
    egui::Frame::NONE
        .inner_margin(4)
        .show(ui, |ui| match cx.fences.get(&key) {
            Some(FenceRes::Svg(svg)) => {
                crate::svg_lite::show(ui, svg, Some(cx.tc.fg));
            }
            Some(FenceRes::Failed(why)) => {
                ui.weak(format!("{lang}: {why}"));
                ui.add(egui::Label::new(RichText::new(text).monospace()).selectable(true));
            }
            Some(FenceRes::Pending) => {
                ui.weak(crate::i18n::trf("md.fence-pending", &[lang]));
            }
            None => {
                ev.fence_req.push((lang.to_string(), text.to_string()));
                ui.weak(crate::i18n::trf("md.fence-pending", &[lang]));
            }
        })
        .response
}

pub fn block_at(blocks: &[Parsed], line: usize) -> Option<usize> {
    blocks.iter().rposition(|b| b.line <= line)
}

/// 链接是否允许交给系统打开 (只放行 http(s) 和 mailto, 不执行任意本地路径 / 协议)
pub fn openable(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:")
}

/// 用系统默认程序打开链接 (调用前须过 `openable`)
pub fn open_url(url: &str) -> Result<(), String> {
    if !openable(url) {
        return Err("只能打开 http / https / mailto 链接".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url.trim()]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(url.trim()).spawn();
    r.map(|_| ()).map_err(|e| format!("无法打开链接: {e}"))
}

/// 画整篇预览 (放在滚动区里)
/// 排版选项: 停靠在右侧 (`DOCKED`) 或覆盖在编辑区上 (`OVERLAY`, 对应 Tauri 的 `#md-preview` + `.md-body-view`)
#[derive(Clone, Copy)]
pub struct MdOpts {
    /// 正文最宽 (Tauri `.md-body-view { max-width: 860px; margin: 0 auto }`), 超出就居中
    pub max_w: f32,
    /// 滚动区内左右留白 (Tauri `#md-preview { padding: 12px 24px }`); 滚动条仍贴滚动区右缘
    pub pad_x: f32,
    pub pad_y: f32,
    /// 点一个块 = 就地编辑它 (覆盖模式下源码被盖住, 跳转没意义); 否则点块 = 编辑区跳到那一行
    pub click_edits: bool,
}

impl MdOpts {
    pub const DOCKED: MdOpts = MdOpts { max_w: f32::INFINITY, pad_x: 0.0, pad_y: 0.0, click_edits: false };
    pub const OVERLAY: MdOpts = MdOpts { max_w: 860.0, pad_x: 24.0, pad_y: 12.0, click_edits: true };

    /// 正文列宽
    pub fn body_w(&self, avail: f32) -> f32 { (avail - 2.0 * self.pad_x).max(40.0).min(self.max_w) }
}

pub fn show(ui: &mut egui::Ui, blocks: &[Parsed], cx: &MdCtx, cache: &mut MdCache, scroll_line: Option<usize>, opts: MdOpts) -> MdEvent {
    let target = scroll_line.and_then(|l| block_at(blocks, l));
    let mut ev = MdEvent::default();
    let w = opts.body_w(ui.available_width());
    if (cache.width - w).abs() > 1.0 {
        cache.width = w;
        cache.heights.clear();
    }
    cache.heights.resize(blocks.len(), 40.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_viewport(ui, |ui, view| {
        ui.add_space(opts.pad_y);
        let avail = ui.available_width();
        let left = ui.cursor().min.x + ((avail - w) / 2.0).max(0.0);
        let col = egui::Rect::from_min_max(egui::pos2(left, ui.cursor().min.y), egui::pos2(left + w, f32::INFINITY));
        ui.scope_builder(egui::UiBuilder::new().max_rect(col), |ui| {
        let origin = ui.cursor().min.y;
        for (n, b) in blocks.iter().enumerate() {
            let y = ui.cursor().min.y - origin;
            let h = cache.heights[n];
            if target != Some(n) && (y + h < view.min.y - 200.0 || y > view.max.y + 200.0) {
                ui.allocate_space(egui::vec2(w, h));
                continue;
            }
            let before = ui.cursor().min.y;
            if cache.edit.as_ref().is_some_and(|e| e.start == b.line) {
                edit_ui(ui, cache, &mut ev);
                ui.add_space(6.0);
                cache.heights[n] = ui.cursor().min.y - before;
                continue;
            }
            let mut click_block = true; // 链接、表格内容自己处理点击; 其它块点击 = 回到源码行
            let r = match &b.block {
                Block::Heading { level, text } => {
                    let size = match level { 1 => 26.0, 2 => 19.5, 3 => 15.2, _ => 13.0 }; // Tauri: 13px 基准的 2em / 1.5em / 1.17em / 1em
                    let r = ui.add(egui::Label::new(RichText::new(text).strong().size(size)).sense(Sense::click()));
                    if *level <= 2 {
                        ui.separator();
                    }
                    r
                }
                Block::Para(t) => inline_ui(ui, t, cx, cache, 13.0, cx.tc.fg, &mut ev),
                Block::Quote(t) => {
                    // Tauri: blockquote 左 3px 粗边 (--border-strong), padding 0 12px, 前景 --fg-muted
                    let r = egui::Frame::NONE
                        .inner_margin(egui::Margin { left: 15, right: 4, top: 2, bottom: 2 })
                        .show(ui, |ui| inline_ui(ui, t, cx, cache, 13.0, cx.tc.fg_dim, &mut ev))
                        .response;
                    let bar = if cx.tc.dark { Color32::from_gray(0x45) } else { Color32::from_gray(0xcc) };
                    ui.painter().rect_filled(egui::Rect::from_min_max(r.rect.left_top(), egui::pos2(r.rect.left() + 3.0, r.rect.bottom())), 0.0, bar);
                    r
                }
                Block::Callout { kind, title, text } => {
                    let (icon, name) = callout_label(kind);
                    let accent = match kind.as_str() {
                        "warning" | "caution" => cx.tc.yellow,
                        "danger" | "error" | "bug" => cx.tc.red,
                        "tip" | "hint" | "success" | "check" | "done" => cx.tc.green,
                        _ => cx.tc.blue,
                    };
                    egui::Frame::NONE
                        .fill(accent.gamma_multiply(0.12))
                        .stroke(egui::Stroke::new(1.0, accent))
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{icon} {}", if title.is_empty() { name } else { title })).strong().color(accent));
                            if !text.is_empty() {
                                inline_ui(ui, text, cx, cache, 14.0, cx.tc.fg, &mut ev);
                            }
                        })
                        .response
                }
                Block::Code { lang, text } if cx.fence_langs.contains(lang) => {
                    click_block = false;
                    fence_ui(ui, lang, text, cx, &mut ev)
                }
                Block::Code { lang, text } => {
                    let r = egui::Frame::NONE
                        .fill(if cx.tc.dark { Color32::from_gray(45) } else { Color32::WHITE })
                        .stroke(egui::Stroke::new(if cx.tc.dark { 0.0 } else { 1.0 }, cx.tc.border))
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            if lang == "mermaid" {
                                ui.weak("Mermaid 图只在 Tauri 版里渲染, 这里显示源码");
                            } else if !lang.is_empty() {
                                ui.weak(lang);
                            }
                            ui.add(egui::Label::new(code_job(text, lang, cx, cx.tc.fg)).selectable(true));
                        })
                        .response;
                    click_block = false;
                    r
                }
                Block::List(items) => {
                    click_block = false;
                    ui.vertical(|ui| list_ui(ui, items, cx, cache, &mut ev)).response
                }
                Block::Table { rows, aligns } => {
                    click_block = false;
                    ui.vertical(|ui| table_ui(ui, n, rows, aligns, cx, cache, &mut ev)).response
                }
                Block::Rule => {
                    ui.separator();
                    ui.label("")
                }
            };
            if target == Some(n) {
                ui.scroll_to_rect(r.rect, Some(egui::Align::TOP));
            }
            if click_block && ev.link.is_none() && r.interact(Sense::click()).clicked() {
                if opts.click_edits { ev.edit_req = Some(n); } else { ev.jump = Some(b.line); }
            }
            // 鼠标在块上时右上角出现 ✎: 就地编辑这一块的源码
            if cache.edit.is_none() && ui.rect_contains_pointer(egui::Rect::from_min_max(r.rect.min, egui::pos2(ui.max_rect().right(), r.rect.bottom().max(r.rect.top() + 18.0)))) {
                let hot = egui::Rect::from_min_size(egui::pos2(ui.max_rect().right() - 24.0, r.rect.top()), egui::vec2(22.0, 18.0));
                let resp = ui.interact(hot, egui::Id::new(("md-edit", n)), Sense::click());
                ui.painter().text(hot.center(), egui::Align2::CENTER_CENTER, "✎", FontId::proportional(13.0), if resp.hovered() { cx.tc.fg } else { cx.tc.fg_dim });
                if resp.tip(crate::i18n::tr("md.edit-block")).clicked() { ev.edit_req = Some(n); }
            }
            ui.add_space(6.0);
            cache.heights[n] = ui.cursor().min.y - before;
        }
        });
    });
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_body_is_860_wide_and_centred_with_24px_gutters() {
        assert_eq!(MdOpts::OVERLAY.body_w(1400.0), 860.0);
        assert_eq!(MdOpts::OVERLAY.body_w(600.0), 552.0, "窄窗口: 两边各留 24");
        assert_eq!(MdOpts::DOCKED.body_w(420.0), 420.0, "停靠模式铺满");
        assert!(MdOpts::OVERLAY.click_edits && !MdOpts::DOCKED.click_edits);
    }

    #[test]
    fn local_image_targets_resolve_against_the_document_dir() {
        let base = Path::new("/docs");
        assert_eq!(resolve_local(Some(base), "img/a.png"), Some(base.join("img/a.png")));
        assert_eq!(resolve_local(Some(base), "https://x.io/a.png"), None, "网络图片不加载");
        assert_eq!(resolve_local(None, "a.png"), None, "没有文档目录就没法解析相对路径");
    }

    #[test]
    fn only_web_and_mail_links_open_and_outline_lines_map_to_blocks() {
        assert!(openable("https://a.io/x?y=1&z=2") && openable("mailto:a@b.c") && openable(" HTTP://x"));
        assert!(!openable("file:///c:/x.exe") && !openable("javascript:alert(1)") && !openable("wiki:Page") && !openable("C:/a.bat"));
        let lines: Vec<String> = ["# T", "", "para", "", "## S", "text"].iter().map(|s| s.to_string()).collect();
        let b = crate::markdown::parse(&lines);
        let i = block_at(&b, 5).unwrap();
        assert!(matches!(b[i].block, Block::Para(_)) && b[i].line == 5);
        assert_eq!(block_at(&b, 4).map(|i| b[i].line), Some(4));
        assert_eq!(block_at(&b, 0).map(|i| b[i].line), Some(0));
        assert!(block_at(&[], 3).is_none());
    }

    #[test]
    fn png_files_load_and_garbage_does_not() {
        let d = std::env::temp_dir().join(format!("ide-fast-png-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let ok = d.join("a.png");
        let mut enc = png::Encoder::new(std::fs::File::create(&ok).unwrap(), 3, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&[255, 0, 0, 255].repeat(6)).unwrap();
        let img = load_png(&ok).expect("PNG 能读");
        assert_eq!(img.size, [3, 2]);
        let bad = d.join("b.png");
        std::fs::write(&bad, b"not a png").unwrap();
        assert!(load_png(&bad).is_none());
        assert!(load_png(&d.join("none.png")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn code_blocks_get_highlight_colours_for_known_languages() {
        let reg = TypeRegistry::default_registry();
        let theme = ThemeColors::builtin(true).syntax_theme();
        let fences = HashMap::new();
        let cx = MdCtx { base: None, registry: &reg, theme: &theme, tc: ThemeColors::builtin(true), fence_langs: &[], fences: &fences };
        let colored = code_job("let x = 1; // c", "rust", &cx, Color32::WHITE);
        assert!(colored.sections.len() > 1, "rust 代码被切成多种颜色");
        let plain = code_job("let x = 1;", "", &cx, Color32::WHITE);
        assert_eq!(plain.sections.len(), 1, "没写语言按纯文本");
    }
}
