// 一次性修复脚本：diagrams.js 的转义与语法错误（按行号精确重写；幂等）
import fs from "node:fs";
import path from "node:path";

const file = path.resolve(process.argv[2]);
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

const AMP = "&" + "amp;";
const LT = "&" + "lt;";
const GT = "&" + "gt;";
const QUOT = "&" + "quot;";
const APOS = "&" + "#39;";

const before = lines.slice(25, 38);

// 26-31: esc()
lines[25] = "  function esc(s) {";
lines[26] = '    return String(s == null ? "" : s)';
lines[27] = '      .replace(/&/g, "' + AMP + '")';
lines[28] = '      .replace(/</g, "' + LT + '").replace(/>/g, "' + GT + '")';
lines[29] = '      .replace(/"/g, "' + QUOT + '").replace(/\'/g, "' + APOS + '");';

// 32-38: el()
lines[31] = "  function el(tag, attrs, children) {";
lines[32] = '    var a = "";';
lines[33] = "    Object.keys(attrs || {}).forEach(function (k) {";
lines[34] = "      if (attrs[k] === undefined || attrs[k] === null) return;";
lines[35] = '      a += " " + k + \'="\' + esc(String(attrs[k])) + \'"\';';
lines[36] = "    });";

fs.writeFileSync(file, lines.join("\n"), "utf8");

console.log("=== BEFORE ===");
before.forEach((l, i) => console.log((i + 26) + ": " + JSON.stringify(l)));
console.log("=== AFTER ===");
lines.slice(25, 38).forEach((l, i) => console.log((i + 26) + ": " + JSON.stringify(l)));
console.log("written:", file);
