// 回归: 日文输入法 (IME) — 用 CDP imeSetComposition / insertText 仿真真实输入法
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withApp, fill } from '../../lib/harness.mjs';
import { ime } from '../../lib/mock.mjs';

test('预编辑文字显示在光标处, 上屏后进文档, 光标在其后', () =>
  withApp({ name: 'ime', portOffset: 40 }, async ({ page, snap }) => {
    await page.locator('#ed-content').click(); await page.keyboard.type('ab');
    const i = ime(page);
    for (const s of ['に', 'にほ', 'にほんご']) await i.preedit(s);
    assert.equal(await page.evaluate(() => document.getElementById('ed-content').innerText.replace(/\n/g, '|')), 'abにほんご');
    await i.commit('日本語'); await page.waitForTimeout(150);
    const s = await snap();
    assert.deepEqual([s.lines, s.cursor], [['ab日本語'], { row: 0, col: 5 }]);
  }));

test('没有 keydown 的 insertText (语音 / 软键盘 / 自动化) 不丢; 继续打字正常', () =>
  withApp({ name: 'ime', portOffset: 41 }, async ({ page, snap }) => {
    await page.locator('#ed-content').click();
    await ime(page).commit('テスト'); await page.waitForTimeout(120);
    await page.keyboard.type('Z');
    assert.deepEqual((await snap()).lines, ['テストZ']);
  }));

test('在选区上输入法上屏 = 替换选区, 撤销回退', () =>
  withApp({ name: 'ime', portOffset: 42 }, async ({ page, snap, press }) => {
    await fill(page, 'abc');
    await press('ArrowRight'); await press('Shift+ArrowRight');       // 选中 "b"
    await ime(page).compose(['か'], '漢'); await page.waitForTimeout(150);
    assert.equal((await snap()).lines[0], 'a漢c');
    await press('Control+z'); await page.waitForTimeout(100);
    assert.notEqual((await snap()).lines[0], 'a漢c');
  }));

test('组合期间按 Enter 不产生换行; 上屏后才进文档', () =>
  withApp({ name: 'ime', portOffset: 43 }, async ({ page, snap }) => {
    await page.locator('#ed-content').click();
    const i = ime(page);
    await i.preedit('あ');
    await page.keyboard.press('Enter');
    assert.deepEqual((await snap()).lines, ['']);
    await i.commit('あ'); await page.waitForTimeout(100);
    assert.deepEqual((await snap()).lines, ['あ']);
  }));
