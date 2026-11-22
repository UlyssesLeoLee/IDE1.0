// Durable Job Dispatch, Task Leasing (FOR UPDATE SKIP LOCKED) & Semantic Units (P0-02, P0-03, P0-05)

import crypto from "crypto";
import { splitLines } from "./_merge.js";
import { sha256Hex, StoreError } from "./_store.js";

export function extractSemanticUnits(tenantId, workspaceId, fileId, language, content) {
  const lines = splitLines(content);
  const units = [];
  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    if (
      trimmed.startsWith("pub fn ") ||
      trimmed.startsWith("fn ") ||
      trimmed.startsWith("export function ") ||
      trimmed.startsWith("export async function ") ||
      trimmed.startsWith("function ") ||
      trimmed.startsWith("def ")
    ) {
      const m = trimmed.match(/(?:fn|function|def)\s+([A-Za-z_][A-Za-z0-9_]*)/);
      const name = m ? m[1] : `block_${i + 1}`;
      const sigHash = sha256Hex(trimmed);
      units.push({
        tenant_id: tenantId,
        workspace_id: workspaceId,
        file_id: fileId,
        language,
        qualified_symbol_path: name,
        node_kind: "function_item",
        signature_hash: sigHash,
        base_content_hash: sigHash,
        node_fingerprint: sha256Hex(`${tenantId}:${fileId}:${name}:${sigHash}`),
        start_line: i + 1,
        end_line: Math.min(lines.length, i + 3),
      });
    }
  }
  return units;
}

export async function dispatchJobDurable(store, { tenantId, workspaceId, actorId, goal, targetFiles, priority = 100, resolveTargetFn }) {
  const jobId = `job_${Date.now()}_${crypto.randomBytes(4).toString("hex")}`;
  const files = Array.isArray(targetFiles) && targetFiles.length > 0 ? targetFiles : [`${workspaceId}:src/math.rs`];

  return store.withTransaction(async (client) => {
    await client.query(
      `INSERT INTO mcp_jobs (job_id, tenant_id, workspace_id, goal, priority, status)
       VALUES ($1, $2, $3, $4, $5, 'DISPATCHED')`,
      [jobId, tenantId, workspaceId, String(goal), Number(priority) || 100]
    );

    const taskIds = [];
    for (const rawTarget of files) {
      const target = resolveTargetFn(rawTarget);
      const docRes = await client.query(
        `SELECT current_revision, content, language FROM mcp_documents
         WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3`,
        [tenantId, target.workspaceId, target.filePath]
      );
      const doc = docRes.rows[0];
      const baseRev = doc ? Number(doc.current_revision) : 1;
      const units = doc
        ? extractSemanticUnits(tenantId, target.workspaceId, target.fileId, doc.language, doc.content)
        : [{ qualified_symbol_path: "__file__", node_fingerprint: sha256Hex(target.fileId) }];
      const chosen = units.length > 0 ? units : [{ qualified_symbol_path: "__file__", node_fingerprint: sha256Hex(target.fileId) }];

      for (const u of chosen) {
        const taskId = `task_${Date.now()}_${crypto.randomBytes(4).toString("hex")}`;
        await client.query(
          `INSERT INTO mcp_tasks
           (task_id, job_id, tenant_id, workspace_id, file_path, file_id, symbol_path,
            base_revision, status, expected_fingerprint, priority)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'READY',$9,$10)`,
          [
            taskId,
            jobId,
            tenantId,
            target.workspaceId,
            target.filePath,
            target.fileId,
            u.qualified_symbol_path,
            baseRev,
            u.node_fingerprint,
            Number(priority) || 100,
          ]
        );
        taskIds.push(taskId);
      }
    }

    await store.recordAudit(client, {
      tenantId,
      actorId,
      workspaceId,
      filePath: files.join(","),
      action: "jobs_dispatch",
      outcome: "DISPATCHED",
      statusCode: 202,
      detail: { job_id: jobId, task_count: taskIds.length },
    });

    return { jobId, taskIds, status: "DISPATCHED" };
  });
}

