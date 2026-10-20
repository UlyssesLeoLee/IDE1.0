// tests/uat/specs/cursor-movement.spec.ts
// Sakura 光标移动命令集 (docs/SPA-02 §3.1, 对标 HLP000220~HLP000263)
//
// 覆盖 M-01 ~ M-11:
//   M-01 単語の左端 / M-02 単語の右端      M-07 整页上下 / M-08 半页上下
//   M-03 行頭 / M-04 行末                 M-09 文件首 / 尾
//   M-05 上下 2 行                         M-10 前后段落
//   M-11 有选区时收缩选区 (Sakura 的统一规则)
//
// 全部在 char (码点) 语义下验证 —— 有一个用例专门用 emoji 确认不劈开代理对.
import { test, expect, type Page } from "@playwright/test";

test.use({ storageState: { cookies: [], origins: [] } });

async function boot(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/editor");
  await page.waitForFunction(() => (window as any).__ideReady?.(), null, { timeout: 15_000 });
  await page.waitForFunction(() => document.activeElement?.id === "ed-content");
  return errors;
}

async function setDoc(page: Page, lines: string[]) {
  return page.evaluate((ls) => {
    const t = (window as any).getActiveTab();
    t.lines = ls;
    t.cursor = { row: 0, col: 0 };
    t.dirty = true;
    (window as any).renderVim(t);
    return true;
  }, lines);
}

/** 当前光标位置 (0-based) */
async function cur(page: Page) {
  return page.evaluate(() => {
    const c = (window as any).getActiveTab().cursor;
    return { row: c.row, col: c.col };
  });
}

/** 用 dispatchCommand 跑一条移动命令 (比模拟按键更贴近真实调用路径) */
async function move(page: Page, cmd: string) {
  await page.evaluate((c: string) => (window as any).dispatchCommand(c), cmd);
}

