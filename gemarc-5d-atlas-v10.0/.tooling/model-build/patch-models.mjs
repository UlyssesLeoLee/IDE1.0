import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const wr = (f, o) => fs.writeFileSync(ROOT + "/models/" + f, JSON.stringify(o, null, 2), "utf8");
const arch = rd("architecture.json");
const rules = rd("causal-rules.json");
const cases = rd("cases.json");

// ---------- (a) 新增第二客户端节点（并发场景需要两个真实前端入口） ----------
if (!arch.nodes.some((n) => n.id === "N-FE-CLIENT2")) {
  arch.nodes.push({
    id: "N-FE-CLIENT2", kind: "Interface", name: "第二客户端窗口",
    qualifiedName: "ide_shell_web::editor.html#client2",
    role: "第二个浏览器窗口/第二个 WebView，共享同一个本地 HTTP 服务",
    domainId: "Interaction", parentId: "RM-EDITAREA", portIds: ["P-FE-FETCHAPI"],
    properties: { purpose: "并发场景的真实来源：两个客户端同时打同一个 127.0.0.1:8123", note: "事实依据：本地服务单例 + 令牌校验（EV-CODE-0025）" },
    verification: "Inferred", evidenceIds: ["EV-CODE-0025", "EV-CODE-0052"]
  });
}
if (!arch.edges.some((e) => e.id === "E-FE2-TO-WB")) {
  arch.edges.push({
    id: "E-FE2-TO-WB", sourceId: "N-FE-CLIENT2", targetId: "N-WB-SERVER",
    sourcePortId: "P-FE-FETCHAPI", targetPortId: null, type: "http", directed: true, guardId: null,
    properties: { note: "第二客户端走同一条 HTTP 通道（服务端串行处理）" },
    verification: "Inferred", evidenceIds: ["EV-CODE-0025"]
  });
}

// ---------- (b) 给规则加输入守卫，避免无关路径互相污染 ----------
const guardFor = (inputName) => ({
  type: "equals", left: { type: "var", var: "input:" + inputName }, right: { type: "literal", value: true }
});
const setGuard = (ruleId, inputName) => {
  const r = rules.rules.find((x) => x.id === ruleId);
  if (!r) return;
  r.guardAst = { type: "and", args: [r.guardAst, guardFor(inputName)] };
  r.note = (r.note ? r.note + " " : "") + "[门禁] 额外要求 input:" + inputName + " 为真";
};
setGuard("R-FE-WB", "webModeAvailable");
setGuard("R-FE-DT", "webModeAvailable");
setGuard("R-DT-STORE", "docRequested");
setGuard("R-DT-PLUGIN", "pluginRequested");

if (!rules.rules.some((r) => r.id === "R-FE2-WB")) {
  rules.rules.splice(rules.rules.findIndex((r) => r.id === "R-FE-WB") + 1, 0, {
    id: "R-FE2-WB", edgeId: "E-FE2-TO-WB", sourceNodeId: "N-FE-CLIENT2", targetNodeId: "N-WB-SERVER",
    when: "Running", then: "Running",
    guardAst: { type: "and", args: [guardFor("webModeAvailable")] },
    durationMs: 2,
    note: "第二客户端并发请求；服务端按事件队列串行处理",
    evidenceIds: ["EV-CODE-0025"]
  });
}

// ---------- (c) 每个 Case 补齐输入 ----------
const baseInputs = {
  tokenValid: true, pathWithinRoot: true, sizeOk: true, utf8Ok: true, fileExists: true,
  methodKnown: true, bufferExists: true, providerRegistered: false, powershellAvailable: true,
  rgAvailable: true, isGitRepo: true, webview2Available: true,
  webModeAvailable: true, docRequested: true, pluginRequested: false
};
const WEB = ["C-NORMAL-EDIT", "C-AUTH-REJECT", "C-PATH-ESCAPE", "C-SIZE-LIMIT", "C-ENCODING-REJECT", "C-FILE-NOT-FOUND", "C-CONCURRENT-TABS"];
const DESKTOP = ["C-TERMINAL-PS", "C-TERMINAL-MISSING", "C-SEARCH-RG", "C-SEARCH-FALLBACK", "C-SCM-COMMIT", "C-GIT-NOT-REPO", "C-PLUGIN-TIMEOUT", "C-E2E-CLICK-WEBVIEW2"];
for (const c of cases.cases) {
  c.inputs = Object.assign({}, baseInputs, c.inputs || {});
  if (WEB.includes(c.id)) { c.inputs.webModeAvailable = true; c.inputs.docRequested = true; }
  if (DESKTOP.includes(c.id)) { c.inputs.webModeAvailable = false; c.inputs.docRequested = false; }
}
const byId = (id) => cases.cases.find((c) => c.id === id);
byId("C-PLUGIN-TIMEOUT").inputs.pluginRequested = true;
byId("C-E2E-CLICK-WEBVIEW2").inputs.webview2Available = false;
byId("C-E2E-CLICK-WEBVIEW2").inputs.powershellAvailable = false;
byId("C-TERMINAL-MISSING").inputs.powershellAvailable = false;
byId("C-TERMINAL-MISSING").inputs.webview2Available = false;
byId("C-AUTH-REJECT").inputs.tokenValid = false;
byId("C-PATH-ESCAPE").inputs.pathWithinRoot = false;
byId("C-SIZE-LIMIT").inputs.sizeOk = false;
byId("C-ENCODING-REJECT").inputs.utf8Ok = false;
byId("C-FILE-NOT-FOUND").inputs.fileExists = false;
byId("C-RPC-UNKNOWN-METHOD").inputs.methodKnown = false;
byId("C-RPC-BUFFER-NOT-FOUND").inputs.bufferExists = false;
byId("C-AI-COMPLETE").inputs.providerRegistered = true;
byId("C-SEARCH-FALLBACK").inputs.rgAvailable = false;
byId("C-GIT-NOT-REPO").inputs.isGitRepo = false;

