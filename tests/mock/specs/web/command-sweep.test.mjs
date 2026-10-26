// 回归 (用户报告「分割后无法输入」类问题的系统性排查):
// 把菜单里的每一个命令都执行一遍, 每执行一个就检查: 没有抛 JS 异常, 并且编辑区仍然能输入文字。
// 会让页面离开 / 阻塞 / 真正改系统状态的命令在 SKIP 里 (原因写在旁边); 本来就会让编辑区不可输入的开关 (只读 / Vim) 在 TOGGLES 里, 做完立即还原。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const SKIP = new Set([
  'exit-all', 'new-window', 'close-all', 'close-open', 'save-close', 'close-tab',   // 关窗口 / 关文档: 之后没有可输入的文档
  'reload', 'print', 'print-preview', 'print-setup', 'browse', 'fullscreen',        // 重载页面 / 系统对话框 / 全屏
  'macro-start', 'macro-stop', 'macro-play',                                        // 录制状态会吞按键, 由宏的专门用例覆盖
  'exec-cmd', 'sqlplus-activate', 'sqlplus-run', 'new-terminal',                    // 起外部进程: 由 terminal.test.mjs 覆盖
  'minimize-all', 'minimize-window', 'topmost', 'maximize-v', 'maximize-h',        // 改桌面窗口
  'import-config', 'export-config', 'reset-settings',                               // 改全局设置
  'open-folder', 'open-file', 'pick-folder', 'file-tree',                           // 系统对话框
]);
const TOGGLES = ['toggle-readonly', 'toggle-vim', 'toggle-word-wrap', 'select-count-mode', 'layout-min', 'layout-full', 'tmp-wrap', 'wrap-window-width'];

function ids(menu) {
  const out = [];
  const walk = (x) => {
    if (typeof x === 'string') { if (x !== '-' && !x.startsWith('@')) out.push(x); }
    else if (Array.isArray(x)) x.forEach(walk);
    else if (x && x.i) x.i.forEach(walk);
  };
  menu.forEach((m) => walk(m[2]));
  return [...new Set(out)];
}

test('菜单里的每个命令执行后: 无异常, 编辑区仍能输入', () =>
  withMocks({ name: 'sw', portOffset: 520, viewport: { width: 1280, height: 800 } }, async ({ page, snap }) => {
    const errs = [];
    page.on('pageerror', (e) => errs.push(String(e.message || e)));
    await fill(page, 'alpha\nbeta\ngamma');
    const all = await page.evaluate(`(${ids.toString()})(MENU_DEF)`);
    const bad = [];
    for (const id of all) {
      if (SKIP.has(id)) continue;
      const n0 = errs.length;
      try {
        await page.evaluate((c) => dispatchCommand(c), id);
        await page.evaluate(() => window.__editIdle()).catch(() => {});
        await page.waitForTimeout(40);
        // 收拾残局: 关对话框 / 浮窗 / 菜单, 把只读等开关还原
        await page.evaluate(() => {
          try { closeModal(); } catch (_) {}
          for (const el of document.querySelectorAll('#sql-win.on, .menu-item.open')) el.classList.remove('on', 'open');
          const t = getActiveTab(); if (t) { t.readOnly = false; t.colSel = null; }
          try { if (typeof ISEARCH !== 'undefined' && ISEARCH.bar) ISEARCH.bar.hidden = true; } catch (_) {}
        });
        if (TOGGLES.includes(id)) { await page.evaluate((c) => dispatchCommand(c), id); await page.evaluate(() => window.__editIdle()).catch(() => {}); await page.evaluate(() => { try { closeModal(); } catch (_) {} }); }
        // 能不能输入: 重置文档 -> 聚焦 -> 打一个字符
        await page.evaluate(() => {
          const t = getActiveTab(); t.lines = ['alpha', 'beta', 'gamma']; t.cursor = { row: 2, col: 5 }; t.sel = null; t.selAnchor = null; t.colSel = null; t.readOnly = false;
          renderVim(t);
        });
        await page.locator('#ed-content').click({ timeout: 3000 });
        await page.keyboard.press('Control+End');
        await page.keyboard.type('q');
        await page.evaluate(() => window.__editIdle());
        const last = (await snap()).lines[2];
        if (last !== 'gammaq') bad.push(`${id}: 执行之后输入 q 得到 ${JSON.stringify(last)}`);
      } catch (e) {
        bad.push(`${id}: ${String(e.message).split('\n')[0]}`);
      }
      if (errs.length > n0) bad.push(`${id}: 抛出异常 ${errs.slice(n0).join(' | ')}`);
    }
    assert.deepEqual(bad, [], '\n' + bad.join('\n'));
  }));
