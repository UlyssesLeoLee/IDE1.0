// 临时布局探针: 打印各区域 boundingBox, 定位 tree-node 被谁遮挡
import { chromium } from '@playwright/test';

const BASE = 'http://127.0.0.1:8123';
const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(BASE + '/editor');

// 打开 fixture 项目
const canon = await (await fetch(BASE + '/api/open_project', {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ path: 'D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project' }),
})).json();
await page.evaluate(async (cp) => { await window.setProjectRoot(cp, true); }, canon);
await page.waitForSelector('.tree-node', { timeout: 5000 });

const ids = ['#app', '#menubar', '#banner', '#activity-bar', '#side-bar', '#side-tabs',
             '#side-body', '#editor-area', '#tabs', '#breadcrumb', '#editor', '#status'];
for (const sel of ids) {
  const box = await page.locator(sel).boundingBox().catch(() => null);
  console.log(sel.padEnd(16), box ? `x=${Math.round(box.x)} y=${Math.round(box.y)} w=${Math.round(box.width)} h=${Math.round(box.height)}` : 'MISSING');
}

const node = page.locator('.tree-node:has-text("README")').first();
console.log('README node    ', JSON.stringify(await node.boundingBox()));

// 谁挡在它上面?
const hit = await page.evaluate(() => {
  const n = [...document.querySelectorAll('.tree-node')].find(e => e.textContent.includes('README'));
  if (!n) return 'no node';
  const r = n.getBoundingClientRect();
  const el = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
  return el ? `${el.tagName}#${el.id}.${el.className}` : 'null';
});
console.log('elementFromPoint:', hit);

await browser.close();