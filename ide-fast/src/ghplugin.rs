//! 从 GitHub Release 安装 / 更新插件 (只在用户明确点击时联网)。
//! 下载和校验都复用 editcore 的插件仓库 (`plugin_host::store::{inspect, install, check_updates}`): 清单与资产先做兼容性检查,
//! 权限须由用户在确认框里同意,插件 id 绑定到安装它的仓库,新旧版本原子换名。这里只补一个更严的 HTTP 客户端和一个后台线程外壳:
//! - 只允许 https,**重定向也只许 https** (`--proto-redir`),大小有上限,总时长有上限;
//! - 用系统目录里的 `curl.exe` (不从当前目录 / PATH 里随便找),找不到时说清楚;
//! - 联网在用完即退的线程里做,界面只轮询结果。

use editcore::plugin_host::store::{self, Http};
use editcore::plugin_host::MAX_ASSET_BYTES;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

/// 单次下载的字节上限 (清单 / 脚本 / API 响应都远小于这个)
pub const MAX_DOWNLOAD: u64 = MAX_ASSET_BYTES;

/// 更严的 curl 客户端: https 限定 (含重定向)、大小 / 时间有上限
pub struct StrictCurl;

impl StrictCurl {
    /// 传给 curl 的参数 (拆出来方便测试)
    pub fn args(url: &str) -> Vec<String> {
        [
            "-sS",
            "-L",
            "--fail",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-redirs",
            "5",
            "--connect-timeout",
            "5",
            "--max-time",
            "30",
            "--max-filesize",
        ]
        .iter()
        .map(|s| s.to_string())
        .chain([
            MAX_DOWNLOAD.to_string(),
            "-H".into(),
            "Accept: application/vnd.github+json".into(),
            "-H".into(),
            "User-Agent: IDE1.0-fast-plugin".into(),
            "--".into(),
            url.to_string(),
        ])
        .collect()
    }
}

impl Http for StrictCurl {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        if !url.starts_with("https://") || url.contains(['\r', '\n']) {
            return Err(format!("只允许 https 地址: {url}"));
        }
        let mut cmd = crate::curlx::curl_command();
        cmd.args(Self::args(url));
        let out = cmd
            .output()
            .map_err(|e| format!("无法运行 curl ({e}): 请确认系统自带 curl 可用"))?;
        if !out.status.success() {
            return Err(format!(
                "下载失败 {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        if out.stdout.len() as u64 > MAX_DOWNLOAD {
            return Err(format!("响应超过大小上限: {url}"));
        }
        Ok(out.stdout)
    }
}

/// 检查结果 (给界面显示)
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub repo: String,
    pub tag: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub installed: Option<String>,
    pub update: bool,
    pub perms: Vec<String>,
    /// 兼容性问题 (空 = 可以安装)
    pub problems: Vec<String>,
}

impl Found {
    pub fn installable(&self) -> bool {
        self.problems.is_empty() && !self.id.is_empty()
    }
}

fn found_of(ins: store::Inspection) -> Found {
    let m = ins.report.manifest.clone();
    Found {
        repo: ins.release.repo.clone(),
        tag: ins.release.tag.clone(),
        id: m.as_ref().map(|m| m.id.clone()).unwrap_or_default(),
        name: m.as_ref().map(|m| m.name.clone()).unwrap_or_default(),
        version: m.as_ref().map(|m| m.version.clone()).unwrap_or_default(),
        installed: ins.installed_version,
        update: ins.update_available,
        perms: m.map(|m| m.permissions).unwrap_or_default(),
        problems: ins
            .report
            .problems
            .iter()
            .map(|p| format!("{}: {}", p.code, p.message))
            .collect(),
    }
}

/// 取最新 Release 并检查 (不安装)
pub fn inspect(http: &dyn Http, root: &std::path::Path, repo_input: &str) -> Result<Found, String> {
    store::inspect(http, root, repo_input, env!("CARGO_PKG_VERSION")).map(found_of)
}

/// 安装 / 更新到最新 Release; `approved` 必须覆盖清单声明的全部权限
pub fn install(
    http: &dyn Http,
    root: &std::path::Path,
    repo_input: &str,
    approved: &[String],
) -> Result<String, String> {
    store::install(http, root, repo_input, env!("CARGO_PKG_VERSION"), approved)
        .map(|i| format!("{} {}", i.manifest.name, i.manifest.version))
}

/// 每个已装插件 (有 repo 的) 查一次
pub fn check_updates(
    http: &dyn Http,
    root: &std::path::Path,
) -> Vec<(String, Result<Found, String>)> {
    store::check_updates(http, root, env!("CARGO_PKG_VERSION"))
        .into_iter()
        .map(|(id, r)| (id, r.map(found_of)))
        .collect()
}

/// 后台任务的结果
pub enum Done {
    Inspected(Result<Found, String>),
    Installed(Result<String, String>),
    Updates(Vec<(String, Result<Found, String>)>),
}

