// Vercel Serverless Function: GET /api/v1/health
export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Idempotency-Key");

  if (req.method === "OPTIONS") {
    return res.status(200).end();
  }

  res.status(200).json({
    status: "ok",
    version: "2.0.0",
    engine: "ide-agent-router-rust",
    platform: "Vercel Serverless / Edge Gateway",
    timestamp: new Date().toISOString(),
  });
}
