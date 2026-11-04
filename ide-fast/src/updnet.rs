//! 自更新的联网部分: 借系统 curl 取 Release 列表 / SHA256SUMS / 安装包, 再把「检查」和「下载 + 校验」两段流程串起来。
//! 约束沿用 `ghplugin::StrictCurl`: 只许 https (含重定向), 每一类请求都有大小和时间上限, 路径解析与不弹控制台走 `curlx`。
//! 只有测试构建才允许 `http://127.0.0.1:<端口>` (对着本机 mock 服务器跑完整流程); 发行版里这个口子不存在。
//! 下载只落到唯一命名的临时文件, **校验通过后**才换名成最终文件, 所以任何失败都不会留下半截 / 未校验的安装包。

use crate::update::{self, AssetInfo, Channel, HashError, InstallKind, Release, Source, Version};
use std::path::{Path, PathBuf};

/// 各类响应的字节上限
pub const MAX_API: u64 = 2 * 1024 * 1024;
pub const MAX_SUMS: u64 = 64 * 1024;
pub const MAX_BINARY: u64 = 120 * 1024 * 1024;
/// 各类请求的总时长上限 (秒)
const SECS_API: u64 = 30;
const SECS_BINARY: u64 = 900;

#[derive(Clone, PartialEq, Debug)]
pub enum UpdErr {
    Net(String),
    Parse(String),
    /// 声明 / 实际大小超过上限
    TooBig(String),
    /// 声明大小和实际下载到的不一致
    Size { name: String, want: u64, got: u64 },
    Hash(HashError),
    Io(String),
}

impl UpdErr {
    /// 给用户看的文字 (跟随界面语言)
    pub fn describe(&self) -> String {
        use crate::i18n::{tr, trf};
        match self {
            UpdErr::Net(e) => trf("upd.e-net", &[e]),
            UpdErr::Parse(e) => trf("upd.e-parse", &[e]),
            UpdErr::TooBig(n) => trf("upd.e-big", &[n]),
            UpdErr::Size { name, want, got } => trf("upd.e-size", &[name, &want.to_string(), &got.to_string()]),
            UpdErr::Hash(HashError::Missing(n)) => trf("upd.e-nosum", &[n]),
            UpdErr::Hash(HashError::Mismatch { name, .. }) => trf("upd.e-mismatch", &[name]),
            UpdErr::Io(e) => format!("{}: {e}", tr("upd.e-io")),
        }
    }
}

/// 取数据的接口: 真实实现是 curl, 逻辑测试用假的
pub trait Fetch {
    fn get(&self, url: &str, max: u64) -> Result<Vec<u8>, UpdErr>;
    fn download(&self, url: &str, dest: &Path, max: u64) -> Result<(), UpdErr>;
}

/// 这个地址用什么协议限定; None = 不许。`allow_local` 只在测试构建为真
pub fn proto_for(url: &str, allow_local: bool) -> Option<&'static str> {
    if url.contains(['\r', '\n']) {
        return None;
    }
    if url.starts_with("https://") {
        Some("=https")
    } else if allow_local && url.starts_with("http://127.0.0.1:") {
        Some("=http,https")
    } else {
        None
    }
}

/// 传给 curl 的参数 (拆出来给测试)。`out` = 写文件 (下载安装包), 否则输出到标准输出
pub fn curl_args(url: &str, max: u64, secs: u64, api: bool, out: Option<&Path>, allow_local: bool) -> Result<Vec<String>, UpdErr> {
    let proto = proto_for(url, allow_local).ok_or_else(|| UpdErr::Net(format!("https only: {url}")))?;
    let mut a: Vec<String> = ["-sS", "-L", "--fail", "--proto", proto, "--proto-redir", proto, "--max-redirs", "5", "--max-time"].iter().map(|s| s.to_string()).collect();
    a.extend([secs.to_string(), "--max-filesize".into(), max.to_string(), "-H".into(), "User-Agent: IDE1.0-fast-update".into()]);
    if api {
        a.extend(["-H".into(), "Accept: application/vnd.github+json".into()]);
    }
    if let Some(p) = out {
        a.extend(["-o".into(), p.display().to_string()]);
    }
    a.extend(["--".into(), url.to_string()]);
    Ok(a)
}

