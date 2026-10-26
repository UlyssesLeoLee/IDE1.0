// harness.mjs — 回归项目的共享工具. 只用「真实输入」(page.keyboard / page.mouse / locator.click / CDP) 触发功能,
// page.evaluate 只用来读状态 (读 getActiveTab()、DOM、localStorage…), 不要用它调 dispatchCommand() 去触发功能。
//
// 环境变量:
//   QA_BIN         ide-shell-web 可执行文件 (默认在 target/debug 下找)
//   QA_TARGET_DIR  cargo 的 target 目录 (设了 CARGO_TARGET_DIR 时对应这里)
//   QA_DESKTOP     ide-shell-desktop 可执行文件 (桌面真机用例用; 不设则跳过)
//   QA_PORT_BASE   起 web server 的起始端口 (默认 8400; 每个用例文件按偏移取端口)
import { chromium } from '../../uat/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const REPO = path.resolve(HERE, '../../..');
export const FIXTURES_SRC = path.resolve(HERE, '../../uat/fixtures/sample-project');
export const ENC_FIXTURES = path.resolve(HERE, '../fixtures/encodings');

function findBin(name) {
  const cands = [
    process.env.QA_TARGET_DIR && path.join(process.env.QA_TARGET_DIR, 'debug', name),
    path.join(REPO, 'target', 'debug', name),
    path.join(REPO, 'target', 'release', name),
  ].filter(Boolean);
  return cands.find((p) => fs.existsSync(p)) || cands[0];
}
export const BIN = process.env.QA_BIN || findBin('ide-shell-web.exe');
export const DESKTOP_BIN = process.env.QA_DESKTOP || null;
export const PORT_BASE = Number(process.env.QA_PORT_BASE || 8400);

/** 把 fixtures 复制到一次性目录, 返回该目录 (不在仓库的 fixtures 上写). */
export function tmpProject(name = 'proj') {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `ide10-mock-${name}-`));
  fs.cpSync(FIXTURES_SRC, dir, { recursive: true });
  return dir;
}

export async function startServer({ port, root, extraEnv = {} }) {
  const addr = `127.0.0.1:${port}`;
  const child = spawn(BIN, [], {
    env: { ...process.env, IDE_SHELL_WEB_ADDR: addr, IDE_SHELL_WEB_NO_AUTH: '1', IDE_SHELL_WEB_TEST_ROOT: root, ...extraEnv },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let log = '';
  child.stdout.on('data', (d) => (log += d));
  child.stderr.on('data', (d) => (log += d));
  const base = `http://${addr}`;
  for (let i = 0; i < 80; i++) {
    try { if ((await fetch(base + '/api/frame')).ok) break; } catch (_) { /* 还没起来 */ }
    await new Promise((r) => setTimeout(r, 250));
    if (child.exitCode !== null) throw new Error('server 提前退出: ' + log);
  }
  return { base, root, stop: () => { try { child.kill(); } catch (_) {} }, log: () => log };
}

/**
 * 打开编辑器页面. 默认是「真正的首次启动态」(什么都不种: 极简布局 + 非模态).
 * layout: 'full' 走完整 IDE 界面; vim: 'on' 开 Vim.
 */
export async function openApp({ base, layout, vim, viewport = { width: 1280, height: 800 }, colorScheme = 'dark' }) {
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport, colorScheme });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e.message || e)));
  if (layout || vim) {
    await page.addInitScript(([l, v]) => {
      try {
        if (!localStorage.getItem('__seeded')) {
          if (l) localStorage.setItem('ide10.layout', l);
          if (v) localStorage.setItem('ide10.vim', v);
          localStorage.setItem('__seeded', '1');
        }
      } catch (_) {}
    }, [layout, vim]);
  }
  return { browser, ctx, page, errors, base };
}
export async function gotoEditor(app) {
  await app.page.goto(app.base + '/editor', { waitUntil: 'domcontentloaded' });
  await app.page.waitForFunction(() => window.__ideReady && window.__ideReady(), null, { timeout: 15000 }).catch(() => {});
  await app.page.waitForTimeout(250);
  return app;
}
export async function closeApp(app) { await app.browser.close().catch(() => {}); }

