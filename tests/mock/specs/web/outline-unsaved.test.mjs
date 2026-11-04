// 回归: 大纲按编辑器里的正文分析 (WBS-02 5.10) —— 未保存的修改、无标题文档也有大纲
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('无标题 / 未保存的 Markdown 文档有大纲, 且跟着正文走', () =>
  withMocks({ name: 'ou', portOffset: 631 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.name = 'note.md'; t.path = ''; t.untitled = true; t.lines = ['# 标题甲', 'x', '## 小节乙']; renderVim(t); });
    await page.evaluate(() => { showSidePane('outline'); });
    await page.waitForFunction(() => document.querySelectorAll('#outline-list .tree-node').length === 2, null, { timeout: 8000 });
    const names = await page.evaluate(() => [...document.querySelectorAll('#outline-list .tree-node')].map((n) => n.textContent));
    assert.match(names.join('|'), /标题甲.*小节乙/);
    await page.evaluate(() => { const t = getActiveTab(); t.lines.push('### 新增丙'); renderVim(t); showSidePane('outline'); });
    await page.waitForFunction(() => document.querySelectorAll('#outline-list .tree-node').length === 3, null, { timeout: 8000 });
  }));
