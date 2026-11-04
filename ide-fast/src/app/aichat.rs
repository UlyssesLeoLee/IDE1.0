//! AI 对话面板 (右侧): 流式回复、工具往返 (内建 + 插件 agent.tool)、补丁提案审核、插入编辑器、取消。
//! 联网只发生在点「发送」之后; 传输、密钥处理、解析都在 `ai.rs` (有单测)。配置在第一次打开面板时才读盘, 启动路径零开销。

use super::*;
use crate::ai::{self, AiConfig, Event, Handle, Msg, ToolCall};
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

struct Proposal {
    doc_id: u64,
    start: usize,
    remove: usize,
    insert: Vec<String>,
    old: Vec<String>,
    reason: String,
}

enum Item {
    User(String),
    Ai(String),
    Tool(String),
    Err(String),
    Sys(String),
    Proposal(Proposal),
}

struct Pending {
    slot: usize,
    call: ToolCall,
    rx: Receiver<Result<String, String>>,
}

struct Run {
    rx: Receiver<Event>,
    handle: Handle,
    msgs: Vec<Value>,
    tools: Vec<Value>,
    round: usize,
    doc_id: u64,
    /// 正在流式追加的 Ai 条目下标
    stream: Option<usize>,
    /// 本轮工具结果: 按调用顺序, None = 还在等插件
    slots: Vec<Option<Value>>,
    pending: Vec<Pending>,
}

#[derive(Default)]
pub(super) struct AiState {
    pub open: bool,
    cfg: Option<AiConfig>,
    settings: bool,
    input: String,
    use_ctx: bool,
    items: Vec<Item>,
    history: Vec<Msg>,
    run: Option<Run>,
    /// 等用户同意发送上下文的提问
    ask: Option<String>,
    /// 设置里「自定义档位」输入框的文字 (打开设置后第一次画时从配置里填)
    efforts_text: Option<String>,
    /// 正在向供应商获取 effort 档位 (后台线程把结果送回这里)
    fetch: Option<Receiver<Result<Vec<String>, String>>>,
    /// 获取档位的结果 / 错误说明
    fetch_msg: String,
}

enum Act {
    Send,
    Cancel,
    Clear,
    Apply(usize),
    Reject(usize),
    Insert(String),
    Copy(String),
    Save,
}

impl App {
    /// 保存 AI 配置; 写盘失败要让用户看见
    fn ai_save_cfg(&mut self) {
        if let Err(e) = self.ai_cfg().save() {
            let m = tr("ai.save-failed").replacen("{}", &e, 1);
            self.ai.items.push(Item::Err(m));
        }
    }

    fn ai_cfg(&mut self) -> &mut AiConfig {
        self.ai.cfg.get_or_insert_with(AiConfig::load)
    }

    pub(super) fn ai_toggle(&mut self) {
        self.ai.open = !self.ai.open;
        if self.ai.open {
            let _ = self.ai_cfg();
        }
    }

    /// 点发送 / Enter: 先校验配置; 要带上下文且没同意过就弹确认
    fn ai_send(&mut self, ctx: &egui::Context) {
        let q = self.ai.input.trim().to_string();
        if q.is_empty() || self.ai.run.is_some() {
            return;
        }
        if !self.ai_cfg().configured() {
            self.ai
                .items
                .push(Item::Err(tr("ai.not-configured").into()));
            self.ai.settings = true;
            return;
        }
        let i = self.active.min(self.docs.len() - 1);
        let has_ctx = self.ai.use_ctx && self.ai_context(i).is_some();
        if has_ctx && !self.ai_cfg().consent {
            self.ai.ask = Some(q);
            return;
        }
        self.ai_start(q, has_ctx, ctx);
    }

    fn ai_context(&self, i: usize) -> Option<ai::Context> {
        let d = &self.docs[i];
        let n = d.name.to_lowercase();
        ai::context_of(
            &d.lines,
            Some(crate::plugin_ui::selection_text(d)),
            d.cursor[0],
            n.ends_with(".md") || n.ends_with(".markdown"),
        )
    }

