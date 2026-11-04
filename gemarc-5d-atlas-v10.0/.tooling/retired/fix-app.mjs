// 修复 app.js 的转义/语法错误（按函数定位，避免行号漂移），并给出定位信息
import fs from "node:fs";
import path from "node:path";

const file = path.resolve(process.argv[2]);
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

const AMP = "&" + "amp;";
const LT = "&" + "lt;";
const GT = "&" + "gt;";
const QUOT = "&" + "quot;";
const APOS = "&" + "#39;";

// 1) esc 函数体：从 "function esc(s) {" 到其后的 "return"/replace 链，重写为固定 5 行
let escStart = lines.findIndex((l) => /^ {2}function esc\(s\) \{/.test(l));
let escEnd = escStart;
while (escEnd < lines.length && !/^ {2}\}/.test(lines[escEnd])) escEnd++;

const newEsc = [
  "  function esc(s) {",
  '    return String(s == null ? "" : s)',
  '      .replace(/&/g, "' + AMP + '")',
  '      .replace(/</g, "' + LT + '").replace(/>/g, "' + GT + '")',
  '      .replace(/"/g, "' + QUOT + '").replace(/\'/g, "' + APOS + '");',
  "  }"
];

// 2) el 里那行带三引号的 replace
let elIdx = -1;
for (let i = 0; i < lines.length; i++) {
  if (/a \+= " " \+ k/.test(lines[i]) && /replace\(/.test(lines[i])) { elIdx = i; break; }
}
if (elIdx === -1) {
  console.log("el() replace line not found — nothing patched");
  process.exit(1);
}
const newElLine = '      a += " " + k + \'="\' + esc(String(attrs[k])) + \'"\';';

// 从后往前改，避免索引漂移
if (escEnd > elIdx) {
  lines.splice(escStart, escEnd - escStart + 1, ...newEsc);
  lines[elIdx] = newElLine;
} else {
  lines[elIdx] = newElLine;
  lines.splice(escStart, escEnd - escStart + 1, ...newEsc);
}

fs.writeFileSync(file, lines.join("\n"), "utf8");
console.log("patched esc() at line", escStart + 1, "and el() replace at line", elIdx + 1);
console.log("--- patched regions ---");
lines.slice(escStart, escStart + 7).forEach((l, i) => console.log((escStart + 1 + i) + ": " + JSON.stringify(l)));
const idx2 = lines.findIndex((l) => /a \+= " " \+ k/.test(l));
console.log((idx2 + 1) + ": " + JSON.stringify(lines[idx2]));
