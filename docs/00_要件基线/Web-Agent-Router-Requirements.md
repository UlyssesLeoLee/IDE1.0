# IDE1.0 Web — Semantic Agent Router & Worker 要件基线定义书

**文档编号**: REQ-ROUTER-2026-V2.0  
**制定组织**: IDE1.0 内核与云架构委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.0 (Stage 3.0 Web Agent Collaboration Standard)  
**状态**: 承认済 (Baseline Locked)  
**追踪标识**: `REQ-ROUTER-001` ~ `REQ-ROUTER-060`

---

## 1. 目的与系统愿景

### 1.1 研发背景
IDE1.0 已经完成了纯 Rust 编辑核心（`editcore`）、会话管理（`ide-kernel`）、桌面终端（Tauri 跨平台）以及单机自包含 Web Shell 的研发与生产落地。
随着大语言模型与多 Agent 系统的普及，代码协作正从“人类交互式手动输入”转向“数百至数万 Agent 实例通过 API 进行高频代码检索、推导与提交”。

传统 Git 分支模型（Feature Branch + Pull Request）面临两大现实瓶颈：
1. **粗粒度锁与分支合并风暴**: 当 50 个 Agent 同时修改同一文件内的不同函数时，Git 产生难以自动解决的文本冲突与大量的串行人工 Review 负担。
2. **缺乏语义一致性与租约保护**: Agent 之间缺乏文件内语义作业边界与并发租约（Fencing Token），极易发生 Stale Read 后的“后发先写”覆盖（Silent Overwrite）。

### 1.2 系统愿景
构建一套部署于 **Vercel** 边缘与 Serverless 体系的 **Agent-First 代码协作服务**：
- **文件级路由 (File-level Routing)**: 维持文件作为一致性协调与权威版本发布的最高防线；
- **文件内语义单元级并行作业 (Semantic Unit Parallelism)**: 基于 AST / Tree-sitter 将文件细化为类、函数、方法、类型与块级作业单元；
- **Patch 级提交 (Structured Node Patch)**: Agent 不直接修改权威代码库，仅提交结构化补丁；
- **CAS + Fencing 租约一致性协调器 (File Commit Coordinator)**: 串行化最终提交决策，保障无静默覆盖、无脑裂、可原子回滚与全链路审计追溯。

---

## 2. 核心不变量与非妥协原则 (Non-Negotiable Principles)

| 原则标识 | 原则名称 | 详细定义与硬性约束 |
|:---|:---|:---|
| **NRP-01** | **Rust First** | 代码解析、语义单元抽取、Patch 应用与冲突仲裁完全由 Rust 核心实现，保持单测与内存安全；Node/TypeScript 仅作路由与控制面适配。 |
| **NRP-02** | **桌面与 Web 语义统一** | 保留 Tauri 桌面端与 Web Shell 的一致交互模型与快捷键系统；不得为了 Web 特性削减既有编辑能力。 |
| **NRP-03** | **架构边界分级** | **文件**是路由与一致性边界；**语义节点**是作业与锁定边界；**Patch**是交付与评审边界。 |
| **NRP-04** | **禁止盲目覆写** | Agent 严禁通过 `PUT /file` 直接覆盖权威源文件，必须通过 Coordinator 经 CAS 与重定位检验后提交。 |
| **NRP-05** | **GUI 解耦** | Agent 不依赖任何前端 DOM、虚拟屏幕或模拟按键；提供纯粹、幂等、健壮的 HTTP API / OpenAPI 3.1 接口。 |
| **NRP-06** | **物理资源有界** | 明确区分“逻辑任务排队目标（支持上万任务入队）”与“物理并发吞吐（受沙箱实例与数据库 CAS 速率限制）”，杜绝伪并发承诺。 |
| **NRP-07** | **设计与测试先行** | 遵循 IPA 流程：要件基线 → 基本设计 → 详细设计 → UT/IT/ST 验证，代码与测试全绿后方可封版。 |
| **NRP-08** | **最小可行复杂度** | 初版在 Vercel Functions + Serverless Postgres + 队列框架下运转，严禁引入过重的 K3s、Kafka 集群。 |

