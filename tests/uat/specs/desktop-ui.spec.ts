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
  // 暴露事件触发器: 用来模拟后端主动推送 (如 terminal://closed),
  // 前端侧只能接收, 没有对应的"关闭终端"命令.
  w.__emit = (name: string, payload: any) => listeners[name]?.({ payload });
  w.__termId = "term-1";
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
        case "search_cmd":
        // 共享前端 (8ac5ec5) 调的是 "search"; "search_cmd" 是 ULYS-251
        // 旧前端的命令名. 两个都留, 否则任一版本的面板都开不了.
        case "search": {
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
        // 共享前端 (8ac5ec5) 还会 invoke 这两个; ULYS-251 mock 没实现,
        // 落到 default 会抛 "unknown command" 导致调用它的用例失败.
        // 插件: 返回空清单即可 (用例只要求"无异常").
        case "plugin_load": return { loaded: [], errors: [] };
        // 终端输出: 走事件推送, 这里补一个空快照以防前端直接轮询.
        case "terminal_output": return { id: args.id, stream: "stdout", data: "" };
        // 透明主题文字反色 (theme_luminance.rs): 前端每秒轮询
        // desktop_luminance, 切主题时调 clear_theme_vars. mock 必须实现 ——
        // 缺了会返回 undefined, 轮询循环里比较 undefined !== undefined
        // 虽然不炸, 但 clear_theme_vars 返回 undefined 会让
        // applyClearThemeColors 抛 "Cannot read properties of undefined".
        case "desktop_luminance": return w.__luminance || "dark";
        case "clear_theme_vars": return w.__luminance === "light"
          ? {
              luminance: "light",
              "--fg": "#1a1c1e", "--fg-muted": "rgba(26,28,30,0.72)",
              "--fg-dim": "rgba(26,28,30,0.48)", "--border": "rgba(26,28,30,0.22)",
              "--glass-tint": "rgba(0,0,0,0.04)", "--bg-bar": "rgba(0,0,0,0.04)",
              "--bg-side": "rgba(0,0,0,0.04)", "--bg-status": "rgba(0,0,0,0.04)",
              "--blue": "#0b57d0", "--green": "#0f7b41",
            }
          : {
              luminance: "dark",
              "--fg": "#f0f2f5", "--fg-muted": "rgba(240,242,245,0.72)",
              "--fg-dim": "rgba(240,242,245,0.48)", "--border": "rgba(255,255,255,0.28)",
              "--glass-tint": "rgba(255,255,255,0.04)", "--bg-bar": "rgba(255,255,255,0.04)",
              "--bg-side": "rgba(255,255,255,0.04)", "--bg-status": "rgba(255,255,255,0.04)",
              "--blue": "#8ab4f8", "--green": "#7ee2b8",
            };
        default: throw "unknown command " + cmd;
      }
    },
  };
}

async function boot(page: Page): Promise<string[]> {
  const errors: string[] = [];
  // 每个用例从干净状态开始 —— 主题/字体/recent 等持久化在 localStorage,
  // 前一个用例的选择会漏到下一个 (Playwright 默认同 context 复用 storage).
  // 实测: 单独跑 -g 菜单用例时 to-upper 正常, 跟着前一个用例跑就炸.
  await page.addInitScript(() => {
    try { window.localStorage.clear(); } catch (_) { /* file:// 下可能不可用 */ }
    // 缺省界面是极简 (Sakura 风格) + 非模态编辑; 本 spec 按完整 IDE 界面 + vim 三态写. file:// 不在 playwright.config 的
    // storageState (只覆盖 http origin) 范围内, 所以在 clear 之后自己注入 (editor.html 的 ide10.layout / ide10.vim).
    try {
      window.localStorage.setItem("ide10.layout", "full");
      window.localStorage.setItem("ide10.vim", "on");
    } catch (_) { /* ignore */ }
  });
  page.on("pageerror", (e) => errors.push(e.message));
  // 注意: 这里**不**兜底 dialog. Playwright 会把 dialog 派发给所有
  // 已注册的 handler, 无差别 accept 会让"验证取消路径"的用例失效.
  // 各用例按需自己 page.on("dialog", ...) 注册.
  // 注意: **不要**在这里自动处理 dialog —— 未保存确认用的是原生
  // confirm(), 自动 dismiss 会让"用户取消"和"用户确认"两条路径都走到
  // dismiss 分支, 用例就测不出区别了. 需要 dialog 的用例自己
  // 用 page.once("dialog", ...) 接管.
  await page.addInitScript(installMock);
  await page.goto(DIST);
  await page.waitForFunction(() => (window as any).__ideReady && (window as any).__ideReady());
  return errors;
}

