// 回归: 折行感知的 ↑ ↓ Home End (WBS-02 5.2) —— 自动换行打开时按显示行走; 没折行的行保持原来的逻辑行语义
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = (page) => page.evaluate(() => {
  dispatchCommand('toggle-word-wrap');
  edContent.style.maxWidth = 'calc(20ch + 40px)';
  const t = getActiveTab();
  t.lines = ['a'.repeat(50), 'short', 'b'.repeat(50)];
  t.cursor = { row: 0, col: 5 }; t.sel = null; renderVim(t); updateCaret(false);
});
const cur = (page) => page.evaluate(() => getActiveTab().cursor);

test('折行时 ↓ 走到下一显示行 (仍在同一逻辑行), 连按能走到下一逻辑行; ↑ 反向', () =>
  withMocks({ name: 'wn', portOffset: 638 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await setup(page);
    await page.evaluate(() => { getActiveTab().cursor = { row: 0, col: 5 }; updateCaret(false); });
    await press('ArrowDown');
    const a = await cur(page);
    assert.equal(a.row, 0, '还在第 1 个逻辑行里 (下一显示行)');
    assert.ok(a.col > 15 && a.col < 30, '大约落在 5 + 一个折行宽度处: ' + a.col);
    await press('ArrowDown'); await press('ArrowDown');
    assert.equal((await cur(page)).row, 1, '走完这个逻辑行的显示行后才到下一逻辑行');
    await press('ArrowUp');
    assert.equal((await cur(page)).row, 0, '回到上一逻辑行的最后一个显示行');
    assert.ok((await cur(page)).col > 30, '是最后一个显示行, 不是行首');
  }));

test('折行时 Home / End 到显示行的行首 / 行尾; 没折行的行仍是逻辑行首尾', () =>
  withMocks({ name: 'wn', portOffset: 639 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await setup(page);
    await page.evaluate(() => { getActiveTab().cursor = { row: 0, col: 25 }; updateCaret(false); });
    await press('Home');
    const h = await cur(page);
    assert.ok(h.row === 0 && h.col > 5 && h.col < 25, '显示行行首, 不是逻辑行首: ' + JSON.stringify(h));
    await press('End');
    const e = await cur(page);
    assert.ok(e.row === 0 && e.col > h.col && e.col < 50, '显示行行尾, 不是逻辑行末: ' + JSON.stringify(e));
    await page.evaluate(() => { getActiveTab().cursor = { row: 1, col: 2 }; updateCaret(false); });
    await press('Home'); assert.deepEqual(await cur(page), { row: 1, col: 0 });
    await press('End'); assert.deepEqual(await cur(page), { row: 1, col: 5 });
  }));

test('不折行 (自动换行关) 时 ↓ 仍是逻辑行', () =>
  withMocks({ name: 'wn', portOffset: 640 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['a'.repeat(50), 'short']; t.cursor = { row: 0, col: 5 }; renderVim(t); });
    await press('ArrowDown');
    assert.equal((await cur(page)).row, 1);
  }));

test('折行时 Shift+↓ / Shift+End 按显示行伸缩选区, 锚点不动', () =>
  withMocks({ name: 'wn', portOffset: 642 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await setup(page);
    await page.evaluate(() => { getActiveTab().cursor = { row: 0, col: 5 }; updateCaret(false); });
    await press('Shift+ArrowDown');
    let s = await page.evaluate(() => ({ sel: getActiveTab().sel, cur: getActiveTab().cursor }));
    assert.deepEqual([s.sel.r0, s.sel.c0, s.sel.r1], [0, 5, 0], '选区从原光标到下一显示行 (仍在第 1 个逻辑行)');
    assert.ok(s.sel.c1 > 15 && s.sel.c1 < 30);
    await press('Shift+ArrowUp');
    s = await page.evaluate(() => ({ sel: getActiveTab().sel }));
    assert.equal(s.sel, null, '退回锚点所在显示行后选区收成空');
    await press('Shift+End');
    s = await page.evaluate(() => ({ sel: getActiveTab().sel }));
    assert.ok(s.sel && s.sel.c0 === 5 && s.sel.c1 > 15 && s.sel.c1 < 30, '选到本显示行行尾: ' + JSON.stringify(s.sel));
  }));
