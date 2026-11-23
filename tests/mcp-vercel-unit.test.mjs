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
  validateNemaSyntax,
} from "../api/v1/_merge.js";
import { evalRustScriptWithStore, parseArgs, splitStatements } from "../api/v1/_rust_eval.js";
import { PgDurableStore, StoreError, getStore } from "../api/v1/_store.js";
import handler, { handleMcpRequest } from "../api/v1/mcp.js";

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
    async fileHistory({ fileId }) {
      return { file_id: fileId, revisions: [{ revision: 1 }] };
    },
    async diffRevisions({ fileId, revA, revB }) {
      return { file_id: fileId, rev_a: revA, rev_b: revB, diff: "+1" };
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

  // Additional _rust_eval branches: help, status, files, read, lines, history, search, diff, edit, delete_lines, coedit, error paths
  const fullScript = [
    'ide::context::help()',
    'ide::context::status()',
    'ide::data::files()',
    'ide::data::read("ws_ok:src/m.rs")',
    'ide::data::lines("ws_ok:src/m.rs", 1, 2)',
    'ide::data::history("ws_ok:src/m.rs")',
    'ide::ops::search("ws_ok:src/m.rs", "let")',
    'ide::ops::diff("ws_ok:src/m.rs", 1, 2)',
    'ide::ops::edit("ws_ok:src/m.rs", 1, 1, "let patched = 1;")',
    'ide::ops::delete_lines("ws_ok:src/m.rs", 2, 2)',
    'ide::ops::coedit("ws_ok:src/m.rs", "fn rewritten() {}\n")',
  ].join(";\n");
  const fullEval = await evalRustScriptWithStore(mockStore, rwPrincipal, fullScript, "agent-py");
  assert.equal(fullEval.ok, true, JSON.stringify(fullEval));
  assert.equal(fullEval.steps.length, 11);
  assert.ok(docs.get("ws_ok:src/m.rs").content.includes("fn rewritten"));

  const emptyEval = await evalRustScriptWithStore(mockStore, rwPrincipal, "   ");
  assert.equal(emptyEval.steps.length, 0);
  const badArgEval = await evalRustScriptWithStore(
    mockStore,
    rwPrincipal,
    [
      'ide::data::lines("ws_ok:src/m.rs")',
      'ide::data::lines("ws_ok:src/m.rs", 5, 1)',
      'ide::ops::diff("ws_ok:src/m.rs")',
      'ide::ops::diff("ws_ok:src/m.rs", "a", "b")',
      'ide::ops::search("ws_ok:src/m.rs")',
      'ide::ops::open("ws_ok:src/m.rs")',
      'ide::ops::edit("ws_ok:src/m.rs")',
      'ide::ops::edit("ws_ok:src/m.rs", 3, 1, "x")',
      'ide::ops::insert("ws_ok:src/m.rs")',
      'ide::ops::insert("ws_ok:src/m.rs", "nan", "x")',
      'ide::ops::append("ws_ok:src/m.rs")',
      'ide::ops::delete_lines("ws_ok:src/m.rs")',
      'ide::ops::delete_lines("ws_ok:src/m.rs", 4, 1)',
      'ide::ops::replace_all("ws_ok:src/m.rs")',
      'ide::ops::coedit("ws_ok:src/m.rs")',
      'ide::ops::coedit("ws_ok:src/m.rs", "bad_rev", "x")',
      'ide::ops::unknown("x")',
    ].join("; ")
  );
  assert.equal(badArgEval.ok, false);
  const coeditWithRev = await evalRustScriptWithStore(
    mockStore,
    rwPrincipal,
    'ide::ops::coedit("ws_ok:src/m.rs", 5, "fn rev5() {}\\t\\r\\\'\\\"\\\\\\n")'
  );
  assert.equal(coeditWithRev.ok, true);
});

test("Full branch coverage for _auth.js error, ACL, payload size, and static token branches", () => {
  assert.throws(() => authenticateRequest({ headers: {} }, {}), { statusCode: 503, code: "AUTH_NOT_CONFIGURED" });

  const secret = "sec-branch";
  const badTokensEnv = {
    IDE_MCP_JWT_SECRET: secret,
    IDE_MCP_TOKENS: "{not-json",
  };
  assert.throws(() => authenticateRequest({ headers: {} }, badTokensEnv), { statusCode: 401 });
  assert.throws(() => authenticateRequest({ headers: { authorization: "Bearer   " } }, badTokensEnv), {
    statusCode: 401,
  });
  assert.throws(() => authenticateRequest({ headers: { authorization: "Bearer a.b.c" } }, badTokensEnv), {
    statusCode: 401,
  });

  // Unsupported JWT alg
  const badAlg = Buffer.from(JSON.stringify({ alg: "none", typ: "JWT" })).toString("base64url") +
    "." +
    Buffer.from(JSON.stringify({ sub: "u1", tenant_id: "t1", exp: 9999999999 })).toString("base64url") +
    ".sig";
  assert.throws(
    () => authenticateRequest({ headers: { authorization: `Bearer ${badAlg}` } }, { IDE_MCP_JWT_SECRET: secret }),
    { statusCode: 401, code: "INVALID_TOKEN" }
  );

  // Valid shape but wrong secret signature & missing sub/tenant
  const wrongJwt = signJwt({ sub: "u1", tenant_id: "t1", workspaces: ["*"], scopes: ["read"] }, "other", 60);
  assert.throws(
    () => authenticateRequest({ headers: { authorization: `Bearer ${wrongJwt}` } }, { IDE_MCP_JWT_SECRET: secret }),
    { statusCode: 401, code: "INVALID_TOKEN" }
  );
  const missingSub = signJwt({ tenant_id: "t1", workspaces: ["*"], scopes: ["read"] }, secret, 60);
  assert.throws(
    () => authenticateRequest({ headers: { authorization: `Bearer ${missingSub}` } }, { IDE_MCP_JWT_SECRET: secret }),
    { statusCode: 401, code: "INVALID_TOKEN" }
  );

  // nbf in future, missing workspaces, missing scopes
  const futureNbf = signJwt(
    { sub: "u1", tenant_id: "t1", nbf: Math.floor(Date.now() / 1000) + 600, workspaces: ["*"], scopes: ["read"] },
    secret,
    1200
  );
  assert.throws(
    () => authenticateRequest({ headers: { authorization: `Bearer ${futureNbf}` } }, { IDE_MCP_JWT_SECRET: secret }),
    { statusCode: 401, code: "TOKEN_NOT_YET_VALID" }
  );
  const noWs = signJwt({ sub: "u1", tenant_id: "t1", scopes: ["read"] }, secret, 60);
  assert.throws(
    () => authenticateRequest({ headers: { authorization: `Bearer ${noWs}` } }, { IDE_MCP_JWT_SECRET: secret }),
    { statusCode: 403, code: "MISSING_WORKSPACE_CLAIM" }
  );
  const noScopes = signJwt({ sub: "u1", tenant_id: "t1", workspaces: ["ws"] }, secret, 60);
  assert.throws(
    () => authenticateRequest({ headers: { authorization: `Bearer ${noScopes}` } }, { IDE_MCP_JWT_SECRET: secret }),
    { statusCode: 403, code: "MISSING_SCOPE_CLAIM" }
  );

  // Static expired token & missing claims
  const staticEnv = {
    IDE_MCP_TOKENS: JSON.stringify([
      { token: "expired-tok", actor_id: "a", tenant_id: "t1", expires_at: "2020-01-01T00:00:00Z", workspaces: ["w"], scopes: ["read"] },
      { token: "noclaims-tok", actor_id: "a", tenant_id: "t1", workspaces: [], scopes: [] },
    ]),
  };
  assert.throws(
    () => authenticateRequest({ headers: { authorization: "Bearer expired-tok" } }, staticEnv),
    { statusCode: 401, code: "TOKEN_EXPIRED" }
  );
  assert.throws(
    () => authenticateRequest({ headers: { authorization: "Bearer noclaims-tok" } }, staticEnv),
    { statusCode: 403, code: "MISSING_TOKEN_CLAIMS" }
  );

  // authorizeScope branches (null principal, file ACL prefix allow & deny)
  assert.throws(() => authorizeScope(null, "ws", "read"), { statusCode: 401 });
  const restricted = {
    actorId: "u2",
    tenantId: "t1",
    workspaces: ["ws"],
    files: ["src/*"],
    scopes: ["read", "write"],
  };
  authorizeScope(restricted, "ws", "read", "src/main.rs");
  assert.throws(() => authorizeScope(restricted, "ws", "read", "secrets/env.json"), {
    statusCode: 403,
    code: "FORBIDDEN_FILE",
  });

  // resolveFileTarget edge cases + enforcePayloadSize string/Buffer
  assert.throws(() => resolveFileTarget("", restricted), { statusCode: 400 });
  assert.throws(() => resolveFileTarget("a\0b", restricted), { statusCode: 400 });
  assert.throws(() => resolveFileTarget("ws:" + "x".repeat(520), restricted), { statusCode: 400 });
  const bigStr = "x".repeat(3 * 1024 * 1024);
  assert.throws(() => enforcePayloadSize({ headers: {}, body: bigStr }), { statusCode: 413 });
  assert.throws(() => enforcePayloadSize({ headers: {}, body: Buffer.alloc(3 * 1024 * 1024) }), { statusCode: 413 });
});

