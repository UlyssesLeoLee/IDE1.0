import { chromium } from "file:///E:/IDE1.0/gemarc-5d-atlas-v10.0/.tooling/node_modules/playwright/index.mjs";
import path from "node:path";
const ROOT = "E:\\IDE1.0\\gemarc-5d-atlas-v10.0";
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g,"/");
const say = (...a)=>console.log(...a);
const INSTR = () => {
  window.__renders = 0; window.__ctxCreated = 0; window.__ctxLost = 0; window.__scenes = 0;
  const og = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function(t, ...r) {
    const c = og.call(this, t, ...r);
    if ((t==="webgl"||t==="webgl2") && c) { window.__ctxCreated++;
      this.addEventListener("webglcontextlost", ()=>{ window.__ctxLost++; window.__lastLostCanvas = this; }); }
    return c;
  };
  const hook = () => {
    if (!window.THREE || !window.THREE.WebGLRenderer) return;
    const p = window.THREE.WebGLRenderer.prototype;
    if (p.__patched) return; p.__patched = true;
    const orig = p.render;
    p.render = function(...a){ window.__renders++; return orig.apply(this, a); };
  };
  const iv = setInterval(hook, 5);
  setTimeout(()=>clearInterval(iv), 15000);
  const oCS = window.GemArcSpatial && window.GemArcSpatial.createScene;
  document.addEventListener("DOMContentLoaded", () => {
    if (window.GemArcSpatial) {
      const o = window.GemArcSpatial.createScene.bind(window.GemArcSpatial);
      window.GemArcSpatial.createScene = function(...a){ window.__scenes++; return o(...a); };
    }
  });
};
const browser = await chromium.launch({ channel: "chrome" });
const ctx = await browser.newContext({ viewport:{width:1440,height:900} });
const page = await ctx.newPage();
const errs=[]; page.on("console",m=>{if(m.type()==="error")errs.push(m.text());}); page.on("pageerror",e=>errs.push("pageerror: "+e.message));
await page.addInitScript(INSTR);
await page.goto(FILE_URL,{waitUntil:"load"});
await page.waitForSelector("#app .g-head",{timeout:20000});
const go = async v => { await page.click(`[data-view="${v}"]`); await page.waitForTimeout(300); };
const meas = async label => {
  await page.evaluate(()=>{ window.__renders=0; });
  await page.waitForTimeout(1000);
  const r = await page.evaluate(()=>({renders:window.__renders, ctx:window.__ctxCreated, lost:window.__ctxLost, scenes:window.__scenes}));
  say(`  ${label}: renders/1s=${r.renders} ctxCreated=${r.ctx} ctxLost=${r.lost} scenes=${r.scenes}`);
  return r;
};
say("### C3 rAF/render loop accumulation ###");
await go("3D"); await page.waitForTimeout(700);
const m1 = await meas("single 3D visit");
await go("5D"); await go("3D"); await page.waitForTimeout(700);
const m2 = await meas("after 3D->5D->3D");
for (let i=0;i<4;i++){ await go("5D"); await go("3D"); }
await page.waitForTimeout(700);
const m3 = await meas("after 4 more 3D->5D->3D cycles");
say(`  VERDICT: renders/1s ${m1.renders} -> ${m2.renders} -> ${m3.renders}  ${m3.renders>m1.renders?"*** rAF LOOPS ACCUMULATE ***":"stable"}`);
say("");
say("### WebGL context exhaustion: is the LIVE canvas ever context-lost? ###");
await go("2D");
for (let i=0;i<25;i++){ await go("3D"); await go("5D"); }
await page.waitForTimeout(1200);
const live = await page.evaluate(()=>{
  const canvases = Array.from(document.querySelectorAll("canvas"));
  const c = canvases[canvases.length-1];
  let gl=null; try{ gl = c && (c.getContext("webgl2")||c.getContext("webgl")); }catch(e){}
  return { canvases: canvases.length, ctxCreated: window.__ctxCreated, ctxLost: window.__ctxLost,
           liveCanvasHasGL: !!gl, liveSize: c? c.width+"x"+c.height : "(none)",
           lastLostWasLive: window.__lastLostCanvas === c,
           lostCanvasStillInDoc: window.__lastLostCanvas ? document.body.contains(window.__lastLostCanvas) : null };
});
say(`  ${JSON.stringify(live)}`);
// is the scene actually rendering pixels?
const px = await page.evaluate(()=>{
  const c = document.querySelector("#s3dStage canvas");
  if (!c) return { canvas:false };
  const gl = c.getContext("webgl2")||c.getContext("webgl");
  if (!gl) return { canvas:true, gl:false };
  const w=c.width,h=c.height; const buf=new Uint8Array(4*16);
  gl.readPixels(Math.floor(w/2), Math.floor(h/2), 4,4, gl.RGBA, gl.UNSIGNED_BYTE, buf);
  return { canvas:true, gl:true, centerPixels:Array.from(buf.slice(0,12)) };
});
say(`  live scene pixels: ${JSON.stringify(px)}`);
say(`  console errors: ${errs.length}`); errs.slice(0,6).forEach(e=>say("   ! "+e));
await browser.close();
