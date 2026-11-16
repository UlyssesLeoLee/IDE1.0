# IDE1.0 Web — PostgreSQL 数据模式与状态机详细设计书

**文档编号**: DD-ROUTER-DB-2026-V2.0  
**制定组织**: IDE1.0 内核与云架构委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.0  
**状态**: 承认済 (Baseline Locked)  
**上游设计**: [Web-Agent-Router-Basic-Design.md](../01_基本设计/Web-Agent-Router-Basic-Design.md)

---

## 1. 实体关系模型 (ER Model)

```
[Tenants] ──1:N──► [Workspaces] ──1:N──► [Files] ──1:N──► [FileRevisions]
                         │                   │
                         │                   ├──1:N──► [SemanticUnits]
                         │                   │
                         ├──1:N──► [Jobs]    └──1:N──► [DependencyEdges]
                         │           │
                         │          1:N
                         │           ▼
                         │        [Tasks] ──1:N──► [TaskLeases]
                         │           │
                         │          1:N
                         │           ▼
                         ├──1:N──► [PatchProposals]
                         │           │
                         │           ▼
                         └──1:N──► [PatchCommits] ──1:1──► [OutboxEvents]
```

---

## 2. 生产级 PostgreSQL DDL 规格定义

```sql
-- ============================================================
-- 1. 租户与工作空间表
-- ============================================================
CREATE TABLE IF NOT EXISTS tenants (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    plan_tier VARCHAR(32) NOT NULL DEFAULT 'standard',
    max_concurrent_workers INT NOT NULL DEFAULT 50,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS workspaces (
    id VARCHAR(64) PRIMARY KEY,
    tenant_id VARCHAR(64) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    default_branch VARCHAR(64) NOT NULL DEFAULT 'main',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 2. 权威文件与修订历史表 (Authoritative File Ledger)
-- ============================================================
CREATE TABLE IF NOT EXISTS files (
    id VARCHAR(128) PRIMARY KEY, -- format: {workspace_id}:{canonical_path}
    workspace_id VARCHAR(64) NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    path VARCHAR(1024) NOT NULL,
    current_revision INT NOT NULL DEFAULT 1,
    latest_content_hash CHAR(64) NOT NULL,
    language VARCHAR(32) NOT NULL,
    is_exclusive_locked BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_file_path UNIQUE (workspace_id, path)
);

CREATE TABLE IF NOT EXISTS file_revisions (
    id BIGSERIAL PRIMARY KEY,
    file_id VARCHAR(128) NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    revision INT NOT NULL,
    content_hash CHAR(64) NOT NULL,
    content_blob_url TEXT NOT NULL, -- S3/Vercel Blob 不可变对象引用
    commit_author VARCHAR(128) NOT NULL,
    commit_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_file_rev UNIQUE (file_id, revision)
);

-- ============================================================
-- 3. AST 语义单元元数据表 (Semantic Units Index)
-- ============================================================
CREATE TABLE IF NOT EXISTS semantic_units (
    id VARCHAR(128) PRIMARY KEY, -- format: {file_id}:{qualified_symbol_path}
    file_id VARCHAR(128) NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    qualified_symbol_path VARCHAR(512) NOT NULL,
    node_kind VARCHAR(64) NOT NULL,
    signature_hash CHAR(64) NOT NULL,
    base_content_hash CHAR(64) NOT NULL,
    node_fingerprint CHAR(64) NOT NULL,
    start_byte INT NOT NULL,
    end_byte INT NOT NULL,
    start_line INT NOT NULL,
    end_line INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_semantic_units_file ON semantic_units(file_id);
CREATE INDEX IF NOT EXISTS idx_semantic_units_fp ON semantic_units(node_fingerprint);

-- ============================================================
-- 4. 任务调度与分布式租约表 (Task Plane & Leases)
-- ============================================================
CREATE TABLE IF NOT EXISTS jobs (
    id VARCHAR(64) PRIMARY KEY,
    workspace_id VARCHAR(64) NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    goal TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'QUEUED', -- QUEUED, RUNNING, COMPLETED, FAILED
    priority INT NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS tasks (
    id VARCHAR(64) PRIMARY KEY,
    job_id VARCHAR(64) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    workspace_id VARCHAR(64) NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    file_id VARCHAR(128) NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    symbol_path VARCHAR(512) NOT NULL,
    base_revision INT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'READY', -- READY, CLAIMED, VALIDATING, COMMITTED, CONFLICT, EXPIRED
    worker_id VARCHAR(64),
    fencing_token INT NOT NULL DEFAULT 0,
    lease_expires_at TIMESTAMPTZ,
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tasks_claimable ON tasks(status, priority, created_at) WHERE status = 'READY';
CREATE INDEX IF NOT EXISTS idx_tasks_file ON tasks(file_id, status);

CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id VARCHAR(64) NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id VARCHAR(64) NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on_task_id)
);

-- ============================================================
-- 5. 变更提议与原子提交表 (Proposals & Commits)
-- ============================================================
CREATE TABLE IF NOT EXISTS patch_proposals (
    id VARCHAR(64) PRIMARY KEY,
    task_id VARCHAR(64) NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    worker_id VARCHAR(64) NOT NULL,
    fencing_token INT NOT NULL,
    file_id VARCHAR(128) NOT NULL,
    base_revision INT NOT NULL,
    symbol_path VARCHAR(512) NOT NULL,
    replacement_text TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'PROPOSED', -- PROPOSED, REJECTED, APPLIED
    evidence_diagnostics JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS patch_commits (
    id VARCHAR(64) PRIMARY KEY,
    task_id VARCHAR(64) NOT NULL REFERENCES tasks(id),
    file_id VARCHAR(128) NOT NULL REFERENCES files(id),
    symbol_path VARCHAR(512) NOT NULL,
    previous_revision INT NOT NULL,
    new_revision INT NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL UNIQUE,
    relocated BOOLEAN NOT NULL DEFAULT FALSE,
    committed_by_worker VARCHAR(64) NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS change_sets (
    id VARCHAR(64) PRIMARY KEY,
    workspace_id VARCHAR(64) NOT NULL REFERENCES workspaces(id),
    description TEXT,
    status VARCHAR(32) NOT NULL DEFAULT 'COMMITTED',
    committed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 6. 可靠事务发件箱 (Transactional Outbox)
-- ============================================================
CREATE TABLE IF NOT EXISTS outbox_events (
    id BIGSERIAL PRIMARY KEY,
    workspace_id VARCHAR(64) NOT NULL,
    event_type VARCHAR(64) NOT NULL, -- FILE_COMMITTED, CHANGE_SET_COMMITTED, CONFLICT_DETECTED
    payload JSONB NOT NULL,
    published BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_outbox_unpublished ON outbox_events(id) WHERE published = FALSE;
```

