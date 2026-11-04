use super::*;
use crate::settings::TypePref;

/// 不落盘的 App; 语言是全局量, 拿锁串行化
fn app(st: Settings) -> (App, std::sync::MutexGuard<'static, ()>) {
    let g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    (App::build(vec![], None, Trace::new(), st, false), g)
}

#[test]
fn markdown_block_click_edits_in_place_and_apply_is_one_undo_step() {
    let (mut a, _g) = app(Settings::default());
    a.new_doc();
    let ai = a.active;
    let orig: Vec<String> = ["# Title", "", "first para", "", "second para"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    a.docs[ai].lines = orig.clone();
    let blocks = crate::markdown::parse(&a.docs[ai].lines);
    let n = blocks.iter().position(|b| b.line == 2).expect("段落块");
    // 点块 -> 进入就地编辑, 编辑框装着该块原文
    a.md_handle(
        ai,
        &blocks,
        &crate::md_view::MdEvent {
            edit_req: Some(n),
            ..Default::default()
        },
    );
    let e = a.md_cache.edit.as_ref().expect("进入编辑");
    assert_eq!((e.start, e.end, e.buf.as_str()), (2, 3, "first para"));
    // 点框外 (有改动) = 应用: 一个补丁, 撤销一次就回到原文
    let ev = crate::md_view::MdEvent {
        apply: Some((
            2,
            3,
            "first
EDITED"
                .into(),
        )),
        ..Default::default()
    };
    a.md_handle(ai, &blocks, &ev);
    assert_eq!(
        a.docs[ai].lines[2..4],
        ["first".to_string(), "EDITED".to_string()]
    );
    a.docs[ai].undo();
    assert_eq!(a.docs[ai].lines, orig, "一次撤销还原整个就地编辑");
    // 同一帧既应用又点了另一块: 不开新编辑 (行号已变)
    a.md_cache.edit = None;
    let ev = crate::md_view::MdEvent {
        apply: Some((2, 3, "x".into())),
        edit_req: Some(0),
        ..Default::default()
    };
    a.md_handle(ai, &blocks, &ev);
    assert!(a.md_cache.edit.is_none());
}

#[test]
fn settings_survive_a_trip_through_the_running_app() {
    let mut st = Settings::default();
    st.dark = false;
    st.font_px = 18.0;
    st.tab_width = 2;
    st.lang = "en".into();
    st.auto_indent = false;
    st.save_backup = true;
    st.font_family = "consola.ttf".into();
    st.line_height = 1.5;
    st.colors.keyword = Some([1, 2, 3]);
    st.view.side_w = 333.0;
    st.view.term_h = 99.0;
    let (a, _g) = app(st.clone());
    let back = a.current_settings();
    assert_eq!(
        (
            back.dark,
            back.font_px,
            back.tab_width,
            back.lang.as_str(),
            back.auto_indent,
            back.save_backup
        ),
        (false, 18.0, 2, "en", false, true)
    );
    assert_eq!(
        (
            back.font_family.as_str(),
            back.line_height,
            back.colors.keyword
        ),
        ("consola.ttf", 1.5, Some([1, 2, 3]))
    );
    assert_eq!((back.view.side_w, back.view.term_h), (333.0, 99.0));
    assert_eq!(
        crate::i18n::cur(),
        crate::i18n::Lang::En,
        "语言设置生效到界面"
    );
    crate::i18n::set(crate::i18n::Lang::Zh);
}

#[test]
fn new_documents_use_the_default_encoding_and_eol() {
    let mut st = Settings::default();
    st.default_encoding = "GBK".into();
    st.default_eol = "LF".into();
    let (mut a, _g) = app(st);
    a.new_doc();
    let d = a.cur();
    assert_eq!((d.charset, d.eol), (Charset::Gbk, Eol::Lf));
    let mut st = Settings::default();
    st.default_encoding = "Unicode".into();
    let (mut b, _g2) = (App::build(vec![], None, Trace::new(), st, false), ());
    b.new_doc();
    assert_eq!(
        b.cur().bom,
        editcore::document_io::Bom::Utf16Le,
        "UTF-16 的新文档带对应的 BOM"
    );
    assert_eq!(
        App::build(
            vec![],
            None,
            Trace::new(),
            Settings {
                default_encoding: "bogus".into(),
                ..Settings::default()
            },
            false
        )
        .docs[0]
            .charset,
        Charset::Utf8,
        "认不出的编码 = UTF-8"
    );
}

#[test]
fn manual_syntax_type_overrides_the_extension_and_unknown_names_fall_back() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].name = "x.txt".into();
    assert_eq!(
        a.type_of(&a.docs[0]).map(|t| t.name.as_str()),
        Some("Plain")
    );
    a.docs[0].syntax = Some("Rust".into());
    assert_eq!(a.type_of(&a.docs[0]).map(|t| t.name.as_str()), Some("Rust"));
    a.docs[0].syntax = Some("NoSuchType".into());
    assert_eq!(
        a.type_of(&a.docs[0]).map(|t| t.name.as_str()),
        Some("Plain"),
        "查不到 = 按扩展名"
    );
}

