// char 语义专项测试 —— 光标列按 code point 而非 UTF-16 code unit.
//
// 背景: JS 的 String.length / slice / indexOf 都按 UTF-16 code unit 计数,
// 而补充平面字符 (emoji、CJK 扩展 B 补字如 𠮷) 是 surrogate pair, 占 2 个
// code unit. 修复前前端 146 处用 .length, 在这些字符上光标会错位.
//
// 日语常用汉字多在 BMP 内 (1 unit == 1 char), 所以日文测试看不出问题 ——
// 这些用例专门覆盖补充平面, 并同时验证 BMP 内行为未回归.
//
// 运行: node char-semantics.spec.mjs   (需先起 ide-shell-web, 见文件头)

import { chromium } from '@playwright/test';

const BASE = process.env.IDE_WEB_URL || 'http://127.0.0.1:8123';
const browser = await chromium.launch();
const page = await browser.newPage();
const errs = [];
page.on('pageerror', (e) => errs.push(e.message));

await page.goto(BASE + '/editor', { waitUntil: 'domcontentloaded' });
// Wait for editor to be fully initialized
await page.waitForFunction(() => {
  const ready = window.__ideReady ? window.__ideReady() : false;
  const tabs = window.TABS;
  console.log('[test] __ideReady:', ready, 'TABS:', tabs ? 'defined' : 'undefined', 'window.TABS:', window.TABS);
  return window.__ideReady() === true && window.TABS !== undefined;
});
await page.waitForTimeout(500);

const results = {};
const check = (name, ok, detail) => {
  results[name] = ok;
  if (!ok) console.log(`  ✗ ${name}${detail ? ' -> ' + JSON.stringify(detail) : ''}`);
};

// ---------- 1. 工具函数本身 ----------
console.log('=== 1. char 工具函数 ===');
const utils = await page.evaluate(() => ({
  chars_emoji: chars('a😀b').length,
  chars_kanjiExtB: chars('𠮷').length,
  chars_jp: chars('日本語').length,
  u16Index_emoji: u16Index('a😀b', 2),
  charSlice_emoji: charSlice('a😀b', 1, 2),
  charLen_emoji: charLen('a😀b'),
}));
console.log(' ', JSON.stringify(utils));
check('chars() 按 code point 计数 (emoji)', utils.chars_emoji === 3);
check('chars() 补字算 1 个 (非 2)', utils.chars_kanjiExtB === 1);
check('charSlice 切出完整 emoji', utils.charSlice_emoji === '😀');
check('charLen emoji', utils.charLen_emoji === 3);

await page.waitForTimeout(300);

// ---------- 2. 光标推进跨补充平面 ----------
console.log('=== 2. 光标按 char 推进 ===');
const caretAdv = await page.evaluate(() => {
  const t = { id: 'test-' + Math.random(), lines: ['a😀b'], cursor: { row: 0, col: 0 }, sel: null, undo: [], redo: [] };
TABS.set(t.id, t);
activeTabId = t.id;
  // 从 col 0 依次右移 3 次, 应到 col 3 (而非 4)
  for (let i = 0; i < 3; i++) moveCaretRight(t);
  return { col: t.cursor.col, lines: t.lines };
});
console.log(' ', JSON.stringify(caretAdv));
check('右移 3 次后 col=3 (char 语义)', caretAdv.col === 3, caretAdv);

await page.waitForTimeout(300);

// ---------- 3. 退格删完整字符 ----------
console.log('=== 3. 退格不删半个 surrogate ===');
const bs = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['a😀b'];
  t.cursor = { row: 0, col: 2 };   // emoji 之后 (char 语义)
  t.sel = null;
  backspace();
  return { lines: t.lines, col: t.cursor.col };
});
console.log(' ', JSON.stringify(bs));
check('退格删掉整个 emoji', bs.lines[0] === 'ab', bs);
check('退格后 col=1', bs.col === 1, bs);

await page.waitForTimeout(300);

// ---------- 4. 退格补字 ----------
console.log('=== 4. 退格删完整 CJK 扩展 B ===');
const bsKanji = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['日本語𠮷'];
  t.cursor = { row: 0, col: 4 };
  t.sel = null;
  backspace();
  return { line: t.lines[0], col: t.cursor.col };
});
console.log(' ', JSON.stringify(bsKanji));
check('退格删掉整个补字', bsKanji.line === '日本語', bsKanji);

