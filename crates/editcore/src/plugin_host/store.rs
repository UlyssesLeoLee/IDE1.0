//! 插件的落盘与 GitHub Release 安装 / 更新 (docs/01_基本设计/PLUGIN-SYSTEM.md §5–§7)。
//!
//! - 默认插件随程序打包 (`bundled`), 离线可用; 用户从 GitHub 装的放在 `root/<id>/`, 登记在 `root/registry.json`。
//! - 网络只经 [`Http`] 这一层 (生产用系统 `curl` 子进程, 不给编辑器引入 TLS 依赖; 测试用假实现)。只在用户点"检查 / 安装 / 更新"时才请求, 不轮询。
//! - 安装 = 先在 `root/.tmp-<id>/` 下载并校验, 通过后原子换名; 任何一步失败都不留半成品, 旧版本保持可用。

use super::{check_compat, is_newer, sha256_hex, Asset, CompatReport, Manifest, MAX_ASSET_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 随程序打包的默认插件: (id, 文件名, 内容)
const BUNDLED: &[(&str, &str, &[u8])] = &[
    ("pele", "manifest.json", include_bytes!("../../plugins/pele/manifest.json")),
    ("pele", "main.js", include_bytes!("../../plugins/pele/main.js")),
    ("pele", "pele.global.js", include_bytes!("../../plugins/pele/pele.global.js")),
    ("pele", "LICENSE", include_bytes!("../../plugins/pele/LICENSE")),
];

pub trait Http {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// 系统自带的 curl (Windows 10+ 自带); 没有就给出清楚的错误
pub struct CurlHttp;
impl Http for CurlHttp {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        if !url.starts_with("https://") {
            return Err(format!("只允许 https 地址: {url}"));
        }
        let out = std::process::Command::new("curl")
            .args(["-sSL", "--fail", "--max-time", "60", "--max-filesize", &MAX_ASSET_BYTES.to_string(), "-H", "Accept: application/vnd.github+json", "-H", "User-Agent: IDE1.0-plugin-host", url])
            .output()
            .map_err(|e| format!("无法运行 curl ({e}): 请确认系统自带 curl 可用"))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(format!("下载失败 {url}: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Registry {
    #[serde(default)]
    plugins: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    version: String,
    repo: String,
    enabled: bool,
    /// 用户安装时同意的权限
    permissions: Vec<String>,
    /// 各资产的 SHA-256
    sha256: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Installed {
    pub manifest: Manifest,
    pub enabled: bool,
    /// bundled = 随程序打包; installed = 用户从 GitHub 装的
    pub source: &'static str,
    pub sha256: BTreeMap<String, String>,
}

fn registry_path(root: &Path) -> PathBuf {
    root.join("registry.json")
}
fn load_registry(root: &Path) -> Registry {
    std::fs::read_to_string(registry_path(root)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save_registry(root: &Path, reg: &Registry) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let tmp = root.join("registry.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(reg).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, registry_path(root)).map_err(|e| e.to_string())
}

fn bundled_file(id: &str, file: &str) -> Option<&'static [u8]> {
    BUNDLED.iter().find(|(i, f, _)| *i == id && *f == file).map(|(_, _, b)| *b)
}
fn bundled_manifest(id: &str) -> Option<Manifest> {
    serde_json::from_slice(bundled_file(id, "manifest.json")?).ok()
}
fn bundled_ids() -> Vec<&'static str> {
    let mut v: Vec<&str> = BUNDLED.iter().map(|(i, _, _)| *i).collect();
    v.dedup();
    v
}

fn valid_file_name(f: &str) -> bool {
    !f.is_empty() && !f.contains(['/', '\\', ':']) && f != ".." && f != "."
}

/// 已有的插件: 打包的默认插件 + 用户安装的 (同 id 时装的版本更新就用装的)
pub fn list(root: &Path) -> Vec<Installed> {
    let reg = load_registry(root);
    let mut out: Vec<Installed> = Vec::new();
    for id in bundled_ids() {
        let Some(m) = bundled_manifest(id) else { continue };
        let installed = reg.plugins.get(id);
        if let Some(e) = installed {
            if let Some(im) = read_installed_manifest(root, id) {
                if is_newer(&im.version, &m.version) || im.version == m.version {
                    out.push(Installed { manifest: im, enabled: e.enabled, source: "installed", sha256: e.sha256.clone() });
                    continue;
                }
            }
        }
        // 默认插件默认启用; 用户禁用过就记在 registry 里
        out.push(Installed { manifest: m, enabled: installed.map(|e| e.enabled).unwrap_or(true), source: "bundled", sha256: BTreeMap::new() });
    }
    for (id, e) in &reg.plugins {
        if out.iter().any(|p| &p.manifest.id == id) {
            continue;
        }
        if let Some(m) = read_installed_manifest(root, id) {
            out.push(Installed { manifest: m, enabled: e.enabled, source: "installed", sha256: e.sha256.clone() });
        }
    }
    out
}

fn read_installed_manifest(root: &Path, id: &str) -> Option<Manifest> {
    serde_json::from_slice(&std::fs::read(root.join(id).join("manifest.json")).ok()?).ok()
}

/// 读插件的一个文件 (入口脚本 / 样式 / 许可证)。装的版本优先于打包的
pub fn read_asset(root: &Path, id: &str, file: &str) -> Result<Vec<u8>, String> {
    if !valid_file_name(file) || !super::valid_id_pub(id) {
        return Err("非法的插件文件名".into());
    }
    let p = list(root).into_iter().find(|p| p.manifest.id == id).ok_or_else(|| format!("没有插件 {id}"))?;
    if p.source == "installed" {
        return std::fs::read(root.join(id).join(file)).map_err(|e| format!("读取 {id}/{file} 失败: {e}"));
    }
    bundled_file(id, file).map(|b| b.to_vec()).ok_or_else(|| format!("插件 {id} 没有文件 {file}"))
}

pub fn set_enabled(root: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let plugin = list(root).into_iter().find(|p| p.manifest.id == id).ok_or_else(|| format!("没有插件 {id}"))?;
    let mut reg = load_registry(root);
    let e = reg.plugins.entry(id.to_string()).or_insert_with(|| Entry { version: plugin.manifest.version.clone(), repo: plugin.manifest.repo.clone(), enabled, permissions: plugin.manifest.permissions.clone(), sha256: plugin.sha256.clone() });
    e.enabled = enabled;
    save_registry(root, &reg)
}

/// 卸载用户装的版本 (打包的默认插件只能禁用, 删掉装的更新则回到打包版本)
pub fn remove(root: &Path, id: &str) -> Result<(), String> {
    if !super::valid_id_pub(id) {
        return Err("非法的插件 id".into());
    }
    let mut reg = load_registry(root);
    reg.plugins.remove(id);
    save_registry(root, &reg)?;
    let dir = root.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------- GitHub Release ----------

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseInfo {
    pub repo: String,
    pub tag: String,
    pub assets: Vec<ReleaseAsset>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

/// `owner/name` 或 `https://github.com/owner/name(.git)` -> `owner/name`
pub fn parse_repo(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/').trim_end_matches(".git");
    let s = s.strip_prefix("https://github.com/").unwrap_or(s);
    let mut it = s.split('/');
    let (o, n) = (it.next()?, it.next()?);
    let ok = |p: &str| !p.is_empty() && p.len() <= 100 && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    (it.next().is_none() && ok(o) && ok(n)).then(|| format!("{o}/{n}"))
}

pub fn fetch_latest(http: &dyn Http, repo: &str) -> Result<ReleaseInfo, String> {
    let repo = parse_repo(repo).ok_or_else(|| format!("仓库地址不合法: {repo} (应为 owner/name)"))?;
    let body = http.get(&format!("https://api.github.com/repos/{repo}/releases/latest"))?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| format!("Release 响应无法解析: {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("仓库还没有 Release")?.to_string();
    let assets = v["assets"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| Some(ReleaseAsset { name: x["name"].as_str()?.to_string(), size: x["size"].as_u64().unwrap_or(0), url: x["browser_download_url"].as_str()?.to_string() })).collect())
        .unwrap_or_default();
    Ok(ReleaseInfo { repo, tag, assets })
}

#[derive(Debug, Clone, Serialize)]
pub struct Inspection {
    pub release: ReleaseInfo,
    pub report: CompatReport,
    /// 已装版本 (若已安装) 和是否有更新
    pub installed_version: Option<String>,
    pub update_available: bool,
}

/// 取最新 Release 并做兼容性检查 (不安装)。清单读不到 / 不合格时 `report.problems` 说明原因, 界面据此报错
pub fn inspect(http: &dyn Http, root: &Path, repo: &str, app_version: &str) -> Result<Inspection, String> {
    let release = fetch_latest(http, repo)?;
    let assets: Vec<Asset> = release.assets.iter().map(|a| Asset { name: a.name.clone(), size: a.size }).collect();
    let manifest_text = release.assets.iter().find(|a| a.name == "manifest.json").and_then(|a| http.get(&a.url).ok()).and_then(|b| String::from_utf8(b).ok());
    let report = check_compat(&assets, manifest_text.as_deref(), app_version);
    let installed_version = report.manifest.as_ref().and_then(|m| list(root).into_iter().find(|p| p.manifest.id == m.id).map(|p| p.manifest.version));
    let update_available = match (&report.manifest, &installed_version) {
        (Some(m), Some(v)) => is_newer(&m.version, v),
        _ => false,
    };
    Ok(Inspection { release, report, installed_version, update_available })
}

/// 安装 / 更新到最新 Release。`approved` = 用户在确认框里同意的权限, 必须覆盖清单声明的全部权限
pub fn install(http: &dyn Http, root: &Path, repo: &str, app_version: &str, approved: &[String]) -> Result<Installed, String> {
    let ins = inspect(http, root, repo, app_version)?;
    if !ins.report.ok {
        let why: Vec<String> = ins.report.problems.iter().map(|p| format!("{}: {}", p.code, p.message)).collect();
        return Err(format!("插件与当前编辑器不匹配, 未安装: {}", why.join("; ")));
    }
    let m = ins.report.manifest.clone().ok_or("缺少清单")?;
    if let Some(p) = m.permissions.iter().find(|p| !approved.contains(p)) {
        return Err(format!("权限「{p}」未获用户同意, 未安装"));
    }
    // 下载全部需要的资产到临时目录
    let tmp = root.join(format!(".tmp-{}", m.id));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| -> Result<BTreeMap<String, String>, String> {
        let mut sums = BTreeMap::new();
        for a in ins.release.assets.iter().filter(|a| a.name == "manifest.json" || a.name == m.entry || a.name == "styles.css" || a.name == "LICENSE" || a.name.ends_with(".js")) {
            if !valid_file_name(&a.name) {
                continue;
            }
            let bytes = http.get(&a.url)?;
            if bytes.len() as u64 > MAX_ASSET_BYTES {
                return Err(format!("资产 {} 超过大小上限", a.name));
            }
            sums.insert(a.name.clone(), sha256_hex(&bytes));
            std::fs::write(tmp.join(&a.name), bytes).map_err(|e| e.to_string())?;
        }
        if !sums.contains_key(&m.entry) {
            return Err(format!("入口脚本 {} 下载失败", m.entry));
        }
        Ok(sums)
    })();
    let sums = match result {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    // 原子换名: 旧版本先挪开, 新版本就位, 成功后删旧的; 失败回滚旧的
    let dest = root.join(&m.id);
    let old = root.join(format!(".old-{}", m.id));
    let _ = std::fs::remove_dir_all(&old);
    let had_old = dest.exists();
    if had_old {
        std::fs::rename(&dest, &old).map_err(|e| e.to_string())?;
    }
    if let Err(e) = std::fs::rename(&tmp, &dest) {
        if had_old {
            let _ = std::fs::rename(&old, &dest);
        }
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("安装失败 (旧版本保持可用): {e}"));
    }
    let _ = std::fs::remove_dir_all(&old);
    let mut reg = load_registry(root);
    let enabled = reg.plugins.get(&m.id).map(|e| e.enabled).unwrap_or(true);
    reg.plugins.insert(m.id.clone(), Entry { version: m.version.clone(), repo: ins.release.repo.clone(), enabled, permissions: approved.to_vec(), sha256: sums.clone() });
    save_registry(root, &reg)?;
    Ok(Installed { manifest: m, enabled, source: "installed", sha256: sums })
}

/// 对每个已装插件 (有 repo 的) 检查是否有新版本 —— 用户点"检查更新"时才调用
pub fn check_updates(http: &dyn Http, root: &Path, app_version: &str) -> Vec<(String, Result<Inspection, String>)> {
    list(root).into_iter().filter(|p| !p.manifest.repo.is_empty()).map(|p| (p.manifest.id.clone(), inspect(http, root, &p.manifest.repo, app_version))).collect()
}

/// 数据目录: IDE_PLUGIN_DIR 优先 (测试 / 便携版), 否则 %APPDATA%/IDE1.0/plugins (非 Windows: ~/.local/share)
pub fn default_root() -> PathBuf {
    if let Ok(p) = std::env::var("IDE_PLUGIN_DIR") {
        return PathBuf::from(p);
    }
    let base = std::env::var("APPDATA").map(PathBuf::from).or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share"))).unwrap_or_else(|_| PathBuf::from("."));
    base.join("IDE1.0").join("plugins")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct Fake(HashMap<String, Vec<u8>>, RefCell<Vec<String>>);
    impl Http for Fake {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.1.borrow_mut().push(url.to_string());
            self.0.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
    }
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-plug-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }
    fn release(repo: &str, tag: &str, files: &[(&str, &str)]) -> Fake {
        let mut m = HashMap::new();
        let assets: Vec<serde_json::Value> = files.iter().map(|(n, c)| {
            let url = format!("https://dl.example/{repo}/{tag}/{n}");
            m.insert(url.clone(), c.as_bytes().to_vec());
            serde_json::json!({ "name": n, "size": c.len(), "browser_download_url": url })
        }).collect();
        m.insert(format!("https://api.github.com/repos/{repo}/releases/latest"), serde_json::to_vec(&serde_json::json!({ "tag_name": tag, "assets": assets })).unwrap());
        Fake(m, RefCell::new(vec![]))
    }
    const MANI: &str = r#"{"id":"hello","name":"Hello","version":"1.0.0","apiVersion":1,"repo":"o/hello","entry":"main.js","permissions":["storage"],"contributes":{"command":[{"id":"hi"}]}}"#;

    #[test]
    fn bundled_pele_is_listed_enabled_and_readable() {
        let root = tmp("bundled");
        let l = list(&root);
        let p = l.iter().find(|p| p.manifest.id == "pele").expect("默认插件 pele");
        assert!(p.enabled && p.source == "bundled");
        assert!(read_asset(&root, "pele", "main.js").unwrap().len() > 10);
        assert!(read_asset(&root, "pele", "../x").is_err());
        set_enabled(&root, "pele", false).unwrap();
        assert!(!list(&root).iter().find(|p| p.manifest.id == "pele").unwrap().enabled, "禁用记在 registry 里");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_repo_forms() {
        assert_eq!(parse_repo("obsidianmd/pele").as_deref(), Some("obsidianmd/pele"));
        assert_eq!(parse_repo("https://github.com/obsidianmd/pele.git/").as_deref(), Some("obsidianmd/pele"));
        assert!(parse_repo("a/b/c").is_none() && parse_repo("..").is_none() && parse_repo("a b/c").is_none());
    }

    #[test]
    fn install_requires_approved_permissions_then_updates_atomically() {
        let root = tmp("install");
        let h = release("o/hello", "v1.0.0", &[("manifest.json", MANI), ("main.js", "ide.register({})")]);
        assert!(install(&h, &root, "o/hello", "0.1.2", &[]).unwrap_err().contains("未获用户同意"));
        assert!(!root.join("hello").exists(), "没同意就不落盘");
        let p = install(&h, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        assert_eq!(p.manifest.version, "1.0.0");
        assert_eq!(p.sha256["main.js"], sha256_hex(b"ide.register({})"));
        assert_eq!(read_asset(&root, "hello", "main.js").unwrap(), b"ide.register({})");
        // 发新版: 能检测到更新, 一键更新后换成新版
        let h2 = release("o/hello", "v1.1.0", &[("manifest.json", &MANI.replace("1.0.0", "1.1.0")), ("main.js", "ide.register({v:2})")]);
        let ins = inspect(&h2, &root, "o/hello", "0.1.2").unwrap();
        assert!(ins.update_available && ins.installed_version.as_deref() == Some("1.0.0"));
        install(&h2, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        assert_eq!(read_asset(&root, "hello", "main.js").unwrap(), b"ide.register({v:2})");
        assert!(!root.join(".old-hello").exists() && !root.join(".tmp-hello").exists(), "不留中间目录");
        remove(&root, "hello").unwrap();
        assert!(list(&root).iter().all(|p| p.manifest.id != "hello"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mismatch_is_reported_and_nothing_is_installed() {
        let root = tmp("mismatch");
        // 没有 manifest.json (像 Pele 上游的 Release)
        let h = release("obsidianmd/pele", "1.12.0", &[("pele.global.js", "x")]);
        let ins = inspect(&h, &root, "obsidianmd/pele", "0.1.2").unwrap();
        assert!(!ins.report.ok && ins.report.problems[0].code == "NO_MANIFEST");
        let err = install(&h, &root, "obsidianmd/pele", "0.1.2", &[]).unwrap_err();
        assert!(err.contains("不匹配") && err.contains("NO_MANIFEST"), "{err}");
        assert!(!root.join("pele").exists());
        // 入口缺失
        let h = release("o/hello", "v1.0.0", &[("manifest.json", MANI)]);
        assert!(install(&h, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap_err().contains("NO_ENTRY"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn failed_download_keeps_old_version() {
        let root = tmp("rollback");
        let h = release("o/hello", "v1.0.0", &[("manifest.json", MANI), ("main.js", "old")]);
        install(&h, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        let mut h2 = release("o/hello", "v1.1.0", &[("manifest.json", &MANI.replace("1.0.0", "1.1.0")), ("main.js", "new")]);
        h2.0.remove("https://dl.example/o/hello/v1.1.0/main.js");   // 入口下载 404
        assert!(install(&h2, &root, "o/hello", "0.1.2", &["storage".into()]).is_err());
        assert_eq!(read_asset(&root, "hello", "main.js").unwrap(), b"old", "旧版本保持可用");
        assert!(!root.join(".tmp-hello").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
