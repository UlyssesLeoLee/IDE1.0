import test from "node:test";
import assert from "node:assert/strict";
import {
  authenticateRequest,
  authorizeScope,
  enforcePayloadSize,
  resolveFileTarget,
  signJwt,
} from "../api/v1/_auth.js";
import {
  checkSyntaxIntegrity,
  diffLines,
  formatUnifiedDiff,
  nemaSemanticMerge,
  parseNemaStrict,
  splitLines,
  threeWayMerge,
} from "../api/v1/_merge.js";
import { evalRustScriptWithStore, parseArgs, splitStatements } from "../api/v1/_rust_eval.js";
import { handleMcpRequest } from "../api/v1/mcp.js";

test("P1-02: threeWayMerge handles multi-insert, delete, and line drift without conflicts", () => {
  const base = "a\nb\nc\nd\ne\nf\n";
  const ours = "a\ni1\ni2\nb\nc\nd\ne\nf\n";
  const theirs = "a\nb\nc\ne\nf\ntail\n";

  const m = threeWayMerge(base, ours, theirs);
  assert.equal(m.clean, true);
  assert.equal(m.conflicts, 0);
  assert.equal(m.text, "a\ni1\ni2\nb\nc\ne\nf\ntail\n");
});

test("P1-02: threeWayMerge clusters partial overlaps accurately", () => {
  const base = "1\n2\n3\n4\n5\n";
  const ours = "1\n2_ours\n3_ours\n4\n5\n";
  const theirs = "1\n2\n3_theirs\n4_theirs\n5\n";

  const m = threeWayMerge(base, ours, theirs);
  assert.equal(m.clean, false);
  assert.equal(m.conflicts, 1);
  assert.ok(m.text.includes("<<<<<<< HEAD (ours)"));
});

test("P1-02: Rust delimiter & syntax check ignores lifetimes, chars, strings, comments", () => {
  const rustCode = `
pub fn keep<'a>(s: &'a str) -> char {
    // comment with { mismatch
    let brace = "}";
    '{'
}
`;
  assert.equal(checkSyntaxIntegrity("rust", rustCode).ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "pub fn broken() {\n").ok, false);
  assert.equal(checkSyntaxIntegrity("rust", "<<<<<<< HEAD\n").ok, false);
});

test("P1-06: parseNemaStrict supports TAB-prefixed Table, 5W1H, and Graph domains", () => {
  const doc =
    "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\n\t?W1 1\nBuild auth flow user today edge\n\t>G1 1\nLoginAPI node Entrypoint\nAuthSvc node Backend\nLoginAPI>Call1>AuthSvc edge Auth\n\n\n";
  const parsed = parseNemaStrict(doc);
  assert.equal(parsed.ok, true, parsed.error);
  assert.equal(parsed.domains.length, 3);
  assert.equal(parsed.domains[0].mode, "table");
  assert.equal(parsed.domains[1].mode, "5w1h");
  assert.equal(parsed.domains[2].mode, "graph");
});

test("P1-06: nemaSemanticMerge preserves domain structures & detects Table row conflicts", () => {
  const base = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\nBeta 200\n\n\n";
  const ours = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 150\nBeta 200\n\n\n";
  const theirs = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\nBeta 200\nGamma 300\n\n\n";

  const okMerge = nemaSemanticMerge(base, ours, theirs);
  assert.equal(okMerge.clean, true);
  assert.ok(okMerge.text.includes("Alpha 150"));
  assert.ok(okMerge.text.includes("Gamma 300"));
  assert.equal(parseNemaStrict(okMerge.text).ok, true);

  // Conflicting modification on the same Table key 'Alpha'
  const conflictTheirs = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 999\nBeta 200\n\n\n";
  const badMerge = nemaSemanticMerge(base, ours, conflictTheirs);
  assert.equal(badMerge.clean, false);
  assert.ok(badMerge.conflicts >= 1);
});