#[test]
fn function_step_jumps_between_outline_entries_and_wraps() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].name = "x.rs".into();
    a.docs[0].lines = ["fn a() {}", "", "fn b() {", "}", "", "fn c() {}"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    a.docs[0].cursor = [3, 0];
    a.step_function(true);
    assert_eq!(a.docs[0].cursor[0], 5);
    a.step_function(true);
    assert_eq!(a.docs[0].cursor[0], 0, "最后一个函数再往下绕回第一个");
    a.step_function(false);
    assert_eq!(a.docs[0].cursor[0], 5, "第一个再往上绕回最后一个");
    a.docs[0].name = "x.txt".into();
    a.docs[0].lines = vec!["plain".into()];
    a.docs[0].rev += 1; // 内容变了, 大纲缓存按修订号失效
    a.docs[0].cursor = [0, 0];
    a.step_function(true);
    assert_eq!(a.docs[0].cursor, [0, 0], "没有大纲的文件: 不动");
    assert!(a.docs[0].tip.contains("没有函数标记"), "{}", a.docs[0].tip);
}

#[test]
fn copy_commands_put_the_expected_text_on_the_clipboard_history() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.docs[0].name = "n.txt".into();
    a.docs[0].lines = vec!["abcdefghij".into()];
    a.docs[0].cursor = [0, 3];
    a.copy_tag(&ctx);
    assert_eq!(
        a.tools.clip.first().map(String::as_str),
        Some("n.txt(1,4)"),
        "没存盘的文档用文档名, 行列从 1 数"
    );
    a.cfg.view.wrap_col = 4;
    a.copy_wrapped(&ctx);
    assert_eq!(
        a.tools.clip.first().map(String::as_str),
        Some("abcd\r\nefgh\r\nij"),
        "无选区 = 当前行, 每 4 字符切一刀"
    );
    a.docs[0].sel = Some([0, 2, 0, 8]);
    a.copy_wrapped(&ctx);
    assert_eq!(
        a.tools.clip.first().map(String::as_str),
        Some("cdef\r\nGH".to_lowercase().as_str()),
        "有选区 = 只复制选区"
    );
}

