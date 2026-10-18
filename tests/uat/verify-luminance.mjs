// 透明玻璃主题 — 文字反色实测
//
// 验证要点 (为什么不能用静态检查):
//   "代码里有 setTheme → invoke → clear_theme_vars" 不等于 "文字真会反色".
//   反色链路有三个断点, 静态都看不出来:
//     1. invoke 的命令名对不上 (前端调 A, 后端注册 B)
//     2. CSS 里写死了 #ffffff !important, 把 JS 设的变量压过去
//     3. 变量设上了, 但选择器不匹配 (设在 root, 规则却在 body 上)
//
// 所以这里连真实 Tauri 窗口, 改注册表模拟浅色系统主题, 然后读
// getComputedStyle 的**实际文字颜色**, 并算 WCAG 对比度。
//
// 用法:
//   npx tauri build --config tauri.uat.conf.json
//   ide-shell-desktop.exe &
//   node verify-luminance.mjs

// [布局] 本脚本需要完整 IDE 界面 + vim 三态 (editor.html 缺省已改为极简 Sakura 风格 + 非模态): 连上页面后自动写 ide10.layout=full / ide10.vim=on 并重载, 见 needFull.
import { chromium } from '@playwright/test';
import { execFileSync } from 'child_process';

const CDP = process.env.IDE_DESKTOP_CDP || 'http://127.0.0.1:9222';
const REG_PATH = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize';

// 读当前值, 结束时还原
function readReg() {
  try {
    return execFileSync('reg', ['query', REG_PATH, '/v', 'AppsUseLightTheme'],
      { encoding: 'utf8' });
  } catch { return null; }
}
function setReg(light) {
  execFileSync('reg', ['add', REG_PATH, '/v', 'AppsUseLightTheme', '/t', 'REG_DWORD',
    '/d', light ? '1' : '0', '/f'], { encoding: 'utf8' });
}
function saveOriginal() {
  const out = readReg();
  if (!out) return null;
  const m = out.match(/AppsUseLightTheme\s+REG_DWORD\s+0x([0-9a-f]+)/i);
  return m ? parseInt(m[1], 16) : null;
}

const original = saveOriginal();
console.log('原始 AppsUseLightTheme =', original);
console.log('CDP =', CDP);

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
// 缺省界面是极简 (Sakura 风格) + 非模态编辑; 本脚本按完整 IDE 界面 + vim 三态写 (要欢迎页 / 标签条 / 状态栏模式标签).
// 开关存在被测 app 的 localStorage (ide10.layout / ide10.vim): 不是 full + vim 就写进去并重载一次; 旧版 app (没有 setLayout) 本来就是完整界面, 跳过.
// 跑完想还原成极简: await page.evaluate(() => { setLayout('min'); localStorage.removeItem('ide10.vim'); })
const needFull = await page.evaluate(() => {
  try {
    if (!window.setLayout) return false;
    if (window.getLayout().preset === 'full' && localStorage.getItem('ide10.vim') === 'on') return false;
    localStorage.setItem('ide10.layout', 'full'); localStorage.setItem('ide10.vim', 'on');
    return true;
  } catch (_) { return false; }
});
if (needFull) {
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__ideReady && window.__ideReady(), null, { timeout: 15000 }).catch(() => {});
}


// ---------- 辅助: 读关键元素的实际文字色 + 算对比度 ----------
const readColors = () => page.evaluate(() => {
  const g = (sel) => {
    const el = document.querySelector(sel);
    if (!el) return { sel, missing: true };
    const cs = getComputedStyle(el);
    return {
      sel,
      color: cs.color,
      background: cs.backgroundColor,
      borderTop: cs.borderTopColor,
    };
  };
  const root = getComputedStyle(document.documentElement);
  return {
    theme: document.documentElement.getAttribute('data-theme'),
    fgVar: root.getPropertyValue('--fg').trim(),
    mutedVar: root.getPropertyValue('--fg-muted').trim(),
    glassVar: root.getPropertyValue('--glass-tint').trim(),
    luminance: root.getPropertyValue('--x-luminance').trim(),
    body: g('body'),
    menubar: g('#menubar'),
    menuBtn: g('.mb-btn'),
    actBtn: g('.act-btn'),
    status: g('#status'),
  };
});

