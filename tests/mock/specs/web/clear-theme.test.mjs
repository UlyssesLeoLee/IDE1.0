// 回归: 透明玻璃主题 —— 子菜单不被父菜单盖住 / 裁掉; 闪光不被窗口边缘截断; 动态反色不再轮询
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('透明主题: 菜单与子菜单都没有 backdrop-filter, 子菜单完整显示在窗口内且在最上层', () =>
  withMocks({ name: 'ct', portOffset: 544 }, async ({ page }) => {
    await page.evaluate(() => setTheme('clear'));
    await page.waitForTimeout(300);
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('app')).backdropFilter), 'none', '#app 不放 backdrop-filter (会成为 fixed 的包含块并拖慢整窗)');
    await page.locator('.menu-item[data-menu="edit"] > .mb-btn').click();
    const sub = page.locator('.menu-item[data-menu="edit"] .menu-row.menu-submenu').first();
    await sub.hover();
    await page.waitForTimeout(250);
    const r = await page.evaluate(() => {
      const row = document.querySelector('.menu-item[data-menu="edit"] .menu-row.menu-submenu:hover');
      const dd = row && row.querySelector(':scope > .submenu-dropdown');
      if (!dd) return null;
      const b = dd.getBoundingClientRect();
      const cx = b.left + Math.min(40, b.width / 2), cy = b.top + Math.min(14, b.height / 2);
      const top = document.elementFromPoint(cx, cy);
      return { w: b.width, h: b.height, inView: b.left >= 0 && b.top >= 0 && b.right <= innerWidth && b.bottom <= innerHeight, onTop: !!(top && dd.contains(top)), bf: getComputedStyle(dd).backdropFilter, parentBf: getComputedStyle(dd.parentElement).backdropFilter };
    });
    assert.ok(r, '子菜单展开了');
    assert.ok(r.w > 40 && r.h > 20, '子菜单有尺寸');
    assert.equal(r.inView, true, '子菜单整个在窗口内');
    assert.equal(r.onTop, true, '子菜单顶部那一点命中的是子菜单本身 (没被父菜单盖住)');
    assert.equal(r.bf, 'none');
    assert.equal(r.parentBf, 'none', '父菜单也没有 backdrop-filter, 不成为 fixed 子菜单的包含块');
  }));

test('闪光: 候选位置再靠边也会被夹到窗口内一个闪光半径以外, 图层不裁剪', () =>
  withMocks({ name: 'ct', portOffset: 545 }, async ({ page }) => {
    await page.evaluate(() => setTheme('clear'));
    const out = await page.evaluate(() => {
      const layer = document.getElementById('crystal-sparkle-layer');
      const orig = getCrystalSparkleCandidateCorners;
      window.getCrystalSparkleCandidateCorners = () => [{ x: 0, y: 0 }, { x: innerWidth, y: 0 }, { x: 0, y: innerHeight }, { x: innerWidth, y: innerHeight }];
      layer.innerHTML = '';
      for (let i = 0; i < 40; i++) spawnCrystalSparkle();
      const els = [...layer.querySelectorAll('.crystal-sparkle')];
      const pts = els.map((e) => ({ x: parseFloat(e.style.left), y: parseFloat(e.style.top), s: parseFloat(e.style.getPropertyValue('--sp-scale')) }));
      return { n: els.length, overflow: getComputedStyle(layer).overflow, minX: Math.min(...pts.map((p) => p.x - 24 * p.s)), minY: Math.min(...pts.map((p) => p.y - 24 * p.s)), maxX: Math.max(...pts.map((p) => p.x + 24 * p.s)), maxY: Math.max(...pts.map((p) => p.y + 24 * p.s)), w: innerWidth, h: innerHeight };
    });
    assert.ok(out.n > 0);
    assert.equal(out.overflow, 'visible');
    assert.ok(out.minX >= 0 && out.minY >= 0 && out.maxX <= out.w && out.maxY <= out.h, JSON.stringify(out));
  }));

test('动态反色不轮询: 页面里没有 desktop_luminance 的周期定时器, 只在事件后采样 (限流)', () =>
  withMocks({ name: 'ct', portOffset: 546 }, async ({ page }) => {
    const calls = [];
    await page.exposeFunction('__lumCall', () => calls.push(Date.now()));
    await page.evaluate(() => {
      const orig = window.invoke;
      window.__origInvoke = orig;
    });
    const src = await page.evaluate(() => startLuminanceWatch.toString() + requestLuminanceCheck.toString());
    assert.doesNotMatch(src, /setInterval/, '没有周期轮询');
    assert.match(src, /focus/);
    assert.match(src, /prefers-color-scheme/);
  }));
