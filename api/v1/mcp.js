// MCP (Model Context Protocol) Server for IDE1.0 Serverless Deployment on Vercel
// Supports JSON-RPC 2.0 over HTTP (POST) and Discovery/Status (GET)

import { coordinator, diffLines, formatUnifiedDiff, threeWayMerge } from "./_state.js";

const TOOLS = [
  {
    name: "headless_coedit",
    description: "Multi-provider concurrent document co-editing engine. Allows Claude, GPT, DeepSeek, Gemini, etc. to edit the same file simultaneously in memory with CAS fast-path and 3-way line auto-merging.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Target document identifier, e.g. 'default:src/main.rs'" },
        author: { type: "string", description: "Editing agent or provider, e.g. 'claude-3-7-sonnet', 'gpt-4o', 'deepseek-v3'" },
        base_revision: { type: "number", description: "The base revision number when the agent inspected the file" },
        new_text: { type: "string", description: "The new document content" },
        op_id: { type: "string", description: "Optional unique operation ID for idempotency" }
      },
      required: ["file_id", "author", "new_text"]
    }
  },
  {
    name: "headless_read",
    description: "Read the current or historical revision text of a headless document.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Document ID" },
        revision: { type: "number", description: "Optional specific revision number" }
      },
      required: ["file_id"]
    }
  },
  {
    name: "headless_history",
    description: "Query document version history, author attributions, timestamps, and line counts.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Document ID" }
      },
      required: ["file_id"]
    }
  },
  {
    name: "headless_list_files",
    description: "List all active documents in headless memory.",
    inputSchema: {
      type: "object",
      properties: {}
    }
  },
  {
    name: "diff_unified",
    description: "Compute Myers Unified Diff between any two texts.",
    inputSchema: {
      type: "object",
      properties: {
        text_a: { type: "string", description: "Original text" },
        text_b: { type: "string", description: "Modified text" },
        label_a: { type: "string", description: "Label for text_a" },
        label_b: { type: "string", description: "Label for text_b" }
      },
      required: ["text_a", "text_b"]
    }
  },
  {
    name: "diff_revisions",
    description: "Compute Unified Diff between two historical revisions of a file.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Document ID" },
        rev_a: { type: "number", description: "Revision A" },
        rev_b: { type: "number", description: "Revision B" }
      },
      required: ["file_id", "rev_a", "rev_b"]
    }
  },
  {
    name: "ide_rust_eval",
    description: "Execute Blender bpy-style Rust automation commands (ide::context::*, ide::data::*, ide::ops::*).",
    inputSchema: {
      type: "object",
      properties: {
        script: { type: "string", description: "Rust statements separated by newlines or semicolons" }
      },
      required: ["script"]
    }
  },
  {
    name: "ide_open_file",
    description: "Open or initialize a document in memory with initial content.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Document ID" },
        content: { type: "string", description: "Initial file content" },
        language: { type: "string", description: "Language syntax mode (rust, js, etc.)" }
      },
      required: ["file_id"]
    }
  },
  {
    name: "ide_edit_lines",
    description: "Replace a range of lines in an existing document (1-based line numbers).",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Document ID" },
        start_line: { type: "number", description: "Start line number (1-based)" },
        end_line: { type: "number", description: "End line number (1-based, inclusive)" },
        replacement: { type: "string", description: "New lines text" }
      },
      required: ["file_id", "start_line", "end_line", "replacement"]
    }
  },
  {
    name: "ide_replace_all",
    description: "Find and replace all occurrences of a string in a document.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Document ID" },
        find: { type: "string", description: "String to find" },
        replace: { type: "string", description: "Replacement string" }
      },
      required: ["file_id", "find", "replace"]
    }
  }
];

