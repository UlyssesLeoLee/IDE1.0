// 回归: 命令级录制的键盘宏 —— 录下键入的文字和执行的命令, 停止后生成 Sakura 格式 .mac, 能被 Rust 解析器接受并回放得到同样的结果
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('录制 -> 生成 .mac -> 回放: 结果一致; 认不出的命令会列出', () =>
  withMocks({ name: 'mr', portOffset: 597 }, async ({ page, press }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['']; t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t); });
    await page.evaluate(() => dispatchCommand('macro-rec-mac'));          // 开始
    await page.locator('#ed-content').focus();
    await page.keyboard.type('hello');
    await press('Enter');
    await page.keyboard.type('x');
    await page.evaluate(async () => { await __editIdle(); });
    await press('Control+i');                                              // 复制行 (Sakura 默认键)
    await page.evaluate(async () => { await __editIdle(); });
    const before = await page.evaluate(() => getActiveTab().lines.slice());
    assert.deepEqual(before, ['hello', 'x', 'x']);
    await page.evaluate(() => dispatchCommand('macro-rec-mac'));          // 停止 -> 新标签放生成的 .mac
    await page.waitForFunction(() => /\.mac$/.test(getActiveTab().name || ''), null, { timeout: 8000 });
    const mac = await page.evaluate(() => getActiveTab().lines.join('\n'));
    assert.match(mac, /S_InsText\("hello"\);/);
    assert.match(mac, /S_Char\(13\);/);
    assert.match(mac, /S_DuplicateLine\(\);/);
    // 回放: 清空文档, 运行生成的宏, 得到同样的三行
    await page.evaluate(async () => {
      const src = getActiveTab().lines.join('\n');
      const t0 = [...TABS.values()].find((t) => !/\.mac$/.test(t.name)); setActiveTab(t0.id);
      t0.lines = ['']; t0.cursor = { row: 0, col: 0 }; t0.sel = null; renderVim(t0);
      await runSakuraMacro(src);
    });
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), before);
  }));
