import { chromium } from "file:///E:/IDE1.0/gemarc-5d-atlas-v10.0/.tooling/node_modules/playwright/index.mjs";
import path from "node:path";
const ROOT = "E:\\IDE1.0\\gemarc-5d-atlas-v10.0";
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const say = (...a) => console.log(...a);
const INSTR = () => {
  window.__ctxCreated = 0; window.__ctxLost = 0; window.__renderCount = 0; window.__scenes = 0;
  const orig = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function(type, ...rest) {
    const c = orig.call(this, type, ...rest);
    if ((type === "webgl" || type === "webgl2") && c) {
      window.__ctxCreated++;
      this.addEventListener("webglcontextlost", () => { window.__ctxLost++; });
    }
    return c;
  };
  const origCreate = window.GemArcSpatial && window.GemArcSpatial.createScene;
  document.addEventListener("DOMContentLoaded", () => {
    if (window.GemArcSpatial) {
      const o = window.GemArcSpatial.createScene.bind(window.GemArcSpatial);
      window.GemArcSpatial.createScene = function(...a){ window.__scenes++; return o(...a); };
    }
  });
};

async function newPage(browser, viewport, init) {
  const ctx = await browser.newContext({ viewport });
  const page = await ctx.newPage();
  const errs = [], remote = [];
  page.on("console", m => { if (m.type()==="error") errs.push(m.text()); });
  page.on("pageerror", e => errs.push("pageerror: "+e.message));
  page.on("request", r => { if (!/^(file|data|blob):/.test(r.url())) remote.push(r.url()); });
  await page.addInitScript(INSTR);
  if (init) await page.addInitScript(init);
  await page.goto(FILE_URL, { waitUntil: "load" });
  await page.waitForSelector("#app .g-head", { timeout: 20000 });
  return { page, errs, remote };
}
const go = async (page, v) => { await page.click(`[data-view="${v}"]`); await page.waitForTimeout(320); };

const browser = await chromium.launch({ channel: "chrome" });

// ===================== C3 : 视图反复切换 =====================
say("################ C3 : 重复切换 / 监听器 / WebGL 上下文 ################");
{
  const { page, errs, remote } = await newPage(browser, { width:1440, height:900 });
  const VIEWS = ["1D","2D","3D","4D","5D","P"];
  const snap = async () => page.evaluate(() => ({
    viewRootChildren: document.querySelector("#viewRoot").children.length,
    totalEls: document.querySelectorAll("*").length,
    ctxCreated: window.__ctxCreated, ctxLost: window.__ctxLost, scenes: window.__scenes,
    canvases: document.querySelectorAll("canvas").length,
    canvasesInDoc: Array.from(document.querySelectorAll("canvas")).map(c=>c.width+"x"+c.height).join(",")
  }));
  await go(page,"2D");
  const n2d = await page.$$eval("#cStage .g-node", n=>n.length);
  say(`2D node count (baseline) = ${n2d}`);
  const base = await snap();
  say(`baseline: ${JSON.stringify(base)}`);
  // 12 cycles through ALL views
  for (let i=0;i<12;i++) { for (const v of VIEWS) await go(page, v); }
  const after = await snap();
  say(`after 12 full cycles (${12*VIEWS.length} view switches): ${JSON.stringify(after)}`);
  say(`  total DOM elements: ${base.totalEls} -> ${after.totalEls}  (delta ${after.totalEls-base.totalEls})`);
  say(`  WebGL contexts created: ${base.ctxCreated} -> ${after.ctxCreated}; lost: ${after.ctxLost}; scenes created: ${after.scenes}`);
  // render calls per second (rAF loop accumulation)
  const measure = async (label) => {
    await page.evaluate(()=>{ window.__renderCount=0; });
    await page.waitForTimeout(1000);
    const rc = await page.evaluate(()=>window.__renderCount);
    say(`  render() calls in 1s [${label}] = ${rc}`);
    return rc;
  };
  await go(page,"3D"); await page.waitForTimeout(600);
  const rc1 = await measure("after 1 visit to 3D");
  await go(page,"5D"); await go(page,"3D"); await page.waitForTimeout(600);
  const rc2 = await measure("after 3D->5D->3D");
  await go(page,"5D"); await go(page,"3D"); await go(page,"5D"); await go(page,"3D"); await page.waitForTimeout(600);
  const rc3 = await measure("after 4 more 3D->5D->3D cycles");
  say(`  rAF/render loop growth: ${rc1} -> ${rc2} -> ${rc3}  ${rc3>rc1 ? "*** LEAK: loops accumulate ***" : "no growth"}`);
  await go(page,"2D");
  const n2d2 = await page.$$eval("#cStage .g-node", n=>n.length);
  say(`2D node count after cycles = ${n2d2} (baseline ${n2d}) ${n2d2!==n2d?"*** DUPLICATE DOM ***":"ok"}`);
  say(`console errors: ${errs.length}`); errs.slice(0,5).forEach(e=>say("   ! "+e));
  say(`remote requests: ${remote.length}`); remote.slice(0,5).forEach(e=>say("   ! "+e));
  await page.close();
}

