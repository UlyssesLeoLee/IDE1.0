//! 设置落盘 (WBS-02 3.1): 偏好、键位、类型别设置、最近文件等原来只存在 WebView 的 localStorage 里 (不可携带、清缓存就没), 现在镜像到一个 JSON 文件。
//!
//! - 文件是 `{ "ide10.xxx": "<前端存的字符串>", … }`: 前端怎么存、这里就原样存, 后端不解释内容, 所以前端加新设置不用改后端。
//! - 只收 `ide10.` 开头的键; 单个值和整个文件有大小上限; 写入是原子的 (临时文件 + 换名)。
//! - API Key 等密钥、各机器自己的同意记录由前端决定不上盘 (见前端的排除表), 这里不替它判断。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const KEY_PREFIX: &str = "ide10.";
pub const MAX_VALUE_BYTES: usize = 256 * 1024;
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;

pub type Settings = BTreeMap<String, String>;

/// 设置文件位置: `IDE_SETTINGS_FILE` 优先 (便携版 / 测试), 否则 `%APPDATA%/IDE1.0/settings.json` (非 Windows: `~/.local/share/IDE1.0/settings.json`)
pub fn default_path() -> PathBuf {
    if let Ok(p) = std::env::var("IDE_SETTINGS_FILE") {
        return PathBuf::from(p);
    }
    let base = std::env::var("APPDATA").map(PathBuf::from).or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share"))).unwrap_or_else(|_| PathBuf::from("."));
    base.join("IDE1.0").join("settings.json")
}

/// 读; 文件不存在 / 损坏 = 空 (损坏的文件不会被默默覆盖: 下一次写之前先备份成 `.bad`, 见 [`write_all`])
pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn valid_key(k: &str) -> bool {
    k.starts_with(KEY_PREFIX) && k.len() <= 128 && k.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}

fn write_all(path: &Path, all: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
    if text.len() > MAX_FILE_BYTES {
        return Err(format!("设置文件会超过 {} 字节上限", MAX_FILE_BYTES));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("无法创建设置目录: {e}"))?;
    }
    // 原来的文件如果读得出但内容不是合法设置 (被人改坏), 先留一份 .bad 再覆盖, 不默默丢用户的东西
    if path.exists() && std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Settings>(&s).ok()).is_none() {
        let _ = std::fs::copy(path, path.with_extension("json.bad"));
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// 设置 / 删除一个键 (`value = None` = 删除)
pub fn set(path: &Path, key: &str, value: Option<&str>) -> Result<(), String> {
    if !valid_key(key) {
        return Err(format!("不合法的设置键: {key}"));
    }
    let mut all = load(path);
    match value {
        Some(v) if v.len() > MAX_VALUE_BYTES => return Err(format!("设置 {key} 的值太大 ({} > {} 字节)", v.len(), MAX_VALUE_BYTES)),
        Some(v) => {
            all.insert(key.to_string(), v.to_string());
        }
        None => {
            all.remove(key);
        }
    }
    write_all(path, &all)
}

/// 导入一份导出的 JSON: 合并进现有设置 (同键以导入的为准); 不合法的键跳过并报告个数
pub fn import(path: &Path, json: &str) -> Result<(usize, usize), String> {
    let incoming: Settings = serde_json::from_str(json).map_err(|e| format!("不是有效的设置 JSON: {e}"))?;
    let mut all = load(path);
    let (mut ok, mut skipped) = (0, 0);
    for (k, v) in incoming {
        if valid_key(&k) && v.len() <= MAX_VALUE_BYTES {
            all.insert(k, v);
            ok += 1;
        } else {
            skipped += 1;
        }
    }
    write_all(path, &all)?;
    Ok((ok, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("settings.json")
    }

    #[test]
    fn set_load_remove_roundtrip() {
        let p = tmp("rt");
        assert!(load(&p).is_empty(), "没有文件 = 空");
        set(&p, "ide10.prefs", Some("{\"tabWidth\":2}")).unwrap();
        set(&p, "ide10.view", Some("{}")).unwrap();
        assert_eq!(load(&p)["ide10.prefs"], "{\"tabWidth\":2}");
        set(&p, "ide10.view", None).unwrap();
        assert!(!load(&p).contains_key("ide10.view"));
        assert!(!p.with_extension("json.tmp").exists(), "没有残留临时文件");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn rejects_foreign_keys_and_oversize_values() {
        let p = tmp("bad");
        assert!(set(&p, "other.key", Some("x")).is_err(), "只收 ide10. 开头的键");
        assert!(set(&p, "ide10.bad key", Some("x")).is_err(), "键里不能有空格");
        assert!(set(&p, "ide10.big", Some(&"x".repeat(MAX_VALUE_BYTES + 1))).is_err());
        assert!(!p.exists(), "被拒绝的写不留文件");
    }

    #[test]
    fn corrupt_file_is_kept_as_bad_before_being_replaced() {
        let p = tmp("corrupt");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{ not json").unwrap();
        assert!(load(&p).is_empty());
        set(&p, "ide10.a", Some("1")).unwrap();
        assert_eq!(std::fs::read_to_string(p.with_extension("json.bad")).unwrap(), "{ not json", "坏文件留了一份");
        assert_eq!(load(&p)["ide10.a"], "1");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn import_merges_and_reports_skipped() {
        let p = tmp("imp");
        set(&p, "ide10.a", Some("old")).unwrap();
        let (ok, skipped) = import(&p, r#"{"ide10.a":"new","ide10.b":"2","evil":"x"}"#).unwrap();
        assert_eq!((ok, skipped), (2, 1));
        let all = load(&p);
        assert_eq!((all["ide10.a"].as_str(), all["ide10.b"].as_str()), ("new", "2"));
        assert!(import(&p, "nope").is_err());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
