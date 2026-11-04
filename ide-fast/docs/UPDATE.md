# 自更新 (检查更新…)

## 默认决定 (用户尚未另行指定, 先按这个做)

| 项 | 决定 | 为什么 |
|---|---|---|
| 更新源 | **GitHub Releases 直连** (`https://api.github.com/repos/UlyssesLeoLee/IDE1.0/releases`), 不经过 Vercel | 发布流程本来就往 Release 上传 msi / 便携 exe / `SHA256SUMS.txt`, 再加一层托管就多一个要维护、要信任、会挂的环节; 单人项目没有流量压力 |
| 通道 | 稳定版 = 最新的非预发布; 开发版 = 在所有 Release (含预发布) 里取版本号最高的。首选项 > 常规 里选, 默认稳定版 | 开发版用户也要能拿到正式版: 同号正式版高于它的预发布 (semver), 所以 `0.3.2-dev.N` 之后发 `0.3.2` 会自然升上去 |
| 何时联网 | 只在 **帮助 > 检查更新…** (命令面板同名) 点击时; 另有 opt-in 的「每天启动时检查一次」, **默认关**, 首帧之后才在后台线程做, 绝不阻塞启动, 没有常驻定时器 | 轻量与速度是产品定位 (CODING-STANDARDS §0.3); 首帧预算不受影响 |
| 校验 | 必须有 Release 里的 `SHA256SUMS.txt` **和它的 ed25519 签名 `SHA256SUMS.txt.sig`**; 先验签, 再核对清单第一行的版本 / 通道绑定 (见「清单的版本 / 通道绑定」), 通过后才信清单里的哈希; 清单里要有该安装包的条目并与下载内容一致; 再粗检文件头。任何一项缺 / 不符都**拒绝并删除下载物** | 哈希防传输损坏; 签名防「能改 Release 的人连清单一起换掉」(清单与安装包同源, 光有哈希只保完整性不保真实性) |
| 安装 | msi 安装的: `msiexec /i <包> /passive`; 便携版: 写成 `<exe>.new`, 用它自己带 `--apply-update` 换掉旧 exe 再启动 | 不用 .bat/.ps1 (会弹控制台, 违背 GUI 子系统约定) |
| 失败 | 当前版本原样保留 (下载只落唯一命名临时文件, 校验通过才换名), 窗口显示原因 + 发布页链接 | |

## 流程

1. **检查** (`updnet::check`): `GET releases?per_page=30` -> `update::parse_releases` (丢掉草稿、tag 不是 semver 的、地址不在 `https://github.com/UlyssesLeoLee/IDE1.0/` 下的资产) -> `update::select` 按通道取比 `CARGO_PKG_VERSION` 新的最高版本。
2. **展示**: 版本、发布说明 (按字符截到 1500)、按**安装方式**选的资产: 装在 `Program Files` 下 = msi (`IDE1.0-Fast_*_x64.msi`), 否则 = 便携 exe (`ide-fast-*-windows-x64.exe`)。便携 exe 放在 Program Files 里换不了 (要管理员), 在别处跑 msi 又会装出第二份, 所以不跨类型回退。没有对应资产就只给发布页链接。
3. **下载并校验** (用户点): 内置公钥是占位 -> 直接拒绝 (不联网) -> Release 缺 `.sig` -> 拒绝 (还没下载) -> 取 `SHA256SUMS.txt` 与 `.sig`, **先验签** (`updnet::verify_sums_sig`, `verify_strict`) -> 核对版本 / 通道绑定 (`updnet::check_binding`) -> 确认清单有该文件条目 (缺 = 还没下载就拒绝) -> 下载到唯一命名临时文件 (`fsx::unique_temp`) -> 实际大小 = Release 声明大小且不超上限 -> SHA-256 一致 -> 文件头 (msi = OLE 复合文档头 `D0 CF 11 E0 A1 B1 1A E1`, exe = `MZ`) -> 才换名成 `<exe>.new` (便携) 或留作 msi 临时文件。任何一步失败都删掉临时文件。
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

