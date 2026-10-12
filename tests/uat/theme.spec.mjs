// 透明玻璃主题 (clear) UAT — 迁移自 main 分支 d7ad19c/4b7c6d5/f44479f.
// 验证 4 个主题都真能切换, 且 clear 主题的半透明+backdrop-filter 实际生效
// (不是只切了个 data-theme 属性).
import { chromium } from '@playwright/test';

const BASE = process.env.IDE_WEB_URL || 'http://127.0.0.1:8123';
const browser = await chromium.launch();
const page = await browser.newPage();
const errs = [];
page.on('pageerror', (e) => errs.push(e.message));

await page.goto(BASE + '/editor', { waitUntil: 'domcontentloaded' });
await page.waitForFunction(
  () => {
    const b = document.getElementById('banner');
    return b && /ide-kernel-core|ide-shell-web/.test(b.textContent || '');
  },
  { timeout: 15000 },
).catch(() => console.log('(banner 未就绪, 继续 — 断言会暴露问题)'));

const results = {};

// --- 1. 四个主题命令都路由到 setTheme ---
console.log('=== 1. 主题命令存在 ===');
results['theme-clear 命令'] = await page.evaluate(() => {
  try { dispatchCommand('theme-clear'); return true; } catch (e) { return e.message; }
});
results['data-theme = clear'] =
  (await page.evaluate(() => document.documentElement.getAttribute('data-theme'))) === 'clear';

console.log('\n=== 2. clear 主题 CSS 实际生效 ===');
const clearStyle = await page.evaluate(() => {
  const cs = (sel, prop) => {
    const el = document.querySelector(sel);
    return el ? getComputedStyle(el).getPropertyValue(prop) : '(元素不存在)';
  };
  const root = getComputedStyle(document.documentElement);
  return {
    rootFg: root.getPropertyValue('--fg').trim(),
    rootBgEdit: root.getPropertyValue('--bg-edit').trim(),
    appBackdrop: cs('#app', 'backdrop-filter'),
    appBg: cs('#app', 'background-color'),
    menubarBackdrop: cs('#menubar', 'backdrop-filter'),
    bodyBg: cs('body', 'background-color'),
  };
});
console.log(' ', JSON.stringify(clearStyle, null, 2));
results['CSS 变量 --fg 为白'] = clearStyle.rootFg === '#ffffff';
results['CSS 变量 --bg-edit 透明'] = clearStyle.rootBgEdit === 'transparent';
results['#app backdrop-filter 生效'] = /blur/.test(clearStyle.appBackdrop);
results['#app 背景半透明'] = /rgba\(/.test(clearStyle.appBg) || clearStyle.appBg === 'transparent';

console.log('\n=== 3. 切走再切回, 确认无 inline style 残留 ===');
await page.evaluate(() => dispatchCommand('theme-dark'));
const darkState = await page.evaluate(() => ({
  attr: document.documentElement.getAttribute('data-theme'),
  inlineBorder: document.documentElement.style.getPropertyValue('--border'),
  bgEdit: document.documentElement.style.getPropertyValue('--bg-edit'),
}));
console.log('  dark:', JSON.stringify(darkState));
results['dark 主题注入变量'] = darkState.bgEdit === '#1e1e1e';

await page.evaluate(() => dispatchCommand('theme-clear'));
const backState = await page.evaluate(() => {
  const root = document.documentElement;
  const cs = getComputedStyle(root);
  return {
    attr: root.getAttribute('data-theme'),
    // 关键: 没有 inline 变量压过 CSS 选择器
    inlineBgEdit: root.style.getPropertyValue('--bg-edit') || '(已清空)',
    computedBgEdit: cs.getPropertyValue('--bg-edit').trim(),
    computedFg: cs.getPropertyValue('--fg').trim(),
    appBackdrop: getComputedStyle(document.getElementById('app')).backdropFilter,
  };
});
console.log('  切回 clear:', JSON.stringify(backState, null, 2));
results['切回 clear 无 inline 污染'] = backState.inlineBgEdit === '(已清空)';
results['切回 clear 仍半透明'] = backState.computedBgEdit === 'transparent';

console.log('\n=== 4. 其余主题仍正常 ===');
for (const [cmd, label, expect] of [
  ['theme-light', '浅色', '#ffffff'],
  ['theme-high-contrast', '高对比度', '#000000'],
  ['theme-dark', '深色', '#1e1e1e'],
]) {
  const v = await page.evaluate((c) => {
    dispatchCommand(c);
    return getComputedStyle(document.documentElement).getPropertyValue('--bg-edit').trim();
  }, cmd);
  console.log(`  ${label}: --bg-edit = ${v}`);
  results[`${label}主题正常`] = v === expect;
}

console.log('\n=== 5. 持久化 ===');
await page.evaluate(() => dispatchCommand('theme-clear'));
const saved = await page.evaluate(() => localStorage.getItem('ide10.theme'));
results['localStorage 记住 clear'] = saved === 'clear';
console.log('  localStorage["ide10.theme"] =', saved);

console.log('\n=== 6. 状态栏提示 ===');
await page.evaluate(() => dispatchCommand('theme-clear'));
const tip = await page.evaluate(() => document.getElementById('s-tip').textContent);
results['状态栏显示中文主题名'] = /透明玻璃/.test(tip || '');
console.log('  s-tip =', JSON.stringify(tip));

console.log('\n=== 7. 运行时错误 ===');
console.log(errs.length ? errs : 'none');
results['无运行时错误'] = errs.length === 0;

console.log('\n=== 汇总 ===');
let fail = 0;
for (const [k, v] of Object.entries(results)) {
  const ok = v === true;
  if (!ok) fail++;
  console.log(`  ${ok ? '\u2713' : '\u2717'} ${k}${ok ? '' : '  -> ' + JSON.stringify(v)}`);
}
await browser.close();
console.log(fail === 0 ? '\n全部通过' : `\n${fail} 项失败`);
process.exit(fail === 0 ? 0 : 1);