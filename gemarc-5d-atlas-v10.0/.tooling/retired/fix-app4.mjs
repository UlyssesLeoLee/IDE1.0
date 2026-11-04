// 修复 app.js：el() 返回 HTML 字符串，却被当作 DOM 节点 appendChild
import fs from "node:fs";
import path from "node:path";

const file = path.resolve(process.argv[2]);
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

// 这些参数是 el() 产出的字符串，必须走 insertAdjacentHTML
const STRING_ARGS = ["el(", "wrap", "win", "diagBar", "svg"];
let changed = 0;
let kept = 0;

for (let i = 0; i < lines.length; i++) {
  const l = lines[i];
  const m = /^(\s*)([A-Za-z_$][\w$]*(?:\([^)]*\))?)\.appendChild\((.*)\);\s*$/.exec(l);
  if (!m) continue;
  const arg = m[3];
  const isString = STRING_ARGS.some((p) => arg.startsWith(p));
  if (!isString) { kept++; continue; }
  lines[i] = `${m[1]}${m[2]}.insertAdjacentHTML("beforeend", ${arg});`;
  changed++;
}

// buildShell：五块一次性写入
for (let i = 0; i < lines.length - 5; i++) {
  if (lines[i].trim() === "app.appendChild(head);") {
    lines[i] = '    app.innerHTML = head + left + center + right + timeline;';
    lines.splice(i + 1, 4);
    changed++;
    break;
  }
}

fs.writeFileSync(file, lines.join("\n"), "utf8");
console.log(`converted ${changed} string-appendChild call(s) to insertAdjacentHTML; kept ${kept} real DOM appendChild(s)`);
