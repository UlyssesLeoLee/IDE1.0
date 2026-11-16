// Vercel Serverless Function: POST /api/v1/jobs/dispatch
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Idempotency-Key");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method !== "POST") return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });

  const { workspace_id, goal, target_files, priority } = req.body || {};
  if (!workspace_id || !goal) {
    return res.status(400).json({ code: "BAD_REQUEST", message: "workspace_id and goal are required" });
  }

  const result = coordinator.dispatchJob(workspace_id, goal, target_files || ["default:src/math.rs"], priority || 100);
  res.status(202).json({
    job_id: result.jobId,
    workspace_id,
    task_count: result.taskIds.length,
    status: result.status,
  });
}
