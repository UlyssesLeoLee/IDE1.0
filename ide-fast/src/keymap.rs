//! 命令注册表 + 键位表: 所有能触发的命令 (cmds.rs 的编辑命令 + 窗口级动作) 登记在这里, 每条有稳定 id、默认键和标签。
//! 快捷键分发、命令面板、键位设置、键位一览、菜单里的快捷键提示全部读这一张表, 所以不会对不上。
//! 用户改过的键 (只存和默认不同的) 放在 `Settings::keys`, 这里用线程局部量缓存生效的绑定 (UI 只在主线程, 和 i18n 的语言量同类)。
//! 本文件不碰 egui 的窗口, 只做解析 / 冲突检测 / 校验, 所以都能单测。

use crate::cmds::{self, Action};
use eframe::egui::{Key, Modifiers};
use std::cell::RefCell;
use std::sync::OnceLock;

use crate::i18n::tr;

/// 窗口级动作 (菜单里有、不属于 editcore 编辑命令的那些)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum App {
    NewDoc, OpenFile, OpenFolder, Save, SaveAs, SaveAll, SaveClose, CloseTab, CloseAll, CloseOpen, Reload, RecentClear, Print, Browse, Props, NewWindow, Exit,
    Undo, Redo, Cut, Copy, Paste, SelectAll,
    Find, Replace, Grep, FindNext, FindPrev, FindWord, FindWordPrev, ReplaceAll,
    DiffTab, DiffFile, DiffList, DiffNext, DiffPrev, DiffClear,
    FoldToggle, UnfoldAll,
    BmToggle, BmNext, BmPrev, BmView, BmReset, BracketJump, HistPrev, HistNext, HistMark,
    Split, NextTab, PrevTab, TabLeft, TabRight, CloseOthers, CloseLeft, CloseRight, Minimize, Maximize, Fullscreen, Topmost,
    MacroToggle, MacroPlay, MacroRunDoc, MacroRunFile, MacroRunPpa,
    Terminal, MdPreview, Dark, Highlight, Wrap, Sidebar, LineNumbers, StatusBar, Whitespace, Ruler, CurLine, Minimap, MinimalMode, ZoomIn, ZoomOut, ZoomReset,
    Prefs, ExportCfg, ImportCfg, ImportSakura, ResetCfg, KeyConfig, KeyList, Palette,
    CopyFname, CopyPath, SelParaPrev, SelParaNext, ModNext, ModPrev, MacroSave, MacroLoad, About,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Run {
    Act(Action),
    App(App),
}

/// 标签: 语义键 (`tr`) 或中文原文 (`trz`)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Label {
    Tr(&'static str),
    Zh(&'static str),
}

impl Label {
    pub fn text(self) -> &'static str {
        match self {
            Label::Tr(k) => crate::i18n::tr(k),
            Label::Zh(z) => crate::i18n::trz(z),
        }
    }
}

pub struct Entry {
    /// 稳定 id (存进 settings.json): App 命令用 i18n 键 / `app.*`, 编辑命令用 `edit.*` / `tf.*` / `lines.*`
    pub id: String,
    pub label: Label,
    /// 分类 (命令面板和键位表里显示)
    pub cat: Label,
    /// 默认键, 多个备选用 `|` 分隔
    pub default: &'static str,
    pub run: Run,
    /// 由编辑区 / 系统原生处理的键 (剪切 / 复制 / 粘贴): 只显示, 不能改
    pub fixed: bool,
    /// 只在比较视图开着时才生效 (和上移 / 下移行共用 Alt+↑↓), 不参与冲突检测
    pub needs_diff: bool,
}

fn app(id: &str, label: Label, cat: &'static str, default: &'static str, a: App) -> Entry {
    Entry { id: id.into(), label, cat: Label::Tr(cat), default, run: Run::App(a), fixed: false, needs_diff: false }
}

fn t(id: &'static str, default: &'static str, cat: &'static str, a: App) -> Entry {
    app(id, Label::Tr(id), cat, default, a)
}

fn z(id: &str, zh: &'static str, default: &'static str, cat: &'static str, a: App) -> Entry {
    app(id, Label::Zh(zh), cat, default, a)
}

