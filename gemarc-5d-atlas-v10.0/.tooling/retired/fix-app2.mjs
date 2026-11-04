// 修 app.js 第 116 行（g-rev 徽章）的括号不配对，并给 build.mjs 加语法门禁
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const root = path.resolve(process.argv[2] || ".");

// ---------- 1) 修 app.js 那一行 ----------
const appFile = path.join(root, "src", "app.js");
const lines = fs.readFileSync(appFile, "utf8").split(/\r?\n/);
let fixed = 0;
for (let i = 0; i < lines.length; i++) {
  const l = lines[i];
  if (!/class: "g-rev"/.test(l)) continue;
  if (l.includes(".slice(0, 7) + ") && !l.includes("slice(0, 7)) + ")) {
    lines[i] = l.replace(".slice(0, 7) + ", ".slice(0, 7)) + ");
    fixed++;
    console.log("fixed g-rev line " + (i + 1));
  }
}
if (fixed) fs.writeFileSync(appFile, lines.join("\n"), "utf8");

// ---------- 2) 给 build.mjs 加语法门禁 ----------
const buildFile = path.join(root, ".tooling", "build.mjs");
let b = fs.readFileSync(buildFile, "utf8");
if (!b.includes("GEMARC_SYNTAX_GATE")) {
  const gate = `
// ---- 语法门禁：任何 src/*.js 有语法错误都不得产出 index.html ----
// GEMARC_SYNTAX_GATE
const vm = await import("node:vm");
const syntaxProblems = [];
for (const f of [...SRC_ORDER]) {
  const p = path.join(SRC, f);
  try {
    new vm.Script(fs.readFileSync(p, "utf8"), { filename: p });
  } catch (e) {
    syntaxProblems.push(f + " => " + e.message);
  }
}
if (syntaxProblems.length) {
  console.error("BUILD ABORTED — src/ 存在语法错误：");
  syntaxProblems.forEach((s) => console.error("  " + s));
  process.exit(1);
}

`;
  b = b.replace("// ---- 1) models", gate + "// ---- 1) models");
  fs.writeFileSync(buildFile, b, "utf8");
  console.log("added syntax gate to build.mjs");
}

console.log("done");
