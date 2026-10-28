//! 热插拔插件底座 (见 docs/01_基本设计/PLUGIN-SYSTEM.md): 清单解析与校验、兼容性检查、版本比较、SHA-256、
//! 插件与内建功能的冲突仲裁。全是纯函数 —— 网络 / 文件落盘在上层, 前端沙箱在 `plugins/host.js`。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod store;

/// 宿主支持的最高插件 API 版本
pub const HOST_API_VERSION: u32 = 1;
/// 单个资产大小上限 (清单 / 入口脚本)
pub const MAX_ASSET_BYTES: u64 = 8 * 1024 * 1024;
/// 宿主认识的权限; 不认识的权限一律报 `UNKNOWN_PERMISSION`
const KNOWN_PERMISSIONS: [&str; 3] = ["editor.read", "editor.write", "storage"];
/// 宿主提供的贡献点
const KNOWN_CONTRIBUTIONS: [&str; 3] = ["markdown.fence", "command", "agent.tool"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default = "one", rename = "apiVersion")]
    pub api_version: u32,
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: Option<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default = "main_js")]
    pub entry: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub contributes: BTreeMap<String, serde_json::Value>,
}
fn one() -> u32 {
    1
}
fn main_js() -> String {
    "main.js".into()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Problem {
    pub code: &'static str,
    pub message: String,
    pub hint: String,
}
fn problem(code: &'static str, message: impl Into<String>, hint: impl Into<String>) -> Problem {
    Problem { code, message: message.into(), hint: hint.into() }
}

#[derive(Debug, Clone, Serialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompatReport {
    pub ok: bool,
    pub manifest: Option<Manifest>,
    pub problems: Vec<Problem>,
}

/// `1.2.3` / `v1.2.3` / `1.2.3-beta` -> (1,2,3); 不合法 = None
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let n = |s: Option<&str>| s?.parse::<u64>().ok();
    let r = (n(it.next())?, n(it.next())?, n(it.next())?);
    if it.next().is_some() { None } else { Some(r) }
}

/// `latest` 比 `current` 新?
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_semver(latest), parse_semver(current)), (Some(a), Some(b)) if a > b)
}

pub(crate) fn valid_id_pub(id: &str) -> bool {
    valid_id(id)
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn valid_entry(e: &str) -> bool {
    !e.is_empty() && !e.contains("..") && !e.starts_with('/') && !e.contains('\\') && !e.contains(':')
}

/// 检查一个 Release (资产清单 + manifest.json 文本) 能不能装进当前宿主。
/// 返回结构化问题, 界面显示它们, "让 AI 给出方案"也以它们为事实依据。
pub fn check_compat(assets: &[Asset], manifest_json: Option<&str>, app_version: &str) -> CompatReport {
    let mut problems = Vec::new();
    let Some(text) = manifest_json else {
        problems.push(problem("NO_MANIFEST", "最新 Release 里没有 manifest.json", "在仓库 Release 里附上 manifest.json 和入口脚本, 或做一个适配仓库"));
        return CompatReport { ok: false, manifest: None, problems };
    };
    let m: Manifest = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            problems.push(problem("BAD_MANIFEST", format!("manifest.json 解析失败: {e}"), "按 docs/01_基本设计/PLUGIN-SYSTEM.md §2 的字段修正清单"));
            return CompatReport { ok: false, manifest: None, problems };
        }
    };
    if !valid_id(&m.id) {
        problems.push(problem("BAD_ID", format!("id「{}」不合法", m.id), "id 只能含小写字母 / 数字 / 连字符, 最长 64"));
    }
    if parse_semver(&m.version).is_none() {
        problems.push(problem("BAD_VERSION", format!("version「{}」不是 x.y.z", m.version), "使用语义化版本号"));
    }
    if m.api_version > HOST_API_VERSION {
        problems.push(problem("API_TOO_NEW", format!("插件要求 apiVersion {}, 宿主最高支持 {}", m.api_version, HOST_API_VERSION), "升级编辑器, 或使用插件的旧版本"));
    }
    if let Some(min) = &m.min_app_version {
        match (parse_semver(min), parse_semver(app_version)) {
            (Some(need), Some(have)) if need > have => problems.push(problem("APP_TOO_OLD", format!("插件要求编辑器 ≥ {min}, 当前 {app_version}"), "升级编辑器")),
            (None, _) => problems.push(problem("BAD_VERSION", format!("minAppVersion「{min}」不是 x.y.z"), "使用语义化版本号")),
            _ => {}
        }
    }
    if !valid_entry(&m.entry) {
        problems.push(problem("BAD_ENTRY", format!("入口「{}」不合法", m.entry), "入口必须是 Release 资产里的相对文件名"));
    } else if !assets.iter().any(|a| a.name == m.entry) {
        problems.push(problem("NO_ENTRY", format!("Release 资产里没有入口脚本 {}", m.entry), "把入口脚本作为 Release 资产上传"));
    }
    for p in &m.permissions {
        let known = KNOWN_PERMISSIONS.contains(&p.as_str()) || p.strip_prefix("net:").is_some_and(|h| !h.is_empty() && !h.contains('/'));
        if !known {
            problems.push(problem("UNKNOWN_PERMISSION", format!("未知权限「{p}」"), "只能声明 editor.read / editor.write / storage / net:<host>"));
        }
    }
    for k in m.contributes.keys() {
        if !KNOWN_CONTRIBUTIONS.contains(&k.as_str()) {
            problems.push(problem("UNKNOWN_CONTRIBUTION", format!("未知贡献点「{k}」"), "见 docs/01_基本设计/PLUGIN-SYSTEM.md §3"));
        }
    }
    for a in assets.iter().filter(|a| a.size > MAX_ASSET_BYTES) {
        problems.push(problem("ASSET_TOO_BIG", format!("资产 {} 有 {} 字节, 超过上限 {}", a.name, a.size, MAX_ASSET_BYTES), "精简插件体积"));
    }
    CompatReport { ok: problems.is_empty(), manifest: Some(m), problems }
}

