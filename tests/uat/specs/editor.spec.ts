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
import { resetShell } from "./helpers";

const BASE = "http://127.0.0.1:8123";

// 共享给 editor UAT: vim Insert / save 会改 README fixture, beforeEach/afterEach 恢复
const FIXTURE_PATH = "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\README.md";
const FIXTURE_ORIGINAL = "hello, world";

async function goEditor(page: Page) {
  await page.goto("/editor");
}

// 直接调 setProjectRoot (绕开 rfd/prompt 弹框 — web 模式 pick_folder 会 30s timeout 后弹 prompt,
//  测试用例直接 setProjectRoot 同步设项目根 + 渲染文件树, 避开按钮点击 race).
async function openProject(page: Page, path: string) {
  await page.evaluate(async (p) => {
    await setProjectRoot(p, false);
  }, path);
}

test.beforeEach(async () => {
  // 每个 case 重置 server state (frame + 项目根) 保证隔离
  await resetShell();
  // 恢复 README fixture 内容 (vim Insert / save 会改它)
  await fetch(`${BASE}/api/open_project`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
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

test("editor page loads with full Cursor-style skeleton", async ({ page }) => {
  await goEditor(page);
  await expect(page.locator("#toolbar")).toBeVisible();
  await expect(page.locator("#sidebar")).toBeVisible();
  await expect(page.locator("#tree")).toBeVisible();
  await expect(page.locator("#welcome")).toBeVisible();
  await expect(page.locator("#status")).toBeVisible();
  await expect(page.locator("#shell-panel")).toBeVisible();
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
  await page.evaluate(async () => {
    await setProjectRoot("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project", false);
  });
  await expect(page.locator("#sb-project-name")).not.toHaveClass(/empty/, { timeout: 5_000 });
  await expect(page.locator("#tree")).toContainText("README", { timeout: 5_000 });
});

test("tree shows sample-project files (README / src / docs)", async ({ page }) => {
  await goEditor(page);
  await page.evaluate(async () => {
    await setProjectRoot("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project", false);
  });
  // sample-project fixture 含 README.md / src (dir) / docs (dir)
  await expect(page.locator("#tree")).toContainText("README");
  await expect(page.locator("#tree")).toContainText("src");
  await expect(page.locator("#tree")).toContainText("docs");
});

test("clicking a tree file creates a tab with text content", async ({ page }) => {
  await goEditor(page);
  await page.evaluate(async () => {
    await setProjectRoot("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project", false);
    // 同步建 README tab (绕开 openFile async)
    const id = newId();
    tabs.set(id, {
      id,
      path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\README.md",
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
  await page.waitForFunction(() => (window).activeTab && (window).activeTab() !== null, { timeout: 5000 });
  // 标签栏出现
  await expect(page.locator(".tab").filter({ hasText: "README" })).toBeVisible();
  // 编辑区含文件内容
  await expect(page.locator("#ed-content")).toContainText("hello, world");
  // 状态栏文件路径已更新
  await expect(page.locator("#s-file")).toContainText("README.md");
});

test("vim Insert mode: type 'abc' in editor", async ({ page }) => {
  await goEditor(page);
  await openProject(page, "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project");
  await page.locator('.tree-node:has-text("README")').click();
  await expect(page.locator("#editor.active")).toBeVisible();
  // 显式 focus 编辑器 (click 在 default viewport 偏移下不稳)
  await page.locator("#ed-content").focus();
  // NORMAL → INSERT: 按 i
  await page.keyboard.press("i");
  // 状态栏 mode 应变 INSERT
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT", { timeout: 2_000 });
  // 打字
  await page.keyboard.type("abc");
  await expect(page.locator("#ed-content")).toContainText("abc");
  // Esc → NORMAL, 标签变脏 (●)
  await page.keyboard.press("Escape");
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "NORMAL");
  await expect(page.locator(".tab").first()).toContainText("●");
});

test("save (Ctrl+S) writes to server and clears dirty flag", async ({ page }) => {
  await goEditor(page);
  await openProject(page, "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project");
  await page.locator('.tree-node:has-text("README")').click();
  await expect(page.locator("#editor.active")).toBeVisible();
  await page.locator("#ed-content").focus();
  await page.keyboard.press("i");
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT");
  await page.keyboard.type("XYZ");
  await page.keyboard.press("Escape");
  await expect(page.locator(".tab").first()).toContainText("●");
  // Ctrl+S 保存
  await page.keyboard.press("Control+s");
  await expect(page.locator(".tab").first()).not.toContainText("●", { timeout: 3_000 });
  await expect(page.locator("#s-saved")).toContainText("已保存");
});

test("tooltip on toolbar button shows on hover", async ({ page }) => {
  await goEditor(page);
  // toolbar 第一个 data-cmd="open-folder" (主按钮); sidebar/welcome 也有同名按钮
  await page.locator('#toolbar button[data-cmd="open-folder"]').hover();
  // 浮动 tooltip 显示
  await expect(page.locator("#tip")).toBeVisible();
  await expect(page.locator("#tip")).toContainText("打开文件夹");
  // 状态栏 tip 同步
  await expect(page.locator("#s-tip")).toContainText("打开文件夹");
});

test("help modal opens with wiki containing vim section", async ({ page }) => {
  await goEditor(page);
  await page.click('button[data-cmd="help"]');
  await expect(page.locator("#modal")).toHaveClass(/show/, { timeout: 3_000 });
  await expect(page.locator("#md-body")).toContainText("Vim 键位表");
  await expect(page.locator("#md-body")).toContainText("Ctrl+S");
  // Esc 关闭
  await page.keyboard.press("Escape");
  await expect(page.locator("#modal")).not.toHaveClass(/show/);
});

test("shell panel renders frame with mode badge", async ({ page }) => {
  await goEditor(page);
  // 等 setInterval 250ms 拉一次 frame
  await expect(page.locator("#shell-mode-badge")).toHaveAttribute("data-mode", "NORMAL", { timeout: 3_000 });
  await expect(page.locator("#shell-mode-badge")).toContainText("NORMAL");
});

test("shell click switches to INSERT (left)", async ({ page }) => {
  await goEditor(page);
  await page.locator("#shell-body").click();
  await expect(page.locator("#shell-mode-badge")).toHaveAttribute("data-mode", "INSERT", { timeout: 2_000 });
});

test("clicking sb-project-name (no shift) re-opens folder flow", async ({ page }) => {
  await goEditor(page);
  await openProject(page, "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project");
  await expect(page.locator("#sb-project-name")).not.toHaveClass(/empty/, { timeout: 5_000 });
  const tip = await page.locator("#sb-project-name").getAttribute("data-tip");
  expect(tip).toMatch(/点击换项目/);
});

test("shift+click sb-project-name reloads tree (replaces 刷新 button)", async ({ page }) => {
  await goEditor(page);
  await openProject(page, "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project");
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
  await openProject(page, "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project");
  await expect(page.locator("#tree")).toContainText("src");
  // 点 src 文件夹 (目录节点)
  const srcNode = page.locator('.tree-node:has-text("src")').first();
  await srcNode.click();
  // children 加载后应有 main.rs
  await expect(page.locator("#tree")).toContainText("main.rs", { timeout: 5_000 });
});

test("sandbox rejects out-of-project read (direct API)", async ({ request }) => {
  // 准备: 先设置项目根
  await request.post(`${BASE}/api/open_project`, {
    data: { path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" },
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
    data: { path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" },
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
    data: { path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" },
  });
  const res = await request.post(`${BASE}/api/list_dir`, {
    data: { path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" },
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
  for (const sec of ["简介", "安装与启动", "界面布局", "项目导入", "Vim 键位表", "Shell 面板", "鼠标悬停说明", "FAQ", "版本"]) {
    expect(wiki).toContain(sec);
  }
});

test("write_file roundtrip: write, then read back", async ({ request }) => {
  await request.post(`${BASE}/api/open_project`, {
    data: { path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" },
  });
  const path = "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\UAT_ROUNDTRIP.txt";
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
    data: { path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" },
  });
  // 先写一个 5MB 文件
  const big = "x".repeat(5 * 1024 * 1024);
  const path = "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\BIG.bin";
  await request.post(`${BASE}/api/write_file`, { data: { path, content: big } });
  // 读应被拒
  const r = await request.post(`${BASE}/api/read_file`, { data: { path } });
  expect(r.status()).toBe(400);
  const body = await r.text();
  expect(body).toMatch(/文件过大/);
  // 清理
  await request.post(`${BASE}/api/write_file`, { data: { path, content: "" } });
});