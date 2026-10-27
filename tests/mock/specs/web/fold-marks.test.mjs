// 回归: 折叠时标记的行对位 —— 折叠隐藏的行不占高度也不占行号栏位置, 书签 / 查找标记要按可见行画
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('折叠后: 书签落在行号栏对的数字上; 隐藏行里的查找标记不画', () =>
  withMocks({ name: 'fm', portOffset: 543 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(async () => {
      const t = getActiveTab();
      t.lines = ['a0', 'fn f() {', '  x();', '  y();', '}', 'tail5', 'tail6'];
      t.cursor = { row: 1, col: 0 }; t.sel = null;
      renderVim(t);
      await foldAtCursor(t);                                  // 折起第 1..4 行
      t.cursor = { row: 5, col: 0 }; renderVim(t);
      toggleBookmark();                                       // 书签在第 5 行 (行号 6)
      FIND_STATE.needle = 'x'; FIND_STATE.marks = true;
      scheduleMarks();
    });
    await page.waitForTimeout(300);
    const r = await page.evaluate(() => {
      const g = CSS.highlights.get('bookmark-gutter');
      return { gutter: document.getElementById('gutter') ? document.getElementById('gutter').textContent : null, bm: g ? [...g].map((x) => x.toString()) : [], hidden: document.querySelectorAll('#ed-content .fold-hidden').length };
    });
    assert.equal(r.hidden, 3, '第 2..4 行隐藏 (起点行 1 仍显示)');
    assert.deepEqual(r.bm, ['6'], '书签标在行号 6 上, 不是被隐藏行号挤偏的别的数字');
  }));
