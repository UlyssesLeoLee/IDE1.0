// 回归: Markdown 工具条 (REQ FR-005..FR-025) —— 只在 Markdown 文档里出现 (INV-03), 按钮走 md-* 命令改文本,
// 预览只读且不改文档 (INV-02), 脚本 / 不安全链接被转义
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = (page, name, lines) => page.evaluate(([n, l]) => {
  const t = getActiveTab(); t.name = n; t.path = ''; t.lines = l.slice(); t.cursor = { row: 0, col: 0 }; t.sel = null; t.dirty = false; renderVim(t);
}, [name, lines]);

test('非 Markdown 文档没有工具条 DOM; Markdown 文档出现并带 Sakura 快捷键提示', () =>
  withMocks({ name: 'mdbar', portOffset: 572 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await setup(page, 'a.txt', ['x']);
    assert.equal(await page.evaluate(() => !!document.getElementById('md-bar')), false, '纯文本零成本');
    await setup(page, 'a.md', ['hello world']);
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('md-bar')).display), 'flex');
    assert.equal(await page.evaluate(() => document.documentElement.getAttribute('data-md')), 'on');
    const tip = await page.evaluate(() => document.querySelector('#toolbar .tb-btn[data-cmd="save"]').title);
    assert.match(tip, /Ctrl\+S/, 'Sakura 工具栏按钮写出快捷键');
    await setup(page, 'b.txt', ['x']);
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('md-bar')).display), 'none', '切回非 MD 隐藏');
  }));

test('按钮 / 快捷键走同一条 md 命令; 预览只读不改文档, 脚本被转义', () =>
  withMocks({ name: 'mdbar', portOffset: 573 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await setup(page, 'n.md', ['hello world', '<script>alert(1)</script>', '[x](javascript:alert(1))']);
    await page.evaluate(() => { const t = getActiveTab(); t.sel = { r0: 0, c0: 0, r1: 0, c1: 5 }; t.cursor = { row: 0, col: 5 }; renderVim(t); });
    await page.click('#md-bar [data-md="md-bold"]');
    await page.waitForFunction(() => getActiveTab().lines[0] === '**hello** world', null, { timeout: 8000 });
    await page.evaluate(() => { const t = getActiveTab(); t.sel = null; t.cursor = { row: 0, col: 0 }; renderVim(t); });
    await page.click('#md-bar [data-md="md-h2"]');
    await page.waitForFunction(() => getActiveTab().lines[0] === '## **hello** world', null, { timeout: 8000 });

    const before = await page.evaluate(() => getActiveTab().lines.slice());
    await page.click('#md-bar [data-md="preview"]');
    await page.waitForSelector('#md-preview .md-body-view h2', { timeout: 8000 });
    const html = await page.evaluate(() => document.getElementById('md-preview').innerHTML);
    assert.ok(!html.includes('<script'), '脚本被转义');
    assert.ok(!html.includes('href="javascript'), '不安全链接不成链接');
    assert.match(html, /<strong>hello<\/strong>/);
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), before, '预览不改文档');
    assert.equal(await page.evaluate(() => getActiveTab().dirty), true, '(之前的编辑已使文档变脏, 预览本身不再改动)');
    await page.click('#md-bar [data-md="preview"]');
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('md-preview')).display), 'none', '再点切回源码');
  }));
