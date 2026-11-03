//! Sakura `.mac` 宏: 解析 / 规划交给 `editcore::macro_sakura::plan` (和 Tauri 版同一套), 这里把每一步动作执行在 `Doc` 上。
//! 能直接对应到 editcore 编辑命令的 `S_` 函数照常执行; 没有对应命令的 (只有默认键的) 明确报「未支持」, 不静默跳过。

use crate::doc::Doc;
use crate::find::FindOpts;
use editcore::macro_sakura::{self, Action};

/// 执行一份宏文本; 返回执行了多少步。语法错误 / 未知函数整体失败, 文档不会被改一半。
pub fn run_mac(doc: &mut Doc, src: &str) -> Result<usize, String> {
    let steps = macro_sakura::plan(src).map_err(|e| e.to_string())?;
    let n = steps.len();
    for s in steps {
        match s.action {
            Action::Command { id, cmd, key, .. } => {
                if let Some(c) = cmd {
                    if !doc.try_edit(&c) {
                        return Err(format!("第 {} 行 {}: 未支持的命令 {c}", s.line, s.name));
                    }
                } else if !key.as_deref().is_some_and(|k| key_action(doc, k)) {
                    return Err(format!("第 {} 行 {}: 这条命令高速版还不支持 ({id})", s.line, s.name));
                }
            }
            Action::InsText { text } => doc.type_text(&text),
            Action::Char { code } => {
                if code == 13 { doc.edit("new-line", None); } else if let Some(c) = char::from_u32(code) { doc.type_text(&c.to_string()); }
            }
            Action::MoveCursor { row, col, select } | Action::MoveCursorLayout { row, col, select } => doc.move_to(row, col, select),
            Action::Jump { line, .. } => doc.goto((line.max(1) as usize) - 1, 0),
            Action::SearchNext { text, flags } => { let o = FindOpts::from_flags(&macro_sakura::SearchFlags::from_bits(flags)); doc.find_step(&text, o, true).map_err(|e| e.to_string())?; }
            Action::SearchPrev { text, flags } => { let o = FindOpts::from_flags(&macro_sakura::SearchFlags::from_bits(flags)); doc.find_step(&text, o, false).map_err(|e| e.to_string())?; }
            Action::ReplaceAll { search, replace, flags } => {
                let f = macro_sakura::SearchFlags::from_bits(flags);
                doc.replace_all(&search, &replace, FindOpts::from_flags(&f), f.in_selection).map_err(|e| e.to_string())?;
            }
            Action::ReplaceNext { search, replace, flags } => {
                // 先找到下一处, 再按 "置换" 的语义换掉它 (正则下 $1 \1 $& 会展开) 并停在再下一处
                let o = FindOpts::from_flags(&macro_sakura::SearchFlags::from_bits(flags));
                if doc.find_step(&search, o, true).map_err(|e| e.to_string())? { doc.replace_one(&search, &replace, o).map_err(|e| e.to_string())?; }
            }
        }
    }
    Ok(n)
}

/// 只有默认键、没有命令名的 `S_` 函数: 按它的默认键 (DOM 写法, 如 `Control+Shift+ArrowLeft`) 在高速版里执行。
/// 移动键走 `nav::step`, 几个常用编辑键走 editcore 命令; 其它返回 false (调用方报「不支持」)。
fn key_action(doc: &mut Doc, key: &str) -> bool {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut base = "";
    for part in key.split('+') {
        match part {
            "Control" | "Ctrl" => ctrl = true,
            "Shift" => shift = true,
            "Alt" => alt = true,
            other => base = other,
        }
    }
    if alt {
        return false; // Alt+ 组合是矩形选择等, 宏里暂不支持
    }
    match base {
        "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp" | "PageDown" => {
            doc.nav(base, ctrl, shift, 20);
            true
        }
        "Backspace" => { doc.edit(if ctrl { "delete-word-start" } else { "backspace" }, None); true }
        "Delete" => { doc.edit(if ctrl { "delete-word-end" } else { "delete" }, None); true }
        "Enter" if !ctrl => { doc.edit("new-line", None); true }
        "i" if ctrl && !shift => { doc.edit("duplicate-line", None); true }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_sakura_macro_on_a_doc() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["foo bar".into(), "second".into()];
        let n = run_mac(&mut d, "S_InsText(\"X\");\nS_DuplicateLine();\nS_ReplaceAll(\"bar\", \"BAZ\", 0);").unwrap();
        assert_eq!(n, 3);
        assert_eq!(d.lines, vec!["Xfoo BAZ", "Xfoo BAZ", "second"], "复制行之后「全部置换」两行都换");
    }

    #[test]
    fn macro_search_flags_reach_the_find_engine() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["foo foobar foo".into()];
        run_mac(&mut d, "S_ReplaceAll(\"foo\", \"X\", 1);").unwrap(); // 0x01 = 单词单位
        assert_eq!(d.lines, vec!["X foobar X"]);
        let mut d2 = Doc::untitled(2, 1);
        d2.lines = vec!["k=v k=v".into()];
        run_mac(&mut d2, "S_ReplaceNext(\"([a-z])=([a-z])\", \"$2=$1\", 4);").unwrap(); // 0x04 = 正则: $1 $2 要展开
        assert_eq!(d2.lines, vec!["v=k k=v"]);
    }

    #[test]
    fn bad_macro_fails_before_changing_anything() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["keep".into()];
        assert!(run_mac(&mut d, "S_InsText(\"x\");\nS_NoSuchFunction();").is_err());
        assert_eq!(d.lines, vec!["keep"]);
    }
}
