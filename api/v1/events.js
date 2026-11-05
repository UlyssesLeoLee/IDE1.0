// Vercel Serverless Function: GET /api/v1/events
import { coordinator } from "./_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();

  const workspace_id = req.query.workspace_id || "default";
  const after_cursor = parseInt(req.query.after_cursor || "0", 10);

  const events = coordinator.getEvents(workspace_id, after_cursor);
  res.status(200).json({ events });
}