- **信任根 = 内置公钥**: 清单由发布者私钥签名, 能改 Release 但拿不到私钥的人无法让更新通过。密钥泄露 / 丢失见下面「轮换」。**公钥还是全 0 占位时更新功能不会安装任何东西** (窗口给出提示, 只留发布页链接)。这不等于 Authenticode 代码签名 (SmartScreen 仍会提示, SignPath 申请中), 两者互补。
- 回滚攻击: 光有签名不够 (签名只证明清单出自发布者, 不证明它属于哪个版本), 所以签名清单的第一行绑定了版本和通道, 验签后逐项核对, 见下面「清单的版本 / 通道绑定」。
- msi 的版本号只有四段数字, **不能表达预发布**: 开发版 msi 的 `0.3.2.0` 与之后同号正式版 msi 一样, 覆盖安装可能被 Windows Installer 当作同版本而不升级。想靠 msi 升级的开发版用户请用便携 exe, 或在开发预发布里让 msi 的第四段递增。
- 便携版换文件时帮手进程是 `.new` 文件本身, 它退出后 `.new` 留在原地, 由下次启动清理。
- 交互式 msi 安装需要 UAC, 自动化里点不了: 与 `packaging/README.md` 一样, 发布前请在真机上走一遍。

## 更新包签名 (ed25519)

### 格式

- 算法 ed25519 (RFC 8032, 纯 Rust `ed25519-dalek` 2.x, 只开验证: `default-features = false`, 无 rand / 无预计算大表; 实测 release exe 体积无可测增长)。
- 被签内容 = Release 里 `SHA256SUMS.txt` 的**原始字节** (不规范化换行, 所以签完不能再改这个文件, 哪怕多一个空格)。
- 签名文件 `SHA256SUMS.txt.sig` = **原始 64 字节** (不是 base64 / hex)。长度不是 64 的一律按无效处理。
- 公钥 = `ide-fast/src/updkey.rs` 里的 `UPDATE_PUBKEY: [u8; 32]`; 全 0 = 未配置。发行版代码只会把这个常量交给 `updnet::stage`, 没有环境变量 / 配置文件 / 命令行旁路 (单元测试通过函数参数注入运行时生成的测试公钥)。

### 清单的版本 / 通道绑定 (防回滚 / 降级)

威胁: 攻击者把**旧版本**的 `SHA256SUMS.txt` + 它的有效 `.sig` + 旧安装包, 挂到一个**新版本**的 Release 页上。旧清单签名有效、哈希也对得上旧安装包, 只靠「验签 + 哈希」会装上旧(有漏洞的)版本。

**语法 (精确)**: 被签名的 `SHA256SUMS.txt` 的**第一行**必须恰好是

```
version: <版本> channel: <通道>
```

- 字面量 `version: `、`<版本>`、` channel: `、`<通道>`, 各段之间恰好一个空格, 行首无空白、前面无空行 / BOM; 行尾为 LF 或 CRLF。
- `<版本>` = **裸 semver** `X.Y.Z` 或 `X.Y.Z-pre.N` (例 `0.3.2`、`0.3.2-dev.20261103.1`); 不带 `v` / `dev-` 前缀, 不带 `+build`。
- `<通道>` = `stable` 或 `dev`, 并且与版本一致: 带预发布号 = `dev`, 否则 = `stable`。
- 其后各行仍是 `<64 位十六进制>  <文件名>`。

**客户端在验签通过之后、下载之前**依次核对 (`updnet::check_binding`), 任何一项不满足 = 拒绝并删除下载物 (此时还没有下载安装包), 窗口用界面语言 (中 / 日 / 英) 显示原因:

| | 检查 | 失败时 |
|---|---|---|
| (a) | 清单版本 == 这个 Release 的版本 (tag) | `upd.e-mf-rel`: 旧清单挂在新 Release 页上 |
| (b) | 清单版本 > 当前 `CARGO_PKG_VERSION` (semver, 预发布按现有顺序: 低于同号正式版, 数字段按数值比) | `upd.e-mf-old`: 相等 / 更低 = 回滚、降级或原地重装 |
| (c) | 清单通道 == 用户所选通道。**例外**: 开发通道也接受 `stable` 清单 (开发通道本来就包含正式版, 见「默认决定」); 稳定通道绝不接受 `dev` 清单 | `upd.e-mf-chan` |
| (d) | 所选安装包文件名含清单的版本串 (作为完整的一段: `1.2.3` 不匹配 `1.2.30` / `x1.2.3`; 正式版不匹配 `1.2.3-dev.1`; msi 的 `1.2.3.0` 可以), **并且**该文件名在已签名的表里 | `upd.e-mf-name` / `upd.e-nosum` |
| — | 第一行缺失 / 不符合上面的语法 | `upd.e-mf-bad` |

之后才是原有的下载 -> 实际大小 -> SHA-256 -> 文件头。因为版本行在签名覆盖范围内, 攻击者既不能改它, 也不能把旧版本的清单换个版本号继续用。

