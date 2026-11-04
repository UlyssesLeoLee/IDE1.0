/**
 * .tooling/apply-audit-fixes.mjs
 * 应用对抗式审核的 findings 到模型层（只改主 agent 拥有的文件）
 *
 * 修复项：
 *   H-1  Case inputs 补齐守卫变量（authPresent / operation / provider / probe …）
 *   H-2  引擎多规则（已改 src/engine.js）→ 模型侧无需改
 *   H-3  CASE-GIT-RECEIVE-UNAUTH 入口改为 N-HTTP-ROUTER（让 E-002 的鉴权规则可达）
 *   H-4  同步规则改走自环边 E-026（失败记在 N-REMOTE-SYNC 自身）
 *   H-5  补 N-REPOS-DIR / N-VAULT-DIR 节点，消除悬空边引用
 *   H-8  CASE-AI-SANITIZE-REDACT 入口改为 N-AI-PROMPT；Anthropic 拒流改走新边 E-027
 *   M-1  /api 端点数 13 → 12
 *   M-2  补 13 个节点的状态机
 *   M-3  规则的 when 对齐源节点可达状态
 *   M-8  E-008 的 git 调用面描述修正
 *   M-10 补 VersionedVault / diff 两个真实节点
 *   M-11 schema.implementation 枚举补 Unknown
 *   M-14 前端层节点不再被宝石化（不进 3D）
 *   M-17 coverageNote 计数修正
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const M = path.join(ROOT, "models");
const read = (n) => JSON.parse(fs.readFileSync(path.join(M, n), "utf8"));
const write = (n, o) => fs.writeFileSync(path.join(M, n), JSON.stringify(o, null, 2) + "\n", "utf8");
const log = (...a) => console.log(...a);
const changes = [];

// ============================================================ cases.json
{
  const c = read("cases.json");
  const byId = Object.fromEntries(c.cases.map((x) => [x.id, x]));

  // --- H-1：守卫变量必须在 inputs 里显式给出 ---
  const INPUTS = {
    "CASE-VAULT-WRITE-NORMAL": { authPresent: true },
    "CASE-VAULT-WRITE-EMPTY": { authPresent: true },
    "CASE-VAULT-PATH-TRAVERSAL": { authPresent: true },
    "CASE-VAULT-RESTORE": { authPresent: true },
    "CASE-VAULT-DIFF-RANGE": { authPresent: true },
    "CASE-CHANGE-NOTE-DISCARDED": { authPresent: true },
    "CASE-ERROR-ENVELOPE-SHAPE": { authPresent: true },
    "CASE-AUTH-MISSING": { authPresent: false },
    "CASE-AUTH-BASIC-MISMATCH": { authPresent: false },
    "CASE-GIT-RECEIVE-UNAUTH": { authPresent: false, operation: "receive-pack" },
    "CASE-GIT-UPLOADPACK-OPEN": { authPresent: false, operation: "upload-pack" },
    "CASE-AI-ANTHROPIC-STREAM-REJECT": { provider: "anthropic", streaming: true },
    "CASE-AI-SANITIZE-REDACT": { content: "-----BEGIN PRIVATE KEY-----\nAKIAIOSFODNN7EXAMPLE\n-----END PRIVATE KEY-----" },
    "CASE-CONSOLE-LOGIN-PROBE": { probe: "/api/repos", authPresent: true },
    "CASE-SYNC-NOT-FAST-FORWARD": { fastForwardable: false },
    "CASE-SYNC-FAST-FORWARD-PUSH": { fastForwardable: true }
  };
  for (const [id, add] of Object.entries(INPUTS)) {
    const t = byId[id];
    if (!t) { log("  !! case not found:", id); continue; }
    t.inputs = Object.assign({}, t.inputs || {}, add);
    changes.push("H-1 cases.json " + id + " inputs += " + JSON.stringify(add));
  }

  // --- H-3：Git 写未授权必须从 HTTP 层进入，才能命中 E-002 的鉴权规则 ---
  {
    const t = byId["CASE-GIT-RECEIVE-UNAUTH"];
    t.entryNodeId = "N-HTTP-ROUTER";
    t.entryNodes = ["N-HTTP-ROUTER"];
    t.entryState = "Running";
    t.nodeIds = ["N-HTTP-ROUTER", "N-GIT-SMART", "N-AUTH", "N-SUBPROCESS"];
    t.edgeIds = ["E-002", "E-009"];
    changes.push("H-3 CASE-GIT-RECEIVE-UNAUTH entry -> N-HTTP-ROUTER");
  }
  {
    const t = byId["CASE-GIT-UPLOADPACK-OPEN"];
    t.entryNodeId = "N-HTTP-ROUTER";
    t.entryNodes = ["N-HTTP-ROUTER"];
    t.entryState = "Running";
    t.nodeIds = ["N-HTTP-ROUTER", "N-GIT-SMART", "N-SMART-FRAME", "N-SUBPROCESS"];
    t.edgeIds = ["E-002", "E-009", "E-010"];
    changes.push("H-3 CASE-GIT-UPLOADPACK-OPEN entry -> N-HTTP-ROUTER");
  }

  // --- H-8：脱敏 Case 必须从 N-AI-PROMPT 进入，否则 E-013 不可达 ---
  {
    const t = byId["CASE-AI-SANITIZE-REDACT"];
    t.entryNodeId = "N-AI-PROMPT";
    t.entryNodes = ["N-AI-PROMPT"];
    t.entryState = "Running";
    t.nodeIds = ["N-AI-PROMPT", "N-AI-SANITIZE", "N-AI-PROVIDER"];
    t.edgeIds = ["E-013", "E-014"];
    changes.push("H-8 CASE-AI-SANITIZE-REDACT entry -> N-AI-PROMPT");
  }
  {
    const t = byId["CASE-AI-ANTHROPIC-STREAM-REJECT"];
    t.nodeIds = ["N-AI-REGISTRY", "N-AI-ANTHROPIC", "N-AI-STREAM", "N-AI-PROVIDER"];
    t.edgeIds = ["E-012", "E-027"];
    changes.push("H-8 CASE-AI-ANTHROPIC-STREAM-REJECT edgeIds -> E-027");
  }

  // --- M-17：coverageNote 计数与悬空引用 ---
  c.coverageNote = "本文件覆盖 16 个 Case，覆盖 Normal / Abnormal / Boundary 三类；" +
    "Timeout / Retry / Recovery / Compensation / Concurrent / Combined 共 6 类在当前模型中无真实规则来源" +
    "（目标系统 src/** 全量 grep 无 retry / backoff / compensate / timeout 实现），因此不虚构 —— 见 ADVERSARIAL_AUDIT.md 的 B-CAUSE 条目。";
  changes.push("M-17 cases.json coverageNote 计数修正 + 去掉悬空 BACKLOG 引用");

  write("cases.json", c);
}

// ==================================================== causal-rules.json
{
  const r = read("causal-rules.json");

  // --- M-2：补目标节点的状态机 ---
  const SM = [
    ["SM-CREDSTORE", "N-CREDSTORE", "Idle"],
    ["SM-VAULT-VERSIONED", "N-VAULT-VERSIONED", "Idle"],
    ["SM-SUBPROCESS", "N-SUBPROCESS", "Idle"],
    ["SM-AI-PROVIDER", "N-AI-PROVIDER", "Idle"],
    ["SM-AI-OPENAI", "N-AI-OPENAI", "Idle"],
    ["SM-AI-ANTHROPIC", "N-AI-ANTHROPIC", "Idle"],
    ["SM-AI-STREAM", "N-AI-STREAM", "Idle"],
    ["SM-AI-PROMPT", "N-AI-PROMPT", "Running"],
    ["SM-AI-SANITIZE", "N-AI-SANITIZE", "Idle"],
    ["SM-REMOTE-STORE", "N-REMOTE-STORE", "Idle"],
    ["SM-CONSOLE-STORES", "N-CONSOLE-STORES", "Idle"],
    ["SM-CONSOLE-API", "N-CONSOLE-API", "Running"],
    ["SM-CONSOLE-APP", "N-CONSOLE-APP", "Running"],
    ["SM-HTTP-ROUTER", "N-HTTP-ROUTER", "Running"]
  ];
  const have = new Set(r.stateMachines.map((s) => s.nodeId));
  for (const [id, nodeId, initial] of SM) {
    if (have.has(nodeId)) continue;
    r.stateMachines.push({
      id, nodeId, initial,
      states: ["Idle", "Running", "Completed", "Failed"],
      terminalStates: ["Completed", "Failed"]
    });
    changes.push("M-2 causal-rules.json + stateMachine " + id + " (" + nodeId + ")");
  }
  // SM-AUTH 的终态补 Blocked 之外的状态已存在；SM-API 补 Blocked（已有）

  // --- M-3：when 必须等于源节点在该步可达的状态 ---
  const WHEN = {
    "R-AUTH-CHECK": "Running",
    "R-AUTH-REJECT": "Running",
    "R-AUTH-READ-CRED": "Authorized",
    "R-VAULT-WRITE": "Running",
    "R-VAULT-REJECT-EMPTY": "Running",
    "R-VAULT-REJECT-TRAVERSAL": "Running",
    "R-VAULT-VERSION-APPEND": "Running",
    "R-GIT-RECEIVE-AUTH": "Running",
    "R-GIT-READ-OPEN": "Running",
    "R-GIT-SPAWN": "Running",
    "R-AI-REGISTRY-PICK": "Running",
    "R-AI-REGISTRY-PICK-ANTHROPIC": "Running",
    "R-AI-STREAM-OPENAI": "Running",
    "R-AI-STREAM-REJECT-ANTHROPIC": "Running",
    "R-AI-SANITIZE": "Running",
    "R-AI-SEND": "Completed",
    "R-SYNC-FASTFORWARD": "Running",
    "R-SYNC-NOT-FASTFORWARD": "Running",
    "R-SYNC-STORE": "Completed",
    "R-CONSOLE-PROBE": "Running",
    "R-CONSOLE-READ-CRED": "Running",
    "R-CONSOLE-MOCK-INTERCEPT": "Running"
  };
  for (const rule of r.rules) {
    const w = WHEN[rule.id];
    if (w && rule.when !== w) {
      changes.push("M-3 " + rule.id + " when " + rule.when + " -> " + w);
      rule.when = w;
    }
  }

  // --- H-4：同步判定改为自环，失败记在 N-REMOTE-SYNC 自身 ---
  {
    const ff = r.rules.find((x) => x.id === "R-SYNC-FASTFORWARD");
    const nf = r.rules.find((x) => x.id === "R-SYNC-NOT-FASTFORWARD");
    if (ff) {
      ff.edgeId = "E-026";
      ff.targetNodeId = "N-REMOTE-SYNC";
      ff.then = "Completed";
      ff.guardAst = { type: "equals", left: { type: "var", var: "input:fastForwardable" }, right: true };
      ff.note = "快进判定成立（本地是远端祖先）→ 同步节点自身进入 Completed，随后 E-025 记录远端";
    }
    if (nf) {
      nf.edgeId = "E-026";
      nf.targetNodeId = "N-REMOTE-SYNC";
      nf.then = "Failed";
      nf.guardAst = { type: "equals", left: { type: "var", var: "input:fastForwardable" }, right: false };
      nf.note = "非快进 → 同步节点自身 Failed，不 push（与 src/remote/sync.rs 的 NotFastForward 分支一致）";
    }
    changes.push("H-4 同步规则改走自环边 E-026（失败记在 N-REMOTE-SYNC 自身）");
  }

  // --- H-8：Anthropic 拒流走独立边 E-027 ---
  {
    const rj = r.rules.find((x) => x.id === "R-AI-STREAM-REJECT-ANTHROPIC");
    if (rj) {
      rj.edgeId = "E-027";
      rj.sourceNodeId = "N-AI-ANTHROPIC";
      rj.targetNodeId = "N-AI-STREAM";
      rj.then = "Failed";
      rj.guardAst = { type: "always" };
      rj.note = "Anthropic 协议 supports_streaming=false，send_stream 返回错误而非回退 —— 失败记在流式通道上";
    }
    changes.push("H-8 R-AI-STREAM-REJECT-ANTHROPIC -> 边 E-027");
  }

  // --- 新增规则：前端 → API 客户端 ---
  r.rules.push({
    id: "R-CONSOLE-CALL",
    edgeId: "E-018",
    sourceNodeId: "N-CONSOLE-APP",
    targetNodeId: "N-CONSOLE-API",
    when: "Running",
    then: "Running",
    guardAst: { type: "always" },
    durationMs: 1,
    evidenceIds: ["EV-055"]
  });
  changes.push("新增规则 R-CONSOLE-CALL (E-018)");

  write("causal-rules.json", r);
}

// ======================================================= architecture.json
{
  const a = read("architecture.json");

  // --- M-1：端点数 ---
  const nApi = a.nodes.find((n) => n.id === "N-API-ROUTER");
  if (nApi && nApi.properties.endpointCount === 13) {
    nApi.properties.endpointCount = 12;
    nApi.properties.endpointBreakdown = "11 条受保护 method-endpoint + 1 条公开 /api/health（共 10 条路径）";
    changes.push("M-1 N-API-ROUTER endpointCount 13 -> 12");
  }
  const r1 = a.containers.find((c) => c.id === "C-R1");
  if (r1) {
    r1.semantic = "12 条 /api method-endpoint（10 条路径）+ 1 条公开 /api/health；除 health 外全部在鉴权层内。";
    changes.push("M-1 C-R1 语义修正");
  }

  // --- M-8：E-008 的 git 调用面 ---
  {
    const e8 = a.edges.find((e) => e.id === "E-008");
    if (e8) {
      e8.type = "spawns";
      e8.properties = {
        commands: ["show-ref", "log", "symbolic-ref", "rev-parse"],
        note: "这四个命令由 api.rs 内部的 run_git / run_show_ref / read_head_* 直接 spawn，不经过 subprocess.rs 的 upload-pack/receive-pack 白名单"
      };
      changes.push("M-8 E-008 属性修正（不经过 smart-HTTP 白名单）");
    }
    const sp = a.nodes.find((n) => n.id === "N-SUBPROCESS");
    if (sp) {
      sp.properties.statelessRpcSubcommandAllowlist = ["upload-pack", "receive-pack"];
      sp.properties.allowlistScope = "仅 git_stateless_rpc（smart-HTTP 路径）";
      sp.properties.otherGitCallSites = "src/server/api.rs 的 run_git 与 src/repo/status.rs 另走无白名单路径";
      delete sp.properties.subcommandAllowlist;
      changes.push("M-8 N-SUBPROCESS 白名单作用域限定");
    }
  }

  // --- H-5 / M-10：补真实节点 ---
  const NEW_NODES = [
    {
      id: "N-REPOS-DIR", kind: "storage", name: "repos_dir", qualifiedName: "gitgit::config::Config::repos_dir",
      role: "Storage", domainId: "Data", parentId: "C-R7", portIds: [],
      properties: { defaultPath: "repos", layout: "<repos_dir>/<name>.git/", configField: "Config.repos_dir" },
      verification: "Implemented", evidenceIds: ["EV-079", "EV-080"]
    },
    {
      id: "N-VAULT-DIR", kind: "storage", name: "vault_file_root", qualifiedName: "gitgit::config::Config::vault_file_root",
      role: "Storage", domainId: "Data", parentId: "C-R8", portIds: [],
      properties: {
        defaultPath: ".gitgit/vault", layout: "<root>/<key>",
        sidecar: "_versions/<key>.versions.json", attachments: "_attachments/<key>/v<N>.bin",
        configField: "Config.vault_file_root"
      },
      verification: "Implemented", evidenceIds: ["EV-079", "EV-080", "EV-032"]
    },
    {
      id: "N-VAULT-TRAIT", kind: "trait", name: "VersionedVault", qualifiedName: "gitgit::server::vault_versioned::VersionedVault",
      role: "Interface", domainId: "Core", parentId: "C-R4", portIds: ["P-VAULT-IN", "P-VAULT-OUT"],
      properties: { implementations: ["FileVault", "MinioVault"], runtimeImplementation: "FileVault" },
      verification: "Implemented", evidenceIds: ["EV-081"]
    },
    {
      id: "N-AI-DIFF", kind: "module", name: "diff", qualifiedName: "gitgit::ai::diff",
      role: "Processor", domainId: "Intelligence", parentId: "C-R5", portIds: [],
      properties: { publicStruct: "DiffRequest", lineCount: 269 },
      verification: "Implemented", evidenceIds: ["EV-082"]
    }
  ];
  const haveNode = new Set(a.nodes.map((n) => n.id));
  for (const n of NEW_NODES) {
    if (haveNode.has(n.id)) continue;
    a.nodes.push(n);
    changes.push("H-5/M-10 + node " + n.id);
  }

  // --- H-4 / H-8：补边 ---
  const NEW_EDGES = [
    {
      id: "E-026", sourceId: "N-REMOTE-SYNC", targetId: "N-REMOTE-SYNC",
      sourcePortId: null, targetPortId: null, type: "fastForwardCheck", directed: true, guardId: null,
      properties: { note: "快进判定：git merge-base --is-ancestor 的结果落在同步节点自身（自环）" },
      verification: "Implemented", evidenceIds: ["EV-038"]
    },
    {
      id: "E-027", sourceId: "N-AI-ANTHROPIC", targetId: "N-AI-STREAM",
      sourcePortId: null, targetPortId: null, type: "streamRejected", directed: true, guardId: null,
      properties: { note: "Anthropic 协议不支持流式，send_stream 拒绝而非回退" },
      verification: "Implemented", evidenceIds: ["EV-045", "EV-042"]
    }
  ];
  const haveEdge = new Set(a.edges.map((e) => e.id));
  for (const e of NEW_EDGES) {
    if (haveEdge.has(e.id)) continue;
    a.edges.push(e);
    changes.push("H-4/H-8 + edge " + e.id);
  }

  write("architecture.json", a);
}

// ============================================================ spatial.json
{
  const s = read("spatial.json");
  // --- M-14：前端层节点不进入 3D，不能被宝石化 ---
  const NOT_IN_3D = ["N-CONSOLE-APP", "N-CONSOLE-API", "N-CONSOLE-STORES", "N-CONSOLE-MOCKS", "N-DESKTOP-APP", "N-PROTOTYPE"];
  if (s.gemstoneMapping && s.gemstoneMapping.byNode) {
    for (const id of NOT_IN_3D) {
      if (s.gemstoneMapping.byNode[id]) {
        delete s.gemstoneMapping.byNode[id];
        changes.push("M-14 spatial.json 移除宝石化 " + id + "（不进 3D）");
      }
    }
    s.gemstoneMapping.scopeRule = "只有出现在 nodeOffsets 且归属某个房间的节点才进入 3D 并使用宝石；前端 UI 层节点（gm-console / gm-desktop / prototype）不进入 3D，不宝石化。";
  }
  // --- 存储层节点补坐标 ---
  if (!s.nodeOffsets["N-REPOS-DIR"]) {
    s.nodeOffsets["N-REPOS-DIR"] = [-8, -16, 0];
    s.nodeOffsets["N-VAULT-DIR"] = [8, -16, 0];
    changes.push("spatial.json + nodeOffsets N-REPOS-DIR / N-VAULT-DIR");
  }
  if (s.rooms["C-R7"]) s.rooms["C-R7"].nodes = ["N-REPOS-DIR"];
  if (s.rooms["C-R8"]) s.rooms["C-R8"].nodes = ["N-VAULT-DIR"];
  // 新增节点在 3D 里的位置：归入既有房间
  if (s.rooms["C-R4"]) {
    s.rooms["C-R4"].nodes = (s.rooms["C-R4"].nodes || []).concat(["N-VAULT-TRAIT"]);
    s.nodeOffsets["N-VAULT-TRAIT"] = [-3, -8, -3];
  }
  if (s.rooms["C-R5"]) {
    s.rooms["C-R5"].nodes = (s.rooms["C-R5"].nodes || []).concat(["N-AI-DIFF"]);
    s.nodeOffsets["N-AI-DIFF"] = [9, -8, -3];
  }
  if (!s.nodeOffsets["N-HTTP-ROUTER"]) s.nodeOffsets["N-HTTP-ROUTER"] = [-13, 1, 0];
  write("spatial.json", s);
}

// ============================================================= schema.json
{
  const sc = read("schema.json");
  const impl = sc.enumerations.implementation;
  if (!impl.includes("Unknown")) {
    impl.push("Unknown");
    changes.push("M-11 schema.json implementation 枚举 + Unknown（与 evidence 的 Unknown 状态一致）");
  }
  write("schema.json", sc);
}

log("=== 变更 " + changes.length + " 项 ===");
changes.forEach((c) => log("  " + c));
