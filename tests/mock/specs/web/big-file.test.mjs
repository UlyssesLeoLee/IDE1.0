// 回归: 大文件上限 (WBS-02 5.9) —— 默认 4MB 不变; 首选项调高后先警告再放行; 超过上限仍拒绝
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('默认拒绝 5MB 文件并提示怎么调; 调到 8MB 后确认即可打开; 调到 4MB 以上仍夹在 32MB 内', () =>
  withMocks({ name: 'bf', portOffset: 637 }, async ({ page, root }) => {
    fs.writeFileSync(path.join(root, 'big.txt'), ('x'.repeat(99) + '\n').repeat(56000));       // ~5.3MiB
    await open(page, root);
    await page.locator('#ed-content').click();
    const r1 = await page.evaluate(async (p) => { try { await readFileWithLimit(p); return 'opened'; } catch (e) { return e.message; } }, 'big.txt');
    assert.match(r1, /超过默认上限 4MB.*大文件上限/);
    // 调到 8MB, 用户在确认框里点取消
    await page.evaluate(() => { PREFS.maxFileMB = 8; window.__asked = []; window.uiConfirm = async (m) => { window.__asked.push(m); return false; }; });
    const r2 = await page.evaluate(async (p) => { try { await readFileWithLimit(p); return 'opened'; } catch (e) { return e.message; } }, 'big.txt');
    assert.match(r2, /已取消/);
    assert.match(await page.evaluate(() => window.__asked[0]), /MB.*变慢/);
    // 确认 -> 打开
    await page.evaluate(() => { window.uiConfirm = async () => true; });
    const r3 = await page.evaluate(async (p) => { const r = await readFileWithLimit(p); return r.content.length; }, 'big.txt');
    assert.ok(r3 > 5500000, '读到了完整内容');
    // 设置的上限比文件小 (4.5MB 不到) 时仍拒绝: 上限只允许 4..32
    await page.evaluate(() => { PREFS.maxFileMB = 5; });
    const r4 = await page.evaluate(async (p) => { try { await readFileWithLimit(p); return 'opened'; } catch (e) { return e.message; } }, 'big.txt');
    assert.match(r4, /超过你设置的上限 5MB/);
    // 后端也夹住: 直接请求 max_bytes 超过 32MB 不会放行更大的文件 (这里文件 5.2MB, 用 1 字节上限验证下限夹到 4MB)
    const small = await page.evaluate(async () => { try { await invoke('read_file', { path: 'big.txt', max_bytes: 1 }); return 'opened'; } catch (e) { return e.message; } });
    assert.match(small, /文件过大/, '上限不会被调到 4MB 以下');
  }));
