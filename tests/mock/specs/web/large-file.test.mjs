// 回归: 大文件窗口化渲染 — 5 万行文档下 DOM 只含可视窗口附近的行; 键入快; 滚动/跳转/点击/选择/查找/书签都对
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const N = 60000;
const load = (page) => page.evaluate((n) => {
  const t = getActiveTab(); t.lines = Array.from({ length: n }, (_, i) => 'line ' + i + ' some text here for testing'); t.cursor = { row: 0, col: 0 }; t.sel = null; t.dirty = false;
  edContent.scrollTop = 0; renderVim(t);
}, N);
const dom = (page) => page.evaluate(() => ({ rows: document.querySelectorAll('#ed-content > .hl-row').length, total: document.getElementById('ed-content').scrollHeight, start: VW.start, end: VW.end, on: VW.on }));

test('5 万行: DOM 只有窗口附近的行, 滚动条高度仍是 行数×行高', () =>
  withMocks({ name: 'lf', portOffset: 270 }, async ({ page }) => {
    await page.locator('#ed-content').click(); await load(page);
    const d = await dom(page);
    assert.equal(d.on, true);
    assert.ok(d.rows < 600, 'DOM 行数很少: ' + d.rows);
    assert.ok(d.total >= N * 18, `滚动高度 ${d.total} ≥ ${N * 18}`);
    // 行号栏与正文对齐: 窗口内第一行的行号 = start+1
    assert.equal(await page.evaluate(() => document.querySelector('#gutter .vgn').textContent.split('\n')[0]), String((await dom(page)).start + 1));
  }));

test('Ctrl+End / Ctrl+Home / 滚轮 / PgDn: 光标行总在窗口里且可见, 状态栏行号正确', () =>
  withMocks({ name: 'lf', portOffset: 271 }, async ({ page, snap, press }) => {
    await page.locator('#ed-content').click(); await load(page); await page.locator('#ed-content').focus();
    await press('Control+End'); await page.waitForTimeout(250);
    let s = await snap();
    assert.equal(s.cursor.row, N - 1);
    let d = await dom(page); assert.ok(d.end >= N - 1 && d.rows < 600, JSON.stringify(d));
    assert.equal(await page.evaluate(() => document.getElementById('caret').classList.contains('on')), true, '光标可见');
    await press('Control+Home'); await page.waitForTimeout(250);
    assert.equal((await snap()).cursor.row, 0); d = await dom(page); assert.equal(d.start, 0);
    await press('PageDown', { times: 5 }); await page.waitForTimeout(250);
    assert.ok((await snap()).cursor.row > 80);
    await page.mouse.move(400, 300); await page.mouse.wheel(0, 90000); await page.waitForTimeout(400);
    d = await dom(page); assert.ok(d.start > 3000, '滚轮滚到很下面后窗口跟着移动: ' + JSON.stringify(d));
    const firstVisible = await page.evaluate(() => { const e = document.getElementById('ed-content'); return Math.floor(e.scrollTop / VW.lh); });
    assert.ok(firstVisible >= d.start && firstVisible < d.end, '可视首行在窗口内');
  }));

test('在中间键入: 打字立即生效且耗时短; 点击落光标按行号正确', () =>
  withMocks({ name: 'lf', portOffset: 272 }, async ({ page, snap, press }) => {
    await page.locator('#ed-content').click(); await load(page); await page.locator('#ed-content').focus();
    await page.evaluate(() => { const t = getActiveTab(); t.cursor = { row: 30000, col: 4 }; edContent.scrollTop = 30000 * 18 - 200; });
    await page.waitForTimeout(300);
    const ms = await page.evaluate(async () => { const a = performance.now(); insertChar('X'); await new Promise((r) => requestAnimationFrame(r)); return performance.now() - a; });
    assert.equal(await page.evaluate(() => getActiveTab().lines[30000]), 'lineX 30000 some text here for testing');
    assert.ok(ms < 120, '键入耗时 ' + Math.round(ms) + 'ms (非窗口化 5 万行约 300ms)');
    // 点击窗口里的一行: 落点行号 = 该行真实行号
    const y = await page.evaluate(() => { const r = document.querySelectorAll('#ed-content > .hl-row')[Math.floor(document.querySelectorAll('#ed-content > .hl-row').length / 2)]; const b = r.getBoundingClientRect(); return { x: b.left + 30, y: b.top + b.height / 2, text: r.textContent }; });
    await page.mouse.click(y.x, y.y); await page.waitForTimeout(150);
    const c = await snap();
    assert.equal(await page.evaluate(() => getActiveTab().lines[getActiveTab().cursor.row]), y.text, '点击落在被点的那一行');
  }));

test('选择 / 查找 / 书签 / 删除行 在大文件里正确 (跨窗口选择 + 行号栏标记)', () =>
  withMocks({ name: 'lf', portOffset: 273 }, async ({ page, snap, press }) => {
    await page.locator('#ed-content').click(); await load(page); await page.locator('#ed-content').focus();
    await press('Control+End'); await page.waitForTimeout(150);
    await press('Shift+Control+Home'); await page.waitForTimeout(400);   // 一口气选到开头 (选区跨出窗口)
    const s = await snap(); assert.deepEqual(s.sel.slice(0, 2), [0, 0]); assert.equal(s.sel[2], N - 1);
    assert.equal(await page.evaluate(() => selectionText(getActiveTab()).split('\n').length), N);
    await press('ArrowLeft');                                             // 收缩到选区开头
    await page.evaluate(() => { const t = getActiveTab(); t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t); });
    // 查找跳到很靠后的一处
    await press('Control+f'); await page.fill('#find-input', 'line 45000 '); await press('Enter'); await page.waitForTimeout(500);
    assert.equal((await snap()).sel[0], 45000);
    assert.equal(await page.evaluate(() => { const e = document.getElementById('ed-content'); const t = getActiveTab(); return t.cursor.row * 18 >= e.scrollTop - 5 && t.cursor.row * 18 <= e.scrollTop + e.clientHeight; }), true, '命中行已滚进可视区');
    await page.keyboard.press('Escape');
    // 书签标记出现在窗口内行号栏
    await page.evaluate(() => { toggleBookmark(); scheduleMarks(); }); await page.waitForTimeout(300);
    assert.equal(await page.evaluate(() => CSS.highlights.get('bookmark-gutter') ? CSS.highlights.get('bookmark-gutter').size : 0), 1);
    // Ctrl+Shift+E 删除整行后仍是 N-1 行, DOM 仍是窗口
    await page.locator('#ed-content').focus(); await press('Control+Shift+e'); await page.waitForTimeout(200);
    assert.equal(await page.evaluate(() => getActiveTab().lines.length), N - 1);
    assert.ok((await dom(page)).rows < 600);
  }));

test('缩回小文件后回到普通渲染 (单文本节点)', () =>
  withMocks({ name: 'lf', portOffset: 274 }, async ({ page }) => {
    await page.locator('#ed-content').click(); await load(page);
    assert.equal((await dom(page)).on, true);
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['a', 'b']; t.cursor = { row: 0, col: 0 }; paintActive(); });
    assert.equal(await page.evaluate(() => VW.on), false);
    assert.equal(await page.evaluate(() => document.getElementById('ed-content').childNodes.length), 1);
    assert.equal(await page.evaluate(() => document.getElementById('ed-content').classList.contains('vwin')), false);
  }));
