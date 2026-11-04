// 回归: 插件可删除 (含内置), 再用 Release 里的插件包 (.zip) 或「恢复内置」装回来 (走真实的 Rust 解包 / 兼容检查 / 权限确认)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const ZIP = path.resolve(import.meta.dirname, '../../../..', 'crates/editcore/plugins');
// 最小的 zip 写出器 (stored, 不压缩): 避免测试依赖别的库
function zipOf(files) {
  const enc = new TextEncoder(); const parts = []; const cd = []; let off = 0;
  const crc = (b) => { let c, t = []; for (let n = 0; n < 256; n++) { c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } let r = 0xffffffff; for (const x of b) r = t[(r ^ x) & 0xff] ^ (r >>> 8); return (r ^ 0xffffffff) >>> 0; };
  for (const [name, data] of files) {
    const nb = enc.encode(name), cs = crc(data), h = Buffer.alloc(30);
    h.writeUInt32LE(0x04034b50, 0); h.writeUInt16LE(20, 4); h.writeUInt32LE(cs, 14); h.writeUInt32LE(data.length, 18); h.writeUInt32LE(data.length, 22); h.writeUInt16LE(nb.length, 26);
    parts.push(h, Buffer.from(nb), Buffer.from(data));
    const c = Buffer.alloc(46); c.writeUInt32LE(0x02014b50, 0); c.writeUInt16LE(20, 4); c.writeUInt16LE(20, 6); c.writeUInt32LE(cs, 16); c.writeUInt32LE(data.length, 20); c.writeUInt32LE(data.length, 24); c.writeUInt16LE(nb.length, 28); c.writeUInt32LE(off, 42);
    cd.push(c, Buffer.from(nb)); off += 30 + nb.length + data.length;
  }
  const cdb = Buffer.concat(cd), e = Buffer.alloc(22);
  e.writeUInt32LE(0x06054b50, 0); e.writeUInt16LE(files.length, 8); e.writeUInt16LE(files.length, 10); e.writeUInt32LE(cdb.length, 12); e.writeUInt32LE(off, 16);
  return Buffer.concat([...parts, cdb, e]);
}

test('删除内置 ppa -> 列表里没了 -> 用插件包装回来 (权限确认) -> 再删再「恢复内置」', () =>
  withMocks({ name: 'pp', portOffset: 648 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const ids = () => page.evaluate(async () => { await plugRefresh(); return { on: PLUG.list.map((p) => p.manifest.id), gone: PLUG.removed.map((x) => x.id) }; });
    assert.ok((await ids()).on.includes('ppa'), '内置 ppa 默认就在');
    await page.evaluate(async () => { await plugApi({ op: 'remove', id: 'ppa' }); });
    let s = await ids(); assert.ok(!s.on.includes('ppa') && s.gone.includes('ppa'));
    const zip = zipOf([['manifest.json', fs.readFileSync(path.join(ZIP, 'ppa/manifest.json'))], ['main.js', fs.readFileSync(path.join(ZIP, 'ppa/main.js'))]]).toString('base64');
    const ins = await page.evaluate((d) => plugApi({ op: 'inspect_package', data: d }), zip);
    assert.ok(ins.report.ok && !ins.conflict && ins.report.manifest.permissions.includes('editor.write'));
    const bad = await page.evaluate(async (d) => { try { await plugApi({ op: 'install_package', data: d, approved: [] }); return 'installed'; } catch (e) { return e.message; } }, zip);
    assert.match(bad, /未获用户同意/);
    await page.evaluate((d) => plugApi({ op: 'install_package', data: d, approved: ['editor.read', 'editor.write'] }), zip);
    s = await ids(); assert.ok(s.on.includes('ppa') && !s.gone.includes('ppa'), '装回来了');
    await page.evaluate(async () => { await plugApi({ op: 'remove', id: 'ppa' }); await plugApi({ op: 'restore_bundled', id: 'ppa' }); });
    assert.ok((await ids()).on.includes('ppa'), '恢复内置');
    // 扩展面板里的按钮: 有「删除」, 已删除的列出「恢复内置」
    await page.evaluate(async () => { await plugApi({ op: 'remove', id: 'doc-graph' }); await plugRefresh(); drawExtensions && (document.getElementById('side-body').innerHTML = '<div id="ext-body"></div>', drawExtensions()); });
    assert.equal(await page.locator('[data-ext-restore="doc-graph"]').count(), 1);
    assert.ok(await page.locator('[data-ext-remove="ppa"]').count() >= 1);
    await page.evaluate(async () => { await plugApi({ op: 'restore_bundled', id: 'doc-graph' }); });
  }));
