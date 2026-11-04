// 回归: 共通設定 —— 草稿 + OK / 取消; 新增的 検索 / ファイル / 編集 页各项真的生效
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const ok = (page) => page.locator('#md-foot button:first-child').click();
const cancel = (page) => page.locator('#md-foot button:nth-child(2)').click();
const tab = (page, name) => page.locator('.cfg-tab', { hasText: name }).click();
const row = (page, text) => page.locator('.cfg-row', { hasText: text });

test('取消丢弃全部草稿; OK 才生效并落盘', () =>
  withMocks({ name: 'cc', portOffset: 613 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => dispatchCommand('common-config'));
    await page.waitForSelector('.cfg-tab');
    await tab(page, '文件');
    await row(page, '外部修改').locator('input').uncheck();
    await row(page, '保留个数').locator('input').fill('5');
    await row(page, '保留个数').locator('input').dispatchEvent('change');
    await cancel(page);
    assert.equal(await page.evaluate(() => [PREFS.checkExternal, PREFS.mruFiles]).then((v) => v.join()), ',', '取消: 什么都没变');
    await page.evaluate(() => dispatchCommand('common-config'));
    await page.waitForSelector('.cfg-tab');
    await tab(page, '文件');
    await row(page, '外部修改').locator('input').uncheck();
    await row(page, '保留个数').locator('input').fill('5');
    await row(page, '保留个数').locator('input').dispatchEvent('change');
    await ok(page);
    assert.deepEqual(await page.evaluate(() => [PREFS.checkExternal, PREFS.mruFiles]), [false, 5]);
    assert.match(await page.evaluate(() => localStorage.getItem('ide10.settings')), /"checkExternal":false/, '落盘');
  }));

test('検索页的默认值作用到查找状态; 编辑页的 Tab 插入空格作用到 Tab 键', () =>
  withMocks({ name: 'cc', portOffset: 614 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.name = 'x.txt'; t.lines = ['']; t.cursor = { row: 0, col: 0 }; renderVim(t); });
    await page.evaluate(() => dispatchCommand('common-config'));
    await page.waitForSelector('.cfg-tab');
    await tab(page, '検索');
    await row(page, '区分大小写').locator('input').check();
    await row(page, '正则').locator('input').check();
    await tab(page, '编辑');
    await row(page, 'Tab 键插入空格').locator('input').check();
    await ok(page);
    assert.deepEqual(await page.evaluate(() => [FIND_STATE.matchCase, FIND_STATE.useRegex]), [true, true]);
    await page.locator('#ed-content').focus();
    await press('Tab');
    await page.waitForFunction(() => /^ +$/.test(getActiveTab().lines[0]), null, { timeout: 8000 });
  }));
