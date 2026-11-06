//! NEMA v1.5 工具条、共享 Codec 对接、租约仲裁与右侧交互式预览面板 (`Table / 5W1H / Graph`)

use super::*;
use crate::i18n::trz;
use nema_codec::{
    candidate_dictionary_details, find_reused_positions, normalize_nema_newlines, render_source,
    toggle_table_header_in_source, CodecFormat, DomainMode, EdgeDirection, NemaCodec,
    NemaLeaseArbitrator, OwnershipRole, RenderedDocument, RenderedDomain, StandardNemaCodec,
    TokenProfile,
};

/// NEMA 预览与状态机缓存
#[derive(Debug, Default, Clone)]
pub struct NemaUiState {
    pub arbitrator: NemaLeaseArbitrator,
    pub selected_edge: Option<(String, String, String, String, String, usize)>,
    pub info_banner: Option<String>,
    pub locked_words: Vec<String>,
    pub cached_render: Option<((u64, u64), Result<RenderedDocument, String>)>,
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
        let on = self.nema_preview;
        let mut toggle = false;
        let mut action: Option<&'static str> = None;

        let frame = egui::Frame::NONE
            .fill(tc.bg_bar)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .stroke(egui::Stroke::NONE);

        egui::Panel::top("nemabar")
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

                // 预览开关钉在最右
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                                    if ui
                                        .button(egui::RichText::new("Token").size(11.0))
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

