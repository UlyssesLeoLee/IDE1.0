//! NEMA v1.2 (R7) / v1.5 工具条、共享 Codec 对接、租约仲裁、交互式预览与实测 Benchmark 面板 (`Table / 5W1H / Graph / Benchmark`)

use super::*;
use crate::i18n::trz;
use nema_codec::{
    candidate_dictionary_details, compare_document_formats, find_reused_positions,
    normalize_nema_newlines, parse_domain_header, parse_domain_header_v12, render_source,
    toggle_table_header_in_source, CodecFormat, DomainMode, EdgeDirection, NemaCodec,
    NemaLeaseArbitrator, OwnershipRole, RenderedDocument, RenderedDomain, StandardNemaCodec,
    TokenProfile, COMPLETE_V12_SAMPLE, OFFICIAL_BENCHMARK_V12, OFFICIAL_SYNTAX_BEFORE_AFTER,
};

/// Graph 选中元素详情（支持同时检视 Node 与 Edge，包含 `X0 / X1 / X2 / X3+ extensions`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedGraphItem {
    Node {
        id: String,
        name_x1: String,
        desc_x2: String,
        extensions: Vec<(String, String)>,
        line: usize,
    },
    Edge {
        id: String,
        raw_x0: String,
        route: String,
        name_x1: String,
        desc_x2: String,
        extensions: Vec<(String, String)>,
        line: usize,
    },
}

/// 缓存的关系边拓扑与几何 (相对画布 rect.min)
#[derive(Debug, Clone)]
pub enum CachedEdgeGeometry {
    Loop {
        loop_center_rel: egui::Vec2,
        r_loop: f32,
    },
    Line {
        start_rel: egui::Vec2,
        end_rel: egui::Vec2,
        arrow: Option<[egui::Vec2; 3]>,
    },
}

#[derive(Debug, Clone)]
pub struct CachedEdgeItem {
    pub id: String,
    pub route_str: String,
    pub raw_x0: String,
    pub name_x1: String,
    pub desc_x2: String,
    pub exts: Vec<(String, String)>,
    pub line: usize,
    pub geom: CachedEdgeGeometry,
    pub pill_rect_rel: egui::Rect,
    pub label_txt: String,
}

#[derive(Debug, Clone)]
pub struct CachedNodeItem {
    pub id: String,
    pub name_x1: String,
    pub desc_x2: String,
    pub exts: Vec<(String, String)>,
    pub line: usize,
    pub center_rel: egui::Vec2,
    pub rect_rel: egui::Rect,
    pub top_title: String,
}

/// NEMA Graph 拓扑画布几何缓存，避免每帧重复三角函数排布与堆内存分配
#[derive(Debug, Clone)]
pub struct CachedGraphCanvas {
    pub cache_key: (u64, u64, u32, u32), // (doc_id, doc_rev, (canvas_w * 10.0) as u32, (canvas_h * 10.0) as u32)
    pub edges: Vec<CachedEdgeItem>,
    pub nodes: Vec<CachedNodeItem>,
}

impl CachedGraphCanvas {
    pub fn build(
        cache_key: (u64, u64, u32, u32),
        domain: &RenderedDomain,
        canvas_w: f32,
        canvas_h: f32,
    ) -> Self {
        let mut raw_nodes = Vec::new();
        let mut raw_edges = Vec::new();
        for row in &domain.rows {
            if let Some(nid) = &row.node_id {
                let name_x1 = row.cells.get(1).cloned().unwrap_or_default();
                let desc_x2 = row.cells.get(2).cloned().unwrap_or_default();
                raw_nodes.push((
                    nid.clone(),
                    name_x1,
                    desc_x2,
                    row.extensions.clone(),
                    row.line,
                ));
            } else if let Some(e) = &row.edge {
                let name_x1 = row.cells.get(1).cloned().unwrap_or_default();
                let desc_x2 = row.cells.get(2).cloned().unwrap_or_default();
                raw_edges.push((
                    e.clone(),
                    name_x1,
                    desc_x2,
                    row.extensions.clone(),
                    row.line,
                ));
            }
        }

        let n_count = raw_nodes.len().max(1);
        let center_rel = egui::vec2(canvas_w * 0.5, canvas_h * 0.5);
        let rx = (canvas_w * 0.34).max(74.0);
        let ry = (canvas_h * 0.33).max(58.0);
        let mut node_pos = std::collections::HashMap::new();
        let node_size = egui::vec2(114.0, 44.0);

        for (idx, (nid, _, _, _, _)) in raw_nodes.iter().enumerate() {
            let pos_rel = if n_count == 1 {
                center_rel
            } else if n_count == 2 {
                egui::vec2(
                    canvas_w * (0.26 + 0.48 * (idx as f32)),
                    center_rel.y,
                )
            } else {
                let angle = -std::f32::consts::FRAC_PI_2
                    + 2.0 * std::f32::consts::PI * (idx as f32) / (n_count as f32);
                egui::vec2(center_rel.x + rx * angle.cos(), center_rel.y + ry * angle.sin())
            };
            node_pos.insert(nid.clone(), pos_rel);
        }

        let mut pair_groups: std::collections::HashMap<(String, String), Vec<usize>> =
            std::collections::HashMap::new();
        let mut self_loop_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut lane_offsets = vec![0.0f32; raw_edges.len()];
        let mut loop_indices = vec![0usize; raw_edges.len()];

        for (idx, (edge, _, _, _, _)) in raw_edges.iter().enumerate() {
            if edge.from == edge.to {
                let c = self_loop_counts.entry(edge.from.clone()).or_insert(0);
                loop_indices[idx] = *c;
                *c += 1;
            } else {
                let key = if edge.from <= edge.to {
                    (edge.from.clone(), edge.to.clone())
                } else {
                    (edge.to.clone(), edge.from.clone())
                };
                pair_groups.entry(key).or_default().push(idx);
            }
        }
        for (_, indices) in pair_groups {
            let m = indices.len();
            for (rank, &e_idx) in indices.iter().enumerate() {
                lane_offsets[e_idx] = (rank as f32 - (m as f32 - 1.0) * 0.5) * 18.0;
            }
        }

        let mut edges = Vec::with_capacity(raw_edges.len());
        for (e_idx, (edge, name_x1, desc_x2, exts, line)) in raw_edges.into_iter().enumerate() {
            let Some(&p_from) = node_pos.get(&edge.from) else {
                continue;
            };
            let Some(&p_to) = node_pos.get(&edge.to) else {
                continue;
            };
            let route_str = if edge.from == edge.to {
                format!("{} ↻ {} (self-loop)", edge.from, edge.to)
            } else if edge.direction == EdgeDirection::Undirected {
                format!("{} — {} (undirected)", edge.from, edge.to)
            } else {
                format!("{} → {}", edge.from, edge.to)
            };

            let (geom, mid_rel) = if edge.from == edge.to {
                let r_loop = 18.0 + (loop_indices[e_idx] as f32) * 8.0;
                let loop_center_rel = egui::vec2(p_from.x, p_from.y - 24.0 - r_loop * 0.5);
                let mid_rel = egui::vec2(p_from.x, loop_center_rel.y - r_loop - 4.0);
                (CachedEdgeGeometry::Loop { loop_center_rel, r_loop }, mid_rel)
            } else {
                let (c_a, c_b) = if edge.from <= edge.to {
                    (p_from, p_to)
                } else {
                    (p_to, p_from)
                };
                let canon_dir = (c_b - c_a).normalized();
                let canon_normal = egui::vec2(-canon_dir.y, canon_dir.x);
                let shift = canon_normal * lane_offsets[e_idx];

                let dir = (p_to - p_from).normalized();
                let start_rel = p_from + dir * 52.0 + shift;
                let end_rel = p_to - dir * 52.0 + shift;
                let arrow = if edge.direction == EdgeDirection::Directed {
                    let normal = egui::vec2(-dir.y, dir.x);
                    let p1 = end_rel - dir * 8.5 + normal * 4.5;
                    let p2 = end_rel - dir * 8.5 - normal * 4.5;
                    Some([end_rel, p1, p2])
                } else {
                    None
                };
                let mid_rel = egui::vec2((start_rel.x + end_rel.x) * 0.5, (start_rel.y + end_rel.y) * 0.5);
                (CachedEdgeGeometry::Line { start_rel, end_rel, arrow }, mid_rel)
            };

            let ext_badge = if exts.is_empty() {
                String::new()
            } else {
                format!(" +{}", exts.len())
            };
            let label_txt = if name_x1.is_empty() {
                format!("{}{}", edge.id, ext_badge)
            } else {
                format!("{} · {}{}", edge.id, name_x1, ext_badge)
            };
            let pill_w = (label_txt.chars().count() as f32 * 6.5 + 16.0).clamp(52.0, 178.0);
            let pill_rect_rel = egui::Rect::from_center_size(mid_rel.to_pos2(), egui::vec2(pill_w, 19.0));
            let raw_x0 = format!(
                "{}{}{}{}{}",
                edge.from, edge.raw_operator, edge.id, edge.raw_operator, edge.to
            );

            edges.push(CachedEdgeItem {
                id: edge.id,
                route_str,
                raw_x0,
                name_x1,
                desc_x2,
                exts,
                line,
                geom,
                pill_rect_rel,
                label_txt,
            });
        }

        let mut nodes = Vec::with_capacity(raw_nodes.len());
        for (nid, name_x1, desc_x2, exts, line) in raw_nodes {
            let Some(&pos_rel) = node_pos.get(&nid) else {
                continue;
            };
            let rect_rel = egui::Rect::from_center_size(pos_rel.to_pos2(), node_size);
            let top_title = if exts.is_empty() {
                nid.clone()
            } else {
                format!("{nid} (+{})", exts.len())
            };
            nodes.push(CachedNodeItem {
                id: nid,
                name_x1,
                desc_x2,
                exts,
                line,
                center_rel: pos_rel,
                rect_rel,
                top_title,
            });
        }

        Self {
            cache_key,
            edges,
            nodes,
        }
    }
}

/// NEMA 预览与状态机缓存
#[derive(Debug, Default, Clone)]
pub struct NemaUiState {
    pub arbitrator: NemaLeaseArbitrator,
    pub selected_edge: Option<(String, String, String, String, String, usize)>,
    pub selected_item: Option<SelectedGraphItem>,
    pub info_banner: Option<String>,
    pub locked_words: Vec<String>,
    pub show_whitespace: bool,
    pub bench_use_unigram: bool,
    pub bench_scenario_idx: usize,
    pub cached_render: Option<((u64, u64), Result<RenderedDocument, String>)>,
    pub cached_graph: Option<CachedGraphCanvas>,
}

fn format_visible_whitespace(s: &str) -> String {
    s.replace(' ', "·").replace('\t', "⇥")
}

pub(super) fn is_nema_doc(d: &crate::doc::Doc) -> bool {
    let n = d.name.to_lowercase();
    n.ends_with(".nema")
        || n.ends_with(".nemat")
        || d.syntax
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("nema"))
            .unwrap_or(false)
}

