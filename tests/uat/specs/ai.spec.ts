// tests/uat/specs/ai.spec.ts
// AI bridge 覆盖 (4 case): PlaceholderAi 行为通过 :ai 命令 + buffer 状态验证.
import { test, expect } from "@playwright/test";
import {
  resetShell,
  getFrame,
  sendKey,
  sectionText,
} from "./helpers";

test.beforeEach(async () => {
  await resetShell();
});

async function typeIntoBuffer(text: string) {
  await sendKey("Char:i");
  for (const c of text) await sendKey(`Char:${c}`);
  await sendKey("Esc");
}

async function runCommand(cmd: string) {
  await sendKey("Char::");
  for (const c of cmd) await sendKey(`Char:${c}`);
  await sendKey("Enter");
  return getFrame();
}

test("ai_suggestion empty when buffer empty (:ai → no suggestion)", async () => {
  const frame = await runCommand("ai");
  expect(sectionText(frame.history)).toContain("<no ai suggestion>");
});

test("ai_suggestion echoes buffer content (PlaceholderAi)", async () => {
  // Insert 输入 "Get-Process" → Normal → :ai
  // 注意: Esc 回 Normal 时 buffer 保留, :ai 读的是当前 buffer
  await typeIntoBuffer("Get-Process");
  const frame = await runCommand("ai");
  const hist = sectionText(frame.history);
  expect(hist).toContain("AI suggestion: Get-Process");
});

test(":ai does not clear buffer (non-destructive)", async () => {
  await typeIntoBuffer("dir");
  await runCommand("ai");
  const frame = await getFrame();
  expect(frame.buffer).toBe("dir");
});

test(":ai twice appends two history lines", async () => {
  await typeIntoBuffer("ls");
  await runCommand("ai");
  const frame = await runCommand("ai");
  const hist = sectionText(frame.history);
  const matches = hist.split("AI suggestion: ls").length - 1;
  expect(matches).toBe(2);
});
