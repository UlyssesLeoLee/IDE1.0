import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");

const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const logs = [];
page.on("console", (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}`));

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 15000 });
await page.click('[data-view="2D"]');
await page.waitForTimeout(1200);

// 用 elementFromPoint 探测命中
const probe = await page.evaluate(() => {
  const hits = document.querySelectorAll("#cSvg .g-edge-hit");
  const h = hits[0];
  const r = h.getBoundingClientRect();
  const cx = r.x + r.width / 2, cy = r.y + r.height / 2;
  const at = document.elementFromPoint(cx, cy);
  const cs = getComputedStyle(h);
  return {
    hitCount: hits.length,
    rect: { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) },
    center: [Math.round(cx), Math.round(cy)],
    elementFromPoint: at ? (at.getAttribute("class") || at.tagName) : null,
    hitStyle: { stroke: cs.stroke, strokeWidth: cs.strokeWidth, pointerEvents: cs.pointerEvents, visibility: cs.visibility },
    svgRect: (() => { const s = document.querySelector("#cSvg").getBoundingClientRect(); return { w: Math.round(s.width), h: Math.round(s.height) }; })()
  };
});
console.log("PROBE:", JSON.stringify(probe, null, 2));

// 用 dispatchEvent 触发（绕过 Playwright 的可点性检查）
const viaDispatch = await page.evaluate(() => {
  const h = document.querySelector("#cSvg .g-edge-hit");
  h.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  return document.querySelector("#rightBody").textContent.slice(0, 80);
});
console.log("after dispatch click, inspector:", JSON.stringify(viaDispatch));

console.log("LOGS:", logs.join(" | "));
await browser.close();
