// 覆盖审计: SPA-03 附录 A 的每一条 Sakura 命令 — 用它的默认键 (没有默认键就走命令) 真的驱动一次,
// 前后抓取快照, 必须有可观察的变化 (文档 / 光标 / 选区 / 标签 / 对话框 / 状态栏 / 剪贴板 / 滚动 / 界面标记)。
// 对照表见 fixtures/sakura-map.mjs。失败会列出具体的 F_ 命令 ID。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';
import { MAP } from '../../fixtures/sakura-map.mjs';

const SEED = ['hello world foo', '  indented line', '', 'third line here', 'ünï 日本語 ABC', 'last line'];
const LONG = Array.from({ length: 200 }, (_, i) => 'line ' + i + ' some words here ' + i);
const CHUNK = 30;

const snapshot = (page) => page.evaluate(() => {
  const t = getActiveTab() || {};
  const q = (s) => { const e = document.querySelector(s); return e ? e.outerHTML.length : -1; };
  return JSON.stringify({
    lines: (t.lines || []).join('\n'), cur: t.cursor, sel: t.sel, col: t.colSel, anchor: t.selAnchor, tabs: TABS.size, active: activeTabId,
    modal: document.getElementById('modal').classList.contains('show'), modalBody: document.getElementById('md-body').innerText.slice(0, 80),
    tip: document.getElementById('s-tip').textContent, status: document.getElementById('status').innerText,
    clip: window.__mock.clip + '|' + CLIPBOARD, scroll: document.getElementById('ed-content').scrollTop,
    html: document.documentElement.outerHTML.length > 0 ? Array.from(document.documentElement.attributes).map((a) => a.name + '=' + a.value).join(';') : '',
    chrome: [q('#menubar'), q('#side-bar'), q('#bottom-panel'), q('#viewbars-top'), q('#funckeys'), q('#grep-win'), q('#tabs'), q('#editor-area')].join(','),
    isearch: !!(document.getElementById('isearch') && !document.getElementById('isearch').hidden), ro: !!t.readOnly, eol: t.eol, enc: t.encoding, diff: !!t.diff,
    ovr: typeof OVERWRITE !== 'undefined' ? OVERWRITE : null, mac: typeof MACRO !== 'undefined' ? MACRO.recording + ':' + MACRO.keys.length : '', bm: JSON.stringify([...BOOKMARKS.entries()]),
    marks: Object.fromEntries(['find-marks', 'diff-chg', 'diff-add'].map((k) => [k, CSS.highlights.get(k) ? CSS.highlights.get(k).size : 0])),
    wrap: VIEW_STATE.wordWrap, dlg: window.__mock.dialogs.length, order: [...TABS.keys()].join(','), selmode: !!t.selMode, eolx: t.eol, ws: Array.from(document.querySelectorAll('.menu-dropdown.open, .menu-item.open')).length,
  });
});

async function reset(page, root, e) {
  await page.evaluate(async ({ root, e, SEED, LONG }) => {
    closeModal();
    try { isearchClose(false); } catch (_) {}
    for (const id of [...TABS.keys()]) closeTab(id);
    await openFile(root + '/' + (e.file || 'f.txt'));
    for (const [id, tt] of [...TABS]) if (tt.untitled && TABS.size > 1) closeTab(id);
    const t = getActiveTab();
    if (!e.file || e.text) t.lines = e.text ? e.text.slice() : (e.long ? LONG.slice() : SEED.slice());
    Object.assign(t, { cursor: { row: Math.min(1, t.lines.length - 1), col: Math.min(5, (t.lines[1] || '').length) }, sel: null, colSel: null, selAnchor: null, dirty: false, readOnly: false, diff: null, baseLines: null, eol: 'crlf' });
    delete t.goalCol;
    window.__mock.clip = e.clip || ''; CLIPBOARD = e.clip || '';
    window.__mock.dialogs.length = 0;
    document.documentElement.style.cssText = '';
    if (e.tabs === 2) { dispatchCommand('new-file'); setActiveTab([...TABS.keys()][0]); }
    renderVim(t);
    if (e.sel) { t.sel = { r0: e.sel[0], c0: e.sel[1], r1: e.sel[2], c1: e.sel[3] }; t.selAnchor = { row: e.sel[0], col: e.sel[1] }; t.cursor = { row: e.sel[2], col: e.sel[3] }; renderVim(t); }
    if (e.pre) { new Function('t', e.pre)(getActiveTab()); renderVim(getActiveTab()); }
    edContent.focus({ preventScroll: true });
  }, { root, e, SEED, LONG });
}

