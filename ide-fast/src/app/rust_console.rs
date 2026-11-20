//! Blender `bpy` 风格的 Rust 自动化控制台 (`ide::ops`, `ide::data`, `ide::context`)。
//!
//! 支持在底层选项卡中输入 Rust 表达式与多行指令操纵当前编辑器状态：
//! - `ide::context::*`：当前上下文（活跃文档、选区、主题、工作区统计、协同状态）
//! - `ide::data::*`：数据对象模型（所有打开文档、文档正文、历史版本）
//! - `ide::ops::*`：操作动作执行器（打开文件、按行修改、全文替换、双文档比对、切换标签、多 Agent 规模扩展）

use super::*;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustConsoleEntry {
    pub command: String,
    pub output: String,
    pub ok: bool,
}

pub struct RustConsoleState {
    pub input: String,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub log: Vec<RustConsoleEntry>,
}

impl Default for RustConsoleState {
    fn default() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
            history_idx: 0,
            log: vec![RustConsoleEntry {
                command: "// Blender bpy-style Rust Console initialized".into(),
                output: "Type ide::context::help() or click the snippet buttons above to execute commands.".into(),
                ok: true,
            }],
        }
    }
}

impl App {
    /// 执行单条或分号/换行分隔的多条 Blender-Style Rust 控制台语句
    pub fn eval_rust_console_str(&mut self, script: &str) -> String {
        let mut results = Vec::new();
        for raw_line in script.lines() {
            for stmt in Self::split_statements(raw_line) {
                let s = stmt.trim();
                if s.is_empty() || s.starts_with("//") {
                    continue;
                }
                let out = self.eval_single_rust_stmt(s);
                results.push(out);
            }
        }
        if results.is_empty() {
            "Empty script (no statements executed)".into()
        } else {
            results.join("\n")
        }
    }

    fn split_statements(line: &str) -> Vec<String> {
        let mut stmts = Vec::new();
        let mut cur = String::new();
        let mut in_quote = false;
        let mut quote_char = ' ';
        let mut escaped = false;

        for ch in line.chars() {
            if escaped {
                cur.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                cur.push(ch);
                escaped = true;
                continue;
            }
            if in_quote {
                if ch == quote_char {
                    in_quote = false;
                }
                cur.push(ch);
            } else if ch == '"' || ch == '\'' {
                in_quote = true;
                quote_char = ch;
                cur.push(ch);
            } else if ch == ';' {
                stmts.push(std::mem::take(&mut cur));
            } else {
                cur.push(ch);
            }
        }
        if !cur.trim().is_empty() {
            stmts.push(cur);
        }
        stmts
    }

