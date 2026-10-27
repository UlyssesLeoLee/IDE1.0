// 回归: PPA 宏的步骤按顺序落地 —— TYPE / CMD / KEY 经桥都是异步的, 以前后面的 KEY 会插到前面的 CMD 之前
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('PPA 宏: TYPE -> CMD -> TYPE 按顺序执行', () =>
  withMocks({ name: 'ppa', portOffset: 547 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { window.prompt = () => 'TYPE abc\nCMD duplicate-line\nTYPE Z\n'; });
    await page.evaluate(async () => { await runPpaMacro(); });
    const lines = await page.evaluate(() => getActiveTab().lines);
    // abc 打出来 -> 整行复制成两行 -> 光标在第二行行尾再打 Z
    assert.deepEqual(lines.slice(0, 2), ['abc', 'abcZ']);
  }));
