//! ide-shell ↔ ide-kernel 接入测试.
//!
//! 验证 TUI shell 的文件编辑确实走 `ide_kernel::Document` 而非单行
//! `InputBuffer`: 关键证据是 **undo 可用** 与 **char 语义正确** ——
//! `InputBuffer` 两者都不具备, 所以这些断言能真正区分"接了内核"与"没接".
//!
//! 测试函数名用 ASCII —— MSVC linker 不接受非 ASCII 符号.

use ide_shell::app::{App, AppConfig};
use ide_shell::mode::Mode;
use std::io::Write;

fn tmp_file(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("ide-shell-kernel-test");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    p
}

fn key(app: &mut App, c: char) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

fn backspace(app: &mut App) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
}

fn enter_insert(app: &mut App) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
}

// ---------- 接入基本行为 ----------

#[test]
fn open_file_loads_into_kernel_document() {
    let p = tmp_file("open_basic.txt", "hello\nworld");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).expect("应能打开");
    assert!(app.has_document());
    let d = app.document().unwrap();
    assert_eq!(d.text(), "hello\nworld");
    assert_eq!(d.line_count(), 2);
    assert_eq!(app.mode(), Mode::Normal, "打开后应在 Normal 模式");
}

#[test]
fn open_missing_file_reports_error() {
    let mut app = App::new(AppConfig::default());
    let r = app.open_file("不存在的文件.xyz");
    assert!(r.is_err(), "不存在的文件应报错");
    assert!(!app.has_document());
}

#[test]
fn typing_goes_to_document_not_command_buffer() {
    let p = tmp_file("typing.txt", "AB");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    enter_insert(&mut app);
    key(&mut app, 'C');
    // 光标在打开时落在文档末尾, 因此插到末尾
    assert_eq!(app.document().unwrap().text(), "ABC");
    // 命令行 buffer 保持空 —— 这是"接了内核"的关键区分点
    assert!(app.buffer().is_empty(), "编辑不应进入命令行 buffer");
}

#[test]
fn without_document_typing_still_goes_to_command_buffer() {
    let mut app = App::new(AppConfig::default());
    enter_insert(&mut app);
    key(&mut app, 'x');
    assert_eq!(app.buffer().as_str(), "x", "未打开文件时保持原 REPL 行为");
    assert!(!app.has_document());
}

// ---------- 内核特性确实生效 ----------

#[test]
fn document_edits_are_undoable() {
    let p = tmp_file("undo.txt", "abc");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    enter_insert(&mut app);
    key(&mut app, 'd');
    assert_eq!(app.document().unwrap().text(), "abcd");
    // InputBuffer 没有 undo, 有 undo 就证明走了内核
    assert!(app.document_mut().unwrap().undo(), "文档编辑应可撤销");
    assert_eq!(app.document().unwrap().text(), "abc");
}

#[test]
fn multibyte_edit_uses_char_semantics() {
    let p = tmp_file("multibyte.txt", "日本");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    // 光标移到行尾 (Normal 模式按 End)
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    enter_insert(&mut app);
    key(&mut app, '語');
    assert_eq!(app.document().unwrap().line_text(0), "日本語");
    backspace(&mut app);
    assert_eq!(
        app.document().unwrap().line_text(0),
        "日本",
        "退格按 char 而非 byte"
    );
}

#[test]
fn document_tracks_dirty_state() {
    let p = tmp_file("dirty.txt", "abc");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    assert!(
        !app.document().unwrap().is_dirty(),
        "刚打开的文件不应 dirty"
    );
    enter_insert(&mut app);
    key(&mut app, 'X');
    assert!(app.document().unwrap().is_dirty(), "编辑后应 dirty");
}

#[test]
fn line_ending_detected_and_preserved_on_save() {
    let p = tmp_file("crlf.txt", "a\r\nb\r\nc");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    assert_eq!(app.document().unwrap().line_count(), 3);
    assert_eq!(
        app.document().unwrap().line_ending(),
        ide_kernel::document::LineEnding::Crlf
    );
    app.save_file().unwrap();
    // 往返无损: CRLF 仍是 CRLF
    let back = std::fs::read_to_string(&p).unwrap();
    assert_eq!(back, "a\r\nb\r\nc", "保存应保持原行尾符");
}

// ---------- 保存 ----------

#[test]
fn save_writes_back_to_disk() {
    let p = tmp_file("save.txt", "original");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    enter_insert(&mut app);
    key(&mut app, '!');
    assert!(app.document().unwrap().is_dirty());
    app.save_file().unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "original!");
    assert!(!app.document().unwrap().is_dirty(), "保存后应清除 dirty");
    assert!(app.document().unwrap().can_undo(), "保存不应清空 undo 历史");
}

