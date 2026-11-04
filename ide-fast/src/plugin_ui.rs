//! 脚本插件命令的 UI 胶水: 组文档参数 (按权限) -> 交给 `plugin_host` 异步跑 -> 每帧轮询 -> 按结果的四种形状分发
//! (错误 / 行补丁 / 摘要 / SVG·HTML·数据)。宿主第一次真的有命令要跑时才创建, 启动路径上没有任何开销。

use crate::doc::Doc;
use crate::plugin_host::{self, Outcome, PluginHost};
use crate::svg_lite::{self, Svg};
use eframe::egui;
use serde_json::{json, Value};
use std::sync::mpsc::{Receiver, TryRecvError};

struct Job {
    rx: Receiver<Result<String, String>>,
    perms: Vec<String>,
    title: String,
    /// 命令作用的文档 (id; 期间被关掉就丢弃结果)
    doc_id: u64,
    /// 启动时文档的修订号: 期间被改过, 行补丁基于旧文本, 不再套用
    rev: u64,
}

/// 结果窗口的内容
pub struct ResultWin {
    pub title: String,
    pub summary: String,
    pub svg: Option<Svg>,
    /// 画不出来 / 不是图时显示的文本 (HTML 剥标签、原始 JSON、SVG 源码)
    pub text: String,
    pub doc_id: u64,
}

#[derive(Default)]
pub struct PlugState {
    host: Option<PluginHost>,
    jobs: Vec<Job>,
    pub win: Option<ResultWin>,
}

/// 选区文字 (多行用 \n 连); 没有选区 = 空串。列按字符数
pub fn selection_text(d: &Doc) -> String {
    let Some([r0, c0, r1, c1]) = d.sel else {
        return String::new();
    };
    let cut = |l: &str, a: usize, b: usize| {
        l.chars()
            .skip(a)
            .take(b.saturating_sub(a))
            .collect::<String>()
    };
    if r0 == r1 {
        return cut(&d.lines[r0], c0, c1);
    }
    let mut parts = vec![cut(&d.lines[r0], c0, usize::MAX)];
    parts.extend(d.lines[r0 + 1..r1].iter().cloned());
    parts.push(cut(&d.lines[r1], 0, c1));
    parts.join("\n")
}

/// 文档参数: 没声明 editor.read 就什么都不给 (和 Tauri 版 plugDocArgs 一致)
pub fn doc_args(d: &Doc, perms: &[String]) -> Value {
    if !perms.iter().any(|p| p == "editor.read") {
        return json!({});
    }
    let sel = d
        .sel
        .map(|[r0, c0, r1, c1]| json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 }));
    json!({ "text": d.lines.join("\n"), "name": d.name, "cursor": { "row": d.cursor[0], "col": d.cursor[1] }, "selection": sel })
}

impl PlugState {
    /// 宿主是否已经被创建过 (测试 / 启动耗时检查用)
    #[allow(dead_code)]
    pub fn started(&self) -> bool {
        self.host.is_some()
    }

    /// 开始跑一个插件命令。`extra` 并进文档参数 (例如 PPA 的 script)
    pub fn start(
        &mut self,
        plugin: &str,
        perms: &[String],
        cmd: &str,
        title: &str,
        doc: &Doc,
        extra: Option<Value>,
    ) {
        let mut arg = doc_args(doc, perms);
        if let (Some(a), Some(Value::Object(e))) = (arg.as_object_mut(), extra) {
            a.extend(e);
        }
        let host = self
            .host
            .get_or_insert_with(|| PluginHost::new(crate::plugins::root()));
        let rx = host.call(plugin, &format!("command:{cmd}"), &json!([arg]));
        self.jobs.push(Job {
            rx,
            perms: perms.to_vec(),
            title: title.to_string(),
            doc_id: doc.id,
            rev: doc.rev,
        });
    }

    /// 直接调插件登记的方法 (如 `markdown.fence` 的 render), 不经命令 / 文档参数; 宿主第一次用到时才创建
    pub fn call_raw(
        &mut self,
        plugin: &str,
        method: &str,
        args: &Value,
    ) -> Receiver<Result<String, String>> {
        let host = self
            .host
            .get_or_insert_with(|| PluginHost::new(crate::plugins::root()));
        host.call(plugin, method, args)
    }

