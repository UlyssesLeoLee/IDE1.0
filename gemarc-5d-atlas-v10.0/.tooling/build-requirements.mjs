/**
 * .tooling/build-requirements.mjs — 生成"需求调整工作台"单文件 HTML
 *
 * 约束（与 5D 图集一致）：
 *   - 单文件、file:// 双击可开、零外部依赖（模型内联，无 fetch/CDN/ESM/eval）
 *   - src-requirements/app.js 必须先过语法门禁，否则不产出产物
 */
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import crypto from "node:crypto";

const ROOT = path.resolve(process.argv[2] || ".");
const MODELS = path.join(ROOT, "models");
const SRC = path.join(ROOT, "src-requirements");
const OUT = path.join(ROOT, "requirement-workbench.html");

const MODEL_ORDER = [
  "requirements.json", "architecture.json", "cases.json", "evidence.json",
  "traceability.json", "causal-rules.json", "prompts.json", "diagram-definitions.json"
];
const read = (p) => fs.readFileSync(p, "utf8");
const safeInline = (code) => code.replace(/<\/script/gi, "<\\/script").replace(/<!--/g, "<\\!--");

// ---- 语法门禁 ----
const appPath = path.join(SRC, "app.js");
try {
  new vm.Script(read(appPath), { filename: appPath });
} catch (e) {
  console.error("BUILD ABORTED — src-requirements/app.js 语法错误: " + e.message);
  process.exit(1);
}

// ---- 模型内联 ----
const models = {};
for (const f of MODEL_ORDER) {
  const p = path.join(MODELS, f);
  if (!fs.existsSync(p)) { console.error("BUILD ABORTED — 缺少模型: " + f); process.exit(1); }
  const raw = read(p);
  try { JSON.parse(raw); } catch (e) {
    console.error("BUILD ABORTED — 模型 JSON 非法: " + f + " => " + e.message);
    process.exit(1);
  }
  models[f.replace(/\.json$/, "")] = JSON.parse(raw);
}
const modelsLiteral = "window.Ide1Models = " + JSON.stringify(models) + ";";

const css = read(path.join(SRC, "styles.css"));
const app = read(appPath);
const rev = (models.evidence.sourceRevisions && models.evidence.sourceRevisions[0]) || {};
const revShort = String(rev.sha || "").slice(0, 7);

const html = `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>IDE1.0 · 需求调整工作台</title>
<meta name="description" content="需求调整工作台 —— 改需求而不是直接改代码：自动计算真实代码影响面并生成实现工单。离线单文件。" />
<meta name="generator" content="ide1-requirement-workbench build.mjs" />
<style>
${css}
</style>
</head>
<body>
<div id="app">
  <header class="g-head">
    <span class="g-mark">REQ</span>
    <div>
      <div class="g-title">IDE1.0 · 需求调整工作台</div>
      <div class="g-rev">事实源 IDE1.0 @ ${revShort} · ${rev.branch || "dev"} · 离线单文件</div>
    </div>
    <div class="g-head-right">
      <span class="g-dirty" id="dirty"></span>
      <button class="g-btn g-drawerbtn" id="drawerLeft" type="button" aria-label="打开/关闭需求列表">目录</button>
      <button class="g-btn g-drawerbtn" id="drawerRight" type="button" aria-label="打开/关闭变更摘要">摘要</button>
    </div>
  </header>

  <aside class="g-left">
    <div class="g-left-top">
      <input class="g-input" id="fSearch" type="search" placeholder="搜索需求 ID / 意图 / 状态…" aria-label="搜索需求" />
      <div class="g-row" id="cats">
        <button class="g-chipbtn active" data-cat="all" type="button">全部</button>
        <button class="g-chipbtn" data-cat="FR" type="button">功能 FR</button>
        <button class="g-chipbtn" data-cat="NFR" type="button">非功能 NFR</button>
        <button class="g-chipbtn" data-cat="IR" type="button">接口 IR</button>
        <button class="g-chipbtn" data-cat="CR" type="button">约束 CR</button>
      </div>
    </div>
    <div class="g-reqlist" id="reqList"></div>
  </aside>

  <main class="g-center">
    <nav class="g-tabs" id="tabs" role="tablist">
      <button class="g-tab active" data-tab="detail" role="tab" type="button" aria-selected="true">需求详情</button>
      <button class="g-tab" data-tab="impact" role="tab" type="button" aria-selected="false">影响面分析</button>
      <button class="g-tab" data-tab="work" role="tab" type="button" aria-selected="false">实现工单</button>
      <button class="g-tab" data-tab="set" role="tab" type="button" aria-selected="false">变更集</button>
    </nav>
    <div id="centerBody"></div>
  </main>

  <aside class="g-right" id="rightPane">
    <div class="g-rightbody" id="sideBody"></div>
  </aside>
</div>
<div id="toast" role="status" aria-live="polite"></div>
<script>
${safeInline(modelsLiteral)}
</script>
<script>
${safeInline(app)}
</script>
<script>
(function () {
  var dl = document.getElementById("drawerLeft");
  var dr = document.getElementById("drawerRight");
  if (dl) dl.addEventListener("click", function () {
    document.body.classList.toggle("g-drawer-left");
    document.body.classList.remove("g-drawer-right");
  });
  if (dr) dr.addEventListener("click", function () {
    document.body.classList.toggle("g-drawer-right");
    document.body.classList.remove("g-drawer-left");
  });
})();
</script>
</body>
</html>
`;

fs.writeFileSync(OUT, html, "utf8");
const sha = crypto.createHash("sha256").update(html).digest("hex");
console.log("built requirement-workbench.html  " + (html.length / 1024).toFixed(1) + " KB");
console.log("  sha256 " + sha);
console.log("  模型内联: " + Object.keys(models).join(", "));