---

## 3. 核心业务原子事务模板

### 3.1 Worker 任务认领事务 (Claim Task with Atomic Fencing Bump)
```sql
BEGIN;

SELECT id, file_id, symbol_path, base_revision, fencing_token 
FROM tasks 
WHERE status = 'READY' 
  AND workspace_id = $1
ORDER BY priority DESC, created_at ASC 
LIMIT 1 
FOR UPDATE SKIP LOCKED;

-- 若成功获取目标行，执行原子认领与 Fencing 递增
UPDATE tasks 
SET status = 'CLAIMED',
    worker_id = $2,
    fencing_token = fencing_token + 1,
    lease_expires_at = NOW() + INTERVAL '60 SECONDS',
    updated_at = NOW()
WHERE id = $target_id;

COMMIT;
```

### 3.2 Coordinator CAS 提交事务 (Optimistic CAS Commit with Outbox)
```sql
BEGIN;

-- 1. 严格检查 Fencing Token 是否有效且未被抢占
SELECT fencing_token, lease_expires_at 
FROM tasks 
WHERE id = $task_id 
FOR UPDATE;

IF current_fencing_token != $request_fencing_token THEN
    ROLLBACK;
    RAISE EXCEPTION 'FENCING_TOKEN_STALE';
END IF;

-- 2. 检查幂等性
IF EXISTS (SELECT 1 FROM patch_commits WHERE idempotency_key = $idempotency_key) THEN
    -- 直接返回已提交的结果
    COMMIT;
    RETURN;
END IF;

-- 3. 锁定并校验文件权威版本
SELECT current_revision, latest_content_hash 
FROM files 
WHERE id = $file_id 
FOR UPDATE;

-- 4. 若 revision 不一致但经 Rust AST-aware 重定位成功：
UPDATE files 
SET current_revision = current_revision + 1,
    latest_content_hash = $new_content_hash,
    updated_at = NOW()
WHERE id = $file_id;

-- 5. 写入修订记录与提交凭据
INSERT INTO file_revisions (file_id, revision, content_hash, content_blob_url, commit_author)
VALUES ($file_id, $new_revision, $new_content_hash, $blob_url, $worker_id);

INSERT INTO patch_commits (id, task_id, file_id, symbol_path, previous_revision, new_revision, idempotency_key, relocated, committed_by_worker)
VALUES ($commit_id, $task_id, $file_id, $symbol_path, $prev_rev, $new_revision, $idempotency_key, $relocated, $worker_id);

UPDATE tasks SET status = 'COMMITTED', updated_at = NOW() WHERE id = $task_id;

-- 6. 事务内插入发件箱消息
INSERT INTO outbox_events (workspace_id, event_type, payload)
VALUES ($workspace_id, 'FILE_COMMITTED', jsonb_build_object(
    'file_id', $file_id,
    'previous_revision', $prev_rev,
    'new_revision', $new_revision,
    'symbol_path', $symbol_path,
    'worker_id', $worker_id
));

COMMIT;
```

---
**确认人**: 
- 分布式数据架构师: Ulysses (Staff Engineer)
- 日本 IPA 评审官: Approved 2026-11-05
