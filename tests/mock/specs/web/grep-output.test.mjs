// 回归: Grep 输出选项 —— 只文件名 / 只命中部分 / 每个文件只取第一处 / 相对路径; 相对路径结果的跳转要接上项目根
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
function seed(root) {
  fs.mkdirSync(path.join(root, 'go'), { recursive: true });
  fs.writeFileSync(path.join(root, 'go', 'a.txt'), 'x needle_1 y\nneedle_2 here\n');
  fs.writeFileSync(path.join(root, 'go', 'b.txt'), 'only needle_3\n');
}
const run = (page, out) => page.evaluate(async (o) => {
  const job = await startGrepJob({ out: o, needle: 'needle', caseSensitive: false, thisOnly: false, folders: ['go'], subfolders: true,
    base: { pattern: 'needle', case_sensitive: false, regex: false, whole_word: false, include_files: [], search_folders: ['go'], subfolders: true } });
  return { hits: job.hits, files: job.files.size, lines: getActiveTab().lines.slice() };
}, out);

test('标准 / 只文件名 / 只命中部分 / 每文件第一处 / 相对路径', () =>
  withMocks({ name: 'go', portOffset: 604 }, async ({ page, root }) => {
    seed(root); await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate((r) => { projectRoot = r; }, root);   // 前端记的项目根 (真实流程里由「打开文件夹」设置)
    const body = (r) => r.lines.filter((l) => !/^(□|検索|フォルダー|\d+ 個)/.test(l) && l.trim());
    let r = await run(page, { form: 'normal' });
    assert.equal(r.hits, 3);
    assert.equal(body(r).length, 3, '标准: 3 处各一行');
    r = await run(page, { form: 'files' });
    assert.equal(body(r).length, 2, '只文件名: 每个文件一行');
    assert.ok(body(r).every((l) => !/\(\d+,\d+\)/.test(l)), '没有 (行,列)');
    r = await run(page, { form: 'match' });
    assert.ok(body(r).every((l) => /: needle$/.test(l)), '只命中部分: 冒号后只剩命中词 ' + JSON.stringify(body(r)));
    r = await run(page, { form: 'normal', first: true });
    assert.equal(body(r).length, 2, '每个文件只取第一处');
    assert.equal(r.hits, 3, '命中数仍按真实命中计');
    r = await run(page, { form: 'normal', relative: true });
    assert.ok(body(r).every((l) => /^go.[ab]\.txt\(/.test(l)), '相对项目根的路径 ' + JSON.stringify(body(r)));
    // 相对路径结果的跳转: 接上项目根打开
    await page.evaluate(() => { const t = getActiveTab(); t.cursor = { row: t.lines.findIndex((l) => /^go.a\.txt\(/.test(l)), col: 0 }; });
    await page.evaluate(() => grepJumpAtCursor(getActiveTab()));
    await page.waitForFunction(() => /a\.txt$/.test(getActiveTab().path || ''), null, { timeout: 10000 });
  }));
