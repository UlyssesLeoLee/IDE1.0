// 回归: 右上角快捷按钮可自定义 (默认只有 Grep), 首选项里增减 / 排序, 点击走命令; 插件 zip 导入走对话框
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const cmds = (page) => page.evaluate(() => [...document.querySelectorAll('#toolbar .tb-btn')].map((b) => b.dataset.cmd));

test('默认只有 Grep; 首选项里添加 / 上移 / 移除 / 恢复默认, OK 才生效并保存', () =>
  withMocks({ name: 'qb', portOffset: 651 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    assert.deepEqual(await cmds(page), ['grep']);
    await page.evaluate(() => commonConfig('快捷按钮'));
    await page.waitForSelector('.qab-list');
    await page.locator('[data-qab-sel]').selectOption('save');            // 添加「保存」(草稿)
    await page.locator('[data-qab-sel]').selectOption('undo');
    assert.deepEqual(await cmds(page), ['grep'], '没点 OK 之前不生效');
    await page.locator('[data-qab-act="up"][data-k="2"]').click();         // undo 上移一位
    await page.locator('#md-foot button:first-child').click();
    assert.deepEqual(await cmds(page), ['grep', 'undo', 'save']);
    assert.deepEqual(await page.evaluate(() => PREFS.qab), ['grep', 'undo', 'save'], '存进首选项');
    await page.evaluate(() => commonConfig('快捷按钮'));
    await page.waitForSelector('.qab-list');
    await page.locator('[data-qab-act="del"][data-k="1"]').click();
    await page.locator('#md-foot button:nth-child(2)').click();
    assert.deepEqual(await cmds(page), ['grep', 'undo', 'save'], '取消: 不变');
    await page.evaluate(() => commonConfig('快捷按钮'));
    await page.waitForSelector('.qab-list');
    await page.locator('[data-qab-act="reset"]').click();
    await page.locator('#md-foot button:first-child').click();
    assert.deepEqual(await cmds(page), ['grep'], '恢复默认');
  }));

test('快捷按钮点击执行命令; 候选命令来自菜单; 重载后保留', () =>
  withMocks({ name: 'qb', portOffset: 652 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => setQab(['grep', 'select-all']));
    const c = await page.evaluate(() => qabCandidates().map((x) => x.cmd));
    assert.ok(c.includes('grep') && c.includes('select-all') && c.length > 30, '候选命令来自菜单');
    await page.evaluate(() => { const t = getActiveTab(); t.lines = ['abc']; renderVim(t); });
    await page.locator('#toolbar .tb-btn[data-cmd="select-all"]').click();
    await page.evaluate(() => window.__editIdle());
    assert.ok(await page.evaluate(() => !!getActiveTab().sel), '全选命令被执行');
    await page.reload(); await page.waitForTimeout(600);
    assert.deepEqual(await cmds(page), ['grep', 'select-all'], '重载后还在');
  }));

test('插件 zip 导入: 对话框里给出清单 / 权限 / 哈希, 同意才装; 菜单里有入口', () =>
  withMocks({ name: 'qb', portOffset: 653 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    assert.ok(await page.evaluate(() => !!CFG_CMDS['plugin-import']), '有命令');
    assert.ok(await page.evaluate(() => MENU_DEF.find((m) => m[0] === 'setting')[2].includes('plugin-import')), '设定菜单里有入口');
    const man = JSON.stringify({ id: 'zip-demo', name: 'Zip Demo', version: '1.0.0', apiVersion: 1, repo: 'o/zip-demo', entry: 'main.js', permissions: ['storage'], contributes: { command: [{ id: 'hi' }] } });
    const files = [['manifest.json', Buffer.from(man)], ['main.js', Buffer.from('ide.register({"command:hi": () => ({summary: "hi"})});')]];
    const crc = (b) => { const t = []; let r = 0xffffffff; for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } for (const x of b) r = t[(r ^ x) & 0xff] ^ (r >>> 8); return (r ^ 0xffffffff) >>> 0; };
    const parts = [], cd = []; let off = 0;
    for (const [name, data] of files) {
      const nb = Buffer.from(name), cs = crc(data), h = Buffer.alloc(30);
      h.writeUInt32LE(0x04034b50, 0); h.writeUInt16LE(20, 4); h.writeUInt32LE(cs, 14); h.writeUInt32LE(data.length, 18); h.writeUInt32LE(data.length, 22); h.writeUInt16LE(nb.length, 26);
      parts.push(h, nb, data);
      const c = Buffer.alloc(46); c.writeUInt32LE(0x02014b50, 0); c.writeUInt16LE(20, 4); c.writeUInt16LE(20, 6); c.writeUInt32LE(cs, 16); c.writeUInt32LE(data.length, 20); c.writeUInt32LE(data.length, 24); c.writeUInt16LE(nb.length, 28); c.writeUInt32LE(off, 42);
      cd.push(c, nb); off += 30 + nb.length + data.length;
    }
    const cdb = Buffer.concat(cd), e = Buffer.alloc(22);
    e.writeUInt32LE(0x06054b50, 0); e.writeUInt16LE(2, 8); e.writeUInt16LE(2, 10); e.writeUInt32LE(cdb.length, 12); e.writeUInt32LE(off, 16);
    const b64 = Buffer.concat([...parts, cdb, e]).toString('base64');
    await page.evaluate((d) => { const bin = Uint8Array.from(atob(d), (ch) => ch.charCodeAt(0)); window.__zipFile = new File([bin], 'zip-demo.zip'); }, b64);
    const flow = page.evaluate(() => pluginImportFlow([window.__zipFile]));
    await page.waitForSelector('#md-body .ext-report');
    const txt = await page.locator('#md-body').innerText();
    assert.match(txt, /Zip Demo/); assert.match(txt, /1\.0\.0/); assert.match(txt, /storage/); assert.match(txt, /sha256/);
    await page.locator('#md-foot button:first-child').click();
    await flow;
    assert.ok((await page.evaluate(async () => { await plugRefresh(); return PLUG.list.map((p) => p.manifest.id); })).includes('zip-demo'), '装上了');
    assert.ok(await page.evaluate(() => !!PLUG.commands['plugin:zip-demo:hi']), '插件命令可用');
  }));
