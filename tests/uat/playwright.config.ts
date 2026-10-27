// tests/uat/playwright.config.ts
// IDE1.0 IDE Shell — Playwright e2e config
//
// webServer 自动启动 cargo build 的 ide-shell-web binary (port 8123),
// 所有 case 跑完后 Playwright 自动关闭 server.
import { defineConfig, devices } from "@playwright/test";
import * as path from "path";

// server binary 路径 (tests/uat → repo root target/debug; 本机用 CARGO_TARGET_DIR 时设 IDE_SHELL_WEB_BIN)
// MSYS bash 路径转换问题: 总是要求 IDE_SHELL_WEB_BIN 设置为绝对路径
// QA_BIN 是 tests/mock 用的同一个变量, 两边通用
if (!process.env.IDE_SHELL_WEB_BIN && process.env.QA_BIN) process.env.IDE_SHELL_WEB_BIN = process.env.QA_BIN;
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
    retries: 1, // race case (vim mode 偶发 lag) 重试一次
  reporter: "list",
  timeout: 10_000,
  expect: { timeout: 3_000 },
  use: {
      baseURL: `http://${ADDR}`,
    // 全新用户的缺省界面是「极简 (Sakura 风格)」布局 + 非模态编辑; 既有用例都是按完整 IDE 界面 + vim 三态写的,
    // 所以统一注入这两个开关 (editor.html 的 ide10.layout / ide10.vim). 不改每个 spec;
    // 要测真正的首次启动状态, 在 spec 里 test.use({ storageState: { cookies: [], origins: [] } }) 覆盖 (见 minimal-layout.spec.ts).
    // origin 必须与 baseURL 完全一致 (含端口), 所以同样取自 IDE_SHELL_WEB_ADDR.
    storageState: {
      cookies: [],
      origins: [{
        origin: `http://${ADDR}`,
        localStorage: [
          { name: "ide10.layout", value: "full" },
          { name: "ide10.vim", value: "on" },
        ],
      }],
    },
    headless: true,
    viewport: { width: 960, height: 600 },
    trace: "on-first-retry",
  },
  // 启动后核对内嵌页面和 editor.html 是否一致 (没重新编译 = 测的是旧页面)
  globalSetup: "./global-setup.ts",
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
      command: BINARY,
      url: `http://${ADDR}/api/frame`,
      reuseExistingServer: process.env.PW_REUSE_SERVER === "1",   // 默认不复用: 端口上残留的旧 server 会让整轮测的是旧二进制
      timeout: 90_000,  // CI Linux cold-start: build + connect 延长
      // 默认 fixtures 项目根 — editor.spec.ts / sandbox.spec.ts 用
      env: {
        IDE_SHELL_WEB_ADDR: ADDR,
        // 测试环境必须开无认证模式 —— 部分 case 直接 fetch("/api/reset")
        // 不带 token, 认证模式下会 401 导致整片用例连锁失败.
        IDE_SHELL_WEB_NO_AUTH: "1",
        // 相对本仓库解析 (原先写死 D:\orcaWork\... 绝对路径, 换机器/CI 即失效)
        IDE_SHELL_WEB_TEST_ROOT: path.resolve(__dirname, "fixtures", "sample-project"),
      },
      stdout: "ignore",
      stderr: "pipe",
    },
  });
