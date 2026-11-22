// Vercel Serverless Function: POST /api/v1/tasks/claim (Durable Postgres Lease + Auth)
import { authenticateRequest, authorizeScope, AuthError, enforcePayloadSize } from "../_auth.js";
import { getStore } from "../_store.js";
import { claimTaskDurable } from "../_tasks.js";

export default async function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Idempotency-Key");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method !== "POST") return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });

  try {
    enforcePayloadSize(req);
    const principal = authenticateRequest(req);
    const store = getStore();
    await store.enforceRateLimit(principal.tenantId);

    const { worker_id, workspace_id, ttl_ms } = req.body || {};
    if (!worker_id || !workspace_id) {
      return res.status(400).json({ code: "BAD_REQUEST", message: "worker_id and workspace_id required" });
    }
    authorizeScope(principal, workspace_id, "write");

    const task = await claimTaskDurable(store, {
      tenantId: principal.tenantId,
      workspaceId: workspace_id,
      workerId: `${principal.actorId}:${worker_id}`,
      ttlMs: ttl_ms,
    });
    if (!task) return res.status(204).end();

    return res.status(200).json({
      task_id: task.taskId,
      job_id: task.jobId,
      workspace_id: task.workspaceId,
      file_id: task.fileId,
      symbol_path: task.symbolPath,
      base_revision: task.baseRevision,
      fencing_token: task.fencingToken,
      lease_expires_at: task.leaseExpiresAt,
      expected_fingerprint: task.expectedFingerprint,
    });
  } catch (err) {
    if (err instanceof AuthError) {
      return res.status(err.statusCode).json({ code: err.code, message: err.message });
    }
    return res.status(err.statusCode || 500).json({ code: err.code || "INTERNAL_ERROR", message: err.message });
  }
}
