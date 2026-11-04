// 诊断：为什么 file:// 打开后 #app .g-head 不出现
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");

const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage();
const logs = [];
page.on("console", (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}\n${(e.stack || "").split("\n").slice(0, 6).join("\n")}`));
page.on("requestfailed", (r) => logs.push(`[requestfailed] ${r.url()} ${r.failure()?.errorText}`));

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForTimeout(3000);

const probe = await page.evaluate(() => {
  const app = document.querySelector("#app");
  return {
    title: document.title,
    hasApp: !!app,
    appChildCount: app ? app.children.length : -1,
    appHtmlLen: app ? app.innerHTML.length : -1,
    hasHead: !!document.querySelector(".g-head"),
    headRect: (() => {
      const h = document.querySelector(".g-head");
      if (!h) return null;
      const r = h.getBoundingClientRect();
      return { w: r.width, h: r.height, top: r.top };
    })(),
    hasModels: typeof window.GemArcModels,
    modelKeys: window.GemArcModels ? Object.keys(window.GemArcModels) : null,
    hasAppNs: typeof window.GemArcApp,
    hasTHREE: typeof window.THREE,
    hasKonva: typeof window.Konva,
    hasGraphology: typeof window.Graphology,
    scriptCount: document.scripts.length,
    readyState: document.readyState
  };
});

console.log("=== PROBE ===");
console.log(JSON.stringify(probe, null, 2));
console.log("=== LOGS (" + logs.length + ") ===");
logs.slice(0, 25).forEach((l) => console.log(l));

await browser.close();