#[test]
fn save_without_document_errors() {
    let mut app = App::new(AppConfig::default());
    assert!(app.save_file().is_err());
}

// ---------- 命令模式不受影响 ----------

#[test]
fn command_mode_still_uses_command_buffer() {
    let p = tmp_file("cmdmode.txt", "abc");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    // 进入 Command 模式后输入不应改文档
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
    key(&mut app, 'w');
    assert_eq!(
        app.document().unwrap().text(),
        "abc",
        "Command 模式不应编辑文档"
    );
}

#[test]
fn open_command_loads_file() {
    let p = tmp_file("viacmd.txt", "via command");
    let mut app = App::new(AppConfig::default());
    app.run_command(&format!("open {}", p.display()));
    assert!(app.has_document());
    assert_eq!(app.document().unwrap().text(), "via command");
}

#[test]
fn undo_command_reverts_edit() {
    let p = tmp_file("undocmd.txt", "base");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    enter_insert(&mut app);
    key(&mut app, 'Z');
    assert_eq!(app.document().unwrap().text(), "baseZ");
    app.run_command("u");
    assert_eq!(app.document().unwrap().text(), "base");
}

// ---------- 边界 ----------

#[test]
fn open_empty_file_yields_single_empty_line() {
    let p = tmp_file("empty.txt", "");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    assert_eq!(app.document().unwrap().line_count(), 1);
    assert_eq!(app.document().unwrap().text(), "");
}

#[test]
fn move_down_crosses_line_boundary() {
    // 三行文件, 打开后光标在第 2 行 (末行) 行尾
    let p = tmp_file("cross.txt", "ab\ncd\nef");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    assert_eq!(app.document().unwrap().caret().row, 2, "打开后光标应在末行");
    // 移到行首再右移 → 下一行行首
    app.on_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    let c = app.document().unwrap().caret();
    assert_eq!(c.row, 2, "行首右移应到下一行");
    assert_eq!(c.col, 1);
}

#[test]
fn move_left_at_line_start_goes_to_prev_line_end() {
    let p = tmp_file("crossleft.txt", "ab\ncd");
    let mut app = App::new(AppConfig::default());
    app.open_file(p.to_str().unwrap()).unwrap();
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    // 打开后光标在末行行尾 (col 2); 回行首再左移
    assert_eq!(app.document().unwrap().caret().row, 1);
    app.on_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    assert_eq!(app.document().unwrap().caret().col, 0);
    app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    let c = app.document().unwrap().caret();
    assert_eq!(c.row, 0, "行首左移应到上一行");
    assert_eq!(c.col, 2, "应落在上一行末尾");
}

// ---------- 命令名匹配边界 ----------

/// 回归: `:e` 曾用 `strip_prefix('e')` 实现, 把 `echo hello` 劫持成
/// "打开文件 hello" —— 未知命令的子进程兜底因此失效.
#[test]
fn command_named_e_does_not_swallow_echo() {
    let mut app = App::new(AppConfig::default());
    app.run_command("echo hello");
    let hist = app.history().last().cloned().unwrap_or_default();
    assert!(
        hist.contains("hello"),
        "echo 应走子进程兜底并输出 hello, 实际 history: {hist:?}"
    );
    assert!(!hist.contains("open 失败"), "echo 不应被当成 open");
}

#[test]
fn command_named_e_does_not_swallow_env_or_exit() {
    for cmd in ["env", "exit", "echo"] {
        let mut app = App::new(AppConfig::default());
        app.run_command(cmd);
        let hist = app.history().last().cloned().unwrap_or_default();
        assert!(
            !hist.contains("open 失败") && !hist.contains("usage: :open"),
            "{cmd} 被误判成 :open, history: {hist:?}"
        );
    }
}

#[test]
fn open_command_with_real_path_still_works() {
    let p = tmp_file("prefix_ok.txt", "content");
    let mut app = App::new(AppConfig::default());
    app.run_command(&format!("open {}", p.display()));
    assert!(app.has_document());
    assert_eq!(app.document().unwrap().text(), "content");
}

#[test]
fn open_without_argument_shows_usage() {
    let mut app = App::new(AppConfig::default());
    app.run_command("open");
    let hist = app.history().last().cloned().unwrap_or_default();
    assert!(hist.contains("usage"), "应提示用法, 实际: {hist:?}");
    assert!(!app.has_document());
}
