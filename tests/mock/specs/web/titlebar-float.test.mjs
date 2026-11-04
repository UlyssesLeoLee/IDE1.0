// 回归: 桌面版原生标题栏跟随主题 (事件驱动); 浮动查找框停在底部时让到终端栏 / 状态栏上方, 不挡「展开终端」
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const darkCalls = (m) => m.tauri.ipc().then((l) => l.filter((x) => x.name === 'set_window_theme').map((x) => x.args.dark));

test('主题切换 -> 通知 Rust 设窗口明暗 (浅色 = 亮标题栏, 深色 / 高对比 / 复古 = 暗); 透明主题按桌面明暗', () =>
  withMocks({ name: 'tb', portOffset: 590, mock: { tauri: true } }, async ({ page, m }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => setTheme('dark'));
    await page.evaluate(() => setTheme('light'));
    await page.waitForFunction(() => document.documentElement.getAttribute('data-theme') === 'light');
    await page.waitForTimeout(200);
    let calls = await darkCalls(m);
    assert.equal(calls[calls.length - 1], false, '浅色主题 -> 亮标题栏');
    await page.evaluate(() => setTheme('dark'));
    await page.waitForTimeout(200);
    calls = await darkCalls(m);
    assert.equal(calls[calls.length - 1], true, '深色主题 -> 暗标题栏');
    await page.evaluate(() => { document.documentElement.setAttribute('data-theme', 'clear'); document.documentElement.setAttribute('data-lum', 'light'); });
    await page.waitForTimeout(200);
    calls = await darkCalls(m);
    assert.equal(calls[calls.length - 1], false, '透明主题 + 浅色桌面 -> 亮标题栏');
    // 同样的明暗不重复通知
    const n = (await darkCalls(m)).length;
    await page.evaluate(() => document.documentElement.setAttribute('data-lum', 'light'));
    await page.waitForTimeout(150);
    assert.equal((await darkCalls(m)).length, n);
  }));

test('浮动查找框让到底部时停在终端栏上方 (不盖住底部面板头)', () =>
  withMocks({ name: 'tb', portOffset: 591 }, async ({ page, press }) => {
    await page.setViewportSize({ width: 620, height: 720 });   // 窄窗口: 浮动框一定会盖住第 2 行的命中
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      const t = getActiveTab(); t.lines = Array.from({ length: 40 }, (_, i) => 'line ' + i + (i === 6 ? ' NEEDLE' : '')); t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t);
    });
    await press('Control+f');
    await page.waitForSelector('#modal.float.show');
    await page.fill('#find-input', 'NEEDLE');   // 命中在第 7 行, 正好在顶部浮动框下面 -> 让到底部
    await page.waitForFunction(() => document.querySelector('#modal .md-box').dataset.pos === 'bottom', null, { timeout: 8000 });
    const r = await page.evaluate(() => {
      const b = document.querySelector('#modal .md-box').getBoundingClientRect();
      const p = document.getElementById('bottom-panel'), pr = p.getBoundingClientRect();
      const s = document.getElementById('status').getBoundingClientRect();
      return { boxBottom: b.bottom, panelTop: pr.height > 0 ? pr.top : 1e9, statusTop: s.height > 0 ? s.top : 1e9 };
    });
    assert.ok(r.boxBottom <= Math.min(r.panelTop, r.statusTop) + 0.5, `查找框下沿 ${r.boxBottom} 应在底部面板 / 状态栏 (${Math.min(r.panelTop, r.statusTop)}) 之上`);
  }));
