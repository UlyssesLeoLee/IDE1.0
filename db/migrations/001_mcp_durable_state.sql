-- IDE1.0 Vercel MCP 持久化事务状态存储迁移脚本 (P0-01..P0-05)
-- 支持多实例 Serverless 无状态水平扩展、单文件悲观行锁 + 乐观 CAS 校验、作用域幂等与审计日志。

CREATE TABLE IF NOT EXISTS mcp_documents (
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  file_id TEXT NOT NULL,
  language TEXT NOT NULL DEFAULT 'plain',
  current_revision BIGINT NOT NULL,
  content TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  next_fencing_token BIGINT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (tenant_id, workspace_id, file_path)
);

CREATE TABLE IF NOT EXISTS mcp_revisions (
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  revision BIGINT NOT NULL,
  content TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  provider TEXT NOT NULL DEFAULT 'mcp',
  op_id TEXT,
  auto_merged BOOLEAN NOT NULL DEFAULT FALSE,
  summary TEXT NOT NULL DEFAULT '',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (tenant_id, workspace_id, file_path, revision),
  FOREIGN KEY (tenant_id, workspace_id, file_path)
    REFERENCES mcp_documents (tenant_id, workspace_id, file_path) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS mcp_proposals (
  proposal_id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  file_id TEXT NOT NULL,
  base_revision BIGINT NOT NULL,
  head_revision BIGINT NOT NULL,
  head_content_hash TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  op_id TEXT,
  proposed_content TEXT NOT NULL,
  proposed_hash TEXT NOT NULL,
  conflict_preview TEXT NOT NULL,
  conflict_diff TEXT NOT NULL,
  conflicts_json JSONB NOT NULL,
  status TEXT NOT NULL DEFAULT 'PENDING_REVIEW',
  resolved_by TEXT,
  resolved_revision BIGINT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_mcp_proposals_scope
  ON mcp_proposals (tenant_id, workspace_id, file_path, status, created_at DESC);

CREATE TABLE IF NOT EXISTS mcp_idempotency (
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  op_id TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  response_json JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (tenant_id, workspace_id, file_path, op_id)
);

CREATE TABLE IF NOT EXISTS mcp_leases (
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  fencing_token BIGINT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (tenant_id, workspace_id, file_path)
);

CREATE TABLE IF NOT EXISTS mcp_jobs (
  job_id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  goal TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 100,
  status TEXT NOT NULL DEFAULT 'DISPATCHED',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS mcp_tasks (
  task_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES mcp_jobs (job_id) ON DELETE CASCADE,
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  file_id TEXT NOT NULL,
  symbol_path TEXT NOT NULL,
  base_revision BIGINT NOT NULL,
  fencing_token BIGINT NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'READY',
  worker_id TEXT,
  lease_expires_at TIMESTAMPTZ,
  expected_fingerprint TEXT,
  priority INTEGER NOT NULL DEFAULT 100,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS mcp_outbox (
  event_id BIGSERIAL PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  file_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  revision BIGINT NOT NULL,
  payload_json JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mcp_outbox_scope
  ON mcp_outbox (tenant_id, workspace_id, event_id);

CREATE TABLE IF NOT EXISTS mcp_audit_log (
  audit_id BIGSERIAL PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  actor_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  action TEXT NOT NULL,
  outcome TEXT NOT NULL,
  op_id TEXT,
  status_code INTEGER NOT NULL,
  detail JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mcp_audit_scope
  ON mcp_audit_log (tenant_id, created_at DESC);

CREATE TABLE IF NOT EXISTS mcp_rate_limits (
  bucket_key TEXT PRIMARY KEY,
  window_start TIMESTAMPTZ NOT NULL,
  req_count INTEGER NOT NULL
);