**发布端**: `examples/sign-sums.rs` 的 `sign` 子命令必须带 `--version X.Y.Z[-pre] --channel stable|dev`, 并核对清单第一行恰好等于 `version: <版本> channel: <通道>` (且通道与版本一致), 不符 / 缺参数一律拒绝签名。`packaging/sign-release.ps1` 对应 `-Version` / `-Channel` (必填)。`packaging/publish-dev.ps1` 生成清单时自己写第一行 (`version: <tag 去 v> channel: dev`), 并把 msi 改名成 `IDE1.0-Fast_<完整 dev 版本>_x64.msi` (原名只有四段数字, 过不了 (d))。发正式版时手工生成清单: 第一行 `version: X.Y.Z channel: stable`, 便携 exe 叫 `ide-fast-X.Y.Z-windows-x64.exe`, msi 叫 `IDE1.0-Fast_X.Y.Z.0_x64.msi` (或 `..._X.Y.Z_x64.msi`), 再 `sign-release.ps1 -Version X.Y.Z -Channel stable`。

**升级注意**: 带这个检查的客户端会拒绝没有版本行的旧格式清单, 所以这次起每个 Release 的清单都必须带版本行; 已发布的旧 Release 对新客户端不再可装 (本来就不该装旧版本)。

### 所有者要做的一次性设置

1. 在**仓库之外**生成私钥 (例如 `D:\secrets\ide-fast-update.key`):
   `powershell -ExecutionPolicy Bypass -File ide-fast/packaging/sign-release.ps1 -GenKey -KeyFile D:\secrets\ide-fast-update.key`
   私钥是 32 字节随机种子, 以 64 个十六进制字符存成文本文件 (脚本会收紧文件权限、拒绝仓库内路径、拒绝覆盖已有文件)。脚本最后会打印一行 `pub const UPDATE_PUBKEY: [u8; 32] = [...]`。
2. 把那一行粘进 `ide-fast/src/updkey.rs` 替换全 0 占位, 提交 (**只有公钥进仓库**), 照常发新版。
3. 设置环境变量 `IDE_FAST_SIGN_KEY` 指向私钥文件 (用户级环境变量即可, 不要写进任何仓库文件 / CI 日志)。

### 私钥保管

- 只放在你自己的机器的仓库外目录, 并做**离线备份** (加密 U 盘 / 密码管理器的安全附件)。不要提交、不要贴到 issue / 聊天、不要放进云同步的仓库目录。
- CI 里要签名的话用 CI 的加密 secret 在构建时写成临时文件, 用完即删; 不要把私钥打进任何构建产物。
- 丢失 = 无法再给已发布的客户端签新版本 (见轮换)。泄露 = 任何人都能给装了该公钥的客户端推更新, 必须立刻轮换。

### 发布流程里多出来的步骤

在原来「构建 -> 算 `SHA256SUMS.txt`」之后、`gh release create` 之前:

```powershell
powershell -ExecutionPolicy Bypass -File ide-fast/packaging/sign-release.ps1 -Sums <out>\SHA256SUMS.txt -Version <X.Y.Z[-pre]> -Channel <stable|dev>
# 产出 <out>\SHA256SUMS.txt.sig, 与安装包 / SHA256SUMS.txt 一起作为 Release 资产上传
```

`publish-dev.ps1` 已经包含这一步 (没设 `IDE_FAST_SIGN_KEY` 就中止发布)。上传前可以用任意 ed25519 工具对照公钥自检; 工具本身 (`examples/sign-sums.rs`) 签完会用公钥自检一遍。一旦 `SHA256SUMS.txt` 有任何改动, 必须重新签。

### 轮换

客户端只信二进制里内置的那把公钥, 所以换钥要走「旧钥签的最后一版 -> 内置新钥的新版」:

1. 生成新密钥对 (`-GenKey` 到新文件)。
2. 发一个**用旧私钥签名**的版本, 其中 `updkey.rs` 换成**新公钥**。已安装的旧客户端用旧公钥验过这一版, 升级后就信任新钥。
3. 之后的 Release 全部用新私钥签; 旧私钥销毁。
4. 泄露场景下: 攻击者也有旧私钥, 在第 2 步之前能签恶意更新给旧客户端; 所以要尽快发第 2 步的版本, 并在发布说明里提示用户手动下载安装 (旧客户端的自动更新信任链已不可靠)。丢失场景下旧客户端无法自动升级, 需要手动安装一个内置新钥的版本。
