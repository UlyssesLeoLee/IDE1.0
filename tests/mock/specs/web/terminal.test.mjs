// 回归 (用户报告): 新建终端无法正常输入命令; SQL*Plus 窗口里 PowerShell 报错是乱码。
//   - 敲的字回车前就能看见 (本地回显), 退格 / 方向键 / 历史 / 输入法 / 粘贴可用
//   - 命令在输出里只出现一次 (shell 的回显被吞掉)
//   - 中文输入输出不乱码; 报错信息里没有 U+FFFD
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const termText = (page) => page.evaluate(() => document.getElementById('terminal-body').innerText);
const waitText = (page, re, ms = 15000) => page.waitForFunction(([src, fl]) => new RegExp(src, fl).test(document.getElementById('terminal-body').innerText), [re.source, re.flags], { timeout: ms });

async function openTerminal(page) {
  await page.evaluate(() => dispatchCommand('new-terminal'));
  await page.waitForFunction(() => /PS .*>/.test(document.getElementById('terminal-body').innerText), null, { timeout: 20000 });
  await page.locator('#terminal-body').click();
}

test('终端: 本地回显 / 退格 / 历史 / 输入法 / 粘贴; 命令只出现一次; 中文不乱码', () =>
  withMocks({ name: 'tm', portOffset: 500 }, async ({ page }) => {
    await openTerminal(page);
    // 本地回显: 回车前就看得见
    await page.keyboard.type('echo hellozz');
    assert.match(await page.locator('#terminal-body .tl-in').last().innerText(), /echo hellozz/);
    await page.keyboard.press('Backspace'); await page.keyboard.press('Backspace');
    assert.match(await page.locator('#terminal-body .tl-in').last().innerText(), /echo hello\s*$/, '退格删掉的是字符, 不是塞进一个空格');
    await page.keyboard.press('ArrowLeft'); await page.keyboard.press('ArrowLeft');
    await page.keyboard.type('X');
    assert.match(await page.locator('#terminal-body .tl-in').last().innerText(), /echo helXlo/, '光标可以左移后在中间插入');
    await page.keyboard.press('Delete');
    await page.keyboard.press('Backspace');                    // 删掉 X
    assert.match(await page.locator('#terminal-body .tl-in').last().innerText(), /echo hel[o ]?\s*$/);
    await page.keyboard.press('Escape');                       // 清掉这一行
    await page.keyboard.type('echo hello');
    await page.keyboard.press('Enter');
    await waitText(page, /^hello$/m);
    const t1 = await termText(page);
    assert.equal((t1.match(/echo hello/g) || []).length, 1, 'shell 的回显被吞掉, 命令只出现一次');
    // 方向键 ↑ 取回上一条
    await page.keyboard.press('ArrowUp');
    assert.match(await page.locator('#terminal-body .tl-in').last().innerText(), /echo hello/);
    await page.keyboard.press('Escape');
    // 输入法上屏 (insertText 触发 input 事件, 与 IME 同路径) + 中文输出不乱码
    await page.keyboard.insertText('echo 日本語テスト');
    await page.keyboard.press('Enter');
    await waitText(page, /^日本語テスト$/m);
    // 多行粘贴: 每一行都执行
    await page.evaluate(() => { const ta = document.getElementById('term-input'); ta.value = 'echo p1\necho p2'; ta.dispatchEvent(new Event('input', { bubbles: true })); });
    await waitText(page, /^p1$/m);
    await page.keyboard.press('Enter');
    await waitText(page, /^p2$/m);
    // 出错信息不能是乱码 (中文系统上 PowerShell 的报错默认是 GBK)
    await page.keyboard.type('nosuchcmd_xyz_123');
    await page.keyboard.press('Enter');
    await waitText(page, /nosuchcmd_xyz_123/);
    await page.waitForTimeout(1500);
    assert.doesNotMatch(await termText(page), /�/, '输出里没有 U+FFFD');
    // Ctrl+L 清屏
    await page.keyboard.press('Control+l');
    assert.doesNotMatch(await termText(page), /hello/);
  }));

test('终端: 会话结束后可以再新建; 已有会话时再点新建只是聚焦', () =>
  withMocks({ name: 'tm', portOffset: 501 }, async ({ page }) => {
    await openTerminal(page);
    const id1 = await page.evaluate(() => terminalId);
    await page.evaluate(() => dispatchCommand('new-terminal'));
    assert.equal(await page.evaluate(() => terminalId), id1, '已有会话: 不重复创建');
    await page.keyboard.type('exit');
    await page.keyboard.press('Enter');
    await waitText(page, /会话已结束/);
    assert.equal(await page.evaluate(() => terminalId), null, '会话结束后 terminalId 清空');
    await page.evaluate(() => dispatchCommand('new-terminal'));
    await page.waitForFunction(() => !!terminalId, null, { timeout: 10000 });
    assert.notEqual(await page.evaluate(() => terminalId), id1);
  }));
