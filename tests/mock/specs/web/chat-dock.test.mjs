// 回归: Chat 右栏 + Agent API —— 不打开就不存在 (零成本); 打开后占自己的栅格列不盖编辑区; 模型经工具行动,
// 写类工具只产出补丁提案, 用户点"应用"才改文档 (一步撤销); 文档内容外发要先确认
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('不打开 Chat 时没有任何 DOM / 工具表请求; 打开后编辑区让位', () =>
  withMocks({ name: 'chat', portOffset: 577 }, async ({ page }) => {
    const calls = [];
    page.on('request', (r) => { if (r.url().includes('/api/agent')) calls.push(r.postData()); });
    await page.locator('#ed-content').click();
    await page.waitForTimeout(500);
    assert.equal(await page.evaluate(() => !!document.getElementById('chat-dock')), false, '未打开: 没有 DOM');
    assert.equal(calls.length, 0, '未打开: 没有 agent 请求');
    const w0 = await page.evaluate(() => document.getElementById('editor-area').getBoundingClientRect().width);
    await page.click('#chat-toggle');
    await page.waitForSelector('#chat-dock', { state: 'visible' });
    const r = await page.evaluate(() => ({ e: document.getElementById('editor-area').getBoundingClientRect(), c: document.getElementById('chat-dock').getBoundingClientRect() }));
    assert.ok(r.e.width < w0 - 200, '编辑区变窄让位');
    assert.ok(r.e.right <= r.c.left + 1, '不重叠');
    await page.click('#chat-close');
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('chat-dock')).display), 'none', '可折叠');
  }));

test('模型调工具 -> 补丁提案 -> 用户应用 (撤销可还原); 外发前确认', () =>
  withMocks({ name: 'chat', portOffset: 578 }, async ({ page }) => {
    let asked = 0, sentCtx = '';
    await page.route('https://llm.test/v1/chat/completions', async (route) => {
      const body = JSON.parse(route.request().postData());
      asked++;
      sentCtx = body.messages[0].content;
      const last = body.messages[body.messages.length - 1];
      const reply = last.role === 'tool'
        ? { role: 'assistant', content: '已提交补丁提案' }
        : { role: 'assistant', content: null, tool_calls: [{ id: 'c1', type: 'function', function: { name: 'propose_patch', arguments: JSON.stringify({ start: 1, remove: 1, insert: ['BETA!'], total_rows: 3, reason: '加感叹号' }) } }] };
      await route.fulfill({ status: 200, contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify({ choices: [{ message: reply }] }) });
    });
    await page.route('https://llm.test/**', (route) => route.request().method() === 'OPTIONS'
      ? route.fulfill({ status: 204, headers: { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*', 'access-control-allow-methods': '*' } }) : route.fallback());
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      localStorage.setItem('ide10.ai', JSON.stringify({ baseUrl: 'https://llm.test/v1', model: 'm', key: 'k' }));
      const t = getActiveTab(); t.lines = ['alpha', 'beta', 'gamma']; t.dirty = false; t.cursor = { row: 1, col: 0 };
      t.sel = { r0: 1, c0: 0, r1: 1, c1: 4 }; renderVim(t);
    });
    await page.evaluate(() => window.__mock.answers.confirm.push(true));   // 外发确认框: 允许
    await page.click('#chat-toggle');
    await page.fill('#chat-input', '把这行改一下');
    await page.press('#chat-input', 'Enter');
    await page.waitForSelector('.chat-proposal', { timeout: 10000 });
    const dl = await page.evaluate(() => window.__mock.dialogs.filter((d) => d.kind === 'confirm' && /AI 端点/.test(d.msg)).length);
    assert.equal(dl, 1, '外发选中内容前确认了一次');
    assert.match(sentCtx, /beta/, '选中内容随请求发出');
    assert.doesNotMatch(sentCtx, /alpha|gamma/, '只发选中的部分, 不发整篇');
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), ['alpha', 'beta', 'gamma'], '提案阶段文档不变');
    await page.click('.chat-proposal button:first-of-type');
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), ['alpha', 'BETA!', 'gamma'], '应用后才改文档');
    await page.evaluate(() => { document.getElementById('ed-content').focus(); });
    await page.keyboard.press('Control+z');
    await page.waitForFunction(() => getActiveTab().lines[1] === 'beta', null, { timeout: 8000 });
  }));
