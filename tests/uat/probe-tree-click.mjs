// 探针: 点树节点后到底发生了什么 (openFile 是否被调用 / 是否抛错)
import { chromium } from '@playwright/test';

const BASE = 'http://127.0.0.1:8123';
const FIX = 'D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project';

const browser = await chromium.launch();
const page = await browser.newPage();
page.on('console', m => console.log('  [console]', m.text()));
page.on('pageerror', e => console.log('  [pageerror]', e.message));

await page.goto(BASE + '/editor');
const canon = await (await fetch(BASE + '/api/open_project', {
  method: 'POST', headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ path: FIX }),
})).json();
console.log('canonical root:', canon);

await page.evaluate(async (cp) => { await window.setProjectRoot(cp, true); }, canon);
await page.waitForSelector('.tree-node', { timeout: 5000 });

// 树节点上的真实 path 值
const paths = await page.evaluate(() => [...document.querySelectorAll('.tree-node')].map(n => n.textContent));
console.log('tree nodes:', paths.join(' | '));

// 直接拿 list_dir 返回的原始 path
const entries = await (await fetch(BASE + '/api/list_dir', {
  method: 'POST', headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ path: canon }),
})).json();
const readme = entries.find(e => e.name.startsWith('README'));
console.log('list_dir README path:', JSON.stringify(readme?.path));

// 用这个 path 直接调 read_file
try {
  const rf = await fetch(BASE + '/api/read_file', {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ path: readme.path }),
  });
  console.log('read_file status:', rf.status, 'body:', (await rf.text()).slice(0, 120));
} catch (e) { console.log('read_file threw:', e.message); }

// 再点一次树节点看 UI
await page.locator('.tree-node:has-text("README")').first().click();
await page.waitForTimeout(800);
console.log('tip after click:', await page.locator('#s-tip').textContent());
console.log('tabs:', await page.locator('.tab').count());
console.log('ed-content:', (await page.locator('#ed-content').textContent()).slice(0, 100));

await browser.close();