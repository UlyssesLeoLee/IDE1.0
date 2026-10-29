// 回归: 桌面版用系统「打开 / 另存为」对话框 (Tauri 替身按脚本回答); 取消不做事; 没有系统对话框 (网页版) 仍是沙箱内输入框
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('系统打开对话框: 选定的文件被打开 (可多选), 取消什么也不做, 不再弹输入框', () =>
  withMocks({ name: 'nd', portOffset: 599, mock: { tauri: true } }, async ({ page, root }) => {
    fs.writeFileSync(path.join(root, 'a.txt'), 'AAA\n'); fs.writeFileSync(path.join(root, 'b.txt'), 'BBB\n');
    await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate((ps) => { window.__mock.native.open.push(ps); }, [path.join(root, 'a.txt'), path.join(root, 'b.txt')]);
    await page.evaluate(() => promptOpenFile());
    await page.waitForFunction(() => [...TABS.values()].filter((t) => /[ab]\.txt$/.test(t.path || '')).length === 2, null, { timeout: 10000 });
    assert.equal(await page.evaluate(() => window.__mock.dialogs.filter((d) => d.kind === 'prompt').length), 0, '没有弹输入框');
    // 取消 (空数组): 不新开标签
    const n = await page.evaluate(() => TABS.size);
    await page.evaluate(() => { window.__mock.native.open.push([]); });
    await page.evaluate(() => promptOpenFile());
    await page.waitForTimeout(500);
    assert.equal(await page.evaluate(() => TABS.size), n);
  }));

test('系统另存为对话框: 写到选定的路径并改绑标签; 取消则保持无标题', () =>
  withMocks({ name: 'nd', portOffset: 600, mock: { tauri: true } }, async ({ page, root }) => {
    await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.untitled = true; t.path = ''; t.lines = ['saved text']; t.dirty = true; renderVim(t); });
    await page.evaluate(() => { window.__mock.native.save.push(null); });                 // 取消
    assert.equal(await page.evaluate(() => saveActive()), false);
    assert.equal(await page.evaluate(() => getActiveTab().untitled), true, '取消后仍是无标题');
    await page.waitForTimeout(100);   // 另存为的"忙"标志在下一个宏任务才复位
    const target = path.join(root, 'out.txt');
    await page.evaluate((p) => { window.__mock.native.save.push(p); }, target);
    assert.equal(await page.evaluate(() => saveActive()), true);
    assert.equal(fs.readFileSync(target, 'utf8').replace(/\r\n/g, '\n'), 'saved text');
    assert.equal(await page.evaluate(() => getActiveTab().untitled), false);
    assert.equal(await page.evaluate(() => window.__mock.dialogs.filter((d) => d.kind === 'prompt').length), 0, '没有弹输入框');
  }));

test('启动参数里的文件 (路径:行:列): 启动后被打开并定位', async () => {
  // launch_files 在页面脚本跑之前就要有答案: 用 addInitScript 预置; 这里直接验证 gotoPathLine 路径 (与启动代码同一入口)
  await withMocks({ name: 'nd', portOffset: 603, mock: { tauri: true } }, async ({ page, root }) => {
    fs.writeFileSync(path.join(root, 'l.txt'), 'one\ntwo\nthree three\n');
    await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate(async (p) => { await gotoPathLine(p, 3, 3); }, path.join(root, 'l.txt'));
    await page.waitForFunction(() => /l\.txt$/.test(getActiveTab().path || '') && getActiveTab().cursor.row === 2, null, { timeout: 10000 });
    assert.equal(await page.evaluate(() => getActiveTab().cursor.col), 3);
  });
});
