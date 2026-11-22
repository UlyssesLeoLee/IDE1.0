// Blender-style Rust DSL engine (`ide::context::*`, `ide::data::*`, `ide::ops::*`)
// Backed by PgDurableStore CAS transactions & strict workspace/file ACL (SEC-001, SEC-002, P0-05).

import crypto from "crypto";
import { AuthError, authorizeScope, resolveFileTarget } from "./_auth.js";
import { splitLines } from "./_merge.js";

function unescapeStringChar(ch) {
  switch (ch) {
    case "n":
      return "\n";
    case "r":
      return "\r";
    case "t":
      return "\t";
    case "\\":
      return "\\";
    case '"':
      return '"';
    case "'":
      return "'";
    default:
      return ch;
  }
}

export function splitStatements(script) {
  const src = String(script || "");
  const stmts = [];
  let cur = "";
  let inQuote = false;
  let quoteChar = "";
  let escaped = false;

  for (let i = 0; i < src.length; i += 1) {
    const ch = src[i];
    if (escaped) {
      cur += ch;
      escaped = false;
      continue;
    }
    if (inQuote) {
      if (ch === "\\") {
        cur += ch;
        escaped = true;
      } else {
        if (ch === quoteChar) inQuote = false;
        cur += ch;
      }
      continue;
    }
    if (ch === "/" && src[i + 1] === "/" && cur.trim() === "") {
      while (i < src.length && src[i] !== "\n") i += 1;
      continue;
    }
    if (ch === '"' || ch === "'") {
      inQuote = true;
      quoteChar = ch;
      cur += ch;
    } else if (ch === ";" || ch === "\n" || ch === "\r") {
      if (cur.trim()) stmts.push(cur.trim());
      cur = "";
    } else {
      cur += ch;
    }
  }
  if (cur.trim()) stmts.push(cur.trim());
  return stmts;
}

export function parseArgs(argsStr) {
  const src = String(argsStr || "");
  const out = [];
  let cur = "";
  let inQuote = false;
  let quoteChar = "";
  let escaped = false;

  for (let i = 0; i < src.length; i += 1) {
    const ch = src[i];
    if (escaped) {
      cur += unescapeStringChar(ch);
      escaped = false;
      continue;
    }
    if (inQuote && ch === "\\") {
      escaped = true;
      continue;
    }
    if (!inQuote && (ch === '"' || ch === "'")) {
      inQuote = true;
      quoteChar = ch;
      continue;
    }
    if (inQuote && ch === quoteChar) {
      inQuote = false;
      continue;
    }
    if (ch === "," && !inQuote) {
      out.push(cur.trim());
      cur = "";
      continue;
    }
    cur += ch;
  }
  if (cur.trim() || (out.length > 0 && src.trimEnd().endsWith(","))) {
    out.push(cur.trim());
  }
  return out;
}

function stripCall(stmt, fnName) {
  const trimmed = stmt.trim();
  if (!trimmed.startsWith(fnName)) return null;
  const rest = trimmed.slice(fnName.length).trim();
  if (rest.startsWith("(") && rest.endsWith(")")) {
    return rest.slice(1, -1);
  }
  return null;
}

export function helpText() {
  return [
    "=== IDE1.0 Blender-Style Rust Scripting API ===",
    'ide::context::status()                         - Workspace & tenant status',
    'ide::context::help()                           - Print API reference',
    'ide::data::files()                             - List authorized documents',
    'ide::data::read("ws:file")                     - Read full document text',
    'ide::data::lines("ws:file", s, e)              - Read 1-based line slice',
    'ide::data::history("ws:file")                  - Inspect document revision history',
    'ide::ops::open("ws:file", "lang", "txt")       - Create doc if absent (no overwrite)',
    'ide::ops::edit("ws:file", s, e, "txt" [,rev])  - Replace 1-based line range via CAS',
    'ide::ops::insert("ws:file", line, "txt")       - Insert text before 1-based line via CAS',
    'ide::ops::append("ws:file", "txt")             - Append text at EOF via CAS',
    'ide::ops::delete_lines("ws:file", s, e)        - Delete 1-based inclusive lines via CAS',
    'ide::ops::coedit("ws:file" [,rev], "txt")      - Full 3-way merge / conflict proposal',
    'ide::ops::replace_all("ws:file", "a", "b")     - Atomic find & replace via CAS',
    'ide::ops::diff("ws:file", rev_a, rev_b)        - Unified diff between revisions',
    'ide::ops::search("ws:file", "needle")          - Search matching lines',
  ].join("\n");
}

