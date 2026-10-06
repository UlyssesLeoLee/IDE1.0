# IDE1.0

> **Stage 3.0 最小骨架 + IDE Shell 骨架** — per ULYS-191 §3 + §4.2.2 brief v0.1.
> 完整 microkernel / session / transaction buffer / search / plugin loader / TUI / txn
> 实装待后续 brief (per DD-01~04 详细设计书).

## §0 三阶段累计交付 (per 2026-10-03 dev 收尾)

| 阶段 | 内容 | 交付 commit / PR | 状态 |
|---|---|---|---|
| **Stage 1** | `.aci.json` schema v0.1 (跨项目 1:1) | Star PR #93 → 已 ship main + dev | ✅ |
| **Stage 2** | Rust `aci-emitter v0.1.0` (10 必填字段 + 4 layer + 5 severity + 4 status + 17 expect_value_types) | [ULYS-224](https://github.com/UlyssesLeoLee/aci-emitter) rev=df28c56 | ✅ |
| **Stage 3.0** | 最小 Cargo skeleton + `ide-kernel-core` + `ide-cli` + `aci-emitter` 集成 | PR #2 (5af5e7c) | ✅ MERGED |
| **§4.4 L1+L2** | mock `cluster_switch` + `plugin_switch` (2 plugin) | PR #3 (c4b62bb) | ✅ MERGED |
| **§4.4 L3** | mock `module_switch` Stage 1 (8 module: ide-cli×4 + ide-kernel-core×4) | PR #4 (54aacf0) | ✅ MERGED |
| **§4.4 L3 收尾** | regression report 标 IDE1.0 ✅ MERGED,跨项目 5/7 累计 | 48425a2 | ✅ MERGED (含 PR #5 squash) |
| **§4.4 stage6** | mock_switch 11 边界/失败用例 (Python `unittest`) + CI `mock-switch-validate` job | PR #5 (15beb40) | ✅ MERGED |
| **IDE Shell** | `crates/ide-shell` 骨架 — TUI (ratatui) + AI bridge (PlaceholderAi) + Vim normal/insert/command + 鼠标 (crossterm EnableMouseCapture) — 30 UT | PR #6 (f0753ab) | ✅ MERGED |
| **Lint 全清** | `.markdownlint.json` 关 MD060 (版本漂移噪音 141) + 修 16 真错 → 0 issues | PR #7 (42212f4) | ✅ MERGED |
| **四层测试矩阵** | UT (61 Rust) + IT (跨 crate 10) + ST (cli_smoke + 26 Python mock) + **UAT (Playwright e2e 40 case, `crates/ide-shell-web` web 渲染)** | PR #9 (本笔) | 🟡 OPEN |

测试矩阵 (本机 rustc 1.98.1 全绿):

- Rust: `cargo test --workspace --all-targets` → **61 测试** (3 ide-cli + 3 ide-cli IT + 2 ide-kernel-core + 38 ide-shell + 8 ide-shell-web UT + 7 ide-shell-web IT)
- Python mock: `test_ide1_mock_switch.py` → **15** + `test_ide1_mock_switch_edge.py` → **11** = **26**
- System: `cli_smoke.sh` → **4/4 PASS**
- UAT (Playwright): `npx playwright test` → **40/40 PASS** (chromium, 6 spec 文件)
- Markdown lint: **0 issues / exit 0**
- CI: **9 jobs** (Rust + Integration + Mock-switch-validate + UAT-Playwright + Cross-language parity + Markdown lint + Rust bench + Cross-project smoke + CodeRabbit)

## §1 简介

**IDE1.0** = 纯 Rust AI Native CLI Development Kernel (per 需求规格书).
Stage 1 (Star `_lib_aci_emit.py` + `.aci.json` schema v0.1) 已 ship,
Stage 2 (Rust `aci-emitter v0.1.0`) 已 ship ([ULYS-224](https://github.com/UlyssesLeoLee/aci-emitter)).

本仓 (IDE1.0) 当前提供:

- **底层 kernel (Stage 3.0 最小骨架)**:
  - 最小 Cargo workspace (4 crates: `ide-kernel-core` + `ide-cli` + **`ide-shell`** + **`ide-shell-web`**)
  - 3 个 placeholder integration tests (`tests/integration.rs`)
  - Bash smoke (`tests/st/cli_smoke.sh`)
  - 9 jobs GitHub Actions CI (含 UAT Playwright job)
  - `.aci.json` schema v0.1 (1:1 拷贝自 Star)
  - 集成设计文档 (`docs/architecture/aci-integration.md`)

- **IDE Shell 骨架 (ULYS-191 §3, 同产品双外观)**:
  - `crates/ide-shell` — TUI + AI + Vim + 鼠标 (PowerShell 替代/增强内核共用)
  - `crates/ide-shell-web` — 同内核 web 渲染 + HTTP server (Playwright UAT target)
  - `tests/uat/` — Playwright e2e 40 case (UT/IT/ST/UAT 四层矩阵补齐)
  - 见 [§8 IDE Shell 骨架](#8-ide-shell-骨架-ulys-191-3) + [§9 四层测试矩阵](#9-四层测试矩阵-utitstuat)

⚠️ **本笔仅最小骨架**. 完整内核实装待后续 brief (Stage 3 方案 B).

## §2 快速开始

```bash
# 1. 编译
cargo build --workspace

# 2. 单元 + 集成测试
cargo test --workspace --all-targets -j 4

# 3. CLI smoke (调 ide-cli emit 一条 ACI assertion)
bash tests/st/cli_smoke.sh

# 4. Lint (严, -D warnings)
cargo clippy --workspace --all-targets --no-deps -- -D warnings

# 5. Format check
cargo fmt --all -- --check
```

CLI 用法:

```bash
# 打印版本
cargo run --quiet -p ide-cli -- --version

# Emit 一条 ACI assertion
cargo run --quiet -p ide-cli -- --aci-emit ide1.0:smoke:smoke-1

# 启动 IDE Shell demo (TUI + Vim + AI placeholder)
cargo run --quiet -p ide-shell --bin ide-shell
```

## §3 ACI 接口 (per §4.2.3)

IDE1.0 集成 [`aci-emitter v0.1.0`](https://github.com/UlyssesLeoLee/aci-emitter) (ULYS-224 已 ship).
所有 ACI assertion 必须符合 `.aci.json` schema v0.1:

- **10 必填字段**: `assertion_id`, `aci_version`, `layer`, `scope`, `expect`, `actual`, `status`, `severity`, `reasoning`, `captured_at`
- **4 测试层**: `ut`, `it`, `st`, `e2e`
- **5 严重度**: `critical`, `high`, `medium`, `low`, `info`
- **4 状态**: `PASS`, `FAIL`, `WARN`, `SKIP`
- **17 expect_value_types**: `response_within_ms`, `response_status_2xx`, `field_equals`, ...

详细字段语义见 [`.aci.json`](./.aci.json).

### 3.1 CLI emit 示例输出

```bash
$ ide-cli --aci-emit ide1.0:smoke:smoke-1
```

输出示例 (sort_keys, ACI v0.1 — 字段按字典序):

```json
{
  "aci_version": "0.1.0-draft",
  "actual": {
    "description": "measured 5ms (placeholder)",
    "type": "response_within_ms",
    "value": 5
  },
  "assertion_id": "ide1.0:smoke:smoke-1",
  "captured_at": "2026-09-24T...",
  "expect": {
    "description": "CLI should start within 100ms",
    "type": "response_within_ms",
    "value": 100
  },
  "layer": "it",
  "reasoning": "actual << expect (20x margin) — placeholder per §4.2.2 brief v0.1",
  "scope": {
    "module": "smoke",
    "project": "ide1.0"
  },
  "severity": "info",
  "status": "PASS"
}
```

### 3.2 Rust 库用法

```rust
use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};

let em = AciEmitter::new(Layer::It);
let assertion = em.build(
    "ide1.0:it:example-1",
    Scope::new("ide1.0".into(), Some("it".into()), None, None, None, None),
    ExpectActual::new(ExpectValueType::WorkflowCompletes, serde_json::json!(true), "should complete"),
    ExpectActual::new(ExpectValueType::WorkflowCompletes, serde_json::json!(true), "did complete"),
    Status::Pass,
    Severity::Info,
    "placeholder",
).unwrap();
```

完整 API 见 [`aci-emitter` upstream docs](https://github.com/UlyssesLeoLee/aci-emitter).

## §4 架构

IDE1.0 当前 4 个详细设计书 (设计阶段, **未实装**):

| DD | 名称 | 状态 |
|---|---|---|
| DD-01 | Microkernel Core 详细设计書 | 设计 (实装待 ULYS-191 后续 brief) |
| DD-02 | Session Transaction Buffer Engine 详细設計書 | 设计 (实装待 ULYS-191 后续 brief) |
| DD-04 | Plugin Loader / Search / TUI | 设计 (实装待 ULYS-191 后续 brief) |

Stage 3.0 集成设计见 [`docs/architecture/aci-integration.md`](./docs/architecture/aci-integration.md).

## §5 守门合规 (13 项, per AGENTS.md §4)

- **#5** no secret leak ✅
- **#6** 中文默认 (README/architecture/regression report 全中文) ✅
- **#7** `unsafe_code = "forbid"` (workspace.lints.rust) ✅
- **#9** subprocess (cli_smoke.sh 调 `cargo run --aci-emit`, 不调用户脚本) ✅
- **#10** author=Ulysses ✅
- **#11** 缺标比错标 (7 deps 全 workspace + 显式 version, 1 git dep 锁 rev) ✅
- **#12** docs 同步 (README + architecture + regression report) ✅
- **#13** W/T/M (单元 + 集成 + 系统) ✅
- **#14v4** PR merge ✅
- **#15** scope creep (1 sub-agent 1 切点) ✅
- **#17** commit 完整 ✅
- **#19v19** Python 化 (IT-3 跨语言 parity 测) ✅
- **#24** vendor 中立 (7 deps + 1 git dep) ✅

## §6 已知缺口 (4 项)

- **G-ACI-03** 跨语言 emitter ≥4 种 (Python ✅ / Bash ✅ / Rust ✅ / TS ⏳ ULYS-191.7)
- **G-ACI-07** 部分项目可能无 mock (本笔 IDE1.0 = 最小骨架, 提供未来 mock 位置)
- **G-ACI-08 (新)** IDE1.0 首次引入 Cargo workspace, design docs 与 code 可能滞后
- **G-SHELL-01 (新, ULYS-191 §3)** IDE Shell 仅骨架,真实 LLM 接入 / 多行 buffer / 历史补全 / PSReadLine 插件形态 均留 Stage 4+

## §7 License

MIT OR Apache-2.0 (dual), per workspace.

## §8 IDE Shell 骨架 (ULYS-191 §3)

> **同产品双外观,内核相通,尽量在一起做** (per D-Boy 10/3 拍板).
> IDE1.0 仓既承载底层 Rust kernel,也在 `crates/ide-shell` 内做 PowerShell 替代/增强
> 内核的 TUI + AI + Vim + 鼠标 骨架。两种外观(全壳替代 vs PSReadLine 插件)共用 ide-shell 内核。

### 8.1 设计目标

| 维度 | 目标 |
|---|---|
| 内核 | 单仓 `crates/ide-shell` 复用 `ide-kernel-core` 提供 version/kernel_status |
| 外壳 1 (全壳替代) | `cargo run -p ide-shell --bin ide-shell` 启动 ratatui TUI,内置 PS-like prompt |
| 外壳 2 (插件形态) | (Stage 4+ 后续 brief,共内核不同 entry) |
| 键位 | Vim normal/insert/command 三态 (hjkl / i / : / Esc / q) |
| 鼠标 | crossterm `EnableMouseCapture` — 左键 Insert / 右键 Normal |
| AI | `AiBackend` trait + `PlaceholderAi` 占位(返回 `AI suggestion: <input>`);真实 LLM 接入 Stage 4+ |
| 桥接 | 复用 Stage 2 `aci-emitter v0.1` 走 .aci.json schema (ACI 必填字段不变) |

### 8.2 模块分层

```text
crates/ide-shell/
├── Cargo.toml                  # workspace 共享 deps, 锁 ratatui 0.28 + crossterm 0.28
└── src/
    ├── lib.rs                  # 模块声明 + App / Mode re-export
    ├── mode.rs                 # Vim 三态枚举 (serde Serialize/Deserialize)      + 2 UT
    ├── input.rs                # 编辑缓冲 (Vec<char> + cursor + 移动/删除)       + 7 UT
    ├── keymap.rs               # KeyEvent + Mode → Action 映射                   + 11 UT
    ├── ai.rs                   # AiBackend trait + PlaceholderAi                 + 2 UT
    ├── render/
    │   ├── mod.rs              # backend 声明 (ratatui_render + web)
    │   ├── ratatui_render.rs   # TTY backend — 三段布局 ratatui widgets
    │   └── web.rs              # Web backend — WebFrame 结构化数据 (JSON)        + 8 UT
    ├── app.rs                  # App 状态机 + 事件循环 + 双 buffer 执行           + 9 UT
    └── bin/
        └── ide-shell.rs        # TUI binary entry — 终端初始化 + 主循环

crates/ide-shell-web/
├── Cargo.toml                  # 复用 ide-shell core, 0 web framework dep (std TcpListener)
├── src/
│   ├── lib.rs                  # HTTP server (/ + /api/frame + /api/event + /api/reset) + 8 UT
│   ├── index.html              # DOM 渲染 (data-mode/data-buffer/data-cursor 属性供 e2e selector)
│   └── bin/
│       └── ide-shell-web.rs    # web server binary entry (默认 127.0.0.1:8123)
└── tests/
    └── integration.rs          # HTTP server 级 IT — 真 TCP 请求验证端点行为     + 7 IT
```

总计 **38 ide-shell 单测 + 8 ide-shell-web UT + 7 ide-shell-web IT**,全部 `cargo test --workspace` 通过。

两个 render backend 共享同一份 `&App` 状态 → TUI 与 Web 视觉/行为 parity (双外观内核相通)。

### 8.3 键位速查

| Mode | 键 | 动作 |
|---|---|---|
| Normal | `hjkl` / ←→ | 光标左/右 |
| Normal | `0` / `$` / Home / End | 行首/行尾 |
| Normal | `i` / `a` | 进 Insert |
| Normal | `:` | 进 Command (预置 `:`) |
| Normal | `x` | 删除右侧 |
| Normal | `q` | 退出 |
| Insert | 字符 | 插入 buffer |
| Insert | Backspace / Delete | 删左/右 |
| Insert | Enter | 执行 buffer 作为命令 |
| Insert | Esc | 回 Normal |
| Command | 字符 | 追加到 cmd buffer |
| Command | Enter | 执行 command(去掉 `:` 前缀) |
| Command | Esc | 回 Normal |
| 全局 | 鼠标左键 | 切 Insert |
| 全局 | 鼠标右键 | 切 Normal |

### 8.4 内置命令

| 命令 | 效果 |
|---|---|
| `:help` / `help` | 打印键位速查 |
| `:version` / `version` | 打印 IDE1.0 kernel banner |
| `:ai` / `ai` | 调 `ai_suggestion()` 当前 buffer(占位回显) |
| `:clear` / `clear` | 清空 history |
| `:q` / `quit` | 退出 |
| 其它 | 兜底走 `cmd /C <cmd>` (Windows) / `sh -c <cmd>` (Unix) 子进程执行 |

### 8.5 不在本笔范围 (Stage 4+)

- 真实 LLM 接入 (Anthropic / OpenAI / MiniMax 任选)
- 行内补全 ghost text 渲染
- 历史命令补全(`Up`/`Down` 翻历史)
- 多行 buffer (现仅单行)
- PowerShell 真实解析(现仅 shell 兜底)
- PSReadLine 插件形态 entry

## §9 四层测试矩阵 (UT/IT/ST/UAT)

> per 「UT,IT,ST,基于 playwright 的 UAT 都要做到位」(2026-10-03).

| 层 | 位置 | 数量 | 工具 | 覆盖 |
|---|---|---|---|---|
| **UT** (单元) | 各 crate `src/` `#[cfg(test)]` | **61** Rust + **26** Python | `cargo test` / `unittest` | mode 状态机 / input buffer / keymap / AI bridge / web render / HTTP server / mock_switch 数据形状 + 11 边界用例 |
| **IT** (集成) | `crates/ide-cli/tests/integration.rs` + `crates/ide-shell-web/tests/integration.rs` | **3 + 7** | `cargo test --all-targets` | ACI schema roundtrip / emitter 兼容性 / HTTP 端点真 TCP 行为 (frame/event/reset/404/quit 信号) |
| **ST** (系统) | `tests/st/cli_smoke.sh` | **4 step** | bash + cargo run | ide-cli 端到端 emit → 10 必填字段 → schema version |
| **UAT** (验收) | `tests/uat/specs/*.spec.ts` | **40** case / 6 spec | **Playwright 1.63 (chromium)** | 浏览器驱动完整用户流: 页面加载 / mode 切换 / Vim 键位 / 鼠标 / AI 建议 / 内置命令 / DOM 属性追踪 / 端到端工作流 |

### 9.1 UAT 架构

```text
Playwright (chromium)
    │  HTTP (fetch /api/event)          DOM (keydown / mousedown)
    ▼                                    ▼
ide-shell-web server ◄────────── index.html (data-mode/data-buffer 属性)
    │
    ▼
ide-shell core (App 状态机 — 与 TUI binary 100% 同源)
```

- Web backend 渲染 `WebFrame` JSON(与 ratatui backend 共享同一 `&App`),Playwright 双通道验证:API 层 (frame JSON) + DOM 层 (`data-*` 属性 / 键盘鼠标事件)。
- `webServer` 由 `playwright.config.ts` 自动启停(`reuseExistingServer` 本地开发复用)。

### 9.2 UAT 跑法

```bash
# 1. build web server binary
cargo build -p ide-shell-web --bin ide-shell-web

# 2. 装 deps (首次)
cd tests/uat && npm install && npx playwright install chromium

# 3. 跑 40 case (webServer 自动启停)
IDE_SHELL_WEB_BIN=../../target/debug/ide-shell-web npx playwright test
```

CI: `uat-playwright` job (ubuntu-latest, Node 20, chromium + browser cache).

## §10 Tauri Desktop 打包 (ULYS-191 §4)

> per 「我需要的是打包安装文件的桌面版」(2026-10-03).

把 IDE Shell 从 web (HTTP server) 升级为 **Tauri 2 桌面应用** — 用户双击 `.msi` 安装,启动后弹原生窗口。同 `ide-shell` core,双 frontend (web + desktop) 共享后端状态机。

### 10.1 架构

```text
                chromium (Playwright UAT)                    Tauri 2 webview (Windows/macOS/Linux)
                        │                                              │
                   fetch /api/*                                  invoke('frame' / 'key' / ...)
                        │                                              │
                        ▼                                              ▼
                ide-shell-web server                      ide-shell-desktop.exe (Tauri managed state)
                        │                                              │
                        └──────────┬───────────────────────────┘
                                   │
                                   ▼  共用同一 `ide-shell::App` 状态机 (38 UT)
                            crates/ide-shell
                                   │
                                   ▼
                            crates/ide-kernel-core
```

- **Tauri 2 已知限制** (per [issue #9362](https://github.com/tauri-apps/tauri/issues/9362)):
  `#[tauri::command]` 在 `lib.rs` root + 同 crate 有 `main.rs` 时 macro 重复定义。本笔绕法: commands 放 `lib.rs` root 但**不用 `pub`**(`__cmd__xxx` macro 是 private 不能跨 module),`invoke_handler!` 在同 module 引用。

- **前端代码复用**: `crates/ide-shell-web/src/index.html` 拷到 `crates/ide-shell-desktop/dist/index.html`,JS 加 ~10 行 adapter 检测 `window.__TAURI_INTERNALS__` 走 `invoke` vs `fetch`。`tests/uat/` Playwright 仍可跑 (走 fetch 分支) — 桌面/Tauri 与 web UAT 一份前端代码。

### 10.2 模块结构

```text
crates/ide-shell-desktop/
├── Cargo.toml              # 依赖 tauri 2.x + ide-shell + crossterm (for KeyEvent 类型)
├── build.rs                # tauri_build::build()
├── tauri.conf.json         # app 配置 (windows.bundle + identifier)
├── capabilities/default.json   # Tauri 2 必需
├── icons/                  # 占位 PNG/ICO (品牌待设计)
│   ├── 32x32.png  128x128.png  128x128@2x.png  icon.ico  Square30x30Logo.png
├── src/
│   ├── main.rs             # entry — ide_shell_desktop::run()
│   └── lib.rs              # ShellState + 6 commands + 5 UT
├── dist/index.html         # 复用 ide-shell-web 的 index.html + tauri adapter
└── tests/integration.rs   # 3 IT (state 并发 / KeyResponse shape / quit 信号)
```

### 10.3 6 个 Tauri commands

| Command | 签名 | 用途 |
|---|---|---|
| `frame` | `() -> WebFrame` | 拉当前 frame JSON |
| `reset` | `() -> WebFrame` | 重置 app |
| `key` | `(code, modifiers) -> KeyResponse` | 派发键盘事件 |
| `mouse` | `(button) -> WebFrame` | 派发鼠标点击 |
| `kernel_banner` | `() -> String` | 复用 ide-kernel-core |
| `get_mode` | `() -> String` | 取当前 mode (避开 Rust 关键字 `mode`) |

### 10.4 打包跑法

```bash
# 1. 装 Tauri CLI (一次性)
cd tools && npm install  # @tauri-apps/cli 2.12

# 2. 出 Windows .msi
cargo build --release -p ide-shell-desktop
cd crates/ide-shell-desktop && ../../../tools/node_modules/.bin/tauri build --bundles msi --config ./tauri.conf.json
# 产物: target/release/bundle/msi/IDE1.0 IDE Shell_0.1.0_x64_en-US.msi (~2.7 MB)

# 3. macOS / Linux 同理 (换 target)
npx tauri build --bundles appimage --config ./tauri.conf.json  # Linux
npx tauri build --bundles dmg --config ./tauri.conf.json        # macOS
```

### 10.5 验证 (本机 rustc 1.98.1 + Node 26)

| 步骤 | 结果 |
|---|---|
| `cargo test --workspace --all-targets` | ✅ **69 测试** (61 + 8 ide-shell-desktop) |
| `cargo build --release -p ide-shell-desktop` | ✅ 8.1 MB `ide-shell-desktop.exe` |
| `npx tauri build --bundles msi` | ✅ **2.7 MB MSI** (`IDE1.0 IDE Shell_0.1.0_x64_en-US.msi`) |
| `cargo clippy -D warnings` | ✅ 0 warning |
| `cargo fmt --check` | ✅ |
| `markdownlint` (CI 同款) | ✅ 0 issues |

CI 增量: 新增 `tauri-build-linux` job (ubuntu-latest, 出 `.deb` + `.AppImage`)。Windows MSI + macOS DMG 留给后续 brief (需要 Windows runner + Apple Developer ID)。

## Phase 3-6 累计交付 (ulys-191-15 ~ 19)

### Phase 3 — Editor 高级特性

- **3.1 Symbol Outline** (`384f5be`): Rust 源文件 `fn main()` 通过 sakura-rs `outline_dispatch` 解析 → JSON entries → Side Bar 显示 + click jump-to-line
- **3.2 Multi-cursor 视觉 v1** (`cb7b901`): Alt+Click 加光标 + Esc 清空 + status `N cursors` badge
- **3.3 MiniMap** (`cdac369`): 右侧代码缩略图 (line 长度条 + cursor 指示)
- **3.4 Breadcrumb** (`cdac369`): 顶部 path 显示 (drive / folders / file)
- **3.5 Folding gutter** (`cdac369`): ▼/▶ marker + click toggle + indent-based detection

### Phase 4 — AI 集成 (ulys-191-18)

- **ide-shell-protocol**: 22 JSON-RPC methods + 3 protocols (HTTP/WS/stdio)
- **Python client demo** (`examples/python_client_demo.py`): 演示 AI agent 通过 stdio/HTTP/WS 控制 IDE buffer
- **持久 StdioSession class**: 一个进程多 messages, 演示 `kernel_info`/`open_buffer`/`insert_at_cursor`/`get_buffer`/`edit_via_provider`/`chat` 完整流程

### Phase 5 — 商业化打磨 (ulys-191-19)

- **Theme toggle**: 🌙/☀️ 状态栏按钮 + CSS variable swap (Light/Dark)
- **Settings persistence**: Ctrl+= / Ctrl+- / Ctrl+0 font-size 调节, localStorage 保存
- **i18n**: English/中文 auto-detect (navigator.language), localStorage 持久

### Phase 6 — 测试 + CI (ulys-191-19)

- **sakura-rs bench**: `cargo run -p sakura-rs --example bench` — insert_str 10000 ops 175ms / **56,971 ops/s**
- **CI benchmark step**: rust-bench job 加 sakura-rs 吞吐量验证 (target ≥ 50k ops/s)
- **四层测试矩阵**: UT (cargo test --workspace) + IT (cargo test --tests) + ST (cli_smoke.sh) + UAT (playwright 68 cases)

### 累计 commits (dev-3)

```text
2b6e356 Phase 5 Theme + Settings + i18n
87e9b89 Phase 4.1 Python client demo
cdac36 Phase 3.3-3.5 MiniMap + Folding + Breadcrumb
cb7b901 Phase 3.2 Multi-cursor 视觉 v1
384f5be Phase 3.1 Outline 面板
34f64e6 web Terminal
2953aae Bottom Panel + Terminal (desktop)
0cb471e sakura-rs crate 全套
```
