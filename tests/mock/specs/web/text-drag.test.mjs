// 回归: 编辑区内拖动选中的文字移动 / Ctrl 复制 (WBS-02 5.6); 没拖动就是普通点击
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = async (page) => {
  await page.locator('#ed-content').click();
  await page.evaluate(() => {
    const t = getActiveTab(); t.lines = ['hello world', 'second', 'third']; t.sel = { r0: 0, c0: 0, r1: 0, c1: 5 }; t.selAnchor = { row: 0, col: 0 }; t.cursor = { row: 0, col: 5 }; renderVim(t);
    window.__pt = (row, col) => { const old = t.cursor; t.cursor = { row, col }; const r = caretRect(t); t.cursor = old; return { x: r.left, y: r.top + 9 }; };
  });
};
const pt = (page, row, col) => page.evaluate(([r, c]) => window.__pt(r, c), [row, col]);
const lines = (page) => page.evaluate(() => getActiveTab().lines.slice());

test('选中的文字拖到别处 = 移动 (一步撤销), 放下后保持选中', () =>
  withMocks({ name: 'td', portOffset: 643 }, async ({ page }) => {
    await setup(page);
    const a = await pt(page, 0, 2), b = await pt(page, 1, 3);
    await page.mouse.move(a.x, a.y); await page.mouse.down();
    await page.mouse.move((a.x + b.x) / 2, (a.y + b.y) / 2, { steps: 4 }); await page.mouse.move(b.x, b.y, { steps: 4 }); await page.mouse.up();
    assert.deepEqual(await lines(page), [' world', 'sechelloond', 'third']);
    assert.deepEqual(await page.evaluate(() => getActiveTab().sel), { r0: 1, c0: 3, r1: 1, c1: 8 });
    await page.evaluate(() => dispatchCommand('undo')); await page.evaluate(() => window.__editIdle());
    assert.deepEqual(await lines(page), ['hello world', 'second', 'third'], '一步撤销');
  }));

test('按住 Ctrl 放下 = 复制; 拖到选区里面 = 不动; 没拖动 = 普通点击收起选区', () =>
  withMocks({ name: 'td', portOffset: 644 }, async ({ page }) => {
    await setup(page);
    const a = await pt(page, 0, 2), b = await pt(page, 2, 2);
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move(b.x, b.y, { steps: 6 });
    await page.keyboard.down('Control'); await page.mouse.up(); await page.keyboard.up('Control');
    assert.deepEqual(await lines(page), ['hello world', 'second', 'thhelloird'], 'Ctrl 放下是复制, 原处还在');
    await setup(page);
    const c = await pt(page, 0, 3);
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move(c.x, c.y, { steps: 4 }); await page.mouse.up();
    assert.deepEqual(await lines(page), ['hello world', 'second', 'third'], '放在选区里面: 不动');
    await setup(page);
    await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.up();
    assert.equal(await page.evaluate(() => getActiveTab().sel), null, '没拖动: 收起选区');
    assert.deepEqual(await page.evaluate(() => getActiveTab().cursor), { row: 0, col: 2 });
  }));
