# IDE1.0 MCP & Vercel 多代理协同加固交付报告

- **目标云端端点**: `https://ide1d0.vercel.app/mcp`
- **Vercel 生产项目**: `lidian727/ide1-0`
- **持久化数据库**: Neon Serverless Postgres (`ide1-mcp-db`, `store_2wZxdNaYjGiPi56k`)
- **实时云端验收日志**: [`docs/MCP-CLOUD-E2E-LOG.txt`](./MCP-CLOUD-E2E-LOG.txt)
- **验收状态**: P0-01..P0-05 全部闭环，P1-01..P1-06 全部落地，本地与真实云端 13 项 E2E 全绿通过 (`lost_updates = 0`, `committed_conflict_markers = 0`, `unauthorized_writes = 0`, `accidental_revision_resets = 0`, `cross_instance_divergence = 0`, `restart_data_loss = 0`)。

---

## 1. 修复前后架构与行为差异报告 (Before / After Diff)

| 编号 | 安全/一致性问题 | 修复前 (Before) | 修复后 (After) | 对应实现 |
|---|---|---|---|---|
| **P0-01** | 冲突仍提交 HEAD 并写入冲突标记 | `coedit_full` / `coedit_concurrent` / `headlessEdit` 遇到重叠冲突时直接把 `<<<<<<<` 写入 `content` 并 `revision += 1` | 冲突或语法失败时 **绝不修改正式 HEAD、不递增 revision、不改变 content_hash**；自动保存为独立 `PENDING_REVIEW` 提案并附带 `proposal_id`、结构化 `conflict_hunks`、`conflict_diff` 与 `conflict_preview`；支持 `headless_list_proposals` 与 `headless_resolve_proposal` 审核提交 | `crates/ide-mcp/src/merge/store.rs`, `api/v1/_store.js` |
| **P0-02** | 云端使用进程内 `Map` | Vercel 无状态函数依赖 `global.__ide_coordinator = new Map()`，冷启动、扩容或重启即丢失全部状态且多实例分裂 | 全量迁移至 Neon Postgres 事务性持久化存储 (`mcp_documents`, `mcp_revisions`, `mcp_proposals`, `mcp_idempotency`, `mcp_leases`, `mcp_outbox`, `mcp_audit_log`, `mcp_rate_limits`)；每次写操作在 `BEGIN ... SELECT ... FOR UPDATE ... COMMIT` 内完成；缺库即 Fail-Closed (`503`) | `db/migrations/001_mcp_durable_state.sql`, `api/v1/_store.js` |
| **P0-03** | 云端接口裸奔无鉴权 | `/mcp` 与 `/api/v1/*` 任意匿名调用均可读写所有工作区 | 强制校验 Bearer Token（短期 HS256 JWT `IDE_MCP_JWT_SECRET` + 作用域令牌 `IDE_MCP_TOKENS`），实施 `tenant_id/workspace_id/file_path` 隔离与 ACL 权限校验，增加 2 MiB 载荷限制 (`413`)、租户级限流 (`429`) 及全量 `mcp_audit_log` | `api/v1/_auth.js`, `api/v1/mcp.js`, `api/v1/{events,jobs,tasks,patches,files}` |
| **P0-04** | 重复打开可能覆盖文档与重置版本 | `open_doc` / `registerFile` 无条件重置 `currentRevision = 1` 并覆盖内容 | 原子 `INSERT ... ON CONFLICT DO NOTHING` (`create_if_absent`) 与 `open_existing` 模式；已存在文档返回 `ALREADY_EXISTS` 与当前 HEAD 版本；强覆盖须显式传 `mode: "force_overwrite"` 且具备权限并单调递增版本号 | `crates/ide-mcp/src/merge/store.rs`, `api/v1/_store.js` |
| **P0-05** | 版本基准与幂等键不足 | `op_id` 为全局 Key 易跨文件碰撞，未校验 `payload_hash` 与 `content_hash`，`base_revision` 可缺省 | 写操作强制传 `base_revision`、`op_id` 与 `agent_id/author`；幂等键严格绑定 `(tenant_id, workspace_id, file_path, op_id)` 且与正式提交在同一事务/锁内原子检查与落库；同 `op_id` 异载荷返回 `IDEMPOTENCY_PAYLOAD_MISMATCH`；SQL CAS 校验 `(revision, content_hash)` 与未过期 `fencing_token` | `crates/ide-mcp/src/merge/store.rs`, `api/v1/_store.js` |
| **P1-01** | 本地存在两套分裂状态 | `McpServer` 同时维护 `workspace` 与 `concurrent_workspace`，Rust Console 与 MCP 工具状态分裂 | 统一为单一权威 `Arc<DocumentStore>`；`ConcurrentCoEditWorkspace = DocumentStore`；`CoEditWorkspace` 委托共享 `Arc<DocumentStore>`；`RustConsoleEvaluator` 直接共享同一份 `DocumentStore` 状态 | `crates/ide-mcp/src/lib.rs`, `rust_console.rs`, `merge/compat.rs` |
| **P1-02** | 3-Way 合并指针错位与缺乏语法防护 | JS `threeWayMerge` 使用锁步循环无法正确处理多处插入/删除与行号漂移；合并后无语法检查 | Rust `merge/algo.rs` 与 JS `_merge.js` 均采用 `regions` 区间重叠聚类算法，精确处理多处增删与行号漂移，并在提交前执行语言级括号/注释/字符串平衡与 JSON/NEMA 语法检查 | `crates/ide-mcp/src/merge/algo.rs`, `api/v1/_merge.js` |
| **P1-03** | `ide_edit_lines` 与 `ide_replace_all` 存在 TOCTOU 竞争 | 先调 `headlessRead` 再发不带 `base_revision` 的 `headlessEdit`，并发写入必丢更新 | 在共享 `DocumentStore` 与 Postgres 事务内以 `base_revision` 快照计算候选文本并走 3-Way 合并 / CAS 提交流水线，消除读改写竞争 | `crates/ide-mcp/src/merge/store.rs`, `api/v1/_store.js` |
| **P1-04** | 内存文档与磁盘真实文件未隔离同步 | 无磁盘绑定哈希校验与原子落盘保护 | 增加 `open_disk_file`、`ide_sync_to_disk`（同目录临时文件 `fsync + rename` + 外部磁盘修改检测 `EXTERNAL_DISK_MODIFICATION`）、`ide_reload_from_disk`、`ide_git_diff` 与 `ide_rollback_revision` | `crates/ide-mcp/src/merge/disk.rs` |
| **P1-05** | 云端与本地 MCP 协议及 Schema 不一致 | 参数名分叉（`text_a`/`old_text`, `rev_a`/`from_revision`）、工具缺失、无 Streamable HTTP 会话头 | 对齐工具名与 JSON Schema，保留 `ide_open_file`、`ide_edit_lines` 等兼容别名，支持 `Mcp-Session-Id`、`DELETE` 关会话、`202` 通知与标准 `structured_data` 响应 | `crates/ide-mcp/src/tools_schema.rs`, `api/v1/mcp.js` |
| **P1-06** | JS 端使用过时反斜杠 NEMA 校验 | 云端假设域头必须含反斜杠且直接换行拼接正文 | Rust `merge/nema.rs` 复用 `nema_codec::parse_source` AST 域感知合并，JS `parseNemaStrict` 与 `nemaSemanticMerge` 严格对齐 Table / 5W1H / Graph / TAB 域头 / 字典 / 转义与 `\n\n\n` 语法封闭性 | `crates/ide-mcp/src/merge/nema.rs`, `api/v1/_merge.js` |

