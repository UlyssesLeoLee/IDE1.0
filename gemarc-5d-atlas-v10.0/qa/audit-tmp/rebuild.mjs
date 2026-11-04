import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
const ROOT = "E:\\IDE1.0\\gemarc-5d-atlas-v10.0";
const OUT  = path.join(ROOT, "qa", "audit-tmp", "rebuilt");
const SRC = path.join(ROOT, "src"), MODELS = path.join(ROOT, "models"), VENDOR = path.join(ROOT, "vendor");
const VENDOR_ORDER = ["three.bundle.js","konva.bundle.js","graph.bundle.js"];
const SRC_ORDER = ["validate.js","graph.js","engine.js","causal.js","prompt.js","diagrams.js","tour.js","spatial.js","prototype.js","app.js"];
const MODEL_ORDER = ["evidence.json","requirements.json","architecture.json","spatial.json","cases.json","causal-rules.json","scenario-deltas.json","prompts.json","prototype.json","diagram-definitions.json","traceability.json","tours.json","schema.json"];
const safeInline = (c) => c.replace(/<\/script/gi, "<\\/script").replace(/<!--/g, "<\\!--");
const read = (p) => fs.readFileSync(p, "utf8");
const models = {};
for (const f of MODEL_ORDER) models[f.replace(/\.json$/,"")] = JSON.parse(read(path.join(MODELS,f)));
const modelsLiteral = "window.GemArcModels = " + JSON.stringify(models) + ";";
const vendorBlocks = VENDOR_ORDER.map(f => "/* vendor: "+f+" */\n"+safeInline(read(path.join(VENDOR,f))));
const srcBlocks = SRC_ORDER.map(f => "/* src: "+f+" */\n"+safeInline(read(path.join(SRC,f))));
const css = read(path.join(SRC,"styles.css"));
const rev = (models.evidence?.sourceRevisions?.[0]?.sha || "").slice(0,7);
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
fs.mkdirSync(OUT, {recursive:true});
fs.writeFileSync(path.join(OUT,"index.html"), html, "utf8");
const sha = crypto.createHash("sha256").update(html).digest("hex");
const orig = fs.readFileSync(path.join(ROOT,"index.html"));
const shaOrig = crypto.createHash("sha256").update(orig).digest("hex");
console.log("REBUILT bytes=" + Buffer.byteLength(html,"utf8") + " sha256=" + sha);
console.log("SHIPPED bytes=" + orig.length + " sha256=" + shaOrig);
console.log("BYTE_IDENTICAL=" + (sha === shaOrig));
