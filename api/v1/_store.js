// Durable Postgres Transaction Store for IDE1.0 Vercel MCP (P0-01..P0-05, P1-02, P1-03, P1-06)

import crypto from "crypto";
import fs from "fs";
import pg from "pg";
import { authorizeScope, isFileAllowed, isWorkspaceAllowed } from "./_auth.js";
import {
  checkSyntaxIntegrity,
  formatUnifiedDiff,
  nemaSemanticMerge,
  parseNemaStrict,
  splitLines,
  threeWayMerge,
} from "./_merge.js";

const { Pool } = pg;

export class StoreError extends Error {
  constructor(statusCode, code, message) {
    super(message);
    this.statusCode = statusCode;
    this.code = code;
  }
}

export function sha256Hex(text) {
  return crypto.createHash("sha256").update(String(text || ""), "utf8").digest("hex");
}

const SCHEMA_SQL = fs.readFileSync(
  new URL("../../db/migrations/001_mcp_durable_state.sql", import.meta.url),
  "utf8"
);

export class PgDurableStore {
  constructor(options = {}) {
    this.pool = options.pool || null;
    this.schemaPromise = null;
  }

  getPool() {
    if (this.pool) return this.pool;
    const url = process.env.DATABASE_URL || process.env.POSTGRES_URL || "";
    if (!url) {
      throw new StoreError(
        503,
        "STORAGE_NOT_CONFIGURED",
        "DATABASE_URL / POSTGRES_URL is required for durable MCP state"
      );
    }
    const useSsl = !url.includes("localhost") && !url.includes("127.0.0.1");
    this.pool = new Pool({
      connectionString: url,
      max: 5,
      idleTimeoutMillis: 15000,
      connectionTimeoutMillis: 8000,
      ssl: useSsl ? { rejectUnauthorized: false } : false,
    });
    return this.pool;
  }

  async ensureSchema() {
    if (!this.schemaPromise) {
      const pool = this.getPool();
      this.schemaPromise = pool.query(SCHEMA_SQL).catch((err) => {
        this.schemaPromise = null;
        throw err;
      });
    }
    return this.schemaPromise;
  }

  async withTransaction(fn) {
    await this.ensureSchema();
    const client = await this.getPool().connect();
    try {
      await client.query("BEGIN");
      const res = await fn(client);
      await client.query("COMMIT");
      return res;
    } catch (err) {
      try {
        await client.query("ROLLBACK");
      } catch {
        // ignore rollback failure
      }
      throw err;
    } finally {
      client.release();
    }
  }

  async enforceRateLimit(tenantId, maxPerMinute = 300) {
    await this.ensureSchema();
    const bucketKey = `tenant:${tenantId}`;
    const res = await this.getPool().query(
      `INSERT INTO mcp_rate_limits (bucket_key, window_start, req_count)
       VALUES ($1, NOW(), 1)
       ON CONFLICT (bucket_key) DO UPDATE SET
         req_count = CASE
           WHEN mcp_rate_limits.window_start < NOW() - INTERVAL '60 seconds' THEN 1
           ELSE mcp_rate_limits.req_count + 1
         END,
         window_start = CASE
           WHEN mcp_rate_limits.window_start < NOW() - INTERVAL '60 seconds' THEN NOW()
           ELSE mcp_rate_limits.window_start
         END
       RETURNING req_count`,
      [bucketKey]
    );
    const count = Number(res.rows[0]?.req_count || 1);
    if (count > maxPerMinute) {
      throw new StoreError(429, "RATE_LIMIT_EXCEEDED", `Rate limit exceeded (${count}/${maxPerMinute} req/min)`);
    }
  }

  async recordAudit(db, { tenantId, actorId, workspaceId, filePath, action, outcome, opId = null, statusCode = 200, detail = null }) {
    try {
      await db.query(
        `INSERT INTO mcp_audit_log
         (tenant_id, actor_id, workspace_id, file_path, action, outcome, op_id, status_code, detail)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)`,
        [tenantId, actorId, workspaceId, filePath, action, outcome, opId, statusCode, detail ? JSON.stringify(detail) : null]
      );
    } catch {
      // Do not mask primary domain errors if standalone audit insert fails
    }
  }

