// 真实鼠标: 点击落光标 / 拖选 / 跨行选区高亮 / Alt 拖出纵向(矩形)选区 / Alt+方向键.
// 以前这些在「普通打字」状态 (整篇一个文本节点) 下完全无效, 有 .hl-row 时列号还整体偏 4 列;
// 而且 column-block 等用例只用 evaluate 造状态, 测不到这条真实输入路径.
import { test, expect, type Page } from "@playwright/test";

async function boot(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/editor");
  await page.waitForFunction(() => (window as any).__ideReady?.(), null, { timeout: 15_000 });
  await page.waitForFunction(() => document.activeElement?.id === "ed-content");
  await page.locator("#ed-content").focus();
  for (const l of ["abcdefghij", "klmnopqrst", "uvwxyzABCD", "EFGHIJKLMN"]) {
    await page.keyboard.type(l);
    await page.keyboard.press("Enter");
  }
  return errors;
}

/** 以第一个字符的真实矩形为原点, 返回 (行, 列) -> 屏幕坐标 */
async function grid(page: Page) {
  const m = await page.evaluate(() => {
    const ed = document.getElementById("ed-content")!;
    const w = document.createTreeWalker(ed, NodeFilter.SHOW_TEXT);
    const n = w.nextNode()!;
    const a = document.createRange(); a.setStart(n, 0); a.setEnd(n, 1);
    const b = document.createRange(); b.setStart(n, 0); b.setEnd(n, 5);
    const ar = a.getBoundingClientRect(), br = b.getBoundingClientRect();
    const lh = parseFloat(getComputedStyle(ed).lineHeight) || 18;
    return { left: ar.left, top: ar.top + ar.height / 2 - lh / 2, lh, cw: (br.right - ar.left) / 5 };
  });
  return { x: (c: number) => m.left + c * m.cw, y: (r: number) => m.top + r * m.lh + m.lh / 2 };
}

const state = (page: Page) => page.evaluate(() => {
  const t = (window as any).getActiveTab();
  const c = t.colSel;
  return { cur: [t.cursor.row, t.cursor.col], sel: t.sel && [t.sel.r0, t.sel.c0, t.sel.r1, t.sel.c1], box: c && [c.r0, c.c0, c.r1, c.c1] };
});

test.describe("真实鼠标: 点击 / 拖选 / Alt 纵向选择", () => {
  // 真正的首次启动态 (极简布局 + 非模态), 不注入 playwright.config 里的 full + vim=on
  test.use({ storageState: { cookies: [], origins: [] } });

  test("点击把光标落在点击处 (普通打字状态)", async ({ page }) => {
    const errors = await boot(page);
    const g = await grid(page);
    await page.mouse.click(g.x(3), g.y(1));
    expect((await state(page)).cur).toEqual([1, 3]);
    expect(errors).toEqual([]);
  });

  test("行内拖选 / 跨行拖选, 跨行高亮逐行正确", async ({ page }) => {
    await boot(page);
    const g = await grid(page);
    await page.mouse.move(g.x(2), g.y(0)); await page.mouse.down(); await page.mouse.move(g.x(6), g.y(0), { steps: 4 }); await page.mouse.up();
    expect((await state(page)).sel).toEqual([0, 2, 0, 6]);
    await page.mouse.click(g.x(0), g.y(3));
    const g2 = await grid(page);
    await page.mouse.move(g2.x(5), g2.y(0)); await page.mouse.down(); await page.mouse.move(g2.x(3), g2.y(2), { steps: 6 }); await page.mouse.up();
    expect((await state(page)).sel).toEqual([0, 5, 2, 3]);
    const painted = await page.evaluate(() => [...document.querySelectorAll("#ed-content .hl-row")].slice(0, 3).map((r) => r.querySelector(".sel-active")?.textContent ?? null));
    expect(painted).toEqual(["fghij", "klmnopqrst", "uvw"]);
  });

  test("Alt+拖动 = 纵向(矩形)选区, 直接输入在每行同列插入", async ({ page }) => {
    const errors = await boot(page);
    const g = await grid(page);
    await page.keyboard.down("Alt");
    await page.mouse.move(g.x(2), g.y(0)); await page.mouse.down();
    await page.mouse.move(g.x(4), g.y(1), { steps: 4 }); await page.mouse.move(g.x(6), g.y(2), { steps: 4 });
    await page.mouse.up(); await page.keyboard.up("Alt");
    expect((await state(page)).box).toEqual([0, 2, 2, 6]);
    await expect(page.locator("#s-colsel")).toContainText("3行");
    await page.keyboard.type("X");
    const lines = await page.evaluate(() => (window as any).getActiveTab().lines.slice(0, 3));
    // Alt 拖出的 3行×4列 里原有的内容被覆盖掉 (cdef / mnop / wxyz), 不是插在它前面
    expect(lines).toEqual(["abXghij", "klXqrst", "uvXABCD"]);
    expect(errors).toEqual([]);
  });

  test("Alt+拖动可以拖到行尾右侧 (虚拟列)", async ({ page }) => {
    await boot(page);
    const g = await grid(page);
    await page.keyboard.down("Alt");
    await page.mouse.move(g.x(8), g.y(0)); await page.mouse.down(); await page.mouse.move(g.x(14), g.y(2), { steps: 6 });
    await page.mouse.up(); await page.keyboard.up("Alt");
    expect((await state(page)).box).toEqual([0, 8, 2, 14]);
  });

  test("Alt+方向键 从点击位置建立纵向选区", async ({ page }) => {
    await boot(page);
    const g = await grid(page);
    await page.mouse.click(g.x(1), g.y(0));
    await page.keyboard.down("Alt");
    for (const k of ["ArrowDown", "ArrowDown", "ArrowRight", "ArrowRight"]) await page.keyboard.press(k);
    await page.keyboard.up("Alt");
    expect((await state(page)).box).toEqual([0, 1, 2, 3]);
  });
});
