//! 真实 GUI 自动化测试驱动 (`--e2e-script <plan.json>`)
//!
//! 仅在通过命令行显式传入 `--e2e-script <path>` 时挂载；常规启动时 `App::e2e_driver` 为 `None`，
//! 对冷启动速度与正常渲染循环保持零开销。

use super::installer_ui::InstallerLaunchMode;
use super::sysopts::BottomTab;
use super::App;
use eframe::egui;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 单个 GUI 自动化步骤描述
#[derive(Clone, Debug)]
pub struct E2eStep {
    pub id: String,
    pub name: String,
    pub op: String,
    pub path: Option<String>,
    pub text: Option<String>,
    pub start: Option<usize>,
    pub remove: Option<usize>,
    pub insert: Option<Vec<String>>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub count: Option<usize>,
    pub script: Option<String>,
    pub expect: Option<String>,
}

impl E2eStep {
    fn from_value(v: &Value, index: usize) -> Result<Self, String> {
        let op = v["op"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("steps[{index}] 缺少非空 op 字段"))?
            .to_string();
        let id = v["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("STEP-{:02}", index + 1));
        let name = v["name"].as_str().unwrap_or(&op).to_string();
        let insert = v.get("insert").and_then(Value::as_array).map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        });
        Ok(Self {
            id,
            name,
            op,
            path: v["path"].as_str().map(String::from),
            text: v["text"].as_str().map(String::from),
            start: v["start"].as_u64().map(|n| n as usize),
            remove: v["remove"].as_u64().map(|n| n as usize),
            insert,
            title: v["title"].as_str().map(String::from),
            content: v["content"].as_str().map(String::from),
            count: v["count"].as_u64().map(|n| n as usize),
            script: v["script"].as_str().map(String::from),
            expect: v["expect"].as_str().map(String::from),
        })
    }
}

/// 单步执行结果记录（写入 JSON 报告）
#[derive(Clone, Debug)]
pub struct E2eStepRecord {
    pub id: String,
    pub name: String,
    pub op: String,
    pub status: String,
    pub detail: String,
    pub error: String,
}

impl E2eStepRecord {
    fn to_json(&self) -> Value {
        json!({
            "Id": self.id,
            "Name": self.name,
            "Op": self.op,
            "Status": self.status,
            "Detail": self.detail,
            "Error": self.error,
        })
    }
}

/// 真实 GUI 逐帧步进驱动器
pub struct E2eDriver {
    pub suite: String,
    pub workspace_root: PathBuf,
    pub report_path: PathBuf,
    pub warmup_frames: u32,
    pub step_delay_ms: u64,
    pub last_step_at: Option<Instant>,
    pub frame_count: u32,
    pub steps: Vec<E2eStep>,
    pub cursor: usize,
    pub results: Vec<E2eStepRecord>,
    pub done: bool,
}

impl E2eDriver {
    /// 从磁盘 JSON 加载并解析路径基准
    pub fn load(plan_path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(plan_path)
            .map_err(|e| format!("无法读取 E2E 脚本 {}: {e}", plan_path.display()))?;
        let plan: Value = serde_json::from_str(&raw)
            .map_err(|e| format!("解析 E2E 脚本失败 {}: {e}", plan_path.display()))?;

        let suite = plan["suite"]
            .as_str()
            .unwrap_or("IDE1.0 Live GUI E2E")
            .to_string();
        let ws_str = plan["workspace_root"].as_str().unwrap_or(".");
        let rep_str = plan["report_path"]
            .as_str()
            .unwrap_or("qa/live-gui-e2e-report.json");
        let warmup_frames = plan["warmup_frames"].as_u64().unwrap_or(2) as u32;
        let step_delay_ms = plan["step_delay_ms"].as_u64().unwrap_or(0);

        let raw_steps = plan["steps"]
            .as_array()
            .ok_or_else(|| "E2E 脚本缺少 steps 数组".to_string())?;
        let mut steps = Vec::with_capacity(raw_steps.len());
        for (idx, item) in raw_steps.iter().enumerate() {
            steps.push(E2eStep::from_value(item, idx)?);
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let plan_dir = plan_path.parent().unwrap_or(&cwd);
        let ws_raw = PathBuf::from(ws_str);
        let workspace_root = if ws_raw.is_absolute() {
            ws_raw
        } else if cwd.join(&ws_raw).exists() {
            cwd.join(&ws_raw)
        } else {
            plan_dir.join(&ws_raw)
        };

        let rep_raw = PathBuf::from(rep_str);
        let report_path = if rep_raw.is_absolute() {
            rep_raw
        } else {
            cwd.join(rep_raw)
        };

        Ok(Self {
            suite,
            workspace_root,
            report_path,
            warmup_frames,
            step_delay_ms,
            last_step_at: None,
            frame_count: 0,
            steps,
            cursor: 0,
            results: Vec::new(),
            done: false,
        })
    }

    fn resolve_path(&self, rel: &str) -> PathBuf {
        let p = PathBuf::from(rel);
        if p.is_absolute() {
            p
        } else {
            self.workspace_root.join(p)
        }
    }

    fn write_report(&self, finished: bool) {
        if let Some(parent) = self.report_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let passed_tests = self.results.iter().filter(|r| r.status == "Passed").count();
        let failed_tests = self.results.iter().filter(|r| r.status == "Failed").count();
        let report = json!({
            "Suite": self.suite,
            "TotalTests": self.results.len(),
            "PassedTests": passed_tests,
            "FailedTests": failed_tests,
            "Finished": finished,
            "Results": self.results.iter().map(E2eStepRecord::to_json).collect::<Vec<_>>(),
        });
        if let Ok(json) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(&self.report_path, json);
        }
    }
}

