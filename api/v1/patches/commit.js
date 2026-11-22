// Vercel Serverless Function: POST /api/v1/patches/commit (Durable Postgres CAS + Auth)
import {
  authenticateRequest,
  authorizeScope,
  AuthError,
  enforcePayloadSize,
  resolveFileTarget,
} from "../_auth.js";
import { getStore } from "../_store.js";
import { commitTaskPatchDurable } from "../_tasks.js";

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

    const idempotencyKey = req.headers?.["idempotency-key"] || null;
    const proposal = req.body || {};
    const target = resolveFileTarget(proposal.file_id, principal);
    authorizeScope(principal, target.workspaceId, "write");

    const result = await commitTaskPatchDurable(store, {
      target,
      actorId: principal.actorId,
      proposal,
      idempotencyKey,
    });

    return res.status(result.ok ? 200 : 409).json(result);
  } catch (err) {
    if (err instanceof AuthError) {
      return res.status(err.statusCode).json({ code: err.code, message: err.message });
    }
    const status = err.statusCode || 409;
    return res.status(status).json({
      code: err.code || (status === 412 ? "FENCING_TOKEN_STALE" : "SEMANTIC_WRITE_CONFLICT"),
      message: err.message,
    });
  }
}