  async openFile({ tenantId, workspaceId, filePath, fileId, actorId, content = "", language = "plain", mode = "create_if_absent" }) {
    const initContent = String(content ?? "");
    const initHash = sha256Hex(initContent);

    return this.withTransaction(async (client) => {
      if (mode === "open_existing") {
        const cur = await client.query(
          `SELECT * FROM mcp_documents WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3`,
          [tenantId, workspaceId, filePath]
        );
        if (cur.rows.length === 0) {
          throw new StoreError(404, "FILE_NOT_FOUND", `File '${fileId}' not found`);
        }
        const row = cur.rows[0];
        return {
          ok: true,
          status: "ALREADY_EXISTS",
          created: false,
          file_id: row.file_id,
          workspace_id: row.workspace_id,
          language: row.language,
          current_revision: Number(row.current_revision),
          content_hash: row.content_hash,
          content: row.content,
        };
      }

      if (mode === "create_if_absent") {
        const inserted = await client.query(
          `INSERT INTO mcp_documents
           (tenant_id, workspace_id, file_path, file_id, language, current_revision, content, content_hash)
           VALUES ($1, $2, $3, $4, $5, 1, $6, $7)
           ON CONFLICT (tenant_id, workspace_id, file_path) DO NOTHING
           RETURNING *`,
          [tenantId, workspaceId, filePath, fileId, language, initContent, initHash]
        );
        if (inserted.rows.length > 0) {
          await client.query(
            `INSERT INTO mcp_revisions
             (tenant_id, workspace_id, file_path, revision, content, content_hash, agent_id, provider, op_id, summary)
             VALUES ($1, $2, $3, 1, $4, $5, $6, 'mcp', 'init', 'Initial open')`,
            [tenantId, workspaceId, filePath, initContent, initHash, actorId]
          );
          await this.recordAudit(client, {
            tenantId,
            actorId,
            workspaceId,
            filePath,
            action: "open_file",
            outcome: "CREATED",
          });
          return {
            ok: true,
            status: "CREATED",
            created: true,
            file_id: fileId,
            workspace_id: workspaceId,
            language,
            current_revision: 1,
            content_hash: initHash,
            content: initContent,
          };
        }
        const existing = await client.query(
          `SELECT * FROM mcp_documents WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3`,
          [tenantId, workspaceId, filePath]
        );
        const row = existing.rows[0];
        return {
          ok: true,
          status: "ALREADY_EXISTS",
          created: false,
          file_id: row.file_id,
          workspace_id: row.workspace_id,
          language: row.language,
          current_revision: Number(row.current_revision),
          content_hash: row.content_hash,
          content: row.content,
        };
      }

      if (mode === "force_overwrite") {
        const cur = await client.query(
          `SELECT * FROM mcp_documents WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3 FOR UPDATE`,
          [tenantId, workspaceId, filePath]
        );
        if (cur.rows.length === 0) {
          return this.openFile({ tenantId, workspaceId, filePath, fileId, actorId, content: initContent, language, mode: "create_if_absent" });
        }
        const row = cur.rows[0];
        const newRev = Number(row.current_revision) + 1;
        await client.query(
          `UPDATE mcp_documents SET current_revision = $4, content = $5, content_hash = $6, updated_at = NOW()
           WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3`,
          [tenantId, workspaceId, filePath, newRev, initContent, initHash]
        );
        await client.query(
          `INSERT INTO mcp_revisions
           (tenant_id, workspace_id, file_path, revision, content, content_hash, agent_id, provider, summary)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'mcp', 'Authorized force_overwrite')`,
          [tenantId, workspaceId, filePath, newRev, initContent, initHash, actorId]
        );
        await this.recordAudit(client, {
          tenantId,
          actorId,
          workspaceId,
          filePath,
          action: "open_file_force_overwrite",
          outcome: "OVERWRITTEN",
        });
        return {
          ok: true,
          status: "OVERWRITTEN",
          created: false,
          file_id: row.file_id,
          workspace_id: row.workspace_id,
          language: row.language,
          current_revision: newRev,
          content_hash: initHash,
          content: initContent,
        };
      }

      throw new StoreError(400, "INVALID_OPEN_MODE", `Unsupported open mode '${mode}'`);
    });
  }