#[test]
fn open_counterpart_switches_between_header_and_source() {
    let dir = std::env::temp_dir().join(format!("idefast-hc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (c, h) = (dir.join("foo.c"), dir.join("foo.hpp"));
    std::fs::write(&c, "int main(){}\n").unwrap();
    std::fs::write(&h, "#pragma once\n").unwrap();
    let (mut a, _g) = app(Settings::default());
    a.open_path(&c);
    assert_eq!(a.cur().path.as_deref(), Some(c.as_path()));
    a.open_counterpart();
    assert_eq!(
        a.cur().path.as_deref(),
        Some(h.as_path()),
        "foo.c -> foo.hpp (foo.h 不存在, 按顺序找下一个)"
    );
    a.open_counterpart();
    assert_eq!(
        a.cur().path.as_deref(),
        Some(c.as_path()),
        "再按回到 foo.c, 不重复开标签"
    );
    let before = a.docs.len();
    std::fs::remove_file(&h).unwrap();
    a.open_counterpart();
    assert_eq!(
        (a.docs.len(), a.cur().tip.contains("没有找到")),
        (before, true),
        "对应文件不存在: 只提示"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn completion_lists_candidates_cycles_and_accepts_as_one_undo_step() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].name = "x.rs".into();
    a.docs[0].lines = vec![
        "fn main() {".into(),
        "    let counter = 1;".into(),
        "    cou + 1".into(),
    ];
    a.docs[0].goto(2, 7);
    a.show_completion();
    let p = a.sys.complete.as_ref().expect("前缀 cou 有候选");
    assert_eq!(
        p.items,
        ["counter"],
        "文件里出现过的标识符; 关键字里没有以 cou 开头的"
    );
    a.docs[0].goto(1, 7);
    a.show_completion();
    assert!(
        a.sys.complete.is_none(),
        "前缀就是 let 本身: 没有更长的候选, 不弹浮层"
    );
    // 前缀 "l": 关键字 let / loop 排在文件标识符前面; 上下键到头绕回
    a.docs[0].lines[2] = "    l".into();
    a.docs[0].goto(2, 5);
    a.show_completion();
    let n = a.sys.complete.as_ref().unwrap().items.len();
    assert!(n >= 2 && a.sys.complete.as_ref().unwrap().items[0].starts_with('l'));
    a.complete_move(-1);
    assert_eq!(
        a.sys.complete.as_ref().unwrap().idx,
        n - 1,
        "第一项再往上绕回最后一项"
    );
    a.complete_move(1);
    assert_eq!(a.sys.complete.as_ref().unwrap().idx, 0);
    // 接受第一项: 前缀换成候选, 光标到候选末尾, 一步撤销
    let first = a.sys.complete.as_ref().unwrap().items[0].clone();
    a.accept_completion();
    assert!(a.sys.complete.is_none());
    assert_eq!(a.docs[0].lines[2], format!("    {first}"));
    assert_eq!(a.docs[0].cursor, [2, 4 + first.chars().count()]);
    a.docs[0].undo();
    assert_eq!(a.docs[0].lines[2], "    l", "一步撤销回到补全前");
}

#[test]
fn completion_reports_when_there_is_nothing_and_closes_when_stale() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].lines = vec!["zq".into()];
    a.docs[0].goto(0, 2);
    a.show_completion();
    assert!(
        a.sys.complete.is_none() && a.docs[0].tip.contains("无补全候选"),
        "{}",
        a.docs[0].tip
    );
    a.docs[0].lines = vec!["alpha_one alpha_two al".into()];
    a.docs[0].goto(0, 22);
    a.show_completion();
    assert!(a.sys.complete.is_some());
    a.docs[0].goto(0, 3); // 光标挪走: 浮层该关
    a.completion_popup(&egui::Context::default());
    assert!(a.sys.complete.is_none(), "光标变了就关掉浮层");
    a.docs[0].goto(0, 22);
    a.show_completion();
    a.docs[0].type_text("x"); // 打字: 修订号变了
    a.completion_popup(&egui::Context::default());
    assert!(a.sys.complete.is_none(), "文档变了就关掉浮层");
}

#[test]
fn goto_definition_jumps_to_the_definition_line_and_reports_misses() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].lines = vec![
        "main();".into(),
        "fn helper() {}".into(),
        "helper();".into(),
        "  + ".into(),
    ];
    a.docs[0].goto(2, 3);
    a.goto_definition();
    assert_eq!(a.docs[0].cursor, [1, 3], "跳到 fn helper 的 helper 上");
    assert!(a.docs[0].tip.contains("定义 @ Ln 2"), "{}", a.docs[0].tip);
    a.docs[0].goto(3, 3);
    a.goto_definition();
    assert!(
        a.docs[0].tip.contains("光标处无标识符"),
        "{}",
        a.docs[0].tip
    );
    a.docs[0].lines[3] = "unknown_sym".into();
    a.docs[0].goto(3, 4);
    a.goto_definition();
    assert_eq!(a.docs[0].cursor, [3, 0], "只出现这一次: 首次出现就是自己");
}

#[test]
fn macro_runs_document_and_window_steps_in_order_and_follows_the_new_active_tab() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.run_mac_src(
        "S_InsText(\"first\");\nS_FileNew();\nS_InsText(\"second\");",
        0,
        &ctx,
    );
    assert_eq!(a.docs.len(), 2, "S_FileNew 开了新标签");
    assert_eq!(a.docs[0].lines, ["first"], "新建之前的文字在原来的文档里");
    assert_eq!(
        a.docs[1].lines,
        ["second"],
        "新建之后的步骤跟着新的活动标签走"
    );
    assert_eq!(a.active, 1);
    assert!(a.cur().tip.contains("宏已执行 (3 步)"), "{}", a.cur().tip);
}