function requireAnyReadScope(principal) {
  if (!principal) {
    throw new AuthError(401, "UNAUTHORIZED", "Authentication required");
  }
  const hasRead =
    principal.scopes.includes("*") ||
    principal.scopes.includes("admin") ||
    principal.scopes.includes("read");
  if (!hasRead) {
    throw new AuthError(
      403,
      "INSUFFICIENT_SCOPE",
      `Required scope 'read' missing for actor '${principal.actorId}'`
    );
  }
}

async function execSingleStatement(store, principal, actor, stmt, opId) {
  const clean = stmt.trim().replace(/;+$/, "");
  const lower = clean.toLowerCase();

  if (
    lower === "ide::context::help()" ||
    lower === "ide::ops::help()" ||
    lower === "help()"
  ) {
    return { ok: true, output: helpText() };
  }

  if (lower === "ide::context::status()") {
    requireAnyReadScope(principal);
    const files = await store.listFiles(principal);
    return {
      ok: true,
      output: `ide::context: active_documents=${files.length}, tenant=${principal.tenantId}, backend="IDE1.0 Durable MCP (Postgres)"`,
    };
  }

  if (lower === "ide::data::files()" || lower === "ide::data::docs()") {
    requireAnyReadScope(principal);
    const files = await store.listFiles(principal);
    return {
      ok: true,
      output: JSON.stringify(files, null, 2),
    };
  }

  let argsStr = stripCall(clean, "ide::data::read");
  if (argsStr !== null) {
    const [fileArg] = parseArgs(argsStr);
    const target = resolveFileTarget(fileArg, principal);
    authorizeScope(principal, target.workspaceId, "read", target.filePath);
    const doc = await store.readFile(target);
    return { ok: true, revision: doc.current_revision, output: doc.content };
  }

  argsStr = stripCall(clean, "ide::data::lines");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 3) {
      return { ok: false, output: "ide::data::lines requires 3 args: (file_id, start_line, end_line)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "read", target.filePath);
    const s = Number.parseInt(parts[1], 10);
    const e = Number.parseInt(parts[2], 10);
    if (!Number.isFinite(s) || !Number.isFinite(e) || s < 1 || e < s) {
      return { ok: false, output: "ide::data::lines requires 1 <= start_line <= end_line" };
    }
    const doc = await store.readFile({ ...target, startLine: s, endLine: e });
    return { ok: true, revision: doc.current_revision, output: doc.content };
  }

  argsStr = stripCall(clean, "ide::data::history");
  if (argsStr !== null) {
    const [fileArg] = parseArgs(argsStr);
    const target = resolveFileTarget(fileArg, principal);
    authorizeScope(principal, target.workspaceId, "read", target.filePath);
    const hist = await store.fileHistory(target);
    return { ok: true, output: JSON.stringify(hist, null, 2) };
  }

  argsStr = stripCall(clean, "ide::ops::diff");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 3) {
      return { ok: false, output: "ide::ops::diff requires 3 args: (file_id, rev_a, rev_b)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "read", target.filePath);
    const revA = Number.parseInt(parts[1], 10);
    const revB = Number.parseInt(parts[2], 10);
    if (!Number.isFinite(revA) || !Number.isFinite(revB)) {
      return { ok: false, output: "ide::ops::diff requires numeric revision numbers" };
    }
    const diff = await store.diffRevisions({ ...target, revA, revB });
    return { ok: true, output: diff };
  }

  argsStr = stripCall(clean, "ide::ops::search");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 2) {
      return { ok: false, output: "ide::ops::search requires 2 args: (file_id, needle)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "read", target.filePath);
    const doc = await store.readFile(target);
    const needle = parts[1];
    const lines = splitLines(doc.content);
    const hits = [];
    for (let i = 0; i < lines.length; i += 1) {
      if (lines[i].includes(needle)) {
        hits.push(`  L${i + 1}: ${lines[i].trimEnd()}`);
      }
    }
    return {
      ok: true,
      revision: doc.current_revision,
      output: `Found ${hits.length} hit(s) for '${needle}':\n${hits.join("\n")}`,
    };
  }

  argsStr = stripCall(clean, "ide::ops::open");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 3) {
      return { ok: false, output: "ide::ops::open requires 3 args: (file_id, language, content)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    const res = await store.openFile({
      ...target,
      actorId: actor,
      content: parts[2],
      language: parts[1] || "plain",
      mode: "create_if_absent",
    });
    return {
      ok: res.ok !== false,
      status: res.status,
      revision: res.current_revision,
      output: `Opened '${res.file_id}' (rev ${res.current_revision}, status=${res.status}, hash=${res.content_hash})`,
    };
  }

  argsStr = stripCall(clean, "ide::ops::edit");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 4) {
      return {
        ok: false,
        output: "ide::ops::edit requires 4 args: (file_id, start_line, end_line, replacement)",
      };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    const startLine = Number.parseInt(parts[1], 10);
    const endLine = Number.parseInt(parts[2], 10);
    if (!Number.isFinite(startLine) || !Number.isFinite(endLine) || startLine < 1 || endLine < startLine) {
      return { ok: false, output: "ide::ops::edit requires 1 <= start_line <= end_line" };
    }
    const cur = await store.readFile(target);
    const baseRevision =
      parts.length >= 5 && Number.isFinite(Number.parseInt(parts[4], 10))
        ? Number.parseInt(parts[4], 10)
        : cur.current_revision;
    const res = await store.editLines({
      ...target,
      actorId: actor,
      baseRevision,
      baseContentHash: parts.length >= 5 ? null : cur.content_hash,
      opId,
      startLine,
      endLine,
      replacement: parts[3],
    });
    return formatMutationStepResult("Edited", res);
  }

  argsStr = stripCall(clean, "ide::ops::insert");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 3) {
      return { ok: false, output: "ide::ops::insert requires 3 args: (file_id, line, text)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    const lineNum = Number.parseInt(parts[1], 10);
    if (!Number.isFinite(lineNum)) {
      return { ok: false, output: `ide::ops::insert: invalid line '${parts[1]}'` };
    }
    const targetLine = lineNum < 1 ? 1 : lineNum;
    const cur = await store.readFile(target);
    let insertion = parts[2];
    if (
      targetLine > cur.total_lines &&
      cur.content.length > 0 &&
      !cur.content.endsWith("\n") &&
      !insertion.startsWith("\n")
    ) {
      insertion = `\n${insertion}`;
    }
    const res = await store.editLines({
      ...target,
      actorId: actor,
      baseRevision: cur.current_revision,
      baseContentHash: cur.content_hash,
      opId,
      startLine: targetLine,
      endLine: 0,
      replacement: insertion,
    });
    return formatMutationStepResult(`Inserted @L${targetLine}`, res);
  }

  argsStr = stripCall(clean, "ide::ops::append");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 2) {
      return { ok: false, output: "ide::ops::append requires 2 args: (file_id, text)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    const cur = await store.readFile(target);
    let nextText = cur.content || "";
    if (nextText.length > 0 && !nextText.endsWith("\n")) nextText += "\n";
    nextText += parts[1];
    if (nextText.length > 0 && !nextText.endsWith("\n")) nextText += "\n";
    const res = await store.coedit({
      ...target,
      actorId: actor,
      baseRevision: cur.current_revision,
      baseContentHash: cur.content_hash,
      opId,
      newText: nextText,
      mode: "git",
    });
    return formatMutationStepResult("Appended", res);
  }

  argsStr = stripCall(clean, "ide::ops::delete_lines");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 3) {
      return {
        ok: false,
        output: "ide::ops::delete_lines requires 3 args: (file_id, start_line, end_line)",
      };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    const startLine = Number.parseInt(parts[1], 10);
    const endLine = Number.parseInt(parts[2], 10);
    if (!Number.isFinite(startLine) || !Number.isFinite(endLine) || startLine < 1 || endLine < startLine) {
      return { ok: false, output: "ide::ops::delete_lines requires 1 <= start_line <= end_line" };
    }
    const cur = await store.readFile(target);
    const res = await store.editLines({
      ...target,
      actorId: actor,
      baseRevision: cur.current_revision,
      baseContentHash: cur.content_hash,
      opId,
      startLine,
      endLine,
      replacement: "",
    });
    return formatMutationStepResult(`Deleted L${startLine}..L${endLine}`, res);
  }

  argsStr = stripCall(clean, "ide::ops::replace_all");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 3) {
      return { ok: false, output: "ide::ops::replace_all requires 3 args: (file_id, from, to)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    const cur = await store.readFile(target);
    const res = await store.replaceAll({
      ...target,
      actorId: actor,
      baseRevision: cur.current_revision,
      baseContentHash: cur.content_hash,
      opId,
      find: parts[1],
      replace: parts[2],
    });
    return formatMutationStepResult(`Replaced (${res.replacements ?? 0})`, res);
  }

  argsStr = stripCall(clean, "ide::ops::coedit");
  if (argsStr !== null) {
    const parts = parseArgs(argsStr);
    if (parts.length < 2) {
      return { ok: false, output: "ide::ops::coedit requires (file_id, [base_rev], new_text)" };
    }
    const target = resolveFileTarget(parts[0], principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);
    let baseRevision;
    let baseHash = null;
    let newText;
    if (parts.length >= 3) {
      baseRevision = Number.parseInt(parts[1], 10);
      if (!Number.isFinite(baseRevision) || baseRevision < 1) {
        return { ok: false, output: `ide::ops::coedit: invalid base_revision '${parts[1]}'` };
      }
      newText = parts[2];
    } else {
      const cur = await store.readFile(target);
      baseRevision = cur.current_revision;
      baseHash = cur.content_hash;
      newText = parts[1];
    }
    const res = await store.coedit({
      ...target,
      actorId: actor,
      baseRevision,
      baseContentHash: baseHash,
      opId,
      newText,
      mode: "git",
    });
    return formatMutationStepResult("Coedited", res);
  }

  return {
    ok: false,
    output: `Unrecognized statement: '${clean}'. Type 'ide::context::help()' to view available functions.`,
  };
}