// ===================== C5 : 坐标手性 =====================
say("");
say("################ C5 : 左手语义 -> 右手渲染 (toThree) ################");
{
  const { page } = await newPage(browser, { width:1440, height:900 });
  await go(page,"3D");
  await page.waitForSelector("#s3dStage canvas", { timeout:15000 });
  await page.waitForTimeout(900);
  const r = await page.evaluate(() => {
    const S = window.GemArcApp.state;
    const sc = S.scene3d;
    const THREEm = window.THREE;
    const cam = sc.camera;
    const camWorld = cam.position.clone();
    // semantic camera position = toSemantic
    const semCam = { x: camWorld.x, y: camWorld.y, z: -camWorld.z };
    // pick nodes that differ ONLY in semantic X within the same room/parent
    const sp = window.GemArcModels.spatial;
    const arch = window.GemArcModels.architecture;
    const off = sp.nodeOffsets;
    const byParent = {};
    arch.nodes.forEach(n => {
      const o = off[n.id]; if (!o) return;
      (byParent[n.parentId] = byParent[n.parentId] || []).push({ id:n.id, sem:o, parent:n.parentId });
    });
    const pairs = [];
    Object.keys(byParent).forEach(pid => {
      const arr = byParent[pid].slice().sort((a,b)=>a.sem[0]-b.sem[0]);
      if (arr.length >= 2) pairs.push([arr[0], arr[arr.length-1]]);
    });
    // project via camera
    const v = new THREEm.Vector3();
    const w = sc.renderer.domElement.clientWidth, h = sc.renderer.domElement.clientHeight;
    const proj = (semOff, parentPos) => {
      const tp = { x: parentPos[0]+semOff[0], y: parentPos[1]+semOff[1], z: -(parentPos[2]+semOff[2]) };
      v.set(tp.x, tp.y, tp.z).project(cam);
      return { screenX: (v.x*0.5+0.5)*w, ndcZ: v.z };
    };
    const results = pairs.slice(0,12).map(([a,b]) => {
      const rp = sp.rooms[a.parent].position;
      const pa = proj(a.sem, rp), pb = proj(b.sem, rp);
      return { parent:a.parent, left:{id:a.id, semX:a.sem[0], screenX:pa.screenX}, right:{id:b.id, semX:b.sem[0], screenX:pb.screenX},
               monotonic: (b.sem[0]-a.sem[0])*(pb.screenX-pa.screenX) >= 0 };
    });
    return { camWorld:{x:camWorld.x,y:camWorld.y,z:camWorld.z}, semCam, results,
             toThreeSrc: String(window.GemArcSpatial.toThree([1,2,3])) };
  });
  say(`  camera world (Three) = ${JSON.stringify(r.camWorld)}  => semantic (toSemantic) = ${JSON.stringify(r.semCam)}`);
  say(`  toThree([1,2,3]) = ${r.toThreeSrc}`);
  let allMono = true;
  r.results.forEach(x => { if(!x.monotonic) allMono=false;
    say(`   ${x.parent}: semX ${x.left.semX} (${x.left.id}) screenX=${x.left.screenX.toFixed(1)}  ->  semX ${x.right.semX} (${x.right.id}) screenX=${x.right.screenX.toFixed(1)}  monotonic=${x.monotonic}`); });
  say(`  ALL pairs: larger semantic +X appears further RIGHT = ${allMono}`);
  const camDirSemantic = { x: 0, y: 0, z: 1 };
  say(`  camera sits at semantic z=${r.semCam.z} (scene front is semantic +Z); camera looks toward +Z`);
  await page.close();
}

