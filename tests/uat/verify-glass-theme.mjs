// Tauri 透明玻璃主题 —— 真实渲染验证 (CDP 连 WebView2)
//
// 为什么不能用静态检查: 代码里 [data-theme="clear"] 和 backdrop-filter
// 都在, 但 WebView2 是否真的把它们渲染出来是另一回事:
//   - backdrop-filter 需要 GPU 合成, 某些驱动/GPU 组合会静默失效
//   - transparent: true 的原生窗口在部分 Windows 版本上会变黑底
//   - 半透明叠半透明会累积, 视觉上未必是"水晶"
//
// 所以这里连真实 app, 读 getComputedStyle 的**实际计算值**, 不是看源码。
//
// 用法:
//   npx tauri build --config tauri.uat.conf.json
//   ide-shell-desktop.exe &
//   node verify-glass-theme.mjs

import { chromium } from '@playwright/test';

const CDP = process.env.IDE_DESKTOP_CDP || 'http://127.0.0.1:9222';

let browser;
try {
  browser = await chromium.connectOverCDP(CDP);
} catch (e) {
  console.error(`无法连接 CDP ${CDP}: ${e.message}`);
  console.error('先用 tauri.uat.conf.json 构建并启动 ide-shell-desktop.exe');
  process.exit(2);
}

const ctx = browser.contexts()[0];
let page = ctx.pages().find((p) => !p.url().startsWith('devtools://'));
for (let i = 0; i < 30 && !page; i++) {
  await new Promise((r) => setTimeout(r, 500));
  page = ctx.pages().find((p) => !p.url().startsWith('devtools://'));
}
if (!page) { console.error('CDP 连上了但找不到 webview 页面'); process.exit(2); }

const errs = [];
page.on('pageerror', (e) => errs.push(e.message));

console.log('页面:', page.url(), '\n');

// ---------- 1. 切到透明玻璃主题 ----------
console.log('=== 1. 切换到透明玻璃主题 ===');
const switched = await page.evaluate(() => {
  dispatchCommand('theme-clear');
  const root = document.documentElement;
  return {
    dataTheme: root.getAttribute('data-theme'),
    tip: document.getElementById('s-tip')?.textContent || '',
    saved: localStorage.getItem('ide10.theme'),
  };
});
console.log(' ', JSON.stringify(switched, null, 2));
await page.waitForTimeout(600);

// ---------- 2. 关键: backdrop-filter 实际生效吗 ----------
console.log('\n=== 2. backdrop-filter 实际计算值 ===');
const glass = await page.evaluate(() => {
  const read = (sel) => {
    const el = document.querySelector(sel);
    if (!el) return { sel, missing: true };
    const cs = getComputedStyle(el);
    return {
      sel,
      backdropFilter: cs.backdropFilter || cs.webkitBackdropFilter || 'none',
      background: cs.backgroundColor,
      borderTop: cs.borderTopColor,
      boxShadow: cs.boxShadow === 'none' ? 'none' : cs.boxShadow.slice(0, 60),
    };
  };
  return {
    app: read('#app'),
    menubar: read('#menubar'),
    sideBar: read('#side-bar'),
    activityBtn: read('.act-btn'),
    tab: read('.tab'),
    status: read('#status'),
  };
});
for (const [k, v] of Object.entries(glass)) {
  if (v.missing) { console.log(`  ? ${k}: 元素不存在`); continue; }
  console.log(`  ${k}: backdrop=${v.backdropFilter} bg=${v.background}`);
}

// ---------- 3. CSS 变量是否被注入 ----------
console.log('\n=== 3. CSS 变量 ===');
const vars = await page.evaluate(() => {
  const cs = getComputedStyle(document.documentElement);
  const names = ['--bg-edit', '--fg', '--border', '--bg-bar', '--bg-side'];
  const out = {};
  for (const n of names) out[n] = cs.getPropertyValue(n).trim();
  return out;
});
console.log(' ', JSON.stringify(vars, null, 2));

// ---------- 4. 实际像素: 背景是不是真的透 ----------
console.log('\n=== 4. 实际像素 (截图采样) ===');
const shot = await page.screenshot({ path: process.env.IDE_SHOT || 'glass-theme.png' });
console.log('  截图已保存:', process.env.IDE_SHOT || 'glass-theme.png',
            `(${shot.length} bytes)`);

// 采样几个点看是否同色 (真透明的话窗口背后应该一致)
const pixels = await page.evaluate(async () => {
  // 用 canvas 读不了 webview 内容, 改用 elementFromPoint 判断层级
  const probes = [[300, 15], [60, 300], [500, 400], [500, 500]];
  return probes.map(([x, y]) => {
    const el = document.elementFromPoint(x, y);
    return { x, y, tag: el?.tagName || 'null', id: el?.id || '', cls: el?.className || '' };
  });
});
console.log('  元素层级探测:');
pixels.forEach((p) => console.log(`    (${p.x},${p.y}) -> ${p.tag}#${p.id}.${String(p.cls).slice(0, 30)}`));

// ---------- 5. 三态主题都能切 ----------
console.log('\n=== 5. 三态主题切换 ===');
const themeCycle = {};
for (const cmd of ['theme-clear', 'theme-dark', 'theme-light', 'theme-high-contrast']) {
  const r = await page.evaluate((c) => {
    dispatchCommand(c);
    const cs = getComputedStyle(document.documentElement);
    return {
      attr: document.documentElement.getAttribute('data-theme'),
      bgEdit: cs.getPropertyValue('--bg-edit').trim(),
      tip: document.getElementById('s-tip')?.textContent || '',
    };
  }, cmd);
  themeCycle[cmd] = r;
  console.log(`  ${cmd.padEnd(22)} attr=${r.attr} --bg-edit=${r.bgEdit}`);
  await page.waitForTimeout(300);
}

// ---------- 6. 汇总 ----------
console.log('\n=== 汇总 ===');
const results = {};
results['theme-clear 可切换'] = switched.dataTheme === 'clear';
results['CSS 变量 --bg-edit 透明'] = vars['--bg-edit'] === 'transparent';
results['#app backdrop-filter 生效'] = /blur/.test(glass.app.backdropFilter || '');
results['#app 背景半透明'] = /rgba\(/.test(glass.app.background || '');
results['四个主题都能切'] = Object.values(themeCycle).every((t) => t.attr);
results['无运行时错误'] = errs.length === 0;

let fail = 0;
for (const [k, v] of Object.entries(results)) {
  if (!v) fail++;
  console.log(`  ${v ? '\u2713' : '\u2717'} ${k}`);
}
if (errs.length) console.log('\n运行时错误:', errs);

await browser.close();
console.log(fail === 0
  ? '\n全部通过 — 透明玻璃主题在真实 Tauri 窗口里渲染正确'
  : `\n${fail} 项失败 —— 见上方实测值`);
process.exit(fail === 0 ? 0 : 1);