    fn parse_two_str_args(inner: &str) -> (String, String) {
        let mut first = String::new();
        let mut second = String::new();
        let mut in_first = true;
        let mut in_quote = false;
        let mut quote_char = ' ';
        let mut escaped = false;

        for ch in inner.chars() {
            if escaped {
                if in_first { first.push(ch); } else { second.push(ch); }
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if in_quote {
                if ch == quote_char {
                    in_quote = false;
                } else if in_first {
                    first.push(ch);
                } else {
                    second.push(ch);
                }
            } else if ch == '"' || ch == '\'' {
                in_quote = true;
                quote_char = ch;
            } else if ch == ',' && in_first {
                in_first = false;
            } else if !in_first {
                second.push(ch);
            }
        }
        (first.trim().to_string(), second.trim().to_string())
    }

    fn eval_single_rust_stmt(&mut self, stmt: &str) -> String {
        let clean = stmt.trim().trim_end_matches(';');

        if clean.eq_ignore_ascii_case("ide::context::help()")
            || clean.eq_ignore_ascii_case("ide::ops::help()")
            || clean.eq_ignore_ascii_case("help()")
        {
            return "=== IDE1.0 Blender-Style Rust API ===\n\
                    ide::context::status()                   - 查看工作区当前状态与活跃文档\n\
                    ide::context::help()                     - 打印全部控制台命令手册\n\
                    ide::data::docs()                        - 列出所有已打开文档及行数\n\
                    ide::data::read()                        - 读取当前文档全部内容 (或指定索引 read(idx))\n\
                    ide::ops::open(\"path\")                   - 打开指定路径文件\n\
                    ide::ops::new_doc(\"title\", \"content\")    - 创建并切换到新文档\n\
                    ide::ops::edit(start, end, \"lines\")      - 替换当前文档指定行 (1-based)\n\
                    ide::ops::replace_all(\"from\", \"to\")      - 全文替换当前文档文本\n\
                    ide::ops::diff_docs(idx_a, idx_b)        - 比对两份已打开文档的差异 (Myers Diff)\n\
                    ide::ops::switch_tab(idx)                - 切换到指定标签索引\n\
                    ide::ops::close_tab(idx)                 - 关闭指定文档标签\n\
                    ide::ops::collab_count(n)                - 设定协同 Agent 规模 (例如 10000)\n\
                    ide::ops::set_theme(\"dark\"|\"light\")      - 动态切换编辑器色彩主题\n\
                    ide::ops::clear()                        - 清空控制台历史日志"
                .into();
        }

        if clean.eq_ignore_ascii_case("ide::context::status()") {
            let active_title = self.docs.get(self.active).map(|d| d.name.as_str()).unwrap_or("(none)");
            let total_docs = self.docs.len();
            let dirty_count = self.docs.iter().filter(|d| d.dirty).count();
            let cur_lines = self.docs.get(self.active).map(|d| d.lines.len()).unwrap_or(0);
            return format!(
                "ide::context: active_idx={}, active_doc=\"{}\", lines={}, open_docs={}, dirty_docs={}, collab_agents={}, theme={}",
                self.active,
                active_title,
                cur_lines,
                total_docs,
                dirty_count,
                self.collab.agent_count,
                if self.dark { "Dark" } else { "Light" }
            );
        }

        if clean.eq_ignore_ascii_case("ide::data::docs()") || clean.eq_ignore_ascii_case("ide::data::files()") {
            if self.docs.is_empty() {
                return "ide::data: no open documents".into();
            }
            let mut list = Vec::new();
            for (i, d) in self.docs.iter().enumerate() {
                let path_str = d.path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "<untitled>".into());
                let marker = if i == self.active { "*" } else { " " };
                list.push(format!(
                    "[{}{}] id={}, lines={}, dirty={}, path={}",
                    marker, i, d.id, d.lines.len(), d.dirty, path_str
                ));
            }
            return format!("ide::data::docs ({}):\n{}", self.docs.len(), list.join("\n"));
        }

        if clean.starts_with("ide::data::read(") && clean.ends_with(')') {
            let inner = &clean["ide::data::read(".len()..clean.len() - 1].trim();
            let idx = if inner.is_empty() {
                self.active
            } else {
                match inner.parse::<usize>() {
                    Ok(i) => i,
                    Err(_) => return format!("ide::data::read: invalid doc index: {}", inner),
                }
            };
            return match self.docs.get(idx) {
                Some(d) => format!("--- [{}] {} ({} lines) ---\n{}", idx, d.name, d.lines.len(), d.text_lf()),
                None => format!("ide::data::read: index {} out of bounds (open docs: {})", idx, self.docs.len()),
            };
        }

        if clean.starts_with("ide::ops::open(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::open(".len()..clean.len() - 1].trim();
            let path_str = inner.trim_matches('"').trim_matches('\'');
            if path_str.is_empty() {
                return "ide::ops::open: path cannot be empty".into();
            }
            self.open_path(Path::new(path_str));
            return format!("ide::ops::open: opened \"{}\"", path_str);
        }

        if clean.starts_with("ide::ops::new_doc(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::new_doc(".len()..clean.len() - 1];
            let (title, content) = Self::parse_two_str_args(inner);
            let final_title = if title.is_empty() { "Untitled".to_string() } else { title };

