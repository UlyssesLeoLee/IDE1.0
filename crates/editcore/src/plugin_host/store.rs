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
    (
        "pele",
        "manifest.json",
        include_bytes!("../../plugins/pele/manifest.json"),
    ),
    (
        "pele",
        "main.js",
        include_bytes!("../../plugins/pele/main.js"),
    ),
    (
        "pele",
        "pele.global.js",
        include_bytes!("../../plugins/pele/pele.global.js"),
    ),
    (
        "pele",
        "LICENSE",
        include_bytes!("../../plugins/pele/LICENSE"),
    ),
    (
        "doc-graph",
        "manifest.json",
        include_bytes!("../../plugins/doc-graph/manifest.json"),
    ),
    (
        "doc-graph",
        "main.js",
        include_bytes!("../../plugins/doc-graph/main.js"),
    ),
    (
        "ppa",
        "manifest.json",
        include_bytes!("../../plugins/ppa/manifest.json"),
    ),
    (
        "ppa",
        "main.js",
        include_bytes!("../../plugins/ppa/main.js"),
    ),
    (
        "theme-crystal",
        "manifest.json",
        include_bytes!("../../plugins/theme-crystal/manifest.json"),
    ),
    (
        "theme-crystal",
        "main.js",
        include_bytes!("../../plugins/theme-crystal/main.js"),
    ),
    (
        "theme-retro16",
        "manifest.json",
        include_bytes!("../../plugins/theme-retro16/manifest.json"),
    ),
    (
        "theme-retro16",
        "main.js",
        include_bytes!("../../plugins/theme-retro16/main.js"),
    ),
    (
        "theme-clear",
        "manifest.json",
        include_bytes!("../../plugins/theme-clear/manifest.json"),
    ),
    (
        "theme-clear",
        "main.js",
        include_bytes!("../../plugins/theme-clear/main.js"),
    ),
    (
        "theme-dark",
        "manifest.json",
        include_bytes!("../../plugins/theme-dark/manifest.json"),
    ),
    (
        "theme-dark",
        "main.js",
        include_bytes!("../../plugins/theme-dark/main.js"),
    ),
    (
        "theme-light",
        "manifest.json",
        include_bytes!("../../plugins/theme-light/manifest.json"),
    ),
    (
        "theme-light",
        "main.js",
        include_bytes!("../../plugins/theme-light/main.js"),
    ),
    (
        "theme-hc",
        "manifest.json",
        include_bytes!("../../plugins/theme-hc/manifest.json"),
    ),
    (
        "theme-hc",
        "main.js",
        include_bytes!("../../plugins/theme-hc/main.js"),
    ),
    (
        "theme-comic",
        "manifest.json",
        include_bytes!("../../plugins/theme-comic/manifest.json"),
    ),
    (
        "theme-comic",
        "main.js",
        include_bytes!("../../plugins/theme-comic/main.js"),
    ),
    (
        "nema-interpreter",
        "manifest.json",
        include_bytes!("../../plugins/nema-interpreter/manifest.json"),
    ),
    (
        "nema-interpreter",
        "main.js",
        include_bytes!("../../plugins/nema-interpreter/main.js"),
    ),
    (
        "nema-interpreter",
        "strict-parser.js",
        include_bytes!("../../plugins/nema-interpreter/strict-parser.js"),
    ),
    (
        "mcp",
        "manifest.json",
        include_bytes!("../../plugins/mcp/manifest.json"),
    ),
    (
        "mcp",
        "main.js",
        include_bytes!("../../plugins/mcp/main.js"),
    ),
    (
        "hooks",
        "manifest.json",
        include_bytes!("../../plugins/hooks/manifest.json"),
    ),
    (
        "hooks",
        "main.js",
        include_bytes!("../../plugins/hooks/main.js"),
    ),
    (
        "skills",
        "manifest.json",
        include_bytes!("../../plugins/skills/manifest.json"),
    ),
    (
        "skills",
        "main.js",
        include_bytes!("../../plugins/skills/main.js"),
    ),
    (
        "mod",
        "manifest.json",
        include_bytes!("../../plugins/mod/manifest.json"),
    ),
    (
        "mod",
        "main.js",
        include_bytes!("../../plugins/mod/main.js"),
    ),
    (
        "automation",
        "manifest.json",
        include_bytes!("../../plugins/automation/manifest.json"),
    ),
    (
        "automation",
        "main.js",
        include_bytes!("../../plugins/automation/main.js"),
    ),
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
        let mut cmd = std::process::Command::new("curl");
        cmd.args([
            "-sSL",
            "--fail",
            "--max-time",
            "60",
            "--max-filesize",
            &MAX_ASSET_BYTES.to_string(),
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: IDE1.0-plugin-host",
            url,
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.stdin(std::process::Stdio::null())
                .creation_flags(0x0800_0000);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("无法运行 curl ({e}): 请确认系统自带 curl 可用"))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(format!(
                "下载失败 {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Registry {
    #[serde(default)]
    plugins: BTreeMap<String, Entry>,
    /// 用户删掉的内置插件 id (不再列出、不再加载; 「恢复」或装插件包才回来)
    #[serde(default)]
    removed: std::collections::BTreeSet<String>,
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
    std::fs::read_to_string(registry_path(root))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn save_registry(root: &Path, reg: &Registry) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let tmp = root.join("registry.json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(reg).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, registry_path(root)).map_err(|e| e.to_string())
}

fn bundled_file(id: &str, file: &str) -> Option<&'static [u8]> {
    BUNDLED
        .iter()
        .find(|(i, f, _)| *i == id && *f == file)
        .map(|(_, _, b)| *b)
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

/// 插件包里的文件名还要躲开 Windows 的坑: 设备名 (nul / con / aux / com1… 带不带扩展名都算)、结尾的点和空格 (会被系统悄悄去掉)
fn windows_safe_name(f: &str) -> bool {
    if f.ends_with('.') || f.ends_with(' ') || f.contains(['<', '>', '"', '|', '?', '*']) {
        return false;
    }
    let stem = f.split('.').next().unwrap_or("").to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    !reserved
}

/// 已有的插件: 打包的默认插件 + 用户安装的 (同 id 时装的版本更新就用装的)
pub fn list(root: &Path) -> Vec<Installed> {
    let reg = load_registry(root);
    let mut out: Vec<Installed> = Vec::new();
    for id in bundled_ids() {
        if reg.removed.contains(id) {
            continue; // 用户删掉了这个内置插件
        }
        let Some(m) = bundled_manifest(id) else {
            continue;
        };
        let installed = reg.plugins.get(id);
        if let Some(e) = installed {
            if let Some(im) = read_installed_manifest(root, id) {
                if is_newer(&im.version, &m.version) || im.version == m.version {
                    out.push(Installed {
                        manifest: im,
                        enabled: e.enabled,
                        source: "installed",
                        sha256: e.sha256.clone(),
                    });
                    continue;
                }
            }
        }
        // 默认插件默认启用; 用户禁用过就记在 registry 里
        out.push(Installed {
            manifest: m,
            enabled: installed.map(|e| e.enabled).unwrap_or(true),
            source: "bundled",
            sha256: BTreeMap::new(),
        });
    }
    for (id, e) in &reg.plugins {
        if out.iter().any(|p| &p.manifest.id == id) {
            continue;
        }
        if let Some(m) = read_installed_manifest(root, id) {
            out.push(Installed {
                manifest: m,
                enabled: e.enabled,
                source: "installed",
                sha256: e.sha256.clone(),
            });
        }
    }
    out
}

/// 获取指定 id 的单个插件 (启动时按需查询目标主题插件清单, 避免扫描全部插件与解析无用 JSON)
pub fn get(root: &Path, id: &str) -> Option<Installed> {
    if !super::valid_id_pub(id) {
        return None;
    }
    let reg = load_registry(root);
    if reg.removed.contains(id) {
        return None;
    }
    let installed = reg.plugins.get(id);
    let bundled = if bundled_ids().contains(&id) {
        bundled_manifest(id)
    } else {
        None
    };
    if let Some(e) = installed {
        if let Some(im) = read_installed_manifest(root, id) {
            if let Some(m) = &bundled {
                if is_newer(&im.version, &m.version) || im.version == m.version {
                    return Some(Installed {
                        manifest: im,
                        enabled: e.enabled,
                        source: "installed",
                        sha256: e.sha256.clone(),
                    });
                }
            } else {
                return Some(Installed {
                    manifest: im,
                    enabled: e.enabled,
                    source: "installed",
                    sha256: e.sha256.clone(),
                });
            }
        }
    }
    if let Some(m) = bundled {
        return Some(Installed {
            manifest: m,
            enabled: installed.map(|e| e.enabled).unwrap_or(true),
            source: "bundled",
            sha256: BTreeMap::new(),
        });
    }
    None
}

fn read_installed_manifest(root: &Path, id: &str) -> Option<Manifest> {
    let mut m: Manifest =
        serde_json::from_slice(&std::fs::read(root.join(id).join("manifest.json")).ok()?).ok()?;
    if m.id != id {
        return None; // 目录名与清单 id 不一致: 不认
    }
    // 更新源以安装时登记的仓库为准, 不信清单里自称的 repo
    if let Some(e) = load_registry(root).plugins.get(id) {
        m.repo = e.repo.clone();
    }
    Some(m)
}

/// 读插件的一个文件 (入口脚本 / 样式 / 许可证)。装的版本优先于打包的
pub fn read_asset(root: &Path, id: &str, file: &str) -> Result<Vec<u8>, String> {
    if !valid_file_name(file) || !super::valid_id_pub(id) {
        return Err("非法的插件文件名".into());
    }
    let p = list(root)
        .into_iter()
        .find(|p| p.manifest.id == id)
        .ok_or_else(|| format!("没有插件 {id}"))?;
    if p.source == "installed" {
        return std::fs::read(root.join(id).join(file))
            .map_err(|e| format!("读取 {id}/{file} 失败: {e}"));
    }
    bundled_file(id, file)
        .map(|b| b.to_vec())
        .ok_or_else(|| format!("插件 {id} 没有文件 {file}"))
}

pub fn set_enabled(root: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let plugin = list(root)
        .into_iter()
        .find(|p| p.manifest.id == id)
        .ok_or_else(|| format!("没有插件 {id}"))?;
    let mut reg = load_registry(root);
    let e = reg.plugins.entry(id.to_string()).or_insert_with(|| Entry {
        version: plugin.manifest.version.clone(),
        repo: plugin.manifest.repo.clone(),
        enabled,
        permissions: plugin.manifest.permissions.clone(),
        sha256: plugin.sha256.clone(),
    });
    e.enabled = enabled;
    save_registry(root, &reg)
}

/// 删除一个插件。用户装的: 连文件一起删; 随程序打包的默认插件: 也可以删 (记入 `removed`, 不再列出 / 加载),
/// 之后能用「恢复内置」或 Release 里的插件包 (.zip) 装回来 —— 数据都在注册表和插件目录里, 程序本体不变。
pub fn remove(root: &Path, id: &str) -> Result<(), String> {
    if !super::valid_id_pub(id) {
        return Err("非法的插件 id".into());
    }
    let mut reg = load_registry(root);
    reg.plugins.remove(id);
    if bundled_ids().contains(&id) {
        reg.removed.insert(id.to_string());
    }
    save_registry(root, &reg)?;
    let dir = root.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 被用户删掉的内置插件 (id, 名字, 版本): 界面列出来, 供「恢复内置」
pub fn removed_bundled(root: &Path) -> Vec<(String, String, String)> {
    load_registry(root)
        .removed
        .iter()
        .filter_map(|id| bundled_manifest(id).map(|m| (id.clone(), m.name, m.version)))
        .collect()
}

/// 恢复一个被删掉的内置插件 (用程序里打包的那份)
pub fn restore_bundled(root: &Path, id: &str) -> Result<(), String> {
    if !bundled_ids().contains(&id) {
        return Err(format!("{id} 不是内置插件"));
    }
    let mut reg = load_registry(root);
    reg.removed.remove(id);
    reg.plugins.remove(id);
    save_registry(root, &reg)?;
    let dir = root.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?; // 装过的旧文件不留孤儿
    }
    Ok(())
}

/// 热插拔更新插件脚本文件（例如 `strict-parser.js`）：落地到 `root/<id>/<script_file>` 并登记为 `installed`，
/// 使其在 `read_asset` 与沙箱加载时立即优先生效；通过 [`restore_bundled`] 可一键还原到随程序打包版本。
pub fn hot_patch_plugin_script(
    root: &Path,
    id: &str,
    script_file: &str,
    content: &[u8],
) -> Result<Installed, String> {
    if !super::valid_id_pub(id) {
        return Err("非法的插件 id".into());
    }
    if !valid_file_name(script_file)
        || !windows_safe_name(script_file)
        || !script_file.ends_with(".js")
    {
        return Err(format!("非法的 JS 脚本文件名: {script_file}"));
    }
    if content.len() as u64 > MAX_ASSET_BYTES {
        return Err("脚本体积超过 8 MiB 上限".into());
    }
    if content.is_empty() {
        return Err("脚本内容不能为空".into());
    }
    let plugin = list(root)
        .into_iter()
        .find(|p| p.manifest.id == id)
        .ok_or_else(|| format!("未找到插件 {id}"))?;
    let m = &plugin.manifest;
    if script_file != m.entry && !m.scripts.iter().any(|s| s == script_file) {
        return Err(format!(
            "脚本 {script_file} 未在插件 {id} 的 entry/scripts 清单中声明"
        ));
    }

    let tmp = root.join(format!(".tmp-patch-{}", m.id));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let staged = (|| -> Result<(BTreeMap<String, String>, Manifest), String> {
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        if plugin.source == "installed" {
            let src_dir = root.join(id);
            if let Ok(rd) = std::fs::read_dir(&src_dir) {
                for ent in rd.flatten() {
                    if ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        let name = ent.file_name().to_string_lossy().into_owned();
                        if valid_file_name(&name) {
                            if let Ok(bytes) = std::fs::read(ent.path()) {
                                files.insert(name, bytes);
                            }
                        }
                    }
                }
            }
        }
        for (bid, bfile, bbytes) in BUNDLED {
            if *bid == id {
                files
                    .entry((*bfile).to_string())
                    .or_insert_with(|| bbytes.to_vec());
            }
        }
        if !files.contains_key("manifest.json") {
            let m_bytes = serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?;
            files.insert("manifest.json".to_string(), m_bytes);
        }
        files.insert(script_file.to_string(), content.to_vec());

        let manifest_text = files
            .get("manifest.json")
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or_else(|| "manifest.json 不是有效的 UTF-8".to_string())?;
        let assets: Vec<Asset> = files
            .iter()
            .map(|(name, bytes)| Asset {
                name: name.clone(),
                size: bytes.len() as u64,
            })
            .collect();
        let report = check_compat(&assets, Some(manifest_text), env!("CARGO_PKG_VERSION"));
        if !report.ok {
            let why: Vec<String> = report
                .problems
                .iter()
                .map(|p| format!("{}: {}", p.code, p.message))
                .collect();
            return Err(format!("热更新校验未通过: {}", why.join("; ")));
        }
        let manifest = report.manifest.ok_or_else(|| "缺少有效清单".to_string())?;

        let mut sums = BTreeMap::new();
        for (name, bytes) in files {
            sums.insert(name.clone(), sha256_hex(&bytes));
            std::fs::write(tmp.join(&name), bytes).map_err(|e| e.to_string())?;
        }
        Ok((sums, manifest))
    })();

    let (sums, manifest) = match staged {
        Ok(ok) => ok,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };

    let dest = root.join(id);
    let old = root.join(format!(".old-patch-{id}"));
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
        return Err(format!("热更新换名失败 (原版本保持可用): {e}"));
    }
    let _ = std::fs::remove_dir_all(&old);

    let mut reg = load_registry(root);
    reg.removed.remove(id);
    reg.plugins.insert(
        id.to_string(),
        Entry {
            version: manifest.version.clone(),
            repo: manifest.repo.clone(),
            enabled: true,
            permissions: manifest.permissions.clone(),
            sha256: sums.clone(),
        },
    );
    save_registry(root, &reg)?;
    Ok(Installed {
        manifest,
        enabled: true,
        source: "installed",
        sha256: sums,
    })
}

// ---------- 插件包 (.zip) ----------
// Release 里的 `plugin-<id>-<版本>.zip`: 里面是 manifest.json + 入口脚本 (+ 样式 / 许可证 / 依赖脚本), 可以放在一层顶层目录里。
// 和从 GitHub 安装走同一套兼容检查和权限确认; 先 inspect_package 给用户看, 同意后才 install_package。

#[derive(Debug, Clone, Serialize)]
pub struct PackageInspection {
    pub report: CompatReport,
    /// 各文件的 SHA-256 (确认框里显示)
    pub files: BTreeMap<String, String>,
    pub installed_version: Option<String>,
    /// 这个 id 已经被别的来源占用 (内置 / 已装, 仓库不同) 时的说明; 有值就不能装
    pub conflict: Option<String>,
}

const MAX_PACKAGE_BYTES: usize = 4 * MAX_ASSET_BYTES as usize;
const MAX_PACKAGE_FILES: usize = 64;

fn unpack(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    use std::io::Read;
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err("插件包太大".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("不是有效的 zip 插件包: {e}"))?;
    if zip.len() > MAX_PACKAGE_FILES {
        return Err(format!("插件包里文件太多 (> {MAX_PACKAGE_FILES})"));
    }
    let mut raw: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total = 0usize;
    for i in 0..zip.len() {
        let f = zip
            .by_index(i)
            .map_err(|e| format!("读取插件包失败: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        if name.starts_with('/')
            || name.contains(':')
            || name.split('/').any(|p| p == ".." || p.is_empty())
        {
            return Err(format!("插件包里有不安全的路径: {name}"));
        }
        let mut buf = Vec::new();
        f.take(MAX_ASSET_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("解压 {name} 失败: {e}"))?;
        if buf.len() as u64 > MAX_ASSET_BYTES {
            return Err(format!("{name} 超过大小上限"));
        }
        total += buf.len();
        if total > MAX_PACKAGE_BYTES {
            return Err("插件包解压后太大".into());
        }
        raw.push((name, buf));
    }
    // 允许所有文件都在同一层顶层目录里 (去掉它)
    let top: Option<&str> = raw
        .first()
        .and_then(|(n, _)| n.split_once('/').map(|(d, _)| d));
    let strip = top.filter(|d| {
        raw.iter().all(|(n, _)| {
            n.strip_prefix(*d)
                .map(|r| r.starts_with('/'))
                .unwrap_or(false)
        })
    });
    let mut files = BTreeMap::new();
    for (n, b) in &raw {
        let rest = match strip {
            Some(d) => &n[d.len() + 1..],
            None => n.as_str(),
        };
        if !valid_file_name(rest) || !windows_safe_name(rest) {
            return Err(format!(
                "插件包里的文件名不合法 (只允许一层, 不能是设备名或以点 / 空格结尾): {n}"
            ));
        }
        // 大小写不同的同名文件在 NTFS 上是同一个文件: 会互相覆盖而登记的哈希对不上, 直接拒绝
        if files.keys().any(|k: &String| k.eq_ignore_ascii_case(rest)) {
            return Err(format!("插件包里有只差大小写的重名文件: {n}"));
        }
        files.insert(rest.to_string(), b.clone());
    }
    Ok(files)
}

fn package_conflict(root: &Path, m: &Manifest) -> Option<String> {
    let owner = load_registry(root)
        .plugins
        .get(&m.id)
        .map(|e| e.repo.clone())
        .or_else(|| bundled_manifest(&m.id).map(|b| b.repo));
    let o = owner.filter(|o| !o.is_empty())?;
    let same = |a: &str, b: &str| match (parse_repo(a), parse_repo(b)) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(&y),
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    };
    (!same(&o, &m.repo)).then(|| {
        format!(
            "插件 id「{}」属于仓库 {o}, 这个包声明的仓库是「{}」, 不能顶替",
            m.id, m.repo
        )
    })
}

pub fn inspect_package(
    root: &Path,
    bytes: &[u8],
    app_version: &str,
) -> Result<PackageInspection, String> {
    let files = unpack(bytes)?;
    let assets: Vec<Asset> = files
        .iter()
        .map(|(n, b)| Asset {
            name: n.clone(),
            size: b.len() as u64,
        })
        .collect();
    let manifest_text = files
        .get("manifest.json")
        .and_then(|b| std::str::from_utf8(b).ok());
    let report = check_compat(&assets, manifest_text, app_version);
    let installed_version = report.manifest.as_ref().and_then(|m| {
        list(root)
            .into_iter()
            .find(|p| p.manifest.id == m.id)
            .map(|p| p.manifest.version)
    });
    let conflict = report
        .manifest
        .as_ref()
        .and_then(|m| package_conflict(root, m));
    Ok(PackageInspection {
        report,
        files: files
            .iter()
            .map(|(n, b)| (n.clone(), sha256_hex(b)))
            .collect(),
        installed_version,
        conflict,
    })
}

/// 安装插件包。`approved` 必须覆盖清单声明的全部权限; 原子换名, 失败保留旧版本
pub fn install_package(
    root: &Path,
    bytes: &[u8],
    app_version: &str,
    approved: &[String],
) -> Result<Installed, String> {
    let ins = inspect_package(root, bytes, app_version)?;
    if !ins.report.ok {
        let why: Vec<String> = ins
            .report
            .problems
            .iter()
            .map(|p| format!("{}: {}", p.code, p.message))
            .collect();
        return Err(format!(
            "插件与当前编辑器不匹配, 未安装: {}",
            why.join("; ")
        ));
    }
    if let Some(c) = ins.conflict {
        return Err(c);
    }
    let m = ins.report.manifest.clone().ok_or("缺少清单")?;
    // 低于程序里内置版本的包装上去也不会生效 (list() 只在已装版本 >= 内置版本时才用已装的): 明说, 不假装装好了
    if let Some(b) = bundled_manifest(&m.id) {
        if is_newer(&b.version, &m.version) {
            return Err(format!(
                "这个包的版本 {} 低于程序内置的 {}, 装上也不会生效; 要用内置版请「恢复内置」",
                m.version, b.version
            ));
        }
    }
    if let Some(p) = m.permissions.iter().find(|p| !approved.contains(p)) {
        return Err(format!("权限「{p}」未获用户同意, 未安装"));
    }
    let files = unpack(bytes)?;
    let tmp = root.join(format!(".tmp-{}", m.id));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let mut sums = BTreeMap::new();
    for (n, b) in &files {
        if let Err(e) = std::fs::write(tmp.join(n), b) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e.to_string());
        }
        sums.insert(n.clone(), sha256_hex(b));
    }
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
    let mut reg = load_registry(root);
    let enabled = reg.plugins.get(&m.id).map(|e| e.enabled).unwrap_or(true);
    reg.removed.remove(&m.id);
    reg.plugins.insert(
        m.id.clone(),
        Entry {
            version: m.version.clone(),
            repo: m.repo.clone(),
            enabled,
            permissions: approved.to_vec(),
            sha256: sums.clone(),
        },
    );
    if let Err(e) = save_registry(root, &reg) {
        // 登记写不进去 (磁盘满 / 被杀毒锁住): 把旧版本换回来, 不留下"文件是新的、登记是旧的"的状态
        let _ = std::fs::remove_dir_all(&dest);
        if had_old {
            let _ = std::fs::rename(&old, &dest);
        }
        return Err(format!("安装失败 (登记写入失败, 旧版本保持可用): {e}"));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(Installed {
        manifest: m,
        enabled,
        source: "installed",
        sha256: sums,
    })
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
    let ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (it.next().is_none() && ok(o) && ok(n)).then(|| format!("{o}/{n}"))
}

pub fn fetch_latest(http: &dyn Http, repo: &str) -> Result<ReleaseInfo, String> {
    let repo =
        parse_repo(repo).ok_or_else(|| format!("仓库地址不合法: {repo} (应为 owner/name)"))?;
    let body = http.get(&format!(
        "https://api.github.com/repos/{repo}/releases/latest"
    ))?;
    let v: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| format!("Release 响应无法解析: {e}"))?;
    let tag = v["tag_name"]
        .as_str()
        .ok_or("仓库还没有 Release")?
        .to_string();
    let assets = v["assets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(ReleaseAsset {
                        name: x["name"].as_str()?.to_string(),
                        size: x["size"].as_u64().unwrap_or(0),
                        url: x["browser_download_url"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
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
pub fn inspect(
    http: &dyn Http,
    root: &Path,
    repo: &str,
    app_version: &str,
) -> Result<Inspection, String> {
    inspect_with_manifest(http, root, repo, app_version).map(|(i, _)| i)
}

/// 同 [`inspect`], 并返回被校验的那份 manifest.json 的**原始字节**: 安装时写入这份, 不再重新下载 (防止校验后被替换)
fn inspect_with_manifest(
    http: &dyn Http,
    root: &Path,
    repo: &str,
    app_version: &str,
) -> Result<(Inspection, Option<Vec<u8>>), String> {
    let release = fetch_latest(http, repo)?;
    let assets: Vec<Asset> = release
        .assets
        .iter()
        .map(|a| Asset {
            name: a.name.clone(),
            size: a.size,
        })
        .collect();
    let manifest_bytes = release
        .assets
        .iter()
        .find(|a| a.name == "manifest.json")
        .and_then(|a| http.get(&a.url).ok());
    let manifest_text = manifest_bytes
        .as_ref()
        .and_then(|b| std::str::from_utf8(b).ok());
    let report = check_compat(&assets, manifest_text, app_version);
    let installed_version = report.manifest.as_ref().and_then(|m| {
        list(root)
            .into_iter()
            .find(|p| p.manifest.id == m.id)
            .map(|p| p.manifest.version)
    });
    let update_available = match (&report.manifest, &installed_version) {
        (Some(m), Some(v)) => is_newer(&m.version, v),
        _ => false,
    };
    Ok((
        Inspection {
            release,
            report,
            installed_version,
            update_available,
        },
        manifest_bytes,
    ))
}

/// 安装 / 更新到最新 Release。`approved` = 用户在确认框里同意的权限, 必须覆盖清单声明的全部权限
pub fn install(
    http: &dyn Http,
    root: &Path,
    repo: &str,
    app_version: &str,
    approved: &[String],
) -> Result<Installed, String> {
    let (ins, manifest_bytes) = inspect_with_manifest(http, root, repo, app_version)?;
    if !ins.report.ok {
        let why: Vec<String> = ins
            .report
            .problems
            .iter()
            .map(|p| format!("{}: {}", p.code, p.message))
            .collect();
        return Err(format!(
            "插件与当前编辑器不匹配, 未安装: {}",
            why.join("; ")
        ));
    }
    let m = ins.report.manifest.clone().ok_or("缺少清单")?;
    let manifest_bytes = manifest_bytes.ok_or("缺少清单")?;
    // 身份: 插件 id 绑定到安装它的仓库 —— 另一个仓库不能顶替已有 id (包括随程序打包的默认插件); 清单里自称的 repo 不作数
    let owner = load_registry(root)
        .plugins
        .get(&m.id)
        .map(|e| e.repo.clone())
        .or_else(|| bundled_manifest(&m.id).map(|b| b.repo));
    if let Some(o) = owner {
        if parse_repo(&o).as_deref() != Some(ins.release.repo.as_str()) {
            return Err(format!(
                "插件 id「{}」属于仓库 {o}, 不能被 {} 顶替",
                m.id, ins.release.repo
            ));
        }
    }
    if let Some(p) = m.permissions.iter().find(|p| !approved.contains(p)) {
        return Err(format!("权限「{p}」未获用户同意, 未安装"));
    }
    // 下载全部需要的资产到临时目录
    let tmp = root.join(format!(".tmp-{}", m.id));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| -> Result<BTreeMap<String, String>, String> {
        let mut sums = BTreeMap::new();
        // 清单用校验过的那份字节, 其余资产才下载
        sums.insert("manifest.json".to_string(), sha256_hex(&manifest_bytes));
        std::fs::write(tmp.join("manifest.json"), &manifest_bytes).map_err(|e| e.to_string())?;
        for a in ins.release.assets.iter().filter(|a| {
            a.name != "manifest.json"
                && (a.name == m.entry
                    || m.scripts.contains(&a.name)
                    || a.name == "styles.css"
                    || a.name == "LICENSE"
                    || a.name.ends_with(".js"))
        }) {
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
    reg.removed.remove(&m.id);
    reg.plugins.insert(
        m.id.clone(),
        Entry {
            version: m.version.clone(),
            repo: ins.release.repo.clone(),
            enabled,
            permissions: approved.to_vec(),
            sha256: sums.clone(),
        },
    );
    save_registry(root, &reg)?;
    Ok(Installed {
        manifest: m,
        enabled,
        source: "installed",
        sha256: sums,
    })
}

/// 对每个已装插件 (有 repo 的) 检查是否有新版本 —— 用户点"检查更新"时才调用
pub fn check_updates(
    http: &dyn Http,
    root: &Path,
    app_version: &str,
) -> Vec<(String, Result<Inspection, String>)> {
    list(root)
        .into_iter()
        .filter(|p| !p.manifest.repo.is_empty())
        .map(|p| {
            (
                p.manifest.id.clone(),
                inspect(http, root, &p.manifest.repo, app_version),
            )
        })
        .collect()
}

/// 数据目录: IDE_PLUGIN_DIR 优先 (测试 / 便携版), 否则 %APPDATA%/IDE1.0/plugins (非 Windows: ~/.local/share)
pub fn default_root() -> PathBuf {
    if let Ok(p) = std::env::var("IDE_PLUGIN_DIR") {
        return PathBuf::from(p);
    }
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|_| PathBuf::from("."));
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
        let assets: Vec<serde_json::Value> = files
            .iter()
            .map(|(n, c)| {
                let url = format!("https://dl.example/{repo}/{tag}/{n}");
                m.insert(url.clone(), c.as_bytes().to_vec());
                serde_json::json!({ "name": n, "size": c.len(), "browser_download_url": url })
            })
            .collect();
        m.insert(
            format!("https://api.github.com/repos/{repo}/releases/latest"),
            serde_json::to_vec(&serde_json::json!({ "tag_name": tag, "assets": assets })).unwrap(),
        );
        Fake(m, RefCell::new(vec![]))
    }
    const MANI: &str = r#"{"id":"hello","name":"Hello","version":"1.0.0","apiVersion":1,"repo":"o/hello","entry":"main.js","permissions":["storage"],"contributes":{"command":[{"id":"hi"}]}}"#;

    #[test]
    fn bundled_pele_is_listed_enabled_and_readable() {
        let root = tmp("bundled");
        let l = list(&root);
        let p = l
            .iter()
            .find(|p| p.manifest.id == "pele")
            .expect("默认插件 pele");
        assert!(p.enabled && p.source == "bundled");
        assert!(read_asset(&root, "pele", "main.js").unwrap().len() > 10);
        assert!(read_asset(&root, "pele", "../x").is_err());
        set_enabled(&root, "pele", false).unwrap();
        assert!(
            !list(&root)
                .iter()
                .find(|p| p.manifest.id == "pele")
                .unwrap()
                .enabled,
            "禁用记在 registry 里"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_bundled_plugin_passes_the_compat_check_it_would_get_from_github() {
        let ids = bundled_ids();
        assert!(ids.contains(&"pele") && ids.contains(&"doc-graph") && ids.contains(&"ppa"));
        for id in ids {
            let assets: Vec<Asset> = BUNDLED
                .iter()
                .filter(|(i, _, _)| *i == id)
                .map(|(_, f, b)| Asset {
                    name: f.to_string(),
                    size: b.len() as u64,
                })
                .collect();
            let manifest = std::str::from_utf8(bundled_file(id, "manifest.json").unwrap()).unwrap();
            let r = check_compat(&assets, Some(manifest), env!("CARGO_PKG_VERSION"));
            assert!(r.ok, "{id}: {:?}", r.problems);
        }
    }

    #[test]
    fn parse_repo_forms() {
        assert_eq!(
            parse_repo("obsidianmd/pele").as_deref(),
            Some("obsidianmd/pele")
        );
        assert_eq!(
            parse_repo("https://github.com/obsidianmd/pele.git/").as_deref(),
            Some("obsidianmd/pele")
        );
        assert!(
            parse_repo("a/b/c").is_none()
                && parse_repo("..").is_none()
                && parse_repo("a b/c").is_none()
        );
    }

    #[test]
    fn install_requires_approved_permissions_then_updates_atomically() {
        let root = tmp("install");
        let h = release(
            "o/hello",
            "v1.0.0",
            &[("manifest.json", MANI), ("main.js", "ide.register({})")],
        );
        assert!(install(&h, &root, "o/hello", "0.1.2", &[])
            .unwrap_err()
            .contains("未获用户同意"));
        assert!(!root.join("hello").exists(), "没同意就不落盘");
        let p = install(&h, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        assert_eq!(p.manifest.version, "1.0.0");
        assert_eq!(p.sha256["main.js"], sha256_hex(b"ide.register({})"));
        assert_eq!(
            read_asset(&root, "hello", "main.js").unwrap(),
            b"ide.register({})"
        );
        // 发新版: 能检测到更新, 一键更新后换成新版
        let h2 = release(
            "o/hello",
            "v1.1.0",
            &[
                ("manifest.json", &MANI.replace("1.0.0", "1.1.0")),
                ("main.js", "ide.register({v:2})"),
            ],
        );
        let ins = inspect(&h2, &root, "o/hello", "0.1.2").unwrap();
        assert!(ins.update_available && ins.installed_version.as_deref() == Some("1.0.0"));
        install(&h2, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        assert_eq!(
            read_asset(&root, "hello", "main.js").unwrap(),
            b"ide.register({v:2})"
        );
        assert!(
            !root.join(".old-hello").exists() && !root.join(".tmp-hello").exists(),
            "不留中间目录"
        );
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
        assert!(
            err.contains("不匹配") && err.contains("NO_MANIFEST"),
            "{err}"
        );
        assert!(!root.join("pele").exists());
        // 入口缺失
        let h = release("o/hello", "v1.0.0", &[("manifest.json", MANI)]);
        assert!(install(&h, &root, "o/hello", "0.1.2", &["storage".into()])
            .unwrap_err()
            .contains("NO_ENTRY"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn id_is_bound_to_its_repo_and_manifest_is_not_refetched() {
        let root = tmp("identity");
        // 另一个仓库不能顶替随程序打包的 pele
        let evil = MANI
            .replace("\"id\":\"hello\"", "\"id\":\"pele\"")
            .replace("o/hello", "evil/pele");
        let h = release(
            "evil/pele",
            "v9.0.0",
            &[("manifest.json", &evil), ("main.js", "x")],
        );
        let e = install(&h, &root, "evil/pele", "0.1.2", &["storage".into()]).unwrap_err();
        assert!(e.contains("不能被") && !root.join("pele").exists(), "{e}");
        // 已装的 hello 不能被别的仓库顶替
        let h = release(
            "o/hello",
            "v1.0.0",
            &[("manifest.json", MANI), ("main.js", "a")],
        );
        install(&h, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        let h2 = release(
            "x/other",
            "v1.0.0",
            &[
                ("manifest.json", &MANI.replace("o/hello", "x/other")),
                ("main.js", "b"),
            ],
        );
        assert!(install(&h2, &root, "x/other", "0.1.2", &["storage".into()])
            .unwrap_err()
            .contains("不能被"));
        assert_eq!(read_asset(&root, "hello", "main.js").unwrap(), b"a");
        // 安装只取一次 manifest.json: 校验过的字节就是落盘的字节
        let h3 = release(
            "o/hello",
            "v1.1.0",
            &[
                ("manifest.json", &MANI.replace("1.0.0", "1.1.0")),
                ("main.js", "c"),
            ],
        );
        install(&h3, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        let n =
            h3.1.borrow()
                .iter()
                .filter(|u| u.ends_with("/manifest.json"))
                .count();
        assert_eq!(n, 1, "manifest.json 只下载一次");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn failed_download_keeps_old_version() {
        let root = tmp("rollback");
        let h = release(
            "o/hello",
            "v1.0.0",
            &[("manifest.json", MANI), ("main.js", "old")],
        );
        install(&h, &root, "o/hello", "0.1.2", &["storage".into()]).unwrap();
        let mut h2 = release(
            "o/hello",
            "v1.1.0",
            &[
                ("manifest.json", &MANI.replace("1.0.0", "1.1.0")),
                ("main.js", "new"),
            ],
        );
        h2.0.remove("https://dl.example/o/hello/v1.1.0/main.js"); // 入口下载 404
        assert!(install(&h2, &root, "o/hello", "0.1.2", &["storage".into()]).is_err());
        assert_eq!(
            read_asset(&root, "hello", "main.js").unwrap(),
            b"old",
            "旧版本保持可用"
        );
        assert!(!root.join(".tmp-hello").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 造一个 zip (测试用): files = (包内路径, 内容)
    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opt = zip::write::SimpleFileOptions::default();
        for (n, c) in files {
            w.start_file(*n, opt).unwrap();
            w.write_all(c).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn bundled_plugin_can_be_deleted_and_comes_back_from_a_package() {
        let root = tmp("pkg-bundled");
        assert!(
            list(&root).iter().any(|p| p.manifest.id == "ppa"),
            "内置 ppa 默认就在"
        );
        remove(&root, "ppa").unwrap();
        assert!(
            !list(&root).iter().any(|p| p.manifest.id == "ppa"),
            "删掉后不再列出"
        );
        assert!(
            removed_bundled(&root).iter().any(|(id, _, _)| id == "ppa"),
            "记在已删除的内置插件里"
        );
        // 用 Release 里的插件包 (就是打包的那份文件) 装回来
        let pkg = make_zip(&[
            (
                "manifest.json",
                bundled_file("ppa", "manifest.json").unwrap(),
            ),
            ("main.js", bundled_file("ppa", "main.js").unwrap()),
        ]);
        let ins = inspect_package(&root, &pkg, "0.1.7").unwrap();
        assert!(
            ins.report.ok && ins.conflict.is_none() && ins.files.contains_key("main.js"),
            "{:?}",
            ins.report.problems
        );
        // 没同意权限 -> 不装
        assert!(install_package(&root, &pkg, "0.1.7", &[])
            .unwrap_err()
            .contains("未获用户同意"));
        let got = install_package(
            &root,
            &pkg,
            "0.1.7",
            &["editor.read".into(), "editor.write".into()],
        )
        .unwrap();
        assert_eq!(got.manifest.id, "ppa");
        assert!(
            list(&root)
                .iter()
                .any(|p| p.manifest.id == "ppa" && p.source == "installed"),
            "装回来了"
        );
        assert!(removed_bundled(&root).is_empty());
        assert!(read_asset(&root, "ppa", "main.js").unwrap().len() > 100);
        // 再删, 再用「恢复内置」回来
        remove(&root, "ppa").unwrap();
        restore_bundled(&root, "ppa").unwrap();
        assert!(list(&root)
            .iter()
            .any(|p| p.manifest.id == "ppa" && p.source == "bundled"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn package_install_is_strict() {
        let root = tmp("pkg-strict");
        #[allow(clippy::no_effect_replace)]
        let m = MANI.replace("o/hello", "o/hello");
        // 一层顶层目录可以; 全新 id 的包可以装
        let ok = make_zip(&[
            ("hello/manifest.json", m.as_bytes()),
            ("hello/main.js", b"a"),
        ]);
        install_package(&root, &ok, "0.1.7", &["storage".into()]).unwrap();
        assert_eq!(read_asset(&root, "hello", "main.js").unwrap(), b"a");
        // 路径穿越 / 多层目录 / 缺清单 / 不是 zip / 仓库不一致顶替
        assert!(inspect_package(
            &root,
            &make_zip(&[("../evil.js", b"x"), ("manifest.json", m.as_bytes())]),
            "0.1.7"
        )
        .is_err());
        assert!(inspect_package(&root, &make_zip(&[("a/b/main.js", b"x")]), "0.1.7").is_err());
        let nomani = inspect_package(&root, &make_zip(&[("main.js", b"x")]), "0.1.7").unwrap();
        assert!(!nomani.report.ok);
        assert!(inspect_package(&root, b"not a zip", "0.1.7").is_err());
        let other = make_zip(&[
            ("manifest.json", m.replace("o/hello", "x/other").as_bytes()),
            ("main.js", b"b"),
        ]);
        let c = inspect_package(&root, &other, "0.1.7").unwrap();
        assert!(c.conflict.is_some());
        assert!(install_package(&root, &other, "0.1.7", &["storage".into()])
            .unwrap_err()
            .contains("不能顶替"));
        assert_eq!(
            read_asset(&root, "hello", "main.js").unwrap(),
            b"a",
            "被拒绝后原版本不变"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn package_review_fixes() {
        let root = tmp("pkg-review");
        let m = MANI;
        // Windows 设备名 / 结尾的点 / 只差大小写的重名都拒绝
        for bad in ["nul", "con.js", "aux.txt", "com1", "main.js.", "x?.js"] {
            assert!(
                inspect_package(
                    &root,
                    &make_zip(&[("manifest.json", m.as_bytes()), (bad, b"x")]),
                    "0.1.7"
                )
                .is_err(),
                "{bad}"
            );
        }
        assert!(inspect_package(
            &root,
            &make_zip(&[
                ("manifest.json", m.as_bytes()),
                ("main.js", b"a"),
                ("MAIN.JS", b"b")
            ]),
            "0.1.7"
        )
        .is_err());
        // 低于内置版本的包: 明说装不上
        let mani = String::from_utf8(bundled_file("ppa", "manifest.json").unwrap().to_vec())
            .unwrap()
            .replace("\"version\": \"0.1.0\"", "\"version\": \"0.0.1\"");
        let low = make_zip(&[("manifest.json", mani.as_bytes()), ("main.js", b"x")]);
        assert!(install_package(
            &root,
            &low,
            "0.1.7",
            &["editor.read".into(), "editor.write".into()]
        )
        .unwrap_err()
        .contains("低于程序内置"));
        // restore_bundled 不留装过的孤儿目录
        let pkg = make_zip(&[
            (
                "manifest.json",
                bundled_file("ppa", "manifest.json").unwrap(),
            ),
            ("main.js", bundled_file("ppa", "main.js").unwrap()),
        ]);
        remove(&root, "ppa").unwrap();
        install_package(
            &root,
            &pkg,
            "0.1.7",
            &["editor.read".into(), "editor.write".into()],
        )
        .unwrap();
        assert!(root.join("ppa").exists());
        restore_bundled(&root, "ppa").unwrap();
        assert!(!root.join("ppa").exists(), "孤儿目录被清掉");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn hot_patch_plugin_script_overrides_and_restore_reverts() {
        let root = tmp("nema-hotpatch");
        let init = get(&root, "nema-interpreter").expect("nema-interpreter bundled");
        assert_eq!(init.source, "bundled");
        let patched_js = b"/* custom hotpatch */\n(function(r){r.NemaStrict={parse:()=>[],fields:()=>[]};})(globalThis);";
        let ins =
            hot_patch_plugin_script(&root, "nema-interpreter", "strict-parser.js", patched_js)
                .unwrap();
        assert_eq!(ins.source, "installed");
        assert_eq!(
            read_asset(&root, "nema-interpreter", "strict-parser.js").unwrap(),
            patched_js
        );
        assert!(read_asset(&root, "nema-interpreter", "main.js").is_ok());

        // Invalid script name or undeclared file must fail cleanly without corrupting state
        assert!(hot_patch_plugin_script(&root, "nema-interpreter", "undeclared.js", b"1").is_err());
        assert!(hot_patch_plugin_script(&root, "nema-interpreter", "../evil.js", b"1").is_err());
        assert_eq!(
            read_asset(&root, "nema-interpreter", "strict-parser.js").unwrap(),
            patched_js
        );

        restore_bundled(&root, "nema-interpreter").unwrap();
        let restored = get(&root, "nema-interpreter").unwrap();
        assert_eq!(restored.source, "bundled");
        let bundled_js = read_asset(&root, "nema-interpreter", "strict-parser.js").unwrap();
        assert!(String::from_utf8_lossy(&bundled_js).contains("MALFORMED_HEADER"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bundled_hot_plug_modules_can_be_toggled_deleted_and_restored() {
        let root = tmp("hotplug-mods");
        let ids = ["mcp", "hooks", "skills", "mod", "automation"];

        // 1. 默认存在且全部启用
        let init_list = list(&root);
        for id in ids {
            let p = init_list
                .iter()
                .find(|p| p.manifest.id == id)
                .unwrap_or_else(|| panic!("默认应包含内置热插拔模块 {id}"));
            assert!(p.enabled, "{id} 默认应为启用状态");
            assert_eq!(p.source, "bundled");
        }

        // 2. 可逐个关闭并立即在 get/list 中生效
        for id in ids {
            set_enabled(&root, id, false).unwrap();
            assert!(!get(&root, id).unwrap().enabled, "{id} 应可关闭");
            set_enabled(&root, id, true).unwrap();
            assert!(get(&root, id).unwrap().enabled, "{id} 应可重新启用");
        }

        // 3. 可在插件页面删除，并记入 removed_bundled，随后可一键恢复
        for id in ids {
            remove(&root, id).unwrap();
            assert!(
                !list(&root).iter().any(|p| p.manifest.id == id),
                "删除后不再出现在活跃列表: {id}"
            );
            assert!(
                removed_bundled(&root).iter().any(|(rid, _, _)| rid == id),
                "删除后应可从已删除内置列表找回: {id}"
            );

            restore_bundled(&root, id).unwrap();
            let restored = get(&root, id).unwrap();
            assert_eq!(restored.source, "bundled");
            assert!(restored.enabled);
        }

        assert!(removed_bundled(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
