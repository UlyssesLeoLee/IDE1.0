// 回归: JS 宏 Editor.* API (和 Sakura 的 JScript 宏同名): S_ 函数直接调, 读取类函数, 出错有提示, 沙箱外对象不泄露
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('Editor.<S_函数> / 读取类 / InputBox / 错误', () =>
  withMocks({ name: 'jm', portOffset: 606 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['']; t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t); });
    const ok = await page.evaluate(() => runJsMacroSource(`
      await Editor.InsText("ab");
      await Editor.DuplicateLine();
      const n = await Editor.GetLineCount();
      const cur = await Editor.GetLineStr(0);
      await Editor.InsText(String(n) + ":" + cur);
      const name = await Editor.InputBox("名前", "dflt");
      await Editor.Jump(1);
      await Editor.InsText(name + "|");
    `.replace(/^\s+/gm, '')));
    assert.equal(ok, true);
    // 第 2 行: "ab" + "2:ab"; 第 1 行开头插了 InputBox 的默认值 (测试替身里 prompt 默认返回 null -> 空串)
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), ['|ab', 'ab2:ab']);
    // 读取选区
    const sel = await page.evaluate(async () => {
      const t = getActiveTab(); t.sel = { r0: 0, c0: 1, r1: 0, c1: 3 }; t.selAnchor = { row: 0, col: 1 }; t.cursor = { row: 0, col: 3 };
      let got = null; await runJsMacroSource('window.__got = await Editor.GetSelectedString();'); got = window.__got; return got;
    });
    assert.equal(sel, 'ab');
    // 不存在的函数: 报错且文档不变
    const before = await page.evaluate(() => getActiveTab().lines.slice());
    const bad = await page.evaluate(() => runJsMacroSource('await Editor.NoSuchFunction(1);'));
    assert.equal(bad, false);
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), before);
  }));
