//! Markdown 结构化搜索窗口 (Tauri 工具条的「结构」按钮, `editor.html` 的 `mdFind`): 一行输入 `类型 [条件]`,
//! 类型 heading / link / image / fence / task / todo, 条件是文字子串 (heading 可写 `#2` 限定层级, fence 写语言名);
//! 命中列表带行号, 点一条跳到那一行。搜索本身是 `editcore::md_struct::find` (和 Tauri 的 `md_find` 同一个函数), 这里只做输入解析和界面。

use super::*;
use editcore::md_struct::{self, Hit};

/// 窗口里的类型快捷钮 (点一下改写输入的第一个词)
pub(super) const KINDS: [&str; 6] = ["heading", "link", "image", "fence", "task", "todo"];

#[derive(Default)]
pub(super) struct MdQuery {
    pub open: bool,
    pub input: String,
    /// 命中结果对应的 (文档 id, 文档修订号, 输入): 没变就不重搜
    key: Option<(u64, u64, String)>,
    pub hits: Vec<Hit>,
    pub err: Option<String>,
    /// 打开时要把焦点给输入框
    focus: bool,
}

/// `类型 [条件]` -> (类型, 条件); 空输入 = 默认 heading (Tauri 提示框的默认值)
pub(super) fn parse(input: &str) -> (String, String) {
    let t = input.trim();
    if t.is_empty() { return ("heading".into(), String::new()); }
    let mut it = t.split_whitespace();
    let kind = it.next().unwrap_or("heading").to_string();
    (kind, it.collect::<Vec<_>>().join(" "))
}

/// 在文档行里搜 (行号为 1 起, 同 editcore)
pub(super) fn search(lines: &[String], input: &str) -> Result<Vec<Hit>, String> {
    let (kind, q) = parse(input);
    md_struct::find(&lines.join("\n"), &kind, &q)
}

/// 把输入的第一个词换成 `kind`, 条件保留
pub(super) fn with_kind(input: &str, kind: &str) -> String {
    let (_, q) = parse(input);
    if q.is_empty() { kind.to_string() } else { format!("{kind} {q}") }
}

impl App {
    pub(super) fn md_struct_open(&mut self) {
        let q = &mut self.sys.mdq;
        q.open = true;
        q.focus = true;
        if q.input.trim().is_empty() { q.input = "heading".into(); }
        q.key = None;
    }

    pub(super) fn md_struct_window(&mut self, ctx: &egui::Context) {
        let ai = self.active.min(self.docs.len() - 1);
        {
            let d = &self.docs[ai];
            let key = (d.id, d.rev, self.sys.mdq.input.clone());
            if self.sys.mdq.key.as_ref() != Some(&key) {
                match search(&d.lines, &key.2) {
                    Ok(h) => { self.sys.mdq.hits = h; self.sys.mdq.err = None; }
                    Err(e) => { self.sys.mdq.hits.clear(); self.sys.mdq.err = Some(e); }
                }
                self.sys.mdq.key = Some(key);
            }
        }
        let tc = self.tc;
        let mut open = true;
        let mut jump: Option<usize> = None;
        egui::Window::new(tr("md.struct-title")).open(&mut open).collapsible(false).default_width(460.0).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
            let q = &mut self.sys.mdq;
            ui.horizontal_wrapped(|ui| {
                let cur = parse(&q.input).0;
                for k in KINDS {
                    if ui.selectable_label(cur == k, k).clicked() { q.input = with_kind(&q.input, k); q.focus = true; }
                }
            });
            let r = ui.add(egui::TextEdit::singleline(&mut q.input).hint_text(tr("md.struct-hint")).desired_width(f32::INFINITY));
            if q.focus { r.request_focus(); q.focus = false; }
            ui.add_space(4.0);
            if let Some(e) = &q.err {
                ui.colored_label(egui::Color32::from_rgb(0xe0, 0x6c, 0x75), e);
            } else if q.hits.is_empty() {
                ui.label(egui::RichText::new(tr("md.struct-none")).color(tc.fg_dim));
            } else {
                ui.label(egui::RichText::new(trf("md.struct-count", &[&q.hits.len().to_string()])).color(tc.fg_dim));
            }
            egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
                for h in &q.hits {
                    let line = format!("{}  ·  {}  ·  {}", trf("md.line", &[&h.line.to_string()]), h.detail, h.text);
                    if ui.add(egui::Button::new(egui::RichText::new(line).size(12.0)).frame(false).wrap_mode(egui::TextWrapMode::Truncate).min_size(egui::vec2(ui.available_width(), 0.0))).clicked() {
                        jump = Some(h.line);
                    }
                }
            });
        });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) { self.sys.mdq.open = false; }
        if let Some(line) = jump {
            self.jump_mark();
            let d = self.cur();
            let row = line.saturating_sub(1).min(d.lines.len().saturating_sub(1));
            d.goto(row, 0);
            self.sys.mdq.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> { s.lines().map(String::from).collect() }

    #[test]
    fn input_is_kind_then_free_text_condition() {
        assert_eq!(parse(""), ("heading".into(), String::new()));
        assert_eq!(parse("  link  "), ("link".into(), String::new()));
        assert_eq!(parse("heading #2  用法  说明"), ("heading".into(), "#2 用法 说明".into()));
    }

    #[test]
    fn search_finds_headings_with_level_and_text_and_skips_fenced_code() {
        let doc = lines("# 总览\n正文\n## 用法\n```sh\n# not a heading\n```\n## 安装\n### 用法细节");
        let all = search(&doc, "heading").unwrap();
        assert_eq!(all.iter().map(|h| h.line).collect::<Vec<_>>(), vec![1, 3, 7, 8], "围栏里的 # 不算标题");
        let h2 = search(&doc, "heading #2").unwrap();
        assert_eq!(h2.iter().map(|h| (h.line, h.detail.as_str())).collect::<Vec<_>>(), vec![(3, "H2"), (7, "H2")]);
        let q = search(&doc, "heading #2 用法").unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!((q[0].line, q[0].text.as_str()), (3, "用法"));
    }

    #[test]
    fn search_other_kinds() {
        let doc = lines("- [ ] 待办\n- [x] 完成\n[官网](https://a.b) ![图](p.png)\n```rust\nfn x(){}\n```");
        assert_eq!(search(&doc, "task").unwrap().len(), 2);
        let todo = search(&doc, "todo").unwrap();
        assert_eq!(todo.len(), 1);
        assert_eq!(todo[0].line, 1);
        assert!(search(&doc, "link").unwrap().iter().any(|h| h.line == 3));
        assert_eq!(search(&doc, "image").unwrap().len(), 1);
        let f = search(&doc, "fence rust").unwrap();
        assert_eq!((f.len(), f[0].line), (1, 4));
    }

    #[test]
    fn unknown_kind_is_an_error_not_a_panic() {
        assert!(search(&lines("# a"), "nope").is_err());
    }

    #[test]
    fn kind_chip_keeps_the_condition() {
        assert_eq!(with_kind("heading #2 用法", "link"), "link #2 用法");
        assert_eq!(with_kind("", "task"), "task");
        assert_eq!(KINDS.len(), 6);
    }
}
