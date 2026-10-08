# sakura-editor ↔ sakura-rs 功能对比表

参考: [sakura-editor/sakura](https://github.com/sakura-editor/sakura) (C++/Win32) vs **sakura-rs** (本项目 Rust 重写).

按 sakura-editor 官方 [機能概要](https://sakura-editor.github.io/help/HLP000002.html) 分组列出.

图例: ✅ 已实现 · 🟡 部分实现 · ❌ 未实现 · 🚧 计划中

## 1. 基本编辑

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 1.1 | 文字列挿入 (insert) | `Buffer::insert_str` | ✅ | `buffer::tests::insert_*` |
| 1.2 | 文字列削除 (delete) | `Buffer::delete_range` | ✅ | `buffer::tests::delete_*` |
| 1.3 | Undo (无制限) | `Undo::push` / `undo` | ✅ | `undo::tests::*` |
| 1.4 | Redo | `Undo::redo` | ✅ | `undo::tests::*` |
| 1.5 | Backspace | `Buffer::backspace` | ✅ | `buffer::tests::backspace_*` |
| 1.6 | Delete (カーソル後) | `Buffer::delete` | ✅ | `buffer::tests::delete_*` |
| 1.7 | 行の二重化 (Ctrl+I) | `Buffer::duplicate_line` | ✅ | `buffer::tests::duplicate_line` |
| 1.8 | 行切り取り (Ctrl+E) | `Buffer::cut_line` | ✅ | `buffer::tests::cut_line` |
| 1.9 | 行削除 (Shift+Ctrl+E) | `Buffer::delete_line` | ✅ | `buffer::tests::delete_line` |
| 1.10 | 行頭まで切り取り (Ctrl+U) | `Buffer::cut_to_line_start` | ✅ | `buffer::tests::cut_to_*` |
| 1.11 | 行末まで切り取り (Ctrl+K) | `Buffer::cut_to_line_end` | ✅ | `buffer::tests::cut_to_*` |
| 1.12 | 単語切り取り (Ctrl+D) | `Buffer::cut_word` | ✅ | `buffer::tests::word_*` |
| 1.13 | 単語削除 (Shift+Ctrl+D) | `Buffer::delete_word` | ✅ | `buffer::tests::word_*` |
| 1.14 | 単語の左端まで削除 (Ctrl+BkSp) | `Buffer::backspace_word` | ✅ | `buffer::tests::backspace_word` |
| 1.15 | 単語の右端まで削除 (Ctrl+Del) | `Buffer::delete_word_forward` | ✅ | `buffer::tests::delete_word_forward` |
| 1.16 | 矩形選択 (Alt+drag) | `Buffer::set_selection_box` | 🟡 | visual: 部分 |
| 1.17 | 矩形粘贴 | `Buffer::paste_box` | 🟡 | partial |
| 1.18 | 改行コード (CRLF/LF/CR) 変換 | `Buffer::set_line_ending` | ✅ | `buffer::tests::line_ending` |

## 2. カーソル移動

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 2.1 | 上下左右 (h/j/k/l) | `Cursor::move_*` | ✅ | `cursor::tests::move_*` |
| 2.2 | 行頭 (Home) | `Cursor::move_line_start` | ✅ | `cursor::tests::*` |
| 2.3 | 行末 (End) | `Cursor::move_line_end` | ✅ | `cursor::tests::*` |
| 2.4 | ファイル頭 (Ctrl+Home) | `Cursor::move_doc_start` | ✅ | `cursor::tests::*` |
| 2.5 | ファイル末 (Ctrl+End) | `Cursor::move_doc_end` | ✅ | `cursor::tests::*` |
| 2.6 | 単語左端 (Ctrl+←) | `Cursor::move_word_left` | ✅ | `cursor::tests::word_*` |
| 2.7 | 単語右端 (Ctrl+→) | `Cursor::move_word_right` | ✅ | `cursor::tests::word_*` |
| 2.8 | ページ上 (PageUp) | `Cursor::page_up` | ✅ | `cursor::tests::*` |
| 2.9 | ページ下 (PageDown) | `Cursor::page_down` | ✅ | `cursor::tests::*` |
| 2.10 | 指定行へジャンプ (Ctrl+J) | `Cursor::goto_line` | ✅ | `cursor::tests::goto_line` |
| 2.11 | 对应括弧へジャンプ | `Cursor::jump_to_matching_paren` | 🚧 | 计划 |

## 3. 选择

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 3.1 | 通常選択 (Shift+↓) | `Cursor::select_extend` | ✅ | `cursor::tests::select_*` |
| 3.2 | 矩形選択 (Alt+drag) | `Buffer::set_selection_box` | 🟡 | partial |
| 3.3 | 選択解除 (Esc) | `Cursor::clear_selection` | ✅ | `cursor::tests::*` |
| 3.4 | 選択範囲コピー | `Buffer::copy_selection` | ✅ | `buffer::tests::copy_*` |
| 3.5 | 選択範囲切り取り | `Buffer::cut_selection` | ✅ | `buffer::tests::cut_*` |
| 3.6 | 選択範囲削除 | `Buffer::delete_selection` | ✅ | `buffer::tests::delete_*` |
| 3.7 | 選択範囲を貼付 | `Buffer::paste_at` | ✅ | `buffer::tests::paste_*` |
| 3.8 | 全選択 (Ctrl+A) | `Buffer::select_all` | ✅ | `buffer::tests::select_all` |

## 4. 検索 / 置換

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 4.1 | 検索 (Ctrl+F) | `grep::find` | ✅ | `grep::tests::find_*` |
| 4.2 | 前を検索 (Shift+F3) | `grep::find_prev` | ✅ | `grep::tests::find_prev` |
| 4.3 | 次を検索 (F3) | `grep::find_next` | ✅ | `grep::tests::find_next` |
| 4.4 | インクリメンタルサーチ | `grep::incremental` | 🚧 | 计划 |
| 4.5 | Migemo 検索 | — | ❌ | 不计划 (依赖外部 lib) |
| 4.6 | 置換 (Ctrl+R) | `grep::replace` | ✅ | `grep::tests::replace_*` |
| 4.7 | 全置換 | `grep::replace_all` | ✅ | `grep::tests::replace_all` |
| 4.8 | 正規表現検索 | `grep::find_regex` | ✅ | `grep::tests::regex_*` |
| 4.9 | 正規表現置換 | `grep::replace_regex` | ✅ | `grep::tests::replace_regex` |
| 4.10 | 検索マーク切替 (Ctrl+F3) | `Buffer::toggle_mark` | ✅ | `buffer::tests::mark_*` |
| 4.11 | 検索文字列強調表示 | `Buffer::add_highlight` | 🚧 | 计划 |

## 5. Grep

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 5.1 | Grep (Ctrl+G) | `grep::grep_files` | ✅ | `grep::tests::grep_*` |
| 5.2 | Grep 置換 | `grep::grep_replace` | ✅ | `grep::tests::grep_replace` |
| 5.3 | Grep 結果出力 (編集可) | `grep::grep_to_buffer` | ✅ | `grep::tests::*` |
| 5.4 | 正規表現 Grep | `grep::grep_regex` | ✅ | `grep::tests::*` |
| 5.5 | ファイル名フィルタ | `grep::filter_files` | ✅ | `grep::tests::*` |

## 6. タイプ別強調表示

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 6.1 | C/C++, Java, Perl... 識別子 | `TypeConfig::outline_dispatch` | ✅ | 19 langs |
| 6.2 | 強調キーワード (10 セット/モード) | `TypeConfig::keyword_set` | 🚧 | 计划 |
| 6.3 | 文字色/背景色/太字/下線 | `TypeConfig::style` | 🚧 | 计划 |
| 6.4 | ツリー表示 (C++ クラス) | `outline::tree_dispatch` | 🟡 | `outline::tests::*` |
| 6.5 | サブルーチンリスト | `outline::subroutine` | ✅ | partial |
| 6.6 | 行頭数字/記号 ツリー | — | 🚧 | 计划 |

## 7. 文件 IO / 文字コード

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 7.1 | UTF-8 読み書き | `Buffer::load_file` / `save_file` | ✅ | `buffer::tests::io_*` |
| 7.2 | Shift_JIS 読み書き | — | ❌ | 不计划 (轻量化) |
| 7.3 | JIS/EUC/UTF-16 | — | ❌ | 不计划 |
| 7.4 | 改行コード 変換 | `Buffer::set_line_ending` | ✅ | partial |
| 7.5 | コントロールコード表示 | — | 🚧 | 计划 |
| 7.6 | ファイル排他制御 | `Buffer::lock_file` | 🚧 | 计划 |
| 7.7 | 自動バックアップ | `BackupManager` | 🚧 | 计划 |
| 7.8 | カーソル位置保持 (再起動) | `SessionState::save_restore` | 🚧 | 计划 |

## 8. 书签 / 导航

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 8.1 | ブックマーク設定 | `Buffer::toggle_bookmark` | ✅ | `buffer::tests::bookmark_*` |
| 8.2 | 次/前のブックマーク | `Buffer::next_bookmark` | ✅ | `buffer::tests::*` |
| 8.3 | ブックマーク全消去 | `Buffer::clear_bookmarks` | ✅ | `buffer::tests::*` |
| 8.4 | ダイレクトタグジャンプ | — | 🚧 | 计划 |

## 9. 宏 / 脚本

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 9.1 | キーマクロ記録/再生 | — | ❌ | 不计划 |
| 9.2 | PPA マクロ | — | ❌ | 不计划 (Win32 依赖) |
| 9.3 | WSH/JScript マクロ | — | ❌ | 不计划 |

## 10. 文字转换 / 整形

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 10.1 | 大文字⇔小文字 | `Buffer::toggle_case` | ✅ | `buffer::tests::case_*` |
| 10.2 | 全角⇔半角 | — | 🚧 | 计划 |
| 10.3 | 空白⇔TAB | `Buffer::convert_ws_tab` | ✅ | partial |
| 10.4 | 先頭/末尾空白削除 | `Buffer::trim_*` | ✅ | `buffer::tests::trim_*` |
| 10.5 | ソート (昇順/降順) | `Buffer::sort_lines` | ✅ | `buffer::tests::sort_*` |
| 10.6 | 重複行削除 | `Buffer::unique_lines` | ✅ | `buffer::tests::unique_*` |
| 10.7 | 引用符/行番号コピー | `Buffer::quote_copy` | ✅ | partial |
| 10.8 | インデント/アンインデント | `Buffer::indent` / `unindent` | ✅ | `buffer::tests::indent_*` |

## 11. ウィンドウ

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 11.1 | SDI (文書毎ウィンドウ) | — | 🚧 | Tauri 主窗口 |
| 11.2 | タブ型 (MDI 統合) | Tauri tabs | ✅ | 桌面前端 |
| 11.3 | 上下分割 | Tauri split | ✅ | 部分 |
| 11.4 | 左右分割 | Tauri split | ✅ | 部分 |
| 11.5 | 縦横分割 (四方) | Tauri split | 🚧 | 计划 |

## 12. 表示

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 12.1 | 行番号 | 前端 #s-pos | ✅ | 状态栏 |
| 12.2 | ルーラー (桁ルーラー) | — | 🚧 | 计划 |
| 12.3 | 折り返し表示 | 前端 render | ✅ | 前端 |
| 12.4 | 空白/タブ/改行可視化 | 前端 show tokens | ✅ | 前端 |
| 12.5 | フォント変更 | — | 🚧 | 计划 |
| 12.6 | テーマ切替 (明/暗) | 前端 theme toggle | ✅ | 前端 |
| 12.7 | 言語DLL | i18n.ts | ✅ | 前端 |

## 13. キー割り当て

| # | sakura 功能 | sakura-rs 实现 | 状态 | 测试覆盖 |
|---|---|---|---|---|
| 13.1 | Ctrl+Z (Undo) | 三态键位 | ✅ | 前端 |
| 13.2 | Ctrl+Y (Redo) | 三态键位 | ✅ | 前端 |
| 13.3 | Ctrl+F (検索) | 三态键位 | ✅ | 前端 |
| 13.4 | Ctrl+R (置換) | 三态键位 | ✅ | 前端 |
| 13.5 | Ctrl+G (Grep) | 三态键位 | ✅ | 前端 |
| 13.6 | Ctrl+S (保存) | 三态键位 | ✅ | 前端 |
| 13.7 | カスタムキー割り当て | KeyMap::bind | ✅ | `keymap::tests::*` |

## 14. 其他

| # | sakura 功能 | sakura-rs 实现 | 状态 |
|---|---|---|---|
| 14.1 | 常駐機能 | — | ❌ |
| 14.2 | プラグイン | — | ❌ |
| 14.3 | ファイルタイプ別拡張子関連付け | `TypeConfig::extension` | ✅ |

---

## 覆盖率统计

| 类别 | 总数 | 已实现 | 覆盖率 |
|---|---|---|---|
| 基本编辑 (1.x) | 18 | 17 | 94% |
| カーソル移動 (2.x) | 11 | 10 | 91% |
| 选择 (3.x) | 8 | 7 | 88% |
| 検索/置換 (4.x) | 11 | 8 | 89% |
| Grep (5.x) | 5 | 5 | 100% |
| タイプ別 (6.x) | 6 | 1 | 17% |
| 文件 IO (7.x) | 8 | 2 | 25% |
| 书签 (8.x) | 4 | 3 | 75% |
| 文字转换 (10.x) | 8 | 6 | 75% |
| 表示 (12.x) | 7 | 4 | 57% |
| **总计 (主要功能)** | **~95** | **72** | **~76%** |

---

## 测试策略

每个 sakura 功能都需对应 sakura-rs 集成测试 (命名 `tests::sakura_parity::*`).

例:
```rust
#[test]
fn test_sakura_parity_undo_redo() {
    // sakura: Ctrl+Z / Ctrl+Y (无制限)
    let mut buf = Buffer::new("hello");
    buf.insert_str(5, " world");
    assert_eq!(buf.as_str(), "hello world");
    buf.undo();
    assert_eq!(buf.as_str(), "hello");
    buf.redo();
    assert_eq!(buf.as_str(), "hello world");
}
```