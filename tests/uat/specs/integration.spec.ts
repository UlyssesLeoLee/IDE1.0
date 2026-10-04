// tests/uat/specs/integration.spec.ts
// 端到端集成场景 (6 case): 模拟真实用户完整操作流, 跨 mode/mouse/command/AI.
import { test, expect } from "@playwright/test";
import {
  resetShell,
  getFrame,
  sendKey,
  sendMouse,
  sectionText,
  waitForMode,
} from "./helpers";

test.beforeEach(async () => {
  await resetShell();
});

test("full vim workflow: Normal → Insert type → Esc → Command version → history", async () => {
  // 1. Normal 起步
  let frame = await getFrame();
  expect(frame.mode).toBe("Normal");

  // 2. i → Insert, 输入命令文本
  await sendKey("Char:i");
  for (const c of "Get-ChildItem") await sendKey(`Char:${c}`);
  frame = await getFrame();
  expect(frame.mode).toBe("Insert");
  expect(frame.buffer).toBe("Get-ChildItem");

  // 3. Esc → Normal (buffer 保留)
  await sendKey("Esc");
  frame = await getFrame();
  expect(frame.mode).toBe("Normal");
  expect(frame.buffer).toBe("Get-ChildItem");

  // 4. : → Command, 输 version, Enter → history 有 kernel banner
  await sendKey("Char::");
  for (const c of "version") await sendKey(`Char:${c}`);
  await sendKey("Enter");
  frame = await getFrame();
  expect(frame.mode).toBe("Normal");
  expect(sectionText(frame.history)).toContain("ide-kernel-core");
});

test("mouse-driven workflow: left click → type → right click → verify Normal", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");

  // 左键 → Insert
  await page.getByTestId("shell").click({ button: "left" });
  await waitForMode(page, "Insert");

  // 打字 (走 DOM keydown → server)
  await page.keyboard.type("pwd");
  await page.waitForFunction(
    () => document.getElementById("shell")?.dataset.buffer === "pwd",
    undefined,
    { timeout: 3_000 },
  );

  // 右键 → Normal
  await page.getByTestId("shell").click({ button: "right" });
  await waitForMode(page, "Normal");

  const frame = await getFrame();
  expect(frame.buffer).toBe("pwd");
});

test("AI suggestion flow: type → :ai → verify echo → buffer intact", async () => {
  await sendKey("Char:i");
  for (const c of "Select-Object") await sendKey(`Char:${c}`);
  await sendKey("Esc");

  await sendKey("Char::");
  for (const c of "ai") await sendKey(`Char:${c}`);
  await sendKey("Enter");

  const frame = await getFrame();
  expect(sectionText(frame.history)).toContain("AI suggestion: Select-Object");
  expect(frame.buffer).toBe("Select-Object");
});

test("cursor navigation after typing: Home → x deletes first char", async () => {
  await sendKey("Char:i");
  for (const c of "abcdef") await sendKey(`Char:${c}`);
  await sendKey("Esc");
  // Normal: Home → x
  await sendKey("Home");
  let frame = await getFrame();
  expect(frame.cursor).toBe(0);
  await sendKey("Char:x");
  frame = await getFrame();
  expect(frame.buffer).toBe("bcdef");
});

test("history accumulates across multiple commands", async () => {
  for (const cmd of ["version", "help", "ai"]) {
    await sendKey("Char::");
    for (const c of cmd) await sendKey(`Char:${c}`);
    await sendKey("Enter");
  }
  const frame = await getFrame();
  const hist = sectionText(frame.history);
  expect(hist).toContain("ide-kernel-core"); // version
  expect(hist).toContain("IDE Shell demo"); // help
  expect(hist).toContain("<no ai suggestion>"); // ai (buffer 空)
});

test("reset endpoint restores initial state", async () => {
  // 弄脏状态
  await sendKey("Char:i");
  for (const c of "junk") await sendKey(`Char:${c}`);
  await sendKey("Char::");
  for (const c of "version") await sendKey(`Char:${c}`);
  await sendKey("Enter");

  // reset
  await resetShell();
  const frame = await getFrame();
  expect(frame.mode).toBe("Normal");
  expect(frame.buffer).toBe("");
  expect(sectionText(frame.history)).not.toContain("ide-kernel-core");
});