    fn ai_start(&mut self, q: String, with_ctx: bool, ctx: &egui::Context) {
        let i = self.active.min(self.docs.len() - 1);
        let c = if with_ctx { self.ai_context(i) } else { None };
        let sys = ai::system_prompt(&self.docs[i].name, self.docs[i].lines.len(), c.as_ref());
        let msgs = ai::base_messages(sys, &self.ai.history, &q);
        let installed = editcore::plugin_host::store::list(&plugins::root());
        let tools = ai::tool_table(&installed);
        self.ai.input.clear();
        self.ai.items.push(Item::User(q));
        let (rx, handle) = self.ai_spawn(&msgs, &tools, ctx);
        self.ai.run = Some(Run {
            rx,
            handle,
            msgs,
            tools,
            round: 0,
            doc_id: self.docs[i].id,
            stream: None,
            slots: vec![],
            pending: vec![],
        });
    }

    fn ai_spawn(
        &mut self,
        msgs: &[Value],
        tools: &[Value],
        ctx: &egui::Context,
    ) -> (Receiver<Event>, Handle) {
        let (tx, rx) = channel();
        let c = ctx.clone();
        let cfg = self.ai_cfg().clone();
        let h = ai::spawn_request(&cfg, msgs.to_vec(), tools.to_vec(), tx, move || {
            c.request_repaint()
        });
        (rx, h)
    }

    /// 每帧: 收流式文字 / 回合结果 / 插件工具结果。没有请求时什么都不做 (无空转轮询)
    pub(super) fn ai_poll(&mut self, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        let mut events = vec![];
        loop {
            match run.rx.try_recv() {
                Ok(e) => events.push(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    events.push(Event::Done(ai::Turn {
                        error: Some("请求线程意外结束".into()),
                        ..Default::default()
                    }));
                    break;
                }
            }
        }
        for e in events {
            match e {
                Event::Text(t) => self.ai_append(&t),
                Event::Done(turn) => self.ai_turn(turn, ctx),
            }
            if self.ai.run.is_none() {
                return;
            }
        }
        self.ai_poll_plugins(ctx);
    }

