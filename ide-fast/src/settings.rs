//! 设置落盘: `%APPDATA%\\IDE1.0 Fast\\settings.json` (非 Windows: `~/.local/share/IDE1.0 Fast/settings.json`)。
//! 存用户偏好 (主题 / 字号 / 字体 / 颜色 / 缩进 / 编码 / 备份 ...) 和界面状态 (面板大小 / 窗口位置 / 语言 / 最近文件);
//! 读不到或解析失败 = 用默认值, 不报错、不挡启动。每个键独立读取 (缺的键保持默认), 所以老版本写的文件、手写的文件都能读。
//! 写入是原子的 (临时文件 + 换名); `IDE_FAST_SETTINGS=off` 关闭落盘 (测试用), `IDE_FAST_SETTINGS=<路径>` 指定文件。

use serde_json::{json, Map, Value};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    pub dark: bool,
    pub font_px: f32,
    pub tab_width: usize,
    pub highlight: bool,
    pub wrap: bool,
    /// 选中的主题插件 id (空 = 内置深 / 浅色)
    pub theme_plugin: String,
    pub recent: Vec<String>,
    /// 最近打开的文件夹
    pub recent_dirs: Vec<String>,
    /// 每个文件上次的光标 (路径, 行, 列), 最多 50 条, 新的在前
    pub positions: Vec<(String, usize, usize)>,
    /// 查找 / Grep 的默认条件
    pub search: SearchPrefs,
    /// 界面语言 zh / ja / en
    pub lang: String,
    pub auto_indent: bool,
    pub smart_indent: bool,
    /// Tab 键插入空格 (而不是制表符)
    pub insert_spaces: bool,
    /// 新文档的编码标签 (`Charset::label`) / 换行符标签 (`Eol::label`)
    pub default_encoding: String,
    pub default_eol: String,
    /// 保存时把旧文件备份到 `.sakura-backup`
    pub save_backup: bool,
    /// 窗口重新获得焦点时检查文件有没有被外部改过
    pub check_external: bool,
    /// 关窗口时有未保存修改先确认
    pub confirm_close: bool,
    /// 超过这个大小(MB)的文件打开前要确认; 硬上限另见 `doc::HARD_LIMIT`
    pub max_file_mb: u32,
    /// 最近文件保留个数
    pub mru_files: usize,
    /// 编辑器字体族 (空 = egui 默认等宽); 行高倍数
    pub font_family: String,
    pub line_height: f32,
    pub colors: Colors,
    /// 按扩展名覆盖的设置
    pub file_types: Vec<TypePref>,
    pub view: ViewState,
    /// 改过的快捷键: (命令 id, 键串), 只存和默认不同的; 键串为空 = 解除绑定 (见 keymap.rs)
    pub keys: Vec<(String, String)>,
    /// 启动时 (没有命令行文件时) 重新打开上次关闭时的标签
    pub restore_session: bool,
    /// 上次关闭时打开的文件 (最多 20 个, 只记有路径的文档) 和活动标签在其中的位置
    pub session: Vec<String>,
    pub session_active: usize,
    /// 自动备份: 每隔 `auto_backup_min` 分钟把有未保存修改的文档写到备份目录 (见 `backup.rs`)
    pub auto_backup: bool,
    pub auto_backup_min: u32,
    /// 打开文件时加「共享读、禁止写」的锁 (见 `backup.rs`)
    pub file_lock: bool,
    /// 终端外观: 字号 (像素) / 保留行数 / 前景 / 背景 (None = 跟随主题)
    pub term_font_px: f32,
    pub term_scrollback: usize,
    pub term_fg: Option<[u8; 3]>,
    pub term_bg: Option<[u8; 3]>,
    /// 半透明主题: 窗口背后用 Windows 的模糊 (DWM) 材质; 关 = 只做 alpha 叠色
    pub glass_blur: bool,
    /// 用系统标题栏 (默认 false = 自绘标题栏: 菜单 / 快捷按钮 / 窗口按钮都在里面)
    pub system_title_bar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark: true, font_px: 13.0, tab_width: 4, highlight: true, wrap: false, theme_plugin: String::new(), recent: vec![], recent_dirs: vec![], positions: vec![], search: SearchPrefs::default(),
            lang: "zh".into(), auto_indent: true, smart_indent: false, insert_spaces: false, default_encoding: "UTF-8".into(), default_eol: "CRLF".into(),
            save_backup: false, check_external: true, confirm_close: true, max_file_mb: 4, mru_files: 12, font_family: String::new(), line_height: 1.0,
            colors: Colors::default(), file_types: vec![], view: ViewState::default(), keys: vec![], restore_session: true, session: vec![], session_active: 0,
            auto_backup: false, auto_backup_min: 5, file_lock: false, term_font_px: 13.0, term_scrollback: 5000, term_fg: None, term_bg: None, glass_blur: true, system_title_bar: false,
        }
    }
}

const MAX_SESSION: usize = 20;

/// 会话文件列表里的路径是不是可以放心去打开: 必须是本地盘符的绝对路径 (UNC / 设备路径会触发网络认证, 一律不要), 长度有上限
pub fn session_path_ok(p: &str) -> bool {
    if p.is_empty() || p.len() > 1024 || p.starts_with("\\\\") || p.starts_with("//") { return false; }
    let path = std::path::Path::new(p);
    if !path.is_absolute() { return false; }
    !path.components().next().is_some_and(|c| matches!(c, std::path::Component::Prefix(x) if !matches!(x.kind(), std::path::Prefix::Disk(_))))
}

/// 颜色覆盖: None = 跟随主题
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Colors {
    pub text: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub keyword: Option<[u8; 3]>,
    pub string: Option<[u8; 3]>,
    pub comment: Option<[u8; 3]>,
    pub number: Option<[u8; 3]>,
}

