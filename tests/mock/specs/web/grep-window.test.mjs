// 回归: Grep 结果是一个编辑窗口 (主编辑区里的新标签, Sakura 格式的文本): 边搜边追加、命中词自动高亮、
// 可在结果里继续查找、Enter / 双击跳转、Esc 停止、每次搜索各开一个标签
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
const grepTab = (page) => page.evaluate(() => { const t = getActiveTab(); return t && t.grep ? { name: t.name, lines: t.lines.slice(), dirty: t.dirty } : null; });
const waitDone = (page, n) => page.waitForFunction((k) => { const t = getActiveTab(); return t && t.grep && t.lines.some((l) => l.startsWith(k + ' 個が検索されました')); }, n, { timeout: 20000 });

test('结果在新的编辑标签里 (对话框收起), 分片到达就追加, Sakura 格式', () =>
  withMocks({ name: 'gw', portOffset: 160 }, async ({ page, root, press }) => {
    seed(root); await open(page, root);
    await page.route('**/api/search', async (route) => { await new Promise((r) => setTimeout(r, 400)); route.continue(); });
    await search(page, press, 'needle');
    assert.equal(await page.evaluate(() => document.getElementById('modal').classList.contains('show')), false, '条件对话框收起');
    await page.waitForFunction(() => { const t = getActiveTab(); return t && t.grep && t.lines.some((l) => /needle_/.test(l)); }, null, { timeout: 8000 });
    const mid = await grepTab(page);
    assert.ok(mid, '活动标签就是 Grep 结果');
    assert.ok(!mid.lines.some((l) => /個が検索されました/.test(l)) || mid.lines.length > 5, '搜索途中结果已经在文档里');
    await waitDone(page, 5);
    const t = await grepTab(page);
    assert.equal(t.name, 'Grep: needle');
    assert.equal(t.dirty, false, '结果文档不算"未保存"');
    assert.equal(t.lines[0], '□検索条件  "needle"');
    const hits = t.lines.filter((l) => /\(\d+,\d+\): /.test(l));
    assert.equal(hits.length, 5, '5 处命中, 每处一行');
    assert.match(hits[0], /[\\/]needle_|f\.txt\(2,1\): needle_|top\.txt\(1,1\): needle_top/, '路径(行,列): 内容');
    assert.equal(await page.evaluate(() => document.querySelectorAll('#grep-win, #grep-results').length), 0, '不再有专用的悬浮页面');
  }));

test('命中词自动高亮; Ctrl+F3 切换', () =>
  withMocks({ name: 'gw', portOffset: 161 }, async ({ page, root, press }) => {
    seed(root); await open(page, root);
    await search(page, press, 'needle');
    await waitDone(page, 5);
    const st = await page.evaluate(() => ({ needle: FIND_STATE.needle, marks: FIND_STATE.marks }));
    assert.deepEqual(st, { needle: 'needle', marks: true });
    await page.waitForFunction(() => CSS.highlights.get('find-marks') && CSS.highlights.get('find-marks').size >= 5, null, { timeout: 8000 });
    await page.locator('#ed-content').focus();
    await press('Control+F3');
    await page.waitForFunction(() => !CSS.highlights.get('find-marks') || CSS.highlights.get('find-marks').size === 0, null, { timeout: 8000 });
    assert.equal(await page.evaluate(() => FIND_STATE.marks), false, '切换后不再高亮');
    await press('Control+F3');
    await page.waitForFunction(() => CSS.highlights.get('find-marks') && CSS.highlights.get('find-marks').size >= 5, null, { timeout: 8000 });
  }));

test('在结果里继续查找 (Ctrl+F), 新词接管高亮', () =>
  withMocks({ name: 'gw', portOffset: 162 }, async ({ page, root, press, snap }) => {
    seed(root); await open(page, root);
    await search(page, press, 'needle');
    await waitDone(page, 5);
    await page.locator('#ed-content').focus();
    await page.evaluate(() => { const t = getActiveTab(); t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t); });
    await press('Control+f');
    await page.fill('#find-input', 'needle_b');
    await press('Enter');
    await page.waitForTimeout(500);
    const s = await snap();
    const line = await page.evaluate((r) => getActiveTab().lines[r], s.sel[0]);
    assert.match(line, /needle_b/, '选中的是结果文档里 needle_b 那一行');
    assert.equal(await page.evaluate(() => FIND_STATE.needle), 'needle_b', '高亮跟着新的查找词');
  }));

test('Enter 跳到命中处 (结果标签保留); 双击同样', () =>
  withMocks({ name: 'gw', portOffset: 163 }, async ({ page, root, press }) => {
    seed(root); await open(page, root);
    await search(page, press, 'needle');
    await waitDone(page, 5);
    const row = await page.evaluate(() => getActiveTab().lines.findIndex((l) => /b[\\/]f\.txt\(2,1\): /.test(l)));
    assert.ok(row > 0, '找到 b/f.txt 的命中行');
    await page.evaluate((r) => { const t = getActiveTab(); t.cursor = { row: r, col: 0 }; renderVim(t); }, row);
    await page.locator('#ed-content').focus();
    await press('Enter');
    await page.waitForFunction(() => getActiveTab() && /b[\\/]f\.txt$/.test(getActiveTab().path), null, { timeout: 8000 });
    assert.equal(await page.evaluate(() => getActiveTab().cursor.row), 1, '落在命中的第 2 行');
    assert.equal(await page.evaluate(() => [...TABS.values()].some((t) => t.grep)), true, '结果标签仍在');
    // 切回结果标签, 双击另一处
    await page.evaluate(() => setActiveTab([...TABS.values()].find((t) => t.grep).id));
    const row2 = await page.evaluate(() => getActiveTab().lines.findIndex((l) => /c[\\/]f\.txt\(2,1\): /.test(l)));
    await page.evaluate((r) => { const t = getActiveTab(); t.cursor = { row: r, col: 0 }; renderVim(t); }, row2);
    await page.evaluate(() => document.getElementById('ed-content').dispatchEvent(new MouseEvent('dblclick', { bubbles: true })));   // 双击监听 -> 光标所在行的命中处
    await page.waitForFunction(() => getActiveTab() && /c[\\/]f\.txt$/.test(getActiveTab().path), null, { timeout: 8000 });
  }));

test('Esc 停止: 不再追加, 尾行写明中断; 每次搜索各开一个标签', () =>
  withMocks({ name: 'gw', portOffset: 164 }, async ({ page, root, press }) => {
    seed(root); await open(page, root);
    await page.route('**/api/search', async (route) => { await new Promise((r) => setTimeout(r, 700)); route.continue(); });
    await search(page, press, 'needle');
    await page.waitForFunction(() => { const t = getActiveTab(); return t && t.grep && t.lines.some((l) => /needle_/.test(l)); }, null, { timeout: 8000 });
    await page.locator('#ed-content').focus();
    await press('Escape');
    await page.waitForFunction(() => getActiveTab().lines.some((l) => /中断/.test(l)), null, { timeout: 8000 });
    const n1 = await page.evaluate(() => getActiveTab().lines.length);
    await page.waitForTimeout(2200);
    assert.equal(await page.evaluate(() => getActiveTab().lines.length), n1, '停止后不再增加');
    await page.unroute('**/api/search');
    await search(page, press, 'ZETA');
    await waitDone(page, 5);
    const names = await page.evaluate(() => [...TABS.values()].filter((t) => t.grep).map((t) => t.name));
    assert.deepEqual(names, ['Grep: needle', 'Grep: ZETA'], '两次搜索两个标签');
  }));