await page.waitForTimeout(300);

// ---------- 5. 在补充平面前插入 ----------
console.log('=== 5. 插入位置正确 ===');
const ins = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['日本語'];
  t.cursor = { row: 0, col: 1 };
  insertChar('X');
  return { line: t.lines[0], col: t.cursor.col };
});
console.log(' ', JSON.stringify(ins));
check('在 col 1 插入 X', ins.line === '日X本語', ins);
check('col 前进 1', ins.col === 2, ins);

await page.waitForTimeout(300);

// ---------- 6. 查找按 char 位置 ----------
console.log('=== 6. 查找返回 char 列 ===');
const findRes = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['a😀bc', 'x😀y'];
  const hits = findInTab(t, 'bc', { matchCase: false });
  const hitsEmoji = findInTab(t, 'y', { matchCase: false });
  return { hits, emojiHit: hitsEmoji[0] };
});
console.log(' ', JSON.stringify(findRes));
// 'a😀bc' char 下标: a=0 😀=1 b=2 c=3
  // UTF-16 下标会是 b=3 c=4 —— 那正是修复前的错误行为
  check('查找返回 char col (非 UTF-16)', findRes.hits[0].col === 2, findRes);
check('emoji 之后的匹配 col 正确', findRes.emojiHit.col === 2, findRes);

await page.waitForTimeout(300);

// ---------- 7. 选区高亮按 char ----------
console.log('=== 7. 选区高亮跨补充平面 ===');
const selHl = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['a😀bc'];
  t.cursor = { row: 0, col: 0 };
  // 选中 "bc" (char 2..4) —— UTF-16 下标会误选成 "c"
  t.sel = { r0: 0, c0: 2, r1: 0, c1: 4 };
  renderVim(t);
  const html = document.getElementById('ed-content').innerHTML;
  const active = html.match(/<span class="sel-active">([\s\S]*?)<\/span>/);
  return { active: active ? active[1] : null };
});
console.log(' ', JSON.stringify(selHl));
check('选区高亮内容为 "bc"', selHl.active === 'bc', selHl);

await page.waitForTimeout(300);

// ---------- 8. BMP 内行为未回归 ----------
console.log('=== 8. BMP 日语行为未回归 ===');
const bmp = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['日本語'];
  t.cursor = { row: 0, col: 3 };
  t.sel = null;
  backspace();
  const afterBs = t.lines[0];
  // 再插回去
  insertChar('語');
  const afterIns = t.lines[0];
  // 查找
  const hits = findInTab(t, '本', { matchCase: false });
  return { afterBs, afterIns, hitCol: hits[0] && hits[0].col };
});
console.log(' ', JSON.stringify(bmp));
check('BMP 退格正常', bmp.afterBs === '日本', bmp);
check('BMP 插入正常', bmp.afterIns === '日本語', bmp);
check('BMP 查找 col=1', bmp.hitCol === 1, bmp);

await page.waitForTimeout(300);

// ---------- 9. 半角片假名 ----------
console.log('=== 9. 半角片假名 (BMP, 但常见日文输入) ===');
const hankaku = await page.evaluate(() => {
  const t = getActiveTab();
  t.lines = ['ｱｲｳ'];
  t.cursor = { row: 0, col: 3 };
  t.sel = null;
  backspace();
  return { line: t.lines[0] };
});
console.log(' ', JSON.stringify(hankaku));
check('半角片假名退格', hankaku.line === 'ｱｲ', hankaku);

console.log('\n=== 运行时错误 ===');
console.log(errs.length ? errs : 'none');
results['无运行时错误'] = errs.length === 0;

console.log('\n=== 汇总 ===');
let fail = 0;
for (const [k, v] of Object.entries(results)) {
  if (!v) fail++;
  console.log(`  ${v ? '\u2713' : '\u2717'} ${k}`);
}
await browser.close();
console.log(fail === 0 ? '\n全部通过 — char 语义一致' : `\n${fail} 项失败`);
process.exit(fail === 0 ? 0 : 1);