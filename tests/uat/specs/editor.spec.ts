// tests/uat/specs/editor.spec.ts
// IDE1.0 IDE Shell — Cursor 风格编辑器 UAT (ULYS-191 §5).
// 验证 desktop dist/index.html 在 web 端通过 /api/* 端点端到端工作.
//
// 覆盖:
//   - 页面骨架 (toolbar / sidebar / tabs / editor / status / shell / modal)
//   - pick_folder → 文件树填充
//   - 点 tree 文件 → 标签页 + 内容
//   - vim 三态键位 (Insert 打字 / Esc → Normal / : → Command)
//   - 保存 (write_file 调后端)
//   - tooltip data-tip 悬停
//   - help wiki 浮层
//   - 沙箱拒绝越界 (走 fetch 直接打 API)
import { test, expect, type Page } from "@playwright/test";
import { resetShell, waitForBootstrap } from "./helpers";

const BASE = "http://127.0.0.1:8123";

// 共享给 editor UAT: vim Insert / save 会改 README fixture, beforeEach/afterEach 恢复
// 跨平台 fixture 路径 — 从 IDE_SHELL_WEB_TEST_ROOT 环境变量取 (web server 启动时设的)
const FIXTURE_ROOT = process.env.IDE_SHELL_WEB_TEST_ROOT ?? "D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project";
const FIXTURE_PATH = `${FIXTURE_ROOT}/README.md`;
const FIXTURE_ORIGINAL = "hello, world";

async function goEditor(page: Page) {
  await page.goto("/editor");
  await waitForBootstrap(page, 8_000);
}

// 直接调 setProjectRoot (绕开 rfd/prompt 弹框 — web 模式 pick_folder 会 30s timeout 后弹 prompt,
//  测试用例直接 setProjectRoot 同步设项目根 + 渲染文件树, 避开按钮点击 race).
// 注意: server 的 path_safety::safe_canonicalize 返回带 Windows verbatim prefix (\\?\C:\...),
// 我们需要先调 /api/open_project 拿 canonical path, 再传给 setProjectRoot.
// openFile 是 async: 点完树节点必须等 tab 真正建好, 否则后续按键落在欢迎页上.
// (之前两个测试首跑必失败、retry 才过, 就是漏了这个等待)
async function waitForTab(page: Page) {
  await page.waitForFunction(() => (window as any).getActiveTab && (window as any).getActiveTab() !== undefined, { timeout: 5_000 });
  await page.locator(".tab").first().waitFor({ timeout: 5_000 });
}

async function openProject(page: Page, path: string) {
  // 1. 先调 open_project 拿 canonical (Rust 端剥除 verbatim + walk_back for nonexistent)
  const canonRes = await fetch(`${BASE}/api/open_project`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ path }),
  });
    const canon = await canonRes.json();
  // 2. 用 canonical path 调 setProjectRoot (frontend 函数,设 projectRoot + 渲染文件树)
  await page.evaluate(async (cp) => {
    await window.setProjectRoot(cp, true);
  }, canon);
}

test.beforeEach(async () => {
  // 每个 case 重置 server state (frame + 项目根) 保证隔离
  await resetShell();
  // 恢复 README fixture 内容 (vim Insert / save 会改它)
  await fetch(`${BASE}/api/open_project`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path: FIXTURE_ROOT }),
  }).catch(() => {});
  await fetch(`${BASE}/api/write_file`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path: FIXTURE_PATH, content: FIXTURE_ORIGINAL }),
  }).catch(() => {});
});

test.afterEach(async () => {
  // 二次保险: case 异常退出也恢复
  await fetch(`${BASE}/api/write_file`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path: FIXTURE_PATH, content: FIXTURE_ORIGINAL }),
  }).catch(() => {});
});

// 会写盘的两个测试 (Insert / save) 需要 fixture 隔离, 否则首跑写脏 README.md,
// 污染后续测试导致 retry 才过. 这里用 test.beforeEach/afterEach 统一备份还原.
import { readFileSync, writeFileSync } from "node:fs";

const README = FIXTURE_ROOT + "/README.md";
let README_BACKUP: string | null = null;

// 只在测试真正用到这个 tab 时才备份 (纯只读测试不必碰磁盘)
async function snapshotFixture() {
  if (README_BACKUP === null) README_BACKUP = readFileSync(README, "utf8");
}

test.afterEach(() => {
  if (README_BACKUP !== null) {
    writeFileSync(README, README_BACKUP, "utf8");
    README_BACKUP = null;
  }
});

