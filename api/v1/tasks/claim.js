// Vercel Serverless Function: POST /api/v1/tasks/claim
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Idempotency-Key");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method !== "POST") return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });

  const { worker_id, workspace_id } = req.body || {};
  if (!worker_id || !workspace_id) {
    return res.status(400).json({ code: "BAD_REQUEST", message: "worker_id and workspace_id required" });
  }

  const task = coordinator.claimTask(worker_id, workspace_id);
  if (!task) {
    return res.status(204).end();
  }

  res.status(200).json({
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
}