---

## 3. 功能要件 (Functional Requirements)

### 3.1 语义单元模型 (REQ-SEMANTIC)
- **REQ-SEMANTIC-01 [多语言解析]**: 系统必须支持 Rust、Python、TypeScript、JavaScript、C、PowerShell 的 AST 解析与顶层/内部符号提取。
- **REQ-SEMANTIC-02 [退化与保底]**: 对于不支持的语言、复杂宏展开、二进制或无法语法解析的文件，系统必须自动退化为文本 Hunk 或独占文件级锁，严禁伪造不安全的节点划分。
- **REQ-SEMANTIC-03 [稳定身份标识]**: 每个语义单元必须具有稳定哈希身份，包含：`workspace_id`、`file_id`、`language`、`qualified_symbol_path`、`node_kind`、`signature_hash`、`base_content_hash`、`node_fingerprint`。
- **REQ-SEMANTIC-04 [节点足迹检测]**: 提取语义单元时必须计算其写足迹（Byte Range + Line Span）及直接引用依赖（Import / Called Functions）。

### 3.2 任务路由与调度 (REQ-ROUTER)
- **REQ-ROUTER-01 [文件一致性分片]**: 路由层以 `consistent_hash(workspace_id, file_id)` 决定逻辑路由分片，确保同一文件的协调状态集中管理。
- **REQ-ROUTER-02 [细粒度任务拆分]**: 允许上层任务分解为针对特定语义节点的独立 Sub-task（例如：Agent A 修改 `fn parse_header`，Agent B 修改 `fn parse_body`）。
- **REQ-ROUTER-03 [依赖 DAG 阻塞]**: 当任务间存在符号调用或接口契约依赖时，建立依赖 DAG，前置任务未 Commit 前，后置任务处于 `BLOCKED` 状态，禁止发放写租约。
- **REQ-ROUTER-04 [热点文件背压]**: 对极高修改频率的文件实施提交频率窗口限流，防止极端并发引发数据库乐观锁活锁（Live-lock）。

### 3.3 租约与提交协调 (REQ-COORD)
- **REQ-COORD-01 [Fencing 租约分配]**: Worker 认领写任务时分配具有 TTL、单调递增 `fencing_token` 的短期租约（默认 60s），支持心跳续约。
- **REQ-COORD-02 [过期租约阻断]**: 当 Worker 租约超时或被更高 fencing_token 抢占时，旧 Worker 发起的一切提交请求必须被 412 (FENCING_TOKEN_STALE) 严格拒绝。
- **REQ-COORD-03 [AST-aware 自动重定位]**: 当提交时文件的 `current_revision > base_revision`，协调器在 Rust 核心中根据符号路径与签名哈希尝试重定位节点锚点；若目标节点未被其他提交修改且结构无歧义，则自动合并；否则返回 409 (CONFLICT)。
- **REQ-COORD-04 [原子版本提交]**: 单文件提交采用 CAS 原子推进 `file_revision`；多文件变更集（ChangeSet）采用 Postgres 数据库事务，要么全部提交，要么全部回滚。
- **REQ-COORD-05 [Transactional Outbox]**: 每次 Commit 成功必须在同事务中插入 `outbox_events` 表，确保消息投递与状态持久化无裂痕。

### 3.4 接口与网关 (REQ-API)
- **REQ-API-01 [OpenAPI 3.1 契约]**: 完整定义并提供符合标准的 API 文档及 Schema，支持端点：
  - `POST /api/v1/jobs/dispatch`
  - `POST /api/v1/tasks/claim`
  - `POST /api/v1/tasks/{task_id}/heartbeat`
  - `POST /api/v1/patches/prepare`
  - `POST /api/v1/patches/commit`
  - `POST /api/v1/changesets`
  - `GET /api/v1/events`
  - `GET /api/v1/files/{file_id}/semantic-units`