// ---------- 冲突仲裁 (docs/01_基本设计/PLUGIN-SYSTEM.md §8.1) ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Prefer {
    Plugin,
    Native,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Plugin,
    Native,
    /// 需要弹一次选择框 (只会在智能功能已激活且用户没定过偏好时出现)
    AskUser,
}

/// 同一能力位上, 内建实现与插件实现谁来处理。
/// - 只有一方存在: 用它; 都没有: None
/// - 用户有明确偏好 (Plugin / Native): 照办
/// - 没有偏好: 智能功能未激活 -> 插件覆盖原生; 已激活 -> 问用户
pub fn resolve_conflict(has_native: bool, has_plugin: bool, prefer: Option<Prefer>, smart_active: bool) -> Option<Choice> {
    match (has_native, has_plugin) {
        (false, false) => None,
        (true, false) => Some(Choice::Native),
        (false, true) => Some(Choice::Plugin),
        (true, true) => Some(match prefer {
            Some(Prefer::Plugin) => Choice::Plugin,
            Some(Prefer::Native) => Choice::Native,
            Some(Prefer::Ask) | None if smart_active => Choice::AskUser,
            Some(Prefer::Ask) | None => Choice::Plugin,
        }),
    }
}

/// 选中的一方失败时的回退: 换另一方 (另一方不存在就没有)
pub fn fallback(failed: Choice, has_native: bool, has_plugin: bool) -> Option<Choice> {
    match failed {
        Choice::Plugin if has_native => Some(Choice::Native),
        Choice::Native if has_plugin => Some(Choice::Plugin),
        _ => None,
    }
}

// ---------- SHA-256 (安装确认框里显示各资产的哈希; 不为此引入依赖) ----------

pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, b) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}


