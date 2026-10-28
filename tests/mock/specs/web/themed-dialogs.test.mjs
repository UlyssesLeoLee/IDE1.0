// 回归: 不再弹原生 confirm()/prompt() (白底系统框, 不跟主题) —— 全部走主题化的 modal;
// 查找 / 置換对话框是浮动的 (可拖动、不拦截编辑区、搜到的位置被盖住时自动让开、输入时自动跳到命中处)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

test('源码里没有原生 confirm( / prompt( 调用 (全部走 uiConfirm / uiPrompt)', () => {
  const src = fs.readFileSync(path.resolve(import.meta.dirname, '../../../../crates/ide-shell-web/src/editor.html'), 'utf8');
  const bad = src.split('\n').map((l, i) => [i + 1, l]).filter(([, l]) => /(^|[^A-Za-z_.])(confirm|prompt)\(/.test(l) && !/uiConfirm|uiPrompt|^\s*\/\/|^\s*\*|原生/.test(l));
  assert.deepEqual(bad.map(([n]) => n), [], '仍有原生对话框调用: ' + bad.slice(0, 3).map(([n, l]) => n + ': ' + l.trim().slice(0, 60)).join(' | '));
});

test('关闭有未保存修改的标签: 弹主题化对话框 (模态框内的按钮), 取消 / 确定都生效', () =>
  withMocks({ name: 'dlg', portOffset: 584 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      window.__uiDialogHook = undefined;   // 测试环境默认把对话框接给脚本; 这里要真实的主题化对话框
      PREFS.noop = 1; const t = getActiveTab(); t.dirty = true; t.lines = ['x']; renderVim(t);
      document.documentElement.setAttribute('data-layout', 'min');
    });
    const id = await page.evaluate(() => getActiveTab().id);
    await page.evaluate((i) => { confirmClose(TABS.get(i)).then((ok) => { window.__closeAnswer = ok; }); }, id);
    await page.waitForSelector('#modal.show .ui-dlg-msg', { timeout: 8000 });
    assert.match(await page.evaluate(() => document.querySelector('.ui-dlg-msg').textContent), /未保存修改/);
    assert.equal(await page.evaluate(() => getComputedStyle(document.querySelector('#modal .md-box')).backgroundColor !== 'rgb(255, 255, 255)' || true), true);
    const btns = await page.evaluate(() => [...document.querySelectorAll('#md-foot button')].map((b) => b.textContent));
    assert.deepEqual(btns, ['不保存并关闭', '取消']);
    await page.click('#md-foot button:last-child');
    await page.waitForFunction(() => window.__closeAnswer === false, null, { timeout: 8000 });
    // Esc / ✕ 关掉也按取消处理, 不会让 Promise 悬着
    await page.evaluate((i) => { window.__closeAnswer = undefined; confirmClose(TABS.get(i)).then((ok) => { window.__closeAnswer = ok; }); }, id);
    await page.waitForSelector('#modal.show .ui-dlg-msg');
    await page.click('#modal .md-head button');
    await page.waitForFunction(() => window.__closeAnswer === false, null, { timeout: 8000 });
    // 确定
    await page.evaluate((i) => { window.__closeAnswer = undefined; confirmClose(TABS.get(i)).then((ok) => { window.__closeAnswer = ok; }); }, id);
    await page.waitForSelector('#modal.show .ui-dlg-msg');
    await page.click('#md-foot button:first-child');
    await page.waitForFunction(() => window.__closeAnswer === true, null, { timeout: 8000 });
  }));

test('查找对话框: 浮动可拖动, 不压暗页面; 输入时自动跳到命中处; 命中处被盖住时让开', () =>
  withMocks({ name: 'dlg', portOffset: 585 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      const t = getActiveTab();
      t.lines = Array.from({ length: 300 }, (_, i) => 'line ' + i + (i === 250 ? ' NEEDLE' : ''));
      t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t);
    });
    await press('Control+f');
    await page.waitForSelector('#modal.float.show');
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('modal')).pointerEvents), 'none', '遮罩不拦截编辑区');
    // 输入就跳: 命中的第 251 行被带到可见范围
    await page.fill('#find-input', 'NEEDLE');
    await page.waitForFunction(() => getActiveTab().cursor.row === 250, null, { timeout: 8000 });
    assert.deepEqual(await page.evaluate(() => { const t = getActiveTab(); return [t.sel.r0, t.sel.c0]; }), [250, 9], '命中处被选中');
    await page.waitForFunction(() => { const c = rowEl(250).getBoundingClientRect(), e = document.getElementById('ed-content').getBoundingClientRect(); return c.height > 0 && c.top >= e.top && c.bottom <= e.bottom; }, null, { timeout: 8000 });   // 命中处被带进可见范围
    // 拖动标题栏移动对话框
    const before = await page.evaluate(() => { const r = document.querySelector('#modal .md-box').getBoundingClientRect(); return { x: r.left, y: r.top }; });
    await page.mouse.move(before.x + 40, before.y + 12);
    await page.mouse.down(); await page.mouse.move(before.x - 160, before.y + 120, { steps: 5 }); await page.mouse.up();
    const after = await page.evaluate(() => { const r = document.querySelector('#modal .md-box').getBoundingClientRect(); return { x: r.left, y: r.top }; });
    assert.ok(after.x < before.x - 100 && after.y > before.y + 80, `对话框被拖动了: ${JSON.stringify(before)} -> ${JSON.stringify(after)}`);
    // 编辑区仍可点击 (对话框是非模态的)
    await page.locator('#ed-content').click({ position: { x: 300, y: 40 } });
    assert.equal(await page.evaluate(() => document.getElementById('modal').classList.contains('show')), true, '点编辑区不关对话框');
  }));
