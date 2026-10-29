// 回归: 外部コマンド実行 —— 运行命令并捕获输出 (新标签 / 插入光标处), 选区可作标准输入; 沙箱: 没打开项目不能运行
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('没打开项目: 接口拒绝运行命令', () =>
  withMocks({ name: 'ex', portOffset: 595 }, async ({ page }) => {
    const r = await page.evaluate(async () => { const x = await fetch('/api/exec_cmd', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ command: 'echo hi' }) }); return x.status; });
    assert.equal(r, 400);
  }));

test('运行命令: 输出开成新标签; 选区作标准输入时过滤后插回光标处', () =>
  withMocks({ name: 'ex', portOffset: 596 }, async ({ page, root }) => {
    await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate(() => dispatchCommand('exec-cmd'));
    await page.waitForSelector('#ex-cmd');
    await page.fill('#ex-cmd', 'echo hello-exec');
    await page.press('#ex-cmd', 'Enter');
    await page.waitForFunction(() => /^出力/.test(getActiveTab().name || ''), null, { timeout: 15000 });
    assert.match(await page.evaluate(() => getActiveTab().lines.join('\n')), /hello-exec/);

    // 选区作标准输入: Windows 用 more, 其它用 cat; 输出替换选区
    const filter = process.platform === 'win32' ? 'more' : 'cat';
    await page.evaluate(() => { const t = [...TABS.values()].find((x) => !/^出力/.test(x.name)); setActiveTab(t.id); t.lines = ['alpha beta']; t.sel = { r0: 0, c0: 0, r1: 0, c1: 5 }; t.selAnchor = { row: 0, col: 0 }; t.cursor = { row: 0, col: 5 }; renderVim(t); });
    await page.evaluate(() => dispatchCommand('exec-cmd'));
    await page.waitForSelector('#ex-cmd');
    assert.equal(await page.evaluate(() => document.getElementById('ex-stdin').checked), true, '有选区: 默认勾上');
    await page.fill('#ex-cmd', filter);
    await page.check('input[name="ex-out"][value="insert"]');
    await page.press('#ex-cmd', 'Enter');
    await page.waitForFunction(() => getActiveTab().lines.join('\n').includes('alpha') && getActiveTab().lines.join('\n').includes('beta'), null, { timeout: 15000 });
  }));
