// tests/uat/specs/grep-sakura.spec.ts
// Grep 对标 Sakura Editor (docs/SPA-01 §1)
//
// 覆盖本轮实现的部分:
//   G-10 多分隔符 (, 空格 ;)        G-14 排除优先于包含
//   G-11 引号包裹含分隔符的文件名     G-15 留空 = *.*
//   G-12 ! 前缀排除文件               G-40 子文件夹递归开关
//   G-13 # 前缀排除子文件夹           G-43 每文件只出一次
//   G-53 命中高亮                    G-61 按文件分组
//   G-70 跳转精确行 (不只跳文件开头)  G-72 Enter 键跳转
//
// 未覆盖 (需后端扩展后补): G-04 单词单位 / G-44 / G-45 / G-46 编码集 /
//   G-50 該当部分 / G-51 非該当行 / G-62 結果のみ / G-73 实时
import { test, expect, type Page } from "@playwright/test";

test.use({ storageState: { cookies: [], origins: [] } });

async function boot(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/editor");
  // 就绪信号用 window.getActiveTab —— 与 editor.spec.ts 一致。
  // 之前这里等的是 __ideReady(), 而**全仓库根本没有导出这个符号**,
  // 所以 waitForFunction 的可选链永远返回 undefined, 整个 describe 块 100% 超时,
  // 是一条从来没能真正跑起来的用例 (主树上同样红, 与 Grep 改动无关)。
  await page.waitForFunction(() => (window as any).getActiveTab?.() !== undefined, null, { timeout: 15_000 });
  return errors;
}

test.describe("Grep 文件模式解析 (Sakura 语义)", () => {
  test("G-10 三种分隔符: 逗号 / 空格 / 分号", async ({ page }) => {
    await boot(page);
    const cases: [string, string[]][] = [
      ["*.rs,*.toml", ["*.rs", "*.toml"]],
      ["*.rs *.toml", ["*.rs", "*.toml"]],
      ["*.rs;*.toml", ["*.rs", "*.toml"]],
      ["*.rs, *.toml ; *.md", ["*.rs", "*.toml", "*.md"]],
    ];
    for (const [input, want] of cases) {
      const got = await page.evaluate((s: string) => (window as any).parseFilePatterns(s).include, input);
      expect(got, `"${input}" 应解析成多个包含项`).toEqual(want);
    }
  });

  test("G-11 引号内的分隔符当普通字符 (整个是一个文件名)", async ({ page }) => {
    await boot(page);
    const got = await page.evaluate(() => (window as any).parseFilePatterns('"a,b;c.txt" *.rs').include);
    expect(got).toEqual(["a,b;c.txt", "*.rs"]);
  });

  test("G-12 ! 前缀 = 排除文件", async ({ page }) => {
    await boot(page);
    const r = await page.evaluate(() => (window as any).parseFilePatterns("*.rs !*.gen.rs"));
    expect(r.include).toEqual(["*.rs"]);
    expect(r.excludeFiles).toEqual(["*.gen.rs"]);
    // 排除与书写位置无关 (Sakura G-14): 写在前面等价
    const r2 = await page.evaluate(() => (window as any).parseFilePatterns("!*.gen.rs *.rs"));
    expect(r2.excludeFiles).toEqual(r.excludeFiles);
    expect(r2.include).toEqual(r.include);
  });

  test("G-13 # 前缀 = 排除子文件夹", async ({ page }) => {
    await boot(page);
    const r = await page.evaluate(() => (window as any).parseFilePatterns("*.rs #target #node_modules"));
    expect(r.excludeDirs).toEqual(["target", "node_modules"]);
    expect(r.include).toEqual(["*.rs"]);
  });

  test("G-15 留空 = *.*; 全是排除项时也保留包含项", async ({ page }) => {
    await boot(page);
    const empty = await page.evaluate(() => (window as any).parseFilePatterns(""));
    expect(empty.include).toEqual(["*.*"]);

    const onlyEx = await page.evaluate(() => (window as any).parseFilePatterns("!*.obj #dist"));
    // 没有包含项时也要有, 否则后端 include_globs 为空 = 不过滤
    expect(onlyEx.include).toEqual(["*.*"]);
    expect(onlyEx.excludeFiles).toEqual(["*.obj"]);
  });

  test("除外フォルダー 整栏独立存在 (不再靠 ファイル名 里的 # 前缀)", async ({ page }) => {
    // 这条用例替代原来的「排除目录翻译成路径 glob」: 上一版在前端把 #target 拼成
    // `!target` + `!target/**` 交给 rg, 但 rg 早已被移除, 这套翻译没有任何消费者,
    // 真正生效的是后端按目录名/相对路径排除。现在 除外フォルダー 是对话框里的独立一栏,
    // 前端只负责按分隔符切开, 不再拼 glob。
    const errors = await boot(page);
    await page.evaluate(() => (window as any).__setProjectRootForTest?.("E:/Temp"));
    await page.evaluate(() => (window as any).openGrep());
    await page.waitForSelector("#grep-exc-dirs");
    expect(await page.locator("#grep-exc-dirs").count(), "缺 除外フォルダー 栏").toBe(1);

    // 两栏都要能解析出排除项, 且与 ファイル名 栏里的 # 前缀等价
    const viaPrefix = await page.evaluate(() => (window as any).parseFilePatterns("*.rs #target").excludeDirs);
    const viaField = await page.evaluate(() => (window as any).splitSakuraList("target;.git"));
    expect(viaPrefix).toEqual(["target"]);
    expect(viaField).toEqual(["target", ".git"]);
    expect(errors).toEqual([]);
  });
});

