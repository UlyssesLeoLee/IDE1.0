// UI 设计体系验证 —— 真实 Tauri 窗口
//
// 为什么静态检查查不出来: token 体系的问题几乎全是**运行时**的:
//   - 变量定义了但组件没用 (写了 var(--bg-rail), CSS 里还写死 #333)
//   - 变量用了但主题没给值 (fallback 到 :root 的深色, 浅色主题下变深色)
//   - 颜色对了但对比度不达标 (色值本身没配错, 配错了组合)
//   - 反色链路断在某一段 (前端调了后端但命令名不对 / CSS 写死 !important)
//
// 所以这里连真实窗口, 读 getComputedStyle 的实际计算值, 算 WCAG 对比度.
//
// 用法:
//   npx tauri build --config tauri.uat.conf.json
//   ide-shell-desktop.exe &
//   node verify-design-tokens.mjs

import { chromium } from '@playwright/test';
import { execFileSync } from 'child_process';

const CDP = process.env.IDE_DESKTOP_CDP || 'http://127.0.0.1:9222';
const REG = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize';

function readReg() {
  try {
    const out = execFileSync('reg', ['query', REG, '/v', 'AppsUseLightTheme'], { encoding: 'utf8' });
    const m = out.match(/AppsUseLightTheme\s+REG_DWORD\s+0x([0-9a-f]+)/i);
    return m ? parseInt(m[1], 16) : null;
  } catch { return null; }
}
function setReg(v) {
  execFileSync('reg', ['add', REG, '/v', 'AppsUseLightTheme', '/t', 'REG_DWORD', '/d', String(v), '/f'],
    { encoding: 'utf8' });
}

let browser;
try {
  browser = await chromium.connectOverCDP(CDP);
} catch (e) {
  console.error(`无法连接 CDP ${CDP}: ${e.message}`);
  process.exit(2);
}
const ctx = browser.contexts()[0];
let page = ctx.pages().find((p) => !p.url().startsWith('devtools://'));
for (let i = 0; i < 30 && !page; i++) {
  await new Promise((r) => setTimeout(r, 500));
  page = ctx.pages().find((p) => !p.url().startsWith('devtools://'));
}
if (!page) { console.error('CDP 连上但无 webview 页面'); process.exit(2); }

const errs = [];
page.on('pageerror', (e) => errs.push(e.message));

const orig = readReg();
const results = {};

// ---------- WCAG 工具 ----------
const parse = (s) => { const m = (s || '').match(/[\d.]+/g); return m ? m.slice(0, 3).map(Number) : null; };
const lum = ([r, g, b]) => {
  const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};
