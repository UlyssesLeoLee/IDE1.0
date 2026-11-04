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

test('doc-graph 插件 (command 贡献点, FR-047): 画标题树 + 跨章节引用, 点节点跳转; 禁用后按钮和命令都消失, 文本编辑不受影响', () =>
  withMocks({ name: 'plug', portOffset: 587 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.waitForFunction(() => typeof PLUG !== 'undefined' && PLUG.commands && PLUG.commands['plugin:doc-graph:show'], null, { timeout: 8000 });
    await page.evaluate(() => {
      const t = getActiveTab(); t.name = 'g.md'; t.path = ''; t.cursor = { row: 0, col: 0 }; t.sel = null;
      t.lines = ['# Intro', 'see [[Usage]]', '## Setup', 'x', '# Usage', 'y', '```', '# not a heading', '```'];
      renderVim(t);
    });
    await page.click('#md-bar [data-md="plugin:doc-graph:show"]');
    await page.waitForSelector('#plug-out svg .gnode', { timeout: 15000 });
    assert.equal(await page.evaluate(() => document.querySelectorAll('#plug-out .gnode').length), 3, '三个标题(围栏里的不算)');
    assert.match(await page.evaluate(() => document.getElementById('plug-sum').textContent), /3 个标题, 1 条跨章节引用/);
    assert.equal(await page.evaluate(() => document.querySelectorAll('iframe[sandbox]').length), 1, '跑在沙箱 iframe 里');
    // 点 Usage 节点: 跳到第 5 行
    await page.evaluate(() => { const g = [...document.querySelectorAll('#plug-out .gnode')].find((x) => /Usage/.test(x.textContent)); g.dispatchEvent(new MouseEvent('click', { bubbles: true })); });
    await page.waitForFunction(() => getActiveTab().cursor.row === 4, null, { timeout: 8000 });
    // 禁用: 命令和按钮消失
    await page.evaluate(async () => { await plugApi({ op: 'toggle', id: 'doc-graph', enabled: false }); await plugRefresh(); });
    assert.equal(await page.evaluate(() => !!PLUG.commands['plugin:doc-graph:show']), false);
    assert.equal(await page.evaluate(() => getComputedStyle(document.querySelector('#md-bar [data-md="plugin:doc-graph:show"]')).display), 'none');
    // 文本主链路照常
    await page.evaluate(() => { const t = getActiveTab(); t.cursor = { row: 0, col: 0 }; renderVim(t); });
    await page.locator('#ed-content').focus();
    await page.keyboard.type('Z');
    await page.waitForFunction(() => getActiveTab().lines[0].startsWith('Z'), null, { timeout: 8000 });
  }));
