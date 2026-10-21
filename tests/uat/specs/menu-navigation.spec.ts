// tests/uat/specs/menu-navigation.spec.ts
// 顶端菜单栏的行为对齐 Sakura / 原生 Windows 菜单, 两种界面布局 (完整 IDE / 极简) 下一模一样:
//   鼠标: 点击展开, 再点 / 点外面 / Esc 收起; 有菜单展开时鼠标挪到别的顶层按钮上立刻切过去 (不用再点); 下拉里悬停子菜单照旧展开.
//   键盘: Alt+助记字母 (F/E/S/V/T/C/W/H) 开对应菜单; 单按 Alt 把焦点交给菜单栏;
//         菜单里 ←→ 换相邻菜单 (首尾循环) / 进出子菜单, ↑↓ 走行 (跳过分隔线和禁用行, 首尾循环), Home/End, Enter 执行, Esc 关闭并回到编辑区;
//         这些按键不会漏进编辑区 (文本和光标都不变).
//
// 两个 describe 各跑一遍同样的用例: "完整布局" 用 playwright.config.ts 注入的 layout=full + vim=on (NORMAL/INSERT 三态, 最容易被漏进去的方向键伤到);
// "极简布局" 用空 storageState = 真正的首次启动 (极简 + 非模态).
// 状态读自 window.getMenuState() ({open, row, sub, bar, armed}), 另外直接看 DOM (.menu-item.open / .kb-active / .kb-open / aria-expanded).
import { test, expect, type Page } from "@playwright/test";

const ADDR = process.env.IDE_SHELL_WEB_ADDR || "127.0.0.1:8123";

const MNEMONIC: Record<string, string> = { f: "file", e: "edit", s: "search", v: "view", t: "tool", c: "setting", w: "window", h: "help" };
const MENUS = ["file", "edit", "search", "view", "tool", "setting", "window", "help"];

interface MenuState { open: string | null; row: string | null; sub: string | null; bar: string | null; armed: boolean }

/** 打开 /editor, 收集 pageerror, 等 bootstrap 完成. */
async function boot(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/editor");
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
  return errors;
}
const st = (page: Page): Promise<MenuState> => page.evaluate(() => (window as any).getMenuState());
const btn = (page: Page, menu: string) => page.locator(`#menubar .menu-item[data-menu="${menu}"] > .mb-btn`);
const focusEditor = (page: Page) => page.evaluate(() => document.getElementById("ed-content")!.focus());
const editorFocused = (page: Page) => page.evaluate(() => document.activeElement === document.getElementById("ed-content"));
const parkMouse = (page: Page) => page.mouse.move(640, 420);   // 把鼠标停在编辑区, 免得它压在菜单栏 / 下拉上干扰键盘用例
/** 当前展开的菜单里, 键盘能走到的顶层行 (直接子节点里的 .menu-row, 去掉禁用行): 与 editor.html 的 rowsOf 同口径. */
const topRows = (page: Page, menu: string): Promise<string[]> =>
  page.evaluate((m) => Array.from(document.querySelector(`.menu-dropdown[data-dropdown="${m}"]`)!.children)
    .filter((r) => r.classList.contains("menu-row") && !r.classList.contains("disabled"))
    .map((r) => (r as HTMLElement).dataset.cmd || (r as HTMLElement).dataset.submenu || ""), menu);
const openMenus = (page: Page) => page.evaluate(() => Array.from(document.querySelectorAll(".menu-item.open")).map((e) => (e as HTMLElement).dataset.menu));
const kbLeft = (page: Page) => page.evaluate(() => document.querySelectorAll("#menubar .kb-active, #menubar .kb-open").length);
const docState = (page: Page) => page.evaluate(() => {
  const w = window as any, t = w.getActiveTab();
  return JSON.stringify({ lines: t.lines, cursor: t.cursor, pos: document.getElementById("s-pos")!.textContent, mode: document.getElementById("s-mode")!.dataset.mode });
});
/** 准备一个有两行文本、光标不在原点的文档, 焦点在编辑区. 完整布局 (vim 开) 先 i 进 INSERT 打字, 再 Esc 回 NORMAL —— 这样漏进去的方向键会真的移动光标. */
async function prepDoc(page: Page, layout: "full" | "min") {
  if (layout === "full") await page.evaluate(() => (window as any).createUntitledDoc());   // 完整布局启动是欢迎页, 没有文档
  await focusEditor(page);
  if (layout === "full") await page.keyboard.press("i");
  await page.keyboard.type("hello");
  await page.keyboard.press("Enter"); await page.evaluate(() => (window as any).__editIdle && (window as any).__editIdle());
  await page.keyboard.type("world");
  if (layout === "full") await page.keyboard.press("Escape");
  await expect.poll(() => page.evaluate(() => (window as any).getActiveTab().lines.join("|"))).toBe("hello|world");
}
/** ↓ 直到高亮行是 cmd (上限 40 次, 防死循环); 返回是否到达. */
async function downTo(page: Page, cmd: string): Promise<boolean> {
  for (let i = 0; i < 40; i++) {
    if ((await st(page)).row === cmd) return true;
    await page.keyboard.press("ArrowDown");
  }
  return (await st(page)).row === cmd;
}

