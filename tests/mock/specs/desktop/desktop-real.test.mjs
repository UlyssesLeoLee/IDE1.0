// 回归 (桌面真机, 需 QA_DESKTOP): 真实 Tauri / WebView2 窗口
//   - Grep 在桌面版正常 (不再「搜索失败: undefined」)
//   - Alt 纵向选择 / 点击 / 拖选 在真实窗口里的行为
//   - 透明玻璃: 背后真实颜色每秒采样, 文字随之反色 (后面垫白 / 黑闪烁的窗口)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { launchDesktop, startBackdrop, hasDesktop } from '../../lib/desktop.mjs';
import { grid } from '../../lib/harness.mjs';

const opts = { skip: hasDesktop ? false : '未设置 QA_DESKTOP (桌面真机用例已跳过)' };

test('桌面版 Grep: 命中正常显示, 没有 undefined', opts, async () => {
  const d = await launchDesktop({ cdpPort: 9444 });
  try {
    const { page } = d;
    await page.locator('#ed-content').click();
    await page.keyboard.press('Control+g');
    await page.fill('#grep-input', 'hello');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(2500);
    const text = await page.evaluate(() => (document.getElementById('grep-status') || {}).innerText + '\n' + ((document.getElementById('grep-results') || {}).innerText || ''));
    assert.match(text, /命中/);
    assert.doesNotMatch(text, /undefined/);
    assert.deepEqual(d.errors, []);
  } finally { await d.close(); }
});

test('桌面版: Alt 拖出 3行×4列, 输入覆盖矩形内容', opts, async () => {
  const d = await launchDesktop({ cdpPort: 9445 });
  try {
    const { page } = d;
    await page.evaluate(() => { try { localStorage.setItem('ide10.vim', 'off'); } catch (_) {} });
    await page.locator('#ed-content').click();
    for (const l of ['abcdefghij', 'klmnopqrst', 'uvwxyzABCD', 'EFGHIJKLMN']) { await page.keyboard.type(l); await page.keyboard.press('Enter'); }
    const g = await grid(page);
    await page.keyboard.down('Alt');
    await page.mouse.move(g.x(2), g.y(0)); await page.mouse.down(); await page.mouse.move(g.x(4), g.y(1), { steps: 4 }); await page.mouse.move(g.x(6), g.y(2), { steps: 4 });
    await page.mouse.up(); await page.keyboard.up('Alt');
    const box = await page.evaluate(() => { const c = getActiveTab().colSel; return [c.r0, c.c0, c.r1, c.c1]; });
    assert.deepEqual(box, [0, 2, 2, 6]);
    await page.keyboard.type('X');
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines.slice(0, 3)), ['abXghij', 'klXqrst', 'uvXABCD']);
  } finally { await d.close(); }
});

test('桌面版透明玻璃: 背后白/黑切换, 文字颜色跟着反色 (≤ 1 个轮询周期)', opts, async () => {
  const bg = startBackdrop('flip');
  await new Promise((r) => setTimeout(r, 2500));
  const d = await launchDesktop({ cdpPort: 9446 });
  try {
    const { page } = d;
    await page.locator('#menubar .menu-item[data-menu="view"] > .mb-btn').click();
    await page.locator('#menubar [data-submenu="theme"]').hover();
    await page.locator('#menubar .menu-row[data-cmd="theme-clear"]').click();
    const seen = new Set();
    for (let i = 0; i < 14; i++) {
      await page.waitForTimeout(1000);
      seen.add(await page.evaluate(() => document.documentElement.getAttribute('data-lum') + '|' + getComputedStyle(document.getElementById('ed-content')).color));
    }
    const lums = new Set([...seen].map((s) => s.split('|')[0]));
    assert.ok(lums.has('light') && lums.has('dark'), '背后白/黑切换时 data-lum 两种都出现过: ' + [...seen].join(' ; '));
    assert.ok(seen.has('light|rgb(0, 0, 0)') && seen.has('dark|rgb(255, 255, 255)'), '浅背景黑字 / 深背景白字');
  } finally { await d.close(); bg.stop(); }
});
