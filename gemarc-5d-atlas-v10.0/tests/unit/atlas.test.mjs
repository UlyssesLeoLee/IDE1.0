/**
 * tests/unit/atlas.test.mjs — 图集模型与 4D/5D 引擎的单元测试
 *
 * 运行：node --test tests/unit/
 * 断言原则：每条测试断言「模型与引擎的真实契约」，而不是断言自己写的快照。
 */
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const rd = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, "models", f), "utf8"));

const arch = rd("architecture.json");
const rules = rd("causal-rules.json");
const cases = rd("cases.json");
const spatial = rd("spatial.json");
const prompts = rd("prompts.json");
const prototype = rd("prototype.json");
const evidence = rd("evidence.json");
const requirements = rd("requirements.json");
const scen = rd("scenario-deltas.json");

const ctx = { console, setTimeout, clearTimeout, Date, Math, JSON };
ctx.globalThis = ctx;
vm.createContext(ctx);
for (const f of ["engine.js", "causal.js"]) {
  vm.runInContext(fs.readFileSync(path.join(ROOT, "src", f), "utf8"), ctx, { filename: f });
}
const Engine = ctx.GemArcEngine;
const Causal = ctx.GemArcCausal;

function makeEngine() {
  return Engine.createEngine({
    version: arch.version, nodes: arch.nodes, edges: arch.edges,
    rules: rules.rules, stateMachines: rules.stateMachines, cases: cases.cases
  });
}
const engine = makeEngine();

const nodeIds = new Set(arch.nodes.map((n) => n.id));
const edgeIds = new Set(arch.edges.map((e) => e.id));
const portIds = new Set(arch.ports.map((p) => p.id));
const evIds = new Set(evidence.evidence.map((e) => e.id));
const reqIds = new Set(requirements.requirements.map((r) => r.id));
const caseIds = new Set(cases.cases.map((c) => c.id));

// ---------------------------------------------------------------- 模型契约

test("每个节点的 role 都是合法功能角色（否则 3D 几何退化）", () => {
  const legal = ["Actor", "Input", "Processor", "Decision", "Storage", "Message", "Interface", "Security", "Agent", "Composition", "Output"];
  for (const n of arch.nodes) assert.ok(legal.includes(n.role), n.id + " 的 role 非法: " + n.role);
});

test("每个节点都有 3D 偏移与宝石映射（缺失会在 3D 中被静默跳过）", () => {
  for (const n of arch.nodes) {
    assert.ok(spatial.nodeOffsets[n.id], "缺少 nodeOffsets: " + n.id);
    assert.ok(spatial.gemstoneMapping.byNode[n.id], "缺少宝石映射: " + n.id);
  }
});

test("节点的端口与父容器都真实存在", () => {
  const containers = new Set(arch.containers.map((c) => c.id));
  for (const n of arch.nodes) {
    for (const p of n.portIds || []) assert.ok(portIds.has(p), n.id + " 引用了不存在的端口 " + p);
    if (n.parentId) assert.ok(containers.has(n.parentId), n.id + " 的 parentId 不存在 " + n.parentId);
  }
});

test("规则的 edgeId 全部指向真实边，且端点与边一致", () => {
  for (const r of rules.rules) {
    assert.ok(edgeIds.has(r.edgeId), r.id + " 的 edgeId 不存在: " + r.edgeId);
    const e = arch.edges.find((x) => x.id === r.edgeId);
    assert.equal(r.sourceNodeId, e.sourceId, r.id + " sourceNodeId 与边不一致");
    assert.equal(r.targetNodeId, e.targetId, r.id + " targetNodeId 与边不一致");
  }
});

test("Case 引用的节点/边/证据全部存在", () => {
  for (const c of cases.cases) {
    for (const id of c.nodeIds || []) assert.ok(nodeIds.has(id), c.id + " 的 nodeId 不存在 " + id);
    for (const id of c.edgeIds || []) assert.ok(edgeIds.has(id), c.id + " 的 edgeId 不存在 " + id);
    for (const id of c.evidenceIds || []) assert.ok(evIds.has(id), c.id + " 的 evidenceId 不存在 " + id);
  }
});

