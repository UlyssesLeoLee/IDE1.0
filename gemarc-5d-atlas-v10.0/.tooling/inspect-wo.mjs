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
  const wo = W.buildWorkOrder("FR-09");
  return { sourceEvidence: wo.sourceEvidence, targetModules: wo.targetModules, cost: wo.cost };
});
await browser.close();
console.log("FR-09 工单 sourceEvidence：");
out.sourceEvidence.forEach((s) => console.log("   " + s));
console.log("cost:", JSON.stringify(out.cost));
