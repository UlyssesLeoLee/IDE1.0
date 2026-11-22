// Vercel Serverless Function: GET /api/v1/files/semantic-units (Durable Postgres + Auth)
import {
  authenticateRequest,
  authorizeScope,
  AuthError,
  resolveFileTarget,
} from "../_auth.js";
import { getStore } from "../_store.js";
import { extractSemanticUnits } from "../_tasks.js";

export default async function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method !== "GET") return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });

  try {
    const principal = authenticateRequest(req);
    const store = getStore();
    await store.enforceRateLimit(principal.tenantId);

    const rawFileId = req.query?.file_id || "default:src/math.rs";
    const target = resolveFileTarget(rawFileId, principal);
    authorizeScope(principal, target.workspaceId, "read");

    const doc = await store.readFile(target);
    const units = extractSemanticUnits(
      target.tenantId,
      target.workspaceId,
      doc.file_id,
      doc.language,
      doc.content
    );
    return res.status(200).json({ file_id: doc.file_id, units });
  } catch (err) {
    if (err instanceof AuthError) {
      return res.status(err.statusCode).json({ code: err.code, message: err.message });
    }
    return res.status(err.statusCode || 500).json({ code: err.code || "INTERNAL_ERROR", message: err.message });
  }
}