const ratio = (fg, bg) => {
  const a = lum(fg), b = lum(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
};

const REGIONS = [
  ['菜单栏', '#menubar', '.mb-btn'],
  ['活动栏', '#activity-bar', '.act-btn'],
  ['侧栏', '#side-bar', '.tree-node'],
  ['标签栏', '#tabs', '.tab'],
  ['编辑区', '#ed-content', '.hl-row'],
  ['行号', '#gutter', null],
  ['状态栏', '#status', '.s-item'],
  ['底部面板', '#bottom-panel', '.bottom-tab'],
];

async function readRegions() {
  return page.evaluate((regions) => {
    const out = {};
    for (const [name, hostSel, fgSel] of regions) {
      const host = document.querySelector(hostSel);
      if (!host) { out[name] = { missing: true }; continue; }
      const hc = getComputedStyle(host);
      const fgEl = fgSel ? document.querySelector(fgSel) : null;
      const fc = fgEl ? getComputedStyle(fgEl) : hc;
      out[name] = { bg: hc.backgroundColor, fg: fc.color };
    }
    return out;
  }, REGIONS);
}

// ---------- 1. token 体系完整性 ----------
console.log('========== 1. Design Token 完整性 ==========');
const REQUIRED = [
  '--sp-3', '--sp-4', '--r-sm', '--r-md', '--r-lg',
  '--fs-xs', '--fs-sm', '--fs-md',
  '--ease', '--dur-fast', '--dur-base',
  '--h-menubar', '--h-status', '--w-rail', '--w-side',
  '--bg-bar', '--bg-side', '--bg-edit', '--bg-rail', '--bg-bottom',
  '--bg-chrome', '--bg-raised', '--bg-status',
  '--bg-hover', '--bg-active', '--bg-input',
  '--border', '--border-strong',
  '--fg', '--fg-muted', '--fg-dim', '--fg-on-accent',
  '--blue', '--green', '--accent-text', '--focus-ring',
];
const tokens = await page.evaluate((names) => {
  const cs = getComputedStyle(document.documentElement);
  const out = {};
  for (const n of names) out[n] = cs.getPropertyValue(n).trim();
  return out;
}, REQUIRED);

const missing = REQUIRED.filter((n) => !tokens[n]);
console.log(`  token 总数 ${REQUIRED.length}, 缺失 ${missing.length}`);
if (missing.length) console.log('  缺失:', missing.join(', '));
results['所有设计 token 已定义'] = missing.length === 0;

// ---------- 2. 四套主题的值域检查 ----------
console.log('\n========== 2. 各主题 token 值 ==========');
for (const theme of ['dark', 'light', 'clear']) {
  await page.evaluate((m) => dispatchCommand('theme-' + m), theme);
  await page.waitForTimeout(500);
  const t = await page.evaluate(() => {
    const cs = getComputedStyle(document.documentElement);
    const get = (n) => cs.getPropertyValue(n).trim();
    return {
      fg: get('--fg'), bgEdit: get('--bg-edit'), bgRail: get('--bg-rail'),
      bgHover: get('--bg-hover'), border: get('--border'), accent: get('--accent-text'),
    };
  });
  console.log(`  [${theme}] fg=${t.fg} bg-edit=${t.bgEdit} rail=${t.bgRail} accent=${t.accent}`);
  results[`${theme} 主题有前景色`] = !!t.fg;
  results[`${theme} 主题有活动栏底色`] = !!t.bgRail;
  results[`${theme} 主题有可读强调色`] = !!t.accent;
}

// ---------- 3. 对比度 (不透明主题) ----------
console.log('\n========== 3. WCAG 对比度 ==========');
for (const theme of ['dark', 'light']) {
  await page.evaluate((m) => dispatchCommand('theme-' + m), theme);
  await page.waitForTimeout(500);
  const r = await readRegions();
  console.log(`\n  --- ${theme} ---`);
  let themeFail = 0;
  for (const [name, v] of Object.entries(r)) {
    if (v.missing) { console.log(`    ? ${name}: 元素缺失`); continue; }
    const bg = parse(v.bg), fg = parse(v.fg);
    if (!bg || !fg) { console.log(`    ? ${name}: 颜色无法解析`); continue; }
    const cr = ratio(fg, bg);
    // 行号槽允许次要文字标准 3:1 (它是辅助信息, 字号 12px 但不该抢注意力)
    const min = name === '行号' ? 4.5 : 4.5;
    const ok = cr >= min;
    if (!ok) themeFail++;
    console.log(`    ${ok ? '✓' : '✗'} ${name.padEnd(10)} ${cr.toFixed(2)}:1`);
  }
  results[`${theme} 主题全部区域达 AA (4.5:1)`] = themeFail === 0;
}

// ---------- 4. 透明主题的反色 ----------
console.log('\n========== 4. 透明主题文字反色 ==========');
const readFg = () => page.evaluate(() => {
  const g = (sel) => { const e = document.querySelector(sel); return e ? getComputedStyle(e).color : ''; };
  return {
    fgVar: getComputedStyle(document.documentElement).getPropertyValue('--fg').trim(),
    menubar: g('.mb-btn'), activity: g('.act-btn'), gutter: g('#gutter'),
  };
});

const fgSamples = {};
for (const [label, val] of [['深色桌面', 0], ['浅色桌面', 1]]) {
  setReg(val);
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__ideReady?.(), { timeout: 15000 }).catch(() => {});
  await page.evaluate(() => dispatchCommand('theme-clear'));
  await page.waitForTimeout(2200);
  const s = await readFg();
  fgSamples[label] = s;
  console.log(`  [${label}] --fg=${s.fgVar}  菜单栏=${s.menubar}  活动栏=${s.activity}  行号=${s.gutter}`);
}
const darkFg = fgSamples['深色桌面'], lightFg = fgSamples['浅色桌面'];
const lumOf = (c) => { const p = parse(c); return p ? lum(p) : 0; };

