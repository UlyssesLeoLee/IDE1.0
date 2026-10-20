// tests/uat/specs/column-block.spec.ts
// 列块选区 / 纵向批量编辑 (对标 Sakura Editor Shift+F6, 见 docs/SPA-01 §2)
//
// 覆盖 SPA-01 的 C-01 ~ C-10:
//   C-01 Shift+F6 进 / 出列块模式          C-06 短行补空格保持列对齐
//   C-02 方向键扩展行列                  C-07 列块复制 (Ctrl+C)
//   C-03 列块删除 (Delete)               C-08 列块与普通选区互斥
//   C-04 直接输入 = 每行插入同一内容      C-09 Esc 退出列块
//   C-05 多行内容按行分配
//
// 运行: npx playwright test specs/column-block.spec.ts
import { test, expect, type Page } from "@playwright/test";

// 真正的首次启动: 不带任何 localStorage (极简布局 + 非模态)
test.use({ storageState: { cookies: [], origins: [] } });

const TOKEN = process.env.IDE_SHELL_WEB_TOKEN || "";

/** 打开编辑器并等它 ready */
async function boot(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/editor");
  await page.waitForFunction(() => (window as any).__ideReady?.(), null, { timeout: 15_000 });
  await page.waitForFunction(() => document.activeElement?.id === "ed-content");
  return errors;
}

/** 直接设置文档内容 (绕开逐字输入的噪音), 返回 true */
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

/** 读回文档内容 */
async function doc(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as any).getActiveTab().lines);
}

/** 列块状态快照 */
async function colSel(page: Page) {
  return page.evaluate(() => {
    const cs = (window as any).getActiveTab().colSel;
    if (!cs) return null;
    const r = (window as any).colSelRect((window as any).getActiveTab());
    return { ...cs, norm: r };
  });
}

