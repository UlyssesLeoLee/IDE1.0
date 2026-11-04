//! 自更新的纯逻辑 (不联网、不碰磁盘, 全部可单测): 版本排序、Release JSON 解析、按通道选版本、按安装方式选资产、SHA256SUMS 解析与校验。
//! 联网与下载在 `updnet.rs`, 安装 / 换文件在 `updapply.rs`, 窗口在 `app/updateui.rs`。设计取舍见 `docs/UPDATE.md`。

use std::cmp::Ordering;
use std::collections::HashMap;

/// 更新通道: 稳定版只看正式 Release; 开发版把预发布也算上 (取其中版本号最高的, 所以正式版超过预发布后自然会被选中)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Channel {
    #[default]
    Stable,
    Dev,
}

impl Channel {
    pub const ALL: [Channel; 2] = [Channel::Stable, Channel::Dev];
    pub fn code(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Dev => "dev",
        }
    }
    pub fn from_code(s: &str) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| c.code() == s)
    }
    pub fn label_key(self) -> &'static str {
        match self {
            Channel::Stable => "upd.ch-stable",
            Channel::Dev => "upd.ch-dev",
        }
    }
}

/// 预发布标识符: 数字 < 字母数字 (semver 规则; 派生的 Ord 按变体顺序先比 Num 再比 Alpha)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Ident {
    Num(u64),
    Alpha(String),
}

/// semver 版本 (忽略 `+build`)
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Version {
    core: (u64, u64, u64),
    pre: Vec<Ident>,
}

impl Version {
    /// `1.2.3` / `v1.2.3` / `v1.2.3-dev.20261103.1` / `dev-1.2.3` (前缀 `v` `dev-` 可有可无); 不合法 = None (这样的 tag 永不会被当成更新)
    pub fn parse(tag: &str) -> Option<Version> {
        let mut s = tag.trim();
        s = s.strip_prefix("dev-").unwrap_or(s);
        s = s.strip_prefix(['v', 'V']).unwrap_or(s);
        let s = s.split('+').next()?;
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut it = core.split('.');
        let mut n = || it.next()?.parse::<u64>().ok();
        let core3 = (n()?, n()?, n()?);
        if it.next().is_some() {
            return None;
        }
        let mut idents = Vec::new();
        if let Some(p) = pre {
            for part in p.split('.') {
                if part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    return None;
                }
                idents.push(if part.bytes().all(|b| b.is_ascii_digit()) { Ident::Num(part.parse().ok()?) } else { Ident::Alpha(part.to_string()) });
            }
        }
        Some(Version { core: core3, pre: idents })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.core.0, self.core.1, self.core.2)?;
        for (i, id) in self.pre.iter().enumerate() {
            f.write_str(if i == 0 { "-" } else { "." })?;
            match id {
                Ident::Num(n) => write!(f, "{n}")?,
                Ident::Alpha(a) => f.write_str(a)?,
            }
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        self.core.cmp(&o.core).then_with(|| match (self.pre.is_empty(), o.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater, // 正式版高于它自己的预发布
            (false, true) => Ordering::Less,
            (false, false) => self.pre.cmp(&o.pre),
        })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// 本程序的版本
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION 是合法 semver")
}

/// 更新源: API 地址 + 允许的下载 / 页面前缀 (资产地址不在前缀下就丢掉, 防止 JSON 把下载指到别处)。测试里指向本机的 mock 服务器
#[derive(Clone, Debug)]
pub struct Source {
    pub api_url: String,
    pub trusted_prefix: String,
}

