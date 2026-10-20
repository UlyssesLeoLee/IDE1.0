// 回归: プラグイン UI — 登记持久化 / 页内输入 / 运行 → 替换选区或插入 / 失败时如实说明 / 移除
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);

test('加载插件失败 (默认构建无 wasm 运行时): 登记仍保留, 失败原因如实显示', () =>
  withMocks({ name: 'pl', portOffset: 230 }, async ({ page }) => {
    await cmd(page, 'plugin-load');
    await page.locator('.md-input').fill('C:/plugins/up.wasm'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(600);
    assert.match(await page.evaluate(() => document.getElementById('md-body').innerText), /wasm/);
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('ide10.plugins'))[0].path), 'C:/plugins/up.wasm');
    await cmd(page, 'plugin-manage');
    assert.match(await page.evaluate(() => document.getElementById('md-body').innerText), /up\.wasm/);
    await page.locator('[data-pl^="del:"]').click();
    assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('ide10.plugins')).length), 0);
  }));

test('插件成功: 对选区运行 → 替换选区; 无选区则作用于全文; 插入模式', () =>
  withMocks({ name: 'pl', portOffset: 231 }, async ({ page, snap, press }) => {
    await page.route('**/api/plugin_load', async (route) => {
      const body = JSON.parse(route.request().postData() || '{}');
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, output: String(body.input).toUpperCase() }) });
    });
    await fill(page, 'abc def');
    await page.evaluate(() => { const t = getActiveTab(); t.sel = { r0: 0, c0: 4, r1: 0, c1: 7 }; renderVim(t); });
    await page.evaluate(() => runPlugin('x.wasm', 'replace')); await page.waitForTimeout(300);
    assert.equal((await snap()).lines[0], 'abc DEF');
    await page.evaluate(() => { getActiveTab().sel = null; return runPlugin('x.wasm', 'replace'); }); await page.waitForTimeout(300);
    assert.equal((await snap()).lines[0], 'ABC DEF');
    const out = await page.evaluate(() => runPlugin('x.wasm', 'show')); await page.waitForTimeout(150);
    assert.equal(out, 'ABC DEF');
    assert.match(await page.evaluate(() => document.getElementById('md-body').innerText), /ABC DEF/);
  }));
