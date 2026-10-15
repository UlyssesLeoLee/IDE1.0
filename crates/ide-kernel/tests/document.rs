//! `ide-kernel` 集成测试 —— 验证编辑语义, 不只是"函数存在".
//!
//! 这些断言刻意覆盖日语编辑器最容易出错的地方:
//! - 多字节字符 (char 而非 byte 位置)
//! - 跨行删除/换行的边界
//! - undo 的事务边界 (一次按键一次撤销)
//! - 多光标的一次撤销

use ide_kernel::document::{Document, Encoding, LineEnding, Pos};
use ide_kernel::session::Session;
use ide_kernel::text_ops::{self, FindOptions};

/// 构造位置
fn pos(r: i32, c: i32) -> Pos {
    Pos { row: r, col: c }
}

// ---------- 基本编辑 ----------

#[test]
/// 插入后光标前进到末尾
fn insert_advances_caret() {
    let mut d = Document::new();
    d.set_text("hello");
    d.set_caret(pos(0, 5));
    let p = d.insert(pos(0, 5), " world");
    assert_eq!(d.text(), "hello world");
    assert_eq!(p, pos(0, 11));
}

#[test]
/// 退格删掉光标前一个字符
fn backspace_removes_prev_char() {
    let mut d = Document::from_text("abc");
    d.set_caret(pos(0, 3));
    d.backspace();
    assert_eq!(d.text(), "ab");
    assert_eq!(d.caret(), pos(0, 2));
}

#[test]
/// 行首退格与上一行合并
fn backspace_at_line_start_joins_prev_line() {
    let mut d = Document::from_text("abc\ndef");
    d.set_caret(pos(1, 0));
    d.backspace();
    assert_eq!(d.text(), "abcdef");
    assert_eq!(d.caret(), pos(0, 3), "光标应落在上一行末尾");
}

#[test]
/// 行尾 Delete 与下一行合并
fn delete_at_line_end_joins_next_line() {
    let mut d = Document::from_text("abc\ndef");
    d.set_caret(pos(0, 3));
    d.delete_forward();
    assert_eq!(d.text(), "abcdef");
    assert_eq!(d.caret(), pos(0, 3));
}

#[test]
/// 文档开头退格无操作
fn backspace_at_origin_is_noop() {
    let mut d = Document::from_text("abc");
    d.set_caret(pos(0, 0));
    d.backspace();
    assert_eq!(d.text(), "abc", "行首退格不应改变内容");
}

#[test]
/// 文档末尾 Delete 无操作
fn delete_at_end_is_noop() {
    let mut d = Document::from_text("abc");
    d.set_caret(pos(0, 3));
    d.delete_forward();
    assert_eq!(d.text(), "abc");
}

// ---------- 多字节字符 (日语编辑器核心承诺) ----------

#[test]
/// 多字节字符按 char 而非 byte 退格
fn multibyte_backspace_by_char_not_byte() {
    // "日本語" = 9 bytes / 3 chars
    let mut d = Document::from_text("日本語");
    assert_eq!(d.line_text(0).chars().count(), 3);
    d.set_caret(pos(0, 3));
    d.backspace();
    assert_eq!(d.line_text(0), "日本");
    d.backspace();
    assert_eq!(d.line_text(0), "日");
}

#[test]
/// 多字节字符插入列位置正确
fn multibyte_insert_at_correct_col() {
    let mut d = Document::from_text("日本");
    d.set_caret(pos(0, 1));
    // col 1 是 "日" 与 "本" 之间 -> 插入后应为 "日語本"
    d.insert(pos(0, 1), "語");
    assert_eq!(d.line_text(0), "日語本");
    assert_eq!(d.line_text(0).chars().count(), 3);
}

#[test]
/// 跨行删除含多字节字符正确
fn multibyte_cross_line_delete() {
    let mut d = Document::from_text("あ\nい");
    // 删掉换行: 从第 0 行末到第 1 行首
    d.delete_range(sakura_rs::cursor::LogicRange {
        start: pos(0, 1),
        end: pos(1, 0),
    });
    assert_eq!(d.line_count(), 1);
    assert_eq!(d.line_text(0), "あい");
}

// ---------- undo / redo ----------

#[test]
/// 插入可撤销可重做
fn insert_undo_redo() {
    let mut d = Document::from_text("abc");
    d.set_caret(pos(0, 3));
    d.insert(pos(0, 3), "d");
    assert_eq!(d.text(), "abcd");
    assert!(d.can_undo());
    assert!(d.undo());
    assert_eq!(d.text(), "abc");
    assert!(d.can_redo());
    assert!(d.redo());
    assert_eq!(d.text(), "abcd");
}