impl Colors {
    pub fn slots(&mut self) -> [&mut Option<[u8; 3]>; 6] {
        [&mut self.text, &mut self.bg, &mut self.keyword, &mut self.string, &mut self.comment, &mut self.number]
    }
    const KEYS: [&'static str; 6] = ["text", "bg", "keyword", "string", "comment", "number"];

    fn to_json(&self) -> Value {
        let mut m = Map::new();
        let all = [self.text, self.bg, self.keyword, self.string, self.comment, self.number];
        for (k, c) in Self::KEYS.iter().zip(all) {
            if let Some(c) = c { m.insert((*k).into(), Value::String(hex(c))); }
        }
        Value::Object(m)
    }

    fn apply_json(&mut self, v: &Value) {
        for (k, slot) in Self::KEYS.iter().zip(self.slots()) {
            *slot = v[*k].as_str().and_then(parse_hex);
        }
    }
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// `#rrggbb` / `rrggbb`
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() { return None; }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// 某个扩展名的覆盖项 (None = 沿用全局)
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TypePref {
    pub ext: String,
    pub tab_width: Option<usize>,
    pub wrap: Option<bool>,
    pub insert_spaces: Option<bool>,
    /// 指定列数折行的列数 (Some(0) = 按窗口宽度, None = 沿用全局)
    pub wrap_col: Option<usize>,
}

/// 界面状态: 面板大小 / 开关 / 窗口位置
#[derive(Clone, PartialEq, Debug)]
pub struct ViewState {
    pub side_open: bool,
    pub side_w: f32,
    pub term_open: bool,
    pub term_h: f32,
    /// 右侧面板宽: AI 对话 / Markdown 预览各记一个
    pub ai_w: f32,
    pub md_w: f32,
    /// 折成细条 (只在对应面板显示时有意义); 右侧 = AI 对话和预览共用一条
    pub side_collapsed: bool,
    pub right_collapsed: bool,
    pub bottom_collapsed: bool,
    pub md_preview: bool,
    pub geom: Option<WinGeom>,
    /// 视图开关: 行号 / 状态栏 / 空白符标记 / 列标尺 / 当前行下划线 / 小地图
    pub line_numbers: bool,
    pub status_bar: bool,
    pub whitespace: bool,
    pub ruler: bool,
    pub cur_line: bool,
    pub minimap: bool,
    /// 工具栏 / 功能键栏 / 活动栏 (默认都关)
    pub toolbar: bool,
    pub funckey: bool,
    pub rail: bool,
    /// 折行列数 (0 = 按窗口宽度); 只在开着自动换行时有效
    pub wrap_col: usize,
    /// 固定列数折行时是否应用行头 / 行尾禁则
    pub kinsoku: bool,
    /// 句读挂在行尾 (ぶら下げ)
    pub hang: bool,
    /// 戳记: 内容 (0 日期 / 1 时间 / 2 日期+时间) 与位置 (0 行首 / 1 光标处), 见 `textconv::StampKind`
    pub stamp_kind: u8,
    pub stamp_pos: u8,
    /// 页面设置 (打印 / 预览)
    pub print: crate::print::PageSetup,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState { side_open: true, side_w: 250.0, term_open: false, term_h: 180.0, ai_w: 340.0, md_w: 420.0, side_collapsed: false, right_collapsed: false, bottom_collapsed: false, md_preview: false, geom: None, line_numbers: true, status_bar: true, whitespace: false, ruler: false, cur_line: true, minimap: false, toolbar: false, funckey: false, rail: false, wrap_col: 0, kinsoku: true, hang: false, stamp_kind: 2, stamp_pos: 0, print: Default::default() }
    }
}

/// 窗口位置 / 大小 (逻辑点); 最大化时 x y w h 是还原后的
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WinGeom {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub maximized: bool,
}

impl WinGeom {
    /// 位置 / 大小明显不对 (最小化时系统给 -32000 之类, 或尺寸太小) 就不要
    pub fn sane(&self) -> bool {
        self.w >= 300.0 && self.h >= 200.0 && self.w <= 10000.0 && self.h <= 10000.0 && self.x > -10000.0 && self.y > -10000.0 && self.x < 20000.0 && self.y < 20000.0
    }
}

/// 查找栏 / Grep 对话框的默认条件 (上次用的, 下次启动恢复)
#[derive(Clone, PartialEq, Debug)]
pub struct SearchPrefs {
    pub find_case: bool,
    pub find_regex: bool,
    pub find_word: bool,
    pub find_wrap: bool,
    pub find_marks: bool,
    pub grep_case: bool,
    pub grep_regex: bool,
    pub grep_word: bool,
    pub grep_sub: bool,
    pub grep_outside: bool,
    pub grep_files: String,
    pub grep_exc_files: String,
    pub grep_exc_folders: String,
}

impl Default for SearchPrefs {
    fn default() -> Self {
        SearchPrefs { find_case: false, find_regex: false, find_word: false, find_wrap: true, find_marks: true, grep_case: false, grep_regex: false, grep_word: false, grep_sub: true, grep_outside: false, grep_files: String::new(), grep_exc_files: String::new(), grep_exc_folders: String::new() }
    }
}

impl SearchPrefs {
    pub fn to_json(&self) -> Value {
        json!({ "find_case": self.find_case, "find_regex": self.find_regex, "find_word": self.find_word, "find_wrap": self.find_wrap, "find_marks": self.find_marks,
                "grep_case": self.grep_case, "grep_regex": self.grep_regex, "grep_word": self.grep_word, "grep_sub": self.grep_sub, "grep_outside": self.grep_outside,
                "grep_files": self.grep_files, "grep_exc_files": self.grep_exc_files, "grep_exc_folders": self.grep_exc_folders })
    }

