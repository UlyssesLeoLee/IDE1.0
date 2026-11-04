/**
 * .tooling/verify-refs.mjs — 引用一致性门禁。
 *
 * 为什么必须有：规则/Case/端口里的 ID 写错时，引擎不会报错，只会让该规则永不触发
 * （被 || [] / map.get 静默兜底），功能整条消失且零报错。
 * 这里对全部模型交叉引用做硬校验，任何悬空引用直接退出非零。
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const M = path.join(ROOT, "models");
const rd = (f) => JSON.parse(fs.readFileSync(path.join(M, f), "utf8"));

const arch = rd("architecture.json");
const rules = rd("causal-rules.json");
const cases = rd("cases.json");
const scen = rd("scenario-deltas.json");
const spatial = rd("spatial.json");
const evidence = rd("evidence.json");
const requirements = rd("requirements.json");

const nodeIds = new Set(arch.nodes.map((n) => n.id));
const edgeIds = new Set(arch.edges.map((e) => e.id));
const portIds = new Set(arch.ports.map((p) => p.id));
const containerIds = new Set(arch.containers.map((c) => c.id));
const evIds = new Set(evidence.evidence.map((e) => e.id));
const reqIds = new Set(requirements.requirements.map((r) => r.id));

const problems = [];
const P = (where, msg) => problems.push(where + ": " + msg);

// --- 规则 → 边/节点 ---
for (const r of rules.rules) {
  if (!edgeIds.has(r.edgeId)) P("rule " + r.id, "edgeId 不存在 → " + r.edgeId + "（该规则永不触发）");
  if (!nodeIds.has(r.sourceNodeId)) P("rule " + r.id, "sourceNodeId 不存在 → " + r.sourceNodeId);
  if (!nodeIds.has(r.targetNodeId)) P("rule " + r.id, "targetNodeId 不存在 → " + r.targetNodeId);
  const e = arch.edges.find((x) => x.id === r.edgeId);
  if (e) {
    if (e.sourceId !== r.sourceNodeId) P("rule " + r.id, "sourceNodeId 与边不一致：" + r.sourceNodeId + " vs " + e.sourceId);
    if (e.targetId !== r.targetNodeId) P("rule " + r.id, "targetNodeId 与边不一致：" + r.targetNodeId + " vs " + e.targetId);
  }
  (r.evidenceIds || []).forEach((id) => { if (!evIds.has(id)) P("rule " + r.id, "evidenceId 不存在 → " + id); });
}
// --- 状态机 → 节点 ---
for (const sm of rules.stateMachines) {
  if (!nodeIds.has(sm.nodeId)) P("stateMachine " + sm.id, "nodeId 不存在 → " + sm.nodeId);
  if (!sm.states.includes(sm.initial)) P("stateMachine " + sm.id, "initial 不在 states 内 → " + sm.initial);
  (sm.terminalStates || []).forEach((s) => { if (!sm.states.includes(s)) P("stateMachine " + sm.id, "terminalState 不在 states 内 → " + s); });
}
// --- Case → 节点/边/证据 ---
for (const c of cases.cases) {
  (c.nodeIds || []).forEach((id) => { if (!nodeIds.has(id)) P("case " + c.id, "nodeId 不存在 → " + id); });
  (c.edgeIds || []).forEach((id) => { if (!edgeIds.has(id)) P("case " + c.id, "edgeId 不存在 → " + id); });
  (c.evidenceIds || []).forEach((id) => { if (!evIds.has(id)) P("case " + c.id, "evidenceId 不存在 → " + id); });
  if (c.entryNodeId && !nodeIds.has(c.entryNodeId)) P("case " + c.id, "entryNodeId 不存在 → " + c.entryNodeId);
  (c.entryNodes || []).forEach((id) => { if (!nodeIds.has(id)) P("case " + c.id, "entryNodes 含不存在的节点 → " + id); });
  const sm = rules.stateMachines.find((s) => s.nodeId === c.entryNodeId);
  if (sm && c.entryState && !sm.states.includes(c.entryState)) P("case " + c.id, "entryState 不在状态机内 → " + c.entryState);
}
// --- 5D 预设 → 目标存在 ---
for (const s of scen.presets) {
  const ok = nodeIds.has(s.targetId) || edgeIds.has(s.targetId);
  if (!ok) P("preset " + s.id, "targetId 不存在 → " + s.targetId + "（干预会退化成 UNKNOWN）");
  if (!cases.cases.find((c) => c.id === s.baseCaseId)) P("preset " + s.id, "baseCaseId 不存在 → " + s.baseCaseId);
}
// --- 节点 → 端口/父容器/证据 ---
for (const n of arch.nodes) {
  (n.portIds || []).forEach((id) => { if (!portIds.has(id)) P("node " + n.id, "portId 不存在 → " + id); });
  if (n.parentId && !containerIds.has(n.parentId)) P("node " + n.id, "parentId 不存在 → " + n.parentId);
  (n.evidenceIds || []).forEach((id) => { if (!evIds.has(id)) P("node " + n.id, "evidenceId 不存在 → " + id); });
  if (!spatial.nodeOffsets[n.id]) P("node " + n.id, "spatial.nodeOffsets 缺失 → 3D 中会被静默跳过");
  if (!spatial.gemstoneMapping.byNode[n.id]) P("node " + n.id, "gemstoneMapping 缺失 → 3D 无形状");
  if (!["Actor", "Input", "Processor", "Decision", "Storage", "Message", "Interface", "Security", "Agent", "Composition", "Output"].includes(n.role)) {
    P("node " + n.id, "role 不是合法功能角色 → " + n.role + "（3D 几何退化为默认体）");
  }
}
// --- 端口 → 节点 ---
for (const p of arch.ports) {
  if (!nodeIds.has(p.nodeId)) P("port " + p.id, "nodeId 不存在 → " + p.nodeId);
}
// --- 边 → 节点/端口/证据 ---
for (const e of arch.edges) {
  if (!nodeIds.has(e.sourceId)) P("edge " + e.id, "sourceId 不存在 → " + e.sourceId);
  if (!nodeIds.has(e.targetId)) P("edge " + e.id, "targetId 不存在 → " + e.targetId);
  if (e.sourcePortId && !portIds.has(e.sourcePortId)) P("edge " + e.id, "sourcePortId 不存在 → " + e.sourcePortId);
  if (e.targetPortId && !portIds.has(e.targetPortId)) P("edge " + e.id, "targetPortId 不存在 → " + e.targetPortId);
  (e.evidenceIds || []).forEach((id) => { if (!evIds.has(id)) P("edge " + e.id, "evidenceId 不存在 → " + id); });
}
// --- 容器父子 ---
for (const c of arch.containers) {
  if (c.parentId && !containerIds.has(c.parentId)) P("container " + c.id, "parentId 不存在 → " + c.parentId);
}
// --- 需求 → 证据/测试 ---
for (const r of requirements.requirements) {
  (r.evidenceIds || []).forEach((id) => { if (!evIds.has(id)) P("requirement " + r.id, "evidenceId 不存在 → " + id); });
}
// --- 空间房间 → 节点 ---
for (const [rid, room] of Object.entries(spatial.rooms)) {
  if (!containerIds.has(rid)) P("spatial room " + rid, "对应容器不存在");
  (room.nodes || []).forEach((id) => { if (!nodeIds.has(id)) P("spatial room " + rid, "含不存在的节点 → " + id); });
}

if (problems.length) {
  console.error("=== 引用一致性校验失败 (" + problems.length + ") ===");
  problems.forEach((p) => console.error("  ✗ " + p));
  process.exit(1);
}
console.log("引用一致性校验通过：节点 " + nodeIds.size + " / 边 " + edgeIds.size + " / 端口 " + portIds.size +
  " / 规则 " + rules.rules.length + " / Case " + cases.cases.length + " / 预设 " + scen.presets.length);
