//! 设置落盘: `%APPDATA%\\IDE1.0 Fast\\settings.json` (非 Windows: `~/.local/share/IDE1.0 Fast/settings.json`)。
//! 只存几项用户偏好 (主题 / 字号 / 最近文件 / Tab 宽度 / 语法高亮开关); 读不到或解析失败 = 用默认值, 不报错、不挡启动。
//! 写入是原子的 (临时文件 + 换名); `IDE_FAST_SETTINGS=off` 关闭落盘 (测试用), `IDE_FAST_SETTINGS=<路径>` 指定文件。

use serde_json::{json, Value};
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
    /// 查找 / Grep 的默认条件
    pub search: SearchPrefs,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { dark: true, font_px: 15.0, tab_width: 4, highlight: true, wrap: false, theme_plugin: String::new(), recent: vec![], search: SearchPrefs::default() }
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
        if let Some(b) = v["dark"].as_bool() { s.dark = b; }
        if let Some(f) = v["font_px"].as_f64() { s.font_px = (f as f32).clamp(8.0, 40.0); }
        if let Some(t) = v["tab_width"].as_u64() { s.tab_width = (t as usize).clamp(1, 16); }
        if let Some(h) = v["highlight"].as_bool() { s.highlight = h; }
        if let Some(w) = v["wrap"].as_bool() { s.wrap = w; }
        if let Some(t) = v["theme_plugin"].as_str() { s.theme_plugin = t.to_string(); }
        s.search.apply_json(&v["search"]);
        if let Some(a) = v["recent"].as_array() {
            s.recent = a.iter().filter_map(|x| x.as_str().map(str::to_string)).take(12).collect();
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        let v = json!({ "dark": self.dark, "font_px": self.font_px, "tab_width": self.tab_width, "highlight": self.highlight, "wrap": self.wrap, "theme_plugin": self.theme_plugin, "recent": self.recent, "search": self.search.to_json() });
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    /// 最近文件: 新的放最前, 去重, 最多 12 个
    pub fn push_recent(&mut self, p: &std::path::Path) {
        let s = p.to_string_lossy().into_owned();
        self.recent.retain(|x| x != &s);
        self.recent.insert(0, s);
        self.recent.truncate(12);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_recent_order() {
        let dir = std::env::temp_dir().join(format!("ide-fast-set-{}", std::process::id()));
        let f = dir.join("s.json");
        std::env::set_var("IDE_FAST_SETTINGS", &f);
        let mut s = Settings::default();
        s.dark = false;
        s.font_px = 19.0;
        s.push_recent(std::path::Path::new("a.txt"));
        s.push_recent(std::path::Path::new("b.txt"));
        s.push_recent(std::path::Path::new("a.txt"));
        s.search.find_regex = true;
        s.search.find_wrap = false;
        s.search.grep_files = "*.rs".into();
        s.search.grep_exc_folders = "target".into();
        s.save();
        let l = Settings::load();
        assert_eq!(l.search, s.search, "查找 / Grep 默认条件落盘后原样读回");
        assert_eq!((l.dark, l.font_px, l.recent.clone()), (false, 19.0, vec!["a.txt".to_string(), "b.txt".to_string()]));
        std::fs::write(&f, "{ broken").unwrap();
        assert_eq!(Settings::load(), Settings::default(), "损坏的设置文件 = 默认值");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::remove_var("IDE_FAST_SETTINGS");
    }
}
