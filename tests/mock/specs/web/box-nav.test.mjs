// 回归: 矩形选择的大步移动 — Ctrl+Alt+←→ 单词 / Ctrl+Alt+Home·End 文件首尾 / Alt+PgUp·PgDn 整页 / Ctrl+Alt+PgUp·PgDn 半页
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = (page, row, col) => page.evaluate(([r, c]) => {
  const t = getActiveTab(); t.lines = Array.from({ length: 120 }, (_, i) => 'word' + i + ' alpha beta'); t.cursor = { row: r, col: c }; t.sel = null; t.colSel = null; renderVim(t);
}, [row, col]);
const box = (page) => page.evaluate(() => { const t = getActiveTab(); const c = t.colSel; return c ? [c.r0, c.c0, c.r1, c.c1] : null; });

test('Ctrl+Alt+→/← 以单词为单位扩展矩形', () =>
  withMocks({ name: 'bn', portOffset: 250 }, async ({ page, press }) => {
    await page.locator('#ed-content').click(); await setup(page, 10, 0); await page.locator('#ed-content').focus();
    await press('Control+Alt+ArrowRight');
    assert.deepEqual(await box(page), [10, 0, 10, 7], 'word10 → 下一个词首 (列 7)');
    await press('Control+Alt+ArrowRight');
    assert.deepEqual(await box(page), [10, 0, 10, 13]);
    await press('Control+Alt+ArrowLeft');
    assert.deepEqual(await box(page), [10, 0, 10, 7]);
  }));

test('Ctrl+Alt+End / Home 扩到文件末/首行; Alt+PgDn 整页, Ctrl+Alt+PgDn 半页', () =>
  withMocks({ name: 'bn', portOffset: 251 }, async ({ page, press }) => {
    await page.locator('#ed-content').click(); await setup(page, 50, 3); await page.locator('#ed-content').focus();
    await press('Control+Alt+End');
    let b = await box(page); assert.equal(b[0], 50); assert.equal(b[2], 119); assert.equal(b[1], 3);
    await setup(page, 50, 3); await page.locator('#ed-content').focus();
    await press('Control+Alt+Home');
    b = await box(page); assert.equal(Math.min(b[0], b[2]), 0);
    await setup(page, 10, 2); await page.locator('#ed-content').focus();
    await press('Alt+PageDown');
    const full = (await box(page))[2] - 10;
    await setup(page, 10, 2); await page.locator('#ed-content').focus();
    await press('Control+Alt+PageDown');
    const half = (await box(page))[2] - 10;
    assert.ok(full >= 5 && half >= 2 && half < full, `整页 ${full} 行, 半页 ${half} 行`);
    await press('Alt+PageUp');
    assert.ok((await box(page))[2] < 10 + half, 'Alt+PgUp 往回收');
  }));
