//! Sakura `.mac` 宏: 解析 / 规划交给 `editcore::macro_sakura::plan` (和 Tauri 版同一套), 这里决定每一步由谁执行、并执行它。
//! 每个 `S_` 函数只有三种结局 (`route`): 在这份 `Doc` 上直接执行; 转交 App 去执行 (保存、打开、查找下一个、拆分窗口这类要动窗口的);
//! 或者**明确报错并说明原因** (层叠窗口、SQL*Plus、外部帮助这类高速版不做的)。不静默跳过, 也不让宏做到一半才发现跑不了:
//! `plan_checked` 先把每一步都过一遍, 有一步跑不了整份宏都不执行。

use crate::doc::{Doc, RecEv};
use crate::find::FindOpts;
use editcore::document_io::Charset;
use editcore::macro_sakura::{self, Action, MacroStep};

/// 一次宏运行的环境
pub struct MacroEnv {
    /// Tab 宽度 (Tab 与空格的转换、缩进用)
    pub tab: usize,
    /// 宏里的剪贴板: `S_Cut` / `S_Copy` 写它, `S_Paste` 读它; 开始时是应用剪贴板历史里最新的一条
    pub clip: String,
    /// 宏运行中改过它 (运行完要同步回系统剪贴板)
    pub clip_changed: bool,
}

impl MacroEnv {
    pub fn new(tab: usize, clip: String) -> Self {
        MacroEnv { tab, clip, clip_changed: false }
    }
}

/// 一步由谁来执行
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// 在宏的目标文档上直接执行
    Doc,
    /// 转交 App: 命令表里的命令 id (`keymap::entry_by_id`), 由 App 按顺序执行
    App(&'static str),
    /// 高速版不执行; 附原因
    No(String),
}