function formatMutationStepResult(verb, res) {
  if (res && res.ok === false && res.status === "CONFLICT") {
    return {
      ok: false,
      status: res.status,
      revision: res.head_revision,
      proposal_id: res.proposal_id,
      output: `CONFLICT (HEAD unchanged at rev ${res.head_revision}): proposal_id=${res.proposal_id}`,
    };
  }
  return {
    ok: res?.ok !== false,
    status: res?.status || "COMMITTED",
    revision: res?.revision,
    output: `${verb} -> rev ${res?.revision ?? "?"} (status=${res?.status || "?"}, hash=${res?.content_hash || "?"})`,
  };
}

export async function evalRustScriptWithStore(store, principal, script, agentId = null) {
  if (!principal) {
    throw new AuthError(401, "UNAUTHORIZED", "Authentication required");
  }
  const suffix = String(agentId || "").trim();
  const actor = suffix ? `${principal.actorId}:${suffix}` : principal.actorId;
  const stmts = splitStatements(script);
  const runNonce = `${Date.now()}_${crypto.randomBytes(4).toString("hex")}`;
  const steps = [];
  let allOk = true;

  for (let i = 0; i < stmts.length; i += 1) {
    const expr = stmts[i];
    const opId = `rusteval_${runNonce}_${i + 1}`;
    try {
      const stepRes = await execSingleStatement(store, principal, actor, expr, opId);
      if (!stepRes.ok) allOk = false;
      steps.push({ expr, ...stepRes });
    } catch (err) {
      // Fail-closed: always surface 401/403 ACL denials directly to the transport layer.
      if (err?.statusCode === 401 || err?.statusCode === 403) {
        throw err;
      }
      allOk = false;
      steps.push({
        expr,
        ok: false,
        output: err?.message || String(err),
      });
    }
  }

  const passed = steps.filter((s) => s.ok).length;
  const summary =
    steps.length === 0
      ? "Empty script (no statements executed)"
      : `Executed ${steps.length} statement(s): ${passed} passed, ${steps.length - passed} failed`;

  return {
    ok: allOk,
    summary,
    steps,
  };
}
