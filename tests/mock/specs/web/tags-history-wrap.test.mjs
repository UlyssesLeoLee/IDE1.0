// 回归: タグファイル作成 / 指定キーワードのタグジャンプ / 履歴の管理 / 指定桁折り返し / 最前面
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const openProject = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('tags 生成 → 指定关键字跳转 (多个候选时弹出选择)', () =>
  withMocks({ name: 'th', portOffset: 220 }, async ({ page, root, snap }) => {
    fs.mkdirSync(path.join(root, 'src'), { recursive: true });
    fs.writeFileSync(path.join(root, 'src', 'a.rs'), 'use x;\n\npub fn alpha() {}\nstruct Beta;\n');
    fs.writeFileSync(path.join(root, 'src', 'b.js'), 'function alpha() {}\nclass Gamma {}\n');
    await openProject(page, root);
    await page.locator('#ed-content').click();
    await cmd(page, 'tags-make'); await page.waitForTimeout(1500);
    const tags = fs.readFileSync(path.join(root, 'tags'), 'utf8');
    assert.match(tags, /^alpha\tsrc\/a\.rs\t3;"/m);
    assert.match(tags, /^Beta\tsrc\/a\.rs\t4;"/m);
    assert.match(tags, /^Gamma\tsrc\/b\.js\t2;"/m);
    await cmd(page, 'tag-jump-keyword'); await page.locator('.md-input').fill('Gamma'); await page.locator('.md-input').press('Enter');
    await page.waitForFunction(() => getActiveTab() && /b\.js$/.test(getActiveTab().path || ''), null, { timeout: 8000 });
    assert.equal((await snap()).cursor.row, 1);
    await cmd(page, 'tag-jump-keyword'); await page.locator('.md-input').fill('alpha'); await page.locator('.md-input').press('Enter');
    await page.waitForSelector('.pick-row');
    assert.equal(await page.evaluate(() => document.querySelectorAll('.pick-row').length), 2, 'alpha 有两个定义');
  }));

test('履歴の管理: 列出最近文件/文件夹, 删除单条, 清空全部', () =>
  withMocks({ name: 'th', portOffset: 221 }, async ({ page }) => {
    await page.evaluate(() => { localStorage.setItem('ide10.recentfiles', JSON.stringify(['C:/a.txt', 'C:/b.txt'])); localStorage.setItem('ide1.0.recent.v1', JSON.stringify([{ path: 'C:/proj', at: 1 }])); });
    await cmd(page, 'history-manager'); await page.waitForTimeout(150);
    assert.equal(await page.evaluate(() => document.querySelectorAll('#md-body .kb-row').length), 3);
    await page.locator('#md-body [data-del="f:0"]').click();
    assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('ide10.recentfiles')).join()), 'C:/b.txt');
    await page.locator('#md-foot [data-cmd="history-clear-all"]').click();
    assert.equal(await page.evaluate(() => localStorage.getItem('ide10.recentfiles')), null);
  }));

test('指定桁で折り返し (臨時): 編集域の幅が桁数に絞られ, 0 で解除', () =>
  withMocks({ name: 'th', portOffset: 222 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const full = await page.evaluate(() => document.getElementById('ed-content').getBoundingClientRect().width);
    await cmd(page, 'tmp-wrap'); await page.locator('.md-input').fill('40'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(200);
    const narrow = await page.evaluate(() => document.getElementById('ed-content').getBoundingClientRect().width);
    assert.ok(narrow < full - 100, `变窄: ${full} → ${narrow}`);
    assert.equal(await page.evaluate(() => VIEW_STATE.wordWrap), true);
    await cmd(page, 'tmp-wrap'); await page.locator('.md-input').fill('0'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(200);
    assert.ok(Math.abs((await page.evaluate(() => document.getElementById('ed-content').getBoundingClientRect().width)) - full) < 2);
  }));

test('最前面 / 最小化: 桌面通道调用窗口插件 (Tauri 替身记录), 网页版给出提示', () =>
  withMocks({ name: 'th', portOffset: 223, mock: { tauri: true } }, async ({ page, m }) => {
    await cmd(page, 'topmost'); await page.waitForTimeout(200);
    const ipc = await m.tauri.ipc();
    const call = ipc.find((c) => c.name === 'plugin:window|set_always_on_top');
    assert.ok(call && call.args.value === true, '调用了 set_always_on_top(true)');
  }));