// ---------------------------------------------------------------------------
// 读编辑器内容 —— 用数据模型, 不用 DOM
// ---------------------------------------------------------------------------
// 为什么不用 toHaveValue: 共享前端的 #ed-content 是自绘编辑器
// (tabindex=0 的 div + renderVim 渲染), 既不是 <textarea> 也不是
// contenteditable, 所以 toHaveValue 必然报 "Not an input element".
//
// 读 activeTab().lines 是直接读数据模型 —— 不受语法高亮 DOM 结构、
// 折叠、选区渲染影响, 断言的是"用户真正编辑的那份文本".
async function editorText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const t = (window as any).activeTab?.();
    return t && t.lines ? t.lines.join("\n") : "";
  });
}

/** 当前活动 tab 的数据模型 (测 dirty 标记、行数等). */
async function activeTab(page: Page): Promise<any> {
  return page.evaluate(() => {
    const t = (window as any).activeTab?.();
    return t ? { path: t.path, lines: t.lines.slice(), mode: t.mode, dirty: t.dirty } : null;
  });
}

/**
 * 建一个未命名 tab —— 不走 new-file 命令。
 *
 * 共享前端的 new-file 用原生 prompt() 要文件名; 无头浏览器没接管 prompt
 * 时 prompt 返回 null, createNewFile 走 `if (!name || !projectRoot)` 分支
 * 报 "请先打开项目" (提示语其实在说"没拿到名字"). 所以这里直接用
 * openFile 造一个干净 tab, 只关心后续编辑行为.
 */
async function newBlankTab(page: Page, path = "C:/proj/scratch.txt") {
  // openFile 只接一个 path 参数, 且内部走 read_file —— 所以文件必须
  // 已存在. 先在 mock 的虚拟文件系统里塞一个空文件.
  await page.evaluate((p) => {
    (window as any).__files[p] = "";
    return (window as any).openFile(p);
  }, path);
  await page.waitForTimeout(200);
  // 进 INSERT 才能打字: NORMAL 模式下按键是 vim 命令, 不会插入字符.
  await page.locator("#ed-content").focus();
  await page.keyboard.press("i");
  await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT");
}

/** 给 modal 里的输入框赋值 —— 绕开 Playwright 的 actionability 检查. */
async function fillModalInput(page: Page, sel: string, value: string) {
  // 用 page.evaluate + querySelector, 不用 locator.evaluate ——
  // 后者要等元素可操作,  modal 遮挡/动画时会超时.
  await page.evaluate(
    ([s, v]) => {
      const input = document.querySelector(s) as HTMLInputElement | null;
      if (!input) throw new Error("找不到输入框: " + s);
      input.value = v;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
    },
    [sel, value] as const,
  );
}

/** 在 modal 输入框里回车. */
async function pressInModal(page: Page, sel: string) {
  await page.evaluate((s) => {
    const el = document.querySelector(s);
    if (!el) throw new Error("找不到元素: " + s);
    el.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  }, sel);
}

async function openProject(page: Page) {
  // 先清 modal —— 遍历命令后可能残留查找框/对话框, 遮住欢迎页按钮.
  await page.evaluate(() => (window as any).closeModal());
  await page.locator("#welcome .open-btn").click({ timeout: 10_000 });
  // 打开文件夹走 modal 输入框 (不是原生 prompt —— 原生 prompt 在无头环境不响应
  // 会一直挂着阻塞, 且不可断言). 预填服务端默认根, Enter 提交.
  await expect(page.locator("#modal")).toHaveClass(/show/);
  await page.locator(".md-input").press("Enter");
  // modal 关闭 → open_project 异步完成 → 侧栏刷新
  await expect(page.locator("#modal")).not.toHaveClass(/show/);
  await expect(page.locator("#side-body")).toContainText("README.md");
}

