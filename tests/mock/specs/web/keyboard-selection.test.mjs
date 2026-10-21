// 回归: Sakura 键盘移动 / 范围选择 / 选区替换式编辑 / Ins 覆盖 / Tab 字符 / 系统剪贴板 (全部真实键盘)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const TEXT = 'foo bar baz\nsecond line here\nthird\nlast';

test('移动与范围选择: Shift+方向 / Ctrl+单词 / Ctrl+Home·End / 收缩', () =>
  withMocks({ name: 'ks', portOffset: 10 }, async ({ page, snap, press }) => {
    await fill(page, TEXT);
    await press('End'); assert.deepEqual((await snap()).cursor, { row: 0, col: 11 });
    await press('Home'); await press('Shift+ArrowRight', { times: 3 });
    assert.deepEqual((await snap()).sel, [0, 0, 0, 3]);
    await press('Shift+ArrowDown'); assert.deepEqual((await snap()).sel, [0, 0, 1, 3]);
    await press('ArrowLeft');                                   // 无 Shift: 收缩到选区起点
    assert.deepEqual([(await snap()).sel, (await snap()).cursor], [null, { row: 0, col: 0 }]);
    await press('Control+ArrowRight'); assert.equal((await snap()).cursor.col, 4);   // Sakura: 到下一个词首
    await press('Control+End'); assert.deepEqual((await snap()).cursor, { row: 3, col: 4 });
    await press('Shift+Control+Home'); assert.deepEqual((await snap()).sel, [0, 0, 3, 4]);
  }));

test('Shift+Ctrl+↓ 选 2 行; PgDn 翻页; 目标列记忆', () =>
  withMocks({ name: 'ks', portOffset: 11 }, async ({ page, snap, press }) => {
    await fill(page, TEXT);
    await press('Shift+Control+ArrowDown'); assert.deepEqual((await snap()).sel, [0, 0, 2, 0]);
    await press('Escape'); await press('Control+Home');
    await press('PageDown'); assert.equal((await snap()).cursor.row, 3);
    await fill(page, 'abcdef\nx\nabcdef');
    await press('ArrowRight', { times: 5 }); await press('ArrowDown'); await press('ArrowDown');
    assert.deepEqual((await snap()).cursor, { row: 2, col: 5 });     // 经过短行再回长行, 列恢复
  }));

test('选区上输入 / Delete / 粘贴 都是「替换」, 撤销能回退', () =>
  withMocks({ name: 'ks', portOffset: 12 }, async ({ page, snap, press, m }) => {
    await fill(page, 'hello world');
    await press('Shift+Control+ArrowRight'); await page.keyboard.type('X');
    assert.equal((await snap()).lines[0], 'Xworld');   // Sakura: Ctrl+Shift+→ 选到下一个词首 (含空格)
    await press('Home'); await press('Shift+ArrowRight', { times: 2 }); await press('Delete');
    assert.equal((await snap()).lines[0], 'orld');
    await m.clipboard.set('PASTED'); await press('Shift+ArrowRight', { times: 2 }); await press('Control+v');
    await page.waitForTimeout(150);
    assert.equal((await snap()).lines[0], 'PASTEDld');
  }));

test('Ins 覆盖模式 + 状态栏; Tab 键插入 TAB 字符; Shift+Tab 反缩进', () =>
  withMocks({ name: 'ks', portOffset: 13 }, async ({ page, snap, press }) => {
    await fill(page, 'abcd');
    await press('Insert'); await page.keyboard.type('XY');
    assert.equal((await snap()).lines[0], 'XYcd');
    assert.equal((await snap()).status.ovr.trim(), '上書');
    await press('Insert'); assert.equal((await snap()).status.ovr.trim(), '挿入');
    await press('End'); await press('Tab'); assert.equal((await snap()).lines[0], 'XYcd\t');
  }));

test('系统剪贴板: Ctrl+C 写入, 外部内容 Ctrl+V 粘贴, F7/F8/F9 与 Ins 系列别名', () =>
  withMocks({ name: 'ks', portOffset: 14 }, async ({ page, snap, press, m }) => {
    await fill(page, 'copy me');
    await press('Control+a'); await press('Control+c'); await page.waitForTimeout(100);
    assert.equal(await m.clipboard.get(), 'copy me');
    await m.clipboard.set('FROM_OUTSIDE'); await press('Control+End'); await press('Control+v'); await page.waitForTimeout(150);
    assert.equal((await snap()).lines[0], 'copy meFROM_OUTSIDE');
    await press('Control+a'); await press('F8'); await page.waitForTimeout(80);
    assert.equal(await m.clipboard.get(), 'copy meFROM_OUTSIDE');
    await press('Shift+Delete'); await page.waitForTimeout(80);
    assert.equal((await snap()).lines[0], '');
    await press('Shift+Insert'); await page.waitForTimeout(150);
    assert.equal((await snap()).lines[0], 'copy meFROM_OUTSIDE');
  }));
