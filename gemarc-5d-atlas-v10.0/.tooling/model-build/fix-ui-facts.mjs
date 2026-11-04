import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const rd = (f) => JSON.parse(fs.readFileSync(ROOT + "/models/" + f, "utf8"));
const wr = (f, o) => fs.writeFileSync(ROOT + "/models/" + f, JSON.stringify(o, null, 2), "utf8");

// ---- 真实事实（取自 crates/ide-shell-web/src/editor.html）----
const ACT = ["资源管理器", "搜索", "源代码管理", "大纲", "扩展"];      // editor.html:1445-1449 data-tip
const EDITOR_LINES = 9518;                                          // 实测行数
const SECTIONS = [
  "Activity Bar (5 项: 资源管理器/搜索/源代码管理/大纲/扩展)",
  "Side Bar (Explorer/搜索/SCM/Outline/Extensions)",
  "Editor (Tabs + Breadcrumb + Gutter + Content)",
  "Bottom Panel (Terminal/Output/Problems)",
  "Status Bar (Mode/Pos/File/Tip/Lang)"
];

// 1) architecture.json：容器名 + editor.html 行数
const arch = rd("architecture.json");
for (const c of arch.containers) {
  if (c.name === "Activity Bar (4 panels)") c.name = "Activity Bar (5 项)";
  if (c.semantic && c.semantic.indexOf("Activity Bar") === 0) {
    c.semantic = "Activity Bar 5 项：" + ACT.join(" / ");
  }
}
const fe = arch.nodes.find((n) => n.id === "N-FE-EDITORHTML");
if (fe) {
  fe.properties = fe.properties || {};
  fe.properties.lines = EDITOR_LINES;
  fe.properties.activityBar = ACT.slice();
  fe.properties.note = "真实结构：Activity Bar 5 项 + Side Bar 5 面板 + Breadcrumb + Bottom Panel 单层（ADR-005）；minimap 在真实实现中为 false（FR-13 仍是将来扩展）";
  fe.evidenceIds = Array.from(new Set((fe.evidenceIds || []).concat(["EV-CODE-0052", "EV-CODE-0054"])));
}
wr("architecture.json", arch);

// 2) spatial.json：房间标签
const sp = rd("spatial.json");
for (const [rid, r] of Object.entries(sp.rooms)) {
  if (r.label === "Activity Bar (4 panels)") r.label = "Activity Bar (5 项)";
}
wr("spatial.json", sp);

// 3) prototype.json：SC-EDITOR 的区域描述
const pr = rd("prototype.json");
const grp = pr.sourceScreens.ideShellWeb;
const scr = grp.screens.find((s) => s.id === "SC-EDITOR");
if (scr) {
  scr.sections = SECTIONS.slice();
  scr.knownIssue = null;
  scr.evidenceIds = Array.from(new Set((scr.evidenceIds || []).concat(["EV-CODE-0054"])));
}
grp.screens = grp.screens.filter((s) => s.id !== "SC-EDITOR" || scr);
pr.realScreenStructure = {
  source: "crates/ide-shell-web/src/editor.html",
  activityBar: ACT,
  activityBarEvidence: "crates/ide-shell-web/src/editor.html:1445-1449",
  sideBarPanes: ["explorer", "search", "scm", "outline", "extensions"],
  editorArea: ["tabs", "breadcrumb", "gutter", "content"],
  bottomPanel: ["Terminal", "Output", "Problems"],
  statusBar: ["Mode", "Pos", "File", "Tip", "Lang"],
  minimap: false,
  note: "以上为代码实测结构；FR-13 迷你地图在真实实现中仍为 false（未来扩展）。"
};
wr("prototype.json", pr);

// 4) evidence.json：补两条真实证据
const ev = rd("evidence.json");
if (!ev.evidence.some((e) => e.id === "EV-CODE-0054")) {
  ev.evidence.push({
    id: "EV-CODE-0054",
    kind: "SourceCode",
    status: "Implemented",
    sourceRevisionId: "REV-IDE1",
    summary: "editor.html 的 Activity Bar 实际为 5 个按钮（data-pane=explorer/search/scm/outline/extensions，data-tip=资源管理器/搜索/源代码管理/大纲/扩展），Side Bar 面板随之 5 个；编辑器区含 tabs/breadcrumb/gutter/content。文档里「Activity Bar (4 panels)」的描述已过时。",
    path: "crates/ide-shell-web/src/editor.html",
    startLine: 1445,
    endLine: 1449,
    revision: "76c4e8f"
  });
}
if (!ev.evidence.some((e) => e.id === "EV-CODE-0055")) {
  ev.evidence.push({
    id: "EV-CODE-0055",
    kind: "SourceCode",
    status: "Implemented",
    sourceRevisionId: "REV-IDE1",
    summary: "editor.html 实际 9518 行（ADR-010 记录的「1348 行」是当时重写后的快照，已随后续功能增长而过时）。",
    path: "crates/ide-shell-web/src/editor.html",
    startLine: 1,
    endLine: 9518,
    revision: "76c4e8f"
  });
}
wr("evidence.json", ev);

console.log("模型内容已按真实代码修正：");
console.log("  Activity Bar: 4 → 5 项（" + ACT.join("/") + "）");
console.log("  Side Bar: 补入「搜索」；Editor 区: 补入 Breadcrumb");
console.log("  editor.html 行数: 1348 → " + EDITOR_LINES);
console.log("  新增证据: EV-CODE-0054（Activity Bar 实测）、EV-CODE-0055（行数）");
