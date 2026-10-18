// 透明主题「动态反色」端到端验证 —— 真改注册表, 真观察前端是否跟着反色.
// 流程: AppsUseLightTheme 0 -> 等轮询 -> 1 -> 等轮询 -> 还原.
// 用法: node verify-invert-live.mjs   (需先启动 ide-shell-web 并设好 env)

import { chromium } from "playwright";
import { execFileSync } from "child_process";

const REG = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const ADDR = process.env.IDE_SHELL_WEB_ADDR || "127.0.0.1:8550";

function setLight(v) {
  execFileSync("reg", ["add", REG, "/v", "AppsUseLightTheme", "/t", "REG_DWORD", "/d", String(v), "/f"], {
    stdio: "ignore",
  });
}
function getLight() {
  try {
    const out = execFileSync("reg", ["query", REG, "/v", "AppsUseLightTheme"], { encoding: "utf8" });
    const m = out.match(/AppsUseLightTheme\s+REG_DWORD\s+0x([0-9a-f]+)/i);
    return m ? parseInt(m[1], 16) : null;
  } catch {
    return null;
  }
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function probe(page) {
  return page.evaluate(() => {
    const cs = getComputedStyle(document.documentElement);
    return {
      fg: cs.getPropertyValue("--fg").trim(),
      tint: cs.getPropertyValue("--bg-bar").trim(),
      lum: document.documentElement.getAttribute("data-lum"),
      bodyColor: getComputedStyle(document.body).color,
    };
  });
}

async function waitFor(page, want, timeoutMs = 6000) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    const p = await probe(page);
    if (p.lum === want) return { ok: true, waited: Date.now() - t0, ...p };
    await sleep(250);
  }
  return { ok: false, ...(await probe(page)) };
}

const original = getLight();
console.log(`原始 AppsUseLightTheme = ${original}`);
const b = await chromium.launch();
const page = await b.newPage();
const errs = [];
page.on("pageerror", (e) => errs.push(e.message));

try {
  await page.goto(`http://${ADDR}/editor`);
  await page.waitForFunction(() => window.getActiveTab && window.getActiveTab() !== undefined, null, { timeout: 15000 });
  // 切到透明玻璃主题
  // 切到透明玻璃主题 —— 必须走真实菜单, setTheme 没导出到 window
  await page.evaluate(() => {
    const row = document.querySelector('[data-cmd="theme-clear"]');
    if (row) row.click();
  });
  await sleep(1200);
  const themeNow = await page.evaluate(() => document.documentElement.getAttribute("data-theme"));
  console.log("当前主题 =", themeNow, themeNow === "clear" ? "(透明玻璃 ✓)" : "(✗ 未能切到透明主题)");
  await sleep(1500);

  console.log("\n--- 场景 1: 系统深色 (0) ---");
  setLight(0);
  const dark = await waitFor(page, "dark");
  console.log("  data-lum =", dark.lum, " --fg =", dark.fg, " 遮罩 =", dark.tint);
  console.log("  ", dark.ok ? "✓ 已同步" : "✗ 超时未同步");

  console.log("\n--- 场景 2: 切到系统浅色 (1) ---");
  setLight(1);
  const light = await waitFor(page, "light");
  console.log("  data-lum =", light.lum, " --fg =", light.fg, " 遮罩 =", light.tint);
  console.log("  ", light.ok ? "✓ 已动态反色" : "✗ 超时未同步");

  console.log("\n--- 场景 3: 切回深色 (0) ---");
  setLight(0);
  const back = await waitFor(page, "dark");
  console.log("  data-lum =", back.lum, " --fg =", back.fg);
  console.log("  ", back.ok ? "✓ 已动态反色回深色" : "✗ 超时未同步");

  console.log("\n--- 遮罩不透明度实测 ---");
  const alphaOf = (s) => {
    const m = s.match(/([\d.]+)\s*\)/);
    return m ? parseFloat(m[1]) : null;
  };
  console.log("  深色档遮罩 alpha =", alphaOf(dark.tint), "(应 >= 0.45)");
  console.log("  浅色档遮罩 alpha =", alphaOf(light.tint), "(应 >= 0.45)");

  const pass = dark.ok && light.ok && back.ok;
  console.log("\npageerrors:", JSON.stringify(errs));
  console.log(pass ? "\n结果: ✓ 通过" : "\n结果: ✗ 失败");
  process.exitCode = pass && errs.length === 0 ? 0 : 1;
} finally {
  if (original !== null) setLight(original);
  await b.close();
}
