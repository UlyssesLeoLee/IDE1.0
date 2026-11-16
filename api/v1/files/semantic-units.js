// Vercel Serverless Function: GET /api/v1/files/semantic-units
import { coordinator } from "../_state.js";

export default function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();

  const fileId = req.query.file_id || "default:src/math.rs";
  const units = coordinator.getSemanticUnits(fileId);

  res.status(200).json({
    file_id: fileId,
    units,
  });
}
