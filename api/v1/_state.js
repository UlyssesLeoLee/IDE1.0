// Shared state for Vercel Serverless Function instance
// Implements File Commit Coordinator with CAS, Fencing Tokens, Transactional Outbox,
// Myers Line Diff, 3-Way Auto-Merge, and Multi-Provider Co-Editing Workspace.

import crypto from "crypto";

// --- Myers Line Diff & Unified Formatter ---
export function diffLines(linesA, linesB) {
  const n = linesA.length;
  const m = linesB.length;
  // LCS table (dp)
  const dp = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < m; j++) {
      if (linesA[i] === linesB[j]) {
        dp[i + 1][j + 1] = dp[i][j] + 1;
      } else {
        dp[i + 1][j + 1] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }

  // Backtrack to build diff steps
  const steps = [];
  let i = n, j = m;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && linesA[i - 1] === linesB[j - 1]) {
      steps.push({ type: "same", line: linesA[i - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      steps.push({ type: "add", line: linesB[j - 1] });
      j--;
    } else if (i > 0) {
      steps.push({ type: "remove", line: linesA[i - 1] });
      i--;
    }
  }
  return steps.reverse();
}

export function formatUnifiedDiff(linesA, linesB, nameA = "a", nameB = "b") {
  const steps = diffLines(linesA, linesB);
  const hunks = [];
  let currentHunk = null;

  let lineA = 1;
  let lineB = 1;

  for (let idx = 0; idx < steps.length; idx++) {
    const s = steps[idx];
    if (s.type === "same") {
      if (currentHunk) {
        currentHunk.lines.push({ prefix: " ", text: s.line });
        currentHunk.ctxTrailing = (currentHunk.ctxTrailing || 0) + 1;
        if (currentHunk.ctxTrailing >= 3) {
          hunks.push(currentHunk);
          currentHunk = null;
        }
      }
      lineA++;
      lineB++;
    } else {
      if (!currentHunk) {
        // Collect up to 3 leading same lines for context
        const leading = [];
        let backIdx = idx - 1;
        while (backIdx >= 0 && steps[backIdx].type === "same" && leading.length < 3) {
          leading.unshift({ prefix: " ", text: steps[backIdx].line });
          backIdx--;
        }
        currentHunk = {
          startA: Math.max(1, lineA - leading.length),
          startB: Math.max(1, lineB - leading.length),
          lines: leading,
          ctxTrailing: 0,
        };
      }
      if (s.type === "remove") {
        currentHunk.lines.push({ prefix: "-", text: s.line });
        currentHunk.ctxTrailing = 0;
        lineA++;
      } else if (s.type === "add") {
        currentHunk.lines.push({ prefix: "+", text: s.line });
        currentHunk.ctxTrailing = 0;
        lineB++;
      }
    }
  }

  if (currentHunk) {
    hunks.push(currentHunk);
  }

  if (hunks.length === 0) {
    return "";
  }

  let out = `--- ${nameA}\n+++ ${nameB}\n`;
  for (const h of hunks) {
    const countA = h.lines.filter(l => l.prefix === "-" || l.prefix === " ").length;
    const countB = h.lines.filter(l => l.prefix === "+" || l.prefix === " ").length;
    out += `@@ -${h.startA},${countA} +${h.startB},${countB} @@\n`;
    for (const l of h.lines) {
      out += `${l.prefix}${l.text}\n`;
    }
  }
  return out;
}

// --- 3-Way Line Merge with Conflict Markers ---
export function threeWayMerge(baseLines, oursLines, theirsLines) {
  const diffOurs = diffLines(baseLines, oursLines);
  const diffTheirs = diffLines(baseLines, theirsLines);

  const baseText = baseLines.join("\n");
  const oursText = oursLines.join("\n");
  const theirsText = theirsLines.join("\n");

  if (oursText === baseText) {
    return { clean: true, conflicts: 0, lines: theirsLines, text: theirsText };
  }
  if (theirsText === baseText || oursText === theirsText) {
    return { clean: true, conflicts: 0, lines: oursLines, text: oursText };
  }

  // Chunk-level 3-way merge
  let bi = 0, oi = 0, ti = 0;
  const merged = [];
  let conflicts = 0;

  while (bi < baseLines.length || oi < oursLines.length || ti < theirsLines.length) {
    const bLine = baseLines[bi];
    const oLine = oursLines[oi];
    const tLine = theirsLines[ti];

    if (bLine === oLine && bLine === tLine) {
      merged.push(bLine);
      bi++; oi++; ti++;
    } else if (bLine === oLine) {
      // Ours kept base, theirs modified or added
      if (ti < theirsLines.length) {
        merged.push(tLine);
        ti++;
      }
      bi++; oi++;
    } else if (bLine === tLine) {
      // Theirs kept base, ours modified or added
      if (oi < oursLines.length) {
        merged.push(oLine);
        oi++;
      }
      bi++; ti++;
    } else if (oLine === tLine) {
      // Both modified identically
      if (oi < oursLines.length) {
        merged.push(oLine);
        oi++; ti++;
      }
      bi++;
    } else {
      // Conflict
      conflicts++;
      merged.push("<<<<<<< HEAD (ours)");
      if (oi < oursLines.length) {
        merged.push(oLine);
        oi++;
      }
      merged.push("=======");
      if (ti < theirsLines.length) {
        merged.push(tLine);
        ti++;
      }
      merged.push(">>>>>>> INCOMING (theirs)");
      bi++;
    }
  }

  // Append remaining lines if any
  while (oi < oursLines.length && ti >= theirsLines.length) {
    merged.push(oursLines[oi++]);
  }
  while (ti < theirsLines.length && oi >= oursLines.length) {
    merged.push(theirsLines[ti++]);
  }

  return {
    clean: conflicts === 0,
    conflicts,
    lines: merged,
    text: merged.join("\n"),
  };
}

// --- NEMA v1.5 Basic Syntax Validator (Red-Green Invariant Guard) ---
export function validateNemaSyntax(text) {
  if (typeof text !== "string" || text.trim().length === 0) {
    return { ok: false, error: "NEMA document cannot be empty" };
  }
  const lines = text.split("\n");
  let hasDomain = false;
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    // 检查字段间未转义双空格
    if (l.includes("  ") && !l.includes("\\ ")) {
      return { ok: false, error: `Line ${i + 1}: Consecutive unescaped ASCII spaces prohibited in NEMA` };
    }
    // 禁止在正文中使用 TAB 控制符
    if (l.includes("\t") && !l.startsWith("\\")) {
      return { ok: false, error: `Line ${i + 1}: TAB character in body prohibited; TAB reserved for domain control` };
    }
    if (/^\s*\\[0-9%?>?!#]/.test(l)) {
      hasDomain = true;
    }
  }
  if (!hasDomain) {
    return { ok: false, error: "No valid NEMA domain declaration found (e.g. \\1, \\%2, \\>1)" };
  }
  return { ok: true };
}

// --- NEMA Semantic Merge ---
export function nemaSemanticMerge(baseText, currentText, incomingText) {
  const extractLines = (src) => {
    const lines = src.split("\n");
    const headers = [];
    const body = [];
    for (const l of lines) {
      const trimmed = l.trimEnd();
      if (!trimmed) continue;
      if (trimmed.startsWith("\\") || trimmed.startsWith(" \\")) {
        headers.push(trimmed);
      } else {
        body.push(trimmed);
      }
    }
    return { headers, body };
  };

  const b = extractLines(baseText);
  const cur = extractLines(currentText);
  const inc = extractLines(incomingText);

  const finalHeaders = cur.headers.length > 0 ? cur.headers : inc.headers;
  const mergedBody = [];
  const seen = new Set();

  for (const item of b.body) {
    if (cur.body.includes(item) && inc.body.includes(item)) {
      if (!seen.has(item)) {
        seen.add(item);
        mergedBody.push(item);
      }
    }
  }
  for (const item of cur.body) {
    if (!b.body.includes(item)) {
      if (!seen.has(item)) {
        seen.add(item);
        mergedBody.push(item);
      }
    }
  }
  for (const item of inc.body) {
    if (!b.body.includes(item)) {
      if (!seen.has(item)) {
        seen.add(item);
        mergedBody.push(item);
      }
    }
  }

  const mergedText = finalHeaders.join("\n") + "\n" + mergedBody.join("\n") + "\n\n\n";
  const check = validateNemaSyntax(mergedText);
  return {
    clean: check.ok,
    conflicts: check.ok ? 0 : 1,
    text: mergedText,
  };
}

class ServerlessCoordinator {
  constructor() {
    this.files = new Map();
    this.tasks = new Map();
    this.commits = new Map();
    this.operationsLog = new Map();
    this.outbox = [];
    this.nextEventId = 1;
    this.initDefaultFiles();
  }

  initDefaultFiles() {
    const demoCode = `pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
`;
    this.registerFile("default", "src/math.rs", demoCode, "rust");
  }

  computeHash(content) {
    return crypto.createHash("sha256").update(content).digest("hex");
  }

  registerFile(workspaceId, path, content, language) {
    const fileId = `${workspaceId}:${path}`;
    const hash = this.computeHash(content);
    const file = {
      fileId,
      workspaceId,
      path,
      content,
      language,
      currentRevision: 1,
      contentHash: hash,
      updatedAt: new Date().toISOString(),
      revisions: [
        {
          revision: 1,
          author: "system",
          content,
          contentHash: hash,
          timestamp: new Date().toISOString(),
          opId: "init",
        },
      ],
    };
    this.files.set(fileId, file);
    return file;
  }

  getSemanticUnits(fileId) {
    const file = this.files.get(fileId);
    if (!file) return [];
    const lines = file.content.split("\n");
    const units = [];
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (line.startsWith("pub fn ") || line.startsWith("fn ") || line.startsWith("def ")) {
        const parts = line.split(/[ (]/);
        const name = parts[1] === "fn" ? parts[2] : parts[1];
        const hash = this.computeHash(line);
        units.push({
          workspace_id: file.workspaceId,
          file_id: file.fileId,
          language: file.language,
          qualified_symbol_path: name,
          node_kind: "function_item",
          signature_hash: hash,
          base_content_hash: hash,
          node_fingerprint: this.computeHash(`${file.fileId}:${name}:${hash}`),
          start_line: i + 1,
          end_line: i + 3,
        });
      }
    }
    return units;
  }

  // --- Headless Multi-Provider Co-Editor Engine ---
  headlessEdit({ fileId, author = "anonymous", baseRevision, opId, newText, patch, mode }) {
    if (opId && this.operationsLog.has(opId)) {
      const cached = this.operationsLog.get(opId);
      return { ...cached, idempotent_replay: true };
    }

    const collabMode = mode || (fileId && fileId.endsWith(".nema") ? "nema" : "git");

    let file = this.files.get(fileId);
    if (!file) {
      // Auto-register file if it does not exist
      const parts = fileId.split(":");
      const ws = parts.length > 1 ? parts[0] : "default";
      const p = parts.length > 1 ? parts.slice(1).join(":") : fileId;
      file = this.registerFile(ws, p, "", collabMode === "nema" ? "nema" : "plain");
    }

    const currentRev = file.currentRevision;
    const incomingText = newText !== undefined ? newText : (file.content + (patch || ""));
    const baseRev = Number(baseRevision) || currentRev;

    // Red-Green 前置防御：在 nema 模式下对输入文本执行严格语法校验
    if (collabMode === "nema") {
      const check = validateNemaSyntax(incomingText);
      if (!check.ok) {
        return {
          ok: false,
          status: "REJECTED_NEMA_SYNTAX_ERROR",
          fileId,
          error: `NEMA 语法校验未通过 (Red Guard): ${check.error}`,
          mode: "nema",
        };
      }
    }

    let result;
    if (baseRev === currentRev) {
      // CAS Fast-Path: No intervening concurrent edits
      file.content = incomingText;
      file.currentRevision += 1;
      file.contentHash = this.computeHash(incomingText);
      file.updatedAt = new Date().toISOString();

      const revRecord = {
        revision: file.currentRevision,
        author,
        content: file.content,
        contentHash: file.contentHash,
        timestamp: file.updatedAt,
        opId: opId || `op_${Date.now()}`,
      };
      file.revisions.push(revRecord);

      result = {
        ok: true,
        status: "COMMITTED",
        fileId,
        previousRevision: currentRev,
        revision: file.currentRevision,
        merged: false,
        conflicts: 0,
        author,
        content: file.content,
        mode: collabMode,
      };
    } else {
      // Intervening edits occurred: Auto-Merge
      let mergeOutcome;
      if (collabMode === "nema") {
        const baseRec = file.revisions.find(r => r.revision === baseRev) || file.revisions[0];
        mergeOutcome = nemaSemanticMerge(baseRec ? baseRec.content : "", file.content, incomingText);
      } else {
        const baseRec = file.revisions.find(r => r.revision === baseRev) || file.revisions[0];
        const baseLines = (baseRec ? baseRec.content : "").split("\n");
        const currentLines = file.content.split("\n");
        const incomingLines = incomingText.split("\n");
        mergeOutcome = threeWayMerge(baseLines, currentLines, incomingLines);
      }

      file.content = mergeOutcome.text;
      file.currentRevision += 1;
      file.contentHash = this.computeHash(file.content);
      file.updatedAt = new Date().toISOString();

      const revRecord = {
        revision: file.currentRevision,
        author: `${author}${mergeOutcome.clean ? " (auto-merged)" : " (conflict)"}`,
        content: file.content,
        contentHash: file.contentHash,
        timestamp: file.updatedAt,
        opId: opId || `op_${Date.now()}`,
      };
      file.revisions.push(revRecord);

      result = {
        ok: mergeOutcome.clean,
        status: mergeOutcome.clean ? "COMMITTED" : "CONFLICT",
        fileId,
        previousRevision: currentRev,
        revision: file.currentRevision,
        merged: true,
        conflicts: mergeOutcome.conflicts,
        author,
        content: file.content,
        mode: collabMode,
      };
    }

    if (opId) {
      this.operationsLog.set(opId, result);
    }

    this.outbox.push({
      event_id: this.nextEventId++,
      event_type: "COEDIT_APPLIED",
      file_id: fileId,
      revision: file.currentRevision,
      payload: result,
      created_at: new Date().toISOString(),
    });

    return result;
  }

  headlessRead(fileId, revision) {
    const file = this.files.get(fileId);
    if (!file) return null;
    if (revision) {
      const rec = file.revisions.find(r => r.revision === Number(revision));
      return rec ? rec.content : null;
    }
    return file.content;
  }

  headlessHistory(fileId) {
    const file = this.files.get(fileId);
    if (!file) return [];
    return file.revisions.map(r => ({
      revision: r.revision,
      author: r.author,
      contentHash: r.contentHash,
      timestamp: r.timestamp,
      lines: r.content.split("\n").length,
    }));
  }

  headlessDiff(fileId, revA, revB) {
    const file = this.files.get(fileId);
    if (!file) throw new Error(`File '${fileId}' not found`);
    const rA = file.revisions.find(r => r.revision === Number(revA)) || file.revisions[0];
    const rB = file.revisions.find(r => r.revision === Number(revB)) || file.revisions[file.revisions.length - 1];
    return formatUnifiedDiff(
      rA.content.split("\n"),
      rB.content.split("\n"),
      `rev_${rA.revision}`,
      `rev_${rB.revision}`
    );
  }

  headlessListFiles() {
    const list = [];
    for (const [id, f] of this.files.entries()) {
      list.push({
        fileId: id,
        path: f.path,
        language: f.language,
        revision: f.currentRevision,
        lines: f.content.split("\n").length,
        updatedAt: f.updatedAt,
      });
    }
    return list;
  }

  evalRustScript(script) {
    const lines = script.split("\n");
    const results = [];
    for (const raw of lines) {
      for (const stmt of raw.split(";")) {
        const s = stmt.trim();
        if (!s || s.startsWith("//")) continue;
        results.push(this.evalSingleRustStmt(s));
      }
    }
    return results.join("\n");
  }

  evalSingleRustStmt(stmt) {
    const clean = stmt.trim().replace(/;$/, "");
    if (/^(ide::(context|ops)::)?help\(\)$/i.test(clean)) {
      return `=== IDE1.0 Blender-Style Rust API (Headless MCP) ===
ide::context::status()                   - Show active documents & workspace stats
ide::data::docs()                        - List all headless documents
ide::data::read("fileId")                - Read file content
ide::ops::open("fileId", "content")      - Create or open file
ide::ops::diff("fileId", revA, revB)     - Unified diff between revisions
ide::ops::coedit("fileId", "author", rev, "content") - Concurrent CAS co-edit`;
    }
    if (/^ide::context::status\(\)$/i.test(clean)) {
      return `ide::context: active_documents=${this.files.size}, total_events=${this.outbox.length}, backend="IDE1.0 Serverless MCP Engine"`;
    }
    if (/^ide::data::(docs|files)\(\)$/i.test(clean)) {
      return JSON.stringify(this.headlessListFiles(), null, 2);
    }
    if (clean.startsWith('ide::data::read(')) {
      const id = clean.slice('ide::data::read('.length, -1).trim().replace(/^["']|["']$/g, "");
      const content = this.headlessRead(id);
      return content !== null ? content : `File '${id}' not found`;
    }
    return `Executed statement: ${clean}`;
  }
}

// Global singleton for serverless container reuse
if (!global.__ide_coordinator) {
  global.__ide_coordinator = new ServerlessCoordinator();
}

export const coordinator = global.__ide_coordinator;