  async readFile({ tenantId, workspaceId, filePath, fileId, revision, startLine, endLine }) {
    await this.ensureSchema();
    const pool = this.getPool();
    const docRes = await pool.query(
      `SELECT * FROM mcp_documents WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3`,
      [tenantId, workspaceId, filePath]
    );
    if (docRes.rows.length === 0) {
      throw new StoreError(404, "FILE_NOT_FOUND", `File '${fileId}' not found`);
    }
    const doc = docRes.rows[0];
    let targetContent = doc.content;
    let targetHash = doc.content_hash;
    let targetRev = Number(doc.current_revision);

    if (revision !== undefined && revision !== null) {
      const revRes = await pool.query(
        `SELECT revision, content, content_hash FROM mcp_revisions
         WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3 AND revision = $4`,
        [tenantId, workspaceId, filePath, Number(revision)]
      );
      if (revRes.rows.length === 0) {
        throw new StoreError(404, "REVISION_NOT_FOUND", `Revision ${revision} not found for '${fileId}'`);
      }
      targetContent = revRes.rows[0].content;
      targetHash = revRes.rows[0].content_hash;
      targetRev = Number(revRes.rows[0].revision);
    }

    const lines = splitLines(targetContent);
    let outputText = targetContent;
    if (startLine || endLine) {
      const s = Math.max(0, (Number(startLine) || 1) - 1);
      const e = Math.min(lines.length, Number(endLine) || lines.length);
      outputText = s < e ? lines.slice(s, e).join("") : "";
    }

    return {
      ok: true,
      file_id: doc.file_id,
      workspace_id: doc.workspace_id,
      language: doc.language,
      revision: targetRev,
      current_revision: Number(doc.current_revision),
      content_hash: targetHash,
      total_lines: lines.length,
      content: outputText,
    };
  }

  async listFiles(principal) {
    await this.ensureSchema();
    const res = await this.getPool().query(
      `SELECT file_id, workspace_id, file_path, language, current_revision, content_hash, updated_at, content
       FROM mcp_documents WHERE tenant_id = $1 ORDER BY updated_at DESC LIMIT 200`,
      [principal.tenantId]
    );
    const allowed = res.rows.filter(
      (r) =>
        isWorkspaceAllowed(principal, r.workspace_id) &&
        isFileAllowed(principal, r.workspace_id, r.file_path)
    );
    return allowed.map((r) => ({
      fileId: r.file_id,
      file_id: r.file_id,
      workspace_id: r.workspace_id,
      path: r.file_path,
      language: r.language,
      revision: Number(r.current_revision),
      current_revision: Number(r.current_revision),
      content_hash: r.content_hash,
      lines: splitLines(r.content).length,
      updatedAt: r.updated_at,
    }));
  }

  async fileHistory({ tenantId, workspaceId, filePath, fileId }) {
    await this.ensureSchema();
    const res = await this.getPool().query(
      `SELECT revision, agent_id, provider, content_hash, auto_merged, summary, op_id, created_at, content
       FROM mcp_revisions
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3
       ORDER BY revision ASC`,
      [tenantId, workspaceId, filePath]
    );
    if (res.rows.length === 0) {
      throw new StoreError(404, "FILE_NOT_FOUND", `File '${fileId}' not found`);
    }
    return res.rows.map((r) => ({
      revision: Number(r.revision),
      author: r.agent_id,
      agent_id: r.agent_id,
      provider: r.provider,
      contentHash: r.content_hash,
      content_hash: r.content_hash,
      auto_merged: r.auto_merged,
      summary: r.summary,
      op_id: r.op_id,
      timestamp: r.created_at,
      lines: splitLines(r.content).length,
    }));
  }

