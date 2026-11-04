import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const wr = (f, o) => fs.writeFileSync(ROOT + "/models/" + f, JSON.stringify(o, null, 2), "utf8");
const rules = rd("causal-rules.json");
const cases = rd("cases.json");

// 修复 1：规则/Case 的 edgeId 写错 —— 该规则原本永不触发
const r = rules.rules.find((x) => x.id === "R-WB-TERM");
if (r && r.edgeId !== "E-WB-TO-TERMINAL") { r.edgeId = "E-WB-TO-TERMINAL"; r.note = (r.note || "") + " [修正] edgeId 曾误写为 E-WB-TO-TERM，导致永不触发；已由 verify-refs.mjs 捕获。"; }
const c = cases.cases.find((x) => x.id === "C-TERMINAL-WEB");
if (c) c.edgeIds = c.edgeIds.map((id) => (id === "E-WB-TO-TERM" ? "E-WB-TO-TERMINAL" : id));

// 修复 2：引擎中「终态节点不再扩散」，UNDO 到达 Completed 后不再回传 Document。
// 语义上 undo 结果要回到文档模型，所以 UNDO 的 Completed 不能是终止态。
const sm = rules.stateMachines.find((x) => x.id === "SM-UNDO");
if (sm) { sm.terminalStates = ["Failed"]; sm.note = "Completed 不是终止态：撤销结果必须回传到 Document，故该节点仍需继续扩散。"; }

wr("causal-rules.json", rules);
wr("cases.json", cases);
console.log("fixed: R-WB-TERM.edgeId ->", r ? r.edgeId : "(none)", "| SM-UNDO.terminalStates ->", sm ? sm.terminalStates : "(none)");
