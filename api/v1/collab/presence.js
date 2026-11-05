// Vercel Serverless Function: /api/v1/collab/presence
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();

  if (req.method === "POST") {
    const body = req.body || {};
    const tenant_id = body.tenant_id || "org_default";
    const workspace_id = body.workspace_id || "default";
    const file_id = body.file_id || "src/math.rs";

    try {
      const result = coordinator.registerPresence(tenant_id, workspace_id, file_id, body);
      return res.status(200).json(result);
    } catch (err) {
      const status = err.statusCode || 400;
      return res.status(status).json({ code: "PRESENCE_ERROR", message: err.message });
    }
  }

  if (req.method === "GET") {
    const tenant_id = req.query.tenant_id || "org_default";
    const workspace_id = req.query.workspace_id || "default";
    const file_id = req.query.file_id || "src/math.rs";

    const session = coordinator.getOrCreateCollabSession(tenant_id, workspace_id, file_id);
    return res.status(200).json({
      tenant_id,
      workspace_id,
      file_id,
      active_collaborators: Array.from(session.collaborators.values()),
      plan: session.upfrontPlan,
      certificate: session.certificate,
    });
  }

  res.status(405).json({ code: "METHOD_NOT_ALLOWED" });
}