test("P0-03: JWT & static token auth, expiration, workspace ACL, and path safety", () => {
  const secret = "test-jwt-secret-123";
  const env = {
    IDE_MCP_JWT_SECRET: secret,
    IDE_MCP_TOKENS: JSON.stringify([
      { token: "tok-ro", actor_id: "reader", tenant_id: "t1", workspaces: ["ws_a"], scopes: ["read"] },
    ]),
  };

  const jwt = signJwt({ sub: "alice", tenant_id: "t1", workspaces: ["audit_*"], scopes: ["read", "write"] }, secret, 120);
  const p = authenticateRequest({ headers: { authorization: `Bearer ${jwt}` } }, env);
  assert.equal(p.tenantId, "t1");
  assert.equal(p.actorId, "alice");
  authorizeScope(p, "audit_123", "write");

  assert.throws(() => authorizeScope(p, "prod_secret", "read"), { statusCode: 403 });

  const expiredJwt = signJwt({ sub: "alice", tenant_id: "t1" }, secret, -5);
  assert.throws(() => authenticateRequest({ headers: { authorization: `Bearer ${expiredJwt}` } }, env), {
    statusCode: 401,
    code: "TOKEN_EXPIRED",
  });

  const ro = authenticateRequest({ headers: { authorization: "Bearer tok-ro" } }, env);
  authorizeScope(ro, "ws_a", "read");
  assert.throws(() => authorizeScope(ro, "ws_a", "write"), { statusCode: 403 });

  assert.throws(() => resolveFileTarget("audit_1:../../etc/passwd", p), { statusCode: 400 });
  assert.throws(() => enforcePayloadSize({ headers: { "content-length": String(5 * 1024 * 1024) } }), {
    statusCode: 413,
  });
  // Parsed req.body exceeding 2 MiB without content-length must also fail closed with 413
  assert.throws(
    () =>
      enforcePayloadSize({
        headers: {},
        body: { jsonrpc: "2.0", params: { content: "x".repeat(2 * 1024 * 1024 + 512) } },
      }),
    { statusCode: 413, code: "PAYLOAD_TOO_LARGE" }
  );

  // File-level ACL check (P0-03)
  const fileJwt = signJwt(
    {
      sub: "file-scoped",
      tenant_id: "t1",
      workspaces: ["ws_a"],
      files: ["src/allowed.rs"],
      scopes: ["read", "write"],
    },
    secret,
    120
  );
  const fp = authenticateRequest({ headers: { authorization: `Bearer ${fileJwt}` } }, env);
  authorizeScope(fp, "ws_a", "write", "src/allowed.rs");
  assert.throws(() => authorizeScope(fp, "ws_a", "read", "src/secret.rs"), {
    statusCode: 403,
    code: "FORBIDDEN_FILE",
  });
});

