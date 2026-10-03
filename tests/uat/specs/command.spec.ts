// tests/uat/specs/command.spec.ts
// Command mode + 内置命令覆盖 (7 case): :version / :help / :ai / :clear / :q / Enter 执行 / 子进程兜底.
import { test, expect } from "@playwright/test";
import {
  resetShell,
  getFrame,
  sendKey,
  sectionText,
  typeChars,
} from "./helpers";

test.beforeEach(async () => {
  await resetShell();
});

/** 执行一条 command: : → 输 cmd → Enter, 返回执行后 frame. */
async function runCommand(cmd: string) {
  await sendKey("Char::");
  for (const c of cmd) await sendKey(`Char:${c}`);
  await sendKey("Enter");
  return getFrame();
}

test(": enters Command mode with empty buffer", async () => {
  await sendKey("Char::");
  const frame = await getFrame();
  expect(frame.mode).toBe("Command");
  expect(frame.buffer).toBe("");
});

test(":version command writes kernel banner to history", async () => {
  const frame = await runCommand("version");
  expect(frame.mode).toBe("Normal");
  const hist = sectionText(frame.history);
  expect(hist).toContain("ide-kernel-core");
  expect(hist).toContain("IDE1.0 ide-shell demo");
});

test(":help command writes keymap cheatsheet to history", async () => {
  const frame = await runCommand("help");
  const hist = sectionText(frame.history);
  expect(hist).toContain("IDE Shell demo");
  expect(hist).toContain("hjkl");
});

test(":ai command with empty buffer reports no suggestion", async () => {
  const frame = await runCommand("ai");
  const hist = sectionText(frame.history);
  // PlaceholderAi 空输入 → "<no ai suggestion>"
  expect(hist).toContain("<no ai suggestion>");
});

test(":clear command empties history", async () => {
  // 先跑 :version 填 history, 再 :clear
  await runCommand("version");
  let frame = await getFrame();
  expect(sectionText(frame.history)).toContain("ide-kernel-core");
  frame = await runCommand("clear");
  // clear 后 history 段只剩空行/边框
  expect(sectionText(frame.history)).not.toContain("ide-kernel-core");
});

test(":q sets quit signal — keep_going=false, server stays alive", async () => {
  const res = await sendKey("Char::");
  expect(res.frame.mode).toBe("Command");
  await sendKey("Char:q");
  const done = await sendKey("Enter");
  expect(done.keep_going).toBe(false);
  expect(sectionText(done.frame.history)).toContain("<quit signal>");
  // server 没被杀: reset 仍可调用
  await resetShell();
  const frame = await getFrame();
  expect(frame.mode).toBe("Normal");
});

test("unknown command falls through to subprocess (echo)", async () => {
  // Windows: cmd /C echo hello → "hello"; Unix: sh -c echo hello
  const frame = await runCommand("echo hello");
  const hist = sectionText(frame.history);
  expect(hist).toContain("hello");
});
