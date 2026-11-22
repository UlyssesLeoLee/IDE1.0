// Vercel Serverless Function: POST /api/v1/tasks/claim (Durable Postgres Lease + Auth)
import { authorizeScope } from "../_auth.js";
import { withProtectedApi } from "../_http.js";
import { claimTaskDurable } from "../_tasks.js";

export default withProtectedApi({
  methods: ["POST"],
  headers: "Content-Type, Authorization, Idempotency-Key",
  checkPayload: true,
  handler: async (req, res, { principal, store }) => {
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
  },
});
