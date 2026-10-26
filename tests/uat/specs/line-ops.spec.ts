// tests/uat/specs/line-ops.spec.ts
// Sakura 增删改特色功能 (对标上游 F_* 命令, 见 docs/SPA-01 §2 扩展)
//
// 覆盖范围:
//   A. Alt 方向键 = 纯键盘矩形选取 (F_UP_BOX / F_DOWN_BOX / F_LEFT_BOX / F_RIGHT_BOX,
//      F_UP2_BOX / F_DOWN2_BOX, F_GOLINETOP_BOX / F_GOLINEEND_BOX)
//   B. 统一增删行: 剪切整行 / 剪切到行首 / 删到行首 / 删到行末 / 删到词首
//   C. 增删类特色: 删行首空白 / 删行尾空白 / 去重连续重复行 / 缩进 / 反缩进
//
// 键位全部照抄 sakura_core/func/CKeyBind.cpp 的官方默认表 (KeyDataInit[], 648-800 行).
// 语义依据 help/sakura/res/HLP000014 / 137 / 166-174 / 210-214.
//
// 运行: npx playwright test specs/line-ops.spec.ts
import { test, expect, type Page } from "@playwright/test";

// 极简布局 + 非模态 —— Sakura 用户肌肉记忆所在的组合
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
    t.sel = null;
    t.colSel = null;
    t.dirty = true;
    (window as any).renderVim(t);
  }, lines);
}

const doc = (page: Page) => page.evaluate(() => (window as any).getActiveTab().lines);
const colSelRect = (page: Page) =>
  page.evaluate(() => {
    const t = (window as any).getActiveTab();
    return t.colSel ? (window as any).colSelRect(t) : null;
  });

// ============================================================ A. Alt 方向键矩形选取

