// tests/uat/specs/helpers.ts
// IDE Shell e2e helper — 封装 API 调用 + 常用断言.
//
// 设计: 单 server 共享状态 (single-user demo), 每个 case beforeEach 调 resetShell()
// 保证隔离.
import type { Page } from "@playwright/test";

const BASE = "http://127.0.0.1:8123";

export interface WebStyle {
  weight?: string;
  fg?: string;
  bg?: string;
}

export interface WebCell {
  ch: string;
  style: WebStyle;
}

export interface WebFrame {
  status: WebCell[][];
  history: WebCell[][];
  prompt: WebCell[][];
  mode: "Normal" | "Insert" | "Command";
  buffer: string;
  cursor: number;
}

/** 重置 server 状态 (Normal + 空 buffer + 空 history). */
export async function resetShell(): Promise<WebFrame> {
  const res = await fetch(`${BASE}/api/reset`, { method: "POST" });
  if (!res.ok) throw new Error(`reset failed: ${res.status}`);
  return (await res.json()) as WebFrame;
}

/** 拉当前 frame. */
export async function getFrame(): Promise<WebFrame> {
  const res = await fetch(`${BASE}/api/frame`);
  if (!res.ok) throw new Error(`frame failed: ${res.status}`);
  return (await res.json()) as WebFrame;
}

/** 发一个按键. */
export async function sendKey(
  code: string,
  modifiers: string[] = [],
): Promise<{ frame: WebFrame; keep_going: boolean }> {
  const res = await fetch(`${BASE}/api/event`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ kind: "key", code, modifiers }),
  });
  if (!res.ok) throw new Error(`event failed: ${res.status}`);
  return await res.json();
}

/** 发一个鼠标点击 ("Left" | "Right"). */
export async function sendMouse(
  button: "Left" | "Right",
): Promise<{ frame: WebFrame; keep_going: boolean }> {
  const res = await fetch(`${BASE}/api/event`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ kind: "mouse", button }),
  });
  if (!res.ok) throw new Error(`mouse failed: ${res.status}`);
  return await res.json();
}

/** 连续输入一串字符 (Insert mode 下). */
export async function typeChars(text: string): Promise<void> {
  for (const c of text) {
    await sendKey(`Char:${c}`);
  }
}

/** 把 frame 的某段 rows 拼接成纯文本 (去 padding). */
export function sectionText(rows: WebCell[][]): string {
  return rows
    .map((row) => row.map((c) => c.ch).join("").trimEnd())
    .join("\n")
    .trim();
}

/** 通过浏览器 UI 路径发键 (走 index.html 的 keydown 监听). */
export async function pageKeyDown(page: Page, key: string): Promise<void> {
  await page.keyboard.press(key);
  // 等 server 处理 + DOM 更新 (setInterval 200ms 兜底, 这里直接等 API)
  await page.waitForFunction(
    () => document.getElementById("shell") !== null,
    undefined,
    { timeout: 2_000 },
  );
}

/** 等 DOM data-mode 与期望一致. */
export async function waitForMode(
  page: Page,
  mode: "Normal" | "Insert" | "Command",
): Promise<void> {
  await page.waitForFunction(
    (m) => document.getElementById("shell")?.dataset.mode === m,
    mode,
    { timeout: 3_000 },
  );
}