    fn ai_append(&mut self, t: &str) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        match run.stream {
            Some(i) => {
                if let Some(Item::Ai(s)) = self.ai.items.get_mut(i) {
                    s.push_str(t);
                }
            }
            None => {
                self.ai.items.push(Item::Ai(t.to_string()));
                run.stream = Some(self.ai.items.len() - 1);
            }
        }
    }

    fn ai_turn(&mut self, turn: ai::Turn, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        run.stream = None;
        if let Some(e) = &turn.error {
            self.ai.items.push(Item::Err(e.clone()));
            self.ai.run = None;
            return;
        }
        run.msgs.push(ai::assistant_message(&turn));
        run.round += 1;
        if turn.tool_calls.is_empty() || run.round >= ai::MAX_ROUNDS {
            self.ai_finish();
            return;
        }
        let doc_id = run.doc_id;
        let rows = self
            .docs
            .iter()
            .find(|d| d.id == doc_id)
            .map_or(0, |d| d.lines.len());
        let installed = editcore::plugin_host::store::list(&plugins::root());
        if let Some(run) = self.ai.run.as_mut() {
            run.slots = vec![None; turn.tool_calls.len()];
        }
        let mut immediate = vec![];
        let mut spawn = vec![];
        for (slot, call) in turn.tool_calls.iter().enumerate() {
            let args = ai::parse_args(&call.args);
            if let Some((pid, tool)) = ai::split_plugin_tool(&call.name) {
                if ai::plugin_tool_declared(&installed, &pid, &tool) {
                    spawn.push((slot, call.clone(), pid, tool, args));
                } else {
                    immediate.push((
                        slot,
                        call.clone(),
                        json!({ "error": format!("unknown tool: {}", call.name) }),
                    ));
                }
            } else {
                immediate.push((
                    slot,
                    call.clone(),
                    ai::exec_builtin(&call.name, &args, rows),
                ));
            }
        }
        for (slot, call, pid, tool, args) in spawn {
            let rx = self
                .plug
                .call_raw(&pid, &format!("agent.tool:{tool}"), &json!([args]));
            if let Some(run) = self.ai.run.as_mut() {
                run.pending.push(Pending { slot, call, rx });
            }
        }
        for (slot, call, r) in immediate {
            self.ai_tool_done(slot, &call, r, doc_id);
        }
        self.ai_next_round(ctx);
    }

    fn ai_poll_plugins(&mut self, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        if run.pending.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        let doc_id = run.doc_id;
        let mut done = vec![];
        let mut i = 0;
        while i < run.pending.len() {
            let r = match run.pending[i].rx.try_recv() {
                Ok(r) => Some(r),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err("插件宿主已停止".into())),
            };
            match r {
                Some(r) => done.push((run.pending.remove(i), r)),
                None => i += 1,
            }
        }
        let rows = self
            .docs
            .iter()
            .find(|d| d.id == doc_id)
            .map_or(0, |d| d.lines.len());
        for (p, r) in done {
            let v = match r {
                Ok(j) => match serde_json::from_str::<Value>(&j) {
                    Ok(v) if v.is_null() => json!({}),
                    Ok(v) => ai::vet_plugin_patch(&v, rows),
                    Err(_) => json!({ "error": "插件工具返回的不是 JSON" }),
                },
                Err(e) => json!({ "error": format!("插件工具失败: {e}") }),
            };
            self.ai_tool_done(p.slot, &p.call, v, doc_id);
        }
        self.ai_next_round(ctx);
    }

    fn ai_tool_done(&mut self, slot: usize, call: &ToolCall, result: Value, doc_id: u64) {
        let line = match result["error"].as_str() {
            Some(e) => trf("ai.tool-fail", &[&call.name, e]),
            None => trf("ai.tool-ok", &[&call.name]),
        };
        self.ai.items.push(Item::Tool(line));
        if result["kind"] == "patch_proposal" {
            let start = result["start"].as_u64().unwrap_or(0) as usize;
            let remove = result["remove"].as_u64().unwrap_or(0) as usize;
            let insert: Vec<String> = result["insert"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let old = self
                .docs
                .iter()
                .find(|d| d.id == doc_id)
                .map(|d| d.lines.iter().skip(start).take(remove).cloned().collect())
                .unwrap_or_default();
            self.ai.items.push(Item::Proposal(Proposal {
                doc_id,
                start,
                remove,
                insert,
                old,
                reason: result["reason"].as_str().unwrap_or("").to_string(),
            }));
        }
        if let Some(run) = self.ai.run.as_mut() {
            if let Some(s) = run.slots.get_mut(slot) {
                *s = Some(ai::tool_message(&call.id, &result));
            }
        }
    }

    /// 本轮工具都有结果了 -> 把结果并进消息, 发下一轮
    fn ai_next_round(&mut self, ctx: &egui::Context) {
        let Some(run) = self.ai.run.as_mut() else {
            return;
        };
        if !run.pending.is_empty() || run.slots.iter().any(Option::is_none) {
            return;
        }
        let results: Vec<Value> = run.slots.drain(..).flatten().collect();
        run.msgs.extend(results);
        let (msgs, tools) = (run.msgs.clone(), run.tools.clone());
        let (rx, handle) = self.ai_spawn(&msgs, &tools, ctx);
        if let Some(run) = self.ai.run.as_mut() {
            run.rx = rx;
            run.handle = handle;
        }
    }

    /// 正常结束: 只留来回的文字进下一轮历史
    fn ai_finish(&mut self) {
        if let Some(run) = self.ai.run.take() {
            let h: Vec<Msg> = run
                .msgs
                .iter()
                .skip(1)
                .filter(|m| {
                    m["role"] == "user"
                        || (m["role"] == "assistant"
                            && m["tool_calls"].is_null()
                            && m["content"].as_str().is_some_and(|s| !s.is_empty()))
                })
                .map(|m| Msg {
                    role: m["role"].as_str().unwrap_or("").into(),
                    content: m["content"].as_str().unwrap_or("").into(),
                })
                .collect();
            self.ai.history = ai::trim_history(h);
        }
    }

    fn ai_cancel(&mut self) {
        if let Some(run) = self.ai.run.take() {
            run.handle.cancel();
            self.ai.items.push(Item::Sys(tr("ai.cancelled").into()));
        }
    }

    fn ai_apply(&mut self, idx: usize) {
        let Some(Item::Proposal(p)) = self.ai.items.get(idx) else {
            return;
        };
        let msg = match self.docs.iter_mut().find(|d| d.id == p.doc_id) {
            Some(d)
                if d.lines.len() >= p.start + p.remove
                    && d.lines[p.start..p.start + p.remove] == p.old[..] =>
            {
                match d.apply_line_patch(p.start, p.remove, p.insert.clone(), None) {
                    Ok(()) => {
                        d.tip = tr("ai.applied").into();
                        Item::Sys(tr("ai.applied").into())
                    }
                    Err(e) => Item::Err(e),
                }
            }
            _ => Item::Err(tr("ai.stale").into()),
        };
        self.ai.items[idx] = msg;
    }

    pub(super) fn ai_windows(&mut self, ctx: &egui::Context) {
        let Some(q) = self.ai.ask.clone() else { return };
        let url = self.ai_cfg().base_url.clone();
        let i = self.active.min(self.docs.len() - 1);
        let (label, n) = self
            .ai_context(i)
            .map(|c| (c.label, c.text.chars().count()))
            .unwrap_or(("", 0));
        let mut choice = None;
        egui::Window::new(tr("ai.consent-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(trf("ai.consent", &[label, &n.to_string(), &url]));
                ui.horizontal(|ui| {
                    if ui.button(tr("ai.allow")).clicked() {
                        choice = Some(true);
                    }
                    if ui.button(tr("ai.deny")).clicked() {
                        choice = Some(false);
                    }
                });
            });
        if let Some(allow) = choice {
            self.ai.ask = None;
            if allow {
                let c = self.ai_cfg();
                c.consent = true;
                self.ai_save_cfg();
            }
            self.ai_start(q, allow, ctx);
        }
    }

    pub(super) fn ai_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut act: Option<Act> = None;
        let busy = self.ai.run.is_some();
        // 头部 (Tauri `#chat-head`): 左「Chat」(11px 等宽), 右 ✕ / 清空 / 「带上选中内容 / 当前章节」勾选
        let tc = self.tc;
        let edge = if tc.dark {
            Color32::from_gray(69)
        } else {
            Color32::from_gray(204)
        };
        let boxed = |t: &str| {
            egui::Button::new(egui::RichText::new(t).size(12.0).color(tc.fg))
                .fill(Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, edge))
                .corner_radius(4.0)
                .min_size(Vec2::new(0.0, 26.0))
        };
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            ui.label(egui::RichText::new("Chat").monospace().size(11.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(boxed("×").min_size(Vec2::new(26.0, 26.0)))
                    .tip(trz("折叠 (Ctrl+Alt+L)"))
                    .clicked()
                {
                    self.ai.open = false;
                }
                if ui.add(boxed(tr("ai.clear"))).clicked() {
                    act = Some(Act::Clear);
                }
                ui.add(egui::Checkbox::new(
                    &mut self.ai.use_ctx,
                    egui::RichText::new(tr("ai.ctx")).size(11.0),
                ));
            });
        });
        if self.ai.settings {
            ui.separator();
            let cfg = self.ai.cfg.get_or_insert_with(AiConfig::load);
            ui.horizontal_wrapped(|ui| {
                for (name, url) in ai::PRESETS {
                    if ui.small_button(*name).clicked() {
                        cfg.base_url = (*url).to_string();
                    }
                }
            });
            egui::Grid::new("ai_cfg").num_columns(2).show(ui, |ui| {
                ui.label(tr("ai.base-url"));
                ui.add(egui::TextEdit::singleline(&mut cfg.base_url).desired_width(220.0));
                ui.end_row();
                ui.label(tr("ai.model"));
                ui.add(egui::TextEdit::singleline(&mut cfg.model).desired_width(220.0));
                ui.end_row();
                ui.label(tr("ai.key"));
                ui.add(
                    egui::TextEdit::singleline(&mut cfg.key)
                        .password(true)
                        .desired_width(220.0),
                );
                ui.end_row();
                ui.label(tr("ai.effort"));
                // 档位下拉: 默认 (不指定) + 获取到 / 手填的档位 (没有就用通用默认); 当前值不在列表里也照样显示
                let opts = ai::options(cfg);
                let shown = if cfg.effort.is_empty() {
                    tr("ai.effort-default").to_string()
                } else {
                    cfg.effort.clone()
                };
                egui::ComboBox::from_id_salt("ai-effort")
                    .width(220.0)
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut cfg.effort,
                            String::new(),
                            tr("ai.effort-default"),
                        );
                        for o in &opts {
                            ui.selectable_value(&mut cfg.effort, o.clone(), o);
                        }
                    });
                ui.end_row();
                ui.label(tr("ai.effort-custom"));
                let text = self
                    .ai
                    .efforts_text
                    .get_or_insert_with(|| cfg.efforts.join(","));
                if ui
                    .add(
                        egui::TextEdit::singleline(text)
                            .hint_text("low,medium,high,xhigh")
                            .desired_width(220.0),
                    )
                    .changed()
                {
                    cfg.efforts = ai::parse_custom(text);
                }
                ui.end_row();
            });
            // 从供应商获取档位: 点了才联网, 后台线程查模型接口 (密钥只发给上面填的接口地址)
            ui.horizontal(|ui| {
                let busy = self.ai.fetch.is_some();
                if ui
                    .add_enabled(!busy, egui::Button::new(tr("ai.effort-fetch")))
                    .clicked()
                {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let snapshot = cfg.clone();
                    let ctx = ui.ctx().clone();
                    let _ =
                        std::thread::Builder::new()
                            .name("ai-efforts".into())
                            .spawn(move || {
                                let _ = tx.send(ai::fetch_efforts(&snapshot));
                                ctx.request_repaint();
                            });
                    self.ai.fetch = Some(rx);
                    self.ai.fetch_msg = tr("ai.effort-fetching").to_string();
                }
                if busy {
                    ui.spinner();
                }
            });
            if let Some(rx) = &self.ai.fetch {
                match rx.try_recv() {
                    Ok(res) => {
                        self.ai.fetch = None;
                        self.ai.fetch_msg = match res {
                            Ok(list) => {
                                let msg = trf(
                                    "ai.effort-got",
                                    &[&list.len().to_string(), &list.join(", ")],
                                );
                                self.ai.efforts_text = Some(list.join(","));
                                cfg.efforts = list;
                                msg
                            }
                            Err(e) => e,
                        };
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => ui
                        .ctx()
                        .request_repaint_after(std::time::Duration::from_millis(150)),
                    Err(_) => self.ai.fetch = None,
                }
            }
            if !self.ai.fetch_msg.is_empty() {
                ui.weak(self.ai.fetch_msg.as_str());
            }
            if let Err(e) = ai::endpoint(&cfg.base_url) {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            ui.weak(tr("ai.key-note"));
            if ui.button(tr("ai.save")).clicked() {
                act = Some(Act::Save);
            }
        }
        ui.separator();
        let avail = (ui.available_height() - 110.0).max(60.0);
        egui::ScrollArea::vertical()
            .max_height(avail)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for idx in 0..self.ai.items.len() {
                    match &self.ai.items[idx] {
                        Item::User(t) => {
                            ui.label(egui::RichText::new(format!("{} {t}", tr("ai.you"))).strong());
                        }
                        Item::Ai(t) => {
                            ui.label(t.as_str());
                            if !t.is_empty() && !busy {
                                ui.horizontal(|ui| {
                                    if ui.small_button(tr("ai.insert")).clicked() {
                                        act = Some(Act::Insert(t.clone()));
                                    }
                                    if ui.small_button(tr("ai.copy")).clicked() {
                                        act = Some(Act::Copy(t.clone()));
                                    }
                                });
                            }
                        }
                        Item::Tool(t) => {
                            ui.weak(t.as_str());
                        }
                        Item::Err(t) => {
                            ui.colored_label(Color32::LIGHT_RED, t.as_str());
                        }
                        Item::Sys(t) => {
                            ui.weak(t.as_str());
                        }
                        Item::Proposal(p) => {
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                let head = trf(
                                    "ai.proposal",
                                    &[&(p.start + 1).to_string(), &p.remove.to_string()],
                                );
                                ui.label(if p.reason.is_empty() {
                                    head
                                } else {
                                    format!("{head} — {}", p.reason)
                                });
                                for l in &p.old {
                                    ui.colored_label(
                                        Color32::from_rgb(220, 110, 110),
                                        format!("- {l}"),
                                    );
                                }
                                for l in &p.insert {
                                    ui.colored_label(
                                        Color32::from_rgb(110, 200, 120),
                                        format!("+ {l}"),
                                    );
                                }
                                ui.horizontal(|ui| {
                                    if ui.button(tr("ai.apply")).clicked() {
                                        act = Some(Act::Apply(idx));
                                    }
                                    if ui.button(tr("ai.reject")).clicked() {
                                        act = Some(Act::Reject(idx));
                                    }
                                });
                            });
                        }
                    }
                    ui.add_space(4.0);
                }
                if busy {
                    ui.weak(tr("ai.thinking"));
                }
            });
        ui.separator();
        // 模型行 (Tauri 聊天底部): 左 = 当前模型 (没设就写「未设置模型」), 右 = 模型设置…
        let model = self.ai_cfg().model.clone();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if model.is_empty() {
                    trz("未设置模型").to_string()
                } else {
                    model
                })
                .size(11.0)
                .color(tc.fg_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(boxed(&format!("{}…", trz("模型设置")))).clicked() {
                    self.ai.settings = !self.ai.settings;
                }
            });
        });
        let id = ui.make_persistent_id("ai_input");
        let enter = ui.memory(|m| m.has_focus(id))
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        // 输入框 + 右侧「发送」(忙时是「取消」): Tauri 的聊天输入是一行两栏
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 62.0).max(80.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.ai.input)
                    .id(id)
                    .desired_rows(3)
                    .desired_width(w)
                    .hint_text(tr("ai.placeholder")),
            );
            if busy {
                if ui.add(boxed(tr("ai.cancel"))).clicked() {
                    act = Some(Act::Cancel);
                }
            } else if ui.add(boxed(tr("ai.send"))).clicked() || enter {
                act = Some(Act::Send);
            }
        });
        match act {
            Some(Act::Send) => self.ai_send(&ctx),
            Some(Act::Cancel) => self.ai_cancel(),
            Some(Act::Clear) => {
                self.ai_cancel();
                self.ai.items.clear();
                self.ai.history.clear();
            }
            Some(Act::Apply(i)) => self.ai_apply(i),
            Some(Act::Reject(i)) => self.ai.items[i] = Item::Sys(tr("ai.rejected").into()),
            Some(Act::Insert(t)) => {
                self.cur().edit("paste", Some(t));
            }
            Some(Act::Copy(t)) => ctx.copy_text(t),
            Some(Act::Save) => {
                self.ai_save_cfg();
                self.ai.settings = false;
            }
            None => {}
        }
    }
}
