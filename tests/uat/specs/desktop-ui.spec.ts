// tests/uat/specs/desktop-ui.spec.ts
// Desktop (Tauri) 前端 dist/index.html 的 UI/UX 回归 (ULYS-251).
//
// 不启动 Tauri: 直接以 file:// 打开 dist/index.html, 用 addInitScript 注入
// window.__TAURI_INTERNALS__.invoke / window.__TAURI__ (event/window) 的内存 mock.
// 目标: 菜单上显示的每一项都能执行 (无 JS 异常, 无 "未实装"/"未知命令" 提示),
// 关键流程 (打开项目/编辑/保存/查找替换/终端) 端到端可用.
import { test, expect, type Page } from "@playwright/test";
import * as path from "path";
import { pathToFileURL } from "url";

const DIST = pathToFileURL(
  path.resolve(__dirname, "../../../crates/ide-shell-desktop/dist/index.html"),
).href;

function installMock() {
  const ROOT = "C:/proj";
  const files: Record<string, string> = {
    "C:/proj/README.md": "hello\nworld\nhello world\n",
    "C:/proj/src/main.rs": "fn helper() {}\n\nfn main() {\n    helper();\n}\n",
  };
  const dirs: Record<string, string[]> = { "C:/proj": ["src", "README.md"], "C:/proj/src": ["main.rs"] };
  const listeners: Record<string, (ev: unknown) => void> = {};
  const w = window as any;
  w.__calls = [];
  w.__files = files;
  w.__fullscreen = false;
  w.__TAURI__ = {
    event: {
      listen: async (name: string, cb: (ev: unknown) => void) => { listeners[name] = cb; return () => {}; },
    },
    window: {
      getCurrentWindow: () => ({
        isFullscreen: async () => w.__fullscreen,
        setFullscreen: async (v: boolean) => { w.__fullscreen = v; },
      }),
    },
  };
  w.__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: any) => {
      w.__calls.push({ cmd, args });
      const norm = (p: string) => p.replace(/\\/g, "/");
      switch (cmd) {
        case "kernel_banner": return "ide-kernel-core mock";
        case "help_wiki": return "IDE1.0 wiki (mock)";
        case "pick_folder": return ROOT;
        case "open_project": return args.path;
        case "list_dir": {
          const p = norm(args.path);
          if (!dirs[p]) throw "不是目录: " + p;
          return dirs[p].map((name) => ({ name, path: p + "/" + name, is_dir: !!dirs[p + "/" + name] }));
        }
        case "read_file": {
          const p = norm(args.path);
          if (!(p in files)) throw "文件不存在: " + p;
          return { name: p.split("/").pop(), path: p, content: files[p] };
        }
        case "write_file": files[norm(args.path)] = args.content; return args.content.length;
        case "search_cmd": {
          const out: any[] = [];
          for (const [p, text] of Object.entries(files)) {
            text.split("\n").forEach((line, i) => {
              if (line.toLowerCase().includes(args.options.pattern.toLowerCase())) out.push({ path: p, line: i + 1, text: line });
            });
          }
          return out;
        }
        case "scm_status": return { branch: "main", files: [{ path: "README.md", status: "modified", staged: false }] };
        case "outline": {
          const p = norm(args.path);
          if (p.endsWith(".rs")) return { path: p, language: "Rust", entries: [{ name: "helper", kind: "function", line: 0 }, { name: "main", kind: "function", line: 2 }], error: null };
          return { path: p, language: "Plain", entries: [], error: null };
        }
        case "terminal_create": return { id: "term-1", title: "PowerShell #1", shell: "powershell.exe", cwd: args.cwd || ROOT, alive: true };
        case "terminal_input":
          setTimeout(() => listeners["terminal://output"]?.({ payload: { id: args.id, stream: "stdout", data: "echo: " + args.data } }), 0);
          return null;
        case "terminal_close": return null;
        default: throw "unknown command " + cmd;
      }
    },
  };
}

async function boot(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("dialog", (d) => { errors.push("native dialog: " + d.message()); d.dismiss(); });
  await page.addInitScript(installMock);
  await page.goto(DIST);
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady());
  return errors;
}

async function openProject(page: Page) {
  await page.locator("#welcome .open-btn").click();
  await expect(page.locator("#side-body")).toContainText("README.md");
}

async function openReadme(page: Page) {
  await openProject(page);
  await page.locator('.tree-node:has-text("README.md")').click();
  await expect(page.locator("#ed-content")).toHaveValue(/hello/);
}

