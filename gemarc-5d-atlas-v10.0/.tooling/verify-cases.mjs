/**
 * .tooling/verify-cases.mjs — 真跑 4D/5D 引擎，产出真实覆盖数字并回写模型。
 *
 * 契约：
 *   - 每个 Case 必须真的被引擎执行（不能只做静态检查）。
 *   - coverage 的分母来自真实模型，不预填。
 *   - 覆盖不足 / 规则未命中 / 死边 都要显式列出，不静默。
 *   - 5D 的每个干预预设必须真的产出解释（UNKNOWN 也算结果，但必须标出来）。
 */
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const ROOT = path.resolve(process.argv[2] || ".");
const M = path.join(ROOT, "models");
const read = (f) => JSON.parse(fs.readFileSync(path.join(M, f), "utf8"));

const arch = read("architecture.json");
const casesDoc = read("cases.json");
const causalDoc = read("causal-rules.json");
const scenDoc = read("scenario-deltas.json");

// ---- 载入引擎（与浏览器同源：src/*.js 是 IIFE，挂到 globalThis） ----
const ctx = { console, setTimeout, clearTimeout, Date, Math, JSON };
ctx.globalThis = ctx;
vm.createContext(ctx);
for (const f of ["engine.js", "causal.js"]) {
  const code = fs.readFileSync(path.join(ROOT, "src", f), "utf8");
  vm.runInContext(code, ctx, { filename: f });
}
const Engine = ctx.GemArcEngine;
const Causal = ctx.GemArcCausal;
if (!Engine || !Causal) throw new Error("engine/causal not loaded");

const model = {
  version: arch.version,
  nodes: arch.nodes,
  edges: arch.edges,
  rules: causalDoc.rules,
  stateMachines: causalDoc.stateMachines,
  cases: casesDoc.cases
};
const engine = Engine.createEngine(model);

// ---- 1) 逐个 Case 真跑 ----
const caseResults = [];
const firedRules = new Set();
const firedEdges = new Set();
const zeroTransitionCases = [];
let failures = 0;

for (const c of casesDoc.cases) {
  let run;
  try {
    run = engine.run(c.id, { seed: c.seed || 1 });
  } catch (e) {
    failures++;
    caseResults.push({ id: c.id, ok: false, error: String((e && e.message) || e) });
    continue;
  }
  const transitions = run.events.filter((e) => e.kind === "Transition");
  if (!transitions.length) zeroTransitionCases.push(c.id);
  run.events.forEach((e) => {
    if (e.ruleId) firedRules.add(e.ruleId);
    if (e.edgeId) firedEdges.add(e.edgeId);
  });
  caseResults.push({
    id: c.id,
    origin: c.origin,
    ok: true,
    events: run.eventCount,
    transitions: transitions.length,
    blocked: run.guardBlocked.length,
    retries: run.events.filter((e) => e.kind === "Retry").length,
    finalStates: Object.fromEntries(Object.entries(run.finalStates).map(([k, v]) => [k, v.state]))
  });
}

// ---- 2) 规则/边覆盖 ----
const ruleIds = causalDoc.rules.map((r) => r.id);
const rulesFired = ruleIds.filter((id) => firedRules.has(id));
const rulesNeverFired = ruleIds.filter((id) => !firedRules.has(id));
const edgesWithRules = new Set(causalDoc.rules.map((r) => r.edgeId));
const allEdges = arch.edges.map((e) => e.id);
const deadEdges = allEdges.filter((id) => !edgesWithRules.has(id));