#[test]
fn macro_with_an_unrunnable_step_does_nothing_and_says_why() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.run_mac_src("S_InsText(\"x\");\nS_Cascade();", 0, &ctx);
    assert_eq!(a.docs[0].lines, [""], "整份宏都没执行");
    assert!(
        a.docs[0].tip.contains("宏未执行")
            && a.docs[0].tip.contains("第 2 行")
            && a.docs[0].tip.contains("层叠窗口"),
        "{}",
        a.docs[0].tip
    );
}

#[test]
fn macro_steps_for_window_list_help_and_panes_run_through_the_app() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.side.open = false;
    a.run_mac_src("S_Outline();\nS_WinList();", 0, &ctx);
    assert!(
        a.side.open && a.side.tab == Tab::Outline,
        "S_Outline 切到大纲页"
    );
    assert_eq!(
        a.tools.pick.as_ref().map(|p| p.kind),
        Some(crate::pick::Kind::Windows),
        "S_WinList 打开窗口一览"
    );
    a.tools.pick = None;
    a.run_mac_src(
        "S_FileTree();\nS_ToggleKeySearch();\nS_HelpContents();",
        0,
        &ctx,
    );
    assert!(
        a.side.tab == Tab::Files && a.cfg.view.key_search,
        "S_FileTree 切文件页, S_ToggleKeySearch 打开自动显示"
    );
    assert_eq!(
        a.cur().name,
        crate::help::MANUAL_NAME,
        "S_HelpContents 开手册"
    );
    a.run_mac_src("S_ChgCharset();", 0, &ctx);
    assert_eq!(
        a.tools.pick.as_ref().map(|p| p.kind),
        Some(crate::pick::Kind::Charset)
    );
    a.run_mac_src("S_FileReopen();", 0, &ctx);
    assert_eq!(
        a.tools.pick.as_ref().map(|p| p.kind),
        Some(crate::pick::Kind::Reopen),
        "后开的列表替换先开的"
    );
    a.run_mac_src("S_ShowTab();", 0, &ctx);
    assert!(
        a.cur().tip.contains("宏未执行") && a.cur().tip.contains("开关"),
        "ShowTab 仍然明确拒绝: {}",
        a.cur().tip
    );
}

#[test]
fn macro_clipboard_reaches_the_app_clipboard_history() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.docs[0].lines = vec!["alpha".into(), "beta".into()];
    a.docs[0].goto(1, 0);
    a.run_mac_src("S_Copy();\nS_GoFileTop();\nS_Paste();", 0, &ctx);
    assert_eq!(
        a.tools.clip.first().map(String::as_str),
        Some("beta\n"),
        "宏里复制的整行同步进了剪贴板历史"
    );
    assert_eq!(
        a.docs[0].lines,
        ["beta", "alpha", "beta"],
        "宏里粘贴用的是宏自己的剪贴板"
    );
}

#[test]
fn macro_slots_run_register_and_report_empty_or_full() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.run_macro_slot(0, &ctx);
    assert!(a.cur().tip.contains("宏 1 是空的"), "{}", a.cur().tip);
    a.cfg.macro_slots[3] = crate::settings::MacroSlot {
        name: "加头".into(),
        kind: "mac".into(),
        code: "S_InsText(\"> \");".into(),
    };
    a.docs[0].lines = vec!["body".into()];
    a.docs[0].goto(0, 0);
    a.run_macro_slot(3, &ctx);
    assert_eq!(
        a.docs[0].lines,
        ["> body"],
        "槽里的 Sakura 宏作用在活动文档上"
    );
    // 登记: 选区优先, 否则整份文档; .js 不登记
    a.docs[0].name = "tidy.mac".into();
    a.docs[0].lines = vec!["S_Undo();".into()];
    a.register_macro_doc();
    let tip = a.cur().tip.clone();
    assert_eq!(
        (
            a.cfg.macro_slots[0].name.as_str(),
            a.cfg.macro_slots[0].kind.as_str(),
            a.cfg.macro_slots[0].code.trim()
        ),
        ("tidy", "mac", "S_Undo();"),
        "{tip}"
    );
    assert!(tip.contains("已登记到宏 1"), "{tip}");
    a.docs[0].name = "x.ppa".into();
    a.register_macro_doc();
    assert_eq!(a.cfg.macro_slots[1].kind, "ppa", ".ppa 文档登记成 PPA");
    a.docs[0].name = "m.js".into();
    a.register_macro_doc();
    assert!(
        a.cur().tip.contains("不运行 JS 宏") && a.cfg.macro_slots[2].is_empty(),
        "{}",
        a.cur().tip
    );
    a.docs[0].name = "full.mac".into();
    for i in 0..crate::settings::MACRO_SLOTS {
        if a.cfg.macro_slots[i].is_empty() {
            a.cfg.macro_slots[i].code = "S_Undo();".into();
        }
    }
    a.register_macro_doc();
    assert!(a.cur().tip.contains("宏槽都满了"), "{}", a.cur().tip);
    a.docs[0].lines = vec!["  ".into()];
    a.register_macro_doc();
    assert!(
        a.cur().tip.contains("没有可登记的内容") || a.cur().tip.contains("宏槽都满了"),
        "{}",
        a.cur().tip
    );
}

