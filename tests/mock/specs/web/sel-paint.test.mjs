// 回归: 选中文字时只有选中的部分着底色, 选区前面没选的文字不着色
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('选区前的文字无底色, 选中的文字有底色', () =>
  withMocks({ name: 'selpaint', portOffset: 570 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      const t = getActiveTab();
      t.lines = ['hello world foo']; t.cursor = { row: 0, col: 11 }; t.sel = { r0: 0, c0: 6, r1: 0, c1: 11 };
      renderVim(t);
    });
    const bg = await page.evaluate(() => {
      const p = document.querySelector('#ed-content .sel-prefix'), a = document.querySelector('#ed-content .sel-active');
      return { pre: getComputedStyle(p).backgroundColor, act: getComputedStyle(a).backgroundColor, preText: p.textContent, actText: a.textContent };
    });
    assert.equal(bg.preText, 'hello ');
    assert.equal(bg.actText, 'world');
    assert.equal(bg.pre, 'rgba(0, 0, 0, 0)', '没选中的前文不着色');
    assert.notEqual(bg.act, 'rgba(0, 0, 0, 0)', '选中的有底色');
  }));