// ===================== C6 : 无 WebGL =====================
say("");
say("################ C6 : WebGL 不可用 -> DEGRADED + 可交互 2D ################");
{
  const noWebGL = () => {
    const orig = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function(type, ...rest) {
      if (type === "webgl" || type === "webgl2") return null;
      return orig.call(this, type, ...rest);
    };
  };
  const { page, errs } = await newPage(browser, { width:1440, height:900 }, noWebGL);
  await go(page,"3D");
  await page.waitForTimeout(900);
  const d = await page.evaluate(() => {
    const deg = Array.from(document.querySelectorAll(".g-degraded")).map(e=>e.textContent.trim());
    const status = document.querySelector("#s3dStatus");
    const svg = document.querySelector("#s3dStage svg");
    const circles = document.querySelectorAll("#s3dStage circle");
    const withDataNode = document.querySelectorAll("#s3dStage [data-node]").length;
    return { degraded: deg, status: status?status.textContent.trim():"(none)",
             hasFallbackSvg: !!svg, circles: circles.length, withDataNode,
             canvasPresent: document.querySelectorAll("#s3dStage canvas").length,
             cssPerspective: (()=>{ const s=document.createElement("div"); s.style.perspective=""; return getComputedStyle(document.querySelector("#s3dStage")||document.body).perspective; })(),
             anyImg: document.querySelectorAll("#s3dStage img").length };
  });
  say(`  .g-degraded banners: ${JSON.stringify(d.degraded)}`);
  say(`  #s3dStatus = "${d.status}"`);
  say(`  fallback svg=${d.hasFallbackSvg} circles=${d.circles} elements with data-node=${d.withDataNode} canvas=${d.canvasPresent} img=${d.anyImg}`);
  say(`  computed perspective on stage = "${d.cssPerspective}"`);
  // is the fallback interactive? click a fallback circle and see if inspector updates
  const inter = await page.evaluate(() => {
    const before = document.querySelector("#rightBody").textContent.trim();
    const c = document.querySelector("#s3dStage circle");
    if (!c) return { before, after: before, clicked:false };
    c.dispatchEvent(new MouseEvent("click", {bubbles:true}));
    return { before, after: document.querySelector("#rightBody").textContent.trim(), clicked:true };
  });
  say(`  fallback click -> inspector changed? ${inter.before !== inter.after} (clicked=${inter.clicked})`);
  // can the user still reach a genuinely interactive 2D view?
  await go(page,"2D");
  await page.waitForTimeout(500);
  const two = await page.evaluate(() => {
    const n = document.querySelectorAll("#cStage .g-node").length;
    const node = document.querySelector('#cStage .g-node[data-node="N-WB-AUTH"]');
    let changed=false, before="", after="";
    if (node) { before = document.querySelector("#rightBody").textContent.trim();
      node.dispatchEvent(new MouseEvent("click",{bubbles:true}));
      after = document.querySelector("#rightBody").textContent.trim(); changed = before!==after; }
    return { nodes:n, interactive2D: changed };
  });
  say(`  2D view still interactive: nodes=${two.nodes} inspector-updates-on-click=${two.interactive2D}`);
  say(`  console errors (no-webgl): ${errs.length}`); errs.slice(0,4).forEach(e=>say("   ! "+e));
  await page.close();
}

