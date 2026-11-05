# IDE1.0 Web — Agent Router & Worker 工作分解结构书 (WBS)

**文档编号**: WBS-ROUTER-2026-V2.0  
**制定组织**: IDE1.0 项目推进委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.1  
**状态**: 承认済 (Baseline Locked & Phase 1 Delivered)

---

## 1. WBS 分解与任务追踪矩阵

| WBS 编码 | 任务名称 | 成果物 | 对应要件 | 责任人 | 状态 |
|:---|:---|:---|:---|:---|:---|
| **WBS-01** | **IPA 架构设计基线** | | | | |
| WBS-01.1 | 要件基线定义书制定 | `docs/00_要件基线/Web-Agent-Router-Requirements.md` | `REQ-ROUTER-*` | Ulysses | **完成 (Done)** |
| WBS-01.2 | 基本设计书制定 | `docs/01_基本设计/Web-Agent-Router-Basic-Design.md` | `REQ-ARCH-*` | Mavis | **完成 (Done)** |
| WBS-01.3 | 语义单元与协调协议详细设计 | `docs/02_详细设计/Semantic-Units-and-Commit-Protocol.md` | `REQ-SEM-*` | Mavis | **完成 (Done)** |
| WBS-01.4 | OpenAPI 3.1 接口契约制定 | `docs/02_详细设计/Router-Worker-API.openapi.yaml` | `REQ-API-*` | Ulysses | **完成 (Done)** |
| WBS-01.5 | Postgres 数据模式与状态机设计 | `docs/02_详细设计/Postgres-Schema-and-State-Machines.md` | `REQ-DB-*` | Ulysses | **完成 (Done)** |
| WBS-01.6 | 测试规格书与追溯矩阵 | `docs/03_测试资料/UT-IT-ST-UAT-Web-Agent-Router.xlsx` | `REQ-TEST-*` | Mavis | **完成 (Done)** |
| WBS-01.7 | Vercel 部署与故障恢复手册 | `docs/05_实装与运维/Vercel-Deployment-and-Recovery.md` | `REQ-OPS-*` | Ulysses | **完成 (Done)** |
| **WBS-02** | **Rust 核心引擎实现** | | | | |
| WBS-02.1 | `crates/ide-agent-router` 骨架建立 | `Cargo.toml`, 模块结构 | `NRP-01` | Mavis | **完成 (Done)** |
| WBS-02.2 | Tree-sitter 多语言符号提取与指纹计算 | `src/parser.rs`, `src/identity.rs` | `REQ-SEMANTIC-01~04` | Mavis | **完成 (Done)** |
| WBS-02.3 | AST-aware Patch 应用与锚点重定位 | `src/patch.rs` | `REQ-COORD-03` | Mavis | **完成 (Done)** |
| WBS-02.4 | CAS 提交协调器与 Fencing 租约机 | `src/coordinator.rs` | `REQ-COORD-01~05` | Ulysses | **完成 (Done)** |
| WBS-02.5 | 协调器单例与内存状态模型 | `src/coordinator.rs` | `REQ-DB-*` | Ulysses | **完成 (Done)** |
| **WBS-03** | **API 网关与 SDK 交付** | | | | |
| WBS-03.1 | HTTP / REST API 服务端实现 (Serverless) | `api/v1/` 路由体系 (dispatch, claim, commit, events) | `REQ-API-01~02` | Ulysses | **完成 (Done)** |
| WBS-03.2 | Agent SDK 客户端 (TypeScript / Python) | `sdk/typescript/`, `sdk/python/` | `REQ-API-03` | Ulysses | **完成 (Done)** |
| **WBS-04** | **Vercel 部署与生产验证** | | | | |
| WBS-04.1 | Vercel 部署配置与静态/Serverless 绑定 | `vercel.json` | `REQ-VERCEL-01` | Ulysses | **完成 (Done)** |
| WBS-04.2 | 生产部署上线并获取公开链接 | `https://ide1-0.vercel.app` | `REQ-VERCEL-02` | Ulysses | **完成 (Done)** |
| WBS-04.3 | 多 Agent 并发与冲突自动化验证 | `crates/ide-agent-router/tests/` (7/7项全绿) | `NFR-CONC-*` | Mavis | **完成 (Done)** |
| WBS-04.4 | 全局文档与 GitHub README 升级 | `README.md`, `CHANGELOG.md` (v0.4.0) | `NRP-02` | Mavis | **完成 (Done)** |
| **WBS-05** | **Postgres 分布式账本与 Outbox 异步派发器** | | | | |
| WBS-05.1 | `LedgerStore` Trait 与内存/SQL 适配实现 | `crates/ide-agent-router/src/ledger.rs` | `REQ-DB-01~05` | Ulysses | **完成 (Done)** |
| WBS-05.2 | Transactional Outbox 轮询与异步事件派发 | `crates/ide-agent-router/src/outbox.rs` | `REQ-COORD-04` | Ulysses | **完成 (Done)** |
| **WBS-06** | **Worker 隔离沙盒环境与差异验证驱动** | | | | |
| WBS-06.1 | `WorkerSandbox` 隔离文件系统驱动与临时工作区 | `crates/ide-agent-router/src/sandbox.rs` | `REQ-EXEC-01` | Mavis | **完成 (Done)** |
| WBS-06.2 | 编译/测试验证引擎与 Patch 验证流水线 | `crates/ide-agent-router/src/verifier.rs` | `REQ-EXEC-02` | Mavis | **完成 (Done)** |

---

## 2. 质量门禁状态 (Quality Gates)

- **门禁 G1 (IPA 架构设计基线)**: **已通过 (Passed)** — 8 份 IPA 标准文档编制完成并通过基线承认。
- **门禁 G2 (Rust 核心单元测试)**: **已通过 (Passed)** — 基础单测与并发验证 100% 覆盖。
- **门禁 G3 (并发冲突仲裁与恢复验证)**: **已通过 (Passed)** — 100 个并发补丁无损合并、1000 任务批量调度、漂移防御测试全绿。
- **门禁 G4 (Vercel 云端部署与端点)**: **已通过 (Passed)** — `https://ide1-0.vercel.app` 生产上线并接入。
- **门禁 G5 (分布式持久化与沙盒驱动)**: **已通过 (Passed)** — WBS-05 与 WBS-06 经双 Worktree 并行开发与 Rebase 集成，20 项全套单测 100% 通过。
