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
  threeWayMerge,
} from "../api/v1/_merge.js";
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

test("P0-03: PgDurableStore listProposals filters by workspace/file ACL and resolveProposal enforces write auth", async () => {
  const { PgDurableStore } = await import("../api/v1/_store.js");
  const rows = [
    { proposal_id: "p1", workspace_id: "ws_ok", file_path: "src/a.rs", file_id: "ws_ok:src/a.rs", status: "PENDING_REVIEW" },
    { proposal_id: "p2", workspace_id: "ws_other", file_path: "src/b.rs", file_id: "ws_other:src/b.rs", status: "PENDING_REVIEW" },
    { proposal_id: "p3", workspace_id: "ws_ok", file_path: "src/secret.rs", file_id: "ws_ok:src/secret.rs", status: "PENDING_REVIEW" },
  ];
  const mockPool = {
    async query(sql) {
      if (String(sql).includes("FROM mcp_proposals")) return { rows };
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
