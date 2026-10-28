# SignPath Foundation 申请准备 (免费开源代码签名)

> 目标: 在下一个版本 (0.1.5 之前) 提交 SignPath Foundation 的申请, 让发布的 Windows 安装包 (.msi / .exe) 带上代码签名, 减少 SmartScreen "未知发布者" 警告。
> 申请本身 (注册账号、填表、提交) 要用你的个人信息, **必须由你自己在 <https://signpath.org/apply> 完成**; 本文把能提前做的准备和可直接粘贴的材料列出来。
> 依据: <https://signpath.org/terms> (2026-10 读取)。通过与否由 SignPath Foundation 逐案决定, 不保证。

## 1. 对照 SignPath 的要求

| 要求 | 现状 | 要做的事 |
|---|---|---|
| OSI 批准的开源许可, 不得商业双重授权, 无专有代码 | 根目录 `LICENSE` 是 **AGPL-3.0** (OSI 批准); 但 `Cargo.toml` 写的是 `MIT OR Apache-2.0` —— **前后不一致** | **先由你决定到底用哪个许可**, 再把 `LICENSE`、`Cargo.toml` (`license =`)、`README`、各 crate 对齐。随程序打包的 Pele 是 MIT (第三方组件, 允许) |
| 团队拥有并维护仓库, 只签自己源码构建出的二进制 | 仓库 `UlyssesLeoLee/IDE1.0`, 桌面程序由本仓库源码构建 | 无 |
| 非提交者的改动 (PR) 必须有团队成员审核; 定义 作者 / 审核者 / 批准者 角色 | 目前单人维护, 没有角色说明 | 在 README 的 Code signing policy 里写明角色与成员 (见 §3); 开 GitHub 分支保护 (PR 需审核) |
| README / 下载页有 **Code signing policy** 小节, 注明 "Free code signing provided by SignPath.io, certificate by SignPath Foundation" | 没有 | 加上 (§3 有现成文字) |
| 隐私政策, 或"除非用户要求否则不向其它联网系统发送信息"的声明 | 编辑器本地运行; 联网只发生在用户主动操作时 (检查插件更新 / 安装插件 / Chat 的 AI 请求); 无遥测 | 把这一点写成隐私声明 (§3) |
| 下载页描述软件功能 | Release 说明有功能列表 | 保持 |
| 构建: 从源码自动化、可验证; CI 配置视为源码 | `.github/workflows/ci.yml` 有 Windows `.msi` 构建任务 (GitHub 托管 runner) | 把"发布构建"整理成一个专门的 release 工作流, 产物上传为 artifact, 交给 SignPath 的 GitHub Action 签名 |
| 每次发布签名前**人工批准** | — | 在 SignPath 里给 release 签名策略设"需要批准" |
| 签名后的二进制元数据: 产品名与项目一致, **每次构建产品版本号一致** | Tauri `productName` / `version` 来自 `tauri.conf.json` | 确认 `productName` = 项目名, 版本号与 tag 一致 (现在 `Cargo.toml` / `tauri.conf.json` 同步改, 已有) |
| 项目要有人在维护, 并且**已以待签名的形式发布过** | 已发布 v0.1.0–v0.1.4, 但 Release 里目前只有免安装 exe, 没有 .msi | 用 CI 或本机 Tauri CLI 产出 .msi 并作为 Release 资产发布一次 |
| 所有团队成员对 SignPath 和代码仓库启用多因素认证 (MFA) | 需你确认 GitHub / SignPath 账号已开 MFA | 你来开启 |
| 无恶意软件 / 潜在有害程序; 不含黑客、漏洞利用功能; 尊重隐私、安装有卸载 | 编辑器 + 终端面板; 没有利用类功能。MSI 自带卸载 | 无 |

## 2. 提交前的待办 (按顺序)

1. **定许可** (AGPL-3.0 还是 MIT/Apache-2.0), 全仓库对齐。 ← 需要你拍板, 我不替你选。
2. README 加 **Code signing policy** 和**隐私声明** (§3), 并在 GitHub Release 说明里放同样的链接。
3. 开 GitHub 分支保护 (`dev` / `main`: PR 需要审核), 并给自己的 GitHub 和 SignPath 账号开 MFA。
4. 产出并发布一次 **.msi**: 本机已装好 Tauri CLI (`crates/ide-shell-desktop/node_modules`, 已被忽略不入库), 用 `npx tauri build --bundles msi`; 正式流程走 CI。
5. 去 <https://signpath.org/apply> 提交申请 (§4 的文字可直接用)。
6. 通过后: 在 SignPath 建项目 / 签名策略 (release 需人工批准), 仓库里加签名用的 GitHub Action 步骤, 把 `SIGNPATH_API_TOKEN` 放进仓库 Secrets。

## 3. 可直接放进 README 的文字

```markdown
## Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

- **Authors / committers**: <GitHub 用户名, 例: @UlyssesLeoLee>
- **Reviewers** (reviews every change from non-committers): <…>
- **Approvers** (approves each release before signing): <…>

Only binaries built from this repository's source by the automated release workflow are signed.
Every release requires manual approval before signing.

## Privacy policy

IDE1.0 runs locally and has no telemetry. The program does not send information to other networked
systems unless you ask it to:

- *Extensions panel*: "check for updates" / "install" contacts GitHub (`api.github.com`, release assets) only when you click it.
- *Chat panel / "ask AI for an improvement plan"*: sends the text you chose (selection or current section) to the AI endpoint **you** configured, only after you confirm; your API key is stored locally and sent only to that endpoint.
```

## 4. 申请表可用的文字 (英文; 表单字段以 signpath.org/apply 实际为准)

- **Project name**: IDE1.0
- **Repository**: https://github.com/UlyssesLeoLee/IDE1.0
- **License**: <对齐后的许可, OSI approved>
- **Description**: A lightweight, fast desktop text editor modelled on Sakura Editor, written in Rust (Tauri). Features: Sakura-compatible editing commands and macros, Grep/replace with encoding awareness, Markdown tools and preview, a sandboxed hot-pluggable extension system, and an optional AI chat panel that only runs when the user opens it.
- **What would be signed**: Windows installer (.msi) and executable built by the repository's GitHub Actions release workflow.
- **Maintainers / contact**: <你的姓名与邮箱>
- **Releases**: https://github.com/UlyssesLeoLee/IDE1.0/releases (v0.1.0 – v0.1.4)

## 5. 风险提示

- SignPath Foundation 不保证通过; 常见被拒原因: 项目太新、没有真实用户、仓库归属 / 许可不清、没有自动化构建。目前 IDE1.0 的 Release 次数多, 但 star / 用户数很可能偏少 —— 申请时如实写, 不要夸大。
- 许可不一致 (AGPL-3.0 vs MIT/Apache-2.0) 是最容易被直接卡住的一点, 请优先处理。