#[test]
/// 多次编辑逐次撤销
fn multiple_edits_undo_stepwise() {
    let mut d = Document::from_text("");
    d.set_caret(pos(0, 0));
    d.insert(pos(0, 0), "a");
    d.insert(pos(0, 1), "b");
    d.insert(pos(0, 2), "c");
    assert_eq!(d.text(), "abc");
    d.undo();
    assert_eq!(d.text(), "ab");
    d.undo();
    assert_eq!(d.text(), "a");
    d.undo();
    assert_eq!(d.text(), "");
}

#[test]
/// 打开文件不产生撤销记录
fn open_file_not_in_undo_stack() {
    let mut d = Document::new();
    d.set_text("loaded content");
    assert!(!d.can_undo(), "打开文件不是编辑操作, 不应可撤销");
    assert!(!d.is_dirty(), "刚打开的文件不应是 dirty");
}

#[test]
/// 编辑后 dirty, 保存恢复, 撤销再 dirty
fn dirty_before_save_dirty_after_undo() {
    let mut d = Document::from_text("abc");
    d.set_caret(pos(0, 3));
    d.insert(pos(0, 3), "d");
    assert!(d.is_dirty());
    d.mark_saved();
    assert!(!d.is_dirty());
    // 撤销后仍是 dirty (内容与磁盘不一致)
    d.undo();
    assert!(d.is_dirty());
}

#[test]
/// 空栈撤销返回 false 而非 panic
fn empty_stack_undo_returns_false() {
    let mut d = Document::from_text("abc");
    assert!(!d.can_undo());
    assert!(!d.undo(), "空栈撤销应返回 false 而非 panic");
    assert!(!d.redo());
}

// ---------- 选区 ----------

#[test]
/// 全选覆盖全文
fn select_all_covers_whole_doc() {
    let mut d = Document::from_text("abc\ndef");
    d.select_all();
    assert!(d.has_selection());
    let sel = d.selection().unwrap().normalized();
    assert_eq!(sel.start, pos(0, 0));
    assert_eq!(sel.end, pos(1, 3));
}

#[test]
/// 反向选区规范化
fn reverse_selection_normalized() {
    let _d = Document::from_text("abcdef");
    let sel = ide_kernel::document::Selection::new(pos(0, 4), pos(0, 1));
    let n = sel.normalized();
    assert_eq!(n.start, pos(0, 1));
    assert_eq!(n.end, pos(0, 4));
}

#[test]
/// 输入字符替换选区
fn type_char_replaces_selection() {
    let mut d = Document::from_text("hello world");
    d.select_all();
    d.type_char('X');
    assert_eq!(d.text(), "X");
}

// ---------- 换行与缩进 ----------

#[test]
/// 回车保留上一行缩进
fn enter_preserves_indent() {
    let mut d = Document::from_text("    indented");
    d.set_caret(pos(0, 12));
    d.insert_newline();
    assert_eq!(d.line_count(), 2);
    assert_eq!(d.line_text(1), "    ");
}

// ---------- 多光标 ----------

#[test]
/// 添加光标计数正确
fn add_cursor_count() {
    let mut d = Document::from_text("abc\ndef");
    assert_eq!(d.cursor_count(), 1);
    d.add_cursor(pos(1, 1));
    assert_eq!(d.cursor_count(), 2);
    // 重复位置不加
    d.add_cursor(pos(1, 1));
    assert_eq!(d.cursor_count(), 2);
    d.clear_extra_cursors();
    assert_eq!(d.cursor_count(), 1);
}

#[test]
/// 多光标在所有位置插入
fn multi_cursor_insert_all_positions() {
    let mut d = Document::from_text("a\nb\nc");
    // 主光标默认在 (0,0), 加两个 -> 共 3 处都应插入
    assert_eq!(d.cursor_count(), 1);
    d.add_cursor(pos(1, 0));
    d.add_cursor(pos(2, 0));
    assert_eq!(d.cursor_count(), 3);
    d.insert_all_cursors("X");
    let lines: Vec<String> = (0..d.line_count()).map(|r| d.line_text(r)).collect();
    assert_eq!(lines, vec!["Xa", "Xb", "Xc"]);
}

