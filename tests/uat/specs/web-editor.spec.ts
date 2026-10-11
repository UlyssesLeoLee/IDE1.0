// tests/uat/specs/web-editor.spec.ts
// Web 版 /editor (与桌面 dist/index.html 同一前端) 对真实 ide-shell-web HTTP API 的端到端回归 (ULYS-251).
// 项目根 = webServer env IDE_SHELL_WEB_TEST_ROOT (fixtures/sample-project).
import { test, expect, type Page } from "@playwright/test";

async function boot(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("dialog", (d) => { errors.push("native dialog: " + d.message()); d.dismiss(); });
  await page.goto("/editor");
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady());
  return errors;
}

// web 无原生对话框: 打开文件夹弹出路径输入框, 预填服务端默认根
async function openProject(page: Page) {
  await page.locator("#welcome .open-btn").click();
  await expect(page.locator("#modal")).toHaveClass(/show/);
  await expect(page.locator(".md-input")).toHaveValue(/sample-project/);
  await page.locator(".md-input").press("Enter");
  await expect(page.locator("#side-body")).toContainText("README.md");
}

test.describe("web /editor (ULYS-251)", () => {
  test("加载无 JS 错误, banner 来自 kernel", async ({ page }) => {
    const errors = await boot(page);
    await expect(page.locator("#menubar .menu-item")).toHaveCount(8);
    await expect(page.locator("#s-tip")).toContainText("ide-kernel-core");
    expect(errors).toEqual([]);
  });

  test("打开项目 → 展开 src → 打开文件 → 编辑 → 保存到磁盘", async ({ page, request }) => {
    const errors = await boot(page);
    await openProject(page);
    await page.locator('.tree-node:has-text("src")').first().click();
    await expect(page.locator("#side-body")).toContainText("main.rs");
    await page.locator('.tree-node:has-text("README.md")').click();
    await expect(page.locator(".tab.active")).toContainText("README.md");
    const original = await page.locator("#ed-content").inputValue();
    expect(original.length).toBeGreaterThan(0);
    // 面包屑为相对路径 (不含 \\?\ 前缀)
    await expect(page.locator("#breadcrumb")).not.toContainText("?");
    const readmePath = await page.evaluate(() => (window as any).getActiveTab().path);
    try {
      await page.locator("#ed-content").press("Shift+A");
      await page.keyboard.type(" uat");
      await page.keyboard.press("Escape");
      await page.keyboard.press("Control+s");
      await expect(page.locator(".tab.active")).not.toHaveClass(/tab-dirty/);
      const res = await request.post("/api/read_file", { data: { path: readmePath } });
      expect((await res.json()).content.replace(/\r\n/g, "\n").split("\n")[0]).toMatch(/ uat$/);
    } finally {
      await request.post("/api/write_file", { data: { path: readmePath, content: original } });
    }
    expect(errors).toEqual([]);
  });

  test("项目搜索 (Ctrl+G) → 点击结果打开文件并跳到行", async ({ page }) => {
    const errors = await boot(page);
    await openProject(page);
    await page.keyboard.press("Control+g");
    await page.locator("#search-input").fill("fn main");
    await page.locator("#search-input").press("Enter");
    const hit = page.locator(".search-hit").filter({ hasText: "main.rs" }).first();
    await expect(hit).toBeVisible();
    await hit.click();
    await expect(page.locator(".tab.active")).toContainText("main.rs");
    await expect(page.locator("#ed-content")).toHaveValue(/fn main/);
    expect(errors).toEqual([]);
  });

  test("大纲 (Rust) 列出函数, 点击跳转", async ({ page }) => {
    const errors = await boot(page);
    await openProject(page);
    await page.locator('.tree-node:has-text("src")').first().click();
    await page.locator('.tree-node:has-text("main.rs")').click();
    await page.keyboard.press("Control+Shift+O");
    await expect(page.locator("#outline-list")).toContainText("main");
    expect(errors).toEqual([]);
  });
});