impl Source {
    pub fn github() -> Source {
        Source { api_url: "https://api.github.com/repos/UlyssesLeoLee/IDE1.0/releases?per_page=30".into(), trusted_prefix: "https://github.com/UlyssesLeoLee/IDE1.0/".into() }
    }
    /// 失败时给用户的兜底页面
    pub fn releases_page(&self) -> String {
        format!("{}releases", self.trusted_prefix)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AssetInfo {
    pub name: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    pub prerelease: bool,
    pub page: String,
    pub notes: String,
    pub assets: Vec<AssetInfo>,
}

/// 解析 `GET /releases` 的 JSON: 去掉草稿和 tag 不是 semver 的; 资产 / 页面地址不在可信前缀下的丢掉 (页面退回 Releases 总页)
pub fn parse_releases(json: &[u8], src: &Source) -> Result<Vec<Release>, String> {
    let v: serde_json::Value = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let arr = v.as_array().ok_or("not an array")?;
    let mut out = Vec::new();
    for r in arr {
        if r["draft"].as_bool().unwrap_or(false) {
            continue;
        }
        let Some(tag) = r["tag_name"].as_str() else { continue };
        let Some(version) = Version::parse(tag) else { continue };
        let page = r["html_url"].as_str().filter(|u| u.starts_with(&src.trusted_prefix)).map(str::to_string).unwrap_or_else(|| src.releases_page());
        let assets = r["assets"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let url = x["browser_download_url"].as_str()?.to_string();
                        let name = x["name"].as_str()?.to_string();
                        (url.starts_with(&src.trusted_prefix) && !name.contains(['/', '\\'])).then(|| AssetInfo { name, size: x["size"].as_u64().unwrap_or(0), url })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Release { tag: tag.to_string(), prerelease: r["prerelease"].as_bool().unwrap_or(false) || version.is_prerelease(), version, page, notes: r["body"].as_str().unwrap_or("").to_string(), assets });
    }
    Ok(out)
}

/// 这个通道下比 `current` 新的最高版本 (没有 = 已是最新)
pub fn select<'a>(releases: &'a [Release], ch: Channel, current: &Version) -> Option<&'a Release> {
    releases.iter().filter(|r| ch == Channel::Dev || !r.prerelease).filter(|r| r.version > *current).max_by(|a, b| a.version.cmp(&b.version))
}

/// 安装方式决定换哪种资产: 装在 Program Files 下 = msi 安装的 (换 exe 要管理员权限, 必须走 msi); 其余 = 便携 exe
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstallKind {
    Msi,
    Portable,
}

impl InstallKind {
    pub fn of_exe(exe: &std::path::Path) -> InstallKind {
        let p = exe.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
        if p.contains("\\program files\\") || p.contains("\\program files (x86)\\") { InstallKind::Msi } else { InstallKind::Portable }
    }
    fn matches(self, name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        match self {
            InstallKind::Portable => n.starts_with("ide-fast-") && n.ends_with("-windows-x64.exe"),
            InstallKind::Msi => n.starts_with("ide1.0-fast_") && n.ends_with("_x64.msi"),
        }
    }
}

pub const SUMS_NAME: &str = "SHA256SUMS.txt";
/// `SHA256SUMS.txt` 的分离签名: **原始 64 字节** ed25519 签名 (不是 base64 / hex), 见 `docs/UPDATE.md`
pub const SIG_NAME: &str = "SHA256SUMS.txt.sig";

/// 该安装方式对应的资产 (别的平台 / 没发布 = None, 界面只给 Release 页面链接)
pub fn pick_asset(r: &Release, kind: InstallKind) -> Option<&AssetInfo> {
    r.assets.iter().find(|a| kind.matches(&a.name))
}

pub fn pick_sums(r: &Release) -> Option<&AssetInfo> {
    r.assets.iter().find(|a| a.name.eq_ignore_ascii_case(SUMS_NAME))
}

pub fn pick_sig(r: &Release) -> Option<&AssetInfo> {
    r.assets.iter().find(|a| a.name.eq_ignore_ascii_case(SIG_NAME))
}

/// `SHA256SUMS.txt`: 每行 `<64 位十六进制>  <文件名>` (或 `<hex> *<文件名>`), 空行忽略; 哈希统一小写, 文件名精确匹配
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((hash, name)) = line.split_once(char::is_whitespace) else { continue };
        let name = name.trim().trim_start_matches('*');
        if hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !name.is_empty() {
            m.insert(name.to_string(), hash.to_ascii_lowercase());
        }
    }
    m
}

/// 签名清单的第一行: `version: X.Y.Z[-pre] channel: stable|dev` (字面量, 冒号后各一个空格, 两段之间一个空格, 行尾可有回车符)。
/// 版本串必须是裸 semver (没有 `v` / `dev-` 前缀, 没有 `+build`); 通道必须和版本一致 (带预发布号 = dev, 否则 = stable)。
/// 任何偏差 = None。这一行在签名覆盖范围内, 所以它把清单绑定到具体的版本和通道 (防回滚, 见 docs/UPDATE.md)。
#[derive(Clone, PartialEq, Debug)]
pub struct SumsHeader {
    pub version: Version,
    /// 清单里的版本原文 (资产文件名必须包含它)
    pub version_str: String,
    pub channel: Channel,
}