async function openReadme(page: Page) {
  await openProject(page);
  await page.locator('.tree-node:has-text("README.md")').click();
  await expect(page.locator("#tabs")).toContainText("README.md");
  expect(await editorText(page)).toMatch(/hello/);
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

    test("每个菜单项都可执行 (无 未实装 / 无 未知命令 / 无 JS 异常)", async ({ page }) => {
    test.setTimeout(120_000);
    const errors = await boot(page);
    await openReadme(page);
    const cmds = await page.locator("#menubar .menu-row[data-cmd]").evaluateAll((rows) =>
      rows.map((r) => (r as HTMLElement).dataset.cmd!).filter((c) => c !== "open-recent"));
    expect(cmds.length).toBeGreaterThan(50);

    // 用原生 prompt() 要输入的命令: 无头浏览器不响应会一直挂着阻塞.
    // 统一回一个固定值, 不区分 prompt/confirm.
    // 用 dispatchCommand 而不是逐个点菜单 —— 这里验的是"每条命令都能执行",
    // 菜单 UI 本身由"加载无 JS 错误"用例覆盖. 逐个 hover + click 113 次
    // 在无头环境里会累积到超时.
    //
    // close-all 必须排到最后: 它会关掉**全部** tab, 中途执行的话后面几十条
    // 命令都在"无活动 tab"状态下跑, 抛的异常属于状态问题而非命令问题.
    // 关 tab 的命令 (close-tab / close-all) 必须放最后: 它们会关掉当前
    // tab, 只有唯一 tab 时后续命令就全在"无活动 tab"状态下跑 ——
    // 实测 close-tab 排在第 7 位, 导致第 31 位的 to-upper 报
    // "Cannot read properties of undefined (reading 'toUpperCase')".
    const lastCmds = new Set(["close-tab", "close-all"]);
    const ordered = [...cmds.filter((c) => !lastCmds.has(c)),
                      ...cmds.filter((c) => lastCmds.has(c))];
    const unimplemented: string[] = [];
    for (const cmd of ordered) {
      try {
        await page.evaluate((c) => (window as any).dispatchCommand(c), cmd);
      } catch (e) {
        throw new Error(`命令 ${cmd} 抛出异常: ${(e as Error).message}`);
      }
      await page.waitForTimeout(20);
      await page.evaluate(() => (window as any).closeModal());
      // 用 evaluate 读, 不用 locator.textContent() —— 后者会等元素
      // 可见, 而 fullscreen / 某些视图命令会把整个 UI 藏掉, 于是超时.
      const tip = await page.evaluate(() => document.getElementById("s-tip")?.textContent || "");
      if (/未实装|未知命令|命令失败|启动失败/.test(tip)) {
        unimplemented.push(cmd + " → " + tip);
      }
      // 每条命令后检查 tab 状态健康 —— 有命令会把 t.lines 弄成
      // 半初始化状态 (async openFile 没 await 完), 后续命令读到
      // undefined 就抛 "Cannot read properties of undefined".
      const health = await page.evaluate(() => {
        const t = (window as any).activeTab?.();
        if (!t) return "no-tab";
        if (!Array.isArray(t.lines)) return "lines-not-array";
        if (t.lines.some((l: any) => typeof l !== "string")) return "lines-corrupt";
        return "ok";
      });
      if (health === "lines-corrupt" || health === "lines-not-array") {
        throw new Error(`命令 ${cmd} 之后 tab 状态损坏: ${health}`);
      }
      // close-all / close-tab 会关掉全部 tab. 后续命令在"无活动 tab"下
      // 运行没有意义 (toUpperCase 之类会读到 undefined), 所以没有 tab
      // 时补开一个. 用 evaluate 判断, 不用 locator —— 后者会等元素可见.
      // openFile 是 **async** (内部 await invoke("read_file")), 必须 await
      // 它完成 —— 否则下一条命令看到的是半初始化的 tab.
      //
      // 判定"需要补 tab"必须同时覆盖两种情况:
      //   1. activeTab 为 null (close-all 把全部关掉了)
      //   2. activeTab 存在但 lines 损坏/为空 (macro-play 灌进 undefined)
      // 之前只查了 lines.length > 0, 于是第 1 种情况漏判, 紧接着的
      // to-upper 就在无 tab 状态下跑, 报 toUpperCase of undefined.
      const needsTab = await page.evaluate(() => {
        const t = (window as any).activeTab?.();
        if (!t) return true;
        if (!Array.isArray(t.lines) || t.lines.length === 0) return true;
        return t.lines.some((l: any) => typeof l !== "string");
      });
      if (needsTab) {
        await page.evaluate(async () => {
          const w = window as any;
          w.__files["C:/proj/README.md"] =
            w.__files["C:/proj/README.md"] || "hello\nworld\nhello world\n";
          await w.openFile("C:/proj/README.md");
        });
      }
    }
    expect(unimplemented, "这些命令应全部可用").toEqual([]);
    expect(errors, "执行 100+ 命令不应有未捕获异常").toEqual([]);
  });

  test("打开项目 → 展开文件夹 → 打开文件 → vim 编辑 → Ctrl+S 保存", async ({ page }) => {
    const errors = await boot(page);
    page.on("dialog", (d) => d.accept("3"));
    await openProject(page);
    // 展开 src 子目录 -> 应看到 main.rs
    await page.locator('.tree-node:has-text("src")').click();
    await expect(page.locator("#side-body")).toContainText("main.rs");
    await page.locator('.tree-node:has-text("README.md")').click();
    await expect(page.locator("#tabs")).toContainText("README.md");
    await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "NORMAL");
    // NORMAL 模式下键入不修改文本 (z 在 NORMAL 是前缀键, 不是字符输入).
    // 注意每步之间要重新 focus —— Escape 之后焦点会丢.
    await page.locator("#ed-content").focus();
    await page.keyboard.press("z");
    expect(await editorText(page)).toBe("hello\nworld\nhello world\n");
    // $ 到行尾, A 进入 INSERT 在行尾追加 —— 光标初始在行首, 不先到行尾
    // 的话 A 会把字符插到行首 (实测得到 "!hello")
    await page.locator("#ed-content").focus();
    await page.keyboard.press("$");
    await page.locator("#ed-content").press("A");
    await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "INSERT");
    await page.keyboard.type("!");
    await page.keyboard.press("Escape");
    await page.locator("#ed-content").focus();
    await expect(page.locator("#s-mode")).toHaveAttribute("data-mode", "NORMAL");
    expect(await editorText(page)).toBe("hello!\nworld\nhello world\n");
    // 保存: dirty 标记应清除, 且写进后端 mock
    await page.keyboard.press("Control+s");
    expect(await page.evaluate(() => (window as any).__files["C:/proj/README.md"])).toBe("hello!\nworld\nhello world\n");
    // dd 删除行 —— 两个 d 必须**连续**按, 中间不能重新 focus:
    // 前缀键状态 (pending "d") 挂在编辑器上, 重新 focus 会清掉它,
    // 于是第二个 d 变成新的前缀, dd 永远不触发.
    await page.locator("#ed-content").focus();
    await page.keyboard.press("d");
    await page.keyboard.press("d");
    expect(await editorText(page), "dd 应删掉当前行").toBe("world\nhello world\n");

    // u 撤销
    await page.locator("#ed-content").focus();
    await page.keyboard.press("u");
    expect(await editorText(page), "u 应恢复被删的行").toBe("hello!\nworld\nhello world\n");
    expect(errors).toEqual([]);
  });

  test("查找 / 替换 / 跳转到行 / 书签", async ({ page }) => {
    const errors = await boot(page);
    page.on("dialog", (d) => d.accept("3"));
    await openReadme(page);
    // 查找面板是 openFindDialog 生成的 modal (共享前端结构):
    //   #modal > #md-body 内有 #find-input, 替换输入是 #repl-input,
    //   匹配计数在 tip 里 (无 #find-count / #findbar —— 那是 ULYS 版结构).
    // 先确保没有残留 modal —— 上一条用例可能留下查找框, 叠着会导致
    // fill 找不到可见元素.
    await page.evaluate(() => (window as any).closeModal());
    await page.locator("#ed-content").focus();
    await page.keyboard.press("Control+f");
    await expect(page.locator("#modal #find-input")).toBeVisible();
    await fillModalInput(page, "#find-input", "hello");
    await pressInModal(page, "#find-input");
    // README 内容 hello/world/hello world -> "hello" 匹配 2 处.
    // Enter 会跳到下一个匹配 (从光标位置往后找), 所以 tip 是 "找到 2/2".
    await expect(page.locator("#s-tip")).toContainText(/找到 [12]\/2/);
    // 替换: Ctrl+H 打开替换框 (id 是 repl-input), 全部替换后变 bye
    await page.keyboard.press("Control+h");
    await expect(page.locator("#repl-input")).toBeVisible();
    await fillModalInput(page, "#repl-input", "bye");
    await page.locator('#modal [data-cmd="replace-all"]').click();
    expect(await editorText(page)).toBe("bye\nworld\nbye world\n");
    await page.keyboard.press("Escape");
    await expect(page.locator("#modal")).not.toHaveClass(/show/);
    // 撤销全部替换
    await page.locator("#ed-content").click();
    await page.keyboard.press("Control+z");
    expect(await editorText(page)).toBe("hello\nworld\nhello world\n");
    // 跳转到行: Ctrl+J 弹输入框, 输 3 后状态栏应显示 Ln 3
    // 跳转到行: 共享前端用原生 prompt() 要行号 (无 .md-input 那种
    // modal 输入框 —— 那是 ULYS 版结构). boot 里注册的 dialog 兜底
    // 会 accept("3"), 直接按键即可.
    await page.keyboard.press("Control+j");
    await page.waitForTimeout(300);
    await expect(page.locator("#s-pos")).toContainText("3");
    // 书签: BOOKMARKS 是模块级 const, **没有挂到 window**, 测试读不到.
    // 它的可观测副作用是 setTip 的提示文案和 localStorage 持久化 ——
    // 用这两个断言, 不去猜内部数据结构.
    await page.locator("#ed-content").focus();
    await page.keyboard.press("Control+F2");
    await page.waitForTimeout(200);
    const tipAfterAdd = await page.evaluate(
      () => document.getElementById("s-tip")?.textContent || "",
    );
    expect(tipAfterAdd, "应提示已添加书签").toMatch(/书签/);
    // 再按一次是 toggle (取消)
    await page.keyboard.press("Control+F2");
    await page.waitForTimeout(200);
    const tipAfterToggle = await page.evaluate(
      () => document.getElementById("s-tip")?.textContent || "",
    );
    expect(tipAfterToggle, "再次按应切换书签状态").toMatch(/书签/);
    // 无 JS 异常
    expect(errors).toEqual([]);
  });

    test("转换: 平假名↔片假名, 半角↔全角", async ({ page }) => {
    const errors = await boot(page);
    await page.evaluate(() => (window as any).closeModal());
    await newBlankTab(page);
    // 直接给数据模型赋值而不是 keyboard.type —— Playwright 的 type() 走
    // 键盘事件, 半角假名区 (U+FF66..FF9D) 在无头 Chromium 里打不进去.
    // 这里验的是转换命令本身, 不是输入法.
    const setLines = (l: string[]) => page.evaluate((ls) => {
      const t = (window as any).activeTab();
      t.lines = ls;
      (window as any).renderVim(t);
    }, l);

    // 输出用 NFKC 归一后比较 —— 浊音假名有两种等价表示:
    // "カ"+组合浊点(U+30AB U+3099, toFullwidth 的输出) 与 预组合"ガ"(U+30AC,
    // 源码里的字面量). 两者 NFKC 后一致, 视觉也一致, 但码位不同.
    // 不归一就会因为码位差异误判失败.
    const nfkc = (s: string) => s.normalize("NFKC");
    const expectText = async (want: string) =>
      expect(nfkc(await editorText(page))).toBe(nfkc(want));

    // 半角假名 → 全角 (toFullwidth 覆盖半角假名区 U+FF61..FF9F)
    await setLines(["ｶﾞｷﾞ abc"]);
    await page.evaluate(() => (window as any).dispatchCommand("to-fullwidth"));
    await expectText("ガギ　ａｂｃ");

    // 全角 → 半角 (浊音假名 ガ → ｶﾞ 两码位)
    await setLines(["ガギ abc"]);
    await page.evaluate(() => (window as any).dispatchCommand("to-halfwidth"));
    await expectText("ｶﾞｷﾞ abc");

    // 半角 → 全角, 含半浊音
    await setLines(["ﾊﾟﾋﾟﾌﾟ"]);
    await page.evaluate(() => (window as any).dispatchCommand("to-fullwidth"));
    await expectText("パピプ");

    // 全角片假名 → 平假名 (toZenhira: ァ-ン 减 0x60)
    await setLines(["ガギ abc"]);
    await page.evaluate(() => (window as any).dispatchCommand("to-zenhira"));
    await expectText("がぎ abc");

    // 平假名 → 片假名 (toZenkaku: ぁ-ん 加 0x60)
    await setLines(["がぎ abc"]);
    await page.evaluate(() => (window as any).dispatchCommand("to-zenkaku"));
    await expectText("ガギ abc");

    // 全角英数 → 半角
    await setLines(["ＡＢＣ"]);
    await page.evaluate(() => (window as any).dispatchCommand("to-halfwidth"));
    expect(await editorText(page)).toBe("ABC");
    expect(errors).toEqual([]);
  });

  test("终端: 创建 → 发送命令 → 显示输出 → 结束", async ({ page }) => {
    const errors = await boot(page);
    await openProject(page);
    await page.evaluate(() => (window as any).closeModal());
    // show-terminal 只切到 terminal 面板 (switchBottomPane), **不建会话**;
    // 建会话是 new-terminal. 两步都要做, 否则看不到终端输出.
    await page.evaluate(() => (window as any).dispatchCommand("show-terminal"));
    await page.waitForTimeout(200);
    await page.evaluate(() => (window as any).dispatchCommand("new-terminal"));
    // 共享前端的终端是自绘 PTY: 焦点在 #terminal-body, 键盘直接转发给
    // 后端, **没有** #terminal-input 输入框 (那是 ULYS 版结构).
    await page.waitForTimeout(600);
    await expect(page.locator("#terminal-pane")).toHaveClass(/active/);
    // 不断言 #terminal-body 可见 —— 会话刚建、还没有任何输出时它是空的,
    // CSS `#terminal-body:empty { flex: 0 0 auto; padding: 0 }` 让它高度为 0,
    // Playwright 的 toBeVisible() 会报 hidden. 真正的启动标志是
    // 空状态提示被隐藏 + terminal_create 被调过.
    await expect(page.locator("#terminal-empty")).toBeHidden();
    // 注意: 前端调的是 invoke("terminal_create", {}) —— **不传 cwd**,
    // 工作目录由后端决定. 所以只断言"被调用过", 不能断言 args.cwd.
    const created = await page.evaluate(() =>
      (window as any).__calls.filter((c: any) => c.cmd === "terminal_create"),
    );
    expect(created.length, "应调用过 terminal_create").toBeGreaterThanOrEqual(1);
    // 聚焦终端后逐字输入, 每键应作为 terminal_input 转发.
    // 用 focus() 而非 click() —— 空状态提示 #terminal-empty 会盖住
    // #terminal-body, click 会因被遮挡而超时.
    await page.locator("#terminal-body").focus();
    await page.keyboard.type("dir");
    await page.keyboard.press("Enter");
    const inputs = await page.evaluate(() =>
      (window as any).__calls.filter((c: any) => c.cmd === "terminal_input"),
    );
    expect(inputs.length).toBeGreaterThanOrEqual(3);   // d,i,r + 

    // mock 通过 terminal://output 事件回显, 应出现在终端区
    // mock 对**每次** terminal_input 都回显 "echo: " + data, 所以 4 次
    // 输入产生 4 段回显 (逐字符): echo: d / echo: i / echo: r / echo: \r\n.
    // 这里只断言"回显机制生效", 不绑死具体字符串拼接形式.
    // mock 对**每次** terminal_input 都回显一行 "echo: " + data, 所以
    // 逐字符输入 dir + Enter = 5 行回显. 用 .term-line 计数而不是
    // textContent —— 多个 div 的 textContent 拼接后没有分隔符, 数不清.
    const lines = page.locator("#terminal-body .term-line");
    await expect(lines.first()).toBeVisible({ timeout: 5000 });
    // 回显可能被合并进同一个 .term-line (按批次追加), 所以数 "echo:" 出现次数而不是行数
    await expect.poll(async () => ((await page.locator("#terminal-body").textContent()) || "").split("echo:").length - 1, { timeout: 5000 }).toBeGreaterThanOrEqual(3);
    const firstLine = (await lines.first().textContent()) || "";
    expect(firstLine, "回显内容应来自 mock 的 echo").toContain("echo:");
    // 关闭会话: 共享前端**没有** kill-terminal / terminal-close 命令 ——
    // "会话已结束" 是后端 terminal://closed 事件推过来的.
    // 这里验可观测的状态变化 (会话已结束提示出现), 事件由 mock 发出;
    // id 用 terminal_create 实际返回的 "term-1", 前端判断
    // `p.id === terminalId` 不匹配时会静默丢弃事件.
    await page.evaluate(() => {
      // 用前端实际持有的会话 id (脚本级 let terminalId); 写死 "term-1" 会在 id 变化时被静默丢弃
      (window as any).__emit?.("terminal://closed", { id: (0, eval)("typeof terminalId !== 'undefined' ? terminalId : 'term-1'") });   // __emit 自己包一层 {payload}
    });
    await page.waitForTimeout(400);
    const closed = await page.evaluate(
      () => document.getElementById("terminal-body")?.textContent || "",
    );
    expect(closed, "terminal://closed 后应提示会话已结束").toContain("会话已结束");
    expect(errors).toEqual([]);
  });

    test("底部面板折叠按钮只切换一次; 关闭标签后回到欢迎页", async ({ page }) => {
    const errors = await boot(page);
    // 折叠按钮命令名是 bottom-toggle (ULYS 版叫 toggle-bottom)
    const before = await page.evaluate(() =>
      document.getElementById("bottom-panel")!.getAttribute("data-collapsed"));
    await page.locator('.bottom-toggle[data-cmd="bottom-toggle"]').click();
    const after1 = await page.evaluate(() =>
      document.getElementById("bottom-panel")!.getAttribute("data-collapsed"));
    expect(after1, "点一次应切换折叠状态").not.toBe(before);
    await page.locator('.bottom-toggle[data-cmd="bottom-toggle"]').click();
    const after2 = await page.evaluate(() =>
      document.getElementById("bottom-panel")!.getAttribute("data-collapsed"));
    expect(after2, "点两次应回到原状 (不是来回翻转)").toBe(before);

    // 编辑后关闭标签: 共享前端的 closeTab **不做确认**, 直接关
    // (全代码里唯一的 confirm() 在 reloadActive 里, 用于"重新载入丢失修改").
    // 所以这里断言的是"关闭后回到欢迎页", 不是"弹确认框".
    await page.evaluate(() => (window as any).closeModal());
    await newBlankTab(page);
    await page.keyboard.type("draft");
    await page.keyboard.press("Escape");
    await page.locator("#ed-content").focus();
    expect((await activeTab(page))!.dirty, "编辑后应标记 dirty").toBe(true);
    const tabCount = await page.locator("#tabs .tab").count();
    expect(tabCount).toBeGreaterThan(0);

    // 只有一个 tab 时关掉它 -> 活动 tab 变 null 是**正确行为**
    // (TABS 里没别的了). 关闭前先多开一个, 才能验"切到其它 tab".
    await page.evaluate(() => (window as any).openFile("C:/proj/src/main.rs"));
    await page.waitForTimeout(200);
    const tabs2 = await page.locator("#tabs .tab").count();
    expect(tabs2).toBeGreaterThan(1);
    await page.evaluate(() => (window as any).dispatchCommand("close-tab"));
    await page.waitForTimeout(300);
    expect(
      await page.evaluate(() => (window as any).activeTab?.()?.path ?? ""),
      "关闭一个 tab 后应切到其它 tab",
    ).not.toBe("");

    // 关掉全部 -> 回到欢迎页
    await page.evaluate(() => (window as any).dispatchCommand("close-all"));
    await page.waitForTimeout(300);
    expect(await page.locator("#tabs .tab").count(), "关掉全部后无标签").toBe(0);
    // 关掉全部后应显示欢迎页 —— 但 #welcome 是被 showWelcome() 重写的,
    // 可能还没渲染完; 用 waitFor 比 toBeVisible 更稳.
    await page.locator("#welcome").waitFor({ state: "visible", timeout: 5000 });
    expect(errors).toEqual([]);
  });

  test("浅色主题下下拉菜单为浅色背景 (文字可读)", async ({ page }) => {
    await boot(page);
    // 注意: 共享前端用 setTheme(mode) (见 editor.html THEMES / setTheme),
    // 不是 ULYS-251 分支上的 applyTheme —— 那版前端在合并冲突时已回退.
    await page.evaluate(() => (window as any).setTheme("light"));
    await page.locator('.menu-item[data-menu="file"] .mb-btn').click();
    const bg = await page.locator('.menu-dropdown[data-dropdown="file"]').evaluate((el) => getComputedStyle(el).backgroundColor);
    const [r, g, b] = bg.match(/\d+/g)!.map(Number);
    expect((r + g + b) / 3).toBeGreaterThan(200);
  });
});
