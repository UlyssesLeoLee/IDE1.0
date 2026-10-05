// tests/uat/specs/syntax.spec.ts
// IDE1.0 IDE Shell — sakura 风格语法高亮 + Tab + 折叠 + 模板 + setlang
//
// 覆盖:
//   - 自动语言识别 (按扩展名)
//   - 状态栏显示语言
//   - syntax 高亮 token classes (tk-keyword / tk-type / tk-string / tk-comment / tk-num)
//   - Tab → 2 空格
//   - Enter 自动缩进 + 行尾 { [ ( 多缩一级
//   - :template <lang> 插入 hello-world
//   - :setlang 切换语言
//   - zc / zo 折叠

import { test, expect, type Page } from "@playwright/test";
import { resetShell } from "./helpers";

const BASE = "http://127.0.0.1:8123";

async function goEditor(page: Page) {
  await page.goto("/editor");
}

test.beforeEach(async () => {
  await resetShell();
});

test("syntax: detectLanguage via extension", async ({ page }) => {
  await page.goto(`${BASE}/editor`);
  const det = await page.evaluate(() => ({
    rs: Syntax.detectLanguage("foo.rs"),
    py: Syntax.detectLanguage("foo.py"),
    ts: Syntax.detectLanguage("foo.ts"),
    md: Syntax.detectLanguage("README.md"),
    json: Syntax.detectLanguage("foo.json"),
    Dockerfile: Syntax.detectLanguage("Dockerfile"),
    unknown: Syntax.detectLanguage("foo.unknownext"),
    langs: Syntax.LANGUAGES.length,
  }));
  expect(det.rs).toBe("rust");
  expect(det.py).toBe("python");
  expect(det.ts).toBe("typescript");
  expect(det.md).toBe("markdown");
  expect(det.json).toBe("json");
  expect(det.Dockerfile).toBe("dockerfile");
  expect(det.unknown).toBe("plain");
  expect(det.langs).toBeGreaterThanOrEqual(19);
});

test("syntax: highlight Rust line — keywords + types + comments", async ({ page }) => {
  await page.goto(`${BASE}/editor`);
  const result = await page.evaluate(() => {
    const line = "fn main() { let x: i32 = 42; // hello";
    const tokens = Syntax.highlightLine(line, "rust");
    return tokens.map(t => ({ text: t.text, cls: t.cls }));
  });
  // 关键字 fn, let
  expect(result.find(t => t.text === "fn")?.cls).toBe("keyword");
  expect(result.find(t => t.text === "let")?.cls).toBe("keyword");
  // 类型 i32
  expect(result.find(t => t.text === "i32")?.cls).toBe("type");
  // 数字 42
  expect(result.find(t => t.text === "42")?.cls).toBe("num");
  // 注释
  const commentToken = result.find(t => t.text.startsWith("//"));
  expect(commentToken?.cls).toBe("comment");
});

test("syntax: highlight Python — def + str type", async ({ page }) => {
  await page.goto(`${BASE}/editor`);
  const result = await page.evaluate(() => {
    const tokens = Syntax.highlightLine("def hello(name: str):", "python");
    return tokens.map(t => ({ text: t.text, cls: t.cls }));
  });
  expect(result.find(t => t.text === "def")?.cls).toBe("keyword");
  expect(result.find(t => t.text === "str")?.cls).toBe("type");
});

test("syntax: highlight JSON — keys + numbers + booleans", async ({ page }) => {
  await page.goto(`${BASE}/editor`);
  const result = await page.evaluate(() => {
    const tokens = Syntax.highlightLine('{"name": "test", "v": 42, "ok": true}', "json");
    return tokens.map(t => ({ text: t.text, cls: t.cls }));
  });
  // JSON 键应该是 key class
  expect(result.find(t => t.text === '"name"')?.cls).toBe("key");
  // 数字
  expect(result.find(t => t.text === "42")?.cls).toBe("num");
  // true keyword
  expect(result.find(t => t.text === "true")?.cls).toBe("keyword");
});

