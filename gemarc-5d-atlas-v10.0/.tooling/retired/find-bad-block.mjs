// 逐块删除定位：删掉哪个 RENDERERS 块后语法通过 ⇒ 问题块
import fs from "node:fs";
import vm from "node:vm";

const file = process.argv[2];
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

function compiles(arr) {
  try { new vm.Script(arr.join("\n")); return null; }
  catch (e) { return e.message; }
}

console.log("full file:", compiles(lines) || "OK");

// 找出 RENDERERS 块的行范围
const blocks = [];
lines.forEach((l, i) => {
  if (/^\s{2}RENDERERS\.\w+\s*=/.test(l)) {
    let e = i;
    while (e < lines.length && lines[e].trim() !== "};") e++;
    blocks.push({ name: (l.match(/RENDERERS\.(\w+)/) || [null, "?"])[1], start: i, end: e });
  }
});
console.log("blocks found:", blocks.map((b) => b.name).join(", "));

// 逐个删除
for (const b of blocks) {
  const copy = lines.slice(0, b.start).concat(lines.slice(b.end + 1));
  const err = compiles(copy);
  if (err === null) {
    console.log(">>> removing " + b.name + " (lines " + (b.start + 1) + "-" + (b.end + 1) + ") makes it COMPILE");
    console.log("    suspect block:");
    for (let k = b.start; k <= b.end; k++) {
      console.log("    " + (k + 1) + ": " + JSON.stringify(lines[k]));
    }
    break;
  }
}

// 另外：分别编译 preamble（含 IIFE 开头）+ epilogue
const firstBlock = blocks[0];
const preamble = lines.slice(0, firstBlock.start).concat(["})"]).join("\n");
console.log("preamble+'})' :", compiles(preamble.split("\n")) || "OK");

const epilogueStart = blocks[blocks.length - 1].end + 1;
const epilogue = lines.slice(epilogueStart);
console.log("epilogue as-is:", compiles(epilogue) || "OK");
console.log("--- epilogue content ---");
epilogue.forEach((l, i) => console.log((epilogueStart + i + 1) + ": " + JSON.stringify(l)));
