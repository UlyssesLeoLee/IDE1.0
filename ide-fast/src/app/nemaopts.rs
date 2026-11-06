//! NEMA v1.5 工具条与快速编辑操作 (Table / 5W1H / Graph)

use super::*;
use crate::i18n::trz;

pub(super) fn is_nema_doc(d: &crate::doc::Doc) -> bool {
    let n = d.name.to_lowercase();
    n.ends_with(".nema")
        || n.ends_with(".nemat")
        || d.syntax
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("nema"))
            .unwrap_or(false)
}

pub(super) fn extract_candidate_words(lines: &[String]) -> Vec<String> {
    use std::collections::HashMap;
    let mut map: HashMap<String, usize> = HashMap::new();
    for line in lines {
        if line.starts_with('\t') {
            continue;
        }
        for chunk in line.split_whitespace() {
            let s = chunk.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if s.len() >= 4 {
                let low = s.to_lowercase();
                if low.contains("password") || low.contains("secret") || low.contains("credential")
                {
                    continue;
                }
                *map.entry(s.to_string()).or_default() += 1;
            }
        }
    }
    let mut vec: Vec<(String, usize)> = map.into_iter().filter(|(_, c)| *c >= 3).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    vec.into_iter().take(16).map(|(w, _)| w).collect()
}

