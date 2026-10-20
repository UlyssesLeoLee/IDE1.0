// desktop.mjs — 启动真正的桌面应用 (Tauri / WebView2), 用 CDP 驱动真实窗口.
//   QA_DESKTOP=<ide-shell-desktop.exe 路径> 才会运行桌面用例; 否则整组跳过。
import { spawn, execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '../../uat/node_modules/playwright/index.mjs';
import { tmpProject, DESKTOP_BIN } from './harness.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const hasDesktop = !!DESKTOP_BIN;

export async function launchDesktop({ cdpPort = 9444 } = {}) {
  const root = tmpProject('desk');
  const child = spawn(DESKTOP_BIN, [], {
    env: { ...process.env, IDE_SHELL_DESKTOP_TEST_ROOT: root, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort}`, WEBVIEW2_USER_DATA_FOLDER: root + '-wv2' },
    stdio: 'ignore',
  });
  let browser;
  for (let i = 0; i < 60; i++) {
    try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`); break; } catch (_) { await new Promise((r) => setTimeout(r, 500)); }
  }
  if (!browser) { try { child.kill(); } catch (_) {} throw new Error('无法连接桌面应用的 CDP 端口 ' + cdpPort); }
  const page = browser.contexts()[0].pages().find((p) => !p.url().startsWith('devtools://'));
  await page.waitForFunction(() => window.__ideReady && window.__ideReady(), null, { timeout: 20000 }).catch(() => {});
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e.message || e)));
  return {
    page, root, errors,
    close: async () => {
      await browser.close().catch(() => {});
      try { execSync(`taskkill /PID ${child.pid} /T /F`, { stdio: 'ignore' }); } catch (_) {}
    },
  };
}

/** 在应用后面垫一个全屏窗口 (flip / color); 返回 stop(). */
export function startBackdrop(mode = 'flip') {
  const p = spawn('powershell.exe', ['-NoProfile', '-STA', '-File', path.join(HERE, 'backdrop.ps1'), '-Mode', mode], { stdio: 'ignore' });
  return { stop: () => { try { execSync(`taskkill /PID ${p.pid} /T /F`, { stdio: 'ignore' }); } catch (_) {} } };
}
