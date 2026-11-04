// 回归: タイプ別設定 —— 每种文件类型各自的 Tab 宽度 / Tab 插入空格 / 自动缩进 / 行注释符; OK / 取消; 不影响别的类型
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const setup = (page, name, lines) => page.evaluate(([n, l]) => {
  const t = getActiveTab(); t.name = n; t.path = ''; delete t.language; t.lines = l.slice(); t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t);
}, [name, lines]);

test('类型别设置: 改 Rust 的 Tab 宽度 / 插入空格 / 行注释; 取消不保存; 别的类型不受影响', () =>
  withMocks({ name: 'ts', portOffset: 605 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await setup(page, 'a.rs', ['fn a() {', '    x();', '}']);
    await page.evaluate(() => dispatchCommand('type-config'));
    await page.waitForSelector('#ts-lang');
    assert.equal(await page.evaluate(() => document.getElementById('ts-lang').value), 'rust', '默认选中当前文档的类型');
    // 取消: 改了也不保存
    await page.fill('#ts-tab', '2');
    await page.click('#md-foot button:nth-child(2)');
    assert.deepEqual(await page.evaluate(() => typeSettingsAll()), {}, '取消 = 不保存');
    // OK
    await page.evaluate(() => dispatchCommand('type-config'));
    await page.waitForSelector('#ts-lang');
    await page.fill('#ts-tab', '2');
    await page.check('#ts-spaces');
    await page.fill('#ts-comment', '#');
    await page.click('#md-foot button:nth-child(1)');
    const saved = await page.evaluate(() => typeSettingsAll());
    assert.deepEqual(saved.rust, { tabWidth: 2, insertSpaces: true, lineComment: '#' }, '只记和默认不同的项');
    assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--tab-size').trim()), '2', 'Rust 文档用 2 宽的 Tab');
    // Tab 键: 补到下一个 Tab 位 (宽 2): 光标在第 0 列 -> 2 个空格
    await page.locator('#ed-content').focus();
    await press('Tab');
    await page.waitForFunction(() => getActiveTab().lines[0] === '  fn a() {', null, { timeout: 8000 });
    // 行注释: 用这个类型的注释符 '#'
    await page.evaluate(() => { const t = getActiveTab(); t.sel = { r0: 0, c0: 0, r1: 1, c1: 3 }; t.cursor = { row: 1, col: 3 }; renderVim(t); });
    await page.evaluate(() => dispatchCommand('toggle-line-comment'));
    await page.waitForFunction(() => getActiveTab().lines[0] === '  # fn a() {', null, { timeout: 8000 });
    // 别的类型 (Python) 不受影响: 仍是共通设置的 Tab 宽度, Tab 是制表符
    await setup(page, 'b.py', ['x = 1']);
    await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue('--tab-size').trim() === '4', null, { timeout: 8000 });
    assert.equal(await page.evaluate(() => tabTextFor(getActiveTab())), '\t');
    assert.equal(await page.evaluate(() => typeSetting(getActiveTab(), 'lineComment')), '#', 'Python 内建的行注释符');
  }));
