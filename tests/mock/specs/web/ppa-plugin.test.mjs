// 回归: PPA 子集插件 (WBS-02 4.4) —— 纯函数解释器 (crates/editcore/plugins/ppa/main.js) + 宿主套用补丁
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const { runScript } = createRequire(import.meta.url)('../../../../crates/editcore/plugins/ppa/main.js');
const run = (script, text = 'a\nb\nc', extra = {}) => runScript({ script, text, name: 'x.txt', ...extra });
const apply = (r, text) => { const l = text.split('\n'); if (r.edit) l.splice(r.edit.start, r.edit.remove, ...r.edit.insert); return l.join('\n'); };

test('变量 / 循环 / 字符串函数: 每行前加行号', () => {
  const r = run(`var i: Integer; s: String;
begin
  for i := 1 to S_GetLineCount(0) do begin
    S_GoFileTop; S_Down; S_GoLineTop;
  end;
end.`, 'a\nb\nc');
  assert.equal(r.error, undefined);
  const r2 = run(`var i: Integer;
procedure Main;
begin
  S_GoFileTop;
  for i := 1 to S_GetLineCount(0) do begin
    S_GoLineTop;
    S_InsText(IntToStr(i) + ': ');
    S_Down;
  end;
end;`, 'a\nb\nc');
  assert.equal(apply(r2, 'a\nb\nc'), '1: a\n2: b\n3: c');
});

test('选区读取 / 函数 / while / repeat / if-else / 大小写', () => {
  const r = run(`function Twice(s: String): String;
begin
  Result := s + s;
end;
var n: Integer;
begin
  n := 0;
  while n < 3 do n := n + 1;
  repeat n := n + 1 until n >= 5;
  if n = 5 then S_InsText(UpperCase(Twice(S_GetSelectedString(0))) + IntToStr(n)) else S_InsText('bad');
end.`, 'abc', { selection: { r0: 0, c0: 0, r1: 0, c1: 3 }, cursor: { row: 0, col: 3 } });
  assert.equal(apply(r, 'abc'), 'ABCABC5');
});

test('S_ReplaceAll / S_DeleteLine / 多行插入 / 补丁是最小行范围', () => {
  const r = run(`begin S_ReplaceAll('b','X',0); S_GoFileTop; S_DeleteLine; S_GoFileEnd; S_InsText(#10 + 'end'); end.`, 'a\nb\nc');
  assert.equal(apply(r, 'a\nb\nc'), 'X\nc\nend');
  const unchanged = run(`begin end.`, 'a');
  assert.equal(unchanged.edit, null);
});

test('错误: 未声明变量 / 不支持的函数 / 语法错 / 死循环都不修改文档', () => {
  assert.match(run(`begin zz := 1; end.`).error, /没有声明/);
  assert.match(run(`begin Foo(1); end.`).error, /不支持的函数/);
  assert.match(run(`begin if end.`).error, /第 1 行/);
  const loop = run(`begin while true do begin end; end.`);
  assert.match(loop.error, /步数/);
  assert.equal(loop.edit, undefined);
});

test('宿主: PPA 插件命令按一步撤销套用; 没有 editor.write 的插件返回 edit 被忽略', () =>
  withMocks({ name: 'ppa', portOffset: 634 }, async ({ page, snap }) => {
    await fill(page, 'a' + String.fromCharCode(10) + 'b');
    const res = await page.evaluate(() => {
      const t = getActiveTab();
      const okPlugin = { manifest: { permissions: ['editor.write'] } }, noPlugin = { manifest: { permissions: [] } };
      const a = applyPluginEdit(t, noPlugin, { start: 0, remove: 1, insert: ['no'] });
      const bad = applyPluginEdit(t, okPlugin, { start: 5, remove: 1, insert: [] });
      const good = applyPluginEdit(t, okPlugin, { start: 1, remove: 1, insert: ['B', 'C'], cursor: { row: 2, col: 1 } });
      return { a, bad, good, lines: t.lines.slice(), cur: t.cursor };
    });
    assert.deepEqual([res.a, res.bad, res.good], [false, false, true]);
    assert.deepEqual(res.lines, ['a', 'B', 'C']);
    assert.deepEqual(res.cur, { row: 2, col: 1 });
    await page.evaluate(() => dispatchCommand('undo'));
    await page.evaluate(() => window.__editIdle());
    assert.deepEqual((await snap()).lines, ['a', 'b'], '一步撤销');
  }));

test('端到端: 宏槽种类选 PPA -> 沙箱里的 PPA 插件运行 -> 一步撤销写回', () =>
  withMocks({ name: 'ppa', portOffset: 635 }, async ({ page, snap }) => {
    await fill(page, 'a' + String.fromCharCode(10) + 'b');
    await page.evaluate(async () => {
      await plugRefresh();
      setMacroSlot(0, { name: 'p', kind: 'ppa', code: "begin S_GoFileEnd; S_InsText('!'); S_ReplaceAll('a','A',0); end." });
    });
    assert.equal(await page.evaluate(() => !!PLUG.commands['plugin:ppa:run']), true, '内置 PPA 插件已启用');
    await page.evaluate(() => dispatchCommand('macro-slot-1'));
    await page.waitForFunction(() => getActiveTab().lines.join('|') === 'A|b!', null, { timeout: 15000 });
    await page.evaluate(() => dispatchCommand('undo')); await page.evaluate(() => window.__editIdle());
    assert.deepEqual((await snap()).lines, ['a', 'b']);
  }));