#[test]
/// 多光标一次撤销全部回退
fn multi_cursor_single_undo_reverts_all() {
    let mut d = Document::from_text("a\nb\nc");
    d.add_cursor(pos(1, 0));
    d.add_cursor(pos(2, 0));
    d.insert_all_cursors("X");
    assert_eq!(d.line_text(1), "Xb");
    assert!(d.undo(), "多光标插入应可撤销");
    // 一次撤销回退**所有**光标的插入, 而不是只撤一个 ——
    // 否则用户按一次 Ctrl+Z 会觉得"少撤了一处".
    assert_eq!(d.line_text(0), "a", "行0 应已回退");
    assert_eq!(d.line_text(1), "b", "行1 应已回退");
    assert_eq!(d.line_text(2), "c", "行2 应已回退");
}

// ---------- 行尾符 ----------

#[test]
/// 行尾符检测
fn line_ending_detection() {
    assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::Crlf);
    assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
    assert_eq!(LineEnding::detect("a\rb\r"), LineEnding::Cr);
}

#[test]
/// 导出按声明的行尾符
fn export_text_uses_declared_eol() {
    let mut d = Document::from_text("a\nb\nc");
    assert_eq!(d.export_text(), "a\nb\nc");
    d.set_line_ending(LineEnding::Crlf);
    assert_eq!(d.export_text(), "a\r\nb\r\nc");
    d.set_line_ending(LineEnding::Cr);
    assert_eq!(d.export_text(), "a\rb\rc");
}

#[test]
/// 装载 CRLF 内容归一化且往返无损
fn load_crlf_normalized_and_lossless() {
    let d = Document::from_text("a\r\nb\r\nc\r\n");
    assert_eq!(d.line_ending(), LineEnding::Crlf);
    // 内部按 LF 存, 导出时还原 CRLF —— 往返无损
    assert_eq!(d.export_text(), "a\r\nb\r\nc\r\n");
}

// ---------- 编码 ----------

#[test]
/// 编码标签往返
fn encoding_label_roundtrip() {
    for e in [
        Encoding::Utf8,
        Encoding::Utf8Bom,
        Encoding::Utf16Le,
        Encoding::ShiftJis,
        Encoding::EucJp,
        Encoding::Latin1,
    ] {
        let round = Encoding::from_label(e.label()).expect("应能解析");
        assert_eq!(round, e, "{} 往返失败", e.label());
    }
    assert_eq!(Encoding::from_label("不存在的编码"), None);
}

#[test]
/// 多字节编码标记
fn multibyte_encoding_flag() {
    assert!(Encoding::ShiftJis.is_multibyte());
    assert!(!Encoding::Latin1.is_multibyte());
}

// ---------- 查找替换 ----------

/// 构造查找选项
fn opts(pattern: &str, case_sensitive: bool) -> FindOptions {
    FindOptions {
        pattern: pattern.into(),
        case_sensitive,
        ..Default::default()
    }
}

#[test]
/// 查找返回第一处
fn find_returns_first_hit() {
    let d = Document::from_text("foo bar\nfoo baz");
    let r = text_ops::find_from_start(&d, &opts("foo", false)).expect("应找到");
    assert_eq!(r.start, pos(0, 0));
    assert_eq!(r.end, pos(0, 3));
}

#[test]
/// 查找大小写敏感
fn find_case_sensitivity() {
    let d = Document::from_text("Foo foo");
    assert!(text_ops::find_from_start(&d, &opts("foo", false)).is_some());
    let cs = text_ops::find_from_start(&d, &opts("Foo", true)).expect("精确匹配应找到");
    assert_eq!(cs.start, pos(0, 0));
    assert!(
        text_ops::find_from_start(&d, &opts("foo", true)).is_some(),
        "第二个 foo 也是小写, 大小写敏感也应匹配"
    );
}

#[test]
/// 空模式查找返回 None
fn find_empty_pattern_returns_none() {
    let d = Document::from_text("abc");
    assert!(text_ops::find_from_start(&d, &opts("", false)).is_none());
}

#[test]
/// 无匹配返回 None
fn find_no_match_returns_none() {
    let d = Document::from_text("abc");
    assert!(text_ops::find_from_start(&d, &opts("xyz", false)).is_none());
}

#[test]
/// 查找全部计数正确
fn find_all_count() {
    let d = Document::from_text("aXbXc\nXd");
    let all = text_ops::find_all(&d, &opts("X", true), 100);
    assert_eq!(all.len(), 3);
}

#[test]
/// 查找全部遵守上限
fn find_all_respects_limit() {
    let d = Document::from_text("XXXXX");
    let all = text_ops::find_all(&d, &opts("X", true), 2);
    assert_eq!(all.len(), 2);
}

