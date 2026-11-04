/** 自审修复后的补充视觉证据：原型子标签页 + 5D 分屏模式 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const OUT = path.join(ROOT, "qa", "screenshots");
fs.mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch({ channel: "chrome" });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const page = await ctx.newPage();
const errs = [];
page.on("pageerror", (e) => errs.push(e.message));
page.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 20000 });

// 1) 原型：切到「组件库」子标签
await page.click('[data-view="P"]');
await page.waitForSelector(".g-subtab", { timeout: 15000 });
await page.waitForTimeout(600);
await page.click('.g-subtab[data-sub="组件库"]');
await page.waitForTimeout(500);
await page.screenshot({ path: path.join(OUT, "prototype-component-library-desktop-1440.png") });
const libCount = await page.$$eval("#prLibrary .g-proto-lib-btn", (b) => b.length);

// 2) 5D：SplitSynchronized3D 分屏
await page.click('[data-view="5D"]');
await page.waitForSelector("#sIvMode", { timeout: 10000 });
await page.selectOption("#sCase", "C-PATH-ESCAPE");
await page.click("#sRun");
await page.waitForTimeout(500);
await page.selectOption("#sIv", "DisableNode");
const tvals = await page.$$eval("#sIvTarget option", (o) => o.map((x) => x.value));
await page.selectOption("#sIvTarget", tvals.includes("N-WB-PATHSAFETY") ? "N-WB-PATHSAFETY" : tvals[0]);
await page.click("#sCompare");
await page.waitForTimeout(900);
await page.selectOption("#sIvMode", "SplitSynchronized3D");
await page.waitForTimeout(1000);
await page.screenshot({ path: path.join(OUT, "5d-split-synchronized-desktop-1440.png") });
const splitActive = await page.evaluate(() => !!(window.GemArcApp.state.scene3d && window.GemArcApp.state.scene3d.state.split.active));

// 3) 5D：BranchTimeline 双泳道
await page.selectOption("#sIvMode", "BranchTimeline");
await page.waitForTimeout(800);
await page.screenshot({ path: path.join(OUT, "5d-branch-timeline-desktop-1440.png") });
const lanes = await page.$$eval(".g-tl-lane", (l) => l.length);

await browser.close();
console.log("组件库按钮:", libCount, "| split.active:", splitActive, "| 时间轴泳道:", lanes, "| console error:", errs.length, errs.slice(0, 3).join(" | "));