/** 每个用例: 起 server + 开页面, 跑完清理. fn({ page, app, srv, root, snap, press, type }) */
export async function withApp(opts, fn) {
  const root = tmpProject(opts.name || 'p');
  const srv = await startServer({ port: PORT_BASE + (opts.portOffset || 0), root, extraEnv: opts.env || {} });
  const app = await openApp({ base: srv.base, layout: opts.layout, vim: opts.vim, viewport: opts.viewport });
  try {
    await gotoEditor(app);
    return await fn({ page: app.page, app, srv, root, snap: (p) => snap(p || app.page), press: (k, o) => press(app.page, k, o), type: async (t) => { await app.page.keyboard.type(t); await app.page.evaluate(() => (window.__editIdle ? window.__editIdle() : null)); } });
  } finally {
    await closeApp(app);
    srv.stop();
  }
}

/** 当前标签页状态 (只读). */
export async function snap(page) {
  await page.evaluate(() => (window.__editIdle ? window.__editIdle() : null)).catch(() => {});   // 经 Rust 桥的操作是异步的: 先等它们落地再取快照
  return page.evaluate(() => {
    const t = (window.getActiveTab && window.getActiveTab()) || null;
    const el = (id) => document.getElementById(id);
    return {
      lines: t ? t.lines.slice() : null,
      cursor: t ? { row: t.cursor.row, col: t.cursor.col } : null,
      sel: t && t.sel ? [t.sel.r0, t.sel.c0, t.sel.r1, t.sel.c1] : null,
      box: t && t.colSel ? [t.colSel.r0, t.colSel.c0, t.colSel.r1, t.colSel.c1] : null,
      meta: t ? { name: t.name, path: t.path, dirty: !!t.dirty, encoding: t.encoding, bom: t.bom, eol: t.eol } : null,
      status: {
        tip: el('s-tip') && el('s-tip').innerText, enc: el('s-enc') && el('s-enc').innerText, eol: el('s-eol') && el('s-eol').innerText,
        ovr: el('s-ovr') && el('s-ovr').innerText, colsel: el('s-colsel') && el('s-colsel').innerText,
      },
      modal: el('modal') && el('modal').classList.contains('show') ? { title: el('md-title').innerText, body: el('md-body').innerText } : null,
    };
  });
}
// 编辑变换在 Rust 里异步执行 (经桥); 按键之后等它们落地再读状态
export async function press(page, combo, { times = 1 } = {}) {
  for (let i = 0; i < times; i++) { await page.keyboard.press(combo); await page.evaluate(() => (window.__editIdle ? window.__editIdle() : null)); }
}

/** 清空并逐行输入一段文本 (真实键盘), 光标回到文首. */
export async function fill(page, text) {
  await page.locator('#ed-content').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await page.evaluate(() => (window.__editIdle ? window.__editIdle() : null));
  const ls = text.split('\n');
  for (let i = 0; i < ls.length; i++) {
    if (ls[i]) await page.keyboard.type(ls[i]);
    if (i < ls.length - 1) { await page.keyboard.press('Enter'); await page.evaluate(() => (window.__editIdle ? window.__editIdle() : null)); }   // Enter 在 Rust 里异步落地
  }
  await page.keyboard.press('Control+Home');
  await page.evaluate(() => (window.__editIdle ? window.__editIdle() : null));   // 移动在 Rust 里异步落地
}
export async function openMenu(page, menu, cmd) {
  await page.locator(`#menubar .menu-item[data-menu="${menu}"] > .mb-btn`).click();
  await page.locator(`#menubar .menu-row[data-cmd="${cmd}"]`).first().click();
}

/** 以第一个字符的真实矩形为原点, 返回 (行,列) -> 屏幕坐标 (等宽 ASCII 文本). */
export async function grid(page) {
  const m = await page.evaluate(() => {
    const ed = document.getElementById('ed-content');
    const w = document.createTreeWalker(ed, NodeFilter.SHOW_TEXT);
    const n = w.nextNode();
    const a = document.createRange(); a.setStart(n, 0); a.setEnd(n, 1);
    const b = document.createRange(); b.setStart(n, 0); b.setEnd(n, 5);
    const ar = a.getBoundingClientRect(), br = b.getBoundingClientRect();
    const lh = parseFloat(getComputedStyle(ed).lineHeight) || 18;
    return { left: ar.left, top: ar.top + ar.height / 2 - lh / 2, lh, cw: (br.right - ar.left) / 5 };
  });
  return { x: (c) => m.left + c * m.cw, y: (r) => m.top + r * m.lh + m.lh / 2 };
}
