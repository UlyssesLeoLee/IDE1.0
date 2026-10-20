// tests/uat/specs/minimal-layout.spec.ts
// 「启动即 Sakura」: 缺省界面 = 一个文本编辑器 + 细状态栏; 导航 / 文件 (活动栏 + 侧栏) 和底端面板不是消失而是折叠成常驻把手
// (左边 16px 细条 / 底部面板头), 点一下展开成浮在编辑区之上的抽屉; 标签条 / 面包屑 / 缩略图 / 右上角终端钮 + banner 仍然隐藏,
// 在 视图 > 界面布局 里可逐项打开; 缺省编辑方式是非模态 (没有 vim 三态), 设置 > Vim 模式 可选开启.
//
// 与其它 spec 的区别: playwright.config.ts 给所有用例注入了 ide10.layout=full + ide10.vim=on (既有用例按完整 IDE 界面写),
// 这里用 test.use({ storageState: 空 }) 覆盖掉, 看到的才是真正的「首次启动」缺省状态.
//
// 覆盖:
//   1. 缺省可见集合 = 菜单栏 + 编辑区 + 细状态栏 + 两个折叠把手 (左细条 / 面板头); 其余区域留在 DOM 里但 display:none, 且各区域严丝合缝铺满视口
//  13. 折叠把手与让位抽屉: 把手的 aria / 提示 / 箭头; **展开让位** (编辑区收缩, 抽屉占栅格不覆盖); 落盘; Esc; 640x480 不溢出;
//      资源管理器展开时刷新 (保住已展开目录和滚动位置); Esc 遇到打开的菜单只关菜单; 完整布局仍是并排停靠
//   2. 启动即有「无标题」文档, 焦点在编辑区, 有闪烁的光标
//   3. 直接打字 (不需要 vim 键), Esc 不换模式, 没有模式标签; 启动瞬间的按键不会被吞
//   4. 视图 > 界面布局 逐项开关 + 落盘 + 重载后仍在
//   5. 预设切换 (极简 / 完整 IDE 界面) 和 ✓ 标记
//   6. 命令需要的区域自动亮出 (打开文件夹 / 搜索 / 终端 / 缩略图)
//   7. 标签条: >=2 个文档才出现
//   8. 无标题文档 Save-As: 惰性确定工作目录 -> 起名 -> 写盘 -> 标签变成普通文件; 沙箱不变; 打开文件不需要先开项目
//   9. Vim 模式开关 (设置菜单): NORMAL/INSERT/VISUAL 机 + 状态栏标签 + 落盘 + 与布局预设互不影响
//  10. 每个新命令在两种预设下都能执行 (不抛错、不出"未实装"提示)
//  11. 首帧无闪 / localStorage 不可用 (隐私模式) 也能启动
//  12. 完整布局 (只注入 layout=full, 不带 vim 键) 的编辑方式跟随缺省值
import { test, expect, type Page, type APIRequestContext } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import { resetShell } from "./helpers";

const ADDR = process.env.IDE_SHELL_WEB_ADDR || "127.0.0.1:8123";
const BAD_TIP = /未实装|未知命令|命令失败|启动失败/;

// 真正的首次启动: 不带任何 localStorage
test.use({ storageState: { cookies: [], origins: [] } });

// ---------------------------------------------------------------- helpers

/** 打开 /editor, 收集 pageerror, 等 bootstrap 完成. */
async function boot(page: Page, url = "/editor"): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
  return errors;
}

interface Box { w: number; h: number; display: string }
async function box(page: Page, sel: string): Promise<Box | null> {
  return page.evaluate((s) => {
    const e = document.querySelector(s);
    if (!e) return null;
    const r = e.getBoundingClientRect();
    return { w: r.width, h: r.height, display: getComputedStyle(e).display };
  }, sel);
}
/** 真的画出来了: 在 DOM 里, 不是 display:none, 宽高都 > 0. */
async function shown(page: Page, sel: string): Promise<boolean> {
  const b = await box(page, sel);
  return !!b && b.display !== "none" && b.w > 0 && b.h > 0;
}
/** 留在 DOM 里 (没被删) 但没画出来. */
async function folded(page: Page, sel: string): Promise<boolean> {
  const b = await box(page, sel);
  return !!b && (b.display === "none" || b.w === 0 || b.h === 0);
}

const docText = (page: Page) => page.evaluate(() => (window as any).getActiveTab().lines.join("\n"));
const tabCount = (page: Page) => page.evaluate(() => (window as any).TABS.size);
const layoutOf = (page: Page) => page.evaluate(() => (window as any).getLayout());
const ls = (page: Page, key: string) => page.evaluate((k) => localStorage.getItem(k), key);
const modeOf = (page: Page) => page.evaluate(() => document.getElementById("s-mode")!.dataset.mode);
const tipOf = (page: Page) => page.evaluate(() => document.getElementById("s-tip")!.textContent || "");

/** 点菜单命令: 先点开菜单, 再悬停子菜单父行 (若有), 最后点命令行. */
async function menuCmd(page: Page, menu: string, cmd: string) {
  await page.locator(`.menu-item[data-menu="${menu}"] > .mb-btn`).click();
  const row = page.locator(`.menu-dropdown[data-dropdown="${menu}"] [data-cmd="${cmd}"]`);
  const parent = row.locator("xpath=ancestor::*[@data-submenu][1]");
  if (await parent.count()) await parent.hover();
  await row.click();
}
/** 菜单行的 ✓ 状态 (打开菜单时刷新). */
async function checkOf(page: Page, menu: string, cmd: string): Promise<string | null> {
  await page.locator(`.menu-item[data-menu="${menu}"] > .mb-btn`).click();
  const v = await page.locator(`[data-cmd="${cmd}"]`).first().getAttribute("data-check");
  await page.keyboard.press("Escape");
  await page.evaluate(() => (window as any).closeAllMenus && (window as any).closeAllMenus());
  return v;
}

/** 打开文件夹现在是页内输入框 (预填 pick_folder 的结果), 不是原生 prompt(): 填入路径并回车 */
async function fillOpenFolderModal(page: Page, root: string) {
  await page.locator(".md-input").waitFor({ state: "visible", timeout: 8_000 });
  await page.locator(".md-input").fill(root);
  await page.locator(".md-input").press("Enter");
}

/** 区域严丝合缝: 竖向 菜单栏/编辑区/底端面板(头)/状态栏 首尾相接铺满视口, 横向 编辑区左边的并排区域同理, 没有横向滚动.
 *  2026-10 起极简布局的抽屉改为**让位式**(占栅格轨道), 所以 min 的并排列与 full 一致;
 *  折叠态下活动栏 / 侧栏是 display:none, rect() 返回 null 被过滤掉, 断言自动只覆盖可见区域. */
async function expectTiled(page: Page, label = "") {
  const r = await page.evaluate(() => {
    const rect = (s: string) => {
      const e = document.querySelector(s);
      if (!e || getComputedStyle(e).display === "none") return null;
      const b = e.getBoundingClientRect();
      return b.width > 0 && b.height > 0 ? { l: b.left, t: b.top, r: b.right, b: b.bottom } : null;
    };
    return {
      iw: innerWidth, ih: innerHeight, sl: document.body.scrollLeft, sw: document.documentElement.scrollWidth,
      rows: ["#menubar", "#editor-area", "#bottom-panel", "#status"].map(rect),
      // 让位式布局下 min 与 full 的并排结构相同; 折叠的列是 display:none -> rect 返回 null
      cols: ["#left-strip", "#activity-bar", "#side-bar", "#editor-area"].map(rect),
    };
  });
  const rows = r.rows.filter(Boolean) as { l: number; t: number; r: number; b: number }[];
  const cols = r.cols.filter(Boolean) as { l: number; t: number; r: number; b: number }[];
  expect(r.sl, `${label} body.scrollLeft`).toBe(0);
  expect(r.sw, `${label} no horizontal overflow`).toBeLessThanOrEqual(r.iw);
  expect(rows[0].t, `${label} first row at top`).toBeCloseTo(0, 0);
  for (let i = 1; i < rows.length; i++) expect(rows[i].t, `${label} row ${i} touches previous`).toBeCloseTo(rows[i - 1].b, 0);
  expect(rows[rows.length - 1].b, `${label} last row reaches bottom`).toBeCloseTo(r.ih, 0);
  expect(cols[0].l, `${label} first column at left`).toBeCloseTo(0, 0);
  for (let i = 1; i < cols.length; i++) expect(cols[i].l, `${label} column ${i} touches previous`).toBeCloseTo(cols[i - 1].r, 0);
  expect(cols[cols.length - 1].r, `${label} last column reaches right`).toBeCloseTo(r.iw, 0);
}

/** 服务端的工作目录 (web 版 pick_folder 的缺省根); 同机, 所以测试进程能直接读写磁盘上的这个目录. */
async function serverRoot(request: APIRequestContext): Promise<string> {
  const res = await request.get("/api/pick_folder");
  expect(res.ok()).toBeTruthy();
  const root: string = await res.json();
  return root.replace(/^\\\\\?\\/, "");   // Windows verbatim 前缀
}

test.beforeEach(async () => {
  await resetShell();   // 清服务端项目根: 每个用例都从「还没有工作目录」开始
});

// ---------------------------------------------------------------- 1. 缺省可见集合

