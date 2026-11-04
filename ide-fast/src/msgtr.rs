//! 状态栏提示 / 面板消息的显示期翻译。
//! 这些消息在代码里是现拼的中文 (`format!("已保存 {n} 个文件")`), 分散在几十处; 与其改每个调用点, 不如在**显示时**按模板翻译:
//! 模板 = 中文原文里把可变部分写成 `{}`, 匹配到就取出各段可变内容 (它们自己也会再翻译一次, 嵌套的提示因此也会被译), 填进日 / 英模板。
//! 没匹配到 = 原样返回 (宁可显示中文也不显示空白)。中文界面直接返回原文, 不做匹配。

use crate::i18n::{cur, Lang};
use std::borrow::Cow;
use std::sync::OnceLock;

/// (中文模板, 日文模板, 英文模板); `{}` 依次对应
type Row = (&'static str, &'static str, &'static str);

const MSG_T: &[Row] = &[
    // ---- 文档 / 编辑
    ("已从磁盘重新载入", "ディスクから再読み込みしました", "Reloaded from disk"),
    ("已按 {} 重新打开", "{} で開き直しました", "Reopened as {}"),
    ("保存时的换行符: {}", "保存時の改行コード: {}", "Line ending on save: {}"),
    ("保存时的编码: {}", "保存時の文字コード: {}", "Encoding on save: {}"),
    ("只有 UTF-8 可以选择带不带 BOM", "BOM の有無を選べるのは UTF-8 だけです", "Only UTF-8 can choose whether to write a BOM"),
    ("保存时带 BOM", "保存時に BOM を付けます", "A BOM will be written on save"),
    ("保存时不带 BOM", "保存時に BOM を付けません", "No BOM will be written on save"),
    ("没有对应的括弧", "対応する括弧がありません", "No matching bracket"),
    ("只读: 不能修改 (编辑 → 只读 可以关掉)", "読み取り専用: 変更できません (編集 → 読み取り専用 で解除)", "Read-only: cannot modify (turn off via Edit > Read-only)"),
    ("未知命令: {}", "不明なコマンド: {}", "Unknown command: {}"),
    ("已处理 {} 行", "{} 行を処理しました", "Processed {} line(s)"),
    ("先选中要转换的文字", "変換する文字を先に選択してください", "Select the text to convert first"),
    ("范围选择模式: 关", "範囲選択モード: オフ", "Range selection mode: off"),
    ("范围选择模式: 用方向键扩选 (F6 / Esc 结束)", "範囲選択モード: 矢印キーで拡張 (F6 / Esc で終了)", "Range selection mode: extend with arrow keys (F6 / Esc to end)"),
    ("已插入戳记: {}", "スタンプを挿入しました: {}", "Inserted stamp: {}"),
    ("没有可撤销的操作", "元に戻す操作はありません", "Nothing to undo"),
    ("没有可重做的操作", "やり直す操作はありません", "Nothing to redo"),
    ("已设置书签", "ブックマークを設定しました", "Bookmark set"),
    ("已取消书签", "ブックマークを解除しました", "Bookmark cleared"),
    ("书签 {}/{}", "ブックマーク {}/{}", "Bookmark {}/{}"),
    ("没有书签", "ブックマークはありません", "No bookmarks"),
    ("宏录制结束 ({} 步) — Ctrl+Shift+L 回放", "マクロの記録を終了 ({} ステップ) — Ctrl+Shift+L で再生", "Macro recorded ({} steps) - Ctrl+Shift+L to replay"),
    ("● 录制宏… 再按 Ctrl+Shift+M 结束", "● マクロ記録中… もう一度 Ctrl+Shift+M で終了", "● Recording macro... press Ctrl+Shift+M again to stop"),
    ("还没有录下的宏", "記録済みのマクロがありません", "No recorded macro yet"),
    ("宏已回放", "マクロを再生しました", "Macro replayed"),
    ("{} (已折返)", "{} (折り返し)", "{} (wrapped)"),
    ("没有找到: {}", "見つかりません: {}", "Not found: {}"),
    ("已到文件末尾 (未折返)", "ファイルの末尾です (折り返しなし)", "Reached end of file (no wrap)"),
    ("已到文件开头 (未折返)", "ファイルの先頭です (折り返しなし)", "Reached start of file (no wrap)"),
    ("只读文档不能置换", "読み取り専用の文書は置換できません", "Cannot replace in a read-only document"),
    ("置换了 1 处; 后面没有了", "1 件置換しました。以降はありません", "Replaced 1; no more after this"),
    ("置换了 1 处; {}", "1 件置換しました; {}", "Replaced 1; {}"),
    ("置换了 {} 处", "{} 件置換しました", "Replaced {}"),
    ("{} · {} · 已读取", "{} · {} · 読み込み済み", "{} · {} · loaded"),
    ("{} · {} · 有无法解码的字节", "{} · {} · デコードできないバイトあり", "{} · {} · undecodable bytes present"),
    ("已设为只读", "読み取り専用にしました", "Set to read-only"),
    ("已取消只读", "読み取り専用を解除しました", "Read-only cleared"),
    ("改写模式", "上書きモード", "Overwrite mode"),
    ("插入模式", "挿入モード", "Insert mode"),
    ("这个类型没有行注释符", "この種類には行コメント記号がありません", "This type has no line-comment marker"),
    ("这一行不是命中行", "この行はヒット行ではありません", "This line is not a hit line"),
    // ---- 文件 / 标签
    ("磁盘上的文件已被删除", "ディスク上のファイルは削除されました", "The file on disk was deleted"),
    ("已保存 {} 个文件", "{} 個のファイルを保存しました", "Saved {} file(s)"),
    ("还没有保存过, 不能按别的编码重新打开", "未保存のため、別の文字コードで開き直せません", "Not saved yet; cannot reopen with another encoding"),
    ("还没有保存过, 没有可重新载入的文件", "未保存のため、再読み込みするファイルがありません", "Not saved yet; nothing to reload"),
    ("还没有保存过, 没有可重新打开的文件", "未保存のため、開き直すファイルがありません", "Not saved yet; nothing to reopen"),
    ("还没有保存过", "まだ保存されていません", "Not saved yet"),
    ("打不开: {}", "開けません: {}", "Cannot open: {}"),
    ("读不了 {}: {}", "{} を読み込めません: {}", "Cannot read {}: {}"),
    ("开不了新窗口: {}", "新しいウィンドウを開けません: {}", "Cannot open a new window: {}"),
    ("没有更早的移动履历", "これより前の移動履歴はありません", "No earlier jump history"),
    ("没有更新的移动履历", "これより後の移動履歴はありません", "No later jump history"),
    ("文件超过 8 MB", "ファイルが 8 MB を超えています", "File is larger than 8 MB"),
    ("文件太大 ({} MB, 上限 {} MB): 高速版把整篇载入内存, 请用专门的大文件查看器", "ファイルが大きすぎます ({} MB, 上限 {} MB): 高速版は全体をメモリに読み込みます。大容量ファイル用ビューアを使ってください", "File too large ({} MB, limit {} MB): the fast edition loads the whole file into memory; use a dedicated large-file viewer"),
    ("看起来是二进制文件 (含 NUL 字节), 不按文本打开", "バイナリファイルのようです (NUL バイトを含む)。テキストとしては開きません", "Looks like a binary file (contains NUL bytes); not opened as text"),
    // ---- 比较 / 宏 / 预览
    ("{}: 没有可显示的差异 (未跟踪文件?)", "{}: 表示できる差分がありません (未追跡ファイル?)", "{}: no differences to show (untracked file?)"),
    ("比较 {} ↔ {}: {}", "比較 {} ↔ {}: {}", "Compare {} <-> {}: {}"),
    ("没有进行中的比较", "進行中の比較はありません", "No comparison in progress"),
    ("当前标签不在比较里", "現在のタブは比較の対象ではありません", "The current tab is not part of the comparison"),
    ("两份文档完全相同", "2 つの文書は完全に同じです", "The two documents are identical"),
    ("{} 处差异", "{} 件の差分", "{} difference(s)"),
    ("读取宏失败: {}", "マクロの読み込みに失敗: {}", "Failed to read macro: {}"),
    ("宏已执行 ({} 步)", "マクロを実行しました ({} ステップ)", "Macro ran ({} steps)"),
    ("宏未执行: {}", "マクロを実行できませんでした: {}", "Macro not run: {}"),
    ("维基链接 [[{}]]: 预览里不跳转", "Wiki リンク [[{}]]: プレビューでは移動しません", "Wiki link [[{}]]: not followed in the preview"),
    ("没有找到标题 #{}", "見出し #{} が見つかりません", "Heading #{} not found"),
    ("不打开此链接: {}", "このリンクは開きません: {}", "Not opening this link: {}"),
    ("只能打开 http / https / mailto 链接", "http / https / mailto のリンクだけ開けます", "Only http / https / mailto links can be opened"),
    ("无法打开链接: {}", "リンクを開けません: {}", "Cannot open link: {}"),
    // ---- 查找 / Grep
    ("没有可找的内容", "検索する内容がありません", "Nothing to search for"),
    ("{}+ 处", "{}+ 件", "{}+ hits"),
    ("{} 处", "{} 件", "{} hit(s)"),
    ("已停止, {} 处命中", "停止しました。{} 件ヒット", "Stopped, {} hit(s)"),
    ("找不到搜索位置", "検索場所が見つかりません", "Cannot find a place to search"),
    ("请输入要搜索的内容", "検索する内容を入力してください", "Enter something to search for"),
    ("还没有搜索结果", "検索結果がまだありません", "No search results yet"),
    ("{} 处命中 (已达 {} 上限)", "{} 件ヒット (上限 {} に到達)", "{} hit(s) (limit {} reached)"),
    ("{} 处命中", "{} 件ヒット", "{} hit(s)"),
    ("搜索中…  (Esc 停止)", "検索中…  (Esc で停止)", "Searching...  (Esc to stop)"),
    ("搜索中… {} 处  (Esc 停止)", "検索中… {} 件  (Esc で停止)", "Searching... {} hit(s)  (Esc to stop)"),
    ("置换 {} 处, 改写 {} 个文件 (已备份为 .bak){}", "{} 件を置換、{} 個のファイルを書き換えました (.bak に退避){}", "Replaced {} in {} file(s) (backed up as .bak){}"),
    (", 跳过 {} 个有未保存修改的已打开文件", ", 未保存の変更がある開いたファイル {} 個をスキップ", ", skipped {} open file(s) with unsaved changes"),
    (", {} 个失败", ", {} 個失敗", ", {} failed"),
    ("{}; 已重新载入 {} 个打开的标签", "{}; 開いているタブ {} 個を再読み込みしました", "{}; reloaded {} open tab(s)"),
    // ---- 侧栏 / Git
    ("名字不能为空", "名前を空にできません", "The name cannot be empty"),
    ("名字里不能有 / \\ : * ? \" < > |", "名前に / \\ : * ? \" < > | は使えません", "The name cannot contain / \\ : * ? \" < > |"),
    ("名字不能以点或空格结尾", "名前の末尾にドットや空白は使えません", "The name cannot end with a dot or a space"),
    ("同名文件或文件夹已存在", "同名のファイルまたはフォルダーが既にあります", "A file or folder with that name already exists"),
    ("新建失败: {}", "作成に失敗: {}", "Create failed: {}"),
    ("重命名失败: {}", "名前の変更に失敗: {}", "Rename failed: {}"),
    ("删除失败: {}", "削除に失敗: {}", "Delete failed: {}"),
    ("不是 Git 仓库", "Git リポジトリではありません", "Not a Git repository"),
    ("无法运行 git: {}", "git を実行できません: {}", "Cannot run git: {}"),
    ("这个仓库的本地配置定义了 filter / diff 外部命令, 高速版不自动运行 git (请在终端里操作)", "このリポジトリのローカル設定に filter / diff の外部コマンドがあるため、高速版は git を自動実行しません (ターミナルで操作してください)", "This repository's local config defines external filter / diff commands, so the fast edition will not run git automatically (use the terminal)"),
    ("分支 {}", "ブランチ {}", "Branch {}"),
    // ---- 插件 / 其它
    ("插件命令失败 (编辑器不受影响): {}", "プラグインコマンドが失敗しました (エディターには影響なし): {}", "Plugin command failed (the editor is unaffected): {}"),
    ("插件宿主已停止", "プラグインホストが停止しました", "The plugin host has stopped"),
    ("无法启动 shell: {}", "シェルを起動できません: {}", "Cannot start shell: {}"),
    // ---- 补全翻译 (0.3.8)
    ("光标处没有单词", "カーソル位置に単語がありません", "No word at the cursor"),
    ("关键字帮助自动显示: 开", "キーワードヘルプの自動表示: オン", "Auto-show keyword help: on"),
    ("关键字帮助自动显示: 关", "キーワードヘルプの自動表示: オフ", "Auto-show keyword help: off"),
    ("帮助: {}", "ヘルプ: {}", "Help: {}"),
    ("只有一个分组", "グループは 1 つだけです", "There is only one group"),
    ("分组: {}", "グループ: {}", "Group: {}"),
    ("已移到分组: {}", "グループへ移動しました: {}", "Moved to group: {}"),
    ("颜色分组都已经有标签在用", "すべての色グループが既に使われています", "Every color group already has tabs"),
    ("全部标签已合并为一组", "すべてのタブを 1 つのグループに統合しました", "All tabs merged into one group"),
    ("没有需要备份的未保存修改", "バックアップが必要な未保存の変更はありません", "No unsaved changes to back up"),
    ("已备份 {} 个文档", "{} 個の文書をバックアップしました", "Backed up {} document(s)"),
    ("备份失败: {}", "バックアップに失敗: {}", "Backup failed: {}"),
    ("选区统计: {}", "選択範囲の集計: {}", "Selection count: {}"),
    ("全部强调: {}", "すべて強調: {}", "Highlight all: {}"),
    ("还没有查找起点 (先查找一次)", "検索開始位置がありません (先に検索してください)", "No find start yet (search once first)"),
    ("已复制 {}", "コピーしました: {}", "Copied {}"),
    ("已在折行位置 (每 {} 字符) 加换行后复制", "折り返し位置 ({} 文字ごと) に改行を入れてコピーしました", "Copied with line breaks at the wrap (every {} chars)"),
    ("这个文档还没有保存, 没有同名文件可找", "この文書は未保存のため、同名のファイルを探せません", "This document is not saved yet, so there is no matching file to look for"),
    ("没有找到对应的 C/C++ 头 / 源文件", "対応する C/C++ ヘッダー / ソースが見つかりません", "No matching C/C++ header / source file found"),
    ("无补全候选 (前缀: {})", "補完候補なし (接頭辞: {})", "No completion candidates (prefix: {})"),
    ("光标处无标识符", "カーソル位置に識別子がありません", "No identifier at the cursor"),
    ("未找到 {} 的定义", "{} の定義が見つかりません", "Definition of {} not found"),
    ("定义 @ Ln {}: {}", "定義 @ Ln {}: {}", "Definition @ Ln {}: {}"),
    ("首次出现 @ Ln {}", "初出 @ Ln {}", "First occurrence @ Ln {}"),
    ("先用 Alt+拖选 建立列块选区", "先に Alt+ドラッグで矩形選択を作ってください", "Make a column selection first with Alt+drag"),
    ("函数 @ {} 行", "関数 @ {} 行", "Function @ line {}"),
    ("没有函数标记 (这种文件类型没有大纲)", "関数マークがありません (このファイル種別にはアウトラインがありません)", "No function markers (this file type has no outline)"),
    ("SPACE {} {} 行", "SPACE {} {} 行", "SPACE {}: {} line(s)"),
    ("缩进", "インデント", "indent"),
    ("反缩进", "逆インデント", "unindent"),
    ("只读文档, 插件的修改已忽略", "読み取り専用の文書のため、プラグインの変更は無視しました", "Read-only document: the plugin's change was ignored"),
    ("插件返回的修改越界, 已拒绝", "プラグインが返した変更が範囲外のため拒否しました", "The plugin's change was out of range and was rejected"),
    ("插件返回的修改行数过多, 已拒绝", "プラグインが返した変更の行数が多すぎるため拒否しました", "The plugin's change had too many lines and was rejected"),
    ("宏 {} 是空的 (首选项 > 宏 里登记)", "マクロ {} は空です (環境設定 > マクロ で登録)", "Macro {} is empty (register it in Preferences > Macro)"),
    ("没有可登记的内容", "登録する内容がありません", "Nothing to register"),
    ("高速版不运行 JS 宏, 没有登记", "高速版は JS マクロを実行しないため、登録しませんでした", "The fast edition does not run JS macros, so nothing was registered"),
    ("已登记到宏 {}: {}", "マクロ {} に登録しました: {}", "Registered to macro {}: {}"),
    ("{} 个宏槽都满了 (首选项 > 宏 里清空一个)", "{} 個のマクロ枠がすべて埋まっています (環境設定 > マクロ で 1 つ空けてください)", "All {} macro slots are full (clear one in Preferences > Macro)"),
    ("宏的目标文档已经关了", "マクロの対象文書が閉じられました", "The macro's target document has been closed"),
    ("命令表里没有 {}", "コマンド表に {} がありません", "Command {} is not in the command table"),
    ("宏在第 {} 步停下 (已执行 {} 步): {}", "マクロは {} ステップ目で停止しました ({} ステップ実行済み): {}", "The macro stopped at step {} ({} step(s) done): {}"),
    ("● 录制宏 (.mac): 再执行一次「宏录制 (.mac)」停止并生成", "● マクロ記録 (.mac): もう一度「マクロ記録 (.mac)」で停止して生成", "● Recording macro (.mac): run “Record Macro (.mac)” again to stop and generate"),
    ("; 没有对应 S_ 函数 (未写入): {}", "; 対応する S_ 関数がありません (書き込まず): {}", "; no matching S_ function (not written): {}"),
    ("没有可写成 .mac 的操作{}", ".mac に書ける操作がありません{}", "Nothing that can be written as .mac{}"),
    ("已生成 {} 条宏语句{} — 保存为 .mac 后可用「运行当前文档 (Sakura .mac)」回放", "マクロ文を {} 件生成しました{} — .mac として保存すると「現在の文書を実行 (Sakura .mac)」で再生できます", "Generated {} macro statement(s){} — save as .mac, then replay with “Run Current Document (Sakura .mac)”"),
    ("第 {} 行 {}: {}", "{} 行目 {}: {}", "Line {} {}: {}"),
    ("第 {} 行 {}: 这条命令要由应用窗口执行 ({}), 这里不支持", "{} 行目 {}: このコマンドはアプリのウィンドウで実行されます ({})。ここでは未対応です", "Line {} {}: this command must run in the app window ({}), not supported here"),
    ("未支持的命令 {}", "未対応のコマンド {}", "Unsupported command {}"),
    ("这条命令高速版还不支持 ({})", "このコマンドは高速版ではまだ未対応です ({})", "This command is not supported in the fast edition yet ({})"),
    ("层叠窗口: 本应用是单窗口多标签 (有意偏差)", "ウィンドウの重ねて表示: このアプリは単一ウィンドウ + タブです (意図的な差異)", "Cascade windows: this app is single-window with tabs (intentional difference)"),
    ("平铺窗口: 本应用是单窗口多标签 (有意偏差)", "ウィンドウの並べて表示: このアプリは単一ウィンドウ + タブです (意図的な差異)", "Tile windows: this app is single-window with tabs (intentional difference)"),
    ("SQL*Plus 刻意不做", "SQL*Plus は意図的に実装していません", "SQL*Plus is deliberately not implemented"),
    ("IME 再変換: egui 没有触发 TSF 的接口", "IME 再変換: egui には TSF を呼び出す手段がありません", "IME reconversion: egui has no way to trigger TSF"),
    ("JS 宏没有移植 (高速版只运行 Sakura .mac 和 PPA)", "JS マクロは移植していません (高速版は Sakura .mac と PPA のみ実行)", "JS macros are not ported (the fast edition runs only Sakura .mac and PPA)"),
    ("标签栏随文档个数和界面样式自动显示, 没有单独的开关", "タブバーは文書数と画面スタイルに応じて自動表示され、個別のスイッチはありません", "The tab bar shows automatically based on document count and UI style; there is no separate switch"),
    ("有未保存的修改: 不能按别的编码重新打开", "未保存の変更があるため、別の文字コードで開き直せません", "There are unsaved changes: cannot reopen with another encoding"),
    ("字符集参数应当是数字, 实际是 \"{}\"", "文字コード引数は数値である必要があります (実際: \"{}\")", "The charset argument must be a number, got \"{}\""),
    ("字符集代码 {} 高速版不支持 (支持 0–8)", "文字コード番号 {} は高速版では未対応です (0–8 に対応)", "Charset code {} is not supported in the fast edition (0–8 supported)"),
    ("未知的编码转换", "不明なコード変換", "Unknown code conversion"),
    ("不是有效的宏文件: {}", "有効なマクロファイルではありません: {}", "Not a valid macro file: {}"),
    ("宏文件里没有 steps", "マクロファイルに steps がありません", "The macro file has no steps"),
    ("宏太长 ({} 步, 上限 {})", "マクロが長すぎます ({} ステップ, 上限 {})", "Macro too long ({} steps, limit {})"),
    ("第 {} 步格式不对", "{} ステップ目の形式が正しくありません", "Step {} is malformed"),
    ("第 {} 步: 命令名不合法 {}", "{} ステップ目: コマンド名が不正です {}", "Step {}: invalid command name {}"),
    ("tags 不是普通文件", "tags は通常のファイルではありません", "tags is not a regular file"),
    ("找不到备份目录", "バックアップフォルダーが見つかりません", "Backup folder not found"),
    ("无法打开浏览器: {}", "ブラウザーを開けません: {}", "Cannot open the browser: {}"),
    ("已在浏览器打开打印页", "印刷ページをブラウザーで開きました", "Opened the print page in the browser"),
    ("插件没有登记 {}", "プラグインが登録されていません {}", "Plugin not registered {}"),
    ("没有安装插件 {}", "プラグイン {} がインストールされていません", "Plugin {} is not installed"),
    ("插件 {} 已禁用", "プラグイン {} は無効です", "Plugin {} is disabled"),
    ("执行超时", "実行がタイムアウトしました", "Execution timed out"),
    ("插件执行超时 (超过 {} 秒), 已终止", "プラグインの実行がタイムアウトしました ({} 秒超過), 終了しました", "The plugin timed out (over {} s) and was stopped"),
    ("插件结果过大 ({} 字节, 上限 {})", "プラグインの結果が大きすぎます ({} バイト, 上限 {})", "The plugin result is too large ({} bytes, limit {})"),
    ("插件没有 editor.write 权限, 修改已忽略", "プラグインに editor.write 権限がないため、変更は無視しました", "The plugin lacks editor.write permission; its change was ignored"),
    ("文档在插件运行期间被修改, 补丁已放弃", "プラグインの実行中に文書が変更されたため、パッチを破棄しました", "The document changed while the plugin ran; the patch was discarded"),
    ("插件与当前版本不匹配: {}", "プラグインが現在のバージョンと合いません: {}", "The plugin does not match this version: {}"),
    ("插件包里没有 manifest.json", "プラグインパッケージに manifest.json がありません", "The plugin package has no manifest.json"),
    ("已安装 {} {}", "{} {} をインストールしました", "Installed {} {}"),
    ("插件工具返回的不是 JSON", "プラグインツールが返した内容は JSON ではありません", "The plugin tool did not return JSON"),
    ("插件工具失败: {}", "プラグインツールが失敗しました: {}", "Plugin tool failed: {}"),
    ("插件的补丁提案无效: {}", "プラグインのパッチ提案が無効です: {}", "The plugin's patch proposal is invalid: {}"),
    ("已提交给用户审核, 尚未套用", "ユーザーの確認に回しました。まだ適用されていません", "Submitted for your review; not applied yet"),
    ("[插件 {}] {}", "[プラグイン {}] {}", "[plugin {}] {}"),
    ("只允许 https 地址: {}", "https のアドレスのみ使用できます: {}", "Only https addresses are allowed: {}"),
    ("无法运行 curl ({}): 请确认系统自带 curl 可用", "curl を実行できません ({}): システムの curl が使えるか確認してください", "Cannot run curl ({}): make sure the system curl is available"),
    ("下载失败 {}: {}", "ダウンロードに失敗 {}: {}", "Download failed {}: {}"),
    ("响应超过大小上限: {}", "応答がサイズ上限を超えました: {}", "Response exceeded the size limit: {}"),
    ("无法创建配置目录: {}", "設定フォルダーを作成できません: {}", "Cannot create the config folder: {}"),
    ("保存 AI 配置失败: {}", "AI 設定の保存に失敗: {}", "Failed to save AI settings: {}"),
    ("接口地址含非法字符", "API アドレスに不正な文字が含まれています", "The API address contains illegal characters"),
    ("接口地址不能为空, 也不能内嵌账号口令", "API アドレスは空にできず、認証情報も埋め込めません", "The API address cannot be empty or embed credentials"),
    ("只允许 https 接口 (本机 localhost / 127.0.0.1 可用 http)", "https の API のみ使用できます (localhost / 127.0.0.1 は http 可)", "Only https endpoints are allowed (http is allowed for localhost / 127.0.0.1)"),
    ("接口地址不要带查询参数", "API アドレスにクエリパラメータを付けないでください", "Do not put query parameters in the API address"),
    ("接口列表里没有模型 {}", "API の一覧にモデル {} がありません", "Model {} is not in the API's list"),
    ("先填好接口地址和 API Key", "先に API アドレスと API Key を入力してください", "Fill in the API address and API key first"),
    ("供应商的模型接口没有公布 effort 档位{}, 可以在下面手动填写", "プロバイダーのモデル API は effort の段階を公開していません{}。下に手動で入力できます", "The provider's model API does not publish effort levels{}; you can type them below"),
    ("密钥含控制字符", "キーに制御文字が含まれています", "The key contains control characters"),
    ("已取消", "キャンセルしました", "Cancelled"),
    ("响应超过大小上限, 已中止", "応答がサイズ上限を超えたため中止しました", "The response exceeded the size limit and was aborted"),
    ("请求失败", "リクエストに失敗しました", "Request failed"),
    ("接口没有返回内容", "API から内容が返りませんでした", "The API returned nothing"),
    ("读不到 HTTP 状态码", "HTTP ステータスコードを読み取れません", "Cannot read the HTTP status code"),
    ("连不上接口", "API に接続できません", "Cannot reach the API"),
    ("接口返回的不是 JSON", "API が返した内容は JSON ではありません", "The API did not return JSON"),
    ("请求线程意外结束", "リクエストスレッドが予期せず終了しました", "The request thread ended unexpectedly"),
    ("后台任务意外结束", "バックグラウンドタスクが予期せず終了しました", "The background task ended unexpectedly"),
    ("重命名 {}", "{} の名前を変更", "Rename {}"),
    ("确定删除 {} ?", "{} を削除しますか?", "Delete {}?"),
    ("未知的编码转换: {}", "不明なコード変換: {}", "Unknown code conversion: {}"),
    ("自动判别: 没有找到合适的编码", "自動判別: 適切な文字コードが見つかりません", "Auto-detect: no suitable encoding found"),
    ("不是合法的 Base64 (含有字母数字 + / = 以外的字符)", "有効な Base64 ではありません (英数字と + / = 以外の文字を含む)", "Not valid Base64 (contains characters other than letters, digits, + / =)"),
    ("没有可解码的 Base64 内容", "デコードできる Base64 の内容がありません", "No Base64 content to decode"),
    ("Base64 长度不对 (被截断了?)", "Base64 の長さが不正です (切り詰められた?)", "Base64 length is wrong (truncated?)"),
    ("不是合法的 uuencode (含有范围外的字符)", "有効な uuencode ではありません (範囲外の文字を含む)", "Not valid uuencode (contains out-of-range characters)"),
    ("没有可解码的 uuencode 内容", "デコードできる uuencode の内容がありません", "No uuencode content to decode"),
    ("不是 SVG", "SVG ではありません", "Not an SVG"),
    ("SVG 里没有可绘制的图元", "SVG に描画できる要素がありません", "The SVG has nothing to draw"),
    ("Mermaid 图只在 Tauri 版里渲染, 这里显示源码", "Mermaid 図は Tauri 版でのみ描画されます。ここではソースを表示します", "Mermaid diagrams render only in the Tauri edition; showing the source here"),
    ("[图片 {}: {}]", "[画像 {}: {}]", "[image {}: {}]"),
    ("[图片 {}: {} (只支持本地 PNG)]", "[画像 {}: {} (ローカルの PNG のみ対応)]", "[image {}: {} (local PNG only)]"),
    ("暂无终端会话", "ターミナルセッションなし", "No terminal session"),
    ("(无名)", "(名前なし)", "(unnamed)"),
    ("无活动", "アクティブなし", "No activity"),
    ("无标题", "無題", "Untitled"),
    ("字符数", "文字数", "Characters"),
    ("字节数 (UTF-8)", "バイト数 (UTF-8)", "Bytes (UTF-8)"),
    ("行数", "行数", "Lines"),
    ("开", "オン", "on"),
    ("关", "オフ", "off"),
    ("空", "空", "empty"),
    ("是", "はい", "yes"),
    ("否", "いいえ", "no"),
    ("无标题 {}", "無題 {}", "Untitled {}"),
    ("名称: {}
路径: {}
大小: {}
修改时间: {}
行数: {}
字符数: {}
编码: {}
BOM: {}
换行符: {}{}
只读: {}", "名前: {}
パス: {}
サイズ: {}
更新日時: {}
行数: {}
文字数: {}
文字コード: {}
BOM: {}
改行コード: {}{}
読み取り専用: {}", "Name: {}
Path: {}
Size: {}
Modified: {}
Lines: {}
Characters: {}
Encoding: {}
BOM: {}
Line ending: {}{}
Read-only: {}"),
    ("{} 字节", "{} バイト", "{} bytes"),
    ("{} (Unix 秒)", "{} (Unix 秒)", "{} (Unix seconds)"),
    ("(尚未保存)", "(未保存)", "(not saved yet)"),
    ("(无)", "(なし)", "(none)"),
    (" (原文件混用)", " (元のファイルは混在)", " (mixed in the original file)"),
    ("无", "なし", "none"),
    ("把「{}」置换为「{}」\n范围: {}\n会改写磁盘上所有命中的文件 (原文件另存为 .bak)。继续吗?", "「{}」を「{}」に置換します\n範囲: {}\nヒットしたディスク上のすべてのファイルを書き換えます (元のファイルは .bak として保存)。続行しますか?", "Replace “{}” with “{}”\nScope: {}\nThis rewrites every matching file on disk (originals are saved as .bak). Continue?"),
    ("注意", "注意", "Note"),
    ("提示", "ヒント", "Tip"),
    ("重要", "重要", "Important"),
    ("警告", "警告", "Warning"),
    ("危险", "危険", "Danger"),
    ("成功", "成功", "Success"),
    ("疑问", "疑問", "Question"),
    ("引用", "引用", "Quote"),
];

struct Tpl {
    pieces: Vec<&'static str>,
    ja: &'static str,
    en: &'static str,
    /// 字面部分总长, 越长越具体, 先匹配
    weight: usize,
}

fn index() -> &'static [Tpl] {
    static IDX: OnceLock<Vec<Tpl>> = OnceLock::new();
    IDX.get_or_init(|| {
        let mut v: Vec<Tpl> = MSG_T
            .iter()
            .map(|(zh, ja, en)| {
                let pieces: Vec<&str> = zh.split("{}").collect();
                Tpl {
                    weight: pieces.iter().map(|p| p.len()).sum(),
                    pieces,
                    ja,
                    en,
                }
            })
            .collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.weight));
        v
    })
}

