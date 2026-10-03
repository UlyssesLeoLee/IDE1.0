// tests/uat/specs/vim-mode.spec.ts
// Vim 键位覆盖 (8 case): hjkl / 0$ / x / Backspace / Delete / cursor clamp.
import { test, expect } from "@playwright/test";
import { resetShell, getFrame, sendKey, waitForMode } from "./helpers";

test.beforeEach(async () => {
  await resetShell();
});

test("hjkl moves cursor in Normal mode over a pre-typed buffer", async () => {
  // 先 Insert 输入 "hello", 回 Normal
  await sendKey("Char:i");
  for (const c of "hello") await sendKey(`Char:${c}`);
  await sendKey("Esc");
  expect((await getFrame()).mode).toBe("Normal");
  // Normal: h 左移
  let frame = await getFrame();
  expect(frame.cursor).toBe(5);
  await sendKey("Char:h");
  frame = await getFrame();
  expect(frame.cursor).toBe(4);
  await sendKey("Char:l");
  frame = await getFrame();
  expect(frame.cursor).toBe(5);
});

test("arrow keys move cursor in Normal mode", async () => {
  await sendKey("Char:i");
  for (const c of "abcd") await sendKey(`Char:${c}`);
  await sendKey("Esc");
  await sendKey("Left");
  let frame = await getFrame();
  expect(frame.cursor).toBe(3);
  await sendKey("Right");
  frame = await getFrame();
  expect(frame.cursor).toBe(4);
});

test("0 moves cursor to line start, $ to line end", async () => {
  await sendKey("Char:i");
  for (const c of "abcdef") await sendKey(`Char:${c}`);
  await sendKey("Esc");
  await sendKey("Char:0");
  let frame = await getFrame();
  expect(frame.cursor).toBe(0);
  await sendKey("Char:$");
  frame = await getFrame();
  expect(frame.cursor).toBe(6);
});

test("Home / End keys also move cursor", async () => {
  await sendKey("Char:i");
  for (const c of "xyz") await sendKey(`Char:${c}`);
  await sendKey("Esc");
  await sendKey("Home");
  expect((await getFrame()).cursor).toBe(0);
  await sendKey("End");
  expect((await getFrame()).cursor).toBe(3);
});

test("x deletes char right of cursor in Normal mode", async () => {
  await sendKey("Char:i");
  for (const c of "abc") await sendKey(`Char:${c}`);
  await sendKey("Esc");
  await sendKey("Char:0"); // cursor=0
  await sendKey("Char:x"); // 删 'a'
  const frame = await getFrame();
  expect(frame.buffer).toBe("bc");
  expect(frame.cursor).toBe(0);
});

test("Backspace deletes left char in Insert mode", async () => {
  await sendKey("Char:i");
  for (const c of "abc") await sendKey(`Char:${c}`);
  await sendKey("Backspace");
  const frame = await getFrame();
  expect(frame.buffer).toBe("ab");
  expect(frame.cursor).toBe(2);
});

test("Backspace at buffer start is no-op (no crash)", async () => {
  await sendKey("Char:i");
  await sendKey("Backspace");
  await sendKey("Backspace");
  const frame = await getFrame();
  expect(frame.buffer).toBe("");
  expect(frame.cursor).toBe(0);
  expect(frame.mode).toBe("Insert");
});

test("cursor clamps at buffer end on move right", async () => {
  await sendKey("Char:i");
  for (const c of "ab") await sendKey(`Char:${c}`);
  // Insert mode Right x5
  for (let i = 0; i < 5; i++) await sendKey("Right");
  const frame = await getFrame();
  expect(frame.cursor).toBe(2); // len("ab")
});
