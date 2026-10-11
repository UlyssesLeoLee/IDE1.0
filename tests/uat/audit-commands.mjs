// 功能审计: 逐个执行 dispatchCommand 的每个 case, 捕获真实异常.
// 只检查「函数是否存在」会漏掉运行时报错和静默占位, 这里实际跑一遍.
import { chromium } from '@playwright/test';

const BASE = 'http://127.0.0.1:8123';
const FIX = 'D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project';

const browser = await chromium.launch();
const page = await browser.newPage();
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(e.message));

await page.goto(BASE + '/editor', { waitUntil: 'domcontentloaded' });
await page.waitForTimeout(1200);

await page.evaluate(async (fix) => {
  await fetch('/api/open_project', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path: fix }),
  });
  await window.setProjectRoot(fix, true);
}, FIX);
await page.locator('.tree-node:has-text("README")').first().click();
await page.waitForFunction(() => window.activeTab && window.activeTab() !== null, { timeout: 5000 });

const commands = await page.evaluate(() =>
  [...new Set([...document.querySelectorAll('[data-cmd]')].map((e) => e.dataset.cmd))]);
console.log('total commands:', commands.length, '\n');

const results = await page.evaluate((cmds) => {
  const out = [];
  const realPrompt = window.prompt;
  const realConfirm = window.confirm;
  window.prompt = () => 'audit_tmp.txt';
  window.confirm = () => true;
  for (const cmd of cmds) {
    try {
      dispatchCommand(cmd);
      out.push({ cmd, ok: true });
    } catch (e) {
      out.push({ cmd, ok: false, err: String((e && e.message) || e) });
    }
    try { closeModal(); } catch (_) { /* ignore */ }
  }
  window.prompt = realPrompt;
  window.confirm = realConfirm;
  return out;
}, commands);

const failed = results.filter((r) => !r.ok);
console.log(`executed ${results.length}, threw ${failed.length}`);
if (failed.length) {
  console.log('\n=== THREW ===');
  failed.forEach((f) => console.log(`  ${f.cmd}: ${f.err}`));
}

// 哪些命令跑完只是 setTip 占位 (提示语含未实装/待/占位)
const stubTips = await page.evaluate((cmds) => {
  const realPrompt = window.prompt;
  const realConfirm = window.confirm;
  window.prompt = () => 'audit_tmp.txt';
  window.confirm = () => true;
  const stubs = [];
  for (const cmd of cmds) {
    try {
      dispatchCommand(cmd);
    } catch (_) { /* ignore */ }
    const tip = document.getElementById('s-tip').textContent;
    if (/未实装|待实装|占位|未接入/.test(tip)) stubs.push({ cmd, tip });
    try { closeModal(); } catch (_) { /* ignore */ }
  }
  window.prompt = realPrompt;
  window.confirm = realConfirm;
  return stubs;
}, commands);

if (stubTips.length) {
  console.log('\n=== STILL STUBBED ===');
  stubTips.forEach((s) => console.log(`  ${s.cmd}: ${s.tip}`));
} else {
  console.log('\nno stubbed commands remain');
}

if (pageErrors.length) {
  console.log('\n=== PAGE ERRORS ===');
  [...new Set(pageErrors)].forEach((e) => console.log('  ' + e));
}
await browser.close();