results['反色: 深色桌面用浅色文字'] = lumOf(darkFg.menubar) > 0.5;
results['反色: 浅色桌面用深色文字'] = lumOf(lightFg.menubar) < 0.5;
results['反色: 活动栏跟随反色'] = darkFg.activity !== lightFg.activity;
results['反色: 行号跟随反色'] = darkFg.gutter !== lightFg.gutter;

// ---------- 5. 交互状态完整 ----------
console.log('\n========== 5. 交互状态 ==========');
await page.evaluate(() => dispatchCommand('theme-dark'));
await page.waitForTimeout(400);
// 先打开一个文件 —— 否则没有 .tab, 标签相关的断言全是"缺失"
await page.locator('#welcome .open-btn').click().catch(() => {});
await page.waitForTimeout(600);
await page.locator('.tree-node:has-text("README.md")').click().catch(() => {});
await page.waitForTimeout(400);
const states = await page.evaluate(() => {
  const out = {};
  for (const [name, sel] of [['菜单按钮', '.mb-btn'], ['活动栏按钮', '.act-btn'],
                              ['标签', '.tab'], ['状态栏项', '.s-item'], ['底部标签', '.bottom-tab']]) {
    const el = document.querySelector(sel);
    if (!el) { out[name] = { missing: true }; continue; }
    const cs = getComputedStyle(el);
    out[name] = {
      transition: cs.transitionDuration,
      borderRadius: cs.borderRadius,
      cursor: cs.cursor,
    };
  }
  out['焦点环'] = (() => {
    const cs = getComputedStyle(document.documentElement);
    return cs.getPropertyValue('--focus-ring').trim();
  })();
  out['滚动条'] = getComputedStyle(document.documentElement).getPropertyValue('--sp-3').trim();
  return out;
});
for (const [name, v] of Object.entries(states)) {
  if (v.missing) { console.log(`  ? ${name}: 缺失`); results[`${name}存在`] = false; continue; }
  console.log(`  ${name}: transition=${v.transition || '-'} radius=${v.borderRadius || '-'} cursor=${v.cursor || '-'}`);
  if (v.cursor !== undefined) results[`${name}有 hover 反馈 (transition)`] = v.transition !== '0s';
  if (v.borderRadius !== undefined) results[`${name}圆角统一`] = v.borderRadius !== '0px';
}
results['焦点环已定义'] = !!states['焦点环'];

if (orig !== null) setReg(orig === 1 ? 1 : 0);
results['无运行时错误'] = errs.length === 0;
if (errs.length) console.log('\n运行时错误:', errs);

// ---------- 汇总 ----------
console.log('\n========== 汇总 ==========');
let fail = 0;
for (const [k, v] of Object.entries(results)) {
  if (!v) fail++;
  console.log(`  ${v ? '✓' : '✗'} ${k}`);
}
await browser.close();
console.log(fail === 0
  ? '\n全部通过 — 设计体系完整, 各主题对比度达标, 反色工作正常'
  : `\n${fail} 项失败`);
process.exit(fail === 0 ? 0 : 1);