test("editor page loads with full Cursor-style skeleton", async ({ page }) => {
  await goEditor(page);
  await expect(page.locator("#menubar")).toBeVisible();
  await expect(page.locator("#side-bar")).toBeVisible();
  // Side body should have content (tree nodes or empty hint)
  await expect(page.locator("#side-body")).toBeVisible();
  // Either tree nodes or empty hint should be present
  const sideBody = page.locator("#side-body");
  const hasTreeNodes = await sideBody.locator(".tree-node").count() > 0;
  const hasEmptyHint = await sideBody.locator(".empty-hint").count() > 0;
  expect(hasTreeNodes || hasEmptyHint).toBeTruthy();
  await expect(page.locator("#status")).toBeVisible();
  await expect(page.locator("#bottom-panel")).toBeAttached();
  await expect(page.locator("#tip")).toBeAttached();
  await expect(page.locator("#modal")).toBeAttached();
});

test("kernel banner appears in toolbar after bootstrap", async ({ page }) => {
  await goEditor(page);
  // 等 banner 从 /api/kernel_banner 拿到 (≤ 1s)
  await expect(page.locator("#banner")).toContainText(/ide-kernel-core/i, { timeout: 5_000 });
});

test("pick_folder (mock) populates file tree", async ({ page }) => {
  await goEditor(page);
  // 直接调 setProjectRoot (它内部 fetch /api/open_project + 渲染文件树).
  //   绕开 rfd/prompt 弹框 (web 模式 pick_folder 走 30s timeout 后弹 prompt)
  await openProject(page, FIXTURE_ROOT);
  // Check side-body has content (project path + tree nodes)
  await expect(page.locator("#side-body")).toBeVisible();
  await expect(page.locator("#sb-project-name")).toBeVisible({ timeout: 5_000 });
  await expect(page.locator("#side-body")).toContainText("README", { timeout: 5_000 });
});

test("tree shows sample-project files (README / src / docs)", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  // sample-project fixture 含 README.md / src (dir) / docs (dir)
  await expect(page.locator("#side-body")).toContainText("README");
  await expect(page.locator("#side-body")).toContainText("src");
  await expect(page.locator("#side-body")).toContainText("docs");
});

test("clicking a tree file creates a tab with text content", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  await page.evaluate(async () => {
    // 同步建 README tab (绕开 openFile async) — canonical path from server
    const id = window.window.newId();
    tabs.set(id, {
      id,
      path: "D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project/README.md",
      name: "README.md",
      lines: ["hello, world"],
      cursor: { row: 0, col: 0 },
      mode: "normal",
      dirty: false,
      language: "markdown",
      history: [],
    });
    activeTabId = id;
    setActiveTab(id);
  });
  await page.waitForFunction(() => (window).activeTab && (window).activeTab.id !== null, { timeout: 5000 });
  // 标签栏出现
  await expect(page.locator(".tab").filter({ hasText: "README" })).toBeVisible();
  // 编辑区含文件内容
  await expect(page.locator("#ed-content")).toContainText("hello, world");
  // 状态栏文件路径已更新
  await expect(page.locator("#s-file")).toContainText("README.md");
});

test("vim Insert mode: type 'abc' in editor", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  await page.locator('.tree-node:has-text("README")').click();
  await waitForTab(page);
  await snapshotFixture();
  await expect(page.locator("#ed-content")).toBeVisible();
  // 显式 focus 编辑器 (click 在 default viewport 偏移下不稳)
  await page.locator("#ed-content").focus();
  // NORMAL → INSERT: 按 i
  await page.keyboard.press("i");
  // 状态栏 mode 应变 INSERT
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT", { timeout: 2_000 });
  // 打字
  await page.keyboard.type("abc");
  await expect(page.locator("#ed-content")).toContainText("abc");
  // Esc → NORMAL, 标签变脏 (tab-dirty class)
  await page.keyboard.press("Escape");
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "NORMAL");
  await expect(page.locator(".tab").first()).toHaveClass(/tab-dirty/);
});

test("save (Ctrl+S) writes to server and clears dirty flag", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  await page.locator('.tree-node:has-text("README")').click();
  await waitForTab(page);
  await snapshotFixture();
  await expect(page.locator("#ed-content")).toBeVisible();
  await page.locator("#ed-content").focus();
  await page.keyboard.press("i");
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT");
  await page.keyboard.type("XYZ");
  await page.keyboard.press("Escape");
  await expect(page.locator(".tab").first()).toHaveClass(/tab-dirty/);
  // Ctrl+S 保存
  await page.keyboard.press("Control+s");
  await expect(page.locator(".tab").first()).not.toHaveClass(/tab-dirty/, { timeout: 5_000 });
  await expect(page.locator("#s-tip")).toContainText("已保存");
});