  async diffRevisions({ tenantId, workspaceId, filePath, fileId, revA, revB }) {
    await this.ensureSchema();
    const res = await this.getPool().query(
      `SELECT revision, content FROM mcp_revisions
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3 AND revision IN ($4, $5)`,
      [tenantId, workspaceId, filePath, Number(revA), Number(revB)]
    );
    const map = new Map(res.rows.map((r) => [Number(r.revision), r.content]));
    const cA = map.get(Number(revA));
    const cB = map.get(Number(revB));
    if (cA === undefined || cB === undefined) {
      throw new StoreError(404, "REVISION_NOT_FOUND", `Revisions ${revA}/${revB} not found for '${fileId}'`);
    }
    const u = formatUnifiedDiff(cA, cB, `rev_${revA}`, `rev_${revB}`);
    return u || "(Identical revisions: zero differences)";
  }

  async acquireLease({ tenantId, workspaceId, filePath, fileId, agentId, ttlMs = 60000 }) {
    const safeTtl = Math.max(1000, Math.min(Number(ttlMs) || 60000, 600000));
    return this.withTransaction(async (client) => {
      const docRes = await client.query(
        `UPDATE mcp_documents
         SET next_fencing_token = next_fencing_token + 1
         WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3
         RETURNING next_fencing_token, current_revision, content_hash`,
        [tenantId, workspaceId, filePath]
      );
      if (docRes.rows.length === 0) {
        throw new StoreError(404, "FILE_NOT_FOUND", `File '${fileId}' not found`);
      }
      const token = Number(docRes.rows[0].next_fencing_token);
      const leaseRes = await client.query(
        `INSERT INTO mcp_leases (tenant_id, workspace_id, file_path, agent_id, fencing_token, expires_at)
         VALUES ($1, $2, $3, $4, $5, NOW() + ($6::bigint * INTERVAL '1 millisecond'))
         ON CONFLICT (tenant_id, workspace_id, file_path) DO UPDATE SET
           agent_id = EXCLUDED.agent_id,
           fencing_token = EXCLUDED.fencing_token,
           expires_at = EXCLUDED.expires_at
         RETURNING expires_at`,
        [tenantId, workspaceId, filePath, agentId, token, safeTtl]
      );
      await this.recordAudit(client, {
        tenantId,
        actorId: agentId,
        workspaceId,
        filePath,
        action: "acquire_lease",
        outcome: "LEASED",
        detail: { fencing_token: token, ttl_ms: safeTtl },
      });
      return {
        ok: true,
        file_id: fileId,
        agent_id: agentId,
        fencing_token: token,
        expires_at: leaseRes.rows[0].expires_at,
        head_revision: Number(docRes.rows[0].current_revision),
        head_content_hash: docRes.rows[0].content_hash,
      };
    });
  }

