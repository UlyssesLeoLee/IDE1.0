// 回归: 标签分组 / 窗口最大化 / 带选区的変更行 / SQL*Plus
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const shown = (page) => page.evaluate(() => [...document.querySelectorAll('#tabs .tab')].map((e) => e.dataset.tabId));

test('标签分组: 移到新分组后标签条只显示当前组; 切换分组 / 合并 / 关闭分组', () =>
  withMocks({ name: 'gw', portOffset: 280 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await press('Control+n'); await press('Control+n'); await page.waitForTimeout(150);
    const ids = await page.evaluate(() => [...TABS.keys()]);
    assert.equal((await shown(page)).length, 3);
    await page.evaluate((id) => { setActiveTab(id); }, ids[2]);
    await cmd(page, 'tab-separate');
    assert.deepEqual(await shown(page), [ids[2]], '新分组里只有被移走的标签');
    await cmd(page, 'prev-group');
    assert.deepEqual(await shown(page), [ids[0], ids[1]]);
    await cmd(page, 'next-group');
    assert.deepEqual(await shown(page), [ids[2]]);
    await cmd(page, 'tab-joint-next');                       // 只有两个组: 绕回第 1 组
    assert.equal((await shown(page)).length, 3);
    await cmd(page, 'tab-separate'); await cmd(page, 'bind-window');
    assert.equal((await shown(page)).length, 3, '合并为一组');
    await cmd(page, 'tab-separate');
    page.once('dialog', (d) => d.accept());
    await cmd(page, 'group-close'); await page.waitForTimeout(200);
    assert.equal(await page.evaluate(() => TABS.size), 2);
  }));

test('(範囲選択) 次/前の変更行: 从光标到变更行建立选区', () =>
  withMocks({ name: 'gw', portOffset: 281 }, async ({ page, snap, press }) => {
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['a', 'b', 'c', 'd', 'e', 'f']; t.dirty = false; renderVim(t); });
    await page.locator('#ed-content').click();
    await press('Control+Home'); await press('ArrowDown', { times: 3 }); await page.keyboard.type('X');   // 改第 4 行
    await press('Control+Home');
    await cmd(page, 'modify-next-sel');
    const s = await snap();
    assert.deepEqual(s.sel.slice(0, 3), [0, 0, 3]);
  }));

test('纵向/横向最大化 (Tauri 窗口插件替身); SQL*Plus 在终端里执行当前文件', () =>
  withMocks({ name: 'gw', portOffset: 282, mock: { tauri: true } }, async ({ page, m }) => {
    await cmd(page, 'maximize-v'); await page.waitForTimeout(300);
    let ipc = await m.tauri.ipc();
    const sz = ipc.filter((c) => c.name === 'plugin:window|set_size').pop();
    assert.deepEqual(sz.args.value.data, { width: 800, height: 1080 }, '纵向: 宽不变, 高 = 显示器高');
    await cmd(page, 'maximize-h'); await page.waitForTimeout(300);
    ipc = await m.tauri.ipc();
    assert.deepEqual(ipc.filter((c) => c.name === 'plugin:window|set_size').pop().args.value.data, { width: 1920, height: 600 });
    await cmd(page, 'minimize-all'); await page.waitForTimeout(200);
    assert.ok((await m.tauri.ipc()).some((c) => c.name === 'plugin:window|minimize'));
  }));

test('再変換: 原生选区对齐到编辑器选区 (之后由 IME 的变换键接手)', () =>
  withMocks({ name: 'gw', portOffset: 283 }, async ({ page }) => {
    await fill(page, 'hello world');
    await page.evaluate(() => { const t = getActiveTab(); t.sel = { r0: 0, c0: 6, r1: 0, c1: 11 }; renderVim(t); dispatchCommand('reconvert'); });
    assert.equal(await page.evaluate(() => getSelection().toString()), 'world');
  }));

test('SQL*Plus: 正常路径在终端执行; 含引号/分隔符的路径被拒绝 (防命令注入)', () =>
  withMocks({ name: 'gw', portOffset: 284, mock: { tauri: true } }, async ({ page, m }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { getActiveTab().path = 'C:\proj\a.sql'; return dispatchCommand('sqlplus-run'); }); await page.waitForTimeout(1500);
    assert.match((await m.tauri.ipc()).filter((c) => c.name === 'terminal_input').map((c) => c.args.data).join(''), /sqlplus @"C:\proj\a\.sql"\r/);
    const before = (await m.tauri.ipc()).filter((c) => c.name === 'terminal_input').length;
    await page.evaluate(() => { getActiveTab().path = 'C:\proj\a" & calc & ".sql'; return dispatchCommand('sqlplus-run'); }); await page.waitForTimeout(500);
    assert.equal((await m.tauri.ipc()).filter((c) => c.name === 'terminal_input').length, before, '没有再发送任何输入');
  }));