test.describe("列块选区 / 纵向批量编辑", () => {
  test("C-08 状态栏显示列块模式 (进/出与尺寸变化)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdef", "abcdef", "abcdef"]);
    await page.locator("#ed-content").focus();

    const ind = page.locator("#s-colsel");
    expect(await ind.isHidden(), "未进列块时状态栏不显示列块项").toBe(true);

    await page.keyboard.press("Shift+F6");
    await expect(ind).toBeVisible();
    expect(await ind.textContent()).toContain("列块");

    // 扩到 3 行 × 2 列, 文案应随之更新
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(ind).toContainText("3行");
    await expect(ind).toContainText("2列");

    // 退出后隐藏
    await page.keyboard.press("Shift+F6");
    await expect(ind).toBeHidden();
    expect(errors).toEqual([]);
  });

  test("C-08 列块与普通选区互斥", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdef", "abcdef"]);
    await page.locator("#ed-content").focus();

    // 先有普通选区
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 0, r1: 0, c1: 3 };
      (window as any).renderVim(t);
    });
    expect(await page.evaluate(() => !!(window as any).getActiveTab().sel)).toBe(true);

    // 进列块应清掉普通选区 (两者不能同时存在, 否则渲染会打架)
    await page.evaluate(() => (window as any).startColumnSelect(0, 0));
    expect(await page.evaluate(() => (window as any).getActiveTab().sel), "列块应清掉普通选区").toBeNull();
    expect(await page.evaluate(() => !!(window as any).getActiveTab().colSel)).toBe(true);
    expect(errors).toEqual([]);
  });

  test("C-01 Shift+F6 进入列块模式, 再按退出", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abc", "def", "ghi"]);
    await page.locator("#ed-content").focus();

    const before = await colSel(page);
    expect(before, "初始没有列块").toBeNull();

    // 进列块模式
    await page.keyboard.press("Shift+F6");
    await page.waitForFunction(() => !!(window as any).getActiveTab().colSel);
    const on = await colSel(page);
    expect(on, "Shift+F6 后应有列块").not.toBeNull();
    expect(on!.norm.r0, "锚点在光标行").toBe(0);
    expect(on!.norm.c0, "锚点在光标列").toBe(0);

    // 再按退出
    await page.keyboard.press("Shift+F6");
    expect(await colSel(page), "再按 Shift+F6 应退出").toBeNull();
    expect(errors).toEqual([]);
  });

  test("C-02 方向键扩展列块 (上下扩行 / 左右扩列)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdef", "abcdef", "abcdef", "abcdef"]);
    await page.locator("#ed-content").focus();
    await page.keyboard.press("Shift+F6");

    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    let s = await colSel(page);
    expect(s!.norm.r0, "下移两次后行范围到第 3 行").toBe(0);
    expect(s!.norm.r1).toBe(2);
    expect(s!.norm.c0, "上下键不该改变列跨度").toBe(0);
    expect(s!.norm.c1).toBe(0);

    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    s = await colSel(page);
    expect(s!.norm.c1, "右移两次后列范围宽 2").toBe(2);
    expect(s!.norm.r1, "左右键不该改变行范围").toBe(2);

    expect(errors).toEqual([]);
  });

  test("C-04 直接输入 = 在每一行同一列插入同一内容", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["aaa", "bbb", "ccc"]);
    await page.locator("#ed-content").focus();

    // 在第 0 行第 1 列建列块, 扩到 3 行
    await page.evaluate(() => (window as any).startColumnSelect(0, 1));
    await page.evaluate(() => (window as any).extendColumnSelect(2, 1));

    await page.keyboard.press("X");
    // 每行第 1 列前都应插入 X: aXaa / bXbb / cXcc
    expect(await doc(page)).toEqual(["aXaa", "bXbb", "cXcc"]);
    expect(errors).toEqual([]);
  });

  test("C-04 连续输入累积 (插入点右移, 可接着打)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["ab", "cd"]);
    await page.locator("#ed-content").focus();
    await page.evaluate(() => (window as any).startColumnSelect(0, 1));
    await page.evaluate(() => (window as any).extendColumnSelect(1, 1));

    await page.keyboard.press("1");
    await page.keyboard.press("2");
    expect(await doc(page)).toEqual(["a12b", "c12d"]);
    expect(errors).toEqual([]);
  });

  test("C-05 多行内容按行分配给列块各行", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["1.", "2.", "3.", "4."]);
    await page.locator("#ed-content").focus();
    await page.evaluate(() => (window as any).startColumnSelect(0, 2));
    await page.evaluate(() => (window as any).extendColumnSelect(3, 2));

    // 直接调 columnInsert 传多行内容 (粘贴多行的等价路径)
    await page.evaluate(() => (window as any).columnInsert("a\nbb\nccc\ndddd"));
    expect(await doc(page)).toEqual(["1.a", "2.bb", "3.ccc", "4.dddd"]);
    expect(errors).toEqual([]);
  });

  test("C-06 短行补空格保持列对齐 (不丢已有内容)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["long line", "ab", "also long"]);
    await page.locator("#ed-content").focus();
    // 第 5 列 (0-based) 建块, 覆盖 3 行; 中间那行只有 2 字符 -> 必须补空格
    await page.evaluate(() => (window as any).startColumnSelect(0, 5));
    await page.evaluate(() => (window as any).extendColumnSelect(2, 5));

    await page.keyboard.press("#");
    const out = await doc(page);
    // 插入点是第 5 列 (即第 6 个字符位) 之前: "long" 占 0-3, 空格占 4, 插入点在 5
    expect(out[0], "长行: 在第 5 列处插入 #").toBe("long #line");
    expect(out[1], "短行: 补空格到第 5 列再插入, 不丢 'ab'").toBe("ab   #");
    expect(out[2], "长行: 在第 5 列处插入 #").toBe("also #long");
    // 三行插入点必须落在同一列 (这才是列块的意义)
    expect(out.map((s) => s.indexOf("#"))).toEqual([5, 5, 5]);
    expect(errors).toEqual([]);
  });

  test("C-03 Delete 删除整块", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdef", "abcdef"]);
    await page.locator("#ed-content").focus();
    await page.evaluate(() => (window as any).startColumnSelect(0, 1));
    await page.evaluate(() => (window as any).extendColumnSelect(1, 3));

    await page.keyboard.press("Delete");
    expect(await doc(page)).toEqual(["adef", "adef"]);
    expect(errors).toEqual([]);
  });

  test("C-07 列块复制 = 按行取该列区间", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abc", "def", "ghi"]);
    await page.locator("#ed-content").focus();
    await page.evaluate(() => (window as any).startColumnSelect(0, 1));
    await page.evaluate(() => (window as any).extendColumnSelect(2, 2));

    await page.keyboard.press("Control+c");
    // 3 行 × 第 1 列 (0-based) = b / e / h
    const clip = await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      return (window as any).columnSelectedText(t);
    });
    expect(clip).toBe("b\ne\nh");
    expect(errors).toEqual([]);
  });

  test("C-09 Esc 退出列块模式", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abc", "def"]);
    await page.locator("#ed-content").focus();
    await page.keyboard.press("Shift+F6");
    expect(await colSel(page), "先确认列块已开").not.toBeNull();

    await page.keyboard.press("Escape");
    expect(await colSel(page), "Esc 应退出列块").toBeNull();
    expect(errors).toEqual([]);
  });

  test("列块内容不被语法高亮标签劈坏 (渲染完整性)", async ({ page }) => {
    const errors = await boot(page);
    // 一段带语法结构的代码, 列块落在 token 中间
    await setDoc(page, ['fn alpha() { let x = 1; }', 'fn beta() { let y = 2; }']);
    await page.locator("#ed-content").focus();
    await page.evaluate(() => (window as any).startColumnSelect(0, 4));
    await page.evaluate(() => (window as any).extendColumnSelect(1, 12));

    // 渲染后: 不应出现未闭合 / 错位的 span, 文本内容应与源一致
    const check = await page.evaluate(() => {
      const rows = [...document.querySelectorAll("#ed-content .hl-row")];
      return {
        n: rows.length,
        // 每个 hl-row 的 textContent 应等于源行
        texts: rows.map((r) => r.textContent),
        // 选区高亮确实落在 DOM 里
        sel: document.querySelectorAll("#ed-content .col-sel-active").length,
      };
    });
    expect(check.n).toBe(2);
    expect(check.texts[0]).toBe('fn alpha() { let x = 1; }');
    expect(check.texts[1]).toBe('fn beta() { let y = 2; }');
    expect(check.sel, "两行都应有列块高亮 span").toBe(2);
    expect(errors).toEqual([]);
  });

  test("Unicode 安全: 列块不劈开代理对字符", async ({ page }) => {
    const errors = await boot(page);
    // 每行第 0 列是 emoji (UTF-16 代理对), 第 1 列是 ASCII
    await setDoc(page, ["😀a", "😀b", "😀c"]);
    await page.locator("#ed-content").focus();
    await page.evaluate(() => (window as any).startColumnSelect(0, 1));
    await page.evaluate(() => (window as any).extendColumnSelect(2, 1));   // 宽度 0 的列块 = 纯插入点 (有宽度时输入会覆盖选中内容)

    await page.keyboard.press("Z");
    // emoji 必须完好 (没被劈成半个代理), 且 Z 插在 emoji 之后
    expect(await doc(page)).toEqual(["😀Za", "😀Zb", "😀Zc"]);
    expect(errors).toEqual([]);
  });
});
