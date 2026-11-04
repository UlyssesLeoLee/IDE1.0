// 验证维度依赖链：1D→2D、2D→3D、3D→4D、4D→5D，以及 2D 的流程动画
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");

const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const logs = [];
page.on("console", (m) => { if (m.type() === "error") logs.push(`[${m.type()}] ${m.text()}`); });
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}\n${(e.stack || "").split("\n").slice(0, 4).join("\n")}`));

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 15000 });

const out = {};

// ---------- 链 1: 1D → 2D ----------
await page.click('[data-view="2D"]');
await page.waitForTimeout(800);
out.chain1_2D_from_1D = await page.evaluate(() => {
  const up = document.querySelector("#cUpstream");
  const dimmed = document.querySelectorAll("#cSvg .g-node.dim").length;
  const total = document.querySelectorAll("#cSvg .g-node").length;
  const focusBtn = document.querySelector("#cFocus");
  return {
    upstreamBadge: up ? up.textContent.replace(/\s+/g, " ").trim() : null,
    focusToggleText: focusBtn ? focusBtn.textContent : null,
    dimmedNodes: dimmed, totalNodes: total,
    hasDim: dimmed > 0 && dimmed < total
  };
});

// 反向：检查器 → Prompt
await page.locator("#cSvg .g-node").first().click();
await page.waitForTimeout(200);
out.chain1_reverse = await page.evaluate(() => {
  const links = Array.from(document.querySelectorAll("#rightBody [data-goprompt]")).map((b) => b.getAttribute("data-goprompt"));
  return { promptLinks: links };
});

// ---------- 导览（流程动画）----------
await page.click('[data-left="dims"]');
await page.waitForTimeout(200);
out.tourEntries = await page.locator("[data-tour]").allTextContents();

await page.locator("[data-tour]").first().click();
await page.waitForTimeout(900);
out.tourState = await page.evaluate(() => {
  const T = window.GemArcTour;
  const st = T ? T.state() : null;
  let err = null;
  try { T.start(T.list()[0].id); } catch (e) { err = e.message + " | " + (e.stack || "").split("\n")[1]; }
  return {
    hasTour: !!T,
    tourCount: T ? T.list().length : -1,
    state: st,
    startError: err,
    barExists: !!document.querySelector("#tourBar")
  };
});
out.tourStep1 = await page.evaluate(() => {
  const bar = document.querySelector("#tourBar");
  return {
    visible: bar && bar.style.display !== "none",
    title: bar ? (bar.querySelector(".g-tour-title") || {}).textContent : null,
    narrLen: bar && bar.querySelector(".g-tour-narr") ? bar.querySelector(".g-tour-narr").textContent.length : 0,
    prog: bar ? (bar.querySelector(".g-tour-prog") || {}).textContent : null,
    controls: bar ? bar.querySelectorAll(".g-tour-btn").length : 0
  };
});

// 前进两步，看是否跨视图
out.tourBarHtml = await page.evaluate(() => {
  const bar = document.querySelector("#tourBar");
  return bar ? bar.innerHTML.slice(0, 400) : "(no #tourBar)";
});
out.tourCtlCount = await page.locator('[data-tourctl]').count();
console.log("=== PART1 ===");
console.log(JSON.stringify(out, null, 2));

if (out.tourCtlCount > 0) {
  await page.locator('[data-tourctl="next"]').click();
  await page.waitForTimeout(700);
  await page.locator('[data-tourctl="next"]').click();
  await page.waitForTimeout(1200);
  out.tourStep3 = await page.evaluate(() => ({
    view: location.hash,
    title: (document.querySelector(".g-tour-title") || {}).textContent,
    narr: ((document.querySelector(".g-tour-narr") || {}).textContent || "").slice(0, 60)
  }));
}

// 导览高亮是否落到 2D 元素上
out.tourHighlight = await page.evaluate(() => ({
  hits: document.querySelectorAll(".g-tour-hit").length,
  rings: document.querySelectorAll(".g-tour-ring").length
}));

// 退出导览
await page.locator(".g-tour-x").click();
await page.waitForTimeout(400);
out.tourExited = await page.evaluate(() => {
  const bar = document.querySelector("#tourBar");
  return { hidden: !bar || bar.style.display === "none" };
});

// ---------- 链 2: 3D → 4D ----------
await page.click('[data-view="4D"]');
await page.waitForTimeout(1500);
await page.selectOption("#sCase", "CASE-VAULT-WRITE-NORMAL");
await page.click("#sRun");
await page.waitForTimeout(700);
const before = await page.evaluate(() => {
  const rings = {};
  document.querySelectorAll("#s3dStage canvas").length;
  return { events: document.querySelectorAll("#tlTrack .g-tl-ev").length };
});
// 点第一个事件 → 3D 状态应变化
await page.locator("#tlTrack .g-tl-ev").first().click();
await page.waitForTimeout(500);
out.chain2_3D_to_4D = await page.evaluate(() => {
  const meta = (document.querySelector("#tlMeta") || {}).textContent || "";
  const cur = document.querySelectorAll("#tlTrack .g-tl-ev.cur").length;
  return { tlMeta: meta, currentMarked: cur };
});
out.chain2_eventCount = before.events;

// ---------- 链 3: 4D → 5D ----------
await page.click('[data-view="5D"]');
await page.waitForTimeout(1200);
await page.selectOption("#sCase", "CASE-VAULT-WRITE-NORMAL");
await page.click("#sRun");
await page.waitForTimeout(400);
await page.selectOption("#sIv", "ChangeGuard");
const targets = await page.locator("#sIvTarget option").allTextContents();
await page.selectOption("#sIvTarget", "E-003");
await page.click("#sCompare");
await page.waitForTimeout(900);
out.chain3_4D_to_5D = await page.evaluate(() => ({
  causal: ((document.querySelector("#causalInfo") || {}).textContent || "").slice(0, 120),
  baselineRunExists: !!window.GemArcApp.state.baselineRun,
  baselineEvents: window.GemArcApp.state.baselineRun ? window.GemArcApp.state.baselineRun.eventCount : -1
}));
out.ivTargets = targets.length;

console.log(JSON.stringify(out, null, 2));
console.log("=== LOGS (" + logs.length + ") ===");
logs.slice(0, 10).forEach((l) => console.log(l));
await browser.close();
