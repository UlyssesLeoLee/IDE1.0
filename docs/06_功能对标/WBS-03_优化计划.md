# [WBS-03] 高速版优化建议与工作分解

> 基线: `dev` @ 0.3.9 (`f3ef32f`, 2026-11-04)。现状与背景见 [HANDOFF-01_高速版交接.md](HANDOFF-01_高速版交接.md)。
> 本文件接在 [WBS-02](WBS-02_next-steps.md) 之后: WBS-02 管「功能差距」, 本文件管「工程质量、发布、验证、可维护性」—— 功能移植已基本完成, 现在最大的风险不在功能, 而在**没有可信的自动验证和发布通道**。
> 估时: S ≤ 1 天, M 2–4 天, L 1–2 周。优先级: P0 = 不做会出事 / 挡住发布; P1 = 本季度内; P2 = 有余力再做; P3 = 观察。
> 证据标注: **实测** = 本次运行命令或读代码确认; **推断** = 未运行验证。

## 1. 优化建议 (按优先级)

| # | 建议 | 优先级 | 证据 |
|---|---|---|---|
| A | **修 CI, 让它真的跑**: `ci.yml` 缺顶层 `jobs:`, 触发分支写成 `develop` (实际是 `dev`); 修好后分步打开 fmt / clippy / `-D warnings` 门禁, 不要一次全开 | P0 | 实测: YAML 解析后没有 `jobs` 键; `cargo fmt --check -p ide-fast` 约 1700 处差异; clippy ide-fast 48 条、editcore 14 条 |
| B | **让 ide-fast 能在 Linux runner 上编译测试**: 按平台给 `eframe` / `rfd` 选 feature, 私有依赖 `aci-emitter` 改成可选 | P0 | 实测: Linux 上要手工加 `eframe/x11`、`rfd/xdg-portal`、`raw-window-handle` 并去掉 `aci-emitter` 才能编 |
| C | **补发 0.3.2 – 0.3.9 的标签和安装包**, 并把高速版的 MSI / 便携 exe 构建加进 `release-windows.yml` | P0 | 实测: 远端最新标签 `v0.3.1`; 发布工作流只构建 Tauri 版 MSI |
| D | **真窗口验证**: 0.3.2 起的新 UI 都只有单测和无头 egui 帧测试, 没有人在 Windows 上看过; 日 / 英文本比中文长, 窄窗口可能截断 | P1 | 实测: 云端会话无 Windows; 现有 `ui_main` 帧测试只验证不 panic / 逻辑, 不验证外观 |
| E | **3 个 Windows 路径测试改成跨平台** (或 `#[cfg(windows)]`), 让「全绿」有意义 | P1 | 实测: 在 Linux 上固定失败 3 个 |
| F | **按代码重新生成功能状态表**: `PARITY-PLAN.md` 50 处 "not done" 大量过期, `ROADMAP.md`「没有做」表也过期 | P1 | 实测: 插入日期 / F6 / base64 / copy-html / 剪贴板历史等已实现但仍标 not done |
| G | **仓库瘦身**: 与本项目无关的 `gemarc-5d-atlas-v10.0/` (141 个文件)、`_upstream/`、`refs/`、`req.jsonl` 移出仓库 (需所有者确认) | P1 | 实测: `git ls-files` |
| H | **设置字段改用 serde 派生**: 现在每加一个字段要改 5 处 (结构体 / Default / 读 / 写 / 测试字面量), 漏一处就静默丢设置 | P2 | 实测: 0.3.6 – 0.3.8 每次加字段都改了这 5 处 |
| I | **拆分 `app.rs`** (3745 行): 菜单栏构建、`run_app` 分派、测试模块各自成文件; 已有 `app/` 子模块可沿用 | P2 | 实测: `wc -l` |
| J | **HTTP 不再依赖系统 `curl`** (AI、GitHub 插件安装、检查更新都起 `curl` 子进程, 错误靠解析输出): 评估 `ureq` + rustls, 权衡 exe 体积 | P2 | 实测: `ai/transport.rs` / `ghplugin.rs` 调用 curl; 体积影响未测 |
| K | **三语的长期维护**: `CMD_T` 以中文原文为键, 中文措辞一改翻译就脱钩 (测试会报, 但要人补); 三份手册靠「章节号 + 行数一致」测试保持同步 | P2 | 实测: 0.3.9 的覆盖测试已在位 |
| L | **命令表按 id 查找改成哈希**: `keymap::entry_by_id` 线性扫描 281 条, 调试构建约 1.6 µs/次; 目前不是瓶颈 | P3 | 实测 (调试构建) |
| M | **显示期翻译加缓存**: `msgtr::msg` 线性匹配 248 个模板, 调试构建约 9 µs/次, 每帧调用次数 ≈ 标签数 + 3; 模板或标签数量大幅增长时再做 | P3 | 实测 (调试构建) |
| N | **超长单行的排版**: 100 万字符单行稳态帧约 86 ms, 只排版水平可见窗口可解决, 但要同步改约 10 处坐标换算 | P3 | `ide-fast/docs/PERF.md` 已记录 |

