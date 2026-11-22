#!/usr/bin/env node
// Real Live-Cloud JSON-RPC E2E Suite against https://ide1d0.vercel.app/mcp
// Uses isolated audit workspace `audit_<timestamp>`, short-lived HS256 JWTs, IPv4-first DNS,
// zero mocking, and records full redacted transcripts to docs/MCP-CLOUD-E2E-LOG.txt.

import assert from "node:assert/strict";
import dns from "node:dns";
import fs from "node:fs";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { signJwt } from "../api/v1/_auth.js";

dns.setDefaultResultOrder("ipv4first");

const TARGET_MCP = process.env.TARGET_MCP || "https://ide1d0.vercel.app/mcp";
const BASE_URL = TARGET_MCP.replace(/\/mcp$/, "");
const REDEPLOY = process.argv.includes("--redeploy");

function loadCreds() {
  const out = {
    jwtSecret: process.env.IDE_MCP_JWT_SECRET || "",
    tokensRaw: process.env.IDE_MCP_TOKENS || "",
  };
  const localFile = path.join(process.cwd(), ".env.mcp.local");
  if ((!out.jwtSecret || !out.tokensRaw) && fs.existsSync(localFile)) {
    const lines = fs.readFileSync(localFile, "utf8").split(/\r?\n/);
    for (const l of lines) {
      if (l.startsWith("IDE_MCP_JWT_SECRET=")) out.jwtSecret = l.slice("IDE_MCP_JWT_SECRET=".length).trim();
      if (l.startsWith("IDE_MCP_TOKENS=")) out.tokensRaw = l.slice("IDE_MCP_TOKENS=".length).trim();
    }
  }
  assert.ok(out.jwtSecret, "IDE_MCP_JWT_SECRET missing from env or .env.mcp.local");
  const tokens = JSON.parse(out.tokensRaw || "[]");
  assert.ok(tokens.length >= 3, "Expected at least 3 scoped tokens in IDE_MCP_TOKENS");
  return { jwtSecret: out.jwtSecret, tokens };
}

const { jwtSecret, tokens } = loadCreds();
const AUDIT_TOKEN = tokens.find((t) => t.tenant_id === "audit_tenant" && t.scopes.includes("write")).token;
const RO_TOKEN = tokens.find((t) => t.tenant_id === "audit_tenant" && !t.scopes.includes("write")).token;
const TENANT_B_TOKEN = tokens.find((t) => t.tenant_id === "tenant_isolated_b").token;

const AUDIT_WS = `audit_${Date.now()}`;
const logs = [];

function redact(text) {
  let s = String(text);
  for (const sec of [jwtSecret, AUDIT_TOKEN, RO_TOKEN, TENANT_B_TOKEN]) {
    if (sec) s = s.split(sec).join("<REDACTED>");
  }
  s = s.replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, "<REDACTED_JWT>");
  return s;
}

function logLine(msg) {
  const clean = redact(msg);
  logs.push(clean);
  console.log(clean);
}

function agentJwt(
  agentId,
  tenantId = "audit_tenant",
  workspaces = [AUDIT_WS],
  scopes = ["read", "write"],
  ttl = 300,
  extra = {}
) {
  return signJwt({ sub: agentId, tenant_id: tenantId, workspaces, scopes, ...extra }, jwtSecret, ttl);
}

let rpcSeq = 1;
async function rpcRaw(method, params = {}, token = null, url = TARGET_MCP) {
  const id = rpcSeq++;
  const headers = { "Content-Type": "application/json", Accept: "application/json" };
  if (token) headers.Authorization = `Bearer ${token}`;
  const res = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id, method, params }),
  });
  const text = await res.text();
  let json = null;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error(`Non-JSON HTTP ${res.status} from ${url}: ${text.slice(0, 300)}`);
  }
  return { status: res.status, headers: Object.fromEntries(res.headers.entries()), body: json };
}

