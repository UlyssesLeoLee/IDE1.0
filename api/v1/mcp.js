// Hardened Vercel Serverless MCP Gateway (Streamable HTTP + JSON-RPC 2.0)
// Enforces Bearer JWT/Token Auth, Tenant/Workspace ACL, Durable Postgres CAS & Conflict Proposals.

import crypto from "crypto";
import {
  authenticateRequest,
  AuthError,
  authorizeScope,
  enforcePayloadSize,
  resolveFileTarget,
} from "./_auth.js";
import { formatUnifiedDiff, threeWayMerge } from "./_merge.js";
import { getStore, StoreError } from "./_store.js";

export const MCP_PROTOCOL_VERSION = "2024-11-05";
export const SERVER_VERSION = "0.9.5";

export const TOOLS = [
  {
    name: "headless_open",
    description: "Open or register a document without overwriting an existing HEAD or resetting its revision.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string", description: "Target document identifier, e.g. 'ws:src/main.rs'" },
        content: { type: "string", description: "Initial content used ONLY if the document does not yet exist" },
        language: { type: "string", description: "Syntax mode (rust, js, ts, json, nema, plain)" },
        agent_id: { type: "string", description: "Optional agent identifier" },
        mode: { type: "string", enum: ["create_if_absent", "open_existing", "force_overwrite"] },
      },
      required: ["file_id"],
    },
  },
  {
    name: "ide_open_file",
    description: "Compatibility alias for headless_open (never overwrites existing HEAD unless mode='force_overwrite').",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        content: { type: "string" },
        language: { type: "string" },
        agent_id: { type: "string" },
        mode: { type: "string", enum: ["create_if_absent", "open_existing", "force_overwrite"] },
      },
      required: ["file_id"],
    },
  },
  {
    name: "headless_read",
    description: "Read the current or historical document text, revision, and SHA-256 content_hash.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        revision: { type: "integer", minimum: 1 },
        start_line: { type: "integer", minimum: 1 },
        end_line: { type: "integer", minimum: 1 },
      },
      required: ["file_id"],
    },
  },
  {
    name: "headless_coedit",
    description: "Atomic CAS + 3-way auto-merge collaborative edit. Overlapping conflicts never mutate HEAD and instead save a reviewable proposal.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        author: { type: "string" },
        agent_id: { type: "string" },
        base_revision: { type: "integer", minimum: 1 },
        base_content_hash: { type: "string" },
        fencing_token: { type: "integer", minimum: 1 },
        op_id: { type: "string", minLength: 1 },
        new_text: { type: "string" },
        mode: { type: "string", enum: ["git", "nema"] },
      },
      required: ["file_id", "base_revision", "op_id", "new_text"],
    },
  },
  {
    name: "headless_edit",
    description: "Atomic line-range edit with CAS base_revision, scoped op_id idempotency, and safe 3-way auto-merge.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        agent_id: { type: "string" },
        author: { type: "string" },
        base_revision: { type: "integer", minimum: 1 },
        base_content_hash: { type: "string" },
        fencing_token: { type: "integer", minimum: 1 },
        start_line: { type: "integer", minimum: 1 },
        end_line: { type: "integer", minimum: 1 },
        replacement: { type: "string" },
        op_id: { type: "string", minLength: 1 },
      },
      required: ["file_id", "base_revision", "op_id", "start_line", "end_line", "replacement"],
    },
  },
  {
    name: "ide_edit_lines",
    description: "Atomic compatibility alias for headless_edit (1-based line replacement inside DB CAS transaction).",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        agent_id: { type: "string" },
        author: { type: "string" },
        base_revision: { type: "integer", minimum: 1 },
        base_content_hash: { type: "string" },
        fencing_token: { type: "integer", minimum: 1 },
        start_line: { type: "integer", minimum: 1 },
        end_line: { type: "integer", minimum: 1 },
        replacement: { type: "string" },
        op_id: { type: "string", minLength: 1 },
      },
      required: ["file_id", "base_revision", "op_id", "start_line", "end_line", "replacement"],
    },
  },
  {
    name: "ide_replace_all",
    description: "Atomic CAS find-and-replace across a document with base_revision and scoped op_id protection.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        agent_id: { type: "string" },
        author: { type: "string" },
        base_revision: { type: "integer", minimum: 1 },
        base_content_hash: { type: "string" },
        fencing_token: { type: "integer", minimum: 1 },
        find: { type: "string", minLength: 1 },
        replace: { type: "string" },
        op_id: { type: "string", minLength: 1 },
      },
      required: ["file_id", "base_revision", "op_id", "find", "replace"],
    },
  },
  {
    name: "headless_three_way_merge",
    description: "Pure 3-way merge algorithm preview without mutating any document.",
    inputSchema: {
      type: "object",
      properties: {
        base: { type: "string" },
        ours: { type: "string" },
        theirs: { type: "string" },
      },
      required: ["base", "ours", "theirs"],
    },
  },
  {
    name: "headless_history",
    description: "Query durable document revision history, SHA-256 content hashes, op_id, and agent attributions.",
    inputSchema: {
      type: "object",
      properties: { file_id: { type: "string" } },
      required: ["file_id"],
    },
  },
  {
    name: "headless_list_files",
    description: "List active documents visible to the authenticated tenant and authorized workspaces.",
    inputSchema: { type: "object", properties: {} },
  },
  {
    name: "headless_list_proposals",
    description: "List pending or resolved conflict proposals stored without polluting HEAD.",
    inputSchema: {
      type: "object",
      properties: { file_id: { type: "string" } },
    },
  },
  {
    name: "headless_resolve_proposal",
    description: "Resolve a conflict proposal with reviewed content using atomic CAS against base_revision.",
    inputSchema: {
      type: "object",
      properties: {
        proposal_id: { type: "string" },
        resolved_content: { type: "string" },
        base_revision: { type: "integer", minimum: 1 },
        agent_id: { type: "string" },
        author: { type: "string" },
        op_id: { type: "string", minLength: 1 },
      },
      required: ["proposal_id", "resolved_content", "base_revision", "op_id"],
    },
  },
  {
    name: "headless_acquire_lease",
    description: "Acquire a monotonically increasing fencing-token lease for a file.",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        agent_id: { type: "string" },
        ttl_ms: { type: "integer", minimum: 1000 },
      },
      required: ["file_id", "agent_id"],
    },
  },
  {
    name: "diff_unified",
    description: "Compute Myers Unified Diff between any two texts (accepts old_text/new_text or text_a/text_b).",
    inputSchema: {
      type: "object",
      properties: {
        old_text: { type: "string" },
        new_text: { type: "string" },
        text_a: { type: "string" },
        text_b: { type: "string" },
        context_lines: { type: "integer", default: 3 },
      },
    },
  },
  {
    name: "diff_revisions",
    description: "Compute Unified Diff between two revisions (accepts from_revision/to_revision or rev_a/rev_b).",
    inputSchema: {
      type: "object",
      properties: {
        file_id: { type: "string" },
        from_revision: { type: "integer" },
        to_revision: { type: "integer" },
        rev_a: { type: "integer" },
        rev_b: { type: "integer" },
      },
      required: ["file_id"],
    },
  },
  {
    name: "ide_rust_eval",
    description:
      "Execute Blender-style ide::context / ide::data / ide::ops Rust scripts to inspect and programmatically modify shared documents under workspace ACL & Postgres CAS.",
    inputSchema: {
      type: "object",
      properties: {
        script: { type: "string" },
        agent_id: { type: "string" },
      },
      required: ["script"],
    },
  },
];

