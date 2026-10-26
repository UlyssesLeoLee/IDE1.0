// 回归 (用户报告): 分割后无法输入文字 / 取消分割后也无法输入; 正则查找里输入 * 无法使用
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const idle = (page) => page.evaluate(() => window.__editIdle());

for (const [mode, panes] of [['split-h', 1], ['split-v', 1], ['split-quad', 3]]) {
  test(`${mode}: 分割后能输入; 点另一个视图在那里输入; 取消分割后还能输入`, () =>
    withMocks({ name: 'sr', portOffset: 480 + ['split-h', 'split-v', 'split-quad'].indexOf(mode) }, async ({ page, snap }) => {
      await fill(page, 'one\ntwo\nthree');
      await page.evaluate((m) => dispatchCommand(m), mode);
      assert.equal(await page.locator('#editor-area > .split-view').count(), panes, '其余视图是镜像');
      assert.notEqual(await page.evaluate(() => getComputedStyle(document.getElementById('editor')).display), 'none', '编辑器没有被隐藏');
      await page.locator('#ed-content').click();
      await page.keyboard.press('Control+Home');
      await page.keyboard.type('A'); await idle(page);
      assert.equal((await snap()).lines[0], 'Aone', '分割后在原视图输入');
      // 镜像里要跟着出现新文字
      await page.waitForTimeout(400);
      const mirrorText = await page.locator('#editor-area > .split-view').first().innerText();
      assert.match(mirrorText, /Aone/, '镜像实时跟随文档');
      // 点另一个视图 -> 编辑器换到那个格子里, 照样能输入
      await page.locator('#editor-area > .split-view').first().dispatchEvent('mousedown');
      await page.waitForTimeout(200);
      assert.equal(await page.locator('#editor-area > .split-view').count(), panes, '视图个数不变');
      await page.locator('#ed-content').click();
      await page.keyboard.press('Control+End');
      await page.keyboard.type('Z'); await idle(page);
      assert.equal((await snap()).lines[2], 'threeZ', '换到另一个视图后能输入');
      // 取消分割: 编辑器回来, 镜像全部移除, 照样能输入
      await page.evaluate((m) => dispatchCommand(m), mode);
      assert.equal(await page.locator('#editor-area > .split-view').count(), 0, '取消分割后没有镜像');
      assert.notEqual(await page.evaluate(() => getComputedStyle(document.getElementById('editor')).display), 'none');
      await page.locator('#ed-content').click();
      await page.keyboard.press('Control+Home');
      await page.keyboard.type('B'); await idle(page);
      assert.equal((await snap()).lines[0], 'BAone', '取消分割后能输入');
    }));
}

test('正则查找: 输入 * 不报错也不吞字; a.*f / o\\*b 可用', () =>
  withMocks({ name: 'sr', portOffset: 490 }, async ({ page, snap, press }) => {
    await fill(page, 'foo*bar\nabc  def\nxyz');
    await press('Control+Home');
    await press('Control+f');
    await page.check('#find-regex');
    await page.locator('#find-input').click();
    await page.keyboard.type('a.*f'); // 真实键盘逐字输入, 中途有一刻输入框里只有 "a." 再到 "a.*"
    assert.equal(await page.inputValue('#find-input'), 'a.*f', '* 没有被吞');
    await page.keyboard.press('Enter'); await idle(page);
    assert.deepEqual((await snap()).sel, [1, 0, 1, 8], 'a.*f 命中 abc  def 整段 (贪婪)');
    // 单独的 * 是无效正则: 给出提示而不是什么都没有
    await page.fill('#find-input', '*');
    await page.waitForTimeout(300);
    assert.match(await page.locator('#find-status').innerText(), /正则|无效/);
    // 转义的 \* 匹配字面星号
    await page.fill('#find-input', 'o\\*b');
    await page.keyboard.press('Enter'); await idle(page);
    assert.deepEqual((await snap()).sel, [0, 2, 0, 5]);
  }));

test('インクリメンタルサーチ (正则): 逐字输入 a.*f 能边输入边选中', () =>
  withMocks({ name: 'sr', portOffset: 491 }, async ({ page, snap, press }) => {
    await fill(page, 'foo*bar\nabc  def\nxyz');
    await press('Control+Home');
    await page.evaluate(() => dispatchCommand('isearch-regex-fwd'));
    await page.waitForSelector('#is-input');
    await page.locator('#is-input').click();
    await page.keyboard.type('a.*f'); await idle(page);
    assert.equal(await page.inputValue('#is-input'), 'a.*f', '* 没有被吞');
    assert.deepEqual((await snap()).sel, [1, 0, 1, 8]);
    assert.doesNotMatch(await page.locator('#is-count').innerText(), /无效|見つかりません/);
  }));