/// 不匹配时发给用户配置的大模型的提示词: 只放事实 (宿主契约、仓库 / tag / 资产、结构化问题), 不含任何密钥。
pub fn mismatch_prompt(repo: &str, tag: &str, asset_names: &[String], report: &CompatReport, app_version: &str) -> String {
    let problems: Vec<String> = report.problems.iter().map(|p| format!("- [{}] {} (提示: {})", p.code, p.message, p.hint)).collect();
    format!(
        "你是插件兼容性顾问。IDE1.0 编辑器 (版本 {app_version}, 插件 apiVersion 最高 {HOST_API_VERSION}) 想从 GitHub 仓库 {repo} 的最新 Release {tag} 安装插件, 但检查未通过。
         
宿主插件契约: Release 资产里必须有 manifest.json (字段: id[a-z0-9-], name, version[x.y.z], apiVersion, minAppVersion?, license, description, repo, entry[默认 main.js], permissions[editor.read|editor.write|storage|net:<host>], contributes[markdown.fence|command|agent.tool]) 和 manifest.entry 指向的入口脚本; 入口脚本在沙箱 iframe 中运行, 通过全局 ide.register({{...}}) 注册贡献点方法。
         
Release 资产: {}

检查发现的问题:
{}
         
请回答: 1) 根因; 2) 给插件作者或用户的具体修改步骤; 3) 如果能写出适配用的 manifest.json / main.js, 给出完整内容; 4) 风险提示。只给建议, 不要假设你能直接修改任何文件。",
        asset_names.join(", "),
        problems.join("
")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets(names: &[&str]) -> Vec<Asset> {
        names.iter().map(|n| Asset { name: n.to_string(), size: 100 }).collect()
    }
    const GOOD: &str = r#"{"id":"pele","name":"Pele","version":"1.12.0","apiVersion":1,"minAppVersion":"0.1.2","permissions":[],"contributes":{"markdown.fence":[{"lang":"mermaid","method":"render"}]}}"#;
    fn codes(r: &CompatReport) -> Vec<&'static str> {
        r.problems.iter().map(|p| p.code).collect()
    }

    #[test]
    fn mismatch_prompt_has_facts_and_no_secrets() {
        let r = check_compat(&[], None, "0.1.2");
        let p = mismatch_prompt("o/r", "v1", &["a.js".to_string()], &r, "0.1.2");
        assert!(p.contains("o/r") && p.contains("v1") && p.contains("NO_MANIFEST") && p.contains("a.js") && p.contains("apiVersion"));
        assert!(!p.to_lowercase().contains("api key") && !p.contains("sk-"));
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let long = vec![b'a'; 1000];
        assert_eq!(sha256_hex(&long).len(), 64);
    }

    #[test]
    fn semver_compare() {
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("1.2.3-beta+x"), Some((1, 2, 3)));
        assert_eq!(parse_semver("1.2"), None);
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn good_manifest_passes() {
        let r = check_compat(&assets(&["manifest.json", "main.js"]), Some(GOOD), "0.1.2");
        assert!(r.ok, "{:?}", r.problems);
        assert_eq!(r.manifest.unwrap().id, "pele");
    }

    #[test]
    fn mismatches_are_reported_structurally() {
        let r = check_compat(&assets(&["pele.global.js"]), None, "0.1.2");
        assert_eq!(codes(&r), ["NO_MANIFEST"]);
        let r = check_compat(&[], Some("{not json"), "0.1.2");
        assert_eq!(codes(&r), ["BAD_MANIFEST"]);
        let r = check_compat(&assets(&["main.js"]), Some(&GOOD.replace("\"apiVersion\":1", "\"apiVersion\":3")), "0.1.2");
        assert_eq!(codes(&r), ["API_TOO_NEW"]);
        let r = check_compat(&assets(&["main.js"]), Some(GOOD), "0.1.0");
        assert_eq!(codes(&r), ["APP_TOO_OLD"]);
        let r = check_compat(&assets(&["other.js"]), Some(GOOD), "0.1.2");
        assert_eq!(codes(&r), ["NO_ENTRY"]);
        let bad = GOOD.replace("\"permissions\":[]", "\"permissions\":[\"fs\",\"net:a.com\"]").replace("\"id\":\"pele\"", "\"id\":\"Pele!\"").replace("\"markdown.fence\"", "\"weird\"");
        let r = check_compat(&assets(&["main.js"]), Some(&bad), "0.1.2");
        assert_eq!(codes(&r), ["BAD_ID", "UNKNOWN_PERMISSION", "UNKNOWN_CONTRIBUTION"]);
    }

    #[test]
    fn entry_cannot_escape_plugin_dir() {
        for e in ["../x.js", "/abs.js", "a\\b.js", "C:x.js"] {
            let m = GOOD.replace("\"permissions\"", &format!("\"entry\":{},\"permissions\"", serde_json::json!(e)));
            assert!(codes(&check_compat(&assets(&[e]), Some(&m), "0.1.2")).contains(&"BAD_ENTRY"), "{e}");
        }
    }

    #[test]
    fn conflict_rules() {
        use Choice::*;
        assert_eq!(resolve_conflict(false, false, None, false), None);
        assert_eq!(resolve_conflict(true, false, Some(Prefer::Plugin), false), Some(Native), "只有一方时直接用它");
        assert_eq!(resolve_conflict(false, true, Some(Prefer::Native), true), Some(Plugin));
        assert_eq!(resolve_conflict(true, true, None, false), Some(Plugin), "智能功能未激活 -> 插件覆盖原生");
        assert_eq!(resolve_conflict(true, true, Some(Prefer::Ask), false), Some(Plugin), "未激活智能功能不弹窗");
        assert_eq!(resolve_conflict(true, true, None, true), Some(AskUser), "激活后首次冲突问用户");
        assert_eq!(resolve_conflict(true, true, Some(Prefer::Native), false), Some(Native), "用户选择最高");
        assert_eq!(resolve_conflict(true, true, Some(Prefer::Native), true), Some(Native));
        assert_eq!(fallback(Plugin, true, true), Some(Native));
        assert_eq!(fallback(Native, true, false), None);
    }
}