  async _executeCasWrite(client, params) {
    const {
      tenantId,
      workspaceId,
      filePath,
      fileId,
      actorId,
      baseRevision,
      baseContentHash = null,
      fencingToken = null,
      opId,
      mode = null,
      payloadHash,
      deriveCandidateFn,
      actionName = "coedit",
    } = params;

    if (!opId || typeof opId !== "string" || !opId.trim()) {
      throw new StoreError(400, "MISSING_OP_ID", "op_id is required for idempotent write operations");
    }
    if (baseRevision === undefined || baseRevision === null || Number(baseRevision) < 1) {
      throw new StoreError(400, "MISSING_BASE_REVISION", "base_revision (>= 1) is required for write operations");
    }
    if (!actorId || typeof actorId !== "string" || !actorId.trim()) {
      throw new StoreError(400, "MISSING_AGENT_ID", "author / agent_id is required for write operations");
    }

    // 1. Lock authoritative file row first -> serializes writers to THIS file cleanly across Vercel instances
    const docRes = await client.query(
      `SELECT * FROM mcp_documents
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3
       FOR UPDATE`,
      [tenantId, workspaceId, filePath]
    );
    if (docRes.rows.length === 0) {
      throw new StoreError(404, "FILE_NOT_FOUND", `File '${fileId}' not found (open it explicitly first)`);
    }
    const doc = docRes.rows[0];
    const headRev = Number(doc.current_revision);
    const headHash = doc.content_hash;

    // 2. Scoped atomic idempotency check BEFORE any state mutation
    const idemRes = await client.query(
      `SELECT payload_hash, response_json FROM mcp_idempotency
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3 AND op_id = $4`,
      [tenantId, workspaceId, filePath, opId]
    );
    if (idemRes.rows.length > 0) {
      const prev = idemRes.rows[0];
      if (prev.payload_hash !== payloadHash) {
        throw new StoreError(
          409,
          "IDEMPOTENCY_PAYLOAD_MISMATCH",
          `op_id '${opId}' was already committed on '${fileId}' with a different payload`
        );
      }
      return { ...prev.response_json, idempotent_replay: true };
    }

    // 3. Validate base_revision & optional base_content_hash
    const baseRevNum = Number(baseRevision);
    const baseRevRes = await client.query(
      `SELECT revision, content, content_hash FROM mcp_revisions
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3 AND revision = $4`,
      [tenantId, workspaceId, filePath, baseRevNum]
    );
    if (baseRevRes.rows.length === 0) {
      throw new StoreError(409, "STALE_OR_MISSING_BASE_REVISION", `Base revision ${baseRevNum} not found for '${fileId}'`);
    }
    const baseRow = baseRevRes.rows[0];
    if (baseContentHash && baseRow.content_hash !== baseContentHash) {
      throw new StoreError(
        409,
        "BASE_HASH_MISMATCH",
        `Content hash mismatch at base_revision ${baseRevNum}: expected ${baseRow.content_hash}, got ${baseContentHash}`
      );
    }

    // 4. Validate fencing token lease if provided
    if (fencingToken !== undefined && fencingToken !== null) {
      const leaseRes = await client.query(
        `SELECT agent_id, fencing_token, expires_at FROM mcp_leases
         WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3`,
        [tenantId, workspaceId, filePath]
      );
      const lease = leaseRes.rows[0];
      const expired = !lease || new Date(lease.expires_at).getTime() <= Date.now();
      if (!lease || expired || Number(lease.fencing_token) !== Number(fencingToken)) {
        throw new StoreError(412, "FENCING_TOKEN_STALE", `Fencing token ${fencingToken} is stale or expired`);
      }
    }

    // 5. Derive incoming candidate content from base snapshot
    const derived = deriveCandidateFn(baseRow.content, doc.content);
    if (derived.noOp) {
      const noOpRes = {
        ok: true,
        status: "NO_OP",
        file_id: fileId,
        revision: headRev,
        new_revision: headRev,
        content_hash: headHash,
        ...(derived.extra || {}),
      };
      await client.query(
        `INSERT INTO mcp_idempotency (tenant_id, workspace_id, file_path, op_id, payload_hash, response_json)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [tenantId, workspaceId, filePath, opId, payloadHash, JSON.stringify(noOpRes)]
      );
      return noOpRes;
    }

    const incomingText = derived.text;
    const collabMode = mode || (filePath.endsWith(".nema") || doc.language === "nema" ? "nema" : "git");

    if (collabMode === "nema") {
      const guard = parseNemaStrict(incomingText);
      if (!guard.ok) {
        return {
          ok: false,
          status: "REJECTED_NEMA_SYNTAX_ERROR",
          file_id: fileId,
          head_revision: headRev,
          head_content_hash: headHash,
          error: `NEMA syntax validation failed: ${guard.error}`,
          mode: "nema",
        };
      }
    }

    // 6. Execute Safe Merge + Post-Merge Syntax Check
    const syntaxLang = collabMode === "nema" ? "nema" : doc.language;
    let finalContent = incomingText;
    let clean = true;
    let conflicts = [];
    let autoMerged = false;

    if (baseRevNum !== headRev) {
      autoMerged = true;
      const mergeOutcome =
        collabMode === "nema"
          ? nemaSemanticMerge(baseRow.content, doc.content, incomingText)
          : threeWayMerge(baseRow.content, doc.content, incomingText);
      finalContent = mergeOutcome.text;
      clean = mergeOutcome.clean;
      conflicts = mergeOutcome.conflictHunks || [];
    }

    if (clean) {
      const syntax = checkSyntaxIntegrity(syntaxLang, finalContent);
      if (!syntax.ok) {
        clean = false;
        conflicts = [
          {
            base_line_start: 1,
            base_line_end: 1,
            ours: "HEAD syntax",
            theirs: syntax.error,
          },
        ];
      }
    }

    // 7. Conflict Branch (P0-01): NEVER mutate HEAD; persist conflict proposal instead!
    if (!clean) {
      const proposalId = `prop_${Date.now()}_${crypto.randomBytes(4).toString("hex")}`;
      const conflictDiff = formatUnifiedDiff(doc.content, incomingText, `head_rev_${headRev}`, `proposal_${proposalId}`);
      const proposedHash = sha256Hex(incomingText);

      await client.query(
        `INSERT INTO mcp_proposals
         (proposal_id, tenant_id, workspace_id, file_path, file_id, base_revision, head_revision,
          head_content_hash, agent_id, op_id, proposed_content, proposed_hash, conflict_preview,
          conflict_diff, conflicts_json, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'PENDING_REVIEW')`,
        [
          proposalId,
          tenantId,
          workspaceId,
          filePath,
          fileId,
          baseRevNum,
          headRev,
          headHash,
          actorId,
          opId,
          incomingText,
          proposedHash,
          finalContent,
          conflictDiff,
          JSON.stringify(conflicts),
        ]
      );

      const conflictRes = {
        ok: false,
        status: "CONFLICT",
        proposal_id: proposalId,
        file_id: fileId,
        base_revision: baseRevNum,
        head_revision: headRev,
        revision: headRev,
        head_content_hash: headHash,
        content_hash: headHash,
        merged: false,
        auto_merged: false,
        conflicts: conflicts.length,
        conflict_hunks: conflicts,
        conflict_diff: conflictDiff,
        conflict_preview: finalContent,
        author: actorId,
        mode: collabMode,
      };

      await client.query(
        `INSERT INTO mcp_idempotency (tenant_id, workspace_id, file_path, op_id, payload_hash, response_json)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [tenantId, workspaceId, filePath, opId, payloadHash, JSON.stringify(conflictRes)]
      );
      await client.query(
        `INSERT INTO mcp_outbox (tenant_id, workspace_id, file_path, file_id, event_type, revision, payload_json)
         VALUES ($1, $2, $3, $4, 'CONFLICT_PROPOSED', $5, $6)`,
        [tenantId, workspaceId, filePath, fileId, headRev, JSON.stringify(conflictRes)]
      );
      await this.recordAudit(client, {
        tenantId,
        actorId,
        workspaceId,
        filePath,
        action: actionName,
        outcome: "CONFLICT_PROPOSAL",
        opId,
        statusCode: 200,
        detail: { proposal_id: proposalId, head_revision: headRev },
      });
      return conflictRes;
    }

    // 8. Clean Commit Branch: Dual CAS Update + Revisions + Idempotency + Outbox + Audit
    const newRev = headRev + 1;
    const newHash = sha256Hex(finalContent);
    const upd = await client.query(
      `UPDATE mcp_documents
       SET current_revision = $4, content = $5, content_hash = $6, updated_at = NOW()
       WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3
         AND current_revision = $7 AND content_hash = $8`,
      [tenantId, workspaceId, filePath, newRev, finalContent, newHash, headRev, headHash]
    );
    if (upd.rowCount !== 1) {
      throw new StoreError(409, "CAS_CONFLICT", "Concurrent modification detected during CAS update");
    }

    await client.query(
      `INSERT INTO mcp_revisions
       (tenant_id, workspace_id, file_path, revision, content, content_hash, agent_id, provider, op_id, auto_merged, summary)
       VALUES ($1, $2, $3, $4, $5, $6, $7, 'mcp', $8, $9, $10)`,
      [
        tenantId,
        workspaceId,
        filePath,
        newRev,
        finalContent,
        newHash,
        actorId,
        opId,
        autoMerged,
        `${actionName} mode=${collabMode} auto_merged=${autoMerged}`,
      ]
    );

    const commitRes = {
      ok: true,
      status: "COMMITTED",
      file_id: fileId,
      previous_revision: headRev,
      revision: newRev,
      new_revision: newRev,
      merged: autoMerged,
      auto_merged: autoMerged,
      conflicts: 0,
      author: autoMerged ? `${actorId} (auto-merged)` : actorId,
      content: finalContent,
      content_hash: newHash,
      mode: collabMode,
      ...(derived.extra || {}),
    };

    await client.query(
      `INSERT INTO mcp_idempotency (tenant_id, workspace_id, file_path, op_id, payload_hash, response_json)
       VALUES ($1, $2, $3, $4, $5, $6)`,
      [tenantId, workspaceId, filePath, opId, payloadHash, JSON.stringify(commitRes)]
    );
    await client.query(
      `INSERT INTO mcp_outbox (tenant_id, workspace_id, file_path, file_id, event_type, revision, payload_json)
       VALUES ($1, $2, $3, $4, 'COEDIT_COMMITTED', $5, $6)`,
      [tenantId, workspaceId, filePath, fileId, newRev, JSON.stringify(commitRes)]
    );
    await this.recordAudit(client, {
      tenantId,
      actorId,
      workspaceId,
      filePath,
      action: actionName,
      outcome: "COMMITTED",
      opId,
      statusCode: 200,
      detail: { new_revision: newRev, auto_merged: autoMerged },
    });
    return commitRes;
  }