test.describe("A. Alt+方向键 = 纯键盘矩形选取", () => {
  test("Alt+↓ 直接起矩形选区 (不用先按 Shift+F6)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["aaaa", "bbbb", "cccc", "dddd"]);
    expect(await colSelRect(page), "初始没有列块").toBeNull();

    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    let r = await colSelRect(page);
    expect(r, "Alt+↓ 应直接建立列块").not.toBeNull();
    expect(r!.r1 - r!.r0, "一次按键扩 1 行 (不是 2 行)").toBe(1);

    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    r = await colSelRect(page);
    expect(r!.r1 - r!.r0, "再按一次到 3 行").toBe(2);
    expect(errors).toEqual([]);
  });

  test("Alt+↑ 收缩: 上边界下移但不会越到另一端之外", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["l1", "l2", "l3", "l4"]);
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());   // 0..3 = 4 行
    let r = await colSelRect(page);
    expect(r!.r1).toBe(3);

    await page.keyboard.press("Alt+ArrowUp"); await page.evaluate(() => (window as any).__editIdle());
    r = await colSelRect(page);
    // 活动角在 r1, Alt+↑ 把它往回拉: 矩形变窄, 但 r1 不能跑到 r0 之上
    expect(r!.r1, "Alt+↑ 收缩, r1 下移").toBe(2);
    expect(r!.r0).toBe(0);
    expect(r!.r1).toBeGreaterThanOrEqual(r!.r0, "不会塌成倒挂");

    // 一直往上按也不能把矩形压成负数
    for (let i = 0; i < 6; i++) await page.keyboard.press("Alt+ArrowUp"); await page.evaluate(() => (window as any).__editIdle());
    r = await colSelRect(page);
    expect(r!.r1, "停在 r0, 不越过另一端").toBe(0);
    expect(r!.r1 - r!.r0, "仍是合法的单行矩形").toBe(0);
    expect(errors).toEqual([]);
  });

  test("Alt+← / → 只动列边界, 不动行范围", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdef", "abcdef", "abcdef"]);
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    const before = await colSelRect(page);
    const rowsBefore = before!.r1 - before!.r0;

    await page.keyboard.press("Alt+ArrowRight"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowRight"); await page.evaluate(() => (window as any).__editIdle());
    const after = await colSelRect(page);
    expect(after!.c1 - after!.c0, "右移 2 次 = 宽 2 列").toBe(2);
    expect(after!.r1 - after!.r0, "左右键不改变行范围").toBe(rowsBefore);

    await page.keyboard.press("Alt+ArrowLeft"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowLeft"); await page.evaluate(() => (window as any).__editIdle());
    expect((await colSelRect(page))!.c1 - (await colSelRect(page))!.c0, "回到塌缩态").toBe(0);
    expect(errors).toEqual([]);
  });

  test("Ctrl+Alt+↑↓ = 矩形上下移 2 行 (F_UP2_BOX)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["l0", "l1", "l2", "l3", "l4", "l5", "l6"]);
    // Playwright 没有 "Ctrl+Alt+X" 的组合名, 要分别按下去再放开
    await page.keyboard.down("Control");
    await page.keyboard.down("Alt");
    await page.keyboard.press("ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.up("Alt");
    await page.keyboard.up("Control");
    const r = await colSelRect(page);
    expect(r!.r1 - r!.r0, "一次跨 2 行 (与 F_DOWN2_BOX 一致)").toBe(2);
    expect(errors).toEqual([]);
  });

  test("Alt+End = 列边界到行末 (F_GOLINEEND_BOX)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abc", "abcdefgh", "ab"]);
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+End"); await page.evaluate(() => (window as any).__editIdle());
    const r = await colSelRect(page);
    // 最长行是 8 字符, End 应按最长行 clamp, 不能停在短行的 2
    expect(r!.c1, "末列按参与行最长行算 (8)").toBe(8);
    expect(r!.c0).toBe(0);

    await page.keyboard.press("Alt+Home"); await page.evaluate(() => (window as any).__editIdle());
    expect((await colSelRect(page))!.c1, "Home 把列边界收回 0").toBe(0);
    expect(errors).toEqual([]);
  });

  test("Alt+方向键建好选区后直接输入 = 每行同列覆盖选区 (与鼠标拖等价)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["aaa", "bbb", "ccc"]);
    await page.keyboard.press("Alt+ArrowRight"); await page.evaluate(() => (window as any).__editIdle());   // 列边界 0 → 1
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());    // 3 行
    await page.keyboard.type("X");

    // 列块有宽度 (1 列) 时输入 = 覆盖选中内容, 落在选区**左端** c0 —— 与 Alt+拖动建立的列块完全一致
    const out = await doc(page);
    expect(out).toEqual(["Xaa", "Xbb", "Xcc"]);
    expect(out.map((s: string) => s.indexOf("X")), "X 落在三行的同一列").toEqual([0, 0, 0]);
    expect(errors).toEqual([]);
  });

  test("Alt+方向键不打开菜单助记 (不吞成 Alt+字母)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abc", "def"]);
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    expect(await page.locator(".menu-item.open").count(), "没开任何菜单").toBe(0);
    expect(await colSelRect(page), "选区确实建立了").not.toBeNull();
    // Alt+字母仍然照旧是菜单助记 (既有契约, 不能被这次改动破坏)
    await page.keyboard.press("Escape"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+f"); await page.evaluate(() => (window as any).__editIdle());
    expect(await page.locator(".menu-item.open").count(), "Alt+F 仍开文件菜单").toBe(1);
    expect(errors).toEqual([]);
  });

  test("列块状态下 Alt+方向键一次只扩一格 (不被两个监听器各扩一次)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["l0", "l1", "l2", "l3", "l4"]);
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    await page.keyboard.press("Alt+ArrowDown"); await page.evaluate(() => (window as any).__editIdle());
    const r = await colSelRect(page);
    // edContent 与 document 两处监听都会看到事件; 这里断言总跨度是 2 而不是 4
    expect(r!.r1 - r!.r0, "两次按键 = 2 行").toBe(2);
    expect(errors).toEqual([]);
  });
});

// ============================================================ B. 统一增删行

