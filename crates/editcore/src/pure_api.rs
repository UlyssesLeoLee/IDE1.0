//! 无状态端点的"类型化入口": 请求结构体 -> JSON 响应 (或给用户看的错误文字)。
//!
//! web 壳 (HTTP `/api/*`) 和桌面壳 (Tauri 命令) 做的是同一件事: 反序列化请求、调 editcore、序列化响应、
//! 把"未知命令"翻成同一句错误。这些规则只在这里写一遍, 两个壳各自只负责传输 (HTTP 状态码 / Tauri 的 Result)。
//!
//! 约定: 返回 `Err(消息)` = 请求本身有问题 (HTTP 400); 对括弧 / 光标移动"没有结果"不是错误, 返回 `{}`。

use crate::{bookmark, bracket, edit_cmd, history, nav, search_ops};
use serde::Serialize;
use serde_json::{json, Value};

fn to_json<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or_default()
}

/// 编辑命令 (删除 / 剪切 / 复制 / 粘贴 / 键入 / 矩形 / 多光标 / 折叠)
pub fn edit_cmd(req: &edit_cmd::EditReq) -> Result<Value, String> {
    edit_cmd::run(req).map(|r| to_json(&r)).ok_or_else(|| "未知的编辑命令".to_string())
}

/// 对括弧; 没有对应括弧 = `{}`
pub fn bracket_pair(req: &bracket::BracketReq) -> Value {
    bracket::find_pair(req).map(|r| to_json(&r)).unwrap_or_else(|| json!({}))
}

/// ブックマーク 切り替え / 次へ / 前へ
pub fn bookmark(req: &bookmark::BmReq) -> Result<Value, String> {
    bookmark::run(req).map(|r| to_json(&r)).ok_or_else(|| "未知的书签操作".to_string())
}

/// 文档内查找 / 置換
pub fn search_text(req: &search_ops::SearchReq) -> Result<Value, String> {
    search_ops::run(req).map(|r| to_json(&r)).ok_or_else(|| "未知的查找操作".to_string())
}

/// 撤销 / 重做栈 (按标签页 id 分开的进程内登记)
pub fn history(req: &history::HistReq) -> Result<Value, String> {
    history::run(req).map(|r| to_json(&r)).ok_or_else(|| "未知的历史操作".to_string())
}

/// 键盘光标移动 / 范围选择; 不处理的键 = `{}`
pub fn nav_step(req: &nav::NavReq) -> Value {
    nav::step(req).map(|r| to_json(&r)).unwrap_or_else(|| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_commands_are_user_facing_errors() {
        let e = edit_cmd(&edit_cmd::EditReq { cmd: "nope".into(), ..Default::default() }).unwrap_err();
        assert_eq!(e, "未知的编辑命令");
    }

    #[test]
    fn no_result_is_an_empty_object_not_an_error() {
        let r = nav_step(&nav::NavReq { key: "Escape".into(), ..Default::default() });
        assert_eq!(r, json!({}));
    }

    #[test]
    fn known_commands_serialise_the_response() {
        let req = edit_cmd::EditReq { cmd: "backspace".into(), row: 0, col: 1, total_rows: 1, rows: vec![(0, "ab".into())], ..Default::default() };
        let v = edit_cmd(&req).unwrap();
        assert_eq!(v["insert"][0], "b");
        assert_eq!(v["changed"], true);
    }
}

/// Markdown 预览: 文本 -> 已转义的 HTML (只读, 不写回)
pub fn md_render(req: &MdRenderReq) -> Value {
    json!({ "html": crate::md_render::render(&req.text) })
}

#[derive(serde::Deserialize)]
pub struct MdRenderReq {
    pub text: String,
}

// ---------- 插件 (docs/01_基本设计/PLUGIN-SYSTEM.md) ----------

/// 插件操作。网络类 (inspect / install / check_updates) 只在用户点按钮时才会发来, 没有后台轮询
#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PluginReq {
    List,
    /// 读插件文件 (入口脚本等), 文本
    Asset { id: String, file: String },
    Toggle { id: String, enabled: bool },
    Remove { id: String },
    Inspect { repo: String },
    Install { repo: String, approved: Vec<String> },
    CheckUpdates,
    /// 插件与内建功能同一能力位冲突时选谁 (docs §8.1)
    Resolve { has_native: bool, has_plugin: bool, prefer: Option<crate::plugin_host::Prefer>, smart_active: bool },
    /// 不匹配时给大模型的提示词 (只含事实, 不含 Key)
    MismatchPrompt { repo: String },
}

pub fn plugins(req: &PluginReq) -> Result<Value, String> {
    use crate::plugin_host::{self as ph, store};
    let root = store::default_root();
    let app = env!("CARGO_PKG_VERSION");
    Ok(match req {
        PluginReq::List => json!({ "plugins": store::list(&root), "host_api": ph::HOST_API_VERSION, "app_version": app }),
        PluginReq::Asset { id, file } => {
            let b = store::read_asset(&root, id, file)?;
            json!({ "text": String::from_utf8(b).map_err(|_| "插件文件不是 UTF-8 文本".to_string())? })
        }
        PluginReq::Toggle { id, enabled } => {
            store::set_enabled(&root, id, *enabled)?;
            json!({ "ok": true })
        }
        PluginReq::Remove { id } => {
            store::remove(&root, id)?;
            json!({ "ok": true })
        }
        PluginReq::Inspect { repo } => to_json(&store::inspect(&store::CurlHttp, &root, repo, app)?),
        PluginReq::Install { repo, approved } => to_json(&store::install(&store::CurlHttp, &root, repo, app, approved)?),
        PluginReq::CheckUpdates => {
            let rows: Vec<Value> = store::check_updates(&store::CurlHttp, &root, app)
                .into_iter()
                .map(|(id, r)| match r {
                    Ok(i) => json!({ "id": id, "update_available": i.update_available, "latest": i.report.manifest.map(|m| m.version), "ok": i.report.ok, "problems": i.report.problems }),
                    Err(e) => json!({ "id": id, "error": e }),
                })
                .collect();
            json!({ "updates": rows })
        }
        PluginReq::Resolve { has_native, has_plugin, prefer, smart_active } => json!({ "choice": ph::resolve_conflict(*has_native, *has_plugin, *prefer, *smart_active) }),
        PluginReq::MismatchPrompt { repo } => {
            let ins = store::inspect(&store::CurlHttp, &root, repo, app)?;
            json!({ "prompt": ph::mismatch_prompt(&ins.release.repo, &ins.release.tag, &ins.release.assets.iter().map(|a| a.name.clone()).collect::<Vec<_>>(), &ins.report, app) })
        }
    })
}

// ---------- Agent API (给内建对话 / 外部 LangGraph, 见 agent_api) ----------

#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AgentReq {
    /// 工具表 (OpenAI tools 格式)
    Tools,
    Call { name: String, #[serde(default)] args: Value },
}

pub fn agent(req: &AgentReq) -> Result<Value, String> {
    match req {
        AgentReq::Tools => Ok(json!({ "tools": crate::agent_api::tools_json() })),
        AgentReq::Call { name, args } => crate::agent_api::call(name, args),
    }
}