test.describe("Sakura 光标移动命令集", () => {
  test("M-03/M-04 行首 / 行末", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["hello world", "second line"]);
    await page.locator("#ed-content").focus();

    await move(page, "cursor-line-end");
    expect(await cur(page)).toEqual({ row: 0, col: 11 });

    await move(page, "cursor-line-start");
    expect(await cur(page)).toEqual({ row: 0, col: 0 });

    // 夹取: 光标已在末尾时再走行末不应越界
    await move(page, "cursor-line-end");
    await move(page, "cursor-line-end");
    expect(await cur(page)).toEqual({ row: 0, col: 11 });
    expect(errors).toEqual([]);
  });

  test("M-01/M-02 单词左 / 右 (Sakura: 词内移到词尾, 词尾再按才进下一个词)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["foo bar baz"]);
    await page.locator("#ed-content").focus();

    // Sakura 语义 (HLP000223 提示: "I have a pen." 光标在 a 前, 「単語の左端」到 pen 之前):
    // 光标在词首时右移 = 走到**当前词的词尾**, 不是直接跳到下一个词.
    await move(page, "cursor-word-right");
    expect(await cur(page), "0 -> 'foo' 的词尾 3").toEqual({ row: 0, col: 3 });
    await move(page, "cursor-word-right");
    expect(await cur(page), "已在词尾, 再按 -> 'bar' 词首 4").toEqual({ row: 0, col: 4 });
    await move(page, "cursor-word-right");
    expect(await cur(page), "'bar' 词尾 7").toEqual({ row: 0, col: 7 });

    // 左移: 从词尾回退到该词词首
    await move(page, "cursor-word-left");
    expect(await cur(page)).toEqual({ row: 0, col: 4 });
    expect(errors).toEqual([]);
  });

  test("M-01/M-02 单词移动跨行 (行末右移换到下一行行首)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["alpha", "beta"]);
    await page.locator("#ed-content").focus();
    await move(page, "cursor-line-end");       // 行 0 col 5
    await move(page, "cursor-word-right");     // 越过行尾 -> 行 1 col 0
    expect(await cur(page)).toEqual({ row: 1, col: 0 });
    expect(errors).toEqual([]);
  });

  test("M-05/M-06 上下 2 行 (Sakura 的「２行ごと」)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["l0", "l1", "l2", "l3", "l4", "l5"]);
    await page.locator("#ed-content").focus();

    await move(page, "cursor-down-2");
    expect(await cur(page)).toEqual({ row: 2, col: 0 });
    await move(page, "cursor-up-2");
    expect(await cur(page)).toEqual({ row: 0, col: 0 });

    // 夹取: 顶部再上移应停在 0
    await move(page, "cursor-up-2");
    expect(await cur(page)).toEqual({ row: 0, col: 0 });
    expect(errors).toEqual([]);
  });

  test("M-09 文件首 / 尾", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["a", "b", "c", "last line"]);
    await page.locator("#ed-content").focus();

    await move(page, "cursor-doc-end");
    expect(await cur(page)).toEqual({ row: 3, col: 9 });

    await move(page, "cursor-doc-start");
    expect(await cur(page)).toEqual({ row: 0, col: 0 });
    expect(errors).toEqual([]);
  });

  test("M-10 前后段落 (空行为界)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["para one line 1", "para one line 2", "", "para two line 1", "para two line 2", "", "para three"]);
    await page.locator("#ed-content").focus();

    await move(page, "cursor-para-next");     // 0 -> 3
    expect((await cur(page)).row).toBe(3);
    await move(page, "cursor-para-next");     // 3 -> 6
    expect((await cur(page)).row).toBe(6);

    await move(page, "cursor-para-prev");     // 6 -> 3
    expect((await cur(page)).row).toBe(3);
    await move(page, "cursor-para-prev");     // 3 -> 0
    expect((await cur(page)).row).toBe(0);

    // 夹取
    await move(page, "cursor-para-prev");
    expect((await cur(page)).row).toBe(0);
    expect(errors).toEqual([]);
  });

  test("M-11 有选区时收缩选区 (Sakura 统一规则)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdefghij", "second"]);
    await page.locator("#ed-content").focus();

    // 建一个选区
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 2, r1: 0, c1: 6 };
      (window as any).renderVim(t);
    });
    expect(await page.evaluate(() => !!(window as any).getActiveTab().sel)).toBe(true);

    // 向「后」的方向的命令: 收缩到终点, 光标落在终点, 选区消失
    await move(page, "cursor-doc-end");
    let c = await cur(page);
    expect(c.row, "收缩后光标在选区终点所在行").toBe(0);
    expect(c.col, "末方向收缩到选区终点 col 6").toBe(6);
    expect(await page.evaluate(() => (window as any).getActiveTab().sel), "选区应被收缩掉").toBeNull();

    // 重建选区, 用「向前」方向的命令
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 3, r1: 0, c1: 7 };
      (window as any).renderVim(t);
    });
    await move(page, "cursor-line-start");
    c = await cur(page);
    expect(c.col, "向前命令收缩到选区起点").toBe(3);
    expect(await page.evaluate(() => (window as any).getActiveTab().sel)).toBeNull();
    expect(errors).toEqual([]);
  });

  test("M-07/M-08 整页 / 半页上下 (页大小由真实高度算出)", async ({ page }) => {
    const errors = await boot(page);
    // 造一个远长于视口的文档
    const many = Array.from({ length: 300 }, (_, i) => "row " + i);
    await setDoc(page, many);
    await page.locator("#ed-content").focus();

    const rows = await page.evaluate(() => (window as any).pageRows());
    expect(rows, "一页应等于可视行数, 且 > 1").toBeGreaterThan(1);

    await move(page, "cursor-page-down");
    const afterPage = await cur(page);
    expect(afterPage.row, "下移一页").toBeGreaterThanOrEqual(rows);

    await move(page, "cursor-page-up");
    expect((await cur(page)).row).toBe(0);

    // 半页应比整页短
    await move(page, "cursor-halfpage-down");
    const half = (await cur(page)).row;
    expect(half, "半页应小于整页").toBeLessThan(rows);
    expect(half, "半页应至少 1 行").toBeGreaterThan(0);
    expect(errors).toEqual([]);
  });

  test("Unicode: 移动不劈开代理对 (emoji 列按码点计)", async ({ page }) => {
    const errors = await boot(page);
    // 每行: 😀 是代理对 (UTF-16 占 2 个码元, 但只有 1 个码点)
    await setDoc(page, ["😀😀abc", "second"]);
    await page.locator("#ed-content").focus();

    // 行末列号应按**码点**算 = 5, 不是 UTF-16 的 7
    await move(page, "cursor-line-end");
    expect(await cur(page), "行末应是码点 5 (3 emoji + 3 ascii)").toEqual({ row: 0, col: 5 });

    // 再右移应换行, 且不落在代理对中间
    await move(page, "cursor-word-right");
    const c = await cur(page);
    expect(c.row, "越过行尾换到下一行").toBe(1);
    expect(c.col).toBe(0);
    expect(errors).toEqual([]);
  });

  test("移动后状态栏 Ln/Col 同步", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["alpha", "beta", "gamma"]);
    await page.locator("#ed-content").focus();

    await move(page, "cursor-doc-end");
    const pos = await page.locator("#s-pos").textContent();
    expect(pos).toContain("Ln 3");
    expect(pos).toContain("Col 6");   // 1-based: "gamma" 长 5
    expect(errors).toEqual([]);
  });
});