  async coedit(params) {
    const payloadHash = sha256Hex(
      JSON.stringify(["coedit", params.fileId, params.actorId, Number(params.baseRevision), params.baseContentHash || "", params.mode || "", params.newText])
    );
    return this.withTransaction((client) =>
      this._executeCasWrite(client, {
        ...params,
        payloadHash,
        actionName: "coedit",
        deriveCandidateFn: () => ({ text: String(params.newText ?? "") }),
      })
    );
  }

  async editLines(params) {
    const { startLine, endLine, replacement } = params;
    const payloadHash = sha256Hex(
      JSON.stringify(["edit_lines", params.fileId, params.actorId, Number(params.baseRevision), Number(startLine), Number(endLine), replacement])
    );
    return this.withTransaction((client) =>
      this._executeCasWrite(client, {
        ...params,
        mode: "git",
        payloadHash,
        actionName: "edit_lines",
        deriveCandidateFn: (baseContent) => {
          const lines = splitLines(baseContent);
          const s = Math.max(0, Math.min(lines.length, (Number(startLine) || 1) - 1));
          const e = Math.max(s, Math.min(lines.length, Number(endLine) || s));
          const normalizedRepl =
            replacement && !replacement.endsWith("\n") ? `${replacement}\n` : String(replacement ?? "");
          const next = lines.slice(0, s).join("") + normalizedRepl + lines.slice(e).join("");
          return { text: next };
        },
      })
    );
  }