const chunks = [];
for (let i = 0; i < MAP.length; i += CHUNK) chunks.push(MAP.slice(i, i + CHUNK));

test('映射表覆盖 SPA-03 附录 A 的全部命令 ID', () => {
  const doc = fs.readFileSync(path.resolve(import.meta.dirname, '../../../../docs/SPA-03_v1-gap-design.md'), 'utf8');
  const ids = [...doc.matchAll(/^\| (?:✅|🟡|⚪|⬜) \| `(F_[A-Za-z0-9_]+)`/gm)].map((m) => m[1]);
  const mapped = new Set(MAP.map((m) => m[0]));
  const missing = [...new Set(ids)].filter((x) => !mapped.has(x));
  assert.ok(ids.length >= 260, '附录 A 解析到 ' + ids.length + ' 条');
  assert.deepEqual(missing, [], '附录 A 里有命令没有出现在对照表中');
});

chunks.forEach((chunk, ci) => {
  test(`Sakura 命令实测 #${ci + 1} (${chunk[0][0]} … ${chunk[chunk.length - 1][0]})`, () =>
    withMocks({ name: 'sc', portOffset: 300 + ci, viewport: { width: 1280, height: 800 } }, async ({ page, root, app }) => {
      const files = { 'f.txt': SEED.join('\n'), 'f.rs': 'fn a() {}\n\nfn b() {}\n\nfn c() {}\n', 'f.c': '#include "f.h"\nint main(){}\n', 'f.h': 'int x;\n' };
      for (const [n, c] of Object.entries(files)) fs.writeFileSync(path.join(root, n), c);
      await page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); await setProjectRoot(r, true); }, root);
      const errs = [];
      page.on('pageerror', (er) => errs.push(String(er.message || er)));
      const failed = [];
      for (const [id, key, cmd, opts = {}] of chunk) {
        const e = { ...opts };
        if (e.file && e.fileText) fs.writeFileSync(path.join(root, e.file), e.fileText);
        for (const [n, c] of Object.entries(e.extra || {})) fs.writeFileSync(path.join(root, n), c);
        try {
          await page.evaluate(() => window.__editIdle());
          await reset(page, root, e);
          await page.evaluate(() => window.__editIdle());   // pre 里的 insertChar / doUndo 经桥是异步的: 落地之后再取"之前"的快照, 否则 F_REDO 这类用例会随机地和"之后"相等
          const before = await snapshot(page);
          const nerr = errs.length;
          if (key) { await page.locator('#ed-content').focus(); await page.keyboard.press(key); } else await page.evaluate((c) => dispatchCommand(c), cmd);
          await page.evaluate(() => window.__editIdle());   // 经 Rust 桥的命令是异步的
          await page.waitForTimeout(450);
          const after = await snapshot(page);
          if (errs.length > nerr) failed.push(`${id}: 抛出异常 ${errs.slice(nerr).join(' | ')}`);
          else if (!e.any && before === after) failed.push(`${id}: ${key || cmd} 没有产生任何可观察的变化`);
        } catch (er) { failed.push(`${id}: 驱动失败 ${String(er.message).split('\n')[0]}`); }
      }
      assert.deepEqual(failed, [], '\n' + failed.join('\n'));
    }));
});
