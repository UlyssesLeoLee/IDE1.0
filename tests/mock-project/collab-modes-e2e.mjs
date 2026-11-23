import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  checkSyntaxIntegrity,
  formatUnifiedDiff,
  nemaSemanticMerge,
  threeWayMerge,
  validateNemaSyntax,
} from "../../api/v1/_state.js";

const mockDir = path.dirname(fileURLToPath(import.meta.url));
const readMock = (rel) =>
  fs.readFileSync(path.join(mockDir, rel), "utf8").replace(/\r\n/g, "\n");

// 1. Myers Unified Diff on Mock math.rs
const mathBase = readMock("src/math.rs");
const mathModified = mathBase.replace("a + b", "a.saturating_add(b)");
const diff = formatUnifiedDiff(mathBase, mathModified, "src/math.rs@v1", "src/math.rs@v2");
assert.ok(diff.includes("@@"), "Unified diff must include hunk header");
assert.ok(diff.includes("-    a + b") && diff.includes("+    a.saturating_add(b)"));

// 2. Git 3-Way Concurrent Auto-Merge on Mock math.rs (clean commutativity)
const mathOurs = mathBase.replace("a + b", "a.saturating_add(b)");
const mathTheirs = mathBase.replace("a * b", "a.saturating_mul(b)");
const mergedMath = threeWayMerge(mathBase, mathOurs, mathTheirs);
assert.equal(mergedMath.clean, true, "Disjoint edits in math.rs must merge cleanly");
assert.equal(mergedMath.conflicts, 0);
assert.ok(mergedMath.text.includes("saturating_add") && mergedMath.text.includes("saturating_mul"));
assert.equal(checkSyntaxIntegrity("rust", mergedMath.text).ok, true);

// 3. Git 3-Way Overlapping Conflict & Standard Conflict Markers on Mock service.rs
const svcBase = readMock("src/service.rs");
const svcOurs = svcBase.replace('"OPERATIONAL"', '"DEGRADED"');
const svcTheirs = svcBase.replace('"OPERATIONAL"', '"MAINTENANCE"');
const conflictSvc = threeWayMerge(svcBase, svcOurs, svcTheirs);
assert.equal(conflictSvc.clean, false);
assert.equal(conflictSvc.conflicts, 1);
assert.ok(conflictSvc.text.includes("<<<<<<< HEAD (ours)"));
assert.ok(conflictSvc.text.includes("======="));
assert.ok(conflictSvc.text.includes(">>>>>>> INCOMING (theirs)"));
assert.equal(checkSyntaxIntegrity("rust", conflictSvc.text).ok, false);

// 4. NEMA Adversarial Syntax Guard (Red) + Semantic Merge Closure (Green) on architecture.nema
const nemaBase = readMock("knowledge/architecture.nema");
assert.equal(validateNemaSyntax(nemaBase).ok, true, "Baseline architecture.nema must be valid");
assert.equal(
  validateNemaSyntax(nemaBase.replace("Verified", "Verified  BadDoubleSpace")).ok,
  false,
  "NEMA red check must reject unescaped double spaces"
);
assert.equal(
  validateNemaSyntax(nemaBase.replace("RustCore Verified", "RustCore\tVerified")).ok,
  false,
  "NEMA red check must reject raw TAB in row body"
);

const nemaOurs = nemaBase.replace(
  "CollabRoster Distributed Scaled",
  "CollabRoster Distributed Scaled\nPluginRuntime Lazy Ready"
);
const nemaTheirs = nemaBase.replace(
  "CollabRoster Distributed Scaled",
  "CollabRoster Distributed Scaled\nLoopBreaker Isolated Active"
);
const nemaMerged = nemaSemanticMerge(nemaBase, nemaOurs, nemaTheirs);
assert.equal(nemaMerged.clean, true, "Disjoint Table rows in architecture.nema must merge cleanly");
assert.equal(nemaMerged.conflicts, 0);
assert.ok(nemaMerged.text.includes("PluginRuntime Lazy Ready"));
assert.ok(nemaMerged.text.includes("LoopBreaker Isolated Active"));
assert.equal(validateNemaSyntax(nemaMerged.text).ok, true, "Merged .nema output must remain valid");

console.log("Mock-project Git/NEMA collab E2E passed (4/4 assertions groups OK)");
