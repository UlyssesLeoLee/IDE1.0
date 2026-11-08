//! 真实多 Agent 协同工作台 (Multi-Agent Collab Dashboard)
//! 支持设置子 Agent 数量、动态标签页切换，每个标签页具备统筹总控命令窗口与单独命令窗口。

use super::*;
use crate::ai::{self, AiConfig, Event, Handle};
use serde_json::json;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// 协同对话消息项
#[derive(Clone, Debug, PartialEq)]
pub struct CollabMsg {
    pub sender: String,
    pub text: String,
    pub is_user: bool,
    pub is_error: bool,
}

impl CollabMsg {
    pub fn user(sender: &str, text: &str) -> Self {
        Self {
            sender: sender.to_string(),
            text: text.to_string(),
            is_user: true,
            is_error: false,
        }
    }

    pub fn agent(sender: &str, text: &str) -> Self {
        Self {
            sender: sender.to_string(),
            text: text.to_string(),
            is_user: false,
            is_error: false,
        }
    }

    pub fn system(text: &str) -> Self {
        Self {
            sender: "系统".to_string(),
            text: text.to_string(),
            is_user: false,
            is_error: false,
        }
    }

    pub fn error(sender: &str, text: &str) -> Self {
        Self {
            sender: sender.to_string(),
            text: text.to_string(),
            is_user: false,
            is_error: true,
        }
    }
}

/// 正在执行的子 Agent 请求状态
pub struct SubAgentRun {
    pub rx: Receiver<Event>,
    pub handle: Handle,
    pub is_master_broadcast: bool,
    pub streaming_text: String,
}

/// 单个子 Agent 状态
pub struct SubAgent {
    pub id: String,
    pub name: String,
    pub role: String,
    pub private_history: Vec<CollabMsg>,
    pub private_input: String,
    pub run: Option<SubAgentRun>,
}

impl SubAgent {
    pub fn new(index: usize) -> Self {
        let (name, role): (String, &'static str) = match index {
            0 => ("Agent #1 (编码员)".to_string(), "负责核心代码生成、算法实现与模块架构落地"),
            1 => ("Agent #2 (审查员)".to_string(), "负责质量审查、边界校验、安全性与不变量审核"),
            2 => ("Agent #3 (测试员)".to_string(), "负责测试用例构建、异常模拟与回归验证"),
            3 => ("Agent #4 (优化员)".to_string(), "负责性能瓶颈排查、并发同步与系统资源优化"),
            4 => ("Agent #5 (架构员)".to_string(), "负责跨模块接口设计与契约约束"),
            n => (format!("Agent #{}", n + 1), "协同协作者"),
        };
        Self {
            id: format!("agent_{}", index + 1),
            name: name.clone(),
            role: role.to_string(),
            private_history: vec![CollabMsg::system(&format!(
                "{} 已就绪。\n职责定位: {}\n可在下方命令窗口向此 Agent 发送针对性指令。",
                name, role
            ))],
            private_input: String::new(),
            run: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }
}

/// 全局协同工作台状态
pub struct CollabState {
    pub open: bool,
    pub agent_count: usize,
    pub active_tab: usize,
    pub agents: Vec<SubAgent>,
    pub master_history: Vec<CollabMsg>,
    pub master_input: String,
}

impl Default for CollabState {
    fn default() -> Self {
        let count = 3;
        let agents = (0..count).map(SubAgent::new).collect();
        Self {
            open: false,
            agent_count: count,
            active_tab: 0,
            agents,
            master_history: vec![CollabMsg::system(
                "🌐 多 Agent 统筹总控已就绪。在此处输入的指令将广播统筹调度所有协同子 Agent。",
            )],
            master_input: String::new(),
        }
    }
}

impl CollabState {
    /// 动态设置子 Agent 数量 (1 ~ 8)，没有运行任务时可自由增减，动态生成标签页
    pub fn set_agent_count(&mut self, new_count: usize) {
        let count = new_count.clamp(1, 8);
        if count == self.agent_count {
            return;
        }
        self.agent_count = count;
        if self.agents.len() < count {
            for i in self.agents.len()..count {
                self.agents.push(SubAgent::new(i));
            }
        } else if self.agents.len() > count {
            self.agents.truncate(count);
        }
        self.active_tab = self.active_tab.min(self.agents.len().saturating_sub(1));
    }

    /// 是否有任一子 Agent 正在运行中
    pub fn any_running(&self) -> bool {
        self.agents.iter().any(|a| a.is_running())
    }

    /// 停止所有正在运行的 Agent
    pub fn stop_all(&mut self) {
        for a in &mut self.agents {
            if let Some(r) = a.run.take() {
                r.handle.cancel();
            }
        }
        self.master_history.push(CollabMsg::system("已中止全员协同任务。"));
    }