const FULL_STATE = {
  cookies: [],
  origins: [{ origin: `http://${ADDR}`, localStorage: [{ name: "ide10.layout", value: "full" }, { name: "ide10.vim", value: "on" }] }],
};

for (const layout of ["full", "min"] as const) {
  test.describe(layout === "full" ? "菜单栏 · 完整布局 (vim 开)" : "菜单栏 · 极简布局 (首次启动)", () => {
    test.use({ storageState: layout === "full" ? FULL_STATE : { cookies: [], origins: [] } });

    // ------------------------------------------------------------ 鼠标
    test("点击展开 / 再点收起; 没有菜单展开时悬停不会打开", async ({ page }) => {
      const errors = await boot(page);
      expect((await page.evaluate(() => (window as any).getLayout())).preset).toBe(layout);
      await btn(page, "edit").hover();
      expect((await st(page)).open).toBeNull();
      await btn(page, "file").click();
      expect((await st(page)).open).toBe("file");
      await expect(btn(page, "file")).toHaveAttribute("aria-expanded", "true");
      await expect(page.locator('.menu-dropdown[data-dropdown="file"]')).toBeVisible();
      await parkMouse(page);              // 鼠标离开按钮再点同一按钮 = 收起
      await btn(page, "file").click();
      expect((await st(page)).open).toBeNull();
      await expect(btn(page, "file")).toHaveAttribute("aria-expanded", "false");
      expect(errors).toEqual([]);
    });

    test("悬停切换: 展开着一个菜单时, 鼠标移到另一个顶层按钮立刻切过去; 紧接着的点击是确认, 不会关掉", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "file").click();
      await btn(page, "edit").hover();
      expect(await openMenus(page)).toEqual(["edit"]);                      // 上一个收起, 只有一个
      await btn(page, "view").hover();
      expect(await openMenus(page)).toEqual(["view"]);
      await btn(page, "help").hover();
      expect(await openMenus(page)).toEqual(["help"]);
      await btn(page, "help").click();                                      // 点在刚悬停切过来的菜单上: 保持打开
      expect(await openMenus(page)).toEqual(["help"]);
      await parkMouse(page);
      await btn(page, "help").click();                                      // 离开后再点 = 收起
      expect(await openMenus(page)).toEqual([]);
      // Playwright 的 click 会先把鼠标挪过去 (触发悬停切换) 再点: 展开着 file 时直接点 tool, 结果必须是 tool 开着
      await btn(page, "file").click();
      await btn(page, "tool").click();
      expect(await openMenus(page)).toEqual(["tool"]);
      expect(errors).toEqual([]);
    });

    test("悬停切换后, 下拉里的子菜单仍然悬停展开 / 悬停离开收起", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "view").click();
      const sub = page.locator('.menu-dropdown[data-dropdown="view"] > [data-submenu="layout"] > .submenu-dropdown');
      await expect(sub).toBeHidden();
      await page.locator('[data-submenu="layout"]').first().hover();
      await expect(sub).toBeVisible();
      await btn(page, "edit").hover();                                      // 切到编辑: 视图及其飞出层都消失
      await expect(sub).toBeHidden();
      await btn(page, "view").hover();                                      // 再切回视图: 子菜单要重新悬停才展开
      expect(await openMenus(page)).toEqual(["view"]);
      await expect(sub).toBeHidden();
      await page.locator('[data-submenu="layout"]').first().hover();
      await expect(sub).toBeVisible();
      expect(errors).toEqual([]);
    });

    test("鼠标: 点外面 / 点编辑区 收起菜单; Esc 收起并把焦点还给编辑区", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "view").click();
      await page.mouse.click(640, 420);
      expect(await openMenus(page)).toEqual([]);
      await btn(page, "file").click();
      await page.keyboard.press("Escape");
      expect(await openMenus(page)).toEqual([]);
      expect(await editorFocused(page)).toBe(true);
      expect(await kbLeft(page)).toBe(0);
      expect(errors).toEqual([]);
    });

    test("键盘高亮和鼠标共用一个: 悬停某行后 ↓ 从它接着走, 鼠标离开该行高亮消失", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "edit").click();
      await page.locator('.menu-dropdown[data-dropdown="edit"] [data-cmd="cut"]').hover();
      expect((await st(page)).row).toBe("cut");
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("copy");
      await parkMouse(page);                                                 // 键盘选的行, 鼠标离开不清
      expect((await st(page)).row).toBe("copy");
      await page.locator('.menu-dropdown[data-dropdown="edit"] [data-cmd="paste"]').hover();
      expect((await st(page)).row).toBe("paste");
      await page.locator('.menu-dropdown[data-dropdown="edit"] > .menu-sep').first().hover();   // 分隔线: 仍在下拉里, 但不在任何行上
      expect((await st(page)).row).toBeNull();                               // 鼠标选的行, 鼠标离开就清
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ 键盘: 行
    test("↑↓: 跳过分隔线, 首尾循环; Home / End; 跳过禁用行", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt+f");
      let s = await st(page);
      expect(s).toMatchObject({ open: "file", row: "new-file" });             // 键盘打开: 第一行已高亮
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("open-folder");
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("open-file");
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("save");                              // 「打开文件...」后面是分隔线, 跳过
      await page.keyboard.press("End");
      const rows = await topRows(page, "file");
      expect((await st(page)).row).toBe(rows[rows.length - 1]);
      await page.keyboard.press("ArrowDown");                                 // 末行 ↓ 回到首行
      expect((await st(page)).row).toBe("new-file");
      await page.keyboard.press("ArrowUp");                                   // 首行 ↑ 回到末行
      expect((await st(page)).row).toBe(rows[rows.length - 1]);
      await page.keyboard.press("Home");
      expect((await st(page)).row).toBe("new-file");
      // 禁用行: 测试里临时把 打开文件夹 标成 disabled
      await page.evaluate(() => document.querySelector('[data-cmd="open-folder"]')!.classList.add("disabled"));
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("open-file");
      await page.keyboard.press("Home");
      await page.keyboard.press("ArrowUp");
      await page.keyboard.press("ArrowUp");
      expect((await st(page)).row).not.toBe("open-folder");
      await page.evaluate(() => document.querySelector('[data-cmd="open-folder"]')!.classList.remove("disabled"));
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    test("每个菜单用 ↓ 能依次走遍所有顶层行并绕回第一行", async ({ page }) => {
      test.setTimeout(40_000);
      const errors = await boot(page);
      await focusEditor(page);
      for (const m of MENUS) {
        await btn(page, m).click();
        const rows = await topRows(page, m);
        expect(rows.length, `${m} 有可走的行`).toBeGreaterThan(2);
        await page.keyboard.press("ArrowDown");                               // 鼠标打开: 第一次 ↓ 到第一行
        for (let i = 0; i < rows.length; i++) {
          expect((await st(page)).row, `${m} 第 ${i + 1} 行`).toBe(rows[i]);
          await page.keyboard.press("ArrowDown");
        }
        expect((await st(page)).row, `${m} 绕回第一行`).toBe(rows[0]);
        await page.keyboard.press("Escape");
        expect(await kbLeft(page)).toBe(0);
      }
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ 键盘: 菜单之间 / 子菜单
    test("←→: 在顶层菜单间移动并打开 (首尾循环), 打开后第一行高亮", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt+f");
      await page.keyboard.press("ArrowLeft");
      expect(await st(page)).toMatchObject({ open: "help", row: "help" });     // 文件 ← 绕到 帮助 (第一行高亮)
      await page.keyboard.press("ArrowRight");
      expect((await st(page)).open).toBe("file");                             // 帮助 → 绕回 文件
      const seen: string[] = [];
      for (let i = 0; i < MENUS.length; i++) {
        seen.push((await st(page)).open!);
        await page.keyboard.press("ArrowRight");
        // 视图菜单的第一行是「界面布局」子菜单父行, 它上面 → 是进入子菜单 (见下一个用例); 再按一次才换菜单
        if ((await st(page)).sub) await page.keyboard.press("ArrowRight");
      }
      expect(seen).toEqual(MENUS);                                            // 一圈正好是 8 个菜单, 顺序 = 菜单栏顺序
      expect(await openMenus(page)).toEqual(["file"]);
      for (const m of ["help", "window", "setting"]) {                          // 反方向: 文件 ← 帮助 ← 窗口 ← 设置
        await page.keyboard.press("ArrowLeft");
        expect((await st(page)).open).toBe(m);
      }
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    test("子菜单: → 进入, ↓ 跳过分隔线, ← 退回父行; 叶子行上 → 去下一个菜单", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt+v");
      expect(await st(page)).toMatchObject({ open: "view", row: "layout", sub: null });   // 视图第一行是 界面布局 子菜单
      const sub = page.locator('.menu-dropdown[data-dropdown="view"] > [data-submenu="layout"] > .submenu-dropdown');
      await expect(sub).toBeHidden();
      await page.keyboard.press("ArrowRight");
      expect(await st(page)).toMatchObject({ open: "view", sub: "layout", row: "layout-min" });
      await expect(sub).toBeVisible();                                         // 键盘展开, 鼠标不在上面
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("layout-full");
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("toggle-activity-bar");                // 分隔线后的第一行
      await page.keyboard.press("End");
      expect((await st(page)).row).toBe("toggle-breadcrumb");
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("layout-min");                          // 子菜单内部循环
      await page.keyboard.press("ArrowLeft");                                  // ← 先退出子菜单, 不换菜单
      expect(await st(page)).toMatchObject({ open: "view", row: "layout", sub: null });
      await expect(sub).toBeHidden();
      await page.keyboard.press("ArrowRight");                                 // 再进
      expect((await st(page)).sub).toBe("layout");
      await page.keyboard.press("ArrowLeft");
      await page.keyboard.press("ArrowDown");                                  // 子菜单已收起, ↓ 在顶层走
      expect((await st(page)).row).not.toBe("layout-min");
      expect(await downTo(page, "toggle-status")).toBe(true);
      await page.keyboard.press("ArrowRight");                                 // 叶子行上 → = 下一个菜单 (工具)
      expect(await st(page)).toMatchObject({ open: "tool", sub: null });
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    test("子菜单父行上 Enter = 进入子菜单 (不关菜单, 不执行)", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt+v");
      await page.keyboard.press("Enter"); await page.evaluate(() => (window as any).__editIdle && (window as any).__editIdle());
      expect(await st(page)).toMatchObject({ open: "view", sub: "layout", row: "layout-min" });
      await page.keyboard.press("Escape");
      expect(await editorFocused(page)).toBe(true);
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ 键盘: 执行
    test("Enter 执行高亮行, 关菜单并把焦点还给编辑区 (键盘打开)", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      expect(await page.evaluate("VIEW_STATE.lineNumbers")).toBe(true);
      await page.keyboard.press("Alt+v");
      expect(await downTo(page, "toggle-line-num")).toBe(true);
      await page.keyboard.press("Enter"); await page.evaluate(() => (window as any).__editIdle && (window as any).__editIdle());
      expect(await openMenus(page)).toEqual([]);
      expect(await kbLeft(page)).toBe(0);
      expect(await editorFocused(page)).toBe(true);
      expect(await page.evaluate("VIEW_STATE.lineNumbers")).toBe(false);       // 命令真的执行了
      await expect(page.locator("#gutter")).toBeHidden();
      await page.evaluate(() => (window as any).dispatchCommand("toggle-line-num"));
      expect(await page.evaluate("VIEW_STATE.lineNumbers")).toBe(true);
      expect(errors).toEqual([]);
    });

    test("鼠标点开菜单后用键盘 ↓ + Enter 照样执行 (不是把菜单再切一下)", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "view").click();                                         // 焦点此时在按钮上
      expect(await downTo(page, "toggle-word-wrap")).toBe(true);
      const before = await page.evaluate("VIEW_STATE.wordWrap");
      await page.keyboard.press("Enter"); await page.evaluate(() => (window as any).__editIdle && (window as any).__editIdle());
      expect(await openMenus(page)).toEqual([]);
      expect(await page.evaluate("VIEW_STATE.wordWrap")).toBe(!before);
      await page.evaluate(() => (window as any).dispatchCommand("toggle-word-wrap"));
      expect(await page.evaluate("VIEW_STATE.wordWrap")).toBe(before);
      expect(errors).toEqual([]);
    });

    test("空格和 Enter 一样执行高亮行; 空格在菜单栏按钮上不会再触发一次点击", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "view").click();
      expect(await downTo(page, "toggle-word-wrap")).toBe(true);
      const before = await page.evaluate("VIEW_STATE.wordWrap");
      await page.keyboard.press("Space");
      expect(await openMenus(page)).toEqual([]);
      expect(await page.evaluate("VIEW_STATE.wordWrap")).toBe(!before);
      await page.evaluate(() => (window as any).dispatchCommand("toggle-word-wrap"));
      // 菜单栏上空格展开 (而不是展开又被 keyup 的 click 收起)
      await page.keyboard.press("Alt");
      await page.keyboard.press("Space");
      expect(await st(page)).toMatchObject({ open: "file", row: "new-file" });
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    test("键盘打开菜单时 ✓ 标记也是新的 (视图菜单)", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.evaluate(() => (window as any).dispatchCommand("toggle-line-num"));   // 行号 -> 关
      await page.keyboard.press("Alt+v");
      await expect(page.locator('[data-cmd="toggle-line-num"]')).toHaveAttribute("data-check", "off");
      await page.keyboard.press("Escape");
      await page.evaluate(() => (window as any).dispatchCommand("toggle-line-num"));   // 行号 -> 开
      await page.keyboard.press("Alt+v");
      await expect(page.locator('[data-cmd="toggle-line-num"]')).toHaveAttribute("data-check", "on");
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ Alt
    test("Alt+助记字母 (与菜单栏下划线字母一致) 打开对应菜单; 菜单开着时直接换菜单", async ({ page }) => {
      const errors = await boot(page);
      // 菜单栏上标的下划线字母 = 助记键表
      const keys = await page.evaluate(() => Array.from(document.querySelectorAll("#menubar .menu-item")).map((it) =>
        [it.querySelector(".mn-key")!.textContent!.toLowerCase(), (it as HTMLElement).dataset.menu]));
      expect(Object.fromEntries(keys)).toEqual(MNEMONIC);
      await focusEditor(page);
      for (const [k, menu] of Object.entries(MNEMONIC)) {
        await page.keyboard.press(`Alt+${k}`);
        expect(await st(page), `Alt+${k}`).toMatchObject({ open: menu, bar: menu });   // 菜单开着, 焦点在它的按钮上
        expect(await openMenus(page)).toEqual([menu]);                                 // 同时只有一个
        expect((await st(page)).row, `Alt+${k} 第一行高亮`).toBe((await topRows(page, menu))[0]);
      }
      await page.keyboard.press("Alt+e");                                              // 帮助开着时 Alt+E 直接换到编辑
      expect((await st(page)).open).toBe("edit");
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    test("单按 Alt: 焦点交给菜单栏 (文件), ←→ 移动不展开, ↓ 展开; 再按 Alt / Esc 退出回编辑区", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt");
      expect(await st(page)).toMatchObject({ bar: "file", open: null });
      await page.keyboard.press("ArrowRight");
      expect(await st(page)).toMatchObject({ bar: "edit", open: null });
      await page.keyboard.press("ArrowLeft");
      await page.keyboard.press("ArrowLeft");
      expect(await st(page)).toMatchObject({ bar: "help", open: null });          // 菜单栏上也循环
      await page.keyboard.press("ArrowDown");
      expect(await st(page)).toMatchObject({ bar: "help", open: "help", row: "help" });
      await page.keyboard.press("ArrowLeft");                                      // 展开后 ← 仍然是打开并切换
      expect(await st(page)).toMatchObject({ open: "window", row: "split-h" });
      await page.keyboard.press("Escape");
      expect(await st(page)).toMatchObject({ bar: null, open: null });
      expect(await editorFocused(page)).toBe(true);
      // 再按一次 Alt 退出
      await page.keyboard.press("Alt");
      expect((await st(page)).bar).toBe("file");
      await page.keyboard.press("Alt");
      expect((await st(page)).bar).toBeNull();
      expect(await editorFocused(page)).toBe(true);
      // 菜单开着时单按 Alt = 关菜单
      await page.keyboard.press("Alt+e");
      expect((await st(page)).open).toBe("edit");
      await page.keyboard.press("Alt");
      expect(await st(page)).toMatchObject({ open: null, bar: null });
      expect(await editorFocused(page)).toBe(true);
      // 菜单栏上直接按助记字母 (不带 Alt) 开对应菜单
      await page.keyboard.press("Alt");
      await page.keyboard.press("v");
      expect(await st(page)).toMatchObject({ open: "view", row: "layout" });
      await page.keyboard.press("Escape");
      expect(errors).toEqual([]);
    });

    test("Alt 被别的操作用掉就不算单按: Alt+字母后松开 Alt 菜单仍开着; Alt+点击不会聚焦菜单栏", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt+f");                                          // 按下 Alt, F, 松开 F, 松开 Alt
      expect(await st(page)).toMatchObject({ open: "file" });                       // 松 Alt 没有把菜单切掉
      await page.keyboard.press("Escape");
      await focusEditor(page);
      await page.keyboard.down("Alt");                                              // Alt+拖选 (列块选择) 一类: 按住 Alt 点编辑区
      await page.mouse.click(640, 420);
      await page.keyboard.up("Alt");
      expect(await st(page)).toMatchObject({ bar: null, open: null, armed: false });
      await page.keyboard.down("Alt");                                              // Alt+Tab 一类: 中间夹了别的键
      await page.keyboard.press("Tab");
      await page.keyboard.up("Alt");
      expect((await st(page)).open).toBeNull();
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ 不漏进编辑区
    test("菜单开着时的方向键 / Home / End / Enter 不会漏进编辑区 (文本、光标、模式都不变)", async ({ page }) => {
      const errors = await boot(page);
      await prepDoc(page, layout);
      await focusEditor(page);
      const before = await docState(page);
      await page.keyboard.press("Alt+f");
      for (const key of ["ArrowDown", "ArrowDown", "ArrowUp", "ArrowRight", "ArrowLeft", "Home", "End", "ArrowLeft", "ArrowRight", "ArrowUp"]) await page.keyboard.press(key);
      await page.keyboard.press("Escape");
      expect(await docState(page)).toBe(before);
      // Alt 单按 + 方向键 / Enter 同理
      await page.keyboard.press("Alt");
      for (const key of ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp", "Escape"]) await page.keyboard.press(key);
      expect(await docState(page)).toBe(before);
      // 执行一个不改文本的命令后, 焦点回到编辑区, 继续能在原处工作
      await page.keyboard.press("Alt+v");
      expect(await downTo(page, "toggle-word-wrap")).toBe(true);
      await page.keyboard.press("Enter"); await page.evaluate(() => (window as any).__editIdle && (window as any).__editIdle());
      await page.evaluate(() => (window as any).dispatchCommand("toggle-word-wrap"));
      expect(await editorFocused(page)).toBe(true);
      expect(await docState(page)).toBe(before);
      expect(errors).toEqual([]);
    });

    test("Alt+助记字母只开菜单, 编辑区不会同时把它当输入 / vim 命令 (INSERT 里不打出字母, NORMAL 里不进 VISUAL)", async ({ page }) => {
      const errors = await boot(page);
      await prepDoc(page, layout);
      await focusEditor(page);
      if (layout === "full") await page.keyboard.press("i");                       // INSERT: 以前 Alt+F 会同时打出 f
      const insertState = await docState(page);
      for (const k of Object.keys(MNEMONIC)) {
        await page.keyboard.press(`Alt+${k}`);
        expect((await st(page)).open).toBe(MNEMONIC[k]);
        await page.keyboard.press("Escape");
      }
      expect(await docState(page)).toBe(insertState);
      if (layout === "full") {
        await page.keyboard.press("Escape");                                       // NORMAL: 以前 Alt+V 会同时进 VISUAL
        const normalState = await docState(page);
        expect(JSON.parse(normalState).mode).toBe("NORMAL");
        for (const k of Object.keys(MNEMONIC)) { await page.keyboard.press(`Alt+${k}`); await page.keyboard.press("Escape"); }
        expect(await docState(page)).toBe(normalState);
      }
      expect(errors).toEqual([]);
    });

    test("关闭菜单后不残留键盘高亮 / 展开的子菜单; aria-expanded 跟随", async ({ page }) => {
      const errors = await boot(page);
      await focusEditor(page);
      await page.keyboard.press("Alt+v");
      await page.keyboard.press("ArrowRight");                                      // 进子菜单
      expect(await kbLeft(page)).toBeGreaterThan(0);
      await expect(btn(page, "view")).toHaveAttribute("aria-expanded", "true");
      await page.mouse.click(640, 420);                                             // 点外面收起
      expect(await kbLeft(page)).toBe(0);
      for (const m of MENUS) await expect(btn(page, m)).toHaveAttribute("aria-expanded", "false");
      for (const m of MENUS) await expect(btn(page, m)).toHaveAttribute("aria-haspopup", "menu");
      // 鼠标点开、键盘走一行、再换成鼠标点另一个菜单: 旧高亮不跟过去
      await btn(page, "edit").click();
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("ArrowDown");
      await btn(page, "search").click();
      expect(await st(page)).toMatchObject({ open: "search", row: null });
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ 焦点: 命令执行后 / 菜单收起后
    test("鼠标点菜单命令后焦点回到编辑区 (和键盘 Enter 一致), 接着就能打字", async ({ page }) => {
      const errors = await boot(page);
      await prepDoc(page, layout);
      for (const [menu, cmd] of [["view", "toggle-word-wrap"], ["view", "toggle-line-num"], ["edit", "select-all"], ["edit", "undo"]]) {
        await btn(page, menu).click();
        await page.locator(`.menu-dropdown[data-dropdown="${menu}"] > [data-cmd="${cmd}"]`).click();
        expect(await openMenus(page), `${cmd} 点完菜单收起`).toEqual([]);
        expect(await editorFocused(page), `${cmd} 点完焦点回到编辑区 (不是 body)`).toBe(true);
      }
      if (layout === "full") await page.keyboard.press("i");               // vim 开: 先进 INSERT 才能打字
      await page.keyboard.type("Z");
      expect(await page.evaluate(() => (window as any).getActiveTab().lines.join("|"))).toContain("Z");
      expect(errors).toEqual([]);
    });

    test("命令自己把焦点放进输入框 (搜索 > 查找) 时不被抢回编辑区; 再点标题收起菜单后焦点在编辑区, 接着打的字母不会去开菜单", async ({ page }) => {
      const errors = await boot(page);
      await prepDoc(page, layout);
      await btn(page, "search").click();
      await page.locator('.menu-dropdown[data-dropdown="search"] > [data-cmd="find"]').click();
      await expect(page.locator("#find-input")).toBeFocused();
      await page.keyboard.press("Escape");
      // 再点标题收起
      await focusEditor(page);
      await btn(page, "file").click();
      await btn(page, "file").click();
      expect(await openMenus(page)).toEqual([]);
      expect(await editorFocused(page), "收起后焦点在编辑区, 不留在菜单栏按钮上").toBe(true);
      await page.keyboard.type("hex");                                          // 以前: 焦点在按钮上, h 会打开「帮助」菜单
      expect(await st(page)).toMatchObject({ open: null, bar: null });
      expect(errors).toEqual([]);
    });

    test("键盘 Enter 执行会弹对话框的命令 (帮助 > wiki): 焦点不进编辑区, 按键不会打进后面的文档; Esc 关掉对话框后焦点回编辑区", async ({ page }) => {
      const errors = await boot(page);
      await prepDoc(page, layout);
      await focusEditor(page);
      await page.keyboard.press("Alt+h");
      expect(await downTo(page, "wiki")).toBe(true);
      const before = await docState(page);
      await page.keyboard.press("Enter"); await page.evaluate(() => (window as any).__editIdle && (window as any).__editIdle());
      await expect(page.locator("#modal")).toHaveClass(/show/);
      expect(await editorFocused(page), "对话框开着, 焦点不能在编辑区").toBe(false);
      await page.keyboard.type("qqq");
      expect(await docState(page), "对话框开着时打的字不进文档").toBe(before);
      expect(await st(page)).toMatchObject({ open: null, bar: null });       // 也没有残留菜单模式
      await page.keyboard.press("Escape");
      await expect(page.locator("#modal")).not.toHaveClass(/show/);
      expect(await editorFocused(page)).toBe(true);
      expect(errors).toEqual([]);
    });

    // ------------------------------------------------------------ 鼠标静止 + 键盘: 一个高亮 / 一层飞出层
    /** 打开的菜单里真正画着高亮底色的行 (悬停 + 键盘高亮都算), 取 data-cmd / data-submenu. */
    const litRows = (page: Page) => page.evaluate(() =>
      Array.from(document.querySelectorAll<HTMLElement>(".menu-item.open .menu-row"))
        .filter((r) => { const bg = getComputedStyle(r).backgroundColor; return r.getClientRects().length > 0 && bg !== "rgba(0, 0, 0, 0)" && bg !== "transparent"; })
        .map((r) => r.dataset.cmd || r.dataset.submenu || ""));
    const visibleFlyouts = (page: Page) => page.evaluate(() =>
      Array.from(document.querySelectorAll<HTMLElement>(".menu-item.open .submenu-dropdown"))
        .filter((d) => d.getClientRects().length > 0).map((d) => d.parentElement!.dataset.submenu));

    test("鼠标停在某行上用键盘走: 只有键盘那一行亮 (停着的鼠标行不亮); 鼠标真的动了再把高亮还给它", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "edit").click();
      const cut = page.locator('.menu-dropdown[data-dropdown="edit"] > [data-cmd="cut"]');
      await cut.hover();
      await expect.poll(() => litRows(page)).toEqual(["cut"]);
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("ArrowDown");
      expect((await st(page)).row).toBe("paste");
      await expect(page.locator("#menubar")).toHaveClass(/kbnav/);
      await expect.poll(() => litRows(page), "键盘接管后只剩 paste 亮, 鼠标停着的 cut 不亮").toEqual(["paste"]);
      const b = (await cut.boundingBox())!;
      await page.mouse.move(b.x + 20, b.y + 4);                                // 在 cut 行里真的挪动一下
      await expect.poll(() => litRows(page)).toEqual(["cut"]);
      expect((await st(page)).row).toBe("cut");
      await expect(page.locator("#menubar")).not.toHaveClass(/kbnav/);
      await page.keyboard.press("Escape");
      await expect(page.locator("#menubar")).not.toHaveClass(/kbnav/);        // 收起菜单也清掉
      expect(errors).toEqual([]);
    });

    test("子菜单: 鼠标悬停着飞出层再 ↓ → 只剩键盘展开的那一层; 键盘行在飞出层里时鼠标走开, 飞出层不收、↑↓ 照走", async ({ page }) => {
      const errors = await boot(page);
      await btn(page, "view").click();
      await page.locator('#menubar [data-submenu="sidebar"]').first().hover();
      expect(await visibleFlyouts(page)).toEqual(["sidebar"]);
      await page.keyboard.press("ArrowDown");                                   // 高亮从停着鼠标的「侧边栏」行接着往下
      await page.keyboard.press("ArrowRight");                                  // 进下一个子菜单父行的飞出层
      expect(await visibleFlyouts(page), "不会同时开两层飞出层").toHaveLength(1);
      await page.keyboard.press("Escape");
      // 鼠标进了飞出层里的行, 键盘接手后鼠标走开
      await btn(page, "view").click();
      await page.locator('#menubar [data-submenu="sidebar"]').first().hover();
      await page.locator('#menubar [data-submenu="sidebar"] > .submenu-dropdown > .menu-row').first().hover();
      await page.keyboard.press("ArrowDown");
      const r1 = (await st(page)).row;
      await page.mouse.move(5, 500, { steps: 1 });
      expect(await visibleFlyouts(page), "鼠标走开后键盘所在的飞出层还在").toEqual(["sidebar"]);
      await page.keyboard.press("ArrowDown");
      const r2 = (await st(page)).row;
      expect(r2, "↓ 照样能走").not.toBe(r1);
      await page.keyboard.press("ArrowUp");
      expect((await st(page)).row).toBe(r1);
      expect(errors).toEqual([]);
    });

    test("菜单之间直接切换 (悬停 / ←→) 没有淡入动画, 第一次从收起状态打开仍有", async ({ page }) => {
      const errors = await boot(page);
      const anim = (menu: string) => page.evaluate((m) => getComputedStyle(document.querySelector(`.menu-item[data-menu="${m}"] > .menu-dropdown`)!).animationName, menu);
      await btn(page, "file").click();
      expect(await anim("file"), "第一次打开: 有淡入").toBe("menu-in");
      await btn(page, "edit").hover();
      expect(await openMenus(page)).toEqual(["edit"]);
      expect(await anim("edit"), "悬停切换: 瞬间出现").toBe("none");
      await page.keyboard.press("ArrowRight");
      expect(await openMenus(page)).toEqual(["search"]);
      expect(await anim("search"), "→ 切换: 瞬间出现").toBe("none");
      await page.keyboard.press("Escape");
      await parkMouse(page);
      await btn(page, "view").click();
      expect(await anim("view"), "收起后重新打开: 又有淡入").toBe("menu-in");
      expect(errors).toEqual([]);
    });
  });
}
