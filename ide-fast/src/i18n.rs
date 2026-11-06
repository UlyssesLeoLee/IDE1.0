//! 界面语言 (中文 / 日本語 / English): `tr(键)` 按当前语言取字符串。
//!
//! 两张表: `T` 是语义键 (`menu.file` ...) -> 三种语言; `CMD_T` 以中文原文作键, 给命令表 (cmds.rs 的标签 / 分组) 用 ——
//! 命令表是数据, 不想在每一行里再塞一个键。表里没有的键 / 原文原样返回 (宁可显示中文也不显示空白)。
//! Tauri 版的 `I18N` 表 (editor.html) 有一批值已被截断损坏 (如 "保存并关…"), 所以这里按 ide-fast 实际用到的菜单 / 对话框重新写了一份,
//! 键名沿用 editor.html 的 `menu.* / sub.* / c.*` 命名风格。
//! 当前语言放进一个全局原子量 (和 Tauri 版的 localStorage 一样是进程级状态), 免得每个 `fn` 都要多带一个参数。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    Ja,
    En,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::Zh, Lang::Ja, Lang::En];
    pub fn code(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::Ja => "ja",
            Lang::En => "en",
        }
    }
    pub fn from_code(s: &str) -> Option<Lang> {
        match s {
            "zh" => Some(Lang::Zh),
            "ja" => Some(Lang::Ja),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
    /// 语言自己的名字 (菜单里各语言都用本语言写, 看不懂当前界面也找得到)
    pub fn native(self) -> &'static str {
        match self {
            Lang::Zh => "中文",
            Lang::Ja => "日本語",
            Lang::En => "English",
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set(l: Lang) {
    LANG.store(l as u8, Ordering::Relaxed);
}

pub fn set_code(code: &str) {
    set(Lang::from_code(code).unwrap_or(Lang::Zh));
}

pub fn cur() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Ja,
        2 => Lang::En,
        _ => Lang::Zh,
    }
}

fn pick(row: &'static [&'static str; 3]) -> &'static str {
    let s = row[cur() as usize];
    if s.is_empty() {
        row[0]
    } else {
        s
    }
}