test("5D 预设的目标与基线 Case 都真实存在（否则退化为 UNKNOWN）", () => {
  for (const s of scen.presets) {
    assert.ok(nodeIds.has(s.targetId) || edgeIds.has(s.targetId), s.id + " 的 targetId 不存在 " + s.targetId);
    assert.ok(caseIds.has(s.baseCaseId), s.id + " 的 baseCaseId 不存在 " + s.baseCaseId);
  }
});

test("Prompt 的四类引用 ID 全部可解析（1D 校验是硬错误）", () => {
  const groups = [prompts.groupA_targetSystemBuilders, prompts.groupB_atlasImplementationPrompts];
  for (const g of groups) for (const p of g.prompts) {
    for (const id of p.requirementIds || []) assert.ok(reqIds.has(id), p.promptId + " 的需求不存在 " + id);
    for (const id of p.nodeEdgeIds || []) assert.ok(nodeIds.has(id) || edgeIds.has(id), p.promptId + " 的节点/边不存在 " + id);
    for (const id of p.caseIds || []) assert.ok(caseIds.has(id), p.promptId + " 的 Case 不存在 " + id);
    for (const id of p.evidenceIds || []) assert.ok(evIds.has(id), p.promptId + " 的证据不存在 " + id);
  }
});

test("需求的证据引用全部存在", () => {
  for (const r of requirements.requirements) {
    for (const id of r.evidenceIds || []) assert.ok(evIds.has(id), r.id + " 的证据不存在 " + id);
  }
});

test("原型工作区具备多标签、预置控件与 Action（不是只有 Konva 图形）", () => {
  assert.ok(prototype.sourceScreens[prototype.primaryScreenGroup].screens.length >= 4, "真实屏幕不足 4 个");
  assert.ok(prototype.seedWidgets.length >= 5, "预置控件不足");
  const withActions = prototype.seedWidgets.filter((w) => (w.actions || []).length > 0);
  assert.ok(withActions.length >= 4, "带 Action 的控件不足");
  assert.ok(prototype.fiveDBinding.bindings.length >= 3, "5D 绑定不足");
  for (const b of prototype.fiveDBinding.bindings) assert.ok(caseIds.has(b.caseId), "绑定指向不存在的 Case " + b.caseId);
});

// ---------------------------------------------------------------- 4D 引擎

test("同一 Case 两次执行得到完全相同的事件轨迹（确定性）", () => {
  const a = engine.run("C-NORMAL-EDIT", { seed: 1 });
  const b = engine.run("C-NORMAL-EDIT", { seed: 1 });
  assert.deepEqual(a.events.map((e) => [e.sequence, e.kind, e.sourceNodeId, e.targetNodeId, e.nextState]),
    b.events.map((e) => [e.sequence, e.kind, e.sourceNodeId, e.targetNodeId, e.nextState]));
});

test("事件序号与逻辑时间严格升序", () => {
  const r = engine.run("C-NORMAL-EDIT", { seed: 1 });
  for (let i = 1; i < r.events.length; i++) {
    assert.ok(r.events[i].sequence > r.events[i - 1].sequence, "sequence 未严格升序");
    assert.ok(r.events[i].logicalTime >= r.events[i - 1].logicalTime, "logicalTime 未升序");
  }
});

test("全部 Case 都能真实执行，无异常抛出", () => {
  for (const c of cases.cases) {
    const r = engine.run(c.id, { seed: c.seed || 1 });
    assert.ok(r.eventCount > 0, c.id + " 没有任何事件");
  }
});

test("守卫缺少变量时判否（缺标比错标安全）", () => {
  const ok = engine.evalGuard({ type: "equals", left: { type: "var", var: "input:missingVar" }, right: { type: "literal", value: true } }, { states: {}, inputs: {}, counters: {} });
  assert.equal(ok, false, "缺失变量不应通过");
});

test("预算上限生效：超限抛 BudgetExceeded 而不是静默截断", () => {
  const tiny = Engine.createEngine({
    nodes: arch.nodes, edges: arch.edges, rules: rules.rules,
    stateMachines: rules.stateMachines, cases: cases.cases
  }, { maxEvents: 2 });
  assert.throws(() => tiny.run("C-NORMAL-EDIT", { seed: 1 }), /maxEvents/);
});

