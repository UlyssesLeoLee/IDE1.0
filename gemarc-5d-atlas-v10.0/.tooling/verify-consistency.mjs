/**
 * .tooling/verify-consistency.mjs — 原型/模型 ↔ 真实代码 的一致性门禁
 *
 * 回答的问题：**图集画的（原型 + 模型）是不是真的对应 IDE1.0 的代码**。
 * 每条检查都从真实代码抽事实，再和 models/ 对撞；不一致即 FAIL。
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const PROJ = process.argv[3] || "E:/IDE1.0";
const rd = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, "models", f), "utf8"));
const arch = rd("architecture.json");
const ev = rd("evidence.json");
const pr = rd("prototype.json");

const findings = [];
const pass = (id, what, e) => console.log(`  ✓ ${id} ${what}  ← ${e}`);
const fail = (id, sev, where, expected, actual, impact, fix, vt) => {
  findings.push({ findingId: id, severity: sev, fileLine: where, expected, actual, impact, fixProposal: fix, verificationTest: vt, status: "OPEN" });
  console.log(`  ✗ ${id} [${sev}] ${where}\n      期望: ${expected}\n      实际: ${actual}`);
};

// ---------- 抽取真实事实 ----------
const webSrc = fs.readFileSync(path.join(PROJ, "crates/ide-shell-web/src/lib.rs"), "utf8");
const realHttp = new Set();
for (const m of webSrc.matchAll(/\("(GET|POST|PUT|DELETE)"\s*,\s*"([^"]+)"\)/g)) realHttp.add(m[1] + " " + m[2]);

const desktopSrc = fs.readFileSync(path.join(PROJ, "crates/ide-shell-desktop/src/lib.rs"), "utf8");
const ghStart = desktopSrc.indexOf("generate_handler![");
const ghBlock = desktopSrc.slice(ghStart, desktopSrc.indexOf("]", ghStart));
const realTauri = new Set();
for (const m of ghBlock.matchAll(/([a-z_]+(?:::[a-z_0-9]+)?)\s*,/g)) {
  const n = m[1].split("::").pop();
  if (n) realTauri.add(n);
}

const protoSrc = fs.readFileSync(path.join(PROJ, "crates/ide-shell-protocol/src/lib.rs"), "utf8");
const realRpc = new Set();
{
  const s = protoSrc.indexOf("match method {");
  for (const m of protoSrc.slice(s, s + 9000).matchAll(/^\s+"([a-z_]+)"\s*=>/gm)) realRpc.add(m[1]);
}

const editorHtml = fs.readFileSync(path.join(PROJ, "crates/ide-shell-web/src/editor.html"), "utf8");
const realActivity = [];
for (const m of editorHtml.matchAll(/data-pane="([a-z]+)"\s+data-tip="([^"]+)"/g)) realActivity.push({ pane: m[1], tip: m[2] });
const realEditorLines = editorHtml.split("\n").length;

// ---------- 模型侧事实 ----------
const modelHttp = new Set(arch.ports.filter((p) => p.properties && p.properties.path).map((p) => p.properties.method + " " + p.properties.path));
const modelTauri = new Set(arch.ports.filter((p) => p.properties && p.properties.command).map((p) => p.properties.command));
const modelRpc = new Set(arch.ports.filter((p) => p.properties && p.properties.method && /^P-RPC-/.test(p.id)).map((p) => p.properties.method));

console.log("=== C-01 HTTP 路由双向一致 ===");
{
  const missModel = [...realHttp].filter((r) => !modelHttp.has(r));
  const fake = [...modelHttp].filter((r) => !realHttp.has(r));
  if (missModel.length) fail("CC-01", "HIGH", "models/architecture.json", "模型登记全部真实 HTTP 路由", "缺 " + missModel.length + " 条：" + missModel.slice(0, 5).join(", "), "2D 画布看不到这些真实接口，改需求时会漏算", "跑 node .tooling/sync-ports.mjs 由代码生成", "node .tooling/verify-consistency.mjs");
  else if (fake.length) fail("CC-01", "HIGH", "models/architecture.json", "模型登记的 HTTP 路由都真实存在", "虚构 " + fake.length + " 条：" + fake.slice(0, 5).join(", "), "图集画出不存在的接口", "修正模型或删端口", "node .tooling/verify-consistency.mjs");
  else pass("CC-01", "HTTP 路由双向一致", realHttp.size + " 条全等");
}

console.log("=== C-02 Tauri command 双向一致 ===");
{
  const miss = [...realTauri].filter((c) => !modelTauri.has(c));
  const fake = [...modelTauri].filter((c) => !realTauri.has(c));
  if (miss.length) fail("CC-02", "HIGH", "models/architecture.json", "模型登记全部真实 Tauri command", "缺 " + miss.length + " 条：" + miss.slice(0, 5).join(", "), "桌面侧真实接口在图集中不可见", "跑 sync-ports.mjs", "node .tooling/verify-consistency.mjs");
  else if (fake.length) fail("CC-02", "HIGH", "models/architecture.json", "模型登记的 Tauri command 都真实存在", "虚构 " + fake.join(", "), "图集画出不可 invoke 的命令", "修正模型", "node .tooling/verify-consistency.mjs");
  else pass("CC-02", "Tauri command 双向一致", realTauri.size + " 条全等（以 generate_handler! 注册表为准）");
}

console.log("=== C-03 JSON-RPC 方法双向一致 ===");
{
  const miss = [...realRpc].filter((m) => !modelRpc.has(m));
  const fake = [...modelRpc].filter((m) => !realRpc.has(m));
  if (miss.length || fake.length) fail("CC-03", "HIGH", "models/architecture.json", "JSON-RPC 方法双向一致", "缺 " + miss.join(",") + " / 多 " + fake.join(","), "AI 集成面描述不可信", "跑 sync-ports.mjs", "node .tooling/verify-consistency.mjs");
  else pass("CC-03", "JSON-RPC 方法双向一致", realRpc.size + " 条全等");
}

console.log("=== C-04 原型屏幕路由必须在真实路由表中 ===");
{
  const bad = [];
  for (const g of Object.keys(pr.sourceScreens)) {
    const grp = pr.sourceScreens[g];
    for (const s of (grp.screens || [])) {
      if (s.status === "Designed") continue;              // 示范屏允许不存在
      const paths = (s.route.match(/(?:GET|POST|PUT|DELETE)\s+(\/[^\s·]+)/g) || []).map((x) => x.replace(/^\w+\s+/, "").split("?")[0]);
      const list = paths.length ? paths : (s.route.match(/\/[a-zA-Z0-9_\-/.]+/g) || []).map((x) => x.split("?")[0]);
      for (const p of list) {
        const hit = [...realHttp].some((r) => r.endsWith(" " + p));
        if (!hit) bad.push(s.id + " → " + p);
      }
    }
  }
  if (bad.length) fail("CC-04", "HIGH", "models/prototype.json", "原型屏幕的路由必须真实存在", bad.join(" | "), "原型画了不存在的页面", "修正屏幕 route 或标 Designed", "node .tooling/verify-consistency.mjs");
  else pass("CC-04", "原型屏幕路由均存在于真实路由表", "实屏幕全部命中");
}

console.log("=== C-05 原型 UI 结构 == editor.html 实测 ===");
{
  const struct = pr.realScreenStructure || {};
  const modelAct = (struct.activityBar || []).slice();
  const realTips = realActivity.map((a) => a.tip);
  const same = modelAct.length === realTips.length && modelAct.every((t, i) => t === realTips[i]);
  if (!same) {
    fail("CC-05", "HIGH", "models/prototype.json", "原型声明的 Activity Bar 与代码实测完全一致", "模型 " + JSON.stringify(modelAct) + " / 代码 " + JSON.stringify(realTips), "原型画出与真实产品不同的界面结构", "按 editor.html 实测更新原型结构", "node .tooling/verify-consistency.mjs");
  } else {
    // 屏幕 sections 必须包含真实面板
    const sc = pr.sourceScreens.ideShellWeb.screens.find((s) => s.id === "SC-EDITOR");
    const miss = realTips.filter((t) => !(sc.sections || []).some((x) => x.indexOf(t) !== -1));
    if (miss.length) fail("CC-05", "HIGH", "models/prototype.json", "SC-EDITOR 的 sections 必须覆盖真实面板", "缺 " + miss.join(","), "原型漏画真实面板", "补 sections", "node .tooling/verify-consistency.mjs");
    else pass("CC-05", "原型 UI 结构与代码实测一致", realTips.join("/") + "，minimap=" + (struct.minimap === false ? "false" : "?"));
  }
}

console.log("=== C-06 证据可核验（路径存在 + 行区间有效 + 摘要命中）===");
{
  const badPath = [], badRange = [], weak = [];
  for (const e of ev.evidence) {
    if (!e.path) continue;
    const abs = path.join(PROJ, e.path);
    if (!fs.existsSync(abs)) { badPath.push(e.id + " → " + e.path); continue; }
    if (e.startLine) {
      const lines = fs.readFileSync(abs, "utf8").split("\n");
      if (e.startLine > lines.length) { badRange.push(e.id + " 行号 " + e.startLine + " 超出文件 " + lines.length); continue; }
      const seg = lines.slice(e.startLine - 1, Math.min(e.endLine || e.startLine, lines.length)).join("\n");
      if (!seg.trim()) { badRange.push(e.id + " 区间为空"); continue; }
      // 启发式：摘要里出现的**代码标识符或路径片段**应能在区间里找到。
      // 注意排除两类假阴性：① 摘要里的斜杠串（"/a/b/c"）并非文件中的一行；
      // ② 纯中文摘要抽不出 ASCII 标识符 → 不算弱证据。
      const toks = (e.summary.match(/[A-Za-z_][A-Za-z0-9_]{3,}/g) || [])
        .filter((t) => !/^(Implemented|Designed|Unknown|Absent|Inferred|Present|TRUE|FALSE)$/.test(t));
      const baseName = (e.path || "").split("/").pop().replace(/\.[a-z]+$/, "");
      const hit = toks.some((t) => seg.indexOf(t) !== -1)
        || (baseName && baseName.length >= 4 && seg.indexOf(baseName.replace(/^_/, "")) !== -1);
      if (!hit && toks.length) weak.push(e.id + " 摘要关键词未出现在区间内（" + toks.slice(0, 3).join(",") + "）");
    }
  }
  if (badPath.length) fail("CC-06", "HIGH", "models/evidence.json", "证据路径必须存在", badPath.join(" | "), "证据指向不存在的文件", "修正或删除该证据", "node .tooling/verify-consistency.mjs");
  else if (badRange.length) fail("CC-06", "HIGH", "models/evidence.json", "证据行区间必须有效", badRange.join(" | "), "证据指向不存在的行", "修正行号", "node .tooling/verify-consistency.mjs");
  else if (weak.length > ev.evidence.length * 0.25) fail("CC-06", "MEDIUM", "models/evidence.json", "证据摘要应能在其行区间里找到依据（≤25% 例外）", weak.length + " 条弱证据：" + weak.slice(0, 4).join(" | "), "证据与代码脱节", "复核这些证据的摘要与行号", "node .tooling/verify-consistency.mjs");
  else pass("CC-06", "证据全部可核验", ev.evidence.length + " 条：路径存在、区间有效、弱证据 " + weak.length + " 条");
}

console.log("=== C-07 无过期计数（editor.html 行数等）===");
{
  const fe = arch.nodes.find((n) => n.id === "N-FE-EDITORHTML");
  const declared = fe && fe.properties ? fe.properties.lines : null;
  if (declared && declared !== realEditorLines) {
    fail("CC-07", "HIGH", "models/architecture.json", "模型声明的行数必须等于真实行数", "模型 " + declared + " / 真实 " + realEditorLines, "图集引用了过期的规模数据", "按实测更新", "node .tooling/verify-consistency.mjs");
  } else {
    // 扫描全模型里可能过期的 "4 panels" 之类
    const stale = [];
    const scan = (obj, pathStr) => {
      if (typeof obj === "string") { if (/Activity Bar \(4/.test(obj)) stale.push(pathStr + " → " + obj); return; }
      if (obj && typeof obj === "object") Object.keys(obj).forEach((k) => scan(obj[k], pathStr + "." + k));
    };
    scan(arch, "architecture.json"); scan(pr, "prototype.json"); scan(rd("spatial.json"), "spatial.json");
    if (stale.length) fail("CC-07", "HIGH", "models/*.json", "不得残留与代码不符的结构描述", stale.join(" | "), "原型与代码结构不一致", "按实测更新", "node .tooling/verify-consistency.mjs");
    else pass("CC-07", "无过期结构/规模声明", "editor.html 实测 " + realEditorLines + " 行，Activity Bar 描述已对齐");
  }
}

console.log("=== C-08 端口 ID 唯一 ===");
{
  const ids = arch.ports.map((p) => p.id);
  const dup = ids.filter((v, i) => ids.indexOf(v) !== i);
  if (dup.length) fail("CC-08", "HIGH", "models/architecture.json", "端口 ID 必须唯一", dup.length + " 个重复：" + [...new Set(dup)].slice(0, 5).join(","), "同一接口被登记多次，影响面统计翻倍", "跑 sync-ports.mjs 去重", "node .tooling/verify-consistency.mjs");
  else pass("CC-08", "端口 ID 唯一", arch.ports.length + " 个端口");
}

const highs = findings.filter((f) => f.severity === "HIGH" && f.status === "OPEN");
console.log("\n=== 一致性核对结论：发现 " + findings.length + " 项（OPEN HIGH: " + highs.length + "）===");
fs.writeFileSync(path.join(ROOT, "qa", "consistency-report.json"), JSON.stringify({
  schema: "ide1-5d-atlas/consistency/v10.0", generatedAt: new Date().toISOString(),
  facts: { http: realHttp.size, tauri: realTauri.size, rpc: realRpc.size, editorLines: realEditorLines, activityBar: realActivity },
  findings, verdict: highs.length ? "FAIL" : "PASS"
}, null, 2), "utf8");
if (highs.length) process.exit(1);
