// 回归: Sakura 尾部命令 — SPACE インデント / デコード保存 / 変更行 / 関数 / タブ移動 / 印刷 / 外部命令 / アウトライン跳转
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const openProject = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('SPACE インデント / 逆 SPACE インデント: 选区各行加减 1 个空格 (Sakura 实测; 与 TAB 宽度无关)', () =>
  withMocks({ name: 'tc', portOffset: 200 }, async ({ page, snap, press }) => {
    await fill(page, 'a\nb\nc');
    await press('Control+a'); await cmd(page, 'indent-space');
    assert.deepEqual((await snap()).lines, [' a', ' b', ' c']);   // Sakura: SPACE インデント = 1 个空格
    await cmd(page, 'unindent-space');
    assert.deepEqual((await snap()).lines, ['a', 'b', 'c']);
    await page.evaluate(() => { PREFS.tabWidth = 2; });
    await press('Control+a'); await cmd(page, 'indent-space');
    assert.equal((await snap()).lines[0], ' a');   // 与 TAB 宽度无关
  }));

test('Base64 / uudecode 解码保存: 选区解码写成文件; 二进制内容给出明确提示', () =>
  withMocks({ name: 'tc', portOffset: 201 }, async ({ page, root, press }) => {
    await openProject(page, root);
    await fill(page, Buffer.from('こんにちは hello', 'utf8').toString('base64'));
    await press('Control+a'); await cmd(page, 'base64-decode');
    await page.locator('.md-input').fill('b64.txt'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(600);
    assert.equal(fs.readFileSync(path.join(root, 'b64.txt'), 'utf8'), 'こんにちは hello');
    // uuencode "Cat"
    await fill(page, 'begin 644 x\n#0V%T\n`\nend');
    await press('Control+a'); await cmd(page, 'uudecode');
    await page.locator('.md-input').fill('uu.txt'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(600);
    assert.equal(fs.readFileSync(path.join(root, 'uu.txt'), 'utf8'), 'Cat');
    await fill(page, '/w=='); await press('Control+a'); await cmd(page, 'base64-decode');   // 0xFF: 不是 UTF-8
    assert.match(await page.evaluate(() => document.getElementById('s-tip').textContent), /解码失败/);
  }));

test('変更行: 次/前的变更行按基线 (上次保存/打开) 计算', () =>
  withMocks({ name: 'tc', portOffset: 202 }, async ({ page, snap, press }) => {
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['a', 'b', 'c', 'd', 'e', 'f']; t.dirty = false; renderVim(t); });
    await page.locator('#ed-content').click();
    await press('Control+Home'); await press('ArrowDown', { times: 2 }); await page.keyboard.type('X');    // 改第 3 行
    await press('Control+End'); await page.keyboard.type('Y');                                           // 改最后一行
    await press('Control+Home');
    await cmd(page, 'modify-next'); assert.equal((await snap()).cursor.row, 2);
    await cmd(page, 'modify-next'); assert.equal((await snap()).cursor.row, 5);
    await cmd(page, 'modify-prev'); assert.equal((await snap()).cursor.row, 2);
  }));

test('タブ移動 / 左を閉じる / 最近のファイル挿入', () =>
  withMocks({ name: 'tc', portOffset: 203 }, async ({ page, root, snap, press }) => {
    await press('Control+n'); await press('Control+n'); await page.waitForTimeout(150);
    const ids0 = await page.evaluate(() => [...TABS.keys()]);
    await page.evaluate((id) => { setActiveTab(id); renderTabs(); }, ids0[0]);
    await cmd(page, 'tab-move-right');
    assert.deepEqual(await page.evaluate(() => [...TABS.keys()]), [ids0[1], ids0[0], ids0[2]]);
    await cmd(page, 'tab-move-left'); await cmd(page, 'tab-move-left');                // 已在最左: 不动
    assert.deepEqual(await page.evaluate(() => [...TABS.keys()]), [ids0[0], ids0[1], ids0[2]]);
    await page.evaluate((id) => { setActiveTab(id); renderTabs(); }, ids0[2]);
    page.once('dialog', (d) => d.accept());
    await cmd(page, 'close-left'); await page.waitForTimeout(200);
    assert.equal(await page.evaluate(() => TABS.size), 1);
    await page.evaluate(() => localStorage.setItem('ide10.recentfiles', JSON.stringify(['C:/x/a.txt'])));
    await fill(page, '');
    await cmd(page, 'ins-recent-file'); await page.locator('.pick-row').first().click();
    assert.equal((await snap()).lines[0], 'C:/x/a.txt');
  }));

test('印刷プレビュー: 分页 + 行号; 印刷设置可保存; ブラウズ生成可打开的页面', () =>
  withMocks({ name: 'tc', portOffset: 204 }, async ({ page, press }) => {
    await page.evaluate(() => { const t = getActiveTab(); t.lines = Array.from({ length: 120 }, (_, i) => 'line' + (i + 1)); renderVim(t); });
    await cmd(page, 'print-preview'); await page.waitForTimeout(150);
    assert.equal(await page.evaluate(() => document.querySelectorAll('.pv-page').length), 3);
    assert.match(await page.evaluate(() => document.querySelector('.pv-page').innerText), /1 line1/);
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    await cmd(page, 'print-setup'); await page.fill('#pp-font', '9'); await page.uncheck('#pp-ln'); await page.locator('#md-foot [data-cmd="print-setup-save"]').click();
    assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('ide10.print')).lineNumbers), false);
    const html = await page.evaluate(() => printHtml(getActiveTab()));
    assert.ok(html.includes('line120') && !/\b120 line120/.test(html));
  }));

test('外部命令: 在终端里执行 (Tauri 替身记录 terminal_input)', () =>
  withMocks({ name: 'tc', portOffset: 205, mock: { tauri: true } }, async ({ page, root, m }) => {
    await openProject(page, root);
    await page.locator('#ed-content').click();
    await cmd(page, 'exec-cmd'); await page.locator('.md-input').fill('echo hi'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(1500);
    const ipc = await m.tauri.ipc();
    const inputs = ipc.filter((c) => c.name === 'terminal_input').map((c) => c.args.data).join('');
    assert.match(inputs, /echo hi\r/);
  }));