pub fn action_id(a: Action) -> String {
    match a {
        Action::Edit(n) => format!("edit.{n}"),
        Action::Transform(k) => format!("tf.{k}"),
        Action::Lines(k) => format!("lines.{k}"),
        Action::Indent => "indent".into(),
        Action::Outdent => "outdent".into(),
        Action::ToggleComment => "toggle-comment".into(),
        Action::GotoLine => "goto-line".into(),
        Action::ToggleOverwrite => "overwrite".into(),
        Action::ToggleReadonly => "readonly".into(),
    }
}

fn build() -> Vec<Entry> {
    use App::*;
    let mut v: Vec<Entry> = vec![
        t("c.new-file", "Ctrl+N", "menu.file", NewDoc), t("c.open-file", "Ctrl+O", "menu.file", OpenFile), t("c.open-folder", "", "menu.file", OpenFolder),
        t("c.save", "Ctrl+S", "menu.file", Save), t("c.save-as", "Ctrl+Shift+S", "menu.file", SaveAs), t("c.save-all", "Ctrl+Alt+Shift+S", "menu.file", SaveAll),
        t("c.save-close", "", "menu.file", SaveClose), t("c.close-tab", "Ctrl+W", "menu.file", CloseTab), t("c.close-all", "Ctrl+Shift+W", "menu.file", CloseAll),
        t("c.close-open", "Ctrl+Shift+F4", "menu.file", CloseOpen), t("c.reload", "", "menu.file", Reload), t("c.recent-clear", "", "menu.file", RecentClear),
        t("c.print", "Ctrl+P", "menu.file", Print), t("c.browse", "", "menu.file", Browse), t("c.file-properties", "Alt+Enter", "menu.file", Props),
        t("c.new-window", "", "menu.file", NewWindow), t("c.exit", "", "menu.file", Exit),
        t("c.undo", "Ctrl+Z", "menu.edit", Undo), t("c.redo", "Ctrl+Y", "menu.edit", Redo),
        t("c.cut", "Ctrl+X", "menu.edit", Cut), t("c.copy", "Ctrl+C", "menu.edit", Copy), t("c.paste", "Ctrl+V", "menu.edit", Paste),
        t("c.select-all", "Ctrl+A", "menu.edit", SelectAll),
        t("c.find", "Ctrl+F|Ctrl+I", "menu.search", Find), t("c.replace", "Ctrl+H|Ctrl+R", "menu.search", Replace),
        t("c.grep", "Ctrl+G|Ctrl+Shift+F|Ctrl+Alt+G", "menu.search", Grep),
        t("c.find-next", "F3", "menu.search", FindNext), t("c.find-prev", "Shift+F3", "menu.search", FindPrev), t("c.find-word", "Ctrl+F3", "menu.search", FindWord),
        z("app.find-word-prev", "向前查找当前词", "Ctrl+Shift+F3", "menu.search", FindWordPrev), t("c.replace-all", "Ctrl+Alt+R", "menu.search", ReplaceAll),
        z("app.diff-tab", "与其他标签比较…", "", "menu.search", DiffTab), z("app.diff-file", "与文件比较…", "", "menu.search", DiffFile), z("app.diff-list", "差异列表", "", "menu.search", DiffList),
        z("app.diff-next", "下一处差异", "Alt+Down", "menu.search", DiffNext), z("app.diff-prev", "上一处差异", "Alt+Up", "menu.search", DiffPrev),
        z("app.diff-clear", "清除比较", "", "menu.search", DiffClear),
        t("c.fold-toggle", "Ctrl+Shift+[", "menu.fold", FoldToggle), t("c.unfold-all", "Ctrl+Shift+]", "menu.fold", UnfoldAll),
        t("c.bookmark-toggle", "Ctrl+F2", "menu.bookmark", BmToggle), t("c.bookmark-next", "F2", "menu.bookmark", BmNext), t("c.bookmark-prev", "Shift+F2", "menu.bookmark", BmPrev),
        t("c.bookmark-view", "Alt+F2", "menu.bookmark", BmView), t("c.bookmark-reset", "Ctrl+Shift+F2", "menu.bookmark", BmReset),
        t("c.bracket-jump", "Ctrl+[|Ctrl+]", "menu.bookmark", BracketJump), t("c.hist-prev", "Alt+P", "menu.bookmark", HistPrev), t("c.hist-next", "Alt+N", "menu.bookmark", HistNext),
        t("c.hist-mark", "", "menu.bookmark", HistMark),
        t("c.split", "Ctrl+\\", "menu.window", Split), t("c.next-tab", "Ctrl+Tab", "menu.window", NextTab), t("c.prev-tab", "Ctrl+Shift+Tab", "menu.window", PrevTab),
        t("c.tab-move-left", "", "menu.window", TabLeft), t("c.tab-move-right", "", "menu.window", TabRight),
        t("c.close-others", "", "menu.window", CloseOthers), t("c.close-left", "", "menu.window", CloseLeft), t("c.close-right", "", "menu.window", CloseRight),
        t("c.minimize-window", "", "menu.window", Minimize), t("c.maximize", "", "menu.window", Maximize), t("c.fullscreen", "F11", "menu.window", Fullscreen), t("c.topmost", "", "menu.window", Topmost),
        t("c.macro-start", "Ctrl+Shift+M", "menu.macro", MacroToggle), t("c.macro-play", "Ctrl+Shift+L", "menu.macro", MacroPlay),
        t("c.macro-run-doc", "", "menu.macro", MacroRunDoc), t("c.macro-run-file", "", "menu.macro", MacroRunFile), t("c.macro-run-ppa", "", "menu.macro", MacroRunPpa),
        t("c.terminal", "Ctrl+`", "menu.view", Terminal), t("c.md-preview", "Ctrl+Shift+V", "menu.view", MdPreview), t("c.dark", "", "menu.view", Dark), t("c.highlight", "", "menu.view", Highlight),
        t("c.wrap", "Alt+Z", "menu.view", Wrap), t("c.sidebar", "Ctrl+B", "menu.view", Sidebar), t("c.line-numbers", "", "menu.view", LineNumbers), t("c.status-bar", "", "menu.view", StatusBar),
        t("c.whitespace", "", "menu.view", Whitespace), t("c.ruler", "", "menu.view", Ruler), t("c.cur-line", "", "menu.view", CurLine), t("c.minimap", "", "menu.view", Minimap),
        t("c.minimal-mode", "", "menu.view", MinimalMode), t("c.zoom-in", "Ctrl+=", "menu.view", ZoomIn), t("c.zoom-out", "Ctrl+-", "menu.view", ZoomOut), t("c.zoom-reset", "Ctrl+0", "menu.view", ZoomReset),
        t("c.common-config", "", "menu.setting", Prefs), t("c.export-config", "", "menu.setting", ExportCfg), t("c.import-config", "", "menu.setting", ImportCfg),
        t("c.import-sakura", "", "menu.setting", ImportSakura), t("c.reset-settings", "", "menu.setting", ResetCfg),
        t("c.key-config", "", "menu.setting", KeyConfig), t("c.key-list", "Ctrl+Q", "menu.help", KeyList), t("c.palette", "Shift+F1|Ctrl+Shift+P", "menu.help", Palette),
        t("c.about", "", "menu.help", About),
        z("app.copy-fname", "复制文件名", "", "menu.edit", CopyFname), z("app.copy-path", "复制完整路径", "", "menu.edit", CopyPath),
        z("app.sel-para-prev", "选到上一段落", "", "menu.edit", SelParaPrev), z("app.sel-para-next", "选到下一段落", "", "menu.edit", SelParaNext),
        z("app.mod-next", "下一处改动", "", "menu.search", ModNext), z("app.mod-prev", "上一处改动", "", "menu.search", ModPrev),
        z("app.macro-save", "保存键盘宏…", "", "menu.macro", MacroSave), z("app.macro-load", "载入键盘宏…", "", "menu.macro", MacroLoad),
    ];
    for e in &mut v {
        match e.run {
            Run::App(App::Cut | App::Copy | App::Paste) => e.fixed = true,
            Run::App(App::DiffNext | App::DiffPrev) => e.needs_diff = true,
            _ => {}
        }
    }
    for c in cmds::CMDS {
        let cat = if c.group.is_empty() { "menu.edit" } else { "" };
        v.push(Entry { id: action_id(c.action), label: Label::Zh(c.label), cat: if cat.is_empty() { Label::Zh(c.group) } else { Label::Tr(cat) }, default: c.keys, run: Run::Act(c.action), fixed: false, needs_diff: false });
    }
    v
}