  async replaceAll(params) {
    const { find, replace } = params;
    if (!find || typeof find !== "string") {
      throw new StoreError(400, "INVALID_ARGUMENT", "'find' must be a non-empty string");
    }
    const payloadHash = sha256Hex(
      JSON.stringify(["replace_all", params.fileId, params.actorId, Number(params.baseRevision), find, String(replace ?? "")])
    );
    return this.withTransaction((client) =>
      this._executeCasWrite(client, {
        ...params,
        mode: "git",
        payloadHash,
        actionName: "replace_all",
        deriveCandidateFn: (baseContent) => {
          const count = baseContent.split(find).length - 1;
          if (count === 0) return { noOp: true, extra: { replacements: 0 } };
          return {
            text: baseContent.split(find).join(String(replace ?? "")),
            extra: { replacements: count },
          };
        },
      })
    );
  }

  async listProposals({ tenantId, principal = null, workspaceId = null, fileId = null }) {
    await this.ensureSchema();
    const res = await this.getPool().query(
      `SELECT proposal_id, file_id, workspace_id, file_path, base_revision, head_revision,
              head_content_hash, agent_id, op_id, proposed_hash, conflict_diff,
              conflicts_json, status, created_at, resolved_by, resolved_revision
       FROM mcp_proposals
       WHERE tenant_id = $1
         AND ($2::text IS NULL OR workspace_id = $2)
         AND ($3::text IS NULL OR file_id = $3)
       ORDER BY created_at DESC LIMIT 100`,
      [tenantId, workspaceId, fileId]
    );
    if (!principal) return res.rows;
    return res.rows.filter(
      (r) =>
        isWorkspaceAllowed(principal, r.workspace_id) &&
        isFileAllowed(principal, r.workspace_id, r.file_path)
    );
  }