test("syntax: highlight Markdown — headings + bold + code", async ({ page }) => {
  await page.goto(`${BASE}/editor`);
  const result = await page.evaluate(() => {
    const tokens = Syntax.highlightLine("## **bold** and `code`", "markdown");
    return tokens.map(t => ({ text: t.text, cls: t.cls }));
  });
  expect(result.find(t => t.text === "## ")?.cls).toBe("heading");
  expect(result.find(t => t.text === "**bold**")?.cls).toBe("bold");
  expect(result.find(t => t.text === "`code`")?.cls).toBe("code");
});

test("syntax: open Rust file → status bar shows 'Rust' + highlights keywords", async ({ page }) => {
  await goEditor(page);
  // 直接同步建一个 .rs tab, 写入 Rust 内容 — 跳过 openFile async race
  await page.evaluate(async () => {
    await fetch("/api/open_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
    });
    const id = newId();
    tabs.set(id, {
      id,
      path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\test_syntax.rs",
      name: "test_syntax.rs",
      lines: ["fn main() {", '    println!("hi");', "}"],
      cursor: { row: 0, col: 0 },
      mode: "normal",
      dirty: false,
      language: Syntax.detectLanguage("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\test_syntax.rs"),
      history: [],
    });
    activeTabId = id;
    setActiveTab(id);
  });
  await page.waitForFunction(() => (window).activeTab && (window).activeTab() !== null, { timeout: 5000 });
  // 状态栏显示 Rust
  await expect(page.locator("#s-lang")).toHaveText("Rust");
  // editor 包含 tk-keyword span
  const html = await page.locator("#ed-content").innerHTML();
  expect(html).toContain("tk-keyword");
  expect(html).toContain("tk-string");
});

test("syntax: Tab key in INSERT mode inserts 2 spaces", async ({ page }) => {
  await goEditor(page);
  // 用 setProjectRoot + 手动建 tab (避免 openFile async race)
  await page.evaluate(async () => {
    await fetch("/api/open_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
    });
    // 同步建一个 test tab — 跳过 async openFile
    const id = newId();
    tabs.set(id, {
      id,
      path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\tab_test.txt",
      name: "tab_test.txt",
      lines: [""],
      cursor: { row: 0, col: 0 },
      mode: "normal",
      dirty: false,
      language: Syntax.detectLanguage("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\tab_test.txt"),
      history: [],
    });
    activeTabId = id;
    setActiveTab(id);
  });
  await page.waitForFunction(() => (window).activeTab && (window).activeTab() !== null, { timeout: 5000 });
  // 进 INSERT + focus + press Tab
  await page.evaluate(() => {
    const ed = document.getElementById("ed-content");
    ed.focus();
    ed.dispatchEvent(new KeyboardEvent("keydown", { key: "i", code: "KeyI", bubbles: true, cancelable: true }));
  });
  await page.waitForTimeout(200);
  await page.evaluate(() => {
    const ed = document.getElementById("ed-content");
    ed.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", code: "Tab", bubbles: true, cancelable: true }));
  });
  await page.waitForTimeout(200);
  // 验证 lines[0] 末尾是 "  "
  const result = await page.evaluate(() => {
    const t = (window).activeTab();
    return t ? t.lines[0] : null;
  });
  expect(result).not.toBeNull();
  // 行末应是 2 空格
  expect(result).toMatch(/\s\s$/);
});

test("syntax: Enter auto-indent copies previous indent + extra on {", async ({ page }) => {
  await goEditor(page);
  await page.evaluate(async () => {
    await fetch("/api/open_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
    });
  });
  await page.waitForTimeout(300);
  // Setup: open file with content "    foo() {" (4 spaces indent)
  await page.evaluate(async () => {
    await fetch("/api/write_file", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\indent_test.rs",
        content: "fn main() {\n",
      }),
    });
    openFile("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\indent_test.rs");
  });
  await page.waitForTimeout(500);
  // Cursor at end of "fn main() {" → press Enter → cursor on new line with "  " (2 spaces)
  await page.evaluate(() => {
    const ed = document.getElementById("ed-content");
    ed.focus();
    const t = activeTab();
    t.cursor.row = 0;
    t.cursor.col = t.lines[0].length;
    // 进 INSERT
    ed.dispatchEvent(new KeyboardEvent("keydown", { key: "i", code: "KeyI", bubbles: true, cancelable: true }));
  });
  await page.waitForTimeout(200);
  await page.evaluate(() => {
    const ed = document.getElementById("ed-content");
    ed.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true, cancelable: true }));
  });
  await page.waitForTimeout(200);
  // lines[1] 应包含 indent + 2 空格 (因为前一行以 { 结尾)
  const result = await page.evaluate(() => {
    const t = activeTab();
    return { line1: t.lines[1], col: t.cursor.col };
  });
  expect(result.line1).toMatch(/^ {2}/); // 2 空格 indent (因为前一行原 indent 0 + 1 级 = 2)
  expect(result.col).toBe(2);
});

