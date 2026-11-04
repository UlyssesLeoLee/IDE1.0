import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const arch = rd("architecture.json");
const ev = rd("evidence.json");

// 新增证据：前端 JS 编辑引擎只有 Cargo.toml 注释提及，按 implementation_gate 不能算 Implemented
if (!ev.evidence.some((e) => e.id === "EV-CODE-0053")) {
  ev.evidence.push({
    id: "EV-CODE-0053",
    kind: "SourceCode",
    status: "Designed",
    sourceRevisionId: "REV-IDE1",
    summary: "ide-shell-web/Cargo.toml 的依赖注释写明「/api/doc/* 的会话状态与编辑语义（与 web 前端 JS 引擎并行迁移中）」。代码树中没有独立的 JS 编辑引擎模块，因此该能力按 Designed 记录，不按 Implemented。",
    path: "crates/ide-shell-web/Cargo.toml",
    startLine: 25,
    endLine: 30,
    revision: "76c4e8f"
  });
}
// N-FE-JSENGINE 降级为 Designed
const n = arch.nodes.find((x) => x.id === "N-FE-JSENGINE");
if (n) {
  n.verification = "Designed";
  n.evidenceIds = ["EV-CODE-0053"];
  n.properties = Object.assign({}, n.properties, {
    note: "仅在 ide-shell-web/Cargo.toml 的注释中被提及为「并行迁移中」；无独立模块与实现代码，故按 Designed 记录",
    implementationState: "not-evidenced-in-code"
  });
}
// 全量复查：还有没有 Implemented 但证据只指向 Cargo.toml / 无证据的节点与边
const evById = new Map(ev.evidence.map((e) => [e.id, e]));
const suspects = [];
const check = (obj, kindLabel) => {
  if (obj.verification !== "Implemented") return;
  const paths = (obj.evidenceIds || []).map((id) => {
    const e = evById.get(id);
    return e ? (e.path || "") : "MISSING:" + id;
  });
  if (!paths.length) { suspects.push(kindLabel + " " + obj.id + "（无证据）"); return; }
  if (paths.every((p) => /(^|[\\/])Cargo\.toml$/.test(p) || p.indexOf("MISSING:") === 0)) {
    suspects.push(kindLabel + " " + obj.id + "（证据仅 " + paths.join(",") + "）");
  }
};
arch.nodes.forEach((n2) => check(n2, "节点"));
arch.edges.forEach((e) => check(e, "边"));

fs.writeFileSync(ROOT + "/models/architecture.json", JSON.stringify(arch, null, 2), "utf8");
fs.writeFileSync(ROOT + "/models/evidence.json", JSON.stringify(ev, null, 2), "utf8");
console.log("N-FE-JSENGINE → Designed；证据 EV-CODE-0053 新增");
console.log(suspects.length ? "仍有可疑 Implemented: " + suspects.join(" | ") : "复核：已无「仅凭 Cargo.toml」的 Implemented 节点/边");
