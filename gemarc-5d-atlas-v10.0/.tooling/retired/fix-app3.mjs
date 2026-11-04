// 根治：抽出 revShort() helper，消除 app.js 里深层嵌套三元导致的括号不配对
import fs from "node:fs";
import path from "node:path";

const file = path.resolve(process.argv[2]);
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

const helper = [
  ``,
  `  /** 事实源 revision 短码（models/evidence.json） */`,
  `  function revShort() {`,
  `    var revs = (M.evidence && M.evidence.sourceRevisions) || [];`,
  `    var sha = revs.length ? (revs[0].sha || '') : '';`,
  `    return sha.slice(0, 7);`,
  `  }`
];

const anchorIdx = lines.findIndex((l) => /^ {2}var \$\$ = function/.test(l));
if (anchorIdx === -1) {
  console.log("anchor not found");
  process.exit(1);
}
lines.splice(anchorIdx + 1, 0, ...helper);

let replaced = 0;
let unhandled = 0;
for (let i = 0; i < lines.length; i++) {
  if (!/sourceRevisions\[0\]/.test(lines[i])) continue;
  if (/class: "g-rev"/.test(lines[i])) {
    lines[i] = `          el("div", { class: "g-rev" }, ["GitGit @ " + revShort() + " · dev"])`;
    replaced++;
  } else if (/class: "g-rev-badge"/.test(lines[i])) {
    lines[i] = `        el("span", { class: "g-rev-badge", id: "revBadge" }, ["rev " + revShort()])`;
    replaced++;
  } else {
    console.log(`UNHANDLED at line ${i + 1}: ${lines[i]}`);
    unhandled++;
  }
}

fs.writeFileSync(file, lines.join("\n"), "utf8");
console.log(`rewrote ${replaced} revision reference(s); unhandled=${unhandled}; helper at line ${anchorIdx + 2}`);
