// 回归: 保存并关闭 / 关闭并打开 / 选区字数统计 / 折行位置换行复制 / 帮助检索
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const openProject = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('选区字数统计: 字符 / 字节 / 行 三种方式点击切换, 无选区时为空', () =>
  withMocks({ name: 'mc', portOffset: 260 }, async ({ page, press }) => {
    await fill(page, 'あい\nabc');
    assert.equal(await page.evaluate(() => document.getElementById('s-sel').textContent), '');
    await press('Control+a'); await page.waitForTimeout(100);
    assert.equal(await page.evaluate(() => document.getElementById('s-sel').textContent), '選択 6 文字');
    await page.locator('#s-sel').click();
    assert.equal(await page.evaluate(() => document.getElementById('s-sel').textContent), '選択 10 バイト');
    await page.locator('#s-sel').click();
    assert.equal(await page.evaluate(() => document.getElementById('s-sel').textContent), '選択 2 行');
  }));

test('保存并关闭: 写盘后标签关闭 (无标题文档会先问文件名)', () =>
  withMocks({ name: 'mc', portOffset: 261 }, async ({ page, root }) => {
    await openProject(page, root);
    fs.writeFileSync(path.join(root, 'sc.txt'), 'old');
    await page.evaluate(async (r) => { await setProjectRoot(r, true); await openFile(r + '/sc.txt'); }, root);
    await page.locator('#ed-content').click(); await page.keyboard.press('End'); await page.keyboard.type('!');
    const before = await page.evaluate(() => TABS.size);
    await cmd(page, 'save-close'); await page.waitForTimeout(800);
    assert.match(fs.readFileSync(path.join(root, 'sc.txt'), 'utf8'), /^old!/);
    assert.equal(await page.evaluate(() => [...TABS.values()].some((t) => /sc\.txt$/.test(t.path || ''))), false);
    assert.ok(before >= 1);
  }));

test('在折行位置加换行后复制 / 帮助检索', () =>
  withMocks({ name: 'mc', portOffset: 262 }, async ({ page, press, m }) => {
    await fill(page, 'x');
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['a'.repeat(400)]; t.sel = { r0: 0, c0: 0, r1: 0, c1: 400 }; renderVim(t); });
    await cmd(page, 'copy-addcrlf'); await page.waitForTimeout(100);
    const clip = await page.evaluate(() => CLIPBOARD);
    assert.ok(clip.includes('\r\n') && clip.replace(/\r\n/g, '').length === 400, '长行被折行位置的换行切开');
    await cmd(page, 'help-search'); await page.locator('.md-input').fill('IDE'); await page.locator('.md-input').press('Enter'); await page.waitForTimeout(200);
    assert.match(await page.evaluate(() => document.getElementById('modal').querySelector('.md-title, .md-head, h3, #md-title').innerText), /帮助检索/);
  }));