        if toggle {
            self.nema_preview = !self.nema_preview;
        }
        match action {
            Some("convert") => self.nema_convert(),
            Some("toggle_header") => self.nema_toggle_header(),
            Some("add_row") => self.nema_add_row(),
            Some("add_cont") => self.nema_add_cont(),
            Some("insert_5w1h") => self.nema_insert_5w1h(),
            Some("insert_graph") => self.nema_insert_graph(),
            Some("insert_dict") => self.nema_insert_dict(),
            Some("insert_ref") => self.nema_insert_ref(),
            Some("candidates") => self.nema_candidates(),
            Some("lock_dict") => self.nema_lock_dict(),
            Some("show_reused") => self.nema_show_reused(),
            Some("token_diff") => self.nema_token_diff(),
            _ => {}
        }
    }

    /// 自动转化 / 编码为 NEMA 规范格式（遵循 Lease 仲裁协议与代码文件保护 `IDE-08`）
    pub fn nema_convert(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw_text = self.docs[i].lines.join("\n");
        let has_domain_header = raw_text.contains("\t\\")
            || raw_text.contains("\t?")
            || raw_text.contains("\t>");
        let is_nema = is_nema_doc(&self.docs[i]) || has_domain_header;

        // 1. 非 NEMA 文档转化为 .nema
        if !is_nema {
            let mut body_lines = vec!["\t\\1".to_string()];
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
        new_lines.insert(cur + 1, "   ContinuationGroup".to_string());
        self.apply_nema_lines(i, new_lines, Some([cur + 1, 3]));
        let tip = trz("已插入纵向意群续接行 (空格缩进)").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_5w1h(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let raw = self.docs[i].lines.join("\n");
        let trimmed = raw.trim_end_matches('\n');
        let next_id = trimmed.lines().filter(|l| l.starts_with('\t')).count() + 1;
        let combined = if trimmed.is_empty() {
            format!(
                "\t?{next_id}\nBuildProject MaintainQuality Rust CoreEngineer WhenReady InWorkspace .\n\n\n"
            )
        } else {
            format!(
                "{trimmed}\n\t?{next_id}\nBuildProject MaintainQuality Rust CoreEngineer WhenReady InWorkspace .\n\n\n"
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
        let next_id = trimmed.lines().filter(|l| l.starts_with('\t')).count() + 1;
        let combined = if trimmed.is_empty() {
            format!(
                "\t>{next_id}\nA ClientApp FrontEnd\nB Gateway ProxyService\nA>X>B Sends RequestPayload\n\n\n"
            )
        } else {
            format!(
                "{trimmed}\n\t>{next_id}\nA ClientApp FrontEnd\nB Gateway ProxyService\nA>X>B Sends RequestPayload\n\n\n"
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
        let codec = StandardNemaCodec;
        self.nema_preview = true;
        if let Ok(choice) = codec.encode(&raw, &TokenProfile::ByteHeuristic) {
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

    /// 右侧交互式 NEMA 可视化面板 (`Table / 5W1H / Graph`)
    pub(super) fn nema_ui(&mut self, ui: &mut egui::Ui) {
        let ai = self.active.min(self.docs.len() - 1);
        let key = (self.docs[ai].id, self.docs[ai].rev);
        if self.nema.cached_render.as_ref().map(|c| c.0) != Some(key) {
            let raw = normalize_nema_newlines(&self.docs[ai].lines.join("\n"));
            let res = if raw.starts_with("\t$") {
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
            ui.strong(egui::RichText::new("NEMA v1.5").color(tc.blue).size(12.0));
            let role_txt = match self.nema.arbitrator.role() {
                OwnershipRole::Observer => {
                    format!("Observer · Epoch #{}", self.nema.arbitrator.current_epoch())
                }
                _ => format!("Lease Active · Epoch #{}", self.nema.arbitrator.current_epoch()),
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
            ] {
                let selected = self.nema_view_mode == idx;
                if ui
                    .selectable_label(selected, egui::RichText::new(label).size(11.0))
                    .clicked()
                {
                    self.nema_view_mode = idx;
                }
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

        ui.separator();

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
                            "取消显式表头 (\\ )"
                        } else {
                            "首行设为表头 (\\ )"
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

    /// 使用 `egui::Painter` 绘制有向/无向多重图 (`A>Z>B` -> $A \to B$, `B<Z<C` -> $C \to B$)，Edge 拥有独立 ID 且可点击检查
    fn render_nema_graph_canvas(
        &mut self,
        ui: &mut egui::Ui,
        domain: &RenderedDomain,
        jump_to_line: &mut Option<usize>,
    ) {
        let tc = self.tc;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for row in &domain.rows {
            if let Some(nid) = &row.node_id {
                let rel = row.cells.get(1).cloned().unwrap_or_default();
                let desc = row.cells.get(2).cloned().unwrap_or_default();
                nodes.push((nid.clone(), rel, desc, row.line));
            } else if let Some(e) = &row.edge {
                let rel = row.cells.get(1).cloned().unwrap_or_default();
                let desc = row.cells.get(2).cloned().unwrap_or_default();
                edges.push((e.clone(), rel, desc, row.line));
            }
        }

        let canvas_w = ui.available_width().max(320.0);
        let canvas_h = 260.0;
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

        let n_count = nodes.len().max(1);
        let center = rect.center();
        let rx = (canvas_w * 0.34).max(70.0);
        let ry = (canvas_h * 0.32).max(55.0);
        let mut node_pos = std::collections::HashMap::new();
        let node_size = egui::vec2(104.0, 42.0);

        for (idx, (nid, _, _, _)) in nodes.iter().enumerate() {
            let pos = if n_count == 1 {
                center
            } else if n_count == 2 {
                egui::pos2(
                    rect.left() + canvas_w * (0.26 + 0.48 * (idx as f32)),
                    center.y,
                )
            } else {
                let angle = -std::f32::consts::FRAC_PI_2
                    + 2.0 * std::f32::consts::PI * (idx as f32) / (n_count as f32);
                egui::pos2(center.x + rx * angle.cos(), center.y + ry * angle.sin())
            };
            node_pos.insert(nid.clone(), pos);
        }

        // 先画边与箭头，并放置可点击的独立 Edge ID 胶囊
        for (e_idx, (edge, rel, desc, line)) in edges.iter().enumerate() {
            let Some(&p_from) = node_pos.get(&edge.from) else {
                continue;
            };
            let Some(&p_to) = node_pos.get(&edge.to) else {
                continue;
            };
            let is_selected = self
                .nema
                .selected_edge
                .as_ref()
                .is_some_and(|s| s.0 == edge.id);
            let stroke_col = if is_selected { tc.orange } else { tc.blue };

            let mid = if edge.from == edge.to {
                let loop_top = egui::pos2(p_from.x, p_from.y - 44.0);
                painter.circle_stroke(
                    egui::pos2(p_from.x, p_from.y - 26.0),
                    18.0,
                    egui::Stroke::new(1.5, stroke_col),
                );
                loop_top
            } else {
                let dir = (p_to - p_from).normalized();
                let start = p_from + dir * 48.0;
                let end = p_to - dir * 48.0;
                painter.line_segment([start, end], egui::Stroke::new(1.6, stroke_col));
                if edge.direction == EdgeDirection::Directed {
                    let normal = egui::vec2(-dir.y, dir.x);
                    let p1 = end - dir * 8.0 + normal * 4.5;
                    let p2 = end - dir * 8.0 - normal * 4.5;
                    painter.add(egui::Shape::convex_polygon(
                        vec![end, p1, p2],
                        stroke_col,
                        egui::Stroke::NONE,
                    ));
                }
                egui::pos2(
                    (start.x + end.x) * 0.5,
                    (start.y + end.y) * 0.5 + ((e_idx % 2) as f32 * 14.0 - 7.0),
                )
            };

            let label_txt = if rel.is_empty() {
                edge.id.clone()
            } else {
                format!("{}: {}", edge.id, rel)
            };
            let pill_w = (label_txt.chars().count() as f32 * 6.5 + 14.0).clamp(48.0, 160.0);
            let pill_rect = egui::Rect::from_center_size(mid, egui::vec2(pill_w, 18.0));
            let resp = ui.interact(
                pill_rect,
                ui.id().with(("nema-edge", &domain.id, &edge.id)),
                egui::Sense::click(),
            );
            painter.rect_filled(
                pill_rect,
                9.0,
                if is_selected || resp.hovered() {
                    tc.bg_active
                } else {
                    tc.bg_bar
                },
            );
            painter.rect_stroke(
                pill_rect,
                9.0,
                egui::Stroke::new(1.0, stroke_col),
                egui::StrokeKind::Inside,
            );
            painter.text(
                pill_rect.center(),
                egui::Align2::CENTER_CENTER,
                &label_txt,
                egui::FontId::proportional(10.0),
                tc.fg,
            );
            if resp.clicked() {
                self.nema.selected_edge = Some((
                    edge.id.clone(),
                    edge.from.clone(),
                    edge.to.clone(),
                    rel.clone(),
                    desc.clone(),
                    *line,
                ));
                *jump_to_line = Some(*line);
            }
        }

        // 再画节点卡片
        for (nid, rel, _desc, line) in &nodes {
            let Some(&pos) = node_pos.get(nid) else {
                continue;
            };
            let nrect = egui::Rect::from_center_size(pos, node_size);
            let resp = ui.interact(
                nrect,
                ui.id().with(("nema-node", &domain.id, nid)),
                egui::Sense::click(),
            );
            painter.rect_filled(
                nrect,
                6.0,
                if resp.hovered() {
                    tc.bg_hover
                } else {
                    tc.bg_bar
                },
            );
            painter.rect_stroke(
                nrect,
                6.0,
                egui::Stroke::new(1.2, tc.border),
                egui::StrokeKind::Inside,
            );
            painter.text(
                egui::pos2(nrect.center().x, nrect.top() + 13.0),
                egui::Align2::CENTER_CENTER,
                nid,
                egui::FontId::proportional(11.5),
                tc.fg,
            );
            painter.text(
                egui::pos2(nrect.center().x, nrect.top() + 28.0),
                egui::Align2::CENTER_CENTER,
                rel,
                egui::FontId::proportional(10.0),
                tc.fg_dim,
            );
            if resp.clicked() {
                *jump_to_line = Some(*line);
            }
        }

        // 显示选中的独立 Edge 详情卡片
        if let Some((eid, from, to, rel, desc, line)) = &self.nema.selected_edge {
            ui.add_space(4.0);
            egui::Frame::NONE
                .fill(tc.bg_edit)
                .stroke(egui::Stroke::new(1.0, tc.blue))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(
                            egui::RichText::new(format!("Edge ID: {eid} ({from} → {to})"))
                                .size(11.0)
                                .color(tc.blue),
                        );
                        if !rel.is_empty() {
                            ui.label(egui::RichText::new(format!("关系: {rel}")).size(11.0));
                        }
                        if !desc.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("说明: {desc}"))
                                    .size(11.0)
                                    .color(tc.fg_dim),
                            );
                        }
                        if ui.small_button(format!("跳至 L{line}")).clicked() {
                            *jump_to_line = Some(*line);
                        }
                    });
                });
        }
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
            "\t\\1".to_string(),
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
            "\t\\1".to_string(),
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
}
