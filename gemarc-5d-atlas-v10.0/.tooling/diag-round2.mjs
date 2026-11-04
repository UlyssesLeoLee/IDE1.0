import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const G = require(path.join(ROOT, "src", "graph.js"));
const C = require(path.join(ROOT, "src", "causal.js"));
const E = require(path.join(ROOT, "src", "engine.js"));
const fs = require("node:fs");

const mkNode = (id) => ({ id, kind: "m", name: id, qualifiedName: id, role: "Processor", domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: [] });
const mkEdge = (id, s, t) => ({ id, sourceId: s, targetId: t, sourcePortId: null, targetPortId: null, type: "calls", directed: true, guardId: null, properties: {}, verification: "Implemented", evidenceIds: [] });

const gMulti = G.createGraph([mkNode("A"), mkNode("B")], [mkEdge("X1", "A", "B"), mkEdge("X2", "A", "B")]);
console.log("MultiEdge: outEdges(A).length =", gMulti.outEdges("A").length, " caps.MultiEdge =", gMulti.capabilities.MultiEdge);
console.log("  capabilities dump:", JSON.stringify(gMulti.capabilities));

const gSelf = G.createGraph([mkNode("A")], [mkEdge("S1", "A", "A")]);
console.log("SelfLoop: outEdges(A).length =", gSelf.outEdges("A").length, " cycles =", gSelf.findCycles().length);

// InsertElement
const arch = JSON.parse(fs.readFileSync(path.join(ROOT, "models", "architecture.json"), "utf8"));
const cr = JSON.parse(fs.readFileSync(path.join(ROOT, "models", "causal-rules.json"), "utf8"));
const cases = JSON.parse(fs.readFileSync(path.join(ROOT, "models", "cases.json"), "utf8"));
const eng = E.createEngine({ nodes: arch.nodes, edges: arch.edges, rules: cr.rules, cases: cases.cases, stateMachines: cr.stateMachines, version: "t" });
const ap = C.applyIntervention(eng.model, { id: "X", kind: "InsertElement", targetId: "E-005", params: { nodeId: "X-CACHE", name: "Vault Cache" } });
console.log("InsertElement error:", ap.error);
console.log("  E-005 still in edges?", ap.model.edges.some((e) => e.id === "E-005"));
console.log("  new edge ids:", ap.model.edges.map((e) => e.id).filter((i) => i.indexOf("X") !== -1));
console.log("  delta ops:", JSON.stringify(ap.delta.operations));

const eng2 = E.createEngine(ap.model);
const c = cases.cases.find((x) => x.id === "CASE-VAULT-WRITE-NORMAL");
const run = eng2.run(c.id, { seed: 1, inputs: Object.assign({}, c.inputs || {}) });
console.log("  transitions after rewrite:", run.events.filter((e) => e.kind === "Transition").map((e) => e.ruleId + "@" + e.edgeId));
