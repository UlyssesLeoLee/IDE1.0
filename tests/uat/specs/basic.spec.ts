// tests/uat/specs/basic.spec.ts
// 基础加载 + mode 切换 + 页面 DOM 集成验证 (10 case).
import { test, expect } from "@playwright/test";
import {
  resetShell,
  getFrame,
  sendKey,
  sectionText,
  waitForMode,
} from "./helpers";

test.beforeEach(async () => {
  await resetShell();
});

test("page loads with shell container and status/history/prompt sections", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByTestId("shell")).toBeVisible();
  await expect(page.getByTestId("status")).toBeVisible();
  await expect(page.getByTestId("history")).toBeVisible();
  await expect(page.getByTestId("prompt")).toBeVisible();
});

test("initial frame is Normal mode with empty buffer", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  const frame = await getFrame();
  expect(frame.mode).toBe("Normal");
  expect(frame.buffer).toBe("");
  expect(frame.cursor).toBe(0);
});

test("status section shows NORMAL badge with blue background", async ({
  page,
}) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  const frame = await getFrame();
  // badge 行 = status[1] (content 行), 含 "NORMAL"
  const badgeRow = frame.status[1];
  const badgeText = badgeRow.map((c) => c.ch).join("").trim();
  expect(badgeText).toContain("NORMAL");
  // 验 background = blue (per web.rs render_status_web)
  const hasBlueBg = badgeRow.some((c) => c.style.bg === "blue");
  expect(hasBlueBg).toBe(true);
});

test("status section shows kernel banner from ide-kernel-core", async ({
  page,
}) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  const frame = await getFrame();
  const text = sectionText(frame.status);
  expect(text).toContain("ide-kernel-core");
  expect(text).toContain("placeholder"); // 当前 kernel_status
});

test("pressing i in Normal mode enters Insert", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  await sendKey("Char:i");
  await waitForMode(page, "Insert");
  const frame = await getFrame();
  expect(frame.mode).toBe("Insert");
});

test("pressing : in Normal mode enters Command", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  await sendKey("Char::");
  await waitForMode(page, "Command");
  const frame = await getFrame();
  expect(frame.mode).toBe("Command");
});

test("pressing Esc in Insert mode returns to Normal", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  await sendKey("Char:i");
  await waitForMode(page, "Insert");
  await sendKey("Esc");
  await waitForMode(page, "Normal");
  const frame = await getFrame();
  expect(frame.mode).toBe("Normal");
});

test("typing chars in Insert mode populates buffer", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  await sendKey("Char:i");
  await waitForMode(page, "Insert");
  for (const c of "abc") {
    await sendKey(`Char:${c}`);
  }
  const frame = await getFrame();
  expect(frame.buffer).toBe("abc");
  expect(frame.cursor).toBe(3);
});

test("prompt prefix changes per mode (▌ / > / :)", async ({ page }) => {
  await page.goto("/");
  // Normal: ▌ (prompt 内容行 = prompt[1], prompt[0] 是边框)
  await waitForMode(page, "Normal");
  let frame = await getFrame();
  let content = frame.prompt[1].map((c) => c.ch).join("");
  expect(content.startsWith("▌")).toBe(true);
  // Insert: >
  await sendKey("Char:i");
  await waitForMode(page, "Insert");
  frame = await getFrame();
  content = frame.prompt[1].map((c) => c.ch).join("");
  expect(content.startsWith(">")).toBe(true);
  // Command: :
  await sendKey("Esc");
  await waitForMode(page, "Normal");
  await sendKey("Char::");
  await waitForMode(page, "Command");
  frame = await getFrame();
  content = frame.prompt[1].map((c) => c.ch).join("");
  expect(content.startsWith(":")).toBe(true);
});

test("DOM data-mode and data-buffer attributes track server state", async ({
  page,
}) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  await sendKey("Char:i");
  await waitForMode(page, "Insert");
  await sendKey("Char:x");
  await sendKey("Char:y");
  await page.waitForFunction(
    () => document.getElementById("shell")?.dataset.buffer === "xy",
    undefined,
    { timeout: 3_000 },
  );
  const modeAttr = await page
    .getByTestId("shell")
    .getAttribute("data-mode");
  expect(modeAttr).toBe("Insert");
});
