/**
 * .tooling/verify-requirements.mjs — 需求调整工作台的浏览器实测
 *
 * 断言必须落在"真的做了事"上：影响面要出现真实文件路径、工单要含交接字段、
 * 变更集要能导出。空壳页面不算通过。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "requirement-workbench.html").replace(/\\/g, "/");
const OUT = path.join(ROOT, "qa");
fs.mkdirSync(OUT, { recursive: true });

const results = [];
let fails = 0;
function check(name, ok, detail) {
  results.push({ name, ok: !!ok, detail: String(detail == null ? "" : detail) });
  if (!ok) fails++;
  console.log(`  ${ok ? "PASS" : "FAIL"}  ${name}${ok ? "" : "  ← " + detail}`);
}

const browser = await chromium.launch({ channel: "chrome" });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const page = await ctx.newPage();
const errs = [];
const remote = [];
page.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
page.on("pageerror", (e) => errs.push("pageerror: " + e.message));
page.on("request", (r) => { if (!/^(file|data|blob):/.test(r.url())) remote.push(r.url()); });

await page.goto(FILE_URL, { waitUntil: "load" });
await page.waitForSelector("#reqList .g-req", { timeout: 20000 });

check("W00 file:// 打开无 console error", errs.length === 0, errs.slice(0, 2).join(" | "));
check("W00 运行期零远程请求", remote.length === 0, remote.slice(0, 2).join(" | "));

const total = await page.$$eval("#reqList .g-req", (b) => b.length);
check("W01 需求列表装载（38 条）", total === 38, total + " 条");

// 搜索过滤
await page.fill("#fSearch", "终端");
await page.waitForTimeout(250);
const filtered = await page.$$eval("#reqList .g-req", (b) => b.length);
check("W01 搜索过滤生效", filtered > 0 && filtered < total, total + " → " + filtered);
await page.fill("#fSearch", "");
await page.waitForTimeout(200);

// 分类过滤
await page.click('#cats .g-chipbtn[data-cat="NFR"]');
await page.waitForTimeout(200);
const nfr = await page.$$eval("#reqList .g-req", (b) => b.map((x) => x.getAttribute("data-req")));
check("W01 分类过滤（NFR）只显示 NFR", nfr.length === 14 && nfr.every((x) => x.startsWith("NFR-")), nfr.join(","));
await page.click('#cats .g-chipbtn[data-cat="all"]');
await page.waitForTimeout(200);

// 选一条有真实代码触点的需求（FR-09 终端）
await page.click('#reqList .g-req[data-req="FR-09"]');
await page.waitForTimeout(300);
const detailTitle = await page.$eval("#centerBody .g-h", (e) => e.textContent || "");
check("W02 需求详情打开", detailTitle.includes("FR-09"), detailTitle);
const hasFields = await page.$$eval("#fIntent, #fType, #fAcc, #fReason", (e) => e.length);
check("W02 需求可编辑（意图/类型/判据/理由）", hasFields === 4, hasFields + " 个字段");

// 影响面
await page.click('#tabs .g-tab[data-tab="impact"]');
await page.waitForTimeout(350);
const files = await page.$$eval("#centerBody .g-path", (e) => e.map((x) => x.textContent));
check("W03 影响面给出真实代码触点（file 路径）", files.length > 0 && /\.rs$|\.html$|\.toml$/.test(files[0]), files.slice(0, 3).join(" | "));
const statTexts = await page.$$eval("#centerBody .g-stat", (e) => e.map((x) => x.textContent.replace(/\s+/g, " ")));
check("W03 影响面给出成本统计", statTexts.length >= 5, statTexts.join(" / "));
const caseTags = await page.$$eval("#centerBody .g-tag-case", (e) => e.map((x) => x.textContent));
check("W03 影响面列出需重跑的 Case", caseTags.length > 0, caseTags.slice(0, 4).join(","));
const testTags = await page.$$eval("#centerBody .g-tag", (e) => e.map((x) => x.textContent).filter((t) => t.indexOf("UT-") === 0 || t.indexOf("UAT-") === 0 || t.indexOf("IT-") === 0 || t.indexOf("CI-") === 0));
check("W03 影响面列出需更新的测试", testTags.length > 0, testTags.slice(0, 4).join(","));

// 变更类型 → 建议随之变化
await page.click('#tabs .g-tab[data-tab="detail"]');
await page.waitForTimeout(250);
await page.selectOption("#fType", "tighten");
await page.waitForTimeout(300);
const note = await page.$eval("#centerBody .g-note", (e) => e.textContent || "").catch(() => "");
check("W04 变更类型给出模型侧/代码侧同步建议", /模型侧/.test(note) && /代码侧/.test(note), note.slice(0, 90));

// 写理由 → 工单包含理由
await page.fill("#fReason", "终端必须在非交互环境也能回显");
await page.waitForTimeout(200);
await page.click('#tabs .g-tab[data-tab="work"]');
await page.waitForTimeout(300);
const wo = await page.$eval("#woBox", (e) => e.textContent || "");
const needFields = ["objective", "requirementIds", "sourceEvidence", "targetModules", "rules", "failureModes", "acceptanceTests", "dependency", "rollback"];
const missing = needFields.filter((f) => wo.indexOf(f) === -1);
check("W05 工单含全部交接字段（9 项）", missing.length === 0, "缺: " + missing.join(","));
check("W05 工单写入了变更理由", wo.indexOf("非交互环境") !== -1, wo.slice(0, 120));
check("W05 工单给出成本面", /\*\*cost\*\*/.test(wo) && /直接代码/.test(wo) && /直接Case/.test(wo), "含 cost 字段（直接/连带代码文件与 Case 数）");

