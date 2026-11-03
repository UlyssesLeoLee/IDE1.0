//! 插件 (声明式部分): 复用 editcore 的插件仓库 (`plugin_host::store`) —— 和 Tauri 版同一个目录、同一套 zip 导入 / 删除 / 恢复内置。
//! 声明式部分在这里: 主题插件 (`contributes.theme` 的颜色变量 -> egui Visuals) 和命令贡献的清单 (`contributes.command`)。
//! 带脚本入口的插件 (PPA / 文档关系图 ...) 的命令由 `plugin_host` (QuickJS) 惰性运行; Pele (mermaid) 需要 DOM, 高速版里不支持。

use eframe::egui::{self, Color32};
use editcore::plugin_host::store::{self, Installed};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn root() -> PathBuf {
    store::default_root()
}

#[derive(Clone)]
pub struct ThemeSpec {
    pub label: String,
    pub vars: BTreeMap<String, String>,
}

pub struct Row {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub theme: Option<ThemeSpec>,
    /// 有脚本入口且没有声明式贡献 -> 脚本插件 (命令走 plugin_host 惰性运行)
    pub script_only: bool,
    /// `contributes.command`: (命令 id, 标题)
    pub commands: Vec<(String, String)>,
    /// manifest 声明的权限 (editor.read / editor.write ...): 决定交给脚本的文档参数和能不能写回
    pub perms: Vec<String>,
}

impl Row {
    /// 脚本插件里高速版跑不了的: 没有命令贡献的脚本插件 (例如 Pele 的 markdown.fence, 需要 DOM)
    pub fn pele_like(&self) -> bool {
        self.script_only && self.commands.is_empty()
    }
}

pub fn rows() -> Vec<Row> {
    store::list(&root()).into_iter().map(row_of).collect()
}

fn row_of(p: Installed) -> Row {
    let m = &p.manifest;
    let theme = m.contributes.get("theme").and_then(|t| {
        let vars: BTreeMap<String, String> = t["vars"].as_object()?.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
        Some(ThemeSpec { label: t["label"].as_str().or(t["name"].as_str()).unwrap_or(&m.name).to_string(), vars })
    });
    let script_only = theme.is_none() && !m.entry.is_empty();
    let commands = m.contributes.get("command").and_then(|c| c.as_array()).map(|a| {
        a.iter().filter_map(|c| {
            let id = c["id"].as_str()?.to_string();
            let title = c["title"].as_str().unwrap_or(&id).to_string();
            Some((id, title))
        }).collect()
    }).unwrap_or_default();
    Row { id: m.id.clone(), name: m.name.clone(), version: m.version.clone(), enabled: p.enabled, theme, script_only, commands, perms: m.permissions.clone() }
}

/// `#rgb` `#rrggbb` `#rrggbbaa` `rgb(r,g,b)` `rgba(r,g,b,a)`
pub fn parse_color(s: &str) -> Option<Color32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let n = |i: usize, w: usize| u8::from_str_radix(h.get(i..i + w)?, 16).ok();
        return match h.len() {
            3 => Some(Color32::from_rgb(n(0, 1)? * 17, n(1, 1)? * 17, n(2, 1)? * 17)),
            6 => Some(Color32::from_rgb(n(0, 2)?, n(2, 2)?, n(4, 2)?)),
            8 => Some(Color32::from_rgba_unmultiplied(n(0, 2)?, n(2, 2)?, n(4, 2)?, n(6, 2)?)),
            _ => None,
        };
    }
    let inner = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?.strip_suffix(')')?;
    let p: Vec<&str> = inner.split(',').map(str::trim).collect();
    if p.len() < 3 {
        return None;
    }
    let c = |i: usize| p[i].parse::<f32>().ok().map(|v| v.clamp(0.0, 255.0) as u8);
    let a = p.get(3).and_then(|a| a.parse::<f32>().ok()).map(|a| (a.clamp(0.0, 1.0) * 255.0) as u8).unwrap_or(255);
    Some(Color32::from_rgba_unmultiplied(c(0)?, c(1)?, c(2)?, a))
}

fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

