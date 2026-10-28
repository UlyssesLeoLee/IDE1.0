// 回归: 插件系统 —— 默认插件 Pele 在沙箱里把 ```mermaid 渲染成 SVG (懒加载), 扩展面板列出插件,
// 禁用后回退成普通代码块, 冲突偏好 (内建优先) 生效, 非 Markdown 不加载任何插件
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = (page, name, lines) => page.evaluate(([n, l]) => {
  const t = getActiveTab(); t.name = n; t.path = ''; t.lines = l.slice(); t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t);
}, [name, lines]);

test('Pele 默认插件: 沙箱里渲染 mermaid, 禁用后回退, 偏好=内建时不调用插件', () =>
  withMocks({ name: 'plug', portOffset: 575 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    // 没用到之前没有任何沙箱 iframe (懒加载)
    assert.equal(await page.evaluate(() => document.querySelectorAll('iframe[sandbox]').length), 0);
    await page.waitForFunction(() => typeof PLUG !== 'undefined' && PLUG.list, null, { timeout: 8000 });
    assert.ok(await page.evaluate(() => PLUG.list.some((p) => p.manifest.id === 'pele' && p.enabled && p.source === 'bundled')), '默认插件 pele');

    await setup(page, 'g.md', ['# t', '', '```mermaid', 'graph TD', '  A-->B', '```']);
    await page.click('#md-bar [data-md="preview"]');
    await page.waitForSelector('#md-preview .md-fence-out svg', { timeout: 20000 });
    const sb = await page.evaluate(() => [...document.querySelectorAll('iframe[sandbox]')].map((f) => f.getAttribute('sandbox')));
    assert.deepEqual(sb, ['allow-scripts'], '沙箱 iframe: 只给 allow-scripts');

    // 禁用 -> 热卸载, 预览回退成普通代码块
    await page.evaluate(async () => { await plugApi({ op: 'toggle', id: 'pele', enabled: false }); await plugRefresh(); });
    assert.equal(await page.evaluate(() => document.querySelectorAll('iframe[sandbox]').length), 0, '禁用立即卸载沙箱');
    await page.evaluate(() => renderMdPreview());
    await page.waitForSelector('#md-preview pre.md-fence', { timeout: 8000 });
    assert.equal(await page.evaluate(() => document.querySelectorAll('#md-preview .md-fence-out svg').length), 0);

    // 启用 + 偏好=内建优先 -> 不调用插件, 仍是代码块
    await page.evaluate(async () => { await plugApi({ op: 'toggle', id: 'pele', enabled: true }); plugSetPref('markdown.fence:mermaid', 'native'); await plugRefresh(); renderMdPreview(); });
    await page.waitForTimeout(1500);
    assert.equal(await page.evaluate(() => document.querySelectorAll('#md-preview .md-fence-out svg').length), 0, '偏好内建: 保持代码块');
    assert.equal(await page.evaluate(() => document.querySelectorAll('iframe[sandbox]').length), 0, '没有调用插件');
  }));

test('扩展面板列出插件并能显示不匹配报告入口', () =>
  withMocks({ name: 'plug', portOffset: 576 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => showSidePane('extensions'));
    await page.waitForSelector('#ext-body .ext-row', { timeout: 8000 });
    const txt = await page.evaluate(() => document.getElementById('ext-body').textContent);
    assert.match(txt, /Pele/);
    assert.match(txt, /检查更新/);
    assert.ok(await page.evaluate(() => !!document.getElementById('ext-inspect')), '有 GitHub 安装入口');
  }));