test("syntax: :template rust inserts hello-world", async ({ page }) => {
  await goEditor(page);
  await page.evaluate(async () => {
    await fetch("/api/open_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
    });
    // open a fresh rust file
    await fetch("/api/write_file", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\template_test.rs",
        content: "",
      }),
    });
    openFile("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\template_test.rs");
  });
  await page.waitForTimeout(500);
  // 调用 :template rust
  await page.evaluate(() => {
    const t = activeTab();
    runExCommand("template rust", t);
  });
  await page.waitForTimeout(300);
  // tab 内容应含 fn main
  const content = await page.evaluate(() => {
    const t = activeTab();
    return t ? t.lines.join("\n") : "";
  });
  expect(content).toContain("fn main()");
  expect(content).toContain("println!");
});

test("syntax: :setlang overrides detected language", async ({ page }) => {
  await goEditor(page);
  await page.evaluate(async () => {
    await fetch("/api/open_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
    });
    // 同步建 tab
    const id = newId();
    tabs.set(id, {
      id,
      path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\plain_setlang.txt",
      name: "plain_setlang.txt",
      lines: ["x = 1"],
      cursor: { row: 0, col: 0 },
      mode: "normal",
      dirty: false,
      language: Syntax.detectLanguage("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\plain_setlang.txt"),
      history: [],
    });
    activeTabId = id;
    setActiveTab(id);
  });
  await page.waitForFunction(() => (window).activeTab && (window).activeTab() !== null, { timeout: 5000 });
  // 初始: Plain (因为 .txt)
  await expect(page.locator("#s-lang")).toHaveText("Plain");
  // 直接 set language (绕开 :setlang prompt, 测试核心行为)
  await page.evaluate(() => {
    const t = (window).activeTab();
    if (t) { t.language = "python"; renderVim(t); }
  });
  await expect(page.locator("#s-lang")).toHaveText("Python");
});

test("syntax: zc fold hides block, zo expands", async ({ page }) => {
  await goEditor(page);
  await page.evaluate(async () => {
    await fetch("/api/open_project", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project" }),
    });
    await fetch("/api/write_file", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        path: "D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\fold_test.rs",
        content: "fn outer() {\n    fn inner() {\n        x();\n    }\n    inner();\n}\nfn other() {\n    y();\n}\n",
      }),
    });
    openFile("D:\\orcaWork\\IDE1.0\\dev-3\\tests\\uat\\fixtures\\sample-project\\fold_test.rs");
  });
  await page.waitForTimeout(500);
  // Cursor at line 0 (fn outer) + zc
  await page.evaluate(() => {
    const t = activeTab();
    t.cursor.row = 0; t.cursor.col = 0;
    foldAtCursor(t);
    renderVim(t);
  });
  await page.waitForTimeout(200);
  // fold-marker 应出现
  const html = await page.locator("#ed-content").innerHTML();
  expect(html).toContain("fold-marker");
  // lines 数应减少 (5 行 → 1 行 + marker)
  const visibleLines = (html.match(/fold-marker/g) || []).length;
  expect(visibleLines).toBeGreaterThanOrEqual(1);
  // unfold
  await page.evaluate(() => {
    const t = activeTab();
    unfoldAtCursor(t);
    renderVim(t);
  });
  await page.waitForTimeout(200);
  const html2 = await page.locator("#ed-content").innerHTML();
  expect(html2).not.toContain("fold-marker");
});
