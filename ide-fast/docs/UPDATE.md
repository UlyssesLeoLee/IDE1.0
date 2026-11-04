# 自更新 (检查更新…)

## 默认决定 (用户尚未另行指定, 先按这个做)

| 项 | 决定 | 为什么 |
|---|---|---|
| 更新源 | **GitHub Releases 直连** (`https://api.github.com/repos/UlyssesLeoLee/IDE1.0/releases`), 不经过 Vercel | 发布流程本来就往 Release 上传 msi / 便携 exe / `SHA256SUMS.txt`, 再加一层托管就多一个要维护、要信任、会挂的环节; 单人项目没有流量压力 |
| 通道 | 稳定版 = 最新的非预发布; 开发版 = 在所有 Release (含预发布) 里取版本号最高的。首选项 > 常规 里选, 默认稳定版 | 开发版用户也要能拿到正式版: 同号正式版高于它的预发布 (semver), 所以 `0.3.2-dev.N` 之后发 `0.3.2` 会自然升上去 |
| 何时联网 | 只在 **帮助 > 检查更新…** (命令面板同名) 点击时; 另有 opt-in 的「每天启动时检查一次」, **默认关**, 首帧之后才在后台线程做, 绝不阻塞启动, 没有常驻定时器 | 轻量与速度是产品定位 (CODING-STANDARDS §0.3); 首帧预算不受影响 |
| 校验 | 必须有 Release 里的 `SHA256SUMS.txt`, 且其中有该安装包的条目并与下载内容一致, 否则**拒绝** (缺清单也拒绝) | 防传输损坏 / 被替换成别的文件 |
| 安装 | msi 安装的: `msiexec /i <包> /passive`; 便携版: 写成 `<exe>.new`, 用它自己带 `--apply-update` 换掉旧 exe 再启动 | 不用 .bat/.ps1 (会弹控制台, 违背 GUI 子系统约定) |
| 失败 | 当前版本原样保留 (下载只落唯一命名临时文件, 校验通过才换名), 窗口显示原因 + 发布页链接 | |

## 流程

1. **检查** (`updnet::check`): `GET releases?per_page=30` -> `update::parse_releases` (丢掉草稿、tag 不是 semver 的、地址不在 `https://github.com/UlyssesLeoLee/IDE1.0/` 下的资产) -> `update::select` 按通道取比 `CARGO_PKG_VERSION` 新的最高版本。
2. **展示**: 版本、发布说明 (按字符截到 1500)、按**安装方式**选的资产: 装在 `Program Files` 下 = msi (`IDE1.0-Fast_*_x64.msi`), 否则 = 便携 exe (`ide-fast-*-windows-x64.exe`)。便携 exe 放在 Program Files 里换不了 (要管理员), 在别处跑 msi 又会装出第二份, 所以不跨类型回退。没有对应资产就只给发布页链接。
3. **下载并校验** (用户点): 先取 `SHA256SUMS.txt` 确认有该文件条目 (缺 = 还没下载就拒绝) -> 下载到唯一命名临时文件 (`fsx::unique_temp`) -> 实际大小 = Release 声明大小且不超上限 -> SHA-256 一致 -> 才换名成 `<exe>.new` (便携) 或留作 msi 临时文件。任何一步失败都删掉临时文件。
4. **安装并重启** (用户点): 登记请求 (`updapply`) 并发关闭命令, 走正常关闭流程 (有未保存修改照常先问, 取消关闭 = 请求作废), 窗口和设置落盘后由 `main` 执行:
   - msi: `%SystemRoot%\System32\msiexec.exe /i <包> /passive`, 本进程退出;
   - 便携: 运行 `<exe>.new --apply-update`。帮手就是新版 exe 自己: 从自己的文件名 (去掉 `.new`) 推出目标路径, **不接受任何路径参数**, 所以开关不能被拿来覆盖任意文件; 旧进程没退出前覆盖会失败, 重试约 10 秒; 成功后以 `--relaunch` 启动新版。下次启动惰性清理遗留的 `.new` / `.upd`。

## 网络约束

沿用 `ghplugin::StrictCurl` 的做法并按请求类型分别设上限 (`updnet.rs`): 只许 https (`--proto =https --proto-redir =https`, 最多 5 次重定向), API 响应 2 MB / 30 s, 校验清单 64 KB / 30 s, 安装包 120 MB / 15 min; URL 在 `--` 之后。API 请求带 `Accept: application/vnd.github+json`, 下载资产不带。只有测试构建才允许 `http://127.0.0.1:<端口>` (本机 mock 服务器跑完整流程), 发行版里没有这个口子 (`proto_for` 的 `allow_local` 参数在非测试构建恒为假, 有单测)。

## 之后加镜像 / Vercel manifest

`update::Source { api_url, trusted_prefix }` 是唯一的源配置, 检查流程只依赖 `Source` 和 `Fetch` 两个抽象:

- **镜像列表**: 把 `Source` 换成 `Vec<Source>` (或 `fn sources() -> Vec<Source>`), `check` 按顺序试, 第一个成功的为准; 每个镜像给自己的 `trusted_prefix`。校验值仍然只信 Release 里的 `SHA256SUMS.txt` 或下面 manifest 里的哈希, 与下载地址无关。
- **Vercel manifest**: 托管一个 `releases.json`, 形状与 GitHub API 的 `/releases` 数组相同 (`tag_name` / `prerelease` / `draft` / `body` / `html_url` / `assets[].{name,size,browser_download_url}`), 把它的地址设成 `api_url`、资产所在域设成 `trusted_prefix` 即可, `parse_releases` 不用改。托管方可以在里面直接放资产哈希 (需要的话给 `AssetInfo` 加一个可选 `sha256` 字段, 优先于 `SHA256SUMS.txt`)。

## Tag 约定 (publish-dev.ps1 产出)

- 正式版: `v0.3.2`
- 开发预发布: `v0.3.2-dev.YYYYMMDD.N` (例 `v0.3.2-dev.20261103.1`), 在 Release 上标 `--prerelease`。是合法 semver 预发布号, 数字段按数值比, 所以 `.10` 高于 `.9`; 且低于同号正式版。
- 不是 semver 的 tag (如 `nightly`) 被忽略, 永远不会被当成更新。`dev-` 前缀 (`dev-0.3.2`) 和 `v` 前缀可有可无。

每个 Release 必须带 `SHA256SUMS.txt` (每行 `<hex>  <文件名>`) 和约定名字的资产: `ide-fast-<版本>-windows-x64.exe`、`IDE1.0-Fast_<版本>_x64.msi` (见 `packaging/README.md`)。

## 已知限制

- **SHA256SUMS 只保证完整性, 不保证来源真实**: 它和安装包在同一个 Release 里, 能改包的人也能改清单。要防这个需要代码签名 (SignPath 申请中) 或独立信道上的签名清单, 现在没有。HTTPS + 固定 GitHub 仓库前缀是目前的信任根。
- msi 的版本号只有四段数字, **不能表达预发布**: 开发版 msi 的 `0.3.2.0` 与之后同号正式版 msi 一样, 覆盖安装可能被 Windows Installer 当作同版本而不升级。想靠 msi 升级的开发版用户请用便携 exe, 或在开发预发布里让 msi 的第四段递增。
- 便携版换文件时帮手进程是 `.new` 文件本身, 它退出后 `.new` 留在原地, 由下次启动清理。
- 交互式 msi 安装需要 UAC, 自动化里点不了: 与 `packaging/README.md` 一样, 发布前请在真机上走一遍。