    /// 停止指定的子 Agent
    pub fn stop_one(&mut self, idx: usize) {
        if let Some(a) = self.agents.get_mut(idx) {
            if let Some(r) = a.run.take() {
                r.handle.cancel();
                a.private_history.push(CollabMsg::system("已中止当前 Agent 任务。"));
            }
        }
    }

    /// 全员统筹下发命令 (广播给所有子 Agent 协同并发执行)
    pub fn send_master(
        &mut self,
        prompt: String,
        cfg: &AiConfig,
        ctx: &egui::Context,
    ) {
        let q = prompt.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.master_input.clear();
        self.master_history.push(CollabMsg::user("统筹管理员", &q));

        if !cfg.configured() {
            self.master_history.push(CollabMsg::error(
                "系统",
                "AI 服务尚未配置 API Key 或 Base URL，请先在首选项或 AI 设置中填入有效配置以启用真实大模型协同。",
            ));
            return;
        }

        // 为每一个子 Agent 启动真实并发请求
        for a in &mut self.agents {
            if let Some(old) = a.run.take() {
                old.handle.cancel();
            }
            let agent_prompt = format!(
                "【多 Agent 协同统筹命令】: {}\n你的身份是: {}，专职角色: {}。\n请在你的专业领域内协同执行此任务，并给出具体执行结果与汇报说明。",
                q, a.name, a.role
            );
            let (tx, rx) = channel();
            let c = ctx.clone();
            let msgs = vec![
                json!({ "role": "system", "content": format!("你是集成在 IDE1.0 中的协作智能体 {}。角色职责: {}。", a.name, a.role) }),
                json!({ "role": "user", "content": agent_prompt }),
            ];
            let handle = ai::spawn_request(cfg, msgs, vec![], tx, move || {
                c.request_repaint();
            });
            a.run = Some(SubAgentRun {
                rx,
                handle,
                is_master_broadcast: true,
                streaming_text: String::new(),
            });
        }
    }

    /// 针对特定子 Agent 单独下达针对性命令
    pub fn send_individual(
        &mut self,
        idx: usize,
        prompt: String,
        cfg: &AiConfig,
        ctx: &egui::Context,
    ) {
        let Some(a) = self.agents.get_mut(idx) else {
            return;
        };
        let q = prompt.trim().to_string();
        if q.is_empty() {
            return;
        }
        a.private_input.clear();
        a.private_history.push(CollabMsg::user("你", &q));

        if !cfg.configured() {
            a.private_history.push(CollabMsg::error(
                "系统",
                "AI 服务尚未配置，请先在设置中填写 Base URL 与 API Key。",
            ));
            return;
        }

        if let Some(old) = a.run.take() {
            old.handle.cancel();
        }

        let (tx, rx) = channel();
        let c = ctx.clone();
        let msgs = vec![
            json!({ "role": "system", "content": format!("你是 {}。角色: {}。现在用户正在与你单独进行深度交互与指令下发。", a.name, a.role) }),
            json!({ "role": "user", "content": q }),
        ];
        let handle = ai::spawn_request(cfg, msgs, vec![], tx, move || {
            c.request_repaint();
        });
        a.run = Some(SubAgentRun {
            rx,
            handle,
            is_master_broadcast: false,
            streaming_text: String::new(),
        });
    }

    /// 每帧轮询接收各个子 Agent 的流式输出与完成事件
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut completed = vec![];
        for (idx, a) in self.agents.iter_mut().enumerate() {
            let Some(run) = a.run.as_mut() else {
                continue;
            };
            let mut got_done = None;
            loop {
                match run.rx.try_recv() {
                    Ok(Event::Text(t)) => {
                        run.streaming_text.push_str(&t);
                        ctx.request_repaint();
                    }
                    Ok(Event::Done(turn)) => {
                        got_done = Some(turn);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        got_done = Some(crate::ai::Turn {
                            error: Some("请求通道异常中断".into()),
                            ..Default::default()
                        });
                        break;
                    }
                }
            }
            if let Some(turn) = got_done {
                completed.push((idx, turn));
            }
        }

        for (idx, turn) in completed {
            if let Some(a) = self.agents.get_mut(idx) {
                let Some(run) = a.run.take() else {
                    continue;
                };
                let content = if run.streaming_text.is_empty() {
                    turn.content
                } else {
                    run.streaming_text
                };

                if let Some(err) = turn.error {
                    a.private_history.push(CollabMsg::error(&a.name, &err));
                    if run.is_master_broadcast {
                        self.master_history.push(CollabMsg::error(&a.name, &err));
                    }
                } else {
                    a.private_history.push(CollabMsg::agent(&a.name, &content));
                    if run.is_master_broadcast {
                        self.master_history.push(CollabMsg::agent(&a.name, &content));
                    }
                }
            }
        }
    }
}