fn index() -> &'static HashMap<&'static str, &'static [&'static str; 3]> {
    static IDX: OnceLock<HashMap<&'static str, &'static [&'static str; 3]>> = OnceLock::new();
    IDX.get_or_init(|| T.iter().map(|(k, v)| (*k, v)).collect())
}

fn zh_index() -> &'static HashMap<&'static str, &'static [&'static str; 3]> {
    static IDX: OnceLock<HashMap<&'static str, &'static [&'static str; 3]>> = OnceLock::new();
    IDX.get_or_init(|| CMD_T.iter().map(|(k, v)| (*k, v)).collect())
}

/// 语义键 -> 当前语言的字符串; 没有这个键 = 返回键本身
pub fn tr(key: &'static str) -> &'static str {
    index().get(key).map(|r| pick(r)).unwrap_or(key)
}

/// 中文原文 -> 当前语言 (命令表的标签 / 分组); 没收录 = 原样返回
pub fn trz(zh: &'static str) -> &'static str {
    zh_index().get(zh).map(|r| pick(r)).unwrap_or(zh)
}

/// 带 `{}` 占位的字符串: 把 `{}` 依次换成参数
pub fn trf(key: &'static str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let mut rest = tr(key);
    while let Some(p) = rest.find("{}") {
        out.push_str(&rest[..p]);
        out.push_str(it.next().copied().unwrap_or(""));
        rest = &rest[p + 2..];
    }
    out.push_str(rest);
    out
}

/// 菜单项文字: 名字 + 两个空格 + 快捷键提示 (键为空 = 只有名字)
pub fn lk(key: &'static str, keys: &str) -> String {
    // 注册表里有的命令用当前生效的键 (改过键就跟着变); 没有的用调用处写的提示
    let shown = crate::keymap::display(key);
    let keys = shown.as_deref().unwrap_or(keys);
    if keys.is_empty() {
        tr(key).to_string()
    } else {
        format!("{}  {}", tr(key), keys)
    }
}

/// 注册表里一条命令的悬停提示: 名字 + 当前生效的键
pub fn lk_for(e: &crate::keymap::Entry) -> String {
    match crate::keymap::display(&e.id) {
        Some(k) if !k.is_empty() => format!("{}  {}", e.label.text(), k),
        _ => e.label.text().to_string(),
    }
}

/// 语言是进程级全局量, 测试之间会互相踩: 会改语言的测试用这把锁串行化
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

type Row = (&'static str, [&'static str; 3]);

/// 语义键表: (键, [中, 日, 英])
pub const T: &[Row] = &[
    // ---- 顶层菜单
    ("menu.file", ["文件", "ファイル", "File"]),
    ("menu.edit", ["编辑", "編集", "Edit"]),
    ("menu.search", ["查找", "検索", "Search"]),
    ("menu.md-overlay", ["Markdown 预览覆盖编辑区", "Markdown プレビューをエディタに重ねる", "Markdown preview over the editor"]),
    ("menu.fold", ["折叠", "折りたたみ", "Folding"]),
    ("menu.plugin", ["插件", "プラグイン", "Plugins"]),
    ("menu.setting", ["设置", "設定", "Settings"]),
    ("menu.bookmark", ["书签", "ブックマーク", "Bookmarks"]),
    ("menu.window", ["窗口", "ウィンドウ", "Window"]),
    ("menu.macro", ["宏", "マクロ", "Macro"]),
    ("menu.view", ["视图", "表示", "View"]),
    ("menu.tool", ["工具", "ツール", "Tools"]),
    ("menu.help", ["帮助", "ヘルプ", "Help"]),
    // ---- 文件
    ("c.new-file", ["新建", "新規作成", "New"]),
    ("c.open-file", ["打开…", "開く…", "Open…"]),
    ("c.open-folder", ["打开文件夹…", "フォルダーを開く…", "Open Folder…"]),
    ("sub.recent", ["最近使用", "最近使ったファイル", "Recent Files"]),
    ("sub.recent-dirs", ["最近的文件夹", "最近使ったフォルダー", "Recent Folders"]),
    ("ui.none", ["(没有)", "(なし)", "(none)"]),
    ("c.recent-clear", ["清除历史", "履歴をクリア", "Clear History"]),
    ("c.save", ["保存", "上書き保存", "Save"]),
    ("c.save-as", ["另存为…", "名前を付けて保存…", "Save As…"]),
    ("c.save-all", ["全部保存", "すべて保存", "Save All"]),
    ("c.save-close", ["保存并关闭", "保存して閉じる", "Save and Close"]),
    ("c.close-tab", ["关闭标签", "タブを閉じる", "Close Tab"]),
    ("c.close-all", ["关闭全部", "すべて閉じる", "Close All"]),
    ("c.close-open", ["关闭并打开…", "閉じて開く…", "Close and Open…"]),
    ("c.reload", ["重新载入 (丢弃修改)", "再読み込み (変更を破棄)", "Reload (discard changes)"]),
    ("sub.reopen", ["以指定编码重新打开", "文字コードを指定して開き直す", "Reopen with Encoding"]),
    ("c.print", ["打印 / 打印预览…", "印刷 / 印刷プレビュー…", "Print / Print Preview…"]),
    ("c.browse", ["在浏览器 / 默认程序中打开", "ブラウザー / 既定のアプリで開く", "Open in Browser / Default App"]),
    ("c.file-properties", ["文件属性…", "ファイルのプロパティ…", "File Properties…"]),
    ("c.new-window", ["新窗口", "新規ウィンドウ", "New Window"]),
    ("c.exit", ["退出", "終了", "Exit"]),
    // ---- 编辑
    ("c.undo", ["撤销", "元に戻す", "Undo"]),
    ("c.redo", ["重做", "やり直し", "Redo"]),
    ("c.cut", ["剪切", "切り取り", "Cut"]),
    ("c.copy", ["复制", "コピー", "Copy"]),
    ("c.paste", ["粘贴", "貼り付け", "Paste"]),
    ("c.select-all", ["全选", "すべて選択", "Select All"]),
    // ---- 查找
    ("c.find", ["查找…", "検索…", "Find…"]),
    ("c.replace", ["置换…", "置換…", "Replace…"]),
    ("c.grep", ["在文件夹中搜索 (Grep)…", "フォルダー内を検索 (Grep)…", "Search in Folder (Grep)…"]),
    ("c.find-next", ["下一个", "次を検索", "Find Next"]),
    ("c.find-prev", ["上一个", "前を検索", "Find Previous"]),
    ("c.find-word", ["查找光标处的单词", "カーソル位置の単語を検索", "Find Word at Cursor"]),
    ("c.replace-all", ["全部置换", "すべて置換", "Replace All"]),
    // ---- 折叠
    ("c.fold-toggle", ["折叠 / 展开当前行", "現在行を折りたたみ / 展開", "Fold / Unfold Current Line"]),
    ("c.unfold-all", ["全部展开", "すべて展開", "Unfold All"]),
    // ---- 插件
    ("c.builtin-theme", ["内置主题 (深 / 浅色)", "内蔵テーマ (ダーク / ライト)", "Built-in Theme (dark / light)"]),
    ("ui.theme-prefix", ["主题: ", "テーマ: ", "Theme: "]),
    ("ui.script-only", ["  (代码块渲染器, 高速版预览暂未接入)", "  (コードブロック描画: 高速版プレビュー未対応)", "  (fence renderer; not wired into the fast preview yet)"]),
    ("c.delete", ["删除", "削除", "Delete"]),
    ("c.plugin-import", ["导入插件 (.zip)…", "プラグインをインポート (.zip)…", "Import Plugin (.zip)…"]),
    ("sub.plugin-restore", ["恢复内置插件", "内蔵プラグインを復元", "Restore Bundled Plugins"]),
    // ---- 设置
    ("c.common-config", ["首选项…", "共通設定…", "Preferences…"]),
    ("sub.uilang", ["界面语言 / 言語 / Language", "表示言語 / 言語 / Language", "Display Language / 言語 / 语言"]),
    ("c.export-config", ["导出配置…", "設定のエクスポート…", "Export Settings…"]),
    ("c.import-config", ["导入配置…", "設定のインポート…", "Import Settings…"]),
    ("c.import-sakura", ["导入 sakura.ini…", "sakura.ini をインポート…", "Import sakura.ini…"]),
    ("c.reset-settings", ["恢复默认设置", "初期設定に戻す", "Reset to Defaults"]),
    // ---- 书签
    ("c.bookmark-toggle", ["设置 / 取消", "設定 / 解除", "Toggle"]),
    ("c.bookmark-next", ["下一个", "次のブックマーク", "Next"]),
    ("c.bookmark-prev", ["上一个", "前のブックマーク", "Previous"]),
    ("c.bookmark-view", ["书签列表…", "ブックマーク一覧…", "Bookmark List…"]),
    ("c.bookmark-reset", ["全部清除", "すべて解除", "Clear All"]),
    ("c.bracket-jump", ["对括弧跳转", "対応する括弧へジャンプ", "Jump to Matching Bracket"]),
    ("c.hist-prev", ["移动履历: 上一个", "移動履歴: 前へ", "Jump History: Previous"]),
    ("c.hist-next", ["移动履历: 下一个", "移動履歴: 次へ", "Jump History: Next"]),
    ("c.hist-mark", ["把当前位置记入履历", "現在位置を履歴に記録", "Record Current Position"]),
    // ---- 窗口
    ("c.split", ["分割窗口 (左右)", "ウィンドウ分割 (左右)", "Split Window (Left/Right)"]),
    ("c.toolbar", ["工具栏", "ツールバー", "Toolbar"]),
    ("c.funckey", ["功能键栏", "ファンクションキー", "Function Key Bar"]),
    ("c.rail", ["活动栏", "アクティビティバー", "Activity Rail"]),
    ("c.print-setup", ["页面设置…", "ページ設定…", "Page Setup..."]),
    ("c.print-preview", ["打印预览…", "印刷プレビュー…", "Print Preview..."]),
    ("c.split-rows", ["分割窗口 (上下)", "ウィンドウ分割 (上下)", "Split Window (Top/Bottom)"]),
    ("c.split-quad", ["四分割窗口", "4分割", "Split Window (Quad)"]),
    ("c.next-tab", ["下一个标签", "次のタブ", "Next Tab"]),
    ("c.prev-tab", ["上一个标签", "前のタブ", "Previous Tab"]),
    ("sub.tabops", ["标签操作", "タブ操作", "Tab Operations"]),
    ("c.tab-move-left", ["标签左移", "タブを左へ移動", "Move Tab Left"]),
    ("c.tab-move-right", ["标签右移", "タブを右へ移動", "Move Tab Right"]),
    ("c.close-others", ["关闭其它标签", "他のタブを閉じる", "Close Other Tabs"]),
    ("c.close-left", ["关闭左侧标签", "左側のタブを閉じる", "Close Tabs to the Left"]),
    ("c.close-right", ["关闭右侧标签", "右側のタブを閉じる", "Close Tabs to the Right"]),
    ("sub.winlist", ["窗口列表", "ウィンドウ一覧", "Window List"]),
    ("c.minimize-window", ["最小化", "最小化", "Minimize"]),
    ("c.maximize", ["最大化 / 还原", "最大化 / 元に戻す", "Maximize / Restore"]),
    ("c.fullscreen", ["全屏", "全画面表示", "Full Screen"]),
    ("c.topmost", ["总在最前", "常に手前に表示", "Always on Top"]),
    // ---- 宏
    ("c.macro-start", ["开始录制", "記録開始", "Start Recording"]),
    ("c.macro-stop", ["结束录制", "記録終了", "Stop Recording"]),
    ("c.macro-play", ["回放", "再生", "Play"]),
    ("c.macro-run-doc", ["运行当前文档 (Sakura .mac)", "現在の文書を実行 (Sakura .mac)", "Run Current Document (Sakura .mac)"]),
    ("c.macro-run-ppa", ["运行当前文档 / 选区 (PPA)", "現在の文書/選択範囲を実行 (PPA)", "Run Document / Selection (PPA)"]),
    ("ui.plugin-enabled", ["启用 / 禁用", "有効 / 無効", "Enable / disable"]),
    ("dlg.plugin-off", ["插件 {} 未启用", "プラグイン {} は無効です", "Plugin {} is not enabled"]),
    ("dlg.plugin-running", ["正在运行插件命令: {}", "プラグインコマンド実行中: {}", "Running plugin command: {}"]),
    ("dlg.ppa-off", ["PPA 插件未启用 (插件菜单里勾选「PPA 宏子集」)", "PPA プラグインが無効です (プラグインメニューで有効化)", "PPA plugin is not enabled (tick it in the Plugins menu)"]),
    ("dlg.ppa-hint", ["把脚本存成 .ppa 文档再运行 (或先选中一段脚本)", ".ppa 文書として保存して実行 (またはスクリプトを選択)", "Save the script as a .ppa document (or select the script) first"]),
    ("dlg.ppa-no-target", ["没有可以运行脚本的另一份文档", "スクリプトを適用する別の文書がありません", "No other document to run the script on"]),
    ("c.macro-run-file", ["运行 .mac 文件…", ".mac ファイルを実行…", "Run .mac File…"]),
    // ---- 视图
    ("c.terminal", ["终端", "ターミナル", "Terminal"]),
    ("c.md-preview", ["Markdown 预览", "Markdown プレビュー", "Markdown Preview"]),
    ("c.dark", ["深色主题", "ダークテーマ", "Dark Theme"]),
    ("c.highlight", ["语法高亮", "構文ハイライト", "Syntax Highlighting"]),
    ("c.wrap", ["自动换行", "右端で折り返す", "Word Wrap"]),
    ("c.sidebar", ["侧栏", "サイドバー", "Sidebar"]),
    ("sub.tabwidth", ["Tab 宽度", "タブ幅", "Tab Width"]),
    ("c.zoom-in", ["放大", "拡大", "Zoom In"]),
    ("c.zoom-out", ["缩小", "縮小", "Zoom Out"]),
    ("c.zoom-reset", ["重置缩放", "拡大率をリセット", "Reset Zoom"]),
    ("c.line-numbers", ["行号", "行番号", "Line Numbers"]),
    ("c.status-bar", ["状态栏", "ステータスバー", "Status Bar"]),
    ("c.whitespace", ["显示空白符", "空白文字を表示", "Show Whitespace"]),
    ("c.ruler", ["列标尺", "桁ルーラー", "Column Ruler"]),
    ("c.cur-line", ["当前行下划线", "カーソル行に下線", "Current Line Underline"]),
    ("c.minimap", ["小地图", "ミニマップ", "Minimap"]),
    ("c.full-mode", ["完整界面 (活动栏 + 侧栏 + 标签栏)", "フル表示 (アクティビティバー + サイドバー + タブ)", "Full Layout (rail + side bar + tabs)"]),
    ("c.minimal-mode", ["极简布局 (仅编辑区)", "ミニマル表示 (編集領域のみ)", "Minimal Layout (editor only)"]),
    ("st.char-col", ["字符列 {}", "文字位置 {}", "Character column {}"]),
    ("st.selected", ["已选 {} 字符 / {} 行", "{} 文字 / {} 行選択", "{} chars / {} lines selected"]),
    ("st.selected-bytes", ["已选 {} 字节 (UTF-8) / {} 行", "{} バイト (UTF-8) / {} 行選択", "{} bytes (UTF-8) / {} lines selected"]),
    ("st.selected-lines", ["已选 {} 行", "{} 行選択", "{} lines selected"]),
    ("st.sel-hint", ["点击切换统计方式: 字符 / 字节 / 行", "クリックで切り替え: 文字 / バイト / 行", "Click to switch: chars / bytes / lines"]),
    ("st.plain", ["纯文本", "テキスト", "Plain Text"]),
    // ---- 帮助
    ("ui.about", ["IDE1.0 高速版 — 原生 Rust (egui)\n编辑语义复用 editcore (与 Tauri 版相同)", "IDE1.0 高速版 — ネイティブ Rust (egui)\n編集の挙動は editcore を再利用 (Tauri 版と同一)", "IDE1.0 Fast — native Rust (egui)\nEditing semantics reuse editcore (same as the Tauri build)"]),
    // ---- 标签右键 / 分组
    ("c.close", ["关闭", "閉じる", "Close"]),
    ("c.close-others-s", ["关闭其它", "他を閉じる", "Close Others"]),
    ("c.close-left-s", ["关闭左侧", "左を閉じる", "Close Left"]),
    ("c.close-right-s", ["关闭右侧", "右を閉じる", "Close Right"]),
    ("c.copy-path", ["复制路径", "パスをコピー", "Copy Path"]),
    ("c.key-config", ["键位设置…", "キー設定…", "Key Bindings…"]),
    ("c.key-list", ["键位一览", "キー一覧", "Key List"]),
    ("c.palette", ["命令面板…", "コマンドパレット…", "Command Palette…"]),
    ("pal.hint", ["输入命令名 (支持模糊匹配), Enter 执行, Esc 关闭", "コマンド名を入力 (あいまい検索), Enter で実行, Esc で閉じる", "Type a command (fuzzy match), Enter to run, Esc to close"]),
    ("pal.none", ["没有匹配的命令", "一致するコマンドがありません", "No matching command"]),
    ("pal.plugin", ["插件", "プラグイン", "Plugin"]),
    ("kc.title", ["键位设置", "キー設定", "Key Bindings"]),
    ("kc.filter", ["筛选命令 / 键", "コマンド / キーで絞り込み", "Filter commands / keys"]),
    ("kc.capture", ["请按下新的快捷键… (Esc 取消)", "新しいキーを押してください… (Esc でキャンセル)", "Press the new shortcut… (Esc to cancel)"]),
    ("kc.rebind", ["改键", "変更", "Rebind"]),
    ("kc.clear", ["清除", "解除", "Clear"]),
    ("kc.default", ["默认", "既定", "Default"]),
    ("kc.reset-all", ["全部恢复默认", "すべて既定に戻す", "Reset all"]),
    ("kc.close", ["关闭", "閉じる", "Close"]),
    ("kc.unbound", ["(未绑定)", "(未割り当て)", "(unbound)"]),
    ("kc.conflict", ["{} 已被其它命令占用: {}", "{} は他のコマンドが使用中: {}", "{} is already used by: {}"]),
    ("kc.steal", ["抢占 (对方解除绑定)", "奪う (相手は解除)", "Take it (unbind the other)"]),
    ("kc.cancel", ["取消", "キャンセル", "Cancel"]),
    ("kc.changed", ["已改 {} 条", "{} 件変更済み", "{} changed"]),
    ("kc.assigned", ["{} → {}", "{} → {}", "{} → {}"]),
    ("kc.fixed", ["系统键", "システムキー", "system"]),
    ("kc.list-title", ["键位一览", "キー一覧", "Key List"]),
    ("kc.copy", ["复制全部", "すべてコピー", "Copy all"]),
    ("kc.copied", ["已复制键位一览", "キー一覧をコピーしました", "Key list copied"]),
    ("kc.err-esc", ["Esc 留给取消, 不能绑定", "Esc はキャンセル用なので割り当てできません", "Esc is reserved for cancel"]),
    ("kc.err-plain", ["需要带 Ctrl 或 Alt (功能键除外), 否则会吃掉正常打字", "Ctrl か Alt が必要です (ファンクションキーを除く)。通常の入力が効かなくなります", "Needs Ctrl or Alt (function keys excepted), otherwise normal typing would break"]),
    ("kc.err-modifier", ["修饰键本身不能绑定, 请按完整的组合", "修飾キー単独は割り当てできません。組み合わせを押してください", "A modifier key alone cannot be bound; press a full combination"]),
    ("kc.err-unknown", ["未知命令", "不明なコマンド", "Unknown command"]),
    ("kc.err-fixed", ["这个键由系统处理, 不能改", "このキーはシステムが処理するため変更できません", "This key is handled by the system and cannot be changed"]),
    ("kc.err-system", ["{} 是系统键, 不能占用", "{} はシステムキーのため使えません", "{} is a system key and cannot be taken"]),
    ("grp.0", ["不分组", "グループなし", "No Group"]),
    ("grp.1", ["红组", "赤グループ", "Red"]),
    ("grp.2", ["橙组", "橙グループ", "Orange"]),
    ("grp.3", ["绿组", "緑グループ", "Green"]),
    ("grp.4", ["蓝组", "青グループ", "Blue"]),
    ("grp.5", ["紫组", "紫グループ", "Purple"]),
    // ---- 状态栏
    ("st.goto-hint", ["转到行…", "行へ移動…", "Go to line…"]),
    ("st.charset-save", ["保存时的编码", "保存時の文字コード", "Encoding when saving"]),
    ("st.reopen-as", ["以此编码重新打开", "この文字コードで開き直す", "Reopen with this encoding"]),
    ("st.add-bom", ["保存时加 BOM (UTF-8)", "保存時に BOM を付ける (UTF-8)", "Add BOM when saving (UTF-8)"]),
    ("st.rm-bom", ["保存时去掉 BOM", "保存時に BOM を外す", "Remove BOM when saving"]),
    ("st.mixed-eol", ["换行混用", "改行コード混在", "Mixed EOL"]),
    ("st.mixed-eol-tip", ["原文件混用了几种换行符; 点左边的换行符选一种统一", "元のファイルに複数の改行コードが混在しています。左の改行コードをクリックして統一してください", "The file mixed several line endings; click the EOL on the left to unify"]),
    ("st.lines", ["共 {} 行", "全 {} 行", "{} lines"]),
    ("st.overwrite", ["改写", "上書き", "OVR"]),
    ("st.readonly", ["只读", "読み取り専用", "Read-only"]),
    ("st.box", ["[矩形]", "[矩形]", "[Box]"]),
    ("st.cursors", ["{} 个光标", "カーソル {} 個", "{} cursors"]),
    // ---- 对话框
    ("dlg.big-title", ["较大的文件", "大きいファイル", "Large File"]),
    ("dlg.big-body", ["{}\n约 {} MB, 打开可能比较慢。继续吗?", "{}\n約 {} MB です。開くのに時間がかかる場合があります。続行しますか?", "{}\nAbout {} MB; opening may be slow. Continue?"]),
    ("dlg.open-failed", ["打开失败", "開けませんでした", "Open Failed"]),
    ("dlg.save-failed", ["保存失败", "保存に失敗しました", "Save Failed"]),
    ("dlg.unsaved-title", ["未保存的修改", "未保存の変更", "Unsaved Changes"]),
    ("dlg.unsaved-body", ["「{}」有未保存的修改, 保存吗?", "「{}」に未保存の変更があります。保存しますか?", "\"{}\" has unsaved changes. Save?"]),
    ("dlg.reopen-title", ["重新打开", "開き直す", "Reopen"]),
    ("dlg.reopen-body", ["丢弃「{}」未保存的修改, 按 {} 重新打开吗?", "「{}」の未保存の変更を破棄し、{} で開き直しますか?", "Discard unsaved changes in \"{}\" and reopen as {}?"]),
    ("dlg.reload-title", ["重新载入", "再読み込み", "Reload"]),
    ("dlg.reload-body", ["丢弃「{}」未保存的修改并从磁盘重新载入吗?", "「{}」の未保存の変更を破棄してディスクから再読み込みしますか?", "Discard unsaved changes in \"{}\" and reload from disk?"]),
    ("dlg.ext-title", ["文件被外部修改", "ファイルが外部で変更されました", "File Changed Externally"]),
    ("dlg.ext-body", ["「{}」在磁盘上被修改了, 而你这里也有未保存的修改。\n重新载入会丢弃你的修改。重新载入吗?", "「{}」はディスク上で変更されましたが、こちらにも未保存の変更があります。\n再読み込みすると変更は破棄されます。再読み込みしますか?", "\"{}\" changed on disk, and you also have unsaved changes.\nReloading discards your changes. Reload?"]),
    ("dlg.ext-reloaded", ["文件在外部被修改, 已重新载入", "ファイルが外部で変更されたため再読み込みしました", "The file changed externally and was reloaded"]),
    ("dlg.exit-title", ["退出", "終了", "Exit"]),
    ("dlg.cant-import", ["无法导入", "インポートできません", "Cannot Import"]),
    ("dlg.import-plugin", ["导入插件", "プラグインのインポート", "Import Plugin"]),
    ("dlg.reset-title", ["恢复默认设置", "初期設定に戻す", "Reset to Defaults"]),
    ("dlg.reset-body", ["把首选项恢复为默认值吗? (最近文件、搜索条件等不受影响)", "環境設定を既定値に戻しますか? (最近のファイルや検索条件は変わりません)", "Reset preferences to defaults? (Recent files and search options are kept)"]),
    ("dlg.settings-exported", ["配置已导出: {}", "設定をエクスポートしました: {}", "Settings exported: {}"]),
    ("dlg.settings-imported", ["配置已导入", "設定をインポートしました", "Settings imported"]),
    ("dlg.settings-bad", ["不是有效的配置文件", "有効な設定ファイルではありません", "Not a valid settings file"]),
    ("dlg.goto-title", ["转到行", "行へ移動", "Go to Line"]),
    ("dlg.goto-bad", ["请输入行号", "行番号を入力してください", "Enter a line number"]),
    ("dlg.bm-title", ["书签列表", "ブックマーク一覧", "Bookmark List"]),
    ("dlg.bm-empty", ["(当前文档没有书签)", "(このドキュメントにブックマークはありません)", "(no bookmarks in this document)"]),
    ("dlg.bm-cleared", ["已清除全部书签", "すべてのブックマークを解除しました", "All bookmarks cleared"]),
    ("dlg.props-title", ["文件属性", "ファイルのプロパティ", "File Properties"]),
    ("dlg.hist-marked", ["已记入移动履历", "移動履歴に記録しました", "Recorded in jump history"]),
    ("dlg.saved", ["已保存 {}", "保存しました {}", "Saved {}"]),
    ("dlg.not-found", ["找不到文件: {}", "ファイルが見つかりません: {}", "File not found: {}"]),
    // ---- 首选项
    ("pf.title", ["首选项", "共通設定", "Preferences"]),
    ("pf.ok", ["确定", "OK", "OK"]),
    ("pf.cancel", ["取消", "キャンセル", "Cancel"]),
    ("pf.apply", ["应用", "適用", "Apply"]),
    ("pf.tab.general", ["常规", "全般", "General"]),
    ("pf.tab.edit", ["编辑", "編集", "Editing"]),
    ("pf.tab.files", ["文件", "ファイル", "Files"]),
    ("pf.tab.font", ["字体与颜色", "フォントと色", "Font & Colors"]),
    ("pf.tab.types", ["文件类型", "ファイルタイプ", "File Types"]),
    ("pf.tab.search", ["查找", "検索", "Search"]),
    ("pf.tab.terminal", ["终端", "ターミナル", "Terminal"]),
    ("pf.tab.toolbar", ["工具栏", "ツールバー", "Toolbar"]),
    ("pf.tab.macro", ["宏", "マクロ", "Macros"]),
    ("pf.macro-hint", ["20 个宏槽: 名字 + 种类 + 代码。在「宏 > 宏登记」里点名字运行, 也可以在键位设置里给「运行宏 N」绑键; 「把当前文档 / 选区登记为宏」会存进第一个空槽。", "20 個のマクロスロット: 名前 + 種類 + コード。「マクロ > マクロ登録」から実行でき、キー設定で「マクロ N を実行」にキーを割り当てられます。", "20 macro slots: name + kind + code. Run them from Macro > Registered macros, or bind \"Run macro N\" in the key settings."]),
    ("pf.macro-empty", ["(空)", "(空)", "(empty)"]),
    ("pf.macro-name", ["名字", "名前", "Name"]),
    ("pf.macro-mac", ["Sakura 宏 (.mac)", "Sakura マクロ (.mac)", "Sakura macro (.mac)"]),
    ("pf.macro-ppa", ["PPA 脚本", "PPA スクリプト", "PPA script"]),
    ("pf.macro-clear", ["清空这个槽", "このスロットを空にする", "Clear this slot"]),
    ("pf.toolbar-hint", ["「视图 > 工具栏」打开后显示的按钮: 可以增减、排序 (像 Office 的快速访问工具栏)。", "「表示 > ツールバー」で表示されるボタン。追加・削除・並べ替えができます。", "Buttons shown when View > Toolbar is on. Add, remove and reorder them."]),
    ("pf.toolbar-add", ["添加命令…", "コマンドを追加…", "Add command…"]),
    ("pf.toolbar-sep", ["添加分隔线", "区切り線を追加", "Add separator"]),
    ("pf.toolbar-reset", ["恢复默认", "既定に戻す", "Restore default"]),
    ("pf.toolbar-sepline", ["──── 分隔线 ────", "──── 区切り線 ────", "──── separator ────"]),
    ("c.plugin-github", ["从 GitHub 安装 / 更新插件…", "GitHub からプラグインをインストール / 更新…", "Install / Update Plugin from GitHub…"]),
    ("c.open-autosave", ["打开自动备份文件…", "自動バックアップファイルを開く…", "Open Auto-backup File…"]),
    ("diff.hunk", ["{}. {}  第 {} 行起 ({} 行)", "{}. {}  {} 行目から ({} 行)", "{}. {}  from line {} ({} lines)"]),
    ("sys.restore-title", ["发现未保存的自动备份", "未保存の自動バックアップが見つかりました", "Unsaved auto-backups found"]),
    ("sys.restore-hint", ["上次没有正常退出, 下列备份里可能有没来得及保存的内容。", "前回正常終了しなかったため、保存されていない内容が残っている可能性があります。", "The last session did not exit cleanly; these backups may hold unsaved work."]),
    ("sys.restore-open", ["打开", "開く", "Open"]),
    ("sys.restore-open-all", ["全部打开", "すべて開く", "Open all"]),
    ("sys.restore-discard", ["全部丢弃", "すべて破棄", "Discard all"]),
    ("sys.restore-later", ["稍后再说", "後で", "Not now"]),
    ("sys.restore-age-min", ["{} 分钟前", "{} 分前", "{} min ago"]),
    ("sys.restore-age-hour", ["{} 小时前", "{} 時間前", "{} h ago"]),
    ("sys.restore-age-day", ["{} 天前", "{} 日前", "{} d ago"]),
    ("sys.restore-age-unknown", ["时间未知", "時刻不明", "time unknown"]),
    ("sys.restore-opened", ["已打开 {} 个备份文件", "{} 個のバックアップを開きました", "Opened {} backup file(s)"]),
    ("sys.restore-discarded", ["已丢弃 {} 个备份文件", "{} 個のバックアップを破棄しました", "Discarded {} backup file(s)"]),
    ("sys.lock-busy", ["{} 正被别的程序写入,没能加锁", "{} は他のプログラムが書き込み中のためロックできません", "{} is being written by another program; could not lock it"]),
    ("sys.lock-unsupported", ["这个平台不支持文件锁", "このプラットフォームはファイルロックに未対応です", "File locking is not supported on this platform"]),
    ("sys.lock-failed", ["{} 加锁失败: {}", "{} のロックに失敗: {}", "Could not lock {}: {}"]),
    ("sys.no-autosave", ["还没有自动备份文件", "自動バックアップファイルはまだありません", "No auto-backup files yet"]),
    ("bt.terminal", ["终端", "ターミナル", "Terminal"]),
    ("bt.output", ["输出", "出力", "Output"]),
    ("bt.problems", ["问题", "問題", "Problems"]),
    ("bt.clear", ["清空", "クリア", "Clear"]),
    ("bt.output-empty", ["还没有输出", "出力はまだありません", "No output yet"]),
    ("bt.problems-empty", ["没有问题 (在 Markdown 预览里点「检查」)", "問題はありません (Markdown プレビューの「チェック」)", "No problems (use Check in the Markdown preview)"]),
    ("md.edit-short", ["编辑", "編集", "Edit"]),
    ("md.lint", ["检查", "チェック", "Check"]),
    ("md.lint-tip", ["文档质量检查: 标题跳级 / 重复标题 / 空链接 / 缺 alt / 围栏未闭合", "ドキュメント検査: 見出しの飛び / 重複 / 空リンク / alt 欠落 / フェンス未閉鎖", "Lint: heading jumps, duplicates, empty links, missing alt, unclosed fences"]),
    ("md.lint-ok", ["文档检查通过: 没有发现问题", "ドキュメント検査: 問題なし", "Check passed: no problems found"]),
    ("md.lint-n", ["文档检查: {} 个问题 (见底部「问题」页)", "ドキュメント検査: {} 件 (下部の「問題」タブ)", "Check found {} problems (see the Problems tab)"]),
    ("md.struct", ["结构", "構造", "Outline"]),
    ("md.struct-tip", ["结构化搜索: 按标题 / 链接 / 图片 / 代码块 / 任务过滤, 点结果跳转", "構造検索: 見出し / リンク / 画像 / コードブロック / タスクで絞り込み、結果クリックで移動", "Structural search: filter by heading / link / image / code fence / task; click a result to jump"]),
    ("md.struct-title", ["结构化搜索", "構造検索", "Structural search"]),
    ("md.struct-hint", ["类型 [条件], 例: heading #2 用法", "種類 [条件] 例: heading #2 用法", "kind [filter], e.g. heading #2 usage"]),
    ("md.struct-none", ["没有匹配项", "一致する項目はありません", "No matches"]),
    ("md.struct-count", ["{} 处匹配, 点一条跳转", "{} 件一致。クリックで移動", "{} matches, click one to jump"]),
    ("c.md-struct", ["结构化搜索…", "構造検索…", "Structural search…"]),
    ("md.line", ["第 {} 行", "{} 行目", "Line {}"]),
    ("md.export-html", ["导出 HTML", "HTML 書き出し", "Export HTML"]),
    ("md.export-tip", ["导出同名 .html (模板可放在设置目录的 md-template.html)", "同名の .html を書き出し (テンプレートは設定フォルダの md-template.html)", "Export a same-name .html (custom template: md-template.html in the settings folder)"]),
    ("md.export-save-first", ["先保存文档再导出", "先に保存してください", "Save the document before exporting"]),
    ("md.exported", ["已导出: {}", "書き出しました: {}", "Exported: {}"]),
    ("md.export-failed", ["导出失败 (源文档未改动): {}", "書き出し失敗 (元の文書は未変更): {}", "Export failed (source untouched): {}"]),
    ("md.edit-block", ["编辑这一块的源码", "このブロックのソースを編集", "Edit this block's source"]),
    ("md.apply", ["应用", "適用", "Apply"]),
    ("md.cancel", ["取消", "キャンセル", "Cancel"]),
    ("md.fence-pending", ["{} 图渲染中…", "{} を描画中…", "Rendering {} diagram…"]),
    ("md.fence-nosvg", ["插件没有返回图", "プラグインが図を返しませんでした", "The plugin returned no diagram"]),
    ("md.fence-big", ["代码块超过 {} KB,不渲染", "ブロックが {} KB を超えるため描画しません", "Block is over {} KB; not rendered"]),
    ("gh.title", ["从 GitHub 安装 / 更新插件", "GitHub からプラグインをインストール / 更新", "Install / Update Plugin from GitHub"]),
    ("gh.note", ["只在点按钮时联网 (https,有大小上限);装之前会列出权限让你确认。", "ボタンを押したときだけ通信します (https・サイズ上限あり)。インストール前に権限を確認します。", "Network is used only when you click (https, size-capped); permissions are shown for approval first."]),
    ("gh.repo", ["仓库", "リポジトリ", "Repository"]),
    ("gh.check", ["检查", "確認", "Check"]),
    ("gh.check-all", ["检查已装插件的更新", "インストール済みの更新を確認", "Check installed plugins for updates"]),
    ("gh.install", ["安装", "インストール", "Install"]),
    ("gh.update", ["更新", "更新", "Update"]),
    ("c.ai-chat", ["AI 对话", "AI チャット", "AI Chat"]),
    ("ai.title", ["AI 对话", "AI チャット", "AI Chat"]),
    ("ai.settings", ["设置", "設定", "Settings"]),
    ("ai.clear", ["清空", "クリア", "Clear"]),
    ("ai.base-url", ["接口地址", "エンドポイント", "Endpoint"]),
    ("ai.model", ["模型", "モデル", "Model"]),
    ("ai.key", ["密钥", "API キー", "API key"]),
    ("ai.effort", ["推理强度", "推論強度", "Reasoning effort"]),
    ("ai.effort-default", ["默认 (不指定)", "既定 (指定しない)", "Default (unspecified)"]),
    ("ai.effort-custom", ["自定义档位", "カスタム段階", "Custom levels"]),
    ("ai.effort-fetch", ["从供应商获取档位", "プロバイダから段階を取得", "Fetch levels from provider"]),
    ("ai.effort-fetching", ["正在查询模型接口…", "モデル API を照会中…", "Querying the models endpoint…"]),
    ("ai.effort-got", ["获取到 {} 档: {}", "{} 段階を取得: {}", "Got {} levels: {}"]),
    ("ai.key-note", ["密钥以明文保存在本机用户设置目录的 ai.json 中 (和 Tauri 版一样), 不写日志; 只在点发送时联网 (https)。", "キーは平文でこの PC のユーザー設定フォルダの ai.json に保存されます (Tauri 版と同じ)。ログには出ません。送信時のみ https で通信します。", "The API key is stored in plain text in ai.json in your per-user settings folder (same as the Tauri version), never logged; network is used only on Send (https)."]),
    ("ai.save-failed", ["保存 AI 配置失败: {}", "AI 設定の保存に失敗しました: {}", "Failed to save AI settings: {}"]),
    ("ai.tool-unknown", ["未知工具: {}", "不明なツール: {}", "Unknown tool: {}"]),
    ("ai.save", ["保存", "保存", "Save"]),
    ("ai.send", ["发送", "送信", "Send"]),
    ("ai.cancel", ["取消", "キャンセル", "Cancel"]),
    ("ai.cancelled", ["已取消。", "キャンセルしました。", "Cancelled."]),
    ("ai.ctx", ["带上选中内容 / 当前章节", "選択範囲 / 現在の章を含める", "Include selection / current section"]),
    ("ai.placeholder", ["向 AI 提问 (Enter 发送, Shift+Enter 换行)", "AI に質問 (Enter で送信、Shift+Enter で改行)", "Ask the AI (Enter sends, Shift+Enter newline)"]),
    ("ai.you", ["你:", "あなた:", "You:"]),
    ("ai.thinking", ["回复中…", "応答中…", "Replying…"]),
    ("ai.insert", ["插入到编辑器", "エディタに挿入", "Insert into editor"]),
    ("ai.copy", ["复制", "コピー", "Copy"]),
    ("ai.apply", ["应用", "適用", "Apply"]),
    ("ai.reject", ["拒绝", "却下", "Reject"]),
    ("ai.applied", ["已套用 AI 补丁 (Ctrl+Z 可撤销)", "AI パッチを適用しました (Ctrl+Z で元に戻せます)", "AI patch applied (Ctrl+Z to undo)"]),
    ("ai.rejected", ["已拒绝一个补丁提案。", "パッチ提案を却下しました。", "Patch proposal rejected."]),
    ("ai.stale", ["文档已变化, 提案不再适用, 未套用。", "文書が変更されたため、提案は適用されませんでした。", "The document changed; the proposal was not applied."]),
    ("ai.proposal", ["补丁提案: 第 {} 行起替换 {} 行", "パッチ提案: {} 行目から {} 行を置換", "Patch proposal: from line {}, replace {} line(s)"]),
    ("ai.tool-ok", ["[{}] → 完成", "[{}] → 完了", "[{}] → done"]),
    ("ai.tool-fail", ["[{}] → 失败: {}", "[{}] → 失敗: {}", "[{}] → failed: {}"]),
    ("ai.not-configured", ["还没有配置 AI: 点右上角齿轮填接口地址 / 模型 / 密钥。", "AI が未設定です。歯車からエンドポイント / モデル / キーを入力してください。", "AI is not configured: open the gear and fill endpoint / model / key."]),
    ("ai.consent-title", ["发送到 AI", "AI に送信", "Send to AI"]),
    ("ai.consent", ["将把「{}」({} 个字符) 发送到你配置的 AI 端点:\n{}\n\n只发这一部分, 不发整篇文档。允许吗? (允许后不再询问)", "「{}」({} 文字) を設定した AI エンドポイントへ送信します:\n{}\n\nこの部分のみ送信し、文書全体は送りません。許可しますか? (以後は確認しません)", "The {} ({} characters) will be sent to your AI endpoint:\n{}\n\nOnly this part is sent, not the whole document. Allow? (You will not be asked again)"]),
    ("ai.allow", ["允许", "許可", "Allow"]),
    ("ai.deny", ["不带上下文发送", "文脈なしで送信", "Send without context"]),
    ("gh.perms", ["权限", "権限", "Permissions"]),
    ("gh.no-perms", ["无", "なし", "none"]),
    ("gh.not-installed", ["尚未安装", "未インストール", "Not installed"]),
    ("gh.update-from", ["已装 {},有新版本", "インストール済み {}、新しいバージョンあり", "Installed {}; a newer version is available"]),
    ("gh.up-to-date", ["已是最新 ({})", "最新です ({})", "Up to date ({})"]),
    ("gh.found", ["找到 {} {}", "{} {} が見つかりました", "Found {} {}"]),
    ("gh.incompatible", ["这个插件与当前编辑器不兼容,不能安装", "このプラグインは現在のエディタと互換性がありません", "This plugin is not compatible with this editor"]),
    ("gh.checking", ["正在检查 {} …", "{} を確認中…", "Checking {} …"]),
    ("gh.checking-all", ["正在检查更新…", "更新を確認中…", "Checking for updates…"]),
    ("gh.installing", ["正在安装 {} …", "{} をインストール中…", "Installing {} …"]),
    ("gh.installed", ["已安装 {}", "インストールしました: {}", "Installed {}"]),
    ("gh.updates", ["{} 个插件有新版本", "{} 件のプラグインに更新あり", "{} plugin(s) have updates"]),
    ("gh.confirm", ["来源: GitHub {}
插件: {} {} ({})
权限: {}

没有经过签名验证,只装你信任的插件。安装吗?", "ソース: GitHub {}
プラグイン: {} {} ({})
権限: {}

署名検証はありません。信頼できるものだけ。インストールしますか?", "Source: GitHub {}
Plugin: {} {} ({})
Permissions: {}

Not signature-verified; install only plugins you trust. Install?"]),
    ("pf.auto-backup", ["自动备份未保存的修改, 每隔", "未保存の変更を自動バックアップ (間隔)", "Auto-back up unsaved changes every"]),
    ("pf.minutes", [" 分钟", " 分", " min"]),
    ("pf.file-lock", ["打开文件时加锁 (别的程序可读不可写)", "開いたファイルをロック (他のプログラムは読み取りのみ)", "Lock opened files (other programs can read, not write)"]),
    ("pf.file-lock-tip", ["Windows: 共享读、禁止写; 保存时自动短暂放开。别的程序正在写的文件会提示锁不上", "Windows: 共有読み取り・書き込み禁止。保存時は一時的に解除", "Windows: shared-read, write-deny; released briefly while saving. Files another program is writing cannot be locked"]),
    ("pf.glass-blur", ["半透明主题: 窗口背后加模糊 (Windows)", "半透明テーマ: ウィンドウ背面をぼかす (Windows)", "Translucent themes: blur behind the window (Windows)"]),
    ("c.check-update", ["检查更新…", "アップデートを確認…", "Check for Updates…"]),
    ("pf.update-channel", ["更新通道", "アップデートチャンネル", "Update channel"]),
    ("pf.update-daily", ["每天启动时检查一次更新", "起動時に1日1回アップデートを確認", "Check for updates once a day at startup"]),
    ("pf.update-daily-tip", ["默认关。开启后只在首帧之后后台联网一次,绝不阻塞启动;也可随时用 帮助 > 检查更新 手动检查。", "既定はオフ。最初の描画後にバックグラウンドで1回だけ通信し、起動をブロックしません。ヘルプ > アップデートを確認 からいつでも手動確認できます。", "Off by default. Runs once in the background after the first frame and never blocks startup; Help > Check for Updates is always available."]),
    ("upd.ch-stable", ["稳定版 (正式发布)", "安定版 (正式リリース)", "Stable (releases)"]),
    ("upd.ch-dev", ["开发版 (含预发布)", "開発版 (プレリリース含む)", "Dev (includes pre-releases)"]),
    ("upd.title", ["检查更新", "アップデートの確認", "Check for Updates"]),
    ("upd.note", ["只在你点按钮 (或开启了每日检查) 时联网: 仅 https、有大小上限;安装包必须通过发布者签名与 SHA-256 校验才会安装。", "ボタンを押したとき (または毎日の確認を有効にした場合) のみ通信します。https のみ・サイズ上限あり。発行元の署名と SHA-256 検証に通ったものだけをインストールします。", "Network is used only when you click (or daily check is on): https only, size-capped; packages are installed only after publisher-signature and SHA-256 verification."]),
    ("upd.current", ["当前版本 {} · 通道: {}", "現在のバージョン {} ・チャンネル: {}", "Current version {} · channel: {}"]),
    ("upd.check", ["检查更新", "確認", "Check now"]),
    ("upd.checking", ["正在检查更新…", "アップデートを確認中…", "Checking for updates…"]),
    ("upd.uptodate", ["已是最新版本 ({})", "最新バージョンです ({})", "You are up to date ({})"]),
    ("upd.available", ["发现新版本 {} (当前 {})", "新しいバージョン {} があります (現在 {})", "New version {} available (current {})"]),
    ("upd.notes", ["更新说明", "リリースノート", "Release notes"]),
    ("upd.asset", ["安装包: {} ({} MB)", "パッケージ: {} ({} MB)", "Package: {} ({} MB)"]),
    ("upd.no-asset", ["这个版本没有适用于本机安装方式的安装包,请到发布页手动下载。", "このバージョンには、この環境向けのパッケージがありません。リリースページから手動で入手してください。", "This release has no package for this installation; download it from the release page."]),
    ("upd.download", ["下载并校验", "ダウンロードして検証", "Download and verify"]),
    ("upd.downloading", ["正在验签、下载并校验 SHA-256…", "署名を確認し、ダウンロードして SHA-256 を検証中…", "Verifying signature, downloading and checking SHA-256…"]),
    ("upd.verified", ["已下载并通过签名与 SHA-256 校验 ({})。", "ダウンロードと署名・SHA-256 の検証が完了しました ({})。", "Downloaded; signature and SHA-256 verified ({})."]),
    ("upd.install", ["安装并重启", "インストールして再起動", "Install and restart"]),
    ("upd.install-tip", ["会先走正常的关闭流程 (有未保存的修改会先问你),关闭后才开始安装。", "通常の終了処理を行い (未保存の変更があれば確認します)、終了後にインストールを開始します。", "Uses the normal close flow (unsaved changes are asked about first); installation starts after the window closes."]),
    ("upd.page", ["打开发布页面", "リリースページを開く", "Open release page"]),
    ("upd.failed", ["更新失败,当前版本保持不变:{}", "アップデートに失敗しました。現在のバージョンのままです: {}", "Update failed; the current version is unchanged: {}"]),
    ("upd.tip-found", ["发现新版本 {} (帮助 > 检查更新)", "新しいバージョン {} があります (ヘルプ > アップデートを確認)", "New version {} available (Help > Check for Updates)"]),
    ("upd.e-net", ["联网失败: {}", "通信に失敗しました: {}", "Network error: {}"]),
    ("upd.e-parse", ["无法解析发布信息: {}", "リリース情報を解析できません: {}", "Cannot parse release info: {}"]),
    ("upd.e-big", ["文件超过大小上限,已拒绝: {}", "サイズ上限を超えたため拒否しました: {}", "File exceeds the size limit, refused: {}"]),
    ("upd.e-size", ["下载大小不符: {} (应为 {} 字节,实际 {} 字节)", "ダウンロードサイズが一致しません: {} (期待 {} バイト、実際 {} バイト)", "Downloaded size mismatch: {} (expected {} bytes, got {})"]),
    ("upd.e-nosum", ["发布的 SHA256SUMS.txt 里没有 {} 的校验值,已拒绝安装", "SHA256SUMS.txt に {} のハッシュがないため、インストールを拒否しました", "SHA256SUMS.txt has no hash for {}; refusing to install"]),
    ("upd.e-mismatch", ["SHA-256 校验不一致,已拒绝并删除下载的文件: {}", "SHA-256 が一致しないため拒否し、ダウンロードしたファイルを削除しました: {}", "SHA-256 mismatch; refused and deleted the download: {}"]),
    ("upd.nokey", ["更新签名密钥未配置: 无法验证更新包的来源,所以不能在此安装。请到发布页手动下载。", "更新用の署名鍵が設定されていません。パッケージの発行元を検証できないため、ここではインストールできません。リリースページから手動で入手してください。", "Update signing key not configured: the package origin cannot be verified, so it cannot be installed here. Download it manually from the release page."]),
    ("upd.e-nokey", ["更新签名密钥未配置,已拒绝安装", "更新用の署名鍵が未設定のため、インストールを拒否しました", "Update signing key not configured; refusing to install"]),
    ("upd.e-sigmissing", ["发布里没有 SHA256SUMS.txt.sig 签名文件,已拒绝安装", "リリースに SHA256SUMS.txt.sig (署名) がないため、インストールを拒否しました", "The release has no SHA256SUMS.txt.sig signature; refusing to install"]),
    ("upd.e-sigbad", ["SHA256SUMS.txt 的签名无效 (可能被篡改或密钥不符),已拒绝并删除下载的文件", "SHA256SUMS.txt の署名が無効です (改ざんまたは鍵の不一致の可能性)。拒否し、ダウンロードしたファイルを削除しました", "Invalid signature on SHA256SUMS.txt (tampered or wrong key); refused and deleted the download"]),
    ("upd.e-mf-bad", ["签名清单第一行不是 \"version: X.Y.Z channel: stable|dev\" 格式,已拒绝并删除下载的文件", "署名済みマニフェストの先頭行が \"version: X.Y.Z channel: stable|dev\" 形式ではありません。拒否し、ダウンロードしたファイルを削除しました", "First line of the signed manifest is not \"version: X.Y.Z channel: stable|dev\"; refused and deleted the download"]),
    ("upd.e-mf-rel", ["签名清单的版本 ({}) 与该发布 ({}) 不符 (疑似旧清单挂在新发布上),已拒绝并删除下载的文件", "署名済みマニフェストのバージョン ({}) がこのリリース ({}) と一致しません (古いマニフェストの流用の疑い)。拒否し、ダウンロードしたファイルを削除しました", "Signed manifest version ({}) does not match this release ({}) (possible stale manifest); refused and deleted the download"]),
    ("upd.e-mf-old", ["签名清单的版本 ({}) 不高于当前版本 ({}) (疑似回滚),已拒绝并删除下载的文件", "署名済みマニフェストのバージョン ({}) が現在のバージョン ({}) より新しくありません (ロールバックの疑い)。拒否し、ダウンロードしたファイルを削除しました", "Signed manifest version ({}) is not newer than the current version ({}) (possible rollback); refused and deleted the download"]),
    ("upd.e-mf-chan", ["签名清单的通道 ({}) 与所选通道 ({}) 不符,已拒绝并删除下载的文件", "署名済みマニフェストのチャンネル ({}) が選択したチャンネル ({}) と一致しません。拒否し、ダウンロードしたファイルを削除しました", "Signed manifest channel ({}) does not match the selected channel ({}); refused and deleted the download"]),
    ("upd.e-mf-name", ["安装包文件名 {} 不含签名清单中的版本号,已拒绝并删除下载的文件", "インストーラ名 {} に署名済みマニフェストのバージョンが含まれていません。拒否し、ダウンロードしたファイルを削除しました", "Installer file name {} does not contain the signed manifest version; refused and deleted the download"]),
    ("upd.e-format", ["下载的文件不是有效的安装包格式,已拒绝并删除: {}", "ダウンロードしたファイルは有効なインストーラ形式ではないため、拒否して削除しました: {}", "The downloaded file is not a valid installer format; refused and deleted: {}"]),
    ("upd.e-io", ["文件操作失败", "ファイル操作に失敗しました", "File operation failed"]),
    ("pf.glass-note", ["半透明主题需要重启后才能透出桌面; 关掉模糊 = 只做颜色叠加", "半透明テーマは再起動後に反映されます", "Translucent themes take effect after a restart; off = colour overlay only"]),
    ("pf.term-font", ["终端字号", "ターミナルのフォントサイズ", "Terminal font size"]),
    ("pf.term-scrollback", ["终端保留行数", "ターミナルの保持行数", "Terminal scrollback lines"]),
    ("pf.term-fg", ["终端文字", "ターミナル文字色", "Terminal text"]),
    ("pf.term-bg", ["终端背景", "ターミナル背景", "Terminal background"]),
    ("pf.language", ["界面语言", "表示言語", "Language"]),
    ("pf.dark", ["深色主题", "ダークテーマ", "Dark theme"]),
    ("pf.highlight", ["语法高亮", "構文ハイライト", "Syntax highlighting"]),
    ("pf.wrap-col", ["折行列数", "折り返し桁数", "Wrap column"]),
    ("pf.wrap-col-tip", ["勾选 = 此类型按指定列数折行 (自动换行开着时)", "オン = この種類は桁指定で折り返す (折り返しが有効な時)", "On = wrap this type at a fixed column (when word wrap is on)"]),
    ("pf.wrap", ["自动换行", "右端で折り返す", "Word wrap"]),
    ("pf.font-size", ["字号", "フォントサイズ", "Font size"]),
    ("pf.tab-width", ["Tab 宽度", "タブ幅", "Tab width"]),
    ("pf.auto-indent", ["自动缩进", "自動インデント", "Auto indent"]),
    ("pf.smart-indent", ["智能缩进 (按语言)", "スマートインデント (言語別)", "Smart indent (per language)"]),
    ("pf.insert-spaces", ["Tab 键插入空格", "Tab キーで空白を挿入", "Insert spaces for Tab"]),
    ("pf.default-enc", ["新文档的编码", "新規文書の文字コード", "Encoding for new documents"]),
    ("pf.default-eol", ["新文档的换行符", "新規文書の改行コード", "Line ending for new documents"]),
    ("pf.save-backup", ["保存时备份旧文件 (.sakura-backup)", "保存時に旧ファイルをバックアップ (.sakura-backup)", "Back up the old file on save (.sakura-backup)"]),
    ("pf.check-external", ["窗口获得焦点时检查外部修改", "ウィンドウ復帰時に外部変更を確認", "Check for external changes on focus"]),
    ("pf.restore-session", ["启动时恢复上次打开的文件", "起動時に前回開いていたファイルを復元", "Reopen last session's files on startup"]),
    ("c.about", ["关于…", "バージョン情報…", "About…"]),
    ("dlg.about-title", ["关于 IDE1.0 高速版", "IDE1.0 高速版について", "About IDE1.0 Fast"]),
    ("dlg.macro-saved", ["键盘宏已保存: {}", "キーボードマクロを保存しました: {}", "Key macro saved: {}"]),
    ("dlg.macro-loaded", ["已载入键盘宏 ({} 步), Ctrl+Shift+L 回放", "キーボードマクロを読み込みました ({} ステップ)。Ctrl+Shift+L で再生", "Key macro loaded ({} steps); Ctrl+Shift+L to play"]),
    ("dlg.macro-empty", ["还没有录下的宏, 没有可保存的内容", "記録したマクロがありません", "No recorded macro to save"]),
    ("dlg.macro-bad", ["读不了这个宏: {}", "このマクロを読み込めません: {}", "Cannot load this macro: {}"]),
    ("dlg.copied-name", ["已复制: {}", "コピーしました: {}", "Copied: {}"]),
    ("dlg.codecnv-done", ["已转换 ({})", "コード変換しました ({})", "Converted ({})"]),
    ("dlg.decoded-text", ["已解码为新标签 ({} 字符)", "デコード結果を新しいタブに開きました ({} 文字)", "Decoded into a new tab ({} chars)"]),
    ("dlg.decoded-saved", ["已解码并保存 {} 字节: {}", "デコードして保存しました ({} バイト): {}", "Decoded and saved {} bytes: {}"]),
    ("dlg.no-unc", ["不能保存到网络 (UNC) 路径", "ネットワーク (UNC) パスには保存できません", "Cannot save to a network (UNC) path"]),
    ("dlg.html-copied", ["已复制彩色 HTML ({} 行)", "カラー HTML をコピーしました ({} 行)", "Copied colored HTML ({} lines)"]),
    ("dlg.tag-none", ["没有找到 {} 的定义", "{} の定義が見つかりません", "No definition found for {}"]),
    ("dlg.tag-jumped", ["已跳转到 {} (第 {} 行)", "{} へジャンプしました ({} 行)", "Jumped to {} (line {})"]),
    ("dlg.tag-empty", ["没有可返回的标签跳转", "戻れるタグジャンプがありません", "No tag jump to go back to"]),
    ("dlg.tag-closed", ["原来的标签已关闭", "元のタブは閉じられています", "The original tab was closed"]),
    ("dlg.tag-noroot", ["先打开一个文件夹再生成 tags", "先にフォルダを開いてください", "Open a folder first"]),
    ("dlg.tag-made", ["已生成 {} 条标签: {}", "{} 件のタグを生成しました: {}", "Wrote {} tags: {}"]),
    ("dlg.tag-fail", ["生成 tags 失败: {}", "tags の生成に失敗: {}", "Could not write tags: {}"]),
    ("dlg.tag-noword", ["光标处没有可跳转的词", "カーソル位置に単語がありません", "No word at the cursor"]),
    ("dlg.drop-moved", ["已移动选中的文字", "選択テキストを移動しました", "Moved the selected text"]),
    ("dlg.drop-copied", ["已复制选中的文字", "選択テキストをコピーしました", "Copied the selected text"]),
    ("dlg.wrap-col", ["已按 {} 列折行", "{} 桁で折り返します", "Wrapping at column {}"]),
    ("dlg.wrap-window", ["已按窗口宽度折行", "ウィンドウ幅で折り返します", "Wrapping at window width"]),
    ("dlg.no-path", ["这个文档还没有保存过, 没有路径", "このドキュメントは未保存のためパスがありません", "This document has not been saved yet, so it has no path"]),
    ("dlg.no-more-changes", ["没有更多改动处了", "これ以上変更箇所はありません", "No more modified lines"]),
    ("pf.confirm-close", ["关闭有未保存修改的窗口前确认", "未保存の変更がある場合に終了前に確認", "Confirm before closing with unsaved changes"]),
    ("pf.max-file", ["打开前确认的文件大小 (MB)", "開く前に確認するファイルサイズ (MB)", "Confirm before opening files above (MB)"]),
    ("pf.mru", ["最近文件个数", "最近使ったファイルの数", "Recent files to remember"]),
    ("pf.font-family", ["编辑器字体", "エディターのフォント", "Editor font"]),
    ("pf.font-default", ["(默认等宽字体)", "(既定の等幅フォント)", "(default monospace)"]),
    ("pf.line-height", ["行高", "行の高さ", "Line height"]),
    ("pf.colors", ["颜色覆盖 (留空 = 跟随主题)", "色の上書き (未設定 = テーマに従う)", "Color overrides (unset = follow theme)"]),
    ("pf.col.text", ["文字", "文字", "Text"]),
    ("pf.col.bg", ["背景", "背景", "Background"]),
    ("pf.col.keyword", ["关键字", "キーワード", "Keyword"]),
    ("pf.col.string", ["字符串", "文字列", "String"]),
    ("pf.col.comment", ["注释", "コメント", "Comment"]),
    ("pf.col.number", ["数字", "数値", "Number"]),
    ("pf.col.clear", ["清除", "クリア", "Clear"]),
    ("pf.col.set", ["设置", "設定", "Set"]),
    ("pf.types-hint", ["按扩展名覆盖: 留空的项沿用全局设置", "拡張子ごとに上書き: 未設定の項目は全体設定に従います", "Override per extension; unset items follow the global setting"]),
    ("pf.type-ext", ["扩展名", "拡張子", "Extension"]),
    ("pf.type-add", ["添加", "追加", "Add"]),
    ("pf.type-remove", ["移除", "削除", "Remove"]),
    ("pf.inherit", ["沿用", "継承", "inherit"]),
    ("pf.find-case", ["区分大小写", "大文字小文字を区別", "Match case"]),
    ("pf.find-regex", ["正则表达式", "正規表現", "Regular expression"]),
    ("pf.find-word", ["全字匹配", "単語単位", "Whole word"]),
    ("pf.find-wrap", ["到末尾后循环查找", "末尾で先頭に戻る", "Wrap around"]),
    ("pf.file", ["设置文件: {}", "設定ファイル: {}", "Settings file: {}"]),
    ("pf.file-off", ["(未启用落盘)", "(保存無効)", "(persistence off)"]),
    ("pf.restart-note", ["字体、行高和颜色在按「应用」或「确定」后生效", "フォント・行の高さ・色は「適用」または OK で反映されます", "Font, line height and colors take effect on Apply or OK"]),
    // ---- 侧栏 / 终端 / 查找条
    ("sd.files", ["文件", "ファイル", "Files"]),
    ("sd.search", ["搜索", "検索", "Search"]),
    ("sd.outline", ["大纲", "アウトライン", "Outline"]),
];

/// 命令表 (cmds.rs) 的标签 / 分组: 中文原文 -> [中, 日, 英]
pub const CMD_T: &[Row] = &[
    // 补全翻译 (0.3.8): 命令表 / 工具窗口 / 悬停提示
    ("多AI协同审查看板…", ["多AI协同审查看板…", "マルチAI共同レビューダッシュボード…", "Multi-AI Review Dashboard…"]),
    ("多AI协同实时编辑演示…", ["多AI协同实时编辑演示…", "マルチAI共同編集デモ…", "Multi-AI Co-Editing Demo…"]),
    ("协同审查", ["协同审查", "共同レビュー", "Co-Edit Review"]),
    ("查找内容", ["查找内容", "検索内容", "Find what"]),
    (
        "剪贴板历史",
        ["剪贴板历史", "クリップボード履歴", "Clipboard History"],
    ),
    (
        "还没有复制过内容 (只记录本程序里的复制 / 剪切)",
        [
            "还没有复制过内容 (只记录本程序里的复制 / 剪切)",
            "まだコピーした内容がありません (このアプリ内のコピー / 切り取りのみ記録)",
            "Nothing copied yet (only copy / cut inside this app is recorded)",
        ],
    ),
    ("粘贴", ["粘贴", "貼り付け", "Paste"]),
    ("字符", ["字符", "文字", "chars"]),
    ("清空", ["清空", "クリア", "Clear"]),
    ("控制字符", ["控制字符", "制御文字", "Control Characters"]),
    (
        "点要插入的控制字符",
        [
            "点要插入的控制字符",
            "挿入する制御文字をクリック",
            "Click a control character to insert",
        ],
    ),
    ("戳记设置", ["戳记设置", "スタンプ設定", "Stamp Settings"]),
    (
        "日期 (YYYY/MM/DD)",
        [
            "日期 (YYYY/MM/DD)",
            "日付 (YYYY/MM/DD)",
            "Date (YYYY/MM/DD)",
        ],
    ),
    (
        "时间 (HH:MM:SS)",
        ["时间 (HH:MM:SS)", "時刻 (HH:MM:SS)", "Time (HH:MM:SS)"],
    ),
    ("日期 + 时间", ["日期 + 时间", "日付 + 時刻", "Date + time"]),
    (
        "行首 (缩进之后, 写成 [戳记] )",
        [
            "行首 (缩进之后, 写成 [戳记] )",
            "行頭 (インデントの後, [スタンプ] 形式)",
            "Line start (after indent, as [stamp] )",
        ],
    ),
    ("光标处", ["光标处", "カーソル位置", "At cursor"]),
    ("预览:", ["预览:", "プレビュー:", "Preview:"]),
    (
        "按指定列数折行",
        ["按指定列数折行", "指定桁で折り返し", "Wrap at Column"],
    ),
    (
        "列数 (半角字符数; 0 = 按窗口宽度)",
        [
            "列数 (半角字符数; 0 = 按窗口宽度)",
            "桁数 (半角文字数; 0 = ウィンドウ幅)",
            "Columns (half-width chars; 0 = window width)",
        ],
    ),
    ("列块替换", ["列块替换", "矩形置換", "Column Replace"]),
    (
        "列块替换为 (多行用换行分隔)",
        [
            "列块替换为 (多行用换行分隔)",
            "矩形を次に置換 (複数行は改行区切り)",
            "Replace the column block with (separate lines with newlines)",
        ],
    ),
    ("应用", ["应用", "適用", "Apply"]),
    (
        "未打开项目",
        ["未打开项目", "プロジェクト未オープン", "No project open"],
    ),
    (
        "点工具栏「打开文件夹」导入项目",
        [
            "点工具栏「打开文件夹」导入项目",
            "ツールバーの「フォルダーを開く」でプロジェクトを読み込み",
            "Use “Open Folder” on the toolbar to import a project",
        ],
    ),
    ("插入", ["插入", "挿入", "Insert"]),
    ("标签分组", ["标签分组", "タブグループ", "Tab Groups"]),
    ("折行方式", ["折行方式", "折り返し方法", "Word Wrap"]),
    (
        "把刚录的键盘宏转成 .mac",
        [
            "把刚录的键盘宏转成 .mac",
            "録画したキーボードマクロを .mac に変換",
            "Convert the Recorded Key Macro to .mac",
        ],
    ),
    ("宏登记", ["宏登记", "マクロ登録", "Macro Slots"]),
    (
        "把当前文档 / 选区登记为宏",
        [
            "把当前文档 / 选区登记为宏",
            "現在の文書 / 選択範囲をマクロとして登録",
            "Register Current Document / Selection as a Macro",
        ],
    ),
    (
        "立即备份",
        ["立即备份", "今すぐバックアップ", "Back Up Now"],
    ),
    (
        "范围选择模式: 关",
        [
            "范围选择模式: 关",
            "範囲選択モード: オフ",
            "Range selection mode: off",
        ],
    ),
    ("新建终端", ["新建终端", "新しいターミナル", "New Terminal"]),
    (
        "新建终端 (Ctrl+`)",
        [
            "新建终端 (Ctrl+`)",
            "新しいターミナル (Ctrl+`)",
            "New Terminal (Ctrl+`)",
        ],
    ),
    (
        "折叠 (Ctrl+Alt+L)",
        [
            "折叠 (Ctrl+Alt+L)",
            "折りたたむ (Ctrl+Alt+L)",
            "Collapse (Ctrl+Alt+L)",
        ],
    ),
    ("未设置模型", ["未设置模型", "モデル未設定", "No model set"]),
    ("模型设置", ["模型设置", "モデル設定", "Model Settings"]),
    ("范围选择", ["范围选择", "範囲選択", "Range"]),
    ("预览", ["预览", "プレビュー", "Preview"]),
    (
        "源码 / 渲染预览切换 (Ctrl+Shift+V)",
        [
            "源码 / 渲染预览切换 (Ctrl+Shift+V)",
            "ソース / レンダリング表示の切り替え (Ctrl+Shift+V)",
            "Toggle source / rendered preview (Ctrl+Shift+V)",
        ],
    ),
    (
        "最近的文件路径",
        [
            "最近的文件路径",
            "最近使ったファイルのパス",
            "Recent File Paths",
        ],
    ),
    (
        "最近的文件夹路径",
        [
            "最近的文件夹路径",
            "最近使ったフォルダーのパス",
            "Recent Folder Paths",
        ],
    ),
    (
        "打开文件夹",
        ["打开文件夹", "フォルダーを開く", "Open Folder"],
    ),
    (
        "打开文件夹 (Ctrl+K O)",
        [
            "打开文件夹 (Ctrl+K O)",
            "フォルダーを開く (Ctrl+K O)",
            "Open Folder (Ctrl+K O)",
        ],
    ),
    ("页面设置", ["页面设置", "ページ設定", "Page Setup"]),
    ("纸张", ["纸张", "用紙", "Paper"]),
    ("横向", ["横向", "横向き", "Landscape"]),
    (
        "字号 (pt)",
        ["字号 (pt)", "フォントサイズ (pt)", "Font size (pt)"],
    ),
    ("页边距 (mm)", ["页边距 (mm)", "余白 (mm)", "Margins (mm)"]),
    (
        "打印行号",
        ["打印行号", "行番号を印刷", "Print line numbers"],
    ),
    (
        "页眉 (文件名)",
        [
            "页眉 (文件名)",
            "ヘッダー (ファイル名)",
            "Header (file name)",
        ],
    ),
    (
        "页脚 (页码)",
        [
            "页脚 (页码)",
            "フッター (ページ番号)",
            "Footer (page number)",
        ],
    ),
    (
        "打印预览…",
        ["打印预览…", "印刷プレビュー…", "Print Preview…"],
    ),
    ("打印预览", ["打印预览", "印刷プレビュー", "Print Preview"]),
    ("页面设置…", ["页面设置…", "ページ設定…", "Page Setup…"]),
    ("打印…", ["打印…", "印刷…", "Print…"]),
    (
        "折叠 / 展开左侧栏",
        [
            "折叠 / 展开左侧栏",
            "左サイドバーの折りたたみ / 展開",
            "Collapse / Expand Left Sidebar",
        ],
    ),
    (
        "折叠 / 展开右侧面板",
        [
            "折叠 / 展开右侧面板",
            "右パネルの折りたたみ / 展開",
            "Collapse / Expand Right Panel",
        ],
    ),
    (
        "折叠 / 展开底部面板",
        [
            "折叠 / 展开底部面板",
            "下パネルの折りたたみ / 展開",
            "Collapse / Expand Bottom Panel",
        ],
    ),
    (
        "折叠左侧栏",
        [
            "折叠左侧栏",
            "左サイドバーを折りたたむ",
            "Collapse Left Sidebar",
        ],
    ),
    (
        "折叠右侧面板",
        [
            "折叠右侧面板",
            "右パネルを折りたたむ",
            "Collapse Right Panel",
        ],
    ),
    (
        "折叠底部面板",
        [
            "折叠底部面板",
            "下パネルを折りたたむ",
            "Collapse Bottom Panel",
        ],
    ),
    (
        "使用系统标题栏",
        [
            "使用系统标题栏",
            "システムのタイトルバーを使う",
            "Use System Title Bar",
        ],
    ),
    ("下一个函数", ["下一个函数", "次の関数", "Next Function"]),
    (
        "上一个函数",
        ["上一个函数", "前の関数", "Previous Function"],
    ),
    (
        "复制路径与光标位置",
        [
            "复制路径与光标位置",
            "パスとカーソル位置をコピー",
            "Copy Path and Cursor Position",
        ],
    ),
    (
        "在折行处加换行复制",
        [
            "在折行处加换行复制",
            "折り返し位置に改行を入れてコピー",
            "Copy with Line Breaks at Wraps",
        ],
    ),
    (
        "打开同名 C/C++ 头 / 源文件",
        [
            "打开同名 C/C++ 头 / 源文件",
            "同名の C/C++ ヘッダー / ソースを開く",
            "Open Matching C/C++ Header / Source",
        ],
    ),
    (
        "(选择) 下一处改动",
        [
            "(选择) 下一处改动",
            "(選択) 次の変更箇所",
            "(Select) Next Change",
        ],
    ),
    (
        "(选择) 上一处改动",
        [
            "(选择) 上一处改动",
            "(選択) 前の変更箇所",
            "(Select) Previous Change",
        ],
    ),
    ("下一个分组", ["下一个分组", "次のグループ", "Next Group"]),
    (
        "上一个分组",
        ["上一个分组", "前のグループ", "Previous Group"],
    ),
    (
        "移到新分组",
        ["移到新分组", "新しいグループへ移動", "Move to New Group"],
    ),
    (
        "移到下一个分组",
        ["移到下一个分组", "次のグループへ移動", "Move to Next Group"],
    ),
    (
        "移到上一个分组",
        [
            "移到上一个分组",
            "前のグループへ移動",
            "Move to Previous Group",
        ],
    ),
    (
        "合并为一组",
        ["合并为一组", "1 つのグループに統合", "Merge into One Group"],
    ),
    (
        "关闭当前分组",
        [
            "关闭当前分组",
            "現在のグループを閉じる",
            "Close Current Group",
        ],
    ),
    (
        "SPACE 缩进",
        ["SPACE 缩进", "SPACE インデント", "SPACE Indent"],
    ),
    (
        "SPACE 反缩进",
        ["SPACE 反缩进", "SPACE 逆インデント", "SPACE Unindent"],
    ),
    (
        "光标行居中",
        ["光标行居中", "カーソル行を中央へ", "Center Cursor Line"],
    ),
    (
        "切换选区统计方式 (字符 / 字节 / 行)",
        [
            "切换选区统计方式 (字符 / 字节 / 行)",
            "選択範囲の集計方式を切り替え (文字 / バイト / 行)",
            "Cycle Selection Count (chars / bytes / lines)",
        ],
    ),
    ("代码补全", ["代码补全", "コード補完", "Code Completion"]),
    (
        "转到定义 (本文件)",
        [
            "转到定义 (本文件)",
            "定義へジャンプ (このファイル)",
            "Go to Definition (this file)",
        ],
    ),
    (
        "切换查找标记 (全部强调)",
        [
            "切换查找标记 (全部强调)",
            "検索マークの切り替え (すべて強調)",
            "Toggle Find Marks (highlight all)",
        ],
    ),
    (
        "回到查找起点",
        ["回到查找起点", "検索開始位置へ戻る", "Back to Find Start"],
    ),
    ("列块删除", ["列块删除", "矩形削除", "Column Delete"]),
    ("列块替换…", ["列块替换…", "矩形置換…", "Column Replace…"]),
    (
        "宏录制 (.mac): 开始 / 停止并生成",
        [
            "宏录制 (.mac): 开始 / 停止并生成",
            "マクロ記録 (.mac): 開始 / 停止して生成",
            "Record Macro (.mac): Start / Stop and Generate",
        ],
    ),
    (
        "宏录制 (.mac)",
        ["宏录制 (.mac)", "マクロ記録 (.mac)", "Record Macro (.mac)"],
    ),
    (
        "停止录制并生成 .mac",
        [
            "停止录制并生成 .mac",
            "記録を停止して .mac を生成",
            "Stop Recording and Generate .mac",
        ],
    ),
    (
        "范围选择模式 (开始 / 结束)",
        [
            "范围选择模式 (开始 / 结束)",
            "範囲選択モード (開始 / 終了)",
            "Range Selection Mode (start / end)",
        ],
    ),
    ("标签跳转", ["标签跳转", "タグジャンプ", "Tag Jump"]),
    (
        "标签跳转: 返回",
        ["标签跳转: 返回", "タグジャンプ: 戻る", "Tag Jump: Back"],
    ),
    (
        "指定关键字的标签跳转…",
        [
            "指定关键字的标签跳转…",
            "キーワード指定のタグジャンプ…",
            "Tag Jump to Keyword…",
        ],
    ),
    (
        "生成标签文件 (tags)",
        [
            "生成标签文件 (tags)",
            "タグファイル (tags) を作成",
            "Make Tags File (tags)",
        ],
    ),
    (
        "增量查找: 向后",
        [
            "增量查找: 向后",
            "インクリメンタル検索: 下へ",
            "Incremental Search: Forward",
        ],
    ),
    (
        "增量查找: 向前",
        [
            "增量查找: 向前",
            "インクリメンタル検索: 上へ",
            "Incremental Search: Backward",
        ],
    ),
    (
        "增量查找: 正则 向后",
        [
            "增量查找: 正则 向后",
            "インクリメンタル検索: 正規表現 下へ",
            "Incremental Search: Regex Forward",
        ],
    ),
    (
        "增量查找: 正则 向前",
        [
            "增量查找: 正则 向前",
            "インクリメンタル検索: 正規表現 上へ",
            "Incremental Search: Regex Backward",
        ],
    ),
    (
        "增量查找: Migemo 向后",
        [
            "增量查找: Migemo 向后",
            "インクリメンタル検索: Migemo 下へ",
            "Incremental Search: Migemo Forward",
        ],
    ),
    (
        "增量查找: Migemo 向前",
        [
            "增量查找: Migemo 向前",
            "インクリメンタル検索: Migemo 上へ",
            "Incremental Search: Migemo Backward",
        ],
    ),
    (
        "Base64 解码",
        ["Base64 解码", "Base64 デコード", "Base64 Decode"],
    ),
    (
        "uudecode 解码",
        ["uudecode 解码", "uudecode デコード", "uudecode Decode"],
    ),
    (
        "复制为彩色 HTML",
        [
            "复制为彩色 HTML",
            "色付き HTML としてコピー",
            "Copy as Colored HTML",
        ],
    ),
    (
        "复制为彩色 HTML (带行号)",
        [
            "复制为彩色 HTML (带行号)",
            "色付き HTML としてコピー (行番号付き)",
            "Copy as Colored HTML (with line numbers)",
        ],
    ),
    (
        "剪贴板历史…",
        ["剪贴板历史…", "クリップボード履歴…", "Clipboard History…"],
    ),
    (
        "插入控制字符…",
        [
            "插入控制字符…",
            "制御文字を挿入…",
            "Insert Control Character…",
        ],
    ),
    ("插入日期", ["插入日期", "日付を挿入", "Insert Date"]),
    ("插入时间", ["插入时间", "時刻を挿入", "Insert Time"]),
    ("插入戳记", ["插入戳记", "スタンプを挿入", "Insert Stamp"]),
    (
        "戳记设置…",
        ["戳记设置…", "スタンプ設定…", "Stamp Settings…"],
    ),
    (
        "按指定列数折行…",
        ["按指定列数折行…", "指定桁で折り返し…", "Wrap at Column…"],
    ),
    (
        "按窗口宽度折行",
        [
            "按窗口宽度折行",
            "ウィンドウ幅で折り返し",
            "Wrap at Window Width",
        ],
    ),
    ("不折行", ["不折行", "折り返さない", "No Wrap"]),
    ("展开", ["展开", "展開", "Expand"]),
    ("折叠", ["折叠", "折りたたむ", "Collapse"]),
    ("最小化", ["最小化", "最小化", "Minimize"]),
    ("最大化", ["最大化", "最大化", "Maximize"]),
    ("还原", ["还原", "元に戻す", "Restore"]),
    (
        "展开导航 / 文件",
        [
            "展开导航 / 文件",
            "ナビ / ファイルを展開",
            "Expand Navigation / Files",
        ],
    ),
    (
        "折叠导航 / 文件",
        [
            "折叠导航 / 文件",
            "ナビ / ファイルを折りたたむ",
            "Collapse Navigation / Files",
        ],
    ),
    ("展开 Chat", ["展开 Chat", "チャットを展開", "Expand Chat"]),
    (
        "折叠 Chat",
        ["折叠 Chat", "チャットを折りたたむ", "Collapse Chat"],
    ),
    (
        "展开底部面板",
        ["展开底部面板", "下パネルを展開", "Expand Bottom Panel"],
    ),
    (
        "粗体 (Ctrl+B)",
        ["粗体 (Ctrl+B)", "太字 (Ctrl+B)", "Bold (Ctrl+B)"],
    ),
    (
        "斜体 (Ctrl+I)",
        ["斜体 (Ctrl+I)", "斜体 (Ctrl+I)", "Italic (Ctrl+I)"],
    ),
    (
        "删除线 (Ctrl+Shift+X)",
        [
            "删除线 (Ctrl+Shift+X)",
            "取り消し線 (Ctrl+Shift+X)",
            "Strikethrough (Ctrl+Shift+X)",
        ],
    ),
    (
        "高亮 ==文字==",
        ["高亮 ==文字==", "ハイライト ==文字==", "Highlight ==text=="],
    ),
    (
        "行内代码 (Ctrl+E)",
        [
            "行内代码 (Ctrl+E)",
            "インラインコード (Ctrl+E)",
            "Inline code (Ctrl+E)",
        ],
    ),
    (
        "标题 1 (Ctrl+Alt+1)",
        [
            "标题 1 (Ctrl+Alt+1)",
            "見出し 1 (Ctrl+Alt+1)",
            "Heading 1 (Ctrl+Alt+1)",
        ],
    ),
    (
        "标题 2 (Ctrl+Alt+2)",
        [
            "标题 2 (Ctrl+Alt+2)",
            "見出し 2 (Ctrl+Alt+2)",
            "Heading 2 (Ctrl+Alt+2)",
        ],
    ),
    (
        "标题 3 (Ctrl+Alt+3)",
        [
            "标题 3 (Ctrl+Alt+3)",
            "見出し 3 (Ctrl+Alt+3)",
            "Heading 3 (Ctrl+Alt+3)",
        ],
    ),
    ("• 列表", ["• 列表", "• リスト", "• List"]),
    ("1. 列表", ["1. 列表", "1. 番号付きリスト", "1. List"]),
    (
        "链接 (Ctrl+Shift+K)",
        [
            "链接 (Ctrl+Shift+K)",
            "リンク (Ctrl+Shift+K)",
            "Link (Ctrl+Shift+K)",
        ],
    ),
    ("表格 3×3", ["表格 3×3", "表 3×3", "Table 3×3"]),
    (
        "{ } 代码块",
        ["{ } 代码块", "{ } コードブロック", "{ } Code block"],
    ),
    (
        "围栏代码块 (自动选安全围栏)",
        [
            "围栏代码块 (自动选安全围栏)",
            "フェンスコードブロック (安全なフェンスを自動選択)",
            "Fenced code block (picks a safe fence)",
        ],
    ),
    ("↑ 章节", ["↑ 章节", "↑ 章", "↑ Section"]),
    (
        "把光标所在章节(含子章节)上移一格",
        [
            "把光标所在章节(含子章节)上移一格",
            "カーソル位置の章 (子章を含む) を 1 つ上へ",
            "Move the section at the cursor (with subsections) up one",
        ],
    ),
    ("↓ 章节", ["↓ 章节", "↓ 章", "↓ Section"]),
    (
        "把光标所在章节(含子章节)下移一格",
        [
            "把光标所在章节(含子章节)下移一格",
            "カーソル位置の章 (子章を含む) を 1 つ下へ",
            "Move the section at the cursor (with subsections) down one",
        ],
    ),
    (
        "Mermaid 图 (需要渲染插件才能在预览里出图)",
        [
            "Mermaid 图 (需要渲染插件才能在预览里出图)",
            "Mermaid 図 (プレビュー表示にはレンダリングプラグインが必要)",
            "Mermaid diagram (needs a rendering plugin to show in the preview)",
        ],
    ),
    ("新建文件", ["新建文件", "新規ファイル", "New File"]),
    ("新建文件夹", ["新建文件夹", "新規フォルダー", "New Folder"]),
    ("Grep 置换", ["Grep 置换", "Grep 置換", "Grep Replace"]),
    ("Grep 结果", ["Grep 结果", "Grep 結果", "Grep Results"]),
    (
        "录制的宏.mac",
        ["录制的宏.mac", "記録したマクロ.mac", "Recorded macro.mac"],
    ),
    (
        "插件包",
        ["插件包", "プラグインパッケージ", "Plugin package"],
    ),
    ("保存 ·", ["保存 ·", "保存 ·", "Save ·"]),
    ("终端 ·", ["终端 ·", "ターミナル ·", "Terminal ·"]),
    ("帮助", ["帮助", "ヘルプ", "Help"]),
    ("运行宏 1", ["运行宏 1", "マクロ 1 を実行", "Run Macro 1"]),
    ("运行宏 2", ["运行宏 2", "マクロ 2 を実行", "Run Macro 2"]),
    ("运行宏 3", ["运行宏 3", "マクロ 3 を実行", "Run Macro 3"]),
    ("运行宏 4", ["运行宏 4", "マクロ 4 を実行", "Run Macro 4"]),
    ("运行宏 5", ["运行宏 5", "マクロ 5 を実行", "Run Macro 5"]),
    ("运行宏 6", ["运行宏 6", "マクロ 6 を実行", "Run Macro 6"]),
    ("运行宏 7", ["运行宏 7", "マクロ 7 を実行", "Run Macro 7"]),
    ("运行宏 8", ["运行宏 8", "マクロ 8 を実行", "Run Macro 8"]),
    ("运行宏 9", ["运行宏 9", "マクロ 9 を実行", "Run Macro 9"]),
    (
        "运行宏 10",
        ["运行宏 10", "マクロ 10 を実行", "Run Macro 10"],
    ),
    (
        "运行宏 11",
        ["运行宏 11", "マクロ 11 を実行", "Run Macro 11"],
    ),
    (
        "运行宏 12",
        ["运行宏 12", "マクロ 12 を実行", "Run Macro 12"],
    ),
    (
        "运行宏 13",
        ["运行宏 13", "マクロ 13 を実行", "Run Macro 13"],
    ),
    (
        "运行宏 14",
        ["运行宏 14", "マクロ 14 を実行", "Run Macro 14"],
    ),
    (
        "运行宏 15",
        ["运行宏 15", "マクロ 15 を実行", "Run Macro 15"],
    ),
    (
        "运行宏 16",
        ["运行宏 16", "マクロ 16 を実行", "Run Macro 16"],
    ),
    (
        "运行宏 17",
        ["运行宏 17", "マクロ 17 を実行", "Run Macro 17"],
    ),
    (
        "运行宏 18",
        ["运行宏 18", "マクロ 18 を実行", "Run Macro 18"],
    ),
    (
        "运行宏 19",
        ["运行宏 19", "マクロ 19 を実行", "Run Macro 19"],
    ),
    (
        "运行宏 20",
        ["运行宏 20", "マクロ 20 を実行", "Run Macro 20"],
    ),
    // 选择列表 / 关键字帮助 (0.3.8)
    (
        "窗口一览…",
        ["窗口一览…", "ウィンドウ一覧…", "Window List…"],
    ),
    ("窗口一览", ["窗口一览", "ウィンドウ一覧", "Window List"]),
    (
        "最近的文件…",
        ["最近的文件…", "最近使ったファイル…", "Recent Files…"],
    ),
    (
        "最近的文件",
        ["最近的文件", "最近使ったファイル", "Recent Files"],
    ),
    (
        "最近的文件夹…",
        ["最近的文件夹…", "最近使ったフォルダー…", "Recent Folders…"],
    ),
    (
        "最近的文件夹",
        ["最近的文件夹", "最近使ったフォルダー", "Recent Folders"],
    ),
    (
        "收藏夹 (最近的文件和文件夹)…",
        [
            "收藏夹 (最近的文件和文件夹)…",
            "お気に入り (最近のファイルとフォルダー)…",
            "Favorites (recent files and folders)…",
        ],
    ),
    ("收藏夹", ["收藏夹", "お気に入り", "Favorites"]),
    (
        "插入最近的文件路径…",
        [
            "插入最近的文件路径…",
            "最近使ったファイルのパスを挿入…",
            "Insert Recent File Path…",
        ],
    ),
    (
        "插入最近的文件路径",
        [
            "插入最近的文件路径",
            "最近使ったファイルのパスを挿入",
            "Insert Recent File Path",
        ],
    ),
    (
        "插入最近的文件夹路径…",
        [
            "插入最近的文件夹路径…",
            "最近使ったフォルダーのパスを挿入…",
            "Insert Recent Folder Path…",
        ],
    ),
    (
        "插入最近的文件夹路径",
        [
            "插入最近的文件夹路径",
            "最近使ったフォルダーのパスを挿入",
            "Insert Recent Folder Path",
        ],
    ),
    (
        "指定字符集…",
        ["指定字符集…", "文字コードセット指定…", "Set Character Set…"],
    ),
    (
        "指定字符集",
        ["指定字符集", "文字コードセット指定", "Set Character Set"],
    ),
    (
        "指定字符集重新打开…",
        [
            "指定字符集重新打开…",
            "文字コードセットを指定して開き直す…",
            "Reopen with Character Set…",
        ],
    ),
    (
        "指定字符集重新打开",
        [
            "指定字符集重新打开",
            "文字コードセットを指定して開き直す",
            "Reopen with Character Set",
        ],
    ),
    ("帮助手册", ["帮助手册", "ヘルプマニュアル", "Help Manual"]),
    (
        "关键字帮助检索…",
        [
            "关键字帮助检索…",
            "キーワードヘルプ検索…",
            "Keyword Help Search…",
        ],
    ),
    (
        "关键字帮助检索",
        [
            "关键字帮助检索",
            "キーワードヘルプ検索",
            "Keyword Help Search",
        ],
    ),
    (
        "关键字帮助: 光标处的词",
        [
            "关键字帮助: 光标处的词",
            "キーワードヘルプ: カーソル位置の語",
            "Keyword Help: Word at Cursor",
        ],
    ),
    (
        "关键字帮助自动显示",
        [
            "关键字帮助自动显示",
            "キーワードヘルプの自動表示",
            "Auto-show Keyword Help",
        ],
    ),
    (
        "显示大纲",
        ["显示大纲", "アウトラインを表示", "Show Outline"],
    ),
    (
        "显示文件树",
        ["显示文件树", "ファイルツリーを表示", "Show File Tree"],
    ),
    (
        "输入以过滤",
        ["输入以过滤", "入力して絞り込み", "Type to filter"],
    ),
    (
        "没有匹配的项",
        ["没有匹配的项", "一致する項目がありません", "No matches"],
    ),
    ("文件夹", ["文件夹", "フォルダー", "Folder"]),
    ("当前", ["当前", "現在", "Current"]),
    // 分组
    ("行", ["行", "行", "Lines"]),
    ("单词", ["单词", "単語", "Words"]),
    ("语法类型", ["语法类型", "構文タイプ", "Syntax type"]),
    (
        "自动 (按扩展名)",
        [
            "自动 (按扩展名)",
            "自動 (拡張子で判定)",
            "Auto (by extension)",
        ],
    ),
    ("新增", ["新增", "追加", "Added"]),
    ("修改", ["修改", "変更", "Changed"]),
    ("删除", ["删除", "削除", "Deleted"]),
    ("复制", ["复制", "コピー", "Copy"]),
    ("转换", ["转换", "変換", "Convert"]),
    ("整理行", ["整理行", "行の整理", "Tidy Lines"]),
    ("缩进", ["缩进", "インデント", "Indent"]),
    ("AI 协同", ["AI 协同", "AI 協調", "AI Collaboration"]),
    ("Markdown", ["Markdown", "Markdown", "Markdown"]),
    // 行
    ("剪切行", ["剪切行", "行を切り取り", "Cut Line"]),
    (
        "复制行 (含换行)",
        [
            "复制行 (含换行)",
            "行をコピー (改行含む)",
            "Copy Lines (with EOL)",
        ],
    ),
    (
        "复制当前行",
        ["复制当前行", "現在行を複製", "Duplicate Line"],
    ),
    ("删除当前行", ["删除当前行", "現在行を削除", "Delete Line"]),
    (
        "删除上方行",
        ["删除上方行", "上の行を削除", "Delete Line Above"],
    ),
    (
        "删除下方行",
        ["删除下方行", "下の行を削除", "Delete Line Below"],
    ),
    (
        "与上一行合并",
        ["与上一行合并", "前の行と結合", "Join with Previous Line"],
    ),
    (
        "在上方插入空行",
        ["在上方插入空行", "上に空行を挿入", "Insert Line Above"],
    ),
    (
        "在下方插入空行",
        ["在下方插入空行", "下に空行を挿入", "Insert Line Below"],
    ),
    ("上移一行", ["上移一行", "行を上へ移動", "Move Line Up"]),
    ("下移一行", ["下移一行", "行を下へ移動", "Move Line Down"]),
    (
        "剪切到行尾",
        ["剪切到行尾", "行末まで切り取り", "Cut to End of Line"],
    ),
    (
        "剪切到行首",
        ["剪切到行首", "行頭まで切り取り", "Cut to Start of Line"],
    ),
    (
        "删除到行尾",
        ["删除到行尾", "行末まで削除", "Delete to End of Line"],
    ),
    (
        "删除到行首",
        ["删除到行首", "行頭まで削除", "Delete to Start of Line"],
    ),
    // 单词
    (
        "选择当前单词",
        ["选择当前单词", "現在の単語を選択", "Select Word"],
    ),
    ("剪切单词", ["剪切单词", "単語を切り取り", "Cut Word"]),
    ("删除单词", ["删除单词", "単語を削除", "Delete Word"]),
    // 复制
    (
        "复制 (行尾统一 CRLF)",
        [
            "复制 (行尾统一 CRLF)",
            "コピー (改行を CRLF に統一)",
            "Copy (normalize to CRLF)",
        ],
    ),
    (
        "复制 (带行号)",
        [
            "复制 (带行号)",
            "コピー (行番号付き)",
            "Copy (with line numbers)",
        ],
    ),
    (
        "复制引用 (> 前缀)",
        [
            "复制引用 (> 前缀)",
            "引用としてコピー (> 付き)",
            "Copy as Quote (> prefix)",
        ],
    ),
    ("矩形粘贴", ["矩形粘贴", "矩形貼り付け", "Paste Rectangle"]),
    // 转换
    ("转为大写", ["转为大写", "大文字に変換", "To Uppercase"]),
    ("转为小写", ["转为小写", "小文字に変換", "To Lowercase"]),
    (
        "半角 → 全角",
        ["半角 → 全角", "半角 → 全角", "Half-width → Full-width"],
    ),
    (
        "全角 → 半角",
        ["全角 → 半角", "全角 → 半角", "Full-width → Half-width"],
    ),
    ("Tab → 空格", ["Tab → 空格", "Tab → 空白", "Tabs → Spaces"]),
    (
        "行首空格 → Tab",
        [
            "行首空格 → Tab",
            "行頭の空白 → Tab",
            "Leading Spaces → Tabs",
        ],
    ),
    // 整理行
    ("升序排序", ["升序排序", "昇順に並べ替え", "Sort Ascending"]),
    (
        "降序排序",
        ["降序排序", "降順に並べ替え", "Sort Descending"],
    ),
    (
        "合并连续重复行",
        [
            "合并连续重复行",
            "連続する重複行を結合",
            "Merge Consecutive Duplicate Lines",
        ],
    ),
    (
        "去重 (保留首次出现)",
        [
            "去重 (保留首次出现)",
            "重複を削除 (最初を残す)",
            "Remove Duplicates (keep first)",
        ],
    ),
    (
        "反转行顺序",
        ["反转行顺序", "行の順序を反転", "Reverse Lines"],
    ),
    (
        "去除行尾空白",
        [
            "去除行尾空白",
            "行末の空白を削除",
            "Trim Trailing Whitespace",
        ],
    ),
    (
        "去除行首空白",
        [
            "去除行首空白",
            "行頭の空白を削除",
            "Trim Leading Whitespace",
        ],
    ),
    ("删除空行", ["删除空行", "空行を削除", "Delete Empty Lines"]),
    // 缩进
    (
        "增加缩进",
        ["增加缩进", "インデントを増やす", "Increase Indent"],
    ),
    (
        "减少缩进",
        ["减少缩进", "インデントを減らす", "Decrease Indent"],
    ),
    // 其它
    (
        "切换行注释",
        ["切换行注释", "行コメントの切り替え", "Toggle Line Comment"],
    ),
    ("转到行…", ["转到行…", "行へ移動…", "Go to Line…"]),
    (
        "改写 / 插入模式",
        [
            "改写 / 插入模式",
            "上書き / 挿入モード",
            "Overwrite / Insert Mode",
        ],
    ),
    ("只读", ["只读", "読み取り専用", "Read-only"]),
    // Markdown
    ("粗体", ["粗体", "太字", "Bold"]),
    ("斜体", ["斜体", "斜体", "Italic"]),
    ("删除线", ["删除线", "取り消し線", "Strikethrough"]),
    ("行内代码", ["行内代码", "インラインコード", "Inline Code"]),
    ("高亮", ["高亮", "ハイライト", "Highlight"]),
    ("标题 1", ["标题 1", "見出し 1", "Heading 1"]),
    ("标题 2", ["标题 2", "見出し 2", "Heading 2"]),
    ("标题 3", ["标题 3", "見出し 3", "Heading 3"]),
    ("标题 4", ["标题 4", "見出し 4", "Heading 4"]),
    ("引用", ["引用", "引用", "Quote"]),
    ("任务", ["任务", "タスク", "Task"]),
    ("无序列表", ["无序列表", "箇条書き", "Bulleted List"]),
    ("有序列表", ["有序列表", "番号付きリスト", "Numbered List"]),
    ("任务列表", ["任务列表", "タスクリスト", "Task List"]),
    ("链接", ["链接", "リンク", "Link"]),
    ("图片", ["图片", "画像", "Image"]),
    ("代码块", ["代码块", "コードブロック", "Code Block"]),
    (
        "Mermaid 图",
        ["Mermaid 图", "Mermaid 図", "Mermaid Diagram"],
    ),
    ("表格", ["表格", "表", "Table"]),
    ("分隔线", ["分隔线", "区切り線", "Horizontal Rule"]),
    ("提示块", ["提示块", "コールアウト", "Callout"]),
    (
        "协同审查看板",
        ["协同审查看板", "レビューボード", "Co-edit Review Dashboard"],
    ),
    (
        "模拟多 AI 协同编辑",
        ["模拟多 AI 协同编辑", "複数AI協調デモ", "Simulate Multi-AI Co-edit"],
    ),
    (
        "复制文件名",
        ["复制文件名", "ファイル名をコピー", "Copy File Name"],
    ),
    (
        "复制完整路径",
        ["复制完整路径", "フルパスをコピー", "Copy Full Path"],
    ),
    (
        "选到上一段落",
        [
            "选到上一段落",
            "前の段落まで選択",
            "Select to Previous Paragraph",
        ],
    ),
    (
        "选到下一段落",
        [
            "选到下一段落",
            "次の段落まで選択",
            "Select to Next Paragraph",
        ],
    ),
    (
        "下一处改动",
        ["下一处改动", "次の変更箇所", "Next Modified Line"],
    ),
    (
        "上一处改动",
        ["上一处改动", "前の変更箇所", "Previous Modified Line"],
    ),
    (
        "保存键盘宏…",
        ["保存键盘宏…", "キーボードマクロを保存…", "Save Key Macro…"],
    ),
    (
        "载入键盘宏…",
        [
            "载入键盘宏…",
            "キーボードマクロを読み込み…",
            "Load Key Macro…",
        ],
    ),
    (
        "向前查找当前词",
        [
            "向前查找当前词",
            "前方のカーソル語を検索",
            "Find Word Backward",
        ],
    ),
    (
        "与其他标签比较…",
        ["与其他标签比较…", "他のタブと比較…", "Compare with Tab…"],
    ),
    (
        "与文件比较…",
        ["与文件比较…", "ファイルと比較…", "Compare with File…"],
    ),
    ("差异列表", ["差异列表", "差分一覧", "Diff List"]),
    ("下一处差异", ["下一处差异", "次の差分", "Next Difference"]),
    (
        "上一处差异",
        ["上一处差异", "前の差分", "Previous Difference"],
    ),
    ("清除比较", ["清除比较", "比較をクリア", "Clear Comparison"]),
    (
        "句读挂在行尾 (ぶら下げ)",
        [
            "句读挂在行尾 (ぶら下げ)",
            "句読点をぶら下げる",
            "Hanging punctuation (burasage)",
        ],
    ),
    (
        "行头 / 行尾禁则 (指定列数折行时)",
        [
            "行头 / 行尾禁则 (指定列数折行时)",
            "行頭・行末禁則 (桁指定の折り返し時)",
            "Line-start/end prohibition (fixed-column wrap)",
        ],
    ),
    (
        "已取消拖放",
        ["已取消拖放", "ドラッグを取り消しました", "Drag cancelled"],
    ),
    ("置换", ["置换", "置換", "Replace"]),
    (
        "显示 / 隐藏置换行 (Ctrl+H)",
        [
            "显示 / 隐藏置换行 (Ctrl+H)",
            "置換行の表示 / 非表示 (Ctrl+H)",
            "Show / hide replace row (Ctrl+H)",
        ],
    ),
    ("查找", ["查找", "検索", "Find"]),
    ("下一个 F3", ["下一个 F3", "次へ F3", "Next F3"]),
    (
        "上一个 Shift+F3",
        ["上一个 Shift+F3", "前へ Shift+F3", "Previous Shift+F3"],
    ),
    (
        "区分大小写",
        ["区分大小写", "大文字小文字を区別", "Match case"],
    ),
    ("单词单位", ["单词单位", "単語単位", "Whole word"]),
    ("正则", ["正则", "正規表現", "Regex"]),
    ("折返", ["折返", "先頭に戻って検索", "Wrap around"]),
    (
        "用罗马字找日文: kensaku 也能找到 検索 / けんさく / ケンサク",
        [
            "用罗马字找日文: kensaku 也能找到 検索 / けんさく / ケンサク",
            "ローマ字で日本語を検索: kensaku で 検索 / けんさく / ケンサク も見つかる",
            "Find Japanese by romaji: kensaku also finds 検索 / けんさく / ケンサク",
        ],
    ),
    ("全部强调", ["全部强调", "すべて強調", "Highlight all"]),
    ("关闭 Esc", ["关闭 Esc", "閉じる Esc", "Close Esc"]),
    ("下一个", ["下一个", "次へ", "Next"]),
    ("上一个", ["上一个", "前へ", "Previous"]),
    ("关闭", ["关闭", "閉じる", "Close"]),
    (
        "置换当前命中并找下一个",
        [
            "置换当前命中并找下一个",
            "現在の一致を置換して次を検索",
            "Replace this match and find next",
        ],
    ),
    ("全部置换", ["全部置换", "すべて置換", "Replace all"]),
    ("仅选区", ["仅选区", "選択範囲のみ", "Selection only"]),
    (
        "搜索内容 (空 = 只按文件名找)",
        [
            "搜索内容 (空 = 只按文件名找)",
            "検索文字列 (空 = ファイル名のみ)",
            "Search text (empty = file names only)",
        ],
    ),
    (
        "文件名: *.rs;*.toml  排除 !*.gen.rs  排除目录 #target",
        [
            "文件名: *.rs;*.toml  排除 !*.gen.rs  排除目录 #target",
            "ファイル: *.rs;*.toml  除外 !*.gen.rs  除外フォルダー #target",
            "Files: *.rs;*.toml  exclude !*.gen.rs  exclude dir #target",
        ],
    ),
    (
        "文件夹 / 排除 / 字符集",
        [
            "文件夹 / 排除 / 字符集",
            "フォルダー / 除外 / 文字コード",
            "Folders / exclusions / charset",
        ],
    ),
    (
        "文件夹 (留空 = 整个项目, 多个用 ; 分隔)",
        [
            "文件夹 (留空 = 整个项目, 多个用 ; 分隔)",
            "フォルダー (空 = プロジェクト全体, 複数は ; 区切り)",
            "Folders (empty = whole project, separate with ;)",
        ],
    ),
    (
        "当前文件所在文件夹",
        [
            "当前文件所在文件夹",
            "現在のファイルのフォルダー",
            "Folder of current file",
        ],
    ),
    (
        "排除文件, 如 *.min.js",
        [
            "排除文件, 如 *.min.js",
            "除外ファイル (例: *.min.js)",
            "Exclude files, e.g. *.min.js",
        ],
    ),
    (
        "排除文件夹, 如 target;dist",
        [
            "排除文件夹, 如 target;dist",
            "除外フォルダー (例: target;dist)",
            "Exclude folders, e.g. target;dist",
        ],
    ),
    ("字符集", ["字符集", "文字コード", "Charset"]),
    ("自动", ["自动", "自動", "Auto"]),
    ("子文件夹", ["子文件夹", "サブフォルダー", "Subfolders"]),
    ("项目外", ["项目外", "プロジェクト外", "Outside project"]),
    (
        "仅当前文件",
        ["仅当前文件", "現在のファイルのみ", "Current file only"],
    ),
    ("输出", ["输出", "出力", "Output"]),
    ("标准", ["标准", "標準", "Normal"]),
    (
        "仅文件名",
        ["仅文件名", "ファイル名のみ", "File names only"],
    ),
    (
        "各文件命中数",
        ["各文件命中数", "ファイル別の件数", "Hits per file"],
    ),
    ("停止  Esc", ["停止  Esc", "中止  Esc", "Stop  Esc"]),
    ("搜索", ["搜索", "検索", "Search"]),
    (
        "结果开成标签",
        ["结果开成标签", "結果をタブで開く", "Open results in tab"],
    ),
    (
        "把结果放进一个只读标签: 可复制、可 Ctrl+F, Enter / 双击命中行跳转",
        [
            "把结果放进一个只读标签: 可复制、可 Ctrl+F, Enter / 双击命中行跳转",
            "結果を読み取り専用タブに出力: コピー・Ctrl+F 可, Enter / ダブルクリックでジャンプ",
            "Put results in a read-only tab: copy, Ctrl+F; Enter / double-click jumps",
        ],
    ),
    (
        "置换为 (正则下可用 $1)",
        [
            "置换为 (正则下可用 $1)",
            "置換後 (正規表現では $1 が使える)",
            "Replace with (use $1 in regex mode)",
        ],
    ),
    (
        "全部置换 (改写文件, 保留 .bak)",
        [
            "全部置换 (改写文件, 保留 .bak)",
            "すべて置換 (ファイルを書き換え, .bak を残す)",
            "Replace all (rewrites files, keeps .bak)",
        ],
    ),
    ("文件", ["文件", "ファイル", "Files"]),
    ("大纲", ["大纲", "アウトライン", "Outline"]),
    ("扩展", ["扩展", "拡張機能", "Extensions"]),
    ("资源管理器", ["资源管理器", "エクスプローラー", "Explorer"]),
    ("源代码管理", ["源代码管理", "ソース管理", "Source Control"]),
    (
        "打开文件夹…",
        ["打开文件夹…", "フォルダーを開く…", "Open Folder..."],
    ),
    ("新建文件…", ["新建文件…", "新規ファイル…", "New File..."]),
    (
        "新建文件夹…",
        ["新建文件夹…", "新規フォルダー…", "New Folder..."],
    ),
    ("刷新", ["刷新", "更新", "Refresh"]),
    (
        "输入命令按 Enter 启动 shell, 或点「+ 新建终端」",
        [
            "输入命令按 Enter 启动 shell, 或点「+ 新建终端」",
            "コマンドを入力して Enter でシェルを起動、または「+ 新しいターミナル」",
            "Type a command and press Enter to start a shell, or click \"+ New terminal\"",
        ],
    ),
    (
        "文件加锁…",
        ["文件加锁…", "ファイルのロック…", "File locking…"],
    ),
    (
        "执行外部命令 (终端)",
        [
            "执行外部命令 (终端)",
            "外部コマンドを実行 (ターミナル)",
            "Run external command (terminal)",
        ],
    ),
    (
        "(没有可显示的大纲)",
        [
            "(没有可显示的大纲)",
            "(表示できるアウトラインがありません)",
            "(no outline to show)",
        ],
    ),
    ("确定", ["确定", "OK", "OK"]),
    ("取消", ["取消", "キャンセル", "Cancel"]),
    ("没有改动", ["没有改动", "変更なし", "No changes"]),
    (
        "已暂存的更改",
        ["已暂存的更改", "ステージ済みの変更", "Staged changes"],
    ),
    ("更改", ["更改", "変更", "Changes"]),
    ("差异", ["差异", "差分", "Diff"]),
    ("取消暂存", ["取消暂存", "ステージ解除", "Unstage"]),
    ("暂存", ["暂存", "ステージ", "Stage"]),
    ("重命名…", ["重命名…", "名前の変更…", "Rename..."]),
    ("删除…", ["删除…", "削除…", "Delete..."]),
    ("比较", ["比较", "比較", "Compare"]),
    (
        "下一处差异 (Alt+↓)",
        [
            "下一处差异 (Alt+↓)",
            "次の差分 (Alt+↓)",
            "Next Difference (Alt+↓)",
        ],
    ),
    (
        "上一处差异 (Alt+↑)",
        [
            "上一处差异 (Alt+↑)",
            "前の差分 (Alt+↑)",
            "Previous Difference (Alt+↑)",
        ],
    ),
    (
        "与哪个标签比较",
        ["与哪个标签比较", "比較するタブ", "Compare with which tab"],
    ),
    (
        "需要至少两个标签",
        [
            "需要至少两个标签",
            "タブが 2 つ以上必要です",
            "Needs at least two tabs",
        ],
    ),
    (
        "没有进行中的比较",
        [
            "没有进行中的比较",
            "比較中のものはありません",
            "No comparison in progress",
        ],
    ),
    ("终端", ["终端", "ターミナル", "Terminal"]),
    ("清屏", ["清屏", "クリア", "Clear"]),
    ("重启", ["重启", "再起動", "Restart"]),
    (
        "结束当前 shell, 下一条命令重新开一个 (Ctrl+C)",
        [
            "结束当前 shell, 下一条命令重新开一个 (Ctrl+C)",
            "現在のシェルを終了し, 次のコマンドで開き直す (Ctrl+C)",
            "End the current shell; the next command starts a new one (Ctrl+C)",
        ],
    ),
    (
        "常驻会话",
        ["常驻会话", "常駐セッション", "Persistent session"],
    ),
    (
        "选区作标准输入",
        [
            "选区作标准输入",
            "選択範囲を標準入力に",
            "Selection as stdin",
        ],
    ),
    (
        "输出替换选区",
        [
            "输出替换选区",
            "出力で選択範囲を置換",
            "Output replaces selection",
        ],
    ),
    ("运行中…", ["运行中…", "実行中…", "Running..."]),
    (
        "输入命令, Enter 执行",
        [
            "输入命令, Enter 执行",
            "コマンドを入力, Enter で実行",
            "Type a command, Enter to run",
        ],
    ),
    ("正则无效", ["正则无效", "正規表現が無効", "Invalid regex"]),
    (
        "增量查找",
        ["增量查找", "インクリメンタル検索", "Incremental search"],
    ),
    ("向后", ["向后", "下方向", "Forward"]),
    ("向前", ["向前", "上方向", "Backward"]),
    (
        "指定关键字的标签跳转",
        [
            "指定关键字的标签跳转",
            "キーワード指定タグジャンプ",
            "Tag jump by keyword",
        ],
    ),
    (
        "选择跳转位置",
        ["选择跳转位置", "ジャンプ先を選択", "Choose a target"],
    ),
    (
        "半角 → 全角 (含假名)",
        [
            "半角 → 全角 (含假名)",
            "半角 → 全角 (かな含む)",
            "Half → Full-width (incl. Kana)",
        ],
    ),
    (
        "全角 → 半角 (含假名)",
        [
            "全角 → 半角 (含假名)",
            "全角 → 半角 (かな含む)",
            "Full → Half-width (incl. Kana)",
        ],
    ),
    (
        "转为全角平假名",
        ["转为全角平假名", "全角ひらがなへ", "To Full-width Hiragana"],
    ),
    (
        "转为半角片假名",
        ["转为半角片假名", "半角カタカナへ", "To Half-width Katakana"],
    ),
    (
        "半角假名 → 全角片假名",
        [
            "半角假名 → 全角片假名",
            "半角カナ → 全角カタカナ",
            "Half Kana → Full Katakana",
        ],
    ),
    (
        "半角假名 → 全角平假名",
        [
            "半角假名 → 全角平假名",
            "半角カナ → 全角ひらがな",
            "Half Kana → Full Hiragana",
        ],
    ),
    (
        "半角英数 → 全角英数",
        [
            "半角英数 → 全角英数",
            "半角英数 → 全角英数",
            "Half → Full-width Alphanumerics",
        ],
    ),
    (
        "全角英数 → 半角英数",
        [
            "全角英数 → 半角英数",
            "全角英数 → 半角英数",
            "Full → Half-width Alphanumerics",
        ],
    ),
    ("编码转换", ["编码转换", "コード変換", "Convert Encoding"]),
    (
        "自动判别 → SJIS",
        ["自动判别 → SJIS", "自動判別 → SJIS", "Auto-detect → SJIS"],
    ),
    (
        "邮件 (JIS) → SJIS",
        [
            "邮件 (JIS) → SJIS",
            "E-Mail (JIS) → SJIS",
            "E-Mail (JIS) → SJIS",
        ],
    ),
    ("EUC → SJIS", ["EUC → SJIS", "EUC → SJIS", "EUC → SJIS"]),
    (
        "Unicode → SJIS",
        ["Unicode → SJIS", "Unicode → SJIS", "Unicode → SJIS"],
    ),
    (
        "UnicodeBE → SJIS",
        ["UnicodeBE → SJIS", "UnicodeBE → SJIS", "UnicodeBE → SJIS"],
    ),
    (
        "UTF-8 → SJIS",
        ["UTF-8 → SJIS", "UTF-8 → SJIS", "UTF-8 → SJIS"],
    ),
    (
        "UTF-7 → SJIS",
        ["UTF-7 → SJIS", "UTF-7 → SJIS", "UTF-7 → SJIS"],
    ),
    ("SJIS → JIS", ["SJIS → JIS", "SJIS → JIS", "SJIS → JIS"]),
    ("SJIS → EUC", ["SJIS → EUC", "SJIS → EUC", "SJIS → EUC"]),
    (
        "SJIS → UTF-8",
        ["SJIS → UTF-8", "SJIS → UTF-8", "SJIS → UTF-8"],
    ),
    (
        "SJIS → UTF-7",
        ["SJIS → UTF-7", "SJIS → UTF-7", "SJIS → UTF-7"],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    use super::TEST_LOCK as LANG_LOCK;

    #[test]
    fn tables_are_complete_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for (k, row) in T.iter().chain(CMD_T.iter()) {
            assert!(row.iter().all(|s| !s.is_empty()), "{k} 缺翻译");
            assert!(row.iter().all(|s| !s.contains('\u{FFFD}')), "{k} 含乱码");
        }
        for (k, _) in T {
            assert!(seen.insert(*k), "重复的键 {k}");
        }
        let mut seen = std::collections::HashSet::new();
        for (k, _) in CMD_T {
            assert!(seen.insert(*k), "命令表重复的键 {k}");
        }
    }

    #[test]
    fn lookup_follows_language_and_falls_back() {
        let _g = LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set(Lang::Zh);
        assert_eq!(tr("menu.file"), "文件");
        set(Lang::Ja);
        assert_eq!(tr("menu.file"), "ファイル");
        set(Lang::En);
        assert_eq!(tr("menu.file"), "File");
        assert_eq!(tr("no.such.key"), "no.such.key", "没收录的键原样返回");
        assert_eq!(trz("切换行注释"), "Toggle Line Comment");
        assert_eq!(trz("没收录的中文"), "没收录的中文");
        assert_eq!(trf("st.lines", &["12"]), "12 lines");
        assert_eq!(
            trf("dlg.big-body", &["a.txt", "9"]),
            "a.txt\nAbout 9 MB; opening may be slow. Continue?"
        );
        assert_eq!(lk("c.save", "Ctrl+S"), "Save  Ctrl+S");
        set_code("ja");
        assert_eq!(cur(), Lang::Ja);
        set_code("garbage");
        assert_eq!(cur(), Lang::Zh, "认不出的语言码 = 中文");
        set(Lang::Zh);
    }

    #[test]
    fn language_codes_roundtrip() {
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
    }

    /// 命令表里每个标签和分组都得有翻译, 以后加命令忘了补这里会在这儿红
    #[test]
    fn every_command_label_is_translated() {
        for c in crate::cmds::CMDS {
            assert!(
                zh_index().contains_key(c.label),
                "命令标签缺翻译: {}",
                c.label
            );
            if !c.group.is_empty() {
                assert!(
                    zh_index().contains_key(c.group),
                    "命令分组缺翻译: {}",
                    c.group
                );
            }
        }
    }

    /// 代码里用到的每个 `tr("...")` / `lk("...")` / `trf("...")` 键都得在表里
    #[test]
    fn every_key_used_in_source_exists() {
        let srcs = [
            include_str!("app.rs"),
            include_str!("find.rs"),
            include_str!("sidebar.rs"),
            include_str!("terminal.rs"),
            include_str!("cmdui.rs"),
            include_str!("keymap.rs"),
            include_str!("prefs.rs"),
            include_str!("updnet.rs"),
            include_str!("app/updateui.rs"),
        ];
        let idx = index();
        let mut n = 0;
        for src in srcs {
            for f in ["tr(\"", "lk(\"", "trf(\""] {
                let mut rest = src;
                while let Some(p) = rest.find(f) {
                    // 排除 xxx_tr(" 这类别的函数名
                    let before = rest[..p].chars().last();
                    rest = &rest[p + f.len()..];
                    if before
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let end = rest.find('"').unwrap();
                    let key = &rest[..end];
                    assert!(idx.contains_key(key), "代码里用了表里没有的键: {key}");
                    n += 1;
                }
            }
        }
        assert!(n > 50, "应当扫到不少键 (实际 {n})");
    }
    // ---------------------------------------------------------------- 翻译覆盖 (以后新增文字忘了补翻译会在这里红)

    fn has_han(s: &str) -> bool {
        s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    }

    /// 把源码里的字符串字面量还原成运行时的样子 (只处理这里用到的几种转义)
    fn unescape(s: &str) -> String {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match it.next() {
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        }
        out
    }

    /// 读 `from` 开头的字面量 (`from` 之后紧跟的第一个 `"` 起, 到没被转义的下一个 `"` 止)
    fn literal_after(src: &str, at: usize) -> Option<String> {
        let rest = &src[at..];
        let open = rest.find('"')?;
        let body = &rest[open + 1..];
        let mut esc = false;
        for (i, c) in body.char_indices() {
            if esc {
                esc = false;
                continue;
            }
            if c == '\\' {
                esc = true;
                continue;
            }
            if c == '"' {
                return Some(unescape(&body[..i]));
            }
        }
        None
    }

    /// 所有非测试源码 (去掉每个文件 `#[cfg(test)]` 之后的部分)
    fn sources() -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                    continue;
                }
                if p.extension().is_some_and(|x| x == "rs") {
                    let s = std::fs::read_to_string(&p).unwrap_or_default();
                    let live = s.split("#[cfg(test)]").next().unwrap_or("").to_string();
                    out.push((p.display().to_string(), live));
                }
            }
        }
        let mut v = vec![];
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut v,
        );
        v
    }

    /// 命令表里每个命令的标签, 在日 / 英界面都不能还是中文
    #[test]
    fn every_registry_label_is_translated_in_every_language() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut bad = vec![];
        set(Lang::En);
        for e in crate::keymap::entries() {
            let t = e.label.text();
            if has_han(t) {
                bad.push(format!("{} ({})", t, e.id));
            }
        }
        set(Lang::Zh);
        assert!(bad.is_empty(), "命令标签缺翻译: {bad:?}");
    }

    /// 代码里每个 `trz("中文")` / `app_label(KApp::X, "中文")` 都得在翻译表里
    #[test]
    fn every_trz_literal_exists_in_the_table() {
        let mut missing = vec![];
        for (file, src) in sources() {
            if file.ends_with("i18n.rs") {
                continue;
            }
            for pat in ["trz(\"", "app_label(KApp::"] {
                let mut from = 0;
                while let Some(p) = src[from..].find(pat) {
                    let at = from + p;
                    from = at + pat.len();
                    let before = src[..at].chars().last();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    // app_label 的字面量在逗号之后; trz 的紧跟在括号后
                    let start = if pat.starts_with("trz") {
                        at + pat.len() - 1
                    } else {
                        match src[at..].find(',') {
                            Some(c) => at + c,
                            None => continue,
                        }
                    };
                    let Some(lit) = literal_after(&src, start) else {
                        continue;
                    };
                    if has_han(&lit) && !zh_index().contains_key(lit.as_str()) {
                        missing.push(format!(
                            "{}: {lit}",
                            file.rsplit('/').next().unwrap_or(&file)
                        ));
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "这些中文没有翻译表条目: {missing:#?}");
    }

    /// 代码里每条直接写进 `tip` 的中文提示 (含 `format!` 的模板) 都得能被 msgtr 翻成日 / 英
    #[test]
    fn every_literal_status_tip_has_a_message_template() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut missing = vec![];
        for (file, src) in sources() {
            if file.ends_with("i18n.rs") || file.ends_with("msgtr.rs") {
                continue;
            }
            let mut from = 0;
            while let Some(p) = src[from..].find("tip = ") {
                let at = from + p + "tip = ".len();
                from = at;
                let head = src[at..].trim_start();
                let lit_from = if head.starts_with("format!(") {
                    at + src[at..].find("format!(").unwrap_or(0)
                } else if head.starts_with('"') {
                    at
                } else {
                    continue;
                };
                let Some(lit) = literal_after(&src, lit_from) else {
                    continue;
                };
                if !has_han(&lit) {
                    continue;
                }
                // `{}` / `{name}` / `{n:>3}` 都当成一个占位
                let mut sample = String::new();
                let mut depth = 0;
                for c in lit.chars() {
                    match c {
                        '{' => {
                            depth += 1;
                            if depth == 1 {
                                sample.push('x');
                            }
                        }
                        '}' => depth = (depth - 1).max(0),
                        _ if depth == 0 => sample.push(c),
                        _ => {}
                    }
                }
                set(Lang::En);
                let out = crate::msgtr::msg(&sample).into_owned();
                set(Lang::Zh);
                if has_han(&out) {
                    missing.push(format!(
                        "{}: {lit}",
                        file.rsplit('/').next().unwrap_or(&file)
                    ));
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "这些状态栏提示没有 msgtr 模板: {missing:#?}"
        );
    }
}
