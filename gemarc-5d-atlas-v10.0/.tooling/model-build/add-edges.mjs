import fs from "node:fs";
const p = "E:/IDE1.0/gemarc-5d-atlas-v10.0/models/architecture.json";
const a = JSON.parse(fs.readFileSync(p, "utf8"));
const mk = (id, sourceId, targetId, type, sourcePortId, targetPortId, note, evidenceIds, extra) => ({
  id, sourceId, targetId,
  sourcePortId: sourcePortId || null,
  targetPortId: targetPortId || null,
  type, directed: true, guardId: null,
  properties: Object.assign({ note }, extra || {}),
  verification: "Implemented", evidenceIds
});
const adds = [
  mk("E-AUTH-TO-WB", "N-WB-AUTH", "N-WB-SERVER", "returns", "P-AUTH-CHECK", null, "校验通过后把控制权交还分发器（通过即继续，越权即阻断）", ["EV-CODE-0025", "EV-CODE-0026"]),
  mk("E-SANDBOX-TO-WB", "N-WB-PATHSAFETY", "N-WB-SERVER", "returns", "P-SANDBOX-GUARD", null, "沙箱判定结果回传分发器", ["EV-DOC-0008", "EV-DOC-0014"]),
  mk("E-BUFFER-TO-UNDO", "N-EC-BUFFER", "N-EC-UNDO", "snapshot", null, "P-EC-UNDO", "编辑动作写入 Ope 缓冲以支持撤销", ["EV-CODE-0039", "EV-DOC-0014"]),
  mk("E-UNDO-TO-DOCUMENT", "N-EC-UNDO", "N-KN-DOCUMENT", "returns", "P-EC-UNDO", "P-DOC-UNDO", "undo/redo 结果回到文档模型", ["EV-CODE-0012", "EV-CODE-0039"]),
  mk("E-TERM-TO-WB", "N-WB-TERMINAL", "N-WB-SERVER", "returns", null, null, "终端输出经轮询接口回到前端", ["EV-CODE-0021"]),
  mk("E-PSH-TO-TERM", "N-OS-PWSH", "N-DT-TERMINAL", "returns", null, null, "stdout/stderr 回流到终端会话", ["EV-CODE-0029", "EV-DOC-0013"]),
  mk("E-RG-TO-SEARCH", "N-EXT-RG", "N-DT-SEARCH", "returns", null, null, "rg 命中结果返回搜索层；rg 缺失则走回落边", ["EV-CODE-0031"]),
  mk("E-GIT-TO-SCM", "N-EXT-GIT", "N-DT-SCM", "returns", null, null, "git porcelain 输出回流给 SCM 层", ["EV-CODE-0030"]),
  mk("E-PLUGIN-TO-DT", "N-DT-PLUGIN", "N-DT-TAURI", "returns", "P-PLUGIN-LOAD", null, "插件执行结果回传 invoke 调用方", ["EV-CODE-0032"]),
  mk("E-DOCIO-TO-AUTOBACKUP", "N-EC-DOCIO", "N-EC-AUTOBACKUP", "snapshot", "P-EC-DOCIO", null, "周期备份触发点（5 分钟 / 10 世代）", ["EV-CODE-0045", "EV-DOC-0001"])
];
const have = new Set(a.edges.map((e) => e.id));
let n = 0;
for (const e of adds) if (!have.has(e.id)) { a.edges.push(e); n++; }
fs.writeFileSync(p, JSON.stringify(a, null, 2), "utf8");
console.log("edges added:", n, "total:", a.edges.length, "nodes:", a.nodes.length, "ports:", a.ports.length);