function reqActor(args, principal) {
  const actor = String(args?.agent_id || args?.author || "").trim();
  if (!actor) {
    throw new StoreError(400, "MISSING_AGENT_ID", "agent_id or author is required for write operations");
  }
  return `${principal.actorId}:${actor}`;
}

async function callMcpTool(store, principal, name, args = {}) {
  switch (name) {
    case "headless_open":
    case "ide_open_file": {
      const target = resolveFileTarget(args.file_id, principal);
      const openMode = args.mode || "create_if_absent";
      const requiredScope = openMode === "force_overwrite" ? "force_overwrite" : openMode === "open_existing" ? "read" : "write";
      authorizeScope(principal, target.workspaceId, requiredScope, target.filePath);
      const actor = String(args.agent_id || args.author || principal.actorId).trim();
      return await store.openFile({
        ...target,
        actorId: actor,
        content: args.content ?? "",
        language: args.language || "plain",
        mode: openMode,
      });
    }

    case "headless_read": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "read", target.filePath);
      return await store.readFile({
        ...target,
        revision: args.revision,
        startLine: args.start_line,
        endLine: args.end_line,
      });
    }

    case "headless_coedit": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "write", target.filePath);
      const actorId = reqActor(args, principal);
      if (typeof args.new_text !== "string") {
        throw new StoreError(400, "MISSING_NEW_TEXT", "new_text string is required");
      }
      return await store.coedit({
        ...target,
        actorId,
        baseRevision: args.base_revision,
        baseContentHash: args.base_content_hash || null,
        fencingToken: args.fencing_token !== undefined ? args.fencing_token : null,
        opId: args.op_id,
        newText: args.new_text,
        mode: args.mode || null,
      });
    }

    case "headless_edit":
    case "ide_edit_lines": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "write", target.filePath);
      const actorId = reqActor(args, principal);
      return await store.editLines({
        ...target,
        actorId,
        baseRevision: args.base_revision,
        baseContentHash: args.base_content_hash || null,
        fencingToken: args.fencing_token !== undefined ? args.fencing_token : null,
        opId: args.op_id,
        startLine: args.start_line,
        endLine: args.end_line,
        replacement: args.replacement ?? "",
      });
    }

    case "ide_replace_all": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "write", target.filePath);
      const actorId = reqActor(args, principal);
      return await store.replaceAll({
        ...target,
        actorId,
        baseRevision: args.base_revision,
        baseContentHash: args.base_content_hash || null,
        fencingToken: args.fencing_token !== undefined ? args.fencing_token : null,
        opId: args.op_id,
        find: args.find,
        replace: args.replace ?? "",
      });
    }

    case "headless_three_way_merge": {
      authorizeScope(principal, principal.defaultWorkspace || "default", "read");
      return threeWayMerge(args.base || "", args.ours || "", args.theirs || "");
    }

    case "headless_history": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "read", target.filePath);
      return await store.fileHistory(target);
    }

    case "headless_list_files": {
      const firstWs = principal.workspaces.includes("*") ? "default" : principal.workspaces[0] || "default";
      authorizeScope(principal, firstWs.replace(/\*$/, "") || "default", "read");
      return await store.listFiles(principal);
    }

    case "headless_list_proposals": {
      let wsId = null;
      let fId = null;
      if (args.file_id) {
        const t = resolveFileTarget(args.file_id, principal);
        authorizeScope(principal, t.workspaceId, "read", t.filePath);
        wsId = t.workspaceId;
        fId = t.fileId;
      } else {
        const hasReadScope =
          principal.scopes.includes("*") ||
          principal.scopes.includes("admin") ||
          principal.scopes.includes("read");
        if (!hasReadScope) {
          throw new AuthError(
            403,
            "INSUFFICIENT_SCOPE",
            `Required scope 'read' missing for actor '${principal.actorId}'`
          );
        }
      }
      const proposals = await store.listProposals({
        tenantId: principal.tenantId,
        principal,
        workspaceId: wsId,
        fileId: fId,
      });
      return { proposals };
    }

    case "headless_resolve_proposal": {
      const hasWriteScope =
        principal.scopes.includes("*") ||
        principal.scopes.includes("admin") ||
        principal.scopes.includes("write");
      if (!hasWriteScope) {
        throw new AuthError(
          403,
          "INSUFFICIENT_SCOPE",
          `Required scope 'write' missing for actor '${principal.actorId}'`
        );
      }
      const actorId = reqActor(args, principal);
      if (typeof args.resolved_content !== "string") {
        throw new StoreError(400, "MISSING_RESOLVED_CONTENT", "resolved_content string is required");
      }
      return await store.resolveProposal({
        tenantId: principal.tenantId,
        principal,
        actorId,
        proposalId: args.proposal_id,
        resolvedContent: args.resolved_content,
        baseRevision: args.base_revision,
        opId: args.op_id,
      });
    }

    case "headless_acquire_lease": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "write", target.filePath);
      const actorId = reqActor(args, principal);
      return await store.acquireLease({
        ...target,
        agentId: actorId,
        ttlMs: args.ttl_ms || 60000,
      });
    }

    case "diff_unified": {
      const oldText = args.old_text !== undefined ? args.old_text : args.text_a || "";
      const newText = args.new_text !== undefined ? args.new_text : args.text_b || "";
      const diff = formatUnifiedDiff(
        oldText,
        newText,
        args.label_a || "original",
        args.label_b || "modified",
        args.context_lines || 3
      );
      return diff || "(Identical texts: zero differences)";
    }

    case "diff_revisions": {
      const target = resolveFileTarget(args.file_id, principal);
      authorizeScope(principal, target.workspaceId, "read", target.filePath);
      const revA = args.from_revision !== undefined ? args.from_revision : args.rev_a;
      const revB = args.to_revision !== undefined ? args.to_revision : args.rev_b;
      return await store.diffRevisions({ ...target, revA, revB });
    }

    case "ide_rust_eval": {
      return await store.evalRustScript(principal, args.script || "", args.agent_id || args.author || null);
    }

    default:
      return null;
  }
}

