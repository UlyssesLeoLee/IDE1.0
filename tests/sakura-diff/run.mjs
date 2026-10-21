// 用法: QA_BIN=<ide-shell-web.exe> node run.mjs [过滤子串] [--md report.md]
// 逐条行为比对: 真实 Sakura (键盘宏) vs 我们的编辑器 (真实按键), 结果文本逐字比较。
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import CASES1, { SEED } from './cases.mjs';
import CASES2 from './cases2.mjs';
import CASES3, { SEED as SEED3 } from './cases3.mjs';
const CASES = [...CASES1, ...CASES2, ...CASES3];
import { runSakura, hasSakura } from './sakura.mjs';
import { withMocks } from '../mock/lib/with-mocks.mjs';

const filter = process.argv[2] && !process.argv[2].startsWith('--') ? process.argv[2] : '';
const mdIdx = process.argv.indexOf('--md');
const mdOut = mdIdx > 0 ? process.argv[mdIdx + 1] : null;
if (!hasSakura) { console.log('没有找到 Sakura Editor (SAKURA_EXE), 跳过'); process.exit(0); }

function setClipboard(text) {
  const dir = fileURLToPath(new URL('./', import.meta.url));
  const f = dir + '.clip.txt';
  fs.writeFileSync(f, text.split('\n').join('\r\n'), 'utf8');
  execFileSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', dir + 'setclip.ps1', f]);
}

function tailOf(c) { return c.tail || (c.sel || /_SEL|SELECT/.test(c.id) ? 'selcopy' : 'marker'); }
function sakuraMacro(c) {
  const m = ['S_GoFileTop();'];
  if (c.sel) { m.push(`S_MoveCursor(${c.sel[0] + 1}, ${c.sel[1] + 1}, 0);`, `S_MoveCursor(${c.sel[2] + 1}, ${c.sel[3] + 1}, 1);`); }
  else { const cur = c.cur || [0, 0]; m.push(`S_MoveCursor(${cur[0] + 1}, ${cur[1] + 1}, 0);`); }
  m.push(...c.sk);
  const tail = tailOf(c);
  if (tail === 'paste') m.push('S_GoFileEnd();', 'S_Paste();');
  else if (tail === 'selcopy') m.push('S_Copy();', 'S_GoFileEnd();', 'S_Paste();');
  else m.push('S_InsText("|");');
  return m;
}

const results = [];
const todo = CASES.filter((c) => !filter || c.id.includes(filter));
// 我们的编辑器: 一个页面里顺序跑完所有用例 (每例重置文档)
async function oursAll() {
  const out = new Map();
  await withMocks({ name: 'skd', portOffset: 400, viewport: { width: 1280, height: 800 } }, async ({ page }) => {
    await page.locator('#ed-content').click();
    for (const c of todo) {
      const seed = c.seed || SEED;
      await page.evaluate(({ c, seed }) => {
        const t = getActiveTab();
        t.lines = seed.replace(/\n$/, '').split('\n'); if (seed.endsWith('\n')) t.lines.push('');
        Object.assign(t, { cursor: { row: 0, col: 0 }, sel: null, colSel: null, selAnchor: null, dirty: false });
        delete t.goalCol; t.undo = []; t.redo = [];
        window.__mock.clip = c.clip || ''; CLIPBOARD = c.clip || '';
        if (c.sel) { t.sel = { r0: c.sel[0], c0: c.sel[1], r1: c.sel[2], c1: c.sel[3] }; t.selAnchor = { row: c.sel[0], col: c.sel[1] }; t.cursor = { row: c.sel[2], col: c.sel[3] }; }
        else t.cursor = { row: (c.cur || [0, 0])[0], col: (c.cur || [0, 0])[1] };
        renderVim(t); edContent.focus({ preventScroll: true });
      }, { c, seed });
      if (c.ours.js) { await page.evaluate((src) => { new Function(src)(); }, c.ours.js); }
      else if (c.ours.key) { await page.locator('#ed-content').focus(); await page.keyboard.press(c.ours.key); } else await page.evaluate((x) => dispatchCommand(x), c.ours.cmd);
      await page.waitForTimeout(250);
      await page.evaluate(({ tail }) => {
        const t = getActiveTab();
        if (tail === 'selcopy') { try { dispatchCommand('copy'); } catch (_) {} }
        if (tail === 'paste' || tail === 'selcopy') { const clip = CLIPBOARD || ''; t.cursor = { row: t.lines.length - 1, col: charLen(t.lines[t.lines.length - 1]) }; t.sel = null; t.colSel = null; pasteText(tail === 'selcopy' ? clip : (window.__mock.clip || CLIPBOARD)); }
        else { if (selNonEmpty(t)) deleteSelection(t); insertCharMulti('|'); }
      }, { tail: tailOf(c) });
      out.set(c.id, await page.evaluate(() => getActiveTab().lines.join('\n')));
    }
  });
  return out;
}

const ours = await oursAll();
for (const c of todo) {
  if (c.clip !== undefined) setClipboard(c.clip); else setClipboard('');
  const r = await runSakura(c.seed || SEED, sakuraMacro(c));
  const o = ours.get(c.id);
  const same = r.finished && r.text === o;
  results.push({ id: c.id, same, finished: r.finished, sakura: r.text, ours: o });
  console.log((same ? 'OK   ' : 'DIFF ') + c.id + (r.finished ? '' : '  (Sakura 宏未正常结束)'));
  if (!same) { console.log('  sakura:', JSON.stringify(r.text)); console.log('  ours  :', JSON.stringify(o)); }
}
const bad = results.filter((r) => !r.same);
console.log(`\n${results.length - bad.length}/${results.length} 一致, ${bad.length} 处差异`);
if (mdOut) {
  fs.writeFileSync(mdOut, `# Sakura 行为比对\n\n${results.length - bad.length}/${results.length} 一致\n\n` + bad.map((r) => `## ${r.id}\n- Sakura: \`${JSON.stringify(r.sakura)}\`\n- 本项目: \`${JSON.stringify(r.ours)}\`\n`).join('\n'));
}