    /// 缺的键保持默认值
    pub fn apply_json(&mut self, v: &Value) {
        let b = |k: &str, t: &mut bool| { if let Some(x) = v[k].as_bool() { *t = x; } };
        b("find_case", &mut self.find_case); b("find_regex", &mut self.find_regex); b("find_word", &mut self.find_word); b("find_wrap", &mut self.find_wrap); b("find_marks", &mut self.find_marks);
        b("grep_case", &mut self.grep_case); b("grep_regex", &mut self.grep_regex); b("grep_word", &mut self.grep_word); b("grep_sub", &mut self.grep_sub); b("grep_outside", &mut self.grep_outside);
        let s = |k: &str, t: &mut String| { if let Some(x) = v[k].as_str() { *t = x.to_string(); } };
        s("grep_files", &mut self.grep_files); s("grep_exc_files", &mut self.grep_exc_files); s("grep_exc_folders", &mut self.grep_exc_folders);
    }
}

pub fn path() -> Option<PathBuf> {
    match std::env::var("IDE_FAST_SETTINGS") {
        Ok(v) if v == "off" => None,
        Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => {
            let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
            Some(base.join("IDE1.0 Fast").join("settings.json"))
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(p) = path() else { return s };
        let Ok(text) = std::fs::read_to_string(p) else { return s };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { return s };
        s.apply_json(&v, true);
        s
    }

    /// 把 JSON 里认得的键套到自己身上 (缺的键保持原值, 数值夹到合法范围)。
    /// `state` = 连最近文件 / 光标位置 / 界面状态一起读 (启动时); 导入别人的配置文件时为 false, 只动偏好。
    pub fn apply_json(&mut self, v: &Value, state: bool) {
        let b = |k: &str, t: &mut bool| { if let Some(x) = v[k].as_bool() { *t = x; } };
        let st = |k: &str, t: &mut String| { if let Some(x) = v[k].as_str() { *t = x.to_string(); } };
        b("dark", &mut self.dark); b("highlight", &mut self.highlight); b("wrap", &mut self.wrap);
        b("auto_indent", &mut self.auto_indent); b("smart_indent", &mut self.smart_indent); b("insert_spaces", &mut self.insert_spaces);
        b("restore_session", &mut self.restore_session);
        b("save_backup", &mut self.save_backup); b("check_external", &mut self.check_external); b("confirm_close", &mut self.confirm_close);
        b("auto_backup", &mut self.auto_backup); b("file_lock", &mut self.file_lock); b("glass_blur", &mut self.glass_blur); b("system_title_bar", &mut self.system_title_bar);
        if let Some(n) = v["auto_backup_min"].as_u64() { self.auto_backup_min = crate::backup::clamp_interval(n.min(u32::MAX as u64) as u32); }
        if let Some(f) = v["term_font_px"].as_f64() { self.term_font_px = (f as f32).clamp(8.0, 32.0); }
        if let Some(n) = v["term_scrollback"].as_u64() { self.term_scrollback = (n as usize).clamp(crate::terminal::MIN_SCROLLBACK, crate::terminal::MAX_SCROLLBACK); }
        if v.get("term_fg").is_some() { self.term_fg = v["term_fg"].as_str().and_then(parse_hex); }
        if v.get("term_bg").is_some() { self.term_bg = v["term_bg"].as_str().and_then(parse_hex); }
        st("theme_plugin", &mut self.theme_plugin); st("default_encoding", &mut self.default_encoding); st("default_eol", &mut self.default_eol); st("font_family", &mut self.font_family);
        if let Some(f) = v["font_px"].as_f64() { self.font_px = (f as f32).clamp(8.0, 40.0); }
        if let Some(f) = v["line_height"].as_f64() { self.line_height = (f as f32).clamp(0.8, 2.5); }
        if let Some(t) = v["tab_width"].as_u64() { self.tab_width = (t as usize).clamp(1, 16); }
        if let Some(t) = v["max_file_mb"].as_u64() { self.max_file_mb = (t as u32).clamp(1, 32); }
        if let Some(t) = v["mru_files"].as_u64() { self.mru_files = (t as usize).clamp(1, 36); }
        if let Some(l) = v["lang"].as_str() { if crate::i18n::Lang::from_code(l).is_some() { self.lang = l.to_string(); } }
        self.colors.apply_json(&v["colors"]);
        if let Some(a) = v["file_types"].as_array() {
            self.file_types = a.iter().filter_map(|t| {
                let ext = t["ext"].as_str()?.trim().trim_start_matches('.').to_lowercase();
                if ext.is_empty() { return None; }
                Some(TypePref { ext, tab_width: t["tab_width"].as_u64().map(|n| (n as usize).clamp(1, 16)), wrap: t["wrap"].as_bool(), insert_spaces: t["insert_spaces"].as_bool(), wrap_col: t["wrap_col"].as_u64().map(|n| if n == 0 { 0 } else { (n as usize).clamp(10, crate::tools::WRAP_COL_MAX) }) })
            }).collect();
        }
        self.search.apply_json(&v["search"]);
        if let Some(o) = v["keys"].as_object() {
            self.keys = crate::keymap::clean(o.iter().filter_map(|(k, x)| Some((k.clone(), x.as_str()?.to_string()))).collect());
        }
        if !state { return; }
        if let Some(a) = v["recent"].as_array() {
            self.recent = a.iter().filter_map(|x| x.as_str().map(str::to_string)).take(36).collect();
        }
        if let Some(a) = v["session"].as_array() {
            self.session = a.iter().filter_map(|x| x.as_str()).filter(|p| session_path_ok(p)).map(str::to_string).take(MAX_SESSION).collect();
        }
        if let Some(n) = v["session_active"].as_u64() { self.session_active = (n as usize).min(MAX_SESSION); }
        if let Some(a) = v["recent_dirs"].as_array() {
            self.recent_dirs = a.iter().filter_map(|x| x.as_str().map(str::to_string)).take(12).collect();
        }
        if let Some(a) = v["positions"].as_array() {
            self.positions = a.iter().filter_map(|x| Some((x[0].as_str()?.to_string(), x[1].as_u64()? as usize, x[2].as_u64()? as usize))).take(50).collect();
        }
        let w = &v["view"];
        b_of(w, "side_open", &mut self.view.side_open); b_of(w, "term_open", &mut self.view.term_open); b_of(w, "md_preview", &mut self.view.md_preview);
        b_of(w, "line_numbers", &mut self.view.line_numbers); b_of(w, "status_bar", &mut self.view.status_bar); b_of(w, "whitespace", &mut self.view.whitespace);
        b_of(w, "ruler", &mut self.view.ruler); b_of(w, "cur_line", &mut self.view.cur_line); b_of(w, "minimap", &mut self.view.minimap);
        b_of(w, "side_collapsed", &mut self.view.side_collapsed); b_of(w, "right_collapsed", &mut self.view.right_collapsed); b_of(w, "bottom_collapsed", &mut self.view.bottom_collapsed);
        b_of(w, "toolbar", &mut self.view.toolbar); b_of(w, "funckey", &mut self.view.funckey); b_of(w, "rail", &mut self.view.rail);
        if let Some(b) = w["kinsoku"].as_bool() { self.view.kinsoku = b; }
        if let Some(b) = w["hang"].as_bool() { self.view.hang = b; }
        if let Some(n) = w["wrap_col"].as_u64() { self.view.wrap_col = if n == 0 { 0 } else { (n as usize).clamp(10, crate::tools::WRAP_COL_MAX) }; }
        let pr = &w["print"];
        if pr.is_object() {
            let mut p = self.view.print.clone();
            if let Some(c) = pr["paper"].as_str() { p.paper = crate::print::Paper::from_code(c); }
            b_of(pr, "landscape", &mut p.landscape); b_of(pr, "line_numbers", &mut p.line_numbers); b_of(pr, "header", &mut p.header); b_of(pr, "footer", &mut p.footer);
            if let Some(f) = pr["font_pt"].as_f64() { p.font_pt = f as f32; }
            if let Some(f) = pr["margin_mm"].as_f64() { p.margin_mm = f as f32; }
            self.view.print = p.clamped();
        }
        if let Some(n) = w["stamp_kind"].as_u64() { self.view.stamp_kind = n.min(2) as u8; }
        if let Some(n) = w["stamp_pos"].as_u64() { self.view.stamp_pos = n.min(1) as u8; }
        if let Some(f) = w["side_w"].as_f64() { self.view.side_w = (f as f32).clamp(120.0, 900.0); }
        if let Some(f) = w["term_h"].as_f64() { self.view.term_h = crate::panels::clamp_size(crate::panels::Dock::Bottom, f as f32); }
        if let Some(f) = w["ai_w"].as_f64() { self.view.ai_w = crate::panels::clamp_size(crate::panels::Dock::Right, f as f32); }
        if let Some(f) = w["md_w"].as_f64() { self.view.md_w = crate::panels::clamp_size(crate::panels::Dock::Right, f as f32); }
        let g = &w["geom"];
        if let (Some(x), Some(y), Some(ww), Some(h)) = (g["x"].as_f64(), g["y"].as_f64(), g["w"].as_f64(), g["h"].as_f64()) {
            let geom = WinGeom { x: x as f32, y: y as f32, w: ww as f32, h: h as f32, maximized: g["maximized"].as_bool().unwrap_or(false) };
            if geom.sane() { self.view.geom = Some(geom); }
        }
    }

    /// 偏好部分 (导出配置用: 不含最近文件 / 光标位置 / 窗口状态)
    pub fn prefs_json(&self) -> Value {
        json!({ "dark": self.dark, "font_px": self.font_px, "tab_width": self.tab_width, "highlight": self.highlight, "wrap": self.wrap, "theme_plugin": self.theme_plugin,
                "lang": self.lang, "restore_session": self.restore_session, "auto_indent": self.auto_indent, "smart_indent": self.smart_indent, "insert_spaces": self.insert_spaces,
                "default_encoding": self.default_encoding, "default_eol": self.default_eol, "save_backup": self.save_backup, "check_external": self.check_external,
                "confirm_close": self.confirm_close, "max_file_mb": self.max_file_mb, "mru_files": self.mru_files, "font_family": self.font_family, "line_height": self.line_height,
                "colors": self.colors.to_json(),
                "auto_backup": self.auto_backup, "auto_backup_min": self.auto_backup_min, "file_lock": self.file_lock, "glass_blur": self.glass_blur, "system_title_bar": self.system_title_bar,
                "term_font_px": self.term_font_px, "term_scrollback": self.term_scrollback, "term_fg": self.term_fg.map(hex), "term_bg": self.term_bg.map(hex),
                "file_types": self.file_types.iter().map(|t| json!({ "ext": t.ext, "tab_width": t.tab_width, "wrap": t.wrap, "insert_spaces": t.insert_spaces, "wrap_col": t.wrap_col })).collect::<Vec<_>>(),
                "search": self.search.to_json(),
                "keys": Value::Object(self.keys.iter().map(|(k, x)| (k.clone(), json!(x))).collect()) })
    }

    pub fn to_json(&self) -> Value {
        let mut v = self.prefs_json();
        let o = v.as_object_mut().expect("prefs_json 是对象");
        o.insert("recent".into(), json!(self.recent));
        o.insert("recent_dirs".into(), json!(self.recent_dirs));
        o.insert("session".into(), json!(self.session));
        o.insert("session_active".into(), json!(self.session_active));
        o.insert("positions".into(), json!(self.positions.iter().map(|(p, r, c)| json!([p, r, c])).collect::<Vec<_>>()));
        let w = &self.view;
        let geom = w.geom.map(|g| json!({ "x": g.x, "y": g.y, "w": g.w, "h": g.h, "maximized": g.maximized }));
        o.insert("view".into(), json!({ "side_open": w.side_open, "side_w": w.side_w, "term_open": w.term_open, "term_h": w.term_h, "ai_w": w.ai_w, "md_w": w.md_w, "side_collapsed": w.side_collapsed, "right_collapsed": w.right_collapsed, "bottom_collapsed": w.bottom_collapsed, "md_preview": w.md_preview, "geom": geom,
            "line_numbers": w.line_numbers, "status_bar": w.status_bar, "whitespace": w.whitespace, "ruler": w.ruler, "cur_line": w.cur_line, "minimap": w.minimap, "toolbar": w.toolbar, "funckey": w.funckey, "rail": w.rail,
            "wrap_col": w.wrap_col, "kinsoku": w.kinsoku, "hang": w.hang, "stamp_kind": w.stamp_kind, "stamp_pos": w.stamp_pos,
            "print": { "paper": w.print.paper.code(), "landscape": w.print.landscape, "font_pt": w.print.font_pt, "margin_mm": w.print.margin_mm, "line_numbers": w.print.line_numbers, "header": w.print.header, "footer": w.print.footer } }));
        v
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        write_atomic(&p, &self.to_json());
    }

    /// 导出偏好到文件
    pub fn export_to(&self, p: &std::path::Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.prefs_json()).map_err(|e| e.to_string())?;
        std::fs::write(p, bytes).map_err(|e| e.to_string())
    }

    /// 从文件导入偏好 (只动偏好, 不动最近文件 / 窗口状态); 不是 JSON 对象 = Err
    pub fn import_from(&mut self, p: &std::path::Path) -> Result<(), String> {
        let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if !v.is_object() { return Err("not an object".into()); }
        self.apply_json(&v, false);
        Ok(())
    }

    /// 偏好恢复默认 (最近文件 / 光标位置 / 查找条件 / 界面状态保留)
    pub fn reset_prefs(&mut self) {
        let keep = Settings { recent: std::mem::take(&mut self.recent), recent_dirs: std::mem::take(&mut self.recent_dirs), positions: std::mem::take(&mut self.positions), search: self.search.clone(), view: self.view.clone(), lang: self.lang.clone(), session: std::mem::take(&mut self.session), session_active: self.session_active, ..Settings::default() };
        *self = keep;
    }

    /// 导入 Sakura 的 sakura.ini: 只认下面这几项明确对得上的键, 其余忽略; 返回导入了几项。
    /// 键名按 Sakura 的 ini 格式: `[Common]` 里的 `nMRUArrNum_MAX` / `bExitConfirm` / `bBackUp`, `[Types(0)]` (基本设定) 里的
    /// `nTabSpace` / `bInsSpace` / `bAutoIndent` / `bSmartIndent`。
    pub fn import_sakura_ini(&mut self, text: &str) -> usize {
        // 先按节收集 (szTypeExts 可能出现在 nTabSpace 之后)
        let mut secs: Vec<(String, Vec<(String, String)>)> = vec![];
        for line in text.lines().take(200_000) {
            let line = line.trim();
            if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) { secs.push((s.to_string(), vec![])); continue; }
            let (Some((k, val)), Some(cur)) = (line.split_once('='), secs.last_mut()) else { continue };
            cur.1.push((k.trim().to_string(), val.trim().to_string()));
        }
        let mut n = 0;
        for (section, kv) in &secs {
            let get = |k: &str| kv.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
            let num = |k: &str| get(k).and_then(|v| v.parse::<i64>().ok());
            let flag = |k: &str| num(k).map(|x| x != 0);
            if section == "Common" {
                if let Some(x) = num("nMRUArrNum_MAX") { self.mru_files = (x.clamp(1, 36)) as usize; n += 1; }
                if let Some(f) = flag("bExitConfirm") { self.confirm_close = f; n += 1; }
                if let Some(f) = flag("bBackUp") { self.save_backup = f; n += 1; }
                if let Some(f) = flag("bRegularExp") { self.search.find_regex = f; n += 1; }
                if let Some(f) = flag("bLoHiCase") { self.search.find_case = f; n += 1; }
                if let Some(f) = flag("bWordOnly") { self.search.find_word = f; n += 1; }
                continue;
            }
            let Some(idx) = section.strip_prefix("Types(").and_then(|r| r.strip_suffix(')')).and_then(|r| r.parse::<usize>().ok()) else { continue };
            // 自动换行: 2 = 按窗口宽度 (Sakura 的 nTextWrapMethod), 其它 = 不换
            let wrap = num("nTextWrapMethod").map(|m| m == 1 || m == 2);
            // 1 = 按指定列数 (nMaxLineKetas), 2 = 按窗口宽度
            let wcol = match num("nTextWrapMethod") {
                Some(1) => Some(num("nMaxLineKetas").map(|k| (k.clamp(10, crate::tools::WRAP_COL_MAX as i64)) as usize).unwrap_or(80)),
                Some(2) => Some(0),
                _ => None,
            };
            let smart = flag("bSmartIndent").or_else(|| num("nSmartIndent").map(|x| x != 0));
            if idx == 0 {
                if let Some(x) = num("nTabSpace") { self.tab_width = x.clamp(1, 16) as usize; n += 1; }
                if let Some(f) = flag("bInsSpace") { self.insert_spaces = f; n += 1; }
                if let Some(f) = flag("bAutoIndent") { self.auto_indent = f; n += 1; }
                if let Some(f) = smart { self.smart_indent = f; n += 1; }
                if let Some(w) = wrap { self.wrap = w; n += 1; }
                if let Some(c) = wcol { self.view.wrap_col = c; }
            } else if let Some(exts) = get("szTypeExts") {
                // 其它文件类型: 按扩展名写进「按扩展名的覆盖项」(最多 64 种, 每种最多 16 个扩展名)
                let (tab, ins) = (num("nTabSpace").map(|x| x.clamp(1, 16) as usize), flag("bInsSpace"));
                if tab.is_none() && ins.is_none() && wrap.is_none() { continue; }
                let wcol = if wrap == Some(true) { wcol } else { None };
                for ext in exts.split([';', ',']).map(|e| e.trim().trim_start_matches('.').to_lowercase()).filter(|e| !e.is_empty() && e.len() <= 16 && !e.contains(['*', '/', '\\'])).take(16) {
                    let slot = match self.file_types.iter().position(|t| t.ext == ext) {
                        Some(i) => i,
                        None if self.file_types.len() < 64 => { self.file_types.push(TypePref { ext, ..Default::default() }); self.file_types.len() - 1 }
                        None => continue,
                    };
                    let t = &mut self.file_types[slot];
                    if tab.is_some() { t.tab_width = tab; }
                    if ins.is_some() { t.insert_spaces = ins; }
                    if wrap.is_some() { t.wrap = wrap; }
                    if wcol.is_some() { t.wrap_col = wcol; }
                }
                n += 1;
            }
        }
        n
    }

