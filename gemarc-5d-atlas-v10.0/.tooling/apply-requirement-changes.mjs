/**
 * .tooling/apply-requirement-changes.mjs — 把工作台导出的变更提案合回 models/
 *
 * 用法：
 *   node .tooling/apply-requirement-changes.mjs --file proposal.json                  # 试运行（只报差异）
 *   node .tooling/apply-requirement-changes.mjs --file proposal.json --apply          # 真正写入
 *   node .tooling/apply-requirement-changes.mjs --file proposal.json --apply --matrix # 同时补追踪矩阵行
 *   node .tooling/apply-requirement-changes.mjs --file proposal.json --root <dir>     # 指定模型根（默认本项目）
 *
 * 安全约束：
 *   - 默认**只试运行**，不写任何文件。
 *   - 只改 models/requirements.json（意图/判据/状态/负责人/变更类型）与可选的 models/traceability.json。
 *   - 写前自动备份到 models/.backup/，写后打印逐字段 diff。
 *   - 需求 ID 不存在 / 提案 schema 不符 → 直接失败，不做半截修改。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
let file = null, rootArg = null;
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (a === "--file") file = argv[++i];
  else if (a === "--root") rootArg = argv[++i];
  else if (a === "--apply" || a === "--matrix") continue;
  else if (!file) file = a;            // 兼容：第一个非选项参数即提案文件
}
const ROOT = path.resolve(rootArg || path.join(HERE, ".."));
const APPLY = argv.includes("--apply");
const MATRIX = argv.includes("--matrix");

if (!file) {
  console.error("用法: node .tooling/apply-requirement-changes.mjs --file <proposal.json> [--root <dir>] [--apply] [--matrix]");
  process.exit(2);
}
const proposalPath = path.resolve(file);
if (!fs.existsSync(proposalPath)) {
  console.error("找不到提案文件: " + proposalPath);
  process.exit(2);
}
let proposal;
try {
  proposal = JSON.parse(fs.readFileSync(proposalPath, "utf8"));
} catch (e) {
  console.error("提案文件不是合法 JSON: " + e.message);
  process.exit(2);
}
if (!proposal.schema || proposal.schema.indexOf("ide1-requirement-change/") !== 0) {
  console.error("提案 schema 不符（期望 ide1-requirement-change/v*）: " + proposal.schema);
  process.exit(2);
}
if (!Array.isArray(proposal.changes) || !proposal.changes.length) {
  console.error("提案里没有 changes");
  process.exit(2);
}

const M = path.join(ROOT, "models");
const reqPath = path.join(M, "requirements.json");
const tracePath = path.join(M, "traceability.json");
if (!fs.existsSync(reqPath)) { console.error("找不到 " + reqPath); process.exit(2); }
const reqs = JSON.parse(fs.readFileSync(reqPath, "utf8"));
const trace = fs.existsSync(tracePath) ? JSON.parse(fs.readFileSync(tracePath, "utf8")) : { matrix: [] };

// ---- 校验 + 计算 diff ----
const diffs = [];
const errors = [];
for (const ch of proposal.changes) {
  const id = ch.requirementId;
  if (!id) { errors.push("变更条目缺少 requirementId"); continue; }
  const rec = reqs.requirements.find((r) => r.id === id);
  if (!rec) { errors.push("需求不存在: " + id); continue; }
  const fields = [];
  if (typeof ch.intent === "string" && ch.intent !== rec.intent) fields.push({ field: "intent", from: rec.intent, to: ch.intent });
  if (Array.isArray(ch.measurableAcceptance)) {
    const from = JSON.stringify(rec.measurableAcceptance || []);
    const to = JSON.stringify(ch.measurableAcceptance);
    if (from !== to) fields.push({ field: "measurableAcceptance", from: rec.measurableAcceptance, to: ch.measurableAcceptance });
  }
  if (typeof ch.status === "string" && ch.status !== (rec.status || "")) fields.push({ field: "status", from: rec.status, to: ch.status });
  if (typeof ch.owner === "string" && ch.owner !== (rec.owner || "")) fields.push({ field: "owner", from: rec.owner, to: ch.owner });
  diffs.push({ id, fields });
}
if (errors.length) {
  console.error("=== 校验失败，未做任何修改 ===");
  errors.forEach((e) => console.error("  ✗ " + e));
  process.exit(1);
}

console.log("=== 变更提案（来自需求调整工作台）===");
console.log("提案文件: " + proposalPath);
console.log("模型根:   " + M);
console.log("变更需求: " + proposal.changes.length + " 条；字段级差异: " +
  diffs.reduce((n, d) => n + d.fields.length, 0) + " 处");
diffs.forEach((d) => {
  console.log("  " + d.id + ": " + (d.fields.length ? d.fields.map((f) => f.field).join(", ") : "无差异"));
  d.fields.forEach((f) => {
    console.log("      " + f.field + ":");
    console.log("        - " + JSON.stringify(f.from));
    console.log("        + " + JSON.stringify(f.to));
  });
});

if (MATRIX) {
  const missing = proposal.changes.filter((ch) => !trace.matrix.find((m) => m.requirementId === ch.requirementId));
  if (missing.length) {
    console.log("\n=== 追踪矩阵将新增/补齐 ===");
    missing.forEach((ch) => {
      const aff = ch.affected || {};
      console.log("  + " + ch.requirementId +
        " | cases: " + (aff.cases || []).join(", ") +
        " | tests: " + (aff.tests || []).join(", ") +
        " | 落点文件: " + (aff.files || []).join(", "));
    });
  }
}

if (!APPLY) {
  console.log("\n（试运行结束，未修改任何文件。加上 --apply 真正写入。）");
  process.exit(0);
}

// ---- 真正写入（带备份） ----
const bakDir = path.join(M, ".backup");
fs.mkdirSync(bakDir, { recursive: true });
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
fs.copyFileSync(reqPath, path.join(bakDir, "requirements." + stamp + ".json"));
if (fs.existsSync(tracePath)) fs.copyFileSync(tracePath, path.join(bakDir, "traceability." + stamp + ".json"));

let changed = 0;
for (const d of diffs) {
  if (!d.fields.length) continue;
  const rec = reqs.requirements.find((r) => r.id === d.id);
  for (const f of d.fields) {
    if (f.field === "measurableAcceptance") rec.measurableAcceptance = f.to;
    else rec[f.field] = f.to;
  }
  const ch = proposal.changes.find((c) => c.requirementId === d.id);
  if (ch && ch.changeType) {
    rec.changeType = ch.changeType;
    rec.changeReason = ch.changeReason || "";
  }
  changed++;
}
fs.writeFileSync(reqPath, JSON.stringify(reqs, null, 2), "utf8");

if (MATRIX) {
  let added = 0;
  for (const ch of proposal.changes) {
    if (trace.matrix.find((m) => m.requirementId === ch.requirementId)) continue;
    const aff = ch.affected || {};
    trace.matrix.push({
      requirementId: ch.requirementId,
      promptIds: [],
      bdRef: "models/requirements.json#" + ch.requirementId,
      ddNodeIds: [],
      ddEdgeIds: [],
      screenIds: [],
      apiRefs: [],
      caseIds: aff.cases || [],
      testIds: aff.tests || []
    });
    added++;
  }
  if (added) fs.writeFileSync(tracePath, JSON.stringify(trace, null, 2), "utf8");
  console.log("追踪矩阵：新增 " + added + " 行（已有行不覆盖，保留原 ddNodeIds 等登记）");
}

console.log("\n=== 已写入 ===");
console.log("更新需求: " + changed + " 条 → models/requirements.json");
console.log("备份: models/.backup/*.json");
console.log("提示: 变更已并入模型正本，但仍需按 v10.0 流程补 ADR / 更新索引后再对外发布。");
