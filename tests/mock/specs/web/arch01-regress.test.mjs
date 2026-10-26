// 回归 (评审发现): 矩形 / 多光标迁到 Rust 之后的几处边界
//   - 撤销让文档缩短后留下的旧列块 / 旧光标不能把真实的最后一行当空行写回去
//   - 多光标隔得很远: 只交光标所在的行, 中间的行不发; 前一段并行少掉的行不影响后一段的光标行号
//   - 旧光标退化到不足两个: 退回单光标输入, 键不会丢
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('旧列块 / 旧光标: 撤销后文档缩短, 输入不会写坏最后一行', () =>
  withMocks({ name: 'a1r', portOffset: 541 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      const t = getActiveTab();
      t.lines = ['l0', 'l1', 'l2', 'l3', 'l4', 'l5', 'l6', 'l7', 'l8', 'l9'];
      t.cursor = { row: 0, col: 0 }; t.sel = null;
      t.colSel = { r0: 25, c0: 0, r1: 26, c1: 0 };      // 文档比它短 (撤销 / 重载后留下的旧列块)
      renderVim(t);
    });
    await page.evaluate(async () => { await columnInsert('X'); });
    assert.equal(await page.evaluate(() => getActiveTab().lines[9]), 'Xl9', '夹到最后一行, 在它左端插入; 不是把它当成空行覆盖');
    // 旧光标 (越界, 且夹到同一处) 退化为单光标: 输入照常落到主光标, 不丢键
    await page.evaluate(() => {
      const t = getActiveTab();
      t.colSel = null; t.lines = ['aa', 'bb']; t.cursor = { row: 1, col: 1 }; t.sel = null;
      t.cursors = [{ row: 8, col: 5 }, { row: 9, col: 5 }];
      renderVim(t);
    });
    await page.evaluate(async () => { await insertCharMulti('Q'); });
    const s = await page.evaluate(() => ({ lines: getActiveTab().lines, multi: !!(getActiveTab().cursors && getActiveTab().cursors.length > 1) }));
    assert.deepEqual(s.lines, ['aa', 'bbQ'], '键没有丢: 旧光标夹到最后一行行尾去重后只剩一个, 退回单光标输入');
    assert.equal(s.multi, false, '多光标模式已退出');
  }));

test('多光标隔得很远: 只交光标所在的行; 前一段并行少掉的行不影响后一段', () =>
  withMocks({ name: 'a1r', portOffset: 542 }, async ({ page }) => {
    const bodies = [];
    page.on('request', (r) => { if (r.url().includes('/api/edit_cmd')) bodies.push(JSON.parse(r.postData() || '{}')); });
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      const t = getActiveTab();
      t.lines = Array.from({ length: 1500 }, (_, i) => 'row' + i);
      t.cursor = { row: 1, col: 0 }; t.sel = null;
      t.cursors = [{ row: 1, col: 0 }, { row: 1400, col: 3 }];
      renderVim(t);
    });
    await page.evaluate(async () => { await backspaceMulti(); });
    const b = bodies.find((x) => x.cmd === 'multi-backspace');
    assert.ok(b, '走了 multi-backspace');
    assert.ok(b.rows.length <= 4, '只交光标所在的行 (和行首退格要并到的上一行), 实际 ' + b.rows.length);
    const s = await page.evaluate(() => { const t = getActiveTab(); return { n: t.lines.length, a: t.lines[0], b: t.lines[1399], cur: t.cursors.map((c) => [c.row, c.col]) }; });
    assert.equal(s.n, 1499, '行首退格并行');
    assert.equal(s.a, 'row0row1');
    assert.equal(s.b, 'ro1400', '第 1400 行 (并行后是 1399) 删掉光标前的一个字符 w');
    assert.deepEqual(s.cur, [[0, 4], [1399, 2]], '后一段的光标行号扣掉前面并掉的一行');
  }));
