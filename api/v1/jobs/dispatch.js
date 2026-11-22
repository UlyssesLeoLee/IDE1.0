// Vercel Serverless Function: POST /api/v1/jobs/dispatch (Durable Postgres + Auth)
import {
  authenticateRequest,
  authorizeScope,
  AuthError,
  enforcePayloadSize,
  resolveFileTarget,
} from "../_auth.js";
import { getStore } from "../_store.js";
import { dispatchJobDurable } from "../_tasks.js";

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

    const { workspace_id, goal, target_files, priority } = req.body || {};
    if (!workspace_id || !goal) {
      return res.status(400).json({ code: "BAD_REQUEST", message: "workspace_id and goal are required" });
    }
    authorizeScope(principal, workspace_id, "write");

    const result = await dispatchJobDurable(store, {
      tenantId: principal.tenantId,
      workspaceId: workspace_id,
      actorId: principal.actorId,
      goal,
      targetFiles: target_files,
      priority: priority || 100,
      resolveTargetFn: (rawId) => {
        const t = resolveFileTarget(rawId, { ...principal, defaultWorkspace: workspace_id });
        authorizeScope(principal, t.workspaceId, "write");
        return t;
      },
    });

    return res.status(202).json({
      job_id: result.jobId,
      workspace_id,
      task_count: result.taskIds.length,
      status: result.status,
    });
  } catch (err) {
    if (err instanceof AuthError) {
      return res.status(err.statusCode).json({ code: err.code, message: err.message });
    }
    return res.status(err.statusCode || 500).json({ code: err.code || "INTERNAL_ERROR", message: err.message });
  }
}