impl App {
    /// 渲染真实 Multi-Agent 协同工作台窗口
    pub(super) fn collab_window(&mut self, ctx: &egui::Context) {
        if !self.collab.open {
            return;
        }

        let mut is_open = self.collab.open;
        let mut master_send: Option<String> = None;
        let mut individual_send: Option<(usize, String)> = None;
        let mut stop_all_clicked = false;
        let mut stop_one_clicked = None;
        let active_tab = self.collab.active_tab;

        egui::Window::new("🤖 多 Agent 协同工作台 (Multi-Agent Collab)")
            .open(&mut is_open)
            .default_size(Vec2::new(760.0, 560.0))
            .min_size(Vec2::new(520.0, 400.0))
            .show(ctx, |ui| {
                // 1. 顶部协同控制条：设置数量与全员状态
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("协同 Agent 数量:").strong());
                    let running = self.collab.any_running();
                    if ui.add_enabled(!running && self.collab.agent_count > 1, egui::Button::new("➖")).clicked() {
                        self.collab.set_agent_count(self.collab.agent_count - 1);
                    }
                    ui.label(
                        egui::RichText::new(format!("{}", self.collab.agent_count))
                            .strong()
                            .color(Color32::from_rgb(0, 190, 255)),
                    );
                    if ui.add_enabled(!running && self.collab.agent_count < 8, egui::Button::new("➕")).clicked() {
                        self.collab.set_agent_count(self.collab.agent_count + 1);
                    }
                    if running {
                        ui.label(egui::RichText::new("⚡ 协同执行中...").color(Color32::from_rgb(0, 220, 130)));
                        if ui.button("⏹ 终止全员").clicked() {
                            stop_all_clicked = true;
                        }
                    } else {
                        ui.label(egui::RichText::new("🟢 全员就绪").color(Color32::from_rgb(120, 200, 120)));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("🧹 清空统筹记录").clicked() {
                            self.collab.master_history.clear();
                        }
                    });
                });

                ui.separator();

                // 2. 子 Agent 标签页栏 (每个子 Agent 一个标签页)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (i, a) in self.collab.agents.iter().enumerate() {
                        let is_active = i == active_tab;
                        let status_marker = if a.is_running() { " ⚡" } else { "" };
                        let label = format!("🤖 {}{}", a.name, status_marker);
                        if ui.selectable_label(is_active, label).clicked() {
                            self.collab.active_tab = i;
                        }
                    }
                });

                ui.separator();

                // 3. 当前选中的子 Agent 标签页内容：上下切分两大独立窗口
                let cur_idx = self.collab.active_tab.min(self.collab.agents.len().saturating_sub(1));
                let cur_agent_name = self.collab.agents.get(cur_idx).map(|a| a.name.clone()).unwrap_or_default();
                let cur_agent_role = self.collab.agents.get(cur_idx).map(|a| a.role.clone()).unwrap_or_default();
                let cur_running = self.collab.agents.get(cur_idx).is_some_and(|a| a.is_running());

                let total_avail = ui.available_height();
                let half_height = (total_avail * 0.48).max(160.0);

                // --- 上部：🌐 统筹命令窗口 (全员广播与协调) ---
                ui.group(|ui| {
                    ui.set_height(half_height);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🌐 统筹命令窗口 (全局广播调度全体 Agent)").strong().color(Color32::from_rgb(0, 190, 255)));
                    });

                    // 统筹历史交互记录展示
                    egui::ScrollArea::vertical()
                        .max_height(half_height - 64.0)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for m in &self.collab.master_history {
                                render_collab_msg(ui, m);
                            }
                        });

                    // 统筹命令输入行
                    ui.horizontal(|ui| {
                        let re = ui.add(
                            egui::TextEdit::singleline(&mut self.collab.master_input)
                                .hint_text("输入统筹命令 (如：派出全部agent分别生成20个随机数，将广播分发给所有Agent)...")
                                .desired_width(ui.available_width() - 150.0),
                        );
                        let enter = re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if (ui.button("🚀 全员统筹下发").clicked() || enter) && !self.collab.master_input.trim().is_empty() {
                            master_send = Some(self.collab.master_input.clone());
                        }
                        if self.collab.any_running() && ui.button("⏹ 停止").clicked() {
                            stop_all_clicked = true;
                        }
                    });
                });

                ui.add_space(4.0);

                // --- 下部：🎯 单独命令当前 Agent 窗口 ---
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("🎯 单独命令窗口 (仅针对 {})", cur_agent_name))
                                .strong()
                                .color(Color32::from_rgb(255, 180, 50)),
                        );
                        ui.weak(format!("职责: {}", cur_agent_role));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🧹 清空私聊").clicked() {
                                if let Some(a) = self.collab.agents.get_mut(cur_idx) {
                                    a.private_history.clear();
                                }
                            }
                        });
                    });

                    // 单独 Agent 历史交互记录展示
                    let sub_height = (ui.available_height() - 40.0).max(100.0);
                    egui::ScrollArea::vertical()
                        .max_height(sub_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if let Some(a) = self.collab.agents.get(cur_idx) {
                                for m in &a.private_history {
                                    render_collab_msg(ui, m);
                                }
                                if let Some(run) = &a.run {
                                    if !run.streaming_text.is_empty() {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(egui::RichText::new(format!("{}: ", a.name)).strong().color(Color32::from_rgb(40, 210, 120)));
                                            ui.label(egui::RichText::new(&run.streaming_text).color(Color32::from_rgb(220, 220, 220)));
                                        });
                                    }
                                }
                            }
                        });

                    // 单独命令输入行
                    if let Some(a) = self.collab.agents.get_mut(cur_idx) {
                        ui.horizontal(|ui| {
                            let re = ui.add(
                                egui::TextEdit::singleline(&mut a.private_input)
                                    .hint_text(format!("单独向 {} 下达针对性指令...", cur_agent_name))
                                    .desired_width(ui.available_width() - 140.0),
                            );
                            let enter = re.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if (ui.button("✉️ 单独发送").clicked() || enter) && !a.private_input.trim().is_empty() {
                                individual_send = Some((cur_idx, a.private_input.clone()));
                            }
                            if cur_running && ui.button("⏹ 停止").clicked() {
                                stop_one_clicked = Some(cur_idx);
                            }
                        });
                    }
                });
            });

        self.collab.open = is_open;

        if stop_all_clicked {
            self.collab.stop_all();
        }
        if let Some(idx) = stop_one_clicked {
            self.collab.stop_one(idx);
        }
        if let Some(prompt) = master_send {
            let cfg = self.ai_cfg().clone();
            self.collab.send_master(prompt, &cfg, ctx);
        }
        if let Some((idx, prompt)) = individual_send {
            let cfg = self.ai_cfg().clone();
            self.collab.send_individual(idx, prompt, &cfg, ctx);
        }
    }

    /// 每帧轮询协同后台任务
    pub(super) fn collab_poll(&mut self, ctx: &egui::Context) {
        self.collab.poll(ctx);
    }
}