- **REQ-API-02 [幂等性保证]**: 所有的写操作必须携带 `Idempotency-Key`，网络重试不产生副作用与二次提交。
- **REQ-API-03 [轻量 SDK]**: 提供符合人体工程学的 TypeScript、Python、Rust SDK 客户端封装与可运行测试用例。

---

## 4. 非功能要件 (Non-Functional Requirements)

| 分类 | 编号 | 要件指标与验证标准 |
|:---|:---|:---|
| **性能** | **NFR-PERF-01** | 单语义单元解析与指纹计算耗时在 Rust 中不超过 10ms (10,000 行代码以内文件)。 |
| **性能** | **NFR-PERF-02** | CAS 提交验证与版本冲突判定的平均处理耗时不超过 25ms。 |
| **并发** | **NFR-CONC-01** | 逻辑排队能力支持 10,000 个任务稳定持久化入库，按调度优先级发放。 |
| **并发** | **NFR-CONC-02** | 同一文件不同非相交函数支持并发生成 Patch，提交时按顺序无损合并。 |
| **安全** | **NFR-SEC-01** | 租户与 Workspace 严格隔离，API 请求必须提供 Scoped Bearer Token。 |
| **安全** | **NFR-SEC-02** | 路径规范化防御：严禁路径遍历（`..`、绝对路径越界、symlink 逃逸）。 |
| **可靠性** | **NFR-REL-01** | Worker 崩溃时，租约超时机制自动回收任务并触发 Re-queue，系统无死锁。 |
| **可观测** | **NFR-OBS-01** | 提交决策、租约抢占、冲突拒绝均有结构化日志与 Event Stream 输出。 |

---

## 5. 日本 IPA 追溯性设计基准 (Traceability Matrix Baseline)

每个要件直接映射至基本设计章节（BD）、详细设计章节（DD）、单元测试用例（UT）、集成测试用例（IT）与系统验证（ST）：

| 要件 ID | 对应基本设计 (BD) | 对应详细设计 (DD) | 验证阶段与方法 |
|:---|:---|:---|:---|
| `REQ-SEMANTIC-01` | BD-02 语义单元架构 | DD-01 Tree-sitter 提取器 | UT-SEM-01 ~ UT-SEM-06 (各语言解析用例) |
| `REQ-SEMANTIC-03` | BD-02 稳定指纹算法 | DD-01 节点哈希与指纹 | UT-SEM-10 (哈希碰撞与重命名测试) |
| `REQ-ROUTER-01` | BD-03 控制面分片 | DD-02 一致性哈希分发 | IT-ROUT-01 (多分片路由平衡) |
| `REQ-COORD-01` | BD-04 租约与 CAS | DD-03 租约协议状态机 | UT-COORD-01, IT-COORD-01 (Fencing 递增) |
| `REQ-COORD-02` | BD-04 冲突拦截 | DD-03 Fencing 冲突检测 | IT-COORD-02 (过期租约提交被拒) |
| `REQ-COORD-03` | BD-04 AST 重定位 | DD-04 三方合并与重定位 | UT-COORD-05, IT-MERGE-01 (同文件不同函数并发合并) |
| `REQ-COORD-04` | BD-05 事务账本 | DD-05 Postgres DDL & CAS | IT-TX-01 (多文件事务原子性) |
| `REQ-API-01` | BD-06 API 网关 | DD-06 OpenAPI 3.1 规格 | ST-API-01 (API 全覆盖自动化调用) |
| `REQ-VERCEL-01` | BD-07 部署拓扑 | DD-07 Functions/Vercel JSON | UAT-VERCEL-01 (真机部署与访问验证) |

---
**签署人**: 
- 内核架构师: Mavis (Rust Core Lead)
- 分布式系统专家: Ulysses (Staff Engineer)
- 日本 IPA 评审官: Approved 2026-11-05
