/**
 * .tooling/mutation-test.mjs — 变异测试：证明测试套件不是恒真断言
 *
 * 做法：在 TEMP 副本上逐个注入缺陷，跑到单元测试上，看是否变红。
 * 每个变异都对应一条真实防线；全红 = 测试有效。
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(process.argv[2] || ".");
// 工作副本放在 .tooling/.mutwork/（向上即可解析 .tooling/node_modules；测完删除）
const TMP = path.join(ROOT, ".tooling", ".mutwork");

// 复制：src + models + tests + .tooling（vitest 配置与 node_modules junction）
function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    if (e.name === "node_modules" || e.name === ".mutwork") continue;
    const s = path.join(from, e.name);
    const d = path.join(to, e.name);
    if (e.isDirectory()) copyDir(s, d);
    else fs.copyFileSync(s, d);
  }
}
copyDir(path.join(ROOT, "src"), path.join(TMP, "src"));
copyDir(path.join(ROOT, "models"), path.join(TMP, "models"));
copyDir(path.join(ROOT, "tests"), path.join(TMP, "tests"));
copyDir(path.join(ROOT, ".tooling"), path.join(TMP, ".tooling"));
// src/*.js 依赖 `module.exports` 导出 —— 只在 CommonJS 解析下生效。
// 向上查找 package.json 时若命中 "type": "module"，require 会拿到 {}。
// 工作副本显式固定为 commonjs，与真实仓库根（无 package.json ⇒ CJS）一致。
fs.writeFileSync(
  path.join(TMP, "package.json"),
  JSON.stringify({ name: "atlas-mutwork", private: true, type: "commonjs" }, null, 2),
  "utf8"
);

// 变异清单：每一项 = 一个真实缺陷注入
const MUTANTS = [
  {
    id: "M-01",
    file: "src/engine.js",
    label: "删除 usedEdgeIds.add(eid)（边可被重复触发）",
    from: "          usedEdgeIds.add(eid);",
    to: "          /* mutated: no usedEdgeIds */"
  },
  {
    id: "M-02",
    file: "src/engine.js",
    label: "whenSatisfied 恒为真（when 约束失效）",
    from: "      if (!rule || !rule.when) return true;",
    to: "      return true; // mutated"
  },
  {
    id: "M-03",
    file: "src/engine.js",
    label: "evalGuard 对缺失变量放行（notEquals(undefined,\"\") 变 true）",
    from: "        if (lv === undefined) return false; // 缺标 ⇒ 判否，不当作\"与某值不同\"",
    to: "        /* mutated: 缺标放行 */"
  },
  {
    id: "M-04",
    file: "src/graph.js",
    label: "自环去重 key 退回旧实现（多个不同自环只报 1 个）",
    from: "        if (!rot.length) return nodes.length ? \"self:\" + nodes[0] : \"\";",
    to: "        if (!rot.length) return \"\"; // mutated"
  },
  {
    id: "M-05",
    file: "src/causal.js",
    label: "旧边只打 removed 标记而保留在边集里（Edge Rewrite 不彻底）",
    from: "      m.edges = m.edges.filter(function (e) { return e.id !== ab.id; });",
    to: "      m.edges = m.edges.map(function (e) { return e.id === ab.id ? Object.assign({}, e, { removed: true }) : e; });"
  },
  {
    id: "M-06",
    file: "src/prompt.js",
    label: "未知 ID 不报错（校验失效）",
    from: "      else errors.push({ field: \"requirementIds\", message: \"unknown requirement id: \" + id });",
    to: "      else warnings.push({ field: \"requirementIds\", message: \"unknown requirement id: \" + id });"
  },
  {
    id: "M-07",
    file: "src/diagrams.js",
    label: "未知 diagramType 退化为 Sequence（不再显式失败）",
    from: "    if (!fn) {",
    to: "    if (!fn) { fn = RENDERERS.UML_Sequence; if (true) return fn(diagram, width || 800, height || 500);"
  },
  {
    id: "M-08",
    file: "src/validate.js",
    label: "数组元素类型不校验（string[] 静默放行）",
    from: "        for (var j = 0; j < val.length; j++) {",
    to: "        for (var j = 0; j < 0; j++) {"
  },
  {
    id: "M-09",
    file: "src/engine.js",
    label: "预算上限失效（maxEvents 不再抛错）",
    from: "        if (events.length > LIM.maxEvents) {",
    to: "        if (false && events.length > LIM.maxEvents) {"
  }
];

function runTests() {
  try {
    execFileSync(process.execPath, [
      path.join(ROOT, ".tooling", "node_modules", "vitest", "vitest.mjs"),
      "run", "--config", path.join(TMP, ".tooling", "vitest.config.mjs")
    ], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], cwd: TMP });
    return { passed: true };
  } catch (e) {
    return { passed: false, out: (e.stdout || "") + (e.stderr || "") };
  }
}

// 基线：未变异必须全绿
const base = runTests();
console.log("baseline (unmutated) tests:", base.passed ? "PASS" : "FAIL");
if (!base.passed) {
  console.log((base.out || "").split("\n").slice(0, 25).join("\n"));
  process.exit(1);
}

let killed = 0;
let survived = 0;
for (const m of MUTANTS) {
  const p = path.join(TMP, m.file);
  const orig = fs.readFileSync(p, "utf8");
  if (!orig.includes(m.from)) {
    console.log(`  ${m.id}  SKIPPED (anchor not found) — ${m.label}`);
    survived++;
    continue;
  }
  fs.writeFileSync(p, orig.replace(m.from, m.to), "utf8");
  const r = runTests();
  fs.writeFileSync(p, orig, "utf8");
  if (r.passed) {
    console.log(`  ${m.id}  SURVIVED  ← 缺陷未被测出: ${m.label}`);
    survived++;
  } else {
    const line = (r.out || "").split("\n").filter((l) => /FAIL|AssertionError|→/.test(l)).slice(0, 2).join(" | ");
    console.log(`  ${m.id}  KILLED   (${m.label})`);
    if (line) console.log(`         ${line.trim().slice(0, 150)}`);
    killed++;
  }
}

console.log("");
console.log(`=== 变异测试：注入 ${MUTANTS.length} 个缺陷 → 杀死 ${killed}，存活 ${survived} ===`);
fs.rmSync(TMP, { recursive: true, force: true });
process.exit(survived === 0 ? 0 : 1);
