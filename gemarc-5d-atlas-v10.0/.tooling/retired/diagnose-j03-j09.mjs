// 诊断 J03（2D 图窗）与 J09/J10（Prototype）的完整失败原因
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

// ---------- J03：逐个图种打开 ----------
await page.click('[data-view="2D"]');
await page.waitForTimeout(1000);
const btns = await page.locator(".g-diagbar .g-diagbtn").allTextContents();
console.log("diagram buttons:", btns.length, btns.join(","));
const perDiag = [];
for (let i = 0; i < btns.length; i++) {
  await page.locator(".g-diagbar .g-diagbtn").nth(i).click();
  await page.waitForTimeout(120);
  const win = page.locator(".g-diagwin").last();
  const n = await win.count();
  if (n === 0) { perDiag.push(btns[i] + ": NO WINDOW"); continue; }
  const svgCount = await win.locator("svg").count();
  const shapeCount = await win.locator("svg *").count();
  const title = await win.locator(".g-dw-title").textContent();
  perDiag.push(`${btns[i]}: svg=${svgCount} shapes=${shapeCount} title="${title}"`);
  await win.locator(".g-dw-close").click();
  await page.waitForTimeout(60);
}
console.log("=== per diagram ===");
perDiag.forEach((x) => console.log("  " + x));

// ---------- J09/J10：Prototype ----------
await page.click('[data-view="P"]');
await page.waitForTimeout(1500);
const p1 = await page.evaluate(() => ({
  tabs: document.querySelectorAll(".g-proto-tab").length,
  konvaCanvas: document.querySelectorAll("#prStage canvas").length,
  widgets: window.GemArcApp.state.protoEditor ? window.GemArcApp.state.protoEditor.serialise().widgets.length : -1,
  textInputs: document.querySelectorAll('#prPreview input[type="text"]').length,
  textareas: document.querySelectorAll("#prPreview textarea").length,
  anyInputs: document.querySelectorAll("#prPreview input").length,
  buttons: document.querySelectorAll("#prPreview button").length,
  previewHead: document.querySelector("#prPreview") ? document.querySelector("#prPreview").innerHTML.slice(0, 400) : null
}));
console.log("=== Prototype state ===");
console.log(JSON.stringify(p1, null, 2));

const before = await page.textContent("#prJson");
await page.click("#prAdd");
await page.waitForTimeout(300);
const added = await page.textContent("#prJson");
await page.click("#prUndo");
await page.waitForTimeout(300);
const undone = await page.textContent("#prJson");
await page.click("#prRedo");
await page.waitForTimeout(300);
const redone = await page.textContent("#prJson");
console.log("undo/redo:", JSON.stringify({
  changedOnAdd: before !== added,
  undoRestored: undone === before,
  redoReapplied: redone === added,
  lens: { before: before.length, added: added.length, undone: undone.length, redone: redone.length }
}));

console.log("=== LOGS (" + logs.length + ") ===");
logs.slice(0, 12).forEach((l) => console.log(l));
await browser.close();
