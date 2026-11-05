// Vercel Serverless Function: POST /api/v1/collab/keystroke
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
  const agent_id = body.agent_id || "anonymous";
  const op = body.op || {};
  const cursor_after = body.cursor_after || [0, 0];
  const selection_after = body.selection_after || null;

  try {
    const broadcast = coordinator.applyKeystroke(
      tenant_id,
      workspace_id,
      file_id,
      agent_id,
      op,
      cursor_after,
      selection_after
    );
    res.status(200).json({ status: "BROADCASTED", broadcast });
  } catch (err) {
    res.status(500).json({ code: "KEYSTROKE_ERROR", message: err.message });
  }
}
