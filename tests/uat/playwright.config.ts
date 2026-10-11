// tests/uat/playwright.config.ts
// IDE1.0 IDE Shell — Playwright e2e config
//
// webServer 自动启动 cargo build 的 ide-shell-web binary (port 8123),
// 所有 case 跑完后 Playwright 自动关闭 server.
import { defineConfig, devices } from "@playwright/test";
import * as path from "path";

// server binary 路径 (tests/uat → repo root target/debug; 本机用 CARGO_TARGET_DIR 时设 IDE_SHELL_WEB_BIN)
// MSYS bash 路径转换问题: 总是要求 IDE_SHELL_WEB_BIN 设置为绝对路径
if (!process.env.IDE_SHELL_WEB_BIN) {
  throw new Error(
    "IDE_SHELL_WEB_BIN env var must be set to absolute path of ide-shell-web binary " +
    "(e.g. 'E:/DevCache/cargo/target/debug/ide-shell-web.exe')"
  );
}
const BINARY = process.env.IDE_SHELL_WEB_BIN;
// 监听地址: 与 server 同一 env (IDE_SHELL_WEB_ADDR), 多个工作区并行跑 UAT 时可错开端口
const ADDR = process.env.IDE_SHELL_WEB_ADDR || "127.0.0.1:8123";

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
      baseURL: `http://${ADDR}`,
    headless: true,
    viewport: { width: 960, height: 600 },
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
      command: BINARY,
      url: `http://${ADDR}/api/frame`,
      reuseExistingServer: !process.env.CI,
      timeout: 90_000,  // CI Linux cold-start: build + connect 延长
      // 默认 fixtures 项目根 — editor.spec.ts / sandbox.spec.ts 用
      env: {
        IDE_SHELL_WEB_ADDR: ADDR,
        // 相对本仓库解析 (原先写死 D:\orcaWork\... 绝对路径, 换机器/CI 即失效)
        IDE_SHELL_WEB_TEST_ROOT: path.resolve(__dirname, "fixtures", "sample-project"),
      },
      stdout: "ignore",
      stderr: "pipe",
    },
  });