test("首次启动: 菜单栏 + 编辑区 + 细状态栏 + 两个折叠把手, 其余折叠 (留在 DOM 里)", async ({ page }) => {
  const errors = await boot(page);
  const lay = await layoutOf(page);
  expect(lay.preset).toBe("min");
  expect(lay.flags).toMatchObject({ activityBar: false, sideBar: false, breadcrumb: false, bottomPanel: false, minimap: false, statusBar: true, tabs: "auto" });
  // 启动本身不写任何设置 (缺省值靠「没有键」表达)
  expect(await ls(page, "ide10.layout")).toBeNull();
  expect(await ls(page, "ide10.vim")).toBeNull();

  // 可见: 菜单栏 + 编辑区 + 细状态栏, 加上两个折叠把手 (左细条 / 底端面板头)
  for (const sel of ["#menubar", "#editor", "#gutter", "#ed-content", "#status", "#left-strip", "#left-strip-btn", "#bottom-panel", "#bottom-head"]) expect(await shown(page, sel), sel).toBe(true);
  for (const sel of ["#activity-bar", "#side-bar", "#bottom-body", "#tabs", "#breadcrumb", "#banner", "#menubar > .tb-btn", "#s-mode"]) {
    expect(await folded(page, sel), `${sel} 应折叠但仍在 DOM`).toBe(true);
  }
  expect((await box(page, "#left-strip"))!.w, "左细条约 16px 宽").toBeCloseTo(16, 0);
  expect((await box(page, "#bottom-head"))!.h, "面板头是一条细条").toBeLessThanOrEqual(28);
  expect(await page.locator("#minimap").count(), "缩略图条按需创建, 默认不存在").toBe(0);
  expect((await page.locator("#gutter").textContent())!.trim()).toBe("1");   // 行号开着
  expect(await page.locator("#welcome").count(), "没有欢迎页").toBe(0);
  expect(await page.title()).toBe("无标题 - IDE1.0");

  // 960x600 和 1280x800 下菜单栏 / 编辑区 / 状态栏首尾相接铺满视口
  await expectTiled(page, "960x600");
  await page.setViewportSize({ width: 1280, height: 800 });
  await expectTiled(page, "1280x800");
  // 状态栏没有 vim 模式标签, 文件名 / 语言仍在
  expect(await page.locator("#s-file").textContent()).toBe("无标题");
  expect(await page.locator("#s-lang").textContent()).toBe("Plain");
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 2. 无标题文档

test("启动即有「无标题」文档, 焦点在编辑区", async ({ page }) => {
  const errors = await boot(page);
  const st = await page.evaluate(() => {
    const t = (window as any).getActiveTab();
    return {
      tabs: (window as any).TABS.size, untitled: !!t.untitled, name: t.name, path: t.path, dirty: t.dirty,
      focus: document.activeElement && document.activeElement.id, pos: document.getElementById("s-pos")!.textContent,
    };
  });
  expect(st).toMatchObject({ tabs: 1, untitled: true, name: "无标题", path: "", dirty: false, focus: "ed-content", pos: "Ln 1, Col 1" });
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 3. 直接打字

test("缺省非模态: 直接打字, vim 键位当普通字符, Esc 无副作用, 没有模式标签", async ({ page }) => {
  const errors = await boot(page);
  await page.keyboard.type("dihello");   // d / i / h 在 vim NORMAL 下是命令; 这里必须原样进文档
  expect(await docText(page)).toBe("dihello");
  expect(await page.locator("#s-pos").textContent()).toBe("Ln 1, Col 8");
  await page.keyboard.press("Escape");
  await page.keyboard.type("jk");        // Esc 后 j/k 也还是字符
  expect(await docText(page)).toBe("dihellojk");
  expect(await modeOf(page)).toBe("INSERT");
  expect(await folded(page, "#s-mode"), "模式标签不显示").toBe(true);
  expect(await page.evaluate(() => (window as any).getActiveTab().dirty)).toBe(true);
  // 回车 / 退格 / 方向键 / Home / End 是基本功
  await page.keyboard.press("Enter");
  await page.keyboard.type("two");
  expect(await docText(page)).toBe("dihellojk\ntwo");
  await page.keyboard.press("Home");
  await page.keyboard.type(">");
  await page.keyboard.press("End");
  await page.keyboard.type("<");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("Backspace");
  // 第 2 行 ">two<" 光标在第 5 列; 上移到第 1 行同一列 (dihel|lojk), 退格删掉 l
  expect(await docText(page)).toBe("dihelojk\n>two<");
  // Alt+字母是菜单助记键 (Alt+F = 文件菜单): 不能同时往文档里打字母
  await page.keyboard.press("Alt+f");
  expect(await docText(page)).toBe("dihelojk\n>two<");
  expect(await page.locator(".menu-item.open").count()).toBe(1);
  expect(errors).toEqual([]);
});

test("光标: 非模态时是一根会闪的竖线, 跟着打字 / 方向键 / Home 走, 长文本滚动跟随; Vim 开着时不画", async ({ page }) => {
  const errors = await boot(page);
  const caret = () => page.evaluate(() => {
    const c = document.getElementById("caret")!, cs = getComputedStyle(c), r = c.getBoundingClientRect();
    // 独立算一遍期望位置: 光标前一个字符在文本节点里的右边缘 (行首则是文本左边缘)
    const host = document.getElementById("ed-content")!, t = (window as any).getActiveTab();
    const only = host.firstChild;
    let expect: number | null = null;
    if (only && only.nodeType === 3) {
      let off = 0;
      for (let i = 0; i < t.cursor.row; i++) off += t.lines[i].length + 1;
      const u = Array.from(t.lines[t.cursor.row] as string).slice(0, t.cursor.col).join("").length;
      const rg = document.createRange();
      rg.setStart(only, off + u); rg.collapse(true);
      const b = rg.getClientRects()[0];
      expect = b ? b.left : null;
    }
    return { display: cs.display, anim: cs.animationName, w: r.width, h: r.height, left: r.left, top: r.top, expect, hostBottom: host.getBoundingClientRect().bottom };
  });
  const c0 = await caret();
  expect(c0).toMatchObject({ display: "block", anim: "caret-blink", w: 2, h: 18 });
  const gutterRight = await page.evaluate(() => document.getElementById("gutter")!.getBoundingClientRect().right);
  expect(c0.left, "光标在行号栏右边, 文本起点").toBeGreaterThanOrEqual(gutterRight);

  await page.keyboard.type("hello");
  let c = await caret();
  expect(c.expect).not.toBeNull();
  expect(Math.abs(c.left - (c.expect as number))).toBeLessThan(1.5);
  expect(c.left).toBeGreaterThan(c0.left);
  await page.keyboard.press("Home");
  c = await caret();
  expect(Math.abs(c.left - c0.left)).toBeLessThan(1.5);
  await page.keyboard.press("Enter");
  c = await caret();
  expect(c.top, "回车后光标落到下一行").toBeGreaterThan(c0.top + 10);

  // 中文 (宽字符) 按真实排版定位: 光标在最后一个字右边
  await page.evaluate(() => { const t = (window as any).getActiveTab(); t.lines = ["ab中文日本語"]; t.cursor = { row: 0, col: 7 }; (window as any).renderVim(t); });
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("ArrowRight");
  c = await caret();
  expect(Math.abs(c.left - (c.expect as number))).toBeLessThan(1.5);

  // 闪烁: 一个周期内不透明度既有 1 又有 0
  const ops = new Set<string>();
  for (let i = 0; i < 14; i++) { ops.add(await page.evaluate(() => getComputedStyle(document.getElementById("caret")!).opacity)); await page.waitForTimeout(100); }
  expect(ops.has("1") && ops.has("0")).toBe(true);

  // 文本长过一屏: 打字 / 方向键让光标始终在可视区内 (编辑区自己滚过去)
  await page.evaluate(() => { const t = (window as any).getActiveTab(); t.lines = Array.from({ length: 80 }, (_, n) => "line " + n); t.cursor = { row: 0, col: 0 }; (window as any).renderVim(t); });
  for (let i = 0; i < 79; i++) await page.keyboard.press("ArrowDown");
  c = await caret();
  expect(c.display).toBe("block");
  expect(c.top + c.h).toBeLessThanOrEqual(c.hostBottom + 0.5);
  expect(await page.evaluate(() => document.getElementById("ed-content")!.scrollTop)).toBeGreaterThan(0);

  // Vim 开: 不画光标 (那里靠状态栏 Ln/Col + 模式标签), 关回来又出现; 失去焦点时也不画
  await page.evaluate(() => (window as any).__layout.setVim(true));
  expect((await caret()).display).toBe("none");
  await page.evaluate(() => (window as any).__layout.setVim(false));
  await page.locator("#ed-content").focus();
  expect((await caret()).display).toBe("block");
  await page.evaluate(() => (document.activeElement as HTMLElement).blur());
  expect((await caret()).display).toBe("none");
  expect(errors).toEqual([]);
});

test("页面一拿到焦点就打字: 头几个键不会被当成 vim 命令吞掉", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/editor");
  // 不等 bootstrap (它是异步的, 启动那一小段正是以前吞键的窗口): 编辑区一有焦点就打
  await page.waitForFunction(() => document.activeElement && document.activeElement.id === "ed-content");
  await page.keyboard.type("hello");
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
  expect(await docText(page)).toBe("hello");
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 4. 视图 > 界面布局

// 极简布局里 活动栏 / 侧边栏 / 底端面板 三项的「显示 / 隐藏」是「展开 / 折叠」: 折叠时留着把手,
// 展开是**让位抽屉** —— 占栅格轨道, 编辑区收缩, 不覆盖正文 (2026-10 契约变更).
// 左抽屉 = 活动栏 + 侧栏: 从折叠态点任一项 = 整个左抽屉展开 (两项一起开); 已展开时再点才是各管各的.
const TOGGLES: { cmd: string; sel: string; flag: string; other?: { sel: string; flag: string }; kept?: string }[] = [
  { cmd: "toggle-activity-bar", sel: "#activity-bar", flag: "activityBar", other: { sel: "#side-bar", flag: "sideBar" } },
  { cmd: "toggle-side-bar", sel: "#side-bar", flag: "sideBar", other: { sel: "#activity-bar", flag: "activityBar" } },
  { cmd: "toggle-tabs", sel: "#tabs", flag: "tabs" },
  { cmd: "toggle-breadcrumb", sel: "#breadcrumb", flag: "breadcrumb" },
  { cmd: "toggle-bottom", sel: "#bottom-body", flag: "bottomPanel", kept: "#bottom-head" },   // 折叠只收正文, 面板头留作把手
  { cmd: "toggle-minimap", sel: "#minimap", flag: "minimap" },
];

for (const tg of TOGGLES) {
  test(`视图菜单 ${tg.cmd}: 展开/显示 -> 落盘 -> 重载仍在 -> 再点折叠/隐藏`, async ({ page }) => {
    const errors = await boot(page);
    expect(await shown(page, tg.sel), "起点: 折叠").toBe(false);
    expect(await checkOf(page, "view", tg.cmd), "起点 ✓ 关").toBe("off");
    const ed0 = (await box(page, "#editor"))!;

    await menuCmd(page, "view", tg.cmd);
    expect(await shown(page, tg.sel), "点一下: 展开/显示").toBe(true);
    if (tg.other) expect(await shown(page, tg.other.sel), "左抽屉整体展开: 另一项也出现").toBe(true);
    expect(await checkOf(page, "view", tg.cmd), "✓ 开").toBe("on");
    await expectTiled(page, tg.cmd);
    const flags = JSON.parse((await ls(page, "ide10.layout.flags")) || "{}");
    expect(flags[tg.flag], "只存与预设不同的项").toBe(true);
    // 让位抽屉会改变编辑区尺寸, 这里只断言开关本身生效, 几何由专门的让位契约测试覆盖
    if (tg.other || tg.kept) expect(await box(page, "#editor"), "编辑区应仍在 DOM 且有尺寸").not.toBeNull();

    await page.reload();
    await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
    expect(await shown(page, tg.sel), "重载后仍展开/显示").toBe(true);
    expect((await layoutOf(page)).flags[tg.flag]).toBe(true);
    await expectTiled(page, tg.cmd + " reload");

    await menuCmd(page, "view", tg.cmd);
    expect(await shown(page, tg.sel), "再点: 折叠/隐藏").toBe(false);
    if (tg.kept) expect(await shown(page, tg.kept), "折叠后把手 (面板头) 仍在").toBe(true);
    if (tg.flag === "tabs") {
      // 标签栏的预设默认是 "auto", 开关是两态 (true/false): 再点 = 明确隐藏, 覆盖项为 false (选预设才回到 auto)
      expect((await layoutOf(page)).flags.tabs).toBe(false);
    } else if (tg.other) {
      // 左抽屉已展开时, 两项各管各的: 这一项收起, 另一项 (和抽屉) 仍在; 存盘里只剩另一项
      expect(await shown(page, tg.other.sel), "另一项不受影响").toBe(true);
      expect(JSON.parse((await ls(page, "ide10.layout.flags")) || "{}")).toEqual({ [tg.other.flag]: true });
    } else {
      expect(await ls(page, "ide10.layout.flags"), "回到预设默认 -> 不再有覆盖项").toBeNull();
    }
    await expectTiled(page, tg.cmd + " off");
    expect(errors).toEqual([]);
  });
}

test("视图菜单 toggle-status: 状态栏隐藏后轨道一并折叠, 再点恢复", async ({ page }) => {
  const errors = await boot(page);
  expect(await shown(page, "#status")).toBe(true);
  expect(await checkOf(page, "view", "toggle-status")).toBe("on");
  const h0 = (await box(page, "#editor"))!.h;
  await menuCmd(page, "view", "toggle-status");
  expect(await shown(page, "#status")).toBe(false);
  expect((await box(page, "#editor"))!.h, "编辑区吃掉状态栏那一行").toBeGreaterThan(h0);
  await expectTiled(page, "status off");
  await page.reload();
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
  expect(await shown(page, "#status"), "重载后仍隐藏").toBe(false);
  await menuCmd(page, "view", "toggle-status");
  expect(await shown(page, "#status")).toBe(true);
  expect(errors).toEqual([]);
});

test("标签栏开关: 1 个文档时强制显示 / 再点隐藏 (auto 只是预设默认)", async ({ page }) => {
  const errors = await boot(page);
  expect(await shown(page, "#tabs")).toBe(false);
  await menuCmd(page, "view", "toggle-tabs");
  expect(await shown(page, "#tabs")).toBe(true);
  expect(await page.locator("#tabs .tab").count()).toBe(1);
  expect((await layoutOf(page)).flags.tabs).toBe(true);
  await menuCmd(page, "view", "toggle-tabs");
  expect(await shown(page, "#tabs")).toBe(false);
  expect((await layoutOf(page)).flags.tabs).toBe(false);
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 5. 预设

test("预设切换: 完整 IDE 界面 <-> 极简, ✓ 只在当前预设上, 覆盖项被清掉", async ({ page }) => {
  const errors = await boot(page);
  expect(await checkOf(page, "view", "layout-min")).toBe("on");
  expect(await checkOf(page, "view", "layout-full")).toBe("off");

  await menuCmd(page, "view", "layout-full");
  expect((await layoutOf(page)).preset).toBe("full");
  expect(await ls(page, "ide10.layout")).toBe("full");
  for (const sel of ["#activity-bar", "#side-bar", "#tabs", "#breadcrumb", "#bottom-panel", "#banner", "#menubar > .tb-btn"]) {
    expect(await shown(page, sel), `${sel} 在完整布局下显示`).toBe(true);
  }
  expect(await shown(page, "#left-strip"), "完整布局没有细条把手").toBe(false);
  expect(await checkOf(page, "view", "layout-full")).toBe("on");
  expect(await checkOf(page, "view", "layout-min")).toBe("off");
  expect(await page.locator("#welcome").count(), "已有文档, 切布局不强行弹欢迎页").toBe(0);
  // 状态栏标签只在 vim 开着时显示 (缺省值按预设, 见 __layout.vimDefault)
  const vimDefaultFull = await page.evaluate(() => (window as any).__layout.vimDefault.full);
  expect(await shown(page, "#s-mode")).toBe(vimDefaultFull);
  await expectTiled(page, "full");

  await menuCmd(page, "view", "layout-min");
  expect((await layoutOf(page)).preset).toBe("min");
  expect(await ls(page, "ide10.layout.flags")).toBeNull();
  expect(await shown(page, "#side-bar")).toBe(false);
  expect(await shown(page, "#banner")).toBe(false);
  expect(await shown(page, "#left-strip"), "回到极简: 细条把手回来").toBe(true);
  expect(await shown(page, "#bottom-head"), "面板头留着").toBe(true);
  expect(await shown(page, "#bottom-body")).toBe(false);
  await expectTiled(page, "min again");

  // setLayout({...}) 把单项叠在预设之上. 抽屉的展开 / 折叠只是瞬时状态, 不算对极简布局的定制 -> layout-min 的 ✓ 仍亮;
  // 真正的定制 (如面包屑) 才让它熄灭
  await page.evaluate(() => (window as any).setLayout({ sideBar: true }));
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await checkOf(page, "view", "layout-min"), "展开着抽屉仍是极简预设").toBe("on");
  expect(await checkOf(page, "view", "toggle-side-bar")).toBe("on");
  await page.evaluate(() => (window as any).setLayout({ breadcrumb: true }));
  expect(await checkOf(page, "view", "layout-min"), "面包屑开着: 不再是原样的极简").toBe("off");
  await menuCmd(page, "view", "layout-min");
  expect(await shown(page, "#side-bar"), "选预设 = 清空覆盖 (抽屉折叠)").toBe(false);
  expect(await shown(page, "#breadcrumb")).toBe(false);
  expect(await ls(page, "ide10.layout.flags")).toBeNull();
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 6. 自动亮出

test("打开文件夹 -> 左抽屉 (活动栏 + 侧栏) 自动展开并列出项目文件; 重载后仍展开", async ({ page, request }) => {
  const errors = await boot(page);
  expect(await shown(page, "#side-bar")).toBe(false);
  const ed0 = (await box(page, "#editor"))!;
  const root = await serverRoot(request);
  await resetShell();
  // web 版 open-folder: POST /api/pick_folder 不通 -> 回退 prompt() 让用户填根目录
  await menuCmd(page, "file", "open-folder");
  await fillOpenFolderModal(page, root);
  await page.waitForFunction(() => (document.querySelector("#side-body")?.textContent || "").includes("README.md"), null, { timeout: 8_000 });
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await shown(page, "#activity-bar"), "折叠的左抽屉整体展开: 活动栏跟着出现").toBe(true);
  expect(await box(page, "#editor"), "左抽屉展开后编辑区仍应可见 (让位, 不覆盖)").not.toBeNull();
  await expectTiled(page, "open-folder");
  expect((await layoutOf(page)).flags).toMatchObject({ sideBar: true, activityBar: true });
  await page.reload();
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
  expect(await shown(page, "#side-bar"), "展开状态随开关落盘").toBe(true);
  expect(errors).toEqual([]);
});

test("Ctrl+Shift+F / 侧栏类命令 -> 左抽屉展开", async ({ page }) => {
  const errors = await boot(page);
  await page.keyboard.press("Control+Shift+F");
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await shown(page, "#activity-bar")).toBe(true);
  expect(await shown(page, "#search-input")).toBe(true);
  await page.evaluate(() => (window as any).setLayout("min"));
  expect(await shown(page, "#side-bar")).toBe(false);
  for (const cmd of ["show-explorer", "show-search", "show-scm", "show-outline"]) {
    await page.evaluate(() => (window as any).setLayout("min"));
    await menuCmd(page, "view", cmd);
    expect(await shown(page, "#side-bar"), cmd).toBe(true);
    expect(await shown(page, "#activity-bar"), cmd).toBe(true);
  }
  // 只剩活动栏的左抽屉 (rail only): 侧栏类命令把侧栏补上
  await page.evaluate(() => (window as any).setLayout("min"));
  await page.evaluate(() => (window as any).setLayout({ activityBar: true }));
  expect(await shown(page, "#side-bar")).toBe(false);
  await menuCmd(page, "view", "show-explorer");
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(errors).toEqual([]);
});

test("终端类命令 -> 底端抽屉展开 (Ctrl+` / 显示终端 / 输出 / 问题 / 新建终端 / 底端面板开关); 折叠后面板头仍在", async ({ page }) => {
  const errors = await boot(page);
  expect(await shown(page, "#bottom-panel"), "面板头始终在").toBe(true);
  expect(await shown(page, "#bottom-body")).toBe(false);
  expect(await page.locator("#bottom-panel").getAttribute("data-collapsed")).toBe("true");
  await page.keyboard.press("Control+Backquote");
  expect(await shown(page, "#bottom-body")).toBe(true);
  expect(await page.locator("#bottom-panel").getAttribute("data-collapsed")).toBe("false");
  expect(await shown(page, "#terminal-pane")).toBe(true);
  await expectTiled(page, "ctrl+`");
  for (const [cmd, pane] of [["show-output", "#output-pane"], ["show-problems", "#problems-pane"], ["show-terminal", "#terminal-pane"]]) {
    await page.evaluate(() => (window as any).setLayout({ bottomPanel: false }));
    expect(await shown(page, "#bottom-body")).toBe(false);
    expect(await shown(page, "#bottom-head")).toBe(true);
    await menuCmd(page, "view", cmd);
    expect(await shown(page, "#bottom-body"), cmd).toBe(true);
    expect(await shown(page, pane), `${cmd} -> ${pane}`).toBe(true);
  }
  // 面板头上的「+ 新建终端」也会把抽屉展开
  await page.evaluate(() => (window as any).setLayout("min"));
  await page.locator('#bottom-head [data-cmd="terminal-new"]').click();
  await page.waitForFunction(() => getComputedStyle(document.getElementById("bottom-body")!).display !== "none", null, { timeout: 8_000 });
  // 极简下「折叠」= 只收正文, 面板头留作把手
  await menuCmd(page, "view", "toggle-bottom");
  expect(await shown(page, "#bottom-body")).toBe(false);
  expect(await shown(page, "#bottom-head")).toBe(true);
  expect(errors).toEqual([]);
});

test("缩略图命令 -> #minimap 出现, 落盘", async ({ page }) => {
  const errors = await boot(page);
  await page.keyboard.type("minimap line");
  await menuCmd(page, "view", "toggle-minimap");
  expect(await page.locator("#minimap").count()).toBe(1);
  expect(await shown(page, "#minimap")).toBe(true);
  expect((await layoutOf(page)).flags.minimap).toBe(true);
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 7. 标签条 auto

test("标签条: >=2 个文档才出现, 回到 1 个又收起 (Ctrl+N / 新建 / Ctrl+W)", async ({ page }) => {
  const errors = await boot(page);
  expect(await shown(page, "#tabs")).toBe(false);
  await page.keyboard.press("Control+n");
  expect(await tabCount(page), "Ctrl+N 恰好新建一个").toBe(2);
  expect(await shown(page, "#tabs")).toBe(true);
  expect(await page.locator("#tabs .tab").count()).toBe(2);
  expect(await page.evaluate(() => (window as any).getActiveTab().name)).toBe("无标题 2");
  await expectTiled(page, "2 tabs");
  await page.keyboard.type("second");
  expect(await docText(page)).toBe("second");
  await menuCmd(page, "file", "new-file");   // 文件 > 新建 也是直接建, 不问文件名
  expect(await tabCount(page)).toBe(3);
  // 关有未保存修改的文档会先确认 (见下面的专项用例); 这里只关心标签条的显隐, 一律确认
  let confirms = 0;
  page.on("dialog", (d) => { confirms++; d.accept(); });
  await page.keyboard.press("Control+F4");
  await page.keyboard.press("Control+F4");
  expect(await tabCount(page)).toBe(1);
  expect(await shown(page, "#tabs"), "回到 1 个 -> 收起").toBe(false);
  await expectTiled(page, "back to 1");
  // 关最后一个: 还是一个空白的无标题文档 (不是欢迎页)
  await page.keyboard.press("Control+F4");
  expect(await tabCount(page)).toBe(1);
  expect(await page.locator("#welcome").count()).toBe(0);
  expect(await page.evaluate(() => (window as any).getActiveTab().untitled)).toBe(true);
  expect(await docText(page), "确认后丢弃有修改的文档, 补上的是空白文档").toBe("");
  expect(confirms, "只有最后那个有未保存修改的文档弹了确认").toBe(1);
  expect(errors).toEqual([]);
});

test("关闭有未保存修改的文档先确认 (Ctrl+W / 标签 ✕ / 关闭全部): 取消不关, 确认才丢; 干净的文档直接关", async ({ page }) => {
  const errors = await boot(page);
  const asked: string[] = [];
  let answer = false;
  page.on("dialog", (d) => { asked.push(d.message()); if (answer) d.accept(); else d.dismiss(); });
  // 唯一的无标题文档: 打了字再 Ctrl+W
  await page.keyboard.type("keep me");
  await page.keyboard.press("Control+F4");
  expect(asked, "弹了一次确认, 提到文档名").toHaveLength(1);
  expect(asked[0]).toContain("无标题");
  expect(await docText(page), "取消 = 文档还在").toBe("keep me");
  answer = true;
  await page.keyboard.press("Control+F4");
  expect(asked).toHaveLength(2);
  expect(await docText(page), "确认 = 丢弃, 补一个空白的无标题文档").toBe("");
  expect(await tabCount(page)).toBe(1);
  // 干净的文档不问: 刚补上的空白文档再 Ctrl+W 直接换一个新的
  await page.keyboard.press("Control+F4");
  expect(asked).toHaveLength(2);
  // 两个标签: 点脏标签的 ✕ 同样先问
  await page.keyboard.type("dirty one");
  await page.keyboard.press("Control+n");
  expect(await shown(page, "#tabs")).toBe(true);
  answer = false;
  await page.locator("#tabs .tab").first().locator("[data-tab-close]").click();
  expect(asked, "点 ✕ 关脏标签先确认").toHaveLength(3);
  expect(await tabCount(page), "取消: 两个标签都在").toBe(2);
  // 关闭全部 (菜单): 有 1 个脏文档, 统一问一次
  await menuCmd(page, "file", "close-all");
  expect(asked, "关闭全部只问一次").toHaveLength(4);
  expect(asked[3]).toContain("1 个文档");
  expect(await tabCount(page), "取消: 都还在").toBe(2);
  answer = true;
  await menuCmd(page, "file", "close-all");
  expect(asked).toHaveLength(5);
  expect(await tabCount(page), "确认后只剩一个空白的无标题文档").toBe(1);
  expect(await docText(page)).toBe("");
  // 全部是干净的: 关闭全部不再问
  await menuCmd(page, "file", "close-all");
  expect(asked).toHaveLength(5);
  expect(errors).toEqual([]);
});

test("脏标记: 标签条隐藏时, 状态栏文件名前有 ● , 窗口标题也带 ●", async ({ page }) => {
  const errors = await boot(page);
  expect(await page.locator("#s-file").getAttribute("data-dirty")).toBe("off");
  await page.keyboard.type("x");
  expect(await page.locator("#s-file").getAttribute("data-dirty")).toBe("on");
  expect(await page.title()).toBe("● 无标题 - IDE1.0");
  expect(await page.locator("#s-file").textContent(), "文件名文本本身不变").toBe("无标题");
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 8. Save-As / 打开

test("无标题文档 Ctrl+S: 惰性取工作目录 -> 只问一次文件名 -> 写盘 -> 标签变成普通文件", async ({ page, request }) => {
  const errors = await boot(page);
  const name = `uat-minimal-${Date.now()}.rs`;
  let prompts = 0;
  let pickFolder = 0;
  page.on("dialog", (d) => { prompts++; d.accept(name); });
  page.on("request", (r) => { if (r.url().endsWith("/api/pick_folder")) pickFolder++; });
  await page.keyboard.type("fn main() {}");
  await page.keyboard.press("Control+s");
  await page.waitForFunction(() => !(window as any).getActiveTab().untitled, null, { timeout: 8_000 });
  const root = await serverRoot(request);
  const target = path.join(root, name);
  try {
    expect(prompts, "Ctrl+S 同时被编辑区和 document 两处监听, 只能弹一次").toBe(1);
    expect(pickFolder, "还没有工作目录: 惰性取一次 (GET)").toBeGreaterThanOrEqual(1);
    const t = await page.evaluate(() => {
      const a = (window as any).getActiveTab();
      return { untitled: a.untitled, dirty: a.dirty, name: a.name, path: a.path, lang: document.getElementById("s-lang")!.textContent, file: document.getElementById("s-file")!.textContent };
    });
    expect(t.untitled).toBe(false);
    expect(t.dirty).toBe(false);
    expect(t.name).toBe(name);
    expect(t.path.replace(/\\/g, "/")).toContain(name);
    expect(t.file).toContain(name);
    expect(t.lang, "语法模式按扩展名重新识别").toBe("Rust");
    expect(fs.readFileSync(target, "utf8").replace(/\r\n/g, "\n")).toBe("fn main() {}");
    expect(await page.title()).toBe(`${name} - IDE1.0`);
    expect(await tabCount(page)).toBe(1);
    // 再按 Ctrl+S 是普通保存: 不再弹框 (光标还在行尾, 新字符接在后面)
    await page.keyboard.type("//");
    expect(await page.evaluate(() => (window as any).getActiveTab().dirty)).toBe(true);
    await page.keyboard.press("Control+s");
    await page.waitForFunction(() => !(window as any).getActiveTab().dirty, null, { timeout: 5_000 });
    expect(prompts).toBe(1);
    expect(fs.readFileSync(target, "utf8").replace(/\r\n/g, "\n")).toBe("fn main() {}//");
  } finally {
    fs.rmSync(target, { force: true });
  }
  expect(errors).toEqual([]);
});

test("Save-As 取消: 标签仍是无标题, 没写任何文件; 另存为命令同理", async ({ page, request }) => {
  const errors = await boot(page);
  let prompts = 0;
  page.on("dialog", (d) => { prompts++; d.dismiss(); });
  await page.keyboard.type("keep me");
  await page.keyboard.press("Control+s");
  await page.waitForTimeout(300);
  expect(prompts).toBe(1);
  expect(await page.evaluate(() => (window as any).getActiveTab().untitled)).toBe(true);
  expect(await page.evaluate(() => (window as any).getActiveTab().dirty)).toBe(true);
  await menuCmd(page, "file", "save-as");
  await page.waitForTimeout(300);
  expect(prompts).toBe(2);
  expect(await docText(page)).toBe("keep me");
  expect(errors).toEqual([]);
});

test("Save-As 沙箱不变: ../ 越界被拒绝, 弹「另存失败」, 文件没写到根目录之外", async ({ page, request }) => {
  const errors = await boot(page);
  const name = `uat-escape-${Date.now()}.txt`;
  page.on("dialog", (d) => d.accept(`../${name}`));
  await page.keyboard.type("nope");
  await page.keyboard.press("Control+s");
  await page.waitForFunction(() => /另存失败/.test(document.getElementById("modal")?.textContent || ""), null, { timeout: 8_000 });
  const root = await serverRoot(request);
  try {
    expect(fs.existsSync(path.join(root, "..", name)), "根目录之外不能出现文件").toBe(false);
    expect(fs.existsSync(path.join(root, name))).toBe(false);
    expect(await page.evaluate(() => (window as any).getActiveTab().untitled), "失败后仍是无标题").toBe(true);
  } finally {
    fs.rmSync(path.join(root, "..", name), { force: true });
  }
  expect(errors).toEqual([]);
});

test("文件 > 打开: 没有项目也能用 (惰性确定工作目录), 且替换空白的无标题文档", async ({ page }) => {
  const errors = await boot(page);
  page.once("dialog", (d) => d.accept("README.md"));
  await menuCmd(page, "file", "open-file");
  await page.waitForFunction(() => (window as any).getActiveTab().name === "README.md", null, { timeout: 8_000 });
  expect(await tabCount(page), "空白无标题文档被替换, 不留多余标签").toBe(1);
  expect(await shown(page, "#tabs")).toBe(false);
  expect(await docText(page)).toContain("hello");
  expect(await page.locator("#s-lang").textContent()).toBe("Markdown");
  expect(errors).toEqual([]);
});

test("Ctrl+Shift+S = 另存为 (有名文档也问新文件名), Ctrl+Alt+S = 全部保存 (每个脏的无标题文档各问一次); 一个按键只弹一次", async ({ page, request }) => {
  const errors = await boot(page);
  const stamp = Date.now();
  const names = ["a", "b", "c", "d"].map((x) => `uat-min-${x}-${stamp}.txt`);
  const answers: string[] = [names[0]];
  const asked: string[] = [];
  page.on("dialog", (d) => { asked.push(d.message()); const n = answers.shift(); if (n) d.accept(n); else d.dismiss(); });
  await page.keyboard.type("one");
  await page.keyboard.press("Control+s");
  await page.waitForFunction(() => !(window as any).getActiveTab().untitled, null, { timeout: 8_000 });
  const root = await serverRoot(request);
  const read = (n: string) => fs.readFileSync(path.join(root, n), "utf8").replace(/\r\n/g, "\n");
  try {
    // 有名文档上的 Ctrl+Shift+S: 走另存为 (以前和 Ctrl+S 一样原地保存, 不问), 标签改绑到新文件
    asked.length = 0;
    answers.push(names[1]);
    await page.keyboard.type("+two");
    await page.keyboard.press("Control+Shift+s");
    await page.waitForFunction((n) => (window as any).getActiveTab().name === n, names[1], { timeout: 8_000 });
    expect(asked, "Ctrl+Shift+S 同时被编辑区和 document 监听, 只能弹一次").toHaveLength(1);
    expect(read(names[1])).toBe("one+two");
    expect(await page.evaluate(() => (window as any).getActiveTab().dirty)).toBe(false);
    // Ctrl+Alt+S: 两个脏的无标题文档 -> 各问一次, 都写盘
    await page.keyboard.press("Control+n");
    await page.keyboard.type("u1");
    await page.keyboard.press("Control+n");
    await page.keyboard.type("u2");
    asked.length = 0;
    answers.push(names[2], names[3]);
    await page.keyboard.press("Control+Alt+s");
    await page.waitForFunction(() => Array.from((window as any).TABS.values()).every((t: any) => !t.dirty), null, { timeout: 8_000 });
    expect(asked, "全部保存: 两个无标题文档各问一次 (不是只问当前那个, 也不是同一个按键问两遍)").toHaveLength(2);
    expect(read(names[2])).toBe("u1");
    expect(read(names[3])).toBe("u2");
    expect(await page.evaluate(() => Array.from((window as any).TABS.values()).map((t: any) => t.name))).toEqual([names[1], names[2], names[3]]);
  } finally {
    for (const n of names) fs.rmSync(path.join(root, n), { force: true });
  }
  expect(errors).toEqual([]);
});

test("Ctrl+G / 搜索 > Grep 搜索 / Grep 替换: 没有项目也能用 (惰性确定工作目录), 对话框开着且输入框有焦点", async ({ page }) => {
  const errors = await boot(page);
  await page.keyboard.press("Control+g");
  await expect(page.locator("#modal")).toHaveClass(/show/);
  await expect(page.locator("#md-title")).toHaveText("Grep 搜索");
  expect(await page.evaluate(() => document.activeElement!.id)).toBe("grep-input");
  await page.keyboard.press("Escape");
  await expect(page.locator("#modal")).not.toHaveClass(/show/);
  await menuCmd(page, "search", "grep-replace");
  await expect(page.locator("#md-title")).toHaveText("Grep 替换");
  expect(await page.evaluate(() => document.activeElement!.id)).toBe("grep-input");
  expect(errors).toEqual([]);
});

test("左抽屉没有项目时给一个真按钮「打开文件夹」(极简布局没有工具栏): 点它走打开文件夹流程", async ({ page, request }) => {
  const errors = await boot(page);
  await page.locator("#left-strip-btn").click();
  const b = page.locator('#side-body button[data-cmd="open-folder"]');
  await expect(b).toBeVisible();
  await expect(page.locator("#side-body")).not.toContainText("工具栏");
  const root = await serverRoot(request);
  await resetShell();
  await b.click();
  await fillOpenFolderModal(page, root);
  await page.waitForFunction(() => (document.querySelector("#side-body")?.textContent || "").includes("README.md"), null, { timeout: 8_000 });
  await expect(page.locator('#side-body button[data-cmd="open-folder"]')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("分屏: 视图紧贴编辑区顶 (标签条 / 面包屑隐藏后没有 2px 空隙), 第二排视图用上边线分隔, 列间距不变", async ({ page }) => {
  const errors = await boot(page);
  await page.keyboard.type("a");
  for (let i = 0; i < 5; i++) { await page.keyboard.press("Enter"); await page.keyboard.type(String.fromCharCode(98 + i)); }
  for (const mode of ["split-h", "split-v", "split-quad"]) {
    await page.evaluate((m) => (window as any).dispatchCommand(m), mode);
    const r = await page.evaluate(() => {
      const area = document.getElementById("editor-area")!;
      const a = area.getBoundingClientRect();
      const views = Array.from(area.querySelectorAll<HTMLElement>(".split-view")).map((v) => {
        const b = v.getBoundingClientRect();
        return { l: b.left, t: b.top, r: b.right, b: b.bottom, row2: v.hasAttribute("data-row2"), bt: parseFloat(getComputedStyle(v).borderTopWidth) };
      });
      const cs = getComputedStyle(area);
      return { a: { l: a.left, t: a.top, r: a.right, b: a.bottom }, views, rowGap: cs.rowGap, colGap: cs.columnGap };
    });
    const n = mode === "split-quad" ? 4 : 2;
    expect(r.views, mode).toHaveLength(n);
    expect(r.rowGap, `${mode} min 下没有行间距`).toBe("0px");
    expect(r.views[0].t, `${mode} 视图贴着编辑区顶`).toBeCloseTo(r.a.t, 0);
    for (const v of r.views) {
      expect(v.row2 ? v.bt : 0, `${mode} 第二排用上边线分隔`).toBe(v.row2 ? 1 : 0);
      expect(v.b, `${mode} 不超出编辑区`).toBeLessThanOrEqual(r.a.b + 0.5);
    }
    if (mode === "split-h") expect(r.views[1].t, "上下两排紧挨 (边线在第二排里)").toBeCloseTo(r.views[0].b, 0);
    if (mode !== "split-h") expect(r.colGap, `${mode} 列间距照旧 1px`).toBe("1px");
    await expectTiled(page, mode);
    await page.evaluate((m) => (window as any).dispatchCommand(m), mode);   // 再发一次 = 关闭分屏
  }
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 9. Vim 模式

test("设置 > Vim 模式: 开 -> 模式标签 + NORMAL/INSERT/VISUAL; 关 -> 回到直接输入; 落盘", async ({ page }) => {
  const errors = await boot(page);
  expect(await checkOf(page, "setting", "toggle-vim")).toBe("off");
  await page.keyboard.type("abc");

  await menuCmd(page, "setting", "toggle-vim");
  expect(await page.evaluate(() => document.documentElement.dataset.vim)).toBe("on");
  expect(await ls(page, "ide10.vim")).toBe("on");
  expect(await shown(page, "#s-mode"), "模式标签出现").toBe(true);
  expect(await modeOf(page)).toBe("NORMAL");
  expect(await checkOf(page, "setting", "toggle-vim")).toBe("on");
  await expectTiled(page, "vim on");

  await page.locator("#ed-content").focus();
  await page.keyboard.type("i");           // NORMAL: i -> INSERT
  expect(await modeOf(page)).toBe("INSERT");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  expect(await modeOf(page)).toBe("NORMAL");
  expect(await docText(page)).toBe("abcX");
  await page.keyboard.type("jj");          // NORMAL: 命令, 不进文档
  expect(await docText(page)).toBe("abcX");
  await page.keyboard.type("v");
  expect(await modeOf(page)).toBe("VISUAL");

  // 在 VISUAL 里直接关掉 vim: 干净地回到直接输入, 选区被清掉
  await menuCmd(page, "setting", "toggle-vim");
  expect(await ls(page, "ide10.vim")).toBe("off");
  expect(await modeOf(page)).toBe("INSERT");
  expect(await folded(page, "#s-mode")).toBe(true);
  expect(await page.evaluate(() => (window as any).getActiveTab().sel ?? null)).toBeNull();
  await page.locator("#ed-content").focus();
  await page.keyboard.type("dd");
  expect(await docText(page)).toBe("abcXdd");
  expect(errors).toEqual([]);
});

test("Vim 模式选择落盘, 重载后仍在; 与布局预设互不影响", async ({ page }) => {
  const errors = await boot(page);
  await page.evaluate(() => (window as any).__layout.setVim(true));
  await page.reload();
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
  expect(await page.evaluate(() => document.documentElement.dataset.vim)).toBe("on");
  expect(await modeOf(page)).toBe("NORMAL");
  await page.locator("#ed-content").focus();
  await page.keyboard.type("i");
  await page.keyboard.type("q");
  expect(await docText(page)).toBe("q");

  // 显式选择胜过预设缺省: 来回切布局, vim 选择不变
  await menuCmd(page, "view", "layout-full");
  expect(await page.evaluate(() => (window as any).__layout.vimOn())).toBe(true);
  await menuCmd(page, "view", "layout-min");
  expect(await page.evaluate(() => (window as any).__layout.vimOn())).toBe(true);
  await page.evaluate(() => (window as any).__layout.setVim(false));
  await menuCmd(page, "view", "layout-full");
  expect(await page.evaluate(() => (window as any).__layout.vimOn()), "显式关掉后, 完整布局也不会把它再打开").toBe(false);
  expect(await ls(page, "ide10.vim")).toBe("off");
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 10. 命令覆盖

const NEW_CMDS = [
  "layout-min", "layout-full", "toggle-activity-bar", "toggle-side-bar", "toggle-tabs", "toggle-breadcrumb", "toggle-vim",
  "toggle-status", "toggle-bottom", "toggle-minimap", "show-explorer", "show-search", "show-scm", "show-outline",
  "show-terminal", "show-output", "show-problems", "new-file", "save", "save-as", "save-all", "open-file", "close-all",
];

for (const preset of ["min", "full"]) {
  for (const answer of ["cancel", "accept"]) {
    test(`命令覆盖 (${preset} 预设, 弹框${answer === "cancel" ? "取消" : "确认"}): 新增/相关命令全部能执行`, async ({ page, request }) => {
      const errors = await boot(page);
      const stamp = Date.now();
      try {
        await page.evaluate((p) => (window as any).setLayout(p), preset);
        const bad = await page.evaluate(async ([cmds, ans, st]) => {
          const w = window as any;
          w.prompt = ans === "accept" ? () => `uat-cmd-${st}.txt` : () => null;
          w.confirm = () => ans === "accept";
          const out: string[] = [];
          for (const c of cmds as string[]) {
            try { w.dispatchCommand(c); } catch (e: any) { out.push(`${c}: THROW ${e.message}`); continue; }
            await new Promise((r) => setTimeout(r, 40));
            const tip = document.getElementById("s-tip")!.textContent || "";
            if (/未实装|未知命令|命令失败|启动失败/.test(tip)) out.push(`${c}: tip=${tip}`);
          }
          return out;
        }, [NEW_CMDS, answer, stamp] as const);
        expect(bad).toEqual([]);
        await page.waitForTimeout(300);
        expect(errors).toEqual([]);
      } finally {
        // 「确认」跑里 save / save-as 会真的写出 uat-cmd-<stamp>.txt: 不管断言过没过都要清掉 (用例失败时也不能在仓库里留文件)
        if (answer === "accept") {
          const base = await serverRoot(request);
          for (const f of fs.readdirSync(base)) if (f.startsWith(`uat-cmd-${stamp}`)) fs.rmSync(path.join(base, f), { force: true });
        }
      }
    });
  }
}

// ---------------------------------------------------------------- 11. 首帧 / 隐私模式

test("首帧无闪: 侧栏出现在 DOM 的那一刻就已是 display:none (细条把手已就位), 也没有欢迎页", async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as any;
    const mo = new MutationObserver(() => {
      const s = document.getElementById("side-bar");
      if (s && !w.__atInsert) {
        w.__atInsert = {
          display: getComputedStyle(s).display, layout: document.documentElement.getAttribute("data-layout"),
          vim: document.documentElement.getAttribute("data-vim"), act: getComputedStyle(document.getElementById("activity-bar") || s).display,
          strip: document.getElementById("left-strip") ? getComputedStyle(document.getElementById("left-strip")!).display : "missing",
        };
      }
    });
    mo.observe(document, { childList: true, subtree: true });
    document.addEventListener("DOMContentLoaded", () => { w.__dcl = { welcome: !!document.getElementById("welcome"), tabs: w.TABS ? w.TABS.size : -1 }; });
  });
  const errors = await boot(page);
  const r = await page.evaluate(() => ({ ins: (window as any).__atInsert, dcl: (window as any).__dcl }));
  expect(r.ins).toMatchObject({ display: "none", layout: "min", vim: "off", act: "none", strip: "block" });
  expect(r.dcl).toEqual({ welcome: false, tabs: 1 });
  expect(errors).toEqual([]);
});

test("localStorage 不可用 (隐私模式): 照常启动, 能打字, 开关在本次会话内生效", async ({ page }) => {
  await page.addInitScript(() => {
    const thrower = () => { throw new DOMException("denied", "SecurityError"); };
    Storage.prototype.getItem = thrower;
    Storage.prototype.setItem = thrower;
    Storage.prototype.removeItem = thrower;
  });
  const errors = await boot(page);
  expect((await layoutOf(page)).preset).toBe("min");
  await page.keyboard.type("private");
  expect(await docText(page)).toBe("private");
  await menuCmd(page, "view", "toggle-side-bar");
  expect(await shown(page, "#side-bar")).toBe(true);
  await menuCmd(page, "setting", "toggle-vim");
  expect(await page.evaluate(() => document.documentElement.dataset.vim)).toBe("on");
  await page.keyboard.press("Control+n");
  expect(await tabCount(page)).toBe(2);
  expect(errors).toEqual([]);
});

// ---------------------------------------------------------------- 12. 完整布局 (只注入 layout=full)

test.describe("完整布局, 不带 vim 键", () => {
  test.use({
    storageState: { cookies: [], origins: [{ origin: `http://${ADDR}`, localStorage: [{ name: "ide10.layout", value: "full" }] }] },
  });

  test("编辑方式跟随缺省值 (__layout.vimDefault.full); 完整布局的其余行为照旧 (欢迎页 / 全部区域)", async ({ page, request }) => {
    const errors = await boot(page);
    const vimDefault: boolean = await page.evaluate(() => (window as any).__layout.vimDefault.full);
    expect((await layoutOf(page)).preset).toBe("full");
    expect(await page.locator("#welcome").count(), "完整布局启动仍是欢迎页").toBe(1);
    expect(await tabCount(page)).toBe(0);
    for (const sel of ["#activity-bar", "#side-bar", "#bottom-panel", "#status", "#menubar > .tb-btn", "#banner"]) expect(await shown(page, sel), sel).toBe(true);
    expect(await shown(page, "#s-mode"), "模式标签随缺省值").toBe(vimDefault);
    await expectTiled(page, "full boot");

    const root = await serverRoot(request);
    await resetShell();
    await page.locator("#welcome .open-btn").click();
    await fillOpenFolderModal(page, root);
    await page.waitForFunction(() => (document.querySelector("#side-body")?.textContent || "").includes("README.md"), null, { timeout: 8_000 });
    await page.locator('.tree-node:has-text("README.md")').first().click();
    await page.waitForFunction(() => (window as any).getActiveTab && (window as any).getActiveTab(), null, { timeout: 8_000 });
    await page.locator("#ed-content").focus();
    const before: string = await docText(page);
    await page.keyboard.type("i");
    if (vimDefault) {
      expect(await docText(page), "vim 开: i 是命令, 不进文档").toBe(before);
      expect(await modeOf(page)).toBe("INSERT");
    } else {
      expect(await docText(page), "vim 关: i 是字符").toBe("i" + before);
      expect(await modeOf(page)).toBe("INSERT");
    }
    expect(errors).toEqual([]);
  });
});

// ---------------------------------------------------------------- 13. 折叠把手 + 覆盖抽屉

interface Rect { l: number; t: number; r: number; b: number; w: number; h: number; display: string; z: string }
async function rectOf(page: Page, sel: string): Promise<Rect> {
  return page.evaluate((s) => {
    const e = document.querySelector(s)!;
    const b = e.getBoundingClientRect(), cs = getComputedStyle(e);
    return { l: b.left, t: b.top, r: b.right, b: b.bottom, w: b.width, h: b.height, display: cs.display, z: cs.zIndex };
  }, sel);
}
/** 视口坐标 (x, y) 处最上面的元素属于哪个区域 */
async function ownerAt(page: Page, x: number, y: number): Promise<string | null> {
  return page.evaluate(([px, py]) => {
    const e = document.elementFromPoint(px, py);
    if (!e) return null;
    return e.closest("#menubar") ? "menubar" : e.closest("#bottom-panel") ? "bottom" : e.closest("#activity-bar, #side-bar") ? "left" : e.closest("#left-strip") ? "strip" : e.closest("#editor") ? "editor" : (e.id || e.tagName);
  }, [x, y] as const);
}
const drawerState = (page: Page) => page.evaluate(() => (window as any).getDrawerState());
const hScrollOk = (page: Page) => page.evaluate(() => document.body.scrollLeft === 0 && document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight);
const bothOpen = (page: Page) => page.evaluate(() => (window as any).setLayout({ activityBar: true, sideBar: true, bottomPanel: true }));
const reloadReady = async (page: Page) => {
  await page.reload();
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady(), null, { timeout: 8_000 });
};

test("折叠把手: 左细条 (aria-label / aria-expanded / 提示 / 箭头) 和底部面板头, 点击展开 / 折叠", async ({ page }) => {
  const errors = await boot(page);
  const btn = page.locator("#left-strip-btn");
  // 折叠态: 细条贯通菜单栏与面板头之间, 约 16px 宽
  const strip = await rectOf(page, "#left-strip"), head = await rectOf(page, "#bottom-head"), mb = await rectOf(page, "#menubar");
  expect(strip.w).toBeCloseTo(16, 0);
  expect(strip.t).toBeCloseTo(mb.b, 0);
  expect(strip.b).toBeCloseTo(head.t, 0);
  expect(await btn.getAttribute("aria-expanded")).toBe("false");
  expect(await btn.getAttribute("aria-label")).toBe("展开导航 / 文件");
  expect(await btn.getAttribute("data-tip")).toBe("展开导航 / 文件");
  expect(await btn.textContent()).toBe("›");
  expect(await btn.getAttribute("aria-controls")).toContain("side-bar");
  // 悬停出提示气泡
  await btn.hover();
  await expect(page.locator("#tip")).toHaveText("展开导航 / 文件");
  // 底部: 面板头是细条, 紧贴状态栏上方; TERMINAL / OUTPUT / PROBLEMS + 展开 钮
  expect(head.h).toBeLessThanOrEqual(28);
  expect(head.b).toBeCloseTo((await rectOf(page, "#status")).t, 0);
  expect(await page.locator("#bottom-head .bottom-tab").allTextContents()).toEqual(["TERMINAL", "OUTPUT", "PROBLEMS"]);
  const tg = page.locator('#bottom-head [data-cmd="bottom-toggle"]');
  expect(await tg.textContent()).toBe("展开");
  expect(await tg.getAttribute("aria-expanded")).toBe("false");

  // 点细条 -> 展开 (箭头反向, 提示 / aria 跟着变), 再点 -> 折叠
  await btn.click();
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await btn.getAttribute("aria-expanded")).toBe("true");
  expect(await btn.getAttribute("aria-label")).toBe("折叠导航 / 文件");
  expect(await btn.textContent()).toBe("‹");
  await expect(page.locator("#tip")).toHaveText("折叠导航 / 文件");   // 鼠标还停在按钮上, 气泡就地换成新文案
  await btn.click();
  expect(await shown(page, "#side-bar")).toBe(false);
  expect(await btn.getAttribute("aria-expanded")).toBe("false");
  expect(await btn.textContent()).toBe("›");
  // 细条空白处 (按钮下方) 也能点
  await page.mouse.click(8, strip.t + 200);
  expect(await shown(page, "#side-bar")).toBe(true);
  await page.mouse.click(8, strip.t + 200);
  expect(await shown(page, "#side-bar")).toBe(false);
  // 点面板头的 PROBLEMS 标签 / 展开 钮 -> 展开正文
  await page.locator('#bottom-head .bottom-tab[data-pane="problems-pane"]').click();
  expect(await shown(page, "#bottom-body")).toBe(true);
  expect(await shown(page, "#problems-pane")).toBe(true);
  expect(await tg.textContent()).toBe("折叠");
  expect(await tg.getAttribute("aria-expanded")).toBe("true");
  await tg.click();
  expect(await shown(page, "#bottom-body")).toBe(false);
  expect(await shown(page, "#bottom-head")).toBe(true);
  expect(await tg.textContent()).toBe("展开");
  await tg.click();
  expect(await shown(page, "#bottom-body")).toBe(true);
  expect(errors).toEqual([]);
});

test("展开是让位不是覆盖: 抽屉占栅格, 编辑区收缩让出空间, 收起后复原", async ({ page }) => {
  // 2026-10 行为契约变更. 早期实现把抽屉做成 position:absolute 浮在编辑区之上,
  // 展开时编辑区尺寸不变 —— 结果抽屉直接盖住正文, 被挡的内容彻底看不到.
  // 现在改为并排停靠: 展开 -> 编辑区收缩; 收起 -> 完全复原.
  const errors = await boot(page);
  for (const [w, h] of [[960, 600], [1280, 800], [1920, 1080]]) {
    await page.setViewportSize({ width: w, height: h });
    await page.evaluate(() => (window as any).setLayout("min"));
    const tag = `${w}x${h}`;

    const ed0 = await rectOf(page, "#editor");
    const strip = await rectOf(page, "#left-strip");
    const mb = await rectOf(page, "#menubar");
    const st = await rectOf(page, "#status");
    expect(ed0.l, `${tag} 起点: 编辑区从细条右缘起`).toBeCloseTo(strip.r, 0);

    // ---- 左抽屉展开: 编辑区向右收缩, 起点右移到抽屉右缘 ----
    await page.locator("#left-strip-btn").click();
    const act = await rectOf(page, "#activity-bar");
    const side = await rectOf(page, "#side-bar");
    const ed1 = await rectOf(page, "#editor");

    expect(act.w, `${tag} 活动栏 44px`).toBeCloseTo(44, 0);
    expect(side.w, `${tag} 侧栏约 240px`).toBeCloseTo(240, 0);
    expect(side.l, `${tag} 侧栏紧接活动栏`).toBeCloseTo(act.r, 0);

    // 核心断言: 编辑区**让位**了 —— 左边界右移, 且右边界不变 (贴视口右边)
    expect(ed1.l, `${tag} 左抽屉展开: 编辑区左边界应右移让位`).toBeGreaterThan(ed0.l + 10);
    expect(ed1.l, `${tag} 编辑区左边界应贴在抽屉右缘`).toBeCloseTo(side.r, 0);
    expect(ed1.w, `${tag} 编辑区应变窄`).toBeLessThan(ed0.w - 100);
    expect(ed1.r, `${tag} 编辑区右边界不变`).toBeCloseTo(ed0.r, 0);

    // 编辑区不能被压到 0 或负宽 (minmax(0,1fr) + min-width:0 保证)
    expect(ed1.w, `${tag} 编辑区仍有可用宽度`).toBeGreaterThan(200);
    // 抽屉仍在菜单栏与状态栏之间
    for (const r of [act, side]) {
      expect(r.t, `${tag} 不盖菜单栏`).toBeGreaterThanOrEqual(mb.b - 0.5);
      expect(r.b, `${tag} 不盖状态栏`).toBeLessThanOrEqual(st.t + 0.5);
    }
    // 无重叠: 抽屉右缘 == 编辑区左缘
    expect(ed1.l, `${tag} 抽屉与编辑区不得重叠`).toBeGreaterThanOrEqual(side.r - 0.5);

    // ---- 底抽屉展开: 编辑区变矮, 下边界上移 ----
    await page.locator('#bottom-head .bottom-tab[data-pane="output-pane"]').click();
    const body = await rectOf(page, "#bottom-body");
    const ed2 = await rectOf(page, "#editor");

    expect(body.w, `${tag} 底抽屉通栏`).toBeCloseTo(w, 0);
    expect(body.h, `${tag} clamp(120px, 40vh, 320px)`).toBeCloseTo(Math.min(Math.max(120, 0.4 * h), 320), 0);
    expect(ed2.b, `${tag} 底抽屉展开: 编辑区下边界应上移让位`).toBeLessThan(ed1.b - 50);
    expect(ed2.h, `${tag} 编辑区应变矮`).toBeLessThan(ed1.h - 50);
    expect(ed2.h, `${tag} 编辑区仍够用`).toBeGreaterThan(100);
    // 无重叠: 编辑区下边界 == 底抽屉上边界
    expect(ed2.b, `${tag} 底抽屉与编辑区不得重叠`).toBeLessThanOrEqual(body.t + 0.5);
    expect(body.t, `${tag} 底抽屉不盖菜单栏`).toBeGreaterThanOrEqual(mb.b - 0.5);

    expect(await hScrollOk(page), `${tag} 无横向滚动条`).toBe(true);

    // ---- 全部收起: 完全复原到起始矩形 ----
    await page.locator('#bottom-head [data-cmd="bottom-toggle"]').click();
    await page.locator("#left-strip-btn").click();
    expect((await shown(page, "#side-bar")) || (await shown(page, "#bottom-body"))).toBe(false);
    const ed3 = await rectOf(page, "#editor");
    expect(ed3, `${tag} 全部收起后编辑区应完全复原`).toEqual(ed0);
  }
  expect(errors).toEqual([]);
});

test("按「最近」重排: 后展开的占整行/整列, 先展开的让位 (互不重叠), 抽屉内 mousedown 抬升, 菜单下拉盖在所有抽屉之上", async ({ page }) => {
  const errors = await boot(page);
  await page.locator("#left-strip-btn").click();
  expect((await drawerState(page)).top).toBe("left");
  await page.locator('#bottom-head .bottom-tab[data-pane="terminal-pane"]').click();
  expect((await drawerState(page)).top, "后展开的占角").toBe("bottom");
  let body = await rectOf(page, "#bottom-body"), side = await rectOf(page, "#side-bar"), panel = await rectOf(page, "#bottom-panel");
  expect(panel.l, "底抽屉横贯整行").toBeCloseTo(0, 0);
  expect(side.b, "先展开的左抽屉让位, 不压到底抽屉").toBeLessThanOrEqual(panel.t + 1);
  expect(await ownerAt(page, 100, body.t + 20), "左下角归底抽屉").toBe("bottom");

  await page.mouse.click(100, (await rectOf(page, "#menubar")).b + 60);   // 在左抽屉里按一下 -> 它成为最近的, 整列归它
  expect((await drawerState(page)).top).toBe("left");
  side = await rectOf(page, "#side-bar"); panel = await rectOf(page, "#bottom-panel");
  expect(panel.l, "底抽屉让位到左抽屉右侧").toBeGreaterThanOrEqual(side.r - 1);
  body = await rectOf(page, "#bottom-body");
  expect(await ownerAt(page, 100, body.t + 20), "左下角归左抽屉").toBe("left");
  await page.mouse.click(600, body.t + 60);                              // 在底抽屉里按一下 -> 又换回去
  expect((await drawerState(page)).top).toBe("bottom");

  // 先折叠再重新展开左抽屉 = 它又成了「最近展开」的
  await page.locator("#left-strip-btn").click();
  await page.locator("#left-strip-btn").click();
  expect((await drawerState(page)).top).toBe("left");
  // 已展开时又被命令点名 (如显示输出) -> 提到最上层
  await page.evaluate(() => (window as any).dispatchCommand("show-output"));
  expect((await drawerState(page)).top).toBe("bottom");

  // 永远不盖菜单栏: 打开视图菜单, 下拉在抽屉重叠区之上
  await page.locator('.menu-item[data-menu="view"] > .mb-btn').click();
  const dd = await rectOf(page, '.menu-dropdown[data-dropdown="view"]');
  expect(await ownerAt(page, dd.l + 20, dd.t + 20), "菜单下拉盖在抽屉之上").toBe("menubar");
  await page.keyboard.press("Escape");
  expect(errors).toEqual([]);
});

test("展开 / 折叠状态落盘: 重载后原样还原 (含按钮文案), 折叠后存盘清空", async ({ page }) => {
  const errors = await boot(page);
  await bothOpen(page);
  expect(JSON.parse((await ls(page, "ide10.layout.flags"))!)).toEqual({ activityBar: true, sideBar: true, bottomPanel: true });
  await reloadReady(page);
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await shown(page, "#activity-bar")).toBe(true);
  expect(await shown(page, "#bottom-body")).toBe(true);
  expect(await page.locator("#left-strip-btn").getAttribute("aria-expanded")).toBe("true");
  expect(await page.locator("#left-strip-btn").textContent()).toBe("‹");
  expect(await page.locator('#bottom-head [data-cmd="bottom-toggle"]').textContent()).toBe("折叠");
  expect(await page.locator("#bottom-panel").getAttribute("data-collapsed")).toBe("false");
  expect(await hScrollOk(page)).toBe(true);
  await page.locator("#left-strip-btn").click();
  await page.locator('#bottom-head [data-cmd="bottom-toggle"]').click();
  expect(await ls(page, "ide10.layout.flags"), "全部折叠 = 回到预设缺省, 不再有覆盖项").toBeNull();
  await reloadReady(page);
  expect(await shown(page, "#side-bar")).toBe(false);
  expect(await shown(page, "#bottom-body")).toBe(false);
  expect(await page.locator("#left-strip-btn").textContent()).toBe("›");
  expect(await page.locator('#bottom-head [data-cmd="bottom-toggle"]').textContent()).toBe("展开");
  expect(errors).toEqual([]);
});

test("Esc: 抽屉里按 Esc 折叠它并把焦点还给编辑区; 编辑区里按 Esc 绝不折叠 (含 Vim 开)", async ({ page }) => {
  const errors = await boot(page);
  const focusId = () => page.evaluate(() => (document.activeElement as HTMLElement).id);
  await page.locator("#left-strip-btn").click();
  await page.locator('.side-tab[data-side-tab="search"]').click();
  expect(await focusId(), "搜索框在抽屉里拿到焦点").toBe("search-input");
  await page.keyboard.press("Escape");
  expect(await shown(page, "#side-bar")).toBe(false);
  expect(await focusId()).toBe("ed-content");
  await page.keyboard.type("ok");                                         // 焦点回到编辑区: 能直接打字
  expect(await docText(page)).toBe("ok");

  await page.locator('#bottom-head .bottom-tab[data-pane="terminal-pane"]').click();
  expect(await shown(page, "#bottom-body")).toBe(true);
  await page.keyboard.press("Escape");                                    // 焦点在面板头的标签上
  expect(await shown(page, "#bottom-body")).toBe(false);
  expect(await shown(page, "#bottom-head")).toBe(true);
  expect(await focusId()).toBe("ed-content");

  // 点在抽屉空白处 (焦点落到 body) 再按 Esc, 也算「在抽屉里」
  await page.locator("#left-strip-btn").click();
  await page.mouse.click(150, (await rectOf(page, "#menubar")).b + 300);
  await page.keyboard.press("Escape");
  expect(await shown(page, "#side-bar")).toBe(false);

  // 键盘焦点在细条按钮上: Enter 展开, Esc 折叠
  await page.locator("#left-strip-btn").focus();
  await page.keyboard.press("Enter");
  expect(await shown(page, "#side-bar")).toBe(true);
  await page.keyboard.press("Escape");
  expect(await shown(page, "#side-bar")).toBe(false);

  // 在编辑区按 Esc 不折叠任何东西 (vim 用户一直在按 Esc)
  await bothOpen(page);
  await page.locator("#ed-content").focus();
  await page.keyboard.press("Escape");
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await shown(page, "#bottom-body")).toBe(true);
  await page.evaluate(() => (window as any).__layout.setVim(true));
  await page.locator("#ed-content").focus();
  await page.keyboard.press("Escape");
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(await shown(page, "#bottom-body")).toBe(true);
  // 对话框开着时 Esc 归对话框, 不折叠抽屉
  await page.evaluate(() => (window as any).__layout.setVim(false));
  await page.locator('#bottom-head .bottom-tab[data-pane="output-pane"]').focus();
  await page.evaluate(() => (window as any).dispatchCommand("settings"));
  expect(await shown(page, "#modal"), "首选项对话框已打开").toBe(true);
  await page.keyboard.press("Escape");
  expect(await shown(page, "#modal")).toBe(false);
  expect(await shown(page, "#bottom-body"), "Esc 先关了对话框").toBe(true);
  expect(errors).toEqual([]);
});

test("折叠后键盘焦点回到编辑区: 点细条 / 面板头按钮收起后能直接打字", async ({ page }) => {
  const errors = await boot(page);
  await page.locator("#left-strip-btn").click();
  await page.locator("#left-strip-btn").click();
  await page.keyboard.type("a");
  await page.locator('#bottom-head [data-cmd="bottom-toggle"]').click();
  await page.locator('#bottom-head [data-cmd="bottom-toggle"]').click();
  await page.keyboard.type("b");
  expect(await docText(page)).toBe("ab");
  expect(errors).toEqual([]);
});

test("窗口很小 (640x480 / 320x240) 两个抽屉都展开: 不溢出, 夹在菜单栏与面板头之间; 缩放窗口时几何跟着夹住", async ({ page }) => {
  const errors = await boot(page);
  await bothOpen(page);
  for (const [w, h] of [[640, 480], [320, 240], [1280, 800], [800, 200], [640, 480]]) {
    await page.setViewportSize({ width: w, height: h });
    await page.waitForTimeout(100);
    const tag = `${w}x${h}`;
    const mb = await rectOf(page, "#menubar"), side = await rectOf(page, "#side-bar"), act = await rectOf(page, "#activity-bar");
    const body = await rectOf(page, "#bottom-body"), head = await rectOf(page, "#bottom-head"), st = await rectOf(page, "#status");
    expect(await hScrollOk(page), `${tag} 无滚动条`).toBe(true);
    expect(side.r, `${tag} 左抽屉 <= 80vw`).toBeLessThanOrEqual(0.8 * w + 1);
    expect(act.r, `${tag} 活动栏在视口内`).toBeLessThanOrEqual(w);
    expect(side.t, `${tag}`).toBeGreaterThanOrEqual(mb.b - 0.5);
    // 最近展开的占整列/整行, 另一个让位: 左抽屉要么在面板头之上, 要么面板头在它右侧
    expect(side.b <= head.t + 0.5 || head.l >= side.r - 0.5, `${tag} 左抽屉与面板头不重叠`).toBe(true);
    expect(body.t, `${tag} 底抽屉够不到菜单栏`).toBeGreaterThanOrEqual(mb.b - 0.5);
    expect(body.t, `${tag} 面板头在正文之上`).toBeGreaterThanOrEqual(head.b - 0.5);
    expect(body.b, `${tag} 正文止于状态栏`).toBeLessThanOrEqual(st.t + 0.5);
    expect(body.w, `${tag} 通栏不超出`).toBeLessThanOrEqual(w + 0.5);
    expect(mb.h, `${tag} 菜单栏不被盖 / 压`).toBeCloseTo(30, 0);
    expect(st.b, `${tag} 状态栏贴底`).toBeCloseTo(h, 0);
    expect(await ownerAt(page, 5, mb.b + 2), `${tag} 细条把手仍可点`).toBe("strip");
  }
  expect(errors).toEqual([]);
});

test("资源管理器: 左抽屉展开时重画文件树 (看得到磁盘上的新文件), 保住已展开的目录", async ({ page, request }) => {
  const errors = await boot(page);
  const root = await serverRoot(request);
  await resetShell();
  await menuCmd(page, "file", "open-folder");
  await fillOpenFolderModal(page, root);
  await page.waitForFunction(() => (document.querySelector("#side-body")?.textContent || "").includes("README.md"), null, { timeout: 8_000 });
  await page.locator('.tree-node:has-text("src")').first().click();
  await page.waitForFunction(() => document.querySelectorAll("#side-body .tree-child").length > 0, null, { timeout: 8_000 });
  const kids = await page.locator("#side-body .tree-child").count();
  const probe = path.join(root, `uat-refresh-probe-${Date.now()}.txt`);
  try {
    fs.writeFileSync(probe, "x");
    await page.locator("#left-strip-btn").click();      // 折叠
    expect(await shown(page, "#side-bar")).toBe(false);
    await page.locator("#left-strip-btn").click();      // 再展开 -> 刷新
    await page.waitForFunction((n) => (document.querySelector("#side-body")?.textContent || "").includes(n), path.basename(probe), { timeout: 8_000 });
    await expect(page.locator("#side-body .tree-child"), "已展开的 src 目录在重画后仍展开").toHaveCount(kids);
    expect(await page.locator("#side-body #explorer-pane").count()).toBe(1);
  } finally {
    fs.rmSync(probe, { force: true });
  }
  expect(errors).toEqual([]);
});

test("预设来回切: 完整布局下区域并排停靠 (不是绝对定位, 没有细条), 回极简又是折叠把手 + 覆盖抽屉", async ({ page }) => {
  const errors = await boot(page);
  await bothOpen(page);
  await page.evaluate(() => (window as any).setLayout("full"));
  const pos = await page.evaluate(() => {
    const p = (s: string) => getComputedStyle(document.querySelector(s)!).position;
    return { act: p("#activity-bar"), side: p("#side-bar"), bottom: p("#bottom-panel"), body: p("#bottom-body"), strip: getComputedStyle(document.getElementById("left-strip")!).display };
  });
  expect(pos.act).not.toBe("absolute");
  expect(pos.side).not.toBe("absolute");
  expect(pos.bottom).not.toBe("absolute");
  expect(pos.body).not.toBe("absolute");
  expect(pos.strip).toBe("none");
  const act = await rectOf(page, "#activity-bar"), side = await rectOf(page, "#side-bar"), area = await rectOf(page, "#editor-area");
  expect(act.l).toBeCloseTo(0, 0);
  expect(side.l).toBeCloseTo(act.r, 0);
  expect(area.l, "完整布局: 编辑区被并排的活动栏 + 侧栏推开").toBeCloseTo(side.r, 0);
  await expectTiled(page, "full docked");
  await page.evaluate(() => (window as any).setLayout("min"));
  expect(await shown(page, "#left-strip")).toBe(true);
  expect(await shown(page, "#side-bar")).toBe(false);
  expect(await shown(page, "#bottom-body")).toBe(false);
  expect(await page.locator("#bottom-panel").getAttribute("data-collapsed")).toBe("true");
  expect((await rectOf(page, "#editor-area")).l).toBeCloseTo(16, 0);
  await expectTiled(page, "back to min");
  expect(errors).toEqual([]);
});

test("Esc 遇到打开的菜单只关菜单, 不连带折叠抽屉 (焦点在抽屉里时也一样)", async ({ page }) => {
  const errors = await boot(page);
  await bothOpen(page);
  await page.locator('.side-tab[data-side-tab="search"]').click();       // 焦点在抽屉里的搜索框
  expect(await page.evaluate(() => document.activeElement!.id)).toBe("search-input");
  await page.keyboard.press("Alt+f");                                     // 打开文件菜单
  expect(await page.locator(".menu-item.open").count()).toBe(1);
  await page.keyboard.press("Escape");
  expect(await page.locator(".menu-item.open").count(), "菜单关了").toBe(0);
  expect(await shown(page, "#side-bar"), "抽屉没被连带折叠").toBe(true);
  expect(await shown(page, "#bottom-body")).toBe(true);
  expect(await page.evaluate(() => document.activeElement!.id), "焦点回到编辑区").toBe("ed-content");
  // 点在抽屉空白处 (焦点落到 body) + 菜单开着 + Esc: 同样只关菜单
  await page.mouse.click(150, (await rectOf(page, "#menubar")).b + 300);
  await page.locator('.menu-item[data-menu="view"] > .mb-btn').click();
  await page.keyboard.press("Escape");
  expect(await page.locator(".menu-item.open").count()).toBe(0);
  expect(await shown(page, "#side-bar")).toBe(true);
  expect(errors).toEqual([]);
});

test("资源管理器: 重画后滚动位置和已展开的目录都还原 (矮窗口, 树超出一屏)", async ({ page, request }) => {
  const errors = await boot(page);
  await page.setViewportSize({ width: 960, height: 260 });
  const root = await serverRoot(request);
  await resetShell();
  await menuCmd(page, "file", "open-folder");
  await fillOpenFolderModal(page, root);
  await page.waitForFunction(() => (document.querySelector("#side-body")?.textContent || "").includes("README.md"), null, { timeout: 8_000 });
  for (const dir of ["docs", "src"]) {
    // 每次点开一个目录, 等子项数量真的增加 (目录是懒加载的; 只等「有子项」会把第二个目录的加载漏掉)
    const n0 = await page.locator("#side-body .tree-child").count();
    await page.locator(`.tree-node.tree-folder:has-text("${dir}")`).first().click();
    await page.waitForFunction((n) => document.querySelectorAll("#side-body .tree-child").length > n, n0, { timeout: 8_000 });
  }
  const before = await page.evaluate(() => {
    const b = document.getElementById("side-body")!;
    b.scrollTop = b.scrollHeight;
    return { st: b.scrollTop, over: b.scrollHeight - b.clientHeight, kids: document.querySelectorAll("#side-body .tree-child").length };
  });
  expect(before.over, "树确实超出一屏").toBeGreaterThan(20);
  expect(before.st).toBeGreaterThan(10);
  await page.locator("#left-strip-btn").click();                          // 折叠
  await page.locator("#left-strip-btn").click();                          // 展开 -> 重画
  // 重画是异步的 (list_dir + 逐个重新展开目录), 还原发生在最后: 等到位置回来, 不要立刻断言
  await page.waitForFunction((st) => Math.abs(document.getElementById("side-body")!.scrollTop - st) <= 1, before.st, { timeout: 8_000 });
  await expect(page.locator("#side-body .tree-child")).toHaveCount(before.kids);
  expect(errors).toEqual([]);
});
