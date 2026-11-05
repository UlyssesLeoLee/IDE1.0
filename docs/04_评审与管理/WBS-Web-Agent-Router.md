# IDE1.0 Web — Agent Router & Worker 工作分解结构书 (WBS)

**文档编号**: WBS-ROUTER-2026-V2.0  
**制定组织**: IDE1.0 项目推进委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.0  
**状态**: 承认済 (Baseline Locked)

---

## 1. WBS 分解与任务追踪矩阵

| WBS 编码 | 任务名称 | 成果物 | 对应要件 | 责任人 | 状态 |
|:---|:---|:---|:---|:---|:---|
| **WBS-01** | **IPA 架构设计基线** | | | | |
| WBS-01.1 | 要件基线定义书制定 | `docs/00_要件基线/Web-Agent-Router-Requirements.md` | `REQ-ROUTER-*` | Ulysses | **完成** |
| WBS-01.2 | 基本设计书制定 | `docs/01_基本设计/Web-Agent-Router-Basic-Design.md` | `REQ-ARCH-*` | Mavis | **完成** |
| WBS-01.3 | 语义单元与协调协议详细设计 | `docs/02_详细设计/Semantic-Units-and-Commit-Protocol.md` | `REQ-SEM-*` | Mavis | **完成** |
| WBS-01.4 | OpenAPI 3.1 接口契约制定 | `docs/02_详细设计/Router-Worker-API.openapi.yaml` | `REQ-API-*` | Ulysses | **完成** |
| WBS-01.5 | Postgres 数据模式与状态机设计 | `docs/02_详细设计/Postgres-Schema-and-State-Machines.md` | `REQ-DB-*` | Ulysses | **完成** |
| WBS-01.6 | 测试规格书与追溯矩阵 | `docs/03_测试资料/UT-IT-ST-UAT-Web-Agent-Router.xlsx` | `REQ-TEST-*` | Mavis | **进行中** |
| WBS-01.7 | Vercel 部署与故障恢复手册 | `docs/05_实装与运维/Vercel-Deployment-and-Recovery.md` | `REQ-OPS-*` | Ulysses | **进行中** |
| **WBS-02** | **Rust 核心引擎实现** | | | | |
| WBS-02.1 | `crates/ide-agent-router` 骨架建立 | `Cargo.toml`, 模块结构 | `NRP-01` | Mavis | **待开始** |
| WBS-02.2 | Tree-sitter 多语言符号提取与指纹计算 | `src/parser.rs`, `src/identity.rs` | `REQ-SEMANTIC-01~04` | Mavis | **待开始** |
| WBS-02.3 | AST-aware Patch 应用与锚点重定位 | `src/patch.rs` | `REQ-COORD-03` | Mavis | **待开始** |
| WBS-02.4 | CAS 提交协调器与 Fencing 租约机 | `src/coordinator.rs` | `REQ-COORD-01~05` | Ulysses | **待开始** |
| WBS-02.5 | 内存与 Postgres 驱动适配器 | `src/ledger.rs` | `REQ-DB-*` | Ulysses | **待开始** |
| **WBS-03** | **API 网关与 SDK 交付** | | | | |
| WBS-03.1 | HTTP / REST API 服务端实现 (Axum/Edge) | `src/api.rs`, `api/v1/` | `REQ-API-01~02` | Ulysses | **待开始** |
| WBS-03.2 | Agent SDK 客户端 (TypeScript / Python) | `sdk/ts`, `sdk/py` | `REQ-API-03` | Ulysses | **待开始** |
| **WBS-04** | **Vercel 部署与生产验证** | | | | |
| WBS-04.1 | Vercel 部署配置与静态/Serverless 绑定 | `vercel.json` | `REQ-VERCEL-01` | Ulysses | **待开始** |
| WBS-04.2 | 生产部署上线并获取公开链接 | Vercel Live Deployment URL | `REQ-VERCEL-02` | Ulysses | **待开始** |
| WBS-04.3 | 多 Agent 并发与冲突自动化验证 | `crates/ide-agent-router/tests/` | `NFR-CONC-*` | Mavis | **待开始** |
| WBS-04.4 | 全局文档与 GitHub README 升级 | `README.md` | `NRP-02` | Mavis | **待开始** |

---
**质量门禁 (Quality Gate)**:
- 门禁 G1: 设计书通过 IPA 全面评审 (已通过)
- 门禁 G2: Rust 核心模块 UT 单测 100% 通过
- 门禁 G3: 真实多 Agent 并发与冲突测试全绿
- 门禁 G4: Vercel 线上端点联通验证通过
