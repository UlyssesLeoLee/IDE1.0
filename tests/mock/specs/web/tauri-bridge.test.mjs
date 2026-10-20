// 回归: Tauri 桌面通道 (用 __TAURI_INTERNALS__ 仿真) — Grep 不再显示 undefined; 错误是字符串时也能读出原因
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('Tauri: Grep 条件被包成 {options}, 裸数组结果被正确读取 (不再「搜索失败: undefined」)', () =>
  withMocks({ name: 'tb', portOffset: 80, mock: { tauri: true } }, async ({ page, root, m, press }) => {
    await page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
    assert.equal(await page.evaluate(() => inTauri), true);
    await page.locator('#ed-content').click();
    await press('Control+g'); await page.fill('#grep-input', 'hello'); await press('Enter');
    await page.waitForFunction(() => /hello/.test((document.getElementById('grep-results') || document.getElementById('md-body') || {}).innerText || ''), null, { timeout: 8000 }).catch(() => {});
    const text = await page.evaluate(() => document.getElementById('grep-results') ? document.getElementById('grep-results').innerText : document.getElementById('md-body').innerText);
    assert.match(text, /hello/);
    assert.doesNotMatch(text, /undefined/);
    const ipc = await m.tauri.ipc();
    const call = ipc.find((c) => c.name === 'search_cmd');
    assert.ok(call && call.args && call.args.options && call.args.options.pattern === 'hello', '参数形如 {options:{pattern}}');
  }));

test('Tauri: invoke 失败 reject 字符串时, 界面显示真实原因而不是 undefined', () =>
  withMocks({ name: 'tb', portOffset: 81, mock: { tauri: true } }, async ({ page, press }) => {
    // 不打开项目 → 后端报「尚未打开项目」(字符串 reject)
    await page.locator('#ed-content').click();
    await press('Control+g'); await page.fill('#grep-input', 'x'); await press('Enter'); await page.waitForTimeout(1200);
    const status = await page.evaluate(() => (document.getElementById('grep-status') || document.getElementById('md-body')).innerText);
    assert.doesNotMatch(status, /undefined/);
  }));

test('pick_folder 替身: 取消 / 指定目录 / 记录调用', () =>
  withMocks({ name: 'tb', portOffset: 82 }, async ({ page, root, m }) => {
    await m.folder.next(root);
    const r1 = await page.evaluate(async () => { const r = await fetch('/api/pick_folder'); return r.status + ' ' + (await r.text()); });
    assert.match(r1, /^200/);
    await m.folder.next(null);
    const r2 = await page.evaluate(async () => (await (await fetch('/api/pick_folder')).text()));
    assert.equal(r2, 'null');
  }));
