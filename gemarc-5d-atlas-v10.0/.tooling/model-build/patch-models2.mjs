import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const wr = (f, o) => fs.writeFileSync(ROOT + "/models/" + f, JSON.stringify(o, null, 2), "utf8");
const arch = rd("architecture.json");
const rules = rd("causal-rules.json");
const cases = rd("cases.json");

const eq = (n) => ({ type: "equals", left: { type: "var", var: "input:" + n }, right: { type: "literal", value: true } });
const andAll = (...gs) => ({ type: "and", args: gs });
const setGuard = (ruleId, g) => {
  const r = rules.rules.find((x) => x.id === ruleId);
  if (!r) throw new Error("no rule " + ruleId);
  r.guardAst = g;
};

// --- 桌面 / Web 模式互斥 + 终端与插件按需触发 ---
setGuard("R-FE-WB", andAll(eq("webModeAvailable")));
setGuard("R-FE-DT", andAll(eq("desktopModeAvailable"), eq("webview2Available")));
setGuard("R-FE2-WB", andAll(eq("webModeAvailable")));
setGuard("R-DT-STORE", andAll(eq("docRequested")));
setGuard("R-DT-PLUGIN", andAll(eq("pluginRequested")));
setGuard("R-DT-TERM", andAll(eq("terminalRequested")));
setGuard("R-WB-TERM", andAll(eq("webModeAvailable"), eq("terminalRequested")));

const base = {
  tokenValid: true, pathWithinRoot: true, sizeOk: true, utf8Ok: true, fileExists: true,
  methodKnown: true, bufferExists: true, providerRegistered: false, powershellAvailable: true,
  rgAvailable: true, isGitRepo: true, webview2Available: true,
  webModeAvailable: false, desktopModeAvailable: false, docRequested: false,
  pluginRequested: false, terminalRequested: false
};
const WEB = ["C-NORMAL-EDIT", "C-AUTH-REJECT", "C-PATH-ESCAPE", "C-SIZE-LIMIT", "C-ENCODING-REJECT", "C-FILE-NOT-FOUND", "C-CONCURRENT-TABS", "C-TERMINAL-WEB"];
const DESK = ["C-TERMINAL-PS", "C-TERMINAL-MISSING", "C-PLUGIN-TIMEOUT", "C-E2E-CLICK-WEBVIEW2", "C-DESKTOP-INVOKE"];
for (const c of cases.cases) {
  c.inputs = Object.assign({}, base, c.inputs || {});
  c.inputs.webModeAvailable = WEB.includes(c.id);
  c.inputs.desktopModeAvailable = DESK.includes(c.id);
}
// 显式覆盖（覆盖上面的批量设定）
const set = (id, patch) => Object.assign(cases.cases.find((c) => c.id === id).inputs, patch);
// --- 插件超时 Case 改为从桌面外壳进入（让 R-DT-PLUGIN 真的可达）---
const plug = cases.cases.find((c) => c.id === "C-PLUGIN-TIMEOUT");
plug.entryNodeId = "N-FE-EDITORHTML";
plug.entryNodes = ["N-FE-EDITORHTML"];
plug.entryState = "Running";
plug.nodeIds = ["N-FE-EDITORHTML", "N-DT-TAURI", "N-DT-PLUGIN"];
plug.edgeIds = ["E-FE-TO-DT", "E-DT-TO-PLUGIN", "E-PLUGIN-TO-DT", "E-PLUGIN-RETRY"];
plug.scenario.steps = ["桌面外壳 invoke plugin_load", "wasm 插件执行", "耗时 500ms 超过 100ms 时限", "失败后按 retry.max=2 退避重试", "自环边进入 Recovered"];

// --- 新增：桌面正常路径 + Web 终端 Case ---
if (!cases.cases.find((c) => c.id === "C-DESKTOP-INVOKE")) {
  cases.cases.push({
    id: "C-DESKTOP-INVOKE", name: "桌面端 Tauri invoke 正常打开文档",
    origin: "ObservedReplay",
    scenario: { steps: ["WebView2 内的前端调用 Tauri invoke", "doc_open 命令进入 DocStore", "Document 绑定行存储并写入撤销快照"], documentation: "IR-01；desktop doc_* command 实测存在（EV-CODE-0028）" },
    expected: ["桌面链路全程无阻断", "Document 与 Buffer 终态 Completed"],
    entryNodeId: "N-FE-EDITORHTML", entryNodes: ["N-FE-EDITORHTML"], entryState: "Running",
    inputs: Object.assign({}, base, { desktopModeAvailable: true, docRequested: true }),
    seed: 1,
    nodeIds: ["N-FE-EDITORHTML", "N-DT-TAURI", "N-KN-DOCSTORE", "N-KN-DOCUMENT", "N-EC-BUFFER", "N-EC-UNDO"],
    edgeIds: ["E-FE-TO-DT", "E-DT-TO-STORE", "E-STORE-TO-DOCUMENT", "E-DOCUMENT-TO-BUFFER", "E-BUFFER-TO-UNDO", "E-UNDO-TO-DOCUMENT"],
    evidenceIds: ["EV-CODE-0028", "EV-CODE-0016"],
    linkedTestIds: ["IT-DESKTOP-CMDS-01"]
  });
}
if (!cases.cases.find((c) => c.id === "C-TERMINAL-WEB")) {
  cases.cases.push({
    id: "C-TERMINAL-WEB", name: "Web 端终端（HTTP 轮询）拉起 PowerShell",
    origin: "ObservedReplay",
    scenario: { steps: ["POST /api/terminal_create", "spawn powershell.exe", "轮询 /api/terminal_output?id=", "输入经 /api/terminal_input 回灌"], documentation: "EV-CODE-0021 的 5 条终端路由" },
    expected: ["终端层进入并回传分发器", "输出可被前端轮询取回"],
    entryNodeId: "N-WB-SERVER", entryNodes: ["N-WB-SERVER"], entryState: "Running",
    inputs: Object.assign({}, base, { webModeAvailable: true, terminalRequested: true }),
    seed: 1,
    nodeIds: ["N-WB-SERVER", "N-WB-TERMINAL", "N-OS-PWSH"],
    edgeIds: ["E-WB-TO-TERM", "E-TERM-TO-PSH", "E-PSH-TO-TERM", "E-TERM-TO-WB"],
    evidenceIds: ["EV-CODE-0021"],
    linkedTestIds: []
  });
}
set("C-NORMAL-EDIT", { docRequested: true });
set("C-CONCURRENT-TABS", { docRequested: true });
set("C-AUTH-REJECT", { tokenValid: false, docRequested: true });
set("C-PATH-ESCAPE", { pathWithinRoot: false, docRequested: true });
set("C-SIZE-LIMIT", { sizeOk: false, docRequested: true });
set("C-ENCODING-REJECT", { utf8Ok: false, docRequested: true });
set("C-FILE-NOT-FOUND", { fileExists: false, docRequested: true });
set("C-TERMINAL-PS", { terminalRequested: true });
set("C-TERMINAL-MISSING", { terminalRequested: true, powershellAvailable: false, webview2Available: false });
set("C-TERMINAL-WEB", { terminalRequested: true, docRequested: false });
set("C-PLUGIN-TIMEOUT", { pluginRequested: true });
set("C-E2E-CLICK-WEBVIEW2", { webview2Available: false, powershellAvailable: false });
set("C-DESKTOP-INVOKE", { docRequested: true });


wr("architecture.json", arch);
wr("causal-rules.json", rules);
wr("cases.json", cases);
console.log("patch2: rules=" + rules.rules.length, "cases=" + cases.cases.length);