  async resolveProposal({ tenantId, principal = null, actorId, proposalId, resolvedContent, baseRevision, opId }) {
    return this.withTransaction(async (client) => {
      const propRes = await client.query(
        `SELECT * FROM mcp_proposals WHERE proposal_id = $1 AND tenant_id = $2 FOR UPDATE`,
        [proposalId, tenantId]
      );
      if (propRes.rows.length === 0) {
        throw new StoreError(404, "PROPOSAL_NOT_FOUND", `Proposal '${proposalId}' not found`);
      }
      const prop = propRes.rows[0];
      if (principal) {
        authorizeScope(principal, prop.workspace_id, "write", prop.file_path);
      }
      if (prop.status !== "PENDING_REVIEW") {
        throw new StoreError(409, "PROPOSAL_ALREADY_RESOLVED", `Proposal '${proposalId}' is already ${prop.status}`);
      }
      const payloadHash = sha256Hex(JSON.stringify(["resolve", proposalId, Number(baseRevision), resolvedContent]));
      const res = await this._executeCasWrite(client, {
        tenantId,
        workspaceId: prop.workspace_id,
        filePath: prop.file_path,
        fileId: prop.file_id,
        actorId,
        baseRevision,
        opId,
        payloadHash,
        actionName: "resolve_proposal",
        deriveCandidateFn: () => ({ text: String(resolvedContent) }),
      });
      if (res.ok) {
        await client.query(
          `UPDATE mcp_proposals SET status = 'RESOLVED', resolved_by = $2, resolved_revision = $3, resolved_at = NOW()
           WHERE proposal_id = $1`,
          [proposalId, actorId, res.revision]
        );
      }
      return res;
    });
  }

  async getEvents(tenantId, workspaceId, afterCursor = 0) {
    await this.ensureSchema();
    const res = await this.getPool().query(
      `SELECT event_id, event_type, workspace_id, file_id, revision, payload_json, created_at
       FROM mcp_outbox
       WHERE tenant_id = $1 AND workspace_id = $2 AND event_id > $3
       ORDER BY event_id ASC LIMIT 200`,
      [tenantId, workspaceId, Number(afterCursor) || 0]
    );
    return res.rows;
  }

  async evalRustScript(principal, script) {
    const clean = String(script || "").trim();
    if (clean === "ide::context::status()" || clean === "ide::context::status();") {
      const files = await this.listFiles(principal);
      return `ide::context: active_documents=${files.length}, tenant=${principal.tenantId}, backend="IDE1.0 Durable MCP (Postgres)"`;
    }
    if (/^ide::data::(docs|files)\(\);?$/i.test(clean)) {
      const files = await this.listFiles(principal);
      return JSON.stringify(files, null, 2);
    }
    return `Safe read-only console executed in tenant '${principal.tenantId}'`;
  }
}

let sharedStore = null;
export function getStore() {
  if (!sharedStore) sharedStore = new PgDurableStore();
  return sharedStore;
}
