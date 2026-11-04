// 回归: 宏登记 (WBS-02 4.3) —— 20 个槽, 首选项「宏」页编辑, macro-slot-N 命令运行, 当前文档可登记进空槽
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

test('登记 .mac 与 JS 宏到槽, 命令运行; 空槽给提示', () =>
  withMocks({ name: 'ms', portOffset: 627 }, async ({ page, snap }) => {
    await fill(page, 'abc');
    await page.evaluate(() => { setMacroSlot(0, { name: 'ins', kind: 'mac', code: 'S_InsText("X");' }); setMacroSlot(1, { name: 'js', kind: 'js', code: 'await Editor.InsText("Y");' }); });
    await page.evaluate(() => { const t = getActiveTab(); t.cursor = { row: 0, col: 0 }; t.sel = null; });
    await page.evaluate(() => dispatchCommand('macro-slot-1')); await page.evaluate(() => window.__editIdle());
    assert.equal((await snap()).lines[0], 'Xabc');
    await page.evaluate(() => dispatchCommand('macro-slot-2')); await page.evaluate(() => window.__editIdle());
    assert.match((await snap()).lines[0], /Y/);
    await page.evaluate(() => dispatchCommand('macro-slot-5'));
    assert.match((await snap()).status.tip, /空的/);
    assert.ok((await page.evaluate(() => allCommandLabels().get('macro-slot-1'))).includes('ins'), '键位里能看到名字');
  }));

test('首选项「宏」页: 编辑后 OK 才登记; 登记当前文档进第一个空槽', () =>
  withMocks({ name: 'ms', portOffset: 628 }, async ({ page }) => {
    await fill(page, 'S_InsText("Z");');
    await page.evaluate(() => { getActiveTab().name = 'z.mac'; });
    await page.evaluate(() => commonConfig('宏'));
    await page.waitForSelector('.cfg-tab.active');
    await page.locator('.cfg-row button', { hasText: '登记到第一个空槽' }).click();
    assert.deepEqual(await page.evaluate(() => [macroSlots()[0].name, macroSlots()[0].kind]), ['z', 'mac']);
  }));