/// 主题变量 -> egui 外观。取不到的变量就沿用深 / 浅基底 (背景亮度决定基底)
pub fn visuals(vars: &BTreeMap<String, String>) -> egui::Visuals {
    let get = |k: &str| vars.get(k).and_then(|v| parse_color(v));
    let edit = get("--bg-edit").unwrap_or(Color32::from_gray(30));
    let mut v = if luminance(edit) < 0.5 { egui::Visuals::dark() } else { egui::Visuals::light() };
    // 半透明颜色在原生窗口里没有底图可透, 先铺到 --bg-edit 上
    let solid = |c: Color32| -> Color32 {
        if c.a() == 255 {
            return c;
        }
        let a = c.a() as f32 / 255.0;
        let mix = |f: u8, b: u8| (f as f32 + b as f32 * (1.0 - a)).min(255.0) as u8; // Color32 是预乘 alpha
        Color32::from_rgb(mix(c.r(), edit.r()), mix(c.g(), edit.g()), mix(c.b(), edit.b()))
    };
    if let Some(c) = get("--bg-side") {
        v.panel_fill = solid(c);
        v.window_fill = solid(c);
    }
    v.extreme_bg_color = edit;
    if let Some(c) = get("--bg-raised") {
        v.faint_bg_color = solid(c);
    }
    if let Some(c) = get("--fg") {
        v.override_text_color = Some(c);
    }
    if let Some(c) = get("--fg-muted") {
        v.widgets.noninteractive.fg_stroke.color = c;
    }
    if let Some(c) = get("--accent-text").or_else(|| get("--blue")) {
        v.hyperlink_color = c;
        v.selection.bg_fill = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 90);
    }
    if let Some(c) = get("--bg-bar") {
        v.widgets.inactive.weak_bg_fill = solid(c);
        v.widgets.noninteractive.bg_fill = solid(c);
    }
    if let Some(c) = get("--bg-input") {
        v.widgets.inactive.bg_fill = solid(c);
    }
    if let Some(c) = get("--border") {
        v.widgets.noninteractive.bg_stroke.color = solid(c);
    }
    if let Some(c) = get("--bg-hover") {
        v.widgets.hovered.weak_bg_fill = solid(c);
        v.widgets.hovered.bg_fill = solid(c);
    }
    if let Some(c) = get("--bg-active") {
        v.widgets.active.weak_bg_fill = solid(c);
        v.widgets.active.bg_fill = solid(c);
    }
    v
}

/// 读 zip 导入前给用户看的确认文字; Err = 包本身不合格
pub fn inspect_zip(bytes: &[u8]) -> Result<(String, Vec<String>), String> {
    let ins = store::inspect_package(&root(), bytes, env!("CARGO_PKG_VERSION"))?;
    if let Some(c) = ins.conflict {
        return Err(c);
    }
    if !ins.report.ok {
        let why: Vec<String> = ins.report.problems.iter().map(|p| format!("{}: {}", p.code, p.message)).collect();
        return Err(format!("插件与当前版本不匹配: {}", why.join("; ")));
    }
    let m = ins.report.manifest.ok_or("插件包里没有 manifest.json")?;
    let note = format!(
        "{} {} (id {}){}\n权限: {}\n文件: {} 个",
        m.name,
        m.version,
        m.id,
        ins.installed_version.map(|v| format!(", 已装 {v}")).unwrap_or_default(),
        if m.permissions.is_empty() { "无".to_string() } else { m.permissions.join(", ") },
        ins.files.len()
    );
    Ok((note, m.permissions.clone()))
}

pub fn install_zip(bytes: &[u8], approved: &[String]) -> Result<String, String> {
    store::install_package(&root(), bytes, env!("CARGO_PKG_VERSION"), approved).map(|i| format!("已安装 {} {}", i.manifest.name, i.manifest.version))
}

/// 启用 / 禁用 (写回插件仓库; 调用方负责让宿主丢掉它的运行环境)
pub fn set_enabled(id: &str, on: bool) -> Result<(), String> {
    store::set_enabled(&root(), id, on)
}

pub fn remove(id: &str) -> Result<(), String> {
    store::remove(&root(), id)
}

pub fn removed_bundled() -> Vec<(String, String, String)> {
    store::removed_bundled(&root())
}

pub fn restore(id: &str) -> Result<(), String> {
    store::restore_bundled(&root(), id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_in_the_forms_theme_plugins_use() {
        assert_eq!(parse_color("#1e1e1e"), Some(Color32::from_rgb(30, 30, 30)));
        assert_eq!(parse_color("#fff"), Some(Color32::from_rgb(255, 255, 255)));
        assert_eq!(parse_color("rgb(1, 2, 3)"), Some(Color32::from_rgb(1, 2, 3)));
        assert_eq!(parse_color("rgba(255,255,255,0.5)").map(|c| c.a()), Some(127));
        assert_eq!(parse_color("var(--x)"), None);
    }

    #[test]
    fn bundled_theme_plugins_load_and_map_to_visuals() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide-fast-plug-{}", std::process::id()));
        std::env::set_var("IDE_PLUGIN_DIR", &dir);
        let rows = rows();
        let dark = rows.iter().find(|r| r.id == "theme-dark").expect("内置深色主题插件");
        let spec = dark.theme.clone().expect("有 theme 贡献");
        let v = visuals(&spec.vars);
        assert_eq!(v.extreme_bg_color, Color32::from_rgb(30, 30, 30));
        assert!(v.dark_mode);
        let light = rows.iter().find(|r| r.id == "theme-light").and_then(|r| r.theme.clone()).expect("浅色");
        assert!(!visuals(&light.vars).dark_mode);
        assert!(rows.iter().any(|r| r.script_only), "脚本类插件被标出来");
        let ppa = rows.iter().find(|r| r.id == "ppa").expect("内置 PPA");
        assert_eq!(ppa.commands.first().map(|c| c.0.as_str()), Some("run"));
        assert!(ppa.perms.iter().any(|p| p == "editor.write"));
        std::env::remove_var("IDE_PLUGIN_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
