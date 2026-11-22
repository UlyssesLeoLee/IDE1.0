// Vercel Serverless Function: GET /api/v1/events (Durable Postgres Outbox + Auth)
import { authenticateRequest, authorizeScope, AuthError } from "./_auth.js";
import { getStore } from "./_store.js";

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

    const workspaceId = String(req.query?.workspace_id || principal.defaultWorkspace || "default");
    authorizeScope(principal, workspaceId, "read");
    const afterCursor = parseInt(req.query?.after_cursor || "0", 10) || 0;

    const events = await store.getEvents(principal.tenantId, workspaceId, afterCursor);
    return res.status(200).json({ events });
  } catch (err) {
    if (err instanceof AuthError) {
      return res.status(err.statusCode).json({ code: err.code, message: err.message });
    }
    return res.status(err.statusCode || 500).json({ code: err.code || "INTERNAL_ERROR", message: err.message });
  }
}
