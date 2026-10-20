// 回归: 左下角归「最近展开」的抽屉 — 后展开的占整列 / 整行, 先展开的自动重新排版去让位; 任何情况下互不重叠
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withApp } from '../../lib/harness.mjs';

const rects = (page) => page.evaluate(() => {
  const r = (s) => { const e = document.querySelector(s); if (!e) return null; if (getComputedStyle(e).display === 'none') return 'hidden'; const b = e.getBoundingClientRect(); return [Math.round(b.left), Math.round(b.top), Math.round(b.right), Math.round(b.bottom)]; };
  return { vp: [innerWidth, innerHeight], strip: r('#left-strip'), rail: r('#activity-bar'), side: r('#side-bar'), editor: r('#editor-area'), bottom: r('#bottom-panel'), status: r('#status'), top: document.documentElement.getAttribute('data-drawer-top') };
});
const overlap = (a, b) => Array.isArray(a) && Array.isArray(b) && a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
const noOverlap = (g) => {
  for (const [x, y] of [['side', 'bottom'], ['rail', 'bottom'], ['editor', 'bottom'], ['editor', 'side'], ['strip', 'bottom'], ['bottom', 'status'], ['side', 'status']]) {
    assert.equal(overlap(g[x], g[y]), false, `${x} 与 ${y} 重叠: ${JSON.stringify(g[x])} / ${JSON.stringify(g[y])}`);
  }
};

test('min: 先开导航再开终端 → 终端后开占整行, 导航被压矮 (不重叠)', () =>
  withApp({ name: 'dl', portOffset: 70 }, async ({ page }) => {
    await page.locator('#left-strip-btn').click(); await page.waitForTimeout(250);
    await page.keyboard.press('Control+`'); await page.waitForTimeout(500);
    const g = await rects(page);
    assert.equal(g.top, 'bottom');
    assert.equal(g.bottom[0], 0); assert.equal(g.bottom[2], g.vp[0]);            // 终端横贯整行
    assert.ok(g.side[3] <= g.bottom[1] + 1, '导航让位, 底边不超过终端顶边');
    noOverlap(g);
  }));

test('min: 先开终端再开导航 → 导航后开撑满整列, 终端缩到编辑区之下 (不重叠)', () =>
  withApp({ name: 'dl', portOffset: 71 }, async ({ page }) => {
    await page.keyboard.press('Control+`'); await page.waitForTimeout(500);
    await page.locator('#left-strip-btn').click(); await page.waitForTimeout(400);
    const g = await rects(page);
    assert.equal(g.top, 'left');
    assert.ok(g.side[3] >= g.status[1] - 1, '导航一直延伸到状态栏');
    assert.ok(g.bottom[0] >= g.side[2] - 1, '终端从导航右侧开始 (让位)');
    noOverlap(g);
  }));

test('min: 只剩一个展开时它就是占角者; 折叠后另一个恢复整行', () =>
  withApp({ name: 'dl', portOffset: 72 }, async ({ page }) => {
    await page.keyboard.press('Control+`'); await page.waitForTimeout(400);
    await page.locator('#left-strip-btn').click(); await page.waitForTimeout(300);
    await page.locator('#left-strip-btn').click(); await page.waitForTimeout(300);   // 折叠导航
    const g = await rects(page);
    assert.equal(g.top, 'bottom');
    assert.equal(g.bottom[0], 0); assert.equal(g.bottom[2], g.vp[0]);
  }));

test('full: 终端后开 / 导航后开 两种顺序都重排且不重叠', () =>
  withApp({ name: 'dl', portOffset: 73, layout: 'full' }, async ({ page }) => {
    await page.keyboard.press('Control+`'); await page.waitForTimeout(500);
    let g = await rects(page);
    assert.equal(g.top, 'bottom'); noOverlap(g);
    // 把侧栏收起再展开 → 导航成为最近展开的
    await page.evaluate(() => { document.querySelector('.act-btn.active') && document.querySelector('.act-btn.active').click(); });
    await page.waitForTimeout(250);
    await page.evaluate(() => { document.querySelector('.act-btn[data-pane="explorer"]').click(); });
    await page.waitForTimeout(300);
    g = await rects(page); noOverlap(g);
  }));
