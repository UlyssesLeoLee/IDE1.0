import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const PROJ = "E:/IDE1.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const wr = (f, o) => fs.writeFileSync(ROOT + "/models/" + f, JSON.stringify(o, null, 2), "utf8");

// 真实行数 = 按 \n 切分的行数（含空行）。
// 注意：PowerShell 的 `Get-Content | Measure-Object -Line` 不计空行，会少算 —— 之前据此写进模型的 9518 是错的。
const real = fs.readFileSync(PROJ + "/crates/ide-shell-web/src/editor.html", "utf8").split("\n").length;
console.log("editor.html 真实行数（含空行）:", real);

const arch = rd("architecture.json");
const fe = arch.nodes.find((n) => n.id === "N-FE-EDITORHTML");
if (fe) { fe.properties = fe.properties || {}; fe.properties.lines = real; }
wr("architecture.json", arch);

const ev = rd("evidence.json");
const e55 = ev.evidence.find((e) => e.id === "EV-CODE-0055");
if (e55) {
  e55.endLine = real;
  e55.summary = "editor.html 实际 " + real + " 行（ADR-010 记录的「1348 行」是当时重写后的快照，已随后续功能增长而过时；注意按空行计的真实行数为 " + real + "）。";
}
wr("evidence.json", ev);
console.log("模型与证据已更新为真实行数:", real);