pub fn parse_sums_header(text: &str) -> Option<SumsHeader> {
    let line = text.split('\n').next()?;
    let line = line.strip_suffix('\r').unwrap_or(line);
    let rest = line.strip_prefix("version: ")?;
    let (vs, ch) = rest.split_once(" channel: ")?;
    if !vs.starts_with(|c: char| c.is_ascii_digit()) || vs.contains(['+', ' ']) {
        return None;
    }
    let version = Version::parse(vs)?;
    let channel = Channel::from_code(ch)?;
    if (channel == Channel::Dev) != version.is_prerelease() {
        return None;
    }
    Some(SumsHeader { version, version_str: vs.to_string(), channel })
}

/// 资产文件名里要有清单版本串, 且是完整的一段: 前面不能紧挨数字 / `.`, 后面不能紧挨数字 (`1.2.3` 不匹配 `1.2.30`);
/// 正式版后面还不能接预发布标记 (`-dev.` / `-rc` / `-alpha` / `-beta` / `-pre`, 即 `1.2.3` 不匹配 `1.2.3-dev.1`)。
/// 文件名里常见的 `-windows-x64` 之类不受影响; msi 的四段版本 `1.2.3.0` 允许 (后面跟 `.`)
pub fn name_has_version(name: &str, ver: &str) -> bool {
    let pre = ver.contains('-');
    name.match_indices(ver).any(|(i, _)| {
        let before = name[..i].chars().next_back();
        let rest = &name[i + ver.len()..];
        let after_ok = match rest.chars().next() {
            None => true,
            Some(c) if c.is_ascii_digit() => false,
            Some('-') if !pre => !["dev.", "rc", "alpha", "beta", "pre"].iter().any(|m| rest[1..].to_ascii_lowercase().starts_with(m)),
            Some(_) => true,
        };
        before.map_or(true, |c| !(c.is_ascii_digit() || c == '.')) && after_ok
    })
}

/// 校验失败的原因
#[derive(Clone, PartialEq, Debug)]
pub enum HashError {
    /// SHA256SUMS 里没有这个文件 (拒绝, 不当作「没校验也行」)
    Missing(String),
    Mismatch { name: String, want: String, got: String },
}

pub fn verify_hash(sums: &HashMap<String, String>, name: &str, got_hex: &str) -> Result<(), HashError> {
    let want = sums.get(name).ok_or_else(|| HashError::Missing(name.to_string()))?;
    if want.eq_ignore_ascii_case(got_hex) { Ok(()) } else { Err(HashError::Mismatch { name: name.to_string(), want: want.clone(), got: got_hex.to_ascii_lowercase() }) }
}

/// 「每天检查一次」到点了吗 (时钟被往回拨过也算到点, 免得永远不再检查)
pub fn due(now: u64, last: u64) -> bool {
    last == 0 || last > now || now - last >= 24 * 3600
}

/// 发布说明在窗口里最多显示这么多字符 (按字符截, 不会把 UTF-8 截成半个字)
pub const NOTES_MAX_CHARS: usize = 1500;