test("Full branch coverage for _merge.js diff, threeWayMerge, syntax check, and NEMA semantic merge", () => {
  assert.equal(formatUnifiedDiff("same\n", "same\n", "a", "b"), "");
  const multi = formatUnifiedDiff(
    "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n",
    "1\n2b\n3\n4\n5\n6\n7\n8\n9b\n10\n",
    "old",
    "new",
    1
  );
  assert.ok(multi.includes("-2\n+2b"));
  assert.ok(multi.includes("-9\n+9b"));

  const sameOut = threeWayMerge("x\n", "y\n", "y\n");
  assert.equal(sameOut.clean, true);
  assert.equal(sameOut.conflicts, 0);
  assert.equal(sameOut.text, "y\n");
  assert.equal(threeWayMerge("a\nb\n", "a\nb\nc\n", "a\nb\n").clean, true);

  assert.equal(checkSyntaxIntegrity("json", '{"ok":true,"arr":[1,2]}').ok, true);
  assert.equal(checkSyntaxIntegrity("json", '{"bad": [}').ok, false);
  assert.equal(checkSyntaxIntegrity("javascript", "function f() { return '/*'; }\n").ok, true);
  assert.equal(checkSyntaxIntegrity("javascript", "function bad() { } }\n").ok, false);

  const badOurs = nemaSemanticMerge("\t|T1 1\n\\%2\n\\# A B\n1 2\n\n\n", "invalid\n", "\t|T1 1\n\\%2\n\\# A B\n1 2\n\n\n");
  assert.equal(badOurs.clean, false);
  const badTheirs = nemaSemanticMerge("\t|T1 1\n\\%2\n\\# A B\n1 2\n\n\n", "\t|T1 1\n\\%2\n\\# A B\n1 2\n\n\n", "invalid\n");
  assert.equal(badTheirs.clean, false);
});

test("Full HTTP & JSON-RPC tool lifecycle coverage for api/v1/mcp.js", async () => {
  const { StoreError, default: defaultHandler } = await import("../api/v1/_state.js").then(async (s) => ({
    StoreError: s.StoreError,
    default: (await import("../api/v1/mcp.js")).default,
  }));

  const makeRes = () => {
    const out = { status: 0, headers: {}, body: null };
    return {
      out,
      setHeader(k, v) { out.headers[k.toLowerCase()] = v; },
      status(code) {
        out.status = code;
        return {
          json(obj) { out.body = obj; return out; },
          end() { return out; },
        };
      },
    };
  };

  const files = new Map();
  const proposals = new Map();
  const store = {
    async enforceRateLimit() {},
    async openFile({ workspaceId, filePath, fileId, content, language }) {
      files.set(fileId, { file_id: fileId, workspace_id: workspaceId, filePath, content, language, revision: 1 });
      return { ok: true, file_id: fileId, revision: 1 };
    },
    async readFile({ fileId }) {
      const f = files.get(fileId);
      if (!f) throw new StoreError(404, "NOT_FOUND", "missing file");
      return f;
    },
    async coedit({ fileId, newText }) {
      const f = files.get(fileId);
      f.content = newText;
      f.revision += 1;
      return { ok: true, revision: f.revision, content: f.content };
    },
    async editLines({ fileId, replacement }) {
      const f = files.get(fileId);
      f.content = replacement;
      f.revision += 1;
      return { ok: true, revision: f.revision };
    },
    async replaceAll({ fileId, find, replace }) {
      const f = files.get(fileId);
      f.content = f.content.split(find).join(replace);
      f.revision += 1;
      return { ok: true, revision: f.revision };
    },
    async fileHistory({ fileId }) {
      return { file_id: fileId, revisions: [{ revision: 1 }] };
    },
    async listFiles() {
      return Array.from(files.values());
    },
    async listProposals() {
      return Array.from(proposals.values());
    },
    async resolveProposal({ proposalId, resolvedContent }) {
      proposals.set(proposalId, { id: proposalId, status: "resolved", resolvedContent });
      return { ok: true, proposal_id: proposalId };
    },
    async acquireLease({ fileId, agentId, ttlMs }) {
      return { ok: true, file_id: fileId, agent_id: agentId, ttl_ms: ttlMs };
    },
    async diffRevisions({ fileId, revA, revB }) {
      return { file_id: fileId, rev_a: revA, rev_b: revB, diff: "+1" };
    },
    async evalRustScript(principal, script, agentId) {
      return evalRustScriptWithStore(store, principal, script, agentId || "mcp");
    },
  };

  const env = { IDE_MCP_JWT_SECRET: "sec-mcp" };
  const adminJwt = signJwt({ sub: "admin1", tenant_id: "t1", workspaces: ["ws"], scopes: ["*"] }, "sec-mcp", 120);
  const authHeaders = { authorization: `Bearer ${adminJwt}`, "mcp-session-id": "sess-1" };

  assert.equal((await handleMcpRequest({ method: "OPTIONS", headers: {} }, makeRes(), { store, env })).status, 200);
  assert.equal((await handleMcpRequest({ method: "DELETE", headers: {} }, makeRes(), { store, env })).status, 204);
  assert.equal(
    (await handleMcpRequest({ method: "GET", headers: { accept: "text/html" } }, makeRes(), { store, env })).status,
    302
  );
  assert.equal((await handleMcpRequest({ method: "GET", headers: {} }, makeRes(), { store, env })).status, 200);
  assert.equal((await handleMcpRequest({ method: "PUT", headers: {} }, makeRes(), { store, env })).status, 405);
  assert.equal((await handleMcpRequest({ method: "POST", headers: {}, body: "{bad" }, makeRes(), { store, env })).status, 400);
  assert.equal((await handleMcpRequest({ method: "POST", headers: {}, body: {} }, makeRes(), { store, env })).status, 400);
  assert.equal(
    (await handleMcpRequest({ method: "POST", headers: {}, body: { method: "notifications/initialized" } }, makeRes(), { store, env })).status,
    202
  );
  assert.equal(
    (await handleMcpRequest({ method: "POST", headers: {}, body: { id: 1, method: "ping" } }, makeRes(), { store, env })).status,
    200
  );
  assert.equal(
    (await handleMcpRequest({ method: "POST", headers: {}, body: { id: 2, method: "tools/list" } }, makeRes(), { store, env })).status,
    200
  );
  assert.equal(
    (await handleMcpRequest({ method: "POST", headers: {}, body: { id: 3, method: "prompts/list" } }, makeRes(), { store, env })).status,
    200
  );

  // Authenticated tools & resources
  const rpc = (method, params) =>
    handleMcpRequest({ method: "POST", headers: authHeaders, body: JSON.stringify({ jsonrpc: "2.0", id: 9, method, params }) }, makeRes(), {
      store,
      env,
    });

  await rpc("tools/call", { name: "ide_open_file", arguments: { file_id: "ws:src/a.rs", content: "fn a() {}\n", language: "rust" } });
  await rpc("tools/call", { name: "headless_read", arguments: { file_id: "ws:src/a.rs" } });
  await rpc("tools/call", { name: "headless_coedit", arguments: { file_id: "ws:src/a.rs", agent_id: "bot", new_text: "fn a() { 1 }\n" } });
  await rpc("tools/call", { name: "headless_edit", arguments: { file_id: "ws:src/a.rs", agent_id: "bot", start_line: 1, end_line: 1, replacement: "fn a() { 2 }\n" } });
  await rpc("tools/call", { name: "ide_replace_all", arguments: { file_id: "ws:src/a.rs", agent_id: "bot", find: "2", replace: "3" } });
  await rpc("tools/call", { name: "headless_three_way_merge", arguments: { base: "a\n", ours: "a\nb\n", theirs: "a\n" } });
  await rpc("tools/call", { name: "headless_history", arguments: { file_id: "ws:src/a.rs" } });
  await rpc("tools/call", { name: "headless_list_files", arguments: {} });
  await rpc("tools/call", { name: "headless_list_proposals", arguments: {} });
  await rpc("tools/call", { name: "headless_list_proposals", arguments: { file_id: "ws:src/a.rs" } });
  await rpc("tools/call", { name: "headless_resolve_proposal", arguments: { agent_id: "bot", proposal_id: "p1", resolved_content: "ok" } });
  await rpc("tools/call", { name: "headless_acquire_lease", arguments: { file_id: "ws:src/a.rs", agent_id: "bot" } });
  await rpc("tools/call", { name: "diff_unified", arguments: { old_text: "a\n", new_text: "b\n" } });
  await rpc("tools/call", { name: "diff_unified", arguments: { old_text: "same\n", new_text: "same\n" } });
  await rpc("tools/call", { name: "diff_revisions", arguments: { file_id: "ws:src/a.rs", from_revision: 1, to_revision: 2 } });
  await rpc("tools/call", { name: "ide_rust_eval", arguments: { script: "ide::context::status()" } });

  const listRes = await rpc("resources/list", {});
  assert.equal(listRes.status, 200);
  const readAll = await rpc("resources/read", { uri: "ide10://files" });
  assert.equal(readAll.status, 200);
  const readOne = await rpc("resources/read", { uri: "ide10://file/ws:src/a.rs" });
  assert.equal(readOne.status, 200);

  // Error branches in mcp.js
  assert.equal((await rpc("tools/call", {})).body.error.code, -32602);
  assert.equal((await rpc("tools/call", { name: "nonexistent_tool", arguments: {} })).body.error.code, -32601);
  assert.equal((await rpc("unknown/method", {})).body.error.code, -32601);
  const missActor = await rpc("tools/call", { name: "headless_coedit", arguments: { file_id: "ws:src/a.rs", new_text: "x" } });
  assert.equal(missActor.body.result.isError, true);
  const missFile = await rpc("tools/call", { name: "headless_read", arguments: { file_id: "ws:missing.rs" } });
  assert.equal(missFile.body.result.isError, true);

  // ServiceUnavailable (503) StoreError + default handler smoke
  const err503 = await handleMcpRequest(
    { method: "POST", headers: authHeaders, body: { jsonrpc: "2.0", id: 10, method: "tools/call", params: { name: "headless_list_files", arguments: {} } } },
    makeRes(),
    {
      store: {
        async enforceRateLimit() {},
        async listFiles() { throw new StoreError(503, "DB_DOWN", "unavailable"); },
      },
      env,
    }
  );
  assert.equal(err503.status, 503);
  assert.equal((await defaultHandler({ method: "OPTIONS", headers: {} }, makeRes())).status, 200);

  // Additional mcp.js uncovered branches: coedit missing new_text, resolve/list ACL 403, missing resolved_content, 413, 500
  const badCoedit = await rpc("tools/call", { name: "headless_coedit", arguments: { file_id: "ws:src/a.rs", agent_id: "bot" } });
  assert.equal(badCoedit.body.result.isError, true);
  const badResolve = await rpc("tools/call", { name: "headless_resolve_proposal", arguments: { agent_id: "bot", proposal_id: "p1" } });
  assert.equal(badResolve.body.result.isError, true);

  const roJwt = signJwt({ sub: "ro", tenant_id: "t1", workspaces: ["ws"], scopes: ["read"] }, "sec-mcp", 120);
  const woJwt = signJwt({ sub: "wo", tenant_id: "t1", workspaces: ["ws"], scopes: ["write"] }, "sec-mcp", 120);
  const roResolve = await handleMcpRequest(
    {
      method: "POST",
      headers: { authorization: `Bearer ${roJwt}` },
      body: { jsonrpc: "2.0", id: 11, method: "tools/call", params: { name: "headless_resolve_proposal", arguments: { agent_id: "b", proposal_id: "p1", resolved_content: "x" } } },
    },
    makeRes(),
    { store, env }
  );
  assert.equal(roResolve.status, 403);
  const woList = await handleMcpRequest(
    {
      method: "POST",
      headers: { authorization: `Bearer ${woJwt}` },
      body: { jsonrpc: "2.0", id: 12, method: "tools/call", params: { name: "headless_list_proposals", arguments: {} } },
    },
    makeRes(),
    { store, env }
  );
  assert.equal(woList.status, 403);
  const res413 = await handleMcpRequest(
    { method: "POST", headers: { "content-length": String(5 * 1024 * 1024) }, body: {} },
    makeRes(),
    { store, env }
  );
  assert.equal(res413.status, 413);
  const res500 = await handleMcpRequest(
    {
      method: "POST",
      headers: authHeaders,
      body: { jsonrpc: "2.0", id: 13, method: "tools/call", params: { name: "headless_list_files", arguments: {} } },
    },
    makeRes(),
    {
      store: {
        async enforceRateLimit() {},
        async listFiles() { throw new Error("unexpected boom"); },
      },
      env,
    }
  );
  assert.equal(res500.status, 500);

  // Additional NEMA & threeWayMerge branches in _merge.js
  assert.equal(parseNemaStrict("").ok, false);
  assert.equal(parseNemaStrict(" \\1 1\n\\%2\n\\# A B\n1  2\n\n\n").ok, false);
  assert.equal(parseNemaStrict(" \\1 1\n\\%2\n\\# A B\n1\t2\n\n\n").ok, false);
  assert.equal(parseNemaStrict("\t>G1 1\nA node X\nMissingNode>E1>A edge Y\n\n\n").ok, false);
  assert.equal(threeWayMerge("", "line1\n", "").clean, true);
  assert.equal(threeWayMerge("a\nb\n", "", "a\nb\n").clean, true);
});