// 变更集 + 导出
await page.click('#tabs .g-tab[data-tab="set"]');
await page.waitForTimeout(300);
const setRows = await page.$$eval("#centerBody .g-table tr", (r) => r.length);
check("W06 变更集累积了改动", setRows >= 1, setRows + " 行");
await page.click("#bExportMd");
await page.waitForTimeout(400);
const exported = await page.$eval("#exportBox", (e) => e.textContent || "");
check("W06 导出 Markdown 提案（含需求与成本汇总）", /需求变更提案/.test(exported) && /FR-09/.test(exported) && /合并变更的总成本面/.test(exported), exported.slice(0, 100));
await page.click("#bExportJson");
await page.waitForTimeout(400);
const expJson = await page.$eval("#exportBox", (e) => e.textContent || "");
let parsed = null;
try { parsed = JSON.parse(expJson); } catch (e) { /* ignore */ }
check("W06 导出 JSON 可解析且含工单", !!parsed && parsed.changes && parsed.changes[0] && !!parsed.changes[0].workOrder, parsed ? parsed.schema : expJson.slice(0, 60));

// 窄屏：抽屉可达，目录不消失
await page.setViewportSize({ width: 390, height: 844 });
await page.waitForTimeout(300);
const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
check("W07 390px 无横向溢出", overflow === 0, overflow + "px");
const drawerBtn = await page.$("#drawerLeft") !== null;
check("W07 窄屏提供目录抽屉开关", drawerBtn, "无 #drawerLeft");
await page.click("#drawerLeft");
await page.waitForTimeout(300);
const leftVisible = await page.evaluate(() => {
  const e = document.querySelector(".g-left");
  return e ? e.getBoundingClientRect().left > -10 : false;
});
check("W07 窄屏可打开需求目录", leftVisible, "left=" + await page.evaluate(() => document.querySelector(".g-left").getBoundingClientRect().left));

// 截图
await page.setViewportSize({ width: 1440, height: 900 });
await page.waitForTimeout(300);
await page.click("#drawerLeft").catch(() => {});
await page.click('#reqList .g-req[data-req="FR-09"]');
await page.waitForTimeout(200);
await page.click('#tabs .g-tab[data-tab="impact"]');
await page.waitForTimeout(400);
await page.screenshot({ path: path.join(OUT, "screenshots", "workbench-impact-desktop-1440.png") });
await page.click('#tabs .g-tab[data-tab="work"]');
await page.waitForTimeout(400);
await page.screenshot({ path: path.join(OUT, "screenshots", "workbench-workorder-desktop-1440.png") });

await browser.close();

fs.writeFileSync(path.join(OUT, "verify-requirements.json"), JSON.stringify({
  schema: "ide1-requirement-workbench/verify/v1.0", generatedAt: new Date().toISOString(),
  total: results.length, passed: results.filter((r) => r.ok).length, failed: fails, results
}, null, 2), "utf8");
console.log("=== 需求工作台实测：" + results.filter((r) => r.ok).length + "/" + results.length + " 通过，失败 " + fails + " ===");
if (fails) process.exit(1);