    /// 某个文件名适用的 (Tab 宽度, 自动换行, Tab 插入空格): 先看按扩展名的覆盖项, 没有就用传进来的全局值 (运行时的全局值在 App 字段里, 不一定和 self 里的一样)
    /// 某个文件名适用的折行列数: 按扩展名覆盖项优先 (仅当那一类也开着折行时有意义), 否则用传进来的全局值
    pub fn wrap_col_for(&self, name: &str, global: usize) -> usize {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
        self.file_types.iter().find(|t| !ext.is_empty() && t.ext == ext).and_then(|t| t.wrap_col).unwrap_or(global)
    }

    pub fn effective_for(&self, name: &str, tab_width: usize, wrap: bool, insert_spaces: bool) -> (usize, bool, bool) {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
        let t = self.file_types.iter().find(|t| !ext.is_empty() && t.ext == ext);
        (t.and_then(|t| t.tab_width).unwrap_or(tab_width), t.and_then(|t| t.wrap).unwrap_or(wrap), t.and_then(|t| t.insert_spaces).unwrap_or(insert_spaces))
    }

    /// 把另一份(首选项草稿)里的偏好并进来: 最近文件 / 光标位置 / 界面状态 / Grep 条件保持自己的 (草稿开着期间它们可能已经变了)
    pub fn adopt_prefs(&mut self, o: &Settings) {
        let mut search = self.search.clone();
        (search.find_case, search.find_regex, search.find_word, search.find_wrap, search.find_marks) = (o.search.find_case, o.search.find_regex, o.search.find_word, o.search.find_wrap, o.search.find_marks);
        let keep = (std::mem::take(&mut self.recent), std::mem::take(&mut self.recent_dirs), std::mem::take(&mut self.positions), self.view.clone(), std::mem::take(&mut self.keys));
        let (session, session_active) = (std::mem::take(&mut self.session), self.session_active);
        // 键位不在首选项草稿里改 (有自己的对话框, 改了立刻生效): 草稿开着期间改过的键位不能被旧草稿盖回去
        *self = Settings { recent: keep.0, recent_dirs: keep.1, positions: keep.2, view: keep.3, keys: keep.4, session, session_active, search, ..o.clone() };
        self.recent.truncate(self.mru_files.max(1));
    }