test("Comprehensive PgDurableStore SQL lifecycle & rate limit / CAS / conflict / proposal resolution", async () => {
  const { PgDurableStore, sha256Hex } = await import("../api/v1/_state.js");

  const docs = new Map();
  const revisions = [];
  const proposals = [];
  const leases = new Map();
  const idems = new Map();
  const outbox = [];
  const rates = new Map();

  const keyOf = (t, w, p) => `${t}|${w}|${p}`;

  async function runSql(rawSql, params = []) {
    const sql = String(rawSql).replace(/\s+/g, " ").trim();
    if (/^(BEGIN|COMMIT|ROLLBACK|CREATE TABLE|CREATE INDEX|ALTER TABLE|INSERT INTO mcp_audit_log)/i.test(sql)) {
      return { rows: [] };
    }
    if (sql.includes("INSERT INTO mcp_rate_limits")) {
      const k = params[0];
      const next = (rates.get(k) || 0) + 1;
      rates.set(k, next);
      return { rows: [{ req_count: next }] };
    }
    if (sql.includes("INSERT INTO mcp_documents")) {
      const [t, w, p, fid, lang, content, hash] = params;
      const k = keyOf(t, w, p);
      if (docs.has(k)) return { rows: [] };
      const row = {
        tenant_id: t,
        workspace_id: w,
        file_path: p,
        file_id: fid,
        language: lang,
        current_revision: 1,
        content,
        content_hash: hash,
        next_fencing_token: 0,
        updated_at: new Date().toISOString(),
      };
      docs.set(k, row);
      return { rows: [{ ...row }] };
    }
    if (sql.includes("SELECT * FROM mcp_documents WHERE tenant_id = $1 AND workspace_id = $2 AND file_path = $3")) {
      const row = docs.get(keyOf(params[0], params[1], params[2]));
      return { rows: row ? [{ ...row }] : [] };
    }
    if (sql.includes("FROM mcp_documents WHERE tenant_id = $1 ORDER BY updated_at")) {
      const list = Array.from(docs.values()).filter((d) => d.tenant_id === params[0]);
      return { rows: list };
    }
    if (sql.includes("UPDATE mcp_documents SET next_fencing_token = next_fencing_token + 1")) {
      const row = docs.get(keyOf(params[0], params[1], params[2]));
      if (!row) return { rows: [] };
      row.next_fencing_token += 1;
      return {
        rows: [{
          next_fencing_token: row.next_fencing_token,
          current_revision: row.current_revision,
          content_hash: row.content_hash,
        }],
      };
    }
    if (sql.includes("UPDATE mcp_documents SET current_revision")) {
      const row = docs.get(keyOf(params[0], params[1], params[2]));
      if (row) {
        if (params.length >= 8 && (row.current_revision !== Number(params[6]) || row.content_hash !== params[7])) {
          return { rowCount: 0, rows: [] };
        }
        row.current_revision = Number(params[3]);
        row.content = params[4];
        row.content_hash = params[5];
        row.updated_at = new Date().toISOString();
        return { rowCount: 1, rows: [{ ...row }] };
      }
      return { rowCount: 0, rows: [] };
    }
    if (sql.includes("INSERT INTO mcp_leases")) {
      const [t, w, p, agentId, token, ttlMs] = params;
      const exp = new Date(Date.now() + Number(ttlMs)).toISOString();
      leases.set(keyOf(t, w, p), { agent_id: agentId, fencing_token: token, expires_at: exp });
      return { rows: [{ expires_at: exp }] };
    }
    if (sql.includes("SELECT agent_id, fencing_token, expires_at FROM mcp_leases")) {
      const l = leases.get(keyOf(params[0], params[1], params[2]));
      return { rows: l ? [{ ...l }] : [] };
    }
    if (sql.includes("INSERT INTO mcp_revisions")) {
      const [t, w, p] = params;
      if (params.length === 7) {
        revisions.push({ tenant_id: t, workspace_id: w, file_path: p, revision: Number(params[3]), content: params[4], content_hash: params[5], agent_id: params[6], provider: "mcp", summary: "", created_at: new Date().toISOString() });
      } else if (params.length === 6) {
        revisions.push({ tenant_id: t, workspace_id: w, file_path: p, revision: 1, content: params[3], content_hash: params[4], agent_id: params[5], provider: "mcp", summary: "Initial open", created_at: new Date().toISOString() });
      } else {
        revisions.push({ tenant_id: t, workspace_id: w, file_path: p, revision: Number(params[3]), content: params[4], content_hash: params[5], agent_id: params[6], provider: params[7], op_id: params[8], summary: params[9], created_at: new Date().toISOString() });
      }
      return { rows: [] };
    }
    if (sql.includes("SELECT revision, content, content_hash FROM mcp_revisions") && sql.includes("revision = $4")) {
      const r = revisions.find((x) => x.tenant_id === params[0] && x.workspace_id === params[1] && x.file_path === params[2] && x.revision === Number(params[3]));
      return { rows: r ? [{ ...r }] : [] };
    }
    if (sql.includes("SELECT revision, content FROM mcp_revisions") && sql.includes("revision IN")) {
      const matches = revisions.filter((x) => x.tenant_id === params[0] && x.workspace_id === params[1] && x.file_path === params[2] && (x.revision === Number(params[3]) || x.revision === Number(params[4])));
      return { rows: matches.map((r) => ({ revision: r.revision, content: r.content })) };
    }
    if (sql.includes("FROM mcp_revisions")) {
      const matches = revisions.filter((x) => x.tenant_id === params[0] && x.workspace_id === params[1] && x.file_path === params[2]);
      return { rows: matches.slice(-30).reverse() };
    }
    if (sql.includes("SELECT payload_hash, response_json FROM mcp_idempotency")) {
      const item = idems.get(`${keyOf(params[0], params[1], params[2])}|${params[3]}`);
      return { rows: item ? [{ ...item }] : [] };
    }
    if (sql.includes("INSERT INTO mcp_idempotency")) {
      idems.set(`${keyOf(params[0], params[1], params[2])}|${params[3]}`, { payload_hash: params[4], response_json: JSON.parse(params[5]) });
      return { rows: [] };
    }
    if (sql.includes("INSERT INTO mcp_proposals")) {
      proposals.push({
        proposal_id: params[0],
        tenant_id: params[1],
        workspace_id: params[2],
        file_path: params[3],
        file_id: params[4],
        agent_id: params[5],
        base_revision: Number(params[6]),
        candidate_content: params[7],
        conflict_content: params[8],
        conflict_count: Number(params[9]),
        status: "PENDING_REVIEW",
        created_at: new Date().toISOString(),
      });
      return { rows: [] };
    }
    if (sql.includes("SELECT * FROM mcp_proposals WHERE proposal_id = $1")) {
      const p = proposals.find((x) => x.proposal_id === params[0] && x.tenant_id === params[1]);
      return { rows: p ? [{ ...p }] : [] };
    }
    if (sql.includes("UPDATE mcp_proposals SET status = 'RESOLVED'")) {
      const p = proposals.find((x) => x.proposal_id === params[0]);
      if (p) { p.status = "RESOLVED"; p.resolved_by = params[1]; p.resolved_revision = params[2]; }
      return { rows: [] };
    }
    if (sql.includes("FROM mcp_proposals")) {
      return { rows: proposals.filter((x) => x.tenant_id === params[0]) };
    }
    if (sql.includes("INSERT INTO mcp_outbox")) {
      const payloadIdx = params.length >= 7 ? 6 : 5;
      const revIdx = params.length >= 7 ? 5 : 4;
      const evType = sql.includes("CONFLICT_DETECTED") ? "CONFLICT_DETECTED" : "COEDIT_COMMITTED";
      outbox.push({
        event_id: outbox.length + 1,
        tenant_id: params[0],
        workspace_id: params[1],
        file_path: params[2],
        file_id: params[3],
        event_type: evType,
        revision: Number(params[revIdx]),
        payload_json: JSON.parse(params[payloadIdx] || "{}"),
        created_at: new Date().toISOString(),
      });
      return { rows: [] };
    }
    if (sql.includes("FROM mcp_outbox")) {
      return {
        rows: outbox.filter((e) => e.tenant_id === params[0] && e.workspace_id === params[1] && e.event_id > Number(params[2])),
      };
    }
    return { rows: [] };
  }

  const pool = {
    query: (sql, params) => runSql(sql, params),
    async connect() {
      return { query: (sql, params) => runSql(sql, params), release() {} };
    },
  };

  const store = new PgDurableStore({ pool });
  const rw = { actorId: "coder", tenantId: "t1", workspaces: ["ws"], files: ["*"], scopes: ["read", "write", "force_overwrite"] };

  // enforceRateLimit ok + 429
  await store.enforceRateLimit("t1", 2);
  await store.enforceRateLimit("t1", 2);
  await assert.rejects(() => store.enforceRateLimit("t1", 2), { statusCode: 429, code: "RATE_LIMIT_EXCEEDED" });

  // openFile create, existing, missing open_existing, force_overwrite
  const created = await store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", content: "line1\nline2\nline3\n", language: "rust", mode: "create_if_absent" });
  assert.equal(created.status, "CREATED");
  const dup = await store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", content: "ignored", mode: "create_if_absent" });
  assert.equal(dup.status, "ALREADY_EXISTS");
  const ex = await store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", mode: "open_existing" });
  assert.equal(ex.status, "ALREADY_EXISTS");
  await assert.rejects(() => store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "missing.rs", fileId: "ws:missing.rs", actorId: "c1", mode: "open_existing" }), { statusCode: 404 });
  const ow = await store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", content: "alpha\nbeta\ngamma\n", language: "rust", mode: "force_overwrite" });
  assert.equal(ow.status, "OVERWRITTEN");
  assert.equal(ow.current_revision, 2);

  // readFile latest, historical revision, slice, missing
  const latest = await store.readFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", startLine: 1, endLine: 2 });
  assert.equal(latest.content, "alpha\nbeta\n");
  const rev1 = await store.readFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", revision: 1 });
  assert.equal(rev1.revision, 1);
  await assert.rejects(() => store.readFile({ tenantId: "t1", workspaceId: "ws", filePath: "missing.rs", fileId: "ws:missing.rs" }), { statusCode: 404 });
  await assert.rejects(() => store.readFile({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", revision: 99 }), { statusCode: 404 });

  // lease, coedit CAS, idempotent replay, mismatch, 3-way merge, conflict proposal & resolution
  const lease = await store.acquireLease({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", agentId: "c1", ttlMs: 30000 });
  assert.equal(lease.fencing_token, 1);
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 2, fencingToken: 999, opId: "op0", newText: "x\n" }), { statusCode: 412 });

  const c1 = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 2, baseContentHash: sha256Hex("alpha\nbeta\ngamma\n"), fencingToken: 1, opId: "op1", newText: "alpha_mod\nbeta\ngamma\n" });
  assert.equal(c1.revision, 3);
  const replay = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 2, baseContentHash: sha256Hex("alpha\nbeta\ngamma\n"), fencingToken: 1, opId: "op1", newText: "alpha_mod\nbeta\ngamma\n" });
  assert.equal(replay.idempotent_replay, true);
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 2, opId: "op1", newText: "different\n" }), { statusCode: 409, code: "IDEMPOTENCY_PAYLOAD_MISMATCH" });

  // Clean 3-way merge from stale rev 2 touching disjoint line 'gamma'
  const merged = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c2", baseRevision: 2, opId: "op2", newText: "alpha\nbeta\ngamma_mod\n" });
  assert.equal(merged.merged, true);
  assert.equal(merged.revision, 4);

  // Overlapping conflict from rev 2 on 'alpha' -> creates proposal + resolveProposal
  const conf = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c3", baseRevision: 2, opId: "op3", newText: "alpha_conflict\nbeta\ngamma\n" });
  assert.equal(conf.ok, false);
  assert.equal(conf.status, "CONFLICT");
  const props = await store.listProposals({ tenantId: "t1", principal: rw, workspaceId: "ws", fileId: "ws:a.rs" });
  assert.equal(props.length, 1);
  const resolved = await store.resolveProposal({ tenantId: "t1", principal: rw, actorId: "coder", proposalId: props[0].proposal_id, resolvedContent: "alpha_resolved\nbeta\ngamma_mod\n", baseRevision: 4, opId: "op-res" });
  assert.equal(resolved.ok, true);
  assert.equal(resolved.revision, 5);
  await assert.rejects(() => store.resolveProposal({ tenantId: "t1", principal: rw, actorId: "coder", proposalId: props[0].proposal_id, resolvedContent: "x", baseRevision: 5, opId: "op-res2" }), { statusCode: 409, code: "PROPOSAL_ALREADY_RESOLVED" });

  // editLines, replaceAll, noOp, fileHistory, listFiles, diffRevisions, getEvents
  const ed = await store.editLines({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "coder", baseRevision: 5, opId: "op-ed", startLine: 2, endLine: 2, replacement: "beta_edited" });
  assert.equal(ed.revision, 6);
  const repAll = await store.replaceAll({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "coder", baseRevision: 6, opId: "op-ra", find: "gamma_mod", replace: "gamma_final" });
  assert.equal(repAll.revision, 7);
  const noOp = await store.replaceAll({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "coder", baseRevision: 7, opId: "op-noop", find: "NOT_PRESENT", replace: "X" });
  assert.equal(noOp.status, "NO_OP");

  assert.ok((await store.listFiles(rw)).length >= 1);
  assert.ok((await store.fileHistory({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs" })).length >= 5);
  const dStr = await store.diffRevisions({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", revA: 5, revB: 7 });
  assert.ok(dStr.includes("-beta") && dStr.includes("+beta_edited"));
  assert.equal(await store.diffRevisions({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", revA: 5, revB: 5 }), "(Identical revisions: zero differences)");
  assert.ok((await store.getEvents("t1", "ws", 0)).length >= 4);

  // Additional _store.js branch coverage: missing file, invalid mode, hash mismatch, missing rev, replaceAll missing find, evalRustScript, NEMA clean merge & conflict
  await assert.rejects(() => store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "bad.rs", fileId: "ws:bad.rs", actorId: "c1", mode: "bogus" }), { statusCode: 400 });
  const owNew = await store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "fresh.rs", fileId: "ws:fresh.rs", actorId: "c1", content: "fn f() {}\n", mode: "force_overwrite" });
  assert.equal(owNew.status, "CREATED");
  await assert.rejects(() => store.fileHistory({ tenantId: "t1", workspaceId: "ws", filePath: "nope.rs", fileId: "ws:nope.rs" }), { statusCode: 404 });
  await assert.rejects(() => store.diffRevisions({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", revA: 1, revB: 999 }), { statusCode: 404 });
  await assert.rejects(() => store.acquireLease({ tenantId: "t1", workspaceId: "ws", filePath: "nope.rs", fileId: "ws:nope.rs", agentId: "c1" }), { statusCode: 404 });
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "", baseRevision: 7, opId: "o1", newText: "x" }), { statusCode: 400 });
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: "nan", opId: "o2", newText: "x" }), { statusCode: 409 });
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 7, opId: "", newText: "x" }), { statusCode: 400 });
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "nope.rs", fileId: "ws:nope.rs", actorId: "c1", baseRevision: 1, opId: "o3", newText: "x" }), { statusCode: 404 });
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 999, opId: "o4", newText: "x" }), { statusCode: 409 });
  await assert.rejects(() => store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 7, baseContentHash: "wrong-hash", opId: "o5", newText: "x" }), { statusCode: 409, code: "BASE_HASH_MISMATCH" });
  await assert.rejects(() => store.replaceAll({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "c1", baseRevision: 7, opId: "o6", find: "", replace: "x" }), { statusCode: 400 });
  await assert.rejects(() => store.resolveProposal({ tenantId: "t1", principal: rw, actorId: "c1", proposalId: "no-prop", resolvedContent: "x", baseRevision: 7, opId: "o7" }), { statusCode: 404 });
  const evalThroughStore = await store.evalRustScript(rw, 'ide::context::status()', "bot1");
  assert.equal(evalThroughStore.ok, true);

  // NEMA syntax & merge branches in PgDurableStore
  await store.openFile({ tenantId: "t1", workspaceId: "ws", filePath: "n.nema", fileId: "ws:n.nema", actorId: "c1", content: " \\1 1\n\\%2\n\\# K V\nA 1\n\n\n", language: "nema", mode: "create_if_absent" });
  const nemaCas = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "n.nema", fileId: "ws:n.nema", actorId: "c1", baseRevision: 1, opId: "op-nema-1", newText: " \\1 1\n\\%2\n\\# K V\nA 1\nB 2\n\n\n" });
  assert.equal(nemaCas.revision, 2);
  const nemaMerge = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "n.nema", fileId: "ws:n.nema", actorId: "c2", baseRevision: 1, opId: "op-nema-2", newText: " \\1 1\n\\%2\n\\# K V\nA 1\nC 3\n\n\n" });
  assert.equal(nemaMerge.merged, true);
  const nemaConflict = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "n.nema", fileId: "ws:n.nema", actorId: "c1", baseRevision: 1, opId: "op-nema-conf", newText: " \\1 1\n\\%2\n\\# K V\nA 99\n\n\n", mode: "nema" });
  // NEMA table row modification at rev 1 vs rev 3
  assert.ok(nemaConflict.revision >= 3 || nemaConflict.status === "CONFLICT");
  const nemaBad = await store.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "n.nema", fileId: "ws:n.nema", actorId: "c1", baseRevision: 3, opId: "op-nema-bad", newText: " \\1 1\n\\%2\n\\# K V\nA  1\n\n\n", mode: "nema" });
  assert.equal(nemaBad.ok, false);
  assert.equal(nemaBad.status, "REJECTED_NEMA_SYNTAX_ERROR");

  // Additional mcp.js parameter alias & 429 branches
  const { StoreError } = await import("../api/v1/_state.js");
  const jwt = signJwt({ sub: "m1", tenant_id: "t1", workspaces: ["ws"], scopes: ["*"] }, "sec-mcp", 120);
  const h = { authorization: `Bearer ${jwt}` };
  const mkRes = () => {
    const out = { status: 0, headers: {}, body: null };
    return { setHeader() {}, status(s) { out.status = s; return { json(b) { out.body = b; return out; }, end() { return out; } }; } };
  };
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 20, method: "notifications/cancelled" } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 21, method: "resources/read", params: { uri: "file:///workspace/ws:a.rs" } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 22, method: "tools/call", params: { name: "diff_unified", arguments: { text_a: "1\n", text_b: "2\n" } } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 23, method: "tools/call", params: { name: "diff_revisions", arguments: { file_id: "ws:a.rs", rev_a: 5, rev_b: 7 } } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 25, method: "tools/call", params: { name: "ide_open_file", arguments: { file_id: "ws:a.rs", mode: "open_existing" } } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 26, method: "tools/call", params: { name: "ide_open_file", arguments: { file_id: "ws:a.rs", author: "a2", mode: "force_overwrite", content: "v8\n", language: "rust" } } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 27, method: "tools/call", params: { name: "ide_edit_lines", arguments: { file_id: "ws:a.rs", author: "a2", base_revision: 8, op_id: "mcp-ed", start_line: 1, end_line: 1, replacement: "v9\n" } } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { jsonrpc: "2.0", id: 28, method: "tools/call", params: { name: "ide_rust_eval", arguments: { script: "ide::context::help()", author: "a2" } } } }, mkRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  const r429 = await handleMcpRequest(
    { method: "POST", headers: h, body: { jsonrpc: "2.0", id: 24, method: "tools/call", params: { name: "headless_list_files", arguments: {} } } },
    mkRes(),
    { store: { async enforceRateLimit() { throw new StoreError(429, "RATE_LIMIT", "slow"); } }, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } }
  );
  assert.equal(r429.status, 429);

  // Additional _merge.js NEMA & syntax check branches
  assert.equal(parseNemaStrict("\t>G1 1\nA node X\nB node Y\nA>E1>B edge Z\nA>E1>B edge Dup\n\n\n").ok, false);
  assert.equal(parseNemaStrict("\t|T1 1\n\\%2\n\\# A B\n1\n\n\n").ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "/* block { comment */ pub fn ok() { let s = \"a\\\"b\"; }").ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "pub fn unclosed(").ok, false);

  // Additional branch sweeps for mcp.js & _store.js optionals / defaults
  const noHdrRes = () => {
    const out = { status: 0, body: null };
    return { setHeader() {}, status(s) { out.status = s; return { json(b) { out.body = b; return out; }, end() { return out; } }; } };
  };
  await handleMcpRequest({ method: "GET", headers: { accept: "application/json" } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "ping" } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "initialize" } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/list" } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "prompts/list" } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/call", params: { name: "ide_open_file", arguments: { file_id: "ws:opt.txt" } } } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/call", params: { name: "headless_read", arguments: { file_id: "ws:opt.txt" } } } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/call", params: { name: "diff_unified", arguments: { old_text: "x\n", new_text: "y\n", label_a: "a", label_b: "b", context_lines: 2 } } } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/call", params: { name: "headless_acquire_lease", arguments: { file_id: "ws:opt.txt", author: "b2", ttl_ms: 5000 } } } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/call", params: { name: "headless_coedit", arguments: { file_id: "ws:opt.txt", author: "b2", base_revision: 1, op_id: "o-opt-1", new_text: "hello\nworld\n", mode: "plain" } } } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await handleMcpRequest({ method: "POST", headers: h, body: { method: "tools/call", params: { name: "ide_replace_all", arguments: { file_id: "ws:opt.txt", author: "b2", base_revision: 2, op_id: "o-opt-2", find: "world", replace: "rust" } } } }, noHdrRes(), { store, env: { IDE_MCP_JWT_SECRET: "sec-mcp" } });
  await store.editLines({ tenantId: "t1", workspaceId: "ws", filePath: "opt.txt", fileId: "ws:opt.txt", actorId: "b2", baseRevision: 3, opId: "o-opt-3", startLine: 2, endLine: 1, replacement: "inserted_line" });
});