export async function claimTaskDurable(store, { tenantId, workspaceId, workerId, ttlMs = 60000 }) {
  const safeTtl = Math.max(5000, Math.min(Number(ttlMs) || 60000, 300000));
  return store.withTransaction(async (client) => {
    const pick = await client.query(
      `SELECT * FROM mcp_tasks
       WHERE tenant_id = $1 AND workspace_id = $2
         AND (status = 'READY' OR (status = 'CLAIMED' AND lease_expires_at < NOW()))
       ORDER BY priority DESC, created_at ASC
       LIMIT 1
       FOR UPDATE SKIP LOCKED`,
      [tenantId, workspaceId]
    );
    if (pick.rows.length === 0) return null;
    const t = pick.rows[0];

    const tokRes = await client.query(
      `UPDATE mcp_documents
       SET next_fencing_token = next_fencing_token + 1
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3
       RETURNING next_fencing_token`,
      [tenantId, t.workspace_id, t.file_path]
    );
    const token = tokRes.rows.length > 0 ? Number(tokRes.rows[0].next_fencing_token) : Number(t.fencing_token || 0) + 1;

    if (tokRes.rows.length > 0) {
      await client.query(
        `INSERT INTO mcp_leases (tenant_id, workspace_id, file_path, agent_id, fencing_token, expires_at)
         VALUES ($1, $2, $3, $4, $5, NOW() + ($6::bigint * INTERVAL '1 millisecond'))
         ON CONFLICT (tenant_id, workspace_id, file_path) DO UPDATE SET
           agent_id = EXCLUDED.agent_id,
           fencing_token = EXCLUDED.fencing_token,
           expires_at = EXCLUDED.expires_at`,
        [tenantId, t.workspace_id, t.file_path, workerId, token, safeTtl]
      );
    }

    const upd = await client.query(
      `UPDATE mcp_tasks
       SET status = 'CLAIMED', worker_id = $2, fencing_token = $3,
           lease_expires_at = NOW() + ($4::bigint * INTERVAL '1 millisecond')
       WHERE task_id = $1
       RETURNING lease_expires_at`,
      [t.task_id, workerId, token, safeTtl]
    );

    return {
      taskId: t.task_id,
      jobId: t.job_id,
      workspaceId: t.workspace_id,
      fileId: t.file_id,
      symbolPath: t.symbol_path,
      baseRevision: Number(t.base_revision),
      fencingToken: token,
      leaseExpiresAt: upd.rows[0].lease_expires_at,
      expectedFingerprint: t.expected_fingerprint,
    };
  });
}

export async function commitTaskPatchDurable(store, { target, actorId, proposal, idempotencyKey }) {
  const opId = idempotencyKey || proposal.op_id;
  if (!opId) {
    throw new StoreError(400, "MISSING_OP_ID", "Idempotency-Key header or op_id is required");
  }
  const newText = proposal.new_text !== undefined ? proposal.new_text : proposal.content;
  if (typeof newText !== "string") {
    throw new StoreError(400, "INVALID_PATCH", "new_text (or content) string is required");
  }

  const res = await store.coedit({
    ...target,
    actorId: proposal.worker_id || proposal.agent_id || actorId,
    baseRevision: proposal.base_revision,
    baseContentHash: proposal.base_content_hash || null,
    fencingToken: proposal.fencing_token !== undefined ? proposal.fencing_token : null,
    opId,
    newText,
    mode: proposal.mode || null,
  });

  if (proposal.task_id) {
    await store.getPool().query(
      `UPDATE mcp_tasks SET status = $2 WHERE task_id = $1 AND tenant_id = $3`,
      [proposal.task_id, res.ok ? "COMMITTED" : "CONFLICT", target.tenantId]
    );
  }
  return res;
}
