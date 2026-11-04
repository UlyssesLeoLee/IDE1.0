import fs from "node:fs";
const p = "E:/IDE1.0/gemarc-5d-atlas-v10.0/models/architecture.json";
const a = JSON.parse(fs.readFileSync(p, "utf8"));
const mk = (id, sourceId, targetId, type, note, evidenceIds, extra) => ({
  id, sourceId, targetId, sourcePortId: null, targetPortId: null,
  type, directed: true, guardId: null,
  properties: Object.assign({ note }, extra || {}),
  verification: "Implemented", evidenceIds
});
const adds = [
  mk("E-TERM-TO-DT", "N-DT-TERMINAL", "N-DT-TAURI", "returns", "桌面端终端会话结果回传 invoke 调用方", ["EV-CODE-0029"]),
  mk("E-PLUGIN-RETRY", "N-DT-PLUGIN", "N-DT-PLUGIN", "self_loop", "插件执行失败后的恢复自环（retry → Recovered）", ["EV-CODE-0032"], { selfLoop: true })
];
const have = new Set(a.edges.map((e) => e.id));
let n = 0;
for (const e of adds) if (!have.has(e.id)) { a.edges.push(e); n++; }
fs.writeFileSync(p, JSON.stringify(a, null, 2), "utf8");
console.log("edges added:", n, "total edges:", a.edges.length);
