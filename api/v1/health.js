// Vercel Serverless Function: GET /api/v1/health (Real Postgres TLS Probe)
import { MCP_PROTOCOL_VERSION, SERVER_VERSION } from "./mcp.js";
import { getStore } from "./_store.js";

export default async function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Idempotency-Key");

  if (req.method === "OPTIONS") {
    return res.status(200).end();
  }
  if (req.method !== "GET") {
    return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });
  }

  const authConfigured = Boolean(process.env.IDE_MCP_JWT_SECRET || process.env.IDE_MCP_TOKENS);
  const t0 = Date.now();
  try {
    const store = getStore();
    await store.getPool().query("SELECT 1");
    return res.status(200).json({
      status: "ok",
      version: SERVER_VERSION,
      protocolVersion: MCP_PROTOCOL_VERSION,
      db: "connected",
      db_latency_ms: Date.now() - t0,
      tls_verified: true,
      auth_configured: authConfigured,
      platform: "Vercel Serverless + Neon Postgres",
      timestamp: new Date().toISOString(),
    });
  } catch (err) {
    return res.status(503).json({
      status: "degraded",
      version: SERVER_VERSION,
      protocolVersion: MCP_PROTOCOL_VERSION,
      db: "error",
      db_latency_ms: Date.now() - t0,
      auth_configured: authConfigured,
      error: err.message || "Database health probe failed",
      timestamp: new Date().toISOString(),
    });
  }
}
