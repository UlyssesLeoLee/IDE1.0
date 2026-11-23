# IDE1.0 Dedicated E2E Mock Project

> 本工程作为 IDE1.0 专属的独立标准测试空间，用于端到端自动验证 IDE1.0 全部核心功能点（并整合已有 Node / PowerShell 测试脚本与生产包冒烟自检）。

## 目录结构
- `feature-matrix.json`：功能点与自动化测试用例映射清单（`F-01` ~ `F-16`）
- `run-all-features.ps1`：一键全功能自动化测试总控脚本（输出报告到 `qa/mock-project-feature-report.json`）
- `CLAUDE.md`：Mock 工作区规则样本，用于测试 `Mod` / `WorkspaceRulesEngine` 上下文注入
- `.mcp.json`：Mock 外部 MCP 服务器配置样本（离线模式 `"disabled": true`）
- `src/`：Rust 源码样本（`main.rs`、`math.rs`、`service.rs`），用于 Diff、并发合并与多 Agent 并行编辑测试
- `knowledge/architecture.nema`：NEMA v1.5 格式数据（含 5W1H 与 Table 域），用于语法封闭性与语义合并验证
- `config/settings.json`：环境与模型配置样本
- `docs/`：`SPEC.md` 与 `markdown-preview.md`，用于 Markdown 预览、就地编辑及 Diff 测试
- `scripts/automation.rs`：Blender `bpy` 风格 `ide::*` Rust 控制台自动化脚本样本
- `fixtures/sticky-note-sample.json`：桌面便笺状态与格式化标准样本

## 覆盖功能点矩阵 (`F-01` ~ `F-16`)
| 编号 | 功能点 | 类别 | 验证目标 |
| :--- | :--- | :--- | :--- |
| `F-01` | Mock 工程资产完整性 | Workspace Fixtures | 所有 Fixture、JSON、`.mcp.json`、`CLAUDE.md`、`.nema` 结构校验 |
| `F-02` | 核心文本引擎与 API | Core Engine | `editcore` 缓冲、Myers Diff、编码转换、Agent API |
| `F-03` | NEMA 协议与封闭性 | NEMA Protocol | `nema-codec` 编解码与红绿语法测试 |
| `F-04` | Agent 路由与守卫 | Agent Router | `ide-agent-router` Skills/Hooks/Memory/LoopBreaker/沙盒 |
| `F-05` | Rust 控制台 DSL | Rust Console DSL | `ide-mcp` + `ide-fast` (`rust_console`) |
| `F-06` | 惰性插件后端运行时 | Agent Plugins | `agent_plugins` 热插拔门控、`installed_cache`、全子引擎卸载 |
| `F-07` | 单 Agent 对话与模型 | Single Agent | `aichat` Cookie 生命周期、热插拔与模型隔离 |
| `F-08` | 多 Agent 协同与对等 | Multi-Agent Collab | `collab` 并行区修改、独占 `LoopBreaker`、`patch_proposal` 与便笺工具 |
| `F-09` | 安装向导 UI 与行为 | Installer UI | `installer_ui` 协议优先、全屏最大化、无暗色遮罩 |
| `F-10` | 桌面便笺 | Sticky Notes | `sticky_notes` CRUD、Markdown 就地编辑与调色板 |
| `F-11` | 终端子系统 | Terminal | `terminal` ANSI TrueColor、CLIXML 与 OSC 133 |
| `F-12` | 主题视图与 i18n | UI & i18n | `view::tests` 与 `every_literal_status_tip_has_a_message_template` |
| `F-13` | 云端 MCP 单元测试 | Serverless MCP | 整合 `tests/mcp-vercel-unit.test.mjs` |
| `F-14` | Git/NEMA 协同 E2E | Collab Engine E2E | `mock-project/collab-modes-e2e.mjs`（实测 `math.rs`/`service.rs`/`architecture.nema`） |
| `F-15` | 格式与静态分析门禁 | Repo Quality Gate | `cargo fmt --check` + `cargo clippy --all-targets -D warnings` |
| `F-16` | 二进制冒烟自检 | Release Smoke Test | 已打包 EXE `--smoke-test`（未打包或 `-SkipBinaryLaunch` 时自动跳过） |
| `F-17` | 真实 GUI 窗口自动实测 | Live GUI E2E | 启动真实 `IDE1.0` 窗口自动执行 `--e2e-script` 全部交互流程 |
| `F-18` | 核心模块覆盖率门禁 | Coverage Gate | 采集 `api/v1` 分支覆盖率与 `nema-codec`/`ide-mcp` `llvm-cov` 报告并校验基线门禁 |

## 常用运行命令

```powershell
# 1. 查看 F-01 ~ F-16 完整功能点清单
powershell -ExecutionPolicy Bypass -File tests/mock-project/run-all-features.ps1 -List

# 2. 按 F-01 -> F-16 顺序逐次全量验证（遇错即停，实时落盘每一步结果）
powershell -ExecutionPolicy Bypass -File tests/mock-project/run-all-features.ps1 -StopOnFailure

# 3. 人工逐项确认式步进验收（每项完成后按 Enter 继续）
powershell -ExecutionPolicy Bypass -File tests/mock-project/run-all-features.ps1 -StepByStep

# 4. 从指定功能点继续向后顺序验证（断点续测）
powershell -ExecutionPolicy Bypass -File tests/mock-project/run-all-features.ps1 -From F-08 -StopOnFailure

# 5. 运行快速核心回归子集（FAST 项）
powershell -ExecutionPolicy Bypass -File tests/mock-project/run-all-features.ps1 -Fast
```
