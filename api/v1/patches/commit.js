// Vercel Serverless Function: POST /api/v1/patches/commit
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Idempotency-Key");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method !== "POST") return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });

  const idempotencyKey = req.headers["idempotency-key"] || null;
  const proposal = req.body || {};

  try {
    const result = coordinator.commitPatch(proposal, idempotencyKey);
    res.status(200).json(result);
  } catch (err) {
    const status = err.statusCode || 409;
    res.status(status).json({
      code: status === 412 ? "FENCING_TOKEN_STALE" : "SEMANTIC_WRITE_CONFLICT",
      message: err.message,
    });
  }
}