test("越界路径场景下 DocStore 不被进入（沙箱与存储守卫双重拦截）", () => {
  const r = engine.run("C-PATH-ESCAPE", { seed: 1 });
  assert.ok(!r.finalStates["N-KN-DOCSTORE"], "越界请求不应进入 DocStore");
  assert.ok(r.guardBlocked.length > 0, "应有守卫阻断记录");
});

test("未知 RPC 方法不改变任何内核状态（零状态转移是预期语义）", () => {
  const r = engine.run("C-RPC-UNKNOWN-METHOD", { seed: 1 });
  assert.equal(r.events.filter((e) => e.kind === "Transition").length, 0, "未知方法不应产生状态转移");
});

// ---------------------------------------------------------------- 5D 因果

test("干预目标不存在时返回 UNKNOWN 而不是 NO_DIFFERENCE", () => {
  const ce = Causal.compare(engine, "C-NORMAL-EDIT", { kind: "DisableNode", targetId: "N-NOT-EXIST", params: {} }, { seed: 1 });
  assert.equal(ce.outcome, "UNKNOWN");
  assert.ok(ce.unknownReason, "UNKNOWN 必须给原因");
});

test("禁用沙箱节点后越界请求仍被存储边守卫挡住（守门点是守卫条件）", () => {
  const ce = Causal.compare(engine, "C-PATH-ESCAPE", { kind: "DisableNode", targetId: "N-WB-PATHSAFETY", params: {} }, { seed: 1 });
  assert.equal(ce.outcome, "DIVERGED", "禁用沙箱节点应产生事件序列分歧");
  assert.ok(!ce.runB.finalStates["N-KN-DOCSTORE"], "禁用沙箱节点不应让越界请求进入 DocStore");
});

test("插入元素必须真正把 A→B 改写为 A→X→B，旧边从模型中移除并留痕", () => {
  const r = Causal.applyIntervention(engine.model, {
    kind: "InsertElement", targetId: "E-WB-TO-STORE",
    params: { nodeId: "X-VALIDATOR", name: "Injected Validator", kind: "Processor", role: "Processor" },
    id: "SD-TEST", caseId: "C-NORMAL-EDIT"
  });
  const op = r.delta.operations[0];
  assert.equal(op.op, "InsertElement");
  // 注意：引擎在 vm 上下文中运行，其数组原型与主上下文不同，
  // strict deepEqual 会因原型不一致误判 —— 必须先转成同 realm 数组再比较。
  assert.deepEqual([...op.removedEdgeIds], ["E-WB-TO-STORE"], "旧边去留必须可查");
  assert.equal(op.newEdgeIds.length, 2, "应生成 A→X 与 X→B 两条边");
  assert.ok(!r.model.edges.some((e) => e.id === "E-WB-TO-STORE"), "旧边必须从实验副本移除");
  assert.ok(r.model.nodes.some((n) => n.id === "X-VALIDATOR"), "新节点必须进入实验副本");
  assert.equal(r.delta.mutates, true);
});

test("干预只作用于副本，正式事实图不被修改", () => {
  const before = arch.edges.length;
  Causal.compare(engine, "C-NORMAL-EDIT", { kind: "DisableNode", targetId: "N-WB-SERVER", params: {} }, { seed: 1 });
  assert.equal(arch.edges.length, before);
  assert.ok(engine.model.edges.some((e) => e.id === "E-WB-TO-STORE"), "事实图的边不应被移除");
});

test("超时干预真实改变终态（Failed ↔ Completed）", () => {
  const ce = Causal.compare(engine, "C-PLUGIN-TIMEOUT", { kind: "ChangeTimeout", targetId: "E-PLUGIN-TO-DT", params: { timeoutMs: 1000 } }, { seed: 1 });
  assert.equal(ce.outcome, "DIVERGED");
  assert.ok(ce.terminalDiff.length > 0, "应存在终态差异");
});

test("全部 5D 预设都能产出解释（不得抛异常）", () => {
  for (const p of scen.presets) {
    const ce = Causal.compare(engine, p.baseCaseId, { kind: p.kind, targetId: p.targetId, params: p.params || {}, id: p.id, caseId: p.baseCaseId }, { seed: 1 });
    assert.ok(["DIVERGED", "NO_DIFFERENCE", "UNKNOWN"].includes(ce.outcome), p.id + " 的结论非法: " + ce.outcome);
  }
});
