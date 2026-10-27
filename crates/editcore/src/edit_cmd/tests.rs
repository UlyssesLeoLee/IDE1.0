//! 命令行为测试: 期望值来自与真实 Sakura 的比对 (tests/sakura-diff 对应用例), 通过公开的 `run` 驱动。

use super::test_support::*;
use super::{run, EditReq, ALL_COMMANDS};


const DOC: &[&str] = &["hello world foo", "  indented line", "", "third line here", "last Line END"];

// 期望值来自与真实 Sakura 的比对 (tests/sakura-diff 对应用例)
#[test]
fn single_char() {
    let r = run_on("delete", 0, 3, None, DOC);
    assert_eq!(apply(DOC, &r)[0], "helo world foo");
    let r = run_on("backspace", 0, 3, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("helo world foo", [0, 2]));
    let r = run_on("backspace", 1, 0, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("hello world foo  indented line", [0, 15]));
    let r = run_on("delete", 0, 15, None, DOC);
    assert_eq!(apply(DOC, &r)[0], "hello world foo  indented line");
    assert!(!run_on("backspace", 0, 0, None, DOC).changed);
}
#[test]
fn selection_replaced_by_delete() {
    let r = run_on("delete", 0, 8, Some([0, 2, 0, 8]), DOC);
    assert_eq!(apply(DOC, &r)[0], "herld foo");
    let r = run_on("backspace", 1, 1, Some([0, 2, 1, 1]), DOC);
    assert_eq!(apply(DOC, &r)[0], "he indented line");
    assert_eq!(r.cursor, [0, 2]);
}
#[test]
fn word_cut_and_delete() {
    let r = run_on("word-cut", 0, 2, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), (" world foo", Some("hello")));
    let r = run_on("word-delete", 0, 15, None, DOC);
    assert_eq!(apply(DOC, &r)[0], "hello world foo  indented line"); // 行末: 删掉换行
    let r = run_on("word-cut", 0, 5, None, DOC);
    assert_eq!(r.clip.as_deref(), Some(" "));
}
#[test]
fn word_to_start_end() {
    let r = run_on("delete-word-start", 0, 8, None, DOC);
    assert_eq!(apply(DOC, &r)[0], "hello rld foo");
    let r = run_on("delete-word-start", 1, 0, None, DOC);
    assert_eq!(r.cursor, [0, 15]); // 行首: 与上一行合并
    let r = run_on("delete-word-end", 0, 8, None, DOC);
    assert_eq!(apply(DOC, &r)[0], "hello wofoo");
    let r = run_on("delete-word-end", 0, 15, None, DOC);
    assert_eq!(apply(DOC, &r)[0], "hello world foo" .to_string() + "indented line"); // 行末: 合并并吃掉下一行开头的空白
}
#[test]
fn line_start_end() {
    let r = run_on("cut-line-start", 0, 5, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), (" world foo", Some("hello")));
    let r = run_on("line-cut-to-end", 0, 5, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), ("hello", Some(" world foo")));
    assert!(!run_on("line-cut-to-end", 0, 15, None, DOC).changed);
    let r = run_on("delete-line-to-end", 0, 2, Some([0, 2, 0, 4]), DOC);
    assert_eq!(apply(DOC, &r)[0], "heo world foo"); // 有选区: 只删选区
}
#[test]
fn whole_line() {
    let r = run_on("cut-line", 1, 4, None, DOC);
    assert_eq!((apply(DOC, &r).len(), r.clip.as_deref(), r.cursor), (4, Some("  indented line"), [1, 0]));
    let r = run_on("delete-line", 4, 2, None, DOC);
    assert_eq!((apply(DOC, &r).len(), r.cursor), (4, [3, 0])); // 最后一行被删: 光标落在新的最后一行
    let r = run_on("delete-line", 0, 0, None, &["only"]);
    assert_eq!(apply(&["only"], &r), [""]);
    let r = run_on("duplicate-line", 1, 4, None, DOC);
    assert_eq!((apply(DOC, &r)[1].as_str(), apply(DOC, &r)[2].as_str(), r.cursor), ("  indented line", "  indented line", [2, 4]));
}
#[test]
fn line_structure() {
    // Enter: 自动缩进 + 行尾 { [ ( 或空白再多一级
    let r = run_on("new-line", 1, 6, None, DOC);
    assert_eq!((r.insert.clone(), r.cursor), (vec!["  inde".to_string(), "  nted line".to_string()], [2, 2]));
    let d = &["fn f() {"];
    let r = run_on("new-line", 0, 8, None, d);
    assert_eq!((r.insert.clone(), r.cursor), (vec!["fn f() {".to_string(), "  ".to_string()], [1, 2]));
    let r = run_on("add-line-above", 1, 4, None, DOC);
    assert_eq!((apply(DOC, &r)[1].as_str(), r.cursor), ("  ", [1, 2]));
    let r = run_on("add-line-below", 1, 4, None, DOC);
    assert_eq!((apply(DOC, &r)[2].as_str(), r.cursor), ("  ", [2, 2]));
    let r = run_on("move-line-up", 1, 4, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("  indented line", [0, 4]));
    let r = run_on("move-line-down", 0, 4, None, DOC);
    assert_eq!((apply(DOC, &r)[1].as_str(), r.cursor), ("hello world foo", [1, 4]));
    assert!(!run_on("move-line-up", 0, 0, None, DOC).changed);
    let r = run_on("join-with-prev", 1, 0, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("hello world fooindented line", [0, 15]));
    let r = run_on("delete-line-up", 1, 3, None, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("  indented line", [0, 0]));
    let r = run_on("delete-line-down", 0, 3, None, DOC);
    assert_eq!((apply(DOC, &r)[1].as_str(), r.cursor), ("", [0, 0]));
}
#[test]
fn clipboard_copy_cut() {
    let r = run_on("copy", 0, 0, Some([0, 2, 0, 8]), DOC);
    assert_eq!((r.changed, r.clip.as_deref(), r.clip_line), (false, Some("llo wo"), false));
    let r = run_on("copy", 1, 3, None, DOC);
    assert_eq!((r.clip.as_deref(), r.clip_line), (Some("  indented line\n"), true)); // 无选区 = 整行
    let r = run_on("cut", 0, 0, Some([0, 2, 1, 3]), DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.clip.as_deref()), ("hendented line", Some("llo world foo\n  i")));
    let r = run_on("cut", 4, 2, None, DOC);
    assert_eq!((apply(DOC, &r).len(), r.cursor, r.clip_line), (4, [3, 2], true)); // 列保留
}
#[test]
fn clipboard_paste() {
    let r = run_text("paste", 0, 3, None, "XY", false, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("helXYlo world foo", [0, 5]));
    let r = run_text("paste", 1, 4, None, "ab\ncd\nef", false, DOC); // 多行粘贴不能吞掉后面的行
    let v = apply(DOC, &r);
    assert_eq!((v[1].as_str(), v[2].as_str(), v[3].as_str(), v.len(), r.cursor), ("  inab", "cd", "efdented line", 7, [3, 2]));
    let r = run_text("paste", 0, 5, Some([0, 2, 0, 5]), "XY", false, DOC); // 替换选区
    assert_eq!(apply(DOC, &r)[0], "heXY world foo");
    let r = run_text("paste", 1, 3, None, "L1\nL2\n", true, DOC); // 整行模式
    let v = apply(DOC, &r);
    assert_eq!((v[1].as_str(), v[2].as_str(), v[3].as_str(), r.cursor), ("L1", "L2", "  indented line", [3, 3]));
    assert!(!run_text("paste", 0, 0, None, "", false, DOC).changed);
}
#[test]
fn clipboard_box_and_variants() {
    let r = run_text("paste-box", 0, 3, None, "ab\ncd", false, DOC);
    let v = apply(DOC, &r);
    assert_eq!((v[0].as_str(), v[1].as_str()), ("helablo world foo", "  icdndented line"));
    let r = run_on("copy-lines", 0, 0, Some([0, 2, 1, 3]), DOC);
    assert_eq!(r.clip.as_deref(), Some("hello world foo\n  indented line\n"));
    let r = run_on("copy-passage", 0, 0, Some([0, 2, 1, 3]), DOC);
    assert_eq!(r.clip.as_deref(), Some("> hello world foo\n>   indented line\n"));
    let r = run_on("copy-linenum", 0, 0, Some([0, 2, 1, 3]), DOC);
    assert_eq!(r.clip.as_deref(), Some("  1:hello world foo\n  2:  indented line\n"));
    let r = run_on("copy-crlf", 0, 0, Some([0, 2, 1, 3]), DOC);
    assert_eq!(r.clip.as_deref(), Some("llo world foo\r\n  i"));
}
#[test]
fn select_word() {
    let r = run_on("select-word", 0, 8, None, DOC);
    assert_eq!((r.sel, r.cursor, r.changed), (Some([0, 6, 0, 11]), [0, 11], false));
    let r = run_on("select-word", 0, 5, None, DOC); // 空白段
    assert_eq!(r.sel, Some([0, 5, 0, 6]));
    let r = run_on("select-word", 0, 15, None, DOC); // 行末: 选整行 (含换行)
    assert_eq!((r.sel, r.cursor), (Some([0, 0, 1, 0]), [1, 0]));
    let r = run_on("select-word", 4, 13, None, DOC); // 最后一行行末: 选到行尾
    assert_eq!(r.sel, Some([4, 0, 4, 13]));
    assert!(run_on("select-word", 2, 0, None, DOC).sel.is_none()); // 空行
}
#[test]
fn malformed_requests_do_not_panic() {
    let r = run_on("delete", 0, 0, Some([1, 3, 0, 2]), DOC); // 反向选区
    assert_eq!(apply(DOC, &r)[0], "hendented line"); // 与正向选区 [0,2]-[1,3] 结果相同
    let r = run_text("paste-box", 9, 0, None, "x", false, DOC); // 光标行超出文档
    assert_eq!(r.remove, 0);
}
#[test]
fn line_mode_paste_and_cut_keep_column() {
    // 期望值来自真实 Sakura (tests/sakura-diff/cases10.mjs)
    let d = &["hello world foo", "  indented line", "third"];
    let r = run_text("paste", 1, 3, Some([1, 3, 1, 15]), "hello world foo\n", true, d);
    let v = apply(d, &r);
    assert_eq!((v[1].as_str(), v[2].as_str(), r.cursor), ("hello world foo", "  i", [2, 3]));
    let r = run_on("cut", 0, 3, None, d);
    assert_eq!((r.cursor, r.clip_line), ([0, 3], true));
}
#[test]
fn typing() {
    let r = run_text("insert-text", 0, 3, None, "X", false, DOC);
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("helXlo world foo", [0, 4]));
    let r = run_text("insert-text", 0, 15, None, "日本", false, DOC); // 行末追加, 光标按字符数前进
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("hello world foo日本", [0, 17]));
    let r = run_text("insert-text", 0, 3, Some([0, 2, 0, 5]), "Q", false, DOC); // 选区被替换
    assert_eq!((apply(DOC, &r)[0].as_str(), r.cursor), ("heQ world foo", [0, 3]));
    // 覆盖模式: 吃掉光标后的字符; 行末则只是追加
    let mut o = run_text("insert-text", 0, 3, None, "Z", false, DOC);
    let _ = &mut o;
    let req = EditReq { cmd: "insert-text".into(), row: 0, col: 3, sel: None, total_rows: DOC.len(), auto_indent: false, smart_indent: false, text: Some("Z".into()), line_mode: false, overwrite: true, box_sel: None, cursors: vec![], rows: vec![(0, DOC[0].to_string())] };
    assert_eq!(apply(DOC, &run(&req).unwrap())[0], "helZo world foo");
    let req = EditReq { col: 15, ..req };
    assert_eq!(apply(DOC, &run(&req).unwrap())[0], "hello world fooZ");
}
#[test]
fn rectangle() {
    const BX: &[&str] = &["abcdef", "gh", "ijklmn"];
    // 复制: 每行一段, 末尾带换行; 塌缩的矩形给空行; 反向矩形和正向一样
    let r = run_box("box-copy", 0, 4, [0, 1, 2, 4], "", BX);
    assert_eq!((r.changed, r.clip.as_deref()), (false, Some("bcd\nh\njkl\n")));
    assert_eq!(run_box("box-copy", 2, 1, [2, 4, 0, 1], "", BX).clip.as_deref(), Some("bcd\nh\njkl\n"));
    assert_eq!(run_box("box-copy", 0, 2, [0, 2, 1, 2], "", BX).clip.as_deref(), Some("\n\n"));
    // 删除: 整块挖空, 光标回矩形左上角, 列块清除; 退格和删除一样 (Sakura 实测), 零宽矩形什么也不删
    let r = run_box("box-delete", 2, 4, [0, 1, 2, 4], "", BX);
    assert_eq!((apply(BX, &r), r.cursor, r.box_sel), (vec!["aef".to_string(), "g".into(), "imn".into()], [0, 1], None));
    assert_eq!(apply(BX, &run_box("box-backspace", 2, 4, [0, 1, 2, 4], "", BX)), apply(BX, &r));
    let r = run_box("box-backspace", 0, 2, [0, 2, 2, 2], "", BX);
    assert_eq!((apply(BX, &r), r.cursor, r.box_sel), (BX.iter().map(|s| s.to_string()).collect::<Vec<_>>(), [0, 2], None));
    // 剪切 = 复制 + 删除
    let r = run_box("box-cut", 2, 4, [0, 1, 2, 4], "", BX);
    assert_eq!((apply(BX, &r)[0].as_str(), r.clip.as_deref()), ("aef", Some("bcd\nh\njkl\n")));
    // 输入: 同一份插到每一行的矩形左端, 短行补空格对齐, 矩形里选中的内容被覆盖, 插完矩形缩成插入点
    let r = run_box("box-insert", 0, 3, [0, 3, 2, 3], "X", BX);
    assert_eq!((apply(BX, &r), r.box_sel), (vec!["abcXdef".to_string(), "gh X".into(), "ijkXlmn".into()], Some([0, 4, 2, 4])));
    let r = run_box("box-insert", 0, 3, [0, 1, 2, 3], "Z", BX);
    assert_eq!((apply(BX, &r)[0].as_str(), apply(BX, &r)[2].as_str(), r.box_sel), ("aZdef", "iZlmn", Some([0, 2, 2, 2])));
    // 多行剪贴板只取第一行 (Sakura 实测)
    let r = run_box("box-insert", 0, 0, [0, 0, 2, 0], "1\n2", BX);
    assert_eq!(apply(BX, &r), vec!["1abcdef".to_string(), "1gh".into(), "1ijklmn".into()]);
    // 回车: 先删矩形内容, 再在左上角换行 (只断一行)
    let r = run_box("box-enter", 2, 4, [0, 1, 2, 4], "", BX);
    assert_eq!((apply(BX, &r), r.cursor, r.box_sel), (vec!["a".to_string(), "ef".into(), "g".into(), "imn".into()], [1, 0], None));
    // 替换: 等宽, 不足补空格, 超宽截断
    let r = run_box("box-replace", 0, 4, [0, 1, 2, 4], "12\nabcdef", BX);
    assert_eq!(apply(BX, &r), vec!["a12 ef".to_string(), "gabc".into(), "i   mn".into()]);
    // 列越过行尾 (End 键) 会夹在参与行的最长长度内
    let r = run_box("box-delete", 0, 0, [0, 3, 1, 9999], "", BX);
    assert_eq!(apply(BX, &r)[..2], ["abc".to_string(), "gh".into()]);
    // 矩形落在文档末尾之外 (文档缩短后留下的旧矩形): 夹到最后一行, 行没交全就什么也不做, 不会把最后一行当空行写回去
    let mut req = EditReq { cmd: "box-insert".into(), row: 0, col: 0, sel: None, total_rows: 3, auto_indent: false, smart_indent: false, text: Some("X".into()), line_mode: false, overwrite: false, box_sel: Some([5, 0, 6, 0]), cursors: vec![], rows: vec![(0, "abcdef".into()), (1, "gh".into())] };
    assert!(!run(&req).unwrap().changed);
    req.rows.push((2, "ijklmn".into()));
    assert_eq!(apply(BX, &run(&req).unwrap())[2], "Xijklmn");
}
#[test]
fn multi_cursor() {
    const M: &[&str] = &["abcdef", "gh", "ijklmn"];
    // 不同行: 每个光标处插入, 光标各自右移
    let r = run_multi("multi-insert", &[[0, 1], [2, 3]], "X", M);
    assert_eq!((apply_multi(M, &r), r.cursors.clone()), (vec!["aXbcdef".to_string(), "gh".into(), "ijkXlmn".into()], Some(vec![[0, 2], [2, 4]])));
    // 同一行多个光标: 后面的光标要把前面的插入也算进去
    let r = run_multi("multi-insert", &[[0, 1], [0, 3]], "Q", M);
    assert_eq!((apply_multi(M, &r)[0].as_str(), r.cursors.clone()), ("aQbcQdef", Some(vec![[0, 2], [0, 5]])));
    // 汉字 / 多个字符
    let r = run_multi("multi-insert", &[[0, 0], [1, 2]], "日本", M);
    assert_eq!((apply_multi(M, &r)[..2].to_vec(), r.cursors.clone()), (vec!["日本abcdef".to_string(), "gh日本".into()], Some(vec![[0, 2], [1, 4]])));
    // 退格: 同行两个光标各删前一个字符
    let r = run_multi("multi-backspace", &[[0, 2], [0, 4]], "", M);
    assert_eq!((apply_multi(M, &r)[0].as_str(), r.cursors.clone()), ("acef", Some(vec![[0, 1], [0, 2]])));
    // 行首退格 = 与上一行合并, 光标落在合并处; 下面的行跟着上移 (光标行号按合并后的算)
    let r = run_multi("multi-backspace", &[[1, 0], [2, 2]], "", M);
    assert_eq!((apply_multi(M, &r), r.cursors.clone()), (vec!["abcdefgh".to_string(), "ikl".to_string() + "mn"], Some(vec![[0, 6], [1, 1]])));
    // 合并后两个光标重合 -> 去重
    let r = run_multi("multi-backspace", &[[0, 3], [1, 0]], "", &["abc", "def"]);
    assert_eq!((apply_multi(&["abc", "def"], &r), r.cursors.clone()), (vec!["abdef".to_string()], Some(vec![[0, 2]])));
    // 都在 (0,0): 没有可删的
    assert!(!run_multi("multi-backspace", &[[0, 0], [0, 0]], "", M).changed);
    // 光标隔得很远: 只交光标所在的行, 各出一个补丁; 前一段并行少掉的行数要算进后一段的光标行号
    let far: Vec<String> = (0..1000).map(|i| format!("l{i}")).collect();
    let far_ref: Vec<&str> = far.iter().map(|s| s.as_str()).collect();
    let r = run_multi("multi-backspace", &[[1, 0], [900, 2]], "", &far_ref);
    let p = r.patches.clone().unwrap();
    assert_eq!(p.len(), 2, "两段各一个补丁, 中间的 898 行不用交");
    assert_eq!(r.cursors, Some(vec![[0, 2], [899, 1]]));
    let applied = apply_multi(&far_ref, &r);
    assert_eq!((applied[0].as_str(), applied.len(), applied[899].as_str()), ("l0l1", 999, "l00"));
    // 光标行没交全 / 光标在文档末尾之外: 夹进文档, 行不全就什么也不做
    let r = run(&EditReq { cmd: "multi-insert".into(), row: 0, col: 0, sel: None, total_rows: 3, auto_indent: false, smart_indent: false, text: Some("X".into()), line_mode: false, overwrite: false, box_sel: None, cursors: vec![[8, 5], [9, 5]], rows: vec![(2, "abc".into())] }).unwrap();
    assert!(!r.changed, "两个旧光标都夹到最后一行行尾 -> 去重后不足两个");
    let r = run(&EditReq { cmd: "multi-insert".into(), row: 0, col: 0, sel: None, total_rows: 3, auto_indent: false, smart_indent: false, text: Some("X".into()), line_mode: false, overwrite: false, box_sel: None, cursors: vec![[0, 0], [2, 0]], rows: vec![(0, "a".into())] }).unwrap();
    assert!(!r.changed, "光标所在的行没交全 -> 什么也不做 (不把真实内容当空行写回去)");
}
#[test]
fn folding() {
    let rows = |ls: &[&str], from: usize| -> Vec<(usize, String)> { ls.iter().enumerate().map(|(i, s)| (from + i, s.to_string())).collect() };
    let req = |cmd: &str, row: usize, ls: &[&str]| EditReq { cmd: cmd.into(), row, col: 0, sel: None, total_rows: 20, auto_indent: false, smart_indent: false, text: None, line_mode: false, overwrite: false, box_sel: None, cursors: vec![], rows: rows(ls, row) };
    // 花括号块 (含嵌套): 从 fn outer 配到最外层的 }
    let code = ["fn outer() {", "    fn inner() {", "        x();", "    }", "    inner();", "}", "fn other() {", "    y();", "}"];
    assert_eq!(run(&req("fold-range", 0, &code)).unwrap().folds, Some(vec![[0, 5]]));
    assert_eq!(run(&req("fold-range", 1, &code[1..])).unwrap().folds, Some(vec![[1, 3]]));
    // 缩进块 (没有花括号): 下一行更深才算; 末尾空行不吞; 顶格也能折
    let py = ["def f():", "    a = 1", "", "    b = 2", "", "x = 3"];
    assert_eq!(run(&req("fold-range", 0, &py)).unwrap().folds, Some(vec![[0, 3]]));
    // 同级的下一行不是块
    assert_eq!(run(&req("fold-range", 1, &py[1..])).unwrap().folds, Some(vec![]));
    // 字符串 / 字符字面量 / 行注释里的花括号不算
    let lit = ["fn f() {", "    let c = '{';", "    let s = \"}\"; // {", "    g();", "}"];
    assert_eq!(run(&req("fold-range", 1, &lit[1..])).unwrap().folds, Some(vec![]), "字面量里的花括号不开块");
    assert_eq!(run(&req("fold-range", 0, &lit)).unwrap().folds, Some(vec![[0, 4]]));
    // 括号一直配不上: 到文档末尾
    assert_eq!(run(&req("fold-range", 0, &["if (a) {", "  b();"])).unwrap().folds, Some(vec![[0, 1]]));
    // 全部折叠: 每个能折的起点各一项 (含嵌套)
    let all = run(&req("fold-all", 0, &code)).unwrap();
    assert_eq!((all.changed, all.folds), (false, Some(vec![[0, 5], [1, 3], [6, 8]])));
}
#[test]
fn unknown() {
    assert!(run(&EditReq { cmd: "nope".into(), row: 0, col: 0, sel: None, total_rows: 1, auto_indent: false, smart_indent: false, text: None, line_mode: false, overwrite: false, box_sel: None, cursors: vec![], rows: vec![] }).is_none());
}

#[test]
fn every_listed_command_is_known_and_unlisted_ones_are_not() {
    for cmd in ALL_COMMANDS {
        assert!(run(&req(cmd, 0, 0).with_doc(&["ab", "cd"]).with_cursors(&[[0, 0], [1, 0]]).with_box([0, 0, 1, 1])).is_some(), "命令表里缺 {cmd}");
    }
    assert!(run(&req("nope", 0, 0)).is_none());
}
