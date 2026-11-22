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

/// 自动检测系统当前语言 (根据环境变量以及 Windows 平台系统区域设置)
pub fn detect_system_lang() -> Lang {
    // 1. 优先读取标准环境变量 (支持手动测试覆盖与跨平台配置)
    for var in ["IDE_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim().to_lowercase();
            if v.starts_with("zh") {
                return Lang::Zh;
            } else if v.starts_with("ja") {
                return Lang::Ja;
            } else if v.starts_with("en") {
                return Lang::En;
            }
        }
    }

    // 2. Windows 平台安全检测用户默认区域与 UI 语言 (纯 Safe Rust)
    if let Some(loc) = editcore::theme_luminance::read_user_locale_name() {
        let s = loc.trim().to_lowercase();
        if s.starts_with("zh") {
            return Lang::Zh;
        } else if s.starts_with("ja") {
            return Lang::Ja;
        } else {
            return Lang::En;
        }
    }

    Lang::Zh
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
#[inline]
pub fn trz(zh: &'static str) -> &'static str {
    if matches!(cur(), Lang::Zh) {
        return zh;
    }
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
    ("c.goto-line", ["转到行…", "指定行へ移動…", "Go to Line…"]),
    ("sub.isearch", ["增量查找", "インクリメンタルサーチ", "Incremental Search"]),
    ("sub.tags", ["标签与符号跳转", "ダイレクトタグジャンプ", "Tags & Symbol Jump"]),
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
    ("c.language", ["当前文件语言…", "ファイルの言語 (タイプ別設定一覧)…", "File Language…"]),
    ("c.type-config", ["类型别设置…", "タイプ別設定…", "Type Settings…"]),
    ("c.font", ["字体设置…", "フォント設定…", "Font Settings…"]),
    ("sub.layout", ["界面布局", "レイアウト", "Layout"]),
    ("sub.dock-panels", ["面板与折叠", "パネルと折りたたみ", "Panels & Docks"]),
    ("sub.ui-bars", ["界面元素", "UIバー", "UI Elements"]),
    ("sub.text-view", ["文本显示", "テキスト表示", "Text Display"]),
    ("sub.zoom", ["缩放", "拡大・縮小", "Zoom"]),
    ("sub.wrap-rules", ["折行规则", "折り返しルール", "Wrap Rules"]),
    ("sub.wrap-mode", ["换行方式", "折り返し方法", "Wrap Mode"]),
    ("c.wrap-none", ["不折行", "折り返さない", "Do Not Wrap"]),
    ("c.wrap-window", ["按窗口宽度折行", "右端で折り返す", "Wrap at Window Width"]),
    ("c.wrap-col", ["按指定列数折行…", "指定桁数で折り返す…", "Wrap at Column…"]),
    ("pf.wrap-col-setting", ["详细换行设置…", "折り返し詳細設定…", "Wrap Settings…"]),
    ("c.minimap-toggle", ["隐藏 / 显示缩略图", "ミニマップの表示/非表示", "Toggle Minimap"]),
    ("c.minimap-collapse", ["折叠缩略图", "ミニマップを折りたたむ", "Collapse Minimap"]),
    ("c.minimap-expand", ["展开缩略图", "ミニマップを展開", "Expand Minimap"]),
    ("c.completion", ["代码补全", "コード補完", "Code Completion"]),
    ("c.goto-def", ["跳转到定义", "定義へジャンプ", "Go to Definition"]),
    ("sub.theme", ["主题", "テーマ", "Theme"]),
    ("c.theme-select", ["主题…", "テーマ選択…", "Theme…"]),
    ("c.theme-crystal", ["透明水晶", "透明クリスタル", "Transparent Crystal"]),
    ("c.theme-clear", ["透明玻璃", "透明ガラス", "Clear Glass"]),
    ("c.theme-dark", ["深色", "ダーク", "Dark"]),
    ("c.theme-light", ["浅色", "ライト", "Light"]),
    ("c.theme-high-contrast", ["高对比度", "ハイコントラスト", "High Contrast"]),
    ("c.theme-retro16", ["复古 16-Bit", "レトロ 16-Bit", "Retro 16-Bit"]),
    ("c.theme-comic", ["美漫 3渲2", "アメコミ 3Dセル調", "Comic Cel-Shaded"]),
    ("sub.uilang", ["界面语言 / 言語 / Language", "表示言語 / 言語 / Language", "Display Language / 言語 / 语言"]),
    ("sub.eolmode", ["换行模式", "改行コード", "Line Endings"]),
    ("sub.charset", ["字符编码", "文字コード", "Character Encoding"]),
    ("c.file-lock-toggle", ["文件加锁 (只读)", "ファイルロック (読み取り専用)", "File Lock (Read-only)"]),
    ("c.export-config", ["导出配置…", "設定のエクスポート…", "Export Settings…"]),
    ("c.import-config", ["导入配置…", "設定のインポート…", "Import Settings…"]),
    ("c.import-sakura", ["导入 ini 配置…", "ini 設定をインポート…", "Import ini settings…"]),
    ("c.reset-settings", ["恢复默认设置", "初期設定に戻す", "Reset to Defaults"]),
    // ---- 书签
    ("sub.bookmark", ["书签", "ブックマーク", "Bookmarks"]),
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
    ("c.split-off", ["取消分割", "分割解除", "Unsplit Window"]),
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
    ("c.macro-run-doc", ["运行当前文档 (宏脚本 .mac)", "現在の文書を実行 (マクロ .mac)", "Run Current Document (Macro .mac)"]),
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
    ("c.nema-convert", ["自动转化 .nema", "NEMA 自動変換", "Auto Convert to NEMA"]),
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
    ("c.minimap", ["缩略图 (小地图)", "縮小マップ / ミニマップ", "Minimap / Thumbnail"]),
    ("c.full-mode", ["完整界面 (活动栏 + 侧栏 + 标签栏)", "フル表示 (アクティビティバー + サイドバー + タブ)", "Full Layout (rail + side bar + tabs)"]),
    ("c.minimal-mode", ["极简布局 (仅编辑区)", "ミニマル表示 (編集領域のみ)", "Minimal Layout (editor only)"]),
    ("st.char-col", ["字符列 {}", "文字位置 {}", "Character column {}"]),
    ("st.selected", ["已选 {} 字符 / {} 行", "{} 文字 / {} 行選択", "{} chars / {} lines selected"]),
    ("st.selected-bytes", ["已选 {} 字节 (UTF-8) / {} 行", "{} バイト (UTF-8) / {} 行選択", "{} bytes (UTF-8) / {} lines selected"]),
    ("st.selected-lines", ["已选 {} 行", "{} 行選択", "{} lines selected"]),
    ("st.sel-hint", ["点击切换统计方式: 字符 / 字节 / 行", "クリックで切り替え: 文字 / バイト / 行", "Click to switch: chars / bytes / lines"]),
    ("st.plain", ["纯文本", "テキスト", "Plain Text"]),
    // ---- 帮助
    ("ui.about", ["IDE1.0 — 原生 Rust (egui)\n编辑语义由 editcore 驱动", "IDE1.0 — ネイティブ Rust (egui)\n編集の挙動は editcore 駆動", "IDE1.0 — native Rust (egui)\nEditing semantics powered by editcore"]),
    // ---- 标签右键 / 分组
    ("c.close", ["关闭", "閉じる", "Close"]),
    ("c.close-others-s", ["关闭其它", "他を閉じる", "Close Others"]),
    ("c.close-left-s", ["关闭左侧", "左を閉じる", "Close Left"]),
    ("c.close-right-s", ["关闭右侧", "右を閉じる", "Close Right"]),
    ("c.copy-path", ["复制路径", "パスをコピー", "Copy Path"]),
    ("c.key-config", ["键位设置…", "キー設定…", "Key Bindings…"]),
    ("c.key-list", ["键位一览", "キー一覧", "Key List"]),
    ("c.palette", ["命令面板…", "コマンドパレット…", "Command Palette…"]),
    ("c.sticky-notes", ["便笺 (Sticky Notes)", "付箋 (Sticky Notes)", "Sticky Notes"]),
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
    ("pf.macro-mac", ["标准宏 (.mac)", "標準マクロ (.mac)", "Standard Macro (.mac)"]),
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
    ("ai.key-note", ["密钥以明文保存在本机用户设置目录的 ai.json 中, 不写日志; 只在点发送时联网 (https)。", "キーは平文でこの PC のユーザー設定フォルダの ai.json に保存されます。ログには出ません。送信時のみ https で通信します。", "The API key is stored in plain text in ai.json in your per-user settings folder, never logged; network is used only on Send (https)."]),
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
    ("pf.tray-resident", ["系统托盘常驻守护 (关闭窗口时最小化到托盘)", "タスクトレイ常駐 (閉じる時に最小化)", "System tray residency (minimize to tray on close)"]),
    ("pf.tray-resident-tip", ["开启后关闭窗口不退出程序，保持托盘常驻秒级热唤醒", "オンにすると閉じるボタンで終了せず、トレイに常駐して高速起動します", "Keep resident in system tray for instant hot-wakeup instead of exiting"]),
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
    ("pf.save-backup", ["保存时备份旧文件 (.backup)", "保存時に旧ファイルをバックアップ (.backup)", "Back up the old file on save (.backup)"]),
    ("pf.check-external", ["窗口获得焦点时检查外部修改", "ウィンドウ復帰時に外部変更を確認", "Check for external changes on focus"]),
    ("pf.restore-session", ["启动时恢复上次打开的文件", "起動時に前回開いていたファイルを復元", "Reopen last session's files on startup"]),
    ("c.about", ["关于…", "バージョン情報…", "About…"]),
    ("c.wiki", ["IDE1.0 官方 Wiki (在线文档)", "IDE1.0 公式 Wiki (オンラインマニュアル)", "IDE1.0 Official Wiki"]),
    ("dlg.file-locked", ["文件锁: 已加锁 (只读)", "ファイルロック: ロック中 (読み取り専用)", "File Lock: Locked (Read-only)"]),
    ("dlg.file-unlocked", ["文件锁: 已解锁", "ファイルロック: 解除", "File Lock: Unlocked"]),
    ("dlg.about-title", ["关于 IDE1.0", "IDE1.0 について", "About IDE1.0"]),
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
    ("pf.font-family", ["编辑器主字体", "エディター主フォント", "Editor font"]),
    ("pf.font-default", ["(主题风格自适应默认字体)", "(テーマ適合の既定フォント)", "(theme adaptive default)"]),
    ("pf.font-cjk-fallback", ["中日英字体优先 (CJK Fallback)", "CJKフォント優先", "CJK Fallback Font"]),
    ("pf.font-cjk-auto", ["(智能最优匹配: 思源/UD/微黑)", "(自動最適: 思源/UD/游ゴシック)", "(Auto: Noto/UD/YaHei)"]),
    ("pf.font-bold", ["粗体 (Bold / 太字)", "太字 (Bold)", "Bold"]),
    ("pf.font-italic", ["斜体 (Italic)", "斜体 (Italic)", "Italic"]),
    ("pf.letter-spacing", ["字符间距 (文字间距)", "文字間隔", "Letter spacing"]),
    ("pf.cjk-fixed-ratio", ["全角/半角 1:2 网格对齐 (经典等宽)", "全角半角 1:2 等幅整列 (クラシック)", "CJK 1:2 Monospace Grid (Classic)"]),
    ("pf.font-preview", ["效果实时预览 (经典代码)", "フォントプレビュー", "Live Font Preview"]),
    ("pf.font-rec-header", ["精选美观免费商用字体", "厳選商用フリー美麗フォント", "Recommended Free Commercial Fonts"]),
    ("pf.font-user-header", ["用户安装字体 (优先推荐)", "ユーザー導入フォント (優先)", "User Installed Fonts (Priority)"]),
    ("pf.font-all-header", ["系统安装字体", "システムフォント一覧", "System Fonts"]),
    ("pf.line-height", ["行间距 / 行高倍率", "行間・行送り倍率", "Line height / spacing"]),
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
    (
        "便笺 (Sticky Notes)",
        ["便笺 (Sticky Notes)", "付箋 (Sticky Notes)", "Sticky Notes"],
    ),
    (
        "更多快捷操作",
        [
            "更多快捷操作",
            "その他のショートカット",
            "More Quick Actions",
        ],
    ),
    (
        "更多快捷操作 (折叠)",
        [
            "更多快捷操作 (折叠)",
            "その他のショートカット (折りたたみ)",
            "More Quick Actions (Collapsed)",
        ],
    ),
    (
        "打开命令面板 (Ctrl+Shift+P / Shift+F1)",
        [
            "打开命令面板 (Ctrl+Shift+P / Shift+F1)",
            "コマンドパレットを開く (Ctrl+Shift+P / Shift+F1)",
            "Open Command Palette (Ctrl+Shift+P / Shift+F1)",
        ],
    ),
    (
        "欢迎与安装向导…",
        [
            "欢迎与安装向导…",
            "ようこそ・インストールウィザード…",
            "Welcome & Setup Wizard…",
        ],
    ),
    (
        "导出诊断与性能日志…",
        [
            "导出诊断与性能日志…",
            "診断およびパフォーマンスログを出力…",
            "Export Diagnostic & Perf Logs…",
        ],
    ),
    (
        "打开日志文件夹 (logs)…",
        [
            "打开日志文件夹 (logs)…",
            "ログフォルダを開く (logs)…",
            "Open Logs Folder (logs)…",
        ],
    ),
    (
        "后台部署线程异常退出，请重试",
        [
            "后台部署线程异常退出，请重试",
            "バックグラウンド配備スレッドが異常終了しました。再試行してください",
            "Background deployment thread terminated unexpectedly, please retry",
        ],
    ),
    (
        "复制主程序到目标目录失败",
        [
            "复制主程序到目标目录失败",
            "メインプログラムの対象ディレクトリへのコピーに失敗しました",
            "Failed to copy main binary to target directory",
        ],
    ),
    (
        "无法写入目标程序文件（可能需要管理员权限或文件正被占用）",
        [
            "无法写入目标程序文件（可能需要管理员权限或文件正被占用）",
            "対象ファイルに書き込めません（管理者権限が必要か、ファイルが使用中の可能性があります）",
            "Cannot write target binary (may require admin privileges or file is in use)",
        ],
    ),
    (
        "无法创建安装目录（可能需要管理员权限）",
        [
            "无法创建安装目录（可能需要管理员权限）",
            "インストールディレクトリを作成できません（管理者権限が必要な可能性があります）",
            "Cannot create install directory (may require admin privileges)",
        ],
    ),
    (
        "无法启动后台部署线程",
        [
            "无法启动后台部署线程",
            "バックグラウンド配備スレッドを開始できません",
            "Cannot spawn background deployment thread",
        ],
    ),
    (
        "未能更新用户 PATH，请手动添加安装目录",
        [
            "未能更新用户 PATH，请手动添加安装目录",
            "ユーザー PATH の更新に失敗しました。手動でインストール先を追加してください",
            "Failed to update user PATH, please add installation directory manually",
        ],
    ),
    (
        "未能更新用户 PATH，请手动移除安装目录",
        [
            "未能更新用户 PATH，请手动移除安装目录",
            "ユーザー PATH の更新に失敗しました。手動でインストール先を削除してください",
            "Failed to update user PATH, please remove installation directory manually",
        ],
    ),
    (
        "部署警告",
        [
            "部署警告",
            "配備警告",
            "Deployment Warning",
        ],
    ),
    (
        "日志自动维护 (LOG CLEANUP)",
        [
            "日志自动维护 (LOG CLEANUP)",
            "ログ自動保守 (LOG CLEANUP)",
            "Log Maintenance (LOG CLEANUP)",
        ],
    ),
    (
        "正在清理超过 7 天的历史诊断日志，请稍候...",
        [
            "正在清理超过 7 天的历史诊断日志，请稍候...",
            "7日以上前の診断ログをクリーンアップしています。しばらくお待ちください...",
            "Cleaning up diagnostic logs older than 7 days, please wait...",
        ],
    ),
    (
        "正在清理",
        [
            "正在清理",
            "クリーンアップ中",
            "Cleaning",
        ],
    ),
    (
        "正在准备清理任务...",
        [
            "正在准备清理任务...",
            "クリーンアップタスクを準備しています...",
            "Preparing cleanup task...",
        ],
    ),
    (
        "显示空白符",
        ["显示空白符", "空白文字を表示", "Show Whitespace"],
    ),
    (
        "已完成 Native 与 Nemat 压缩效益分析",
        [
            "已完成 Native 与 Nemat 压缩效益分析",
            "Native と Nemat の圧縮効果分析が完了しました",
            "Completed Native vs Nemat compression efficiency analysis",
        ],
    ),
    (
        "已扫描并列出高频词典候选词",
        [
            "已扫描并列出高频词典候选词",
            "頻出辞書候補を一覧表示しました",
            "Scanned and listed frequent dictionary candidates",
        ],
    ),
    (
        "显示字典与高频复用词位置",
        [
            "显示字典与高频复用词位置",
            "辞書参照と頻出再利用位置を表示",
            "Show dictionary & reused token positions",
        ],
    ),
    (
        "已高亮并列出文档内字典与复用位置",
        [
            "已高亮并列出文档内字典与复用位置",
            "文書内の辞書参照と再利用位置を一覧表示しました",
            "Listed dictionary references and reused positions",
        ],
    ),
    (
        "源代码文件已受保护：已在右侧新建 .nema 标签页，原文件未修改",
        [
            "源代码文件已受保护：已在右侧新建 .nema 标签页，原文件未修改",
            "ソースコードを保護：新規 .nema タブを作成しました（元ファイルは未変更）",
            "Source code file protected: opened converted .nema in a new tab",
        ],
    ),
    (
        "自动转化 / 编码为 NEMA (Ctrl+Alt+M)",
        [
            "自动转化 / 编码为 NEMA (Ctrl+Alt+M)",
            "NEMA に自動変換 / エンコード (Ctrl+Alt+M)",
            "Auto convert / encode to NEMA (Ctrl+Alt+M)",
        ],
    ),
    (
        "查看 Native vs Nemat Token 节省率与对比",
        [
            "查看 Native vs Nemat Token 节省率与对比",
            "Native と Nemat の Token 比較を表示",
            "View Native vs Nemat Token savings comparison",
        ],
    ),
    (
        "固化当前高频候选词并生成 ^N 引用",
        [
            "固化当前高频候选词并生成 ^N 引用",
            "辞書候補を固定して ^N 参照を生成",
            "Lock dictionary candidates and emit ^N references",
        ],
    ),
    (
        "扫描并查看文档内高频词典候选词",
        [
            "扫描并查看文档内高频词典候选词",
            "頻出辞書候補スキャン",
            "Scan and inspect dictionary candidates",
        ],
    ),
    (
        "自动转化 / 编码为 NEMA (Ctrl+Shift+M)",
        [
            "自动转化 / 编码为 NEMA (Ctrl+Shift+M)",
            "NEMA に自動変換 / エンコード (Ctrl+Shift+M)",
            "Auto convert / encode to NEMA (Ctrl+Shift+M)",
        ],
    ),
    (
        "插入 ^0 词典引用原子",
        [
            "插入 ^0 词典引用原子",
            "辞書参照 ^0 を挿入",
            "Insert ^0 dictionary reference atom",
        ],
    ),
    (
        "在文首插入共享词典条目 (\\t$N)",
        [
            "在文首插入共享词典条目 (\\t$N)",
            "ファイル先頭に共有辞書エントリを挿入",
            "Insert shared dictionary entry at top",
        ],
    ),
    (
        "插入 Graph 拓扑节点与边模板",
        [
            "插入 Graph 拓扑节点与边模板",
            "Graph テンプレートを挿入",
            "Insert Graph template",
        ],
    ),
    (
        "插入 5W1H 模板",
        [
            "插入 5W1H 模板",
            "5W1H テンプレートを挿入",
            "Insert 5W1H template",
        ],
    ),
    (
        "插入空格缩进续接行 (延续上一行当前列)",
        [
            "插入空格缩进续接行 (延续上一行当前列)",
            "空白インデントの継続行を挿入",
            "Insert space-indented continuation row",
        ],
    ),
    (
        "插入新记录行",
        [
            "插入新记录行",
            "新規レコード行を挿入",
            "Insert new record row",
        ],
    ),
    (
        "切换 / 插入 \\ 表头行",
        [
            "切换 / 插入 \\ 表头行",
            "見出し行を切り替え / 挿入",
            "Toggle / insert \\ header row",
        ],
    ),
    (
        "已插入词典引用 ^0",
        [
            "已插入词典引用 ^0",
            "辞書参照 ^0 を挿入しました",
            "Dictionary reference ^0 inserted",
        ],
    ),
    (
        "已在文首插入共享词典条目",
        [
            "已在文首插入共享词典条目",
            "ファイル先頭に共有辞書エントリを挿入しました",
            "Shared dictionary entry inserted at top",
        ],
    ),
    (
        "已追加 Graph 拓扑节点与边模板",
        [
            "已追加 Graph 拓扑节点与边模板",
            "Graph ノード・エッジテンプレートを追加しました",
            "Graph nodes and edges template appended",
        ],
    ),
    (
        "已追加 5W1H 语义模板",
        [
            "已追加 5W1H 语义模板",
            "5W1H テンプレートを追加しました",
            "5W1H template appended",
        ],
    ),
    (
        "已插入纵向意群续接行 (空格缩进)",
        [
            "已插入纵向意群续接行 (空格缩进)",
            "縦方向の意味継続行を挿入しました (空白インデント)",
            "Vertical continuation row inserted (space indented)",
        ],
    ),
    (
        "已插入列索引意群续接行 ( N value)",
        [
            "已插入列索引意群续接行 ( N value)",
            "列インデックス付きの継続行 ( N value) を挿入しました",
            "Column-indexed continuation row inserted ( N value)",
        ],
    ),
    (
        "已插入新记录行",
        [
            "已插入新记录行",
            "新規レコード行を挿入しました",
            "New record row inserted",
        ],
    ),
    (
        "已取消表头行 (转为自动 column1...)",
        [
            "已取消表头行 (转为自动 column1...)",
            "ヘッダー行を解除しました (自動 column1...)",
            "Header row removed (reverted to auto column1...)",
        ],
    ),
    (
        "已将首行设置为显式表头 (\\ )",
        [
            "已将首行设置为显式表头 (\\ )",
            "先頭行を明示ヘッダーに設定しました (\\ )",
            "First row set as explicit header (\\ )",
        ],
    ),
    (
        "已将首行设置为显式表头 (\\# )",
        [
            "已将首行设置为显式表头 (\\# )",
            "先頭行を明示ヘッダー (\\# ) に設定しました",
            "First row set as explicit header (\\# )",
        ],
    ),
    (
        "配置 / 热更新 .NEMA 语法解释器插件 (strict-parser.js / 外部脚本)",
        [
            "配置 / 热更新 .NEMA 语法解释器插件 (strict-parser.js / 外部脚本)",
            ".NEMA 文法インタープリタプラグインを管理 / ホットリロード",
            "Configure / hot-reload .NEMA grammar interpreter plugin (strict-parser.js / external)",
        ],
    ),
    (
        "当前文档体积较小或复用词少，原生 .nema 已是最优",
        [
            "当前文档体积较小或复用词少，原生 .nema 已是最优",
            "サイズが小さいか重複語が少ないため、原生 .nema が最適です",
            "Document is small or has few repeated words; native .nema is optimal",
        ],
    ),
    (
        "已转换为 .nemat 传输格式 (往返校验通过)",
        [
            "已转换为 .nemat 传输格式 (往返校验通过)",
            "NEMA .nemat 転送形式に変換しました (可逆検証済み)",
            "Converted to .nemat transport format (Round-trip verified)",
        ],
    ),
    (
        "已解码为 NEMA 原生格式 (Round-trip verified)",
        [
            "已解码为 NEMA 原生格式 (Round-trip verified)",
            "NEMA 原生形式に復号しました (可逆検証済み)",
            "Decoded to native NEMA (Round-trip verified)",
        ],
    ),
    (
        "已自动转换为 NEMA 表格格式 (.nema)",
        [
            "已自动转换为 NEMA 表格格式 (.nema)",
            "NEMA テーブル形式に自動変換しました (.nema)",
            "Auto-converted to NEMA table (.nema)",
        ],
    ),
    (
        "自动转化 .nema",
        ["自动转化 .nema", "NEMA 自動変換", "Auto Convert to NEMA"],
    ),
    ("水晶透明", ["水晶透明", "クリスタル透明", "Crystal"]),
    ("透明玻璃", ["透明玻璃", "透明ガラス", "Clear Glass"]),
    ("深色", ["深色", "ダーク", "Dark"]),
    ("浅色", ["浅色", "ライト", "Light"]),
    (
        "高对比度",
        ["高对比度", "ハイコントラスト", "High Contrast"],
    ),
    (
        "复古 16-Bit",
        ["复古 16-Bit", "レトロ 16-Bit", "Retro 16-Bit"],
    ),
    (
        "美漫 3渲2",
        ["美漫 3渲2", "アメコミ 3Dセル調", "Comic Cel-Shaded"],
    ),
    ("移动履历", ["移动履历", "移動履歴", "Jump History"]),
    (
        "只读模式",
        ["只读模式", "読み取り専用モード", "Read-Only Mode"],
    ),
    (
        "面板与折叠",
        ["面板与折叠", "パネルと折りたたみ", "Panels & Docks"],
    ),
    ("界面元素", ["界面元素", "UIバー", "UI Elements"]),
    ("文本显示", ["文本显示", "テキスト表示", "Text Display"]),
    ("折行规则", ["折行规则", "折り返しルール", "Wrap Rules"]),
    (
        "重置缩放 (100%)",
        [
            "重置缩放 (100%)",
            "拡大率をリセット (100%)",
            "Reset Zoom (100%)",
        ],
    ),
    (
        "跳转到定义",
        ["跳转到定义", "定義へジャンプ", "Go to Definition"],
    ),
    ("界面布局", ["界面布局", "レイアウト", "Layout"]),
    // 补全翻译 (0.3.8): 命令表 / 工具窗口 / 悬停提示
    (
        "多AI协同审查看板…",
        [
            "多AI协同审查看板…",
            "マルチAI共同レビューダッシュボード…",
            "Multi-AI Review Dashboard…",
        ],
    ),
    (
        "多AI协同实时编辑演示…",
        [
            "多AI协同实时编辑演示…",
            "マルチAI共同編集デモ…",
            "Multi-AI Co-Editing Demo…",
        ],
    ),
    ("协同审查", ["协同审查", "共同レビュー", "Co-Edit Review"]),
    (
        "选择语言 (类型一览)",
        [
            "选择语言 (类型一览)",
            "言語選択 (タイプ別一覧)",
            "Select Language (Type List)",
        ],
    ),
    ("语言", ["语言", "言語", "Language"]),
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
    ("▲ 折叠", ["▲ 折叠", "▲ 折りたたむ", "▲ Collapse"]),
    (
        "折叠 Markdown 工具条到顶端 (点击隐藏)",
        [
            "折叠 Markdown 工具条到顶端 (点击隐藏)",
            "Markdown ツールバーを上部に折りたたむ (クリックで非表示)",
            "Collapse Markdown toolbar to top (Click to hide)",
        ],
    ),
    (
        "展开 Markdown 工具条 (点击复原)",
        [
            "展开 Markdown 工具条 (点击复原)",
            "Markdown ツールバーを展開 (クリックで復元)",
            "Expand Markdown toolbar (Click to restore)",
        ],
    ),
    (
        "折叠 NEMA 工具条到顶端 (点击隐藏)",
        [
            "折叠 NEMA 工具条到顶端 (点击隐藏)",
            "NEMA ツールバーを上部に折りたたむ (クリックで非表示)",
            "Collapse NEMA toolbar to top (Click to hide)",
        ],
    ),
    (
        "展开 NEMA 工具条 (点击复原)",
        [
            "展开 NEMA 工具条 (点击复原)",
            "NEMA ツールバーを展開 (クリックで复元)",
            "Expand NEMA toolbar (Click to restore)",
        ],
    ),
    (
        "▼ 展开 Markdown 工具条",
        [
            "▼ 展开 Markdown 工具条",
            "▼ Markdown ツールバーを展開",
            "▼ Expand Markdown Toolbar",
        ],
    ),
    (
        "▼ 展开 NEMA 工具条",
        [
            "▼ 展开 NEMA 工具条",
            "▼ NEMA ツールバーを展開",
            "▼ Expand NEMA Toolbar",
        ],
    ),
    (
        "Markdown 工具条",
        ["Markdown 工具条", "Markdown ツールバー", "Markdown Toolbar"],
    ),
    (
        "NEMA 工具条",
        ["NEMA 工具条", "NEMA ツールバー", "NEMA Toolbar"],
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
        "智能体对话面板",
        ["智能体对话面板", "AIチャットパネル", "Agent Chat Panel"],
    ),
    (
        "多 Agent 并行协作台",
        [
            "多 Agent 并行协作台",
            "マルチAgent並列コラボ",
            "Multi-Agent Collab Bench",
        ],
    ),
    (
        "多 Agent 并行修改当前文件",
        [
            "多 Agent 并行修改当前文件",
            "マルチAgent並列編集",
            "Parallel Multi-Agent Edit",
        ],
    ),
    (
        "模拟多 AI 协同编辑",
        [
            "模拟多 AI 协同编辑",
            "複数AI協調デモ",
            "Simulate Multi-AI Co-edit",
        ],
    ),
    (
        "运行四阶段协同流水线",
        [
            "运行四阶段协同流水线",
            "4段階協調パイプライン実行",
            "Run 4-Stage Collab Pipeline",
        ],
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
    (
        "+ 新建终端会话 (PowerShell)",
        [
            "+ 新建终端会话 (PowerShell)",
            "+ 新規ターミナルセッション (PowerShell)",
            "+ New Terminal Session (PowerShell)",
        ],
    ),
    (
        "▶ 运行四阶段完整流水线",
        [
            "▶ 运行四阶段完整流水线",
            "▶ 4段階パイプライン実行",
            "▶ Run 4-Stage Pipeline",
        ],
    ),
    (
        "▶ 实时协同光标",
        [
            "▶ 实时协同光标",
            "▶ リアルタイム協調カーソル",
            "▶ Live Co-Edit Cursors",
        ],
    ),
    (
        "独立标签页报告",
        [
            "独立标签页报告",
            "別タブでレポートを開く",
            "Open Report in Tab",
        ],
    ),
    (
        "多 AI 协同编辑 & 独立审查平台 (Multi-AI Co-Edit & Review)",
        [
            "多 AI 协同编辑 & 独立审查平台 (Multi-AI Co-Edit & Review)",
            "マルチ AI 協調編集 & 独立レビュー基盤 (Multi-AI Co-Edit & Review)",
            "Multi-AI Co-Edit & Independent Review Platform",
        ],
    ),
    (
        "立即启动或重置常驻 PowerShell 会话",
        [
            "立即启动或重置常驻 PowerShell 会话",
            "常駐 PowerShell セッションを即座に起動またはリセット",
            "Start or reset persistent PowerShell session immediately",
        ],
    ),
    (
        "+ 启动 PowerShell 终端会话",
        [
            "+ 启动 PowerShell 终端会话",
            "+ PowerShell ターミナルを起動",
            "+ Start PowerShell Terminal Session",
        ],
    ),
    (
        "底部: 终端 (TERMINAL)",
        [
            "底部: 终端 (TERMINAL)",
            "下部: ターミナル (TERMINAL)",
            "Bottom: Terminal (TERMINAL)",
        ],
    ),
    (
        "底部: 输出 (OUTPUT)",
        [
            "底部: 输出 (OUTPUT)",
            "下部: 出力 (OUTPUT)",
            "Bottom: Output (OUTPUT)",
        ],
    ),
    (
        "底部: 问题 (PROBLEMS)",
        [
            "底部: 问题 (PROBLEMS)",
            "下部: 問題 (PROBLEMS)",
            "Bottom: Problems (PROBLEMS)",
        ],
    ),
    (
        "底部: 协同审查",
        [
            "底部: 协同审查",
            "下部: 協調レビュー",
            "Bottom: Collab Review",
        ],
    ),
    (
        "与本地文件比较…",
        [
            "与本地文件比较…",
            "ローカルファイルと比較…",
            "Compare with Local File…",
        ],
    ),
    (
        "与本地文件比较...",
        [
            "与本地文件比较...",
            "ローカルファイルと比較…",
            "Compare with Local File…",
        ],
    ),
    (
        "文件差异对比 (Diff)",
        [
            "文件差异对比 (Diff)",
            "ファイル差分比較 (Diff)",
            "File Diff Comparison",
        ],
    ),
    (
        "文件差异对比 (Diff)...",
        [
            "文件差异对比 (Diff)...",
            "ファイル差分比較 (Diff)...",
            "File Diff Comparison...",
        ],
    ),
    (
        "下一处差异 (F7)",
        [
            "下一处差异 (F7)",
            "次の差分 (F7)",
            "Next Diff (F7)",
        ],
    ),
    (
        "上一处差异 (Shift+F7)",
        [
            "上一处差异 (Shift+F7)",
            "前の差分 (Shift+F7)",
            "Previous Diff (Shift+F7)",
        ],
    ),
    (
        "查看差异列表…",
        [
            "查看差异列表…",
            "差分リストを表示…",
            "View Diff List…",
        ],
    ),
    (
        "查看差异列表...",
        [
            "查看差异列表...",
            "差分リストを表示…",
            "View Diff List…",
        ],
    ),
    (
        "退出对比 (清除差异)",
        [
            "退出对比 (清除差异)",
            "比較を終了 (差分クリア)",
            "Exit Diff (Clear)",
        ],
    ),
    (
        "与已打开标签页比较:",
        [
            "与已打开标签页比较:",
            "開いているタブと比較:",
            "Compare with Opened Tab:",
        ],
    ),
    (
        "打开本地文件",
        [
            "打开本地文件",
            "ローカルファイルを開く",
            "Open Local File",
        ],
    ),
    (
        "首次启动较慢，组件初始化中…",
        [
            "首次启动较慢，组件初始化中…",
            "初回起動時は少し時間がかかります。コンポーネント初期化中…",
            "First launch is slower, initializing components…",
        ],
    ),
    (
        "正在准备文件选择接口与微内核比对引擎，请稍候…",
        [
            "正在准备文件选择接口与微内核比对引擎，请稍候…",
            "ファイル選択ダイアログと差分エンジンを準備しています。しばらくお待ちください…",
            "Preparing file dialog and diff engine, please wait…",
        ],
    ),
    (
        "正在准备文件选择接口，请稍候…",
        [
            "正在准备文件选择接口，请稍候…",
            "ファイル選択ダイアログを準備しています。しばらくお待ちください…",
            "Preparing file dialog, please wait…",
        ],
    ),
    (
        "界面语言 (Language):",
        [
            "界面语言 (Language):",
            "表示言語 (Language):",
            "Interface Language:",
        ],
    ),
    (
        "环境配置与安装参数 (CONFIGURATION)",
        [
            "环境配置与安装参数 (CONFIGURATION)",
            "環境設定とインストールパラメータ (CONFIGURATION)",
            "Environment & Installation Configuration",
        ],
    ),
    (
        "自定义目标安装路径与环境联动，保障纯净绿色的极速微内核体验",
        [
            "自定义目标安装路径与环境联动，保障纯净绿色的极速微内核体验",
            "インストール先と環境連携をカスタマイズし、純粋で高速なマイクロカーネル体験を保証します",
            "Customize installation path and system integration for a pure, fast microkernel experience",
        ],
    ),
    (
        "目标安装与工作目录 (Installation Directory):",
        [
            "目标安装与工作目录 (Installation Directory):",
            "インストール先・作業ディレクトリ (Installation Directory):",
            "Installation & Working Directory:",
        ],
    ),
    (
        "浏览 (BROWSE)...",
        [
            "浏览 (BROWSE)...",
            "参照 (BROWSE)...",
            "Browse...",
        ],
    ),
    (
        "选择 IDE 1.0 安装或工作目录",
        [
            "选择 IDE 1.0 安装或工作目录",
            "IDE 1.0 のインストールまたは作業フォルダーを選択",
            "Select IDE 1.0 Installation or Working Directory",
        ],
    ),
    (
        "所需空间: ~85 MB  |  架构: x86_64 Windows (Native)  |  零第三方依赖 · 秒速起航",
        [
            "所需空间: ~85 MB  |  架构: x86_64 Windows (Native)  |  零第三方依赖 · 秒速起航",
            "必要容量: 約 85 MB  |  アーキテクチャ: x86_64 Windows (Native)  |  依存関係ゼロ・超高速起動",
            "Required: ~85 MB  |  Arch: x86_64 Windows (Native)  |  Zero dependencies, instant launch",
        ],
    ),
    (
        "将 IDE1.0 添加至全局环境变量 PATH",
        [
            "将 IDE1.0 添加至全局环境变量 PATH",
            "IDE 1.0 を環境変数 PATH に追加",
            "Add IDE1.0 to system PATH environment variable",
        ],
    ),
    (
        "可在任意终端 (PowerShell / CMD) 键入 IDE1.0 秒级呼出",
        [
            "可在任意终端 (PowerShell / CMD) 键入 IDE1.0 秒级呼出",
            "任意のターミナル (PowerShell / CMD) から IDE1.0 で即座に起動できます",
            "Launch IDE1.0 instantly from any terminal (PowerShell / CMD)",
        ],
    ),
    (
        "创建桌面与开始菜单极速启动快捷方式",
        [
            "创建桌面与开始菜单极速启动快捷方式",
            "デスクトップとスタートメニューにショートカットを作成",
            "Create Desktop and Start Menu shortcuts",
        ],
    ),
    (
        "生成正版水晶图标，双击秒开，随时直达工作空间",
        [
            "生成正版水晶图标，双击秒开，随时直达工作空间",
            "純正クリスタルアイコンを生成し、ダブルクリックでいつでもワークスペースへ直行",
            "Crystal icon for instant access to your workspace",
        ],
    ),
    (
        "启用 Windows 系统托盘常驻守护 (Resident Daemon)",
        [
            "启用 Windows 系统托盘常驻守护 (Resident Daemon)",
            "Windows システムトレイ常駐デーモンを有効化",
            "Enable Windows System Tray resident daemon",
        ],
    ),
    (
        "最小化时驻留托盘，毫秒级热唤醒，防误关守护活跃会话",
        [
            "最小化时驻留托盘，毫秒级热唤醒，防误关守护活跃会话",
            "最小化時はトレイに常駐し、ミリ秒で復元。誤終了を防ぎセッションを保護",
            "Minimizes to tray with instant wake-up, protects active sessions",
        ],
    ),
    (
        "立即安装 (INSTALL NOW)",
        [
            "立即安装 (INSTALL NOW)",
            "今すぐインストール (INSTALL NOW)",
            "Install Now",
        ],
    ),
    (
        "退出向导",
        [
            "退出向导",
            "ウィザードを終了",
            "Exit Wizard",
        ],
    ),
    (
        "1. 环境配置",
        [
            "1. 环境配置",
            "1. 環境設定",
            "1. Configuration",
        ],
    ),
    (
        "2. 极速部署",
        [
            "2. 极速部署",
            "2. 高速デプロイ",
            "2. Deployment",
        ],
    ),
    (
        "3. 部署完成",
        [
            "3. 部署完成",
            "3. 完了",
            "3. Finished",
        ],
    ),
    (
        "正在部署 IDE 1.0 (DEPLOYING PACKAGES)",
        [
            "正在部署 IDE 1.0 (DEPLOYING PACKAGES)",
            "IDE 1.0 をデプロイ中 (DEPLOYING PACKAGES)",
            "Deploying IDE 1.0 Packages",
        ],
    ),
    (
        "纯 Rust 原生释放，解压核心微内核与环境配置文件...",
        [
            "纯 Rust 原生释放，解压核心微内核与环境配置文件...",
            "純粋な Rust ネイティブバイナリを展開し、コアマイクロカーネルと環境設定を構成中...",
            "Extracting pure Rust native binaries and configuring environment...",
        ],
    ),
    (
        "部署中...",
        [
            "部署中...",
            "デプロイ中...",
            "Deploying...",
        ],
    ),
    (
        "[OK] 校验系统环境与依赖完整性...",
        [
            "[OK] 校验系统环境与依赖完整性...",
            "[OK] システム環境と依存関係の整合性を確認...",
            "[OK] Verified system environment and dependencies...",
        ],
    ),
    (
        "释放 IDE1.0.exe 原生极速微内核至目标目录...",
        [
            "释放 IDE1.0.exe 原生极速微内核至目标目录...",
            "IDE1.0.exe ネイティブマイクロカーネルをターゲットフォルダーに展開...",
            "Extracting IDE1.0.exe native microkernel to target directory...",
        ],
    ),
    (
        "部署 3D 赛璐珞与晶透毛玻璃视觉管线预设...",
        [
            "部署 3D 赛璐珞与晶透毛玻璃视觉管线预设...",
            "3D セル画風およびガラスエフェクトのビジュアルパイプラインを設定...",
            "Configuring 3D Cel-shaded and glass pipeline presets...",
        ],
    ),
    (
        "写入系统环境变量 PATH 与桌面快捷启动入口...",
        [
            "写入系统环境变量 PATH 与桌面快捷启动入口...",
            "環境変数 PATH とデスクトップショートカットを登録...",
            "Registering system PATH and desktop shortcuts...",
        ],
    ),
    (
        "首选参数配置与常驻托盘守护者完成同步初始化！",
        [
            "首选参数配置与常驻托盘守护者完成同步初始化！",
            "環境設定とトレイ常駐デーモンの同期初期化が完了！",
            "Preferences and system tray resident daemon initialized!",
        ],
    ),
    (
        "[..] 正在处理中...",
        [
            "[..] 正在处理中...",
            "[..] 処理中...",
            "[..] In progress...",
        ],
    ),
    (
        "部署成功！IDE 1.0 已准备就绪！(SUCCESS)",
        [
            "部署成功！IDE 1.0 已准备就绪！(SUCCESS)",
            "デプロイ完了！IDE 1.0 の準備が整いました！(SUCCESS)",
            "Deployment Successful! IDE 1.0 is ready!",
        ],
    ),
    (
        "所有核心依赖与配置已装载完毕，即刻享受极致的纯 Rust 原生开发体验！",
        [
            "所有核心依赖与配置已装载完毕，即刻享受极致的纯 Rust 原生开发体验！",
            "すべてのコア依存関係と構成の準備が完了しました。極上のネイティブ開発体験をお楽しみください！",
            "All core components and configurations are loaded. Enjoy the ultimate pure Rust native experience!",
        ],
    ),
    (
        "启动 IDE 1.0 极速体验 (Launch immediately)",
        [
            "启动 IDE 1.0 极速体验 (Launch immediately)",
            "IDE 1.0 を今すぐ起動 (Launch immediately)",
            "Launch IDE 1.0 immediately",
        ],
    ),
    (
        "保持微内核极速冷启动，秒级唤醒原生编辑器",
        [
            "保持微内核极速冷启动，秒级唤醒原生编辑器",
            "マイクロカーネルの超高速コールドスタートを維持し、瞬時にネイティブエディタを起動",
            "Maintain microkernel fast cold-start with instant native editor launch",
        ],
    ),
    (
        "完成并启动 (FINISH)",
        [
            "完成并启动 (FINISH)",
            "完了して起動 (FINISH)",
            "Finish & Launch",
        ],
    ),
    (
        "退出",
        [
            "退出",
            "終了",
            "Exit",
        ],
    ),
    (
        "校验本地 CPU 硬件加速扩展指令集 (AVX2/SSE4.2)...",
        [
            "校验本地 CPU 硬件加速扩展指令集 (AVX2/SSE4.2)...",
            "ローカル CPU ハードウェアアクセラレーション命令セット (AVX2/SSE4.2) を検証...",
            "Verifying local CPU hardware acceleration extensions (AVX2/SSE4.2)...",
        ],
    ),
    (
        "安装与工作空间目录",
        [
            "安装与工作空间目录",
            "インストール先とワークスペース",
            "Installation & Workspace Directory",
        ],
    ),
    (
        "终端命令行集成",
        [
            "终端命令行集成",
            "ターミナルコマンドライン連携",
            "Terminal CLI Integration",
        ],
    ),
    (
        "系统驻留守护",
        [
            "系统驻留守护",
            "システム常駐デーモン",
            "System Resident Daemon",
        ],
    ),
    (
        "完成后立即启动 IDE 1.0 (Launch IDE 1.0 Now)",
        [
            "完成后立即启动 IDE 1.0 (Launch IDE 1.0 Now)",
            "完了後すぐに IDE 1.0 を起動 (Launch IDE 1.0 Now)",
            "Launch IDE 1.0 immediately upon completion",
        ],
    ),
    (
        "完成并启动 IDE 1.0 (LAUNCH NOW)",
        [
            "完成并启动 IDE 1.0 (LAUNCH NOW)",
            "完了して IDE 1.0 を起動 (LAUNCH NOW)",
            "Finish & Launch IDE 1.0 (LAUNCH NOW)",
        ],
    ),
    (
        "已绑定目标目录",
        [
            "已绑定目标目录",
            "バインド済みターゲットフォルダー",
            "Bound Target Directory",
        ],
    ),
    (
        "已添加至全局 PATH (终端键入 IDE1.0 秒级唤醒)",
        [
            "已添加至全局 PATH (终端键入 IDE1.0 秒级唤醒)",
            "環境変数 PATH に追加済み (ターミナルから IDE1.0 で即起動)",
            "Added to system PATH (launch IDE1.0 instantly from terminal)",
        ],
    ),
    (
        "未配置全局环境变量 PATH",
        [
            "未配置全局环境变量 PATH",
            "環境変数 PATH は未設定",
            "System PATH not configured",
        ],
    ),
    (
        "系统托盘常驻守护已开启 (毫秒级热启动与状态守护)",
        [
            "系统托盘常驻守护已开启 (毫秒级热启动与状态守护)",
            "システムトレイ常駐デーモン有効 (ミリ秒高速起動と状態保護)",
            "System tray resident daemon enabled (instant hot-wakeup and state guard)",
        ],
    ),
    (
        "系统托盘守护未启用",
        [
            "系统托盘守护未启用",
            "システムトレイ常駐は無効",
            "System tray daemon disabled",
        ],
    ),
    (
        "多Agent协同工作台",
        [
            "多Agent协同工作台",
            "マルチAgent協調ダッシュボード",
            "Multi-Agent Collab Dashboard",
        ],
    ),
    (
        "智能助手对话",
        [
            "智能助手对话",
            "AIアシスタント対話",
            "AI Assistant Chat",
        ],
    ),
    (
        "操作",
        [
            "操作",
            "アクション",
            "Actions",
        ],
    ),
    (
        "清空对话历史",
        [
            "清空对话历史",
            "対話履歴をクリア",
            "Clear Chat History",
        ],
    ),
    (
        "复制全部对话",
        [
            "复制全部对话",
            "対話全体をコピー",
            "Copy Full Conversation",
        ],
    ),
    (
        "模型与接口设置…",
        [
            "模型与接口设置…",
            "モデル・エンドポイント設定…",
            "Model & Endpoint Settings…",
        ],
    ),
    (
        "切换至协同工作台",
        [
            "切换至协同工作台",
            "協調ダッシュボードに切替",
            "Switch to Collab Dashboard",
        ],
    ),
    (
        "自动应用补丁",
        [
            "自动应用补丁",
            "パッチを自動適用",
            "Auto-apply Patches",
        ],
    ),
    (
        "手动确认补丁",
        [
            "手动确认补丁",
            "パッチを手動確認",
            "Manually Review Patches",
        ],
    ),
    (
        "带上选区与当前章节上下文",
        [
            "带上选区与当前章节上下文",
            "選択範囲と現在の章を含める",
            "Include Selection & Section Context",
        ],
    ),
    (
        "免点击直接应用补丁到文件",
        [
            "免点击直接应用补丁到文件",
            "クリック不要でファイルを直接変更",
            "Directly Apply Changes Without Click",
        ],
    ),
    (
        "供应商预设:",
        [
            "供应商预设:",
            "プロバイダープリセット:",
            "Provider Preset:",
        ],
    ),
    (
        "自定义供应商",
        [
            "自定义供应商",
            "カスタムプロバイダー",
            "Custom Provider",
        ],
    ),
    (
        "快捷技能",
        [
            "快捷技能",
            "クイックコマンド",
            "Quick Skills",
        ],
    ),
    (
        "打开模型配置…",
        [
            "打开模型配置…",
            "モデル設定を開く…",
            "Open Model Settings…",
        ],
    ),
    (
        "采纳操作",
        [
            "采纳操作",
            "採用アクション",
            "Apply Actions",
        ],
    ),
    (
        "插入到当前光标处",
        [
            "插入到当前光标处",
            "現在のカーソル位置に挿入",
            "Insert at Current Cursor",
        ],
    ),
    (
        "复制全部回复",
        [
            "复制全部回复",
            "応答全体をコピー",
            "Copy Full Response",
        ],
    ),
    (
        "安装向导",
        [
            "安装向导",
            "セットアップ",
            "Setup Wizard",
        ],
    ),
    (
        "环境配置",
        [
            "环境配置",
            "環境設定",
            "Configuration",
        ],
    ),
    (
        "极速部署",
        [
            "极速部署",
            "高速デプロイ",
            "Deployment",
        ],
    ),
    (
        "部署完成",
        [
            "部署完成",
            "完了",
            "Finished",
        ],
    ),
    (
        "即将完成安装...",
        [
            "即将完成安装...",
            "間もなく完了します...",
            "Finalizing installation...",
        ],
    ),
    (
        "系统终端",
        [
            "系统终端",
            "システム端末",
            "System Shell",
        ],
    ),
    (
        "管理员",
        [
            "管理员",
            "管理者",
            "Admin",
        ],
    ),
    (
        "标准用户",
        [
            "标准用户",
            "標準ユーザー",
            "Standard User",
        ],
    ),
    (
        "常驻托盘",
        [
            "常驻托盘",
            "トレイ常駐",
            "Tray Resident",
        ],
    ),
    (
        "独立窗口",
        [
            "独立窗口",
            "独立ウィンドウ",
            "Standalone",
        ],
    ),
    (
        "维护向导",
        [
            "维护向导",
            "メンテナンス",
            "Maintenance",
        ],
    ),
    (
        "修复向导",
        [
            "修复向导",
            "修復ウィザード",
            "Repair Wizard",
        ],
    ),
    (
        "卸载向导",
        [
            "卸载向导",
            "アンインストール",
            "Uninstall",
        ],
    ),
    (
        "许可协议",
        [
            "许可协议",
            "使用許諾契約",
            "License Agreement",
        ],
    ),
    (
        "【已阅读至末尾】您已通读协议，请在下方勾选同意条款以解锁继续按钮",
        [
            "【已阅读至末尾】您已通读协议，请在下方勾选同意条款以解锁继续按钮",
            "【末尾まで読了】規約を確認しました。下の同意をチェックして続行してください",
            "[Reached End] You have read the agreement. Check agree below to continue",
        ],
    ),
    (
        "【需阅读至底部】请拖动右侧滚动条或使用鼠标滚轮向下阅读至协议全文底部",
        [
            "【需阅读至底部】请拖动右侧滚动条或使用鼠标滚轮向下阅读至协议全文底部",
            "【末尾まで要確認】右側のスクロールバーまたはホイールで一番下までお読みください",
            "[Scroll Required] Please scroll down to the bottom of the agreement to continue",
        ],
    ),
    (
        "同意并继续 (AGREE & NEXT)",
        [
            "同意并继续 (AGREE & NEXT)",
            "同意して次へ (AGREE & NEXT)",
            "Agree & Next (AGREE & NEXT)",
        ],
    ),
    (
        "我已完整阅读并同意《IDE 1.0 最终用户许可与使用协议》",
        [
            "我已完整阅读并同意《IDE 1.0 最终用户许可与使用协议》",
            "「IDE 1.0 エンドユーザー使用許諾契約」をすべて読み同意します",
            "I have read and agree to the IDE 1.0 End User License Agreement",
        ],
    ),
    (
        "拒绝并退出",
        [
            "拒绝并退出",
            "同意せず終了",
            "Decline & Exit",
        ],
    ),
    (
        "最终用户许可与使用协议 (EULA)",
        [
            "最终用户许可与使用协议 (EULA)",
            "エンドユーザー使用許諾契約 (EULA)",
            "End User License Agreement (EULA)",
        ],
    ),
    (
        "确认已理解代码执行风险、本地文件修改权限与责任限制条款",
        [
            "确认已理解代码执行风险、本地文件修改权限与责任限制条款",
            "コード実行リスク、ローカルファイル変更権限および責任制限事項を理解しました",
            "Acknowledged code execution risks, local file permissions, and liability limits",
        ],
    ),
    (
        "许可协议阅读进度状态",
        [
            "许可协议阅读进度状态",
            "規約読了ステータス",
            "Agreement Reading Progress",
        ],
    ),
    (
        "请通读下方开源许可与使用条款；必须拉到底部并勾选同意后方可继续安装",
        [
            "请通读下方开源许可与使用条款；必须拉到底部并勾选同意后方可继续安装",
            "下記の利用規約をお読みください。一番下までスクロールして同意すると続行できます",
            "Please read the license terms below; scroll to the bottom and check agree to continue",
        ],
    ),
    (
        "前往发布主页手动下载",
        [
            "前往发布主页手动下载",
            "リリースページから手動ダウンロード",
            "Go to Release Page for Manual Download",
        ],
    ),
    (
        "发布主页",
        [
            "发布主页",
            "リリースページ",
            "Release Page",
        ],
    ),
    (
        "发现新版本",
        [
            "发现新版本",
            "新バージョン検出",
            "New Version",
        ],
    ),
    (
        "发现新版本就绪",
        [
            "发现新版本就绪",
            "新バージョンが利用可能です",
            "New Version Ready",
        ],
    ),
    (
        "已签名",
        [
            "已签名",
            "署名済み",
            "Signed",
        ],
    ),
    (
        "当前已是最新版本 (UP TO DATE)",
        [
            "当前已是最新版本 (UP TO DATE)",
            "最新バージョンです (UP TO DATE)",
            "You are Up to Date (UP TO DATE)",
        ],
    ),
    (
        "您当前运行的是最新版，无需更新；可随时重新检查",
        [
            "您当前运行的是最新版，无需更新；可随时重新检查",
            "現在最新版を実行しています。更新は不要です。いつでも再確認できます",
            "You are running the latest version. No updates needed; check again anytime",
        ],
    ),
    (
        "无直接可执行资产 (请访问发布主页下载)",
        [
            "无直接可执行资产 (请访问发布主页下载)",
            "直接実行可能なアセットがありません (リリースページより取得)",
            "No direct binary asset available (please visit the release page)",
        ],
    ),
    (
        "更新包就绪，校验全量通过",
        [
            "更新包就绪，校验全量通过",
            "更新パッケージ検証完了、準備完了",
            "Update Package Ready, Verification Passed",
        ],
    ),
    (
        "更新安全策略",
        [
            "更新安全策略",
            "アップデートセキュリティポリシー",
            "Update Security Policy",
        ],
    ),
    (
        "更新就绪",
        [
            "更新就绪",
            "更新準備完了",
            "Update Ready",
        ],
    ),
    (
        "更新异常",
        [
            "更新异常",
            "更新エラー",
            "Update Error",
        ],
    ),
    (
        "更新通道",
        [
            "更新通道",
            "アップデートチャネル",
            "Update Channel",
        ],
    ),
    (
        "未配置公钥",
        [
            "未配置公钥",
            "公開鍵未設定",
            "No Public Key Configured",
        ],
    ),
    (
        "本机当前版本",
        [
            "本机当前版本",
            "現在のバージョン",
            "Current Installed Version",
        ],
    ),
    (
        "极速桌面开发微内核",
        [
            "极速桌面开发微内核",
            "超高速デスクトップマイクロカーネル",
            "Ultra-Fast Desktop Microkernel",
        ],
    ),
    (
        "检查官方渠道发布的最新稳定版与增量补丁",
        [
            "检查官方渠道发布的最新稳定版与增量补丁",
            "公式チャンネルから最新の安定版とパッチを確認します",
            "Check for official stable releases and incremental patches",
        ],
    ),
    (
        "检查更新",
        [
            "检查更新",
            "更新を確認",
            "Check Updates",
        ],
    ),
    (
        "检查更新异常 (UPDATE ERROR)",
        [
            "检查更新异常 (UPDATE ERROR)",
            "アップデート確認エラー (UPDATE ERROR)",
            "Update Check Error (UPDATE ERROR)",
        ],
    ),
    (
        "正在下载",
        [
            "正在下载",
            "ダウンロード中",
            "Downloading",
        ],
    ),
    (
        "正在后台高速下载中...",
        [
            "正在后台高速下载中...",
            "バックグラウンドでダウンロード中...",
            "Downloading in background...",
        ],
    ),
    (
        "正在拉取发行包资产并同步校验哈希完整性...",
        [
            "正在拉取发行包资产并同步校验哈希完整性...",
            "アセットを取得し、ハッシュ整合性を検証しています...",
            "Fetching release package and verifying hash integrity...",
        ],
    ),
    (
        "正在连接官方发行源校验最新版本号与数字签名...",
        [
            "正在连接官方发行源校验最新版本号与数字签名...",
            "公式ソースに接続し最新バージョンと署名を検証中...",
            "Connecting to official source to verify version and signature...",
        ],
    ),
    (
        "正在连接校验新版本 (CHECKING)...",
        [
            "正在连接校验新版本 (CHECKING)...",
            "新バージョンを確認中 (CHECKING)...",
            "Checking for New Version (CHECKING)...",
        ],
    ),
    (
        "正在高速下载更新包",
        [
            "正在高速下载更新包",
            "高速ダウンロード中",
            "Downloading Update Package",
        ],
    ),
    (
        "版本更新与升级 (UPDATES)",
        [
            "版本更新与升级 (UPDATES)",
            "アップデートとアップグレード (UPDATES)",
            "Version Updates & Upgrades (UPDATES)",
        ],
    ),
    (
        "目标发布版本与资产",
        [
            "目标发布版本与资产",
            "対象バージョンとアセット",
            "Target Release & Assets",
        ],
    ),
    (
        "立即下载更新 (DOWNLOAD NOW)",
        [
            "立即下载更新 (DOWNLOAD NOW)",
            "今すぐダウンロード (DOWNLOAD NOW)",
            "Download Now (DOWNLOAD NOW)",
        ],
    ),
    (
        "立即安装并重启 (INSTALL NOW)",
        [
            "立即安装并重启 (INSTALL NOW)",
            "今すぐインストールして再起動 (INSTALL NOW)",
            "Install Now & Restart (INSTALL NOW)",
        ],
    ),
    (
        "重新检查更新 (CHECK AGAIN)",
        [
            "重新检查更新 (CHECK AGAIN)",
            "再確認 (CHECK AGAIN)",
            "Check Again (CHECK AGAIN)",
        ],
    ),
    (
        "官方已有新版本发布，建议升级以获得最新特性与缺陷修复",
        [
            "官方已有新版本发布，建议升级以获得最新特性与缺陷修复",
            "新しい公式リリースが利用可能です。最新機能と修正を取得するため更新をおすすめします",
            "A new official release is available. Upgrading is recommended for the latest features and fixes",
        ],
    ),
    (
        "签名校验与 SHA-256 验证无误，随时可应用更新并极速重启",
        [
            "签名校验与 SHA-256 验证无误，随时可应用更新并极速重启",
            "署名と SHA-256 の検証に成功しました。今すぐ更新を適用して再起動できます",
            "Signature and SHA-256 verified. Ready to apply update and restart",
        ],
    ),
    (
        "维护模式",
        [
            "维护模式",
            "メンテナンス",
            "Maintenance",
        ],
    ),
    (
        "执行维护",
        [
            "执行维护",
            "実行中",
            "Processing",
        ],
    ),
    (
        "维护完成",
        [
            "维护完成",
            "完了",
            "Complete",
        ],
    ),
    (
        "修复确认",
        [
            "修复确认",
            "修復確認",
            "Repair Options",
        ],
    ),
    (
        "正在修复",
        [
            "正在修复",
            "修復中",
            "Repairing",
        ],
    ),
    (
        "修复完成",
        [
            "修复完成",
            "修復完了",
            "Repaired",
        ],
    ),
    (
        "卸载选项",
        [
            "卸载选项",
            "削除設定",
            "Uninstall Options",
        ],
    ),
    (
        "正在卸载",
        [
            "正在卸载",
            "削除中",
            "Uninstalling",
        ],
    ),
    (
        "卸载完成",
        [
            "卸载完成",
            "削除完了",
            "Uninstalled",
        ],
    ),
    (
        "返回维护",
        [
            "返回维护",
            "メンテナンスへ戻る",
            "Back",
        ],
    ),
    (
        "检测到本机已安装 IDE 1.0 (MAINTENANCE)",
        [
            "检测到本机已安装 IDE 1.0 (MAINTENANCE)",
            "既存の IDE 1.0 を検出しました (MAINTENANCE)",
            "Existing IDE 1.0 Detected (MAINTENANCE)",
        ],
    ),
    (
        "选择修复现有安装、卸载移除组件，或更改目标目录与集成选项",
        [
            "选择修复现有安装、卸载移除组件，或更改目标目录与集成选项",
            "既存のインストールを修復・削除するか、インストール先と設定を変更します",
            "Repair your installation, uninstall components, or change install options",
        ],
    ),
    (
        "当前已安装实例",
        [
            "当前已安装实例",
            "検出されたインストール",
            "Installed Instance",
        ],
    ),
    (
        "修复",
        [
            "修复",
            "修復",
            "Repair",
        ],
    ),
    (
        "卸载",
        [
            "卸载",
            "削除",
            "Remove",
        ],
    ),
    (
        "配置",
        [
            "配置",
            "変更",
            "Modify",
        ],
    ),
    (
        "修复安装 (Repair Installation)",
        [
            "修复安装 (Repair Installation)",
            "インストールを修復 (Repair Installation)",
            "Repair Installation",
        ],
    ),
    (
        "重新释放主程序并修复快捷方式、PATH 与注册表，完整保留个人配置",
        [
            "重新释放主程序并修复快捷方式、PATH 与注册表，完整保留个人配置",
            "実行ファイル・ショートカット・PATH・レジストリを修復し個人設定を保持します",
            "Restore binaries, shortcuts, PATH and registry while keeping your settings",
        ],
    ),
    (
        "卸载移除 (Uninstall IDE 1.0)",
        [
            "卸载移除 (Uninstall IDE 1.0)",
            "IDE 1.0 をアンインストール (Uninstall)",
            "Uninstall IDE 1.0",
        ],
    ),
    (
        "移除主程序、快捷方式、环境变量与卸载记录，可选清理个人配置",
        [
            "移除主程序、快捷方式、环境变量与卸载记录，可选清理个人配置",
            "本体・ショートカット・PATH・登録情報を削除します（個人設定は任意）",
            "Remove binaries, shortcuts, PATH and uninstall entry (user data optional)",
        ],
    ),
    (
        "更改配置或安装目录 (Modify / Reinstall)",
        [
            "更改配置或安装目录 (Modify / Reinstall)",
            "設定やインストール先を変更 (Modify / Reinstall)",
            "Modify Settings or Reinstall",
        ],
    ),
    (
        "修改安装目标路径或重新配置桌面图标、PATH 与常驻托盘选项",
        [
            "修改安装目标路径或重新配置桌面图标、PATH 与常驻托盘选项",
            "インストール先フォルダーやショートカット・PATH・トレイ常駐を再設定します",
            "Choose a different install directory or reconfigure shortcuts, PATH and tray",
        ],
    ),
    (
        "直接启动 IDE 1.0",
        [
            "直接启动 IDE 1.0",
            "IDE 1.0 を起動",
            "Launch IDE 1.0",
        ],
    ),
    (
        "修复现有安装 (REPAIR INSTALLATION)",
        [
            "修复现有安装 (REPAIR INSTALLATION)",
            "既存のインストールを修復 (REPAIR INSTALLATION)",
            "Repair Existing Installation (REPAIR)",
        ],
    ),
    (
        "保留您的全部偏好与会话，重新部署核心程序并修复系统集成项",
        [
            "保留您的全部偏好与会话，重新部署核心程序并修复系统集成项",
            "すべての個人設定とセッションを保持したまま、本体とシステム統合を修復します",
            "Keep all preferences and sessions while restoring binaries and system links",
        ],
    ),
    (
        "目标修复目录",
        [
            "目标修复目录",
            "修復対象フォルダー",
            "Target Repair Directory",
        ],
    ),
    (
        "校验并修复全局环境变量 PATH",
        [
            "校验并修复全局环境变量 PATH",
            "ユーザー環境変数 PATH を検証・修復",
            "Verify and Repair User PATH",
        ],
    ),
    (
        "确保终端中键入 IDE1.0 可直接唤起当前安装目录",
        [
            "确保终端中键入 IDE1.0 可直接唤起当前安装目录",
            "任意の端末から IDE1.0 をすぐ起動できるよう PATH を修復します",
            "Ensure typing IDE1.0 in any terminal launches this installation",
        ],
    ),
    (
        "重建桌面与开始菜单快捷方式",
        [
            "重建桌面与开始菜单快捷方式",
            "デスクトップとスタートメニューのショートカットを再作成",
            "Rebuild Desktop and Start Menu Shortcuts",
        ],
    ),
    (
        "修复损坏或丢失的桌面与开始菜单水晶图标入口",
        [
            "修复损坏或丢失的桌面与开始菜单水晶图标入口",
            "破損または紛失したショートカットとクリスタルアイコンを復元します",
            "Restore missing or broken Desktop and Start Menu shortcuts",
        ],
    ),
    (
        "同步系统托盘常驻守护配置",
        [
            "同步系统托盘常驻守护配置",
            "システムトレイ常駐デーモン設定を同期",
            "Sync System Tray Resident Daemon",
        ],
    ),
    (
        "恢复最小化驻留托盘与毫秒级热唤醒能力",
        [
            "恢复最小化驻留托盘与毫秒级热唤醒能力",
            "トレイ常駐と瞬時復帰の設定を復元します",
            "Restore minimize-to-tray residency and instant wake-up",
        ],
    ),
    (
        "开始修复 (REPAIR NOW)",
        [
            "开始修复 (REPAIR NOW)",
            "今すぐ修復 (REPAIR NOW)",
            "Repair Now (REPAIR NOW)",
        ],
    ),
    (
        "卸载移除 IDE 1.0 (UNINSTALL)",
        [
            "卸载移除 IDE 1.0 (UNINSTALL)",
            "IDE 1.0 のアンインストール (UNINSTALL)",
            "Uninstall IDE 1.0 (UNINSTALL)",
        ],
    ),
    (
        "安全停止关联托盘进程并移除程序与集成项，全程无黑框闪烁",
        [
            "安全停止关联托盘进程并移除程序与集成项，全程无黑框闪烁",
            "関連プロセスを安全に終了し、端末画面を出さずに本体と統合項目を削除します",
            "Safely stop resident processes and remove components headlessly",
        ],
    ),
    (
        "待卸载安装目录",
        [
            "待卸载安装目录",
            "アンインストール対象フォルダー",
            "Directory to Uninstall",
        ],
    ),
    (
        "移除桌面与开始菜单快捷方式",
        [
            "移除桌面与开始菜单快捷方式",
            "デスクトップとスタートメニューのショートカットを削除",
            "Remove Desktop and Start Menu Shortcuts",
        ],
    ),
    (
        "清理 Desktop 与 Start Menu 中的 IDE1.0 启动快捷方式",
        [
            "清理 Desktop 与 Start Menu 中的 IDE1.0 启动快捷方式",
            "デスクトップとスタートメニューから IDE1.0 のショートカットを消去します",
            "Delete IDE1.0 shortcuts from Desktop and Start Menu",
        ],
    ),
    (
        "清理环境变量 PATH 与卸载注册表项",
        [
            "清理环境变量 PATH 与卸载注册表项",
            "環境変数 PATH とアンインストール登録を削除",
            "Clean PATH Entry and Uninstall Registry",
        ],
    ),
    (
        "从用户 PATH 移除安装目录并注销 Windows 应用列表记录",
        [
            "从用户 PATH 移除安装目录并注销 Windows 应用列表记录",
            "ユーザー PATH からパスを除外し、アプリ一覧の登録を解除します",
            "Remove install dir from User PATH and unregister from Windows Apps",
        ],
    ),
    (
        "同时删除个人偏好配置与会话缓存",
        [
            "同时删除个人偏好配置与会话缓存",
            "個人設定とセッションキャッシュも削除",
            "Also Delete User Preferences and Session Cache",
        ],
    ),
    (
        "清空 IDE1.0 Fast 配置目录（默认关闭以保留您的个人配置）",
        [
            "清空 IDE1.0 Fast 配置目录（默认关闭以保留您的个人配置）",
            "IDE1.0 Fast 設定フォルダーを削除します（個人設定保護のため既定オフ）",
            "Erase IDE1.0 Fast settings folder (off by default to keep your config)",
        ],
    ),
    (
        "确认卸载 (UNINSTALL NOW)",
        [
            "确认卸载 (UNINSTALL NOW)",
            "アンインストールを実行 (UNINSTALL NOW)",
            "Uninstall Now (UNINSTALL NOW)",
        ],
    ),
    (
        "正在修复 IDE 1.0 (REPAIRING INSTALLATION)",
        [
            "正在修复 IDE 1.0 (REPAIRING INSTALLATION)",
            "IDE 1.0 を修復中 (REPAIRING INSTALLATION)",
            "Repairing IDE 1.0 (REPAIRING)",
        ],
    ),
    (
        "正在校验核心二进制完整性并重建快捷方式与系统集成...",
        [
            "正在校验核心二进制完整性并重建快捷方式与系统集成...",
            "コア実行ファイルの整合性を検証し、ショートカットと統合を再構築中...",
            "Verifying core binaries and rebuilding shortcuts and system integration...",
        ],
    ),
    (
        "检查运行中的 IDE 1.0 实例并准备安全修复...",
        [
            "检查运行中的 IDE 1.0 实例并准备安全修复...",
            "実行中の IDE 1.0 インスタンスを確認して修復を準備中...",
            "Checking running IDE 1.0 instances for safe repair...",
        ],
    ),
    (
        "校验并重新释放 IDE1.0.exe 核心主程序...",
        [
            "校验并重新释放 IDE1.0.exe 核心主程序...",
            "IDE1.0.exe コア実行ファイルを検証して再展開中...",
            "Verifying and restoring IDE1.0.exe core executable...",
        ],
    ),
    (
        "修复 3D 赛璐珞与晶透主题运行时资源...",
        [
            "修复 3D 赛璐珞与晶透主题运行时资源...",
            "3D セルシェードおよびクリスタルテーマ設定を修復中...",
            "Repairing 3D cel-shaded and translucent theme runtime presets...",
        ],
    ),
    (
        "重建桌面/开始菜单快捷方式与环境变量 PATH...",
        [
            "重建桌面/开始菜单快捷方式与环境变量 PATH...",
            "デスクトップ・スタートメニューのショートカットと PATH を再構築中...",
            "Rebuilding Desktop/Start Menu shortcuts and User PATH...",
        ],
    ),
    (
        "刷新系统卸载注册表与托盘守护配置...",
        [
            "刷新系统卸载注册表与托盘守护配置...",
            "アンインストール登録情報とトレイ常駐設定を更新中...",
            "Refreshing Windows uninstall registration and tray daemon config...",
        ],
    ),
    (
        "即将完成修复...",
        [
            "即将完成修复...",
            "間もなく修復が完了します...",
            "Finalizing repair...",
        ],
    ),
    (
        "修复完成！IDE 1.0 已恢复至最佳状态 (REPAIRED)",
        [
            "修复完成！IDE 1.0 已恢复至最佳状态 (REPAIRED)",
            "修復完了！IDE 1.0 は正常な状態に復元されました (REPAIRED)",
            "Repair Complete! IDE 1.0 Is Ready (REPAIRED)",
        ],
    ),
    (
        "核心主程序、快捷方式与环境变量均已校验完毕，个人配置完好无损",
        [
            "核心主程序、快捷方式与环境变量均已校验完毕，个人配置完好无损",
            "本体・ショートカット・PATH の検証が完了し、個人設定は保持されています",
            "Core executable, shortcuts and PATH verified; user settings intact",
        ],
    ),
    (
        "已修复安装目录",
        [
            "已修复安装目录",
            "修復済みフォルダー",
            "Repaired Directory",
        ],
    ),
    (
        "正在卸载 IDE 1.0 (UNINSTALLING)",
        [
            "正在卸载 IDE 1.0 (UNINSTALLING)",
            "IDE 1.0 をアンインストール中 (UNINSTALLING)",
            "Uninstalling IDE 1.0 (UNINSTALLING)",
        ],
    ),
    (
        "正在静默移除已部署的程序文件、快捷方式与注册表记录...",
        [
            "正在静默移除已部署的程序文件、快捷方式与注册表记录...",
            "プログラム本体・ショートカット・登録情報を静かに削除しています...",
            "Silently removing installed binaries, shortcuts and registry entries...",
        ],
    ),
    (
        "停止关联的 IDE 1.0 托盘守护与后台实例...",
        [
            "停止关联的 IDE 1.0 托盘守护与后台实例...",
            "関連する IDE 1.0 トレイ常駐とプロセスを終了中...",
            "Stopping associated IDE 1.0 tray daemon and background instances...",
        ],
    ),
    (
        "移除桌面与开始菜单快捷方式入口...",
        [
            "移除桌面与开始菜单快捷方式入口...",
            "デスクトップとスタートメニューのショートカットを削除中...",
            "Removing Desktop and Start Menu shortcuts...",
        ],
    ),
    (
        "从用户环境变量 PATH 移除安装路径...",
        [
            "从用户环境变量 PATH 移除安装路径...",
            "ユーザー環境変数 PATH からインストール先を除外中...",
            "Removing install location from User PATH...",
        ],
    ),
    (
        "注销 Windows 应用列表卸载注册表项...",
        [
            "注销 Windows 应用列表卸载注册表项...",
            "Windows アプリ一覧のアンインストール登録を解除中...",
            "Unregistering IDE 1.0 from Windows Apps registry...",
        ],
    ),
    (
        "清理目标目录主程序与关联残留文件...",
        [
            "清理目标目录主程序与关联残留文件...",
            "対象フォルダー内の本体と関連ファイルを削除中...",
            "Cleaning up installed binaries in target directory...",
        ],
    ),
    (
        "即将完成卸载...",
        [
            "即将完成卸载...",
            "間もなくアンインストールが完了します...",
            "Finalizing uninstallation...",
        ],
    ),
    (
        "卸载完成！IDE 1.0 已从本机移除 (UNINSTALLED)",
        [
            "卸载完成！IDE 1.0 已从本机移除 (UNINSTALLED)",
            "アンインストール完了！IDE 1.0 を削除しました (UNINSTALLED)",
            "Uninstallation Complete! IDE 1.0 Removed (UNINSTALLED)",
        ],
    ),
    (
        "已安全清理所选程序组件与系统集成项，感谢您的使用",
        [
            "已安全清理所选程序组件与系统集成项，感谢您的使用",
            "選択されたコンポーネントと統合項目を安全に削除しました",
            "Selected binaries and system integrations have been safely removed",
        ],
    ),
    (
        "程序文件清理",
        [
            "程序文件清理",
            "プログラムファイルの削除",
            "Binary Cleanup",
        ],
    ),
    (
        "已清理目标目录",
        [
            "已清理目标目录",
            "削除済みフォルダー",
            "Cleaned Directory",
        ],
    ),
    (
        "快捷方式与环境变量",
        [
            "快捷方式与环境变量",
            "ショートカットと環境変数",
            "Shortcuts and Environment",
        ],
    ),
    (
        "已移除所选快捷方式、环境变量与注册表项",
        [
            "已移除所选快捷方式、环境变量与注册表项",
            "ショートカット・PATH・レジストリ登録をすべて削除しました",
            "Removed shortcuts, PATH entry and uninstall registration",
        ],
    ),
    (
        "已按选项保留部分系统集成项",
        [
            "已按选项保留部分系统集成项",
            "選択に従い一部のシステム統合項目を保持しました",
            "Kept unselected system integration items as requested",
        ],
    ),
    (
        "个人配置状态",
        [
            "个人配置状态",
            "個人設定の状態",
            "User Configuration",
        ],
    ),
    (
        "已按选择删除个人配置与会话缓存",
        [
            "已按选择删除个人配置与会话缓存",
            "選択に従い個人設定とセッションキャッシュを削除しました",
            "User preferences and session cache were removed",
        ],
    ),
    (
        "已为您保留个人偏好设置与会话配置",
        [
            "已为您保留个人偏好设置与会话配置",
            "個人設定とセッション構成はそのまま保持されました",
            "Your personal preferences and session configuration were preserved",
        ],
    ),
    (
        "完成并关闭向导 (FINISH)",
        [
            "完成并关闭向导 (FINISH)",
            "完了して閉じる (FINISH)",
            "Finish and Close (FINISH)",
        ],
    ),
    (
        "部分受保护程序文件需要管理员权限才能删除",
        [
            "部分受保护程序文件需要管理员权限才能删除",
            "一部の保護されたファイルの削除には管理者権限が必要です",
            "Some protected files require administrator privileges to remove",
        ],
    ),
    (
        "展开 Agent 协同平台",
        [
            "展开 Agent 协同平台",
            "Agent 協調プラットフォームを展開",
            "Expand Agent Collaboration Platform",
        ],
    ),
    (
        "折叠 Agent 协同平台",
        [
            "折叠 Agent 协同平台",
            "Agent 協調プラットフォームを折りたたむ",
            "Collapse Agent Collaboration Platform",
        ],
    ),
    (
        "全员协同调度",
        [
            "全员协同调度",
            "チーム並列オーケストレーション",
            "Multi-Agent Orchestration",
        ],
    ),
    (
        "搜索 Agent 名称或职责…",
        [
            "搜索 Agent 名称或职责…",
            "Agent の名前や役割を検索…",
            "Search agent name or role…",
        ],
    ),
    (
        "未找到匹配的 Agent",
        [
            "未找到匹配的 Agent",
            "一致する Agent が見つかりません",
            "No matching agents found",
        ],
    ),
    (
        "Agent 数量",
        [
            "Agent 数量",
            "Agent 数",
            "Agent Count",
        ],
    ),
    (
        "清空当前会话",
        [
            "清空当前会话",
            "現在のセッションをクリア",
            "Clear Active Session",
        ],
    ),
    (
        "切换至全员协同调度",
        [
            "切换至全员协同调度",
            "チーム並列オーケストレーションへ切替",
            "Switch to Multi-Agent Orchestration",
        ],
    ),
    (
        "返回单 Agent 精准控制",
        [
            "返回单 Agent 精准控制",
            "単一 Agent 精密制御へ戻る",
            "Back to Single-Agent Precision Control",
        ],
    ),
    (
        "分配当前文件",
        [
            "分配当前文件",
            "現在のファイルを割り当て",
            "Assign Active File",
        ],
    ),
    (
        "取消文件分配",
        [
            "取消文件分配",
            "ファイル割り当てを解除",
            "Unassign File",
        ],
    ),
    (
        "模型名称 (可选)",
        [
            "模型名称 (可选)",
            "モデル表示名 (任意)",
            "Display Name (Optional)",
        ],
    ),
    (
        "已配置模型",
        [
            "已配置模型",
            "設定済みモデル",
            "Configured Models",
        ],
    ),
    (
        "删除模型",
        [
            "删除模型",
            "モデルを削除",
            "Remove Model",
        ],
    ),
    (
        "尚未配置模型，请先添加",
        [
            "尚未配置模型，请先添加",
            "モデルが未設定です。先に設定を追加してください",
            "No models configured yet. Add one first.",
        ],
    ),
    (
        "添加或管理模型…",
        [
            "添加或管理模型…",
            "モデルを追加または管理…",
            "Add or Manage Models…",
        ],
    ),
    (
        "跟随全局默认",
        [
            "跟随全局默认",
            "グローバル既定に従う",
            "Follow Global Default",
        ],
    ),
    (
        "搜索已配置模型…",
        [
            "搜索已配置模型…",
            "設定済みモデルを検索…",
            "Search configured models…",
        ],
    ),
    (
        "精准控制",
        [
            "精准控制",
            "精密制御",
            "Control",
        ],
    ),
    (
        "Agent 编队精准控制",
        [
            "Agent 编队精准控制",
            "Agent 編成・精密制御",
            "Agent Roster & Precision Control",
        ],
    ),
    (
        "Agent 协同编队状态",
        [
            "Agent 协同编队状态",
            "Agent 協働編成ステータス",
            "Agent Team Status",
        ],
    ),
    (
        "就绪",
        [
            "就绪",
            "準備完了",
            "Ready",
        ],
    ),
    (
        "运行中",
        [
            "运行中",
            "実行中",
            "Running",
        ],
    ),
    (
        "收起模型与接口设置",
        [
            "收起模型与接口设置",
            "モデル・API設定を閉じる",
            "Collapse Model & API Settings",
        ],
    ),
    ("程序异常崩溃", ["程序异常崩溃", "異常終了", "Unexpected Crash"]),
    ("启动异常", ["启动异常", "起動エラー", "Startup Error"]),
    (
        "IDE1.0 遇到严重错误并已记录日志。",
        [
            "IDE1.0 遇到严重错误并已记录日志。",
            "IDE1.0 で重大なエラーが発生し、ログを記録しました。",
            "IDE1.0 encountered a critical error and saved a log.",
        ],
    ),
    ("日志路径", ["日志路径", "ログパス", "Log path"]),
    (
        "点击「反馈问题」将打开 GitHub Issues 页面。",
        [
            "点击「反馈问题」将打开 GitHub Issues 页面。",
            "「問題を報告」をクリックすると GitHub Issues を開きます。",
            "Click \"Report Issue\" to open GitHub Issues.",
        ],
    ),
    ("反馈问题", ["反馈问题", "問題を報告", "Report Issue"]),
    (
        "提示：文档关系图基于 Markdown 标题结构与引用生成。请在当前文档中添加 #、## 等标题，或使用 [[标题]] 引用关系后再试。",
        [
            "提示：文档关系图基于 Markdown 标题结构与引用生成。请在当前文档中添加 #、## 等标题，或使用 [[标题]] 引用关系后再试。",
            "ヒント：ドキュメント関係図は Markdown の見出し構造と参照から生成されます。現在のドキュメントに # / ## 見出しや [[見出し]] 参照を追加して再試行してください。",
            "Tip: Document Graph is generated from Markdown headings and links. Add # / ## headings or [[Heading]] references to the current document and try again.",
        ],
    ),
    (
        "插件命令失败 (编辑器不受影响)",
        [
            "插件命令失败 (编辑器不受影响)",
            "プラグインコマンドに失敗しました (エディタには影響ありません)",
            "Plugin command failed (editor unaffected)",
        ],
    ),
    ("日志", ["日志", "ログ", "Log"]),
    (
        "部分旧版本文件受权限保护未能完全清除，请手动移除",
        [
            "部分旧版本文件受权限保护未能完全清除，请手动移除",
            "一部の旧バージョンファイルは保護されており削除できませんでした。手動で削除してください",
            "Some legacy version files are protected and could not be removed; please delete manually",
        ],
    ),
    (
        "将自动卸载清理旧版本目录",
        [
            "将自动卸载清理旧版本目录",
            "旧バージョンフォルダーを自動アンインストールします",
            "Legacy installation will be automatically removed",
        ],
    ),
    (
        "架构: x86_64 Windows (Native)",
        [
            "架构: x86_64 Windows (Native)",
            "アーキテクチャ: x86_64 Windows (Native)",
            "Arch: x86_64 Windows (Native)",
        ],
    ),
    (
        "后台无阻塞拉取更新包，完成校验后即可极速重启应用...",
        [
            "后台无阻塞拉取更新包，完成校验后即可极速重启应用...",
            "バックグラウンドで更新を取得中です。検証完了後に再起動して適用できます...",
            "Fetching update package asynchronously; verify and restart cleanly once ready...",
        ],
    ),
    (
        "安全校验版本号、签名清单与可用下载包...",
        [
            "安全校验版本号、签名清单与可用下载包...",
            "バージョン・署名マニフェスト・配布パッケージを安全に検証しています...",
            "Verifying release version, signature manifest and available assets...",
        ],
    ),
    (
        "正在下载并校验数字签名与 SHA-256 (DOWNLOADING)",
        [
            "正在下载并校验数字签名与 SHA-256 (DOWNLOADING)",
            "署名と SHA-256 をダウンロード・検証中 (DOWNLOADING)",
            "Downloading & Verifying Signature / SHA-256",
        ],
    ),
    (
        "正在连接校验官方发行源 (CHECKING RELEASE)",
        [
            "正在连接校验官方发行源 (CHECKING RELEASE)",
            "公式リリースソースへ接続して確認中 (CHECKING RELEASE)",
            "Connecting to Official Release Feed (CHECKING)",
        ],
    ),
    ("我知道了 (GOT IT)", ["我知道了 (GOT IT)", "了解しました (GOT IT)", "Got It (OK)"]),
    (
        "请先完整阅读许可协议",
        ["请先完整阅读许可协议", "ライセンス契約を最後までお読みください", "Please Read the License Agreement"],
    ),
    (
        "请先将上方《IDE 1.0 最终用户许可与使用协议》滚动阅读至底部，并勾选同意条款后再点击下一步。",
        [
            "请先将上方《IDE 1.0 最终用户许可与使用协议》滚动阅读至底部，并勾选同意条款后再点击下一步。",
            "上の「IDE 1.0 エンドユーザー使用許諾契約」を最後までスクロールし、同意にチェックを入れてから次へ進んでください。",
            "Please scroll the License Agreement above all the way to the bottom and accept the terms before clicking Next.",
        ],
    ),
    (
        "请先勾选同意许可协议",
        ["请先勾选同意许可协议", "同意チェックボックスにチェックしてください", "Please Accept the License Agreement"],
    ),
    (
        "请先点击勾选下方「我已完整阅读并同意《IDE 1.0 最终用户许可与使用协议》」条款卡片，再点击继续。",
        [
            "请先点击勾选下方「我已完整阅读并同意《IDE 1.0 最终用户许可与使用协议》」条款卡片，再点击继续。",
            "下の「IDE 1.0 エンドユーザー使用許諾契約に同意します」カードにチェックを入れてから続行してください。",
            "Please check the License Agreement acceptance box below before proceeding to the next step.",
        ],
    ),
    (
        "安装目录不能为空",
        ["安装目录不能为空", "インストール先フォルダーを入力してください", "Installation Directory Required"],
    ),
    (
        "请先填写或点击浏览选择一个有效的目标安装目录后再继续安装。",
        [
            "请先填写或点击浏览选择一个有效的目标安装目录后再继续安装。",
            "有効なインストール先フォルダーを入力または選択してから続行してください。",
            "Please enter or browse to a valid installation directory before installing.",
        ],
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
        for (k, row) in CMD_T {
            assert!(seen.insert(*k), "命令表重复的键 {k}");
            assert_eq!(
                *k, row[0],
                "CMD_T 键必须与中文译文一致以保证 Lang::Zh O(1) 直通"
            );
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

    #[test]
    fn no_emoji_or_hollow_box_icons_in_i18n_tables() {
        let is_forbidden = |c: char| -> bool {
            let cp = c as u32;
            cp >= 0x1F000
                || cp == 0xFE0F
                || (0x2600..=0x27BF).contains(&cp)
                || matches!(cp, 0x2B50 | 0x25A1 | 0x25A6 | 0x25C8 | 0x22EE | 0x25BE)
        };
        for (k, row) in T.iter().chain(CMD_T.iter()) {
            for s in std::iter::once(k).chain(row.iter()) {
                assert!(
                    !s.chars().any(is_forbidden),
                    "i18n string contains forbidden emoji/hollow-box symbol: {s:?}"
                );
            }
        }
    }
}
