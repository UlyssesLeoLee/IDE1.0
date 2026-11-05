// Vercel Serverless Function: GET /api/v1/collab/stream
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();

  const tenant_id = req.query.tenant_id || "org_default";
  const workspace_id = req.query.workspace_id || "default";
  const file_id = req.query.file_id || null;
  const after_cursor = parseInt(req.query.after_cursor || "0", 10);

  const events = coordinator.getCollabEvents(tenant_id, workspace_id, file_id, after_cursor);

  // If client requests text/event-stream, return formatted SSE
  if (req.headers.accept && req.headers.accept.includes("text/event-stream")) {
    res.setHeader("Content-Type", "text/event-stream");
    res.setHeader("Cache-Control", "no-cache");
    res.setHeader("Connection", "keep-alive");

    for (const ev of events) {
      res.write(`id: ${ev.event_id}\n`);
      res.write(`event: ${ev.event_type}\n`);
      res.write(`data: ${JSON.stringify(ev)}\n\n`);
    }
    return res.end();
  }

  res.status(200).json({
    tenant_id,
    workspace_id,
    events,
    cursor: events.length > 0 ? events[events.length - 1].event_id : after_cursor,
  });
}
