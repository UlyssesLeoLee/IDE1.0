# IDE1.0 Web — Semantic Agent Router & Worker 基本设计书

**文档编号**: BD-ROUTER-2026-V2.0  
**制定组织**: IDE1.0 内核与云架构委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.0  
**状态**: 承认済 (Baseline Locked)  
**上游要件**: [Web-Agent-Router-Requirements.md](../00_要件基线/Web-Agent-Router-Requirements.md)

---

## 1. 架构全景与拓扑 (System Topology)

本系统采用 **三平面分离架构 (Control Plane + Semantic Task Plane + Execution Plane)**，实现控制流与数据流的彻底解耦，保障在超高并发 Agent 访问下的无锁并行与提交一致性。

```
Browser IDE / Tauri Desktop / External Agent / MCP Tool / CLI
                         │
                 HTTPS Agent Gateway (JWT / Scope Auth)
                         │
            ┌────────────┴────────────┐
            │    Vercel Router API    │ (Next.js Edge / Functions)
            │  (Auth / Planner / Qry) │
            └────────────┬────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                                 │
┌───────▼────────┐               ┌────────▼────────┐
│   PostgreSQL   │ (Ledger)      │  Vercel Queues  │ (Task Outbox)
│ (Revs / Leases)│               │ (Rate-Limited)  │
└───────┬────────┘               └────────┬────────┘
        │                                 │
        │                        ┌────────▼────────┐
        │                        │Worker Dispatcher│
        │                        └────────┬────────┘
        │                                 │
        │                        ┌────────▼────────┐
        │                        │ Vercel Sandbox  │ (Isolated Forks)
        │                        │ (Drives / WASM) │
        │                        └────────┬────────┘
        │                                 │
        │                        ┌────────▼────────┐
        │                        │Rust Semantic Core
        │                        │(Parse / Patch)  │
        │                        └────────┬────────┘
        │                                 │
┌───────▼─────────────────────────────────▼────────┐
│              File Commit Coordinator             │ (Rust Core)
│         CAS → Anchor Relocate → New Rev          │
└───────┬─────────────────────────────────┬────────┘
        │                                 │
┌───────▼────────┐               ┌────────▼────────┐
│Immutable Blob  │               │ Git Projection  │ (Background Sync)
│(Content Store) │               │     Worker      │
└────────────────┘               └────────┬────────┘
                                          │
                                 ┌────────▼────────┐
                                 │IDE Event Feed / │ (Review UI / SSE)
                                 │Web Workbench    │
                                 └─────────────────┘
```

---

## 2. 职责边界划分 (Separation of Concerns)

### 2.1 Router 控制面 (Vercel Functions / Next.js API)
1. **鉴权与租户隔离**: 验证 Bearer Token，按 `tenant_id`、`workspace_id` 与资源范围限制权限。
2. **文件级路由与分片**: 以 `consistent_hash(workspace_id, file_id)` 将请求路由到正确的逻辑协调通道。
3. **语义索引读取**: 查询已缓存的 AST 语义单元与依赖图，不在短生命周期的 Router 请求路径中全量重解析整个代码仓库。
4. **任务分解与依赖排期**: 将高层目标拆解为针对独立 `symbol_path` 的 `Semantic Task`，根据依赖 DAG 计算拓扑序。
5. **任务凭证与租约分发**: 签发带有 `fencing_token`、有限 TTL 的短期认领凭据。

### 2.2 Worker 执行面 (Vercel Sandbox / Drives / Rust Agent Runner)
1. **沙箱隔离运行**: 在只读源快照与私有写写副本中运行，严禁直接通过网络或文件系统覆盖权威源文件。
2. **Rust 语义单元代码生成**: 调用 Rust 核心库生成符合 AST 契约的替换文本或结构化节点补丁。
3. **局部检验与证据收集**: 在沙箱中执行格式化、局部单测或静态分析，记录输出日志与诊断作为 Proposal 证据。
4. **提交 Patch Proposal**: 将包含 `base_revision`、`unit_id`、`expected_fingerprint` 与 `replacement_text` 的 Patch 提交至 Coordinator。

### 2.3 Coordinator 协调面 (Rust Engine + Postgres CAS)
1. **单文件 CAS 决策**: 保证同一文件在权威版本推进时的严格串行化判定。
2. **租约有效性判定 (Fencing)**: 校验请求中的 `fencing_token` 是否与当前数据库最高有效 Token 一致；过期则直接 412 拒斥。
3. **AST-aware 自动重定位**: 若目标文件的权威版本已领先（并发写入了其它函数），Coordinator 重新调用 Rust Tree-sitter 计算最新节点偏移，只要目标节点的指纹与写足迹未被改动，则安全重定位并合并。
4. **事务提交与 Outbox 记录**: 在同一原子事务中更新 `file_revisions`、写入 `patch_commits` 并向 `outbox_events` 插入提交事件。
5. **Git 投影解耦**: 权威版本归属数据库与对象存储，Git 仓库由独立的 Git Projection Worker 异步拉取 Outbox 进行提交，避免把外部 Git 的高延迟纳入高频事务临界区。

