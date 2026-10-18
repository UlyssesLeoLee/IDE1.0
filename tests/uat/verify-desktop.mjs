// 实机验证 Tauri 桌面版 — 通过 CDP 连 WebView2 驱动真实 app.
// 与 audit-desktop-parity.mjs (静态检查) 互补: 这里验证只有真机能跑的部分
//   - Tauri IPC invoke 往返 (kernel_banner / list_dir / read_file / write_file)
//   - terminal://output 事件通道 (web 版走轮询, Tauri 走事件)
//   - 原生文件夹选择器 rfd (无法自动点, 但能验证 invoke 返回结构)
//
// 用法:
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222" \
//     ide-shell-desktop.exe &
//   node verify-desktop.mjs
// [布局] 本脚本需要完整 IDE 界面 + vim 三态 (editor.html 缺省已改为极简 Sakura 风格 + 非模态): 连上页面后自动写 ide10.layout=full / ide10.vim=on 并重载, 见 needFull.
import { chromium } from '@playwright/test';

const CDP = process.env.IDE_DESKTOP_CDP || 'http://127.0.0.1:9222';
const FIX = process.env.IDE_DESKTOP_FIXTURE
  || 'D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project';

let browser;
try {
  browser = await chromium.connectOverCDP(CDP);
} catch (e) {
  console.error(`无法连接 CDP ${CDP}: ${e.message}`);
  console.error('请先启动: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222" ide-shell-desktop.exe');
  process.exit(2);
}

const ctx = browser.contexts()[0];
let page = ctx.pages().find((p) => !p.url().startsWith('devtools://'));
if (!page) {
  // 等 webview 页面出现
  for (let i = 0; i < 30 && !page; i++) {
    await new Promise((r) => setTimeout(r, 500));
    page = ctx.pages().find((p) => !p.url().startsWith('devtools://'));
  }
}
if (!page) {
  console.error('CDP 连上了, 但找不到 webview 页面');
  process.exit(2);
}

const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
// 缺省界面是极简 (Sakura 风格) + 非模态编辑; 本脚本按完整 IDE 界面 + vim 三态写 (要欢迎页 / 标签条 / 状态栏模式标签).
// 开关存在被测 app 的 localStorage (ide10.layout / ide10.vim): 不是 full + vim 就写进去并重载一次; 旧版 app (没有 setLayout) 本来就是完整界面, 跳过.
// 跑完想还原成极简: await page.evaluate(() => { setLayout('min'); localStorage.removeItem('ide10.vim'); })
const needFull = await page.evaluate(() => {
  try {
    if (!window.setLayout) return false;
    if (window.getLayout().preset === 'full' && localStorage.getItem('ide10.vim') === 'on') return false;
    localStorage.setItem('ide10.layout', 'full'); localStorage.setItem('ide10.vim', 'on');
    return true;
  } catch (_) { return false; }
});
if (needFull) {
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__ideReady && window.__ideReady(), null, { timeout: 15000 }).catch(() => {});
}


console.log('页面:', page.url(), '\n');
console.log('=== 1. Tauri runtime 就位 ===');
console.log(JSON.stringify(await page.evaluate(() => ({
  inTauri: typeof inTauri !== 'undefined' ? inTauri : 'undefined',
  internals: typeof window.__TAURI_INTERNALS__,
  eventApi: typeof (window.__TAURI_INTERNALS__ || {}).event,
})), null, 2));

console.log('\n=== 2. Tauri IPC invoke 往返 (kernel_banner) ===');
const banner = await page.evaluate(async () => {
  try {
    return { ok: true, value: await invoke('kernel_banner') };
  } catch (e) { return { ok: false, err: e.message }; }
});
console.log(' ', JSON.stringify(banner));

console.log('\n=== 3. 前端能力在真 webview 里可用 ===');
const caps = await page.evaluate(() => ({
  syntax: typeof Syntax !== 'undefined' ? 'object' : typeof Syntax,
  dispatch: typeof dispatchCommand,
  addCursor: typeof addCursor,
  columnSelect: typeof startColumnSelect,
  completion: typeof completionCandidates,
  moveLines: typeof moveLines,
  fold: typeof foldAtCursor,
  minimap: typeof buildMinimap,
  actButtons: document.querySelectorAll('.act-btn').length,
  sideTabs: document.querySelectorAll('.side-tab').length,
  bottomPanel: !!document.getElementById('bottom-panel'),
}));
console.log(' ', JSON.stringify(caps, null, 2));

console.log('\n=== 4. 打开项目 + 读文件 (IPC 往返 + 路径沙箱) ===');
const io = await page.evaluate(async (fix) => {
  const out = {};
  try {
    out.canonical = await invoke('open_project', { path: fix });
  } catch (e) { out.openErr = e.message; return out; }
  await window.setProjectRoot(out.canonical, true);
  try {
    const entries = await invoke('list_dir', { path: out.canonical });
    out.entries = entries.map((e) => e.name);
  } catch (e) { out.listErr = e.message; }
  try {
    const rf = await invoke('read_file', {
      path: out.canonical.replace(/^\\\\\?\\/, '') + '\\README.md',
    });
    out.readKeys = Object.keys(rf);
    out.readHead = String(rf.content || '').slice(0, 40);
  } catch (e) { out.readErr = e.message; }
  return out;
}, FIX);
console.log(' ', JSON.stringify(io, null, 2));

