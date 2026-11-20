// E2E Verification for Git-mode and Nema-mode Co-Editing on Serverless Coordinator
import { coordinator, validateNemaSyntax, nemaSemanticMerge, threeWayMerge } from "../api/v1/_state.js";
import assert from "assert";

console.log("=== 1. Testing NEMA Syntax Invariant Validator (Red & Green) ===");

// Red 1: Consecutive unescaped double spaces
const bad1 = " \\1 1\n\\%2\n\\# C1 C2\nA  B\n\n\n";
const resBad1 = validateNemaSyntax(bad1);
assert.strictEqual(resBad1.ok, false, "Red 1: Double spaces must fail");
console.log("✓ Red 1 (Double space rejection) passed");

// Red 2: TAB in body
const bad2 = " \\1 1\n\\%2\n\\# C1 C2\nA\tB\n\n\n";
const resBad2 = validateNemaSyntax(bad2);
assert.strictEqual(resBad2.ok, false, "Red 2: TAB in body must fail");
console.log("✓ Red 2 (TAB in body rejection) passed");

// Green: Valid NEMA document
const goodNema = " \\1 1\n\\%2\n\\# Key Val\nAlpha 100\n\n\n";
const resGood = validateNemaSyntax(goodNema);
assert.strictEqual(resGood.ok, true, "Green: Valid NEMA must pass");
console.log("✓ Green (Valid NEMA) passed");

console.log("=== 2. Testing NEMA-mode Multi-Agent Co-Editing ===");
const nemaFile = "project:dataset.nema";
coordinator.registerFile("project", "dataset.nema", goodNema, "nema");

// Agent A commits Beta on base rev 1
const resA = coordinator.headlessEdit({
  fileId: nemaFile,
  author: "claude-3-7-sonnet",
  baseRevision: 1,
  newText: " \\1 1\n\\%2\n\\# Key Val\nAlpha 100\nBeta 200\n\n\n",
  mode: "nema"
});
assert.strictEqual(resA.ok, true);
assert.strictEqual(resA.revision, 2);
assert.strictEqual(resA.merged, false);
console.log("✓ Agent A Fast-Path commit passed");

// Agent B concurrently commits Gamma on base rev 1
const resB = coordinator.headlessEdit({
  fileId: nemaFile,
  author: "gpt-4o",
  baseRevision: 1,
  newText: " \\1 1\n\\%2\n\\# Key Val\nAlpha 100\nGamma 300\n\n\n",
  mode: "nema"
});
assert.strictEqual(resB.ok, true);
assert.strictEqual(resB.revision, 3);
assert.strictEqual(resB.merged, true);
assert.strictEqual(resB.conflicts, 0);
assert(resB.content.includes("Alpha 100"));
assert(resB.content.includes("Beta 200"));
assert(resB.content.includes("Gamma 300"));
console.log("✓ Agent B Concurrent NEMA Semantic Auto-Merge passed");

// Red Injection test: Agent C attempts to inject invalid syntax
const resC = coordinator.headlessEdit({
  fileId: nemaFile,
  author: "attacker",
  baseRevision: 3,
  newText: " \\1 1\n\\%2\n\\# Key Val\nA  B (bad double space)\n\n\n",
  mode: "nema"
});
assert.strictEqual(resC.ok, false);
assert.strictEqual(resC.status, "REJECTED_NEMA_SYNTAX_ERROR");
console.log("✓ Red Injection Rejection passed");

console.log("=== 3. Testing Git-mode Multi-Agent Co-Editing ===");
const gitFile = "project:app.rs";
const rustBase = "fn f1() { 1 }\nfn f2() { 2 }\nfn f3() { 3 }\n";
coordinator.registerFile("project", "app.rs", rustBase, "rust");

// Claude modifies f1 (Rev 1 -> Rev 2)
const gitA = coordinator.headlessEdit({
  fileId: gitFile,
  author: "claude",
  baseRevision: 1,
  newText: "fn f1() { 111 }\nfn f2() { 2 }\nfn f3() { 3 }\n",
  mode: "git"
});
assert.strictEqual(gitA.ok, true);
assert.strictEqual(gitA.revision, 2);

// GPT concurrently modifies f3 (Rev 1 -> Rev 3)
const gitB = coordinator.headlessEdit({
  fileId: gitFile,
  author: "gpt",
  baseRevision: 1,
  newText: "fn f1() { 1 }\nfn f2() { 2 }\nfn f3() { 333 }\n",
  mode: "git"
});
assert.strictEqual(gitB.ok, true);
assert.strictEqual(gitB.revision, 3);
assert.strictEqual(gitB.merged, true);
assert(gitB.content.includes("fn f1() { 111 }"));
assert(gitB.content.includes("fn f3() { 333 }"));
console.log("✓ Git Clean 3-Way Auto-Merge passed");

// DeepSeek concurrently modifies f1 (conflict)
const gitConflict = coordinator.headlessEdit({
  fileId: gitFile,
  author: "deepseek",
  baseRevision: 1,
  newText: "fn f1() { 999 }\nfn f2() { 2 }\nfn f3() { 3 }\n",
  mode: "git"
});
assert.strictEqual(gitConflict.ok, false);
assert.strictEqual(gitConflict.status, "CONFLICT");
assert(gitConflict.content.includes("<<<<<<< HEAD (ours)"));
console.log("✓ Git Conflict Marking passed");

console.log("\nALL COLLABORATION MODES TESTS PASSED SUCCESSFULLY!");