/// 全部命令 (固定顺序, 下标稳定)
pub fn entries() -> &'static [Entry] {
    static E: OnceLock<Vec<Entry>> = OnceLock::new();
    E.get_or_init(build)
}

pub fn entry_by_id(id: &str) -> Option<&'static Entry> {
    entries().iter().find(|e| e.id == id)
}

// ---------------------------------------------------------------- 键的解析 / 格式化

const SYMS: &[(&str, Key)] = &[
    ("/", Key::Slash), ("\\", Key::Backslash), ("`", Key::Backtick), ("[", Key::OpenBracket), ("]", Key::CloseBracket),
    ("=", Key::Equals), ("-", Key::Minus), (",", Key::Comma), (".", Key::Period), (";", Key::Semicolon), ("'", Key::Quote),
];

pub type Combo = (Modifiers, Key);

/// `Ctrl+Shift+K` -> 一个组合键; 认不出来 = None
pub fn parse_combo(spec: &str) -> Option<Combo> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let mut m = Modifiers::NONE;
    let mut key = None;
    // 键本身可能是 "+" 以外的符号; "Ctrl++" 不支持 (用 Ctrl+= 代替)
    for part in spec.split('+') {
        match part.trim() {
            "Ctrl" => m |= Modifiers::COMMAND,
            "Shift" => m |= Modifiers::SHIFT,
            "Alt" => m |= Modifiers::ALT,
            "" => return None,
            other => {
                if key.is_some() {
                    return None;
                }
                key = SYMS.iter().find(|(s, _)| *s == other).map(|(_, k)| *k).or_else(|| Key::from_name(other));
                key?;
            }
        }
    }
    key.map(|k| (m, k))
}

