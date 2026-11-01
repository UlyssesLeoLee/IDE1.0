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