            self.new_doc();
            let cur = self.cur();
            cur.name = final_title.clone();
            if !content.is_empty() {
                cur.lines = content.lines().map(String::from).collect();
                if cur.lines.is_empty() {
                    cur.lines.push(String::new());
                }
            }
            return format!("ide::ops::new_doc: created new doc \"{}\" (active_idx={})", final_title, self.active);
        }

        if clean.starts_with("ide::ops::edit(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::edit(".len()..clean.len() - 1];
            let parts: Vec<&str> = inner.splitn(3, ',').collect();
            if parts.len() < 3 {
                return "ide::ops::edit: requires (start_line, end_line, \"replacement_text\")".into();
            }
            let start = match parts[0].trim().parse::<usize>() {
                Ok(n) => n,
                Err(_) => return format!("ide::ops::edit: invalid start line: {}", parts[0]),
            };
            let end = match parts[1].trim().parse::<usize>() {
                Ok(n) => n,
                Err(_) => return format!("ide::ops::edit: invalid end line: {}", parts[1]),
            };
            let repl = parts[2].trim().trim_matches('"').trim_matches('\'');
            let repl_lines: Vec<String> = repl.lines().map(String::from).collect();

            if self.docs.is_empty() {
                return "ide::ops::edit: no open document".into();
            }
            let cur = self.cur();
            let len = cur.lines.len();
            let s_idx = start.saturating_sub(1).min(len);
            let e_idx = end.min(len);

            cur.lines.splice(s_idx..e_idx, repl_lines);
            if cur.lines.is_empty() {
                cur.lines.push(String::new());
            }
            cur.dirty = true;
            return format!(
                "ide::ops::edit: replaced lines {}..={} in active doc, total lines now {}",
                start, end, cur.lines.len()
            );
        }

        if clean.starts_with("ide::ops::replace_all(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::replace_all(".len()..clean.len() - 1];
            let (from, to) = Self::parse_two_str_args(inner);
            if from.is_empty() {
                return "ide::ops::replace_all: search string cannot be empty".into();
            }

            if self.docs.is_empty() {
                return "ide::ops::replace_all: no open document".into();
            }
            let cur = self.cur();
            let mut count = 0;
            for line in &mut cur.lines {
                if line.contains(&from) {
                    *line = line.replace(&from, &to);
                    count += 1;
                }
            }
            if count > 0 {
                cur.dirty = true;
            }
            return format!("ide::ops::replace_all: replaced {} occurrences of \"{}\" with \"{}\"", count, from, to);
        }

        if clean.starts_with("ide::ops::diff_docs(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::diff_docs(".len()..clean.len() - 1];
            let parts: Vec<&str> = inner.splitn(2, ',').collect();
            if parts.len() < 2 {
                return "ide::ops::diff_docs: requires (idx_a, idx_b)".into();
            }
            let a = match parts[0].trim().parse::<usize>() {
                Ok(n) => n,
                Err(_) => return format!("ide::ops::diff_docs: invalid idx_a: {}", parts[0]),
            };
            let b = match parts[1].trim().parse::<usize>() {
                Ok(n) => n,
                Err(_) => return format!("ide::ops::diff_docs: invalid idx_b: {}", parts[1]),
            };
            if a >= self.docs.len() || b >= self.docs.len() {
                return format!("ide::ops::diff_docs: index out of range (open docs: {})", self.docs.len());
            }

            let name_a = self.docs[a].name.clone();
            let name_b = self.docs[b].name.clone();
            let hunks = crate::diff::diff_lines(&self.docs[a].lines, &self.docs[b].lines);

            if hunks.is_empty() {
                return format!("ide::ops::diff_docs: [{}] \"{}\" and [{}] \"{}\" are identical", a, name_a, b, name_b);
            }

            let mut out = format!("--- [{}] {}\n+++ [{}] {}\n", a, name_a, b, name_b);
            for h in &hunks {
                out.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    h.a_start + 1,
                    h.a_len,
                    h.b_start + 1,
                    h.b_len
                ));
                for line in &self.docs[a].lines[h.a_start..h.a_start + h.a_len] {
                    out.push_str(&format!("-{}\n", line));
                }
                for line in &self.docs[b].lines[h.b_start..h.b_start + h.b_len] {
                    out.push_str(&format!("+{}\n", line));
                }
            }
            return out;
        }

        if clean.starts_with("ide::ops::switch_tab(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::switch_tab(".len()..clean.len() - 1].trim();
            let idx = match inner.parse::<usize>() {
                Ok(n) => n,
                Err(_) => return format!("ide::ops::switch_tab: invalid index: {}", inner),
            };
            if idx < self.docs.len() {
                self.active = idx;
                return format!("ide::ops::switch_tab: switched to tab [{}] \"{}\"", idx, self.docs[idx].name);
            } else {
                return format!("ide::ops::switch_tab: index {} out of range (open docs: {})", idx, self.docs.len());
            }
        }

        if clean.starts_with("ide::ops::close_tab(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::close_tab(".len()..clean.len() - 1].trim();
            let idx = match inner.parse::<usize>() {
                Ok(n) => n,
                Err(_) => return format!("ide::ops::close_tab: invalid index: {}", inner),
            };
            if idx < self.docs.len() {
                let name = self.docs[idx].name.clone();
                self.docs.remove(idx);
                if self.active >= self.docs.len() && !self.docs.is_empty() {
                    self.active = self.docs.len() - 1;
                }
                return format!("ide::ops::close_tab: closed tab [{}] \"{}\"", idx, name);
            } else {
                return format!("ide::ops::close_tab: index {} out of range (open docs: {})", idx, self.docs.len());
            }
        }

        if clean.starts_with("ide::ops::collab_count(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::collab_count(".len()..clean.len() - 1].trim();
            let n = match inner.parse::<usize>() {
                Ok(v) => v,
                Err(_) => return format!("ide::ops::collab_count: invalid count: {}", inner),
            };
            self.collab.agent_count = n;
            self.collab.open = true;
            return format!("ide::ops::collab_count: updated active collab agents to {} (collab panel opened)", n);
        }

        if clean.starts_with("ide::ops::set_theme(") && clean.ends_with(')') {
            let inner = &clean["ide::ops::set_theme(".len()..clean.len() - 1].trim();
            let theme = inner.trim_matches('"').trim_matches('\'').to_lowercase();
            return match theme.as_str() {
                "dark" => {
                    self.dark = true;
                    "ide::ops::set_theme: switched to Dark theme".into()
                }
                "light" => {
                    self.dark = false;
                    "ide::ops::set_theme: switched to Light theme".into()
                }
                other => format!("ide::ops::set_theme: unknown theme: {} (expected \"dark\" or \"light\")", other),
            };
        }

        if clean.eq_ignore_ascii_case("ide::ops::clear()") || clean.eq_ignore_ascii_case("clear()") {
            self.sys.rust_con.log.clear();
            return "ide::ops::clear: console cleared".into();
        }

        format!("Unrecognized Rust statement: \"{}\". Type ide::context::help() for reference.", stmt)
    }

    /// 底部面板: Rust 控制台 UI
    pub(super) fn rust_console_ui(&mut self, ui: &mut egui::Ui) {
        let mut to_run: Option<String> = None;

        Self::bottom_toolbar(self.tc.fg_dim, ui, |ui| {
            if ui.button("status()").clicked() {
                to_run = Some("ide::context::status()".into());
            }
            if ui.button("docs()").clicked() {
                to_run = Some("ide::data::docs()".into());
            }
            if ui.button("help()").clicked() {
                to_run = Some("ide::context::help()".into());
            }
            if ui.button("diff_docs(0, 1)").clicked() {
                to_run = Some("ide::ops::diff_docs(0, 1)".into());
            }
            if ui.button("collab(10000)").clicked() {
                to_run = Some("ide::ops::collab_count(10000)".into());
            }
            if ui.button(tr("bt.clear")).clicked() {
                self.sys.rust_con.log.clear();
            }
            if ui.button("×").clicked() {
                self.term.open = false;
            }
        });

        // 如果工具栏快捷按钮被点击，直接执行
        if let Some(cmd) = to_run {
            let output = self.eval_rust_console_str(&cmd);
            self.sys.rust_con.log.push(RustConsoleEntry {
                command: cmd,
                output,
                ok: true,
            });
        }

        let avail_h = ui.available_height();
        let input_bar_h = 32.0;
        let log_h = (avail_h - input_bar_h).max(40.0);

        // 控制台日志滚动输出区
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height(log_h)
            .show(ui, |ui| {
                if self.sys.rust_con.log.is_empty() {
                    ui.weak("Rust Console is ready. Type ide::context::help() or any ide::* statement.");
                }
                for entry in &self.sys.rust_con.log {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("rust>")
                                .monospace()
                                .color(if self.tc.comic {
                                    Color32::from_rgb(255, 204, 0)
                                } else {
                                    Color32::from_rgb(100, 180, 255)
                                }),
                        );
                        ui.add(egui::Label::new(
                            egui::RichText::new(&entry.command)
                                .monospace()
                                .strong(),
                        ).selectable(true));
                    });

                    let out_col = if entry.ok {
                        if self.tc.dark {
                            Color32::from_rgb(180, 230, 180)
                        } else {
                            Color32::from_rgb(30, 120, 30)
                        }
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };

                    ui.add(egui::Label::new(
                        egui::RichText::new(&entry.output)
                            .monospace()
                            .color(out_col),
                    ).selectable(true));
                    ui.add_space(2.0);
                }
            });

        ui.separator();

        // 底部单行输入交互行
        let mut submit = false;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("rust>")
                    .monospace()
                    .strong()
                    .color(if self.tc.comic {
                        Color32::from_rgb(255, 204, 0)
                    } else {
                        Color32::from_rgb(100, 180, 255)
                    }),
            );

            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.sys.rust_con.input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(ui.available_width() - 80.0)
                    .hint_text("Enter ide::ops::*, ide::data::*, or ide::context::* expression (e.g. ide::context::status())"),
            );

            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }

            if ui.button("Run / 执行").clicked() {
                submit = true;
            }
        });

        if submit {
            let cmd = self.sys.rust_con.input.trim().to_string();
            if !cmd.is_empty() {
                self.sys.rust_con.history.push(cmd.clone());
                self.sys.rust_con.history_idx = self.sys.rust_con.history.len();
                let output = self.eval_rust_console_str(&cmd);
                let ok = !output.starts_with("Unrecognized") && !output.contains("out of bounds") && !output.contains("invalid");
                self.sys.rust_con.log.push(RustConsoleEntry {
                    command: cmd,
                    output,
                    ok,
                });
                self.sys.rust_con.input.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::build(vec![], None, Trace::new(), Settings::default(), false)
    }

    #[test]
    fn test_rust_console_help() {
        let mut app = test_app();
        let res = app.eval_rust_console_str("ide::context::help()");
        assert!(res.contains("ide::context::status()"));
        assert!(res.contains("ide::ops::diff_docs"));
    }

    #[test]
    fn test_rust_console_status_and_docs() {
        let mut app = test_app();
        let st = app.eval_rust_console_str("ide::context::status()");
        assert!(st.contains("ide::context:"));
        assert!(st.contains("open_docs=1"));

        let docs = app.eval_rust_console_str("ide::data::docs()");
        assert!(docs.contains("ide::data::docs"));
    }

    #[test]
    fn test_rust_console_ops_lifecycle() {
        let mut app = test_app();
        // create new doc
        let res = app.eval_rust_console_str("ide::ops::new_doc(\"sample.rs\", \"fn main() { println!(\\\"hi\\\"); }\")");
        assert!(res.contains("created new doc"));

        // read data
        let content = app.eval_rust_console_str("ide::data::read()");
        assert!(content.contains("println!(\"hi\")"));

        // replace
        let repl = app.eval_rust_console_str("ide::ops::replace_all(\"hi\", \"world\")");
        assert!(repl.contains("replaced 1 occurrences"));
        let content_after = app.eval_rust_console_str("ide::data::read()");
        assert!(content_after.contains("println!(\"world\")"));

        // collab scale
        let collab = app.eval_rust_console_str("ide::ops::collab_count(10000)");
        assert!(collab.contains("10000"));
        assert_eq!(app.collab.agent_count, 10000);

        // diff docs
        app.new_doc();
        let cur = app.cur();
        cur.lines = vec!["line 1".into(), "line 2 modified".into()];
        let diff_out = app.eval_rust_console_str("ide::ops::diff_docs(0, 1)");
        assert!(diff_out.contains("@@"));
    }
}
