import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
const ROOT = "E:\\IDE1.0\\gemarc-5d-atlas-v10.0";
const PROJ = "E:\\IDE1.0";
const rd = (p) => JSON.parse(fs.readFileSync(p, "utf8"));
const M = (n) => rd(path.join(ROOT, "models", n));
const arch = M("architecture.json"), ev = M("evidence.json"), rules = M("causal-rules.json");
const cases = M("cases.json"), scen = M("scenario-deltas.json"), diag = M("diagram-definitions.json");
const proto = M("prototype.json");
const evById = new Map(ev.evidence.map(e => [e.id, e]));

const ctx = { console, setTimeout, clearTimeout, Date, Math, JSON }; ctx.globalThis = ctx; vm.createContext(ctx);
for (const f of ["engine.js","causal.js","diagrams.js","prototype.js"]) {
  vm.runInContext(fs.readFileSync(path.join(ROOT,"src",f),"utf8"), ctx, { filename: f });
}
const Engine = ctx.GemArcEngine, Causal = ctx.GemArcCausal, Diagrams = ctx.GemArcDiagrams, Proto = ctx.GemArcPrototype;
const out = [];
const say = (...a) => { const s = a.join(" "); out.push(s); console.log(s); };

say("################ C2 : Implemented 证据强度 ################");
const implNodes = arch.nodes.filter(n => n.verification === "Implemented");
const implEdges = arch.edges.filter(e => e.verification === "Implemented");
say(`Implemented nodes=${implNodes.length} / edges=${implEdges.length} (of ${arch.nodes.length}/${arch.edges.length})`);
const weak = [];
for (const n of implNodes) {
  const evs = (n.evidenceIds||[]).map(id => evById.get(id)).filter(Boolean);
  if (!evs.length) { weak.push({id:n.id, why:"NO EVIDENCE"}); continue; }
  const kinds = evs.map(e => e.kind);
  const nonCode = evs.filter(e => e.kind !== "SourceCode");
  const cargo = evs.filter(e => e.path && /Cargo\.toml$/i.test(e.path));
  if (nonCode.length === evs.length) { weak.push({id:n.id, why:"only non-SourceCode evidence: "+kinds.join(",")+" ["+evs.map(e=>e.id).join(",")+"]"}); continue; }
  if (cargo.length === evs.length) { weak.push({id:n.id, why:"only Cargo.toml dependency declaration: "+cargo.map(e=>e.path+":"+e.startLine+"-"+e.endLine).join(",")}); continue; }
  // per-evidence path/line validation against the real repo
  for (const e of evs) {
    if (!e.path) continue;
    const abs = path.join(PROJ, e.path);
    if (!fs.existsSync(abs)) { weak.push({id:n.id, why:`evidence ${e.id} path MISSING in IDE1.0: ${e.path}`}); continue; }
    const st = fs.statSync(abs);
    if (st.isDirectory()) { weak.push({id:n.id, why:`evidence ${e.id} points at a DIRECTORY: ${e.path}`}); continue; }
    const lines = fs.readFileSync(abs,"utf8").split(/\r?\n/);
    const total = lines.length;
    const sl = e.startLine, el = e.endLine;
    if (sl==null || el==null) { weak.push({id:n.id, why:`evidence ${e.id} has no line range`}); continue; }
    if (sl < 1 || el < sl || el > total) { weak.push({id:n.id, why:`evidence ${e.id} line range ${sl}-${el} OUT OF BOUNDS (file has ${total} lines) path=${e.path}`}); continue; }
    const body = lines.slice(sl-1, el).join("\n");
    const isTrait = /^\s*(pub\s+)?(async\s+)?trait\s+\w+/m.test(body) && !/fn\s+\w+\s*\(/.test(body);
    const isDto = /derive\([^)]*Deserialize/.test(body) && !/impl\s/.test(body) && !/\bfn\b/.test(body);
    const isMock = /\bmock!\b|\bMock\b|todo!\(|unimplemented!\(/.test(body);
    const isTodo = /\bTODO\b|\bFIXME\b/.test(body);
    const fnCount = (body.match(/\bfn\s+\w+/g)||[]).length;
    if (isMock) weak.push({id:n.id, why:`evidence ${e.id} body contains mock/todo!/unimplemented!: ${e.path}:${sl}-${el}`});
    else if (isTrait && fnCount===0) weak.push({id:n.id, why:`evidence ${e.id} is a TRAIT DECLARATION only (no fn body): ${e.path}:${sl}-${el}`});
    else if (isDto && fnCount===0) weak.push({id:n.id, why:`evidence ${e.id} looks like a DTO (derive only, no fn): ${e.path}:${sl}-${el}`});
    else if (isTodo && fnCount===0) weak.push({id:n.id, why:`evidence ${e.id} body is TODO/FIXME only: ${e.path}:${sl}-${el}`});
  }
}
say(`WEAK/INVALID Implemented nodes: ${weak.length}`);
weak.forEach(w => say(`   !! ${w.id}: ${w.why}`));

say("");
say("################ C4 : diagramType 分派 ################");
const src = fs.readFileSync(path.join(ROOT,"src","diagrams.js"),"utf8");
const lines = src.split(/\r?\n/);
const rdefs = [];
lines.forEach((l,i)=>{ const m = l.match(/^  RENDERERS\.([A-Za-z_]+) = function \(d, w, h\)/); if(m) rdefs.push({type:m[1], line:i+1}); });
say(`renderers registered=${rdefs.length} -> ${rdefs.map(r=>r.type).join(",")}`);
say(`distinct diagramTypes in model=${new Set(diag.diagrams.map(d=>d.diagramType)).size} / definitions=${diag.diagrams.length}`);
const unsupported = diag.diagrams.filter(d => Diagrams.render(d,720,460).includes("Unsupported diagramType"));
say(`definitions rendering "Unsupported diagramType": ${unsupported.length}`);
// field requirements per renderer
const reqFields = {};
rdefs.forEach((r,idx)=>{
  const start = r.line-1;
  const end = idx+1<rdefs.length ? rdefs[idx+1].line-1 : lines.length;
  const body = lines.slice(start,end).join("\n");
  const fset = new Set();
  for (const m of body.matchAll(/\bd\.([a-zA-Z][a-zA-Z0-9]*)/g)) fset.add(m[1]);
  reqFields[r.type] = [...fset];
});
let fieldMiss = 0;
for (const d of diag.diagrams) {
  const need = (reqFields[d.diagramType]||[]).filter(f=>f!=="title");
  const missing = need.filter(f => !d[f] || (Array.isArray(d[f]) && d[f].length===0));
  if (missing.length) { fieldMiss++; say(`   !! ${d.id} (${d.diagramType}) missing fields used by its renderer: ${missing.join(",")}`); }
}
say(`definitions missing renderer-required fields: ${fieldMiss}`);
// distinct output per type (no silent fallback)
const sig = new Map();
for (const d of diag.diagrams) {
  const svg = Diagrams.render(d,720,460);
  const key = d.diagramType;
  if (!sig.has(key)) sig.set(key, svg);
}
let dupSig = 0;
const seen = new Map();
for (const [t,s] of sig) { if (seen.has(s)) { dupSig++; say(`   !! ${t} renders byte-identical to ${seen.get(s)}`); } else seen.set(s,t); }
say(`distinct diagramTypes producing byte-identical SVG: ${dupSig}`);
const svgLens = [...sig].map(([t,s])=>t+"="+s.length).join(" ");
say(`svg lengths: ${svgLens}`);

say("");
say("################ C7 : 无关干预不得制造分歧 ################");
const model = { version: arch.version, nodes: arch.nodes, edges: arch.edges, rules: rules.rules, stateMachines: rules.stateMachines, cases: cases.cases };
const engine = Engine.createEngine(model);
const psh = arch.nodes.find(n=>n.id==="N-OS-PWSH");
say(`N-OS-PWSH exists in model: ${!!psh}`);
// which nodes are actually touched by C-PATH-ESCAPE
const runEsc = engine.run("C-PATH-ESCAPE",{seed:1});
const touched = new Set(runEsc.events.map(e=>e.sourceNodeId).concat(runEsc.events.map(e=>e.targetNodeId)));
say(`nodes touched by C-PATH-ESCAPE: ${[...touched].filter(Boolean).sort().join(",")}`);
const tests = [
  ["C-PATH-ESCAPE","DisableNode","N-OS-PWSH"],
  ["C-PATH-ESCAPE","DisableNode","N-WB-AUTH"],
  ["C-PATH-ESCAPE","DisableEdge","E-WB-TO-AUTH"],
  ["C-NORMAL-EDIT","DisableNode","N-OS-PWSH"],
  ["C-TERMINAL-PS","DisableNode","N-OS-PWSH"],
  ["C-PATH-ESCAPE","ChangeGuard","E-WB-TO-AUTH"]
];
for (const [c,kind,tid] of tests) {
  const ce = Causal.compare(engine, c, { kind, targetId: tid, params: kind==="ChangeGuard"?{guardAst:{type:"never"}}:{} }, { seed: 1 });
  const isTouched = touched.has(tid);
  say(`   ${c} ${kind} ${tid} => outcome=${ce.outcome} firstDiv=${ce.firstDivergence?ce.firstDivergence.atSequence:"null"} affected=${ce.affectedNodes.length} targetTouchedByCase=${isTouched} unknownReason=${ce.unknownReason||"-"}`);
}

say("");
say("################ C8/C9 : InsertElement 真改写（引擎实跑） ################");
const applied = Causal.applyIntervention(model, { kind:"InsertElement", targetId:"E-WB-TO-STORE",
  params:{ nodeId:"X-VALIDATOR", name:"Injected Validator", kind:"Processor", role:"Processor" }, id:"SD-TEST", caseId:"C-NORMAL-EDIT" });
const op = (applied.delta.operations||[])[0]||{};
say(`op=${op.op} removedEdgeIds=${JSON.stringify(op.removedEdgeIds)} newEdgeIds=${JSON.stringify(op.newEdgeIds)} mutates=${applied.delta.mutates}`);
say(`old edge gone from experimental model: ${!applied.model.edges.some(e=>e.id==="E-WB-TO-STORE")}`);
say(`new node present: ${applied.model.nodes.some(n=>n.id==="X-VALIDATOR")}`);
const rulesForNew = applied.model.rules.filter(r=>r.edgeId==="E-WB-TO-STORE->X"||r.edgeId==="E-WB-TO-STORE-X->");
say(`new rules created for new edges: ${rulesForNew.length} -> ${rulesForNew.map(r=>r.id+" edge="+r.edgeId+" when="+r.when+" then="+r.then).join(" | ")}`);
const engB = Engine.createEngine(applied.model);
const runB = engB.run("C-NORMAL-EDIT",{seed:1});
const firedEdges = [...new Set(runB.events.filter(e=>e.kind==="Transition").map(e=>e.edgeId))];
say(`transitions fired on rewritten model: ${firedEdges.join(",")}`);
say(`old edge E-WB-TO-STORE fired? ${firedEdges.includes("E-WB-TO-STORE")}`);
say(`new edges fired? toX=${firedEdges.includes("E-WB-TO-STORE->X")} Xto=${firedEdges.includes("E-WB-TO-STORE-X->")}`);
say(`X-VALIDATOR reached in run? ${!!runB.finalStates["X-VALIDATOR"]} state=${runB.finalStates["X-VALIDATOR"]?runB.finalStates["X-VALIDATOR"].state:"-"}`);
say(`original fact graph untouched? E-WB-TO-STORE still in engine.model: ${engine.model.edges.some(e=>e.id==="E-WB-TO-STORE")}`);

say("");
say("################ C10 : 原型序列化内容 ################");
const pe = Proto.createEditor ? Proto.createEditor(proto) : null;
say(`Prototype API keys: ${Object.keys(Proto).join(",")}`);
if (pe) {
  const ser = pe.serialise ? pe.serialise() : null;
  if (ser) {
    say(`serialise() top-level keys: ${Object.keys(ser).join(",")}`);
    say(`widgets=${(ser.widgets||[]).length} actions=${(ser.widgets||[]).reduce((n,w)=>n+((w.actions||[]).length),0)}`);
    say(`has stateTable=${!!ser.stateTable} has fixtures=${!!ser.fixtures||!!ser.fixtureTable} has guards=${!!ser.guards}`);
    say(`stateTable=${JSON.stringify(ser.stateTable).slice(0,200)}`);
  }
}

say("");
say("################ 报告数字核对 ################");
say(`evidence.total=${ev.evidence.length} EV-CODE=${ev.evidence.filter(e=>/^EV-CODE/.test(e.id)).length} EV-DOC=${ev.evidence.filter(e=>/^EV-DOC/.test(e.id)).length}`);
say(`nodes=${arch.nodes.length} edges=${arch.edges.length} ports=${arch.ports.length} containers=${arch.containers.length}`);
say(`cases=${cases.cases.length} rules=${rules.rules.length} presets=${scen.presets.length} diagrams=${diag.diagrams.length} prompts=${(proto&&0)||0}`);
say(`stateMachines=${rules.stateMachines.length}`);
const req = M("requirements.json"); say(`requirements=${req.requirements.length} excluded=${(req.exclusions||req.excluded||[]).length}`);
const sm = rules.stateMachines.map(s=>s.nodeId);
const missingSm = arch.nodes.filter(n=>n.verification==="Implemented" && !sm.includes(n.id)).map(n=>n.id);
say(`Implemented nodes WITHOUT a state machine: ${missingSm.length} -> ${missingSm.join(",")}`);
fs.writeFileSync(path.join(ROOT,"qa","audit-tmp","adv-node-out.txt"), out.join("\n"), "utf8");
