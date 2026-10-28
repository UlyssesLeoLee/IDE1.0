// 回归: Grep 置換 (按原编码写回 + 跳过未保存的已打开文件) 与 Sakura 宏 (.mac) 运行的前后端接线
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('Grep 置換: SJIS 文件按原编码写回, 有备份, 未保存的已打开文件跳过', () =>
  withMocks({ name: 'grepw', portOffset: 581 }, async ({ page, root }) => {
    fs.mkdirSync(path.join(root, 'gr'), { recursive: true });
    // こんにちは (SJIS 字节) + " foo\r\nfoo end\r\n"
    fs.writeFileSync(path.join(root, 'gr', 'sj.txt'), Buffer.concat([Buffer.from([0x82, 0xB1, 0x82, 0xF1, 0x82, 0xC9, 0x82, 0xBF, 0x82, 0xCD]), Buffer.from(' foo\r\nfoo end\r\n')]));
    fs.writeFileSync(path.join(root, 'gr', 'u8.txt'), 'foo here\n');
    await open(page, root);
    const rep = await page.evaluate(async () => (await fetch('/api/grep_replace', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({
      pattern: 'foo', replacement: 'bar', search_folders: ['gr'], backup: true, skip_paths: [] }) })).json());
    assert.equal(rep.total_replacements, 3);
    const raw = fs.readFileSync(path.join(root, 'gr', 'sj.txt'));
    assert.deepEqual([...raw.subarray(0, 10)], [0x82, 0xB1, 0x82, 0xF1, 0x82, 0xC9, 0x82, 0xBF, 0x82, 0xCD], 'SJIS 字节原样保留');
    assert.equal(raw.subarray(10).toString('latin1'), ' bar\r\nbar end\r\n', 'CRLF 原样保留');
    assert.equal(fs.readFileSync(path.join(root, 'gr', 'u8.txt'), 'utf8'), 'bar here\n');
    assert.ok(fs.existsSync(path.join(root, 'gr', 'u8.txt.bak')), '改写前备份');
    const rep2 = await page.evaluate(async (p) => (await fetch('/api/grep_replace', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({
      pattern: 'bar', replacement: 'baz', search_folders: ['gr'], skip_paths: [p] }) })).json(), path.join(root, 'gr', 'u8.txt'));
    assert.equal(rep2.files_skipped, 1, '未保存的已打开文件不改写');
    assert.equal(fs.readFileSync(path.join(root, 'gr', 'u8.txt'), 'utf8'), 'bar here\n');
  }));

test('Sakura 宏: 规划在 Rust, 前端逐步执行; 语法错误整体失败不执行', () =>
  withMocks({ name: 'grepw', portOffset: 582 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => { const t = getActiveTab(); t.name = 'x.mac'; t.lines = ['abc']; t.cursor = { row: 0, col: 0 }; t.sel = null; renderVim(t); });
    const ok = await page.evaluate(async () => runSakuraMacro('S_InsText("X");\nS_Char(13);\nS_InsText("Y");'));
    assert.equal(ok, true);
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), ['X', 'Yabc'], '插入 X, 回车, 插入 Y');
    const before = await page.evaluate(() => getActiveTab().lines.slice());
    const bad = await page.evaluate(async () => runSakuraMacro('S_InsText("Z");\nS_NoSuchFunc(1);'));
    assert.equal(bad, false);
    assert.deepEqual(await page.evaluate(() => getActiveTab().lines), before, '有语法错误: 一步都不执行');
  }));

test('Grep 置換是写操作: 不开项目被拒绝; 请求里放开"搜索项目外"也不能改到项目根之外的文件', () =>
  withMocks({ name: 'grepw', portOffset: 583 }, async ({ page, root }) => {
    const outside = fs.mkdtempSync(path.join(path.dirname(root), 'outside-'));
    fs.writeFileSync(path.join(outside, 'o.txt'), 'foo outside\n');
    const post = (body) => page.evaluate(async (b) => { const r = await fetch('/api/grep_replace', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(b) }); return { status: r.status, body: await r.text() }; }, body);
    const body = { pattern: 'foo', replacement: 'bar', search_folders: [outside], allow_outside_root: true };
    assert.equal((await post(body)).status, 400, '没有打开项目: 拒绝');
    await open(page, root);
    await post(body);   // 项目已打开, 请求自称允许项目外: 服务端忽略这个开关
    assert.equal(fs.readFileSync(path.join(outside, 'o.txt'), 'utf8'), 'foo outside\n', '项目根之外的文件没被改');
    fs.rmSync(outside, { recursive: true, force: true });
  }));