export default async function handler(req, res) {
  // CORS setup
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

  if (req.method === "OPTIONS") {
    return res.status(200).end();
  }

  // GET: Discovery & Status
  if (req.method === "GET") {
    return res.status(200).json({
      server: "IDE1.0 MCP Server (Vercel Serverless)",
      version: "0.8.6",
      status: "operational",
      protocolVersion: "2024-11-05",
      endpoints: {
        mcp: "/api/v1/mcp",
        health: "/api/v1/health",
        events: "/api/v1/events"
      },
      tools: TOOLS.map(t => ({ name: t.name, description: t.description }))
    });
  }

  // POST: MCP JSON-RPC 2.0 Handler
  if (req.method === "POST") {
    let body = req.body;
    if (typeof body === "string") {
      try {
        body = JSON.parse(body);
      } catch (e) {
        return res.status(400).json({
          jsonrpc: "2.0",
          error: { code: -32700, message: "Parse error" },
          id: null
        });
      }
    }

    const { id, method, params } = body || {};

    if (!method) {
      return res.status(400).json({
        jsonrpc: "2.0",
        error: { code: -32600, message: "Invalid Request: missing method" },
        id: id || null
      });
    }

    try {
      switch (method) {
        case "initialize":
          return res.status(200).json({
            jsonrpc: "2.0",
            id,
            result: {
              protocolVersion: "2024-11-05",
              capabilities: {
                tools: { listChanged: false },
                resources: { subscribe: false, listChanged: false },
                prompts: { listChanged: false }
              },
              serverInfo: {
                name: "ide1-mcp-server",
                version: "0.8.6"
              }
            }
          });

        case "ping":
          return res.status(200).json({ jsonrpc: "2.0", id, result: {} });

        case "tools/list":
          return res.status(200).json({ jsonrpc: "2.0", id, result: { tools: TOOLS } });

        case "resources/list": {
          const files = coordinator.headlessListFiles();
          const resources = files.map(f => ({
            uri: `file:///workspace/${f.fileId}`,
            name: f.fileId,
            mimeType: "text/plain"
          }));
          return res.status(200).json({ jsonrpc: "2.0", id, result: { resources } });
        }

        case "resources/read": {
          const uri = params?.uri || "";
          const fileId = uri.replace(/^file:\/\/\/workspace\//, "");
          const content = coordinator.headlessRead(fileId);
          if (content === null) {
            return res.status(200).json({
              jsonrpc: "2.0",
              id,
              error: { code: -32001, message: `Resource '${uri}' not found` }
            });
          }
          return res.status(200).json({
            jsonrpc: "2.0",
            id,
            result: { contents: [{ uri, mimeType: "text/plain", text: content }] }
          });
        }

        case "prompts/list":
          return res.status(200).json({
            jsonrpc: "2.0",
            id,
            result: {
              prompts: [
                {
                  name: "review_code",
                  description: "Review open file changes and suggest improvements",
                  arguments: [{ name: "file_id", description: "Target document ID", required: true }]
                }
              ]
            }
          });

        case "tools/call": {
          const { name, arguments: args } = params || {};
          let textResult = "";

          switch (name) {
            case "headless_coedit": {
              const outcome = coordinator.headlessEdit({
                fileId: args.file_id,
                author: args.author,
                baseRevision: args.base_revision,
                opId: args.op_id,
                newText: args.new_text
              });
              textResult = JSON.stringify(outcome, null, 2);
              break;
            }

            case "headless_read": {
              const content = coordinator.headlessRead(args.file_id, args.revision);
              if (content === null) {
                textResult = `File '${args.file_id}' not found`;
              } else {
                textResult = content;
              }
              break;
            }

            case "headless_history": {
              const hist = coordinator.headlessHistory(args.file_id);
              textResult = JSON.stringify(hist, null, 2);
              break;
            }

            case "headless_list_files": {
              const list = coordinator.headlessListFiles();
              textResult = JSON.stringify(list, null, 2);
              break;
            }

            case "diff_unified": {
              const linesA = (args.text_a || "").split("\n");
              const linesB = (args.text_b || "").split("\n");
              textResult = formatUnifiedDiff(linesA, linesB, args.label_a || "original", args.label_b || "modified");
              if (!textResult) {
                textResult = "(Identical texts: zero differences)";
              }
              break;
            }

            case "diff_revisions": {
              textResult = coordinator.headlessDiff(args.file_id, args.rev_a, args.rev_b);
              break;
            }

            case "ide_rust_eval": {
              textResult = coordinator.evalRustScript(args.script || "");
              break;
            }

            case "ide_open_file": {
              const parts = (args.file_id || "").split(":");
              const ws = parts.length > 1 ? parts[0] : "default";
              const p = parts.length > 1 ? parts.slice(1).join(":") : args.file_id;
              const f = coordinator.registerFile(ws, p, args.content || "", args.language || "plain");
              textResult = JSON.stringify(f, null, 2);
              break;
            }

            case "ide_edit_lines": {
              const content = coordinator.headlessRead(args.file_id);
              if (content === null) {
                textResult = `Error: file '${args.file_id}' not found`;
                break;
              }
              const lines = content.split("\n");
              const s = Math.max(0, (args.start_line || 1) - 1);
              const e = Math.min(lines.length, args.end_line || lines.length);
              const replLines = (args.replacement || "").split("\n");
              lines.splice(s, e - s, ...replLines);
              const newContent = lines.join("\n");
              const outcome = coordinator.headlessEdit({
                fileId: args.file_id,
                author: "ide_edit_lines",
                newText: newContent
              });
              textResult = JSON.stringify(outcome, null, 2);
              break;
            }

            case "ide_replace_all": {
              const content = coordinator.headlessRead(args.file_id);
              if (content === null) {
                textResult = `Error: file '${args.file_id}' not found`;
                break;
              }
              const newContent = content.split(args.find).join(args.replace);
              const outcome = coordinator.headlessEdit({
                fileId: args.file_id,
                author: "ide_replace_all",
                newText: newContent
              });
              textResult = JSON.stringify(outcome, null, 2);
              break;
            }

            default:
              return res.status(200).json({
                jsonrpc: "2.0",
                id,
                error: { code: -32601, message: `Tool '${name}' not found` }
              });
          }

          return res.status(200).json({
            jsonrpc: "2.0",
            id,
            result: {
              content: [{ type: "text", text: textResult }]
            }
          });
        }

        default:
          return res.status(200).json({
            jsonrpc: "2.0",
            id,
            error: { code: -32601, message: `Method '${method}' not found` }
          });
      }
    } catch (err) {
      return res.status(500).json({
        jsonrpc: "2.0",
        id,
        error: { code: -32603, message: err.message || "Internal error" }
      });
    }
  }

  return res.status(405).json({ error: "Method not allowed" });
}
