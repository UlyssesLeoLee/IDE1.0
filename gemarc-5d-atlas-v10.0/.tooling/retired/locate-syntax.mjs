// 精确定位"多余的右括号"：带匹配栈的 scanner，正确处理正则字面量与字符串
import fs from "node:fs";

const file = process.argv[2];
const src = fs.readFileSync(file, "utf8");
const lines = src.split(/\r?\n/);

const OPEN = { "(": ")", "[": "]", "{": "}" };
const CLOSE = { ")": "(", "]": "[", "}": "{" };

// 上一个 significant token 的字符（用于判定 `/` 是否为正则）
let prevSig = "";
const stack = [];
let found = null;

function isRegexStart(p) {
  return p === "" || "(,=:[!&|?{};+-*%<>~^".includes(p) ||
    /return|typeof|case|in|of|do|else|yield|await|void|new/.test(p);
}

outer:
for (let li = 0; li < lines.length; li++) {
  const line = lines[li];
  if (inStr || inBlockComment || inTemplate) {
    console.log("!! line " + (li + 1) + " entered with inStr=" + inStr +
      " inBlockComment=" + inBlockComment + " inTemplate=" + inTemplate +
      "  |  " + JSON.stringify(line.trim().slice(0, 80)));
  }
  let i = 0;
  const depthBefore = stack.length;
  while (i < line.length) {
    const ch = line[i];
    const nx = line[i + 1];

    // 注释
    if (ch === "/" && nx === "/") break;
    if (ch === "/" && nx === "*") {
      // 块注释跨行
      let end = line.indexOf("*/", i + 2);
      if (end === -1) {
        // 跨到下一行
        let k = li + 1;
        while (k < lines.length) {
          const p = lines[k].indexOf("*/");
          if (p !== -1) { li = k; i = p + 2; found = found || { type: "block-comment", line: li + 1 }; continue outer; }
          k++;
        }
        break outer;
      }
      i = end + 2;
      continue;
    }
    // 字符串
    if (ch === '"' || ch === "'" || ch === "`") {
      const quote = ch;
      let j = i + 1;
      while (j < line.length) {
        if (line[j] === "\\") { j += 2; continue; }
        if (line[j] === quote) break;
        j++;
      }
      if (j >= line.length) { /* 跨行字符串 */ }
      prevSig = "str";
      i = j + 1;
      continue;
    }
    // 正则字面量
    if (ch === "/" && isRegexStart(prevSig)) {
      let j = i + 1;
      let inClass = false;
      while (j < line.length) {
        if (line[j] === "\\") { j += 2; continue; }
        if (line[j] === "[") inClass = true;
        else if (line[j] === "]") inClass = false;
        else if (line[j] === "/" && !inClass) break;
        j++;
      }
      prevSig = "regex";
      i = j + 1;
      continue;
    }
    // 括号
    if (OPEN[ch]) { stack.push({ ch, line: li + 1 }); prevSig = ch; i++; continue; }
    if (CLOSE[ch]) {
      const top = stack[stack.length - 1];
      if (!top) {
        found = { type: "extra-close", char: ch, line: li + 1, text: line.trim().slice(0, 100) };
        break outer;
      }
      if (OPEN[top.ch] !== ch) {
        found = { type: "mismatch", char: ch, expected: OPEN[top.ch], openedLine: top.line, line: li + 1, text: line.trim().slice(0, 100) };
        break outer;
      }
      stack.pop();
      prevSig = ch;
      i++;
      continue;
    }
    if (!/\s/.test(ch)) prevSig = ch;
    i++;
  }
  if (stack.length !== depthBefore) {
    console.log("depth " + depthBefore + " -> " + stack.length + "  at line " + (li + 1) + "  |  " + JSON.stringify(line.trim().slice(0, 95)));
  }
}

if (found) {
  console.log("PROBLEM:", JSON.stringify(found, null, 2));
  console.log("--- open stack at failure (inner -> outer) ---");
  stack.slice().reverse().forEach((s, k) => console.log("  #" + k + "  " + s.ch + "  opened at line " + s.line));
  const L = found.line;
  for (let k = Math.max(0, L - 4); k < Math.min(lines.length, L + 2); k++) {
    console.log((k + 1) + ": " + JSON.stringify(lines[k]));
  }
} else {
  console.log("no bracket problem found");
  console.log("unclosed stack:", JSON.stringify(stack));
}
