// Root handler for IDE1.0 Web & MCP Serverless
// On GET: Serves the 3D Cel-Shaded Diamond Stage and AI Integration Guide
// On POST: Dispatches directly to MCP JSON-RPC 2.0 Engine

import mcpHandler from "./v1/mcp.js";
import fs from "fs";
import path from "path";

export default async function handler(req, res) {
  // CORS setup
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") {
    return res.status(200).end();
  }

  // Handle MCP JSON-RPC POST at root
  if (req.method === "POST") {
    return mcpHandler(req, res);
  }

  // Serve 3D Diamond Index on GET / HEAD
  if (req.method === "GET" || req.method === "HEAD") {
    // If client specifically requests JSON (e.g. MCP automated discovery), forward to mcpHandler
    const accept = req.headers["accept"] || "";
    if (accept.includes("application/json") && !accept.includes("text/html")) {
      return mcpHandler(req, res);
    }

    try {
      const htmlPath = path.join(process.cwd(), "public", "index.html");
      if (fs.existsSync(htmlPath)) {
        const html = fs.readFileSync(htmlPath, "utf-8");
        res.setHeader("Content-Type", "text/html; charset=utf-8");
        return res.status(200).send(html);
      }
    } catch (_err) {
      // fallback to mcpHandler discovery if filesystem read fails
    }
  }

  return mcpHandler(req, res);
}