test("Extra branch sweep for 90%+ Node branch coverage across _merge.js, _rust_eval.js, _auth.js, and mcp.js", async () => {
  // _merge.js: NEMA delete-vs-modify, both-add-domain, and header conflict
  const baseT = "\t|T1 1\n\\%2\n\\# K V\nAlpha 1\nBeta 2\n\n\n";
  const delT = "\t|T1 1\n\\%2\n\\# K V\nAlpha 1\n\n\n";
  const modBeta = "\t|T1 1\n\\%2\n\\# K V\nAlpha 1\nBeta 22\n\n\n";
  assert.equal(nemaSemanticMerge(baseT, modBeta, delT).clean, false);
  assert.equal(nemaSemanticMerge(baseT, delT, modBeta).clean, false);
  const addDomA = "\t|T1 1\n\\%2\n\\# K V\nAlpha 1\nBeta 2\n\t?W1 1\nTaskA how who when where why\n\n\n";
  const addDomB = "\t|T1 1\n\\%2\n\\# K V\nAlpha 1\nBeta 2\n\t?W2 1\nTaskB how who when where why\n\n\n";
  const addDomAConf = "\t|T1 1\n\\%2\n\\# K V\nAlpha 1\nBeta 2\n\t?W1 1\nTaskA diff who when where why\n\n\n";
  assert.equal(nemaSemanticMerge(baseT, addDomA, addDomB).clean, true);
  assert.ok(nemaSemanticMerge(baseT, addDomA, addDomAConf).text.length > 0);

  // _merge.js: CRLF input in diff/merge, Python/plain syntax check, escaped strings in JS/Rust
  assert.equal(formatUnifiedDiff("a\r\nb\r\n", "a\r\nc\r\n").includes("-b"), true);
  assert.equal(checkSyntaxIntegrity("python", "def f(x):\n    return (x + 1)\n").ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "pub fn s() { let c = '\\''; let x = [1, 2]; }").ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "pub fn bad() { let a = [1; }").ok, false);

  // _rust_eval.js: single quotes, escaped quotes, trailing semicolon & comments, missing args
  const rawDocs = new Map([
    ["ws:f.rs", { file_id: "ws:f.rs", workspace_id: "ws", filePath: "f.rs", language: "rust", content: "l1\nl2\n", current_revision: 1, content_hash: "h1" }],
    ["ws:notrail.rs", { file_id: "ws:notrail.rs", workspace_id: "ws", filePath: "notrail.rs", language: "rust", content: "abc", current_revision: 1, content_hash: "h2" }],
  ]);
  const miniStore = {
    async enforceRateLimit() {},
    async listFiles() { return Array.from(rawDocs.values()); },
    async readFile({ fileId }) {
      const doc = rawDocs.get(fileId);
      if (!doc) throw new StoreError(404, "FILE_NOT_FOUND", `Missing ${fileId}`);
      return { ...doc, total_lines: splitLines(doc.content).length };
    },
    async openFile({ fileId, content, language }) {
      rawDocs.set(fileId, { file_id: fileId, workspace_id: "ws", filePath: fileId.split(":")[1] || fileId, language: language || "plain", content: content || "", current_revision: 1, content_hash: "h1" });
      return { ok: true, current_revision: 1 };
    },
    async editLines({ fileId, replacement }) {
      if (fileId === "ws:conflict.rs") return { ok: false, status: "CONFLICT", head_revision: 7, proposal_id: "prop_7" };
      const d = rawDocs.get(fileId);
      d.content = replacement;
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: "hnew" };
    },
    async coedit({ fileId, newText }) {
      if (fileId === "ws:conflict.rs") return { ok: false, status: "CONFLICT", head_revision: 7, proposal_id: "prop_7" };
      const d = rawDocs.get(fileId);
      d.content = newText;
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: "hnew" };
    },
    async replaceAll({ fileId }) {
      const d = rawDocs.get(fileId);
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, replacements: 1, content_hash: "hnew" };
    },
    async fileHistory() { return [{ revision: 1 }]; },
    async diffRevisions() { return ""; },
    async listProposals() { return [{ proposal_id: "p1", status: "PENDING_REVIEW" }]; },
    async resolveProposal() { return { ok: true, status: "COMMITTED", revision: 9 }; },
    async acquireLease({ agentId }) { return { ok: true, agent_id: agentId, fencing_token: 3 }; },
    async evalRustScript(principalArg, scriptArg, agentArg) {
      return await evalRustScriptWithStore(this, principalArg, scriptArg, agentArg);
    },
  };
  const p = { actorId: "a", tenantId: "t", workspaces: ["ws"], files: ["*"], scopes: ["read", "write"] };
  assert.equal(splitStatements("ide::context::help();\n// trailing comment\n").length, 1);
  assert.deepEqual(parseArgs("'a', \"b\\\"c\", 42"), ["a", 'b"c', "42"]);
  assert.deepEqual(parseArgs("'x\\r\\t\\\\\\'\\q', "), ["x\r\t\\'q", ""]);
  const multi = await evalRustScriptWithStore(miniStore, p, 'ide::ops::help();\nide::data::docs()');
  assert.equal(multi.ok, true);

  // mcp.js: wildcard workspace '*', explicit revision & base_content_hash & fencing_token passthrough
  const starJwt = signJwt({ sub: "star", tenant_id: "t", workspaces: ["*"], scopes: ["read", "write"], default_workspace: "ws" }, "sec-star", 60);
  const resOut = () => {
    const out = { status: 0, headers: {}, body: null };
    return {
      setHeader(k, v) { out.headers[k] = v; },
      status(s) {
        out.status = s;
        return { json(b) { out.body = b; return out; }, end() { return out; } };
      },
    };
  };
  const callStar = (method, params, extraReq = {}) =>
    handleMcpRequest(
      { method: "POST", headers: { authorization: `Bearer ${starJwt}` }, body: { jsonrpc: "2.0", id: 1, method, params }, ...extraReq },
      resOut(),
      { store: miniStore, env: { IDE_MCP_JWT_SECRET: "sec-star" } }
    );
  assert.equal((await callStar("tools/call", { name: "headless_list_files", arguments: {} })).status, 200);
  assert.equal((await callStar("tools/call", { name: "headless_read", arguments: { file_id: "ws:f.rs", revision: 1, start_line: 1, end_line: 1 } })).status, 200);
  assert.equal((await callStar("tools/call", { name: "headless_coedit", arguments: { file_id: "ws:f.rs", author: "s1", base_revision: 1, base_content_hash: "h1", fencing_token: 0, op_id: "x1", new_text: "new\n", mode: "git" } })).status, 200);
  assert.equal((await callStar("tools/call", { name: "headless_edit", arguments: { file_id: "ws:f.rs", author: "s1", base_revision: 2, base_content_hash: "h1", fencing_token: 0, op_id: "x2", start_line: 1, end_line: 1, replacement: "r\n" } })).status, 200);
  assert.equal((await callStar("tools/call", { name: "ide_replace_all", arguments: { file_id: "ws:f.rs", author: "s1", base_revision: 3, base_content_hash: "h1", fencing_token: 0, op_id: "x3", find: "r", replace: "q" } })).status, 200);
});