    /// 插件被禁用 / 删除: 丢掉运行环境 (宿主没创建过就什么都不用做)
    pub fn unload(&mut self, plugin: &str) {
        if let Some(h) = &self.host {
            h.unload(plugin);
        }
    }

    /// 每帧调用: 收完成的任务, 把结果落到文档 / 提示 / 结果窗口
    pub fn poll(&mut self, docs: &mut [Doc], _active: usize, ctx: &egui::Context) {
        if self.jobs.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        let mut done = vec![];
        for (i, j) in self.jobs.iter().enumerate() {
            match j.rx.try_recv() {
                Ok(r) => done.push((i, r)),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => done.push((i, Err("插件宿主已停止".into()))),
            }
        }
        for (i, r) in done.into_iter().rev() {
            let job = self.jobs.remove(i);
            // 发起命令的文档已关闭: 丢弃结果, 不写到别的文档
            let Some(ix) = docs.iter().position(|d| d.id == job.doc_id) else {
                continue;
            };
            let tip = self.finish(&job, r, &mut docs[ix]);
            if let Some(t) = tip {
                docs[ix].tip = t;
            }
        }
    }

    /// 处理一个结果, 返回要显示的提示
    fn finish(&mut self, job: &Job, r: Result<String, String>, doc: &mut Doc) -> Option<String> {
        let json = match r {
            Ok(j) => j,
            Err(e) => return Some(format!("插件命令失败 (编辑器不受影响): {e}")),
        };
        let out = plugin_host::parse_outcome(&json);
        self.dispatch(job, out, doc)
    }

    fn dispatch(&mut self, job: &Job, out: Outcome, doc: &mut Doc) -> Option<String> {
        if let Some(e) = out.error {
            return Some(e);
        }
        let shows = out.svg.is_some() || out.html.is_some() || out.raw.is_some();
        if let Some(p) = out.edit {
            let tip = if !job.perms.iter().any(|p| p == "editor.write") {
                "插件没有 editor.write 权限, 修改已忽略".to_string()
            } else if doc.rev != job.rev {
                "文档在插件运行期间被修改, 补丁已放弃".to_string()
            } else {
                match doc.apply_line_patch(p.start, p.remove, p.insert, p.cursor) {
                    Ok(()) => out.summary.clone(),
                    Err(e) => e,
                }
            };
            return Some(tip);
        }
        if !shows {
            return if out.summary.is_empty() {
                None
            } else {
                Some(out.summary)
            };
        }
        let (svg, text) = match (&out.svg, &out.html, &out.raw) {
            (Some(s), _, _) => match svg_lite::parse(s) {
                Ok(v) => (Some(v), String::new()),
                Err(_) => (None, s.clone()),
            },
            (_, Some(h), _) => (None, plugin_host::strip_html(h)),
            (_, _, Some(r)) => (None, r.clone()),
            _ => (None, String::new()),
        };
        let tip = out.summary.clone();
        self.win = Some(ResultWin {
            title: job.title.clone(),
            summary: out.summary,
            svg,
            text,
            doc_id: doc.id,
        });
        Some(tip) // 状态栏的「正在运行」提示换成摘要
    }

