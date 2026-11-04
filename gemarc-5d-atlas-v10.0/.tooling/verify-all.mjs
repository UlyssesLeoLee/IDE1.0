// 最终全链路验证：build → 语法门禁 → 模型键名门禁 → 单测 → 产物一致性 → 交付物清点
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import crypto from "node:crypto";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

const ROOT = path.resolve(process.argv[2] || ".");
const line = (s) => console.log(s);
const sha = (p) => crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex");

line("=== 1. 源码语法（node:vm 逐文件）===");
let synOk = 0;
const srcFiles = fs.readdirSync(path.join(ROOT, "src")).filter((x) => x.endsWith(".js")).sort();
for (const f of srcFiles) {
  try {
    new vm.Script(fs.readFileSync(path.join(ROOT, "src", f), "utf8"), { filename: f });
    synOk++;
  } catch (e) {
    line("  FAIL " + f + " => " + e.message);
  }
}
line("  " + synOk + " / " + srcFiles.length + " 个模块语法通过");

line("=== 2. 模型 JSON 可解析 + id 唯一 ===");
const M = {};
for (const f of fs.readdirSync(path.join(ROOT, "models")).sort()) {
  const p = path.join(ROOT, "models", f);
  if (f.endsWith(".json")) M[f.replace(".json", "")] = JSON.parse(fs.readFileSync(p, "utf8"));
}
line("  models 解析 OK: " + Object.keys(M).length + " 个 json");
const dupCheck = (arr, label) => {
  const s = new Set(); const d = [];
  arr.forEach((x) => { if (s.has(x)) d.push(x); s.add(x); });
  if (d.length) line("  DUP " + label + ": " + d.join(","));
  return d.length === 0;
};
const uniqOk =
  dupCheck(M.architecture.nodes.map((n) => n.id), "node") &&
  dupCheck(M.architecture.edges.map((e) => e.id), "edge") &&
  dupCheck(M["causal-rules"].rules.map((r) => r.id), "rule") &&
  dupCheck(M.cases.cases.map((c) => c.id), "case") &&
  dupCheck(M.evidence.evidence.map((e) => e.id), "evidence") &&
  dupCheck(M["causal-rules"].stateMachines.map((s) => s.id), "stateMachine");
line("  id 唯一性: " + (uniqOk ? "OK" : "有重复"));

line("=== 3. 引用完整性 ===");
const nodeIds = new Set(M.architecture.nodes.map((n) => n.id));
const edgeIds = new Set(M.architecture.edges.map((e) => e.id));
const evIds = new Set(M.evidence.evidence.map((e) => e.id));
const caseIds = new Set(M.cases.cases.map((c) => c.id));
const reqIds = new Set(M.requirements.requirements.map((r) => r.id));
const dangling = [];
M.architecture.edges.forEach((e) => {
  if (!nodeIds.has(e.sourceId)) dangling.push("edge " + e.id + " src " + e.sourceId);
  if (!nodeIds.has(e.targetId)) dangling.push("edge " + e.id + " tgt " + e.targetId);
});
M["causal-rules"].rules.forEach((r) => {
  if (r.edgeId && !edgeIds.has(r.edgeId)) dangling.push("rule " + r.id + " edge " + r.edgeId);
});
M.cases.cases.forEach((c) => {
  (c.evidenceIds || []).forEach((i) => { if (!evIds.has(i)) dangling.push("case " + c.id + " ev " + i); });
  (c.edgeIds || []).forEach((i) => { if (!edgeIds.has(i)) dangling.push("case " + c.id + " edge " + i); });
});
M.prompts.groupB_atlasImplementationPrompts.prompts.forEach((p) => {
  (p.requirementIds || []).forEach((i) => { if (!reqIds.has(i)) dangling.push("prompt " + p.promptId + " req " + i); });
  (p.caseIds || []).forEach((i) => { if (!caseIds.has(i)) dangling.push("prompt " + p.promptId + " case " + i); });
  (p.evidenceIds || []).forEach((i) => { if (!evIds.has(i)) dangling.push("prompt " + p.promptId + " ev " + i); });
});
line("  悬空引用: " + (dangling.length ? dangling.join("; ") : "0"));

line("=== 4. 全部 Case 实跑（转移数）===");
const E = require(path.join(ROOT, "src", "engine.js"));
const eng = E.createEngine({
  nodes: M.architecture.nodes, edges: M.architecture.edges,
  rules: M["causal-rules"].rules, cases: M.cases.cases,
  stateMachines: M["causal-rules"].stateMachines, version: "t"
});
let zero = 0;
for (const c of M.cases.cases) {
  const r = eng.run(c.id, { seed: c.seed || 1, inputs: Object.assign({}, c.inputs || {}) });
  const t = r.events.filter((e) => e.kind === "Transition");
  if (t.length === 0) { zero++; line("  ZERO " + c.id); }
}
line("  零转移 Case: " + zero + " / " + M.cases.cases.length);

line("=== 5. 产物一致性 ===");
const idx = path.join(ROOT, "index.html");
const dist = path.join(ROOT, "dist", "index.html");
const same = sha(idx) === sha(dist);
line("  index.html sha256 : " + sha(idx).slice(0, 16));
line("  dist/index.html  : " + (same ? "同字节 ✓" : "不一致 ✗"));
line("  index.html 大小  : " + (fs.statSync(idx).size / 1024).toFixed(1) + " KB");

line("=== 6. 交付物清点 ===");
const walk = (d, out = []) => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    if (e.name === "node_modules" || e.name === ".tooling") continue;
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p, out); else out.push(p);
  }
  return out;
};
const files = walk(ROOT);
const total = files.reduce((s, p) => s + fs.statSync(p).size, 0);
line("  文件 " + files.length + " 个 · " + (total / 1024).toFixed(1) + " KB");

const ok = synOk === srcFiles.length && uniqOk && dangling.length === 0 && zero === 0 && same;
line("");
line(ok ? "=== 全链路验证：PASS ===" : "=== 全链路验证：FAIL ===");
process.exit(ok ? 0 : 1);
