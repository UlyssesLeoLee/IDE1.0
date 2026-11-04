// 修复 155 行重复的 " */ */"，并对 src/ 下所有 .js 做语法校验（用 node 自身 parser）
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const root = process.argv[2] || "src";

// --- 1) 修复重复注释闭合符 ---
const target = path.join(root, "diagrams.js");
if (fs.existsSync(target)) {
  const lines = fs.readFileSync(target, "utf8").split(/\r?\n/);
  let fixed = 0;
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (l.trimEnd().endsWith("*/ */")) {
      lines[i] = l.trimEnd().slice(0, -"*/ */".length).trimEnd() + " */";
      fixed++;
      console.log("fixed duplicate close at line " + (i + 1) + ": " + JSON.stringify(lines[i]));
    }
  }
  if (fixed) fs.writeFileSync(target, lines.join("\n"), "utf8");
}

// --- 2) 全量语法校验 ---
let bad = 0;
for (const f of fs.readdirSync(root).filter((x) => x.endsWith(".js")).sort()) {
  const p = path.join(root, f);
  const src = fs.readFileSync(p, "utf8");
  try {
    new vm.Script(src, { filename: p });
    console.log("OK    " + f + "  (" + src.length + " bytes)");
  } catch (e) {
    bad++;
    console.log("FAIL  " + f + "  => " + e.message);
    const m = /:(\d+)\n/.exec(e.stack || "");
    if (m) {
      const L = src.split(/\r?\n/);
      const badLine = Number(m[1]);
      console.log("      line " + badLine + ": " + JSON.stringify(L[badLine - 1]));
    }
  }
}
console.log(bad === 0 ? "ALL SYNTAX OK" : bad + " FILE(S) FAILED");
process.exit(bad === 0 ? 0 : 1);