    /// 结果窗口; 返回被点中的 (文档 id, 0 起的行号)
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        dark_text: Option<egui::Color32>,
    ) -> Option<(u64, usize)> {
        let w = self.win.as_ref()?;
        let mut open = true;
        let mut jump = None;
        let doc_id = w.doc_id;
        egui::Window::new(w.title.clone())
            .open(&mut open)
            .default_size([520.0, 420.0])
            .resizable(true)
            .default_pos(egui::pos2(24.0, 56.0))
            .show(ctx, |ui| {
                if !w.summary.is_empty() {
                    ui.label(&w.summary);
                    ui.separator();
                }
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(svg) = &w.svg {
                            if let Some(line) = svg_lite::show(ui, svg, dark_text) {
                                jump = Some((doc_id, line.saturating_sub(1)));
                            }
                        } else {
                            ui.add(
                                egui::Label::new(egui::RichText::new(&w.text).monospace())
                                    .selectable(true),
                            );
                        }
                    });
            });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.win = None;
        }
        jump
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(lines: &[&str]) -> Doc {
        let mut d = Doc::untitled(7, 1);
        d.lines = lines.iter().map(|s| s.to_string()).collect();
        d
    }
    fn perms(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn doc_args_need_editor_read() {
        let mut d = doc(&["ab", "cd"]);
        d.sel = Some([0, 1, 1, 1]);
        assert_eq!(doc_args(&d, &perms(&[])), json!({}));
        assert_eq!(
            doc_args(&d, &perms(&["editor.write"])),
            json!({}),
            "只有 write 不给文本"
        );
        let a = doc_args(&d, &perms(&["editor.read"]));
        assert_eq!(a["text"], "ab\ncd");
        assert_eq!(a["selection"]["r1"], 1);
    }

    #[test]
    fn selection_text_spans_rows_by_chars() {
        let mut d = doc(&["中文abc", "mid", "xyz"]);
        d.sel = Some([0, 1, 2, 2]);
        assert_eq!(selection_text(&d), "文abc\nmid\nxy");
        d.sel = Some([0, 0, 0, 2]);
        assert_eq!(selection_text(&d), "中文");
        d.sel = None;
        assert_eq!(selection_text(&d), "");
    }

    #[test]
    fn edit_outcome_needs_write_permission_and_is_validated() {
        let mut st = PlugState::default();
        let mut d = doc(&["a", "b"]);
        let patch = r#"{"summary":"已修改","edit":{"start":0,"remove":1,"insert":["X"]}}"#;
        let ro = Job {
            rx: std::sync::mpsc::channel().1,
            perms: perms(&["editor.read"]),
            title: "t".into(),
            doc_id: 7,
            rev: 0,
        };
        let tip = st.finish(&ro, Ok(patch.into()), &mut d).unwrap();
        assert!(tip.contains("editor.write") && d.lines[0] == "a", "{tip}");
        let mut rw = Job {
            perms: perms(&["editor.read", "editor.write"]),
            ..ro
        };
        assert_eq!(
            st.finish(&rw, Ok(patch.into()), &mut d).as_deref(),
            Some("已修改")
        );
        assert_eq!(d.lines, vec!["X", "b"]);
        assert!(
            st.finish(&rw, Ok(patch.into()), &mut d)
                .unwrap()
                .contains("修改"),
            "旧修订号的补丁被放弃"
        );
        rw.rev = d.rev;
        let bad = r#"{"edit":{"start":5,"remove":1,"insert":[]}}"#;
        assert!(st
            .finish(&rw, Ok(bad.into()), &mut d)
            .unwrap()
            .contains("越界"));
        assert!(st
            .finish(&rw, Err("炸了".into()), &mut d)
            .unwrap()
            .contains("炸了"));
        assert!(st.win.is_none());
    }

    #[test]
    fn svg_and_data_results_open_the_window() {
        let mut st = PlugState::default();
        let mut d = doc(&["# a"]);
        let j = Job {
            rx: std::sync::mpsc::channel().1,
            perms: perms(&["editor.read"]),
            title: "图".into(),
            doc_id: 7,
            rev: 0,
        };
        let svg = r##"{"svg":"<svg width=\"50\" height=\"20\"><g data-line=\"1\"><circle cx=\"5\" cy=\"5\" r=\"3\" fill=\"#fff\"/></g></svg>","summary":"1 个标题"}"##;
        assert_eq!(
            st.finish(&j, Ok(svg.into()), &mut d).as_deref(),
            Some("1 个标题")
        );
        let w = st.win.take().unwrap();
        assert!(w.svg.is_some() && w.summary == "1 个标题" && w.doc_id == 7);
        // 解析不了的 SVG 退回显示源码; 没有已知字段的结果显示数据
        st.finish(&j, Ok(r#"{"svg":"<svg></svg>"}"#.into()), &mut d);
        assert!(st.win.take().unwrap().svg.is_none());
        st.finish(&j, Ok(r#"{"headings":2}"#.into()), &mut d);
        assert!(st.win.take().unwrap().text.contains("headings"));
        assert_eq!(
            st.finish(&j, Ok(r#"{"summary":"仅提示"}"#.into()), &mut d)
                .as_deref(),
            Some("仅提示")
        );
        assert!(st.win.is_none());
    }

    #[test]
    fn host_is_not_created_until_a_command_runs() {
        let mut st = PlugState::default();
        assert!(!st.started() && st.jobs.is_empty());
        st.unload("ppa"); // 没起宿主时 unload 不会把它建出来
        assert!(!st.started());
    }
}
