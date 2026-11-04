// 修：规则优先级（拒绝类在前）+ 补 N-CONSOLE-MOCKS 状态机 + Case 补齐 inputs
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const M = path.join(ROOT, "models");
const read = (n) => JSON.parse(fs.readFileSync(path.join(M, n), "utf8"));
const write = (n, o) => fs.writeFileSync(path.join(M, n), JSON.stringify(o, null, 2) + "\n", "utf8");

// ---- 1) causal-rules：补状态机 + 规则优先级排序 ----
{
  const r = read("causal-rules.json");
  const have = new Set(r.stateMachines.map((s) => s.nodeId));
  if (!have.has("N-CONSOLE-MOCKS")) {
    r.stateMachines.push({
      id: "SM-CONSOLE-MOCKS", nodeId: "N-CONSOLE-MOCKS", initial: "Running",
      states: ["Idle", "Running", "Completed", "Failed"], terminalStates: ["Completed", "Failed"]
    });
    console.log("+ stateMachine SM-CONSOLE-MOCKS");
  }

  // 规则优先级 = 数组顺序。拒绝类必须排在接受类之前，
  // 否则路径穿越 / 空值这类 Case 会先命中"正常写入"规则。
  const PRIORITY = [
    "R-VAULT-REJECT-TRAVERSAL",
    "R-VAULT-REJECT-EMPTY",
    "R-VAULT-WRITE",
    "R-AUTH-REJECT",
    "R-AUTH-CHECK",
    "R-GIT-RECEIVE-AUTH",
    "R-GIT-READ-OPEN",
    "R-AI-STREAM-REJECT-ANTHROPIC",
    "R-AI-STREAM-OPENAI",
    "R-SYNC-NOT-FASTFORWARD",
    "R-SYNC-FASTFORWARD"
  ];
  const rank = (id) => { const i = PRIORITY.indexOf(id); return i === -1 ? PRIORITY.length : i; };
  const before = r.rules.map((x) => x.id);
  r.rules = r.rules.slice().sort((a, b) => rank(a.id) - rank(b.id));
  console.log("rule order:", r.rules.map((x) => x.id).join(","));
  if (before.join(",") !== r.rules.map((x) => x.id).join(",")) console.log("  (order changed)");

  // 在规则表里写明优先级语义
  r.priorityRule = "rules 数组顺序即优先级：同一 edgeId 上先命中的规则生效。拒绝类规则必须排在接受类之前。";
  write("causal-rules.json", r);
}

// ---- 2) cases：补齐 inputs，让守卫能正确分流 ----
{
  const c = read("cases.json");
  const ADD = {
    "CASE-VAULT-PATH-TRAVERSAL": { value: "x", key: "../escape" },
    "CASE-VAULT-RESTORE": { value: "x", key: "openai", target_version: 1 },
    "CASE-VAULT-DIFF-RANGE": { value: "x", key: "openai", base: 1, head: 2 },
    "CASE-AUTH-MISSING": { value: "x", key: "openai" },
    "CASE-AUTH-BASIC-MISMATCH": { value: "x", key: "openai" },
    "CASE-CHANGE-NOTE-DISCARDED": { value: "x", key: "openai", change_note: "审计备注" },
    "CASE-ERROR-ENVELOPE-SHAPE": { value: "x", key: "openai" },
    "CASE-VAULT-WRITE-EMPTY": { value: "", key: "openai" },
    "CASE-VAULT-WRITE-NORMAL": { value: "sk-example-value", key: "openai" }
  };
  for (const [id, add] of Object.entries(ADD)) {
    const t = c.cases.find((x) => x.id === id);
    if (!t) continue;
    t.inputs = Object.assign({}, t.inputs || {}, add);
  }
  write("cases.json", c);
  console.log("cases inputs patched:", Object.keys(ADD).length);
}