    /// 最近文件夹: 同最近文件的规则
    pub fn push_recent_dir(&mut self, p: &std::path::Path) {
        let s = p.to_string_lossy().into_owned();
        self.recent_dirs.retain(|x| x != &s);
        self.recent_dirs.insert(0, s);
        self.recent_dirs.truncate(12);
    }

    /// 记住某文件的光标位置 (已有的挪到最前)
    pub fn remember_pos(&mut self, p: &std::path::Path, row: usize, col: usize) {
        let s = p.to_string_lossy().into_owned();
        self.positions.retain(|x| x.0 != s);
        self.positions.insert(0, (s, row, col));
        self.positions.truncate(50);
    }

    /// 最近文件: 新的放最前, 去重, 最多 `mru_files` 个
    pub fn push_recent(&mut self, p: &std::path::Path) {
        let s = p.to_string_lossy().into_owned();
        self.recent.retain(|x| x != &s);
        self.recent.insert(0, s);
        self.recent.truncate(self.mru_files.max(1));
    }
}

fn b_of(v: &Value, k: &str, t: &mut bool) {
    if let Some(x) = v[k].as_bool() { *t = x; }
}

fn write_atomic(p: &std::path::Path, v: &Value) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = crate::fsx::atomic_write(p, &serde_json::to_vec_pretty(v).unwrap_or_default(), false);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 这些测试都改环境变量 IDE_FAST_SETTINGS, 要串行
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn roundtrip_and_recent_order() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide-fast-set-{}", std::process::id()));
        let f = dir.join("s.json");
        std::env::set_var("IDE_FAST_SETTINGS", &f);
        let mut s = Settings::default();
        s.dark = false;
        s.font_px = 19.0;
        s.push_recent(std::path::Path::new("a.txt"));
        s.push_recent(std::path::Path::new("b.txt"));
        s.push_recent(std::path::Path::new("a.txt"));
        s.push_recent_dir(std::path::Path::new("d1"));
        s.push_recent_dir(std::path::Path::new("d2"));
        s.push_recent_dir(std::path::Path::new("d1"));
        s.remember_pos(std::path::Path::new("a.txt"), 7, 3);
        s.remember_pos(std::path::Path::new("b.txt"), 1, 0);
        s.remember_pos(std::path::Path::new("a.txt"), 9, 2);
        s.search.find_regex = true;
        s.search.find_wrap = false;
        s.search.grep_files = "*.rs".into();
        s.search.grep_exc_folders = "target".into();
        s.save();
        let l = Settings::load();
        assert_eq!(l.recent_dirs, vec!["d1".to_string(), "d2".to_string()]);
        assert_eq!(l.positions[0], ("a.txt".to_string(), 9, 2), "再记一次的挪到最前");
        assert_eq!(l.positions.len(), 2, "同一文件只留一条");
        assert_eq!(l.search, s.search, "查找 / Grep 默认条件落盘后原样读回");
        assert_eq!((l.dark, l.font_px, l.recent.clone()), (false, 19.0, vec!["a.txt".to_string(), "b.txt".to_string()]));
        std::fs::write(&f, "{ broken").unwrap();
        assert_eq!(Settings::load(), Settings::default(), "损坏的设置文件 = 默认值");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("IDE_FAST_SETTINGS");
    }

    #[test]
    fn every_new_key_roundtrips() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide-fast-set2-{}", std::process::id()));
        let f = dir.join("s.json");
        std::env::set_var("IDE_FAST_SETTINGS", &f);
        let mut s = Settings::default();
        s.system_title_bar = true; s.lang = "ja".into(); s.auto_indent = false; s.smart_indent = true; s.insert_spaces = true;
        s.default_encoding = "UTF-16LE".into(); s.default_eol = "LF".into(); s.save_backup = true; s.check_external = false; s.confirm_close = false;
        s.max_file_mb = 16; s.mru_files = 20; s.font_family = "Consolas".into(); s.line_height = 1.4;
        s.colors.text = Some([1, 2, 3]); s.colors.keyword = Some([255, 0, 128]);
        s.file_types = vec![TypePref { ext: "md".into(), tab_width: Some(2), wrap: Some(true), insert_spaces: None, wrap_col: Some(66) }];
        s.view = ViewState { side_open: false, side_w: 300.0, term_open: true, term_h: 222.0, ai_w: 360.0, md_w: 480.0, side_collapsed: true, right_collapsed: true, bottom_collapsed: true, md_preview: true, geom: Some(WinGeom { x: 10.0, y: 20.0, w: 900.0, h: 600.0, maximized: true }), line_numbers: false, status_bar: false, whitespace: true, ruler: true, cur_line: false, minimap: true, toolbar: true, funckey: true, rail: true, wrap_col: 72, kinsoku: false, hang: true, stamp_kind: 1, stamp_pos: 1, print: crate::print::PageSetup { paper: crate::print::Paper::Letter, landscape: true, font_pt: 12.0, margin_mm: 20.0, line_numbers: false, header: false, footer: false } };
        s.save();
        assert_eq!(Settings::load(), s, "新增的键全部落盘后原样读回");
        // 超范围的值被夹回合法范围; 不合理的窗口位置丢掉
        std::fs::write(&f, r#"{"max_file_mb":999,"mru_files":0,"line_height":9,"lang":"xx","view":{"geom":{"x":-32000,"y":-32000,"w":160,"h":28}}}"#).unwrap();
        let l = Settings::load();
        assert_eq!((l.max_file_mb, l.mru_files, l.line_height, l.lang.as_str(), l.view.geom), (32, 1, 2.5, "zh", None));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("IDE_FAST_SETTINGS");
    }

    #[test]
    fn mru_limit_follows_setting() {
        let mut s = Settings::default();
        s.mru_files = 3;
        for n in 0..6 { s.push_recent(std::path::Path::new(&format!("f{n}.txt"))); }
        assert_eq!(s.recent, vec!["f5.txt", "f4.txt", "f3.txt"]);
    }

    #[test]
    fn export_import_and_reset() {
        let dir = std::env::temp_dir().join(format!("ide-fast-set3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("cfg.json");
        let mut a = Settings::default();
        a.tab_width = 8; a.font_family = "Consolas".into(); a.recent = vec!["x.txt".into()]; a.colors.bg = Some([9, 9, 9]);
        a.export_to(&f).unwrap();
        let mut b = Settings::default();
        b.recent = vec!["keep.txt".into()];
        b.import_from(&f).unwrap();
        assert_eq!((b.tab_width, b.font_family.as_str(), b.colors.bg), (8, "Consolas", Some([9, 9, 9])));
        assert_eq!(b.recent, vec!["keep.txt".to_string()], "导入只动偏好, 不动最近文件");
        std::fs::write(&f, "[1,2]").unwrap();
        assert!(b.import_from(&f).is_err(), "不是对象 = 拒绝");
        std::fs::write(&f, "nope").unwrap();
        assert!(b.import_from(&f).is_err());
        b.view.side_w = 400.0;
        b.reset_prefs();
        assert_eq!((b.tab_width, b.font_family.as_str(), b.colors.bg), (4, "", None));
        assert_eq!((b.recent.clone(), b.view.side_w), (vec!["keep.txt".to_string()], 400.0), "恢复默认保留最近文件和界面状态");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sakura_ini_import_maps_known_keys_only() {
        let ini = "[Common]\nnMRUArrNum_MAX=20\nbExitConfirm=0\nbBackUp=1\nfoo=3\n[Types(0)]\nnTabSpace=8\nbInsSpace=1\nbAutoIndent=0\nnSmartIndent=1\n[Types(1)]\nnTabSpace=2\n";
        let mut s = Settings::default();
        let n = s.import_sakura_ini(ini);
        assert_eq!(n, 7);
        assert_eq!((s.mru_files, s.confirm_close, s.save_backup, s.tab_width, s.insert_spaces, s.auto_indent, s.smart_indent), (20, false, true, 8, true, false, true));
        assert_eq!(Settings::default().clone().import_sakura_ini("garbage"), 0);
    }

    #[test]
    fn sakura_ini_import_covers_search_flags_wrap_and_per_type_overrides() {
        let ini = "[Common]\nbRegularExp=1\nbLoHiCase=1\nbWordOnly=1\n[Types(0)]\nnTextWrapMethod=2\n[Types(2)]\nnTabSpace=2\nbInsSpace=1\nszTypeExts=c;.CPP;*.bad;h\nnTextWrapMethod=0\n[Types(3)]\nnTabSpace=9\n";
        let mut s = Settings::default();
        let n = s.import_sakura_ini(ini);
        assert_eq!(n, 5, "3 个查找开关 + 全局换行 + 一个带扩展名的类型");
        assert_eq!((s.search.find_regex, s.search.find_case, s.search.find_word, s.wrap), (true, true, true, true));
        let exts: Vec<_> = s.file_types.iter().map(|t| t.ext.as_str()).collect();
        assert_eq!(exts, vec!["c", "cpp", "h"], "含通配符的扩展名被丢弃, 大小写归一");
        assert!(s.file_types.iter().all(|t| t.tab_width == Some(2) && t.insert_spaces == Some(true) && t.wrap == Some(false)));
        assert_eq!(s.tab_width, 4, "没有 szTypeExts 的类型不动全局值");
    }

    #[test]
    fn sakura_ini_import_reads_fixed_wrap_columns_per_type() {
        let ini = "[Types(0)]
nTextWrapMethod=1
nMaxLineKetas=100
[Types(4)]
szTypeExts=txt
nTextWrapMethod=1
nMaxLineKetas=40
[Types(5)]
szTypeExts=md
nTextWrapMethod=2
[Types(6)]
szTypeExts=log
nTextWrapMethod=0
nMaxLineKetas=30
nTabSpace=2
";
        let mut s = Settings::default();
        s.import_sakura_ini(ini);
        assert_eq!((s.wrap, s.view.wrap_col), (true, 100));
        assert_eq!(s.wrap_col_for("a.TXT", 100), 40);
        assert_eq!(s.wrap_col_for("a.md", 100), 0, "按窗口宽度 = 0");
        assert_eq!(s.wrap_col_for("a.log", 100), 100, "不折行的类型不带列数");
        assert_eq!(s.wrap_col_for("a.rs", 100), 100);
        assert_eq!(s.wrap_col_for("Makefile", 77), 77);
    }

    #[test]
    fn per_extension_overrides_win_over_global() {
        let mut s = Settings::default();
        s.tab_width = 4; s.wrap = false;
        s.file_types = vec![TypePref { ext: "md".into(), tab_width: Some(2), wrap: Some(true), insert_spaces: Some(true), wrap_col: None }, TypePref { ext: "go".into(), tab_width: Some(8), ..Default::default() }];
        assert_eq!(s.effective_for("Notes.MD", 4, false, false), (2, true, true), "扩展名不分大小写");
        assert_eq!(s.effective_for("main.go", 4, false, false), (8, false, false), "没覆盖的项沿用全局");
        assert_eq!(s.effective_for("a.rs", 6, true, true), (6, true, true), "全局值用传进来的");
        assert_eq!(s.effective_for("Makefile", 4, false, false), (4, false, false), "没有扩展名 = 全局");
    }

    #[test]
    fn adopt_prefs_keeps_live_lists_and_grep_state() {
        let mut live = Settings::default();
        live.recent = vec!["a".into(), "b".into(), "c".into()];
        live.search.grep_files = "*.rs".into();
        live.view.side_w = 321.0;
        let mut draft = Settings::default();
        draft.recent = vec!["stale".into()];
        draft.tab_width = 2; draft.mru_files = 2;
        draft.search.find_case = true; draft.search.grep_files = "old".into();
        live.adopt_prefs(&draft);
        assert_eq!(live.tab_width, 2);
        assert_eq!(live.recent, vec!["a", "b"], "最近文件用现在的, 并按新的个数截断");
        assert!(live.search.find_case, "查找默认条件取草稿的");
        assert_eq!(live.search.grep_files, "*.rs", "Grep 条件不被草稿覆盖");
        assert_eq!(live.view.side_w, 321.0);
    }

    #[test]
    fn hex_colors_parse() {
        assert_eq!(parse_hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("00ff00"), Some([0, 255, 0]));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(hex([1, 2, 255]), "#0102ff");
    }

    #[test]
    fn custom_keys_roundtrip_and_hostile_values_are_dropped() {
        let mut s = Settings::default();
        s.keys = vec![("c.save".into(), String::new()), ("c.wrap".into(), "Ctrl+Shift+J".into())];
        let back = {
            let mut t = Settings::default();
            t.apply_json(&s.to_json(), true);
            t
        };
        assert_eq!(back.keys, s.keys);
        // 导出的偏好里也带键位
        assert!(s.prefs_json()["keys"]["c.wrap"].is_string());
        let mut t = Settings::default();
        t.apply_json(&json!({ "keys": { "c.wrap": "A", "nope": "Ctrl+J", "c.copy": "Ctrl+K", "c.sidebar": 5, "c.find": "Ctrl+Shift+J" } }), false);
        assert_eq!(t.keys, vec![("c.find".to_string(), "Ctrl+Shift+J".to_string())]);
        // 草稿不能盖掉草稿开着期间改的键位; 恢复默认会清掉键位
        let mut live = Settings::default();
        live.keys = vec![("c.wrap".into(), "F9".into())];
        live.adopt_prefs(&Settings::default());
        assert_eq!(live.keys, vec![("c.wrap".to_string(), "F9".to_string())]);
        live.reset_prefs();
        assert!(live.keys.is_empty());
    }

    #[test]
    fn session_roundtrips_and_refuses_unsafe_paths() {
        let mut s = Settings::default();
        s.session = vec!["C:\\work\\a.txt".into(), "D:\\b.md".into()];
        s.session_active = 1;
        let mut back = Settings::default();
        back.apply_json(&s.to_json(), true);
        assert_eq!(back.session, s.session);
        assert_eq!(back.session_active, 1);
        assert!(back.restore_session, "默认开启");
        assert!(!session_path_ok("\\\\evil\\share\\x.txt"), "UNC");
        assert!(!session_path_ok("//evil/share/x.txt"));
        assert!(!session_path_ok("relative\\x.txt"));
        assert!(!session_path_ok(""));
        assert!(!session_path_ok(&format!("C:\\{}", "a".repeat(2000))));
        let mut t = Settings::default();
        let many: Vec<String> = (0..100).map(|i| format!("C:\\f{i}.txt")).collect();
        t.apply_json(&json!({ "session": many, "session_active": 9999, "restore_session": false }), true);
        assert_eq!(t.session.len(), 20, "封顶");
        assert_eq!(t.session_active, 20);
        assert!(!t.restore_session);
        // 导入别人的配置 (state = false) 不带会话
        let mut u = Settings::default();
        u.apply_json(&json!({ "session": ["C:\\x.txt"] }), false);
        assert!(u.session.is_empty());
        // 恢复默认 / 草稿合并不丢会话
        t.reset_prefs();
        assert_eq!(t.session.len(), 20);
        t.adopt_prefs(&Settings::default());
        assert_eq!(t.session.len(), 20);
    }
}
