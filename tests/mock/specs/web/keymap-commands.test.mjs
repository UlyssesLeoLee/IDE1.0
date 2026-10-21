// 回归: Sakura 默认键位表 + 编辑 / 转换 / 复制命令 (真实键盘); 菜单快捷键标注 = 真实绑定
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

test('単語切り取り / 行末まで切り取り / 選単語', () =>
  withMocks({ name: 'km', portOffset: 50 }, async ({ page, snap, press, m }) => {
    await fill(page, 'foo bar baz\nsecond');
    await press('ArrowRight', { times: 5 }); await press('Control+d'); await page.waitForTimeout(80);
    assert.deepEqual([(await snap()).lines[0], await m.clipboard.get()], ['foo  baz', 'bar']);
    await press('Control+z'); await press('Home'); await press('ArrowRight', { times: 5 });
    await press('Control+k'); assert.equal((await snap()).lines[0], 'foo b');
    await press('Control+z'); await press('Home'); await press('ArrowRight', { times: 5 });
    await press('Control+w'); assert.deepEqual((await snap()).sel, [0, 4, 0, 7]);
  }));

test('Alt+A/D 排序, Alt+L/R trim, Alt+M 合并重复行, Ctrl+I 复制行, Ctrl+F6/F7 大小写', () =>
  withMocks({ name: 'km', portOffset: 51 }, async ({ page, snap, press }) => {
    await fill(page, 'c\nb\na'); await press('Control+a');
    await press('Alt+a'); assert.deepEqual((await snap()).lines, ['a', 'b', 'c']);
    await press('Alt+d'); assert.deepEqual((await snap()).lines, ['c', 'b', 'a']);
    await fill(page, '  x  \n\ty'); await press('Control+a');
    await press('Alt+l'); assert.deepEqual((await snap()).lines, ['x  ', 'y']);
    await press('Alt+r'); assert.deepEqual((await snap()).lines, ['x', 'y']);
    await fill(page, 'a\na\nb'); await press('Control+a'); await press('Alt+m');
    assert.deepEqual((await snap()).lines, ['a', 'b']);
    await fill(page, 'abc'); await press('Control+i'); assert.deepEqual((await snap()).lines, ['abc', 'abc']);
    await fill(page, 'hello'); await press('Control+a');
    await press('Control+F7'); assert.equal((await snap()).lines[0], 'HELLO');
    await press('Control+F6'); assert.equal((await snap()).lines[0], 'hello');
  }));

test('Shift+F8 CRLF 复制, Shift+F9 矩形贴付け, 复制变体 (行番号付き)', () =>
  withMocks({ name: 'km', portOffset: 52 }, async ({ page, snap, press, m }) => {
    await fill(page, 'ab\ncd'); await press('Control+a'); await press('Shift+F8'); await page.waitForTimeout(80);
    assert.equal(await m.clipboard.get(), 'ab\r\ncd');
    await fill(page, '12\n34'); await press('Control+a'); await press('F8');
    await press('Control+End'); await press('Shift+F9');
    assert.deepEqual((await snap()).lines, ['12', '3412', '  34']);
    await fill(page, 'l1\nl2\nl3'); await press('Control+a');
    await page.evaluate(() => dispatchCommand('copy-linenum')); await page.waitForTimeout(80);
    assert.equal(await m.clipboard.get(), '  1:l1\n  2:l2\n  3:l3\n');
  }));

test('移動履歴: 登记当前位置, 跳走后 Alt+P 回到登记位置', () =>
  withMocks({ name: 'km', portOffset: 53 }, async ({ page, snap, press }) => {
    await fill(page, '1\n2\n3\n4\n5');
    await press('Control+End');
    await page.evaluate(() => dispatchCommand('jumphist-set'));
    await press('Control+Home'); await press('Alt+p');
    assert.equal((await snap()).cursor.row, 4);
  }));

test('菜单上的快捷键标注与真实绑定一致 (标注由键表生成)', () =>
  withMocks({ name: 'km', portOffset: 54 }, async ({ page }) => {
    const cmds = ['word-cut', 'line-cut-to-end', 'sort-asc', 'replace', 'close-tab', 'open-folder', 'paste-box', 'jumphist-prev'];
    const labels = await page.evaluate((cmds) => Object.fromEntries(cmds.map((c) => [c, (document.querySelector('.menu-row[data-cmd="' + c + '"] .mr-shortcut') || {}).textContent || ''])), cmds);
    assert.deepEqual(labels, { 'word-cut': 'Ctrl+D', 'line-cut-to-end': 'Ctrl+K', 'sort-asc': 'Alt+A', replace: 'Ctrl+R', 'close-tab': 'Ctrl+F4', 'open-folder': 'Ctrl+Alt+O', 'paste-box': 'Shift+F9', 'jumphist-prev': 'Alt+P' });
  }));

test('没有活动标签时新命令给提示, 不抛异常', () =>
  withMocks({ name: 'km', portOffset: 55 }, async ({ page, app }) => {
    const errs = await page.evaluate(() => {
      const out = [];
      const saved = [...window.TABS.entries()];
      window.TABS.clear(); window.activeTabId = null;
      for (const c of ['word-cut', 'copy-html', 'tab-to-space', 'to-zenei', 'cur-line-center']) { try { dispatchCommand(c); } catch (e) { out.push(c + ':' + e.message); } }
      saved.forEach(([k, v]) => window.TABS.set(k, v));
      return out;
    });
    assert.deepEqual(errs, []);
    assert.deepEqual(app.errors, []);
  }));
