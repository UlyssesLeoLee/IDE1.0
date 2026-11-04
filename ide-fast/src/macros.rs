//! Sakura `.mac` 宏: 解析 / 规划交给 `editcore::macro_sakura::plan` (和 Tauri 版同一套), 这里把每一步动作执行在 `Doc` 上。
//! 能直接对应到 editcore 编辑命令的 `S_` 函数照常执行; 没有对应命令的 (只有默认键的) 明确报「未支持」, 不静默跳过。

use crate::doc::{Doc, RecEv};
use crate::find::FindOpts;
use editcore::macro_sakura::{self, Action};

/// 执行一份宏文本; 返回执行了多少步。语法错误 / 未知函数整体失败, 文档不会被改一半。
pub fn run_mac(doc: &mut Doc, src: &str, tab: usize) -> Result<usize, String> {
    let steps = macro_sakura::plan(src).map_err(|e| e.to_string())?;
    let n = steps.len();
    for s in steps {
        match s.action {
            Action::Command { id, cmd, key, .. } => {
                if let Some(c) = cmd {
                    if !doc_command(doc, &c, tab) {
                        return Err(format!("第 {} 行 {}: 未支持的命令 {c}", s.line, s.name));
                    }
                } else if !key.as_deref().is_some_and(|k| key_action(doc, k, tab)) {
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

/// 录出来的 `.mac` 要开成新文档时的文字: 一行注释头 + 宏语句; 返回 (文字, 语句条数)
pub fn mac_document(out: &macro_sakura::RecOut) -> (String, usize) {
    let n = out.text.lines().filter(|l| !l.trim().is_empty()).count();
    (format!("/* 键盘录制 */\n{}", out.text), n)
}

/// Sakura 默认键 <-> editcore 编辑命令: 取自 Tauri 版 `KEYMAP` 里属于编辑命令的那些键 (它们在 Sakura 的 `S_` 表里只有键、没有命令名)。
/// 运行 `.mac` 时按键找命令 (`key_action`), 录成 `.mac` 时按命令找键 (`rec_to_mac`), 两边共用这一张表, 所以录出来的宏能原样跑回去。
/// 单测保证: 每个键在 `SAKURA_COMMANDS` 里有对应的 `S_` 函数, 每个命令 editcore 认得。
const EDIT_KEYS: &[(&str, &str)] = &[
    ("Control+d", "word-cut"), ("Control+Shift+d", "word-delete"), ("Control+k", "line-cut-to-end"), ("Control+i", "duplicate-line"),
    ("Control+w", "select-word"), ("F7", "cut"), ("F8", "copy"), ("Shift+F8", "copy-crlf"), ("F9", "paste"),
];

/// 只在运行 `.mac` 时用的键: 选区变换 (大小写 / 全半角 / Tab 与空格), 高速版里走 `Doc::transform_sel` 而不是 editcore 编辑命令 (键盘宏不录它们, 所以没有反向查找)
const TRANSFORM_KEYS: &[(&str, &str)] = &[
    ("Control+F6", "lower"), ("Control+F7", "upper"), ("Control+F8", "to-halfwidth"), ("Control+F9", "to-zenkaku"), ("Control+F10", "to-zenhira"),
    ("Control+F11", "hankata-to-zenkata"), ("Control+F12", "hankata-to-zenhira"), ("Control+Alt+F5", "tab2space"), ("Control+Shift+Alt+F5", "space2tab"),
];

/// 同上, 整理行 (选中的行; 没有选区 = 全文): 排序 / 去行首、行尾空白
const LINES_KEYS: &[(&str, &str)] = &[("Alt+a", "sort-asc"), ("Alt+d", "sort-desc"), ("Alt+l", "trim-start"), ("Alt+r", "trim-end"), ("Alt+m", "uniq-consecutive")];

fn edit_for_key(key: &str) -> Option<&'static str> {
    EDIT_KEYS.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
}

fn key_for_edit(cmd: &str) -> Option<&'static str> {
    EDIT_KEYS.iter().find(|(_, c)| *c == cmd).map(|(k, _)| *k)
}

/// 宏里带命令 id 的 `S_` 函数: 只动这一个文档的在这里执行 —— editcore 编辑命令、`editcore::edit_ops` 的文本变换、SPACE 缩进、按段落选择、
/// 改动行 / 函数步进、插入日期时间、只读切换、换行符改写、编码转换。窗口 / 标签 / 对话框 / 剪贴板类命令不属于「作用在一个文档上的宏」,
/// 返回 false (调用方报「不支持」, 不静默跳过)。
fn doc_command(doc: &mut Doc, c: &str, tab: usize) -> bool {
    use editcore::document_io::Eol;
    match c {
        "indent-space" => doc.space_indent(false),
        "unindent-space" => doc.space_indent(true),
        "sel-para-next" => doc.select_para(true),
        "sel-para-prev" => doc.select_para(false),
        "modify-next" => { doc.step_changed(true); }
        "modify-prev" => { doc.step_changed(false); }
        "modify-next-sel" => { doc.step_changed_sel(true); }
        "modify-prev-sel" => { doc.step_changed_sel(false); }
        "funclist-next" => { doc.step_function(true); }
        "funclist-prev" => { doc.step_function(false); }
        "insert-time" => doc.type_text(&crate::textconv::local_now().time()),
        "insert-date" => doc.type_text(&crate::textconv::local_now().date()),
        "toggle-readonly" => doc.readonly = !doc.readonly,
        "eol-crlf" => doc.set_eol(Eol::Crlf),
        "eol-lf" => doc.set_eol(Eol::Lf),
        "eol-cr" => doc.set_eol(Eol::Cr),
        _ if c.starts_with("codecnv-") => { doc.convert_selection(|t| crate::textconv::codecnv(&c["codecnv-".len()..], t)); }
        _ if editcore::edit_ops::transform_text(c, "", tab).is_some() => doc.transform_sel(c, tab),
        _ => return doc.try_edit(c),
    }
    true
}

/// 只有默认键、没有命令名的 `S_` 函数: 按它的默认键 (DOM 写法, 如 `Control+Shift+ArrowLeft`) 在高速版里执行。
/// 编辑键查 `EDIT_KEYS`, 移动键走 `nav::step`, 退格 / 删除 / 回车走 editcore 命令; 其它返回 false (调用方报「不支持」)。
fn key_action(doc: &mut Doc, key: &str, tab: usize) -> bool {
    if let Some(cmd) = edit_for_key(key) {
        return doc.try_edit(cmd);
    }
    if let Some((_, kind)) = TRANSFORM_KEYS.iter().find(|(k, _)| *k == key) {
        doc.transform_sel(kind, tab);
        return true;
    }
    if let Some((_, kind)) = LINES_KEYS.iter().find(|(k, _)| *k == key) {
        doc.lines_op(kind);
        return true;
    }
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
        _ => false,
    }
}

/// 高速版录到的键盘宏 -> Sakura `.mac` 文本。生成器是 `editcore::macro_sakura::record_to_mac` (和 Tauri 版「宏录制 (.mac)」同一个):
/// 键入的文字合成 `S_InsText`, 编辑命令按命令名反查 `S_` 函数, 移动键按默认键反查; 找不到对应 `S_` 函数的步骤记进 `skipped`, 不写进宏里。
pub fn rec_to_mac(steps: &[RecEv]) -> macro_sakura::RecOut {
    use macro_sakura::RecEvent;
    let events: Vec<RecEvent> = steps
        .iter()
        .map(|s| match s {
            RecEv::Edit(c, Some(t)) if c == "insert-text" => RecEvent::Text { text: t.clone() },
            RecEv::Edit(c, _) if matches!(c.as_str(), "new-line" | "backspace" | "delete") => RecEvent::Edit { cmd: c.clone() },
            // 其它编辑命令: 先按命令名反查 S_ 函数, 反查不到再用它的 Sakura 默认键 (`EDIT_KEYS`)
            RecEv::Edit(c, _) => RecEvent::Command { cmd: c.clone(), key: key_for_edit(c).map(str::to_string) },
            RecEv::Nav(k, ctrl, shift) => {
                // 键名用表里的 DOM 写法 (`Control+Shift+ArrowLeft`); 同时当作「命令名」, 万一反查不到就原样出现在 skipped 里
                let key = format!("{}{}{k}", if *ctrl { "Control+" } else { "" }, if *shift { "Shift+" } else { "" });
                RecEvent::Command { cmd: key.clone(), key: Some(key) }
            }
        })
        .collect();
    macro_sakura::record_to_mac(&events)
}

/// 录下的键盘宏最多这么多步 / 每个命令名、键名最长这么多字符 / 插入文字最长这么多字符 (宏文件是不可信输入)
const MAX_STEPS: usize = 100_000;
const MAX_NAME: usize = 64;
const MAX_TEXT: usize = 200_000;

/// 键盘宏 -> JSON 文本: `{"version":1,"steps":[["e","命令","文字|null"],["n","键",ctrl,shift]]}`
pub fn macro_to_json(steps: &[RecEv]) -> String {
    let v: Vec<serde_json::Value> = steps.iter().map(|s| match s {
        RecEv::Edit(c, t) => serde_json::json!(["e", c, t]),
        RecEv::Nav(k, c, sh) => serde_json::json!(["n", k, c, sh]),
    }).collect();
    serde_json::to_string_pretty(&serde_json::json!({ "version": 1, "steps": v })).unwrap_or_default()
}

/// JSON 文本 -> 键盘宏; 格式不对 / 命令名不合法 / 超长都拒绝 (整份拒绝, 不执行一半)
pub fn macro_from_json(text: &str) -> Result<Vec<RecEv>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("不是有效的宏文件: {e}"))?;
    let steps = v["steps"].as_array().ok_or("宏文件里没有 steps")?;
    if steps.len() > MAX_STEPS {
        return Err(format!("宏太长 ({} 步, 上限 {MAX_STEPS})", steps.len()));
    }
    let mut out = Vec::with_capacity(steps.len());
    for (i, s) in steps.iter().enumerate() {
        let bad = || format!("第 {} 步格式不对", i + 1);
        let kind = s[0].as_str().ok_or_else(bad)?;
        let name = s[1].as_str().ok_or_else(bad)?;
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(bad());
        }
        match kind {
            "e" => {
                // 命令名只能是 editcore 的 kebab-case 写法; 名字对不上的命令回放时什么也不做 (和录制时一样走 Doc::edit)
                if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                    return Err(format!("第 {} 步: 命令名不合法 {name}", i + 1));
                }
                let t = match &s[2] {
                    serde_json::Value::Null => None,
                    serde_json::Value::String(t) if t.chars().count() <= MAX_TEXT => Some(t.clone()),
                    _ => return Err(bad()),
                };
                out.push(RecEv::Edit(name.to_string(), t));
            }
            "n" => out.push(RecEv::Nav(name.to_string(), s[2].as_bool().ok_or_else(bad)?, s[3].as_bool().ok_or_else(bad)?)),
            _ => return Err(bad()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_sakura_macro_on_a_doc() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["foo bar".into(), "second".into()];
        let n = run_mac(&mut d, "S_InsText(\"X\");\nS_DuplicateLine();\nS_ReplaceAll(\"bar\", \"BAZ\", 0);", 4).unwrap();
        assert_eq!(n, 3);
        assert_eq!(d.lines, vec!["Xfoo BAZ", "Xfoo BAZ", "second"], "复制行之后「全部置换」两行都换");
    }

    #[test]
    fn macro_search_flags_reach_the_find_engine() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["foo foobar foo".into()];
        run_mac(&mut d, "S_ReplaceAll(\"foo\", \"X\", 1);", 4).unwrap(); // 0x01 = 单词单位
        assert_eq!(d.lines, vec!["X foobar X"]);
        let mut d2 = Doc::untitled(2, 1);
        d2.lines = vec!["k=v k=v".into()];
        run_mac(&mut d2, "S_ReplaceNext(\"([a-z])=([a-z])\", \"$2=$1\", 4);", 4).unwrap(); // 0x04 = 正则: $1 $2 要展开
        assert_eq!(d2.lines, vec!["v=k k=v"]);
    }

    #[test]
    fn bad_macro_fails_before_changing_anything() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["keep".into()];
        assert!(run_mac(&mut d, "S_InsText(\"x\");\nS_NoSuchFunction();", 4).is_err());
        assert_eq!(d.lines, vec!["keep"]);
    }

    /// 录一段键盘宏 -> 转成 .mac -> 在另一份一模一样的文档上运行, 结果必须和直接回放录制一样
    #[test]
    fn recorded_key_macro_exports_to_mac_that_replays_the_same() {
        let start = vec!["alpha beta".to_string(), "second line".to_string()];
        let mut rec = Doc::untitled(1, 1);
        rec.lines = start.clone();
        rec.macro_toggle();
        rec.type_text("X");
        rec.nav("ArrowRight", true, false, 20);
        rec.nav("ArrowRight", false, true, 20);
        rec.edit("duplicate-line", None);
        rec.edit("new-line", None);
        rec.edit("backspace", None);
        rec.nav("End", false, false, 20);
        rec.type_text("é\"\\ end");
        rec.macro_toggle();
        let out = rec_to_mac(&rec.last_macro);
        assert!(out.skipped.is_empty(), "这几种操作都有对应 S_ 函数: {:?}", out.skipped);
        assert!(out.text.contains("S_InsText(\"X\");") && out.text.contains("S_DuplicateLine();"), "{}", out.text);
        assert!(out.text.contains("S_WordRight();") && out.text.contains("S_Right_Sel();"), "移动键按默认键反查: {}", out.text);
        assert!(out.text.contains("S_InsText(\"é\\\"\\\\ end\");"), "文字转义: {}", out.text);
        // 同一份起始文档: 回放录制 vs 运行导出的 .mac
        let mut replay = Doc::untitled(2, 1);
        replay.lines = start.clone();
        replay.last_macro = rec.last_macro.clone();
        replay.macro_play();
        let mut viamac = Doc::untitled(3, 1);
        viamac.lines = start;
        run_mac(&mut viamac, &out.text, 4).unwrap();
        assert_eq!(viamac.lines, replay.lines, "导出的 .mac 与直接回放结果一致");
        assert_eq!(viamac.cursor, replay.cursor);
    }

    /// EDIT_KEYS 的每一项都要两头对得上: 键在 Sakura 表里有 S_ 函数 (且就是这个键), 命令 editcore 认得并真的能执行
    #[test]
    fn every_edit_key_entry_resolves_both_ways() {
        let mut bad: Vec<String> = vec![];
        for (key, cmd) in EDIT_KEYS {
            let out = rec_to_mac(&[RecEv::Edit((*cmd).into(), None)]);
            if !out.skipped.is_empty() {
                bad.push(format!("{cmd}: 录不成 S_ 函数 (键 {key})"));
                continue;
            }
            match macro_sakura::plan(&out.text).map(|s| s.into_iter().next().map(|x| x.action)) {
                Ok(Some(Action::Command { key: Some(k), .. })) if k == *key => {}
                other => bad.push(format!("{cmd}: 录出来的 S_ 函数默认键不是 {key}: {other:?} / {}", out.text.trim())),
            }
            let mut d = Doc::untitled(1, 1);
            d.lines = vec!["abc def".into(), "xyz".into()];
            if run_mac(&mut d, &out.text, 4).is_err() {
                bad.push(format!("{cmd}: 高速版运行不了自己录的宏 (editcore 不认识这条命令?)"));
            }
            if edit_for_key(key) != Some(*cmd) || key_for_edit(cmd) != Some(*key) {
                bad.push(format!("{cmd}: 表里两个方向的查找不一致"));
            }
        }
        assert!(bad.is_empty(), "EDIT_KEYS 里这些项不合格:\n{}", bad.join("\n"));
        let mut keys: Vec<_> = EDIT_KEYS.iter().map(|e| e.0).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), EDIT_KEYS.len(), "键不能重复");
    }

    /// 只运行不录制的键 (选区变换 / 整理行): 键要对得上 Sakura 的 S_ 函数, 并且真的改了文档
    /// 带命令 id 的 S_ 函数里, 只动文档本身的那些高速版都能运行 (以前报「这条命令高速版还不支持」); 窗口类的仍然明确报错
    #[test]
    fn document_level_sakura_functions_run_and_window_level_ones_fail_loudly() {
        let fresh = |lines: &[&str]| { let mut d = Doc::untitled(1, 1); d.lines = lines.iter().map(|s| s.to_string()).collect(); d };
        for name in [
            "IndentSpace", "UnindentSpace", "ToHanEi", "ToHankata", "ToZenEi", "GoNextParagraph_Sel", "GoPrevParagraph_Sel", "InsTime", "ModifyLineNext", "ModifyLinePrev",
            "ModifyLineNext_Sel", "ModifyLinePrev_Sel", "FuncListNext", "FuncListPrev", "ViewMode", "ChgmodEOLCR", "ChgmodEOLCRLF", "ChgmodEOLLF", "CodeCnvUNICODEBEtoSJIS",
        ] {
            let mut d = fresh(&["abc def", "", "xyz"]);
            d.select_all();
            run_mac(&mut d, &format!("S_{name}();"), 4).unwrap_or_else(|e| panic!("S_{name}: {e}"));
        }
        let mut d = fresh(&["a", "", "b"]);
        d.select_all();
        run_mac(&mut d, "S_IndentSpace();", 4).unwrap();
        assert_eq!(d.lines, [" a", "", " b"], "SPACE 缩进: 1 个空格, 空行不动 (Sakura 实测)");
        run_mac(&mut d, "S_UnindentSpace();", 4).unwrap();
        assert_eq!(d.lines, ["a", "", "b"]);
        let mut d = fresh(&["x"]);
        run_mac(&mut d, "S_ViewMode();", 4).unwrap();
        assert!(d.readonly);
        run_mac(&mut d, "S_ChgmodEOLLF();", 4).unwrap();
        assert_eq!(d.eol, editcore::document_io::Eol::Lf);
        let mut d = fresh(&["fn a() {}", "", "fn b() {", "}"]);
        d.name = "x.rs".into();
        run_mac(&mut d, "S_FuncListNext();", 4).unwrap();
        assert_eq!(d.cursor[0], 2, "下一个函数");
        run_mac(&mut d, "S_FuncListPrev();", 4).unwrap();
        assert_eq!(d.cursor[0], 0);
        let mut d = fresh(&["a", "", "b", "c"]);
        run_mac(&mut d, "S_GoNextParagraph_Sel();", 4).unwrap();
        assert!(d.sel.is_some(), "按段落选择");
        let mut d = fresh(&["x"]);
        let e = run_mac(&mut d, "S_NextGroup();", 4).unwrap_err();
        assert!(e.contains("next-group") || e.contains("不支持"), "窗口 / 标签类命令: 明确报错, 不静默跳过: {e}");
        assert_eq!(d.lines, ["x"]);
    }

    #[test]
    fn run_only_keys_resolve_to_s_functions_and_act() {
        use macro_sakura::RecEvent;
        for (key, _) in TRANSFORM_KEYS.iter().chain(LINES_KEYS.iter()) {
            // 借录制器的反查确认: 这个键在 Sakura 表里有 S_ 函数, 且是只有键的命令
            let out = macro_sakura::record_to_mac(&[RecEvent::Command { cmd: (*key).into(), key: Some((*key).into()) }]);
            assert!(out.skipped.is_empty(), "{key} 在 Sakura 表里没有对应的 S_ 函数");
            assert!(matches!(macro_sakura::plan(&out.text).unwrap()[0].action, Action::Command { key: Some(_), cmd: None, .. }), "{key}: {}", out.text);
        }
        let run = |lines: &[&str], key: &str| -> Vec<String> {
            let mut d = Doc::untitled(1, 1);
            d.lines = lines.iter().map(|s| s.to_string()).collect();
            d.select_all();
            let out = macro_sakura::record_to_mac(&[RecEvent::Command { cmd: key.into(), key: Some(key.into()) }]);
            run_mac(&mut d, &out.text, 4).unwrap();
            d.lines
        };
        assert_eq!(run(&["Hello Wörld"], "Control+F6"), ["hello wörld"]);
        assert_eq!(run(&["Hello Wörld"], "Control+F7"), ["HELLO WÖRLD"]);
        assert_eq!(run(&["b", "c", "a"], "Alt+a"), ["a", "b", "c"]);
        assert_eq!(run(&["b", "c", "a"], "Alt+d"), ["c", "b", "a"]);
        assert_eq!(run(&["  x  ", "\ty "], "Alt+l"), ["x  ", "y "]);
        assert_eq!(run(&["\u{3000}x"], "Alt+l"), ["\u{3000}x"], "只去空格和 Tab, 不去全角空格 (Sakura 实测)");
        assert_eq!(run(&["a", "a", "b", "a"], "Alt+m"), ["a", "b", "a"], "合并连续重复行");
        assert_eq!(run(&["  x  ", "\ty "], "Alt+r"), ["  x", "\ty"]);
        assert_eq!(run(&["\tb"], "Control+Alt+F5"), ["    b"], "行首 Tab 按 tab 宽度展开成空格");
    }

    #[test]
    fn exported_document_keeps_a_comment_header_and_still_parses() {
        let out = rec_to_mac(&[RecEv::Edit("insert-text".into(), Some("hi".into())), RecEv::Edit("new-line".into(), None), RecEv::Nav("ArrowLeft".into(), false, false)]);
        let (text, n) = mac_document(&out);
        assert!(text.starts_with("/* 键盘录制 */\n"));
        assert_eq!(n, 3);
        assert_eq!(macro_sakura::plan(&text).expect("带注释头的宏仍能被解析").len(), n);
    }

    #[test]
    fn steps_without_an_s_function_are_reported_not_written() {
        let steps = vec![RecEv::Edit("insert-text".into(), Some("a".into())), RecEv::Edit("no-such-edit".into(), None), RecEv::Nav("ArrowUp".into(), true, true)];
        let out = rec_to_mac(&steps);
        assert!(out.text.starts_with("S_InsText(\"a\");"));
        assert_eq!(out.skipped, ["no-such-edit"]);
        assert!(!out.text.contains("no-such-edit"));
    }

    #[test]
    fn key_macro_json_roundtrip() {
        let m = vec![RecEv::Edit("insert-text".into(), Some("héllo \"q\"\n".into())), RecEv::Nav("ArrowDown".into(), true, false), RecEv::Edit("delete-line".into(), None)];
        let back = macro_from_json(&macro_to_json(&m)).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn hostile_macro_files_are_rejected_whole() {
        assert!(macro_from_json("not json").is_err());
        assert!(macro_from_json("{}").is_err());
        assert!(macro_from_json(r#"{"steps":[["e","Delete Line; rm",null]]}"#).is_err(), "命令名不合法");
        assert!(macro_from_json(r#"{"steps":[["e","delete-line",null],["x","a"]]}"#).is_err(), "一步坏了整份拒绝");
        assert!(macro_from_json(r#"{"steps":[["n","ArrowDown","yes",false]]}"#).is_err());
        let long = "a".repeat(MAX_NAME + 1);
        assert!(macro_from_json(&format!(r#"{{"steps":[["n","{long}",false,false]]}}"#)).is_err());
        let many = format!(r#"{{"steps":[{}]}}"#, vec![r#"["n","ArrowDown",false,false]"#; MAX_STEPS + 1].join(","));
        assert!(macro_from_json(&many).is_err(), "步数封顶");
    }
}