console.log('\n=== 5. 语法高亮在真 webview 渲染 ===');
const hl = await page.evaluate(async () => {
  await window.openFile(
    'D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project/fold_test.rs');
  await new Promise((r) => setTimeout(r, 600));
  const t = window.activeTab();
  t.lines = ['fn main() { let x = 42; } // hi'];
  t.cursor = { row: 0, col: 0 };
  window.renderVim(t);
  const html = document.getElementById('ed-content').innerHTML;
  return {
    hasKeyword: html.includes('tk-keyword'),
    hasNum: html.includes('tk-num'),
    hasComment: html.includes('tk-comment'),
    lang: document.getElementById('s-lang')?.textContent,
  };
});
console.log(' ', JSON.stringify(hl));

console.log('\n=== 6. 编辑命令在真 webview 生效 ===');
const editRes = await page.evaluate(async () => {
  const t = window.activeTab();
  t.lines = ['alpha', 'beta', 'gamma'];
  t.cursor = { row: 0, col: 0 };
  t.sel = null;
  window.renderVim(t);
  window.dispatchCommand('to-upper');
  const upper = t.lines.join('|');
  window.dispatchCommand('move-line-down');
  // 光标在 row0 时下移: 该行移到 row1, 光标随之到 row1
  return { upper, before: ['alpha','beta','gamma'], after: t.lines.slice(), cursorRow: t.cursor.row };
});
console.log(' ', JSON.stringify(editRes));

console.log('\n=== 7. 终端: 真实 PowerShell session + 事件通道 ===');
const term = await page.evaluate(async () => {
  document.getElementById('bottom-panel').setAttribute('data-collapsed', 'false');
  window.switchBottomPane('terminal-pane');
  const before = document.querySelectorAll('#terminal-body .term-line').length;
  // 走真实入口: createTerminal 是 async (setupTerminalChannel + 建 session + 隐藏空态)
  await window.createTerminal();
  const listen = findTauriListen();
  return { listenFound: typeof listen,
           withGlobalTauri: typeof window.__TAURI__,
           before, channel: window.TERM_CHANNEL,
           emptyHidden: document.getElementById('terminal-empty').style.display === 'none',
           tip: (document.getElementById('s-tip') || {}).textContent };
});
console.log(' ', JSON.stringify(term));

// 等 PowerShell 冷启动 + 事件推送
await page.waitForTimeout(5000);
const termLines = await page.evaluate(() =>
  [...document.querySelectorAll('#terminal-body .term-line')].map((e) => e.textContent));
console.log('  通道:', term.channel, '| 输出行数:', termLines.length);
console.log('  前 3 行:', JSON.stringify(termLines.slice(0, 3)));
const sawPrompt = termLines.some((l) => /PS .*>/.test(l));

// 真发一条命令, 确认不是只有提示符
await page.evaluate(() => window.sendTerminal('Write-Host IDE1.0-DESKTOP-UAT-OK\n'));
await page.waitForTimeout(3500);
const after = await page.evaluate(() =>
  [...document.querySelectorAll('#terminal-body .term-line')].map((e) => e.textContent));
const sawEcho = after.some((l) => l.includes('IDE1.0-DESKTOP-UAT-OK'));
console.log('  回显 UAT 标记:', sawEcho ? 'YES' : 'NO', '| 总行数:', after.length);


console.log('\n=== 8. 页面运行时错误 ===');
console.log(errors.length ? errors : 'none');

// ---- 汇总 ----
const results = {
  'Tauri runtime (kernel_banner IPC)': banner.ok,
  'Activity Bar 5 图标': caps.actButtons === 5,
  'Side Bar 5 tabs': caps.sideTabs === 5,
  '语法高亮引擎': caps.syntax === 'object',
  '命令路由可用': caps.dispatch === 'function',
  '多重光标/列块/补全/行操作/折叠/缩略图':
    caps.addCursor === 'function' && caps.columnSelect === 'function' &&
    caps.completion === 'function' && caps.moveLines === 'function' &&
    caps.fold === 'function' && caps.minimap === 'function',
  'open_project IPC': !!io.canonical,
  'list_dir IPC': Array.isArray(io.entries) && io.entries.length > 0,
  'read_file IPC': !!io.readKeys,
  '高亮 keyword/num/comment': hl.hasKeyword && hl.hasNum && hl.hasComment,
  '状态栏语言 = Rust': hl.lang === 'Rust',
  '编辑命令 大写': editRes.upper === 'ALPHA|BETA|GAMMA',
  // 注意: 上一步 to-upper 已把内容变大写, 所以这里比对大写形式
  '编辑命令 下移行': editRes.after[0] === 'BETA' && editRes.after[1] === 'ALPHA' &&
                     editRes.after[2] === 'GAMMA' && editRes.cursorRow === 1,
  '终端 session 创建 (真实入口)': term.emptyHidden && term.channel === 'event',
  'PowerShell 提示符出现': sawPrompt,
  'PowerShell 回显命令': sawEcho,
  'Tauri withGlobalTauri (event API)': term.listenFound === 'function',
};
console.log('\n=== 汇总 ===');
for (const [k, v] of Object.entries(results)) console.log(`  ${v ? '\u2713' : '\u2717'} ${k}`);

await browser.close();
const allOk = Object.values(results).every(Boolean) && errors.length === 0;
console.log(allOk ? '\n全部通过 — 桌面版实机功能验证 OK' : '\n有未通过项');
process.exit(allOk ? 0 : 1);