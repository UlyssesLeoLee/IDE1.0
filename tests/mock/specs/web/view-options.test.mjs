// 回归: 表示(视图)附件 — 工具栏 / 功能键栏 / 标尺 / 空白标记 / 当前行下划线 / 閲覧モード / 文件属性 / 窗口一览 / 关闭其它
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const box = (page, sel) => page.evaluate((s) => { const e = document.querySelector(s); if (!e) return null; const cs = getComputedStyle(e); if (cs.display === 'none') return 'hidden'; const b = e.getBoundingClientRect(); return [Math.round(b.left), Math.round(b.top), Math.round(b.right), Math.round(b.bottom)]; }, sel);
const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);

test('默认全关; 工具栏 / 功能键栏 / 标尺 打开后各占自己的一行, 编辑区不被盖住', () =>
  withMocks({ name: 'vo', portOffset: 170 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    for (const s of ['#toolbar', '#ruler', '#funckeys']) assert.equal(await box(page, s), 'hidden', s + ' 默认隐藏');
    const ed0 = await box(page, '#editor');
    await cmd(page, 'toggle-toolbar'); await cmd(page, 'toggle-ruler'); await cmd(page, 'toggle-funckey');
    const tb = await box(page, '#toolbar'), rl = await box(page, '#ruler'), fk = await box(page, '#funckeys'), ed = await box(page, '#editor');
    assert.ok(Array.isArray(tb) && Array.isArray(rl) && Array.isArray(fk));
    assert.ok(tb[3] <= rl[1] + 1 && rl[3] <= ed[1] + 1, '工具栏 → 标尺 → 编辑区 自上而下');
    assert.ok(ed[3] <= fk[1] + 1, '功能键栏在编辑区之下');
    assert.ok(ed[3] - ed[1] > 100, '编辑区仍有高度');
    assert.ok((await page.evaluate(() => document.querySelectorAll('#toolbar .tb-btn').length)) >= 10);
    assert.equal(await page.evaluate(() => document.querySelectorAll('#funckeys .fk').length), 12);
    assert.match(await page.evaluate(() => document.querySelector('#ruler .rl-in').textContent), /^\.{4}\+\.{4}\d/);
    // 关回去, 编辑区回到原位
    await cmd(page, 'toggle-toolbar'); await cmd(page, 'toggle-ruler'); await cmd(page, 'toggle-funckey');
    assert.deepEqual(await box(page, '#editor'), ed0);
  }));

test('工具栏按钮 / 功能键栏按钮 真的执行命令; 开关跨重载保留', () =>
  withMocks({ name: 'vo', portOffset: 171 }, async ({ page, snap, press }) => {
    await fill(page, 'abc');
    await cmd(page, 'toggle-toolbar'); await cmd(page, 'toggle-funckey');
    await press('Control+a');
    await page.locator('#funckeys .fk', { hasText: 'コピー' }).click();
    await page.locator('#toolbar .tb-btn', { hasText: '切り取り' }).click(); await page.evaluate(() => window.__editIdle());
    assert.equal((await snap()).lines[0], '');
    await page.locator('#toolbar .tb-btn', { hasText: '元に戻す' }).click(); await page.evaluate(() => window.__editIdle());
    assert.equal((await snap()).lines[0], 'abc');
    await page.reload(); await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => document.documentElement.getAttribute('data-v-toolbar')), 'on');
  }));

test('空白标记: TAB / 空格 / 全角空格 / 行尾空白 分别着色; 当前行下划线跟着光标', () =>
  withMocks({ name: 'vo', portOffset: 172 }, async ({ page, press }) => {
    await fill(page, 'x');
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['a\tb c　d  ']; renderVim(t); });
    await cmd(page, 'toggle-ws'); await page.waitForTimeout(300);
    const n = await page.evaluate(() => Object.fromEntries(['ws-tab', 'ws-space', 'ws-zen', 'ws-trail'].map((k) => [k, CSS.highlights.get(k) ? CSS.highlights.get(k).size : 0])));
    assert.deepEqual(n, { 'ws-tab': 1, 'ws-space': 1, 'ws-zen': 1, 'ws-trail': 2 });
    await cmd(page, 'toggle-ws'); await page.waitForTimeout(200);
    assert.equal(await page.evaluate(() => CSS.highlights.get('ws-tab') ? CSS.highlights.get('ws-tab').size : 0), 0);
    await fill(page, 'one\ntwo\nthree'); await press('Control+Home');
    await cmd(page, 'toggle-curline'); await press('ArrowDown'); await page.waitForTimeout(200);
    const y1 = await page.evaluate(() => parseFloat(document.getElementById('curline').style.top));
    await press('ArrowDown'); await page.waitForTimeout(200);
    const y2 = await page.evaluate(() => parseFloat(document.getElementById('curline').style.top));
    assert.ok(y2 - y1 >= 15, '光标下移一行, 下划线跟着下移: ' + y1 + ' → ' + y2);
  }));

test('閲覧モード: 键入 / 删除 / 粘贴 都被拒; 退出后恢复', () =>
  withMocks({ name: 'vo', portOffset: 173 }, async ({ page, snap, press, m }) => {
    await fill(page, 'abc');
    await cmd(page, 'toggle-readonly');
    await page.keyboard.type('XYZ'); await press('Backspace'); await press('Delete'); await press('Enter'); await press('Tab');
    await m.clipboard.set('P'); await press('Control+v'); await page.waitForTimeout(150);
    assert.deepEqual((await snap()).lines, ['abc']);
    await press('ArrowLeft');           // 光标仍可移动
    await cmd(page, 'toggle-readonly');
    await page.keyboard.type('Q');
    assert.equal((await snap()).lines[0].includes('Q'), true);
  }));

test('文件属性 (Alt+Enter) / 窗口一览 / 关闭其它标签 / F5 重绘', () =>
  withMocks({ name: 'vo', portOffset: 174 }, async ({ page, snap, press }) => {
    await fill(page, 'hello world\nline2');
    await press('Alt+Enter'); await page.waitForTimeout(150);
    const txt = await page.evaluate(() => document.getElementById('md-body').innerText);
    assert.match(txt, /行数\s*2/); assert.match(txt, /单词数\s*3/); assert.match(txt, /字符数\s*17/);
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    await press('Control+n'); await press('Control+n'); await page.waitForTimeout(200);
    assert.equal(await page.evaluate(() => TABS.size), 3);
    await cmd(page, 'window-list'); await page.waitForTimeout(150);
    assert.equal(await page.evaluate(() => document.querySelectorAll('#md-body .win-row').length), 3);
    await page.locator('#md-body .win-row').first().click(); await page.waitForTimeout(200);
    page.once('dialog', (d) => d.accept());
    await cmd(page, 'close-others'); await page.waitForTimeout(300);
    assert.equal(await page.evaluate(() => TABS.size), 1);
    await press('F5'); await page.waitForTimeout(100);
    assert.deepEqual((await snap()).lines.length >= 1, true);
  }));