test.describe("B. 统一增删行 (Sakura F_* 语义)", () => {
  test("Ctrl+E 剪切整行 (F_CUT_LINE)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["first", "second", "third"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 1, col: 0 };
      (window as any).renderVim(t);
    });
    await page.keyboard.press("Control+e"); await page.evaluate(() => (window as any).__editIdle());
    expect(await doc(page)).toEqual(["first", "third"]);
    const clip = await page.evaluate(() => (window as any).CLIPBOARD);
    expect(clip, "剪掉的行进剪贴板").toBe("second");
    expect(errors).toEqual([]);
  });

  test("Ctrl+E 删掉最后一行后文档仍有 1 行 (不能变成 0 行)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["only"]);
    await page.keyboard.press("Control+e"); await page.evaluate(() => (window as any).__editIdle());
    expect((await doc(page)).length, "永远至少 1 行").toBe(1);
    expect(errors).toEqual([]);
  });

  test("Ctrl+U 剪切到行首 (F_LineCutToStart)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["hello world"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 6 };
      (window as any).renderVim(t);
    });
    await page.keyboard.press("Control+u"); await page.evaluate(() => (window as any).__editIdle());
    expect(await doc(page)).toEqual(["world"]);
    expect(await page.evaluate(() => (window as any).CLIPBOARD)).toBe("hello ");
    expect(await page.evaluate(() => (window as any).getActiveTab().cursor.col), "光标回行首").toBe(0);
    expect(errors).toEqual([]);
  });

  test("Ctrl+Shift+U 删到行首 (F_LineDeleteToStart, 不动剪贴板)", async ({ page }) => {
    const errors = await boot(page);
    // col 7 = "prefix-" 的正后方, 删掉它应留下 "body"
    await setDoc(page, ["prefix-body"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 7 };
      t.dirty = true;
      (window as any).renderVim(t);
    });
    const clip0 = await page.evaluate(() => (window as any).CLIPBOARD);
    await page.keyboard.press("Control+Shift+u"); await page.evaluate(() => (window as any).__editIdle());
    expect(await doc(page)).toEqual(["body"]);
    expect(await page.evaluate(() => (window as any).CLIPBOARD), "删除不进剪贴板").toBe(clip0);
    expect(await page.evaluate(() => (window as any).getActiveTab().cursor.col), "光标回行首").toBe(0);
    expect(errors).toEqual([]);
  });

  test("Ctrl+Shift+K 删到行末 (F_LineDeleteToEnd, 不吞换行符)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["keep-tail", "second line"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 4 };
      (window as any).renderVim(t);
    });
    await page.keyboard.press("Control+Shift+k"); await page.evaluate(() => (window as any).__editIdle());
    expect(await doc(page), "只删到行末, 换行保留").toEqual(["keep", "second line"]);
    expect(errors).toEqual([]);
  });

  test("Ctrl+Shift+K 光标已在行末时不动 (上游语义: 不在这里吞换行)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abc", "def"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 3 };
      (window as any).renderVim(t);
    });
    await page.keyboard.press("Control+Shift+k"); await page.evaluate(() => (window as any).__editIdle());
    expect(await doc(page)).toEqual(["abc", "def"]);
    expect(errors).toEqual([]);
  });

  test("Ctrl+Backspace 删到词首 (F_WordDeleteToStart)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["alpha beta"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 10 };   // beta 词内
      (window as any).renderVim(t);
    });
    await page.keyboard.press("Control+Backspace"); await page.evaluate(() => (window as any).__editIdle());
    expect(await doc(page)).toEqual(["alpha "]);
    expect(errors).toEqual([]);
  });

  test("Ctrl+Delete = 删到词尾 (Sakura F_WordDeleteToEnd 实测); Ctrl+Shift+Backspace = 删上方行", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["l1", "l2", "l3"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 1, col: 0 };   // 在 l2 上
      (window as any).renderVim(t);
    });
    await page.keyboard.press("Control+Delete"); await page.evaluate(() => (window as any).__editIdle());          // Sakura: 删到词尾 -> l2 被删空
    expect(await doc(page)).toEqual(["l1", "", "l3"]);
    await page.keyboard.press("Control+Shift+Backspace"); await page.evaluate(() => (window as any).__editIdle()); // 删上方行 -> 删掉 l1
    expect(await doc(page)).toEqual(["", "l3"]);
    expect(errors).toEqual([]);
  });

  test("既有 Ctrl+K O 打开文件夹 chord 没被 Ctrl+Shift+K 破坏", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["l1", "l2"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 3 };
      (window as any).renderVim(t);
    });
    // Sakura: Ctrl+K = F_LineCutToEnd (行末まで切り取り); 旧的 Ctrl+K O 弦序列已让位, 打开文件夹改 Ctrl+Alt+O
    await page.evaluate(() => { const t = (window as any).getActiveTab(); t.cursor = { row: 0, col: 1 }; (window as any).renderVim(t); });
    await page.keyboard.press("Control+k"); await page.evaluate(() => (window as any).__editIdle());
    await page.waitForTimeout(120);
    expect(await doc(page), "Ctrl+K 剪切到行末").toEqual(["l", "l2"]);
    expect(errors).toEqual([]);
  });

  test("新增命令都能从菜单派发 (无未实装提示)", async ({ page }) => {
    const errors = await boot(page);
    const cmds = [
      "cut-line", "cut-line-start", "delete-line-to-start", "delete-line-to-end",
      "delete-word-start", "delete-word-end", "ltrim-lines", "rtrim-lines",
      "uniq-consecutive", "indent-selection", "unindent-selection",
    ];
    const bad = await page.evaluate((list) => {
      const w = window as any;
      const out: string[] = [];
      for (const c of list as string[]) {
        const t = w.getActiveTab();
        t.lines = ["  a  ", "  a  ", "b"];
        t.cursor = { row: 0, col: 1 };
        t.sel = null;
        t.colSel = null;
        try {
          w.dispatchCommand(c);
        } catch (e: any) {
          out.push(`${c}: THROW ${e.message}`);
          continue;
        }
        const tip = document.getElementById("s-tip")!.textContent || "";
        if (/未实装|未知命令|命令失败/.test(tip)) out.push(`${c}: tip=${tip}`);
      }
      return out;
    }, cmds);
    expect(bad).toEqual([]);
    expect(errors).toEqual([]);
  });
});

