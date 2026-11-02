// 回归: 界面一轮 —— 侧栏默认无横向标签 / md 目录标签可关; 增量搜索是终端栏标签页且可再点关闭;
// 快捷按钮在标题栏; 首选项「AI 模型」页 + 聊天 effort; 按下方底色自动反色
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = (page, name, lines) => page.evaluate(([n, l]) => {
  const t = getActiveTab(); t.name = n; t.path = '/x/' + n; t.untitled = false; t.lines = l.slice(); t.cursor = { row: 0, col: 0 }; t.sel = null; t.dirty = false; renderVim(t);
}, [name, lines]);
const disp = (page, sel) => page.evaluate((s) => { const e = document.querySelector(s); return e ? getComputedStyle(e).display : null; }, sel);

test('侧栏默认没有横向标签条; md 文档出现可关闭的目录标签, 关掉不再自动开', () =>
  withMocks({ name: 'ui', portOffset: 620 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await setup(page, 'a.txt', ['x']);
    assert.equal(await disp(page, '#side-tabs'), 'none', '默认只有竖向导航');
    await setup(page, 'b.md', ['# t']);
    await page.evaluate(() => revealRegion('sideBar'));
    await page.waitForSelector('#side-tabs[data-extra="on"] .side-tab .st-x');
    assert.equal(await disp(page, '#side-tabs'), 'flex');
    await page.locator('#side-tabs .st-x').click();
    assert.equal(await disp(page, '#side-tabs'), 'none', '关掉后横向标签条消失');
    await setup(page, 'b.md', ['# t2']);
    assert.equal(await disp(page, '#side-tabs'), 'none', '用户关过的文档不再自动开');
  }));

test('增量搜索是终端栏的标签页: 自动打开, 点 × / 再点标签 / Esc 都能关', () =>
  withMocks({ name: 'ui', portOffset: 621 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => dispatchCommand('isearch-fwd'));
    await page.waitForSelector('#isearch-pane.active #is-input');
    assert.equal(await page.evaluate(() => document.getElementById('isearch-tab').hidden), false);
    await page.locator('#isearch-tab .st-x').click();
    assert.equal(await page.evaluate(() => document.getElementById('isearch-tab').hidden), true, '× 关闭');
    await page.evaluate(() => dispatchCommand('isearch-fwd'));
    await page.waitForSelector('#isearch-pane.active #is-input');
    await page.keyboard.press('Escape');
    assert.equal(await page.evaluate(() => document.getElementById('isearch-tab').hidden), true, 'Esc 关闭');
  }));

test('快捷按钮在最顶端标题栏里, 窗口控制钮网页版不显示; 活动栏再点一次收起侧栏', () =>
  withMocks({ name: 'ui', portOffset: 622 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const r = await page.evaluate(() => ({ tb: document.getElementById('toolbar').getBoundingClientRect().bottom, mb: document.getElementById('menubar').getBoundingClientRect().bottom, n: document.querySelectorAll('#menubar #toolbar .tb-btn').length }));
    assert.ok(r.n >= 1 && r.tb <= r.mb, '快捷按钮属于标题栏');
    assert.equal(await disp(page, '#win-ctl'), 'none', '网页版没有窗口控制钮');
    await setup(page, 'm.md', ['x']);
    assert.equal(await page.evaluate(() => document.querySelector('#viewbars-top #toolbar')), null, 'md 文档下面没有 Sakura 快捷按钮行');
    await page.evaluate(() => showSidePane('explorer'));
    await page.locator('.act-btn[data-pane="explorer"]').click();
    assert.equal(await page.evaluate(() => layoutFlag('sideBar')), false, '再点一次收起');
  }));

test('首选项「AI 模型」页保存接口 / 模型 / effort, 聊天窗只留 effort 选择', () =>
  withMocks({ name: 'ui', portOffset: 623 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => commonConfig('AI'));
    await page.waitForSelector('.cfg-tab.active');
    const inputs = page.locator('.cfg-row input[type="text"], .cfg-row input[type="password"]');
    await inputs.nth(1).fill('m-1'); await inputs.nth(1).dispatchEvent('change');
    await page.locator('.cfg-row select').selectOption('high');
    await page.locator('#md-foot button:first-child').click();
    assert.deepEqual(await page.evaluate(() => { const c = aiConfig(); return [c.model, c.effort]; }), ['m-1', 'high']);
    await page.evaluate(() => toggleChat(true));
    await page.waitForSelector('#chat-effort');
    assert.equal(await page.evaluate(() => !!document.getElementById('chat-ai') || !!document.getElementById('cai-model')), false, '聊天窗里没有模型设置');
    assert.equal(await page.locator('#chat-effort').inputValue(), 'high');
    assert.deepEqual(await page.evaluate(() => aiEffortField(aiConfig())), { reasoning_effort: 'high' });
  }));

