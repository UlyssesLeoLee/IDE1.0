// 回归: 查找 / 置換 / 検索マーク / 対括弧 / ブックマーク / インクリメンタルサーチ / タグジャンプ (真实键盘 + 对话框)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const TEXT = 'foo Foo food\nbar foo\nfoo(a[1]) {x}';
const hl = (page, name) => page.evaluate((n) => (CSS.highlights.get(n) ? [...CSS.highlights.get(n)].length : 0), name);

test('查找: 光标处命中也算; 大小写 / 单词单位 / 正则 / 上下检索 / 循环', () =>
  withMocks({ name: 'se', portOffset: 60 }, async ({ page, snap, press }) => {
    await fill(page, TEXT);
    await press('Control+f'); await page.fill('#find-input', 'foo'); await press('Enter');
    assert.deepEqual((await snap()).sel, [0, 0, 0, 3]);              // 光标在 (0,0), 该处命中先被找到
    assert.match((await snap()).status.tip, /1\/5/);
    await press('Enter'); assert.deepEqual((await snap()).sel, [0, 4, 0, 7]);
    await page.check('#find-case'); await page.keyboard.press('Escape');
    await press('Control+Home'); await press('F3'); assert.match((await snap()).status.tip, /1\/4/);
    await press('Control+f'); await page.uncheck('#find-case'); await page.check('#find-word'); await page.keyboard.press('Escape');
    await press('Control+Home'); await press('F3', { times: 4 });
    assert.match((await snap()).status.tip, /4\/4/);                 // food 被单词单位排除
    await press('Control+f'); await page.uncheck('#find-word'); await page.check('#find-regex'); await page.fill('#find-input', 'fo+d?'); await page.keyboard.press('Escape');
    await press('Control+Home'); await press('F3');
    assert.equal(await page.evaluate(() => FIND_STATE.hits.length), 5);
    await press('Shift+F3'); assert.match((await snap()).status.tip, /末尾から再検索|5\/5|1\/5/);
  }));

test('検索マーク: 全部强调, Ctrl+F3 切换', () =>
  withMocks({ name: 'se', portOffset: 61 }, async ({ page, press }) => {
    await fill(page, TEXT);
    await press('Control+f'); await page.fill('#find-input', 'foo'); await press('Enter'); await page.keyboard.press('Escape');
    await page.waitForTimeout(150);
    assert.equal(await hl(page, 'find-marks'), 5);
    await press('Control+F3'); await page.waitForTimeout(150);
    assert.equal(await hl(page, 'find-marks'), 0);
  }));

test('置換: 正则分组 $1, すべて置換 计数, 范围=选择范围, 一个置換', () =>
  withMocks({ name: 'se', portOffset: 62 }, async ({ page, snap, press }) => {
    await fill(page, TEXT);
    await press('Control+r'); await page.check('#find-regex'); await page.fill('#find-input', '(fo)o'); await page.fill('#repl-input', '[$1]');
    await page.locator('#md-foot [data-cmd="replace-all"]').click();
    assert.deepEqual((await snap()).lines, ['[fo] [Fo] [fo]d', 'bar [fo]', '[fo](a[1]) {x}']);
    assert.match((await snap()).status.tip, /置換 5/);
    await press('Control+z');
    assert.deepEqual((await snap()).lines, TEXT.split('\n'));
    // 选择范围: 只换第二行
    await press('Control+Home'); await press('ArrowDown'); await press('Home'); await press('Shift+End');
    await press('Control+r'); await page.uncheck('#find-regex'); await page.fill('#find-input', 'foo'); await page.fill('#repl-input', 'Z');
    await page.check('input[name="find-scope"][value="sel"]');
    await page.locator('#md-foot [data-cmd="replace-all"]').click();
    assert.deepEqual((await snap()).lines, ['foo Foo food', 'bar Z', 'foo(a[1]) {x}']);
  }));

test('対括弧: 强调 + Ctrl+[ 跳转; ブックマーク: Ctrl+F2 / F2 / Alt+F2 一览 / 全解除', () =>
  withMocks({ name: 'se', portOffset: 63 }, async ({ page, snap, press }) => {
    await fill(page, TEXT);
    await press('ArrowDown', { times: 2 }); await press('ArrowRight', { times: 3 }); await page.waitForTimeout(150);
    assert.equal(await hl(page, 'bracket-pair'), 2);
    await press('Control+['); assert.deepEqual((await snap()).cursor, { row: 2, col: 8 });
    await press('Control+Home'); await press('Control+F2'); await press('ArrowDown', { times: 2 }); await press('Control+F2');
    await press('Control+Home'); await press('F2'); assert.equal((await snap()).cursor.row, 2);
    await press('Alt+F2'); assert.match((await snap()).modal.body, /1: foo Foo food[\s\S]*3: foo\(a\[1\]\)/);
    await press('Escape');
    await press('Control+Shift+F2'); await press('F2'); assert.match((await snap()).status.tip, /无书签/);
  }));

test('インクリメンタルサーチ: 边输入边跳, Esc 回到起点', () =>
  withMocks({ name: 'se', portOffset: 64 }, async ({ page, snap, press }) => {
    await fill(page, TEXT);
    await page.locator('#menubar .menu-item[data-menu="search"] > .mb-btn').click();
    await page.locator('#menubar .menu-row[data-cmd="isearch-fwd"]').click();
    await page.keyboard.type('ba');
    assert.deepEqual((await snap()).sel, [1, 0, 1, 2]);
    await page.keyboard.press('Escape');
    assert.deepEqual([(await snap()).sel, (await snap()).cursor], [null, { row: 0, col: 0 }]);
  }));

test('タグジャンプ (F12) 跳到定义, Shift+F12 回到原位置', () =>
  withMocks({ name: 'se', portOffset: 65 }, async ({ page, snap, press }) => {
    await fill(page, 'fn alpha() {}\nfn beta() { alpha(); }');
    await press('ArrowDown'); await press('End'); await press('ArrowLeft', { times: 8 });
    await press('F12'); assert.deepEqual((await snap()).cursor.row, 0);
    await press('Shift+F12'); assert.equal((await snap()).cursor.row, 1);
  }));

test('MIGEMO インクリメンタルサーチ: ローマ字で かな / カナ / 半角カナ にヒット (候補は Rust 展開)', () =>
  withMocks({ name: 'se', portOffset: 66 }, async ({ page, snap, press }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['hello', 'にほんご世界', 'カタカナ', 'ｶﾀｶﾅ半角']; t.cursor = { row: 0, col: 0 }; renderVim(t); });
    await page.evaluate(() => dispatchCommand('isearch-migemo-fwd'));
    await page.keyboard.type('nihongo'); await page.waitForTimeout(700);
    assert.deepEqual((await snap()).sel, [1, 0, 1, 4]);
    await page.keyboard.press('Escape');
    await page.evaluate(() => { getActiveTab().cursor = { row: 0, col: 0 }; dispatchCommand('isearch-migemo-fwd'); });
    await page.keyboard.type('katakana'); await page.waitForTimeout(700);
    assert.deepEqual((await snap()).sel.slice(0, 2), [2, 0]);
    await page.keyboard.press('Enter'); await page.waitForTimeout(400);
    assert.deepEqual((await snap()).sel.slice(0, 2), [3, 0], '下一处 = 半角カナ');
    await page.keyboard.press('Escape');
  }));