// ---------- (d) 新增 Concurrent / Combined 两个 Case ----------
if (!byId("C-CONCURRENT-TABS")) {
  cases.cases.push({
    id: "C-CONCURRENT-TABS", name: "两个客户端同时操作同一本地服务",
    origin: "DesignedSimulation",
    scenario: {
      steps: ["客户端 A 发起打开请求", "客户端 B 几乎同时发起请求", "单例 HTTP 服务按事件队列串行处理", "令牌校验对两个客户端分别成立"],
      documentation: "本地服务为单实例 + 每请求令牌校验（auth.rs）；两个客户端并存是模型推演，项目文档未覆盖该并发场景"
    },
    expected: ["两个客户端的请求都被串行处理而非互相覆盖", "会话状态不出现交叉污染"],
    entryNodeId: "N-FE-EDITORHTML", entryNodes: ["N-FE-EDITORHTML", "N-FE-CLIENT2"], entryState: "Running",
    inputs: Object.assign({}, baseInputs, { webModeAvailable: true, docRequested: true }),
    seed: 1,
    nodeIds: ["N-FE-EDITORHTML", "N-FE-CLIENT2", "N-WB-SERVER"],
    edgeIds: ["E-FE-TO-WB", "E-FE2-TO-WB"],
    evidenceIds: ["EV-CODE-0025", "EV-CODE-0026"],
    linkedTestIds: []
  });
}
if (!byId("C-COMBINED-BREACH")) {
  cases.cases.push({
    id: "C-COMBINED-BREACH", name: "越界 + 超限 + 非 UTF-8 + 文件不存在 四重失败叠加",
    origin: "DesignedSimulation",
    scenario: {
      steps: ["路径越界", "文件超过 4MB", "内容非 UTF-8", "目标文件不存在", "四道判定同时失败"],
      documentation: "E001/E002/E003/E004 四条错误码的叠加场景；文档未组合测试，本 Case 为模型推演"
    },
    expected: ["链路在最早一道守卫处停止", "没有任何一层被误放行"],
    entryNodeId: "N-FE-EDITORHTML", entryNodes: ["N-FE-EDITORHTML"], entryState: "Running",
    inputs: Object.assign({}, baseInputs, { pathWithinRoot: false, sizeOk: false, utf8Ok: false, fileExists: false }),
    seed: 1,
    nodeIds: ["N-FE-EDITORHTML", "N-WB-SERVER", "N-WB-PATHSAFETY", "N-KN-DOCSTORE"],
    edgeIds: ["E-FE-TO-WB", "E-WB-TO-SANDBOX", "E-WB-TO-STORE", "E-STORE-TO-DOCUMENT"],
    evidenceIds: ["EV-DOC-0008"],
    linkedTestIds: []
  });
}
// 零转移是预期语义（未知方法不应改动状态），标注原因
const zt = byId("C-RPC-UNKNOWN-METHOD");
zt.expected = ["不进入 Kernel", "内核状态不被改动（因此本 Case 没有任何状态转移 —— 这是预期语义，不是覆盖缺口）", "返回 -32601"];

wr("architecture.json", arch);
wr("causal-rules.json", rules);
wr("cases.json", cases);
console.log("patched: nodes=" + arch.nodes.length, "edges=" + arch.edges.length, "rules=" + rules.rules.length, "cases=" + cases.cases.length);