test("P0-03 & SEC-001: PgDurableStore listProposals filters by DB + ACL and resolveProposal enforces write auth", async () => {
  const { PgDurableStore, buildPgSslConfig } = await import("../api/v1/_store.js");
  // SEC-003: PostgreSQL TLS must verify certificates remotely and never disable verification
  assert.equal(buildPgSslConfig("postgres://user:pass@localhost:5432/db"), false);
  assert.deepEqual(buildPgSslConfig("postgres://user:pass@ep-cool.neon.tech/neondb"), {
    rejectUnauthorized: true,
  });
  assert.deepEqual(
    buildPgSslConfig("postgres://user:pass@ep-cool.neon.tech/neondb", {
      PG_CA_CERT: "LINE1\\nLINE2",
    }),
    { rejectUnauthorized: true, ca: "LINE1\nLINE2" }
  );

  // SEC-002: Missing workspaces or scopes claims MUST fail closed with 403
  assert.throws(
    () =>
      authenticateRequest(
        { headers: { authorization: `Bearer ${signJwt({ sub: "a", tenant_id: "t1", scopes: ["read"] }, "s", 60)}` } },
        { IDE_MCP_JWT_SECRET: "s" }
      ),
    { statusCode: 403, code: "MISSING_WORKSPACE_CLAIM" }
  );
  assert.throws(
    () =>
      authenticateRequest(
        { headers: { authorization: `Bearer ${signJwt({ sub: "a", tenant_id: "t1", workspaces: ["ws_ok"] }, "s", 60)}` } },
        { IDE_MCP_JWT_SECRET: "s" }
      ),
    { statusCode: 403, code: "MISSING_SCOPE_CLAIM" }
  );
  assert.throws(
    () =>
      authenticateRequest(
        { headers: { authorization: "Bearer bad-static" } },
        { IDE_MCP_TOKENS: JSON.stringify([{ token: "bad-static", actor_id: "x", tenant_id: "t1", workspaces: ["ws_ok"] }]) }
      ),
    { statusCode: 403, code: "MISSING_TOKEN_CLAIMS" }
  );

  let capturedParams = null;
  const rows = [
    { proposal_id: "p1", tenant_id: "t1", workspace_id: "ws_ok", file_path: "src/a.rs", file_id: "ws_ok:src/a.rs", status: "PENDING_REVIEW" },
    { proposal_id: "p2", tenant_id: "t1", workspace_id: "ws_other", file_path: "src/b.rs", file_id: "ws_other:src/b.rs", status: "PENDING_REVIEW" },
    { proposal_id: "p3", tenant_id: "t1", workspace_id: "ws_ok", file_path: "src/secret.rs", file_id: "ws_ok:src/secret.rs", status: "PENDING_REVIEW" },
  ];
  const mockPool = {
    async query(sql, params) {
      if (String(sql).includes("FROM mcp_proposals")) {
        capturedParams = params;
        return { rows };
      }
      return { rows: [] };
    },
    async connect() {
      return {
        async query(sql) {
          if (String(sql).includes("FROM mcp_proposals")) {
            return {
              rows: [
                {
                  proposal_id: "p2",
                  tenant_id: "t1",
                  workspace_id: "ws_other",
                  file_path: "src/b.rs",
                  file_id: "ws_other:src/b.rs",
                  status: "PENDING_REVIEW",
                },
              ],
            };
          }
          return { rows: [] };
        },
        release() {},
      };
    },
  };
  const store = new PgDurableStore({ pool: mockPool });
  store.schemaPromise = Promise.resolve();

  const principal = {
    actorId: "a1",
    tenantId: "t1",
    workspaces: ["ws_ok"],
    files: ["src/a.rs"],
    scopes: ["read", "write"],
  };
  const visible = await store.listProposals({ tenantId: "t1", principal });
  assert.deepEqual(capturedParams[3], ["ws_ok"]);
  assert.deepEqual(visible.map((r) => r.proposal_id), ["p1"]);

  // Wrong-workspace resolveProposal MUST be rejected with 403
  await assert.rejects(
    () =>
      store.resolveProposal({
        tenantId: "t1",
        principal,
        actorId: "a1",
        proposalId: "p2",
        resolvedContent: "x",
        baseRevision: 1,
        opId: "res-x",
      }),
    { statusCode: 403, code: "FORBIDDEN_WORKSPACE" }
  );

  // Read-only principal resolving own workspace proposal MUST be rejected with 403
  const roPrincipal = { actorId: "ro", tenantId: "t1", workspaces: ["ws_other"], files: ["*"], scopes: ["read"] };
  await assert.rejects(
    () =>
      store.resolveProposal({
        tenantId: "t1",
        principal: roPrincipal,
        actorId: "ro",
        proposalId: "p2",
        resolvedContent: "x",
        baseRevision: 1,
        opId: "res-ro",
      }),
    { statusCode: 403, code: "INSUFFICIENT_SCOPE" }
  );
});

test("P1-05 & P0-03: handleMcpRequest Streamable HTTP lifecycle & fail-closed auth", async () => {
  const makeRes = () => {
    const out = { status: 200, headers: {}, body: null };
    return {
      setHeader(k, v) {
        out.headers[k.toLowerCase()] = v;
      },
      status(code) {
        out.status = code;
        return {
          json(obj) {
            out.body = obj;
            return out;
          },
          end() {
            return out;
          },
        };
      },
    };
  };

  const fakeStore = {
    async enforceRateLimit() {},
  };

  // Initialize returns protocolVersion and Mcp-Session-Id
  const initRes = await handleMcpRequest(
    { method: "POST", headers: {}, body: { jsonrpc: "2.0", id: 1, method: "initialize", params: {} } },
    makeRes(),
    { store: fakeStore, env: { IDE_MCP_JWT_SECRET: "sec" } }
  );
  assert.equal(initRes.status, 200);
  assert.equal(initRes.body.result.protocolVersion, "2024-11-05");

  // Unauthenticated tools/call MUST fail closed with 401
  const unauthRes = await handleMcpRequest(
    {
      method: "POST",
      headers: {},
      body: { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "headless_read", arguments: { file_id: "default:src/math.rs" } } },
    },
    makeRes(),
    { store: fakeStore, env: { IDE_MCP_JWT_SECRET: "sec" } }
  );
  assert.equal(unauthRes.status, 401);

  // Valid token for wrong workspace MUST return 403
  const jwt = signJwt({ sub: "agent1", tenant_id: "t1", workspaces: ["audit_only"], scopes: ["read", "write"] }, "sec", 60);
  const forbidRes = await handleMcpRequest(
    {
      method: "POST",
      headers: { authorization: `Bearer ${jwt}` },
      body: { jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: "headless_read", arguments: { file_id: "secret_ws:file.rs" } } },
    },
    makeRes(),
    { store: fakeStore, env: { IDE_MCP_JWT_SECRET: "sec" } }
  );
  assert.equal(forbidRes.status, 403);
});

