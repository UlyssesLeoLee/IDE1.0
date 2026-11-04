// 在 preamble 内部逐个顶层函数编译，定位自身不平衡的函数
import fs from "node:fs";
import vm from "node:vm";

const file = process.argv[2];
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

// 顶层函数：行以 "  function name(" 开头；结束于 trim() === "}"
const funcs = [];
for (let i = 0; i < lines.length; i++) {
  const m = /^ {2}function (\w+)\s*\(/.exec(lines[i]);
  if (!m) continue;
  let e = i;
  while (e < lines.length && lines[e].trim() !== "}") e++;
  funcs.push({ name: m[1], start: i, end: e });
}

console.log("top-level functions in preamble:", funcs.map((f) => f.name).join(", "));

for (const f of funcs) {
  const body = lines.slice(f.start, f.end + 1).join("\n");
  try {
    new vm.Script("(function(){\n" + body + "\n})");
    console.log("OK   " + f.name + "  (lines " + (f.start + 1) + "-" + (f.end + 1) + ")");
  } catch (e) {
    console.log("FAIL " + f.name + "  (lines " + (f.start + 1) + "-" + (f.end + 1) + ")  => " + e.message);
    for (let k = f.start; k <= f.end; k++) {
      console.log("    " + (k + 1) + ": " + JSON.stringify(lines[k]));
    }
  }
}