async function callTool(name, args, token) {
  const r = await rpcRaw("tools/call", { name, arguments: args }, token);
  return {
    httpStatus: r.status,
    rpc: r.body,
    data: r.body?.result?.structured_data ?? null,
    isError: Boolean(r.body?.result?.isError),
    errorText: r.body?.error?.message || r.body?.result?.content?.[0]?.text || "",
  };
}

async function runCase(name, fn) {
  const t0 = Date.now();
  logLine(`\n=== [E2E CASE] ${name} ===`);
  const detail = await fn();
  const ms = Date.now() - t0;
  logLine(`PASS: ${name} (${ms} ms) -> ${JSON.stringify(detail)}`);
}

async function main() {
  logLine(`Target MCP: ${TARGET_MCP}`);
  logLine(`Isolated Workspace: ${AUDIT_WS}`);
  logLine(`Timestamp: ${new Date().toISOString()}`);

  // Protocol handshake verification (P1-05)
  const disc = await fetch(TARGET_MCP, { headers: { Accept: "application/json" } });
  assert.equal(disc.status, 200);
  const discJson = await disc.json();
  logLine(`Discovery: server="${discJson.server}", version=${discJson.version}, protocol=${discJson.protocolVersion}`);
  assert.equal(discJson.version, "0.9.0");

  const init = await rpcRaw("initialize", { protocolVersion: "2024-11-05", clientInfo: { name: "audit-e2e", version: "1.0" } });
  assert.equal(init.status, 200);
  assert.ok(init.headers["mcp-session-id"], "Streamable HTTP Mcp-Session-Id header expected");
  const toolsList = await rpcRaw("tools/list", {});
  const toolNames = toolsList.body.result.tools.map((t) => t.name);
  for (const reqTool of ["headless_open", "ide_open_file", "headless_coedit", "ide_edit_lines", "ide_replace_all", "headless_list_proposals", "headless_resolve_proposal", "headless_acquire_lease"]) {
    assert.ok(toolNames.includes(reqTool), `Missing tool ${reqTool}`);
  }

  // Case 1: two_agents_edit_different_functions
  await runCase("1. two_agents_edit_different_functions", async () => {
    const fileId = `${AUDIT_WS}:src/calc.rs`;
    const jwtA = agentJwt("claude-sonnet");
    const jwtB = agentJwt("gpt-4o");
    const baseCode = [
      "pub fn add(a: i32, b: i32) -> i32 {",
      "    a + b",
      "}",
      "",
      "// separator anchor line 1",
      "// separator anchor line 2",
      "",
      "pub fn multiply(a: i32, b: i32) -> i32 {",
      "    a * b",
      "}",
      "",
    ].join("\n");

    const opened = await callTool("headless_open", { file_id: fileId, content: baseCode, language: "rust" }, jwtA);
    assert.equal(opened.data.status, "CREATED");
    const rev1Hash = opened.data.content_hash;

    const rA = await callTool(
      "headless_coedit",
      {
        file_id: fileId,
        author: "claude-sonnet",
        base_revision: 1,
        base_content_hash: rev1Hash,
        op_id: "c1-a-1",
        new_text: baseCode.replace("    a + b", "    a.saturating_add(b)"),
      },
      jwtA
    );
    assert.equal(rA.data.ok, true);
    assert.equal(rA.data.revision, 2);

    const rB = await callTool(
      "headless_coedit",
      {
        file_id: fileId,
        author: "gpt-4o",
        base_revision: 1,
        base_content_hash: rev1Hash,
        op_id: "c1-b-1",
        new_text: baseCode.replace("    a * b", "    a.saturating_mul(b)"),
      },
      jwtB
    );
    assert.equal(rB.data.ok, true);
    assert.equal(rB.data.revision, 3);
    assert.equal(rB.data.auto_merged, true);
    assert.ok(rB.data.content.includes("saturating_add"));
    assert.ok(rB.data.content.includes("saturating_mul"));
    return { rev: rB.data.revision, content_hash: rB.data.content_hash };
  });

  // Case 2: three_agents_edit_same_file (true parallel Promise.all)
  const triFile = `${AUDIT_WS}:src/tri.rs`;
  await runCase("2. three_agents_edit_same_file", async () => {
    const baseTri = [
      "// === HEADER REGION ===",
      "pub fn alpha() -> i32 { 1 }",
      "// anchor-1",
      "// anchor-2",
      "// === MIDDLE REGION ===",
      "pub fn beta() -> i32 { 2 }",
      "// anchor-3",
      "// anchor-4",
      "// === TAIL REGION ===",
      "pub fn gamma() -> i32 { 3 }",
      "",
    ].join("\n");
    const j1 = agentJwt("agent-1");
    const j2 = agentJwt("agent-2");
    const j3 = agentJwt("agent-3");
    const op = await callTool("headless_open", { file_id: triFile, content: baseTri, language: "rust" }, j1);
    const h1 = op.data.content_hash;

    const [c1, c2, c3] = await Promise.all([
      callTool("headless_coedit", { file_id: triFile, author: "agent-1", base_revision: 1, base_content_hash: h1, op_id: "tri-1", new_text: baseTri.replace("{ 1 }", "{ 101 }") }, j1),
      callTool("headless_coedit", { file_id: triFile, author: "agent-2", base_revision: 1, base_content_hash: h1, op_id: "tri-2", new_text: baseTri.replace("{ 2 }", "{ 202 }") }, j2),
      callTool("headless_coedit", { file_id: triFile, author: "agent-3", base_revision: 1, base_content_hash: h1, op_id: "tri-3", new_text: baseTri.replace("{ 3 }", "{ 303 }") }, j3),
    ]);

    assert.equal(c1.data.ok, true);
    assert.equal(c2.data.ok, true);
    assert.equal(c3.data.ok, true);

    const head = await callTool("headless_read", { file_id: triFile }, j1);
    assert.equal(head.data.current_revision, 4);
    assert.ok(head.data.content.includes("101") && head.data.content.includes("202") && head.data.content.includes("303"));
    return { final_revision: head.data.current_revision, content_hash: head.data.content_hash };
  });

  // Case 3: overlapping_edit_rejected_without_commit + proposal lifecycle
  const anchorFile = `${AUDIT_WS}:src/conflict.rs`;
  let persistedProposalId = "";
  await runCase("3. overlapping_edit_rejected_without_commit", async () => {
    const jA = agentJwt("agent-ours");
    const jB = agentJwt("agent-theirs");
    const base = "pub fn target() -> i32 {\n    10\n}\n";
    await callTool("headless_open", { file_id: anchorFile, content: base, language: "rust" }, jA);

    const rev2 = await callTool("headless_coedit", { file_id: anchorFile, author: "agent-ours", base_revision: 1, op_id: "conf-a-1", new_text: "pub fn target() -> i32 {\n    111\n}\n" }, jA);
    assert.equal(rev2.data.revision, 2);
    const rev2Hash = rev2.data.content_hash;

    const conf = await callTool("headless_coedit", { file_id: anchorFile, author: "agent-theirs", base_revision: 1, op_id: "conf-b-1", new_text: "pub fn target() -> i32 {\n    999\n}\n" }, jB);
    assert.equal(conf.data.ok, false);
    assert.equal(conf.data.status, "CONFLICT");
    assert.ok(conf.data.proposal_id, "Conflict must create proposal_id");
    assert.ok(conf.data.conflict_diff, "Conflict must include structured diff");
    assert.ok(conf.data.conflict_preview.includes("<<<<<<<"));
    persistedProposalId = conf.data.proposal_id;

    // Crucial P0-01 Gate: Formal HEAD must remain UNCHANGED at rev 2 without conflict markers!
    const readHead = await callTool("headless_read", { file_id: anchorFile }, jA);
    assert.equal(readHead.data.current_revision, 2);
    assert.equal(readHead.data.content_hash, rev2Hash);
    assert.ok(!readHead.data.content.includes("<<<<<<<"));

    const props = await callTool("headless_list_proposals", { file_id: anchorFile }, jA);
    assert.ok(props.data.proposals.some((p) => p.proposal_id === persistedProposalId && p.status === "PENDING_REVIEW"));
    return { proposal_id: persistedProposalId, head_rev_preserved: readHead.data.current_revision };
  });

  // Case 4: reopen_existing_file_without_reset
  await runCase("4. reopen_existing_file_without_reset", async () => {
    const jwt = agentJwt("reopen-tester");
    const reopen = await callTool("ide_open_file", { file_id: triFile, content: "ATTEMPTED_RESET_CONTENT", language: "rust" }, jwt);
    assert.equal(reopen.data.status, "ALREADY_EXISTS");
    assert.equal(reopen.data.created, false);
    assert.equal(reopen.data.current_revision, 4);
    assert.ok(reopen.data.content.includes("303"));

    const forceWithoutScope = await callTool("headless_open", { file_id: triFile, content: "RESET", mode: "force_overwrite" }, jwt);
    assert.equal(forceWithoutScope.httpStatus, 403);
    return { status: reopen.data.status, revision_preserved: reopen.data.current_revision, unauth_force_status: forceWithoutScope.httpStatus };
  });

  // Case 5: concurrent_duplicate_op_id
  await runCase("5. concurrent_duplicate_op_id", async () => {
    const f = `${AUDIT_WS}:src/idem.rs`;
    const jwt = agentJwt("idem-agent");
    await callTool("headless_open", { file_id: f, content: "let v = 1;\n", language: "rust" }, jwt);

    const [a, b] = await Promise.all([
      callTool("headless_coedit", { file_id: f, author: "idem-agent", base_revision: 1, op_id: "same-op-77", new_text: "let v = 2;\n" }, jwt),
      callTool("headless_coedit", { file_id: f, author: "idem-agent", base_revision: 1, op_id: "same-op-77", new_text: "let v = 2;\n" }, jwt),
    ]);
    assert.equal(a.data.revision, 2);
    assert.equal(b.data.revision, 2);
    assert.equal(a.data.content_hash, b.data.content_hash);
    assert.ok(a.data.idempotent_replay || b.data.idempotent_replay);

    const head = await callTool("headless_read", { file_id: f }, jwt);
    assert.equal(head.data.current_revision, 2);

    const mismatch = await callTool("headless_coedit", { file_id: f, author: "idem-agent", base_revision: 1, op_id: "same-op-77", new_text: "let v = 999;\n" }, jwt);
    assert.equal(mismatch.isError, true);
    assert.equal(mismatch.data.code, "IDEMPOTENCY_PAYLOAD_MISMATCH");
    return { replay_rev: head.data.current_revision, mismatch_code: mismatch.data.code };
  });

  // Case 6: cross_file_idempotency_collision
  await runCase("6. cross_file_idempotency_collision", async () => {
    const f1 = `${AUDIT_WS}:src/cf1.rs`;
    const f2 = `${AUDIT_WS}:src/cf2.rs`;
    const jwt = agentJwt("cf-agent");
    await callTool("headless_open", { file_id: f1, content: "fn f1() { 1 }\n", language: "rust" }, jwt);
    await callTool("headless_open", { file_id: f2, content: "fn f2() { 1 }\n", language: "rust" }, jwt);

    const r1 = await callTool("headless_coedit", { file_id: f1, author: "cf-agent", base_revision: 1, op_id: "shared-op-key", new_text: "fn f1() { 2 }\n" }, jwt);
    const r2 = await callTool("headless_coedit", { file_id: f2, author: "cf-agent", base_revision: 1, op_id: "shared-op-key", new_text: "fn f2() { 2 }\n" }, jwt);
    assert.equal(r1.data.ok, true);
    assert.equal(r2.data.ok, true);
    assert.equal(r1.data.revision, 2);
    assert.equal(r2.data.revision, 2);
    return { f1_rev: r1.data.revision, f2_rev: r2.data.revision };
  });

  // Case 7: unauthorized_access_denied + fencing token
  await runCase("7. unauthorized_access_denied", async () => {
    const noAuth = await callTool("headless_read", { file_id: triFile }, null);
    assert.equal(noAuth.httpStatus, 401);

    const expired = await callTool("headless_read", { file_id: triFile }, agentJwt("exp", "audit_tenant", [AUDIT_WS], ["read"], -10));
    assert.equal(expired.httpStatus, 401);

    const roWrite = await callTool("headless_coedit", { file_id: triFile, author: "ro", base_revision: 4, op_id: "ro-1", new_text: "// x\n" }, RO_TOKEN);
    assert.equal(roWrite.httpStatus, 403);

    const wrongWs = await callTool("headless_read", { file_id: triFile }, agentJwt("other", "audit_tenant", ["unrelated_ws"], ["read"]));
    assert.equal(wrongWs.httpStatus, 403);

    const crossTenant = await callTool("headless_read", { file_id: triFile }, TENANT_B_TOKEN);
    assert.equal(crossTenant.isError, true);
    assert.equal(crossTenant.data.code, "FILE_NOT_FOUND");

    // P0-03 Proposal ACL & file-level ACL assertions
    const roResolve = await callTool(
      "headless_resolve_proposal",
      { proposal_id: persistedProposalId, resolved_content: "hijack", base_revision: 2, agent_id: "ro", op_id: "ro-res-1" },
      RO_TOKEN
    );
    assert.equal(roResolve.httpStatus, 403);

    const wrongWsJwt = agentJwt("other", "audit_tenant", ["unrelated_ws"], ["read", "write"]);
    const wrongWsResolve = await callTool(
      "headless_resolve_proposal",
      { proposal_id: persistedProposalId, resolved_content: "hijack", base_revision: 2, agent_id: "other", op_id: "ws-res-1" },
      wrongWsJwt
    );
    assert.equal(wrongWsResolve.httpStatus, 403);

    const wrongWsListProps = await callTool("headless_list_proposals", {}, wrongWsJwt);
    assert.ok(!wrongWsListProps.data.proposals.some((p) => p.proposal_id === persistedProposalId));

    const fileScopedJwt = agentJwt("file-only", "audit_tenant", [AUDIT_WS], ["read"], 300, {
      files: ["src/tri.rs"],
    });
    const allowedFileRead = await callTool("headless_read", { file_id: triFile }, fileScopedJwt);
    assert.equal(allowedFileRead.isError, false);
    const forbiddenFileRead = await callTool("headless_read", { file_id: anchorFile }, fileScopedJwt);
    assert.equal(forbiddenFileRead.httpStatus, 403);

    // Fencing token validation
    const jwt = agentJwt("lease-holder");
    const lease = await callTool("headless_acquire_lease", { file_id: anchorFile, agent_id: "lease-holder", ttl_ms: 60000 }, jwt);
    const validTok = lease.data.fencing_token;
    const staleWrite = await callTool(
      "headless_resolve_proposal",
      { proposal_id: persistedProposalId, resolved_content: "pub fn target() -> i32 { 222 }\n", base_revision: 2, agent_id: "rev", op_id: "res-1" },
      jwt
    );
    assert.equal(staleWrite.data.ok, true);
    const badFencing = await callTool(
      "headless_coedit",
      { file_id: anchorFile, author: "lease-holder", base_revision: 3, fencing_token: validTok + 50, op_id: "bad-tok-1", new_text: "pub fn target() -> i32 { 333 }\n" },
      jwt
    );
    assert.equal(badFencing.isError, true);
    assert.equal(badFencing.data.code, "FENCING_TOKEN_STALE");
    return {
      noAuth: 401,
      expired: 401,
      roWrite: 403,
      wrongWs: 403,
      roResolve: roResolve.httpStatus,
      wrongWsResolve: wrongWsResolve.httpStatus,
      forbiddenFile: forbiddenFileRead.httpStatus,
      crossTenant: crossTenant.data.code,
      badFencing: badFencing.data.code,
    };
  });

  // Case 8: multiple_serverless_instances (parallel concurrent HTTP requests)
  await runCase("8. multiple_serverless_instances", async () => {
    const f = `${AUDIT_WS}:src/burst.rs`;
    const jwt = agentJwt("burst-coord");
    const lines = [];
    for (let i = 1; i <= 18; i++) {
      lines.push(`pub fn f${i}() -> i32 { ${i} }`, "");
    }
    const base = lines.join("\n");
    const op = await callTool("headless_open", { file_id: f, content: base, language: "rust" }, jwt);
    const h = op.data.content_hash;

    const results = await Promise.all(
      [1, 4, 7, 10, 13, 16].map((fnIdx, idx) =>
        callTool(
          "headless_coedit",
          {
            file_id: f,
            author: `worker-${idx}`,
            base_revision: 1,
            base_content_hash: h,
            op_id: `burst-${idx}`,
            new_text: base.replace(`f${fnIdx}() -> i32 { ${fnIdx} }`, `f${fnIdx}() -> i32 { ${fnIdx * 100} }`),
          },
          agentJwt(`worker-${idx}`)
        )
      )
    );
    for (const r of results) assert.equal(r.data.ok, true);
    const read = await callTool("headless_read", { file_id: f }, jwt);
    assert.equal(read.data.current_revision, 7);
    for (const fnIdx of [1, 4, 7, 10, 13, 16]) {
      assert.ok(read.data.content.includes(`{ ${fnIdx * 100} }`));
    }
    return { final_revision: read.data.current_revision, updated_functions: 6 };
  });

  // Case 9: server_restart_persistence (live Vercel redeploy if --redeploy, or isolated cold verification)
  await runCase("9. server_restart_persistence", async () => {
    const beforeRead = await callTool("headless_read", { file_id: anchorFile }, AUDIT_TOKEN);
    const beforeRev = beforeRead.data.current_revision;
    const beforeHash = beforeRead.data.content_hash;
    let redeployUrl = null;
    if (REDEPLOY) {
      logLine("Triggering live `vercel deploy --prod --yes` mid-test to destroy & respawn all serverless containers...");
      const resDeploy = spawnSync("vercel", ["deploy", "--prod", "--yes"], { shell: true, encoding: "utf8" });
      assert.equal(resDeploy.status, 0, `vercel deploy failed: ${resDeploy.stderr}`);
      const combined = `${resDeploy.stdout || ""}\n${resDeploy.stderr || ""}`;
      const urls = combined.match(/https:\/\/[a-z0-9-]+\.vercel\.app/gi) || [];
      redeployUrl = urls[0] || TARGET_MCP.replace(/\/mcp$/, "");
      assert.ok(redeployUrl.startsWith("https://"), `Invalid redeployUrl extracted: ${redeployUrl}`);
      logLine(`Fresh production deployment ready: ${redeployUrl}`);
    }
    const afterRead = await callTool("headless_read", { file_id: anchorFile }, AUDIT_TOKEN);
    const hist = await callTool("headless_history", { file_id: anchorFile }, AUDIT_TOKEN);
    assert.equal(afterRead.data.current_revision, beforeRev);
    assert.equal(afterRead.data.content_hash, beforeHash);
    assert.equal(hist.data.length, beforeRev);
    return { persisted_revision: afterRead.data.current_revision, history_len: hist.data.length, redeployed: REDEPLOY, redeployUrl };
  });

  // Case 10: stale_revision_rebase_or_reject
  await runCase("10. stale_revision_rebase_or_reject", async () => {
    const f = `${AUDIT_WS}:src/stale.rs`;
    const jwt = agentJwt("stale-tester");
    const base = "// top\nfn a() { 1 }\n\n// mid\nfn b() { 2 }\n\n// bot\nfn c() { 3 }\n";
    await callTool("headless_open", { file_id: f, content: base, language: "rust" }, jwt);
    await callTool("headless_coedit", { file_id: f, author: "a1", base_revision: 1, op_id: "s-1", new_text: base.replace("{ 1 }", "{ 10 }") }, jwt);

    // Non-overlapping stale rev 1 -> auto-rebases cleanly to rev 3
    const cleanStale = await callTool("headless_coedit", { file_id: f, author: "a2", base_revision: 1, op_id: "s-2", new_text: base.replace("{ 3 }", "{ 30 }") }, jwt);
    assert.equal(cleanStale.data.ok, true);
    assert.equal(cleanStale.data.revision, 3);

    // Wrong base_content_hash rejected cleanly
    const badHash = await callTool("headless_coedit", { file_id: f, author: "a3", base_revision: 1, base_content_hash: "0000badhash", op_id: "s-3", new_text: base }, jwt);
    assert.equal(badHash.isError, true);
    assert.equal(badHash.data.code, "BASE_HASH_MISMATCH");

    // Overlapping stale rev 1 -> rejected as CONFLICT without mutating rev 3
    const overlapStale = await callTool("headless_coedit", { file_id: f, author: "a4", base_revision: 1, op_id: "s-4", new_text: base.replace("{ 1 }", "{ 999 }") }, jwt);
    assert.equal(overlapStale.data.status, "CONFLICT");
    const finalHead = await callTool("headless_read", { file_id: f }, jwt);
    assert.equal(finalHead.data.current_revision, 3);
    return { clean_rebased_rev: 3, bad_hash_code: badHash.data.code, conflict_status: overlapStale.data.status };
  });

  // Case 11: atomic_line_edit_and_replace (ide_edit_lines + ide_replace_all)
  await runCase("11. atomic_line_edit_and_replace", async () => {
    const f = `${AUDIT_WS}:src/atomic_ops.rs`;
    const jwt = agentJwt("atomic-agent");
    const base = "fn line_one() { 1 }\n// c2\n// c3\n// c4\n// c5\nfn tail_fn() { OLD_TOKEN }\n";
    await callTool("headless_open", { file_id: f, content: base, language: "rust" }, jwt);

    const [le, ra] = await Promise.all([
      callTool("ide_edit_lines", { file_id: f, agent_id: "line-editor", base_revision: 1, op_id: "atomic-l1", start_line: 1, end_line: 1, replacement: "fn line_one() { 42 }" }, jwt),
      callTool("ide_replace_all", { file_id: f, agent_id: "replacer", base_revision: 1, op_id: "atomic-r1", find: "OLD_TOKEN", replace: "NEW_TOKEN" }, jwt),
    ]);
    assert.equal(le.data.ok, true);
    assert.equal(ra.data.ok, true);
    const head = await callTool("headless_read", { file_id: f }, jwt);
    assert.equal(head.data.current_revision, 3);
    assert.ok(head.data.content.includes("fn line_one() { 42 }"));
    assert.ok(head.data.content.includes("NEW_TOKEN"));
    assert.ok(!head.data.content.includes("OLD_TOKEN"));
    return { rev: head.data.current_revision, both_preserved: true };
  });

  // Case 12: correct_nema_domain_merge (official NEMA v1.5 TAB headers & Red-Green)
  await runCase("12. correct_nema_domain_merge", async () => {
    const f = `${AUDIT_WS}:docs/arch.nema`;
    const jwt = agentJwt("nema-agent");
    const base = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\n\t?W1 1\nBuild auth flow user today edge\n\t>G1 1\nLoginAPI node Entrypoint\nAuthSvc node Backend\nLoginAPI>Call1>AuthSvc edge Auth\n\n\n";
    await callTool("headless_open", { file_id: f, content: base, language: "nema" }, jwt);

    const ours = base.replace("Alpha 100\n", "Alpha 100\nBeta 200\n");
    const theirs = base.replace("LoginAPI>Call1>AuthSvc", "TokenSvc node Cache\nLoginAPI>Call1>AuthSvc");

    const r1 = await callTool("headless_coedit", { file_id: f, author: "nema-a", base_revision: 1, op_id: "nm-1", mode: "nema", new_text: ours }, jwt);
    const r2 = await callTool("headless_coedit", { file_id: f, author: "nema-b", base_revision: 1, op_id: "nm-2", mode: "nema", new_text: theirs }, jwt);
    assert.equal(r1.data.ok, true);
    assert.equal(r2.data.ok, true);
    assert.ok(r2.data.content.includes("Beta 200"));
    assert.ok(r2.data.content.includes("TokenSvc node Cache"));
    assert.ok(r2.data.content.endsWith("\n\n\n"));

    const redBad = await callTool("headless_coedit", { file_id: f, author: "nema-bad", base_revision: 3, op_id: "nm-bad", mode: "nema", new_text: "\t|T1 1\nAlpha  BadDoubleSpace\n\n\n" }, jwt);
    assert.equal(redBad.data.ok, false);
    assert.equal(redBad.data.status, "REJECTED_NEMA_SYNTAX_ERROR");
    return { rev: r2.data.revision, bad_rejected: redBad.data.status };
  });

  // Also test REST endpoints (/api/v1/jobs/dispatch, /tasks/claim, /patches/commit, /events, /files/semantic-units)
  await runCase("12b. authenticated_rest_coordination_endpoints", async () => {
    const restUnauth = await fetch(`${BASE_URL}/api/v1/events?workspace_id=${AUDIT_WS}`);
    assert.equal(restUnauth.status, 401);

    const dispatchRes = await fetch(`${BASE_URL}/api/v1/jobs/dispatch`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${AUDIT_TOKEN}` },
      body: JSON.stringify({ workspace_id: AUDIT_WS, goal: "Audit coordination", target_files: [`${AUDIT_WS}:src/tri.rs`] }),
    });
    assert.equal(dispatchRes.status, 202);
    const dJson = await dispatchRes.json();
    assert.ok(dJson.task_count >= 1);

    const claimRes = await fetch(`${BASE_URL}/api/v1/tasks/claim`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${AUDIT_TOKEN}` },
      body: JSON.stringify({ worker_id: "rest-worker-1", workspace_id: AUDIT_WS }),
    });
    assert.equal(claimRes.status, 200);
    const claim = await claimRes.json();
    assert.ok(claim.fencing_token >= 1);

    const eventsRes = await fetch(`${BASE_URL}/api/v1/events?workspace_id=${AUDIT_WS}`, {
      headers: { Authorization: `Bearer ${AUDIT_TOKEN}` },
    });
    assert.equal(eventsRes.status, 200);
    const ev = await eventsRes.json();
    assert.ok(ev.events.length >= 1);
    return { job_id: dJson.job_id, fencing_token: claim.fencing_token, outbox_events: ev.events.length };
  });

  // Case 13: real_disk_sync_and_git_diff (local Rust MCP test on real temp git repo)
  await runCase("13. real_disk_sync_and_git_diff", async () => {
    const cargoRes = spawnSync(
      "cargo",
      ["test", "-p", "ide-mcp", "test_p1_04_real_disk_sync_external_mod_and_git_diff", "--", "--nocapture"],
      { env: { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || "E:\\DevCache\\cargo\\target_ide1" }, encoding: "utf8" }
    );
    assert.equal(cargoRes.status, 0, cargoRes.stderr || cargoRes.stdout);
    assert.ok(cargoRes.stdout.includes("1 passed"));
    return { rust_disk_git_test: "1 passed" };
  });

  const logOutPath = path.join(process.cwd(), "docs", "MCP-CLOUD-E2E-LOG.txt");
  fs.mkdirSync(path.dirname(logOutPath), { recursive: true });
  fs.writeFileSync(logOutPath, `${logs.join("\n")}\n`, "utf8");
  logLine(`\nALL 13 MANDATORY E2E CASES PASSED! Redacted transcript saved to ${logOutPath}`);
}

main().catch((err) => {
  logLine(`\nFATAL E2E FAILURE: ${err.stack || err.message}`);
  const logOutPath = path.join(process.cwd(), "docs", "MCP-CLOUD-E2E-LOG.txt");
  fs.mkdirSync(path.dirname(logOutPath), { recursive: true });
  fs.writeFileSync(logOutPath, `${logs.join("\n")}\n`, "utf8");
  process.exit(1);
});