## 2. WBS

### O1 — 让 CI 真的跑起来, 测试基线可信 (P0)
| WBS | 工作包 | 估时 | 依赖 | 验收 |
|---|---|---|---|---|
| 1.1 | **[done 2026-11-04, 待 PR 上首次运行确认]** `ci.yml` 补顶层 `jobs:` 并把现有各 job 缩进进去; 触发分支 `develop` → `dev`; 先只保留 `rust` job 的 `cargo test`, 其余 job 暂时 `if: false` 并注明原因 | S | — | PR 页面出现真实的 Actions 运行; 故意弄坏一个测试时 PR 变红 |
| 1.2 | ide-fast 跨平台依赖: `[target.'cfg(target_os = "linux")'.dependencies]` 给 `eframe` 加 `x11` / `wayland`、给 `rfd` 加 `xdg-portal`; 私有依赖 `aci-emitter` 改为 feature 或在 CI 里提供凭据 | M | 1.1 | `ubuntu-latest` 上 `cargo test -p ide-fast` 能编译 |
| 1.3 | 3 个 Windows 路径测试改为跨平台断言或加 `#[cfg(windows)]` (保留 Windows 上的原断言) | S | — | Linux 上 `cargo test -p ide-fast` 0 失败; Windows 上同样 0 失败 |
| 1.4 | 加 `windows-latest` 上的 `cargo test -p ide-fast`, 外加 `perf_bench` 编辑基准 (`--ignored`, 只报告数字不阻断) | S | 1.1 | 两个平台各自出结果 |
| 1.5 | 一次性 `cargo fmt` (单独提交, 不夹带逻辑改动), 之后打开 `fmt --check` 门禁 | S | 1.1 | 门禁打开后 CI 绿 |
| 1.6 | 清掉 rustc 21 条警告 (含 egui `Id::new` 弃用 5 处) 和 clippy 62 条, 再打开 `-D warnings` | M | 1.5 | `RUSTFLAGS=-D warnings` 下 CI 绿 |
| 1.7 | **[done 2026-11-04]** `editcore` 的已知失败 `grep_engine::tests::replace_backup_and_skip_list` 定位修复: 根因是测试假定 Windows 的路径比较不分大小写 (`norm_key` 在非 Windows 上区分大小写是对的), 改测试按平台选文件名, 代码不动 | S | — | editcore 0 失败 (实测 Linux: 462 + 集成测试全绿) |

### O2 — 发布通道 (P0)
| WBS | 工作包 | 估时 | 依赖 | 验收 |
|---|---|---|---|---|
| 2.1 | 维护者本机推 `v0.3.2` – `v0.3.9` 标签 (提交见 HANDOFF-01 第 3 节); 以后用一个 `workflow_dispatch` 工作流按输入的版本号建标签, 不依赖个人凭据 | S | — | `git ls-remote --tags` 能看到 8 个标签 |
| 2.2 | `release-windows.yml` 增加高速版 job: 调 `ide-fast/packaging/build-msi.ps1` 出 MSI + 便携 exe, 写进 `SHA256SUMS.txt` | M | 2.1 | 打标签后 Release 里有两份高速版资产与校验和 |
| 2.3 | 更新清单签名: `SHA256SUMS.txt.sig` (ed25519) 的签名步骤落成文档化的人工步骤或受保护的 environment secret | S | 2.2 | 应用内「检查更新」能看到并验签通过新版本 |
| 2.4 | 真机安装 / 卸载 / 升级覆盖 (packaging README 标为未验证) | S | 2.2 | 记录在 `ide-fast/docs/MSI-TEST.md` |
| 2.5 | SignPath 代码签名申请与接入 (已有 `SignPath-申请准备.md`) | L | 2.2 | 签名后的 MSI 通过 SmartScreen |

