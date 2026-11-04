// 回归: 打开文件可带 路径:行:列; 有未保存修改时关窗口 / 刷新要先提示 (网页版 beforeunload)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

test('splitPathLineCol: 路径 / 路径:行 / 路径:行:列; 盘符不当行号', () =>
  withMocks({ name: 'oc', portOffset: 592 }, async ({ page }) => {
    const r = await page.evaluate(() => ['a.txt', 'src/a.rs:12', 'src/a.rs:12:5', 'C:/x/a.txt', 'C:/x/a.txt:3:4', 'D:5', ':7'].map(splitPathLineCol));
    assert.deepEqual(r, [
      { path: 'a.txt', line: 0, col: 0 }, { path: 'src/a.rs', line: 12, col: 1 }, { path: 'src/a.rs', line: 12, col: 5 },
      { path: 'C:/x/a.txt', line: 0, col: 0 }, { path: 'C:/x/a.txt', line: 3, col: 4 }, { path: 'D:5', line: 0, col: 0 }, { path: ':7', line: 0, col: 0 },
    ]);
  }));

test('打开文件时写 路径:行:列, 光标落在那里', () =>
  withMocks({ name: 'oc', portOffset: 593 }, async ({ page, root }) => {
    fs.writeFileSync(path.join(root, 'g.txt'), 'one\ntwo\nthree three\nfour\n');
    await page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
    await page.locator('#ed-content').click();
    await page.evaluate(() => { window.__mock.answers.prompt.push('g.txt:3:4'); });
    await page.evaluate(() => { promptOpenFile(); });
    await page.waitForFunction(() => /g\.txt$/.test(getActiveTab().path || '') && getActiveTab().cursor.row === 2, null, { timeout: 10000 });
    assert.equal(await page.evaluate(() => getActiveTab().cursor.col), 3, '第 4 列 = 下标 3');
  }));

test('有未保存修改: 关页面触发 beforeunload 提示; 没有修改不提示', () =>
  withMocks({ name: 'oc', portOffset: 594 }, async ({ page, app }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.dirty = false; });
    const evClean = await page.evaluate(() => { const e = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(e); return e.defaultPrevented; });
    assert.equal(evClean, false, '干净: 不拦');
    await page.evaluate(() => { const t = getActiveTab(); t.dirty = true; });
    const evDirty = await page.evaluate(() => { const e = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(e); return e.defaultPrevented; });
    assert.equal(evDirty, true, '有未保存修改: 拦下并让浏览器提示');
    await page.evaluate(() => { const t = getActiveTab(); t.dirty = false; });
  }));
