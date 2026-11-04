/**
 * .tooling/build.mjs — 构建可离线运行的单文件 index.html
 *
 * 交付物要求（v10.0 portable_html_delivery）：
 *   - 根目录 index.html 可 file:// 双击打开，核心浏览/原型/因果演示全部离线完成。
 *   - 禁止：fetch 本地 JSON、file:// ES module import、动态 Worker 加载、远程字体/CDN、任意 eval、外部服务。
 *   - 因此：models 以 JS 对象字面量内联；vendor 以经典 <script> 内联；我的源码是 IIFE，按序拼接。
 *   - 同一份内容复制到 dist/ 作为 static-http 发布产物。
 */
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";

const ROOT = path.resolve(process.argv[2] || ".");
const SRC = path.join(ROOT, "src");
const MODELS = path.join(ROOT, "models");
const VENDOR = path.join(ROOT, "vendor");
const DIST = path.join(ROOT, "dist");

// 依赖顺序：vendor 先（模块加载期就要读到全局），再是我方模块（IIFE 依赖顺序）
const VENDOR_ORDER = ["three.bundle.js", "konva.bundle.js", "graph.bundle.js"];
const SRC_ORDER = [
  "validate.js",
  "graph.js",
  "engine.js",
  "causal.js",
  "prompt.js",
  "diagrams.js",
  "tour.js",
  "spatial.js",
  "prototype.js",
  "app.js"
];
const MODEL_ORDER = [
  "evidence.json",
  "requirements.json",
  "architecture.json",
  "spatial.json",
  "cases.json",
  "causal-rules.json",
  "scenario-deltas.json",
  "prompts.json",
  "prototype.json",
  "diagram-definitions.json",
  "traceability.json",
  "tours.json",
  "schema.json"
];

/** 内联进 <script> 时必须转义结束标签，避免提前闭合 */
function safeInline(code) {
  return code.replace(/<\/script/gi, "<\\/script").replace(/<!--/g, "<\\!--");
}

function read(p) {
  if (!fs.existsSync(p)) throw new Error("missing required file: " + p);
  return fs.readFileSync(p, "utf8");
}

function sha256(s) {
  return crypto.createHash("sha256").update(s).digest("hex");
}


// ---- 语法门禁：任何 src/*.js 有语法错误都不得产出 index.html ----
// GEMARC_SYNTAX_GATE
const vm = await import("node:vm");
const syntaxProblems = [];
for (const f of [...SRC_ORDER]) {
  const p = path.join(SRC, f);
  try {
    new vm.Script(fs.readFileSync(p, "utf8"), { filename: p });
  } catch (e) {
    syntaxProblems.push(f + " => " + e.message);
  }
}
if (syntaxProblems.length) {
  console.error("BUILD ABORTED — src/ 存在语法错误：");
  syntaxProblems.forEach((s) => console.error("  " + s));
  process.exit(1);
}

// ---- 模型键名核对：src 里的 M.<key> 必须能在模型键清单里解析 ----
// 连字符的键（causal-rules / diagram-definitions）不能���点号访问，
// 写错会静默得到 undefined，被 || [] 兜底成空集合 —— 功能整条消失且零报错。
const modelKeys = new Set(MODEL_ORDER.map((f) => f.replace(/\.json$/, "")));
const srcText = SRC_ORDER.map((f) => fs.readFileSync(path.join(SRC, f), "utf8")).join("\n");
const dotAccess = new Set();
const dotRe = /\bM\.([A-Za-z_][A-Za-z0-9_]*)/g;
let dotMatch;
while ((dotMatch = dotRe.exec(srcText)) !== null) dotAccess.add(dotMatch[1]);
const bogusKeys = Array.from(dotAccess).filter((k) => !modelKeys.has(k));
if (bogusKeys.length) {
  console.error("BUILD ABORTED — src 里引用了不存在的模型键：" + bogusKeys.join(", "));
  console.error("  可用键：" + Array.from(modelKeys).sort().join(", "));
  process.exit(1);
}