// ============================================================ C. 增删类特色功能

test.describe("C. 增删类特色功能", () => {
  test("删除行首空白 (F_LTRIM) 选区限定", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["    a", "\t\tb", "  c"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 0, r1: 1, c1: 3 };
      (window as any).renderVim(t);
    });
    await page.evaluate(() => (window as any).dispatchCommand("ltrim-lines"));
    const out = await doc(page);
    expect(out[0]).toBe("a");
    expect(out[1], "tab 缩进也删").toBe("b");
    expect(out[2], "选区外的行不动").toBe("  c");
    expect(errors).toEqual([]);
  });

  test("删除行尾空白 (F_RTRIM) 选区限定", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["a   ", "b\t\t", "c  "]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 0, r1: 1, c1: 3 };
      (window as any).renderVim(t);
    });
    await page.evaluate(() => (window as any).dispatchCommand("rtrim-lines"));
    const out = await doc(page);
    expect(out[0]).toBe("a");
    expect(out[1]).toBe("b");
    expect(out[2], "选区外不动").toBe("c  ");
    expect(errors).toEqual([]);
  });

  test("去重**连续**重复行: 非连续的重复必须保留 (F_UNIQ 语义, 不是全局去重)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["a", "a", "b", "a"]);
    await page.evaluate(() => { const t = (window as any).getActiveTab(); t.sel = { r0: 0, c0: 0, r1: 3, c1: 1 }; });
    await page.evaluate(() => (window as any).dispatchCommand("uniq-consecutive"));
    expect(await doc(page), "只有连续的 a 被合并, 中间隔着的 a 保留").toEqual(["a", "b", "a"]);
    expect(errors).toEqual([]);
  });

  test("去重连续重复行: 空行夹在中间时两侧的重复都保留", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["a", "", "a"]);
    await page.evaluate(() => { const t = (window as any).getActiveTab(); t.sel = { r0: 0, c0: 0, r1: 2, c1: 1 }; });
    await page.evaluate(() => (window as any).dispatchCommand("uniq-consecutive"));
    expect(await doc(page), "空行隔开 = 不算连续").toEqual(["a", "", "a"]);
    expect(errors).toEqual([]);
  });

  test("缩进 / 反缩进: TAB 缩进总是插入 TAB 字符 (Sakura F_INDENT_TAB 实测), 反缩进还原", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["alpha", "beta", "gamma"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 0, r1: 2, c1: 5 };
      (window as any).renderVim(t);
    });
    await page.evaluate(() => (window as any).dispatchCommand("indent-selection"));
    expect(await doc(page)).toEqual(["	alpha", "	beta", "	gamma"])   // Sakura: TAB 缩进插入 TAB 字符;

    await page.evaluate(() => (window as any).dispatchCommand("unindent-selection"));
    expect(await doc(page), "反缩进回到原样").toEqual(["alpha", "beta", "gamma"]);
    expect(errors).toEqual([]);
  });

  test("缩进: tab 缩进文件用 tab 单位", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["\talpha", "\tbeta"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.sel = { r0: 0, c0: 0, r1: 1, c1: 5 };
      (window as any).renderVim(t);
    });
    await page.evaluate(() => (window as any).dispatchCommand("indent-selection"));
    expect(await doc(page)).toEqual(["\t\talpha", "\t\tbeta"]);
    await page.evaluate(() => (window as any).dispatchCommand("unindent-selection"));
    expect(await doc(page)).toEqual(["\talpha", "\tbeta"]);
    expect(errors).toEqual([]);
  });

  test("无选区时缩进不生效 (不抢 Tab 键本身)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["one", "two"]);
    await page.evaluate(() => (window as any).dispatchCommand("indent-selection"));
    expect(await doc(page), "无选区 = 不动").toEqual(["one", "two"]);
    expect(errors).toEqual([]);
  });

  test("列块下缩进 = 在列块起点插入 (每行同一列, 与列块输入一致)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["abcdef", "abcdef"]);
    await page.evaluate(() => {
      const w = window as any;
      w.startColumnSelect(0, 3);
      w.extendColumnSelect(1, 3);
    });
    await page.evaluate(() => (window as any).dispatchCommand("indent-selection"));
    const out = await doc(page);
    expect(out.map((s: string) => s.indexOf("  ")), "插入点落在同一列").toEqual([3, 3]);
    expect(errors).toEqual([]);
  });

  test("所有增删改都进撤销栈 (一次 Ctrl+Z 能回退)", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["head-body", "second"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 4 };
      (window as any).renderVim(t);
    });
    await page.evaluate(() => (window as any).dispatchCommand("delete-to-line-start") || (window as any).dispatchCommand("delete-line-to-start"));
    expect(await doc(page)).toEqual(["-body", "second"]);
    await page.evaluate(() => (window as any).dispatchCommand("undo"));
    expect(await doc(page), "撤销恢复原样").toEqual(["head-body", "second"]);
    expect(errors).toEqual([]);
  });

  test("Unicode 安全: 增删按码点切, 不劈开代理对", async ({ page }) => {
    const errors = await boot(page);
    await setDoc(page, ["😀😀tail"]);
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.cursor = { row: 0, col: 2 };   // 两个 emoji 之后 (按码点计)
      (window as any).renderVim(t);
    });
    await page.evaluate(() => (window as any).dispatchCommand("delete-line-to-start"));
    const out = await doc(page);
    expect(out[0], "emoji 被整体删掉, 没留下半个代理").toBe("tail");
    expect([...out[0]].length).toBe(4);
    expect(errors).toEqual([]);
  });
});