/// 按模板取出各 `{}` 的内容; 不匹配 = None
fn capture<'a>(pieces: &[&str], s: &'a str) -> Option<Vec<&'a str>> {
    let (first, rest) = pieces.split_first()?;
    if rest.is_empty() {
        return (s == *first).then(Vec::new);
    }
    let mut tail = s.strip_prefix(first)?;
    let last = *rest.last()?;
    tail = tail.strip_suffix(last)?;
    let mut caps = vec![];
    for mid in &rest[..rest.len() - 1] {
        if mid.is_empty() {
            return None; // 相邻的 {}{} 无法切分
        }
        let at = tail.find(mid)?;
        caps.push(&tail[..at]);
        tail = &tail[at + mid.len()..];
    }
    caps.push(tail);
    Some(caps)
}

fn fill(tpl: &str, caps: &[String]) -> String {
    let mut out = String::new();
    let mut it = caps.iter();
    let mut rest = tpl;
    while let Some(p) = rest.find("{}") {
        out.push_str(&rest[..p]);
        out.push_str(it.next().map(|s| s.as_str()).unwrap_or(""));
        rest = &rest[p + 2..];
    }
    out.push_str(rest);
    out
}

fn translate(s: &str, depth: u8) -> Option<String> {
    for t in index() {
        let Some(caps) = capture(&t.pieces, s) else {
            continue;
        };
        let caps: Vec<String> = caps
            .iter()
            .map(|c| {
                if depth < 2 {
                    translate(c, depth + 1).unwrap_or_else(|| (*c).to_string())
                } else {
                    (*c).to_string()
                }
            })
            .collect();
        return Some(fill(if cur() == Lang::Ja { t.ja } else { t.en }, &caps));
    }
    None
}

