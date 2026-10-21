// 回归: 首次启动态 (什么都不种) — c18815d / 34511fc 带回的三个阻塞缺陷 + 设置菜单被截断
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withApp, fill } from '../../lib/harness.mjs';

test('首次启动不是 Vim 模式: 直接打字, 文字完整进文档', () =>
  withApp({ name: 'fl', portOffset: 0 }, async ({ page, snap, app }) => {
    assert.equal(await page.evaluate(() => LAYOUT.vimOn()), false);
    await page.locator('#ed-content').click();
    await page.keyboard.type('hello world');
    assert.deepEqual((await snap()).lines, ['hello world']);
    assert.deepEqual(app.errors, []);
  }));

test('下拉菜单里没有漏出的源码注释文字 (// SPA-03 …)', () =>
  withApp({ name: 'fl', portOffset: 1 }, async ({ page }) => {
    const junk = await page.evaluate(() => [...document.querySelectorAll('.menu-dropdown')].flatMap((d) => d.innerText.split('\n').filter((l) => /\/\/|SPA-03|fix:/.test(l))));
    assert.deepEqual(junk, []);
  }));

test('每个菜单的结构完整: 行都是下拉菜单的直接子项 (没有未闭合的 div 吞掉后面的行)', () =>
  withApp({ name: 'fl', portOffset: 2 }, async ({ page }) => {
    const bad = await page.evaluate(() => {
      const out = [];
      for (const d of document.querySelectorAll('#menubar .menu-dropdown')) {
        for (const row of d.querySelectorAll('.menu-row[data-cmd]')) {
          // 菜单行不应嵌在另一个菜单行里 (除了子菜单的飞出层)
          const p = row.parentElement;
          if (!(p === d || p.classList.contains('submenu-dropdown'))) out.push(d.dataset.dropdown + ':' + row.dataset.cmd);
        }
      }
      return out;
    });
    assert.deepEqual(bad, []);
    const settingCmds = await page.evaluate(() => [...document.querySelectorAll('.menu-dropdown[data-dropdown="setting"] > .menu-row')].map((r) => r.dataset.cmd));
    for (const c of ['theme-select', 'language', 'toggle-vim', 'charset']) assert.ok(settingCmds.includes(c), '设置菜单缺 ' + c);
  }));

test('Shift+Tab 不抛 ReferenceError; 对多行选区反缩进', () =>
  withApp({ name: 'fl', portOffset: 3 }, async ({ page, snap, press, app }) => {
    await fill(page, '  a\n  b');
    await press('Control+a'); await page.waitForTimeout(150);
    await press('Shift+Tab'); await page.waitForTimeout(150);
    // fill() 用真实 Enter, 自动缩进让第二行是 4 空格 → Sakura 反缩进最多去掉 4 个空格 → 0
    assert.deepEqual((await snap()).lines, ['a', 'b']);
    assert.deepEqual(app.errors, []);
  }));

test('关闭对话框后焦点回到编辑区 (点按钮后还能继续用键盘)', () =>
  withApp({ name: 'fl', portOffset: 4 }, async ({ page, snap, press }) => {
    await page.locator('#ed-content').click();
    await page.keyboard.type('abc');
    await press('Control+f');
    await page.fill('#find-input', 'b');
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    assert.equal(await page.evaluate(() => document.activeElement.id), 'ed-content');
    await press('Control+Home');
    assert.deepEqual((await snap()).cursor, { row: 0, col: 0 });
  }));