#[test]
fn tab_group_commands_switch_move_merge_and_close() {
    let (mut a, _g) = app(Settings::default());
    a.new_doc();
    a.new_doc();
    a.new_doc();
    for (d, g) in a.docs.iter_mut().zip([0u8, 2, 2, 4]) {
        d.group = g;
    }
    a.active = 0;
    a.group_switch(true);
    assert_eq!(a.active, 1, "下一个分组 (2) 里最靠左的标签");
    a.group_switch(true);
    assert_eq!(a.active, 3);
    a.group_switch(true);
    assert_eq!(a.active, 0, "最后一个分组再往后绕回第一个");
    a.group_switch(false);
    assert_eq!(a.active, 3, "往前绕回最后一个");
    a.group_joint(false);
    assert_eq!(a.docs[3].group, 2, "当前标签移到上一个分组");
    a.active = 0;
    a.group_separate();
    assert_eq!(a.docs[0].group, 3, "新分组 = 现有最大编号 + 1");
    a.group_bind_all();
    assert!(
        a.docs.iter().all(|d| d.group == 0),
        "合并为一组 = 都回到无分组"
    );
    a.group_switch(true);
    assert!(a.cur().tip.contains("只有一个分组"), "{}", a.cur().tip);
    a.docs[1].group = 2;
    a.docs[2].group = 2;
    a.active = 1;
    a.group_close();
    assert_eq!(a.docs.len(), 2, "当前分组里的两个标签关掉了");
    assert!(a.docs.iter().all(|d| d.group == 0) && a.active < a.docs.len());
}

#[test]
fn small_view_and_search_commands_update_their_state() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].lines = vec!["abc".into(), "def".into()];
    a.center_cursor();
    assert!(
        a.center_once && a.docs[0].reveal,
        "光标行居中: 下一帧居中并滚到光标"
    );
    assert_eq!(a.sys.sel_count, 0);
    a.cycle_sel_count();
    a.cycle_sel_count();
    assert_eq!(a.sys.sel_count, 2);
    assert!(a.cur().tip.contains("行数"), "{}", a.cur().tip);
    a.cycle_sel_count();
    assert_eq!(a.sys.sel_count, 0, "字符 -> 字节 -> 行 -> 回到字符");
    let m = a.find.marks;
    a.toggle_find_marks();
    assert_eq!(a.find.marks, !m);
    a.jump_find_start();
    assert!(a.cur().tip.contains("还没有查找起点"), "没查找过: 只提示");
    a.find.needle = "e".into();
    a.docs[0].goto(1, 2);
    a.jump_find_start();
    assert_eq!(a.docs[0].cursor, [0, 0], "回到查找开始时的位置");
    a.docs[0].goto(1, 1);
    a.space_indent_cmd(false);
    assert_eq!(a.docs[0].lines[1], " def", "缩进当前行 1 个空格");
    a.space_indent_cmd(true);
    assert_eq!(a.docs[0].lines[1], "def");
}

#[test]
fn column_delete_needs_a_box_selection_and_removes_the_block() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].lines = vec!["abcdef".into(), "ghijkl".into(), "mnopqr".into()];
    a.column_delete();
    assert_eq!(a.docs[0].lines[0], "abcdef", "没有列块选区: 什么也不改");
    assert!(a.docs[0].tip.contains("Alt+拖选"));
    a.docs[0].box_sel = Some([0, 1, 2, 3]); // 第 0..=2 行, 第 1..3 列
    a.column_delete();
    assert_eq!(a.docs[0].lines, vec!["adef", "gjkl", "mpqr"]);
    assert_eq!(a.docs[0].cursor, [0, 1], "光标回到矩形左上角");
}

