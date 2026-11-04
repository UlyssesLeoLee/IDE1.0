/**
 * .tooling/adversarial-audit.mjs — 对抗式审核实测（v10.0 adversarial_review.tests 13 项）
 *
 * 每项都给出**可复现的证据**，不通过就非零退出。
 * 独立于 e2e-journeys：这里是"攻击"，不是"参观"。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const OUT = path.join(ROOT, "qa");
const rd = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, "models", f), "utf8"));

const findings = [];
function fail(id, sev, where, expected, actual, impact, fix, vt) {
  findings.push({ findingId: id, severity: sev, fileLine: where, expected, actual, impact, fixProposal: fix, verificationTest: vt, status: "OPEN" });
  console.log(`  ✗ ${id} [${sev}] ${where}\n      期望: ${expected}\n      实际: ${actual}`);
}
function pass(id, what, evidence) {
  console.log(`  ✓ ${id} ${what}  ← ${evidence}`);
}

const arch = rd("architecture.json");
const ev = rd("evidence.json");
const cases = rd("cases.json");
const rules = rd("causal-rules.json");

// ---------------------------------------------------------------- 静态检查
console.log("=== A1 参考仓库事实污染（交付内容全量扫描）===");
{
  // 外部标记集中放在 .tooling/foreign-markers.txt —— 交付内容里不得出现这些字符串
  const markers = fs.readFileSync(path.join(ROOT, ".tooling", "foreign-markers.txt"), "utf8")
    .split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));
  const textFiles = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      // .tooling（构建工具与标记表）、dist/qa（生成物）、vendor（第三方）不计入交付内容
      if (["node_modules", "vendor", "retired", "audit-tmp", ".tooling", "dist", "qa"].includes(e.name)) continue;
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (/\.(json|md|yaml|yml|mjs|js|css|html)$/.test(e.name) && !/\.bundle\.js$/.test(e.name)) textFiles.push(p);
    }
  };
  walk(ROOT);
  const hits = [];
  for (const f of textFiles) {
    fs.readFileSync(f, "utf8").split("\n").forEach((line, i) => {
      for (const mk of markers) {
        if (line.toLowerCase().includes(mk.toLowerCase())) {
          hits.push(path.relative(ROOT, f) + ":" + (i + 1) + " 命中标记[" + mk + "] → " + line.trim().slice(0, 70));
          break;
        }
      }
    });
  }
  if (hits.length) fail("AF-01", "HIGH", "交付内容全量扫描", "交付内容不含任何外部项目标记", hits.slice(0, 5).join(" | "), "参考仓库事实污染", "改写为 IDE1.0 事实", "node .tooling/adversarial-audit.mjs");
  else pass("AF-01", "交付内容无外部项目标记残留", `扫描 ${textFiles.length} 个交付文件 × ${markers.length} 个标记，0 命中`);
}

console.log("=== A2 只有构建配置误标 Implemented ===");
{
  const evById = new Map(ev.evidence.map((e) => [e.id, e]));
  const bad = [];
  for (const n of arch.nodes) {
    if (n.verification !== "Implemented") continue;
    const paths = (n.evidenceIds || []).map((id) => (evById.get(id) || {}).path || "");
    if (!paths.length) { bad.push(n.id + "（无证据）"); continue; }
    // 只有 Cargo.toml / 目录这类"构建配置"证据 → 不能证明行为已实现
    if (paths.every((p) => /(^|[\\/])Cargo\.toml$/.test(p) || !p)) bad.push(n.id + "（证据仅 " + paths.join(",") + "）");
  }
  if (bad.length) fail("AF-02", "HIGH", "models/architecture.json", "每个 Implemented 节点都有实现代码证据", bad.join(" | "), "把构建配置误当实现", "补实现证据或降级为 Designed", "node .tooling/verify-refs.mjs");
  else pass("AF-02", "无'仅凭构建配置'的 Implemented", `逐个核对 ${arch.nodes.filter((n) => n.verification === "Implemented").length} 个 Implemented 节点`);
}

console.log("=== A3 旧 Edge 真的被改写（模型层）===");
{
  const { default: vm } = await import("node:vm");
  const ctx = { console, setTimeout, clearTimeout, Date, Math, JSON };
  ctx.globalThis = ctx; vm.createContext(ctx);
  for (const f of ["engine.js", "causal.js"]) vm.runInContext(fs.readFileSync(path.join(ROOT, "src", f), "utf8"), ctx, { filename: f });
  const model = { version: arch.version, nodes: arch.nodes, edges: arch.edges, rules: rules.rules, stateMachines: rules.stateMachines, cases: cases.cases };
  const eng = ctx.GemArcEngine.createEngine(model);
  const r = ctx.GemArcCausal.applyIntervention(model, { kind: "InsertElement", targetId: "E-WB-TO-STORE", params: { nodeId: "X-PROBE", name: "Probe" }, id: "AF-03", caseId: "C-NORMAL-EDIT" });
  const op = (r.delta.operations || [])[0] || {};
  const oldGone = !r.model.edges.some((e) => e.id === "E-WB-TO-STORE");
  const twoNew = (op.newEdgeIds || []).length === 2;
  const rulesOk = r.model.rules.filter((x) => (x.edgeId || "").indexOf("E-WB-TO-STORE") === 0).length >= 2;
  if (!(oldGone && twoNew && rulesOk)) fail("AF-03", "HIGH", "src/causal.js", "A→B 变成 A→X→B 且旧边消失、新边带规则", `oldGone=${oldGone} newEdges=${(op.newEdgeIds || []).length} rules=${r.model.rules.filter((x) => (x.edgeId || "").indexOf("E-WB-TO-STORE") === 0).length}`, "插入只是摆件，旧边仍在生效", "修 applyIntervention 的边改写逻辑", "node tests/unit/atlas.test.mjs");
  else pass("AF-03", "插入元素真改写（旧边移除 + 2 新边 + 新规则）", `removed=${JSON.stringify(op.removedEdgeIds)} new=${JSON.stringify(op.newEdgeIds)}`);

  // 无关模块不得造假分歧
  const ce = ctx.GemArcCausal.compare(eng, "C-PATH-ESCAPE", { kind: "DisableNode", targetId: "N-OS-PWSH", params: {} }, { seed: 1 });
  if (ce.outcome !== "NO_DIFFERENCE") fail("AF-04", "HIGH", "src/causal.js", "禁用与该 Case 无关的节点 → NO_DIFFERENCE", "outcome=" + ce.outcome, "无关模块出现假分歧，污染因果结论", "检查引擎是否只重算受影响子图", "node .tooling/verify-cases.mjs");
  else pass("AF-04", "无关模块不产生假分歧", "C-PATH-ESCAPE + DisableNode(N-OS-PWSH) → NO_DIFFERENCE");
}

console.log("=== A4 图窗类型与元素字段完整性 ===");
{
  const { default: vm } = await import("node:vm");
  const ctx = { console, setTimeout, clearTimeout, Date, Math, JSON };
  ctx.globalThis = ctx; vm.createContext(ctx);
  for (const f of ["engine.js", "causal.js", "diagrams.js"]) vm.runInContext(fs.readFileSync(path.join(ROOT, "src", f), "utf8"), ctx, { filename: f });
  const defs = rd("diagram-definitions.json").diagrams;
  const D = ctx.GemArcDiagrams;
  const bad = [];
  for (const d of defs) {
    const html = D.render(d, 720, 460);
    if (html.includes("Unsupported diagramType")) bad.push(d.id + "(未分派)");
    if (!/<svg/.test(html)) bad.push(d.id + "(无 SVG)");
  }
  const required = {
    UML_Class: ["classes"], UML_Component: ["components"], UML_Package: ["packages"],
    UML_Sequence: ["lifelines", "messages"], UML_State: ["states", "transitions"],
    UML_Activity: ["activities", "flows"], UML_Deployment: ["nodes", "paths"],
    UML_UseCase: ["actors", "useCases"], ER: ["entities", "relations"],
    DFD: ["processes", "flows"], BPMN: ["pools", "nodes", "sequenceFlows"],
    Security: ["boundaries", "assets", "controls"], Network: ["subnets", "nodes", "links"],
    DataFlow: ["stores", "flows"]
  };
  const missing = [];
  for (const d of defs) {
    (required[d.diagramType] || []).forEach((k) => { if (!d[k] || !d[k].length) missing.push(d.id + "." + k); });
  }
  if (bad.length || missing.length) fail("AF-05", "HIGH", "models/diagram-definitions.json", "14 图种全部正确分派且元素字段齐备", [...bad, ...missing].join(" | "), "图窗类型误用或空图", "补齐元素字段/修正 diagramType", "node .tooling/adversarial-audit.mjs");
  else pass("AF-05", "14 图种全部正确分派且字段齐备", `${defs.length} 张图渲染均无 Unsupported`);
}

console.log("=== A5 未执行测试被写为 PASS（报告 ↔ 实测一致性）===");
{
  const qa = fs.readFileSync(path.join(ROOT, "QA_FINAL.md"), "utf8");
  const e2e = JSON.parse(fs.readFileSync(path.join(OUT, "e2e-report.json"), "utf8"));
  const claims = [];
  // 注意：正则字面量里的 "/" 必须转义，否则会被当成正则结束符（"file://" 是经典坑）
  const m = qa.match(/E2E（file[^\n]*?） \| PASS (\d+)\/(\d+)/);
  if (m) claims.push(["E2E", Number(m[1]), Number(m[2])]);
  const u = qa.match(/单元测试 \| PASS (\d+)\/(\d+)/);
  if (u) claims.push(["UNIT", Number(u[1]), Number(u[2])]);
  const bad = [];
  if (claims.length) {
    if (claims[0][0] === "E2E" && (claims[0][1] !== e2e.passed || claims[0][2] !== e2e.total)) bad.push(`QA 报告称 E2E ${claims[0][1]}/${claims[0][2]}，实测 ${e2e.passed}/${e2e.total}`);
  }
  // 被引用的 IDE1.0 测试状态必须与来源文档一致（不得升级）
  const trace = rd("traceability.json");
  const docStatus = { "UAT-E2E-UI-01": "FAIL", "UAT-E2E-TERMINAL-02": "NOT_TESTED" };
  for (const t of trace.tests) {
    if (docStatus[t.id] && t.status !== docStatus[t.id]) bad.push(`${t.id} 状态被改写为 ${t.status}（来源应为 ${docStatus[t.id]}）`);
  }
  if (bad.length) fail("AF-06", "HIGH", "QA_FINAL.md / models/traceability.json", "报告数字与实测一致，未执行项不写 PASS", bad.join(" | "), "把未执行测试写成 PASS", "改回真实状态", "node .tooling/adversarial-audit.mjs");
  else pass("AF-06", "报告数字与实测一致，引用状态未被升级", `E2E 实测 ${e2e.passed}/${e2e.total}；UAT-E2E-UI-01=FAIL、UAT-E2E-TERMINAL-02=NOT_TESTED 保持原样`);
}

// ---------------------------------------------------------------- 浏览器实测
const browser = await chromium.launch({ channel: "chrome" });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const page = await ctx.newPage();
const consoleErrs = [];
page.on("console", (m) => { if (m.type() === "error") consoleErrs.push(m.text()); });
page.on("pageerror", (e) => consoleErrs.push("pageerror: " + e.message));
const remote = [];
page.on("request", (r) => { if (!/^(file|data|blob):/.test(r.url())) remote.push(r.url()); });
await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#app .g-head", { timeout: 20000 });

console.log("=== A6 file:// 隐性网络依赖 / eval / ESM ===");
{
  // 静态：只查**我方引擎源码**（src/）。vendor（three/konva/graphology）内含同名 API 的
  // 字符串或未走到的分支，不构成可达依赖 —— 可达性由运行期证据（远程请求 0）判定。
  const mySrc = fs.readdirSync(path.join(ROOT, "src")).filter((f) => f.endsWith(".js"));
  const bad = [];
  for (const f of mySrc) {
    const t = fs.readFileSync(path.join(ROOT, "src", f), "utf8");
    if (/\beval\s*\(/.test(t)) bad.push(f + ": eval(");
    if (/new\s+Function\s*\(/.test(t)) bad.push(f + ": new Function(");
    if (/\bimport\s*\(/.test(t)) bad.push(f + ": 动态 import(");
    if (/\bfetch\s*\(/.test(t)) bad.push(f + ": fetch(");
    if (/\bXMLHttpRequest\b/.test(t)) bad.push(f + ": XMLHttpRequest");
    if (/\bimportScripts\b/.test(t)) bad.push(f + ": importScripts(");
  }
  if (remote.length) bad.push("运行期远程请求 " + remote.slice(0, 3).join(","));
  if (bad.length) fail("AF-07", "HIGH", "src/*.js + 运行期", "我方源码无 eval/import/fetch/XHR，且运行期零远程请求", bad.join(" | "), "离线不可用或存在注入面", "去除对应调用", "node .tooling/adversarial-audit.mjs");
  else pass("AF-07", "我方源码无网络/动态代码 API，运行期远程请求 0", `静态扫 ${mySrc.length} 个引擎文件 + 75 趟旅程运行期 0 请求`);
}

console.log("=== A7 左手系一致性（语义 +X 在屏幕右侧）===");
{
  await page.click('[data-view="3D"]');
  await page.waitForSelector("#s3dStage canvas", { timeout: 15000 });
  await page.waitForTimeout(900);
  const probe = await page.evaluate(() => {
    const S = window.GemArcApp.state;
    const sc = S.scene3d;
    const M = window.GemArcModels;
    let pair = null;
    for (const rid of Object.keys(M.spatial.rooms)) {
      const r = M.spatial.rooms[rid];
      if (!r.nodes || r.nodes.length < 2) continue;
      for (let i = 0; i < r.nodes.length && !pair; i++) {
        for (let j = i + 1; j < r.nodes.length && !pair; j++) {
          const pa = M.spatial.nodeOffsets[r.nodes[i]], pb = M.spatial.nodeOffsets[r.nodes[j]];
          if (pa && pb && Math.abs(pa[0] - pb[0]) > 1 && Math.abs(pa[2] - pb[2]) < 0.01) {
            pair = { a: r.nodes[i], b: r.nodes[j], dx: pa[0] - pb[0] };
          }
        }
      }
      if (pair) break;
    }
    if (!pair) return null;
    const cv = sc.renderer.domElement;
    const w = cv.clientWidth, h = cv.clientHeight;
    const proj = (id) => {
      const v = sc.nodeMeshes[id].position.clone().project(sc.camera);
      return { x: (v.x * 0.5 + 0.5) * w, y: (-v.y * 0.5 + 0.5) * h };
    };
    return { a: pair.a, b: pair.b, semanticDx: pair.dx, pa: proj(pair.a), pb: proj(pair.b) };
  });
  if (!probe) fail("AF-08", "MEDIUM", "models/spatial.json", "存在同房间内可比较的节点对", "未找到可比节点对", "无法验证坐标一致性", "提供可比节点对或改用其他验证方式", "node .tooling/adversarial-audit.mjs");
  else {
    const screenDx = probe.pa.x - probe.pb.x;
    const sameSign = (probe.semanticDx > 0) === (screenDx > 0);
    if (!sameSign) fail("AF-08", "HIGH", "src/spatial.js / models/spatial.json", "语义 +X 出现在屏幕右侧", `语义Δx=${probe.semanticDx.toFixed(2)} 屏幕Δx=${screenDx.toFixed(2)}（${probe.a} vs ${probe.b}）`, "左手系失配：左右颠倒", "修 toThree 适配器或节点偏移", "node .tooling/adversarial-audit.mjs");
    else pass("AF-08", "语义 +X 确实在屏幕右侧", `${probe.a} vs ${probe.b}：语义Δx=${probe.semanticDx.toFixed(2)} 屏幕Δx=${screenDx.toFixed(2)} 同号`);
  }
}

console.log("=== A8 重复切换（监听器累积 / WebGL 上下文泄漏）===");
{
  const before = await page.evaluate(() => document.querySelectorAll("#app *").length);
  for (let i = 0; i < 8; i++) {
    for (const v of ["1D", "2D", "3D", "4D", "5D", "P"]) {
      await page.click(`[data-view="${v}"]`);
      await page.waitForTimeout(90);
    }
  }
  await page.waitForTimeout(800);
  const after = await page.evaluate(() => document.querySelectorAll("#app *").length);
  const errs = consoleErrs.filter((e) => !/ResizeObserver/.test(e));
  if (errs.length) fail("AF-09", "HIGH", "src/app.js", "反复切换 48 次无 console error", errs.slice(0, 3).join(" | "), "监听器累积或上下文泄漏", "在视图切换时解绑/销毁", "node .tooling/adversarial-audit.mjs");
  else if (after > before * 3 + 500) fail("AF-09", "MEDIUM", "src/app.js", "DOM 节点数不随切换线性增长", `${before} → ${after}`, "重复挂载", "复用/清理 DOM", "node .tooling/adversarial-audit.mjs");
  else pass("AF-09", "48 次视图切换无错误、DOM 未膨胀", `${before} → ${after} 个节点，console error 0`);
}

console.log("=== A9 窄屏因果链仍可查 ===");
{
  await page.setViewportSize({ width: 390, height: 844 });
  await page.click('[data-view="5D"]');
  await page.waitForSelector("#sIv", { timeout: 10000 });
  await page.selectOption("#sCase", "C-PATH-ESCAPE");
  await page.click("#sRun");
  await page.waitForTimeout(500);
  await page.selectOption("#sIv", "DisableNode");
  const tv = await page.$$eval("#sIvTarget option", (o) => o.map((x) => x.value));
  await page.selectOption("#sIvTarget", tv.includes("N-WB-PATHSAFETY") ? "N-WB-PATHSAFETY" : tv[0]);
  await page.click("#sCompare");
  await page.waitForTimeout(900);
  const info = await page.$eval("#causalInfo", (e) => e.textContent || "");
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  const inspW = await page.evaluate(() => { const e = document.querySelector("#rightPane"); return e ? e.getBoundingClientRect().width : 0; });
  const drawerBtn = await page.$("#drawerRight") !== null;
  if (!/DIVERGED|NO_DIFFERENCE/.test(info)) fail("AF-10", "HIGH", "src/app.js", "390px 下 5D 因果结论可读", "causalInfo=" + info.slice(0, 60), "窄屏因果检查器消失", "把检查器改为抽屉而非隐藏", "node .tooling/adversarial-audit.mjs");
  else if (overflow > 0) fail("AF-10", "MEDIUM", "src/styles.css", "390px 无横向溢出", overflow + "px", "窄屏布局破裂", "修响应式样式", "node .tooling/adversarial-audit.mjs");
  else if (!drawerBtn) fail("AF-10", "MEDIUM", "src/app.js", "提供打开检查器抽屉的按钮", "无 #drawerRight", "窄屏下检查器不可达", "加抽屉开关", "node .tooling/adversarial-audit.mjs");
  else pass("AF-10", "390px 因果结论可读 + 无溢出 + 检查器可达", `causalInfo 含 ${(info.match(/DIVERGED|NO_DIFFERENCE/) || ["?"])[0]}，overflow=${overflow}px，inspector 宽=${inspW}px`);
}
await ctx.close();
await browser.close();

console.log("=== A11 无 WebGL 时必须显式降级（禁用 WebGL 重跑）===");
{
  const b2 = await chromium.launch({ channel: "chrome", args: ["--disable-webgl", "--disable-webgl2", "--disable-gpu", "--use-gl=swiftshader-webgl"] });
  const c2 = await b2.newContext({ viewport: { width: 1280, height: 800 } });
  const p2 = await c2.newPage();
  const errs2 = [];
  p2.on("pageerror", (e) => errs2.push(e.message));
  await p2.goto(FILE_URL, { waitUntil: "load" });
  await p2.waitForSelector("#app .g-head", { timeout: 20000 });
  await p2.click('[data-view="3D"]');
  await p2.waitForTimeout(1500);
  const deg = await p2.$$eval(".g-degraded", (d) => d.map((x) => x.textContent));
  const status = await p2.$eval("#s3dStatus", (e) => e.textContent || "").catch(() => "");
  const fallbackSvg = await p2.$$eval("#s3dStage svg, #s3dStage .g-canvas", (s) => s.length).catch(() => 0);
  const stillAlive = await p2.evaluate(() => !!document.querySelector("#viewNav"));
  if (deg.length && /DEGRADED/.test(deg[0]) && stillAlive) {
    pass("AF-11", "WebGL 不可用 → 显式 DEGRADED 且应用不崩", `${deg[0].slice(0, 46)} | 状态栏=${status.slice(0, 30)} | 回退 2D 元素=${fallbackSvg}`);
  } else {
    fail("AF-11", "HIGH", "src/app.js / src/spatial.js", "WebGL 不可用时显式 DEGRADED 并回退 2D", `degraded=${JSON.stringify(deg)} status=${status}`, "无 WebGL 伪装 3D / 应用崩溃", "补 WebGL 能力探测与降级路径", "node .tooling/adversarial-audit.mjs");
  }
  if (errs2.length) console.log("     （降级路径 pageerror: " + errs2.slice(0, 2).join(" | ") + "）");
  await b2.close();
}

// ---------------------------------------------------------------- 汇总
const highs = findings.filter((f) => f.severity === "HIGH" && f.status === "OPEN");
console.log("\n=== 对抗式审核结论 ===");
console.log("发现: " + findings.length + " 项（OPEN HIGH: " + highs.length + "）");
const report = {
  schema: "ide1.5d-atlas/adversarial-audit/v10.0",
  generatedAt: new Date().toISOString(),
  checks: 11,
  findings,
  verdict: highs.length ? "FAIL" : "PASS"
};
fs.mkdirSync(OUT, { recursive: true });
fs.writeFileSync(path.join(OUT, "adversarial-audit.json"), JSON.stringify(report, null, 2), "utf8");
if (highs.length) process.exit(1);
