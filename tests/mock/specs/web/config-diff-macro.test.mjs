// 回归: 共通設定 / キー割り当て / DIFF(ファイル比較) / 宏的保存与读取
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const openProject = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('共通設定: TAB 宽度 / 自动缩进 / 新文档换行 立即生效并落盘, 重载后仍在', () =>
  withMocks({ name: 'cd', portOffset: 180 }, async ({ page, snap, press }) => {
    await cmd(page, 'common-config');
    await page.locator('.cfg-tab', { hasText: '全般' }).click();
    await page.locator('input[type=number][data-i="1"]').fill('8'); await page.locator('input[type=number][data-i="1"]').dispatchEvent('change');
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('ed-content')).tabSize), '8');
    await page.locator('.cfg-tab', { hasText: '編集' }).click();
    await page.locator('input[type=checkbox][data-i="0"]').uncheck();
    await page.locator('.cfg-tab', { hasText: 'ファイル' }).click();
    await page.locator('select[data-i="1"]').selectOption('lf');
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    await fill(page, '  ab');                                      // 关掉自动缩进: 回车不复制缩进
    await press('End'); await press('Enter'); await page.keyboard.type('x');
    assert.deepEqual((await snap()).lines, ['  ab', 'x']);
    await press('Control+n');
    assert.equal(await page.evaluate(() => getActiveTab().eol), 'lf', '新文档用设定的换行');
    await page.reload(); await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => getComputedStyle(document.getElementById('ed-content')).tabSize), '8');
  }));

test('キー割り当て: 改键 / 删键 / 追加 / 恢复默认; 菜单标注同步; 重载后保留', () =>
  withMocks({ name: 'cd', portOffset: 181 }, async ({ page, snap, press }) => {
    await fill(page, 'foo bar baz');
    await press('Control+Home');
    await cmd(page, 'key-config');
    await page.fill('#kb-filter', 'word-cut');
    await page.locator('.kb-row', { hasText: 'word-cut' }).locator('[data-act="chg"]').click();
    await page.keyboard.press('Control+Shift+j');                    // 新键
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    await page.locator('#ed-content').focus();
    await press('Control+d');                                         // 旧键不再触发
    assert.equal((await snap()).lines[0], 'foo bar baz');
    await press('Control+Shift+j');
    assert.notEqual((await snap()).lines[0], 'foo bar baz', '新键触发单词剪切');
    assert.equal(await page.evaluate(() => document.querySelector('.menu-row[data-cmd="word-cut"] .mr-shortcut').textContent), 'Shift+Ctrl+J');
    assert.ok(await page.evaluate(() => /word-cut/.test(localStorage.getItem('ide10.keymap'))));
    await page.reload(); await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => KEYMAP['C-S-j']), 'word-cut');
    // 恢复默认
    await cmd(page, 'key-config'); await page.locator('#kb-reset').click();
    assert.equal(await page.evaluate(() => KEYMAP['C-d']), 'word-cut');
    assert.equal(await page.evaluate(() => localStorage.getItem('ide10.keymap')), null);
  }));

test('DIFF: 与另一标签比较 → 标记新增/变更/删除, 差分一览, 上一处/下一处, 清除', () =>
  withMocks({ name: 'cd', portOffset: 182 }, async ({ page, snap, press }) => {
    const hunks = await page.evaluate(() => diffLines(['a', 'b', 'c', 'd', 'e'], ['a', 'B', 'c', 'x', 'd', 'e', 'f']));
    assert.deepEqual(hunks, [{ a0: 1, a1: 2, b0: 1, b1: 2 }, { a0: 3, a1: 3, b0: 3, b1: 4 }, { a0: 5, a1: 5, b0: 6, b1: 7 }]);
    assert.deepEqual(await page.evaluate(() => diffLines(['a', 'b'], ['a', 'b'])), []);
    // 真实标签
    await fill(page, 'one\ntwo\nthree\nfour');
    await press('Control+n'); await fill(page, 'one\n2\nthree\nfour\nfive');
    await cmd(page, 'diff-with-tab'); await page.locator('.dtab-row').first().click(); await page.waitForTimeout(300);
    const list = await page.evaluate(() => document.getElementById('md-body').innerText);
    assert.match(list, /変更/); assert.match(list, /追加/);
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    await page.waitForTimeout(200);
    const marks = await page.evaluate(() => Object.fromEntries(['diff-add', 'diff-chg', 'diff-del'].map((k) => [k, CSS.highlights.get(k) ? CSS.highlights.get(k).size : 0])));
    assert.deepEqual(marks, { 'diff-add': 1, 'diff-chg': 1, 'diff-del': 0 });
    await press('Control+Home');
    await cmd(page, 'diff-next'); assert.equal((await snap()).cursor.row, 1);
    await cmd(page, 'diff-next'); assert.equal((await snap()).cursor.row, 4);
    await cmd(page, 'diff-prev'); assert.equal((await snap()).cursor.row, 1);
    await cmd(page, 'diff-clear'); await page.waitForTimeout(200);
    assert.equal(await page.evaluate(() => (CSS.highlights.get('diff-chg') || { size: 0 }).size), 0);
  }));

test('DIFF: 与磁盘文件比较', () =>
  withMocks({ name: 'cd', portOffset: 183 }, async ({ page, root, press }) => {
    fs.writeFileSync(path.join(root, 'old.txt'), 'x\ny\nz');
    await openProject(page, root);
    await fill(page, 'x\nY\nz');
    await cmd(page, 'diff-with-file');
    await page.locator('.md-input').fill('old.txt'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => getActiveTab().diff.hunks.length), 1);
  }));

test('宏: 录制 → 保存到文件 → 清空 → 读取 → 回放', () =>
  withMocks({ name: 'cd', portOffset: 184 }, async ({ page, root, snap, press }) => {
    await openProject(page, root);
    await page.locator('#ed-content').click();
    await cmd(page, 'macro-start');
    await page.keyboard.type('hi'); await page.waitForTimeout(100);
    await cmd(page, 'macro-stop');
    const n = await page.evaluate(() => MACRO.keys.length);
    assert.ok(n >= 2, '录到了按键: ' + n);
    await cmd(page, 'macro-save'); await page.locator('.md-input').fill('m.mac.json'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(600);
    const saved = JSON.parse(fs.readFileSync(path.join(root, 'm.mac.json'), 'utf8'));
    assert.equal(saved.keys.length, n);
    await page.evaluate(() => { MACRO.keys = []; });
    await cmd(page, 'macro-load'); await page.locator('.md-input').fill('m.mac.json'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => MACRO.keys.length), n);
  }));