pub fn truncate_notes(s: &str, max_chars: usize) -> String {
    let s = s.trim();
    match s.char_indices().nth(max_chars) {
        Some((i, _)) => format!("{}…", s[..i].trim_end()),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("应能解析 {s}"))
    }

    #[test]
    fn versions_order_by_semver_including_prereleases() {
        assert!(v("0.3.2") > v("0.3.1") && v("0.10.0") > v("0.9.9") && v("1.0.0") > v("0.99.99"));
        assert!(v("0.3.1") > v("0.3.1-dev.5"), "正式版高于它自己的预发布");
        assert!(v("0.3.2-dev.1") > v("0.3.1"), "下一个版本的预发布仍高于当前正式版");
        assert!(v("0.3.1-dev.10") > v("0.3.1-dev.9"), "数字标识符按数值比, 不按字典序");
        assert!(v("0.3.1-dev.2") > v("0.3.1-dev"), "多一段的更高");
        assert!(v("0.3.1-dev.1") > v("0.3.1-1"), "数字 < 字母数字");
        assert!(v("0.3.1-rc.1") > v("0.3.1-beta.7"));
        assert_eq!(v("v0.3.1+build5"), v("0.3.1"), "build 元数据忽略");
        assert_eq!(v("dev-0.3.2"), v("0.3.2"), "dev- 前缀可有可无");
        assert_eq!(v("v0.3.2-dev.20261103.1").cmp(&v("v0.3.2-dev.20261103.2")), Ordering::Less);
    }

    #[test]
    fn unparsable_tags_are_none() {
        for bad in ["", "latest", "1.2", "1.2.3.4", "v1.x.3", "1.2.3-", "1.2.3-a..b", "1.2.3-é", "dev-20261103", "-1.2.3", "99999999999999999999.0.0"] {
            assert!(Version::parse(bad).is_none(), "{bad:?}");
        }
        assert!(current_version() >= v("0.0.1"));
    }

    #[test]
    fn channel_codes_roundtrip() {
        for c in Channel::ALL {
            assert_eq!(Channel::from_code(c.code()), Some(c));
        }
        assert_eq!(Channel::from_code("nightly"), None);
        assert_eq!(Channel::default(), Channel::Stable);
    }

    fn src() -> Source {
        Source { api_url: "x".into(), trusted_prefix: "https://github.com/o/r/".into() }
    }

    fn json() -> Vec<u8> {
        let a = |n: &str, size: u64| serde_json::json!({ "name": n, "size": size, "browser_download_url": format!("https://github.com/o/r/releases/download/t/{n}") });
        serde_json::to_vec(&serde_json::json!([
            { "tag_name": "v0.3.1", "prerelease": false, "draft": false, "body": "stable 0.3.1", "html_url": "https://github.com/o/r/releases/tag/v0.3.1", "assets": [a("ide-fast-0.3.1-windows-x64.exe", 10), a(SUMS_NAME, 5)] },
            { "tag_name": "v0.3.2", "prerelease": false, "draft": true, "body": "草稿", "html_url": "https://github.com/o/r/releases/tag/v0.3.2", "assets": [] },
            { "tag_name": "v0.3.2-dev.20261103.1", "prerelease": true, "draft": false, "body": "dev build", "html_url": "https://github.com/o/r/releases/tag/v0.3.2-dev.20261103.1",
              "assets": [a("ide-fast-0.3.2-dev.20261103.1-windows-x64.exe", 20), a("IDE1.0-Fast_0.3.2.0_x64.msi", 30), a(SUMS_NAME, 5),
                         { "name": "evil.exe", "size": 1, "browser_download_url": "https://evil.example/evil.exe" }] },
            { "tag_name": "nightly", "prerelease": true, "draft": false, "html_url": "https://evil.example/x", "assets": [] },
            { "tag_name": "v0.3.0", "prerelease": false, "draft": false, "html_url": "https://evil.example/page", "assets": [] },
        ]))
        .unwrap()
    }

    #[test]
    fn release_json_is_parsed_and_untrusted_parts_dropped() {
        let r = parse_releases(&json(), &src()).unwrap();
        let tags: Vec<&str> = r.iter().map(|x| x.tag.as_str()).collect();
        assert_eq!(tags, ["v0.3.1", "v0.3.2-dev.20261103.1", "v0.3.0"], "草稿和非 semver 的 tag 被跳过");
        assert!(!r[0].prerelease && r[1].prerelease);
        assert_eq!(r[1].assets.len(), 3, "指向别处的资产被丢掉");
        assert!(r[1].assets.iter().all(|a| a.url.starts_with("https://github.com/o/r/")));
        assert_eq!(r[2].page, "https://github.com/o/r/releases", "页面地址不可信 = 退回 Releases 总页");
        assert_eq!(r[0].notes, "stable 0.3.1");
        assert!(parse_releases(b"{}", &src()).is_err() && parse_releases(b"not json", &src()).is_err());
        assert!(parse_releases(b"[]", &src()).unwrap().is_empty());
    }

    #[test]
    fn channels_pick_the_right_release() {
        let r = parse_releases(&json(), &src()).unwrap();
        let cur = v("0.3.0");
        assert_eq!(select(&r, Channel::Stable, &cur).map(|x| x.tag.as_str()), Some("v0.3.1"), "稳定通道不看预发布");
        assert_eq!(select(&r, Channel::Dev, &cur).map(|x| x.tag.as_str()), Some("v0.3.2-dev.20261103.1"));
        let cur = v("0.3.1");
        assert!(select(&r, Channel::Stable, &cur).is_none(), "已是最新");
        assert_eq!(select(&r, Channel::Dev, &cur).map(|x| x.tag.as_str()), Some("v0.3.2-dev.20261103.1"));
        assert!(select(&r, Channel::Dev, &v("0.3.2")).is_none(), "0.3.2 正式版高于 0.3.2 的预发布: 不倒退");
        assert!(select(&r, Channel::Dev, &v("0.3.2-dev.20261103.1")).is_none(), "同版本不重复提示");
        assert_eq!(select(&r, Channel::Stable, &v("0.3.1-dev.9")).map(|x| x.tag.as_str()), Some("v0.3.1"), "开发版用户切回稳定通道: 同号正式版更高, 会被提示");
    }

    #[test]
    fn dev_build_does_not_offer_itself_to_stable_users() {
        // 当前 0.3.1 (稳定) 不会被提示 0.3.1-dev.X 这种更低的预发布
        let rel = |t: &str| Release { tag: t.into(), version: v(t), prerelease: v(t).is_prerelease(), page: String::new(), notes: String::new(), assets: vec![] };
        let rs = [rel("v0.3.1-dev.7"), rel("v0.3.1")];
        assert!(select(&rs, Channel::Dev, &v("0.3.1")).is_none());
        assert_eq!(select(&rs, Channel::Dev, &v("0.3.1-dev.3")).map(|x| x.tag.as_str()), Some("v0.3.1"), "开发版用户先拿到正式版");
    }

    #[test]
    fn assets_follow_the_install_kind() {
        let r = parse_releases(&json(), &src()).unwrap();
        let dev = &r[1];
        assert_eq!(pick_asset(dev, InstallKind::Portable).unwrap().name, "ide-fast-0.3.2-dev.20261103.1-windows-x64.exe");
        assert_eq!(pick_asset(dev, InstallKind::Msi).unwrap().name, "IDE1.0-Fast_0.3.2.0_x64.msi");
        assert!(pick_asset(&r[2], InstallKind::Portable).is_none(), "没有资产 = None, 界面给页面链接");
        assert_eq!(pick_sums(dev).unwrap().name, SUMS_NAME);
        assert_eq!(InstallKind::of_exe(std::path::Path::new(r"C:\Program Files\IDE1.0 Fast\ide-fast.exe")), InstallKind::Msi);
        assert_eq!(InstallKind::of_exe(std::path::Path::new(r"C:\Program Files (x86)\X\ide-fast.exe")), InstallKind::Msi);
        assert_eq!(InstallKind::of_exe(std::path::Path::new(r"D:\tools\ide-fast.exe")), InstallKind::Portable);
        assert_eq!(InstallKind::of_exe(std::path::Path::new("C:/Users/a/Program Files stuff/ide-fast.exe")), InstallKind::Portable, "只认目录名恰好是 Program Files");
    }

    const H1: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn sums_parse_both_formats_and_verify_strictly() {
        let text = format!("{H1}  a.exe\n\n{}  *b.msi\r\nnot-a-hash  c.zip\n{H1}\n{}  with space.txt\n", "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855", H1);
        let m = parse_sums(&text);
        assert_eq!(m.len(), 3, "{m:?}");
        assert_eq!(m["a.exe"], H1);
        assert_eq!(m["b.msi"], "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "哈希转小写, * 前缀去掉");
        assert!(m.contains_key("with space.txt"));
        assert_eq!(verify_hash(&m, "a.exe", &H1.to_ascii_uppercase()), Ok(()));
        assert!(matches!(verify_hash(&m, "a.exe", "00"), Err(HashError::Mismatch { .. })));
        assert_eq!(verify_hash(&m, "A.EXE", H1), Err(HashError::Missing("A.EXE".into())), "文件名精确匹配");
        assert_eq!(verify_hash(&HashMap::new(), "a.exe", H1), Err(HashError::Missing("a.exe".into())), "缺条目 = 拒绝");
    }

    #[test]
    fn notes_are_cut_by_characters() {
        assert_eq!(truncate_notes("  short  ", 10), "short");
        let s = "更新内容".repeat(10);
        let t = truncate_notes(&s, 5);
        assert_eq!(t.chars().count(), 6);
        assert!(t.ends_with('…') && s.starts_with(t.trim_end_matches('…')));
        assert_eq!(truncate_notes("abc", 3), "abc");
    }

    #[test]
    fn daily_check_is_due_after_24_hours_or_a_clock_rewind() {
        assert!(due(100, 0) && due(90_000, 1) && due(10, 99));
        assert!(!due(1000, 1000) && !due(1000 + 86_399, 1000) && due(1000 + 86_400, 1000));
    }

    #[test]
    fn dev_tag_example_from_the_publish_script_parses() {
        // docs/UPDATE.md 与 packaging/publish-dev.ps1 约定的 tag 格式
        assert!(v("v0.3.2-dev.20261103.1").is_prerelease());
        assert!(v("v0.3.2-dev.20261103.1") > v("0.3.1"));
    }
}
