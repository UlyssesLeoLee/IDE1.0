// 回归: キー割り当て 对话框的「按键查找」—— 按下一个键, 显示它绑定的命令并把列表筛过去; 没绑的键也有说明
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('按键查找: 已绑定的键显示命令 (含 Markdown 覆盖说明), 未绑定的键提示可以分配', () =>
  withMocks({ name: 'kl', portOffset: 610 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => dispatchCommand('key-config'));
    await page.waitForSelector('#kb-find', { timeout: 8000 });
    await page.click('#kb-find');
    await page.waitForSelector('#kb-capture');
    assert.match(await page.evaluate(() => document.getElementById('kb-capture').textContent), /要查找/);
    await page.keyboard.press('Control+i');                 // Sakura: 复制行; Markdown 文档里是斜体
    await page.waitForFunction(() => [...document.querySelectorAll('#md-body .empty-hint')].some((e) => /→/.test(e.textContent) && /Ctrl\+I/.test(e.textContent) && !/下列按键/.test(e.textContent)), null, { timeout: 8000 });
    const note1 = await page.evaluate(() => [...document.querySelectorAll('#md-body .empty-hint')].map((e) => e.textContent).join('|'));
    assert.match(note1, /Markdown: 斜体 \(md-italic\)/);
    assert.match(note1, /非 Markdown 文档里是/);
    await page.click('#kb-find');
    await page.keyboard.press('Control+Alt+Shift+F11');     // 没绑定
    await page.waitForFunction(() => [...document.querySelectorAll('#md-body .empty-hint')].some((e) => /没有绑定命令/.test(e.textContent)), null, { timeout: 8000 });
  }));

test('捕获新按键时, 已绑定的键不会被当命令执行 (只是被记下来)', () =>
  withMocks({ name: 'kl', portOffset: 612 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['row']; t.cursor = { row: 0, col: 0 }; renderVim(t); });
    await page.evaluate(() => dispatchCommand('key-config'));
    await page.waitForSelector('#kb-add');
    await page.click('#kb-add');
    await page.waitForSelector('#kb-capture');
    await page.evaluate(() => window.__mock.answers.confirm.push(false));   // 冲突时问"要替换吗": 不替换
    await page.keyboard.press('Control+i');                                  // Sakura 绑的是「复制行」: 捕获模式下不能真的复制行
    await page.waitForTimeout(500);
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), ['row'], '没有执行复制行');
    assert.equal(await page.evaluate(() => window.__mock.dialogs.filter((d) => d.kind === 'confirm' && /已分配给/.test(d.msg)).length), 1, '走了冲突确认');
  }));
