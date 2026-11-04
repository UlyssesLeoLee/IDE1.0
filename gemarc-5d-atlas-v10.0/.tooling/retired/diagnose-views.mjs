// 诊断 E2E 剩余失败：2D / 4D / 5D / Prototype 四视图的实际 DOM 状态
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");

const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const logs = [];
page.on("console", (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}\n${(e.stack || "").split("\n").slice(0, 5).join("\n")}`));

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 15000 });

// ---------- 2D ----------
await page.click('[data-view="2D"]');
await page.waitForTimeout(1200);
const d2 = await page.evaluate(() => ({
  svgExists: !!document.querySelector("#cSvg"),
  svgChildren: document.querySelector("#cSvg") ? document.querySelector("#cSvg").children.length : -1,
  nodeCount: document.querySelectorAll("#cSvg .g-node").length,
  edgeCount: document.querySelectorAll("#cSvg .g-edge").length,
  portCount: document.querySelectorAll("#cSvg .g-port").length,
  diagBtns: document.querySelectorAll(".g-diagbar .g-diagbtn").length,
  meta: document.querySelector("#cMeta") ? document.querySelector("#cMeta").textContent : null,
  diagHostHtmlLen: document.querySelector("#diagHost") ? document.querySelector("#diagHost").innerHTML.length : -1
}));
console.log("=== 2D ===");
console.log(JSON.stringify(d2, null, 2));

// ---------- 4D ----------
await page.click('[data-view="4D"]');
await page.waitForTimeout(800);
const d4pre = await page.evaluate(() => ({
  hasCaseSel: !!document.querySelector("#sCase"),
  caseOptions: document.querySelectorAll("#sCase option").length,
  firstCase: document.querySelector("#sCase option") ? document.querySelector("#sCase option").value : null,
  modelInputs: (() => {
    const c = window.GemArcModels.cases.cases.find((x) => x.id === "CASE-VAULT-WRITE-NORMAL");
    return c ? c.inputs : null;
  })()
}));
console.log("=== 4D pre ===");
console.log(JSON.stringify(d4pre, null, 2));

await page.selectOption("#sCase", "CASE-VAULT-WRITE-NORMAL");
await page.click("#sRun");
await page.waitForTimeout(800);
const d4 = await page.evaluate(() => ({
  runInfo: document.querySelector("#runInfo") ? document.querySelector("#runInfo").textContent : null,
  tlEvents: document.querySelectorAll("#tlTrack .g-tl-ev").length,
  tlMeta: document.querySelector("#tlMeta") ? document.querySelector("#tlMeta").textContent : null,
  engineDirect: (() => {
    try {
      const m = window.GemArcModels;
      const E = window.GemArcEngine;
      const eng = E.createEngine({
        nodes: m.architecture.nodes, edges: m.architecture.edges,
        rules: m["causal-rules"].rules, cases: m.cases.cases,
        stateMachines: m["causal-rules"].stateMachines, version: "t"
      });
      const c = m.cases.cases.find((x) => x.id === "CASE-VAULT-WRITE-NORMAL");
      const r = eng.run(c.id, { seed: 1, inputs: Object.assign({}, c.inputs) });
      return { events: r.eventCount, transitions: r.events.filter((e) => e.kind === "Transition").length };
    } catch (e) { return { error: e.message }; }
  })()
}));
console.log("=== 4D post ===");
console.log(JSON.stringify(d4, null, 2));

// ---------- 5D ----------
await page.click('[data-view="5D"]');
await page.waitForTimeout(800);
const d5pre = await page.evaluate(() => ({
  hasIv: !!document.querySelector("#sIv"),
  ivOptions: document.querySelectorAll("#sIv option").length,
  hasCase: !!document.querySelector("#sCase")
}));
console.log("=== 5D pre ===");
console.log(JSON.stringify(d5pre, null, 2));
if (d5pre.hasIv) {
  await page.selectOption("#sIv", "InjectResourceFailure");
  const targetOpts = await page.evaluate(() => Array.from(document.querySelectorAll("#sIvTarget option")).map((o) => o.value));
  console.log("ivTargets:", JSON.stringify(targetOpts.slice(0, 8)));
  if (targetOpts.includes("N-VAULT-FILE")) {
    await page.selectOption("#sIvTarget", "N-VAULT-FILE");
    await page.selectOption("#sCase", "CASE-VAULT-WRITE-NORMAL");
    await page.click("#sRun");
    await page.waitForTimeout(400);
    await page.click("#sCompare");
    await page.waitForTimeout(800);
    const d5 = await page.evaluate(() => ({
      causalInfo: document.querySelector("#causalInfo") ? document.querySelector("#causalInfo").textContent.slice(0, 400) : null
    }));
    console.log("=== 5D post ===");
    console.log(JSON.stringify(d5, null, 2));
  }
}

// ---------- Prototype ----------
await page.click('[data-view="P"]');
await page.waitForTimeout(1200);
const dp = await page.evaluate(() => {
  const stage = document.querySelector("#prStage");
  return {
    tabs: document.querySelectorAll(".g-proto-tab").length,
    konvaCanvas: document.querySelectorAll("#prStage canvas").length,
    konvaChildren: stage ? stage.children.length : -1,
    prJson: document.querySelector("#prJson") ? document.querySelector("#prJson").textContent.slice(0, 260) : null,
    previewInputs: document.querySelectorAll("#prPreview input").length,
    previewButtons: document.querySelectorAll("#prPreview button").length,
    appState: (() => {
      try {
        const ed = window.GemArcApp && window.GemArcApp.state && window.GemArcApp.state.protoEditor;
        if (!ed) return "no editor in state";
        const s = ed.serialise();
        return { widgets: s.widgets.length, format: s.format, keys: Object.keys(s) };
      } catch (e) { return "err: " + e.message; }
    })(),
    konvaGlobal: typeof window.Konva,
    stageFind: (() => {
      try {
        const st = window.GemArcApp.state.protoEditor.stage;
        return { all: st.getLayers()[0].getChildren().length, named: st.getLayers()[0].find(".widget").length };
      } catch (e) { return "err: " + e.message; }
    })()
  };
});
console.log("=== Prototype ===");
console.log(JSON.stringify(dp, null, 2));

console.log("=== LOGS (" + logs.length + ") ===");
logs.slice(0, 20).forEach((l) => console.log(l));
await browser.close();