pub fn key_name(k: Key) -> String {
    SYMS.iter().find(|(_, s)| *s == k).map(|(s, _)| s.to_string()).unwrap_or_else(|| k.name().to_string())
}

pub fn format_combo(m: Modifiers, k: Key) -> String {
    let mut s = String::new();
    if m.command || m.ctrl { s.push_str("Ctrl+"); }
    if m.alt { s.push_str("Alt+"); }
    if m.shift { s.push_str("Shift+"); }
    s.push_str(&key_name(k));
    s
}

/// 一串备选 (`A|B`) -> 组合键列表, 认不出的跳过
pub fn parse_all(spec: &str) -> Vec<Combo> {
    spec.split('|').filter_map(parse_combo).collect()
}

/// 规范化一串备选: 全部认得 -> 规范写法 (Ctrl+Alt+Shift+键); 有认不出的 = None; 空串 = Some("") (解除绑定)
pub fn normalize(spec: &str) -> Option<String> {
    if spec.trim().is_empty() {
        return Some(String::new());
    }
    let mut out = vec![];
    for p in spec.split('|') {
        let (m, k) = parse_combo(p)?;
        let s = format_combo(m, k);
        if !out.contains(&s) { out.push(s); }
    }
    Some(out.join("|"))
}

/// 只是修饰键 / 锁定键本身 (按 Ctrl+Alt+K 时 egui 会先报一个 ControlLeft): 不是一个可绑定的「键」
pub fn is_modifier_key(k: Key) -> bool {
    use Key::*;
    matches!(k, ShiftLeft | ShiftRight | ControlLeft | ControlRight | AltLeft | AltRight | SuperLeft | SuperRight | CapsLock | NumLock | ScrollLock | Fn)
}

