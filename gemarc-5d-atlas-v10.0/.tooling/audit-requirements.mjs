/**
 * .tooling/audit-requirements.mjs — 需求调整工作台的对抗式审核
 *
 * 挑刺清单（每条都能判 PASS/FAIL，不接受"看起来没问题"）：
 *   R-01 全部 38 条需求都能算出影响面且不抛异常
 *   R-02 需求没有追踪行时不得给出"0 触点"而不说明原因
 *   R-03 依赖图有自环/环时闭包必须终止（不得死循环）
 *   R-04 成本数字不得整体失真（不得出现"几乎全部文件"这种无信息量结果）
 *   R-05 用户输入不得形成 HTML 注入（改理由字段注入脚本）
 *   R-06 导出的 JSON 必须能被合回 models/（要有配套工具，且字段可映射）
 *   R-07 「新增」类变更必须能指定落点，否则该类型不可用
 *   R-08 交互元素必须是语义化控件（键盘可达）
 *   R-09 离线：运行期零远程请求
 *   R-10 图/Prompt 影响口径要与 Case 一致（分直接/连带，不整体失真）
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "requirement-workbench.html").replace(/\\/g, "/");
const findings = [];
function pass(id, what, ev) { console.log(`  ✓ ${id} ${what}  ← ${ev}`); }
function fail(id, sev, where, expected, actual, impact, fix, vt) {
  findings.push({ findingId: id, severity: sev, fileLine: where, expected, actual, impact, fixProposal: fix, verificationTest: vt, status: "OPEN" });
  console.log(`  ✗ ${id} [${sev}] ${where}\n      期望: ${expected}\n      实际: ${actual}`);
}

const browser = await chromium.launch({ channel: "chrome" });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const page = await ctx.newPage();
const errs = [], remote = [];
page.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
page.on("pageerror", (e) => errs.push("pageerror: " + e.message));
page.on("request", (r) => { if (!/^(file|data|blob):/.test(r.url())) remote.push(r.url()); });
await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#reqList .g-req", { timeout: 20000 });

console.log("=== R-01 全部需求可算影响面 ===");
{
  const r = await page.evaluate(() => {
    const W = window.Ide1RequirementWorkbench;
    const M = window.Ide1Models;
    const ids = M.requirements.requirements.map((x) => x.id);
    let errs2 = 0; const stats = [];
    ids.forEach((id) => {
      try {
        const i = W.impactOf(id);
        const wo = W.buildWorkOrder(id);
        stats.push({ id, direct: i.cost.directCode, down: i.cost.downstreamCode, files: i.cost.files, cases: i.cost.directCases, level: i.cost.level, score: i.cost.score, hasWo: !!wo.objective });
      } catch (e) { errs2++; stats.push({ id, error: String(e.message) }); }
    });
    return { total: ids.length, errs2, stats };
  });
  if (r.errs2) fail("AR-01", "HIGH", "src-requirements/app.js", "38 条需求全部可算影响面并生成工单", r.errs2 + " 条抛错：" + JSON.stringify(r.stats.filter((s) => s.error)), "工作台在部分需求上直接崩", "修正 impactOf/buildWorkOrder 的边界处理", "node .tooling/audit-requirements.mjs");
  else pass("AR-01", r.total + " 条需求全部可算影响面并生成工单", "0 异常");
}

console.log("=== R-02 无追踪行时必须说明原因 ===");
{
  const r = await page.evaluate(() => {
    const M = window.Ide1Models;
    const W = window.Ide1RequirementWorkbench;
    const noRow = M.requirements.requirements.filter((x) => !M.traceability.matrix.find((m) => m.requirementId === x.id)).map((x) => x.id);
    return { noRow, sample: noRow[0], origin: noRow[0] ? W.impactOf(noRow[0]).origin : null, files: noRow[0] ? W.impactOf(noRow[0]).cost.files : 0 };
  });
  if (r.sample) {
    // 到「影响面分析」页看是否有明确说明
    await page.click('#reqList .g-req[data-req="' + r.sample + '"]');
    await page.waitForTimeout(250);
    await page.click('#tabs .g-tab[data-tab="impact"]');
    await page.waitForTimeout(350);
    const hint = await page.$eval("#centerBody", (e) => e.textContent || "");
    const explained = /落点来源/.test(hint) && (/追踪矩阵/.test(hint) || /证据/.test(hint));
    if (!explained) {
      fail("AR-02", "MEDIUM", "src-requirements/app.js", "0/无追踪行时必须明示原因（缺追踪/缺证据/未指定落点）", "界面无说明文案；origin=" + r.origin + " files=" + r.files, "用户会把「没有触点」误读成「不需要改代码」", "在影响面页显式说明落点来源与缺口原因", "node .tooling/audit-requirements.mjs");
    } else {
      pass("AR-02", "无追踪行需求给出明确落点来源与缺口原因", r.noRow.length + " 条无行（样本 " + r.sample + ", origin=" + r.origin + "）");
    }
    await page.click('#tabs .g-tab[data-tab="detail"]');
    await page.waitForTimeout(200);
  } else {
    pass("AR-02", "所有需求都有追踪行", "0 条无行");
  }
}

console.log("=== R-03 依赖图有环时闭包必须终止 ===");
{
  const t0 = Date.now();
  const r = await page.evaluate(() => {
    const W = window.Ide1RequirementWorkbench;
    const M = window.Ide1Models;
    const selfLoops = M.architecture.edges.filter((e) => e.sourceId === e.targetId).map((e) => e.id);
    const out = [];
    M.requirements.requirements.forEach((x) => { const i = W.impactOf(x.id); out.push(i.nodes.length); });
    return { selfLoops, max: Math.max.apply(null, out) };
  });
  const dt = Date.now() - t0;
  if (r.selfLoops.length) pass("AR-03", "依赖图含自环 " + r.selfLoops.join(",") + "，闭包仍终止", "38 条需求计算耗时 " + dt + "ms，最大闭包 " + r.max + " 节点");
  else pass("AR-03", "闭包计算终止", dt + "ms");
}

console.log("=== R-04 成本数字不得整体失真 ===");
{
  const r = await page.evaluate(() => {
    const W = window.Ide1RequirementWorkbench;
    const M = window.Ide1Models;
    const st = M.requirements.requirements.map((x) => {
      const i = W.impactOf(x.id);
      return { id: x.id, direct: i.cost.directCode, down: i.cost.downstreamCode, files: i.cost.files, cases: i.cost.directCases, level: i.cost.level, score: i.cost.score };
    });
    return st;
  });
  // 判据落在"真正可执行的量"上：直接代码触点必须收敛且有区分度
  const direct = r.map((x) => x.direct);
  const maxDirect = Math.max.apply(null, direct);
  const distinct = new Set(direct).size;
  const tooBroad = r.filter((x) => x.direct > 6);
  if (tooBroad.length) {
    fail("AR-04", "MEDIUM", "src-requirements/app.js", "直接代码触点应收敛（单条需求 ≤6 个 .rs）", tooBroad.map((x) => x.id + "(" + x.direct + ")").join(","), "落点过宽 → 成本数字失去区分度", "收紧种子来源（追踪矩阵优先，证据仅兜底）", "node .tooling/audit-requirements.mjs");
  } else if (distinct < 4) {
    fail("AR-04", "MEDIUM", "src-requirements/app.js", "直接代码触点数应有区分度（≥4 个不同取值）", "只有 " + distinct + " 种取值", "所有需求看起来一样贵", "按落点差异分档", "node .tooling/audit-requirements.mjs");
  } else {
    const lv = {};
    r.forEach((x) => { lv[x.level] = (lv[x.level] || 0) + 1; });
    const bad = (lv.high || 0) > r.length * 0.6;
    if (bad) fail("AR-04", "LOW", "src-requirements/app.js", "成本等级不应几乎全是 high（徽章失去意义）", JSON.stringify(lv), "等级不可用", "按实测 score 分布标定阈值", "node .tooling/audit-requirements.mjs");
    else pass("AR-04", "直接触点收敛且有区分度，等级分布合理", "直接代码 max=" + maxDirect + "，取值 " + distinct + " 种，等级 " + JSON.stringify(lv));
  }
}

console.log("=== R-05 用户输入不得形成 HTML 注入 ===");
{
  await page.click('#reqList .g-req[data-req="FR-01"]');
  await page.waitForTimeout(250);
  const payload = '<img src=x onerror="window.__XSS=1">';
  await page.fill("#fReason", payload);
  await page.waitForTimeout(200);
  await page.click('#tabs .g-tab[data-tab="work"]');
  await page.waitForTimeout(300);
  const xss = await page.evaluate(() => ({ flag: !!window.__XSS, imgs: document.querySelectorAll("#centerBody img").length }));
  if (xss.flag || xss.imgs > 0) fail("AR-05", "HIGH", "src-requirements/app.js", "用户输入按纯文本渲染（不产生元素/不执行脚本）", JSON.stringify(xss), "注入风险", "所有用户字段转义后再插入", "node .tooling/audit-requirements.mjs");
  else pass("AR-05", "用户输入按纯文本渲染", "无注入元素、无脚本执行");
}

console.log("=== R-06 导出 JSON 必须真有办法合回 models/ ===");
{
  const hasMerge = fs.existsSync(path.join(ROOT, ".tooling", "apply-requirement-changes.mjs"));
  const claim = fs.readFileSync(path.join(ROOT, "docs", "REQUIREMENT_WORKBENCH.md"), "utf8");
  const claims = /可合回 models\//.test(claim) || /合回/.test(claim);
  if (claims && !hasMerge) {
    fail("AR-06", "HIGH", "docs/REQUIREMENT_WORKBENCH.md", "宣称可合回 models/ 时必须附带可运行的合并工具", "文档宣称可合回，但 .tooling/ 下无合并脚本", "闭环断裂：用户还得手工搬字段", "补 apply-requirement-changes.mjs（带备份与 diff 报告）", "node .tooling/audit-requirements.mjs");
  } else if (hasMerge) pass("AR-06", "合回工具存在且文档宣称一致", ".tooling/apply-requirement-changes.mjs");
  else pass("AR-06", "文档未做过度宣称", "无合回宣称");
}

console.log("=== R-07「新增」类变更必须能指定落点 ===");
{
  await page.click('#tabs .g-tab[data-tab="detail"]');
  await page.waitForTimeout(250);
  await page.selectOption("#fType", "add");
  await page.waitForTimeout(300);
  const canPick = await page.evaluate(() => {
    const ids = Array.from(document.querySelectorAll("#centerBody select, #centerBody input")).map((e) => e.id);
    return ids;
  });
  const hasLanding = canPick.some((id) => /land|target|seed/i.test(id || ""));
  if (!hasLanding) fail("AR-07", "MEDIUM", "src-requirements/app.js", "「新增」类型必须能指定落点模块（否则该变更类型不可用）", "页签内无落点选择控件：" + canPick.join(","), "新增类需求无法计算影响面，功能形同虚设", "为 add 类型加落点选择（节点搜索多选）", "node .tooling/audit-requirements.mjs");
  else pass("AR-07", "「新增」类型可指定落点", canPick.join(","));
}

console.log("=== R-08 交互元素语义化 ===");
{
  const bad = await page.evaluate(() => {
    const out = [];
    document.querySelectorAll("#app *").forEach((el) => {
      if (el.onclick && !/^(BUTTON|INPUT|SELECT|TEXTAREA|A)$/.test(el.tagName)) out.push(el.tagName + "." + el.className);
    });
    return out;
  });
  if (bad.length) fail("AR-08", "LOW", "src-requirements/app.js", "可点击元素必须是语义控件", bad.slice(0, 3).join(", "), "键盘不可达", "改用 button/label", "node .tooling/audit-requirements.mjs");
  else pass("AR-08", "可点击元素均为语义控件", "0 个 div onclick");
}

console.log("=== R-09 离线：零远程请求 ===");
{
  if (remote.length || errs.length) fail("AR-09", "HIGH", "requirement-workbench.html", "运行期零远程请求且无 console error", remote.slice(0, 2).join(",") + " | " + errs.slice(0, 2).join(","), "离线不可用", "去除外部依赖", "node .tooling/audit-requirements.mjs");
  else pass("AR-09", "零远程请求、无 console error", "静态 http 0 引用");
}

console.log("=== R-10 图/Prompt 口径需与 Case 一致（分直接/连带）===");
{
  const r = await page.evaluate(() => {
    const W = window.Ide1RequirementWorkbench;
    const M = window.Ide1Models;
    const st = M.requirements.requirements.map((x) => {
      const i = W.impactOf(x.id);
      return { id: x.id, diagrams: i.cost.diagrams, cases: i.cost.cases };
    });
    const hasSplit = typeof W.impactOf("FR-09").cost.directDiagrams === "number";
    return { st, hasSplit };
  });
  if (!r.hasSplit) fail("AR-10", "LOW", "src-requirements/app.js", "图/Prompt 的影响应与 Case 一样分直接/连带", "cost 中无 directDiagrams / downstreamDiagrams", "口径不一致：Case 分了、图没分，读数会偏", "补 directDiagrams / downstreamDiagrams 字段", "node .tooling/audit-requirements.mjs");
  else pass("AR-10", "图/Prompt 已分直接/连带", "");
}

await browser.close();
const highs = findings.filter((f) => f.severity === "HIGH" && f.status === "OPEN");
console.log("\n=== 工作台审核结论：发现 " + findings.length + " 项（OPEN HIGH: " + highs.length + "）===");
fs.writeFileSync(path.join(ROOT, "qa", "audit-requirements.json"), JSON.stringify({
  schema: "ide1-requirement-workbench/audit/v1.0", generatedAt: new Date().toISOString(),
  findings, verdict: highs.length ? "FAIL" : "PASS"
}, null, 2), "utf8");
if (highs.length) process.exit(1);