// 死边原因：能说清的说清，说不清的也必须登记（缺标比错标安全）
const DEAD_REASON = {
  "E-DOCUMENT-TO-UNDO": "撤销写入在本模型里落在 Buffer→Ope 边；Document→Ope 是编译期依赖而非独立触发点。",
  "E-SHELL-TO-KERNELCORE": "kernel_banner 是纯展示依赖，无运行时状态转移。",
  "E-NAV-TO-BUFFER": "导航与行存储是包内调用，不构成可观察的请求链路。",
  "E-TYPECONFIG-TO-TYPES": "语法配置装载是进程内静态初始化。",
  "E-EDITCMD-TO-UNDO": "编辑命令写入撤销缓冲是函数内调用，与 Buffer→Ope 边重复建模。",
  "E-EDITCMD-TO-MACRO": "宏展开发生在命令执行内部，不是独立边界。",
  "E-MIGEMO-TO-SEARCHOPS": "romaji 检索是同一查找运算的可选后端。",
  "E-DOCIO-TO-ENCODING": "编码读写是函数内调用。",
  "E-PLUGIN-TO-ECPLUGIN": "插件接口是编译期依赖，运行期不产生可观察转移。",
  "E-CLI-TO-ACI": "CLI 只在 --aci-emit 时一次性输出，不参与 IDE 会话链路。",
  "E-GREP-TO-GREPENGINE": "文件枚举与内容搜索在同一次调用内完成。",
  "E-WB-TO-SCM": "SCM 有独立的桌面侧入口（scm_* command），Web 侧路由未接规则。",
  "E-WB-TO-SEARCH": "搜索有独立的桌面侧入口（search_cmd），Web 侧 /api/search 未接规则。",
  "E-WB-TO-OUTLINE": "大纲解析有独立桌面入口，Web 侧 /api/outline 未接规则。",
  "E-WB-TO-PLUGIN": "插件加载有独立桌面入口，Web 侧 /api/plugin_load 未接规则。",
  "E-WB-TO-FS": "文件读写经由 DocStore 的文档路径，未单列规则。",
  "E-WB-TO-SHELL": "前端与 App 的状态共享是渲染层耦合，不是请求转移。",
  "E-FE-TO-SHELL": "前端共享 App 状态属渲染层耦合。",
  "E-FE2-TO-WB": "第二客户端入口已由 R-FE2-WB 覆盖（并发 Case 使用）。",
  "E-DT-TO-SCM": "桌面 SCM 由独立的 scm_* command 进入，未在前端链路上建模。",
  "E-DT-TO-SEARCH": "桌面搜索由独立的 search_cmd 进入。",
  "E-DT-TO-OUTLINE": "桌面大纲由独立的 outline command 进入。",
  "E-DT-SANDBOX": "桌面侧 path_safety 复用同一校验模块，非独立请求链路。",
  "E-SCM-TO-DT": "SCM 是桌面 command 直接调用，不经外壳路由。",
  "E-SEARCH-TO-DT": "搜索是桌面 command 直接调用。",
  "E-OUTLINE-TO-DT": "大纲是桌面 command 直接调用。",
  "E-PLUGIN-TO-ECPLUGIN": "插件接口是编译期依赖。",
  "E-DOCIO-TO-FS": "文件 IO 是函数内调用。",
  "E-EDITCORE-BUFFER": "行存储与 Ope 缓冲的关系由 E-BUFFER-TO-UNDO 表达。",
  "E-NAV-BUFFER": "导航依赖已并入行存储边。",
  "E-KERNEL-TO-BUFFER": "protocol crate 自带 Buffer 实现，与 editcore 行存储是并列依赖。",
  "E-RPC-ERR-BUFFER": "错误边在 C-RPC-BUFFER-NOT-FOUND 中触发。",
  "E-AUTH-TO-WB": "认证回程边在所有 Web Case 中触发。"
};
const deadEdgeList = deadEdges.map((id) => ({
  edgeId: id,
  reason: DEAD_REASON[id] || "静态依赖 / 展示依赖 / 编译期依赖：保留在事实图中以支持影响面分析，但 4D 事件模型不为其建模状态转移。"
}));

// ---- 3) Case 类别覆盖 ----
const CLASSES = ["Normal", "Abnormal", "Boundary", "Concurrent", "Timeout", "Retry", "Recovery", "Compensation", "Combined", "Environment"];
const KEYWORDS = {
  Normal: ["normal", "正常"],
  Abnormal: ["reject", "escape", "limit", "not-found", "not_repo", "missing", "异常", "拒绝", "越界", "超限"],
  Boundary: ["limit", "escape", "超限", "越界"],
  Concurrent: ["concurrent", "并发"],
  Timeout: ["timeout", "超时"],
  Retry: ["retry", "重试"],
  Recovery: ["recover", "恢复"],
  Compensation: ["undo", "补偿", "撤销"],
  Combined: ["combined", "组合", "叠加", "四重"],
  Environment: ["webview2", "missing", "ci", "环境", "linux"]
};
const clsHit = {};
for (const c of casesDoc.cases) {
  const text = (c.id + " " + c.name + " " + JSON.stringify(c.scenario || {})).toLowerCase();
  for (const k of CLASSES) {
    if (KEYWORDS[k].some((w) => text.includes(w.toLowerCase()))) clsHit[k] = (clsHit[k] || 0) + 1;
  }
}
const covered = CLASSES.filter((k) => clsHit[k]);
const notCovered = CLASSES.filter((k) => !clsHit[k]);

// 零转移必须是「预期语义」，否则算覆盖缺口
const ZERO_TRANSITION_REASON = {
  "C-RPC-UNKNOWN-METHOD": "预期零转移：未知方法必须不改动任何内核状态（-32601 只返回错误）",
  "C-TERMINAL-MISSING": "预期零转移：环境无 powershell.exe，终端链路整体不推进",
  "C-E2E-CLICK-WEBVIEW2": "预期零转移：WebView2 缺失时 invoke 从未被调用，Rust 侧零状态变化",
  "C-COMBINED-BREACH": "预期零转移：四道守卫同时失败，链路在最早一道（路径沙箱）即停止，没有任何状态转移"
};
const unjustifiedZero = zeroTransitionCases.filter((id) => !ZERO_TRANSITION_REASON[id]);

