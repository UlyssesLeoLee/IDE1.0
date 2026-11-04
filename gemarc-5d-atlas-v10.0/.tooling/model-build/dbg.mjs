import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const ctx = { console, setTimeout, clearTimeout, Date, Math, JSON };
ctx.globalThis = ctx; vm.createContext(ctx);
for (const f of ["engine.js", "causal.js"]) vm.runInContext(fs.readFileSync(ROOT + "/src/" + f, "utf8"), ctx, { filename: f });
const arch = rd("architecture.json"), rules = rd("causal-rules.json"), cases = rd("cases.json");
const eng = ctx.GemArcEngine.createEngine({
  version: arch.version, nodes: arch.nodes, edges: arch.edges, rules: rules.rules,
  stateMachines: rules.stateMachines, cases: cases.cases
});
for (const id of process.argv.slice(2)) {
  const c = cases.cases.find((x) => x.id === id);
  const run = eng.run(id, { seed: 1 });
  console.log("### " + id + "  inputs=" + JSON.stringify(c.inputs));
  run.events.forEach((e) => console.log(`  #${e.sequence} t=${e.logicalTime} ${e.kind.padEnd(18)} ${e.sourceNodeId} -> ${e.targetNodeId}  rule=${e.ruleId} edge=${e.edgeId} ${e.guard ? "guard=" + e.guard : ""}`));
  console.log("  final:", JSON.stringify(Object.fromEntries(Object.entries(run.finalStates).map(([k, v]) => [k, v.state]))));
}
