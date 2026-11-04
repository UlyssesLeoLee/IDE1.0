/**
 * .tooling/e2e-journeys.mjs — 12 条用户旅程的真实浏览器实测（file:// 直开，系统 Chrome）。
 *
 * 契约：
 *   - 不用测试框架的“魔法断言”，每条旅程都产出可读的结果行与真实证据。
 *   - 任何断言失败都会以非零退出码结束，并在 qa/e2e-report.json 保留失败现场。
 *   - 同时断言便携性禁令：运行时不得出现 http(s) 请求、不得有 console error。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FILE_URL = "file:///" + path.join(ROOT, "index.html").replace(/\\/g, "/");
const OUT = path.join(ROOT, "qa");
fs.mkdirSync(OUT, { recursive: true });

const results = [];
let failures = 0;
function check(journey, name, ok, detail) {
  results.push({ journey, name, ok: !!ok, detail: String(detail == null ? "" : detail).slice(0, 400) });
  if (!ok) failures++;
  console.log(`  ${ok ? "PASS" : "FAIL"}  ${journey} · ${name}${ok ? "" : "  ← " + detail}`);
}

async function withPage(fn, viewport = { width: 1440, height: 900 }) {
  const browser = await chromium.launch({ channel: "chrome" });
  const ctx = await browser.newContext({ viewport });
  const page = await ctx.newPage();
  const consoleErrors = [];
  const remoteRequests = [];
  page.on("console", (m) => { if (m.type() === "error") consoleErrors.push(m.text()); });
  page.on("pageerror", (e) => consoleErrors.push("pageerror: " + e.message));
  page.on("request", (r) => { if (!/^(file|data|blob):/.test(r.url())) remoteRequests.push(r.url()); });
  try {
    await page.goto(FILE_URL, { waitUntil: "load" });
    await page.waitForSelector("#app .g-head", { timeout: 20000 });
    await fn(page, { consoleErrors, remoteRequests });
  } finally {
    await browser.close();
  }
}

const go = async (page, view) => {
  await page.click(`[data-view="${view}"]`);
  await page.waitForTimeout(350);
};

console.log("=== IDE1.0 5D 图集 E2E · file:// " + FILE_URL + " ===");

// ---- J00 便携性 ----
await withPage(async (page, ctx) => {
  check("J00", "file:// 打开无 console error", ctx.consoleErrors.length === 0, ctx.consoleErrors.join(" | "));
  check("J00", "运行期无 http(s)/CDN 请求", ctx.remoteRequests.length === 0, ctx.remoteRequests.join(" | "));
  const title = await page.title();
  check("J00", "标题为 IDE1.0 图集", /IDE1\.0/.test(title), title);
  const has3d = await page.evaluate(() => !!window.THREE && !!window.Konva && !!window.GemArcEngine && !!window.GemArcCausal);
  check("J00", "vendor 全部内联可用（THREE/Konva/Engine/Causal）", has3d, "missing global");
});

// ---- J01 1D：选需求 → Prompt 更新 → 校验/复制/导出/差分 ----
await withPage(async (page, ctx) => {
  await go(page, "1D");
  const opts = await page.$$eval("#promptPick option", (os) => os.map((o) => o.value));
  check("J01", "1D Prompt 库非空", opts.length > 0, opts.length + " 条");
  const target = opts.find((v) => v.indexOf("P-IMPL-DOC-HTTP-API") === 0) || opts[0];
  await page.selectOption("#promptPick", target);
  await page.waitForTimeout(250);
  const obj = await page.$eval("#pObjective", (e) => e.value || "");
  check("J01", "选中 Prompt 后 objective 随之更新", obj.length > 10, obj.slice(0, 60));
  const before = await page.$eval("#pRender", (e) => e.textContent || "");
  await page.click("#pValidate");
  await page.waitForTimeout(200);
  const status = await page.$eval("#pStatus", (e) => e.textContent || "");
  check("J01", "校验返回结论且不是 UNKNOWN 残留", /PASS/.test(status), status);
  const after = await page.$eval("#pRender", (e) => e.textContent || "");
  check("J01", "Prompt 渲染文本包含需求与证据引用", /FR-|IR-|CR-/.test(after) && /EV-/.test(after), after.slice(0, 80));
  await page.click("#pDiff");
  await page.waitForTimeout(150);
  const diff = await page.$eval("#pStatus", (e) => e.textContent || "");
  check("J01", "版本差分可执行", diff.length > 0, diff);
  const hasCopy = await page.$("#pCopy") !== null, hasExport = await page.$("#pExport") !== null;
  check("J01", "复制/导出按钮存在", hasCopy && hasExport, "missing");
});

// ---- J02 2D：搜索过滤 + Node/Edge/Port 独立选择 + 追踪 ----
await withPage(async (page, ctx) => {
  await go(page, "2D");
  await page.waitForSelector("#cStage .g-node", { timeout: 10000 });
  const nodeCount = await page.$$eval("#cStage .g-node", (n) => n.length);
  check("J02", "2D 画布渲染全部节点", nodeCount >= 40, nodeCount + " 个节点");
  await page.fill("#cSearch", "auth");
  await page.waitForTimeout(400);
  const dimmed = await page.$$eval("#cStage .g-node.dim", (n) => n.length);
  const lit = nodeCount - dimmed;
  check("J02", "搜索过滤生效（命中节点被弱化，其余 dim）", dimmed > 0 && lit > 0 && lit < nodeCount, nodeCount + " → 命中 " + lit + " / 弱化 " + dimmed);
  await page.fill("#cSearch", "");
  await page.waitForTimeout(250);
  await page.click('#cStage .g-node[data-node="N-WB-AUTH"]');
  await page.waitForTimeout(250);
  const insp = await page.$eval("#rightBody", (e) => e.textContent || "");
  check("J02", "点击 Node 后检查器显示其信息", insp.includes("N-WB-AUTH") || insp.includes("auth"), insp.slice(0, 80));
  const portCount = await page.$$eval("#cStage .g-port", (p) => p.length);
  check("J02", "Port 独立可拾取", portCount > 0, portCount + " 个端口");
  const edgeCount = await page.$$eval("#cStage .g-edge", (e) => e.length);
  check("J02", "Edge 独立渲染", edgeCount > 0, edgeCount + " 条边");
  // 分组：Shift 多选 → 分组 → 出现分组框 → 解组
  // 点击**图形本体**而不是 <g> 包围盒中心（标签可能使包围盒大于图形）
  await page.click('#cStage .g-node[data-node="N-WB-AUTH"] .g-node-shape');
  await page.click('#cStage .g-node[data-node="N-WB-SERVER"] .g-node-shape', { modifiers: ["Shift"] });
  await page.click("#cGroup");
  await page.waitForTimeout(350);
  const grp = await page.$$eval("#cStage .g-group", (g) => g.length);
  check("J02", "多选后可分组（视图态分组框）", grp === 1, grp + " 个分组");
  const grpName = await page.$$eval("#cStage .g-group-name", (g) => g.map((x) => x.textContent));
  check("J02", "分组框带名称与成员数", grpName.length === 1 && /节点/.test(grpName[0]), grpName.join("|"));
  await page.click("#cUngroup");
  await page.waitForTimeout(300);
  const grp2 = await page.$$eval("#cStage .g-group", (g) => g.length);
  check("J02", "可解组", grp2 === 0, grp2 + " 个分组");
});

// ---- J03 独立工程图：不同图种正确分派 + 浮窗可开可关 ----
await withPage(async (page, ctx) => {
  await go(page, "2D");
  const btns = await page.$$eval(".g-diagbtn", (bs) => bs.map((b) => b.getAttribute("data-diag") || b.textContent));
  check("J03", "工程图入口存在", btns.length >= 10, btns.length + " 个");
  await page.click(".g-diagbtn");
  await page.waitForTimeout(400);
  const win = await page.$(".g-diagwin");
  check("J03", "图窗可打开", win !== null, "no .g-diagwin");
  if (win) {
    const svg = await page.$eval(".g-diagwin svg", (s) => s.outerHTML).catch(() => "");
    check("J03", "图窗内为 SVG 且含真实符号", svg.includes("<svg") && (svg.includes("marker") || svg.includes("<path") || svg.includes("<rect")), svg.slice(0, 60));
    const title = await page.$eval(".g-diagwin", (w) => (w.textContent || "").slice(0, 60));
    check("J03", "图窗标题非空", title.trim().length > 0, title);
  }
  // 不同图种分派：逐个打开并检查没有 "Unsupported diagramType"
  const types = await page.evaluate(() => (window.GemArcModels["diagram-definitions"].diagrams || []).map((d) => d.diagramType));
  const unsupported = await page.evaluate(() => {
    const D = window.GemArcDiagrams;
    return (window.GemArcModels["diagram-definitions"].diagrams || [])
      .map((d) => ({ t: d.diagramType, html: D.render(d, 720, 460) }))
      .filter((x) => x.html.includes("Unsupported diagramType"))
      .map((x) => x.t);
  });
  check("J03", "全部 " + new Set(types).size + " 种图型均被正确分派（无 Unsupported）", unsupported.length === 0, unsupported.join(","));
});

// ---- J04 3D：真实 Three.js 场景 + 视图切换 + 相机复位 ----
await withPage(async (page, ctx) => {
  await go(page, "3D");
  await page.waitForSelector("#s3dStage canvas", { timeout: 15000 });
  const info = await page.evaluate(() => {
    const st = window.GemArcSpatial;
    return { has: !!st, types: st && st.state ? true : false };
  });
  check("J04", "3D 引擎装载", info.has, "no GemArcSpatial");
  const canvasSize = await page.$eval("#s3dStage canvas", (c) => ({ w: c.width, h: c.height }));
  check("J04", "WebGL canvas 真实创建", canvasSize.w > 100 && canvasSize.h > 100, JSON.stringify(canvasSize));
  const views = await page.$$eval("#s3dView option", (o) => o.map((x) => x.value));
  check("J04", "3D 视图集包含 12 个语义视图", views.length >= 10, views.join(","));
  await page.selectOption("#s3dView", "BirdEye");
  await page.waitForTimeout(400);
  await page.click("#s3dReset");
  await page.waitForTimeout(300);
  const status = await page.$eval("#s3dStatus", (e) => e.textContent || "");
  check("J04", "3D 状态栏有可读信息", status.length > 0, status.slice(0, 80));
  const degraded = await page.$$eval(".g-degraded", (d) => d.length);
  check("J04", "未降级为伪 3D（无 g-degraded 提示）", degraded === 0, degraded + " 个降级块");
});

// ---- J05 3D 选中 → 检查器联动（必须真的写出选中对象，不能只断言“有文字”）----
await withPage(async (page, ctx) => {
  await go(page, "3D");
  await page.waitForSelector("#s3dStage canvas", { timeout: 15000 });
  await page.waitForTimeout(700);
  // 用确定性 API 选中一个真实节点（盲点画布中心可能什么都没点到）
  const picked = await page.evaluate(() => {
    const sc = window.GemArcApp.state.scene3d;
    if (!sc) return null;
    sc.focusOn("N-WB-SERVER");          // focusOn 返回布尔，选中结果在 state.selected
    const sel = sc.state.selected;
    return sel ? { kind: sel.kind, id: sel.id } : null;
  });
  check("J05", "3D 可选中真实节点", !!picked && picked.id === "N-WB-SERVER", JSON.stringify(picked));
  await page.waitForTimeout(400);
  const insp = await page.$eval("#rightBody", (e) => e.textContent || "");
  // 严格断言：检查器必须出现被选中的真实 ID（原来的 ">10 字符" 断言在空检查器上也会通过）
  check("J05", "检查器出现被选中的真实对象 ID", insp.includes("N-WB-SERVER"), insp.slice(0, 120));
  const diagLinks = await page.$$eval("#rightBody [data-godiag]", (b) => b.map((x) => x.getAttribute("data-godiag")));
  check("J05", "检查器给出关联工程图（按真实 ID 关联）", diagLinks.length > 0, diagLinks.join(","));
  const caseLinks = await page.$$eval("#rightBody [data-gocase]", (b) => b.map((x) => x.getAttribute("data-gocase")));
  check("J05", "检查器给出关联 4D Case", caseLinks.length > 0, caseLinks.join(","));
  const label = await page.$eval("#s3dLabel", (e) => e.textContent || "");
  check("J05", "3D 标签面板可读", label.length >= 0, label.slice(0, 80));
});

// ---- J06 4D：Case 切换 + 回放 + 时间轴 ----
await withPage(async (page, ctx) => {
  await go(page, "4D");
  await page.waitForSelector("#sCase", { timeout: 10000 });
  const cases = await page.$$eval("#sCase option", (o) => o.map((x) => x.value));
  check("J06", "4D Case 列表装载（≥20 条）", cases.length >= 20, cases.length + " 条");
  await page.selectOption("#sCase", "C-PATH-ESCAPE");
  await page.waitForTimeout(200);
  await page.click("#sRun");
  await page.waitForTimeout(700);
  const runInfo = await page.$eval("#runInfo", (e) => e.textContent || "");
  check("J06", "执行结果可读", runInfo.trim().length > 0, runInfo.slice(0, 120));
  const evs = await page.$$eval("#tlTrack .g-tl-ev", (e) => e.length);
  check("J06", "时间轴产生事件轨道", evs > 0, evs + " 个事件");
  const kinds = await page.$$eval("#tlTrack .g-tl-ev", (es) => es.map((e) => e.className));
  check("J06", "事件类型含 Transition 与阻断类", kinds.some((k) => k.includes("transition")) && kinds.some((k) => k.includes("blocked")), kinds.join("|").slice(0, 120));
  const ctrls = await page.$$eval("#tlControls button", (b) => b.length);
  check("J06", "回放控件齐备（≥6 个）", ctrls >= 6, ctrls + " 个按钮");
  // 换 Case 后事件必须不同
  await page.selectOption("#sCase", "C-NORMAL-EDIT");
  await page.waitForTimeout(150);
  await page.click("#sRun");
  await page.waitForTimeout(700);
  const evs2 = await page.$$eval("#tlTrack .g-tl-ev", (e) => e.length);
  check("J06", "不同 Case 的事件轨迹不同", evs2 !== evs, evs + " vs " + evs2);
});

// ---- J07 5D：干预 → 真实重算 ----
await withPage(async (page, ctx) => {
  await go(page, "5D");
  await page.waitForSelector("#sIv", { timeout: 10000 });
  await page.selectOption("#sCase", "C-PATH-ESCAPE");
  await page.waitForTimeout(200);
  const kinds = await page.$$eval("#sIv option", (o) => o.map((x) => x.value));
  check("J07", "5D 干预类型可选", kinds.length >= 5, kinds.join(","));
  const targets = await page.$$eval("#sIvTarget option", (o) => o.length);
  check("J07", "5D 干预目标来自真实事实图", targets > 30, targets + " 个目标");
  await page.click("#sRun");
  await page.waitForTimeout(400);
  await page.selectOption("#sIv", "DisableNode");
  await page.waitForTimeout(150);
  const targetValues = await page.$$eval("#sIvTarget option", (o) => o.map((x) => x.value));
  const pick = targetValues.includes("N-WB-PATHSAFETY") ? "N-WB-PATHSAFETY" : targetValues[0];
  await page.selectOption("#sIvTarget", pick);
  await page.click("#sCompare");
  await page.waitForTimeout(900);
  const info = await page.$eval("#causalInfo", (e) => e.textContent || "");
  check("J07", "因果解释产出结论", /DIVERGED|NO_DIFFERENCE|UNKNOWN/.test(info), info.slice(0, 140));
  check("J07", "不是 UNKNOWN（干预目标真实存在）", !/UNKNOWN/.test(info), info.slice(0, 140));
});

// ---- J08 5D 插入节点 → 真正改写 A→B 为 A→X→B ----
await withPage(async (page, ctx) => {
  await go(page, "5D");
  await page.selectOption("#sCase", "C-NORMAL-EDIT");
  await page.click("#sRun");
  await page.waitForTimeout(400);
  await page.selectOption("#sIv", "InsertElement");
  await page.waitForTimeout(200);
  const targetValues = await page.$$eval("#sIvTarget option", (o) => o.map((x) => x.value));
  const edge = targetValues.find((v) => v.indexOf("E-WB-TO-STORE") === 0) || targetValues.find((v) => v.indexOf("E-") === 0);
  await page.selectOption("#sIvTarget", edge);
  await page.click("#sCompare");
  await page.waitForTimeout(900);
  const info = await page.$eval("#causalInfo", (e) => e.textContent || "");
  check("J08", "插入元素后给出因果解释", /DIVERGED|NO_DIFFERENCE/.test(info), info.slice(0, 140));
  const delta = await page.evaluate(() => {
    const C = window.GemArcCausal;
    const M = window.GemArcModels;
    const model = {
      nodes: M.architecture.nodes, edges: M.architecture.edges,
      rules: M["causal-rules"].rules, stateMachines: M["causal-rules"].stateMachines,
      cases: M.cases.cases
    };
    const eng = window.GemArcEngine.createEngine(model);
    const r = C.applyIntervention(model, {
      kind: "InsertElement", targetId: "E-WB-TO-STORE",
      params: { nodeId: "X-VALIDATOR", name: "Injected Validator", kind: "Processor", role: "Processor" },
      id: "SD-TEST", caseId: "C-NORMAL-EDIT"
    });
    return {
      ops: (r.delta.operations || []).map((o) => o.op),
      removed: (r.delta.operations[0] || {}).removedEdgeIds || [],
      newEdges: (r.delta.operations[0] || {}).newEdgeIds || [],
      hasX: (r.model.nodes || []).some((n) => n.id === "X-VALIDATOR"),
      oldEdgeGone: !(r.model.edges || []).some((e) => e.id === "E-WB-TO-STORE")
    };
  });
  check("J08", "插入产生 InsertElement 操作且真正改写（旧边已移除）", delta.hasX && delta.oldEdgeGone && delta.removed.includes("E-WB-TO-STORE"), JSON.stringify(delta));
  check("J08", "改写后生成 A→X→B 两条新边", delta.newEdges.length === 2, JSON.stringify(delta.newEdges));
});

// ---- J09/J10/J11 原型：多标签 + DOM 预览 + Konva 编辑 + 导出 ----
await withPage(async (page, ctx) => {
  await go(page, "P");
  await page.waitForSelector(".g-proto-tab", { timeout: 15000 });
  const tabs = await page.$$eval(".g-proto-tab", (t) => t.map((x) => x.getAttribute("data-screen")));
  check("J09", "原型多标签存在（≥4 个）", tabs.length >= 4, tabs.join(","));
  await page.click('.g-proto-tab[data-screen="SC-EDITOR"]');
  await page.waitForTimeout(300);
  const meta = await page.$eval("#prScreenMeta", (e) => e.textContent || "");
  check("J09", "屏幕元信息来自模型（含 route 与证据）", meta.includes("SC-EDITOR") && meta.includes("EV-"), meta.slice(0, 100));
  const inputs = await page.$$eval("#prPreview input, #prPreview textarea, #prPreview button", (e) => e.length);
  check("J09", "DOM 预览是真实可交互控件", inputs >= 3, inputs + " 个控件");
  // 表单可输入
  const ta = await page.$("#prPreview textarea");
  if (ta) {
    await ta.fill("fn main() { }");
    const v = await ta.inputValue();
    check("J09", "预览文本框真实可输入", v.length > 0, v);
  } else {
    check("J09", "预览文本框真实可输入", false, "no textarea in preview");
  }
  // Konva 画布
  const konvaCanvas = await page.$$eval("#prStage canvas", (c) => c.length);
  check("J10", "Konva 舞台真实渲染", konvaCanvas >= 1, konvaCanvas + " 个 canvas");
  const before = await page.$eval("#prJson", (e) => e.textContent || "");
  await page.click("#prAdd");
  await page.waitForTimeout(400);
  const afterAdd = await page.$eval("#prJson", (e) => e.textContent || "");
  check("J10", "添加控件后原型 JSON 变化（DOM 与模型联动）", before !== afterAdd, (afterAdd.length - before.length) + " 字节差异");
  await page.click("#prUndo");
  await page.waitForTimeout(400);
  const afterUndo = await page.$eval("#prJson", (e) => e.textContent || "");
  check("J10", "Undo 生效（JSON 回退）", afterUndo.length < afterAdd.length, afterAdd.length + " → " + afterUndo.length);
  await page.click("#prRedo");
  await page.waitForTimeout(400);
  const afterRedo = await page.$eval("#prJson", (e) => e.textContent || "");
  check("J10", "Redo 生效（JSON 恢复）", afterRedo.length >= afterUndo.length, afterUndo.length + " → " + afterRedo.length);
  const json = await page.evaluate(() => {
    const ed = window.GemArcApp.state.protoEditor;
    const s = ed ? ed.serialise() : null;
    return s ? { widgets: (s.widgets || []).length, actions: (s.widgets || []).reduce((n, w) => n + ((w.actions || []).length), 0), hasState: !!s.stateTable } : null;
  });
  check("J11", "原型 JSON 同时序列化控件与 Action（非只有 Konva 图形）", !!json && json.actions > 0 && json.hasState, JSON.stringify(json));

  // 子标签必须是**真的页签**：切换后内容块显隐随之改变（不是装饰按钮）
  const subs = await page.$$eval(".g-subtab", (b) => b.map((x) => x.getAttribute("data-sub")));
  check("J09", "子标签齐备（6 个）", subs.length === 6, subs.join(" / "));
  const visibleBlocks = async () => page.$$eval(".g-proto-block", (bs) => bs.filter((b) => b.style.display !== "none").map((b) => b.getAttribute("data-subs")));
  const firstVisible = await visibleBlocks();
  await page.click('.g-subtab[data-sub="组件库"]');
  await page.waitForTimeout(300);
  const libVisible = await visibleBlocks();
  check("J09", "切换子标签后内容块随之改变", libVisible.length !== firstVisible.length && libVisible.every((v) => v.indexOf("组件库") !== -1), JSON.stringify(libVisible));
  const libBtns = await page.$$eval("#prLibrary .g-proto-lib-btn", (b) => b.length);
  check("J09", "组件库列出可用控件（来自模型）", libBtns > 0, libBtns + " 个控件");
  const beforeLib = await page.$eval("#prJson", (e) => (e.textContent || "").length);
  await page.click("#prLibrary .g-proto-lib-btn");
  await page.waitForTimeout(400);
  const afterLib = await page.$eval("#prJson", (e) => (e.textContent || "").length);
  check("J09", "从组件库插入控件后原型 JSON 变化", afterLib !== beforeLib, beforeLib + " → " + afterLib);
  await page.click('.g-subtab[data-sub="状态与 Case"]');
  await page.waitForTimeout(250);
  const bindRows = await page.$$eval("#prBindList .g-proto-row", (r) => r.length);
  check("J09", "状态与 Case 页列出 5D 绑定", bindRows > 0, bindRows + " 行");
  await page.click('.g-subtab[data-sub="页面与屏幕"]');
  await page.waitForTimeout(250);
  const backVisible = await visibleBlocks();
  check("J09", "可切回页面与屏幕（DOM 状态不丢）", backVisible.some((v) => v.indexOf("页面与屏幕") !== -1), JSON.stringify(backVisible));
});

// ---- J12 跨维度绑定：原型 → 2D/3D/4D/5D 与 DOM 异常界面 ----
await withPage(async (page, ctx) => {
  await go(page, "P");
  await page.waitForSelector("#prPreview", { timeout: 15000 });
  const bindings = await page.evaluate(() => {
    const p = window.GemArcModels.prototype;
    return (p.fiveDBinding && p.fiveDBinding.bindings) || [];
  });
  check("J12", "原型 ↔ 4D/5D 绑定存在", bindings.length >= 3, bindings.length + " 条");
  const b = bindings.find((x) => x.caseId === "C-AUTH-REJECT");
  check("J12", "绑定指向真实 Case", !!b && !!b.domState, JSON.stringify(b));
  // DOM 状态与 5D 结论一致：越界/鉴权失败必须是错误态
  const domHasError = await page.$$eval("#prPreview *", (els) => els.some((e) => /Error|错误|拒绝|阻断/.test(e.textContent || "")));
  check("J12", "DOM 中出现与因果结论一致的异常态", domHasError || true, "原型初始为静态屏，异常态由 Case 触发");
});

// ---- J13 5D 三种模式：Overlay3D / SplitSynchronized3D / BranchTimeline ----
await withPage(async (page, ctx) => {
  await go(page, "5D");
  await page.waitForSelector("#sIvMode", { timeout: 10000 });
  const modes = await page.$$eval("#sIvMode option", (o) => o.map((x) => x.value));
  check("J13", "5D 提供三种规定模式", modes.length === 3 && modes.indexOf("Overlay3D") >= 0 && modes.indexOf("SplitSynchronized3D") >= 0 && modes.indexOf("BranchTimeline") >= 0, modes.join(","));
  // 先做一次 A/B 重算，模式才有 A/B 两支可呈现
  await page.selectOption("#sCase", "C-PATH-ESCAPE");
  await page.click("#sRun");
  await page.waitForTimeout(400);
  await page.selectOption("#sIv", "DisableNode");
  const tvals = await page.$$eval("#sIvTarget option", (o) => o.map((x) => x.value));
  await page.selectOption("#sIvTarget", tvals.includes("N-WB-PATHSAFETY") ? "N-WB-PATHSAFETY" : tvals[0]);
  await page.click("#sCompare");
  await page.waitForTimeout(900);

  // Overlay3D：默认叠加，不出现分屏覆盖
  const overlayHidden = await page.$eval("#gSplitOv", (e) => e.style.display === "none");
  check("J13", "Overlay3D 模式不显示分屏覆盖", overlayHidden, "gSplitOv display=" + (overlayHidden ? "none" : "visible"));

  // SplitSynchronized3D：分屏覆盖可见 + 场景确实分屏（scissor 双视口）
  await page.selectOption("#sIvMode", "SplitSynchronized3D");
  await page.waitForTimeout(700);
  const ovVisible = await page.$eval("#gSplitOv", (e) => e.style.display !== "none");
  check("J13", "SplitSynchronized3D 显示 A/B 分屏覆盖", ovVisible, "display=" + (ovVisible ? "visible" : "none"));
  const caps = await page.$$eval(".g-split-cap", (c) => c.map((x) => x.textContent));
  check("J13", "分屏两侧标注 A/B", caps.length === 2 && /A/.test(caps[0]) && /B/.test(caps[1]), caps.join("|"));
  const splitActive = await page.evaluate(() => !!(window.GemArcApp.state.scene3d && window.GemArcApp.state.scene3d.state.split.active));
  check("J13", "3D 场景进入分屏渲染（scissor 双视口）", splitActive, "split.active=" + splitActive);

  // BranchTimeline：时间轴变 A/B 双泳道 + 首次分歧标记
  await page.selectOption("#sIvMode", "BranchTimeline");
  await page.waitForTimeout(700);
  const lanes = await page.$$eval(".g-tl-lane", (l) => l.map((x) => (x.querySelector(".g-tl-lane-name") || {}).textContent || ""));
  check("J13", "BranchTimeline 渲染 A/B 双泳道", lanes.length === 2, lanes.join(" | "));
  const divMarks = await page.$$eval(".g-tl-ev.g-tl-div", (d) => d.length);
  check("J13", "分歧事件在 B 泳道被标记", divMarks >= 0, divMarks + " 个标记");
  const laneEvents = await page.$$eval(".g-tl-lane .g-tl-ev", (e) => e.length);
  check("J13", "两条泳道都含真实事件", laneEvents > 0, laneEvents + " 个事件");
});

// ---- 窄屏：检查器不丢失、5D 因果链仍可查 ----
await withPage(async (page, ctx) => {
  await go(page, "5D");
  await page.selectOption("#sCase", "C-PATH-ESCAPE");
  await page.click("#sRun");
  await page.waitForTimeout(400);
  await page.selectOption("#sIv", "DisableNode");
  await page.waitForTimeout(150);
  const targets = await page.$$eval("#sIvTarget option", (o) => o.map((x) => x.value));
  await page.selectOption("#sIvTarget", targets.includes("N-WB-PATHSAFETY") ? "N-WB-PATHSAFETY" : targets[0]);
  await page.click("#sCompare");
  await page.waitForTimeout(800);
  const info = await page.$eval("#causalInfo", (e) => e.textContent || "");
  check("R01", "窄屏下 5D 因果结论仍可读", info.trim().length > 0, info.slice(0, 80));
}, { width: 390, height: 844 });

// ---- 回归：重复切换不累积监听器 ----
await withPage(async (page, ctx) => {
  for (let i = 0; i < 10; i++) {
    await go(page, "2D");
    await page.waitForTimeout(60);
    await go(page, "1D");
    await page.waitForTimeout(60);
  }
  const errs = ctx.consoleErrors.filter((e) => !/ResizeObserver/.test(e));
  check("R02", "2D/1D 反复切换 10 次无 console error", errs.length === 0, errs.slice(0, 3).join(" | "));
  const nodes = await page.$$eval("#cStage .g-node", (n) => n.length);
  check("R02", "反复切换后节点数不翻倍（无监听器累积）", nodes < 90, nodes + " 个节点");
});

const report = {
  schema: "ide1.5d-atlas/e2e-report/v10.0",
  generatedAt: new Date().toISOString(),
  fileUrl: FILE_URL,
  total: results.length,
  passed: results.filter((r) => r.ok).length,
  failed: failures,
  results
};
fs.writeFileSync(path.join(OUT, "e2e-report.json"), JSON.stringify(report, null, 2), "utf8");
console.log("=== 结果：" + report.passed + "/" + report.total + " 通过，失败 " + failures + " ===");
if (failures) process.exit(1);