// ===================== C12 : 390px =====================
say("");
say("################ C12 : 390px 检查器与 5D 因果结论 ################");
{
  const { page, errs } = await newPage(browser, { width:390, height:844 });
  await go(page,"5D");
  await page.selectOption("#sCase","C-PATH-ESCAPE"); await page.waitForTimeout(200);
  await page.click("#sRun"); await page.waitForTimeout(500);
  await page.selectOption("#sIv","DisableNode"); await page.waitForTimeout(200);
  const tg = await page.$$eval("#sIvTarget option", o=>o.map(x=>x.value));
  await page.selectOption("#sIvTarget", tg.includes("N-WB-PATHSAFETY")?"N-WB-PATHSAFETY":tg[0]);
  await page.click("#sCompare"); await page.waitForTimeout(900);
  const r = await page.evaluate(() => {
    const vis = (el) => { if(!el) return {exists:false};
      const cs = getComputedStyle(el); const rect = el.getBoundingClientRect();
      return { exists:true, display:cs.display, visibility:cs.visibility, opacity:cs.opacity,
               w:Math.round(rect.width), h:Math.round(rect.height), top:Math.round(rect.top), left:Math.round(rect.left),
               overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth }; };
    const info = document.querySelector("#causalInfo");
    const right = document.querySelector("#rightBody");
    const rightPanel = document.querySelector("#right");
    return {
      causalInfo: info ? info.textContent.trim() : "(missing)",
      causalInfoVis: vis(info),
      rightBodyVis: vis(right), rightPanelVis: vis(rightPanel),
      rightBodyText: right ? right.textContent.trim().slice(0,160) : "(missing)",
      overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth
    };
  });
  say(`  causalInfo text = "${r.causalInfo}"`);
  say(`  causalInfo visibility = ${JSON.stringify(r.causalInfoVis)}`);
  say(`  #right (inspector panel) = ${JSON.stringify(r.rightPanelVis)}`);
  say(`  #rightBody = ${JSON.stringify(r.rightBodyVis)}`);
  say(`  inspector text = "${r.rightBodyText}"`);
  say(`  horizontal overflow = ${r.overflowX}px`);
  say(`  console errors: ${errs.length}`); errs.slice(0,4).forEach(e=>say("   ! "+e));
  await page.close();
}

// ===================== C10 : 原型 DOM 预览真实控件 =====================
say("");
say("################ C10 : 原型 DOM 预览可输入 / 序列化含 actions+guards+state+fixtures ################");
{
  const { page } = await newPage(browser, { width:1440, height:900 });
  await go(page,"P");
  await page.waitForSelector(".g-proto-tab", { timeout:15000 });
  const tab = await page.$$eval(".g-proto-tab", t=>t.map(x=>x.getAttribute("data-screen")));
  say(`  proto tabs: ${tab.join(",")}`);
  const inputs = await page.$$eval("#prPreview input, #prPreview textarea, #prPreview button, #prPreview select", e=>e.map(x=>x.tagName+":"+(x.type||"")));
  say(`  DOM preview controls: ${inputs.length} -> ${inputs.join(",")}`);
  const ta = await page.$("#prPreview textarea");
  if (ta) { await ta.fill("fn main(){ /*audit*/ }"); const v = await ta.inputValue();
    say(`  textarea accepted input: "${v}"`); } else say("  no textarea");
  const btn = await page.$("#prPreview button");
  if (btn) { const b0 = await page.evaluate(()=>document.querySelector("#prJson").textContent);
    await btn.click(); await page.waitForTimeout(400);
    const b1 = await page.evaluate(()=>document.querySelector("#prJson").textContent);
    say(`  preview button click changed prototype JSON: ${b0!==b1}`); }
  const ser = await page.evaluate(() => {
    const ed = window.GemArcApp.state.protoEditor;
    if (!ed) return null;
    const s = ed.serialise();
    return { keys:Object.keys(s), widgets:(s.widgets||[]).length,
             actions:(s.widgets||[]).reduce((n,w)=>n+((w.actions||[]).length),0),
             guards:(s.widgets||[]).reduce((n,w)=>n+((w.guards||[]).length),0),
             stateTable: !!s.stateTable, fixtures: s.fixtures?Object.keys(s.fixtures).length:0,
             sample: JSON.stringify((s.widgets||[])[0]).slice(0,300) };
  });
  say(`  serialise(): ${JSON.stringify(ser)}`);
  await page.close();
}
await browser.close();