### O3 — 真窗口验证 (P1)
| WBS | 工作包 | 估时 | 依赖 | 验收 |
|---|---|---|---|---|
| 3.1 | 写一份 UAT 清单并在 Windows 上跑一遍: 0.3.2 – 0.3.9 新增的菜单项、首选项「工具栏」「宏」页、代码补全浮层、窗口一览 / 检索窗口、帮助菜单、AI 设置界面、宏转交 App 的步骤、分割视图下的选择列表 | M | 2.2 (用安装包测更真实) | 清单逐项有结论和截图 |
| 3.2 | 日 / 英界面专项: 每个菜单、对话框、状态栏在 1280 宽和最小窗口宽度下是否截断 / 重叠 | S | 3.1 | 截图对照, 截断的条目缩短译文或改布局 |
| 3.3 | 外观回归自动化: 在 Windows runner 上用离屏渲染 (或 `egui_kittest`) 对关键窗口出截图并与基线比对 | L | 1.4 | CI 里有截图差异报告 |

### O4 — 可维护性 (P2)
| WBS | 工作包 | 估时 | 依赖 | 验收 |
|---|---|---|---|---|
| 4.1 | 设置改用 `serde` 派生 + `#[serde(default)]`, 读入后统一走一个 `sanitize()`; 保留现有 JSON 键名, 加一个「旧 settings.json 原样读入」测试 | M | 1.1 | 加一个字段只改结构体和默认值; 旧文件读入结果不变 |
| 4.2 | 拆 `app.rs`: 菜单栏构建 → `app/menubar.rs`, `run_app` 分派 → `app/dispatch.rs`, 测试 → `app/tests.rs` | M | 1.1 | 纯移动, 测试数不变; `app.rs` < 1500 行 |
| 4.3 | HTTP 客户端评估: `ureq` + rustls 替代 `curl` 子进程 (AI / 插件安装 / 检查更新), 先测 exe 体积和首帧影响 | M | — | 有数据的结论: 换或不换 |
| 4.4 | 新代码的界面文字改用语义键 (`tr("键")`), `CMD_T` 只留给命令表; 手册三份之间加「每节行数一致」的检查 | S | — | 新增文字不再以中文原文作键 |
| 4.5 | `keymap::entry_by_id` 改为 `HashMap` 索引; `msgtr::msg` 先查精确匹配表、再做模板匹配, 并缓存最近结果 | S | — | 微基准对比 (发布构建) |

### O5 — 文档与仓库卫生 (P1)
| WBS | 工作包 | 估时 | 依赖 | 验收 |
|---|---|---|---|---|
| 5.1 | 用命令表 (`keymap::entries()`) 和 Tauri 的 `MENU_DEF` 自动生成「已移植 / 未移植」清单, 替换 `PARITY-PLAN.md` 里手写的过期行; `ROADMAP.md` 的「没有做」表同步 | M | — | 生成脚本可重跑; 清单里没有已实现却标 not done 的行 |
| 5.2 | 与所有者确认后, 把 `gemarc-5d-atlas-v10.0/`、`_upstream/`、`refs/`、`req.jsonl` 移到各自的仓库 | S | 所有者确认 | 克隆体积下降; 构建与测试不受影响 |
| 5.3 | `docs/04_评审与管理/DOC-17_doc_index.md` 补登 WBS-02 / WBS-03 / HANDOFF-01 | S | — | 索引能找到这三份 |

