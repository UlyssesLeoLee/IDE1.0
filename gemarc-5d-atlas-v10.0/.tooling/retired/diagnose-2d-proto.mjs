// 针对性诊断：2D 画布/图窗 + Prototype 的剩余失败
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");

const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const logs = [];
page.on("console", (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}\n${(e.stack || "").split("\n").slice(0, 4).join("\n")}`));

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 15000 });

// ---------- 2D ----------
await page.click('[data-view="2D"]');
await page.waitForTimeout(1200);
const d2 = await page.evaluate(() => {
  const host = document.querySelector("#diagHost");
  return {
    hostExists: !!host,
    hostChildren: host ? host.children.length : -1,
    hostHTML: host ? host.innerHTML.slice(0, 200) : null,
    diagBtns: document.querySelectorAll(".g-diagbar .g-diagbtn").length,
    nodeCount: document.querySelectorAll("#cSvg .g-node").length,
    edgeCount: document.querySelectorAll("#cSvg .g-edge").length,
    portCount: document.querySelectorAll("#cSvg .g-port").length,
    portsInModel: window.GemArcModels.architecture.nodes.filter((n) => (n.portIds || []).length).length,
    meta: document.querySelector("#cMeta") ? document.querySelector("#cMeta").textContent : null
  };
});
console.log("=== 2D ===");
console.log(JSON.stringify(d2, null, 2));

// 点一个 node 看检查器
await page.locator("#cSvg .g-node").first().click();
const insp = await page.textContent("#rightBody");
console.log("inspector after node click (first 120):", JSON.stringify(insp.slice(0, 120)));

await page.locator("#cSvg .g-edge-hit").first().click();
const insp2 = await page.textContent("#rightBody");
console.log("inspector after edge click (first 120):", JSON.stringify(insp2.slice(0, 120)));

// ---------- Prototype ----------
await page.click('[data-view="P"]');
await page.waitForTimeout(1200);
const dp = await page.evaluate(() => ({
  tabs: document.querySelectorAll(".g-proto-tab").length,
  konvaCanvas: document.querySelectorAll("#prStage canvas").length,
  widgets: window.GemArcApp.state.protoEditor ? window.GemArcApp.state.protoEditor.serialise().widgets.length : -1,
  previewInputs: document.querySelectorAll("#prPreview input[type=text], #prPreview textarea").length,
  previewAllInputs: document.querySelectorAll("#prPreview input").length,
  previewButtons: document.querySelectorAll("#prPreview button").length,
  previewHtmlHead: document.querySelector("#prPreview") ? document.querySelector("#prPreview").innerHTML.slice(0, 300) : null
}));
console.log("=== Prototype ===");
console.log(JSON.stringify(dp, null, 2));

// 试点击一个 preview button
if (dp.previewButtons > 0) {
  await page.locator("#prPreview button").first().click();
  await page.waitForTimeout(200);
  console.log("after preview button click, prJson len:", (await page.textContent("#prJson")).length);
}

// add → undo → redo
const before = await page.textContent("#prJson");
await page.click("#prAdd");
await page.waitForTimeout(250);
const added = await page.textContent("#prJson");
await page.click("#prUndo");
await page.waitForTimeout(250);
const undone = await page.textContent("#prJson");
await page.click("#prRedo");
await page.waitForTimeout(250);
const redone = await page.textContent("#prJson");
console.log("undo/redo:", JSON.stringify({
  changedOnAdd: before !== added,
  undoRestored: undone === before,
  redoReapplied: redone === added,
  lens: { before: before.length, added: added.length, undone: undone.length, redone: redone.length }
}));

console.log("=== LOGS (" + logs.length + ") ===");
logs.slice(0, 15).forEach((l) => console.log(l));
await browser.close();
