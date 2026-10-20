// 回归: 真实鼠标 — 点击落光标 / 拖选 / Alt 纵向(矩形)选择; 在矩形里输入 = 覆盖选中内容 (不是插在前面)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withApp, grid } from '../../lib/harness.mjs';

async function seed(page) {
  await page.locator('#ed-content').click();
  for (const l of ['abcdefghij', 'klmnopqrst', 'uvwxyzABCD', 'EFGHIJKLMN']) { await page.keyboard.type(l); await page.keyboard.press('Enter'); }
  return grid(page);
}

test('点击把光标落在点击处; 行内 / 跨行拖选, 跨行高亮逐行正确', () =>
  withApp({ name: 'mb', portOffset: 20 }, async ({ page, snap }) => {
    const g = await seed(page);
    await page.mouse.click(g.x(3), g.y(1));
    assert.deepEqual((await snap()).cursor, { row: 1, col: 3 });
    await page.mouse.move(g.x(2), g.y(0)); await page.mouse.down(); await page.mouse.move(g.x(6), g.y(0), { steps: 4 }); await page.mouse.up();
    assert.deepEqual((await snap()).sel, [0, 2, 0, 6]);
    await page.mouse.click(g.x(0), g.y(3));
    const g2 = await grid(page);
    await page.mouse.move(g2.x(5), g2.y(0)); await page.mouse.down(); await page.mouse.move(g2.x(3), g2.y(2), { steps: 6 }); await page.mouse.up();
    assert.deepEqual((await snap()).sel, [0, 5, 2, 3]);
    const painted = await page.evaluate(() => [...document.querySelectorAll('#ed-content .hl-row')].slice(0, 3).map((r) => r.querySelector('.sel-active')?.textContent ?? null));
    assert.deepEqual(painted, ['fghij', 'klmnopqrst', 'uvw']);
  }));

test('Alt+拖动 = 3行×4列; 输入 X 覆盖矩形里的内容 (cdef/mnop/wxyz)', () =>
  withApp({ name: 'mb', portOffset: 21 }, async ({ page, snap, app }) => {
    const g = await seed(page);
    await page.keyboard.down('Alt');
    await page.mouse.move(g.x(2), g.y(0)); await page.mouse.down();
    await page.mouse.move(g.x(4), g.y(1), { steps: 4 }); await page.mouse.move(g.x(6), g.y(2), { steps: 4 });
    await page.mouse.up(); await page.keyboard.up('Alt');
    assert.deepEqual((await snap()).box, [0, 2, 2, 6]);
    assert.match((await snap()).status.colsel, /3行/);
    await page.keyboard.type('X');
    assert.deepEqual((await snap()).lines.slice(0, 3), ['abXghij', 'klXqrst', 'uvXABCD']);
    assert.deepEqual(app.errors, []);
  }));

test('矩形里退格 = 删掉选中的矩形内容 (保留插入点)', () =>
  withApp({ name: 'mb', portOffset: 22 }, async ({ page, snap }) => {
    const g = await seed(page);
    await page.keyboard.down('Alt');
    await page.mouse.move(g.x(2), g.y(0)); await page.mouse.down(); await page.mouse.move(g.x(4), g.y(2), { steps: 5 }); await page.mouse.up();
    await page.keyboard.up('Alt');
    await page.keyboard.press('Backspace');
    assert.deepEqual((await snap()).lines.slice(0, 3), ['abefghij', 'klopqrst', 'uvyzABCD']);
  }));

test('Alt+拖动可以拖到行尾右侧 (虚拟列); Alt+方向键建立纵向选区', () =>
  withApp({ name: 'mb', portOffset: 23 }, async ({ page, snap }) => {
    const g = await seed(page);
    await page.keyboard.down('Alt');
    await page.mouse.move(g.x(8), g.y(0)); await page.mouse.down(); await page.mouse.move(g.x(14), g.y(2), { steps: 6 }); await page.mouse.up();
    await page.keyboard.up('Alt');
    assert.deepEqual((await snap()).box, [0, 8, 2, 14]);
    await page.keyboard.press('Escape'); await page.mouse.click(g.x(1), g.y(0));
    await page.keyboard.down('Alt');
    for (const k of ['ArrowDown', 'ArrowDown', 'ArrowRight', 'ArrowRight']) await page.keyboard.press(k);
    await page.keyboard.up('Alt');
    assert.deepEqual((await snap()).box, [0, 1, 2, 3]);
  }));
