// 回归 (桌面真机, 需 QA_DESKTOP): 真实 Tauri 里的终端 —— 本地回显 / 退格 / 回车执行, 中文输出与报错不乱码, 退出后可再新建。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { launchDesktop, hasDesktop } from '../../lib/desktop.mjs';

const opts = { skip: hasDesktop ? false : '未设置 QA_DESKTOP (桌面真机用例已跳过)' };
const text = (page) => page.evaluate(() => document.getElementById('terminal-body').innerText);
const waitText = (page, re, ms = 20000) => page.waitForFunction(([src, fl]) => new RegExp(src, fl).test(document.getElementById('terminal-body').innerText), [re.source, re.flags], { timeout: ms });

test('桌面版终端: 本地回显 / 退格 / 执行; 中文不乱码; 会话结束后可再新建', opts, async () => {
  const d = await launchDesktop({ cdpPort: 9470 });
  try {
    const { page } = d;
    await page.evaluate(() => { try { localStorage.setItem('ide10.vim', 'off'); } catch (_) {} });
    await page.evaluate(() => dispatchCommand('new-terminal'));
    await waitText(page, /PS .*>/);
    await page.locator('#terminal-body').click();
    // 本地回显 + 退格
    await page.keyboard.type('echo hellozz');
    assert.match(await page.locator('#terminal-body .tl-in').last().innerText(), /echo hellozz/);
    await page.keyboard.press('Backspace'); await page.keyboard.press('Backspace');
    await page.keyboard.press('Enter');
    await waitText(page, /^hello$/m);
    assert.equal(((await text(page)).match(/echo hello/g) || []).length, 1, 'shell 的回显被吞掉, 命令只出现一次');
    // 中文输出 (输入法上屏同路径)
    await page.keyboard.insertText('echo 日本語テスト');
    await page.keyboard.press('Enter');
    await waitText(page, /^日本語テスト$/m);
    // 报错信息不能是乱码
    await page.keyboard.type('nosuchcmd_xyz_123');
    await page.keyboard.press('Enter');
    await waitText(page, /nosuchcmd_xyz_123/);
    await page.waitForTimeout(1500);
    assert.doesNotMatch(await text(page), /�/, '桌面版终端输出里没有 U+FFFD');
    // 退出后可再新建
    await page.keyboard.type('exit');
    await page.keyboard.press('Enter');
    await waitText(page, /会话已结束/);
    assert.equal(await page.evaluate(() => terminalId), null);
    await page.evaluate(() => dispatchCommand('new-terminal'));
    await page.waitForFunction(() => !!terminalId, null, { timeout: 15000 });
    assert.deepEqual(d.errors, []);
  } finally { await d.close(); }
});

test('桌面版: 分割后 / 取消分割后都能输入', opts, async () => {
  const d = await launchDesktop({ cdpPort: 9471 });
  try {
    const { page } = d;
    await page.evaluate(() => { try { localStorage.setItem('ide10.vim', 'off'); } catch (_) {} });
    await page.locator('#ed-content').click();
    await page.keyboard.type('abc');
    await page.evaluate(() => dispatchCommand('split-v'));
    await page.locator('#ed-content').click();
    await page.keyboard.press('Control+End'); await page.keyboard.type('X');
    await page.evaluate(() => window.__editIdle());
    assert.equal(await page.evaluate(() => getActiveTab().lines[0]), 'abcX', '分割后能输入');
    await page.evaluate(() => dispatchCommand('split-v'));
    await page.locator('#ed-content').click();
    await page.keyboard.press('Control+End'); await page.keyboard.type('Y');
    await page.evaluate(() => window.__editIdle());
    assert.equal(await page.evaluate(() => getActiveTab().lines[0]), 'abcXY', '取消分割后能输入');
    assert.deepEqual(d.errors, []);
  } finally { await d.close(); }
});
