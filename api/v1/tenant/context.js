// Vercel Serverless Function: GET /api/v1/tenant/context
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();

  const tenant_id = req.query.tenant_id || "org_default";
  const context = coordinator.getTenantContext(tenant_id);

  res.status(200).json(context);
}