#[test]
fn column_replace_opens_with_the_block_text_and_applies_per_row() {
    let (mut a, _g) = app(Settings::default());
    a.docs[0].lines = vec!["abcdef".into(), "ghijkl".into()];
    a.column_replace();
    assert!(!a.tools.col_open, "没有列块选区: 不弹窗");
    a.docs[0].box_sel = Some([0, 1, 1, 3]);
    a.docs[0].cursor = [1, 3];
    a.column_replace();
    assert!(a.tools.col_open);
    assert_eq!(
        (a.docs[0].box_sel, a.docs[0].cursor),
        (Some([0, 1, 1, 3]), [1, 3]),
        "取预填文字不能弄丢列块选区和光标"
    );
    assert!(a.docs[0].rec.is_none() && a.docs[0].lines == vec!["abcdef", "ghijkl"]);
    a.docs[0].edit("box-replace", Some("12\nXY".into()));
    assert_eq!(
        a.docs[0].lines,
        vec!["a12def", "gXYjkl"],
        "每一行换成对应的一行文字"
    );
}

#[test]
fn doc_prefs_follow_settings_and_extension_overrides() {
    let mut st = Settings::default();
    st.auto_indent = false;
    st.smart_indent = true;
    st.insert_spaces = false;
    st.file_types = vec![TypePref {
        ext: "py".into(),
        insert_spaces: Some(true),
        tab_width: Some(2),
        wrap: Some(true),
        wrap_col: None,
    }];
    let (mut a, _g) = app(st);
    a.docs[0].name = "x.py".into();
    a.new_doc();
    a.docs[1].name = "y.txt".into();
    a.sync_doc_prefs();
    assert_eq!(
        (
            a.docs[0].auto_indent,
            a.docs[0].smart_indent,
            a.docs[0].insert_spaces
        ),
        (false, true, true),
        ".py 的 Tab 插入空格被覆盖为开"
    );
    assert!(!a.docs[1].insert_spaces);
    assert_eq!(a.eff("x.py"), (2, true, true));
    assert_eq!(a.eff("y.txt"), (4, false, false));
}

#[test]
fn prefs_dialog_applies_on_ok_and_discards_on_cancel() {
    let (mut a, _g) = app(Settings::default());
    a.recent = vec!["a.txt".into(), "b.txt".into(), "c.txt".into()];
    a.open_prefs();
    {
        let p = a.prefs.as_mut().unwrap();
        p.draft.tab_width = 8;
        p.draft.mru_files = 2;
        p.draft.lang = "ja".into();
        p.draft.insert_spaces = true;
    }
    assert_eq!(
        (a.tab, a.cfg.insert_spaces),
        (4, false),
        "草稿没提交前不影响运行状态"
    );
    a.recent.push("d.txt".into()); // 对话框开着时又打开了文件: 提交不能把它冲掉
    a.prefs_commit();
    assert_eq!(
        (a.tab, a.cfg.insert_spaces, a.cfg.lang.as_str()),
        (8, true, "ja")
    );
    assert_eq!(crate::i18n::cur(), crate::i18n::Lang::Ja);
    assert_eq!(
        a.recent,
        vec!["a.txt", "b.txt"],
        "最近文件个数按新设置截断, 内容用现在的而不是草稿里的旧快照"
    );
    // 取消: 草稿丢弃
    a.prefs = None;
    a.open_prefs();
    a.prefs.as_mut().unwrap().draft.tab_width = 3;
    a.prefs = None;
    assert_eq!(a.tab, 8);
    crate::i18n::set(crate::i18n::Lang::Zh);
}

#[test]
fn reset_keeps_recent_files_but_restores_preferences() {
    let (mut a, _g) = app(Settings {
        tab_width: 9,
        font_family: "x.ttf".into(),
        ..Settings::default()
    });
    a.recent = vec!["keep.txt".into()];
    let mut next = a.current_settings();
    next.reset_prefs();
    a.apply_settings(&next);
    assert_eq!(
        (a.tab, a.cfg.font_family.as_str(), a.recent.clone()),
        (4, "", vec!["keep.txt".to_string()])
    );
}

#[test]
fn right_dock_cap_leaves_editor_room() {
    assert_eq!(right_cap(900.0), 660.0);
    assert_eq!(right_cap(300.0), 100.0, "窗口再窄也不会把上限压到 0");
    assert!(right_room(560.0) && !right_room(399.0));
}