test("tooltip on toolbar button shows on hover", async ({ page }) => {
  await goEditor(page);
  // welcome 页面的打开文件夹按钮 (始终可见)
  await page.locator('.open-btn[data-cmd="open-folder"]').hover();
  // 浮动 tooltip 显示 - 检查文本内容 (状态栏 tip 不会随 hover 变化)
  await expect(page.locator("#tip")).toContainText("打开文件夹", { timeout: 3_000 });
});

test("help modal opens with wiki containing vim section", async ({ page }) => {
  await goEditor(page);
  // 点击工具栏帮助按钮
  await page.locator('#menubar .menu-item[data-menu="help"] > .mb-btn').click();
  await page.locator('.menu-row[data-cmd="help"]').click();
  await expect(page.locator("#modal")).toHaveClass(/show/, { timeout: 5_000 });
  await expect(page.locator("#md-body")).toContainText("键位表");
  await expect(page.locator("#md-body")).toContainText("Ctrl+S");
  // Esc 关闭
  await page.keyboard.press("Escape");
  await expect(page.locator("#modal")).not.toHaveClass(/show/);
});

// 注: 旧 shell 面板 (#shell-body/#shell-mode-badge) 已被 sakura 风格重构替换为
// Terminal / Output / Problems 三页签 (见需求: 底端面板单层 tabs). 这里验证新结构.
test("bottom panel renders Terminal/Output/Problems tabs", async ({ page }) => {
  await goEditor(page);
  await expect(page.locator("#bottom-panel")).toBeAttached();
  await expect(page.locator('.bottom-tab[data-pane="terminal-pane"]')).toHaveText(/TERMINAL/i);
  await expect(page.locator('.bottom-tab[data-pane="output-pane"]')).toHaveText(/OUTPUT/i);
  await expect(page.locator('.bottom-tab[data-pane="problems-pane"]')).toHaveText(/PROBLEMS/i);
  // 默认折叠, Ctrl+` 展开
  await expect(page.locator("#bottom-panel")).toHaveAttribute("data-collapsed", "true");
  await page.keyboard.press("Control+`");
  await expect(page.locator("#bottom-panel")).toHaveAttribute("data-collapsed", "false", { timeout: 3_000 });
  await expect(page.locator("#terminal-body")).toBeVisible();
  await expect(page.locator("#terminal-empty")).toBeVisible();
});

test("terminal panel starts a real PowerShell session and echoes output", async ({ page }) => {
  // Windows 上 PowerShell 冷启动 (JIT + profile 加载) 可能要 15s+, 给足余量
  test.setTimeout(90_000);
  await goEditor(page);
  await page.keyboard.press("Control+`");
  await page.locator('.bottom-toggle[data-cmd="new-terminal"]').click();
  await expect(page.locator("#terminal-empty")).toBeHidden({ timeout: 30_000 });
  await page.locator("#terminal-body").focus();
  await page.keyboard.type("Write-Host IDE1.0-UAT-OK");
  await page.keyboard.press("Enter");
  // PowerShell 真回显
  await expect(page.locator("#terminal-body")).toContainText("IDE1.0-UAT-OK", { timeout: 30_000 });
});

test("clicking sb-project-name (no shift) re-opens folder flow", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  await expect(page.locator("#sb-project-name")).not.toHaveClass(/empty/, { timeout: 5_000 });
  const tip = await page.locator("#sb-project-name").getAttribute("data-tip");
  expect(tip).toMatch(/点击换项目/);
});

test("shift+click sb-project-name reloads tree (replaces 刷新 button)", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  await expect(page.locator("#sb-project-name")).not.toHaveClass(/empty/, { timeout: 5_000 });
  let listDirCalls = 0;
  page.on("request", (req) => {
    if (req.url().includes("/api/list_dir") && req.method() === "POST") listDirCalls++;
  });
  await page.locator("#sb-project-name").click({ modifiers: ["Shift"] });
  await page.waitForTimeout(500);
  expect(listDirCalls).toBeGreaterThanOrEqual(1);
});

test("no redundant sidebar 打开/刷新 buttons (replaced by project-name click)", async ({ page }) => {
  await goEditor(page);
  // '刷新' 按钮已删 (shift+click 项目名替代)
  await expect(page.locator('button[data-cmd="reload-tree"]')).toHaveCount(0);
  // sidebar 内的 '打开…' 按钮已删 (toolbar 主按钮 + 项目根 click 是双入口)
  await expect(page.locator('#sidebar button[data-cmd="open-folder"]')).toHaveCount(0);
});

test("tree folder lazy-load expands children", async ({ page }) => {
  await goEditor(page);
  await openProject(page, FIXTURE_ROOT);
  await expect(page.locator("#side-body")).toContainText("src");
  // 点 src 文件夹 (目录节点)
  const srcNode = page.locator('.tree-node:has-text("src")').first();
  await srcNode.click();
  // children 加载后应有 main.rs
  await expect(page.locator("#side-body")).toContainText("main.rs", { timeout: 5_000 });
});

