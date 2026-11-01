// 回归: 共通設定「色・フォント」页 (WBS-02 3.5) —— 字体 / 行高 / 语法颜色真的作用到编辑区, 清除后回到主题默认
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('字体 / 行高 / 关键字颜色 OK 后生效, 清除后还原', () =>
  withMocks({ name: 'cf', portOffset: 625 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => commonConfig('色・フォント'));
    await page.waitForSelector('.cfg-tab.active');
    const t = page.locator('.cfg-row input[type="text"]');
    await t.fill('Courier New, monospace'); await t.dispatchEvent('change');
    const n = page.locator('.cfg-row input[type="number"]');
    await n.fill('24'); await n.dispatchEvent('change');
    await page.locator('.cfg-row', { hasText: '关键字颜色' }).locator('input[type="color"]').fill('#ff0000');
    await page.locator('.cfg-row', { hasText: '关键字颜色' }).locator('input[type="color"]').dispatchEvent('change');
    await page.locator('#md-foot button:first-child').click();
    const r = await page.evaluate(() => ({ ff: getComputedStyle(document.getElementById('ed-content')).fontFamily, lh: getComputedStyle(document.getElementById('ed-content')).lineHeight, css: document.getElementById('user-colors').textContent }));
    assert.match(r.ff, /Courier New/); assert.equal(r.lh, '24px'); assert.match(r.css, /\.tk-keyword\{color:#ff0000/i);
    await page.evaluate(() => { PREFS.fontFamily = ''; PREFS.lineHeight = 0; PREFS.colors = {}; applyColorFont(); });
    assert.equal(await page.evaluate(() => document.getElementById('user-colors').textContent), '');
  }));