export async function handleMcpRequest(req, res, deps = {}) {
  const store = deps.store || getStore();
  const env = deps.env || process.env;

  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, Mcp-Session-Id, Idempotency-Key");
  res.setHeader("Vary", "Authorization");

  if (req.method === "OPTIONS") return res.status(200).end();
  if (req.method === "DELETE") return res.status(204).end();

  if (req.method === "GET") {
    const accept = req.headers?.["accept"] || "";
    if (accept.includes("text/html") && !accept.includes("application/json")) {
      res.setHeader("Location", "/");
      return res.status(302).end();
    }
    return res.status(200).json({
      server: "IDE1.0 MCP Server (Vercel Durable Postgres)",
      version: SERVER_VERSION,
      status: "operational",
      protocolVersion: MCP_PROTOCOL_VERSION,
      auth: "Bearer token or short-lived HMAC JWT required for tools/call and resources/*",
      endpoints: { mcp: "/api/v1/mcp", health: "/api/v1/health", events: "/api/v1/events" },
      tools: TOOLS.map((t) => ({ name: t.name, description: t.description })),
    });
  }

  if (req.method !== "POST") {
    return res.status(405).json({ error: "Method not allowed" });
  }

  const reqSessionId = req.headers?.["mcp-session-id"];
  if (reqSessionId) {
    res.setHeader("Mcp-Session-Id", String(reqSessionId));
  }

  try {
    enforcePayloadSize(req);
  } catch (err) {
    return res.status(err.statusCode || 413).json({
      jsonrpc: "2.0",
      error: { code: -32600, message: err.message },
      id: null,
    });
  }

  let body = req.body;
  if (typeof body === "string") {
    try {
      body = JSON.parse(body);
    } catch {
      return res.status(400).json({ jsonrpc: "2.0", error: { code: -32700, message: "Parse error" }, id: null });
    }
  }

  const { id, method, params } = body || {};
  if (!method) {
    return res.status(400).json({
      jsonrpc: "2.0",
      error: { code: -32600, message: "Invalid Request: missing method" },
      id: id ?? null,
    });
  }

  if (method === "notifications/initialized" || method === "notifications/cancelled" || method === "cancelled") {
    return res.status(202).end();
  }

  if (method === "initialize") {
    const sessionId = req.headers?.["mcp-session-id"] || crypto.randomUUID();
    res.setHeader("Mcp-Session-Id", sessionId);
    return res.status(200).json({
      jsonrpc: "2.0",
      id: id ?? null,
      result: {
        protocolVersion: MCP_PROTOCOL_VERSION,
        capabilities: {
          tools: { listChanged: false },
          resources: { subscribe: false, listChanged: false },
          prompts: { listChanged: false },
        },
        serverInfo: { name: "ide10-mcp", version: SERVER_VERSION },
      },
    });
  }

  if (method === "ping") {
    return res.status(200).json({ jsonrpc: "2.0", id: id ?? null, result: {} });
  }

  if (method === "tools/list") {
    return res.status(200).json({ jsonrpc: "2.0", id: id ?? null, result: { tools: TOOLS } });
  }

  if (method === "prompts/list") {
    return res.status(200).json({
      jsonrpc: "2.0",
      id: id ?? null,
      result: {
        prompts: [
          {
            name: "review_code",
            description: "Review pending conflict proposals or open files",
            arguments: [{ name: "file_id", description: "Target document ID", required: true }],
          },
        ],
      },
    });
  }

  // Strictly authenticate & rate-limit all state/data access methods (P0-03)
  let principal;
  try {
    principal = authenticateRequest(req, env);
    await store.enforceRateLimit(principal.tenantId);
  } catch (err) {
    const status = err.statusCode || 401;
    return res.status(status).json({
      jsonrpc: "2.0",
      id: id ?? null,
      error: { code: -32001, message: `${err.code || "AUTH_ERROR"}: ${err.message}` },
    });
  }

  try {
    if (method === "resources/list") {
      const files = await store.listFiles(principal);
      const resources = [
        { uri: "ide10://files", name: "Workspace Open Documents", mimeType: "application/json" },
        ...files.map((f) => ({ uri: `ide10://file/${f.file_id}`, name: f.file_id, mimeType: "text/plain" })),
      ];
      return res.status(200).json({ jsonrpc: "2.0", id: id ?? null, result: { resources } });
    }

    if (method === "resources/read") {
      const uri = params?.uri || "";
      if (uri === "ide10://files") {
        const files = await store.listFiles(principal);
        return res.status(200).json({
          jsonrpc: "2.0",
          id: id ?? null,
          result: { contents: [{ uri, mimeType: "application/json", text: JSON.stringify(files, null, 2) }] },
        });
      }
      const rawId = uri.startsWith("ide10://file/")
        ? uri.slice("ide10://file/".length)
        : uri.replace(/^file:\/\/\/workspace\//, "");
      const target = resolveFileTarget(rawId, principal);
      authorizeScope(principal, target.workspaceId, "read", target.filePath);
      const doc = await store.readFile(target);
      return res.status(200).json({
        jsonrpc: "2.0",
        id: id ?? null,
        result: { contents: [{ uri, mimeType: "text/plain", text: doc.content }] },
      });
    }

    if (method === "tools/call") {
      const { name, arguments: args } = params || {};
      if (!name) {
        return res.status(200).json({
          jsonrpc: "2.0",
          id: id ?? null,
          error: { code: -32602, message: "Missing tool name" },
        });
      }
      const outcome = await callMcpTool(store, principal, name, args || {});
      if (outcome === null) {
        return res.status(200).json({
          jsonrpc: "2.0",
          id: id ?? null,
          error: { code: -32601, message: `Tool '${name}' not found` },
        });
      }
      const text = typeof outcome === "string" ? outcome : JSON.stringify(outcome, null, 2);
      const isErr = Boolean(outcome && typeof outcome === "object" && outcome.ok === false);
      return res.status(200).json({
        jsonrpc: "2.0",
        id: id ?? null,
        result: {
          content: [{ type: "text", text }],
          structured_data: outcome,
          isError: isErr,
        },
      });
    }

    return res.status(200).json({
      jsonrpc: "2.0",
      id: id ?? null,
      error: { code: -32601, message: `Method '${method}' not found` },
    });
  } catch (err) {
    if (err instanceof AuthError) {
      return res.status(err.statusCode).json({
        jsonrpc: "2.0",
        id: id ?? null,
        error: { code: -32001, message: `${err.code}: ${err.message}` },
      });
    }
    if (err instanceof StoreError) {
      if (err.statusCode === 503 || err.statusCode === 429) {
        return res.status(err.statusCode).json({
          jsonrpc: "2.0",
          id: id ?? null,
          error: { code: -32000, message: `${err.code}: ${err.message}` },
        });
      }
      return res.status(200).json({
        jsonrpc: "2.0",
        id: id ?? null,
        result: {
          content: [{ type: "text", text: `Error [${err.code}]: ${err.message}` }],
          structured_data: { ok: false, code: err.code, message: err.message },
          isError: true,
        },
      });
    }
    return res.status(500).json({
      jsonrpc: "2.0",
      id: id ?? null,
      error: { code: -32603, message: err.message || "Internal server error" },
    });
  }
}

export default async function handler(req, res) {
  return handleMcpRequest(req, res);
}
