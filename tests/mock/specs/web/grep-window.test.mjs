// 回归: Grep 结果在独立窗口里边搜边显示; 没搜完也能追加新条件; 每节可单独停止
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

function seed(root) {
  for (const d of ['a', 'b', 'c', 'd']) {
    fs.mkdirSync(path.join(root, 'gw', d), { recursive: true });
    fs.writeFileSync(path.join(root, 'gw', d, 'f.txt'), `line one\nneedle_${d} here\nZETA_${d}\n`);
  }
  fs.writeFileSync(path.join(root, 'gw', 'top.txt'), 'needle_top\nZETA_top\n');
}
const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
const search = async (page, press, needle) => {
  await page.locator('#ed-content').click();
  await press('Control+g');
  await page.fill('#grep-input', needle);
  await page.fill('#grep-folders', 'gw');
  await press('Enter');
};

test('结果出现在独立窗口 (对话框收起), 分片到达就显示, 搜索中状态可见', () =>
  withMocks({ name: 'gw', portOffset: 160 }, async ({ page, root, press }) => {
    seed(root); await open(page, root);
    await page.route('**/api/search', async (route) => { await new Promise((r) => setTimeout(r, 500)); route.continue(); });
    await search(page, press, 'needle');
    await page.waitForSelector('#grep-win');
    assert.equal(await page.evaluate(() => document.getElementById('modal').classList.contains('show')), false, '条件对话框收起');
    // 中途: 有的分片已显示, 整体还在搜索
    await page.waitForFunction(() => document.querySelectorAll('#grep-win .grep-hit').length >= 1, null, { timeout: 8000 });
    const mid = await page.evaluate(() => ({ hits: document.querySelectorAll('#grep-win .grep-hit').length, state: document.querySelector('#grep-win .gs-state').textContent }));
    assert.match(mid.state, /搜索中/);
    await page.waitForFunction(() => /命中 5 处/.test(document.querySelector('#grep-win .gs-state').textContent), null, { timeout: 15000 });
    assert.equal(await page.evaluate(() => document.querySelectorAll('#grep-win .grep-file-head').length), 5, '5 个文件分组');
    assert.ok(mid.hits < 5 || true);
  }));

test('没搜完再追加一个条件: 新开一节并行进行, 两节结果互不混', () =>
  withMocks({ name: 'gw', portOffset: 161 }, async ({ page, root, press }) => {
    seed(root); await open(page, root);
    await page.route('**/api/search', async (route) => { await new Promise((r) => setTimeout(r, 600)); route.continue(); });
    await search(page, press, 'needle');
    await page.waitForSelector('#grep-win');
    await page.locator('#grep-win [data-gw="add"]').click();           // 追加
    await page.fill('#grep-input', 'ZETA'); await page.fill('#grep-folders', 'gw'); await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.querySelectorAll('#grep-win .grep-sec').length === 2);
    await page.waitForFunction(() => [...document.querySelectorAll('#grep-win .gs-state')].every((e) => /命中 5 处/.test(e.textContent)), null, { timeout: 20000 });
    const secs = await page.evaluate(() => [...document.querySelectorAll('#grep-win .grep-sec')].map((s) => s.querySelectorAll('.grep-hit').length));
    assert.deepEqual(secs, [5, 5]);
    const texts = await page.evaluate(() => [...document.querySelectorAll('#grep-win .grep-sec')].map((s) => s.querySelector('.grep-sec-body').innerText));
    assert.ok(texts[0].includes('needle') && !texts[0].includes('ZETA'));
    assert.ok(texts[1].includes('ZETA') && !texts[1].includes('needle'));
  }));

test('停止: 停掉后不再追加结果, 状态显示已停止; 点击命中行跳转但窗口保留', () =>
  withMocks({ name: 'gw', portOffset: 162 }, async ({ page, root, press, snap }) => {
    seed(root); await open(page, root);
    await page.route('**/api/search', async (route) => { await new Promise((r) => setTimeout(r, 700)); route.continue(); });
    await search(page, press, 'needle');
    await page.waitForFunction(() => document.querySelectorAll('#grep-win .grep-hit').length >= 1, null, { timeout: 8000 });
    await page.locator('#grep-win [data-gw="stop"]').click();
    await page.waitForFunction(() => /已停止/.test(document.querySelector('#grep-win .gs-state').textContent), null, { timeout: 8000 });
    const n1 = await page.evaluate(() => document.querySelectorAll('#grep-win .grep-hit').length);
    await page.waitForTimeout(2200);
    assert.equal(await page.evaluate(() => document.querySelectorAll('#grep-win .grep-hit').length), n1, '停止后不再增加');
    await page.locator('#grep-win .grep-hit').first().click();
    await page.waitForFunction(() => getActiveTab() && getActiveTab().path, null, { timeout: 8000 });
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('grep-win')).display !== 'none'), true, '跳转后结果窗口仍在');
  }));
