// 回归: 文档正文镜像 (editcore::doc_mirror) —— 大文档的查找 / 置換不再每次把整篇发给 Rust
//   - 第一次登记整篇, 之后只交内容指纹; 前端改过正文 (哪怕是直接改行数组) 指纹对不上, Rust 要整篇, 结果跟着新正文走
//   - 前端 docFingerprint 与 Rust 的 doc_mirror::fingerprint 逐字相同
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

test('大文档查找: 先登记整篇, 之后只交指纹; 改过正文后重新登记, 结果不过期', () =>
  withMocks({ name: 'docm', portOffset: 530 }, async ({ page }) => {
    // 与 Rust 单测 (doc_mirror::tests) 同一组输入 / 同一组期望值
    assert.deepEqual(await page.evaluate(() => [window.docFingerprint(['abc', 'def']), window.docFingerprint(['日本語😀', 'x'])]),
      [[2700757720, 821783810], [2129295297, 4181592631]]);

    const bodies = [];
    page.on('request', (r) => { if (r.url().includes('/api/search_text')) bodies.push(JSON.parse(r.postData() || '{}')); });
    await page.locator('#ed-content').click();
    await page.evaluate(() => {
      const t = getActiveTab();
      t.lines = Array.from({ length: 1500 }, (_, i) => 'line ' + i + (i % 3 === 0 ? ' foo' : ''));   // 500 行含 foo
      t.cursor = { row: 0, col: 0 }; t.sel = null;
      FIND_STATE.marks = false;                  // 关掉搜索标记: 它会在后台自己发 hits 请求, 干扰对请求的计数
      FIND_STATE.needle = 'foo';
    });
    const hits = () => page.evaluate(async () => (await searchText('hits')).total);

    assert.equal(await hits(), 500);
    assert.deepEqual(bodies.map((b) => b.lines.length), [0, 1500], '第一次: 先交指纹 (Rust 没有) -> 补发整篇');
    bodies.length = 0;
    assert.equal(await hits(), 500);
    assert.deepEqual(bodies.map((b) => b.lines.length), [0], '第二次: 只交指纹');
    assert.equal(bodies[0].doc_len, 1500);

    // 直接改行数组 (不经过任何编辑命令): 指纹变了, 不会用到旧正文
    bodies.length = 0;
    await page.evaluate(() => { getActiveTab().lines[3] = 'line 3 no match'; getActiveTab().lines[4] = 'line 4 foo'; });
    assert.equal(await hits(), 500, '行 3 的 foo 没了, 行 4 多了一个');
    assert.deepEqual(bodies.map((b) => b.lines.length), [0, 1500], '指纹对不上 -> 补发整篇');
    bodies.length = 0;
    await page.evaluate(() => { getActiveTab().lines[0] = 'no match here'; });
    assert.equal(await hits(), 499);

    // 置換全部也用镜像里那份; 结果套回文档
    bodies.length = 0;
    const out = await page.evaluate(async () => {
      const r = await searchText('replace_all', { repl: 'BAR', sel: null });
      return { count: r.count, rows: r.rows.length, first: r.rows[0] };
    });
    assert.equal(out.count, 499);
    assert.equal(out.rows, 499);
    assert.deepEqual(bodies.map((b) => b.lines.length), [0], '正文镜像已经是最新的, 只交指纹');

    // 小文档照旧整篇发
    bodies.length = 0;
    await page.evaluate(() => { getActiveTab().lines = ['foo', 'bar foo']; });
    assert.equal(await hits(), 2);
    assert.deepEqual(bodies.map((b) => b.lines.length), [2]);
  }));