/// 起一条用完即退的线程做联网活儿,界面轮询返回的收件箱
pub fn spawn(
    root: PathBuf,
    job: impl FnOnce(&dyn Http, &std::path::Path) -> Done + Send + 'static,
) -> Receiver<Done> {
    let (tx, rx) = channel();
    let _ = std::thread::Builder::new()
        .name("plugin-net".into())
        .spawn(move || {
            let _ = tx.send(job(&StrictCurl, &root));
        });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Fake(HashMap<String, Vec<u8>>);
    impl Http for Fake {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.0.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
    }

    const MANI: &str = r#"{"id":"hello","name":"Hello","version":"1.2.0","apiVersion":1,"repo":"o/hello","entry":"main.js","permissions":["editor.read"],"contributes":{"command":[{"id":"hi"}]}}"#;

    fn release(tag: &str, files: &[(&str, &str)]) -> Fake {
        let mut m = HashMap::new();
        let assets: Vec<serde_json::Value> = files
            .iter()
            .map(|(n, c)| {
                let url = format!("https://dl.example/o/hello/{tag}/{n}");
                m.insert(url.clone(), c.as_bytes().to_vec());
                serde_json::json!({ "name": n, "size": c.len(), "browser_download_url": url })
            })
            .collect();
        m.insert(
            "https://api.github.com/repos/o/hello/releases/latest".into(),
            serde_json::to_vec(&serde_json::json!({ "tag_name": tag, "assets": assets })).unwrap(),
        );
        Fake(m)
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-fast-gh-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn inspect_then_install_needs_the_permissions_to_be_approved() {
        let root = tmp("ok");
        let http = release(
            "v1.2.0",
            &[("manifest.json", MANI), ("main.js", "ide.register({})")],
        );
        let f = inspect(&http, &root, "https://github.com/o/hello").unwrap();
        assert!(
            f.installable() && f.installed.is_none() && !f.update,
            "{f:?}"
        );
        assert_eq!(
            (f.id.as_str(), f.version.as_str(), f.perms.clone()),
            ("hello", "1.2.0", vec!["editor.read".to_string()])
        );
        // 没同意权限 -> 不装
        let err = install(&http, &root, "o/hello", &[]).unwrap_err();
        assert!(
            err.contains("editor.read") && !root.join("hello").exists(),
            "{err}"
        );
        let done = install(&http, &root, "o/hello", &f.perms).unwrap();
        assert_eq!(done, "Hello 1.2.0");
        assert!(
            root.join("hello").join("main.js").is_file()
                && root.join("hello").join("manifest.json").is_file()
        );
        // 装好之后:同版本没有更新,新 Release 有更新
        let again = inspect(&http, &root, "o/hello").unwrap();
        assert_eq!(
            (again.installed.as_deref(), again.update),
            (Some("1.2.0"), false)
        );
        let newer = release(
            "v1.3.0",
            &[
                ("manifest.json", &MANI.replace("1.2.0", "1.3.0")),
                ("main.js", "ide.register({})"),
            ],
        );
        let upd = check_updates(&newer, &root);
        let hello = upd
            .iter()
            .find(|(id, _)| id == "hello")
            .expect("已装插件被检查");
        assert!(hello.1.as_ref().unwrap().update, "{hello:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn incompatible_releases_are_reported_and_not_installed() {
        let root = tmp("bad");
        let http = release("v1", &[("manifest.json", MANI)]); // 缺入口脚本
        let f = inspect(&http, &root, "o/hello").unwrap();
        assert!(
            !f.installable() && f.problems.iter().any(|p| p.starts_with("NO_ENTRY")),
            "{f:?}"
        );
        assert!(
            install(&http, &root, "o/hello", &f.perms).is_err() && !root.join("hello").exists()
        );
        assert!(
            inspect(&http, &root, "not a repo!").is_err(),
            "仓库地址不合法"
        );
        assert!(
            inspect(&Fake(HashMap::new()), &root, "o/hello").is_err(),
            "网络错误原样报告"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_curl_only_speaks_https_and_caps_size_and_time() {
        assert!(StrictCurl
            .get("http://example.com/x")
            .unwrap_err()
            .contains("https"));
        assert!(StrictCurl.get("file:///C:/x").is_err());
        assert!(StrictCurl.get("https://a/b\nHost: evil").is_err());
        let a = StrictCurl::args("https://x/y");
        let pos = |k: &str| {
            a.iter()
                .position(|s| s == k)
                .unwrap_or_else(|| panic!("缺 {k}: {a:?}"))
        };
        assert_eq!(a[pos("--proto") + 1], "=https");
        assert_eq!(
            a[pos("--proto-redir") + 1],
            "=https",
            "重定向也不许降到 http"
        );
        assert_eq!(a[pos("--max-filesize") + 1], MAX_DOWNLOAD.to_string());
        assert_eq!(a[pos("--connect-timeout") + 1], "5");
        assert!(a.contains(&"--max-time".to_string()));
        assert_eq!(a[a.len() - 2], "--", "URL 在 -- 之后,不会被当成选项");
    }
}