/// 辅助渲染单条协同消息
fn render_collab_msg(ui: &mut egui::Ui, m: &CollabMsg) {
    ui.horizontal_wrapped(|ui| {
        if m.is_user {
            ui.label(egui::RichText::new(format!("{}: ", m.sender)).strong().color(Color32::from_rgb(100, 180, 255)));
            ui.label(egui::RichText::new(&m.text).color(Color32::from_rgb(235, 235, 235)));
        } else if m.is_error {
            ui.label(egui::RichText::new(format!("{}: ", m.sender)).strong().color(Color32::from_rgb(255, 90, 90)));
            ui.label(egui::RichText::new(&m.text).color(Color32::from_rgb(255, 130, 130)));
        } else if m.sender == "系统" {
            ui.label(egui::RichText::new(&m.text).weak().italics().size(11.5));
        } else {
            ui.label(egui::RichText::new(format!("{}: ", m.sender)).strong().color(Color32::from_rgb(40, 210, 120)));
            ui.label(egui::RichText::new(&m.text).color(Color32::from_rgb(220, 220, 220)));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collab_agent_count_and_tab_expansion() {
        let mut s = CollabState::default();
        assert_eq!(s.agent_count, 3);
        assert_eq!(s.agents.len(), 3);
        assert_eq!(s.active_tab, 0);

        // 扩充至 5 个 Agent
        s.set_agent_count(5);
        assert_eq!(s.agent_count, 5);
        assert_eq!(s.agents.len(), 5);
        assert_eq!(s.agents[4].name, "Agent #5 (架构员)");

        // 缩减至 2 个 Agent
        s.active_tab = 4;
        s.set_agent_count(2);
        assert_eq!(s.agent_count, 2);
        assert_eq!(s.agents.len(), 2);
        assert_eq!(s.active_tab, 1); // 自动限制到 1

        // 边界保护：最小 1，最大 8
        s.set_agent_count(0);
        assert_eq!(s.agent_count, 1);
        s.set_agent_count(99);
        assert_eq!(s.agent_count, 8);
    }

    #[test]
    fn collab_messages_and_roles() {
        let a = SubAgent::new(0);
        assert!(a.name.contains("Agent #1"));
        assert!(a.role.contains("代码"));
        assert!(!a.private_history.is_empty());

        let msg = CollabMsg::user("统筹管理员", "写20个随机数");
        assert!(msg.is_user && !msg.is_error && msg.text == "写20个随机数");

        let err = CollabMsg::error("Agent #2", "HTTP 429 额度耗尽");
        assert!(!err.is_user && err.is_error && err.sender == "Agent #2");
    }
}