---

## 2. 数据库迁移与 Vercel 部署配置

- **Schema DDL**: [`db/migrations/001_mcp_durable_state.sql`](../db/migrations/001_mcp_durable_state.sql)
  - 表清单：`mcp_documents`, `mcp_revisions`, `mcp_proposals`, `mcp_idempotency`, `mcp_leases`, `mcp_jobs`, `mcp_tasks`, `mcp_outbox`, `mcp_audit_log`, `mcp_rate_limits`。
  - 所有写事务以 `SELECT ... FROM mcp_documents WHERE (tenant_id, workspace_id, file_path) ... FOR UPDATE` 锁定单文件权威行，并在更新时以 `WHERE current_revision = $headRev AND content_hash = $headHash` 二次把关。
- **Vercel 环境配置** (`lidian727/ide1-0`)：
  - 连接 Neon Postgres 存储 `ide1-mcp-db` (`DATABASE_URL`, `POSTGRES_URL` 等已注入 Production/Preview/Development)。
  - 配置 Sensitive Secret 环境变量：`IDE_MCP_JWT_SECRET`（HS256 短期令牌签名密钥）与 `IDE_MCP_TOKENS`（分租户/工作区/读写权限令牌清单），未在代码库中硬编码或提交任何明文凭证。
  - `.vercelignore` 排除本地安装包 (`dist-fast/`)、Rust 构建物 (`crates/`, `target/`) 与 `.env.*` 文件，加速纯净 Serverless 构建。

---

## 3. 全量验证与真实云端 E2E 实测结果

### 3.1 静态检查与本地单测/集成测试
- `cargo fmt -p ide-mcp`：100% 通过。
- `cargo clippy -p editcore -p nema-codec -p ide-mcp -- -D warnings`：0 警告通过。
- `cargo test -p ide-mcp -- --test-threads=1`：全套 25 项（16 项单元 + 2 项 OCC 数学基准 + 6 项红绿对称/不变量测试 + 1 项三代理协同测试）全部通过。
- `node --test tests/mcp-vercel-unit.test.mjs`：8 项核心算法/鉴权/NEMA v1.5/MCP 协议测试 100% 通过。