pub struct Curl;

impl Curl {
    fn run(&self, url: &str, max: u64, out: Option<&Path>) -> Result<std::process::Output, UpdErr> {
        let secs = if out.is_some() { SECS_BINARY } else { SECS_API };
        let api = out.is_none() && url.contains("api.github.com");
        let args = curl_args(url, max, secs, api, out, cfg!(test))?;
        let o = crate::curlx::curl_command().args(args).output().map_err(|e| UpdErr::Net(format!("curl: {e}")))?;
        if o.status.success() { Ok(o) } else { Err(UpdErr::Net(String::from_utf8_lossy(&o.stderr).trim().to_string())) }
    }
}

impl Fetch for Curl {
    fn get(&self, url: &str, max: u64) -> Result<Vec<u8>, UpdErr> {
        let o = self.run(url, max, None)?;
        if o.stdout.len() as u64 > max { Err(UpdErr::TooBig(url.to_string())) } else { Ok(o.stdout) }
    }
    fn download(&self, url: &str, dest: &Path, max: u64) -> Result<(), UpdErr> {
        self.run(url, max, Some(dest)).map(|_| ())
    }
}

/// 找到的新版本 (资产为 None = 这个平台 / 安装方式没有对应安装包, 界面只给页面链接)
#[derive(Clone, PartialEq, Debug)]
pub struct Candidate {
    pub release: Release,
    pub asset: Option<AssetInfo>,
    pub sums: Option<AssetInfo>,
    pub kind: InstallKind,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Outcome {
    UpToDate,
    Available(Box<Candidate>),
}

/// 检查: 取 Release 列表 -> 按通道选出比当前新的最高版本
pub fn check(f: &dyn Fetch, src: &Source, ch: Channel, current: &Version, kind: InstallKind) -> Result<Outcome, UpdErr> {
    let json = f.get(&src.api_url, MAX_API)?;
    let releases = update::parse_releases(&json, src).map_err(UpdErr::Parse)?;
    Ok(match update::select(&releases, ch, current) {
        None => Outcome::UpToDate,
        Some(r) => Outcome::Available(Box::new(Candidate { asset: update::pick_asset(r, kind).cloned(), sums: update::pick_sums(r).cloned(), release: r.clone(), kind })),
    })
}

/// 下载并校验好的安装包
#[derive(Clone, PartialEq, Debug)]
pub struct Staged {
    pub path: PathBuf,
    pub kind: InstallKind,
    pub sha256: String,
}

/// 下载 + 校验。顺序: 声明大小检查 -> 取 SHA256SUMS 并确认有这个文件的条目 (缺 = 拒绝, 此时还没下载) -> 下载到唯一临时文件
/// -> 实际大小 -> SHA-256。全部通过才成为最终文件: 便携版换名成 `new_exe` (`ide-fast.exe.new`), msi 留在临时目录里的唯一命名文件。
pub fn stage(f: &dyn Fetch, c: &Candidate, tmp_dir: &Path, new_exe: &Path) -> Result<Staged, UpdErr> {
    let (asset, sums) = match (&c.asset, &c.sums) {
        (Some(a), Some(s)) => (a, s),
        (Some(a), None) => return Err(UpdErr::Hash(HashError::Missing(a.name.clone()))),
        (None, _) => return Err(UpdErr::Io("no asset".into())),
    };
    if asset.size > MAX_BINARY || sums.size > MAX_SUMS {
        return Err(UpdErr::TooBig(asset.name.clone()));
    }
    let text = String::from_utf8_lossy(&f.get(&sums.url, MAX_SUMS)?).into_owned();
    let table = update::parse_sums(&text);
    if !table.contains_key(&asset.name) {
        return Err(UpdErr::Hash(HashError::Missing(asset.name.clone())));
    }
    let ext = if c.kind == InstallKind::Msi { "msi" } else { "tmp" };
    let tmp = crate::fsx::unique_temp(tmp_dir, "ide-fast-update", ext);
    let r = download_checked(f, asset, &table, &tmp);
    let sha = match r {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    };
    if c.kind == InstallKind::Msi {
        return Ok(Staged { path: tmp, kind: c.kind, sha256: sha });
    }
    let _ = std::fs::remove_file(new_exe); // 上次留下的旧 .new
    if let Err(e) = std::fs::rename(&tmp, new_exe) {
        let _ = std::fs::remove_file(&tmp);
        return Err(UpdErr::Io(e.to_string()));
    }
    Ok(Staged { path: new_exe.to_path_buf(), kind: c.kind, sha256: sha })
}

fn download_checked(f: &dyn Fetch, a: &AssetInfo, table: &std::collections::HashMap<String, String>, tmp: &Path) -> Result<String, UpdErr> {
    f.download(&a.url, tmp, MAX_BINARY)?;
    let got = std::fs::metadata(tmp).map_err(|e| UpdErr::Io(e.to_string()))?.len();
    if got > MAX_BINARY {
        return Err(UpdErr::TooBig(a.name.clone()));
    }
    if a.size > 0 && got != a.size {
        return Err(UpdErr::Size { name: a.name.clone(), want: a.size, got });
    }
    let bytes = std::fs::read(tmp).map_err(|e| UpdErr::Io(e.to_string()))?;
    let sha = editcore::plugin_host::sha256_hex(&bytes);
    update::verify_hash(table, &a.name, &sha).map_err(UpdErr::Hash)?;
    Ok(sha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::HashMap;

    /// 假的取数据端: 按地址返回内容, 记下调用次数
    struct Fake {
        files: HashMap<String, Vec<u8>>,
        gets: Cell<u32>,
        downloads: Cell<u32>,
    }
    impl Fake {
        fn new(files: &[(&str, &[u8])]) -> Fake {
            Fake { files: files.iter().map(|(u, b)| (u.to_string(), b.to_vec())).collect(), gets: Cell::new(0), downloads: Cell::new(0) }
        }
    }
    impl Fetch for Fake {
        fn get(&self, url: &str, _max: u64) -> Result<Vec<u8>, UpdErr> {
            self.gets.set(self.gets.get() + 1);
            self.files.get(url).cloned().ok_or_else(|| UpdErr::Net(format!("404 {url}")))
        }
        fn download(&self, url: &str, dest: &Path, _max: u64) -> Result<(), UpdErr> {
            self.downloads.set(self.downloads.get() + 1);
            let b = self.files.get(url).ok_or_else(|| UpdErr::Net(format!("404 {url}")))?;
            std::fs::write(dest, b).map_err(|e| UpdErr::Io(e.to_string()))
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-fast-upd-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const EXE: &[u8] = b"MZ new exe bytes";

    fn cand(name: &str, size: u64, with_sums: bool) -> Candidate {
        let asset = AssetInfo { name: name.into(), size, url: "https://x/a".into() };
        let sums = AssetInfo { name: update::SUMS_NAME.into(), size: 10, url: "https://x/sums".into() };
        let release = Release { tag: "v9.9.9".into(), version: Version::parse("9.9.9").unwrap(), prerelease: false, page: "https://x/page".into(), notes: String::new(), assets: vec![] };
        Candidate { release, asset: Some(asset), sums: with_sums.then_some(sums), kind: InstallKind::Portable }
    }

    fn sums_for(name: &str, bytes: &[u8]) -> Vec<u8> {
        format!("{}  {name}\n", editcore::plugin_host::sha256_hex(bytes)).into_bytes()
    }

    #[test]
    fn curl_args_pin_https_and_cap_size_and_time() {
        let a = curl_args("https://api.github.com/x", MAX_API, 30, true, None, false).unwrap();
        let pos = |k: &str| a.iter().position(|s| s == k).unwrap_or_else(|| panic!("缺 {k}: {a:?}"));
        assert_eq!(a[pos("--proto") + 1], "=https");
        assert_eq!(a[pos("--proto-redir") + 1], "=https", "重定向也不许降到 http");
        assert_eq!(a[pos("--max-filesize") + 1], MAX_API.to_string());
        assert_eq!(a[pos("--max-time") + 1], "30");
        assert_eq!(a[a.len() - 2], "--", "URL 在 -- 之后");
        assert!(a.iter().any(|s| s.starts_with("Accept:")) && !a.contains(&"-o".to_string()));
        let d = curl_args("https://github.com/o/r/a.exe", MAX_BINARY, 900, false, Some(Path::new("C:/t/x.tmp")), false).unwrap();
        assert!(!d.iter().any(|s| s.starts_with("Accept:")), "下载资产不带 API 的 Accept 头");
        assert_eq!(d[d.iter().position(|s| s == "-o").unwrap() + 1], "C:/t/x.tmp");
    }

    #[test]
    fn plain_http_is_refused_unless_local_test_mode() {
        for u in ["http://example.com/x", "ftp://a/b", "file:///C:/x", "https://a/b\nHost: evil", "http://127.0.0.1.evil.com/x", "http://localhost:80/x"] {
            assert!(curl_args(u, 1, 1, false, None, false).is_err(), "{u:?}");
            assert!(curl_args(u, 1, 1, false, None, true).is_err(), "测试口子也只认 127.0.0.1:端口: {u:?}");
        }
        assert!(curl_args("http://127.0.0.1:8080/x", 1, 1, false, None, false).is_err(), "发行版配置下本机 http 也不行");
        let a = curl_args("http://127.0.0.1:8080/x", 1, 1, false, None, true).unwrap();
        assert!(a.contains(&"=http,https".to_string()));
    }

    #[test]
    fn happy_path_writes_new_exe_only_after_verification() {
        let d = tmpdir("ok");
        let new_exe = d.join("ide-fast.exe.new");
        std::fs::write(&new_exe, b"stale").unwrap();
        let f = Fake::new(&[("https://x/a", EXE), ("https://x/sums", &sums_for("ide-fast-9.9.9-windows-x64.exe", EXE))]);
        let c = cand("ide-fast-9.9.9-windows-x64.exe", EXE.len() as u64, true);
        let st = stage(&f, &c, &d, &new_exe).unwrap();
        assert_eq!(st.path, new_exe);
        assert_eq!(std::fs::read(&new_exe).unwrap(), EXE, "旧 .new 被替换");
        assert_eq!(st.sha256, editcore::plugin_host::sha256_hex(EXE));
        let names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["ide-fast.exe.new"], "没有遗留临时文件: {names:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hash_mismatch_is_refused_and_leaves_nothing_behind() {
        let d = tmpdir("bad");
        let new_exe = d.join("ide-fast.exe.new");
        let f = Fake::new(&[("https://x/a", b"tampered!!!!!!!!"), ("https://x/sums", &sums_for("a.exe", EXE))]);
        let e = stage(&f, &cand("a.exe", 16, true), &d, &new_exe).unwrap_err();
        assert!(matches!(e, UpdErr::Hash(HashError::Mismatch { .. })), "{e:?}");
        assert!(!new_exe.exists());
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0, "临时文件已删");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_sums_entry_or_asset_is_refused_before_downloading() {
        let d = tmpdir("nosum");
        let new_exe = d.join("x.new");
        // SHA256SUMS 里没有这个文件
        let f = Fake::new(&[("https://x/a", EXE), ("https://x/sums", &sums_for("other.exe", EXE))]);
        let e = stage(&f, &cand("a.exe", EXE.len() as u64, true), &d, &new_exe).unwrap_err();
        assert_eq!(e, UpdErr::Hash(HashError::Missing("a.exe".into())));
        assert_eq!(f.downloads.get(), 0, "校验清单不全 = 根本不下载");
        // Release 没带 SHA256SUMS.txt
        let f = Fake::new(&[("https://x/a", EXE)]);
        assert!(matches!(stage(&f, &cand("a.exe", 1, false), &d, &new_exe), Err(UpdErr::Hash(HashError::Missing(_)))));
        assert_eq!((f.gets.get(), f.downloads.get()), (0, 0));
        assert!(!new_exe.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn size_caps_are_enforced_before_and_after_download() {
        let d = tmpdir("size");
        let new_exe = d.join("x.new");
        let f = Fake::new(&[("https://x/a", EXE), ("https://x/sums", &sums_for("a.exe", EXE))]);
        let e = stage(&f, &cand("a.exe", MAX_BINARY + 1, true), &d, &new_exe).unwrap_err();
        assert!(matches!(e, UpdErr::TooBig(_)));
        assert_eq!((f.gets.get(), f.downloads.get()), (0, 0), "声明就超限: 不联网");
        // 声明大小和实际不符
        let e = stage(&f, &cand("a.exe", EXE.len() as u64 + 5, true), &d, &new_exe).unwrap_err();
        assert!(matches!(e, UpdErr::Size { .. }), "{e:?}");
        assert!(!new_exe.exists() && std::fs::read_dir(&d).unwrap().count() == 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn msi_stays_in_the_temp_dir_with_a_unique_msi_name() {
        let d = tmpdir("msi");
        let f = Fake::new(&[("https://x/a", EXE), ("https://x/sums", &sums_for("IDE1.0-Fast_9.9.9.0_x64.msi", EXE))]);
        let mut c = cand("IDE1.0-Fast_9.9.9.0_x64.msi", EXE.len() as u64, true);
        c.kind = InstallKind::Msi;
        let st = stage(&f, &c, &d, &d.join("unused.new")).unwrap();
        assert!(st.path.starts_with(&d) && st.path.extension().unwrap() == "msi" && st.path.is_file());
        assert!(!d.join("unused.new").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn check_reports_up_to_date_and_errors() {
        let src = Source { api_url: "https://x/api".into(), trusted_prefix: "https://github.com/o/r/".into() };
        let cur = Version::parse("0.3.1").unwrap();
        let f = Fake::new(&[("https://x/api", b"[]")]);
        assert_eq!(check(&f, &src, Channel::Stable, &cur, InstallKind::Portable), Ok(Outcome::UpToDate));
        assert!(matches!(check(&Fake::new(&[]), &src, Channel::Stable, &cur, InstallKind::Portable), Err(UpdErr::Net(_))));
        assert!(matches!(check(&Fake::new(&[("https://x/api", b"nope")]), &src, Channel::Stable, &cur, InstallKind::Portable), Err(UpdErr::Parse(_))));
    }

    // ---- 本机 mock HTTP 服务器 + 真 curl, 跑完整流程 ----

    fn curl_works() -> bool {
        crate::curlx::curl_command().arg("--version").output().is_ok_and(|o| o.status.success())
    }

    /// 在已绑定的监听器上起一个只应答 GET 的 HTTP/1.1 服务器 (线程随测试进程结束)
    fn spawn_server(l: std::net::TcpListener, routes: HashMap<String, Vec<u8>>) {
        use std::io::{Read, Write};
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { continue };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (code, body) = match routes.get(&path) {
                    Some(b) => ("200 OK", b.clone()),
                    None => ("404 Not Found", b"nope".to_vec()),
                };
                let _ = write!(s, "HTTP/1.1 {code}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
            }
        });
    }

    fn serve(routes: HashMap<String, Vec<u8>>) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}/", l.local_addr().unwrap().port());
        spawn_server(l, routes);
        base
    }

    /// 在 mock 服务器上放一个 Release (带便携 exe + SHA256SUMS), `sums_hash_of` = 清单里写的内容 (用来造哈希不符)
    fn mock_release(exe: &[u8], sums_hash_of: &[u8]) -> Source {
        // 端口要先知道才能写 JSON: 先 bind 拿端口, 路由表造好后再交给服务器线程
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}/", l.local_addr().unwrap().port());
        let name = "ide-fast-9.9.9-windows-x64.exe";
        let asset = |n: &str, size: usize| serde_json::json!({ "name": n, "size": size, "browser_download_url": format!("{base}o/r/dl/{n}") });
        let sums = format!("{}  {name}\n", editcore::plugin_host::sha256_hex(sums_hash_of));
        let json = serde_json::to_vec(&serde_json::json!([{ "tag_name": "v9.9.9", "prerelease": false, "draft": false, "body": "notes", "html_url": format!("{base}o/r/releases/tag/v9.9.9"),
            "assets": [asset(name, exe.len()), asset(update::SUMS_NAME, sums.len())] }]))
        .unwrap();
        let routes: HashMap<String, Vec<u8>> = [("/releases".to_string(), json), (format!("/o/r/dl/{name}"), exe.to_vec()), (format!("/o/r/dl/{}", update::SUMS_NAME), sums.into_bytes())].into_iter().collect();
        spawn_server(l, routes);
        Source { api_url: format!("{base}releases"), trusted_prefix: format!("{base}o/r/") }
    }

    #[test]
    fn full_flow_against_a_local_mock_server() {
        if !curl_works() {
            eprintln!("skip: no curl");
            return;
        }
        let cur = Version::parse("0.3.1").unwrap();
        // 1. 正常: 检查 -> 发现 9.9.9 -> 下载校验 -> 得到 .new
        let src = mock_release(EXE, EXE);
        let out = check(&Curl, &src, Channel::Stable, &cur, InstallKind::Portable).unwrap();
        let Outcome::Available(c) = out else { panic!("应发现新版本") };
        assert_eq!((c.release.tag.as_str(), c.release.notes.as_str()), ("v9.9.9", "notes"));
        assert_eq!(c.asset.as_ref().unwrap().size, EXE.len() as u64);
        let d = tmpdir("flow");
        let new_exe = d.join("ide-fast.exe.new");
        let st = stage(&Curl, &c, &d, &new_exe).unwrap();
        assert_eq!(std::fs::read(&st.path).unwrap(), EXE);
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        // 当前已是 9.9.9: 不提示
        assert_eq!(check(&Curl, &src, Channel::Stable, &Version::parse("9.9.9").unwrap(), InstallKind::Portable), Ok(Outcome::UpToDate));
        // 2. 清单里的哈希和实际文件不符: 拒绝, 不留文件
        let bad = mock_release(EXE, b"something else");
        let Outcome::Available(c) = check(&Curl, &bad, Channel::Stable, &cur, InstallKind::Portable).unwrap() else { panic!() };
        let d2 = tmpdir("flow-bad");
        let e = stage(&Curl, &c, &d2, &d2.join("x.new")).unwrap_err();
        assert!(matches!(e, UpdErr::Hash(HashError::Mismatch { .. })), "{e:?}");
        assert_eq!(std::fs::read_dir(&d2).unwrap().count(), 0);
        // 3. 地址 404: 网络错误
        let mut gone = (*c).clone();
        gone.asset.as_mut().unwrap().url.push_str("-missing");
        assert!(matches!(stage(&Curl, &gone, &d2, &d2.join("x.new")), Err(UpdErr::Net(_))));
        assert_eq!(std::fs::read_dir(&d2).unwrap().count(), 0);
        let _ = (std::fs::remove_dir_all(&d), std::fs::remove_dir_all(&d2));
    }

    #[test]
    fn serve_helper_answers_404() {
        if !curl_works() {
            return;
        }
        let base = serve(HashMap::from([("/a".to_string(), b"hi".to_vec())]));
        assert_eq!(Curl.get(&format!("{base}a"), 100).unwrap(), b"hi");
        assert!(matches!(Curl.get(&format!("{base}b"), 100), Err(UpdErr::Net(_))));
        assert!(matches!(Curl.get(&format!("{base}a"), 1), Err(UpdErr::Net(_) | UpdErr::TooBig(_))), "超过 --max-filesize 的响应被拒");
    }
}
