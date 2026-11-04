// 修：E-026 id 冲突 —— 同步自环边改用 E-028；并做全局 id 唯一性校验
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const M = path.join(ROOT, "models");
const read = (n) => JSON.parse(fs.readFileSync(path.join(M, n), "utf8"));
const write = (n, o) => fs.writeFileSync(path.join(M, n), JSON.stringify(o, null, 2) + "\n", "utf8");

// ---- 1) architecture：把同步自环边补成 E-028 ----
{
  const a = read("architecture.json");
  const ids = a.edges.map((e) => e.id);
  console.log("edge ids:", ids.join(","));
  const dup = ids.filter((x, i) => ids.indexOf(x) !== i);
  console.log("duplicate edge ids:", dup.length ? dup.join(",") : "none");

  if (!a.edges.some((e) => e.id === "E-028")) {
    a.edges.push({
      id: "E-028",
      sourceId: "N-REMOTE-SYNC", targetId: "N-REMOTE-SYNC",
      sourcePortId: null, targetPortId: null,
      type: "fastForwardCheck", directed: true, guardId: null,
      properties: { note: "快进判定：git merge-base --is-ancestor 的结果落在同步节点自身（自环）" },
      verification: "Implemented",
      evidenceIds: ["EV-038"]
    });
    console.log("+ architecture edge E-028 (N-REMOTE-SYNC self-loop)");
  }
  write("architecture.json", a);
}

// ---- 2) causal-rules：同步规则改绑 E-028 ----
{
  const r = read("causal-rules.json");
  let n = 0;
  for (const rule of r.rules) {
    if ((rule.id === "R-SYNC-FASTFORWARD" || rule.id === "R-SYNC-NOT-FASTFORWARD") && rule.edgeId !== "E-028") {
      console.log(rule.id + ": " + rule.edgeId + " -> E-028");
      rule.edgeId = "E-028";
      n++;
    }
  }
  write("causal-rules.json", r);
  console.log("rules rebinding:", n);
}

// ---- 3) cases：同步 Case 的 edgeIds 补 E-028 ----
{
  const c = read("cases.json");
  for (const t of c.cases) {
    if (t.id === "CASE-SYNC-NOT-FAST-FORWARD" || t.id === "CASE-SYNC-FAST-FORWARD-PUSH") {
      t.edgeIds = Array.from(new Set((t.edgeIds || []).filter((x) => x !== "E-026").concat(["E-028"])));
      console.log(t.id + " edgeIds ->", t.edgeIds.join(","));
    }
  }
  write("cases.json", c);
}

// ---- 4) 全局唯一性校验 ----
{
  const a = read("architecture.json");
  const ev = read("evidence.json");
  const cr = read("causal-rules.json");
  const cs = read("cases.json");
  const problems = [];
  const uniq = (arr, label) => {
    const seen = new Set();
    arr.forEach((x) => {
      if (seen.has(x)) problems.push(label + " duplicate id: " + x);
      seen.add(x);
    });
  };
  uniq(a.nodes.map((n) => n.id), "node");
  uniq(a.edges.map((e) => e.id), "edge");
  uniq(a.containers.map((c) => c.id), "container");
  uniq(a.ports.map((p) => p.id), "port");
  uniq(ev.evidence.map((e) => e.id), "evidence");
  uniq(cr.rules.map((r) => r.id), "rule");
  uniq(cr.stateMachines.map((s) => s.id), "stateMachine");
  uniq(cs.cases.map((c) => c.id), "case");
  const nodeIds = new Set(a.nodes.map((n) => n.id));
  const edgeIds = new Set(a.edges.map((e) => e.id));
  a.edges.forEach((e) => {
    if (!nodeIds.has(e.sourceId)) problems.push("edge " + e.id + " source missing: " + e.sourceId);
    if (!nodeIds.has(e.targetId)) problems.push("edge " + e.id + " target missing: " + e.targetId);
  });
  cr.rules.forEach((r) => {
    if (r.edgeId && !edgeIds.has(r.edgeId)) problems.push("rule " + r.id + " edge missing: " + r.edgeId);
    if (r.sourceNodeId && !nodeIds.has(r.sourceNodeId)) problems.push("rule " + r.id + " source missing: " + r.sourceNodeId);
    if (r.targetNodeId && !nodeIds.has(r.targetNodeId)) problems.push("rule " + r.id + " target missing: " + r.targetNodeId);
  });
  console.log(problems.length ? "PROBLEMS:\n  " + problems.join("\n  ") : "uniqueness + referential integrity OK");
}
