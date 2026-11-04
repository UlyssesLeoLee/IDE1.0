import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const dPath = ROOT + "/models/diagram-definitions.json";
const aPath = ROOT + "/models/architecture.json";
const d = JSON.parse(fs.readFileSync(dPath, "utf8"));
const a = JSON.parse(fs.readFileSync(aPath, "utf8"));
const nodeIds = new Set(a.nodes.map((n) => n.id));
const edgeIds = new Set(a.edges.map((e) => e.id));

// 每张图 → 它真正描述的节点/边（供检查器按真实 ID 联动）
const MAP = {
  "DG-UML-COMPONENT-CRATES": {
    n: ["N-FE-EDITORHTML", "N-WB-SERVER", "N-DT-TAURI", "N-PT-RPC", "N-KN-DOCSTORE", "N-EC-BUFFER", "N-CLI", "N-KERNELCORE", "N-SHELL-TUI"],
    e: ["E-FE-TO-WB", "E-WB-TO-STORE", "E-DT-TO-STORE", "E-RPC-TO-KERNEL", "E-STORE-TO-DOCUMENT", "E-CLI-TO-ACI"]
  },
  "DG-UML-PACKAGE-EDITCORE": {
    n: ["N-EC-BUFFER", "N-EC-UNDO", "N-EC-HISTORY", "N-EC-TYPECONFIG", "N-EC-TYPES", "N-EC-GREP", "N-EC-GREPENGINE", "N-EC-SEARCHOPS", "N-EC-ENCODING", "N-EC-DOCIO", "N-EC-AUTOBACKUP", "N-EC-MIGEMO", "N-EC-NAV", "N-EC-PLUGIN", "N-EC-DAEMON", "N-EC-MACRO", "N-EC-EDITCMD"],
    e: ["E-BUFFER-TO-UNDO", "E-HISTORY-TO-UNDO", "E-TYPECONFIG-TO-TYPES", "E-GREP-TO-GREPENGINE", "E-SEARCHOPS-TO-BUFFER", "E-DOCIO-TO-ENCODING", "E-DOCIO-TO-FS", "E-DOCIO-TO-AUTOBACKUP", "E-EDITCMD-TO-UNDO", "E-EDITCMD-TO-MACRO", "E-NAV-TO-BUFFER", "E-MIGEMO-TO-SEARCHOPS", "E-PLUGIN-TO-ECPLUGIN"]
  },
  "DG-UML-CLASS-DOCUMENT": {
    n: ["N-KN-DOCUMENT", "N-KN-DOCSTORE", "N-EC-BUFFER"],
    e: ["E-STORE-TO-DOCUMENT", "E-DOCUMENT-TO-BUFFER"]
  },
  "DG-UML-CLASS-RPC": {
    n: ["N-PT-RPC", "N-PT-KERNEL"],
    e: ["E-RPC-TO-KERNEL", "E-RPC-ERR-BUFFER"]
  },
  "DG-UML-SEQUENCE-OPEN": {
    n: ["N-FE-EDITORHTML", "N-WB-SERVER", "N-WB-AUTH", "N-WB-PATHSAFETY", "N-KN-DOCSTORE", "N-KN-DOCUMENT"],
    e: ["E-FE-TO-WB", "E-WB-TO-AUTH", "E-AUTH-TO-WB", "E-WB-TO-SANDBOX", "E-SANDBOX-TO-WB", "E-WB-TO-STORE", "E-STORE-TO-DOCUMENT"]
  },
  "DG-UML-SEQUENCE-TERMINAL": {
    n: ["N-DT-TAURI", "N-DT-TERMINAL", "N-OS-PWSH", "N-WB-TERMINAL"],
    e: ["E-DT-TO-TERMINAL", "E-TERM-TO-PSH", "E-PSH-TO-TERM", "E-TERM-TO-DT", "E-WB-TO-TERMINAL", "E-TERM-TO-WB"]
  },
  "DG-UML-STATE-MODE": {
    n: ["N-PT-KERNEL", "N-SHELL-TUI"],
    e: ["E-RPC-TO-KERNEL"]
  },
  "DG-UML-STATE-DOCUMENT": {
    n: ["N-KN-DOCUMENT", "N-KN-DOCSTORE", "N-KN-SESSION"],
    e: ["E-STORE-TO-DOCUMENT", "E-SESSION-TO-DOCUMENT", "E-DOCUMENT-TO-BUFFER"]
  },
  "DG-UML-ACTIVITY-OPEN": {
    n: ["N-FE-EDITORHTML", "N-WB-SERVER", "N-WB-AUTH", "N-WB-PATHSAFETY", "N-KN-DOCSTORE", "N-KN-DOCUMENT"],
    e: ["E-FE-TO-WB", "E-WB-TO-AUTH", "E-WB-TO-SANDBOX", "E-WB-TO-STORE", "E-STORE-TO-DOCUMENT"]
  },
  "DG-UML-DEPLOYMENT-RUNTIME": {
    n: ["N-DT-TAURI", "N-WB-SERVER", "N-DT-TERMINAL", "N-OS-PWSH", "N-EXT-GIT", "N-EXT-RG", "N-OS-FS", "N-OS-WEBVIEW2", "N-FE-EDITORHTML"],
    e: ["E-DT-TO-TERMINAL", "E-TERM-TO-PSH", "E-SCM-TO-GIT", "E-SEARCH-TO-RG", "E-WB-TO-FS", "E-DT-TO-WEBVIEW2"]
  },
  "DG-UML-USECASE-SHELL": {
    n: ["N-FE-EDITORHTML", "N-DT-TAURI", "N-KN-DOCSTORE", "N-DT-SEARCH", "N-DT-SCM", "N-DT-TERMINAL", "N-DT-OUTLINE", "N-PT-AIPROVIDER"],
    e: ["E-FE-TO-WB", "E-FE-TO-DT", "E-WB-TO-SEARCH", "E-WB-TO-SCM", "E-WB-TO-OUTLINE"]
  },
  "DG-ER-SESSION": {
    n: ["N-KN-SESSION", "N-KN-DOCSTORE", "N-KN-DOCUMENT", "N-EC-UNDO", "N-EC-DOCIO", "N-EC-BUFFER"],
    e: ["E-SESSION-TO-DOCUMENT", "E-STORE-TO-DOCUMENT", "E-BUFFER-TO-UNDO", "E-UNDO-TO-DOCUMENT", "E-DOCIO-TO-FS"]
  },
  "DG-DFD-OPEN-FILE": {
    n: ["N-FE-EDITORHTML", "N-WB-SERVER", "N-KN-DOCSTORE", "N-OS-FS"],
    e: ["E-FE-TO-WB", "E-WB-TO-STORE", "E-STORE-TO-DOCUMENT", "E-DOCIO-TO-FS"]
  },
  "DG-BPMN-AUTH": {
    n: ["N-WB-SERVER", "N-WB-AUTH"],
    e: ["E-WB-TO-AUTH", "E-AUTH-TO-WB"]
  },
  "DG-SECURITY-BOUNDARY": {
    n: ["N-WB-AUTH", "N-WB-PATHSAFETY", "N-WB-SERVER", "N-OS-FS", "N-DT-PLUGIN"],
    e: ["E-WB-TO-AUTH", "E-WB-TO-SANDBOX", "E-WB-TO-STORE", "E-DT-TO-PLUGIN"]
  },
  "DG-NETWORK-LOCAL": {
    n: ["N-FE-EDITORHTML", "N-FE-CLIENT2", "N-WB-SERVER", "N-DT-TAURI", "N-OS-WEBVIEW2", "N-DT-TERMINAL"],
    e: ["E-FE-TO-WB", "E-FE2-TO-WB", "E-FE-TO-DT", "E-DT-TO-WEBVIEW2", "E-DT-TO-TERMINAL"]
  },
  "DG-DATAFLOW-EDIT": {
    n: ["N-EC-BUFFER", "N-EC-UNDO", "N-EC-HISTORY", "N-EC-DOCIO", "N-EC-AUTOBACKUP"],
    e: ["E-BUFFER-TO-UNDO", "E-HISTORY-TO-UNDO", "E-DOCIO-TO-AUTOBACKUP", "E-DOCIO-TO-FS"]
  }
};

const problems = [];
let patched = 0;
for (const dg of d.diagrams) {
  const m = MAP[dg.id];
  if (!m) { problems.push("no mapping for diagram " + dg.id); continue; }
  (m.n || []).forEach((id) => { if (!nodeIds.has(id)) problems.push(dg.id + " → 未知节点 " + id); });
  (m.e || []).forEach((id) => { if (!edgeIds.has(id)) problems.push(dg.id + " → 未知边 " + id); });
  dg.nodeIds = m.n || [];
  dg.edgeIds = m.e || [];
  patched++;
}
if (problems.length) {
  console.error("=== 図の関連 ID 検証に失敗 ===");
  problems.forEach((p) => console.error("  ✗ " + p));
  process.exit(1);
}
fs.writeFileSync(dPath, JSON.stringify(d, null, 2), "utf8");
console.log("diagrams patched:", patched, "/", d.diagrams.length);