test("Branch coverage completion across _merge.js, _rust_eval.js, _auth.js, _store.js, and mcp.js", async () => {
  // 1) _merge.js: edge cases in Myers, coarse fallback, delimiter checks, strict NEMA parser, semantic merge
  assert.equal(diffLines("", "").steps.length, 0);
  assert.equal(diffLines("", "a\n").steps[0].op, "ins");
  assert.equal(diffLines("a\n", "").steps[0].op, "del");
  const bigA = Array.from({ length: 1600 }, (_, i) => `A_${i}\n`).join("");
  const bigB = Array.from({ length: 1600 }, (_, i) => `B_${i}\n`).join("");
  assert.equal(diffLines(bigA, bigB).coarse, true);
  assert.ok(formatUnifiedDiff(bigA, bigB).includes("@@"));
  assert.equal(threeWayMerge("a\nb\n", "a\nb\n", "a\nb\n").clean, true);
  assert.equal(threeWayMerge("1\n2\n3\n4\n", "1\n2a\n3\n4\n", "1\n2b\n3\n4b\n").clean, false);

  assert.equal(checkSyntaxIntegrity("json", '{"a":1}').ok, true);
  assert.equal(checkSyntaxIntegrity("json", "{bad").ok, false);
  assert.equal(checkSyntaxIntegrity("js", "const s = `hi \\${1}` // end").ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "/* outer /* inner \n */ outer */ fn ok() { let r = r#\"q\"#; }").ok, true);
  assert.equal(checkSyntaxIntegrity("rust", "/* unclosed").ok, false);
  assert.equal(checkSyntaxIntegrity("rust", "fn unclosedRaw() { let s = r#\"abc\"; }").ok, true);
  assert.equal(checkSyntaxIntegrity("js", "fun())").ok, false);
  assert.equal(checkSyntaxIntegrity("js", "({)}").ok, false);
  assert.equal(checkSyntaxIntegrity("js", "fn x() {").ok, false);
  assert.equal(checkSyntaxIntegrity("js", "const bad = \"unterminated").ok, false);
  assert.equal(checkSyntaxIntegrity("rust", "<<<<<<< HEAD").ok, false);
  assert.equal(checkSyntaxIntegrity("rust", ">>>>>>> branch").ok, false);

  // NEMA validator error & dialect paths
  const badNemas = [
    "",
    "\t|T1 1\r\nRow 1\n\n\n",
    "\t|T1 1\nRow 1\n",
    "\t|T1 1\nRow 1\n\n\n\n",
    "\n\n\n",
    "\t|T1 1\nRow 1\n\nRow 2\n\n\n",
    "\t$3000\n\t|T1 1\nRow 1\n\n\n",
    "loose body\n\n\n",
    "\t|T1 1\n\t|T2 1\nRow 1\n\n\n",
    "\t|T1 1\n\n\n",
    "\t|T1 1\nRow 1\n\t>T1 1\nN1 d\n\n\n",
    "\t|T1 1\nRow\t1\n\n\n",
    "\t|T1 1\n\\%2\nA B C\n\n\n",
    "\t?W1 1\n1 2 3 4 5 6 7\n\n\n",
    "\t|T1 1\n  cont_before_rec\n\n\n",
    "\t|T1 1\nRow 1\n   \n\n\n",
    "\t>G1 1\nA>bad edge\n\n\n",
    "\t>G1 1\n1bad desc\n\n\n",
    "\t>G1 1\nN1 a\nN1 b\n\n\n",
    "\t>G1 1\nN1 a\nN1>e1>N1 x\nN1>e1>N1 y\n\n\n",
    "\t>G1 1\nN1 a\nN1>e1>MISSING x\n\n\n",
    "\t|T1 1\nA  B\n\n\n",
    "\t|T1 1\nA \n\n\n",
    "\t|T1 0\nA\n\n\n",
    "\t|!bad\nA\n\n\n",
    "\t$1\nk1 v1\n\n\n",
  ];
  for (const doc of badNemas) {
    assert.equal(validateNemaSyntax(doc).ok, false);
  }
  assert.equal(validateNemaSyntax("\t$1\nk1 v1\n |T1 1\n\\%2\nA B\n\n\n").ok, true);
  assert.equal(validateNemaSyntax("\t\\T1 1\n\\# ColA ColB\n\n\n").ok, true);
  assert.equal(validateNemaSyntax("\tD1 1\nColA ColB\n\n\n").ok, true);
  assert.equal(validateNemaSyntax("\t>G1 1\n N1 nodeDesc\nN1>e1>N1 selfEdge\n\n\n").ok, true);

  // nemaSemanticMerge extra branches: invalid ours/theirs/base, dict edits, clean delete by ours/theirs, header edits
  const nValid = "\t$1\nk v1\n\t|T1 1\n\\%2\nA 1\nB 2\n\n\n";
  const nDictTheirs = "\t$1\nk v2\n\t|T1 1\n\\%2\nA 1\nB 2\n\n\n";
  const nDictOurs = "\t$1\nk v3\n\t|T1 1\n\\%2\nA 1\nB 2\n\n\n";
  const nDelB = "\t$1\nk v1\n\t|T1 1\n\\%2\nA 1\n\n\n";
  const nHdr2 = "\t$1\nk v1\n\t|T1 2\n\\%2\nA 1\nB 2\n\n\n";
  assert.equal(nemaSemanticMerge(nValid, "bad", nValid).clean, false);
  assert.equal(nemaSemanticMerge(nValid, nValid, "bad").clean, false);
  assert.equal(nemaSemanticMerge("invalid-base\n", nValid, nValid).clean, true);
  assert.equal(nemaSemanticMerge("x\n", "\t|T1 1\nA\n\n\n", "\t|T1 1\nB\n\n\n").clean, false);
  assert.equal(nemaSemanticMerge(nValid, nValid, nDictTheirs).clean, true);
  assert.equal(nemaSemanticMerge(nValid, nDictOurs, nDictTheirs).clean, false);
  assert.equal(nemaSemanticMerge(nValid, nDelB, nValid).clean, true);
  assert.equal(nemaSemanticMerge(nValid, nValid, nDelB).clean, true);
  assert.equal(nemaSemanticMerge(nValid, nValid, nHdr2).clean, true);
  assert.equal(nemaSemanticMerge(nValid, "\t$1\nk v1\n\t|T1 3\n\\%2\nA 1\nB 2\n\n\n", nHdr2).clean, false);

  // 2) _rust_eval.js: all error paths, no-newline insert, CONFLICT step, non-ACL thrown Error
  const docsMap = new Map([
    ["ws:f.rs", { file_id: "ws:f.rs", workspace_id: "ws", filePath: "f.rs", language: "rust", content: "alpha\nbeta\n", current_revision: 1, content_hash: "h1" }],
    ["ws:notrail.rs", { file_id: "ws:notrail.rs", workspace_id: "ws", filePath: "notrail.rs", language: "rust", content: "single", current_revision: 1, content_hash: "h2" }],
    ["ws:conflict.rs", { file_id: "ws:conflict.rs", workspace_id: "ws", filePath: "conflict.rs", language: "rust", content: "c1\n", current_revision: 7, content_hash: "h7" }],
  ]);
  const rStore = {
    async listFiles() { return Array.from(docsMap.values()); },
    async readFile({ fileId }) {
      const d = docsMap.get(fileId);
      if (!d) throw new StoreError(404, "FILE_NOT_FOUND", `Missing ${fileId}`);
      return { ...d, total_lines: splitLines(d.content).length };
    },
    async openFile({ fileId, content, language }) {
      docsMap.set(fileId, { file_id: fileId, workspace_id: "ws", filePath: fileId, language, content, current_revision: 1, content_hash: "h1" });
      return { ok: true, status: "CREATED", revision: 1, content_hash: "h1" };
    },
    async editLines({ fileId }) {
      if (fileId === "ws:conflict.rs") return { ok: false, status: "CONFLICT", head_revision: 7, proposal_id: "p_c" };
      const d = docsMap.get(fileId);
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: "h2" };
    },
    async coedit({ fileId }) {
      if (fileId === "ws:conflict.rs") return { ok: false, status: "CONFLICT", head_revision: 7, proposal_id: "p_c" };
      const d = docsMap.get(fileId);
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, content_hash: "h2" };
    },
    async replaceAll({ fileId }) {
      const d = docsMap.get(fileId);
      d.current_revision += 1;
      return { ok: true, status: "COMMITTED", revision: d.current_revision, replacements: 0, content_hash: "h2" };
    },
    async fileHistory() { return []; },
    async diffRevisions() { return "--- a\n+++ b\n"; },
    async enforceRateLimit() {},
    async listProposals() { return [{ proposal_id: "p1", status: "PENDING_REVIEW" }]; },
    async resolveProposal() { return { ok: true, status: "COMMITTED", revision: 9 }; },
    async acquireLease({ agentId }) { return { ok: true, agent_id: agentId, fencing_token: 3 }; },
    async evalRustScript(principalArg, scriptArg, agentArg) {
      return await evalRustScriptWithStore(this, principalArg, scriptArg, agentArg);
    },
  };
  const rwP = { actorId: "a1", tenantId: "t1", workspaces: ["ws"], files: ["*"], scopes: ["read", "write"] };
  const wOnly = { actorId: "w1", tenantId: "t1", workspaces: ["ws"], files: ["*"], scopes: ["write"] };
  await assert.rejects(() => evalRustScriptWithStore(rStore, null, "help()"), { statusCode: 401 });
  await assert.rejects(() => evalRustScriptWithStore(rStore, wOnly, "ide::context::status()"), { statusCode: 403 });
  const rSweep = await evalRustScriptWithStore(
    rStore,
    rwP,
    [
      'ide::data::lines("ws:f.rs")',
      'ide::data::lines("ws:f.rs", 5, 1)',
      'ide::ops::diff("ws:f.rs")',
      'ide::ops::diff("ws:f.rs", "x", "y")',
      'ide::ops::search("ws:f.rs")',
      'ide::ops::search("ws:f.rs", "alpha")',
      'ide::ops::search("ws:f.rs", "missing_needle")',
      'ide::ops::insert("ws:notrail.rs")',
      'ide::ops::insert("ws:notrail.rs", "bad", "x")',
      'ide::ops::insert("ws:notrail.rs", 0, "head")',
      'ide::ops::insert("ws:notrail.rs", 99, "tail")',
      'ide::ops::append("ws:notrail.rs")',
      'ide::ops::append("ws:notrail.rs", "end")',
      'ide::ops::delete_lines("ws:f.rs")',
      'ide::ops::delete_lines("ws:f.rs", 3, 1)',
      'ide::ops::replace_all("ws:f.rs")',
      'ide::ops::coedit("ws:f.rs")',
      'ide::ops::coedit("ws:f.rs", "bad", "x")',
      'ide::ops::coedit("ws:f.rs", 1, "updated")',
      'ide::ops::coedit("ws:conflict.rs", 7, "conflict")',
      'ide::data::read("ws:missing.rs")',
      'ide::data::read broken_call',
      'unknown::fn()',
    ].join("\n"),
    "bot"
  );
  assert.equal(rSweep.ok, false);
  assert.ok(rSweep.steps.some((s) => s.output.includes("CONFLICT (HEAD unchanged at rev 7)")));
  assert.ok(rSweep.steps.some((s) => s.output.includes("Unrecognized statement")));

  // 3) _auth.js: static token unmatched loop fallthrough, uppercase Authorization header, claims errors, ACL prefix, payload sizes
  const staticEnv = {
    IDE_MCP_TOKENS: JSON.stringify([
      { token: "good-tok", actor_id: "svc", tenant_id: "t1", workspaces: ["ws*"], files: ["src/*"], scopes: ["admin"], default_workspace: "ws1" },
      { token: "exp-tok", expires_at: "2000-01-01T00:00:00Z", workspaces: ["ws1"], scopes: ["read"] },
      { token: "noclaim-tok", workspaces: [], scopes: ["read"] },
    ]),
  };
  assert.throws(() => authenticateRequest({ headers: { Authorization: "Bearer nope" } }, staticEnv), { code: "UNAUTHORIZED" });
  assert.throws(() => authenticateRequest({ headers: { authorization: "Bearer exp-tok" } }, staticEnv), { code: "TOKEN_EXPIRED" });
  assert.throws(() => authenticateRequest({ headers: { authorization: "Bearer noclaim-tok" } }, staticEnv), { code: "MISSING_TOKEN_CLAIMS" });
  const goodP = authenticateRequest({ headers: { Authorization: "Bearer good-tok" } }, staticEnv);
  assert.equal(goodP.defaultWorkspace, "ws1");
  authorizeScope(goodP, "ws1", "write", "src/main.rs");
  assert.throws(() => authorizeScope(goodP, "ws1", "write", "secret/key.txt"), { code: "FORBIDDEN_FILE" });
  assert.equal(resolveFileTarget("lib.rs", goodP).fileId, "ws1:lib.rs");
  assert.throws(() => resolveFileTarget("", goodP), { code: "INVALID_FILE_ID" });
  assert.throws(() => resolveFileTarget("bad ws:f.rs", goodP), { code: "INVALID_FILE_PATH" });
  assert.throws(() => enforcePayloadSize({ headers: { "content-length": String(2 * 1024 * 1024 + 1) } }), { code: "PAYLOAD_TOO_LARGE" });
  assert.throws(() => enforcePayloadSize({ body: "x".repeat(2 * 1024 * 1024 + 1) }), { code: "PAYLOAD_TOO_LARGE" });
  assert.throws(() => enforcePayloadSize({ body: Buffer.alloc(2 * 1024 * 1024 + 1) }), { code: "PAYLOAD_TOO_LARGE" });

  // 4) _store.js: getPool unconfigured/configured, ensureSchema reset, withTransaction rollback catch, recordAudit swallow, CAS branches
  const prevDbUrl = process.env.DATABASE_URL;
  const prevPgUrl = process.env.POSTGRES_URL;
  delete process.env.DATABASE_URL;
  delete process.env.POSTGRES_URL;
  assert.throws(() => new PgDurableStore().getPool(), { code: "STORAGE_NOT_CONFIGURED" });
  process.env.POSTGRES_URL = "postgres://user:pass@db.example.com:5432/ide";
  const cfgPool = new PgDurableStore().getPool();
  assert.ok(cfgPool);
  await cfgPool.end().catch(() => {});
  if (prevDbUrl !== undefined) process.env.DATABASE_URL = prevDbUrl; else delete process.env.DATABASE_URL;
  if (prevPgUrl !== undefined) process.env.POSTGRES_URL = prevPgUrl; else delete process.env.POSTGRES_URL;
  assert.ok(getStore());

  let schemaFailOnce = true;
  const flakeyClient = {
    async query(sql) {
      if (sql === "ROLLBACK") throw new Error("rollback failed");
      if (sql === "BEGIN" || sql === "COMMIT") return { rows: [] };
      return { rows: [] };
    },
    release() {},
  };
  const flakeyPool = {
    async query(sql) {
      if (schemaFailOnce && String(sql).includes("CREATE TABLE")) {
        schemaFailOnce = false;
        throw new Error("transient ddl err");
      }
      if (String(sql).includes("INSERT INTO mcp_audit_log")) throw new Error("audit err");
      return { rows: [] };
    },
    async connect() { return flakeyClient; },
  };
  const errStore = new PgDurableStore({ pool: flakeyPool });
  await assert.rejects(() => errStore.ensureSchema(), /transient ddl err/);
  await errStore.ensureSchema();
  await assert.rejects(() => errStore.withTransaction(async () => { throw new Error("tx boom"); }), /tx boom/);
  await errStore.recordAudit(flakeyPool, { tenantId: "t", actorId: "a", workspaceId: "w", filePath: "p", action: "x", outcome: "ok" });

  // _store.js: checkSyntaxIntegrity failure turning clean merge into conflict, missing baseRevision, rowCount!==1 CAS_CONFLICT, cross-tenant resolveProposal
  let forceBadUpdate = false;
  const miniRow = { tenant_id: "t1", workspace_id: "ws", file_path: "a.rs", file_id: "ws:a.rs", language: "rust", current_revision: 2, content: "fn a() {\n  let x = 1;\n}\n", content_hash: "h2" };
  const baseRow = { revision: 1, content: "fn a() {\n  let x = 0;\n}\n", content_hash: "h1" };
  const branchClient = {
    async query(sql, params) {
      const s = String(sql);
      if (s === "BEGIN" || s === "COMMIT" || s === "ROLLBACK") return { rows: [] };
      if (s.includes("FROM mcp_proposals WHERE proposal_id")) {
        if (params?.[0] === "p_other_tenant") return { rows: [{ proposal_id: "p_other_tenant", tenant_id: "other", workspace_id: "ws", file_path: "a.rs", file_id: "ws:a.rs", status: "PENDING_REVIEW" }] };
        return { rows: [] };
      }
      if (s.includes("FROM mcp_documents") && s.includes("FOR UPDATE")) return { rows: [miniRow] };
      if (s.includes("FROM mcp_leases")) return { rows: [] };
      if (s.includes("FROM mcp_idempotency")) return { rows: [] };
      if (s.includes("FROM mcp_revisions") && s.includes("revision = $4")) return { rows: [baseRow] };
      if (s.startsWith("UPDATE mcp_documents")) return { rowCount: forceBadUpdate ? 0 : 1, rows: [] };
      return { rows: [], rowCount: 1 };
    },
    release() {},
  };
  const bPool = { async query() { return { rows: [] }; }, async connect() { return branchClient; } };
  const bStore = new PgDurableStore({ pool: bPool });
  await assert.rejects(
    () => bStore.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "u", baseRevision: 0, opId: "o0", newText: "x" }),
    { code: "MISSING_BASE_REVISION" }
  );
  const syntaxConflict = await bStore.coedit({
    tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "u", baseRevision: 1, opId: "o1",
    newText: "fn a() {\n  let x = 0;\n// unclosed {\n", mode: "git",
  });
  assert.equal(syntaxConflict.status, "CONFLICT");
  forceBadUpdate = true;
  await assert.rejects(
    () => bStore.coedit({ tenantId: "t1", workspaceId: "ws", filePath: "a.rs", fileId: "ws:a.rs", actorId: "u", baseRevision: 2, opId: "o2", newText: "fn a() {}" }),
    { code: "CAS_CONFLICT" }
  );
  await assert.rejects(
    () => bStore.resolveProposal({ tenantId: "t1", actorId: "u", proposalId: "p_other_tenant", resolvedContent: "ok", baseRevision: 2, opId: "o3" }),
    { code: "PROPOSAL_NOT_FOUND" }
  );

  // 5) mcp.js: HTTP methods, headers, tool fallbacks, error mappings, default handler
  const mkRes = () => {
    const out = { status: 0, headers: {}, body: null };
    return {
      setHeader(k, v) { out.headers[k] = v; },
      status(s) {
        out.status = s;
        return { json(b) { out.body = b; return out; }, end() { return out; } };
      },
    };
  };
  const starJwt2 = signJwt({ sub: "u", tenant_id: "t1", workspaces: ["*"], scopes: ["read", "write"] }, "sec2", 60);
  const roJwt = signJwt({ sub: "ro", tenant_id: "t1", workspaces: ["ws"], scopes: ["read"] }, "sec2", 60);
  const woJwt = signJwt({ sub: "wo", tenant_id: "t1", workspaces: ["ws"], scopes: ["write"] }, "sec2", 60);
  const mcpEnv = { IDE_MCP_JWT_SECRET: "sec2" };
  const callM = (jwt, method, params, extra = {}) =>
    handleMcpRequest(
      { method: "POST", headers: { authorization: `Bearer ${jwt}`, "mcp-session-id": "sess-1", ...(extra.headers || {}) }, body: extra.rawBody ?? { jsonrpc: "2.0", id: 9, method, params } },
      mkRes(),
      { store: rStore, env: mcpEnv }
    );

  assert.equal((await handler({ method: "OPTIONS", headers: {} }, mkRes())).status, 200);
  assert.equal((await handleMcpRequest({ method: "DELETE", headers: {} }, mkRes(), { store: rStore, env: mcpEnv })).status, 204);
  assert.equal((await handleMcpRequest({ method: "PUT", headers: {} }, mkRes(), { store: rStore, env: mcpEnv })).status, 405);
  assert.equal((await handleMcpRequest({ method: "GET", headers: { accept: "text/html" } }, mkRes(), { store: rStore, env: mcpEnv })).status, 302);
  assert.equal((await handleMcpRequest({ method: "GET", headers: { accept: "application/json" } }, mkRes(), { store: rStore, env: mcpEnv })).status, 200);
  assert.equal((await callM(starJwt2, "", {}, { rawBody: "{bad-json" })).status, 400);
  assert.equal((await callM(starJwt2, "", {}, { rawBody: {} })).status, 400);
  assert.equal((await callM(starJwt2, "", {}, { rawBody: "x".repeat(2 * 1024 * 1024 + 1) })).status, 413);
  assert.equal((await callM(starJwt2, "notifications/cancelled", {})).status, 202);
  assert.equal((await callM(starJwt2, "initialize", {})).status, 200);
  assert.equal((await callM(starJwt2, "ping", {})).status, 200);
  assert.equal((await callM(starJwt2, "prompts/list", {})).status, 200);
  assert.equal((await callM(starJwt2, "resources/read", { uri: "ide10://files" })).status, 200);
  assert.equal((await callM(starJwt2, "resources/read", { uri: "file:///workspace/ws:f.rs" })).status, 200);
  assert.equal((await callM(starJwt2, "unknown/method", {})).body.error.code, -32601);
  assert.equal((await callM(starJwt2, "tools/call", {})).body.error.code, -32602);
  assert.equal((await callM(starJwt2, "tools/call", { name: "no_such_tool", arguments: {} })).body.error.code, -32601);

  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_open", arguments: { file_id: "ws:open.rs", mode: "open_existing" } })).status, 200);
  assert.equal((await callM(roJwt, "tools/call", { name: "headless_open", arguments: { file_id: "ws:open.rs", mode: "force_overwrite" } })).status, 403);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_coedit", arguments: { file_id: "ws:f.rs" } })).body.result.isError, true);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_coedit", arguments: { file_id: "ws:f.rs", agent_id: "a", new_text: 123 } })).body.result.isError, true);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_list_proposals", arguments: {} })).status, 200);
  assert.equal((await callM(woJwt, "tools/call", { name: "headless_list_proposals", arguments: {} })).status, 403);
  assert.equal((await callM(roJwt, "tools/call", { name: "headless_resolve_proposal", arguments: {} })).status, 403);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_resolve_proposal", arguments: { proposal_id: "p1", agent_id: "a", resolved_content: 123 } })).body.result.isError, true);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_resolve_proposal", arguments: { proposal_id: "p1", agent_id: "a", resolved_content: "ok", base_revision: 1, op_id: "op" } })).status, 200);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_acquire_lease", arguments: { file_id: "ws:f.rs", agent_id: "a" } })).status, 200);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_three_way_merge", arguments: {} })).status, 200);
  assert.equal((await callM(starJwt2, "tools/call", { name: "headless_history", arguments: { file_id: "ws:f.rs" } })).status, 200);
  assert.equal((await callM(starJwt2, "tools/call", { name: "diff_unified", arguments: { text_a: "a", text_b: "a" } })).body.result.content[0].text, "(Identical texts: zero differences)");
  assert.equal((await callM(starJwt2, "tools/call", { name: "diff_revisions", arguments: { file_id: "ws:f.rs", rev_a: 1, rev_b: 1 } })).status, 200);
  assert.equal((await callM(starJwt2, "tools/call", { name: "ide_rust_eval", arguments: { author: "a" } })).status, 200);

  // StoreError 503 & generic 500 mapping in mcp.js
  const fail503 = { async enforceRateLimit() {}, async listFiles() { throw new StoreError(503, "DB_DOWN", "down"); } };
  const fail500 = { async enforceRateLimit() {}, async listFiles() { throw new Error("unexpected bug"); } };
  assert.equal((await handleMcpRequest({ method: "POST", headers: { authorization: `Bearer ${starJwt2}` }, body: { jsonrpc: "2.0", id: 1, method: "resources/list" } }, mkRes(), { store: fail503, env: mcpEnv })).status, 503);
  assert.equal((await handleMcpRequest({ method: "POST", headers: { authorization: `Bearer ${starJwt2}` }, body: { jsonrpc: "2.0", id: 2, method: "resources/list" } }, mkRes(), { store: fail500, env: mcpEnv })).status, 500);
});