// ---- 4) 5D 干预预设真跑 ----
const interventionResults = [];
for (const preset of scenDoc.presets || []) {
  const iv = {
    kind: preset.kind, targetId: preset.targetId,
    params: preset.params || {}, id: preset.id, caseId: preset.baseCaseId
  };
  let ce;
  try {
    ce = Causal.compare(engine, preset.baseCaseId, iv, { seed: 1 });
  } catch (e) {
    interventionResults.push({ id: preset.id, ok: false, error: String((e && e.message) || e) });
    continue;
  }
  interventionResults.push({
    id: preset.id,
    ok: true,
    baseCaseId: preset.baseCaseId,
    outcome: ce.outcome,
    firstDivergenceAt: ce.firstDivergence ? ce.firstDivergence.atSequence : null,
    affectedNodes: ce.affectedNodes,
    affectedEdges: ce.affectedEdges,
    terminalDiff: (ce.terminalDiff || []).length,
    blocking: ce.blocking,
    feedback: ce.feedback,
    recovery: ce.recovery,
    compensation: ce.compensation,
    converged: ce.converged,
    unknownReason: ce.unknownReason || null,
    deltaOps: ((ce.intervention && ce.intervention.delta && ce.intervention.delta.operations) || []).map((o) => o.op + (o.removedEdgeIds ? "[" + o.removedEdgeIds.join(",") + "]" : ""))
  });
}

// ---- 5) 回写覆盖（真实分母） ----
casesDoc.coverage = {
  casesTotal: casesDoc.cases.length,
  zeroTransitionCases: zeroTransitionCases.length,
  zeroTransitionDetail: zeroTransitionCases.map((id) => ({ id, reason: ZERO_TRANSITION_REASON[id] || "未解释 —— 覆盖缺口" })),
  unjustifiedZeroTransition: unjustifiedZero,
  rulesTotal: ruleIds.length,
  rulesFiredBySomeCase: rulesFired.length,
  edgesTotal: allEdges.length,
  edgesWithRules: edgesWithRules.size,
  deadEdges: deadEdges.length,
  caseClassesCovered: covered,
  caseClassesNotCovered: notCovered,
  declaredButNotFired: rulesNeverFired.map((id) => ({ ruleId: id, reason: "未被任何 Case 触发" })),
  note: "由 .tooling/verify-cases.mjs 真实执行全部 Case 后回写"
};
causalDoc.coverage = {
  edgesTotal: allEdges.length,
  edgesWithRules: edgesWithRules.size,
  deadEdges: deadEdges.length,
  rulesTotal: ruleIds.length,
  note: "由 .tooling/verify-cases.mjs 回写；死边明细见本文件 deadEdges 数组",
  unreachableRules: rulesNeverFired.map((id) => ({ ruleId: id, reason: "未被任何 Case 触发" }))
};
causalDoc.deadEdges = deadEdgeList;
fs.writeFileSync(path.join(M, "cases.json"), JSON.stringify(casesDoc, null, 2), "utf8");
fs.writeFileSync(path.join(M, "causal-rules.json"), JSON.stringify(causalDoc, null, 2), "utf8");

const report = {
  schema: "ide1.5d-atlas/verify-cases/v10.0",
  generatedAt: new Date().toISOString(),
  cases: caseResults,
  interventions: interventionResults,
  coverage: casesDoc.coverage,
  rulesNeverFired: rulesNeverFired,
  edgesNeverFired: deadEdges
};
fs.writeFileSync(path.join(ROOT, "qa", "verify-cases.json"), JSON.stringify(report, null, 2), "utf8");

console.log("=== CASE 执行 ===");
caseResults.forEach((r) => {
  if (!r.ok) console.log(`  FAIL ${r.id}: ${r.error}`);
  else console.log(`  ${r.id.padEnd(24)} ev=${String(r.events).padStart(3)} tr=${String(r.transitions).padStart(2)} blk=${String(r.blocked).padStart(2)} retry=${r.retries}`);
});
console.log("=== 覆盖 ===");
console.log("  cases:", casesDoc.coverage.casesTotal, "零转移:", zeroTransitionCases.length, "(未解释:", unjustifiedZero.length + ")");
console.log("  rules fired:", rulesFired.length + "/" + ruleIds.length, "never:", rulesNeverFired.join(",") || "-");
console.log("  edges with rules:", edgesWithRules.size + "/" + allEdges.length, " dead:", deadEdges.length);
console.log("  classes covered:", covered.join(","));
console.log("  classes NOT covered:", notCovered.join(",") || "(none)");
console.log("=== 5D 干预 ===");
interventionResults.forEach((r) => {
  if (!r.ok) console.log(`  FAIL ${r.id}: ${r.error}`);
  else console.log(`  ${r.id.padEnd(28)} ${String(r.outcome).padEnd(14)} div@${r.firstDivergenceAt} aff=${r.affectedNodes.length} tdiff=${r.terminalDiff} ops=${(r.deltaOps || []).join("+")}`);
});
if (failures) { console.error("FAILURES:", failures); process.exit(1); }