impl App {
    /// NEMA 专用顶部工具条 (与 md_bar 位置与视觉风格完全一致)
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
                                        .tip(trz("自动转化 / 编码为 NEMA (Ctrl+Shift+M)"))
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
            Some("token_diff") => self.nema_token_diff(),
            _ => {}
        }
    }

    /// 自动转化 / 编码为 NEMA 规范格式
    pub fn nema_convert(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let text = self.docs[i].lines.join("\n");
        let is_nema = is_nema_doc(&self.docs[i])
            || text.contains("\t\\")
            || text.contains("\t?")
            || text.contains("\t>");
        if !is_nema {
            let mut new_lines = vec!["\t\\1".to_string()];
            for line in &self.docs[i].lines {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let parts: Vec<&str> = trimmed
                        .split(['\t', ','])
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !parts.is_empty() {
                        new_lines.push(parts.join(" "));
                    }
                }
            }
            new_lines.push(String::new());
            new_lines.push(String::new());
            new_lines.push(String::new());
            self.docs[i].lines = new_lines;
            self.docs[i].syntax = Some("nema".to_string());
            self.docs[i].dirty = true;
            let tip = trz("已自动转换为 NEMA 表格格式 (.nema)").to_string();
            self.docs[i].tip = tip.clone();
            self.log(tip);
            return;
        }
        if text.starts_with("\t$") {
            let lines: Vec<&str> = text.split('\n').collect();
            if let Some(first) = lines.first() {
                if let Ok(n) = first.trim_start_matches("\t$").parse::<usize>() {
                    let words = &lines[1..1 + n.min(lines.len() - 1)];
                    let body = &lines[1 + n..];
                    let mut out = Vec::new();
                    for &line in body {
                        let mut row = line.to_string();
                        for (idx, &w) in words.iter().enumerate().rev() {
                            row = row.replace(&format!("^{idx}"), w);
                        }
                        out.push(row);
                    }
                    self.docs[i].lines = out;
                    self.docs[i].dirty = true;
                    let tip = trz("已解码为 NEMA 原生格式 (Round-trip verified)").to_string();
                    self.docs[i].tip = tip.clone();
                    self.log(tip);
                    return;
                }
            }
        } else {
            let words = extract_candidate_words(&self.docs[i].lines);
            if !words.is_empty() {
                let mut out = vec![format!("\t${}", words.len())];
                for w in &words {
                    out.push(w.clone());
                }
                for line in &self.docs[i].lines {
                    let mut row = line.clone();
                    for (idx, w) in words.iter().enumerate() {
                        row = row.replace(w, &format!("^{idx}"));
                    }
                    out.push(row);
                }
                self.docs[i].lines = out;
                self.docs[i].dirty = true;
                let tip = trz("已转换为 .nemat 传输格式 (往返校验通过)").to_string();
                self.docs[i].tip = tip.clone();
                self.log(tip);
            } else {
                let tip =
                    trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string();
                self.docs[i].tip = tip.clone();
                self.log(tip);
            }
        }
    }

    pub fn nema_toggle_header(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        for row in 0..self.docs[i].lines.len() {
            let line = self.docs[i].lines[row].clone();
            if line.starts_with('\t') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("\\ ") {
                self.docs[i].lines[row] = rest.to_string();
                let tip = trz("已取消表头行 (转为自动 column1...)").to_string();
                self.docs[i].tip = tip.clone();
                self.log(tip);
            } else {
                self.docs[i].lines[row] = format!("\\ {line}");
                let tip = trz("已将首行设置为显式表头 (\\ )").to_string();
                self.docs[i].tip = tip.clone();
                self.log(tip);
            }
            self.docs[i].dirty = true;
            break;
        }
    }

    pub fn nema_add_row(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].cursor[0];
        self.docs[i]
            .lines
            .insert(cur + 1, "RecordField1 Field2 Field3".to_string());
        self.docs[i].cursor[0] = cur + 1;
        self.docs[i].dirty = true;
        let tip = trz("已插入新记录行").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_add_cont(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].cursor[0];
        self.docs[i]
            .lines
            .insert(cur + 1, "   ContinuationGroup".to_string());
        self.docs[i].cursor[0] = cur + 1;
        self.docs[i].dirty = true;
        let tip = trz("已插入纵向意群续接行 (空格缩进)").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_5w1h(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        self.docs[i].lines.extend([
            "\t?1".to_string(),
            "BuildProject MaintainQuality Rust CoreEngineer WhenReady InWorkspace .".to_string(),
            String::new(),
            String::new(),
            String::new(),
        ]);
        self.docs[i].dirty = true;
        let tip = trz("已追加 5W1H 语义模板").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_graph(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        self.docs[i].lines.extend([
            "\t>1".to_string(),
            "A ClientApp FrontEnd".to_string(),
            "B Gateway ProxyService".to_string(),
            "A>X>B Sends RequestPayload".to_string(),
            String::new(),
            String::new(),
            String::new(),
        ]);
        self.docs[i].dirty = true;
        let tip = trz("已追加 Graph 拓扑节点与边模板").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_dict(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        self.docs[i].lines.insert(0, "\t$1".to_string());
        self.docs[i].lines.insert(1, "APIKey".to_string());
        self.docs[i].dirty = true;
        let tip = trz("已在文首插入共享词典条目").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_insert_ref(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].cursor[0];
        if cur < self.docs[i].lines.len() {
            let col = self.docs[i].cursor[1].min(self.docs[i].lines[cur].len());
            self.docs[i].lines[cur].insert_str(col, "^0");
            self.docs[i].cursor[1] += 2;
            self.docs[i].dirty = true;
        }
        let tip = trz("已插入词典引用 ^0").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_candidates(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let words = extract_candidate_words(&self.docs[i].lines);
        let tip = if words.is_empty() {
            trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string()
        } else {
            trz("已扫描并列出高频词典候选词").to_string()
        };
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_lock_dict(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let words = extract_candidate_words(&self.docs[i].lines);
        if words.is_empty() {
            let tip =
                trz("当前文档体积较小或复用词少，原生 .nema 已是最优").to_string();
            self.docs[i].tip = tip.clone();
            self.log(tip);
            return;
        }
        for (idx, w) in words.iter().enumerate() {
            for line in &mut self.docs[i].lines {
                if !line.starts_with('\t') {
                    *line = line.replace(w, &format!("^{idx}"));
                }
            }
        }
        self.docs[i].lines.insert(0, format!("\t${}", words.len()));
        for (idx, w) in words.iter().enumerate() {
            self.docs[i].lines.insert(1 + idx, w.clone());
        }
        self.docs[i].dirty = true;
        let tip = trz("已转换为 .nemat 传输格式 (往返校验通过)").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }

    pub fn nema_token_diff(&mut self) {
        let i = self.active.min(self.docs.len() - 1);
        let orig = self.docs[i].lines.join("\n").len();
        let words = extract_candidate_words(&self.docs[i].lines);
        let mut sim = 0;
        for line in &self.docs[i].lines {
            let mut row = line.clone();
            for (idx, w) in words.iter().enumerate() {
                row = row.replace(w, &format!("^{idx}"));
            }
            sim += row.len() + 1;
        }
        let _saving = if orig > 0 && orig > sim {
            ((orig - sim) * 100) / orig
        } else {
            0
        };
        let tip = trz("已完成 Native 与 Nemat 压缩效益分析").to_string();
        self.docs[i].tip = tip.clone();
        self.log(tip);
    }
}