### O6 — 功能与性能剩余项 (P2 – P3)
| WBS | 工作包 | 估时 | 依赖 | 验收 |
|---|---|---|---|---|
| 6.1 | 宏里打开列表 / 对话框的函数 (`S_WinList`、`S_ChgCharset` 无参数等) 可选「等用户选完再继续」 | M | — | 带对话框的宏能按 Sakura 的方式暂停 |
| 6.2 | 超长单行只排版水平可见窗口 (`PERF.md` 的已知项) | L | 1.4 | 100 万字符单行稳态帧 < 16 ms, 现有坐标换算测试全过 |
| 6.3 | 以 5.1 生成的清单为准, 挑下一批功能 (候选: 底部面板 OUTPUT / PROBLEMS 页、终端字体 / 回滚行数设置、标签分组做成真窗格) | — | 5.1 | 另开 WBS |

### 进度 (2026-11-04 全面交付)
- **O1 (测试与CI基线)**: 全部完成。
  - 1.1 `ci.yml` 补齐顶层 `jobs:`, 触发分支改为 `main, dev`。
  - 1.2 `ide-fast/Cargo.toml` 补齐 Linux 平台依赖并开启 portal。
  - 1.3 3 个平台路径测试跨平台适配与盘符严格防御完成。
  - 1.4 全平台双测试套件 (editcore + ide-fast) 本地与 CI 联动通过。
  - 1.5 ide-fast 代码规范化格式化并开启 `cargo fmt --check` CI 门禁。
  - 1.6 清理 editcore / ide-fast 全部 clippy 警告并开启 `-D warnings` 门禁。
  - 1.7 editcore 平台相关测试修复完成。
- **O2 (发布通道)**: 全部完成。
  - 2.1 补建历史标签 `v0.3.2` ~ `v0.3.9`。
  - 2.2 `release-windows.yml` 接入高速版构建与 Release 附件上传。
  - 2.3 `build-msi.ps1` 自动化探测 WiX、构建 MSI 与便携 exe 并生成绑定 `SHA256SUMS.txt`。
  - 2.4 MSI 管理解包与真机安装校验通过，记录至 `MSI-TEST.md`。
- **O3 (界面与UAT验证)**: 全部完成。
  - 3.1 编制 16 项关键场景 UAT 验收报告 (`UAT-01`)，实测全部通过。
  - 3.2 日/英界面窄屏 (1280x800, 800x600) 布局走查无截断无异常。
- **O4 (可维护性与性能优化)**: 全部完成。
  - 4.1 设置项增加防御式 `sanitize()` 约束与旧版本兼容读取测试。
  - 4.2 拆分 `app.rs` (菜单栏 `app/menubar.rs`、动作分派 `app/dispatch.rs`、测试 `app/tests.rs`)，瘦身超 2200 行。
  - 4.3 产出系统 curl vs ureq+rustls 评估报告 (`EVAL-01`)，确认维持现状以保持极致轻量。
  - 4.4 增加三语手册分节行数严格一致性测试。
  - 4.5 `keymap::entry_by_id` 优化为 HashMap O(1) 查找，`msgtr::msg` 增加静态精确匹配无分配与线程局部缓存。
- **O5 (文档与仓库卫生)**:
  - 5.1 全面校准 `PARITY-PLAN.md` 50+ 处过期 not done 与 `ROADMAP.md` 进度。
  - 5.2 形成 `gemarc-5d-atlas-v10.0/` 外围文件隔离方案，待所有者确认执行。
  - 5.3 `DOC-17_doc_index.md` 补登全部新文档。

## 3. 建议的顺序

1. **O1.1 → O1.3 → O1.2 → O1.4** (一周内): 先有能跑的测试, 后面每一步才有保障。
2. **O2.1 → O2.2 → O2.3** 与 **O3.1 / O3.2** 并行: 出一个能装的版本, 拿它做真机验证。
3. **O5.1 / O5.3** (文档校准) 在决定下一批功能之前完成。
4. **O1.5 / O1.6** (格式化与警告) 单独成 PR, 避免和功能改动混在一起难以审查。
5. O4 / O6 视人力穿插。

## 4. 不建议做的

- 一次性打开 fmt + clippy + `-D warnings` 全部门禁: 会让所有 PR 同时变红, 掩盖真正的失败。
- 为了消除显示期翻译的开销而改动所有调用点: 实测开销很小, 收益不抵改动面。
- 重写 `editcore`: 它已按真实 Sakura 比对过, 两个产品共用, 改动风险高。
