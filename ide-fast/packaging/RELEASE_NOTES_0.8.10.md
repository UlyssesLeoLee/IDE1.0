# IDE1.0 v0.8.10 — Vercel MCP 持久化事务存储、权限边界全面加固与 Rust 内核规范治理

## 1. 持久化与并发安全 (P0-01 .. P0-05)
- **Neon Serverless Postgres 事务核心**：全面取代进程内易失 `Map`，支持冷启动、多实例扩容及 `vercel deploy` 容器销毁重建后的零丢失恢复。
- **冲突提案隔离**：任何重叠编辑或语法完整性失败绝不修改正式 HEAD、`revision` 或 `content_hash`，而是落库为 `PENDING_REVIEW` 提案，返回结构化 Diff 供人工或 Review Agent 审核解决。
- **作用域幂等与双 CAS + Fencing Token**：强制 `base_revision`、`op_id` 与 `agent_id/author`，在事务锁内联合校验 `(current_revision, content_hash)`、`payload_hash` 与未过期单调 `fencing_token`。

## 2. 鉴权、文件级 ACL 与提案权限闭环
- **租户 / 工作区 / 文件三级隔离**：新增基于 Bearer HS256 JWT（`IDE_MCP_JWT_SECRET`）与作用域静态令牌（`IDE_MCP_TOKENS`）的细粒度 `workspaces` + `files/allowed_files` ACL 校验，默认 Fail-Closed。
- **提案接口越权修复**：`headless_resolve_proposal` 在事务锁内强制校验目标提案所属工作区及文件的 `write` 权限，`headless_list_proposals` 严格按可读工作区与文件权限过滤；`enforcePayloadSize` 覆盖无 `content-length` 的已解析 `req.body`（硬上限 2 MiB）。

## 3. Rust `ide-mcp` 统一存储与工程规范修复
- **单一 `DocumentStore` 与真实磁盘集成**：`McpServer` 与 `RustConsoleEvaluator` 共用 `Arc<DocumentStore>`，统一 3-Way 区间聚类合并与 NEMA v1.5 严格域级合并。
- **安全与健壮性加固**：Windows 外部命令调用（`handle_launch_browser`、`git_diff_workspace`）全面启用 `CREATE_NO_WINDOW` 并消除 `-Command` 字符串插值；全量 `RwLock`/`Mutex` 接入锁中毒自愈；磁盘读写接入路径穿越防护与 `auto_backup::backup_before_save` 写前备份；`force_overwrite` 增加显式授权守卫，JSON-RPC `isError` 与结果 `ok` 状态对齐。

## 4. 全量测试与云端生产验收
- `cargo fmt` / `cargo clippy -- -D warnings`：`0` 警告通过
- `cargo test -p editcore -p nema-codec -p ide-mcp -- --test-threads=1`：全部通过
- `node --test tests/mcp-vercel-unit.test.mjs`：9 项核心安全/算法/鉴权单元全通过
- `node tests/mcp-cloud-e2e.mjs --redeploy`（真实生产端 `https://ide1d0.vercel.app/mcp`）：全部强制 E2E 用例（含提案/文件级权限、多实例并发与中途中止重启）PASS
