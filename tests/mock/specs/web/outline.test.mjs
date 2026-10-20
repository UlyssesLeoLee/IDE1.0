// 回归: 大纲 — C++/JS/Go/Java/Markdown 的层级大纲, 点击跳转到定义行
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const openProject = (page, root) => page.evaluate(async (r) => { await fetch('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: r }) }); }, root);

test('C++ 大纲: namespace → class → method 缩进成层级; 点击条目跳到定义行', () =>
  withMocks({ name: 'ol', portOffset: 240 }, async ({ page, root, snap }) => {
    fs.writeFileSync(path.join(root, 'w.cpp'), '#include <x>\nnamespace app {\nclass Widget {\npublic:\n  void draw() const {\n    helper();\n  }\n};\n}\nint main() { return 0; }\n');
    fs.writeFileSync(path.join(root, 'd.md'), '# Title\n## Sub\ntext\n');
    await openProject(page, root);
    await page.evaluate(async (r) => { await setProjectRoot(r, true); await openFile(r + '/w.cpp'); switchSidePane('outline'); }, root);
    await page.waitForSelector('#outline-list .tree-node', { timeout: 8000 });
    const rows = await page.evaluate(() => [...document.querySelectorAll('#outline-list .tree-node')].map((e) => [e.textContent, +e.dataset.depth, e.style.paddingLeft]));
    assert.deepEqual(rows.map((r) => r.slice(0, 2)), [['N app', 0], ['C Widget', 1], ['m draw', 2], ['ƒ main', 0]]);
    assert.ok(parseFloat(rows[2][2]) > parseFloat(rows[1][2]) && parseFloat(rows[1][2]) > parseFloat(rows[0][2]), '缩进随层级增加');
    await page.locator('#outline-list .tree-node', { hasText: 'draw' }).click();
    await page.waitForTimeout(200);
    assert.equal((await snap()).cursor.row, 4, '跳到 draw 所在行');
    await page.evaluate(async (r) => { await openFile(r + '/d.md'); switchSidePane('outline'); }, root);
    await page.waitForFunction(() => /Sub/.test(document.getElementById('outline-list').innerText), null, { timeout: 8000 });
    assert.deepEqual(await page.evaluate(() => [...document.querySelectorAll('#outline-list .tree-node')].map((e) => +e.dataset.depth)), [0, 1]);
  }));
