import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "requirement-workbench.html").replace(/\\/g, "/");
const browser = await chromium.launch({ channel: "chrome" });
const page = await (await browser.newContext()).newPage();
await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#reqList .g-req");
const out = await page.evaluate(() => {
  const W = window.Ide1RequirementWorkbench;
  const ids = ["FR-01", "FR-03", "FR-05", "FR-06", "FR-07", "FR-08", "FR-09", "FR-10", "FR-15", "NFR-05", "NFR-06", "IR-01", "IR-02", "IR-03", "CR-03"];
  return ids.map((id) => {
    const i = W.impactOf(id);
    return {
      id, intent: (i.req.intent || "").slice(0, 16),
      direct: i.cost.directFiles, down: i.cost.downstreamFiles,
      nodes: i.cost.nodes, cases: i.cost.cases, tests: i.cost.tests,
      graphs: i.cost.diagrams, level: i.cost.level,
      files: i.directFiles.map((f) => f.path.replace("crates/", ""))
    };
  });
});
await browser.close();
for (const r of out) {
  console.log(`${r.id.padEnd(7)} ${r.level.padEnd(5)} 直接${String(r.direct).padStart(2)} 连带${String(r.down).padStart(2)} 节点${String(r.nodes).padStart(2)} Case${String(r.cases).padStart(2)} 测试${String(r.tests).padStart(2)} 图${r.graphs}  ${r.intent}`);
  console.log("        " + r.files.join(", "));
}