/// 用户新绑的键是否安全: 不能把普通打字键 (没有 Ctrl / Alt) 抢走; 功能键和 Insert / Delete 之类的导航键可以单独用
pub fn validate_binding(m: Modifiers, k: Key) -> Result<(), String> {
    use Key::*;
    if is_modifier_key(k) { return Err(tr("kc.err-modifier").into()); }
    let fkey = matches!(k, F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 | F13 | F14 | F15 | F16 | F17 | F18 | F19 | F20);
    if fkey || m.command || m.ctrl || m.alt {
        if matches!(k, Escape) { return Err(tr("kc.err-esc").into()); }
        return Ok(());
    }
    Err(tr("kc.err-plain").into())
}

// ---------------------------------------------------------------- 生效的键位

/// 只存和默认不同的: (命令 id, 键串); 键串为空 = 解除绑定
pub type Overrides = Vec<(String, String)>;

pub fn effective<'a>(ov: &'a [(String, String)], e: &'a Entry) -> &'a str {
    ov.iter().find(|(i, _)| *i == e.id).map(|(_, k)| k.as_str()).unwrap_or(e.default)
}

/// 读配置文件来的覆盖表: 丢掉不认识的 id / 写不对的键 / 和默认相同的项, 规范化, 去重, 数量封顶 (配置文件不可信)
pub fn clean(raw: Vec<(String, String)>) -> Overrides {
    let mut out: Overrides = vec![];
    for (id, keys) in raw {
        if out.len() >= 400 || keys.len() > 80 { continue; }
        let Some(e) = entry_by_id(&id) else { continue };
        if e.fixed { continue; }
        let Some(n) = normalize(&keys) else { continue };
        if parse_all(&n).iter().any(|(m, k)| validate_binding(*m, *k).is_err()) { continue; }
        set_binding(&mut out, &id, &n);
    }
    out.sort(); // 配置文件里是按键名排序的对象, 这里也排好, 读写前后一致
    out
}

/// 设置一条绑定 (规范化过的串); 和默认相同 = 删除覆盖项
pub fn set_binding(ov: &mut Overrides, id: &str, keys: &str) {
    let Some(e) = entry_by_id(id) else { return };
    ov.retain(|(i, _)| i != id);
    if normalize(e.default).as_deref() != Some(keys) { ov.push((id.to_string(), keys.to_string())); }
}

/// 同一个组合键现在还被谁占着 (不含自己、不含只在比较视图里生效的命令)
pub fn find_conflicts(ov: &[(String, String)], id: &str, c: Combo) -> Vec<&'static Entry> {
    let me = entry_by_id(id);
    entries().iter().filter(|e| e.id != id && !e.needs_diff && !me.is_some_and(|m| m.needs_diff) && parse_all(effective(ov, e)).contains(&c)).collect()
}

/// 给命令 `id` 绑上组合键 `c`: 校验 -> 被占用的命令把这个键让出来 (多备选的只去掉这一个) -> 返回被让出的命令。
/// 占着它的是系统键 (剪切 / 复制 / 粘贴) 时拒绝。
pub fn assign(ov: &mut Overrides, id: &str, c: Combo) -> Result<Vec<&'static Entry>, String> {
    let e = entry_by_id(id).ok_or_else(|| tr("kc.err-unknown").to_string())?;
    if e.fixed { return Err(tr("kc.err-fixed").into()); }
    validate_binding(c.0, c.1)?;
    let taken = find_conflicts(ov, id, c);
    if let Some(f) = taken.iter().find(|t| t.fixed) {
        return Err(crate::i18n::trf("kc.err-system", &[f.label.text()]));
    }
    for t in &taken {
        let rest: Vec<String> = parse_all(effective(ov, t)).into_iter().filter(|x| *x != c).map(|(m, k)| format_combo(m, k)).collect();
        set_binding(ov, &t.id, &rest.join("|"));
    }
    set_binding(ov, id, &format_combo(c.0, c.1));
    Ok(taken)
}

struct State {
    ov: Overrides,
    /// 生效的绑定: (命令下标, 修饰键, 键), 修饰键多的在前 (Ctrl+Shift+F3 要先于 F3 消费)
    bound: Vec<(usize, Modifiers, Key)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { ov: vec![], bound: bind_order(&[]) });
}

