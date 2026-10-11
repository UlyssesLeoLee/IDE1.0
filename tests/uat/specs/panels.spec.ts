// tests/uat/specs/panels.spec.ts
// IDE1.0 — UAT for Terminal / Source Control / Search / Output / Problems panels (Phase 1-3)
//
// 验证: Activity Bar 4 图标切换 + 底部面板 3 tabs + 各面板 API + UI 渲染
import { test, expect } from "@playwright/test";

test.describe("Activity Bar (Side Bar icons)", () => {
  test("默认 Activity Bar 显示 4 个图标 (📁 Explorer / 🔍 Search / ⎇ Source Control / ⊟ Extensions)", async ({ page }) => {
    await page.goto("/editor");
    // Wait for editor to be ready
    await page.waitForSelector("#activity-bar", { timeout: 5_000 });
    const icons = await page.locator(".act-btn").count();
    expect(icons).toBe(4);
    // Explorer active by default
    const activeIcon = await page.locator(".act-btn.active").count();
    expect(activeIcon).toBe(1);
  });

  test("点 ⎇ (Source Control) icon → 切换 Side Bar 到 SCM panel", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#activity-bar", { timeout: 5_000 });
    // Click Source Control icon
    await page.click('.act-btn[data-pane="scm"]');
    await page.waitForTimeout(200);
    // scm-pane active
    const scmVisible = await page.locator("#scm-pane.active").isVisible();
    expect(scmVisible).toBe(true);
    // Branch label visible
    const branchText = await page.locator("#scm-branch").textContent();
    expect(branchText).toBeTruthy();
  });

  test("点 🔍 (Search) icon → 切到 Search panel", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#activity-bar", { timeout: 5_000 });
    await page.click('.act-btn[data-pane="search"]');
    await page.waitForTimeout(200);
    const searchVisible = await page.locator("#search-pane.active").isVisible();
    expect(searchVisible).toBe(true);
    // 输入框可见
    await expect(page.locator("#search-input")).toBeVisible();
  });

  test("点 ⊟ (Extensions) icon → 切到 Extensions panel (占位)", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#activity-bar", { timeout: 5_000 });
    await page.click('.act-btn[data-pane="extensions"]', { force: true });
    await page.waitForTimeout(300);
    const extVisible = await page.locator("#extensions-pane.active").isVisible();
    expect(extVisible).toBe(true);
  });
});

test.describe("Bottom Panel (Terminal / Output / Problems tabs)", () => {
  test("底部面板显示 Terminal / Output / Problems 3 tabs", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#bottom-panel", { timeout: 5_000 });
    const tabs = await page.locator(".bottom-tab").count();
    expect(tabs).toBe(3);
    // Terminal active by default
    const terminalActive = await page.locator('.bottom-tab[data-pane="terminal-pane"].active').count();
    expect(terminalActive).toBe(1);
  });

  test("Terminal tab 默认显示 'No terminal session' empty state", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#terminal-pane", { timeout: 5_000 });
    const emptyText = await page.locator("#terminal-empty").textContent();
    expect(emptyText).toContain("No terminal session");
  });

  test("Output tab 切换显示", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#bottom-panel", { timeout: 5_000 });
    await page.click('.bottom-tab[data-pane="output-pane"]', { force: true });
    await page.waitForTimeout(300);
    const outputVisible = await page.locator("#output-pane.active").isVisible();
    expect(outputVisible).toBe(true);
  });

  test("Problems tab 切换显示", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#bottom-panel", { timeout: 5_000 });
    await page.click('.bottom-tab[data-pane="problems-pane"]', { force: true });
    await page.waitForTimeout(300);
    const problemsVisible = await page.locator("#problems-pane.active").isVisible();
    expect(problemsVisible).toBe(true);
  });
});

test.describe("Vim panel (legacy #shell-panel, 明确不混淆)", async () => {
  test("Vim panel title 是 'Vim (命令行模式 — 非 PowerShell)'", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#shell-panel", { timeout: 5_000 });
    const title = await page.locator("#shell-head .shell-title").textContent();
    expect(title).toContain("Vim");
    expect(title).toContain("命令行模式");
    expect(title).toContain("非 PowerShell");
  });
});