impl App {
    /// 挂载真实 GUI 自动化驱动，并隔离磁盘持久化防止污染用户日常配置
    pub fn attach_e2e_driver(&mut self, driver: E2eDriver) {
        self.persist = false;
        self.sticky_notes.persist = false;
        self.e2e_driver = Some(driver);
    }

    /// 每帧步进执行一条真实 GUI E2E 动作（未启用 `--e2e-script` 时零开销早退）
    pub(super) fn e2e_driver_tick(&mut self, ctx: &egui::Context) {
        let Some(mut driver) = self.e2e_driver.take() else {
            return;
        };
        if driver.done {
            self.e2e_driver = Some(driver);
            return;
        }
        driver.frame_count = driver.frame_count.saturating_add(1);
        if driver.frame_count == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        if driver.frame_count <= driver.warmup_frames {
            ctx.request_repaint();
            self.e2e_driver = Some(driver);
            return;
        }

        if driver.step_delay_ms > 0 {
            if let Some(prev) = driver.last_step_at {
                let target = Duration::from_millis(driver.step_delay_ms);
                let elapsed = prev.elapsed();
                if elapsed < target {
                    ctx.request_repaint_after(target - elapsed);
                    self.e2e_driver = Some(driver);
                    return;
                }
            }
        }
        ctx.request_repaint();

        if driver.cursor >= driver.steps.len() {
            driver.done = true;
            driver.write_report(true);
            self.force_exit = true;
            self.confirm_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            self.e2e_driver = Some(driver);
            return;
        }

        let step = driver.steps[driver.cursor].clone();
        driver.cursor += 1;
        driver.last_step_at = Some(Instant::now());

        let mut close_requested = false;
        let outcome = self.execute_e2e_step(&driver, &step, &mut close_requested);
        let (status, detail, error) = match outcome {
            Ok(msg) => ("Passed".to_string(), msg, String::new()),
            Err(err) => ("Failed".to_string(), String::new(), err),
        };
        driver.results.push(E2eStepRecord {
            id: step.id,
            name: step.name,
            op: step.op,
            status,
            detail,
            error,
        });

        let finished = close_requested || driver.cursor >= driver.steps.len();
        if finished {
            driver.done = true;
        }
        driver.write_report(finished);
        if finished {
            self.force_exit = true;
            self.confirm_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.e2e_driver = Some(driver);
    }

    fn active_doc_text(&self) -> Result<String, String> {
        let d = self
            .docs
            .get(self.active)
            .ok_or_else(|| "当前没有活跃文档".to_string())?;
        Ok(d.lines.join("\n"))
    }

    fn execute_e2e_step(
        &mut self,
        driver: &E2eDriver,
        step: &E2eStep,
        close_requested: &mut bool,
    ) -> Result<String, String> {
        match step.op.as_str() {
            "open_file" => {
                let rel = step
                    .path
                    .as_deref()
                    .ok_or_else(|| "open_file 缺少 path 参数".to_string())?;
                let abs = driver.resolve_path(rel);
                if !abs.exists() {
                    return Err(format!("目标文件不存在: {}", abs.display()));
                }
                self.open_path_no_save(&abs);
                let active_path = self
                    .docs
                    .get(self.active)
                    .and_then(|d| d.path.as_ref())
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let fname = abs.file_name().and_then(|s| s.to_str()).unwrap_or(rel);
                if !active_path.contains(fname) {
                    return Err(format!("活跃文档未切换至 {fname}，当前: {active_path}"));
                }
                Ok(format!("已打开并聚焦文件: {active_path}"))
            }
            "assert_doc_contains" => {
                let needle = step
                    .text
                    .as_deref()
                    .ok_or_else(|| "assert_doc_contains 缺少 text 参数".to_string())?;
                let body = self.active_doc_text()?;
                if !body.contains(needle) {
                    return Err(format!("活跃文档不包含期望文本: {needle}"));
                }
                Ok(format!("断言命中文本: {needle}"))
            }
            "assert_doc_not_contains" => {
                let needle = step
                    .text
                    .as_deref()
                    .ok_or_else(|| "assert_doc_not_contains 缺少 text 参数".to_string())?;
                let body = self.active_doc_text()?;
                if body.contains(needle) {
                    return Err(format!("活跃文档意外包含禁用文本: {needle}"));
                }
                Ok(format!("断言确认不包含: {needle}"))
            }
            "patch_active_doc" => {
                let start = step.start.unwrap_or(0);
                let remove = step.remove.unwrap_or(0);
                let insert = step.insert.clone().unwrap_or_default();
                let doc = self
                    .docs
                    .get_mut(self.active)
                    .ok_or_else(|| "patch_active_doc 时无活跃文档".to_string())?;
                doc.apply_line_patch(start, remove, insert.clone(), None)?;
                Ok(format!(
                    "已对活跃文档应用行补丁 (start={start}, remove={remove}, insert={}行)",
                    insert.len()
                ))
            }
            "undo_active_doc" => {
                let doc = self
                    .docs
                    .get_mut(self.active)
                    .ok_or_else(|| "undo_active_doc 时无活跃文档".to_string())?;
                doc.undo();
                Ok("已执行活跃文档撤销".into())
            }
            "open_sticky_notes" => {
                self.sticky_notes.persist = false;
                let title = step.title.as_deref().unwrap_or("E2E 实时便笺");
                let content = step.content.as_deref().unwrap_or("# E2E\n实测创建成功");
                let res = self.sticky_notes.exec_tool(
                    "sticky_note_create",
                    &json!({ "title": title, "content": content }),
                );
                if let Some(err) = res["error"].as_str() {
                    return Err(format!("创建桌面便笺失败: {err}"));
                }
                if !self.sticky_notes.open {
                    return Err("sticky_notes.open 应为 true".into());
                }
                Ok(format!("已打开桌面便笺并创建笔记: {title}"))
            }
            "open_ai_collab" => {
                self.ai.open = true;
                self.ai.collab_mode = true;
                if let Some(cnt) = step.count {
                    self.collab.set_agent_count(cnt);
                    if self.collab.agents.len() != cnt {
                        return Err(format!(
                            "协同编队数量不一致: expected={cnt}, actual={}",
                            self.collab.agents.len()
                        ));
                    }
                }
                Ok(format!(
                    "已打开多 Agent 协同面板 (agents={})",
                    self.collab.agents.len()
                ))
            }
            "open_rust_console" => {
                self.term.open = true;
                self.sys.bottom = BottomTab::RustConsole;
                let script = step.script.as_deref().unwrap_or("ide::context::status()");
                let out = self.eval_rust_console_str(script);
                if let Some(expect) = step.expect.as_deref() {
                    if !out.contains(expect) {
                        return Err(format!(
                            "Rust 控制台输出未匹配预期 `{expect}`，实际输出: {out}"
                        ));
                    }
                }
                Ok(format!("Rust 控制台执行 `{script}` 成功: {out}"))
            }
            "open_installer_and_maximize" => {
                self.open_installer_wizard(InstallerLaunchMode::Auto, false);
                self.installer.maximized = true;
                if !self.installer.open || !self.installer.maximized {
                    return Err("安装向导未能打开并切换到全屏最大化状态".into());
                }
                Ok("安装向导已打开并进入最大化状态".into())
            }
            "verify_fullscreen_edge_flush" => {
                let dummy_ctx = egui::Context::default();
                let prev_full = self.fullscreen;
                self.fullscreen = true;
                let inset_full = self.maximized_inset(&dummy_ctx);
                self.fullscreen = false;
                let inset_norm = self.maximized_inset(&dummy_ctx);
                self.fullscreen = prev_full;
                if inset_full != 0.0 || inset_norm != 0.0 {
                    return Err(format!(
                        "全屏/最大化四周内边距不为 0: fullscreen={inset_full}, normal={inset_norm}"
                    ));
                }
                if super::titlebar::MAX_OVERSHOOT != 0.0 {
                    return Err(format!(
                        "MAX_OVERSHOOT 应为 0.0，实际为 {}",
                        super::titlebar::MAX_OVERSHOOT
                    ));
                }
                Ok("全屏与最大化模式四周 0px 贴边无内边距校验通过".into())
            }
            "verify_markdown_bold_render" => {
                let doc = self
                    .docs
                    .get(self.active)
                    .ok_or_else(|| "verify_markdown_bold_render 时无活跃文档".to_string())?;
                let full = doc.lines.join("\n");
                let spans = crate::markdown::inline(&full);
                let bold_count = spans.iter().filter(|s| s.bold).count();
                if bold_count < 2 {
                    return Err(format!(
                        "Markdown 文档应解析出至少 2 处加粗 Span (**bold** / __bold__)，实际={bold_count}"
                    ));
                }
                let bold_fam = crate::fonts::editor_bold_family();
                if matches!(bold_fam, egui::FontFamily::Proportional) {
                    return Err("editor_bold_family 不应退化为未具名的 Proportional".into());
                }
                Ok(format!(
                    "Markdown 加粗渲染校验通过 (bold_spans={bold_count}, family={bold_fam:?})"
                ))
            }
            "verify_grep_search" => {
                let query = step.text.as_deref().unwrap_or("EditorKernel");
                let search_root = driver.workspace_root.clone();
                self.side.open = false;
                self.cfg.view.side_collapsed = true;
                self.focus_grep();
                if !self.side.open || self.cfg.view.side_collapsed || !self.side.grep.focus_pattern
                {
                    return Err(
                        "focus_grep 在左侧栏折叠时未自动展开 side_collapsed 或未设置搜索框焦点"
                            .into(),
                    );
                }
                self.side.grep.o.pattern = query.to_string();
                self.side.grep.o.folders = search_root.display().to_string();
                self.grep_start();
                let cur_doc = &self.docs[self.active];
                if !cur_doc.grep_result || cur_doc.name != "Grep 结果" {
                    return Err(format!(
                        "grep_start 未默认在主编辑区打开并激活 `Grep 结果` 标签页: active_name={}",
                        cur_doc.name
                    ));
                }
                let mut opts = crate::grep::GrepOpts::default();
                opts.pattern = query.to_string();
                opts.folders = search_root.display().to_string();
                let shards = crate::grep::plan(&search_root, &opts)?;
                let mut total_hits = 0usize;
                let mut sample_hit: Option<(std::path::PathBuf, usize, usize)> = None;
                for (shard_root, shard_opts) in &shards {
                    let res = editcore::grep_engine::search(shard_root, shard_opts)
                        .map_err(|e| format!("Grep 检索失败: {e}"))?;
                    if sample_hit.is_none() {
                        if let Some(m) = res.first() {
                            let hit_line = format!("{}({},1): {}", m.path, m.line, m.text);
                            sample_hit = crate::grep::parse_result_line(&hit_line);
                        }
                    }
                    total_hits += res.len();
                }
                if total_hits == 0 {
                    return Err(format!(
                        "Grep 检索 `{query}` 在 `{}` 下未命中任何结果",
                        search_root.display()
                    ));
                }
                if sample_hit.is_none() {
                    return Err("Grep 结果行双击跳转解析 parse_result_line 返回 None".into());
                }
                Ok(format!(
                    "Grep 检索校验通过 (`{query}` 命中 {total_hits} 处, 折叠侧栏自动展开、主编辑区 `Grep 结果` 标签页默认打开与双击跳转解析均正常)"
                ))
            }
            "verify_agreement_and_hooks" => {
                let agreement = super::installer_ui::USER_AGREEMENT_TEXT;
                if !agreement.contains("默认适用区域：中华人民共和国（中国大陆地区）")
                    || !agreement.contains("中华人民共和国法律")
                {
                    return Err("用户协议默认适用区域未更新为中华人民共和国（中国大陆地区）".into());
                }
                let rt = self.ai.plugin_rt_mut();
                let pre = rt.run_pre_input_hook("请检查当前代码".to_string())?;
                if pre.is_empty() {
                    return Err("Hooks Pre-Input 生命周期钩子返回空文本".into());
                }
                let gate = rt.check_tool_gate("write_file", &json!({ "path": "../secret.env" }));
                if gate.is_ok() {
                    return Err("Hooks path-sanitizer 未拦截 `../` 越界路径写入".into());
                }
                let masked = rt.run_post_response_hook(
                    "token=sk-abcdefghijklmnopqrstuvwxyz123456".to_string(),
                );
                if !masked.contains("REDACTED") && !masked.contains("***") {
                    return Err(format!(
                        "Hooks Post-Response secret-mask 未对明文 sk- 密钥脱敏: {masked}"
                    ));
                }
                Ok("中国默认区域用户协议 v1.1 与 Hooks 插件全生命周期钩子校验通过".into())
            }
            "verify_incremental_search" => {
                let needle = step.text.as_deref().unwrap_or("Ready");
                let doc = self
                    .docs
                    .get_mut(self.active)
                    .ok_or_else(|| "verify_incremental_search 时无活跃文档".to_string())?;
                doc.goto(0, 0);
                self.find.open_isearch(doc, true, false, false);
                for ch in needle.chars() {
                    self.find.needle.push(ch);
                    self.find.changed(doc);
                }
                let first_sel = doc
                    .sel
                    .ok_or_else(|| format!("正向增量查找 `{needle}` 未能选中命中项"))?;
                self.find.step(doc, true);
                let second_sel = doc
                    .sel
                    .ok_or_else(|| format!("增量查找向下步进 `{needle}` 后选区丢失"))?;
                if second_sel == first_sel {
                    return Err(format!("增量查找向下步进未前进到下一处命中: {first_sel:?}"));
                }
                self.find.step(doc, false);
                if doc.sel != Some(first_sel) {
                    return Err(format!(
                        "增量查找向上步进未回到第一处命中: expected={first_sel:?}, actual={:?}",
                        doc.sel
                    ));
                }
                self.find.needle.clear();
                self.find.changed(doc);
                if doc.cursor != [0, 0] || doc.sel.is_some() {
                    return Err(format!(
                        "清空增量查找关键字后光标未恢复到起点 [0,0]: cursor={:?}, sel={:?}",
                        doc.cursor, doc.sel
                    ));
                }
                self.find.open = false;
                self.find.isearch = None;
                Ok(format!(
                    "增量查找校验通过 (`{needle}` 正反向步进与清空回退全部准确)"
                ))
            }
            "verify_md_alt_box_selection" => {
                let doc = self
                    .docs
                    .get_mut(self.active)
                    .ok_or_else(|| "verify_md_alt_box_selection 时无活跃文档".to_string())?;
                // 选取中英混排的 Markdown 表格行，验证按视觉显示列 [13, 18) 对齐截取 "Ready"
                let r0 = doc
                    .lines
                    .iter()
                    .position(|l| l.contains("| 核心内核 |") && l.contains("Ready"))
                    .ok_or_else(|| {
                        "未找到中英混排 Markdown 表格测试行 `| 核心内核 |`".to_string()
                    })?;
                let r1 = r0 + 1;
                doc.box_start(r0, 13);
                doc.box_drag(r1, 18);
                let copied = doc
                    .edit("box-copy", None)
                    .ok_or_else(|| "Markdown Alt 矩形列选复制返回 None".to_string())?;
                if copied != "Ready\nReady\n" {
                    return Err(format!(
                        "Markdown Alt 矩形列选在中英混排行发生错位: expected=\"Ready\\nReady\\n\", actual={copied:?}"
                    ));
                }
                doc.box_start(r0, 13);
                doc.box_drag(r1, 18);
                doc.edit("box-insert", Some("PASS!".into()));
                if !doc.lines[r0].contains("| 核心内核 | PASS! |")
                    || !doc.lines[r1].contains("| editcore | PASS! |")
                {
                    return Err(format!(
                        "Markdown Alt 矩形列选替换后未对齐: row0={}, row1={}",
                        doc.lines[r0], doc.lines[r1]
                    ));
                }
                doc.undo();
                doc.box_sel = None;
                Ok("Markdown 文档中英混排 Alt 矩形列选对齐与编辑校验通过".into())
            }
            "close_app" => {
                *close_requested = true;
                Ok("触发主窗口优雅退出".into())
            }
            other => Err(format!("未知的 E2E 操作类型: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Settings, Trace};

    #[test]
    fn e2e_driver_executes_plan_steps_and_writes_report() {
        let g = crate::i18n::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide_e2e_drv_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let src = dir.join("sample.rs");
        std::fs::write(&src, "fn alpha() -> i32 {\n    1\n}\n").unwrap();
        let md = dir.join("preview.md");
        std::fs::write(
            &md,
            "# Title\n- **星号加粗** 与 __双下划线加粗__\n| 核心内核 | Ready |\n| editcore | Ready |\n",
        )
        .unwrap();
        let report_path = dir.join("report.json");
        let plan_path = dir.join("plan.json");
        std::fs::write(
            &plan_path,
            serde_json::to_string_pretty(&json!({
                "suite": "Driver Unit Plan",
                "workspace_root": dir.display().to_string(),
                "report_path": report_path.display().to_string(),
                "warmup_frames": 0,
                "steps": [
                    { "id": "S1", "name": "Open file", "op": "open_file", "path": "sample.rs" },
                    { "id": "S2", "name": "Assert original", "op": "assert_doc_contains", "text": "fn alpha()" },
                    { "id": "S3", "name": "Patch line", "op": "patch_active_doc", "start": 1, "remove": 1, "insert": ["    99"] },
                    { "id": "S4", "name": "Assert patched", "op": "assert_doc_contains", "text": "99" },
                    { "id": "S5", "name": "Undo patch", "op": "undo_active_doc" },
                    { "id": "S6", "name": "Assert restored", "op": "assert_doc_not_contains", "text": "99" },
                    { "id": "S7", "name": "Sticky note", "op": "open_sticky_notes", "title": "UnitNote", "content": "ok" },
                    { "id": "S8", "name": "Collab panel", "op": "open_ai_collab", "count": 4 },
                    { "id": "S9", "name": "Rust console", "op": "open_rust_console", "script": "ide::ops::collab_count(5)", "expect": "5" },
                    { "id": "S10", "name": "Installer maximize", "op": "open_installer_and_maximize" },
                    { "id": "S11", "name": "Fullscreen flush", "op": "verify_fullscreen_edge_flush" },
                    { "id": "S12", "name": "Open markdown", "op": "open_file", "path": "preview.md" },
                    { "id": "S13", "name": "Markdown bold", "op": "verify_markdown_bold_render" },
                    { "id": "S14", "name": "Incremental search", "op": "verify_incremental_search", "text": "Ready" },
                    { "id": "S15", "name": "Markdown Alt box sel", "op": "verify_md_alt_box_selection" },
                    { "id": "S16", "name": "Grep search", "op": "verify_grep_search", "text": "alpha" },
                    { "id": "S17", "name": "Agreement & Hooks", "op": "verify_agreement_and_hooks" },
                    { "id": "S18", "name": "Close", "op": "close_app" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let driver = E2eDriver::load(&plan_path).unwrap();
        let mut app = App::build(vec![], None, Trace::new(), Settings::default(), false);
        app.attach_e2e_driver(driver);
        let ctx = egui::Context::default();
        for _ in 0..22 {
            app.e2e_driver_tick(&ctx);
        }

        let drv = app.e2e_driver.as_ref().unwrap();
        assert!(drv.done);
        assert_eq!(drv.results.len(), 18);
        assert!(
            drv.results.iter().all(|r| r.status == "Passed"),
            "{:?}",
            drv.results
        );
        let rep_raw = std::fs::read_to_string(&report_path).unwrap();
        let rep: Value = serde_json::from_str(&rep_raw).unwrap();
        assert_eq!(rep["PassedTests"], 18);
        assert_eq!(rep["FailedTests"], 0);
        assert_eq!(rep["Finished"], true);

        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