test('自动反色看实际底色: 深色底块上的深色字被改成浅色', () =>
  withMocks({ name: 'ui', portOffset: 624 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const r = await page.evaluate(() => {
      const root = document.createElement('div'); root.style.cssText = 'background:#ffffff;color:#000';
      root.innerHTML = '<div id="blk" style="background:#101010"><span id="sp" style="color:#000">hi</span></div><span id="ok" style="color:#000">ok</span>';
      document.body.appendChild(root); fixContrast(root);
      const o = { sp: getComputedStyle(root.querySelector('#sp')).color, ok: getComputedStyle(root.querySelector('#ok')).color }; root.remove(); return o;
    });
    assert.notEqual(r.sp, 'rgb(0, 0, 0)', '黑底块上的黑字被反成浅色');
    assert.equal(r.ok, 'rgb(0, 0, 0)', '白底上的黑字保持');
  }));

test('effort 档位从供应商模型接口取 (数组 / supported 对象 / 列表里找本模型), 取不到才用默认, 也可自定义', () =>
  withMocks({ name: 'ui', portOffset: 649 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const ex = await page.evaluate(() => ({
      arr: extractEfforts({ id: 'm', supported_reasoning_efforts: ['low', 'high', 'xhigh'] }),
      obj: extractEfforts({ capabilities: { effort: { supported: true, low: { supported: true }, medium: { supported: true }, high: { supported: false }, max: { supported: true } } } }),
      lv: extractEfforts({ reasoning_effort: { levels: ['minimal', 'medium'] } }),
      none: extractEfforts({ id: 'm', context_window: 1 }),
    }));
    assert.deepEqual(ex.arr, ['low', 'high', 'xhigh']);
    assert.deepEqual(ex.obj, ['low', 'medium', 'max']);
    assert.deepEqual(ex.lv, ['minimal', 'medium']);
    assert.deepEqual(ex.none, []);
    // 列表接口: 找本模型那一项 (别的模型的档位不算)
    await page.route('https://api.vendor.test/v1/**', (route) => route.fulfill({ status: 200, headers: { 'access-control-allow-origin': '*', 'content-type': 'application/json' },
      body: JSON.stringify(route.request().url().endsWith('/models') ? { data: [{ id: 'other', efforts: ['low'] }, { id: 'mine', capabilities: { reasoning_effort: ['low', 'medium', 'high', 'max'] } }] } : {}) }));
    const got = await page.evaluate(async () => { aiSaveConfig({ baseUrl: 'https://api.vendor.test/v1', model: 'mine', key: 'k', effort: '' }); return aiFetchEfforts(); });
    assert.deepEqual(got, ['low', 'medium', 'high', 'max']);
    // 存下来之后聊天窗和首选项的下拉用这些档位
    await page.evaluate((l) => { aiSaveConfig(Object.assign(aiConfig(), { efforts: l, effort: 'max' })); toggleChat(true); chatModelLabel(); }, got);
    await page.waitForSelector('#chat-effort');
    assert.deepEqual(await page.evaluate(() => [...document.querySelectorAll('#chat-effort option')].map((o) => o.value)), ['', 'low', 'medium', 'high', 'max']);
    assert.equal(await page.locator('#chat-effort').inputValue(), 'max');
    // 没公布档位 -> 抛错提示手动填; 没有存档位 -> 通用默认
    await page.route('https://api.none.test/v1/**', (route) => route.fulfill({ status: 200, headers: { 'access-control-allow-origin': '*', 'content-type': 'application/json' }, body: '{"data":[]}' }));
    const err = await page.evaluate(async () => { aiSaveConfig({ baseUrl: 'https://api.none.test/v1', model: 'x', key: 'k' }); try { await aiFetchEfforts(); return 'ok'; } catch (e) { return e.message; } });
    assert.match(err, /没有公布 effort 档位.*手动/);
    assert.deepEqual(await page.evaluate(() => aiEffortList(aiConfig())), ['minimal', 'low', 'medium', 'high']);
  }));