fn bind_order(ov: &[(String, String)]) -> Vec<(usize, Modifiers, Key)> {
    let mut v: Vec<(usize, Modifiers, Key)> = vec![];
    for (i, e) in entries().iter().enumerate() {
        if e.fixed { continue; }
        for (m, k) in parse_all(effective(ov, e)) { v.push((i, m, k)); }
    }
    let n = |m: &Modifiers| m.command as u8 + m.alt as u8 + m.shift as u8;
    v.sort_by_key(|(i, m, _)| (std::cmp::Reverse(n(m)), *i));
    v
}

/// 换一份覆盖表 (启动 / 改键 / 恢复默认 / 导入配置)
pub fn set_overrides(ov: &[(String, String)]) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.ov = ov.to_vec();
        s.bound = bind_order(ov);
    });
}

/// 当前要监听的绑定 (命令下标, 修饰键, 键)
pub fn bound() -> Vec<(usize, Modifiers, Key)> {
    STATE.with(|s| s.borrow().bound.clone())
}

/// 命令 `id` 当前生效的键串 (备选用 `|` 分隔; 解除绑定 = 空串)
pub fn keys_of(id: &str) -> String {
    STATE.with(|s| entry_by_id(id).map(|e| effective(&s.borrow().ov, e).to_string())).unwrap_or_default()
}

/// 菜单里显示的快捷键提示; `None` = 不是注册表里的命令 (调用方用自己的写法)
pub fn display(id: &str) -> Option<String> {
    // 宏菜单项在录制 / 停止之间换标签, 键是同一个
    let id = if id == "c.macro-stop" { "c.macro-start" } else { id };
    entry_by_id(id)?;
    Some(keys_of(id).split('|').filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" / "))
}

/// 键位一览的文本 (命令面板同款顺序): "分类\t命令\t键" 每行一条, 只列有键的
pub fn list_text() -> String {
    let mut out = String::new();
    for e in entries() {
        let k = display(&e.id).unwrap_or_default();
        if k.is_empty() { continue; }
        out.push_str(&format!("{}\t{}\t{}\n", e.cat.text(), e.label.text(), k));
    }
    out
}

// ---------------------------------------------------------------- 命令面板的模糊匹配

/// 子序列匹配 (忽略大小写): 查询串的字符按顺序都出现在 `hay` 里才算命中; 分数越高越靠前
/// (连续命中、命中在词首、命中在开头加分; 跨度越大扣分)。空查询 = 命中, 分数 0。
pub fn fuzzy_score(query: &str, hay: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let h: Vec<char> = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    let (mut qi, mut score, mut last, mut first) = (0usize, 0i32, None::<usize>, None::<usize>);
    for (i, &c) in h.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            score += 10;
            if last == Some(i.wrapping_sub(1)) && i > 0 { score += 15; }
            if i == 0 || !h[i - 1].is_alphanumeric() { score += 8; }
            first.get_or_insert(i);
            last = Some(i);
            qi += 1;
        }
    }
    if qi < q.len() {
        return None;
    }
    let span = (last.unwrap_or(0) - first.unwrap_or(0)) as i32;
    Some(score - span.min(40) - first.unwrap_or(0).min(20) as i32 / 2)
}

/// 面板里要搜的文字: 当前语言标签 + 中文标签 + id + 分类
pub fn haystack(e: &Entry) -> String {
    let zh = match e.label {
        Label::Zh(z) => z.to_string(),
        Label::Tr(_) => String::new(),
    };
    format!("{} {} {} {}", e.label.text(), zh, e.id, e.cat.text())
}