async function clickMenuRow(page: Page, cmd: string) {
  const row = page.locator(`#menubar .menu-row[data-cmd="${cmd}"]`).first();
  const menu = row.locator("xpath=ancestor::div[contains(@class,'menu-item')][1]");
  await menu.locator(".mb-btn").click();
  const parentRow = row.locator("xpath=ancestor::div[contains(@class,'menu-submenu')][1]");
  if (await parentRow.count()) await parentRow.hover();
  await row.click();
}

test.describe("desktop dist UI (ULYS-251)", () => {
  test("加载无 JS 错误, 菜单/工具栏渲染完整", async ({ page }) => {
    const errors = await boot(page);
    await expect(page.locator("#menubar .menu-item")).toHaveCount(8);
    await expect(page.locator("#welcome")).toBeVisible();
    await expect(page.locator("#s-tip")).toContainText("ide-kernel-core mock");
    // 状态栏完整处于视口内 (body 默认外边距曾导致其被裁掉)
    const box = await page.locator("#status").boundingBox();
    const vh = page.viewportSize()!.height;
    expect(box!.y + box!.height).toBeLessThanOrEqual(vh);
    expect(errors).toEqual([]);
  });

  test("每个菜单项都可执行 (无异常 / 无 未实装 / 无 未知命令)", async ({ page }) => {
    test.setTimeout(90_000);
    const errors = await boot(page);
    await openReadme(page);
    const cmds = await page.locator("#menubar .menu-row[data-cmd]").evaluateAll((rows) =>
      rows.map((r) => (r as HTMLElement).dataset.cmd!).filter((c) => c !== "open-recent"));
    expect(cmds.length).toBeGreaterThan(50);
    // 关闭类命令最后执行, 保证其他命令有打开的文件可操作
    const last = ["close-tab", "close-all", "reset-settings", "fullscreen"];
    const ordered = [...cmds.filter((c) => !last.includes(c)), ...last.filter((c) => cmds.includes(c))];
    for (const cmd of ordered) {
      await clickMenuRow(page, cmd);
      await page.waitForTimeout(30);
      // 命令可能打开对话框 / 查找栏 → 关掉以便继续
      if (await page.locator("#modal.show").count()) await page.keyboard.press("Escape");
      if (await page.locator("#modal.show").count()) await page.locator("#md-foot .md-btn").first().click();
      const tip = await page.locator("#s-tip").textContent();
      expect(tip, cmd).not.toMatch(/未实装|未知命令|命令失败/);
    }
    expect(errors).toEqual([]);
  });

  test("打开项目 → 展开文件夹 → 打开文件 → vim 编辑 → Ctrl+S 保存", async ({ page }) => {
    const errors = await boot(page);
    await openProject(page);
    await page.locator('.tree-node:has-text("src")').click();
    await expect(page.locator("#side-body")).toContainText("main.rs");
    await page.locator('.tree-node:has-text("README.md")').click();
    await expect(page.locator(".tab.active")).toContainText("README.md");
    await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "NORMAL");
    // NORMAL 模式下键入不修改文本
    await page.locator("#ed-content").press("z");
    await expect(page.locator("#ed-content")).toHaveValue("hello\nworld\nhello world\n");
    await page.locator("#ed-content").press("A");
    await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT");
    await page.keyboard.type("!");
    await page.keyboard.press("Escape");
    await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "NORMAL");
    await expect(page.locator(".tab.active")).toHaveClass(/tab-dirty/);
    await page.keyboard.press("Control+s");
    await expect(page.locator(".tab.active")).not.toHaveClass(/tab-dirty/);
    expect(await page.evaluate(() => (window as any).__files["C:/proj/README.md"])).toBe("hello!\nworld\nhello world\n");
    // dd 删除行, u 撤销
    await page.locator("#ed-content").press("d");
    await page.locator("#ed-content").press("d");
    await expect(page.locator("#ed-content")).toHaveValue("world\nhello world\n");
    await page.locator("#ed-content").press("u");
    await expect(page.locator("#ed-content")).toHaveValue("hello!\nworld\nhello world\n");
    expect(errors).toEqual([]);
  });

  test("查找 / 替换 / 跳转到行 / 书签", async ({ page }) => {
    const errors = await boot(page);
    await openReadme(page);
    await page.keyboard.press("Control+f");
    await expect(page.locator("#findbar")).toBeVisible();
    await page.locator("#find-input").fill("hello");
    await page.locator("#find-input").press("Enter");
    await expect(page.locator("#find-count")).toHaveText(/\d \/ 2/);
    // 仅查找模式下 "全部替换" 只展开替换框, 不会用空串替换
    await page.keyboard.press("Control+Alt+r");
    await expect(page.locator("#findbar")).toHaveClass(/with-replace/);
    await expect(page.locator("#ed-content")).toHaveValue("hello\nworld\nhello world\n");
    await page.keyboard.press("Control+h");
    await page.locator("#replace-input").fill("bye");
    await page.locator('#findbar [data-cmd="replace-all"]').click();
    await expect(page.locator("#ed-content")).toHaveValue("bye\nworld\nbye world\n");
    await page.keyboard.press("Escape");
    await expect(page.locator("#findbar")).toBeHidden();
    // Ctrl+Z 撤销全部替换 (原生撤销栈)
    await page.locator("#ed-content").focus();
    await page.keyboard.press("Control+z");
    await expect(page.locator("#ed-content")).toHaveValue("hello\nworld\nhello world\n");
    await page.keyboard.press("Control+j");
    await page.locator(".md-input").fill("3");
    await page.locator(".md-input").press("Enter");
    await expect(page.locator("#s-pos")).toContainText("Ln 3");
    await page.keyboard.press("Control+F2");
    await expect(page.locator("#gutter")).toContainText("◆ 3");
    expect(errors).toEqual([]);
  });

  test("转换: 半角片假名 ↔ 全角, 全角英数", async ({ page }) => {
    const errors = await boot(page);
    await page.keyboard.press("Control+n");
    await page.keyboard.type("ｶﾞｷﾞ abc");
    await page.keyboard.press("Escape");
    await page.locator("#ed-content").press("Control+a");
    await clickMenuRow(page, "to-zenkaku");
    await expect(page.locator("#ed-content")).toHaveValue("ガギ abc");
    await clickMenuRow(page, "to-hankata");
    await expect(page.locator("#ed-content")).toHaveValue("ｶﾞｷﾞ abc");
    await clickMenuRow(page, "to-zenhira");
    await expect(page.locator("#ed-content")).toHaveValue("がぎ abc");
    await clickMenuRow(page, "to-fullwidth");
    await expect(page.locator("#ed-content")).toHaveValue("がぎ　ａｂｃ");
    expect(errors).toEqual([]);
  });

  test("终端: 创建 → 发送命令 → 显示输出 → 结束", async ({ page }) => {
    const errors = await boot(page);
    await openProject(page);
    await page.locator('#menubar .tb-btn[data-cmd="show-terminal"]').click();
    await expect(page.locator("#terminal-input")).toBeVisible();
    const created = await page.evaluate(() => (window as any).__calls.filter((c: any) => c.cmd === "terminal_create"));
    expect(created).toHaveLength(1);
    expect(created[0].args.cwd).toBe("C:/proj");
    await page.locator("#terminal-input").fill("dir");
    await page.locator("#terminal-input").press("Enter");
    await expect(page.locator("#terminal-body")).toContainText("echo: dir");
    await page.locator('.bottom-toggle[data-cmd="kill-terminal"]').click();
    await expect(page.locator("#terminal-body")).toContainText("会话已结束");
    await expect(page.locator("#terminal-input")).toBeHidden();
    expect(errors).toEqual([]);
  });

  test("底部面板折叠按钮只切换一次; 关闭未保存标签会询问", async ({ page }) => {
    const errors = await boot(page);
    await page.locator('.bottom-toggle[data-cmd="toggle-bottom"]').click();
    await expect(page.locator("#bottom-panel")).toHaveAttribute("data-collapsed", "true");
    await page.locator('.bottom-toggle[data-cmd="toggle-bottom"]').click();
    await expect(page.locator("#bottom-panel")).toHaveAttribute("data-collapsed", "false");
    await page.keyboard.press("Control+n");
    await page.keyboard.type("draft");
    await page.keyboard.press("Control+w");
    await expect(page.locator("#modal")).toHaveClass(/show/);
    await expect(page.locator("#md-body")).toContainText("未保存");
    await page.locator("#md-foot .md-btn", { hasText: "不保存" }).click();
    await expect(page.locator(".tab")).toHaveCount(0);
    await expect(page.locator("#welcome")).toBeVisible();
    expect(errors).toEqual([]);
  });

  test("浅色主题下下拉菜单为浅色背景 (文字可读)", async ({ page }) => {
    await boot(page);
    await page.evaluate(() => (window as any).applyTheme("light"));
    await page.locator('.menu-item[data-menu="file"] .mb-btn').click();
    const bg = await page.locator('.menu-dropdown[data-dropdown="file"]').evaluate((el) => getComputedStyle(el).backgroundColor);
    const [r, g, b] = bg.match(/\d+/g)!.map(Number);
    expect((r + g + b) / 3).toBeGreaterThan(200);
  });
});
