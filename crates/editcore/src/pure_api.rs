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