#[test]
fn view_toggles_persist_and_minimal_layout_hides_panels() {
    let mut st = Settings::default();
    st.view.ruler = true;
    st.view.whitespace = true;
    st.view.line_numbers = false;
    let (mut a, _g) = app(st);
    let back = a.current_settings();
    assert_eq!(
        (
            back.view.ruler,
            back.view.whitespace,
            back.view.line_numbers,
            back.view.status_bar
        ),
        (true, true, false, true)
    );
    a.side.open = true;
    a.term.open = true;
    a.md_preview = true;
    a.set_minimal();
    assert_eq!(
        (a.side.open, a.term.open, a.md_preview, a.cfg.view.ruler),
        (false, false, false, false)
    );
    assert!(a.cfg.view.status_bar, "极简布局仍留状态栏");
}

fn set_lines(d: &mut Doc, text: &str) {
    d.lines = text.split('\n').map(str::to_string).collect();
    d.rev += 1;
}

#[test]
fn comparing_two_tabs_marks_lines_tracks_edits_and_ends_when_a_tab_closes() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.new_doc();
    set_lines(&mut a.docs[0], "a\nb\nc");
    set_lines(&mut a.docs[1], "a\nX\nc\nd");
    a.active = 0;
    a.start_diff(1);
    let v = a.diff.as_ref().expect("比较已开始");
    assert_eq!(v.hunks.len(), 2, "第 2 行改了, 末尾多一行");
    let (l, r) = (v.left, v.right);
    assert!(
        a.diff.as_ref().unwrap().marks_of(l).is_some()
            && a.diff.as_ref().unwrap().marks_of(r).is_some()
    );
    // 改右边: 版本号变了, 重算
    set_lines(&mut a.docs[1], "a\nb\nc");
    a.diff_refresh();
    assert!(a.diff.as_ref().unwrap().hunks.is_empty());
    // 下一处差异在没有差异时给提示而不是动光标
    a.diff_step(true);
    assert_eq!(a.docs[0].cursor, [0, 0]);
    // 关掉一份: 比较结束
    a.docs.truncate(1);
    a.diff_refresh();
    assert!(a.diff.is_none());
}

#[test]
fn diff_step_walks_hunks_and_wraps() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.new_doc();
    set_lines(&mut a.docs[0], "1\n2\n3\n4\n5\n6");
    set_lines(&mut a.docs[1], "1\nX\n3\n4\nY\n6");
    a.active = 1; // 右侧当前
    a.docs[1].cursor = [0, 0];
    a.diff = Some(crate::diff::DiffView::new(
        a.docs[0].id,
        a.docs[1].id,
        &a.docs[0].lines,
        &a.docs[1].lines,
        (a.docs[0].rev, a.docs[1].rev),
    ));
    a.diff_step(true);
    assert_eq!(a.docs[1].cursor[0], 1);
    a.diff_step(true);
    assert_eq!(a.docs[1].cursor[0], 4);
    a.diff_step(true);
    assert_eq!(a.docs[1].cursor[0], 1, "到头回绕");
    a.diff_step(false);
    assert_eq!(a.docs[1].cursor[0], 4, "往回也回绕");
}

#[test]
fn renaming_a_file_or_folder_retargets_open_tabs() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.docs[0].path = Some(PathBuf::from("/p/old/a.txt"));
    a.docs[0].name = "a.txt".into();
    a.docs_follow_rename(
        std::path::Path::new("/p/old"),
        std::path::Path::new("/p/new"),
    );
    assert_eq!(
        a.docs[0].path.as_deref(),
        Some(std::path::Path::new("/p/new/a.txt"))
    );
    a.docs_follow_rename(
        std::path::Path::new("/p/new/a.txt"),
        std::path::Path::new("/p/new/b.md"),
    );
    assert_eq!(
        (a.docs[0].name.as_str(), a.docs[0].path.as_deref()),
        ("b.md", Some(std::path::Path::new("/p/new/b.md")))
    );
}

#[test]
fn opening_a_text_doc_makes_a_read_only_tab() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.open_text_doc("Diff: x.diff", "@@ -1 +1 @@\n-a\n+b\n");
    let d = a.cur();
    assert!(d.readonly && !d.dirty && d.name == "Diff: x.diff");
    assert_eq!(d.lines, vec!["@@ -1 +1 @@", "-a", "+b"]);
}

