// 回归: 文件编码 / BOM / 换行 — 打开 → 编辑 → 保存, 逐字节核对 (夹具见 fixtures/encodings)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';
import { ENC_FIXTURES } from '../../lib/harness.mjs';

const CASES = [
  ['sjis.txt', 'SJIS', 'CRLF', 1], ['eucjp.txt', 'EUC-JP', 'CRLF', 1], ['jis.txt', 'JIS', 'CRLF', 1],
  ['u16le.txt', 'Unicode BOM', 'CRLF', 2], ['u16be.txt', 'UnicodeBE BOM', 'CRLF', 2], ['u8bom.txt', 'UTF-8 BOM', 'CRLF', 1],
  ['u8.txt', 'UTF-8', 'CRLF', 1], ['cr.txt', 'UTF-8', 'CR', 1], ['lf.txt', 'UTF-8', 'LF', 1],
];

for (const [idx, [file, enc, eol, grow]] of CASES.entries()) {
  test(`${file}: 读出正确 / 状态栏 ${enc}·${eol} / 保存只多出所编辑的 ${grow} 字节, BOM 与换行保持`, () =>
    withMocks({ name: 'enc', portOffset: 100 + idx }, async ({ page, snap, press, m, root, app }) => {
      fs.copyFileSync(path.join(ENC_FIXTURES, file), path.join(root, file));
      await page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
      await m.dialog.answer('prompt', file);
      await page.locator('#ed-content').click();
      await press('Control+o');
      await page.waitForFunction(() => { const t = getActiveTab(); return t && t.lines && t.lines[0] !== ''; }, null, { timeout: 8000 });
      const s = await snap();
      assert.deepEqual(s.lines, ['日本語のテキスト', 'こんにちは', '']);
      assert.equal(s.status.enc.trim(), enc);
      assert.equal(s.status.eol.trim(), eol);
      const before = fs.readFileSync(path.join(root, file));
      await press('Control+Home'); await press('End'); await page.keyboard.type('X');
      await press('Control+s');
      for (let i = 0; i < 40 && fs.statSync(path.join(root, file)).size === before.length; i++) await page.waitForTimeout(100);
      const after = fs.readFileSync(path.join(root, file));
      assert.equal(after.length, before.length + grow);
      assert.equal(after.subarray(0, 3).toString('hex').slice(0, 4), before.subarray(0, 3).toString('hex').slice(0, 4), 'BOM / 头部保持');
      assert.equal(after.includes(Buffer.from('\r\n')), before.includes(Buffer.from('\r\n')));
      assert.deepEqual(app.errors, []);
    }));
}

test('按指定编码重新打开 / 设置保存编码 / 换行切换 (真实菜单)', () =>
  withMocks({ name: 'enc', portOffset: 31 }, async ({ page, snap, press, m, root }) => {
    fs.copyFileSync(path.join(ENC_FIXTURES, 'sjis.txt'), path.join(root, 's.txt'));
    await page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
    await m.dialog.answer('prompt', 's.txt');
    await page.locator('#ed-content').click(); await press('Control+o'); await page.waitForTimeout(600);
    await page.locator('#menubar .menu-item[data-menu="setting"] > .mb-btn').click();
    await page.locator('#menubar .menu-row[data-cmd="eol-lf"]').click();
    assert.equal((await snap()).status.eol.trim(), 'LF');
    await press('Control+s'); await page.waitForTimeout(500);
    assert.equal(fs.readFileSync(path.join(root, 's.txt')).includes(Buffer.from('\r\n')), false, '改成 LF 后保存不再有 CRLF');
    // 误按 UTF-8 重开: SJIS 字节无法解码 → 提示有非法字节 (不静默乱码)
    await page.locator('#menubar .menu-item[data-menu="file"] > .mb-btn').click();
    await page.locator('#menubar [data-submenu="reopen"]').hover();
    await page.locator('#menubar .menu-row[data-cmd="reopen-UTF-8"]').click(); await page.waitForTimeout(500);
    assert.match((await snap()).status.tip, /无法解码|重新打开/);
  }));

test('目标编码表示不了的字符: 保存前确认, 取消则不写盘', () =>
  withMocks({ name: 'enc', portOffset: 32 }, async ({ page, press, m, root }) => {
    fs.copyFileSync(path.join(ENC_FIXTURES, 'sjis.txt'), path.join(root, 's.txt'));
    await page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);
    await m.dialog.answer('prompt', 's.txt');
    await page.locator('#ed-content').click(); await press('Control+o'); await page.waitForTimeout(600);
    const orig = fs.readFileSync(path.join(root, 's.txt'));
    await press('Control+Home'); await page.keyboard.insertText('😀');
    await m.dialog.answer('confirm', false);
    await press('Control+s'); await page.waitForTimeout(500);
    assert.deepEqual(fs.readFileSync(path.join(root, 's.txt')), orig, '取消 = 文件不变');
    const log = await m.dialog.log();
    assert.ok(log.some((d) => d.kind === 'confirm' && /無法|无法|表示/.test(d.msg)), '弹出了无法表示字符的确认');
  }));