// WCAG 相对亮度 + 对比度
function parseRgb(str) {
  const m = str.match(/[\d.]+/g);
  return m ? m.slice(0, 3).map(Number) : null;
}
function lum([r, g, b]) {
  const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
function contrast(fg, bg) {
  const a = lum(fg), b = lum(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

const results = {};

// ================= 场景 1: 系统深色主题 =================
console.log('\n========== 场景 1: 系统深色 (AppsUseLightTheme=0) ==========');
setReg(false);
await page.reload({ waitUntil: 'domcontentloaded' });
await page.waitForFunction(() => window.__ideReady && window.__ideReady(), { timeout: 15000 })
  .catch(() => {});
await page.evaluate(() => dispatchCommand('theme-clear'));
await page.waitForTimeout(2000);
const dark = await readColors();
console.log('  --fg =', dark.fgVar, '| --fg-muted =', dark.mutedVar, '| --glass-tint =', dark.glassVar);
console.log('  body color =', dark.body.color, '| #menubar color =', dark.menubar.color);
console.log('  .mb-btn color =', dark.menuBtn.color);

// 深色系统主题下: 文字必须偏亮 (L > 0.5)
const darkFg = parseRgb(dark.body.color);
const darkL = lum(darkFg);
console.log('  body 文字相对亮度 =', darkL.toFixed(3), darkL > 0.5 ? '(亮 ✓)' : '(暗 ✗ 应该亮)');
results['深色系统 → 浅色文字'] = darkL > 0.5;
results['深色系统: --fg 不是硬编码 #ffffff 之外的坏值'] = /^#f|^rgb\(2[0-9][0-9]/.test(dark.fgVar);

// 对比度: 深色系统主题下, 对最坏背景 (纯黑) 仍要 >= 4.5
const darkContrast = contrast(darkFg, [0, 0, 0]);
console.log('  对纯黑背景对比度 =', darkContrast.toFixed(2), darkContrast >= 4.5 ? '✓ AA' : '✗ 不达标');
results['深色系统: 对最坏背景达 WCAG AA'] = darkContrast >= 4.5;

// ================= 场景 2: 系统浅色主题 =================
console.log('\n========== 场景 2: 系统浅色 (AppsUseLightTheme=1) ==========');
setReg(true);
// 轮询 watch 是 1s, 多等几拍
await page.waitForTimeout(3000);
const light = await readColors();
console.log('  --fg =', light.fgVar, '| --fg-muted =', light.mutedVar, '| --glass-tint =', light.glassVar);
console.log('  body color =', light.body.color, '| #menubar color =', light.menubar.color);
console.log('  .mb-btn color =', light.menuBtn.color);

const lightFg = parseRgb(light.body.color);
const lightL = lum(lightFg);
console.log('  body 文字相对亮度 =', lightL.toFixed(3), lightL < 0.5 ? '(暗 ✓)' : '(亮 ✗ 应该暗)');
results['浅色系统 → 深色文字'] = lightL < 0.5;
results['浅色系统: 文字与深色系统不同 (真的反色了)'] = dark.body.color !== light.body.color;

const lightContrast = contrast(lightFg, [255, 255, 255]);
console.log('  对纯白背景对比度 =', lightContrast.toFixed(2), lightContrast >= 4.5 ? '✓ AA' : '✗ 不达标');
results['浅色系统: 对最坏背景达 WCAG AA'] = lightContrast >= 4.5;

// ================= 场景 3: 动态跟随 =================
console.log('\n========== 场景 3: 动态跟随 (不重载页面, 切回深色) ==========');
setReg(false);
await page.waitForTimeout(3000);   // 等 1s 轮询命中
const back = await readColors();
console.log('  切回后 body color =', back.body.color);
results['动态跟随: 切回深色后文字变亮'] = back.body.color === dark.body.color;

// ================= 汇总 =================
console.log('\n========== 汇总 ==========');
let fail = 0;
for (const [k, v] of Object.entries(results)) {
  if (!v) fail++;
  console.log(`  ${v ? '✓' : '✗'} ${k}`);
}
if (errs.length) { console.log('\n运行时错误:'); errs.forEach(e => console.log('  ', e)); }
results['无运行时错误'] = errs.length === 0;

// 还原注册表
if (original !== null) { setReg(original === 1); console.log('\n已还原 AppsUseLightTheme =', original); }
else console.log('\n读不到原值, 已设回深色 (0)');

await browser.close();
console.log(fail === 0
  ? '\n全部通过 — 透明主题文字随桌面明暗动态反色'
  : `\n${fail} 项失败`);
process.exit(fail === 0 ? 0 : 1);