test("diffLines + formatUnifiedDiff handles trailing lines consistently", () => {
  const u = formatUnifiedDiff("a\nb\nc\n", "a\nb2\nc\n", "rev1", "rev2");
  assert.ok(u.includes("-b\n+b2"));
  assert.equal(diffLines(["a\n"], ["a\n"]).coarse, false);
});

test("Blender-style ide_rust_eval parses semicolons/escapes & enforces ACL", async () => {
  const script = `
    // comment line
    ide::ops::open("ws_ok:src/m.rs", "rust", "let x = 1; let y = 2;\\nlet z = 3;\\n");
    ide::ops::insert("ws_ok:src/m.rs", 2, "// mid;");
    ide::ops::append("ws_ok:src/m.rs", "let w = 4;");
    ide::ops::replace_all("ws_ok:src/m.rs", "let w = 4;", "const W: i32 = 4;");
  `;
  const stmts = splitStatements(script);
  assert.equal(stmts.length, 4);
  const args = parseArgs('"ws_ok:src/m.rs", "rust", "a; b;\\nc"');
  assert.deepEqual(args, ["ws_ok:src/m.rs", "rust", "a; b;\nc"]);

  // Minimal in-memory store mock implementing PgDurableStore contract for _rust_eval
  const docs = new Map();
  const mockStore = {
    async listFiles() {
      return [];
    },
    async openFile({ workspaceId, filePath, fileId, content, language }) {
      const rec = { file_id: fileId, workspace_id: workspaceId, filePath, language, content, current_revision: 1, content_hash: "h1" };
      docs.set(fileId, rec);
      return { ok: true, status: "CREATED", file_id: fileId, current_revision: 1, content_hash: "h1" };
    },
    async readFile({ fileId, startLine, endLine }) {
      const d = docs.get(fileId);
      const lines = splitLines(d.content);
      let out = d.content;
      if (startLine || endLine) {
        const s = Math.max(0, (Number(startLine) || 1) - 1);
        const e = Math.min(lines.length, Number(endLine) || lines.length);
        out = lines.slice(s, e).join("");
      }
      return { ...d, total_lines: lines.length, content: out };
    },
    async editLines({ fileId, startLine, endLine, replacement }) {
      const d = docs.get(fileId);
      const lines = splitLines(d.content);
      const s = Math.max(0, Math.min(lines.length, (Number(startLine) || 1) - 1));
      const e = Number(endLine) < Number(startLine) ? s : Math.max(s, Math.min(lines.length, Number(endLine) || s));
      const repl = replacement && !replacement.endsWith("\n") ? `${replacement}\n` : String(replacement ?? "");
      d.content = lines.slice(0, s).join("") + repl + lines.slice(e).join("");
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: `h${d.current_revision}`, content: d.content };
    },
    async replaceAll({ fileId, find, replace }) {
      const d = docs.get(fileId);
      const count = d.content.split(find).length - 1;
      d.content = d.content.split(find).join(replace);
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: `h${d.current_revision}`, replacements: count };
    },
    async coedit({ fileId, newText }) {
      const d = docs.get(fileId);
      d.content = newText;
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: `h${d.current_revision}` };
    },
  };

  const rwPrincipal = { actorId: "coder", tenantId: "t1", workspaces: ["ws_ok"], files: ["*"], scopes: ["read", "write"] };
  const res = await evalRustScriptWithStore(mockStore, rwPrincipal, script, "agent-py");
  assert.equal(res.ok, true);
  assert.equal(res.steps.length, 4);
  const finalDoc = docs.get("ws_ok:src/m.rs");
  assert.equal(finalDoc.current_revision, 4);
  assert.ok(finalDoc.content.includes("let x = 1; let y = 2;"));
  assert.ok(finalDoc.content.includes("// mid;"));
  assert.ok(finalDoc.content.includes("const W: i32 = 4;"));

  // Read-only principal MUST fail closed with 403 when trying write ops
  const roPrincipal = { actorId: "ro", tenantId: "t1", workspaces: ["ws_ok"], files: ["*"], scopes: ["read"] };
  await assert.rejects(
    () => evalRustScriptWithStore(mockStore, roPrincipal, 'ide::ops::edit("ws_ok:src/m.rs", 1, 1, "hijack")'),
    { statusCode: 403, code: "INSUFFICIENT_SCOPE" }
  );
});