#[test]
/// 替换当前替换一处
fn replace_current_replaces_one() {
    let mut d = Document::from_text("foo foo foo");
    let o = text_ops::replace_current(&mut d, &opts("foo", true), "bar");
    assert_eq!(o.replaced, 1);
    assert_eq!(d.text(), "bar foo foo");
}

#[test]
/// 全部替换
fn replace_all_replaces_everything() {
    let mut d = Document::from_text("cat cat\ncat");
    let o = text_ops::replace_all(&mut d, &opts("cat", true), "dog", 100);
    assert_eq!(o.replaced, 3);
    assert_eq!(d.text(), "dog dog\ndog");
}

#[test]
/// 替换后长度变化位置仍正确
fn replace_all_handles_length_change() {
    // "aXaXa" 把 X 换成 "LONG" —— 倒序执行才不会错位
    let mut d = Document::from_text("aXaXa");
    text_ops::replace_all(&mut d, &opts("X", true), "LONG", 100);
    assert_eq!(d.text(), "aLONGaLONGa");
}

#[test]
/// 全部替换可撤销
fn replace_all_undoable() {
    let mut d = Document::from_text("cat cat");
    text_ops::replace_all(&mut d, &opts("cat", true), "dog", 100);
    assert_eq!(d.text(), "dog dog");
    // delete+insert 各一条 undo 记录, 因此要撤销两次才回到原状
    let mut n = 0;
    while d.text() != "cat cat" && d.can_undo() && n < 20 {
        d.undo();
        n += 1;
    }
    assert_eq!(d.text(), "cat cat");
}

// ---------- Session ----------

#[test]
fn session_多标签切换() {
    let mut s = Session::new();
    let a = s.open_text(Some("a.txt"), "AAA");
    let b = s.open_text(Some("b.txt"), "BBB");
    assert_eq!(s.len(), 2);
    assert_eq!(s.active_id(), Some(b));
    assert_eq!(s.active().unwrap().text(), "BBB");

    assert!(s.activate(a));
    assert_eq!(s.active().unwrap().text(), "AAA");

    s.next_tab();
    assert_eq!(s.active_id(), Some(b));
    s.prev_tab();
    assert_eq!(s.active_id(), Some(a));
}

#[test]
fn session_关闭激活标签_激活相邻() {
    let mut s = Session::new();
    let a = s.open_text(Some("a.txt"), "A");
    let b = s.open_text(Some("b.txt"), "B");
    let c = s.open_text(Some("c.txt"), "C");
    assert_eq!(s.active_id(), Some(c));
    s.close(c);
    assert_eq!(s.active_id(), Some(b), "应激活前一个标签");
    s.close(b);
    assert_eq!(s.active_id(), Some(a));
    s.close(a);
    assert_eq!(s.active_id(), None);
    assert!(s.is_empty());
}

#[test]
fn session_关闭非激活标签_激活不变() {
    let mut s = Session::new();
    let a = s.open_text(Some("a.txt"), "A");
    let b = s.open_text(Some("b.txt"), "B");
    s.activate(b);
    s.close(a);
    assert_eq!(s.active_id(), Some(b));
}

#[test]
fn session_dirty_tabs_只列未保存() {
    let mut s = Session::new();
    let a = s.open_text(Some("a.txt"), "A");
    let b = s.open_text(Some("b.txt"), "B");
    s.get_mut(b).unwrap().set_caret(pos(0, 1));
    s.get_mut(b).unwrap().insert(pos(0, 1), "x");
    assert_eq!(s.dirty_tabs(), vec![b]);
    s.get_mut(a).unwrap().set_caret(pos(0, 1));
    s.get_mut(a).unwrap().insert(pos(0, 1), "y");
    assert_eq!(s.dirty_tabs(), vec![a, b]);
}

#[test]
fn session_标签状态含光标位置() {
    let mut s = Session::new();
    let a = s.open_text(Some("a.txt"), "hello");
    s.get_mut(a).unwrap().set_caret(pos(0, 3));
    let states = s.tab_states();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].path.as_deref(), Some("a.txt"));
    assert_eq!(states[0].caret_col, 3);
}

// ---------- 位置夹取 ----------

#[test]
fn clamp_超范围位置被夹取() {
    let d = Document::from_text("ab\ncd");
    assert_eq!(d.clamp_pos(pos(99, 99)), pos(1, 2));
    assert_eq!(d.clamp_pos(pos(-5, -5)), pos(0, 0));
    assert_eq!(d.clamp_pos(pos(0, 99)), pos(0, 2));
}