#[test]
fn choosing_a_theme_plugin_switches_colors_and_syntax_theme() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("ide-fast-app-theme-{}", std::process::id()));
    std::env::set_var("IDE_PLUGIN_DIR", &dir);
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    a.set_theme_plugin("theme-light");
    assert!(a.theme_vars.is_some());
    let tc = crate::view::ThemeColors::from_vars(a.theme_vars.as_ref().unwrap());
    assert!(!tc.dark);
    a.set_theme_plugin("");
    assert!(a.theme_vars.is_none() && a.theme_id.is_empty());
    std::env::remove_var("IDE_PLUGIN_DIR");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn geometry_is_remembered_only_in_the_normal_state() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(
        egui::RawInput {
            viewport_id: egui::ViewportId::ROOT,
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    outer_rect: Some(Rect::from_min_size(
                        Pos2::new(40.0, 50.0),
                        Vec2::new(900.0, 700.0),
                    )),
                    inner_rect: Some(Rect::from_min_size(
                        Pos2::new(48.0, 80.0),
                        Vec2::new(884.0, 660.0),
                    )),
                    ..Default::default()
                },
            ))
            .collect(),
            ..Default::default()
        },
        |ui| a.track_geometry(ui.ctx()),
    );
    out.textures_delta.clear();
    let g = a.geom_now.expect("正常状态记下了位置");
    assert_eq!((g.x, g.y, g.w, g.h), (40.0, 50.0, 884.0, 660.0));
    assert!(!a.maximized_now);
}

#[test]
fn session_is_restored_only_without_command_line_files() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
    let dir = std::env::temp_dir().join(format!("idefast-session-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    std::fs::write(&a, "aaa").unwrap();
    std::fs::write(&b, "bbb").unwrap();
    let mut st = Settings::default();
    st.session = vec![
        a.display().to_string(),
        dir.join("gone.txt").display().to_string(),
        b.display().to_string(),
        "\\\\evil\\share\\x.txt".into(),
    ];
    st.session_active = 2;
    let app1 = App::build(vec![], None, Trace::new(), st.clone(), false);
    let names: Vec<_> = app1.docs.iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["a.txt", "b.txt"], "不存在的 / UNC 的跳过");
    assert_eq!(
        app1.docs[app1.active].name, "b.txt",
        "活动标签回到上次的位置"
    );
    // 往回写: 当前打开的文件就是下次的会话
    let now = app1.current_settings();
    assert_eq!(
        now.session,
        vec![a.display().to_string(), b.display().to_string()]
    );
    assert_eq!(now.session_active, 1);
    // 命令行给了文件 = 不恢复; 关掉开关 = 不恢复
    let app2 = App::build(
        vec![a.display().to_string()],
        None,
        Trace::new(),
        st.clone(),
        false,
    );
    assert_eq!(app2.docs.len(), 1);
    st.restore_session = false;
    assert_eq!(
        App::build(vec![], None, Trace::new(), st, false).docs.len(),
        1,
        "只有一份空白文档"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_snapshot_survives_the_close_confirmation_loop() {
    let _g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner()); // App::build 会写全局语言
    let dir = std::env::temp_dir().join(format!("idefast-exit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    let mut st = Settings::default();
    st.session = vec![a.display().to_string(), b.display().to_string()];
    st.session_active = 1;
    let mut app = App::build(vec![], None, Trace::new(), st, false);
    // 关窗请求: 先冻结, 之后确认循环把标签一个个关掉也不影响要写下去的会话
    app.exit_session = Some((app.session_paths(), app.session_active()));
    app.docs.truncate(1);
    let now = app.current_settings();
    assert_eq!(now.session.len(), 2, "两个文件都在会话里");
    assert_eq!(now.session_active, 1);
    // 用户取消关窗: 快照清掉, 会话又跟着现在打开的文件走
    app.exit_session = None;
    assert_eq!(app.current_settings().session.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 100 万字符的单行: 一帧能画完, 光标在行中间 / 选区 / 查找命中都不 panic, 撑宽用整行长度
#[test]
fn a_million_char_single_line_renders_with_caret_selection_and_marks() {
    let (mut a, _g) = app(Settings::default());
    let ctx = egui::Context::default();
    a.docs[0].lines = vec!["word ".repeat(200_000)];
    a.docs[0].goto(0, 500_000);
    a.docs[0].sel = Some([0, 499_990, 0, 500_010]);
    for _ in 0..3 {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| a.ui_main(ui));
        out.textures_delta.clear();
    }
    assert!(
        a.max_cols >= 1_000_000,
        "横向滚动区按整行长度撑开, 不是按窗口"
    );
}
