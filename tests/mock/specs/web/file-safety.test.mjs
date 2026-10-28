// 回归: 保存走原子写 (+可选备份), 外部改动在窗口重新获得焦点时检测一次
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const open = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('保存时备份旧文件 (首选项开启), 不留临时文件', () =>
  withMocks({ name: 'fsafe', portOffset: 579 }, async ({ page, root }) => {
    fs.writeFileSync(path.join(root, 'a.txt'), 'old\n');
    await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate(async (p) => {
      PREFS.saveBackup = true;
      await openFile(p);
      const t = getActiveTab(); t.lines = ['new']; t.dirty = true;
      await saveActive();
    }, path.join(root, 'a.txt'));
    assert.equal(fs.readFileSync(path.join(root, 'a.txt'), 'utf8'), 'new');
    const bak = fs.readdirSync(path.join(root, '.sakura-backup'));
    assert.equal(bak.length, 1, '备份了一份旧文件');
    assert.equal(fs.readFileSync(path.join(root, '.sakura-backup', bak[0]), 'utf8'), 'old\n');
    assert.deepEqual(fs.readdirSync(root).filter((f) => f.includes('.tmp')), [], '没有残留临时文件');
  }));

test('外部改动: 重新获得焦点时发现, 未改动的文档问是否重新载入', () =>
  withMocks({ name: 'fsafe', portOffset: 580 }, async ({ page, root }) => {
    const f = path.join(root, 'b.txt');
    fs.writeFileSync(f, 'one\n');
    await open(page, root);
    await page.locator('#ed-content').click();
    await page.evaluate(async (p) => { await openFile(p); }, f);
    await page.waitForFunction(() => getActiveTab().stamp && getActiveTab().stamp.exists, null, { timeout: 8000 });
    await page.evaluate(() => window.__mock.answers.confirm.push(true));
    fs.writeFileSync(f, 'two changed\n');
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await page.waitForFunction(() => getActiveTab().lines.join('\n').startsWith('two changed'), null, { timeout: 8000 });
    const asked = await page.evaluate(() => window.__mock.dialogs.filter((d) => /外部修改/.test(d.msg)).length);
    assert.equal(asked, 1, '只问一次');
    // 同一状态再次获得焦点: 不再重复询问
    await page.evaluate(() => { window.dispatchEvent(new Event('focus')); });
    await page.waitForTimeout(2000);
    assert.equal(await page.evaluate(() => window.__mock.dialogs.filter((d) => /外部修改/.test(d.msg)).length), 1);
  }));
