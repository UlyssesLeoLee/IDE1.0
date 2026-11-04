import fs from "node:fs";
const p = "E:/IDE1.0/gemarc-5d-atlas-v10.0/src/app.js";
let t = fs.readFileSync(p, "utf8");

// 把 openDiagramWindow 从 render2D 的嵌套作用域提到模块作用域：
// 检查器（renderRight）在任何视图下都要能打开图窗，不能依赖 render2D 已经跑过。
const start = t.indexOf("  function openDiagramWindow(diagId) {");
if (start === -1) throw new Error("openDiagramWindow not found");
const endMarker = "  // 检查器（别的作用域）需要打开图窗 —— 暴露到 state\n  S.openDiagramWindow = openDiagramWindow;\n";
const endIdx = t.indexOf(endMarker);
if (endIdx === -1) throw new Error("end marker not found");
const block = t.slice(start, endIdx);
// 去缩进 2 空格 → 模块作用域
const unindented = block.split("\n").map((l) => (l.startsWith("  ") ? l.slice(2) : l)).join("\n");
t = t.slice(0, start) + t.slice(endIdx + endMarker.length);

// 插入到模块作用域：renderRight 之前
const anchor = "  /** 选中对象 → 关联工程图（按真实 nodeIds/edgeIds 关联，不是按名字猜） */";
if (!t.includes(anchor)) throw new Error("anchor not found");
t = t.replace(anchor, unindented.replace(/\n+$/, "") + "\n\n" + anchor);

// 暴露给 state（模块级函数，任何视图都可用）
t = t.replace(
  "  function renderRight() {",
  "  S.openDiagramWindow = openDiagramWindow;\n\n  function renderRight() {"
);

fs.writeFileSync(p, t, "utf8");
console.log("openDiagramWindow moved to module scope");
