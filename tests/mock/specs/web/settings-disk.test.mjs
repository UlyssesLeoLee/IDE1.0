// 回归: 设置落盘 —— ide10.* 的本地设置镜像到磁盘文件 (API Key 不上盘); 启动时磁盘上的值覆盖本地; 导入 / 导出
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const api = (page, body) => page.evaluate(async (b) => (await fetch('/api/settings', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(b) })).json(), body);

test('写入本地设置会镜像到磁盘; 删除也镜像; API Key 不上盘', () =>
  withMocks({ name: 'sd', portOffset: 607 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      localStorage.setItem('ide10.view', '{"toolbar":true}');
      localStorage.setItem('ide10.ai', '{"key":"SECRET"}');
      localStorage.setItem('ide10.tmp', 'x'); localStorage.removeItem('ide10.tmp');
    });
    await page.waitForTimeout(900);
    const all = (await api(page, { op: 'all' })).settings;
    assert.equal(all['ide10.view'], '{"toolbar":true}');
    assert.ok(!('ide10.ai' in all), '密钥不上盘');
    assert.ok(!('ide10.tmp' in all), '删除也同步');
  }));

test('启动时磁盘覆盖本地并重载一次: 偏好按磁盘上的值初始化', () =>
  withMocks({ name: 'sd', portOffset: 608 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await api(page, { op: 'set', key: 'ide10.settings', value: '{"tabWidth":8}' });
    await page.reload();
    await page.waitForFunction(() => typeof PREFS !== 'undefined' && PREFS.tabWidth === 8, null, { timeout: 15000 });
    assert.equal(await page.evaluate(() => localStorage.getItem('ide10.settings')), '{"tabWidth":8}');
  }));

test('导出 / 导入: 合并进现有设置, 不合法的键被跳过', () =>
  withMocks({ name: 'sd', portOffset: 609 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await api(page, { op: 'set', key: 'ide10.a', value: '1' });
    const ex = (await api(page, { op: 'export' })).json;
    assert.match(ex, /"ide10\.a": "1"/);
    const r = await api(page, { op: 'import', json: '{"ide10.a":"2","ide10.b":"3","evil":"x"}' });
    assert.deepEqual([r.imported, r.skipped], [2, 1]);
    const all = (await api(page, { op: 'all' })).settings;
    assert.deepEqual([all['ide10.a'], all['ide10.b']], ['2', '3']);
    const bad = await page.evaluate(async () => (await fetch('/api/settings', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ op: 'set', key: 'other.k', value: 'x' }) })).status);
    assert.equal(bad, 400, '非 ide10. 的键被拒绝');
  }));
