// 逐个顶层块编译定位：用 node 权威 parser
// 切分规则：本文件每个顶层块（函数定义 / RENDERERS 赋值）都以独立一行 "};" 结束
import fs from "node:fs";
import vm from "node:vm";

const file = process.argv[2];
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
const total = lines.length;

const isTopClose = (l) => l.trim() === "};";

// 找出顶层块边界：形如 "  RENDERERS.X = function (...) {" 的行
const starts = [];
lines.forEach((l, i) => {
  if (/^\s{2}(RENDERERS\.\w+|function \w+|var \w+)/.test(l) && l.includes("{")) starts.push(i);
});

console.log("candidate top-level starts:", starts.length);

// 逐个 RENDERERS 块单独编译
const rendererStarts = starts.filter((i) => /RENDERERS\./.test(lines[i]));
rendererStarts.forEach((start) => {
  let end = start;
  while (end < total && !isTopClose(lines[end])) end++;
  const block = lines.slice(start, end + 1).join("\n");
  const name = (lines[start].match(/RENDERERS\.(\w+)/) || [null, "?"])[1];
  try {
    new vm.Script("(function(){\n" + block + "\n})");
    console.log("OK   " + name + "  (lines " + (start + 1) + "-" + (end + 1) + ")");
  } catch (e) {
    console.log("FAIL " + name + "  (lines " + (start + 1) + "-" + (end + 1) + ")  => " + e.message);
    const m = /:(\d+)\n/.exec(e.stack || "");
    if (m) {
      const bad = start + Number(m[1]);
      console.log("     offending line " + (bad + 1) + ": " + JSON.stringify(lines[bad]));
      for (let k = Math.max(0, bad - 2); k < Math.min(total, bad + 2); k++) {
        console.log("     " + (k + 1) + ": " + JSON.stringify(lines[k]));
      }
    }
  }
});

// 前导块（esc/el/text/...）单独编译
const firstRenderer = rendererStarts[0];
const preamble = lines.slice(0, firstRenderer).join("\n");
try {
  new vm.Script("(function(){\n" + preamble + "\n})");
  console.log("OK   preamble (lines 1-" + firstRenderer + ")");
} catch (e) {
  console.log("FAIL preamble => " + e.message);
  const m = /:(\d+)\n/.exec(e.stack || "");
  if (m) {
    const bad = Number(m[1]) - 3;
    console.log("     offending line " + (bad + 1) + ": " + JSON.stringify(lines[bad]));
  }
}
