// 回归: 撤销栈在 Rust 后端 (editcore::history), 按「页面会话 + 标签 id」登记
//   - 重新载入页面后, 新文档不能撤销到上一次会话同名标签 (t1) 的补丁上
//   - 撤销 / 重做 往返
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

test('撤销 / 重做 往返; 重新载入后不会撤销到旧会话的补丁', () =>
  withMocks({ name: 'hi', portOffset: 470 }, async ({ page, snap, press }) => {
    await fill(page, 'abc');
    await press('End'); await page.keyboard.type('XYZ'); await page.evaluate(() => window.__editIdle());
    assert.equal((await snap()).lines[0], 'abcXYZ');
    await press('Control+z');
    assert.equal((await snap()).lines[0], 'abcXY');
    await press('Control+y');
    assert.equal((await snap()).lines[0], 'abcXYZ');
    const idBefore = await page.evaluate(() => getActiveTab().id);
    await page.reload(); await page.waitForTimeout(800);
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['fresh']; t.cursor = { row: 0, col: 0 }; renderVim(t); });
    assert.equal(await page.evaluate(() => getActiveTab().id), idBefore, '重新载入后标签 id 从头数, 与上一次相同');
    await press('Control+z');
    assert.deepEqual((await snap()).lines, ['fresh'], '没有任何改动的新文档, 撤销什么也不做');
  }));
