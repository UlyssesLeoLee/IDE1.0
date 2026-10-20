// 回归: コード変換 (Rust reinterpret_text) — 选区/全文按 X→Y 重新解读, 往返可还原; Alt+F 键位
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const text = (page) => page.evaluate(() => getActiveTab().lines.join('\n'));
const settle = (page) => page.waitForTimeout(500);

for (const [fwd, back, label, sample] of [
  ['codecnv-sjis2euc', 'codecnv-euc2sjis', 'EUC', 'あいうえお'],       // EUC 字节全在 0xA1-0xDF: 当 SJIS 读是半角カナ, 无损
  ['codecnv-sjis2utf8', 'codecnv-utf82sjis', 'UTF-8', 'こんにちは'],
  ['codecnv-sjis2jis', 'codecnv-email', 'JIS', '日本語のテキスト'],
]) {
  test(`SJIS→${label} 变成乱码形, ${label}→SJIS 还原 (往返)`, () =>
    withMocks({ name: 'cc', portOffset: 210 + ['EUC', 'UTF-8', 'JIS'].indexOf(label) }, async ({ page, press }) => {
      await fill(page, 'x');
      await page.evaluate((v) => { const t = getActiveTab(); t.lines = [v]; renderVim(t); }, sample);
      await cmd(page, fwd); await settle(page);
      const mid = await text(page);
      assert.notEqual(mid, sample, fwd + ' 之后文本变了');
      await cmd(page, back); await settle(page);
      assert.equal(await text(page), sample, back + ' 还原');
    }));
}

test('自動判別→SJIS: 把被误读成 SJIS 的 EUC 文字修好; 表示不了的字符给出提示; 键位 Alt+F10', () =>
  withMocks({ name: 'cc', portOffset: 214 }, async ({ page, press }) => {
    await fill(page, 'x');
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['あいうえお']; renderVim(t); });
    await cmd(page, 'codecnv-sjis2euc'); await settle(page);
    await cmd(page, 'codecnv-auto2sjis'); await settle(page);
    assert.equal(await text(page), 'あいうえお');
    await cmd(page, 'codecnv-sjis2euc'); await settle(page);
    await press('Alt+F10'); await settle(page);
    assert.equal(await text(page), 'あいうえお', 'Alt+F10 = EUC→SJIS');
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['😀']; t.sel = null; renderVim(t); });
    await cmd(page, 'codecnv-sjis2euc'); await settle(page);
    assert.match(await page.evaluate(() => document.getElementById('s-tip').textContent), /失败|UNMAPPABLE|无法表示/);
    assert.equal(await text(page), '😀', '失败时文本不变');
  }));
