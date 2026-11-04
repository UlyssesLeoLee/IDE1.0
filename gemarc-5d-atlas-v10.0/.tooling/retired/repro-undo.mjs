// 精确复现 E2E J09 的步骤并 diff undo 前后的 JSON
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");

const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
page.on("pageerror", (e) => console.log("[pageerror]", e.message));

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 15000 });
await page.click('[data-view="P"]');
await page.waitForTimeout(1200);

// —— 完全照抄 E2E 的步骤 ——
await page.locator("#prPreview input[type=text]").first().fill("hello-atlas");
await page.locator("#prPreview button").first().click();
await page.waitForTimeout(150);

const before = await page.locator("#prJson").textContent();
await page.click("#prAdd");
await page.waitForTimeout(200);
const added = await page.locator("#prJson").textContent();
await page.click("#prUndo");
await page.waitForTimeout(200);
const undone = await page.locator("#prJson").textContent();

const wb = JSON.parse(before), wu = JSON.parse(undone);
console.log("before widgets:", wb.widgets.map((w) => w.id).join(","));
console.log("undone widgets:", wu.widgets.map((w) => w.id).join(","));
console.log("before len:", before.length, "undone len:", undone.length);
console.log("equal:", before === undone);

// 找出第一个差异位置
let i = 0;
while (i < Math.min(before.length, undone.length) && before[i] === undone[i]) i++;
console.log("first diff at char", i);
console.log("before  :", JSON.stringify(before.slice(Math.max(0, i - 120), i + 160)));
console.log("undone  :", JSON.stringify(undone.slice(Math.max(0, i - 120), i + 160)));

// undo 栈状态
const st = await page.evaluate(() => {
  const ed = window.GemArcApp.state.protoEditor;
  return { hasEditor: !!ed };
});
console.log("editor:", JSON.stringify(st));

await browser.close();
