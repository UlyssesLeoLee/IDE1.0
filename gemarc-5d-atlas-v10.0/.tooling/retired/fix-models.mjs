// 修正 diagram-definitions.json 的键名 typo（"label:" → "label"），并校验全部 models JSON
import fs from "node:fs";
import path from "node:path";

const modelsDir = process.argv[2];

// 1) 修正 typo
const dg = path.join(modelsDir, "diagram-definitions.json");
let text = fs.readFileSync(dg, "utf8");
const bad = 'label:';
const count = text.split(bad).length - 1;
if (count > 0) {
  text = text.split(bad).join("label");
  fs.writeFileSync(dg, text, "utf8");
}
console.log("fixed 'label:' occurrences =", count);

// 2) 校验全部 models/*.json 可解析
let bad2 = 0;
for (const f of fs.readdirSync(modelsDir).filter((x) => x.endsWith(".json")).sort()) {
  const p = path.join(modelsDir, f);
  try {
    const j = JSON.parse(fs.readFileSync(p, "utf8"));
    const keys = Object.keys(j).join(",");
    console.log("OK    " + f.padEnd(28) + " top-level: " + keys);
  } catch (e) {
    bad2++;
    console.log("FAIL  " + f + " => " + e.message);
  }
}
console.log(bad2 === 0 ? "ALL MODELS JSON OK" : bad2 + " FAILED");
process.exit(bad2 === 0 ? 0 : 1);