/// 过滤 + 排序: 返回命令下标 (分数高的在前, 同分保持表里的顺序); 面板自己用 cmdui::palette_items (还要混进插件命令), 这个只给测试用
#[cfg(test)]
pub fn palette_filter(query: &str) -> Vec<usize> {
    let mut hits: Vec<(i32, usize)> = entries().iter().enumerate().filter_map(|(i, e)| fuzzy_score(query, &haystack(e)).map(|s| (s, i))).collect();
    hits.sort_by_key(|&(s, i)| (std::cmp::Reverse(s), i));
    hits.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_roundtrip_for_every_key() {
        for &k in Key::ALL {
            let spec = format_combo(Modifiers::COMMAND | Modifiers::SHIFT, k);
            assert_eq!(parse_combo(&spec), Some((Modifiers::COMMAND | Modifiers::SHIFT, k)), "{spec}");
        }
    }

    #[test]
    fn default_keys_all_parse_and_are_canonical() {
        for e in entries() {
            let n = normalize(e.default);
            assert!(n.is_some(), "{} 的默认键写错了: {}", e.id, e.default);
            assert_eq!(n.as_deref(), Some(e.default), "{} 的默认键不是规范写法 (Ctrl+Alt+Shift+键)", e.id);
        }
    }

    #[test]
    fn ids_are_unique_and_labels_translated() {
        let mut seen = std::collections::HashSet::new();
        for e in entries() {
            assert!(seen.insert(e.id.clone()), "重复的命令 id: {}", e.id);
            if let Label::Tr(k) = e.label { assert_ne!(crate::i18n::tr(k), k, "缺翻译键: {k}"); }
            if let Label::Tr(k) = e.cat { assert_ne!(crate::i18n::tr(k), k, "缺分类翻译键: {k}"); }
            if let Label::Zh(z) = e.label { assert_ne!(crate::i18n::trz(z), "", "{z}"); }
        }
        assert!(entries().len() > 100 && entry_by_id("edit.cut-line").is_some(), "应当把 cmds.rs 的命令也并进来");
    }

    #[test]
    fn default_bindings_do_not_collide() {
        for e in entries() {
            for c in parse_all(e.default) {
                let others = find_conflicts(&[], &e.id, c);
                assert!(others.is_empty(), "默认键冲突: {} 和 {} 都用 {}", e.id, others[0].id, format_combo(c.0, c.1));
            }
        }
    }

    #[test]
    fn conflicts_are_detected_and_stolen() {
        let mut ov = vec![];
        let c = parse_combo("Ctrl+B").unwrap(); // 默认是 侧栏
        let taken = find_conflicts(&ov, "c.wrap", c);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].id, "c.sidebar");
        let displaced = assign(&mut ov, "c.wrap", c).unwrap();
        assert_eq!(displaced.len(), 1);
        assert_eq!(keys_with(&ov, "c.wrap"), "Ctrl+B");
        assert_eq!(keys_with(&ov, "c.sidebar"), "", "被抢的命令解除绑定");
        // 多备选: 只去掉被抢的那一个
        let c2 = parse_combo("Ctrl+I").unwrap();
        assign(&mut ov, "c.wrap", c2).unwrap();
        assert_eq!(keys_with(&ov, "c.find"), "Ctrl+F");
    }

    fn keys_with(ov: &[(String, String)], id: &str) -> String {
        effective(ov, entry_by_id(id).unwrap()).to_string()
    }

    #[test]
    fn unsafe_and_system_keys_are_refused() {
        let mut ov = vec![];
        assert!(assign(&mut ov, "c.wrap", parse_combo("A").unwrap()).is_err(), "普通字母键会吃掉打字");
        assert!(assign(&mut ov, "c.wrap", parse_combo("Shift+A").unwrap()).is_err());
        assert!(assign(&mut ov, "c.wrap", parse_combo("Escape").unwrap()).is_err());
        assert!(assign(&mut ov, "c.wrap", parse_combo("Ctrl+C").unwrap()).is_err(), "Ctrl+C 是系统键");
        assert!(assign(&mut ov, "c.copy", parse_combo("Ctrl+K").unwrap()).is_err(), "系统键本身不能改");
        assert!(assign(&mut ov, "c.wrap", parse_combo("F9").unwrap()).is_ok(), "功能键可以单独用");
        assert!(assign(&mut ov, "c.wrap", parse_combo("Ctrl+ControlLeft").unwrap()).is_err(), "修饰键本身不能当键");
        assert!(ov.iter().all(|(i, _)| i != "c.copy"));
    }

    #[test]
    fn overrides_only_keep_differences() {
        let mut ov = vec![];
        set_binding(&mut ov, "c.wrap", "F9");
        assert_eq!(ov.len(), 1);
        set_binding(&mut ov, "c.wrap", "Alt+Z"); // 回到默认
        assert!(ov.is_empty());
        set_binding(&mut ov, "c.wrap", "");
        assert_eq!(ov, vec![("c.wrap".to_string(), String::new())], "空串 = 解除绑定, 要留着");
    }

    #[test]
    fn untrusted_config_is_cleaned() {
        let raw = vec![
            ("c.wrap".to_string(), "ctrl+x".to_string()),         // 小写认不出 (规范写法是 Ctrl)
            ("c.wrap".to_string(), "Ctrl+Shift+J".to_string()),
            ("no.such".to_string(), "Ctrl+J".to_string()),         // 未知 id
            ("c.sidebar".to_string(), "A".to_string()),            // 不安全
            ("c.copy".to_string(), "Ctrl+K".to_string()),          // 系统键不可改
            ("c.find".to_string(), "x".repeat(500)),               // 超长
            ("c.print".to_string(), "Ctrl+P".to_string()),         // 与默认相同
        ];
        let out = clean(raw);
        assert_eq!(out, vec![("c.wrap".to_string(), "Ctrl+Shift+J".to_string())]);
        let many: Vec<(String, String)> = (0..2000).map(|_| ("c.wrap".to_string(), "F9".to_string())).collect();
        assert!(clean(many).len() <= 1);
    }

    #[test]
    fn dispatch_order_puts_more_modifiers_first() {
        set_overrides(&[]);
        let b = bound();
        let pos = |id: &str, spec: &str| {
            let c = parse_combo(spec).unwrap();
            b.iter().position(|(i, m, k)| entries()[*i].id == id && (*m, *k) == c).unwrap_or_else(|| panic!("{id} 没有绑定 {spec}"))
        };
        assert!(pos("app.find-word-prev", "Ctrl+Shift+F3") < pos("c.find-next", "F3"));
        assert!(pos("c.bookmark-reset", "Ctrl+Shift+F2") < pos("c.bookmark-toggle", "Ctrl+F2"));
        assert!(pos("c.bookmark-prev", "Shift+F2") < pos("c.bookmark-next", "F2"));
        assert!(b.iter().all(|(i, _, _)| !entries()[*i].fixed), "系统键不进分发表");
    }

    #[test]
    fn menu_hints_follow_the_active_bindings() {
        set_overrides(&[]);
        assert_eq!(display("c.save").as_deref(), Some("Ctrl+S"));
        assert_eq!(display("c.replace").as_deref(), Some("Ctrl+H / Ctrl+R"));
        assert_eq!(display("c.macro-stop").as_deref(), Some("Ctrl+Shift+M"));
        assert_eq!(display("not.a.command"), None);
        set_overrides(&[("c.save".into(), "Ctrl+Alt+K".into())]);
        assert_eq!(display("c.save").as_deref(), Some("Ctrl+Alt+K"));
        set_overrides(&[("c.save".into(), String::new())]);
        assert_eq!(display("c.save").as_deref(), Some(""), "解除绑定 = 没有提示");
        assert!(!list_text().contains("Ctrl+S\n"));
        set_overrides(&[]);
        assert!(list_text().lines().any(|l| l.ends_with("\tCtrl+S")));
    }

    #[test]
    fn fuzzy_matching_ranks_sensibly() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
        assert!(fuzzy_score("xyz", "Save All").is_none());
        let word_start = fuzzy_score("sa", "Save All").unwrap();
        let scattered = fuzzy_score("sa", "Close Tab Always").unwrap_or(i32::MIN);
        assert!(word_start > scattered);
        assert!(fuzzy_score("SAVE", "save as").is_some(), "忽略大小写");
        assert!(fuzzy_score("ss", "save as").is_some(), "子序列");
        let hits = palette_filter("save");
        assert!(!hits.is_empty());
        let first = &entries()[hits[0]];
        assert!(first.id.contains("save"), "最靠前的应该是保存类命令, 实际 {}", first.id);
        // 中文标签也能搜
        let h = palette_filter("转到行");
        assert!(h.iter().any(|&i| entries()[i].id == "goto-line"));
        // 空查询 = 全部命令
        assert_eq!(palette_filter("").len(), entries().len());
    }
}