// ---- 1) models → 一个对象字面量
const models = {};
for (const f of MODEL_ORDER) {
  const p = path.join(MODELS, f);
  const raw = read(p);
  try {
    JSON.parse(raw);
  } catch (e) {
    throw new Error("model JSON invalid: " + f + " => " + e.message);
  }
  const key = f.replace(/\.json$/, "");
  models[key] = JSON.parse(raw);
}
const modelsLiteral = "window.GemArcModels = " + JSON.stringify(models) + ";";

// ---- 2) vendor + src
const vendorBlocks = VENDOR_ORDER.map((f) => {
  const code = read(path.join(VENDOR, f));
  return "/* vendor: " + f + " */\n" + safeInline(code);
});
const srcBlocks = SRC_ORDER.map((f) => {
  const code = read(path.join(SRC, f));
  return "/* src: " + f + " */\n" + safeInline(code);
});
const css = read(path.join(SRC, "styles.css"));

// ---- 3) 组装 HTML
const rev = (models.evidence?.sourceRevisions?.[0]?.sha || "").slice(0, 7);
const html = `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>IDE1.0 · 5D 工程设计图集 v10.0</title>
<meta name="description" content="IDE1.0 5D 工程设计图集 v10.0 —— 0D–5D + Prototype，file:// 双击可用。事实源 IDE1.0 @ ${rev}。" />
<meta name="generator" content="ide1.5d-atlas build.mjs" />
<style>
${css}
</style>
</head>
<body>
<div id="app"></div>
<noscript>本图集需要 JavaScript 才能渲染 3D 场景与因果实验。</noscript>
<script>
${vendorBlocks.join("\n")}
</script>
<script>
${safeInline(modelsLiteral)}
</script>
<script>
${srcBlocks.join("\n")}
</script>
</body>
</html>
`;

const outIndex = path.join(ROOT, "index.html");
fs.writeFileSync(outIndex, html, "utf8");

// ---- 4) dist 镜像（static-http 发布产物）
fs.mkdirSync(DIST, { recursive: true });
fs.writeFileSync(path.join(DIST, "index.html"), html, "utf8");

// ---- 5) 构建报告
const report = {
  schema: "ide1.5d-atlas/build-report/v10.0",
  builtAt: new Date().toISOString(),
  revision: rev,
  entry: "index.html",
  publishDir: "dist/index.html",
  totalBytes: Buffer.byteLength(html, "utf8"),
  blocks: {
    css: Buffer.byteLength(css, "utf8"),
    models: Buffer.byteLength(modelsLiteral, "utf8"),
    vendor: vendorBlocks.map((b, i) => ({ file: VENDOR_ORDER[i], bytes: Buffer.byteLength(b, "utf8") })),
    src: srcBlocks.map((b, i) => ({ file: SRC_ORDER[i], bytes: Buffer.byteLength(b, "utf8") }))
  },
  sha256: {
    "index.html": sha256(html),
    "dist/index.html": sha256(html)
  },
  modelCounts: Object.fromEntries(
    Object.entries(models).map(([k, v]) => [
      k,
      Array.isArray(v) ? v.length
        : v && v.nodes ? `${v.nodes.length} nodes / ${(v.edges || []).length} edges`
        : v && v.evidence ? `${v.evidence.length} evidence`
        : v && v.cases ? `${v.cases.length} cases`
        : v && v.rules ? `${v.rules.length} rules`
        : v && v.diagrams ? `${v.diagrams.length} diagrams`
        : "—"
    ])
  )
};
fs.writeFileSync(path.join(ROOT, "build-report.json"), JSON.stringify(report, null, 2), "utf8");

console.log("built index.html  " + (report.totalBytes / 1024).toFixed(1) + " KB");
console.log("  sha256 " + report.sha256["index.html"]);
console.log("  dist/index.html  (same bytes)");
console.log("  models: " + JSON.stringify(report.modelCounts));