/// 只动文档本身的 `S_` 函数 (按函数名)
#[derive(Clone, Copy)]
enum DocOp {
    Undo,
    Redo,
    SelectAll,
    Nop,
    /// editcore 编辑命令, 不碰剪贴板
    Edit(&'static str),
    /// editcore 编辑命令, 结果写进宏剪贴板 (剪切 / 复制类)
    EditClip(&'static str),
    /// 用宏剪贴板的内容执行编辑命令 (粘贴类)
    Paste(&'static str),
    IndentTab(bool),
    InsDate,
    ToggleIns,
    BeginSelect,
    BeginBox,
    Cancel,
    Bookmark(&'static str),
    BookmarkClear,
    CodeCnv(&'static str),
    /// 键盘扩展矩形选区: (移动键, 带 Ctrl, 步数)
    BoxNav(&'static str, bool, usize),
    /// 光标移动若干步 (不选)
    Nav(&'static str, usize),
    /// 光标移到下一 / 上一个段落边界 (不选)
    ParaMove(bool),
    Reopen(Charset),
}

fn doc_op(name: &str) -> Option<DocOp> {
    use DocOp::*;
    Some(match name {
        "Undo" => Undo,
        "Redo" => Redo,
        "SelectAll" => SelectAll,
        "Redraw" => Nop,
        "Cut" => EditClip("cut"),
        "Copy" => EditClip("copy"),
        "CopyCRLF" => EditClip("copy-crlf"),
        "Paste" => Paste("paste"),
        "PasteBox" => Paste("paste-box"),
        "CutLine" => EditClip("cut-line"),
        "DeleteLine" => Edit("delete-line"),
        "LineCutToStart" => EditClip("cut-line-start"),
        "LineDeleteToEnd" => Edit("delete-line-to-end"),
        "LineDeleteToStart" => Edit("delete-line-to-start"),
        "CopyLinesAsPassage" => EditClip("copy-passage"),
        "IndentTab" => IndentTab(false),
        "UnindentTab" => IndentTab(true),
        "InsDate" => InsDate,
        "ChgmodINS" => ToggleIns,
        "BeginSelect" => BeginSelect,
        "BeginBoxSelect" => BeginBox,
        "CancelMode" => Cancel,
        "BookmarkSet" => Bookmark("toggle"),
        "BookmarkNext" => Bookmark("next"),
        "BookmarkPrev" => Bookmark("prev"),
        "BookmarkReset" => BookmarkClear,
        "CodeCnvAUTOtoSJIS" => CodeCnv("auto2sjis"),
        "CodeCnvEMAIL" => CodeCnv("email"),
        "CodeCnvEUCtoSJIS" => CodeCnv("euc2sjis"),
        "CodeCnvSJIStoEUC" => CodeCnv("sjis2euc"),
        "CodeCnvSJIStoJIS" => CodeCnv("sjis2jis"),
        "CodeCnvSJIStoUTF7" => CodeCnv("sjis2utf7"),
        "CodeCnvSJIStoUTF8" => CodeCnv("sjis2utf8"),
        "CodeCnvUNICODEtoSJIS" => CodeCnv("unicode2sjis"),
        "CodeCnvUTF7toSJIS" => CodeCnv("utf72sjis"),
        "CodeCnvUTF8toSJIS" => CodeCnv("utf82sjis"),
        "Up2" => Nav("ArrowUp", 2),
        "Down2" => Nav("ArrowDown", 2),
        "GoNextParagraph" => ParaMove(true),
        "GoPrevParagraph" => ParaMove(false),
        "Left_BoxSel" => BoxNav("ArrowLeft", false, 1),
        "Right_BoxSel" => BoxNav("ArrowRight", false, 1),
        "Up_BoxSel" => BoxNav("ArrowUp", false, 1),
        "Down_BoxSel" => BoxNav("ArrowDown", false, 1),
        "Up2_BoxSel" => BoxNav("ArrowUp", false, 2),
        "Down2_BoxSel" => BoxNav("ArrowDown", false, 2),
        "WordLeft_BoxSel" => BoxNav("ArrowLeft", true, 1),
        "WordRight_BoxSel" => BoxNav("ArrowRight", true, 1),
        "GoLineTop_BoxSel" | "GoLogicalLineTop_BoxSel" => BoxNav("Home", false, 1),
        "GoLineEnd_BoxSel" => BoxNav("End", false, 1),
        "GoFileTop_BoxSel" => BoxNav("Home", true, 1),
        "GoFileEnd_BoxSel" => BoxNav("End", true, 1),
        "1PageUp_BoxSel" => BoxNav("PageUp", false, 1),
        "1PageDown_BoxSel" => BoxNav("PageDown", false, 1),
        "HalfPageUp_BoxSel" => BoxNav("PageUp", true, 1),
        "HalfPageDown_BoxSel" => BoxNav("PageDown", true, 1),
        "FileReopenEUC" => Reopen(Charset::EucJp),
        "FileReopenJIS" => Reopen(Charset::Iso2022Jp),
        "FileReopenSJIS" => Reopen(Charset::ShiftJis),
        "FileReopenUNICODE" => Reopen(Charset::Utf16Le),
        "FileReopenUNICODEBE" => Reopen(Charset::Utf16Be),
        "FileReopenUTF7" => Reopen(Charset::Utf7),
        "FileReopenUTF8" => Reopen(Charset::Utf8),
        "FileReopenCESU8" => Reopen(Charset::Cesu8),
        "FileReopenLatin1" => Reopen(Charset::Latin1),
        _ => return None,
    })
}

/// 要动窗口 / 文件 / 查找状态的 `S_` 函数 → 命令表里对应命令的 id (按函数名); 宏运行器把它们按顺序转交 App
const APP_FUNCS: &[(&str, &str)] = &[
    ("About", "c.about"), ("Browse", "c.browse"), ("CopyFilename", "app.copy-fname"), ("CopyPath", "app.copy-path"), ("CreateKeyBindList", "c.key-list"),
    ("ExecCommandDialog", "app.exec-cmd"), ("ExitAll", "c.exit"), ("ExitAllEditors", "c.exit"),
    ("FileClose", "c.close-tab"), ("FileCloseOpen", "c.close-open"), ("FileNew", "c.new-file"), ("FileNewNewWindow", "c.new-window"), ("FileOpen", "c.open-file"),
    ("FileSave", "c.save"), ("FileSaveAs", "c.save-as"), ("FileSaveAll", "c.save-all"), ("FileSaveClose", "c.save-close"),
    ("WinClose", "c.close-tab"), ("WinCloseAll", "c.close-all"), ("TabCloseLeft", "c.close-left"), ("TabCloseOther", "c.close-others"), ("TabCloseRight", "c.close-right"),
    ("TabMoveLeft", "c.tab-move-left"), ("TabMoveRight", "c.tab-move-right"), ("NextWindow", "c.next-tab"), ("PrevWindow", "c.prev-tab"),
    ("NextGroup", "app.group-next"), ("PrevGroup", "app.group-prev"), ("TabSeparate", "app.tab-separate"), ("TabJointNext", "app.tab-joint-next"), ("TabJointPrev", "app.tab-joint-prev"),
    ("BindWindow", "app.bind-window"), ("GroupClose", "app.group-close"),
    ("Font", "c.common-config"), ("Option", "c.common-config"), ("OptionType", "c.common-config"), ("TypeList", "c.common-config"),
    ("GrepDialog", "c.grep"), ("GrepReplaceDialog", "c.grep"), ("ReplaceDialog", "c.replace"), ("SearchDialog", "c.find"), ("SearchNext", "c.find-next"), ("SearchPrev", "c.find-prev"),
    ("SearchClearMark", "app.find-marks"), ("JumpSrchStartPos", "app.find-start"), ("JumpDialog", "goto-line"), ("JumpHistNext", "c.hist-next"), ("JumpHistPrev", "c.hist-prev"),
    ("JumpHistSet", "c.hist-mark"), ("BracketPair", "c.bracket-jump"), ("CurLineCenter", "app.cur-center"), ("Hokan", "app.completion"),
    ("ISearchNext", "app.is-fwd"), ("ISearchPrev", "app.is-back"), ("ISearchRegexpNext", "app.is-regex-fwd"), ("ISearchRegexpPrev", "app.is-regex-back"),
    ("ISearchMigemoNext", "app.is-migemo-fwd"), ("ISearchMigemoPrev", "app.is-migemo-back"),
    ("BookmarkView", "c.bookmark-view"), ("Compare", "app.diff-tab"), ("DiffDialog", "app.diff-list"), ("DiffNext", "app.diff-next"), ("DiffPrev", "app.diff-prev"), ("DiffReset", "app.diff-clear"),
    ("ExecKeyMacro", "c.macro-play"), ("RecKeyMacro", "c.macro-start"), ("SaveKeyMacro", "app.macro-save"), ("LoadKeyMacro", "app.macro-load"),
    ("MaximizeH", "c.maximize"), ("MaximizeV", "c.maximize"), ("MinimizeAll", "c.minimize-window"), ("TopMost", "c.topmost"),
    ("MenuAllFunc", "c.palette"), ("OpenHfromtoC", "app.open-hc"),
    ("Print", "c.print"), ("PrintPageSetup", "c.print-setup"), ("PrintPreview", "c.print-preview"), ("PropertyFile", "c.file-properties"),
    ("ShowFuncKey", "c.funckey"), ("ShowStatusbar", "c.status-bar"), ("ShowToolbar", "c.toolbar"), ("ShowMiniMap", "c.minimap"), ("SelectCountMode", "app.sel-count-mode"),
    ("SplitH", "c.split-rows"), ("SplitV", "c.split"), ("SplitVH", "c.split-quad"),
    ("TagJump", "app.tag-jump"), ("DirectTagJump", "app.tag-jump"), ("TagJumpBack", "app.tag-back"), ("TagJumpKeyword", "app.tag-keyword"), ("TagsMake", "app.tags-make"),
    ("TmpWrapNoWrap", "app.wrap-none"), ("TmpWrapSetting", "app.wrap-col"), ("TmpWrapWindow", "app.wrap-window"), ("WrapWindowWidth", "app.wrap-window"),
    ("Base64Decode", "app.base64"), ("UUDecode", "app.uudecode"), ("CopyAddCRLF", "app.copy-wrapped"), ("CopyColorHtml", "app.copy-html"), ("CopyColorHtmlLineNumber", "app.copy-html-ln"),
    ("CopyTag", "app.copy-tag"), ("CtrlCodeDialog", "app.ctrl-codes"), ("WinOutput", "c.terminal"),
    ("WinList", "app.window-list"), ("DlgWinList", "app.window-list"),
    ("HelpContents", "app.help"), ("ExtHelp1", "app.ext-help"), ("ExtHtmlHelp", "app.ext-help"), ("HelpSearch", "app.help-search"), ("ToggleKeySearch", "app.key-search"),
    ("Favorite", "app.pick-favorite"), ("FileUsedRecently", "app.pick-recent"), ("InsFileUsedRecently", "app.ins-recent-file"), ("InsFolderUsedRecently", "app.ins-recent-dir"),
    ("ChgCharset", "app.pick-charset"), ("FileReopen", "app.pick-reopen"), ("Outline", "app.pane-outline"), ("FileTree", "app.pane-files"),
];

/// 高速版明确不做的 `S_` 函数, 和原因 (不做 ≠ 漏做: 都是有意的取舍或这里做不到)
const NOT_APPLICABLE: &[(&str, &str)] = &[
    ("Cascade", "层叠窗口: 本应用是单窗口多标签 (有意偏差)"),
    ("TileH", "平铺窗口: 本应用是单窗口多标签 (有意偏差)"),
    ("TileV", "平铺窗口: 本应用是单窗口多标签 (有意偏差)"),
    ("ActivateSQLPlus", "SQL*Plus 刻意不做"),
    ("PLSQLCompileOnSQLPlus", "SQL*Plus 刻意不做"),
    ("Reconvert", "IME 再変換: egui 没有触发 TSF 的接口"),
    ("ExecExtMacro", "JS 宏没有移植 (高速版只运行 Sakura .mac 和 PPA)"),
    ("ShowTab", "标签栏随文档个数和界面样式自动显示, 没有单独的开关"),
];

/// 一步该由谁执行
pub fn route(step: &MacroStep) -> Route {
    let Action::Command { id, cmd, key, .. } = &step.action else { return Route::Doc };
    let name = step.name.strip_prefix("S_").unwrap_or(&step.name);
    if doc_op(name).is_some() {
        return Route::Doc;
    }
    if let Some((_, app)) = APP_FUNCS.iter().find(|(n, _)| *n == name) {
        return Route::App(app);
    }
    if let Some((_, why)) = NOT_APPLICABLE.iter().find(|(n, _)| *n == name) {
        return Route::No((*why).to_string());
    }
    match (cmd, key) {
        (Some(c), _) if doc_command_known(c) => Route::Doc,
        (Some(c), _) => Route::No(format!("未支持的命令 {c}")),
        (None, Some(k)) if key_known(k) => Route::Doc,
        _ => Route::No(format!("这条命令高速版还不支持 ({id})")),
    }
}

/// 解析并整体预检: 解析错误、或有一步 `route` 是 `No`, 整份宏都不执行
pub fn plan_checked(src: &str) -> Result<Vec<MacroStep>, String> {
    let steps = macro_sakura::plan(src).map_err(|e| e.to_string())?;
    for s in &steps {
        if let Route::No(why) = route(s) {
            return Err(format!("第 {} 行 {}: {why}", s.line, s.name));
        }
    }
    Ok(steps)
}

/// 只在一份文档上执行宏 (没有 App 可以转交): 要转交 App 的步骤整体报错。测试和不带窗口的场合用
pub fn run_mac(doc: &mut Doc, src: &str, tab: usize) -> Result<usize, String> {
    let steps = plan_checked(src)?;
    for s in &steps {
        if let Route::App(id) = route(s) {
            return Err(format!("第 {} 行 {}: 这条命令要由应用窗口执行 ({id}), 这里不支持", s.line, s.name));
        }
    }
    let mut env = MacroEnv::new(tab, String::new());
    for s in &steps {
        run_step(doc, s, &mut env)?;
    }
    Ok(steps.len())
}

/// 执行一步。`Ok(None)` = 已在 `doc` 上执行完; `Ok(Some(id))` = 这一步要由 App 执行命令 `id` (这里什么也没做); `Err` = 跑不了或出错
pub fn run_step(doc: &mut Doc, s: &MacroStep, env: &mut MacroEnv) -> Result<Option<&'static str>, String> {
    match route(s) {
        Route::App(id) => return Ok(Some(id)),
        Route::No(why) => return Err(format!("第 {} 行 {}: {why}", s.line, s.name)),
        Route::Doc => {}
    }
    let tab = env.tab;
    match &s.action {
        Action::Command { id, cmd, key, .. } => {
            let name = s.name.strip_prefix("S_").unwrap_or(&s.name);
            if let Some(op) = doc_op(name) {
                run_op(doc, op, env).map_err(|e| format!("第 {} 行 {}: {e}", s.line, s.name))?;
            } else if let Some(c) = cmd {
                if !doc_command(doc, c, tab) {
                    return Err(format!("第 {} 行 {}: 未支持的命令 {c}", s.line, s.name));
                }
            } else if !key.as_deref().is_some_and(|k| key_action(doc, k, tab)) {
                return Err(format!("第 {} 行 {}: 这条命令高速版还不支持 ({id})", s.line, s.name));
            }
        }
        Action::InsText { text } => doc.type_text(text),
        Action::Char { code } => {
            if *code == 13 { doc.edit("new-line", None); } else if let Some(c) = char::from_u32(*code) { doc.type_text(&c.to_string()); }
        }
        Action::MoveCursor { row, col, select } | Action::MoveCursorLayout { row, col, select } => doc.move_to(*row, *col, *select),
        Action::Jump { line, .. } => doc.goto((((*line).max(1)) as usize) - 1, 0),
        Action::SearchNext { text, flags } => { let o = FindOpts::from_flags(&macro_sakura::SearchFlags::from_bits(*flags)); doc.find_step(text, o, true).map_err(|e| e.to_string())?; }
        Action::SearchPrev { text, flags } => { let o = FindOpts::from_flags(&macro_sakura::SearchFlags::from_bits(*flags)); doc.find_step(text, o, false).map_err(|e| e.to_string())?; }
        Action::ReplaceAll { search, replace, flags } => {
            let f = macro_sakura::SearchFlags::from_bits(*flags);
            doc.replace_all(search, replace, FindOpts::from_flags(&f), f.in_selection).map_err(|e| e.to_string())?;
        }
        Action::ReplaceNext { search, replace, flags } => {
            // 先找到下一处, 再按 "置换" 的语义换掉它 (正则下 $1 \1 $& 会展开) 并停在再下一处
            let o = FindOpts::from_flags(&macro_sakura::SearchFlags::from_bits(*flags));
            if doc.find_step(search, o, true).map_err(|e| e.to_string())? { doc.replace_one(search, replace, o).map_err(|e| e.to_string())?; }
        }
    }
    Ok(None)
}

fn run_op(doc: &mut Doc, op: DocOp, env: &mut MacroEnv) -> Result<(), String> {
    match op {
        DocOp::Undo => doc.undo(),
        DocOp::Redo => doc.redo(),
        DocOp::SelectAll => doc.select_all(),
        DocOp::Nop => {}
        DocOp::Edit(c) => { doc.edit(c, None); }
        DocOp::EditClip(c) => {
            if let Some(t) = doc.edit(c, None) {
                if !t.is_empty() {
                    env.clip = t;
                    env.clip_changed = true;
                }
            }
        }
        DocOp::Paste(c) => {
            if !env.clip.is_empty() {
                doc.edit(c, Some(env.clip.clone()));
            }
        }
        DocOp::IndentTab(outdent) => doc.indent(outdent, env.tab),
        DocOp::InsDate => doc.type_text(&crate::textconv::local_now().date()),
        DocOp::ToggleIns => doc.overwrite = !doc.overwrite,
        DocOp::BeginSelect => doc.toggle_sel_mode(),
        DocOp::BeginBox => { let [r, c] = doc.cursor; doc.box_start(r, c); }
        DocOp::Cancel => {
            doc.sel = None;
            doc.anchor = None;
            doc.box_sel = None;
            doc.extra.clear();
            doc.sel_mode = false;
        }
        DocOp::Bookmark(op) => doc.bookmark(op),
        DocOp::BookmarkClear => doc.bookmarks.clear(),
        DocOp::CodeCnv(id) => { doc.convert_selection(|t| crate::textconv::codecnv(id, t)); }
        DocOp::BoxNav(key, ctrl, times) => doc.box_nav(key, ctrl, times),
        DocOp::Nav(key, times) => { for _ in 0..times { doc.nav(key, false, false, 20); } }
        DocOp::ParaMove(fwd) => {
            doc.select_para(fwd);
            let c = doc.cursor;
            doc.sel = None;
            doc.anchor = None;
            doc.cursor = c;
        }
        DocOp::Reopen(cs) => {
            if doc.dirty {
                return Err("有未保存的修改: 不能按别的编码重新打开".into());
            }
            doc.reopen_as(cs)?;
        }
    }
    Ok(())
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

/// `doc_command` 认不认这条命令 (预检用: 不执行, 只判断)。和 `doc_command` 的分支一一对应
fn doc_command_known(c: &str) -> bool {
    matches!(
        c,
        "indent-space" | "unindent-space" | "sel-para-next" | "sel-para-prev" | "modify-next" | "modify-prev" | "modify-next-sel" | "modify-prev-sel" | "funclist-next" | "funclist-prev"
            | "insert-time" | "insert-date" | "toggle-readonly" | "eol-crlf" | "eol-lf" | "eol-cr"
    ) || c.strip_prefix("codecnv-").is_some_and(codecnv_known)
        || editcore::edit_ops::transform_text(c, "", 4).is_some()
        || editcore::edit_cmd::run(&editcore::edit_cmd::EditReq { cmd: c.into(), row: 0, col: 0, total_rows: 1, rows: vec![(0, String::new())], ..Default::default() }).is_some()
}

/// 编码转换 id 存在吗 (对空文本转换会因为「自动判别不出」而失败, 所以只看是不是「未知的编码转换」)
fn codecnv_known(id: &str) -> bool {
    !matches!(crate::textconv::codecnv(id, ""), Err(e) if e.starts_with("未知的编码转换"))
}

/// `key_action` 认不认这个默认键 (预检用)
fn key_known(key: &str) -> bool {
    if edit_for_key(key).is_some() || TRANSFORM_KEYS.iter().any(|(k, _)| *k == key) || LINES_KEYS.iter().any(|(k, _)| *k == key) {
        return true;
    }
    let mut alt = false;
    let mut ctrl = false;
    let mut base = "";
    for part in key.split('+') {
        match part {
            "Control" | "Ctrl" => ctrl = true,
            "Shift" => {}
            "Alt" => alt = true,
            other => base = other,
        }
    }
    !alt && (matches!(base, "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp" | "PageDown" | "Backspace" | "Delete") || (base == "Enter" && !ctrl))
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

    fn step_of(name: &str) -> MacroStep {
        macro_sakura::plan(&format!("S_{name}();")).unwrap_or_else(|e| panic!("S_{name}: {e}")).remove(0)
    }

    /// 宏函数表里的每一个函数都有明确的结局: 在文档上跑 / 转交 App (命令 id 真实存在) / 不执行且登记了原因
    #[test]
    fn every_sakura_function_is_runnable_handed_to_the_app_or_explained() {
        let (mut doc_n, mut app_n, mut no_n) = (0, 0, 0);
        let mut unexplained = vec![];
        for n in macro_sakura::command_names() {
            match route(&step_of(n)) {
                Route::Doc => doc_n += 1,
                Route::App(id) => {
                    app_n += 1;
                    assert!(crate::keymap::entry_by_id(id).is_some(), "S_{n} 转交的命令 {id} 不在命令表里");
                }
                Route::No(_) => {
                    no_n += 1;
                    if !NOT_APPLICABLE.iter().any(|(m, _)| *m == n) {
                        unexplained.push(n);
                    }
                }
            }
        }
        assert!(unexplained.is_empty(), "这些函数既跑不了又没登记原因: {unexplained:?}");
        assert_eq!(doc_n + app_n + no_n, macro_sakura::command_names().len());
        // 实测: 268 个函数 = 142 在文档上跑 + 103 转交 App + 23 明确不执行
        assert!(doc_n >= 140 && app_n >= 118 && no_n <= 8, "文档上能跑 {doc_n}, 转交 App {app_n}, 不执行 {no_n}");
        let names = macro_sakura::command_names();
        for (name, why) in NOT_APPLICABLE {
            assert!(!why.is_empty() && names.contains(name), "不适用表里的 {name} 不在宏函数表里 (过期的登记)");
        }
        for (name, _) in APP_FUNCS {
            assert!(names.contains(name), "转交表里的 {name} 不在宏函数表里");
        }
    }

    /// 路由到「文档上执行」的函数, 在一份普通文档上都真的跑得通 (只有重开编码要有文件路径, 在这里会报错)
    #[test]
    fn every_doc_routed_function_actually_runs() {
        for n in macro_sakura::command_names() {
            let s = step_of(n);
            if route(&s) != Route::Doc {
                continue;
            }
            let mut d = Doc::untitled(1, 1);
            d.lines = vec!["abc def".into(), "".into(), "xyz".into(), "".into(), "end".into()];
            d.select_all();
            let mut env = MacroEnv::new(4, "clip".into());
            let r = run_step(&mut d, &s, &mut env);
            if n.starts_with("FileReopen") {
                assert!(r.is_err(), "没有保存过的文档不能重开: S_{n}");
            } else {
                assert_eq!(r, Ok(None), "S_{n} 在文档上执行失败");
            }
        }
    }

    #[test]
    fn window_level_functions_are_handed_to_the_app_in_order_and_touch_nothing_here() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["keep".into()];
        let mut env = MacroEnv::new(4, String::new());
        assert_eq!(run_step(&mut d, &step_of("FileSave"), &mut env), Ok(Some("c.save")));
        assert_eq!(run_step(&mut d, &step_of("SplitV"), &mut env), Ok(Some("c.split")));
        assert_eq!(run_step(&mut d, &step_of("SearchNext"), &mut env), Ok(Some("c.find-next")));
        assert_eq!(d.lines, ["keep"]);
        assert_eq!(route(&step_of("FileSave")), Route::App("c.save"));
    }

    #[test]
    fn one_unrunnable_step_stops_the_whole_macro_before_anything_changes() {
        let e = plan_checked("S_InsText(\"x\");\nS_Cascade();").unwrap_err();
        assert!(e.contains("第 2 行") && e.contains("层叠窗口"), "{e}");
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["keep".into()];
        assert!(run_mac(&mut d, "S_InsText(\"x\");\nS_Reconvert();", 4).is_err());
        assert_eq!(d.lines, ["keep"], "整份宏都没执行");
        let e = run_mac(&mut d, "S_InsText(\"x\");\nS_FileSave();", 4).unwrap_err();
        assert!(e.contains("c.save") && e.contains("不支持"), "只能在文档上跑的入口遇到窗口类步骤: {e}");
        assert_eq!(d.lines, ["keep"]);
    }

    #[test]
    fn macro_clipboard_connects_cut_copy_and_paste() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["one".into(), "two".into(), "three".into()];
        d.goto(1, 0);
        let mut env = MacroEnv::new(4, String::new());
        run_step(&mut d, &step_of("Copy"), &mut env).unwrap();
        assert_eq!((env.clip.as_str(), env.clip_changed), ("two\n", true), "没有选区的复制 = 整行 (带换行)");
        d.goto(0, 0);
        run_step(&mut d, &step_of("Paste"), &mut env).unwrap();
        assert_eq!(d.lines, ["two", "one", "two", "three"], "整行粘贴到光标处");
        d.goto(3, 0);
        run_step(&mut d, &step_of("CutLine"), &mut env).unwrap();
        assert_eq!((env.clip.as_str(), d.lines.len()), ("three", 3), "剪切整行: 行没了, 行文字进宏剪贴板 (editcore 的 Sakura 语义: 不带换行)");
        let mut env2 = MacroEnv::new(4, "ZZ".into());
        d.goto(0, 0);
        run_step(&mut d, &step_of("Paste"), &mut env2).unwrap();
        assert!(d.lines[0].starts_with("ZZ"), "{:?}", d.lines);
        assert!(!env2.clip_changed, "只粘贴不改剪贴板");
    }

    #[test]
    fn box_selection_grows_from_the_keyboard_and_other_document_functions_behave() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["abcdef".into(), "ghijkl".into(), "mnopqr".into()];
        d.goto(0, 1);
        let mut env = MacroEnv::new(4, String::new());
        for name in ["Right_BoxSel", "Right_BoxSel", "Down_BoxSel"] {
            run_step(&mut d, &step_of(name), &mut env).unwrap();
        }
        assert_eq!((d.box_sel, d.cursor), (Some([0, 1, 1, 3]), [1, 3]), "矩形从起点拉到光标");
        run_step(&mut d, &step_of("Left_BoxSel"), &mut env).unwrap();
        assert_eq!(d.box_sel, Some([0, 1, 1, 2]), "往回按: 矩形缩小, 起点不动");
        run_step(&mut d, &step_of("CancelMode"), &mut env).unwrap();
        assert!(d.box_sel.is_none() && d.sel.is_none());
        // 书签 / 缩进 / 段落 / 向下两行 / 插入模式
        d.lines = vec!["a".into(), "".into(), "b".into(), "c".into(), "d".into()];
        d.goto(0, 0);
        run_step(&mut d, &step_of("BookmarkSet"), &mut env).unwrap();
        assert_eq!(d.bookmarks.len(), 1, "书签设上了");
        run_step(&mut d, &step_of("BookmarkReset"), &mut env).unwrap();
        assert!(d.bookmarks.is_empty());
        run_step(&mut d, &step_of("Down2"), &mut env).unwrap();
        assert_eq!(d.cursor[0], 2);
        run_step(&mut d, &step_of("GoNextParagraph"), &mut env).unwrap();
        assert!(d.sel.is_none(), "只移动光标, 不选");
        let was = d.overwrite;
        run_step(&mut d, &step_of("ChgmodINS"), &mut env).unwrap();
        assert_eq!(d.overwrite, !was);
        d.goto(0, 0);
        run_step(&mut d, &step_of("IndentTab"), &mut env).unwrap();
        assert!(d.lines[0].starts_with(' ') || d.lines[0].starts_with('\t'), "{:?}", d.lines[0]);
        run_step(&mut d, &step_of("Undo"), &mut env).unwrap();
        assert_eq!(d.lines[0], "a", "撤销");
        run_step(&mut d, &step_of("Redo"), &mut env).unwrap();
        assert_ne!(d.lines[0], "a", "重做");
    }

    #[test]
    fn code_conversion_functions_convert_the_selection() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["abc".into()];
        d.select_all();
        let mut env = MacroEnv::new(4, String::new());
        run_step(&mut d, &step_of("CodeCnvSJIStoUTF8"), &mut env).unwrap();
        assert_eq!(d.lines, ["abc"], "纯 ASCII 转换后不变, 但函数能跑");
        for id in ["auto2sjis", "email", "euc2sjis", "sjis2euc", "sjis2jis", "sjis2utf7", "sjis2utf8", "unicode2sjis", "utf72sjis", "utf82sjis"] {
            assert!(codecnv_known(id), "编码转换 id {id} 不存在");
        }
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
