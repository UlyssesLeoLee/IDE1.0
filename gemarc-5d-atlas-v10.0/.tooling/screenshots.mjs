/**
 * .tooling/screenshots.mjs — 视觉证据采集（真实浏览器截图）
 *
 * 按「维度 × 典型状态 × 视口」采集，并记录每张图的可访问性/布局观测：
 *   - 是否有横向溢出
 *   - 检查器/时间轴是否可见
 *   - 关键文字是否溢出容器
 * 输出：qa/screenshots/*.png + qa/screenshots.json
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const OUT = path.join(ROOT, "qa", "screenshots");
fs.mkdirSync(OUT, { recursive: true });

const VIEWPORTS = [
  { name: "desktop-1440", width: 1440, height: 900 },
  { name: "narrow-390", width: 390, height: 844 }
];
const SHOTS = [
  { view: "1D", file: "1d-prompt", state: "默认 Prompt（目标系统实现提示词）" },
  { view: "2D", file: "2d-canvas", state: "全元素无限画布" },
  { view: "3D", file: "3d-scene", state: "三维图论场景（Overview）" },
  { view: "4D", file: "4d-timeline", state: "动态 Case 回放" },
  { view: "5D", file: "5d-lab", state: "因果实验台" },
  { view: "P", file: "prototype", state: "Prototype 工作区（Konva + DOM 预览）" }
];

const browser = await chromium.launch({ channel: "chrome" });
const report = [];

for (const vp of VIEWPORTS) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height } });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(FILE_URL, { waitUntil: "load" });
  await page.waitForSelector("#app .g-head", { timeout: 20000 });

  for (const shot of SHOTS) {
    await page.click(`[data-view="${shot.view}"]`);
    await page.waitForTimeout(shot.view === "3D" || shot.view === "4D" || shot.view === "5D" ? 1400 : 700);
    if (shot.view === "4D" || shot.view === "5D") {
      const run = await page.$("#sRun");
      if (run) { await run.click(); await page.waitForTimeout(800); }
    }
    const file = path.join(OUT, `${shot.file}-${vp.name}.png`);
    await page.screenshot({ path: file, fullPage: false });

    const obs = await page.evaluate(() => {
      const de = document.documentElement;
      const insp = document.querySelector("#rightPane");
      const tl = document.querySelector("#timeline");
      const overflow = de.scrollWidth - de.clientWidth;
      // 文字溢出：找出宽度明显超出父容器的文本节点元素
      let clipped = 0;
      document.querySelectorAll("#app *").forEach((el) => {
        if (el.children.length === 0 && el.textContent && el.textContent.trim().length > 6) {
          if (el.scrollWidth - el.clientWidth > 4) clipped++;
        }
      });
      return {
        horizontalOverflow: overflow,
        inspectorVisible: insp ? insp.getBoundingClientRect().width > 0 : false,
        timelineVisible: tl ? getComputedStyle(tl).display !== "none" : false,
        clippedTextCount: clipped,
        webglCanvas: !!document.querySelector("#s3dStage canvas"),
        konvaCanvas: !!document.querySelector("#prStage canvas")
      };
    });
    report.push({ view: shot.view, state: shot.state, viewport: vp.name, file: path.relative(ROOT, file).replace(/\\/g, "/"), ...obs, pageErrors: errors.length });
    console.log(`  ${vp.name.padEnd(14)} ${shot.view.padEnd(3)} overflow=${obs.horizontalOverflow} inspector=${obs.inspectorVisible} timeline=${obs.timelineVisible} clipped=${obs.clippedTextCount} → ${path.basename(file)}`);
  }
  await ctx.close();
}
await browser.close();
fs.writeFileSync(path.join(ROOT, "qa", "screenshots.json"), JSON.stringify({ schema: "ide1.5d-atlas/screenshots/v10.0", generatedAt: new Date().toISOString(), shots: report }, null, 2), "utf8");
console.log("screenshots written: " + report.length);
