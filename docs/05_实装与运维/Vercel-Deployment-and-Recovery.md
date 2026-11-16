# IDE1.0 Web — Vercel 部署与故障恢复运维手册

**文档编号**: OPS-VERCEL-2026-V2.0  
**制定组织**: IDE1.0 内核与云架构委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.0  
**状态**: 承认済 (Baseline Locked)

---

## 1. Vercel 生产拓扑与配置架构

### 1.1 部署分层架构
- **静态层 (Static Shell)**: 自包含的前端 Web Workbench（由 `crates/ide-shell-web/src/editor.html` 生成并打包至 `crates/ide-shell-desktop/dist/`），由 Vercel 全球边缘 CDN 托管，提供秒级打开体验。
- **控制面层 (Serverless Functions)**: 承载 `/api/v1` 路由，处理鉴权、语义单元只读查询、任务调度与 CAS 协调决策。
- **持久层 (PostgreSQL Ledger)**: 采用 Serverless Postgres（Neon / Vercel Postgres / Supabase），通过连接池维护权威版本与租约状态。
- **对象存储 (Immutable Blob)**: 权威代码快照以 SHA-256 为键存入 Vercel Blob / S3 兼容存储，支持大文件与历史快照高效读取。

### 1.2 `vercel.json` 规范
```json
{
  "version": 2,
  "outputDirectory": "crates/ide-shell-desktop/dist",
  "cleanUrls": true,
  "rewrites": [
    {
      "source": "/api/v1/:path*",
      "destination": "/api/v1/:path*"
    },
    {
      "source": "/(.*)",
      "destination": "/index.html"
    }
  ],
  "headers": [
    {
      "source": "/(.*)",
      "headers": [
        {
          "key": "X-Frame-Options",
          "value": "DENY"
        },
        {
          "key": "X-Content-Type-Options",
          "value": "nosniff"
        },
        {
          "key": "Cross-Origin-Opener-Policy",
          "value": "same-origin"
        }
      ]
    }
  ]
}
```

---

## 2. 生产高可用与故障恢复机制 (Fault Recovery)

### 2.1 Worker 崩溃与租约恢复 (Worker Crash Recovery)
- **现象**: Worker 认领了任务并获得了租约（`status = CLAIMED`），但因宿主崩溃或网络异常未能发出心跳与 Patch。
- **恢复策略**:
  1. 调度守护任务每 5 秒扫描 `tasks` 表：
     ```sql
     UPDATE tasks 
     SET status = 'READY', 
         worker_id = NULL,
         retry_count = retry_count + 1,
         updated_at = NOW()
     WHERE status = 'CLAIMED' 
       AND lease_expires_at < NOW()
       AND retry_count < 5;
     ```
  2. 若该 Worker 后续延迟恢复并尝试提交 Patch，Coordinator 校验其提交中的 `fencing_token` 时发现小于数据库最新 Token，立即拒绝（412 Precondition Failed），彻底消除脏写风险。

### 2.2 消息队列重复投递幂等处理 (Duplicate Message Idempotency)
- **现象**: 消息队列或网络重试导致相同的 Patch 提交请求多次到达。
- **恢复策略**:
  1. 每个 Commit 请求必须包含由客户端或调度器生成的 `Idempotency-Key`。
  2. `patch_commits` 表将 `idempotency_key` 设为 `UNIQUE`。
  3. Coordinator 命中唯一键冲突时，直接返回此前已成功记录的 `commit_id` 与 `new_revision`，不重复推进文件版本号，不重复发射 Outbox 事件。

### 2.3 热点文件版本冲突退避 (Hot-file Exponential Backoff)
- **现象**: 多个 Agent 激烈争抢修改同一热点文件，导致大量 CAS 失败。
- **恢复策略**:
  1. 客户端 SDK 内置指数退避与随机抖动（Jitter）：
     `wait_time = base_backoff * (2 ^ attempt) + rand(0, 50ms)`。
  2. Worker 重新拉取最新的文件内容与 AST 节点偏移，重新验证局部代码后再行提交。

---
**确认人**: 
- 云架构师: Ulysses (Staff Engineer)
- 运维总监: Mavis (Operations Lead)
