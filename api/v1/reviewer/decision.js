// Vercel Serverless Function: POST /api/v1/reviewer/decision
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method !== "POST") return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });

  const body = req.body || {};
  const tenant_id = body.tenant_id || "org_default";
  const workspace_id = body.workspace_id || "default";
  const file_id = body.file_id || "src/math.rs";

  try {
    const certificate = coordinator.submitReviewDecision(tenant_id, workspace_id, file_id, body);
    res.status(200).json({ status: "ACCEPTED", certificate });
  } catch (err) {
    res.status(500).json({ code: "DECISION_ERROR", message: err.message });
  }
}
