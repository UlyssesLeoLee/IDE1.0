// tests/uat/playwright.config.ts
// IDE1.0 IDE Shell — Playwright e2e config
//
// webServer 自动启动 cargo build 的 ide-shell-web binary (port 8123),
// 所有 case 跑完后 Playwright 自动关闭 server.
import { defineConfig, devices } from "@playwright/test";

// server binary 路径 (tests/uat → repo root target/debug; 本机用 CARGO_TARGET_DIR 时设 IDE_SHELL_WEB_BIN)
const BINARY =
  process.env.IDE_SHELL_WEB_BIN ??
  "../../target/debug/ide-shell-web" + (process.platform === "win32" ? ".exe" : "");

export default defineConfig({
  testDir: "./specs",
    fullyParallel: false, // 共享一个 server (single-user demo)
    // 强制串行 — editor.spec.ts 依赖 web server bootstrap 在 page 内的初始化,
    // parallel 多 page 同时 hit server 偶发 race (vim Insert case mode badge 滞后)
    workers: 1,
    retries: process.env.CI ? 1 : 1, // race case (vim mode 偶发 lag) 重试一次
  reporter: "list",
  timeout: 10_000,
  expect: { timeout: 3_000 },
  use: {
    baseURL: "http://127.0.0.1:8123",
    headless: true,
    viewport: { width: 960, height: 600 },
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
      command: BINARY,
      url: "http://127.0.0.1:8123/api/frame",
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
      // 默认 fixtures 项目根 — editor.spec.ts / sandbox.spec.ts 用
      env: {
        IDE_SHELL_WEB_TEST_ROOT: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project",
      },
      stdout: "ignore",
      stderr: "pipe",
    },
  });