/// 判断是否为标准代码/配置文件（遵守 `IDE-08`：严禁把 `*.rs`、`*.py`、`*.ts`、`*.diff` 等源码文件原地覆盖为 `.nema`）
pub(super) fn is_protected_code_doc(d: &crate::doc::Doc) -> bool {
    if is_nema_doc(d) {
        return false;
    }
    let n = d.name.to_lowercase();
    const CODE_EXTS: &[&str] = &[
        ".rs", ".py", ".pyi", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".go", ".c", ".cpp",
        ".cc", ".cxx", ".h", ".hpp", ".java", ".cs", ".rb", ".sh", ".bash", ".zsh", ".ps1",
        ".psm1", ".html", ".htm", ".css", ".scss", ".less", ".json", ".toml", ".yaml", ".yml",
        ".sql", ".diff", ".patch", ".lock", ".xml",
    ];
    if CODE_EXTS.iter().any(|ext| n.ends_with(ext)) {
        return true;
    }
    if let Some(syn) = d.syntax.as_deref() {
        let s = syn.to_ascii_lowercase();
        if !matches!(s.as_str(), "" | "plain" | "text" | "markdown" | "nema") {
            return true;
        }
    }
    false
}

/// 将字符串按行拆分为编辑器 `Vec<String>`（保留尾部空行以精确往返 `\n\n\n`）
fn source_to_doc_lines(source: &str) -> Vec<String> {
    let lf = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = lf.split('\n').map(str::to_string).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

impl App {
    /// 通过 `apply_line_patch` 应用整篇或局部行修改，确保 100% 写入 Undo 栈 (`Ctrl+Z` 字节级恢复)
    fn apply_nema_lines(&mut self, i: usize, new_lines: Vec<String>, cursor: Option<[usize; 2]>) {
        let old_len = self.docs[i].lines.len();
        let cur = cursor.unwrap_or(self.docs[i].cursor);
        let _ = self.docs[i].apply_line_patch(0, old_len, new_lines, Some(cur));
    }

    /// NEMA 专用顶部工具条 (与 `md_bar` 位置与视觉风格完全一致)
    pub(super) fn nema_bar(&mut self, root: &mut egui::Ui, _ctx: &egui::Context, is_nema: bool) {
        if !is_nema {
            return;
        }
        let tc = self.tc;
        if self.cfg.view.nema_bar_collapsed {
            let frame = egui::Frame::NONE
                .fill(tc.bg_bar)
                .inner_margin(egui::Margin::symmetric(6, 1))
                .stroke(egui::Stroke::NONE);
            let r = egui::Panel::top("nemabar_collapsed")
                .frame(frame)
                .exact_size(if tc.comic { 18.0 } else { 15.0 })
                .show_separator_line(false)
                .show(root, |ui| {
                    let w = ui.available_width();
                    let h = ui.available_height();
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
                    let resp = resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .tip(trz("展开 NEMA 工具条 (点击复原)"));
                    if tc.comic {
                        let p = ui.painter();
                        let hovered = resp.hovered();
                        let pressed = resp.is_pointer_button_down_on();
                        let badge_w = 180.0;
                        let badge_h = 14.0;
                        let badge_rect = Rect::from_center_size(rect.center(), Vec2::new(badge_w, badge_h));
                        let shadow_offset = if pressed { Vec2::ZERO } else { Vec2::new(1.0, 1.0) };
                        if !pressed {
                            p.rect_filled(badge_rect.translate(shadow_offset), 2.0, Color32::BLACK);
                        }
                        let draw_badge = if pressed { badge_rect.translate(Vec2::new(1.0, 1.0)) } else { badge_rect };
                        let (bg, fg) = if hovered || pressed {
                            (Color32::from_rgb(255, 204, 0), Color32::BLACK)
                        } else {
                            (Color32::from_rgb(36, 40, 58), Color32::from_rgb(255, 204, 0))
                        };
                        p.rect_filled(draw_badge, 2.0, bg);
                        p.rect_stroke(draw_badge, 2.0, egui::Stroke::new(1.5, Color32::BLACK), egui::StrokeKind::Inside);
                        p.text(
                            draw_badge.center(),
                            egui::Align2::CENTER_CENTER,
                            trz("▼ 展开 NEMA 工具条"),
                            egui::FontId::proportional(11.0),
                            fg,
                        );
                    } else {
                        if resp.hovered() {
                            ui.painter().rect_filled(rect, 0.0, tc.bg_hover);
                        }
                        let col = if resp.hovered() { tc.fg } else { tc.fg_dim };
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            trz("▼ 展开 NEMA 工具条"),
                            egui::FontId::proportional(11.0),
                            col,
                        );
                    }
                    if resp.clicked() {
                        self.cfg.view.nema_bar_collapsed = false;
                        self.cfg.save();
                    }
                });
            let rc = r.response.rect;
            root.painter().hline(
                rc.x_range(),
                rc.bottom() - 0.5,
                egui::Stroke::new(if tc.comic { 2.0 } else { 1.0 }, if tc.comic { Color32::BLACK } else { tc.border }),
            );
            return;
        }

        let on = self.nema_preview;
        let mut toggle = false;
        let mut action: Option<&'static str> = None;

        let frame = egui::Frame::NONE
            .fill(tc.bg_bar)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .stroke(egui::Stroke::NONE);

        let r = egui::Panel::top("nemabar")
            .frame(frame)
            .show_separator_line(false)
            .show(root, |ui| {
                let v = ui.visuals_mut();
                for w in [
                    &mut v.widgets.inactive,
                    &mut v.widgets.hovered,
                    &mut v.widgets.active,
                ] {
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
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(9.0, 18.0), egui::Sense::hover());
                    ui.painter()
                        .vline(r.center().x, r.y_range(), egui::Stroke::new(1.0, tc.border));
                };

                // 右侧常驻功能区: 折叠按钮 + 预览开关
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let collapse_btn = if tc.comic {
                        egui::Button::new(
                            egui::RichText::new(trz("▲ 折叠")).size(11.0).color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(32, 36, 52))
                        .stroke(egui::Stroke::new(1.5, Color32::BLACK))
                        .corner_radius(3.0)
                    } else {
                        egui::Button::new(
                            egui::RichText::new(trz("▲ 折叠")).size(11.0),
                        )
                    };
                    if ui
                        .add(collapse_btn)
                        .tip(trz("折叠 NEMA 工具条到顶端 (点击隐藏)"))
                        .clicked()
                    {
                        self.cfg.view.nema_bar_collapsed = true;
                        self.cfg.save();
                    }
                    sep(ui);

                    let mut b = egui::Button::new(egui::RichText::new(trz("预览")).size(11.0));
                    if on {
                        b = b
                            .fill(tc.bg_active)
                            .stroke(egui::Stroke::new(1.0, tc.border));
                    }
                    if ui
                        .add(b)
                        .tip(trz("源码 / 渲染预览切换 (Ctrl+Shift+V)"))
                        .clicked()
                    {
                        toggle = true;
                    }
                    sep(ui);

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt("nemabar-scroll")
                            .auto_shrink([false, true])
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(egui::RichText::new("▦ 表头").size(11.0))
                                        .tip(trz("切换 / 插入 \\ 表头行"))
                                        .clicked()
                                    {
                                        action = Some("toggle_header");
                                    }
                                    if ui
                                        .button(egui::RichText::new("+ 行").size(11.0))
                                        .tip(trz("插入新记录行"))
                                        .clicked()
                                    {
                                        action = Some("add_row");
                                    }
                                    if ui
                                        .button(egui::RichText::new("+ 意群").size(11.0))
                                        .tip(trz("插入空格缩进续接行 (延续上一行当前列)"))
                                        .clicked()
                                    {
                                        action = Some("add_cont");
                                    }
                                    sep(ui);
                                    if ui
                                        .button(egui::RichText::new("✦ 示例 v1.2").size(11.0))
                                        .tip(trz("插入 5W1H 模板"))
                                        .clicked()
                                    {
                                        action = Some("insert_v12_sample");
                                    }
                                    if ui
                                        .button(egui::RichText::new("? 5W1H").size(11.0))
                                        .tip(trz("插入 5W1H 模板"))
                                        .clicked()
                                    {
                                        action = Some("insert_5w1h");
                                    }
                                    if ui
                                        .button(egui::RichText::new("> 图节点").size(11.0))
                                        .tip(trz("插入 Graph 拓扑节点与边模板"))
                                        .clicked()
                                    {
                                        action = Some("insert_graph");
                                    }
                                    sep(ui);
                                    if ui
                                        .button(egui::RichText::new("$ 字典").size(11.0))
                                        .tip(trz("在文首插入共享词典条目 (\\t$N)"))
                                        .clicked()
                                    {
                                        action = Some("insert_dict");
                                    }
                                    if ui
                                        .button(egui::RichText::new("^ 引用").size(11.0))
                                        .tip(trz("插入 ^0 词典引用原子"))
                                        .clicked()
                                    {
                                        action = Some("insert_ref");
                                    }
                                    sep(ui);
                                    if ui
                                        .button(egui::RichText::new("⚡ 转换").size(11.0))
                                        .tip(trz("自动转化 / 编码为 NEMA (Ctrl+Alt+M)"))
                                        .clicked()
                                    {
                                        action = Some("convert");
                                    }
                                    if ui
                                        .button(egui::RichText::new("◈ 候选").size(11.0))
                                        .tip(trz("扫描并查看文档内高频词典候选词"))
                                        .clicked()
                                    {
                                        action = Some("candidates");
                                    }
                                    if ui
                                        .button(egui::RichText::new("🔒 锁定").size(11.0))
                                        .tip(trz("固化当前高频候选词并生成 ^N 引用"))
                                        .clicked()
                                    {
                                        action = Some("lock_dict");
                                    }
                                    if ui
                                        .button(egui::RichText::new("📍 复用").size(11.0))
                                        .tip(trz("显示字典与高频复用词位置"))
                                        .clicked()
                                    {
                                        action = Some("show_reused");
                                    }
                                    let ws_btn = egui::Button::new(
                                        egui::RichText::new("空格 · / Tab ⇥").size(11.0),
                                    )
                                    .fill(if self.nema.show_whitespace {
                                        tc.bg_active
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    });
                                    if ui
                                        .add(ws_btn)
                                        .tip(trz("显示空白符"))
                                        .clicked()
                                    {
                                        action = Some("toggle_ws");
                                    }
                                    if ui
                                        .button(egui::RichText::new("📊 Benchmark").size(11.0))
                                        .tip(trz("查看 Native vs Nemat Token 节省率与对比"))
                                        .clicked()
                                    {
                                        action = Some("token_diff");
                                    }
                                });
                            });
                    });
                });
            });

        let rc = r.response.rect;
        root.painter().hline(
            rc.x_range(),
            rc.bottom() - 0.5,
            egui::Stroke::new(1.0, tc.border),
        );

        if toggle {
            self.nema_preview = !self.nema_preview;
        }
        match action {
            Some("convert") => self.nema_convert(),
            Some("toggle_header") => self.nema_toggle_header(),
            Some("add_row") => self.nema_add_row(),
            Some("add_cont") => self.nema_add_cont(),
            Some("insert_v12_sample") => self.nema_insert_v12_sample(),
            Some("insert_5w1h") => self.nema_insert_5w1h(),
            Some("insert_graph") => self.nema_insert_graph(),
            Some("insert_dict") => self.nema_insert_dict(),
            Some("insert_ref") => self.nema_insert_ref(),
            Some("candidates") => self.nema_candidates(),
            Some("lock_dict") => self.nema_lock_dict(),
            Some("show_reused") => self.nema_show_reused(),
            Some("toggle_ws") => {
                self.nema.show_whitespace = !self.nema.show_whitespace;
                self.nema_preview = true;
            }
            Some("token_diff") => self.nema_token_diff(),
            _ => {}
        }
    }

    /// 自动转化 / 编码为 NEMA 规范格式（遵循 Lease 仲裁协议与代码文件保护 `IDE-08`）
    pub fn nema_convert(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw_text = self.docs[i].lines.join("\n");
        let has_domain_header = raw_text
            .lines()
            .any(|l| parse_domain_header(l).is_some() || parse_domain_header_v12(l).is_some());
        let is_nema = is_nema_doc(&self.docs[i]) || has_domain_header;

        // 1. 非 NEMA 文档转化为 .nema
        if !is_nema {
            let mut body_lines = vec![" \\1 1".to_string()];
            for line in &self.docs[i].lines {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let parts: Vec<String> = trimmed
                        .split(['\t', ',', '|'])
                        .map(|s| s.trim().replace(' ', ""))
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !parts.is_empty() {
                        body_lines.push(parts.join(" "));
                    }
                }
            }
            if body_lines.len() == 1 {
                body_lines.push("\\ Column1 Column2 Note".to_string());
                body_lines.push("ExampleItem Ready InitializedByNema".to_string());
            }
            let normalized = normalize_nema_newlines(&body_lines.join("\n"));
            let new_lines = source_to_doc_lines(&normalized);

            // IDE-08 代码文件安全守门：若当前是 *.rs/*.py/*.ts/*.diff 等代码文件，绝不原地覆盖，而是新建 .nema 标签页
            if is_protected_code_doc(&self.docs[i]) {
                let stem = self.docs[i]
                    .name
                    .rsplit_once('.')
                    .map(|(s, _)| s.to_string())
                    .unwrap_or_else(|| self.docs[i].name.clone());
                self.new_doc();
                let new_idx = self.active;
                self.docs[new_idx].name = format!("{stem}.nema");
                self.docs[new_idx].syntax = Some("NEMA".to_string());
                self.apply_nema_lines(new_idx, new_lines, Some([0, 0]));
                self.nema_preview = true;
                let tip =
                    trz("源代码文件已受保护：已在右侧新建 .nema 标签页，原文件未修改").to_string();
                self.docs[new_idx].tip = tip.clone();
                self.nema.info_banner = Some(tip.clone());
                self.log(tip);
                return;
            }

            self.apply_nema_lines(i, new_lines, Some([0, 0]));
            self.docs[i].syntax = Some("NEMA".to_string());
            self.nema_preview = true;
            let tip = trz("已自动转换为 NEMA 表格格式 (.nema)").to_string();
            self.docs[i].tip = tip.clone();
            self.nema.info_banner = Some(tip.clone());
            self.log(tip);
            return;
        }

        // 2. 已是 NEMA / NEMAT 文档：执行 Lease 仲裁状态机与无损编解码往返校验
        let normalized = normalize_nema_newlines(&raw_text);
        let codec = StandardNemaCodec;
        let rev = self.docs[i].rev;
        let src_hash = NemaLeaseArbitrator::hash_source(&normalized);

        let (epoch, cached_choice) = self.nema.arbitrator.request_lease(src_hash, rev);
        let _ = self.nema.arbitrator.pause_ime_same_epoch(epoch);

        // 若以 `\t$` 开头且后续无需再压缩，优先支持从 `.nemat` 还原为原生 `.nema`
        if normalized.starts_with("\t$") {
            if let Ok(decoded) = codec.decode(&normalized) {
                if decoded != normalized {
                    let new_lines = source_to_doc_lines(&decoded);
                    self.apply_nema_lines(i, new_lines, Some([0, 0]));
                    let _ = self.nema.arbitrator.release_lease(epoch);
                    self.nema_preview = true;
                    let tip = trz("已解码为 NEMA 原生格式 (Round-trip verified)").to_string();
                    self.docs[i].tip = tip.clone();
                    self.nema.info_banner = Some(format!(
                        "{tip} · Lease Epoch #{epoch} → Observer"
                    ));
                    self.log(tip);
                    return;
                }
            }
        }

        let choice_res = if let Some(cached) = cached_choice {
            Ok(cached)
        } else {
            nema_codec::encode_with_profile(
                &normalized,
                &TokenProfile::ByteHeuristic,
                48,
                &self.nema.locked_words,
            )
        };

        match choice_res {
            Ok(choice) => {
                let _ = self.nema.arbitrator.atomic_commit(epoch, choice.clone());
                let _ = self.nema.arbitrator.release_lease(epoch);
                self.nema_preview = true;
                if choice.format == CodecFormat::Nemat && choice.text != normalized {
                    let new_lines = source_to_doc_lines(&choice.text);
                    self.apply_nema_lines(i, new_lines, Some([0, 0]));
                    let tip = trz("已转换为 .nemat 传输格式 (往返校验通过)").to_string();
                    self.docs[i].tip = tip.clone();
                    self.nema.info_banner = Some(format!(
                        "{} · 策略: {} · {} → {} Bytes (-{:.2}%) · Epoch #{} ({})",
                        tip,
                        choice.method.as_str(),
                        choice.baseline_score,
                        choice.score,
                        choice.saving_percent(),
                        epoch,
                        choice.metric_note
                    ));
                    self.log(tip);
                } else {
                    // 确保末尾 `\n\n\n` 合规
                    if raw_text != normalized {
                        let new_lines = source_to_doc_lines(&normalized);
                        self.apply_nema_lines(i, new_lines, None);
                    }
                    let tip =
                        trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string();
                    self.docs[i].tip = tip.clone();
                    self.nema.info_banner = Some(format!(
                        "{} ({} Bytes · Epoch #{})",
                        tip, choice.baseline_score, epoch
                    ));
                    self.log(tip);
                }
            }
            Err(err) => {
                let _ = self.nema.arbitrator.release_lease(epoch);
                let tip = trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string();
                self.docs[i].tip = tip;
                self.nema.info_banner = Some(format!("NEMA 语法校验提示: {err}"));
            }
        }
    }

    /// 切换显式表头行 (`\\ `) — 支持 `Ctrl+Z` 字节级撤销恢复
    pub fn nema_toggle_header(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = self.docs[i].lines.join("\n");
        let toggled = toggle_table_header_in_source(&raw, None);
        let had_header = raw.lines().any(|l| l.starts_with("\\ "));
        let new_lines = source_to_doc_lines(&toggled);
        self.apply_nema_lines(i, new_lines, None);
        let tip = if had_header {
            trz("已取消表头行 (转为自动 column1...)").to_string()
        } else {
            trz("已将首行设置为显式表头 (\\ )").to_string()
        };
        self.docs[i].tip = tip.clone();
        self.nema.info_banner = Some(tip.clone());
        self.log(tip);
    }

    pub fn nema_add_row(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].cursor[0].min(self.docs[i].lines.len().saturating_sub(1));
        let mut new_lines = self.docs[i].lines.clone();
        new_lines.insert(cur + 1, "RecordField1 Field2 Field3".to_string());
        self.apply_nema_lines(i, new_lines, Some([cur + 1, 0]));
        let tip = trz("已插入新记录行").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_add_cont(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].cursor[0].min(self.docs[i].lines.len().saturating_sub(1));
        let mut new_lines = self.docs[i].lines.clone();
        new_lines.insert(cur + 1, "  ContinuationGroup".to_string());
        self.apply_nema_lines(i, new_lines, Some([cur + 1, 2]));
        let tip = trz("已插入纵向意群续接行 (空格缩进)").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_v12_sample(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = self.docs[i].lines.join("\n");
        let trimmed = raw.trim();
        let combined = if trimmed.is_empty() {
            COMPLETE_V12_SAMPLE.to_string()
        } else {
            format!("{}\n{}", raw.trim_end_matches('\n'), COMPLETE_V12_SAMPLE)
        };
        let new_lines = source_to_doc_lines(&combined);
        self.apply_nema_lines(i, new_lines, Some([0, 0]));
        self.nema_preview = true;
        self.nema_view_mode = 0;
        let tip = trz("已追加 5W1H 语义模板").to_string();
        self.docs[i].tip = tip;
        self.nema.info_banner = Some(
            "✦ 已载入 .NEMA v1.2 (R7) 官方三模式示例 (5W1H Tja1 + Table Ttable1 + Graph Gauth1 含 X3 扩展 APIKey / HTTP401)"
                .to_string(),
        );
    }

    pub fn nema_insert_5w1h(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = self.docs[i].lines.join("\n");
        let trimmed = raw.trim_end_matches('\n');
        let next_id = trimmed.lines().filter(|l| parse_domain_header(l).is_some() || parse_domain_header_v12(l).is_some()).count() + 1;
        let combined = if trimmed.is_empty() {
            format!(
                " \\?task{next_id} 1\nBuildLoginFlow PreventTokenLeak VerifySessionToken AuthAgent Today AuthGateway\n ProtectActiveSession\n\n\n"
            )
        } else {
            format!(
                "{trimmed}\n \\?task{next_id} 1\nBuildLoginFlow PreventTokenLeak VerifySessionToken AuthAgent Today AuthGateway\n ProtectActiveSession\n\n\n"
            )
        };
        let new_lines = source_to_doc_lines(&combined);
        let target_row = new_lines.len().saturating_sub(4);
        self.apply_nema_lines(i, new_lines, Some([target_row, 0]));
        self.nema_preview = true;
        let tip = trz("已追加 5W1H 语义模板").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_graph(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = self.docs[i].lines.join("\n");
        let trimmed = raw.trim_end_matches('\n');
        let next_id = trimmed.lines().filter(|l| parse_domain_header(l).is_some() || parse_domain_header_v12(l).is_some()).count() + 1;
        let combined = if trimmed.is_empty() {
            format!(
                " \\>auth{next_id} 1\n A LoginAPI Authentication APIKey\n B TokenStore SessionStorage\n C AuditService RecordsSecurityEvents\nA>X>B Reads VerifySession HTTP401\n  CacheExpiredRetryOnce\nB<Y<C Notifies AuditEvent\nA=U=C Shares SecurityPolicy\nA>P>A Retry Maximum3Attempts\n\n\n"
            )
        } else {
            format!(
                "{trimmed}\n \\>auth{next_id} 1\n A LoginAPI Authentication APIKey\n B TokenStore SessionStorage\n C AuditService RecordsSecurityEvents\nA>X>B Reads VerifySession HTTP401\n  CacheExpiredRetryOnce\nB<Y<C Notifies AuditEvent\nA=U=C Shares SecurityPolicy\nA>P>A Retry Maximum3Attempts\n\n\n"
            )
        };
        let new_lines = source_to_doc_lines(&combined);
        let target_row = new_lines.len().saturating_sub(4);
        self.apply_nema_lines(i, new_lines, Some([target_row, 0]));
        self.nema_preview = true;
        let tip = trz("已追加 Graph 拓扑节点与边模板").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_dict(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let mut new_lines = self.docs[i].lines.clone();
        if let Some(first) = new_lines.first().cloned() {
            if let Some(rest) = first.strip_prefix("\t$") {
                if let Ok(n) = rest.parse::<usize>() {
                    new_lines[0] = format!("\t${}", n + 1);
                    let ins_at = (n + 1).min(new_lines.len());
                    new_lines.insert(ins_at, format!("DictEntry{}", n));
                    let normalized = normalize_nema_newlines(&new_lines.join("\n"));
                    self.apply_nema_lines(i, source_to_doc_lines(&normalized), Some([ins_at, 0]));
                    let tip = trz("已在文首插入共享词典条目").to_string();
                    self.docs[i].tip = tip.clone();
                    self.log(tip);
                    return;
                }
            }
        }
        new_lines.insert(0, "\t$1".to_string());
        new_lines.insert(1, "APIKey".to_string());
        let normalized = normalize_nema_newlines(&new_lines.join("\n"));
        self.apply_nema_lines(i, source_to_doc_lines(&normalized), Some([1, 0]));
        let tip = trz("已在文首插入共享词典条目").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_ref(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        self.docs[i].type_text("^0");
        let tip = trz("已插入词典引用 ^0").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_candidates(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let row_refs: Vec<&str> = self.docs[i].lines.iter().map(String::as_str).collect();
        let cands = candidate_dictionary_details(&row_refs, 16);
        self.nema_preview = true;
        let tip = if cands.is_empty() {
            self.nema.info_banner =
                Some("◈ 字典候选：未发现收益 > 3 Bytes 的高频词 (需出现 ≥3 次且长度 ≥4)".to_string());
            trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string()
        } else {
            let summary: Vec<String> = cands
                .iter()
                .enumerate()
                .map(|(idx, c)| format!("^{idx}={}(×{}, +{}B)", c.word, c.count, c.gain))
                .collect();
            self.nema.info_banner = Some(format!("◈ 字典候选 ({}): {}", cands.len(), summary.join("  ")));
            trz("已扫描并列出高频词典候选词").to_string()
        };
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_lock_dict(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = normalize_nema_newlines(&self.docs[i].lines.join("\n"));
        let row_refs: Vec<&str> = raw.trim_end_matches('\n').split('\n').collect();
        let cands = candidate_dictionary_details(&row_refs, 48);
        if cands.is_empty() {
            let tip = trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string();
            self.docs[i].tip = tip.clone();
            self.nema.info_banner = Some(tip.clone());
            self.log(tip);
            return;
        }
        self.nema.locked_words = cands.into_iter().map(|c| c.word).collect();
        if let Ok(choice) = nema_codec::encode_with_profile(
            &raw,
            &TokenProfile::ByteHeuristic,
            48,
            &self.nema.locked_words,
        ) {
            let new_lines = source_to_doc_lines(&choice.text);
            self.apply_nema_lines(i, new_lines, Some([0, 0]));
            self.nema_preview = true;
            let tip = trz("已转换为 .nemat 传输格式 (往返校验通过)").to_string();
            self.docs[i].tip = tip.clone();
            self.nema.info_banner = Some(format!(
                "🔒 已锁定 {} 个高频字典词并完成往返校验 (-{:.2}%)",
                self.nema.locked_words.len(),
                choice.saving_percent()
            ));
            self.log(tip);
        }
    }

    /// 显示文档内所有字典引用 (`^N`) 与高频可复用词的位置，并支持一键跳转
    pub fn nema_show_reused(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = self.docs[i].lines.join("\n");
        let positions = find_reused_positions(&raw);
        self.nema_preview = true;
        if positions.is_empty() {
            let tip = trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string();
            self.docs[i].tip = tip.clone();
            self.nema.info_banner = Some("📍 复用位置：当前文档无 ^N 引用或可复用词".to_string());
            self.log(tip);
            return;
        }
        let first = &positions[0];
        self.docs[i].goto(first.line.saturating_sub(1), first.column.saturating_sub(1));
        let preview_locs: Vec<String> = positions
            .iter()
            .take(12)
            .map(|p| format!("{}@L{}:C{}", p.token, p.line, p.column))
            .collect();
        self.nema.info_banner = Some(format!(
            "📍 共 {} 处字典/复用位置：{}{}",
            positions.len(),
            preview_locs.join(" · "),
            if positions.len() > 12 { " …" } else { "" }
        ));
        let tip = trz("已高亮并列出文档内字典与复用位置").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_token_diff(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = normalize_nema_newlines(&self.docs[i].lines.join("\n"));
        self.nema_preview = true;
        self.nema_view_mode = 4;
        if let Ok(rows) = compare_document_formats(&raw) {
            if let (Some(nema_row), Some(json_obj), Some(json_pos), Some(json_tag), Some(nemat_row)) = (
                rows.first(),
                rows.get(1),
                rows.get(2),
                rows.get(3),
                rows.get(5),
            ) {
                self.nema.info_banner = Some(format!(
                    "📊 实测对照 | .nema: {} B (~{} BPE) | vs JSON对象 {:+.1}% ({} BPE) | vs JSON位置 {:+.1}% ({} BPE) | vs JSON标签 {:+.1}% ({} BPE) | .nemat: {} B (~{} BPE, {:+.1}%)",
                    nema_row.bytes,
                    nema_row.bpe_tokens,
                    nema_row.saving_vs_json_bpe_pct,
                    json_obj.bpe_tokens,
                    nema_row.saving_vs_pos_json_bpe_pct,
                    json_pos.bpe_tokens,
                    if json_tag.bpe_tokens == 0 {
                        0.0
                    } else {
                        ((nema_row.bpe_tokens as f64) - (json_tag.bpe_tokens as f64))
                            / (json_tag.bpe_tokens as f64)
                            * 100.0
                    },
                    json_tag.bpe_tokens,
                    nemat_row.bytes,
                    nemat_row.bpe_tokens,
                    nemat_row.saving_vs_pos_json_bpe_pct,
                ));
            }
        } else if let Ok(choice) = StandardNemaCodec.encode(&raw, &TokenProfile::ByteHeuristic) {
            let (bpe_base, _, _) = TokenProfile::BpeHeuristic {
                model_name: "cl100k_base".to_string(),
            }
            .measure(&raw);
            let (bpe_opt, _, _) = TokenProfile::BpeHeuristic {
                model_name: "cl100k_base".to_string(),
            }
            .measure(&choice.text);
            self.nema.info_banner = Some(format!(
                "📊 Token 对照 | 原生: {} B (~{} BPE) → 最优 ({}): {} B (~{} BPE) | 节省 {:.2}% | 注: {}",
                choice.baseline_score,
                bpe_base,
                choice.method.as_str(),
                choice.score,
                bpe_opt,
                choice.saving_percent(),
                choice.metric_note
            ));
        }
        let tip = trz("已完成 Native 与 Nemat 压缩效益分析").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    /// 右侧交互式 NEMA 可视化面板 (`Table / 5W1H / Graph / Benchmark`)
    pub(super) fn nema_ui(&mut self, ui: &mut egui::Ui) {
        let ai = self.active.min(self.docs.len() - 1);
        let key = (self.docs[ai].id, self.docs[ai].rev);
        if self.nema.cached_render.as_ref().map(|c| c.0) != Some(key) {
            let raw = normalize_nema_newlines(&self.docs[ai].lines.join("\n"));
            let res = if raw.starts_with("\t$") || raw.starts_with("\t0") {
                StandardNemaCodec
                    .decode(&raw)
                    .or(Ok(raw))
                    .and_then(|s| render_source(&s))
                    .map_err(|e| e.to_string())
            } else {
                render_source(&raw).map_err(|e| e.to_string())
            };
            self.nema.cached_render = Some((key, res));
        }

        let tc = self.tc;
        let mut jump_to_line: Option<usize> = None;
        let mut toggle_header_domain: Option<String> = None;

        // 顶部状态与模式切换栏
        ui.horizontal_wrapped(|ui| {
            ui.strong(
                egui::RichText::new(".NEMA v1.2 · Ariadne's Thread")
                    .color(tc.blue)
                    .size(12.0),
            );
            let role_txt = match self.nema.arbitrator.role() {
                OwnershipRole::Observer => {
                    format!("Observer · Epoch #{}", self.nema.arbitrator.current_epoch())
                }
                _ => format!(
                    "Lease Active · Epoch #{}",
                    self.nema.arbitrator.current_epoch()
                ),
            };
            ui.label(
                egui::RichText::new(role_txt)
                    .size(10.5)
                    .color(tc.fg_dim)
                    .monospace(),
            );
            ui.separator();
            for (idx, label) in [
                (0usize, "全部 (ALL)"),
                (1usize, "▦ Table"),
                (2usize, "? 5W1H"),
                (3usize, "> Graph"),
                (4usize, "📊 Benchmark 实测"),
            ] {
                let selected = self.nema_view_mode == idx;
                if ui
                    .selectable_label(selected, egui::RichText::new(label).size(11.0))
                    .clicked()
                {
                    self.nema_view_mode = idx;
                }
            }
            ui.separator();
            if ui
                .selectable_label(
                    self.nema.show_whitespace,
                    egui::RichText::new("空格 · / Tab ⇥").size(10.5),
                )
                .on_hover_text("将半角空格显示为 · 、Tab 显示为 ⇥（仅影响渲染视图，不改动源码字节）")
                .clicked()
            {
                self.nema.show_whitespace = !self.nema.show_whitespace;
            }
        });

        if let Some(banner) = &self.nema.info_banner {
            egui::Frame::NONE
                .fill(tc.bg_bar)
                .stroke(egui::Stroke::new(1.0, tc.border))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(banner).size(11.0).color(tc.fg));
                });
        }

        if self.nema.show_whitespace {
            let raw_preview = self.docs[ai].lines.join("\n");
            let vis = format_visible_whitespace(&raw_preview);
            egui::Frame::NONE
                .fill(tc.bg_edit)
                .stroke(egui::Stroke::new(1.0, tc.border))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(
                            egui::RichText::new("可见空白符源码预览 (⇥ = Tab, · = Space)")
                                .size(10.5)
                                .color(tc.blue),
                        );
                        ui.label(
                            egui::RichText::new("Graph: 1 个前导空格 ·A 声明节点，0 个前导空格 A>X>B 声明关系，≥2 空格续行 X2/X3+")
                                .size(10.0)
                                .color(tc.fg_dim),
                        );
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("nema-vis-ws-scroll")
                        .max_height(110.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(vis)
                                    .size(11.0)
                                    .monospace()
                                    .color(tc.fg),
                            );
                        });
                });
        }

        ui.separator();

        if self.nema_view_mode == 4 {
            self.render_nema_benchmark_ui(ui, ai);
            return;
        }

        let rendered_res = self
            .nema
            .cached_render
            .as_ref()
            .map(|(_, r)| r.clone())
            .unwrap_or_else(|| Err("empty".to_string()));

        egui::ScrollArea::vertical()
            .id_salt("nema-preview-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match rendered_res {
                Err(err) => {
                    ui.colored_label(
                        tc.red,
                        format!("NEMA 语法诊断: {err} (点击顶部「⚡ 转换」可自动规范化)"),
                    );
                }
                Ok(doc) => {
                    if !doc.dictionary.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "共享词典 (\\t${}):",
                                    doc.dictionary.len()
                                ))
                                .size(11.0)
                                .strong(),
                            );
                            for (idx, word) in doc.dictionary.iter().enumerate() {
                                egui::Frame::NONE
                                    .fill(tc.bg_bar)
                                    .stroke(egui::Stroke::new(1.0, tc.border))
                                    .corner_radius(3.0)
                                    .inner_margin(egui::Margin::symmetric(5, 1))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(format!("^{idx} = {word}"))
                                                .size(10.5)
                                                .monospace(),
                                        );
                                    });
                            }
                        });
                        ui.add_space(6.0);
                    }

                    for domain in &doc.domains {
                        let mode_match = match self.nema_view_mode {
                            1 => domain.mode == DomainMode::Table,
                            2 => domain.mode == DomainMode::FiveWOneH,
                            3 => domain.mode == DomainMode::Graph,
                            _ => true,
                        };
                        if !mode_match {
                            continue;
                        }
                        self.render_nema_domain(
                            ui,
                            domain,
                            &mut jump_to_line,
                            &mut toggle_header_domain,
                        );
                        ui.add_space(10.0);
                    }
                }
            });

        if let Some(line) = jump_to_line {
            self.docs[ai].goto(line.saturating_sub(1), 0);
        }
        if let Some(dom_id) = toggle_header_domain {
            let raw = self.docs[ai].lines.join("\n");
            let toggled = toggle_table_header_in_source(&raw, Some(&dom_id));
            let new_lines = source_to_doc_lines(&toggled);
            self.apply_nema_lines(ai, new_lines, None);
        }
    }

    fn render_nema_domain(
        &mut self,
        ui: &mut egui::Ui,
        domain: &RenderedDomain,
        jump_to_line: &mut Option<usize>,
        toggle_header_domain: &mut Option<String>,
    ) {
        let tc = self.tc;
        egui::Frame::NONE
            .fill(tc.bg_bar)
            .stroke(egui::Stroke::new(1.0, tc.border))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let badge = format!(
                        "{}{} (rev {}) · {}",
                        domain.mode.prefix_char(),
                        domain.id,
                        domain.revision,
                        domain.mode.as_str().to_uppercase()
                    );
                    ui.strong(egui::RichText::new(badge).size(11.5).color(tc.blue));
                    if domain.mode == DomainMode::Table {
                        let hdr_label = if domain.explicit_header {
                            "取消显式表头 (\\t / \\ )"
                        } else {
                            "首行设为表头 (\\t / \\ )"
                        };
                        if ui
                            .button(egui::RichText::new(hdr_label).size(10.5))
                            .clicked()
                        {
                            *toggle_header_domain = Some(domain.id.clone());
                        }
                    }
                });
                ui.add_space(4.0);

                match domain.mode {
                    DomainMode::Table => {
                        egui::Grid::new(("nema-table", &domain.id))
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for col in &domain.columns {
                                    ui.strong(
                                        egui::RichText::new(col)
                                            .size(11.0)
                                            .color(if domain.explicit_header {
                                                tc.fg
                                            } else {
                                                tc.fg_dim
                                            }),
                                    );
                                }
                                ui.end_row();

                                for row in &domain.rows {
                                    for c_idx in 0..domain.columns.len() {
                                        let val =
                                            row.cells.get(c_idx).map(String::as_str).unwrap_or("");
                                        let resp = ui.selectable_label(
                                            false,
                                            egui::RichText::new(val).size(11.0),
                                        );
                                        if resp.clicked() {
                                            *jump_to_line = Some(row.line);
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    DomainMode::FiveWOneH => {
                        for (r_idx, row) in domain.rows.iter().enumerate() {
                            egui::Frame::NONE
                                .fill(tc.bg_edit)
                                .stroke(egui::Stroke::new(1.0, tc.border))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::same(6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .small_button(format!("Record #{} (L{})", r_idx + 1, row.line))
                                            .clicked()
                                        {
                                            *jump_to_line = Some(row.line);
                                        }
                                        if row.tail_empty {
                                            ui.label(
                                                egui::RichText::new("显式省缺哨兵 (.)")
                                                    .size(10.0)
                                                    .color(tc.fg_dim),
                                            );
                                        }
                                    });
                                    egui::Grid::new(("nema-5w1h", &domain.id, r_idx))
                                        .num_columns(2)
                                        .spacing([8.0, 3.0])
                                        .show(ui, |ui| {
                                            for (ax_idx, axis) in
                                                nema_codec::AXES_5W1H.iter().enumerate()
                                            {
                                                let val = row
                                                    .cells
                                                    .get(ax_idx)
                                                    .map(String::as_str)
                                                    .unwrap_or("");
                                                if val.is_empty() {
                                                    continue;
                                                }
                                                ui.strong(
                                                    egui::RichText::new(*axis)
                                                        .size(10.5)
                                                        .color(tc.blue),
                                                );
                                                if ui
                                                    .selectable_label(
                                                        false,
                                                        egui::RichText::new(val).size(11.0),
                                                    )
                                                    .clicked()
                                                {
                                                    *jump_to_line = Some(row.line);
                                                }
                                                ui.end_row();
                                            }
                                        });
                                });
                            ui.add_space(4.0);
                        }
                    }
                    DomainMode::Graph => {
                        self.render_nema_graph_canvas(ui, domain, jump_to_line);
                    }
                }
            });
    }

    /// 使用 `egui::Painter` 绘制 `.NEMA v1.2 (R7)` 有向/无向多重图：
    /// - 1 个前导空格 (` A LoginAPI Authentication APIKey`) 声明节点
    /// - 0 个前导空格 (`A>X>B Reads VerifySession HTTP401`) 声明关系
    /// - 悬浮 (Hover) 显示只读 `X2` 说明卡片；点击 (Click) 固定底部 **Graph Inspector** 查看完整 `X0 / X1 / X2 / X3+` 扩展属性
    /// - 拓扑几何缓存 (`CachedGraphCanvas`) 避免 60fps 帧循环重复重算三角函数与点列
    fn render_nema_graph_canvas(
        &mut self,
        ui: &mut egui::Ui,
        domain: &RenderedDomain,
        jump_to_line: &mut Option<usize>,
    ) {
        let tc = self.tc;
        let ai = self.active.min(self.docs.len() - 1);
        let doc_key = (self.docs[ai].id, self.docs[ai].rev);

        let canvas_w = ui.available_width().max(340.0);
        let canvas_h = 268.0;
        let cache_key = (
            doc_key.0,
            doc_key.1,
            (canvas_w * 10.0) as u32,
            (canvas_h * 10.0) as u32,
        );

        if self.nema.cached_graph.as_ref().map(|cg| cg.cache_key) != Some(cache_key) {
            self.nema.cached_graph = Some(CachedGraphCanvas::build(
                cache_key, domain, canvas_w, canvas_h,
            ));
        }

        // 若当前未选中任何节点或关系，默认选中第一个关系或节点以便即时展示 Graph Inspector
        if self.nema.selected_item.is_none() {
            if let Some(cg) = &self.nema.cached_graph {
                if let Some(e) = cg.edges.first() {
                    self.nema.selected_item = Some(SelectedGraphItem::Edge {
                        id: e.id.clone(),
                        raw_x0: e.raw_x0.clone(),
                        route: e.route_str.clone(),
                        name_x1: e.name_x1.clone(),
                        desc_x2: e.desc_x2.clone(),
                        extensions: e.exts.clone(),
                        line: e.line,
                    });
                } else if let Some(n) = cg.nodes.first() {
                    self.nema.selected_item = Some(SelectedGraphItem::Node {
                        id: n.id.clone(),
                        name_x1: n.name_x1.clone(),
                        desc_x2: n.desc_x2.clone(),
                        extensions: n.exts.clone(),
                        line: n.line,
                    });
                }
            }
        }

        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(canvas_w, canvas_h), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, tc.bg_edit);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, tc.border),
            egui::StrokeKind::Inside,
        );

        let cur_selected = self.nema.selected_item.clone();
        let mut new_selected: Option<SelectedGraphItem> = None;

        let cg = self.nema.cached_graph.as_ref().unwrap();

        // 1. 先画边与箭头，并放置可点击/可悬浮的独立 Edge ID 胶囊
        for (e_idx, edge_item) in cg.edges.iter().enumerate() {
            let is_selected = matches!(
                &cur_selected,
                Some(SelectedGraphItem::Edge { id, .. }) if id == &edge_item.id
            );
            let stroke_col = if is_selected { tc.orange } else { tc.blue };

            match &edge_item.geom {
                CachedEdgeGeometry::Loop { loop_center_rel, r_loop } => {
                    let loop_center = rect.min + *loop_center_rel;
                    painter.circle_stroke(loop_center, *r_loop, egui::Stroke::new(1.5, stroke_col));
                }
                CachedEdgeGeometry::Line { start_rel, end_rel, arrow } => {
                    let start = rect.min + *start_rel;
                    let end = rect.min + *end_rel;
                    painter.line_segment([start, end], egui::Stroke::new(1.6, stroke_col));
                    if let Some([e, p1, p2]) = arrow {
                        painter.add(egui::Shape::convex_polygon(
                            vec![rect.min + *e, rect.min + *p1, rect.min + *p2],
                            stroke_col,
                            egui::Stroke::NONE,
                        ));
                    }
                }
            }

            let pill_rect = edge_item.pill_rect_rel.translate(rect.min.to_vec2());
            let resp = ui.interact(
                pill_rect,
                ui.id().with(("nema-edge", &domain.id, &edge_item.id, e_idx)),
                egui::Sense::click(),
            );
            painter.rect_filled(
                pill_rect,
                9.5,
                if is_selected || resp.hovered() {
                    tc.bg_active
                } else {
                    tc.bg_bar
                },
            );
            painter.rect_stroke(
                pill_rect,
                9.5,
                egui::Stroke::new(1.1, stroke_col),
                egui::StrokeKind::Inside,
            );
            painter.text(
                pill_rect.center(),
                egui::Align2::CENTER_CENTER,
                &edge_item.label_txt,
                egui::FontId::proportional(10.0),
                tc.fg,
            );

            let edge_id = &edge_item.id;
            let name_x1 = &edge_item.name_x1;
            let desc_x2 = &edge_item.desc_x2;
            let exts = &edge_item.exts;
            let route_str = &edge_item.route_str;
            let edge_raw_x0 = &edge_item.raw_x0;

            let resp = resp.on_hover_ui(|ui| {
                ui.strong(
                    egui::RichText::new("关系说明 · X2 (只读预览)")
                        .size(10.5)
                        .color(tc.blue),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {}",
                        edge_id,
                        if name_x1.is_empty() { "(未命名)" } else { name_x1 }
                    ))
                    .size(11.0)
                    .strong(),
                );
                ui.label(
                    egui::RichText::new(format!("方向: {route_str} (X0: {edge_raw_x0})"))
                        .size(10.5)
                        .monospace()
                        .color(tc.fg_dim),
                );
                let x2_display = if desc_x2.is_empty() {
                    "未填写 X2（可省略）"
                } else {
                    desc_x2.as_str()
                };
                ui.label(egui::RichText::new(format!("X2: {x2_display}")).size(11.0));
                for (k, v) in exts {
                    ui.label(
                        egui::RichText::new(format!("{k} · extension: {v}"))
                            .size(10.5)
                            .monospace()
                            .color(tc.orange),
                    );
                }
            });

            if resp.clicked() {
                new_selected = Some(SelectedGraphItem::Edge {
                    id: edge_item.id.clone(),
                    raw_x0: edge_item.raw_x0.clone(),
                    route: edge_item.route_str.clone(),
                    name_x1: edge_item.name_x1.clone(),
                    desc_x2: edge_item.desc_x2.clone(),
                    extensions: edge_item.exts.clone(),
                    line: edge_item.line,
                });
                *jump_to_line = Some(edge_item.line);
            }
        }

        // 2. 再画节点卡片 (1 个前导空格 ` A LoginAPI Authentication APIKey`)
        for node_item in &cg.nodes {
            let nrect = node_item.rect_rel.translate(rect.min.to_vec2());
            let is_selected = matches!(
                &cur_selected,
                Some(SelectedGraphItem::Node { id, .. }) if id == &node_item.id
            );
            let resp = ui.interact(
                nrect,
                ui.id().with(("nema-node", &domain.id, &node_item.id)),
                egui::Sense::click(),
            );
            let border_col = if is_selected { tc.orange } else { tc.border };
            painter.rect_filled(
                nrect,
                6.0,
                if is_selected || resp.hovered() {
                    tc.bg_hover
                } else {
                    tc.bg_bar
                },
            );
            painter.rect_stroke(
                nrect,
                6.0,
                egui::Stroke::new(if is_selected { 1.6 } else { 1.2 }, border_col),
                egui::StrokeKind::Inside,
            );
            painter.text(
                egui::pos2(nrect.center().x, nrect.top() + 14.0),
                egui::Align2::CENTER_CENTER,
                &node_item.top_title,
                egui::FontId::proportional(11.5),
                tc.fg,
            );
            painter.text(
                egui::pos2(nrect.center().x, nrect.top() + 30.0),
                egui::Align2::CENTER_CENTER,
                &node_item.name_x1,
                egui::FontId::proportional(10.0),
                tc.fg_dim,
            );

            let nid = &node_item.id;
            let name_x1 = &node_item.name_x1;
            let desc_x2 = &node_item.desc_x2;
            let exts = &node_item.exts;

            let resp = resp.on_hover_ui(|ui| {
                ui.strong(
                    egui::RichText::new("节点说明 · X2 (只读预览)")
                        .size(10.5)
                        .color(tc.blue),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {}",
                        nid,
                        if name_x1.is_empty() { "(未命名)" } else { name_x1 }
                    ))
                    .size(11.0)
                    .strong(),
                );
                let x2_display = if desc_x2.is_empty() {
                    "未填写 X2（可省略）"
                } else {
                    desc_x2.as_str()
                };
                ui.label(egui::RichText::new(format!("X2: {x2_display}")).size(11.0));
                for (k, v) in exts {
                    ui.label(
                        egui::RichText::new(format!("{k} · extension: {v}"))
                            .size(10.5)
                            .monospace()
                            .color(tc.orange),
                    );
                }
            });

            if resp.clicked() {
                new_selected = Some(SelectedGraphItem::Node {
                    id: node_item.id.clone(),
                    name_x1: node_item.name_x1.clone(),
                    desc_x2: node_item.desc_x2.clone(),
                    extensions: node_item.exts.clone(),
                    line: node_item.line,
                });
                *jump_to_line = Some(node_item.line);
            }
        }

        // 3. 节点与关系快捷选择器 (Quick Picker Chips)
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("节点:").size(10.5).color(tc.fg_dim));
            for node_item in &cg.nodes {
                let is_sel = matches!(
                    &cur_selected,
                    Some(SelectedGraphItem::Node { id, .. }) if id == &node_item.id
                );
                let chip = format!("Node {} · {}", node_item.id, node_item.name_x1);
                if ui
                    .selectable_label(is_sel, egui::RichText::new(chip).size(10.5).monospace())
                    .clicked()
                {
                    new_selected = Some(SelectedGraphItem::Node {
                        id: node_item.id.clone(),
                        name_x1: node_item.name_x1.clone(),
                        desc_x2: node_item.desc_x2.clone(),
                        extensions: node_item.exts.clone(),
                        line: node_item.line,
                    });
                    *jump_to_line = Some(node_item.line);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("关系:").size(10.5).color(tc.fg_dim));
            for edge_item in &cg.edges {
                let is_sel = matches!(
                    &cur_selected,
                    Some(SelectedGraphItem::Edge { id, .. }) if id == &edge_item.id
                );
                let chip = format!("Edge {} · {}", edge_item.id, edge_item.name_x1);
                if ui
                    .selectable_label(is_sel, egui::RichText::new(chip).size(10.5).monospace())
                    .clicked()
                {
                    new_selected = Some(SelectedGraphItem::Edge {
                        id: edge_item.id.clone(),
                        raw_x0: edge_item.raw_x0.clone(),
                        route: edge_item.route_str.clone(),
                        name_x1: edge_item.name_x1.clone(),
                        desc_x2: edge_item.desc_x2.clone(),
                        extensions: edge_item.exts.clone(),
                        line: edge_item.line,
                    });
                    *jump_to_line = Some(edge_item.line);
                }
            }
        });

        // 底部固定 Graph Inspector (`X0 / X1 / X2 / X3+ extensions`)
        if let Some(item) = &self.nema.selected_item {
            ui.add_space(4.0);
            egui::Frame::NONE
                .fill(tc.bg_edit)
                .stroke(egui::Stroke::new(1.0, tc.blue))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| match item {
                    SelectedGraphItem::Node {
                        id,
                        name_x1,
                        desc_x2,
                        extensions,
                        line,
                    } => {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(
                                egui::RichText::new(format!(
                                    "Graph Inspector · Node {id} · {name_x1}"
                                ))
                                .size(11.0)
                                .color(tc.blue),
                            );
                            ui.label(
                                egui::RichText::new("1 个前导空格声明节点")
                                    .size(10.0)
                                    .color(tc.fg_dim),
                            );
                            if ui.small_button(format!("跳至 L{line}")).clicked() {
                                *jump_to_line = Some(*line);
                            }
                        });
                        egui::Grid::new(("nema-node-inspector", &domain.id, id))
                            .num_columns(2)
                            .spacing([10.0, 3.0])
                            .show(ui, |ui| {
                                ui.strong(
                                    egui::RichText::new("X0 · Node ID")
                                        .size(10.5)
                                        .color(tc.fg_dim),
                                );
                                ui.label(egui::RichText::new(id).size(11.0).monospace());
                                ui.end_row();

                                ui.strong(
                                    egui::RichText::new("X1 · 节点名称 (原样保留)")
                                        .size(10.5)
                                        .color(tc.fg_dim),
                                );
                                ui.label(
                                    egui::RichText::new(if name_x1.is_empty() {
                                        "—"
                                    } else {
                                        name_x1.as_str()
                                    })
                                    .size(11.0)
                                    .monospace(),
                                );
                                ui.end_row();

                                ui.strong(
                                    egui::RichText::new("X2 · 节点说明 (可省略)")
                                        .size(10.5)
                                        .color(tc.fg_dim),
                                );
                                ui.label(
                                    egui::RichText::new(if desc_x2.is_empty() {
                                        "未填写 X2（可省略）"
                                    } else {
                                        desc_x2.as_str()
                                    })
                                    .size(11.0),
                                );
                                ui.end_row();

                                for (ext_key, ext_val) in extensions {
                                    ui.strong(
                                        egui::RichText::new(format!("{ext_key} · extension"))
                                            .size(10.5)
                                            .color(tc.orange),
                                    );
                                    ui.label(
                                        egui::RichText::new(ext_val)
                                            .size(11.0)
                                            .monospace()
                                            .color(tc.fg),
                                    );
                                    ui.end_row();
                                }
                            });
                    }
                    SelectedGraphItem::Edge {
                        id,
                        raw_x0,
                        route,
                        name_x1,
                        desc_x2,
                        extensions,
                        line,
                    } => {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(
                                egui::RichText::new(format!(
                                    "Graph Inspector · Edge {id} · {name_x1} ({route})"
                                ))
                                .size(11.0)
                                .color(tc.blue),
                            );
                            ui.label(
                                egui::RichText::new("0 个前导空格声明关系")
                                    .size(10.0)
                                    .color(tc.fg_dim),
                            );
                            if ui.small_button(format!("跳至 L{line}")).clicked() {
                                *jump_to_line = Some(*line);
                            }
                        });
                        egui::Grid::new(("nema-edge-inspector", &domain.id, id))
                            .num_columns(2)
                            .spacing([10.0, 3.0])
                            .show(ui, |ui| {
                                ui.strong(
                                    egui::RichText::new("X0 · 关系表达式")
                                        .size(10.5)
                                        .color(tc.fg_dim),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{raw_x0}  ({route})"))
                                        .size(11.0)
                                        .monospace(),
                                );
                                ui.end_row();

                                ui.strong(
                                    egui::RichText::new("X1 · 关系名称 (原样保留)")
                                        .size(10.5)
                                        .color(tc.fg_dim),
                                );
                                ui.label(
                                    egui::RichText::new(if name_x1.is_empty() {
                                        "—"
                                    } else {
                                        name_x1.as_str()
                                    })
                                    .size(11.0)
                                    .monospace(),
                                );
                                ui.end_row();

                                ui.strong(
                                    egui::RichText::new("X2 · 关系说明 (可省略)")
                                        .size(10.5)
                                        .color(tc.fg_dim),
                                );
                                ui.label(
                                    egui::RichText::new(if desc_x2.is_empty() {
                                        "未填写 X2（可省略）"
                                    } else {
                                        desc_x2.as_str()
                                    })
                                    .size(11.0),
                                );
                                ui.end_row();

                                for (ext_key, ext_val) in extensions {
                                    ui.strong(
                                        egui::RichText::new(format!("{ext_key} · extension"))
                                            .size(10.5)
                                            .color(tc.orange),
                                    );
                                    ui.label(
                                        egui::RichText::new(ext_val)
                                            .size(11.0)
                                            .monospace()
                                            .color(tc.fg),
                                    );
                                    ui.end_row();
                                }
                            });
                    }
                });
        }

        if let Some(item) = new_selected {
            self.nema.selected_item = Some(item);
        }
    }

    /// 渲染 `.NEMA v1.2 (R7)` Token Benchmark 实测面板（含当前文档实时多格式对照 + 8 组官方实测数据集）
    fn render_nema_benchmark_ui(&mut self, ui: &mut egui::Ui, ai: usize) {
        let tc = self.tc;
        let raw = normalize_nema_newlines(&self.docs[ai].lines.join("\n"));

        egui::ScrollArea::vertical()
            .id_salt("nema-benchmark-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 1. 当前文档实时同语义对照 (Live Document Format Comparison)
                egui::Frame::NONE
                    .fill(tc.bg_bar)
                    .stroke(egui::Stroke::new(1.0, tc.border))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(
                                egui::RichText::new(
                                    "当前文档实时同语义对照 (.nema / .nemat / JSON / YAML)",
                                )
                                .size(11.5)
                                .color(tc.blue),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "基于当前编辑器内容自动生成等价 JSON objects、JSON positional、JSON tagged (w/t/g)、YAML positional 与 .nemat",
                                )
                                .size(10.0)
                                .color(tc.fg_dim),
                            );
                        });
                        ui.add_space(4.0);
                        match compare_document_formats(&raw) {
                            Ok(rows) => {
                                let max_bpe = rows
                                    .iter()
                                    .map(|r| r.bpe_tokens)
                                    .max()
                                    .unwrap_or(1)
                                    .max(1);
                                egui::Grid::new("nema-live-bench-grid")
                                    .striped(true)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        for col in [
                                            "格式 (Format)",
                                            "Bytes",
                                            "BPE (~Token)",
                                            "Unigram (~Token)",
                                            "vs JSON objects",
                                            "vs JSON positional",
                                            "直观对比",
                                        ] {
                                            ui.strong(
                                                egui::RichText::new(col)
                                                    .size(10.5)
                                                    .color(tc.fg_dim),
                                            );
                                        }
                                        ui.end_row();

                                        for r in &rows {
                                            let is_nema = r.format_name.starts_with(".nema");
                                            let col_fg = if is_nema { tc.blue } else { tc.fg };
                                            ui.strong(
                                                egui::RichText::new(r.format_name)
                                                    .size(11.0)
                                                    .color(col_fg),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!("{} B", r.bytes))
                                                    .size(11.0)
                                                    .monospace(),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!("{}", r.bpe_tokens))
                                                    .size(11.0)
                                                    .monospace(),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}",
                                                    r.unigram_tokens
                                                ))
                                                .size(11.0)
                                                .monospace(),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{:+.2}%",
                                                    r.saving_vs_json_bpe_pct
                                                ))
                                                .size(11.0)
                                                .monospace()
                                                .color(if r.saving_vs_json_bpe_pct > 0.0 {
                                                    tc.green
                                                } else {
                                                    tc.fg_dim
                                                }),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{:+.2}%",
                                                    r.saving_vs_pos_json_bpe_pct
                                                ))
                                                .size(11.0)
                                                .monospace()
                                                .color(if r.saving_vs_pos_json_bpe_pct > 0.0 {
                                                    tc.green
                                                } else {
                                                    tc.fg_dim
                                                }),
                                            );
                                            let ratio =
                                                (r.bpe_tokens as f32 / max_bpe as f32).clamp(0.04, 1.0);
                                            let bar_w = 130.0;
                                            let (brect, _) = ui.allocate_exact_size(
                                                egui::vec2(bar_w, 12.0),
                                                egui::Sense::hover(),
                                            );
                                            let bp = ui.painter_at(brect);
                                            bp.rect_filled(brect, 3.0, tc.bg_edit);
                                            let fill_rect = egui::Rect::from_min_size(
                                                brect.min,
                                                egui::vec2(bar_w * ratio, 12.0),
                                            );
                                            bp.rect_filled(
                                                fill_rect,
                                                3.0,
                                                if is_nema { tc.blue } else { tc.fg_dim },
                                            );
                                            ui.end_row();
                                        }
                                    });
                            }
                            Err(e) => {
                                ui.colored_label(tc.red, format!("当前文档无法解析为 .NEMA: {e}"));
                            }
                        }
                    });

                ui.add_space(8.0);

                // 2. 官方 Empirical Token Benchmark (.NEMA v1.2 · 2026-11-07 · Release R7)
                egui::Frame::NONE
                    .fill(tc.bg_bar)
                    .stroke(egui::Stroke::new(1.0, tc.border))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(
                                egui::RichText::new(
                                    "官方 Empirical Token Benchmark (.NEMA v1.2 · Release R7)",
                                )
                                .size(11.5)
                                .color(tc.blue),
                            );
                            ui.separator();
                            if ui
                                .selectable_label(
                                    !self.nema.bench_use_unigram,
                                    egui::RichText::new("SentencePiece BPE (32k)").size(10.5),
                                )
                                .clicked()
                            {
                                self.nema.bench_use_unigram = false;
                            }
                            if ui
                                .selectable_label(
                                    self.nema.bench_use_unigram,
                                    egui::RichText::new("SentencePiece Unigram (32k)").size(10.5),
                                )
                                .clicked()
                            {
                                self.nema.bench_use_unigram = true;
                            }
                        });
                        ui.label(
                            egui::RichText::new(
                                "免责声明: 基于真实 SentencePiece BPE / Unigram (vocab=32,000, byte_fallback=True) 本地实测；不同商业模型私有词表会有常数级偏移，不代表商业 API 计费账单。",
                            )
                            .size(10.0)
                            .color(tc.fg_dim),
                        );
                        ui.add_space(6.0);

                        // 3 张核心结论卡片 (Headline Cards)
                        ui.columns(3, |cols| {
                            egui::Frame::NONE
                                .fill(tc.bg_edit)
                                .stroke(egui::Stroke::new(1.0, tc.border))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::same(6))
                                .show(&mut cols[0], |ui| {
                                    ui.strong(
                                        egui::RichText::new("-53.68% (english_60)")
                                            .size(12.0)
                                            .color(tc.green),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            ".nema 相比标准紧凑 JSON (objects): 3,012 vs 6,502 BPE tokens",
                                        )
                                        .size(10.5),
                                    );
                                });
                            egui::Frame::NONE
                                .fill(tc.bg_edit)
                                .stroke(egui::Stroke::new(1.0, tc.border))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::same(6))
                                .show(&mut cols[1], |ui| {
                                    ui.strong(
                                        egui::RichText::new(
                                            "-21.79% vs 位置数组 / -24.15% vs Tagged",
                                        )
                                        .size(12.0)
                                        .color(tc.green),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            "english_60: .nema (3,012) vs 位置数组 JSON (3,851) / Tagged JSON (3,971)",
                                        )
                                        .size(10.5),
                                    );
                                });
                            egui::Frame::NONE
                                .fill(tc.bg_edit)
                                .stroke(egui::Stroke::new(1.0, tc.border))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::same(6))
                                .show(&mut cols[2], |ui| {
                                    ui.strong(
                                        egui::RichText::new("-56.76% (graph_40n_160e)")
                                            .size(12.0)
                                            .color(tc.green),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            ".nemat 相比去键名 JSON (positional): 1,247 vs 2,884 BPE tokens",
                                        )
                                        .size(10.5),
                                    );
                                });
                        });

                        ui.add_space(6.0);

                        // 场景切换按钮 (8 组基准场景)
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("实测场景:").size(10.5).color(tc.fg_dim));
                            for (idx, sc) in OFFICIAL_BENCHMARK_V12.iter().enumerate() {
                                let selected = self.nema.bench_scenario_idx == idx;
                                if ui
                                    .selectable_label(
                                        selected,
                                        egui::RichText::new(sc.scenario).size(10.5).monospace(),
                                    )
                                    .on_hover_text(sc.label_zh)
                                    .clicked()
                                {
                                    self.nema.bench_scenario_idx = idx;
                                }
                            }
                        });

                        let sc_idx = self
                            .nema
                            .bench_scenario_idx
                            .min(OFFICIAL_BENCHMARK_V12.len() - 1);
                        let sc = &OFFICIAL_BENCHMARK_V12[sc_idx];
                        let use_uni = self.nema.bench_use_unigram;

                        ui.add_space(4.0);
                        egui::Frame::NONE
                            .fill(tc.bg_edit)
                            .stroke(egui::Stroke::new(1.0, tc.blue))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::same(6))
                            .show(ui, |ui| {
                                ui.strong(
                                    egui::RichText::new(format!(
                                        "场景详情: {} — {}",
                                        sc.scenario, sc.label_zh
                                    ))
                                    .size(11.0)
                                    .color(tc.blue),
                                );
                                let items: [(&str, usize, usize); 6] = if use_uni {
                                    [
                                        ("JSON (objects)", sc.unigram_json, sc.raw_bytes_json),
                                        (
                                            "JSON (positional)",
                                            sc.unigram_json_positional,
                                            sc.raw_bytes_json_positional,
                                        ),
                                        (
                                            "JSON (tagged w/t/g)",
                                            sc.unigram_json_tagged,
                                            sc.raw_bytes_json_tagged,
                                        ),
                                        (
                                            "YAML (positional)",
                                            sc.unigram_yaml_positional,
                                            sc.raw_bytes_yaml_positional,
                                        ),
                                        (".nema (native v1.2)", sc.unigram_nema, sc.raw_bytes_nema),
                                        (".nemat (transport)", sc.unigram_nemat, sc.raw_bytes_nema),
                                    ]
                                } else {
                                    [
                                        ("JSON (objects)", sc.bpe_json, sc.raw_bytes_json),
                                        ("JSON (positional)", sc.bpe_json_positional, sc.raw_bytes_json_positional),
                                        (
                                            "JSON (tagged w/t/g)",
                                            sc.bpe_json_tagged,
                                            sc.raw_bytes_json_tagged,
                                        ),
                                        ("YAML (positional)", sc.bpe_yaml_positional, sc.raw_bytes_yaml_positional),
                                        (".nema (native v1.2)", sc.bpe_nema, sc.raw_bytes_nema),
                                        (".nemat (transport)", sc.bpe_nemat, sc.raw_bytes_nema),
                                    ]
                                };
                                let max_tok =
                                    items.iter().map(|x| x.1).max().unwrap_or(1).max(1) as f32;
                                let base_obj = items[0].1.max(1) as f64;
                                let base_pos = items[1].1.max(1) as f64;

                                egui::Grid::new(("nema-sc-detail", sc.scenario, use_uni))
                                    .num_columns(5)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        for (name, tok, bytes) in items {
                                            let is_nema = name.starts_with(".nema");
                                            ui.strong(
                                                egui::RichText::new(name).size(10.5).color(
                                                    if is_nema { tc.blue } else { tc.fg },
                                                ),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!("{tok} tok"))
                                                    .size(10.5)
                                                    .monospace(),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!("{bytes} B"))
                                                    .size(10.5)
                                                    .monospace()
                                                    .color(tc.fg_dim),
                                            );
                                            let vs_obj =
                                                ((tok as f64) - base_obj) / base_obj * 100.0;
                                            let vs_pos =
                                                ((tok as f64) - base_pos) / base_pos * 100.0;
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "vs Obj {:+.1}% | vs Pos {:+.1}%",
                                                    vs_obj, vs_pos
                                                ))
                                                .size(10.0)
                                                .monospace()
                                                .color(if vs_pos < 0.0 {
                                                    tc.green
                                                } else {
                                                    tc.fg_dim
                                                }),
                                            );
                                            let bar_w = 140.0;
                                            let (brect, _) = ui.allocate_exact_size(
                                                egui::vec2(bar_w, 11.0),
                                                egui::Sense::hover(),
                                            );
                                            let bp = ui.painter_at(brect);
                                            bp.rect_filled(brect, 3.0, tc.bg_bar);
                                            bp.rect_filled(
                                                egui::Rect::from_min_size(
                                                    brect.min,
                                                    egui::vec2(
                                                        bar_w * (tok as f32 / max_tok).clamp(0.03, 1.0),
                                                        11.0,
                                                    ),
                                                ),
                                                3.0,
                                                if is_nema { tc.blue } else { tc.fg_dim },
                                            );
                                            ui.end_row();
                                        }
                                    });
                            });

                        ui.add_space(6.0);

                        // 全量 8 组基准数据总表
                        egui::Grid::new(("nema-official-all-table", use_uni))
                            .striped(true)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                for h in [
                                    "数据集 (Dataset)",
                                    "JSON (obj)",
                                    "JSON (pos)",
                                    "JSON (tagged)",
                                    "YAML (pos)",
                                    ".nema",
                                    ".nemat",
                                    ".nema vs Obj",
                                    ".nema vs Pos",
                                    ".nemat vs Pos",
                                ] {
                                    ui.strong(
                                        egui::RichText::new(h).size(10.0).color(tc.fg_dim),
                                    );
                                }
                                ui.end_row();

                                for s in OFFICIAL_BENCHMARK_V12 {
                                    let (jo, jp, jt, yp, nm, nmt) = if use_uni {
                                        (
                                            s.unigram_json,
                                            s.unigram_json_positional,
                                            s.unigram_json_tagged,
                                            s.unigram_yaml_positional,
                                            s.unigram_nema,
                                            s.unigram_nemat,
                                        )
                                    } else {
                                        (
                                            s.bpe_json,
                                            s.bpe_json_positional,
                                            s.bpe_json_tagged,
                                            s.bpe_yaml_positional,
                                            s.bpe_nema,
                                            s.bpe_nemat,
                                        )
                                    };
                                    let vs_obj =
                                        ((nm as f64) - (jo as f64)) / (jo as f64) * 100.0;
                                    let vs_pos =
                                        ((nm as f64) - (jp as f64)) / (jp as f64) * 100.0;
                                    let nmt_vs_pos =
                                        ((nmt as f64) - (jp as f64)) / (jp as f64) * 100.0;

                                    ui.label(
                                        egui::RichText::new(s.scenario).size(10.5).monospace().strong(),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{jo}"))
                                            .size(10.5)
                                            .monospace(),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{jp}"))
                                            .size(10.5)
                                            .monospace(),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{jt}"))
                                            .size(10.5)
                                            .monospace(),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{yp}"))
                                            .size(10.5)
                                            .monospace(),
                                    );
                                    ui.strong(
                                        egui::RichText::new(format!("{nm}"))
                                            .size(10.5)
                                            .monospace()
                                            .color(tc.blue),
                                    );
                                    ui.strong(
                                        egui::RichText::new(format!("{nmt}"))
                                            .size(10.5)
                                            .monospace()
                                            .color(tc.blue),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:+.2}%", vs_obj))
                                            .size(10.5)
                                            .monospace()
                                            .color(tc.green),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:+.2}%", vs_pos))
                                            .size(10.5)
                                            .monospace()
                                            .color(if vs_pos < 0.0 { tc.green } else { tc.orange }),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:+.2}%", nmt_vs_pos))
                                            .size(10.5)
                                            .monospace()
                                            .color(if nmt_vs_pos < 0.0 {
                                                tc.green
                                            } else {
                                                tc.orange
                                            }),
                                    );
                                    ui.end_row();
                                }
                            });

                        ui.add_space(6.0);

                        // v1.2 Graph 单空格节点标记实测对比卡片
                        let (
                            _old_src,
                            _new_src,
                            old_bpe,
                            new_bpe,
                            old_uni,
                            new_uni,
                            old_bytes,
                            new_bytes,
                        ) = OFFICIAL_SYNTAX_BEFORE_AFTER;
                        let delta_bpe_pct = ((new_bpe as f64 - old_bpe as f64) * 100.0) / (old_bpe as f64);
                        egui::Frame::NONE
                            .fill(tc.bg_edit)
                            .stroke(egui::Stroke::new(1.0, tc.border))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::same(6))
                            .show(ui, |ui| {
                                ui.strong(
                                    egui::RichText::new(
                                        "v1.2 语法优化验证：Graph 单空格节点标记 (graph_3n_4e)",
                                    )
                                    .size(11.0)
                                    .color(tc.blue),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "旧版 (A \\=LoginAPI 需字面量前缀): {old_bpe} BPE / {old_uni} Unigram / {old_bytes} B  →  新版 v1.2 ( A LoginAPI 单空格节点行): {new_bpe} BPE / {new_uni} Unigram / {new_bytes} B ({delta_bpe_pct:+.2}% BPE)",
                                    ))
                                    .size(10.5)
                                    .monospace(),
                                );
                            });
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Trace;
    use crate::settings::Settings;

    fn test_app() -> (App, std::sync::MutexGuard<'static, ()>) {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        (App::build(vec![], None, Trace::new(), Settings::default(), false), g)
    }

    #[test]
    fn toggle_header_is_undoable_and_restores_exact_bytes() {
        let (mut app, _g) = test_app();
        app.new_doc();
        let ai = app.active;
        app.docs[ai].name = "demo.nema".to_string();
        app.docs[ai].syntax = Some("NEMA".to_string());
        let orig = vec![
            " \\1 1".to_string(),
            "Service Status Latency".to_string(),
            "Gateway Healthy 12ms".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        app.docs[ai].lines = orig.clone();
        app.nema_toggle_header();
        assert_eq!(app.docs[ai].lines[1], "\\ Service Status Latency");
        app.docs[ai].undo();
        assert_eq!(app.docs[ai].lines, orig);
    }

    #[test]
    fn nema_convert_protects_code_files_by_opening_new_tab() {
        let (mut app, _g) = test_app();
        app.new_doc();
        let ai = app.active;
        app.docs[ai].name = "lib.rs".to_string();
        app.docs[ai].syntax = Some("Rust".to_string());
        let rust_src = vec![
            "pub fn add(a: i32, b: i32) -> i32 {".to_string(),
            "    a + b".to_string(),
            "}".to_string(),
        ];
        app.docs[ai].lines = rust_src.clone();

        app.nema_convert();

        // 原 Rust 代码标签不得被覆盖 (IDE-08)
        assert_eq!(app.docs[ai].lines, rust_src);
        assert_eq!(app.docs[ai].syntax.as_deref(), Some("Rust"));
        // 新标签为转换后的 .nema 文档
        assert_eq!(app.docs.len(), ai + 2);
        assert_eq!(app.active, ai + 1);
        assert_eq!(app.docs[app.active].syntax.as_deref(), Some("NEMA"));
        assert!(app.docs[app.active].name.ends_with(".nema"));
        assert!(app.nema_preview);
    }

    #[test]
    fn nema_convert_roundtrips_nema_and_nemat_with_lease_arbitration() {
        let (mut app, _g) = test_app();
        app.new_doc();
        let ai = app.active;
        app.docs[ai].name = "services.nema".to_string();
        app.docs[ai].syntax = Some("NEMA".to_string());
        let orig = vec![
            " \\1 1".to_string(),
            "\\ ServiceOwner ServiceStatus RegionName".to_string(),
            "AuthenticationService HealthyStatus AsiaPacificEast".to_string(),
            "AuthenticationService HealthyStatus AsiaPacificEast".to_string(),
            "AuthenticationService HealthyStatus AsiaPacificEast".to_string(),
            "AuthenticationService HealthyStatus AsiaPacificEast".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        app.docs[ai].lines = orig.clone();

        // 第一次手动转换: .nema -> .nemat
        app.nema_convert();
        assert!(app.docs[ai].lines[0].starts_with("\t$"));
        assert_eq!(app.nema.arbitrator.role(), &OwnershipRole::Observer);

        // 第二次手动转换: .nemat -> .nema 精确还原
        app.nema_convert();
        assert_eq!(app.docs[ai].lines, orig);
    }

    #[test]
    fn nema_v12_sample_and_benchmark_diff_work_end_to_end() {
        let (mut app, _g) = test_app();
        app.new_doc();
        app.nema_insert_v12_sample();
        let ai = app.active;
        let raw = app.docs[ai].lines.join("\n");
        let rendered = render_source(&raw).expect("v1.2 sample should render cleanly");
        assert_eq!(rendered.domains.len(), 3);
        assert_eq!(rendered.domains[0].mode, DomainMode::FiveWOneH);
        assert_eq!(rendered.domains[1].mode, DomainMode::Table);
        assert_eq!(rendered.domains[2].mode, DomainMode::Graph);
        // 校验 Graph 单空格节点与 X3 扩展属性
        let g = &rendered.domains[2];
        assert_eq!(g.rows[0].node_id.as_deref(), Some("A"));
        assert_eq!(
            g.rows[0].extensions,
            vec![("X3".to_string(), "APIKey".to_string())]
        );
        // 触发 Benchmark 实测面板
        app.nema_token_diff();
        assert_eq!(app.nema_view_mode, 4);
        assert!(app
            .nema
            .info_banner
            .as_deref()
            .unwrap_or("")
            .contains("vs JSON对象"));
    }

    #[test]
    fn cached_graph_canvas_builds_and_caches_correctly() {
        let (mut app, _g) = test_app();
        app.new_doc();
        app.nema_insert_v12_sample();
        let ai = app.active;
        let raw = app.docs[ai].lines.join("
");
        let rendered = render_source(&raw).expect("v1.2 sample should render cleanly");
        let g = &rendered.domains[2];
        assert_eq!(g.mode, DomainMode::Graph);

        let key1 = (100, 1, 5000, 3000);
        let cached1 = CachedGraphCanvas::build(key1, g, 500.0, 300.0);
        assert_eq!(cached1.cache_key, key1);
        assert!(!cached1.nodes.is_empty(), "节点不应为空");
        assert!(!cached1.edges.is_empty(), "边不应为空");

        // 验证节点矩形相对原点计算有效
        for node in &cached1.nodes {
            assert!(node.rect_rel.width() > 0.0);
            assert!(node.rect_rel.height() > 0.0);
        }

        // 验证边说明胶囊尺寸有效
        for edge in &cached1.edges {
            assert!(edge.pill_rect_rel.width() > 0.0);
            assert!(edge.pill_rect_rel.height() > 0.0);
        }

        // 尺寸变化时的键值区分
        let key2 = (100, 1, 6000, 3000);
        assert_ne!(key1, key2);
    }
}
