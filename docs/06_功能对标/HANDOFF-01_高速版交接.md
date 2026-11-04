# [HANDOFF-01] 高速版 (ide-fast) 现状与交接

> 编写: 2026-11-04, 基线 `dev` @ 0.3.9 (`f3ef32f`)。配套的优化计划见 [WBS-03_优化计划.md](WBS-03_优化计划.md)。
> 写法约定: **实测** = 本次在代码 / 命令输出里确认过; **推断** = 根据代码阅读得出, 没有运行验证; **未验证** = 没有条件验证。

## 1. 一句话现状

高速版 (`ide-fast`, egui 原生 Rust) 已把 Tauri 版的菜单 / 命令 / 设置项逐项移植完 (0.3.2 – 0.3.8 共七批), 0.3.9 补全了中 / 日 / 英三语。**单元测试覆盖较好, 但 CI 实际没有在跑、新 UI 没有在真窗口里验证过、0.3.2 之后的版本都没有打标签和出安装包** —— 这三件是接手后最先要处理的 (见 WBS-03 的 O1 / O2 / O3)。

## 2. 产品与仓库地图

| 部分 | 位置 | 说明 |
|---|---|---|
| 编辑内核 | `crates/editcore` | 无状态的编辑 / 移动 / 查找 / 编码 / 宏解析; 两个产品共用。行为按真实 Sakura 比对过 (`tests/sakura-diff`) |
| Tauri 版 (桌面) | `crates/ide-shell-desktop` + 网页前端 `crates/ide-shell-web/src/editor.html` (单文件约 1.36 万行) | 网页 UI, 依赖 WebView2; 也是网页版 / 其他 agent 的移植基底 |
| 高速版 | `ide-fast/` (约 2.9 万行 Rust) | 原生窗口, 首帧约 240–260 ms (见 `ide-fast/docs/PERF.md`); 独立安装包 |
| 文档 | `docs/` (设计 / 测试 / 评审), `ide-fast/docs/` (高速版专用: `PARITY-PLAN.md` `ROADMAP.md` `PERF.md` `UPDATE.md` …) | |

**移植方向**: 一律是 *Tauri 版 → 高速版*。高速版的功能清单以 Tauri 版的 `MENU_DEF` / `KEYMAP` / `PREFS` 为准 (`ide-fast/docs/PARITY-PLAN.md`)。