/// 把一条中文消息翻成当前界面语言; 中文界面 / 没有模板 = 原样返回
pub fn msg(s: &str) -> Cow<'_, str> {
    if cur() == Lang::Zh || s.is_empty() {
        return Cow::Borrowed(s);
    }
    match translate(s, 0) {
        Some(t) => Cow::Owned(t),
        None => Cow::Borrowed(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{set, TEST_LOCK};

    #[test]
    fn templates_translate_and_capture_values() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set(Lang::En);
        assert_eq!(msg("已保存 3 个文件"), "Saved 3 file(s)");
        assert_eq!(msg("已停止, 12 处命中"), "Stopped, 12 hit(s)");
        assert_eq!(
            msg("5 处命中 (已达 5000 上限)"),
            "5 hit(s) (limit 5000 reached)"
        );
        assert_eq!(
            msg("置换 4 处, 改写 2 个文件 (已备份为 .bak), 1 个失败"),
            "Replaced 4 in 2 file(s) (backed up as .bak), 1 failed"
        );
        assert_eq!(msg("1/9 (已折返)"), "1/9 (wrapped)");
        assert_eq!(msg("置换了 1 处; 1/3"), "Replaced 1; 1/3");
        assert_eq!(
            msg("置换了 1 处; 没有找到: x"),
            "Replaced 1; Not found: x",
            "嵌套的提示也会翻译"
        );
        assert_eq!(msg("没有任何模板的文字"), "没有任何模板的文字");
        set(Lang::Ja);
        assert_eq!(msg("没有找到: abc"), "見つかりません: abc");
        set(Lang::Zh);
        assert_eq!(msg("已保存 3 个文件"), "已保存 3 个文件", "中文界面原样");
    }

    #[test]
    fn every_template_has_matching_placeholders_in_all_languages() {
        for (zh, ja, en) in MSG_T {
            let n = zh.matches("{}").count();
            assert_eq!(ja.matches("{}").count(), n, "{zh}");
            assert_eq!(en.matches("{}").count(), n, "{zh}");
            assert!(!ja.is_empty() && !en.is_empty());
        }
    }

    #[test]
    fn matching_is_anchored_at_both_ends() {
        assert_eq!(
            capture(&["打不开: ", ""], "打不开: x.txt"),
            Some(vec!["x.txt"])
        );
        assert_eq!(capture(&["", " 处"], "5 处命中"), None);
        assert_eq!(capture(&["a", "b", "c"], "a1b2c"), Some(vec!["1", "2"]));
        assert_eq!(capture(&["无标题"], "无标题 2"), None);
    }
}