### 3.2 真实 Vercel 生产云端 E2E 测试 (`node tests/mcp-cloud-e2e.mjs --redeploy`)
以独立隔离工作区 `audit_1795320404138` 对 `https://ide1d0.vercel.app/mcp` 执行真实 HTTPS JSON-RPC 调用：

| 用例 | 结果 | 关键观测事实 |
|---|---|---|
| 1. `two_agents_edit_different_functions` | PASS | 双代理基于同一 `rev 1` 分别修改不同函数，自动 3-Way 合入 `rev 3` |
| 2. `three_agents_edit_same_file` | PASS | 3 代理真正并发 (`Promise.all`) 修改同文件上/中/下区域，收敛至 `rev 4` 无丢失 |
| 3. `overlapping_edit_rejected_without_commit` | PASS | 重叠修改返回 `CONFLICT` + `proposal_id` + `conflict_diff`；正式 HEAD 维持 `rev 2` 且不含冲突标记 |
| 4. `reopen_existing_file_without_reset` | PASS | 重复打开返回 `ALREADY_EXISTS` 并保留 `rev 4`；无权 `force_overwrite` 返回 `403` |
| 5. `concurrent_duplicate_op_id` | PASS | 同 `op_id` 并发重放仅提交一次 (`rev 2`)；同 `op_id` 改载荷触发 `IDEMPOTENCY_PAYLOAD_MISMATCH` |
| 6. `cross_file_idempotency_collision` | PASS | 两份不同文件使用相同 `op_id` 均独立成功提交 |
| 7. `unauthorized_access_denied` | PASS | 无令牌 `401`、过期 JWT `401`、只读写入 `403`、越界工作区 `403`、跨租户隔离 `FILE_NOT_FOUND`、过期/错误 Fencing Token `FENCING_TOKEN_STALE` |
| 8. `multiple_serverless_instances` | PASS | 6 路并发长短连接打向 Neon Postgres 无丢失收敛至 `rev 7` |
| 9. `server_restart_persistence` | PASS | 测试中途实执行 `vercel deploy --prod --yes` 重建全部生产 Serverless 实例，重启后版本 `rev 3` 与完整历史无损保留 |
| 10. `stale_revision_rebase_or_reject` | PASS | 不重叠陈旧修改安全 Rebase；错 Hash 拒止 `BASE_HASH_MISMATCH`；重叠陈旧修改产生提案不污染 `rev 3` |
| 11. `atomic_line_edit_and_replace` | PASS | `ide_edit_lines` 与 `ide_replace_all` 并发执行均保留在 `rev 3` |
| 12. `correct_nema_domain_merge` | PASS | 官方 TAB 域头 Table + 5W1H + Graph 并行合并保留 `\n\n\n` 且双空格非法输入被拦截 |
| 12b. `authenticated_rest_coordination_endpoints` | PASS | `/api/v1/jobs/dispatch`, `/api/v1/tasks/claim`, `/api/v1/events` 鉴权与持久化租约验证通过 (`24` 条 Outbox 事件) |
| 13. `real_disk_sync_and_git_diff` | PASS | 真实临时 Git 仓库原子落盘、`git diff` 验证、外部修改检测拒绝及版本回滚全通过 |

---

## 4. 安全审查结论与回滚方案

### 安全审查结论
1. **无硬编码密钥**：`IDE_MCP_JWT_SECRET`、`IDE_MCP_TOKENS` 与 `DATABASE_URL` 全部注入 Vercel Sensitive Secrets 环境变量，本地验证凭证存放于 `.gitignore` 忽略的 `.env.mcp.local`。
2. **安全边界清晰**：路径穿越 (`..`, `\0`, `/` 开头) 在 `_auth.js` 首层即拦截返回 `400`；请求体硬上限 2 MiB (`413`)；HMAC 签名与静态令牌均使用 `crypto.timingSafeEqual` 防止时序侧信道；`Cargo.toml` 全仓维持 `#![forbid(unsafe_code)]`。
3. **日志脱敏**：E2E 脚本与产出日志已对 JWT 与 Bearer 令牌自动脱敏为 `<REDACTED>` / `<REDACTED_JWT>`。

### 生产回滚方案
1. **应用层瞬时回滚**：若新部署出现异常，可通过 `vercel rollback <previous-deployment-url>` 在数秒内切回上一个已就绪的生产 Deployment，无需变更代码库。
2. **数据库兼容回滚**：新增 Postgres 表均为独立 `mcp_*` 命名空间，不改动现有表结构；如需清理测试审计数据，可按 `tenant_id = 'audit_tenant'` 且 `workspace_id LIKE 'audit_%'` 条件安全删除而零影响其他租户。
3. **代码回滚**：使用 `git revert <release-commit-sha>` 即可完整回退提交记录，Rust 与 Vercel API 可独立部署恢复。