### 分支与 PR 流程 (实测)
- `main` 与 `dev` **都要保留**。开发提交在 `dev`, 每一轮一个提交 + 版本号 (`Cargo.toml` 的 workspace 版本、`Cargo.lock` 里 9 个成员、`crates/ide-shell-desktop/tauri.conf.json`), 然后开 `dev → main` 的 PR, 合并方式用 **merge commit**, 合并后把 `dev` 快进到 `main` 的合并提交。
- 当前: `main` = 0.3.8 合并提交 `a5ba8b7`; `dev` = 0.3.9 `f3ef32f`; [PR #21](https://github.com/UlyssesLeoLee/IDE1.0/pull/21) (0.3.9, 草稿) 等待合并。

## 3. 版本与发布状态

| 版本 | 内容 | 提交 | 标签 |
|---|---|---|---|
| 0.3.2 | 第一批: 宏录制 .mac、上下一个函数、列块删除 / 替换、Alt 聚焦菜单栏、结构化搜索、检查更新 | `355b10e` | **未推** |
| 0.3.3 | 第二批: 删除上 / 下方行、复制路径与光标位置、在折行处加换行复制、打开同名头 / 源文件 | `926ca93` | **未推** |
| 0.3.4 | 第三批: 标签分组命令、SPACE 缩进 (此版实现有误, 0.3.5 修正)、立即备份、选区统计 | `1c19885` | **未推** |
| 0.3.5 | SPACE 缩进修正 + 第四批: Sakura 默认键补齐、合并连续重复行 | `21fc6f9` | **未推** |
| 0.3.6 | 第五批: 代码补全、转到定义、工具栏自定义、AI 推理强度 | `a4b853f` | **未推** |
| 0.3.7 | 第六批: Sakura 宏 268 个函数全覆盖、宏登记槽 | `1620a3a` | **未推** |
| 0.3.8 | 第七批: 窗口一览、关键字帮助、宏函数补齐 + 代码审查修正 | `8a6e470` | **未推** |
| 0.3.9 | 中 / 日 / 英三语补全 | `f3ef32f` | **未推** |

- 远端已有的最新标签是 `v0.3.1`。云端会话推标签被拒 (HTTP 403, 凭据策略), 需要维护者在本机执行:
  ```bash
  git fetch origin
  git tag v0.3.2 355b10e && git tag v0.3.3 926ca93 && git tag v0.3.4 1c19885 && git tag v0.3.5 21fc6f9
  git tag v0.3.6 a4b853f && git tag v0.3.7 1620a3a && git tag v0.3.8 8a6e470 && git tag v0.3.9 f3ef32f
  git push origin v0.3.2 v0.3.3 v0.3.4 v0.3.5 v0.3.6 v0.3.7 v0.3.8 v0.3.9
  ```
- **没有出过任何安装包**: 高速版的 MSI / 便携 exe 要在 Windows 上用 `ide-fast/packaging/build-msi.ps1` 构建 (WiX v3); 更新机制要求 `SHA256SUMS.txt` 带发布者的 ed25519 签名 `SHA256SUMS.txt.sig` (见 `ide-fast/docs/UPDATE.md`), 私钥不在仓库里。`.github/workflows/release-windows.yml` **只构建 Tauri 版的 MSI**, 不构建高速版。

## 4. 构建与测试

### Windows (正式环境)
`cargo build -p ide-fast` / `cargo test -p ide-fast` 直接可用 (推断: 依赖的默认 feature 面向 Windows)。

### Linux (云端会话的做法, 实测)
仓库原样在 Linux 上编不过高速版, 需要一个草稿工作区:
1. 根 `Cargo.toml` 的 workspace 成员去掉 Tauri 相关的 7 个 crate, 并去掉私有 git 依赖 `aci-emitter` (会话没有权限拉取);
2. `ide-fast/Cargo.toml`: `eframe` 加 `"x11"` feature, `rfd` 加 `["xdg-portal", "async-std"]`, 补 `raw-window-handle = "0.6"`;
3. 源码同步进去再 `cargo test -p ide-fast --bin ide-fast`。注意 `cp -a` 会保留旧的修改时间, cargo 可能不重编 —— 同步后 `touch` 改过的文件。

### 测试基线 (实测, 0.3.9)
| crate | 通过 | 失败 | 说明 |
|---|---|---|---|
| `ide-fast` | 491 | 3 | 失败的 3 个是测试里写死 `C:\` / `D:\` 的 Windows 路径断言 (`settings::session_roundtrips_and_refuses_unsafe_paths`、`tagjump::rel_paths_are_checked`、`updapply::new_and_target_paths_are_inverse_and_strict`), 在 Linux 上必然失败, 由来已久 |
| `editcore` | 462 (+ 集成测试) | 0 | 原先失败的 `grep_engine::tests::replace_backup_and_skip_list` 已修 (测试假定了 Windows 的大小写不敏感路径) |

编译警告: `ide-fast` 21 条 (rustc), clippy 48 条; `editcore` clippy 14 条 (实测)。

## 5. 接手需要知道的代码结构

- **命令表是唯一来源** (`ide-fast/src/keymap.rs`): 每个命令一行 (`t(...)` 用 i18n 键, `z(...)` 用中文标签), 菜单、快捷键、命令面板、工具栏、宏 (`APP_FUNCS`)、键位设置都读它。新增命令 = 加 `App` 枚举变体 + 一行 `z(...)` + 在 `App::run_app` 里分派。
- **设置** (`settings.rs`): 新增一个字段要改 结构体 / `Default` / `apply_json` (读, 带清洗) / `prefs_json` (写) / 往返测试的字面量, 共 5 处。`adopt_prefs` 整体采纳首选项草稿, 但 `view` / 最近文件 / 键位保留当前值。
- **三语** (`i18n.rs` + `msgtr.rs`):
  - `tr("键")` 查语义键表 `T`; `trz("中文")` 查以中文原文为键的 `CMD_T`; 状态栏提示 / 错误信息是代码里现拼的中文, 显示时由 `msgtr::msg` 按模板 (`MSG_T`, `{}` 为可变部分) 翻译。
  - 三个测试防漏翻: 命令标签在英文界面不含汉字、源码里每个 `trz` / `app_label` 字面量都在表里、每条直接写进 `tip` 的中文都有模板。**新增界面文字时这几个测试会提醒你补翻译。**
  - 内置手册三份: `help.txt` / `help_ja.txt` / `help_en.txt`, 测试要求三份章节号和行数一致 —— 改一份就要同步另外两份。
- **宏** (`macros.rs`): 每个 Sakura `S_` 函数只有三种结局 —— 在文档上执行 (`DocOp`)、转交应用执行命令表里的某个命令 (`APP_FUNCS`)、明确拒绝并写原因 (`NOT_APPLICABLE`)。整份宏先预检, 有一步跑不了就一步也不执行。打开列表 / 对话框类的函数在宏里**不会等用户选**, 宏会继续往下跑。
- **通用选择列表** (`pick.rs` + `app/picks.rs`): 窗口一览、最近文件、字符集、帮助检索共用; 行里带 `value` (文档 id / 路径 / 行号), 选中时按值找目标, 不按行号回查。
- **帧内顺序** (`app.rs` 的 `update`): 分割视图时有一句「活动标签 = 焦点窗格的文档」; 会切换标签的窗口 (如选择列表) 必须放在它之后, 否则选择会被同一帧盖回去 (0.3.8 修过一次)。

## 6. 已知问题与风险

| # | 问题 | 依据 | 影响 |
|---|---|---|---|
| R1 | **(2026-11-04 已改, 待首次运行确认)** **CI 实际没有在跑**: `.github/workflows/ci.yml` 没有顶层 `jobs:` 键 (各 job 直接写在根上), GitHub 会判定工作流无效; 触发分支写的是 `develop`, 实际分支叫 `dev` | 实测 (YAML 解析结果没有 `jobs`) | PR 上「CI 通过」不代表跑过任何测试 |
| R2 | 即使修好结构, CI 会立即红: `cargo fmt --check` (仓库没按 rustfmt 格式化, ide-fast 约 1700 处差异)、`clippy -D warnings`、`RUSTFLAGS=-D warnings`、Linux runner 编不过 ide-fast、私有依赖 `aci-emitter` | 实测 | 需要分步收紧, 见 WBS-03 O1 |
| R3 | **新 UI 都没在真窗口里验证**: 0.3.2 起每批的菜单项、首选项页、补全浮层、选择列表外观、帮助菜单、AI 设置、日 / 英界面的排版 | 云端会话没有 Windows | 可能有布局 / 交互问题 |
| R4 | 0.3.2 – 0.3.9 没有标签、没有安装包, 应用内「检查更新」看不到这些版本 | 实测 | 用户拿不到 |
| R5 | `ide-fast/docs/PARITY-PLAN.md` 有 50 处 "not done", 很多已过期 (如插入日期 / 时间、F6 范围选择、base64、copy-html、剪贴板历史都已实现); `ROADMAP.md` 的「没有做」表同样过期 | 实测 (逐项与代码对照过其中十余项) | 新人会误判剩余工作 |
| R6 | 仓库根有与本项目无关的内容被跟踪: `gemarc-5d-atlas-v10.0/` (约 140 个文件)、`_upstream/`、`refs/`、`req.jsonl` | 实测 (`git ls-files`) | 克隆变大、搜索噪音; 删除前需要所有者确认 |
| R7 | 更新包签名私钥、SignPath 代码签名都依赖仓库外的人工步骤 | `UPDATE.md` / `release-windows.yml` | 发布无法全自动 |

## 7. 未做 / 有意不做

- **做不到**: IME 再変換 (egui 没有触发 TSF 再変換的接口)。
- **按决定不做**: JavaScript 宏、Vim 模式、SQL*Plus、层叠 / 平铺窗口 (单窗口多标签, Tauri 版也没有)。
- **已知性能项**: 100 万字符的单行稳态帧约 86 ms (`PERF.md`), 正常文件不受影响。
- **保持原样的非翻译文字**: 发给 AI 的系统提示词、Sakura 风格的状态栏「挿入 / 上書」、Grep 结果文档里的日文、终端 shell 输出。

## 8. 接手后的前五件事

1. 合并 [PR #21](https://github.com/UlyssesLeoLee/IDE1.0/pull/21) (0.3.9), 在本机推标签 `v0.3.2` – `v0.3.9` (命令见第 3 节)。
2. 修 CI 结构, 让 PR 上真的跑 `cargo test -p ide-fast` (WBS-03 O1.1 – O1.4)。
3. 在 Windows 真机上按 WBS-03 O3.1 的清单把 0.3.2 – 0.3.9 的新 UI 过一遍, 尤其是日 / 英界面。
4. 构建高速版 MSI / 便携 exe 并签名, 发一次 Release (WBS-03 O2)。
5. 按代码重新校准 `PARITY-PLAN.md` / `ROADMAP.md` (WBS-03 O5.1), 再决定下一批功能。