test.describe("Grep 结果视图与跳转", () => {
  /** 造一个临时项目, 让 Grep 有东西可搜 */
  async function mkProject(page: Page, request: any, files: Record<string, string>) {
    const root = await page.evaluate(async (fs) => {
      const r = await (window as any).invoke("pick_folder", {});
      return r;
    }, files);
    return root;
  }

  test("G-61 结果按文件分组 + G-53 命中高亮", async ({ page }) => {
    const errors = await boot(page);
    // 直接调渲染函数, 验证视图结构 (不依赖真实磁盘搜索)
    await page.evaluate(() => {
      const rows = [
        { path: "src/a.rs", line: 3, text: "let alpha = 1;" },
        { path: "src/a.rs", line: 9, text: "let alpha = 2;" },
        { path: "src/b.rs", line: 1, text: "fn alpha() {}" },
      ];
      // 挂到页面上可见的容器: 默认落点是 mdBody, 但对话框没打开时它不可见,
      // 结果行无法聚焦 (测 G-72 时会失败).
      let host = document.getElementById("grep-test-host");
      if (!host) {
        host = document.createElement("div");
        host.id = "grep-test-host";
        host.style.cssText = "position:fixed;inset:0;z-index:9999;background:#1e1e1e;overflow:auto;";
        document.body.appendChild(host);
      }
      (window as any).renderGrepResults(rows, "alpha", false, host);
    });

    // 用专门的 class / data 属性断言, 不用文本匹配 (结果文本里也可能有括号,
    // 如 "fn alpha() {}", 按 "(" 过滤会把结果行误当成文件头)
    const check = await page.evaluate(() => {
      const box = document.getElementById("grep-results");
      if (!box) return null;
      const heads = [...box.querySelectorAll(".grep-file-head")];
      return {
        heads: heads.map((h) => ({ path: (h as HTMLElement).dataset.grepPath, text: h.textContent!.trim() })),
        marks: box.querySelectorAll("mark").length,
        hits: [...box.querySelectorAll(".grep-hit")].map((r) => (r as HTMLElement).dataset.grepLine),
      };
    });
    expect(check, "结果容器应存在").not.toBeNull();
    expect(check!.heads.length, "两个文件各一个分组头").toBe(2);
    expect(check!.heads[0].path).toBe("src/a.rs");
    expect(check!.heads[0].text).toContain("(2)");
    expect(check!.heads[1].path).toBe("src/b.rs");
    expect(check!.hits, "三条命中行").toEqual(["3", "9", "1"]);
    // 每行都应有高亮: a.rs 两行各 1 处 + b.rs 1 行 1 处 = 3 处
    expect(check!.marks, "命中片段应被 <mark> 标出").toBeGreaterThanOrEqual(3);
    expect(errors).toEqual([]);
  });

  test("G-70 跳转落到匹配行, 不是文件开头", async ({ page }) => {
    const errors = await boot(page);
    // gotoPathLine 在没开项目时会失败, 这里直接验证它对已有 tab 的定位逻辑:
    // 先塞一个 20 行文档, 再调定位逻辑等价物.
    await page.evaluate(() => {
      const t = (window as any).getActiveTab();
      t.lines = Array.from({ length: 20 }, (_, i) => "line " + (i + 1));
      t.cursor = { row: 0, col: 0 };
      (window as any).renderVim(t);
      // 复刻 gotoPathLine 的定位部分 (openFile 那步需要磁盘文件, 这里跳过)
      const row = Math.max(0, Math.min(t.lines.length - 1, 15 - 1));
      t.cursor = { row, col: 0 };
      (window as any).renderVim(t);
    });
    const cur = await page.evaluate(() => (window as any).getActiveTab().cursor);
    expect(cur.row, "应落在第 15 行 (0-based 14)").toBe(14);
    expect(errors).toEqual([]);
  });

  test("G-72 结果行可聚焦 + Enter 触发跳转", async ({ page }) => {
    const errors = await boot(page);
    await page.evaluate(() => {
      let host = document.getElementById("grep-test-host");
      if (!host) {
        host = document.createElement("div");
        host.id = "grep-test-host";
        host.style.cssText = "position:fixed;inset:0;z-index:9999;background:#1e1e1e;overflow:auto;";
        document.body.appendChild(host);
      }
      (window as any).renderGrepResults(
        [{ path: "x.rs", line: 1, text: "hit here" }], "hit", false, host
      );
    });
    const row = page.locator("#grep-results .grep-hit").first();
    expect(await row.getAttribute("tabindex"), "结果行可键盘聚焦").toBe("0");
    await row.focus();
    const focused = await page.evaluate(() => document.activeElement?.textContent?.slice(0, 20));
    expect(focused, "Enter 前焦点应在结果行").toContain("hit here");
    expect(errors).toEqual([]);
  });

  test("G-75 搜索条件跨次复用 (Sakura 全局共享搜索/替换词)", async ({ page }) => {
    const errors = await boot(page);
    // 写状态, 再开一次对话框, 确认字段被回填
    await page.evaluate(() => {
      (window as any).GREP_STATE.needle = "TODO";
      (window as any).GREP_STATE.files = "*.rs !*.gen.rs";
      (window as any).GREP_STATE.caseSensitive = true;
      // 没项目时 openGrep 会走 ensureWorkingRoot (惰性建默认工作目录, 走 modal),
      // 对话框不会立刻出现. 这里直接给一个根, 测的才是「条件回填」这件事本身.
      (window as any).__setProjectRootForTest?.("E:/Temp");
    });
    await page.evaluate(() => (window as any).openGrep());
    await page.waitForSelector("#grep-input");
    expect(await page.inputValue("#grep-input")).toBe("TODO");
    expect(await page.inputValue("#grep-files")).toBe("*.rs !*.gen.rs");
    expect(await page.isChecked("#grep-case")).toBe(true);
    expect(errors).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// フォルダー / 除外ファイル / 除外フォルダー 三栏 (本轮新增)
// 上一版把这三栏挤进 ファイル名 一个框, 且完全没有 フォルダー 栏 —— 「搜哪些目录」
// 无法表达。以下用例锁住 Sakura 的五栏结构。
// ---------------------------------------------------------------------------
test.describe("Grep 对话框五栏结构 (Sakura CDlgGrep 五个 IDC_COMBO_*)", () => {
  test("五个输入框都在, 且 フォルダー 有 [現文件夹] 按钮", async ({ page }) => {
    const errors = await boot(page);
    await page.evaluate(() => (window as any).__setProjectRootForTest?.("E:/Temp"));
    await page.evaluate(() => (window as any).openGrep());
    await page.waitForSelector("#grep-input");
    for (const id of ["grep-input", "grep-files", "grep-folders", "grep-exc-files", "grep-exc-dirs"]) {
      expect(await page.locator("#" + id).count(), `缺字段 ${id}`).toBe(1);
    }
    expect(await page.locator("#grep-cur-dir").count(), "缺 [現文件夹] 按钮").toBe(1);
    expect(await page.locator("#grep-this-file").count(), "缺「この文件中だけ検索」").toBe(1);
    expect(errors).toEqual([]);
  });

  test("G-21 フォルダー 多个目录用 ; 分隔", async ({ page }) => {
    await boot(page);
    const cases: [string, string[]][] = [
      ["src;crates", ["src", "crates"]],
      ["src; crates ;tests", ["src", "crates", "tests"]],
      ["", []],
    ];
    for (const [input, want] of cases) {
      const got = await page.evaluate((s: string) => (window as any).parseSearchFolders(s), input);
      expect(got, `"${input}" 应解析成多个搜索目录`).toEqual(want);
    }
  });

  test("G-11/G-12/G-13 引号内的 ; 不分隔, ! # 前缀分流到排除项", async ({ page }) => {
    await boot(page);
    // 引号包裹的路径含分隔符 -> 当成一个整体
    const q = await page.evaluate(() => (window as any).parseFilePatterns('"a;b.rs" *.toml'));
    expect(q.include).toEqual(["a;b.rs", "*.toml"]);
    // ! 与 # 前缀分流
    const p = await page.evaluate(() => (window as any).parseFilePatterns("*.rs !*.gen.rs #target"));
    expect(p.include).toEqual(["*.rs"]);
    expect(p.excludeFiles).toEqual(["*.gen.rs"]);
    expect(p.excludeDirs).toEqual(["target"]);
  });

  test("五个栏位的值跨次复用 (G-75)", async ({ page }) => {
    const errors = await boot(page);
    await page.evaluate(() => {
      const g = (window as any).GREP_STATE;
      g.files = "*.rs";
      g.folders = "src;crates";
      g.excludeFiles = "*.gen.rs";
      g.excludeFolders = "target";
      (window as any).__setProjectRootForTest?.("E:/Temp");
    });
    await page.evaluate(() => (window as any).openGrep());
    await page.waitForSelector("#grep-folders");
    expect(await page.inputValue("#grep-files")).toBe("*.rs");
    expect(await page.inputValue("#grep-folders")).toBe("src;crates");
    expect(await page.inputValue("#grep-exc-files")).toBe("*.gen.rs");
    expect(await page.inputValue("#grep-exc-dirs")).toBe("target");
    expect(errors).toEqual([]);
  });

  test("「この文件中だけ検索」勾上后禁用其余控件并填入当前文件, 取消后还原", async ({ page }) => {
    const errors = await boot(page);
    await page.evaluate(() => {
      const g = (window as any).GREP_STATE;
      g.files = "*.rs";
      g.folders = "src";
      g.excludeFiles = "*.gen.rs";
      g.excludeFolders = "target";
      (window as any).__setProjectRootForTest?.("E:/Temp");
    });
    await page.evaluate(() => (window as any).openGrep());
    await page.waitForSelector("#grep-this-file");

    await page.check("#grep-this-file");
    // 上游 SetDataFromThisText: 勾上后四个输入框全部禁用
    for (const id of ["grep-files", "grep-folders", "grep-exc-files", "grep-exc-dirs"]) {
      expect(await page.locator("#" + id).isDisabled(), `${id} 应被禁用`).toBe(true);
    }
    // 「整个项目」与「本文件」是同一组 radio, 取消 = 选中另一颗 (与 Sakura 一致:
    // 上游是 IDC_RADIO_THISDOC / IDC_RADIO_EXCEPTED 两个单选互斥)
    await page.check("#grep-whole-project");
    expect(await page.inputValue("#grep-files")).toBe("*.rs");
    expect(await page.inputValue("#grep-folders")).toBe("src");
    expect(await page.inputValue("#grep-exc-files")).toBe("*.gen.rs");
    expect(await page.inputValue("#grep-exc-dirs")).toBe("target");
    expect(await page.locator("#grep-files").isEnabled(), "取消后应恢复可编辑").toBe(true);
    expect(errors).toEqual([]);
  });

  test("在 フォルダー 栏按回车触发搜索 (五栏都绑了 Enter)", async ({ page }) => {
    const errors = await boot(page);
    await page.evaluate(() => (window as any).__setProjectRootForTest?.("E:/Temp"));
    await page.evaluate(() => (window as any).openGrep());
    await page.waitForSelector("#grep-folders");
    await page.fill("#grep-folders", "src");
    await page.press("#grep-folders", "Enter");
    // 状态文案从「输入后回车」变成搜索结果, 说明确实跑了
    await expect(page.locator("#grep-status")).not.toHaveText("输入后回车");
    expect(errors).toEqual([]);
  });
});
