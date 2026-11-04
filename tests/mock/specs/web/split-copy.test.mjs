// 回归: 分屏后点视图底边 -> 该视图得到独立副本, 可单独修改、单独保存 (另存为); 原文档不受影响
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('点分屏底边创建独立副本: 改副本不动原文档, 点回另一视图原文档还在', () =>
  withMocks({ name: 'splitcopy', portOffset: 571 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['alpha', 'beta']; t.dirty = false; renderVim(t); window.__orig = t.id; splitView('v'); });
    await page.waitForSelector('.split-view[data-edges]');
    assert.equal(await page.evaluate(() => document.querySelectorAll('.split-copy-bar').length), 0, '文字下面不再有额外的"底边"条');
    // 点分割线 (镜像视图靠近编辑器的那条边) = 在这个视图创建独立副本
    const r = await page.evaluate(() => { const b = document.querySelector('.split-view').getBoundingClientRect(); return { x: b.left + 2, y: b.top + b.height / 2 }; });
    await page.mouse.click(r.x, r.y);
    await page.waitForFunction(() => getActiveTab().untitled === true, null, { timeout: 8000 });
    const s = await page.evaluate(() => { const t = getActiveTab(); return { id: t.id, orig: window.__orig, untitled: t.untitled, dirty: t.dirty, lines: t.lines, name: t.name, pinned: Object.values(SPLIT.pin) }; });
    assert.notEqual(s.id, s.orig, '活动文档换成了副本');
    assert.equal(s.untitled, true, '副本无路径, 保存 = 另存为');
    assert.deepEqual(s.lines, ['alpha', 'beta']);
    assert.deepEqual(s.pinned, [s.orig], '另一视图固定显示原文档');
    await page.evaluate(() => { const t = getActiveTab(); t.lines[0] = 'CHANGED'; renderVim(t); });
    await page.waitForTimeout(400);
    const txt = await page.evaluate(() => document.querySelector('.split-view').textContent);
    assert.match(txt, /alpha/, '镜像视图仍是原文档');
    await page.evaluate(() => document.querySelector('.split-view').dispatchEvent(new MouseEvent('mousedown', { bubbles: true })));
    const back = await page.evaluate(() => ({ id: getActiveTab().id, orig: window.__orig, l0: getActiveTab().lines[0] }));
    assert.equal(back.id, back.orig);
    assert.equal(back.l0, 'alpha', '原文档未被副本的修改影响');
  }));
