// 回归: 状态栏字段 (WBS-02 5.7) —— 光标处字符码位 / EOL / EOF, 总行数
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

test('状态栏显示光标处字符的码位, 行尾 EOL, 末行行尾 EOF, 位置提示里有总行数', () =>
  withMocks({ name: 'sf', portOffset: 632 }, async ({ page, press }) => {
    await fill(page, 'Ab' + String.fromCharCode(10) + '漢');
    const code = () => page.evaluate(() => document.getElementById('s-code').textContent);
    await press('Control+Home');
    assert.equal(await code(), 'U+0041');
    await press('End'); assert.equal(await code(), 'EOL');
    await press('ArrowDown'); await press('Home'); assert.equal(await code(), 'U+6F22');
    await press('End'); assert.equal(await code(), 'EOF');
    assert.match(await page.evaluate(() => document.getElementById('s-pos').title), /共 2 行/);
  }));

test('宏录制中状态栏显示 ● REC, 停止后隐藏', () =>
  withMocks({ name: 'sf', portOffset: 633 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => dispatchCommand('macro-toggle'));
    await page.waitForFunction(() => !document.getElementById('s-rec').hidden);
    assert.match(await page.evaluate(() => document.getElementById('s-rec').textContent), /REC/);
    await page.evaluate(() => dispatchCommand('macro-toggle'));
    await page.waitForFunction(() => document.getElementById('s-rec').hidden);
  }));

test('显示列: 全角占 2 格, Tab 补到 Tab 位; 纯 ASCII 行不加后缀', () =>
  withMocks({ name: 'sf', portOffset: 636 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const r = await page.evaluate(() => {
      const t = getActiveTab(); t.lines = ['ab', '漢字x', String.fromCharCode(9) + 'y']; t.cursor = { row: 0, col: 2 }; updateCaretColumn();
      const ascii = document.getElementById('s-pos').textContent;
      t.cursor = { row: 1, col: 2 }; updateCaretColumn(); const cjk = document.getElementById('s-pos').textContent;
      t.cursor = { row: 2, col: 1 }; updateCaretColumn(); const tab = document.getElementById('s-pos').textContent;
      return { ascii, cjk, tab, w: [cellWidth(0x41), cellWidth(0x6f22), cellWidth(0x301), displayCol('a' + String.fromCharCode(9) + 'b', 2, 4)] };
    });
    assert.equal(r.ascii, 'Ln 1, Col 3');
    assert.equal(r.cjk, 'Ln 2, Col 3 (表示 5)');
    assert.equal(r.tab, 'Ln 3, Col 2 (表示 5)');
    assert.deepEqual(r.w, [1, 2, 0, 4]);
  }));