---

## 3. 核心对象与状态机 (State Machine Models)

### 3.1 语义单元模型 (Semantic Unit)
```rust
pub struct SemanticUnit {
    pub workspace_id: String,
    pub file_id: String,
    pub language: SupportedLanguage,
    pub qualified_symbol_path: String, // e.g. "crate::buffer::DocLineMgr::insert_str"
    pub node_kind: String,              // e.g. "function_item", "impl_item"
    pub signature_hash: String,         // SHA256(signature_tokens)
    pub base_content_hash: String,      // SHA256(node_body)
    pub parent_fingerprint: String,     // Parent node hash for context
    pub node_fingerprint: String,       // Composite stable identifier
    pub byte_range: (usize, usize),
    pub line_range: (usize, usize),
}
```

### 3.2 任务状态机 (Task Lifecycle)
```
      [PENDING]
          │ (Dependencies satisfied)
          ▼
       [READY]
          │ (Worker claims task -> Issue Lease + Fencing Token)
          ▼
      [CLAIMED]
     ┌────┴───────────────────────────┐
     │ (Heartbeat expires / failed)   │ (Patch proposed)
     ▼                                ▼
 [EXPIRED] ──► [READY] (Re-queue) [VALIDATING]
                                      │
                         ┌────────────┴────────────┐
                         │ (Coordinator CAS pass)  │ (Conflict / Rejected)
                         ▼                         ▼
                    [COMMITTED]                [CONFLICT] ──► [RETRY / ABORT]
```

---

## 4. 容错与并发控制策略 (Integrity & Fault Tolerance)

| 场景 | 潜在风险 | 防御与解决机制 |
|:---|:---|:---|
| **双 Agent 修改同文件不同函数** | Git 产生冲突或串行提交失败 | AST 节点锚点重定位：只要写足迹不重叠，后提交者自动更新绝对坐标，CAS 成功递增。 |
| **双 Agent 修改同一函数** | 覆盖或逻辑混淆 | 语义写集冲突拦截：严格拒绝第二个提交，返回 409 `SEMANTIC_WRITE_CONFLICT` 并附带冲突差异。 |
| **Worker 假死/网络分区** | 任务永久挂起，阻塞后续任务 | 租约 TTL 机制：无心跳续约 60 秒后自动释放，后续提交因 Fencing Token 过期被 412 拦截，任务重入队。 |
| **高热点文件并发请求风暴** | 数据库乐观重试活锁 | 逻辑分片排队 + 令牌桶限流：同一文件限制每秒权威 CAS 提交频次，平滑消费。 |
| **消息队列重复投递** | 重复应用 Patch 导致代码重复 | 幂等性主键：`patch_commits` 以 `(task_id, idempotency_key)` 为唯一索引，重复请求返回已存在结果。 |

---

## 5. 模块工程结构与复用边界 (Code Organization)

系统复用既有高质量 Rust 内核，并新增微内核协作模块：

```
IDE1.0/
├── crates/
│   ├── editcore/            # [复用] 核心字符缓冲区、行管理、编码与撤销重做
│   ├── nema-codec/          # [新增核心] .nema/.nemat 多维结构化语义解析、共享词典、RLE 与租约仲裁
│   ├── ide-kernel/          # [复用] 权威文档状态管理、Session 与只读快照
│   ├── ide-shell-protocol/  # [复用] 协议底座、JSON-RPC / REST 基础
│   ├── ide-shell-web/       # [复用] Web Shell 静态资产与编辑器界面
│   ├── ide-shell-desktop/   # [复用] Tauri 跨平台桌面壳工程
│   └── ide-agent-router/    # [新增核心] 
│       ├── src/
│       │   ├── parser/      # Tree-sitter 多语言解析器与符号抽取
│       │   ├── identity/    # 语义单元稳定身份哈希与指纹算法
│       │   ├── patch/       # AST-aware 补丁应用、重定位与冲突检测
│       │   ├── coordinator/ # CAS 判定、Fencing Token 租约机与事务内存
│       │   ├── reviewer/    # 最初规划、实时指引与统筹审查验收证书 (AcceptanceCertificate)
│       │   ├── verifier/    # 构建/测试/质量/规范四级自动验证门禁
│       │   ├── ledger/      # PostgreSQL Schema 映射与 Outbox 事件
│       │   └── api/         # OpenAPI 路由处理与 Axum / Edge 适配器
├── ide-fast/                # [复用+增强] 原生高速版 (含 NEMA Table/5W1H/Graph 实时交互面板)
├── web/                     # Vercel Next.js 控制面与 Workbench 前端增强
├── docs/                    # 日本 IPA 规约文档全集
│   ├── 01_基本设计/BD-002_Multi-Agent-NEMA-Collab-Platform.md
│   └── 02_详细设计/DD-002_NEMA-Aggregator-Git-Verify-Pipeline.md
└── vercel.json              # Vercel Serverless / Edge 路由部署配置
```

---
**确认人**: 
- 架构委员会审核: Approved 2026-11-05