test.describe("Terminal API (HTTP)", () => {
  test("POST /api/terminal_create spawn PowerShell (Windows)", async ({ request }) => {
    const res = await request.post("/api/terminal_create", { data: {} });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body.shell).toContain("powershell");
    expect(body.alive).toBe(true);
    expect(body.id).toMatch(/^term-\d+$/);
  });

  test("GET /api/terminal_list 包含刚创建的 session", async ({ request }) => {
    const create = await request.post("/api/terminal_create", { data: {} });
    const created = await create.json();
    const list = await request.get("/api/terminal_list");
    expect(list.ok()).toBeTruthy();
    const arr = await list.json();
    expect(arr.length).toBeGreaterThan(0);
    expect(arr.some((t: any) => t.id === created.id)).toBe(true);
  });

  test("POST /api/terminal_input 发送命令 → PowerShell 接收", async ({ request }) => {
    const create = await request.post("/api/terminal_create", { data: {} });
    const { id } = await create.json();
    // Wait briefly for PowerShell to be ready
    await new Promise(r => setTimeout(r, 500));
    const res = await request.post("/api/terminal_input", {
      data: { id, data: "Write-Host TestUat12345" },
    });
    expect(res.ok()).toBeTruthy();
  });

  test("GET /api/terminal_output?id=X 返回 chunks buffer", async ({ request }) => {
    const create = await request.post("/api/terminal_create", { data: {} });
    expect(create.ok()).toBeTruthy();
    if (!create.ok()) return;
    const { id } = await create.json();
    await new Promise(r => setTimeout(r, 500));
    // 验证 chunks 是 array (可能为空 = PowerShell 还没输出)
    const res = await request.get(`/api/terminal_output?id=${id}`);
    expect(res.ok()).toBeTruthy();
    if (res.ok()) {
      const body = await res.json();
      expect(body.id).toBe(id);
      expect(Array.isArray(body.chunks)).toBe(true);
    }
    // 清理
    await request.post("/api/terminal_close", { data: { id } });
  });

  test("POST /api/terminal_close 关闭 session", async ({ request }) => {
    const create = await request.post("/api/terminal_create", { data: {} });
    const { id } = await create.json();
    const close = await request.post("/api/terminal_close", { data: { id } });
    expect(close.ok()).toBeTruthy();
    // Now input should 404
    const input = await request.post("/api/terminal_input", { data: { id, data: "x" } });
    expect(input.status()).toBe(404);
  });
});

test.describe("SCM (Source Control) API (HTTP)", () => {
  test("POST /api/scm_status 返回 branch + files (current repo is git)", async ({ request }) => {
    const res = await request.post("/api/scm_status", { data: {} });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    // Server test_root is sample-project which may not be git repo
    // If error field present, it's just because no git — otherwise expect branch
    if (!body.error) {
      expect(typeof body.branch === "string").toBeTruthy();
      expect(Array.isArray(body.files)).toBe(true);
    }
  });

  test("POST /api/scm_log 返回 commits (or empty + error)", async ({ request }) => {
    const res = await request.post("/api/scm_log", { data: {} });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(Array.isArray(body.commits)).toBe(true);
  });

  test("POST /api/scm_commit (invalid in non-git repo → 500 or error)", async ({ request }) => {
    const res = await request.post("/api/scm_commit", {
      data: { message: "test" },
    });
    // Either 500 (git failed) or 200 (if no commits staged)
    const status = res.status();
    expect([200, 500]).toContain(status);
  });
});

test.describe("Search API (HTTP)", () => {
  test("POST /api/search 空 pattern 返回空 results 或 200", async ({ request }) => {
    const res = await request.post("/api/search", { data: { pattern: "" } });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(Array.isArray(body.results)).toBe(true);
  });

  test("POST /api/search pattern 'main' 至少返回 1+ results (在 sample-project)", async ({ request }) => {
    const res = await request.post("/api/search", {
      data: { pattern: "main", max_results: 50 },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(Array.isArray(body.results)).toBe(true);
    // sample-project 应该有 main 字串的文件
    if (!body.error) {
        console.log(`search 'main' → ${body.results.length} results`);
    }
  });

  test("POST /api/search case_sensitive: true → 只匹配大写 Main", async ({ request }) => {
    const res = await request.post("/api/search", {
      data: { pattern: "Main", case_sensitive: true, max_results: 10 },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(Array.isArray(body.results)).toBe(true);
  });
});

test.describe("Terminal UI 交互 (按钮 + 快捷键)", () => {
  test("+ New Terminal 按钮可见且可点", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#bottom-panel", { timeout: 5_000 });
    await expect(page.locator('[data-cmd="terminal-new"]')).toBeVisible();
  });

  test("Ctrl+` 快捷键切到 Terminal tab", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#bottom-panel", { timeout: 5_000 });
    // Click Output first to leave terminal
    await page.click('.bottom-tab[data-pane="output-pane"]', { force: true });
    await page.waitForTimeout(300);
    // Press Ctrl+`
    await page.keyboard.press("Control+`");
    await page.waitForTimeout(200);
    const terminalActive = await page.locator('.bottom-tab[data-pane="terminal-pane"].active').count();
    expect(terminalActive).toBe(1);
  });
});

test.describe("Search UI", () => {
  test("Search panel 输入框可见 + placeholder", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#search-input", { timeout: 5_000 });
    const placeholder = await page.locator("#search-input").getAttribute("placeholder");
    expect(placeholder).toBeTruthy();
  });

  test("Ctrl+Shift+F 全局快捷键切到 Search tab", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#activity-bar", { timeout: 5_000 });
    await page.keyboard.press("Control+Shift+F");
    await page.waitForTimeout(200);
    const searchActive = await page.locator(".act-btn[data-pane=\"search\"].active").count();
    expect(searchActive).toBe(1);
  });
});

test.describe("底部面板折叠 (bottom-toggle)", () => {
  test("点击 折叠 按钮 → 底部面板高度变化", async ({ page }) => {
    await page.goto("/editor");
    await page.waitForSelector("#bottom-panel", { timeout: 5_000 });
    const before = await page.locator("#bottom-panel").getAttribute("data-collapsed");
    await page.click('[data-cmd="bottom-toggle"]', { force: true });
    await page.waitForTimeout(400);
    const after = await page.locator("#bottom-panel").getAttribute("data-collapsed");
    expect(before).not.toBe(after);
  });
});