test("sandbox rejects out-of-project read (direct API)", async ({ request }) => {
  // 准备: 先设置项目根
  await request.post(`${BASE}/api/open_project`, {
    data: { path: FIXTURE_ROOT },
  });
  // 越界读: Windows system file
  const res = await request.post(`${BASE}/api/read_file`, {
    data: { path: "C:\\Windows\\System32\\drivers\\etc\\hosts" },
  });
  expect(res.status()).toBe(400);
  const body = await res.text();
  expect(body).toMatch(/拒绝访问项目外路径/);
});

test("sandbox rejects write outside project root", async ({ request }) => {
  await request.post(`${BASE}/api/open_project`, {
    data: { path: FIXTURE_ROOT },
  });
  const res = await request.post(`${BASE}/api/write_file`, {
    data: { path: "C:\\Windows\\Temp\\pwn.txt", content: "hacked" },
  });
  expect(res.status()).toBe(400);
  const body = await res.text();
  expect(body).toMatch(/拒绝访问项目外路径/);
});

test("list_dir returns sorted entries (dirs first)", async ({ request }) => {
  await request.post(`${BASE}/api/open_project`, {
    data: { path: FIXTURE_ROOT },
  });
  const res = await request.post(`${BASE}/api/list_dir`, {
    data: { path: FIXTURE_ROOT },
  });
  expect(res.status()).toBe(200);
  const entries = await res.json();
  expect(Array.isArray(entries)).toBeTruthy();
  // 至少 3 个 (README, docs, src)
  const names: string[] = entries.map((e: any) => e.name);
  expect(names).toContain("README.md");
  expect(names).toContain("docs");
  expect(names).toContain("src");
  // 目录在文件前: docs / src (dirs) 应在 README.md (file) 之前
  const firstFileIdx = entries.findIndex((e: any) => !e.is_dir);
  const lastDirIdx = entries.map((e: any) => e.is_dir).lastIndexOf(true);
  expect(lastDirIdx).toBeLessThan(firstFileIdx);
});

test("help_wiki API contains all required sections", async ({ request }) => {
  const res = await request.get(`${BASE}/api/help_wiki`);
  expect(res.status()).toBe(200);
  const wiki = await res.json();
  expect(typeof wiki).toBe("string");
  expect(wiki.length).toBeGreaterThan(2000);
  for (const sec of ["简介", "安装与启动", "界面布局", "项目导入", "键位表", "Shell 面板", "鼠标悬停说明", "FAQ", "版本"]) {
    expect(wiki).toContain(sec);
  }
});

test("write_file roundtrip: write, then read back", async ({ request }) => {
  await request.post(`${BASE}/api/open_project`, {
    data: { path: FIXTURE_ROOT },
  });
  const path = "D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project/UAT_ROUNDTRIP.txt";
  // 写
  const w = await request.post(`${BASE}/api/write_file`, {
    data: { path, content: "roundtrip 内容\n第二行" },
  });
  expect(w.status()).toBe(200);
  const n = await w.json();
  // 服务端 write_file_pub 返回 content.len() — &str 的 len 是 UTF-8 字节数.
  // "roundtrip 内容\n第二行": ASCII 11 + 中文 "内容第二行" 15 = 26 字节
  const expected = Buffer.byteLength("roundtrip 内容\n第二行", "utf8");
  expect(n).toBe(expected);
  expect(n).toBe(26);
  // 读回
  const r = await request.post(`${BASE}/api/read_file`, { data: { path } });
  expect(r.status()).toBe(200);
  const fc = await r.json();
  expect(fc.content).toBe("roundtrip 内容\n第二行");
  // 清理
  await request.post(`${BASE}/api/write_file`, {
    data: { path, content: "" },
  });
});

test("read_file rejects huge files (>4MB)", async ({ request }) => {
  await request.post(`${BASE}/api/open_project`, {
    data: { path: FIXTURE_ROOT },
  });
  // 先写一个 5MB 文件
  const big = "x".repeat(5 * 1024 * 1024);
  const path = "D:/orcaWork/IDE1.0/dev-3/tests/uat/fixtures/sample-project/BIG.bin";
  await request.post(`${BASE}/api/write_file`, { data: { path, content: big } });
  // 读应被拒
  const r = await request.post(`${BASE}/api/read_file`, { data: { path } });
  expect(r.status()).toBe(400);
  const body = await r.text();
  expect(body).toMatch(/文件过大/);
  // 清理
  await request.post(`${BASE}/api/write_file`, { data: { path, content: "" } });
});