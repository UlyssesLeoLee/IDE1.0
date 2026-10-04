// tests/uat/specs/mouse.spec.ts
// 鼠标事件覆盖 (5 case): 左键 Insert / 右键 Normal / API mouse / DOM mousedown / contextmenu 禁用.
import { test, expect } from "@playwright/test";
import { resetShell, getFrame, sendMouse, waitForMode } from "./helpers";

test.beforeEach(async () => {
  await resetShell();
});

test("left click via API enters Insert mode", async () => {
  await sendMouse("Left");
  const frame = await getFrame();
  expect(frame.mode).toBe("Insert");
});

test("right click via API enters Normal mode", async () => {
  // 先左键到 Insert
  await sendMouse("Left");
  expect((await getFrame()).mode).toBe("Insert");
  await sendMouse("Right");
  expect((await getFrame()).mode).toBe("Normal");
});

test("DOM mousedown (left) on shell switches to Insert", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  await page.getByTestId("shell").click({ button: "left" });
  await waitForMode(page, "Insert");
});

test("DOM mousedown (right) on shell switches to Normal", async ({ page }) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  // 先左键 Insert
  await page.getByTestId("shell").click({ button: "left" });
  await waitForMode(page, "Insert");
  // 右键回 Normal
  await page.getByTestId("shell").click({ button: "right" });
  await waitForMode(page, "Normal");
});

test("contextmenu (right-click menu) is suppressed on page", async ({
  page,
}) => {
  await page.goto("/");
  await waitForMode(page, "Normal");
  // 监听 contextmenu 是否 defaultPrevented
  const prevented = await page.evaluate(async () => {
    const ev = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      button: 2,
    });
    document.dispatchEvent(ev);
    return ev.defaultPrevented;
  });
  expect(prevented).toBe(true);
});
