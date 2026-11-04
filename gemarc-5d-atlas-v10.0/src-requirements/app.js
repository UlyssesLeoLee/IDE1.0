/**
 * src-requirements/app.js — 需求调整工作台（离线单文件）
 *
 * 目标：把"调整真实代码"的高消耗前移成"调整需求/设计模型"——
 *   改需求 → 自动算出真实代码的触点（文件:行、依赖方、被依赖方）
 *   → 自动算出要重跑的 Case、要更新的测试、要同步的图与 Prompt
 *   → 产出可直接交给实现者（人或 AI）的变更工单。
 *
 * 事实来源：5D 图集的正本模型（models/*.json，构建期内联）。本工作台**不改事实**，
 * 只产出「变更提案」，由人工/脚本合回 models/。
 */
(function (root) {
  "use strict";

  var M = root.Ide1Models || {};
  var REQS = (M.requirements && M.requirements.requirements) || [];
  var ARCH = M.architecture || { nodes: [], edges: [], ports: [], containers: [] };
  var TRACE = M.traceability || { matrix: [], tests: [] };
  var CASES = (M.cases && M.cases.cases) || [];
  var EVID = (M.evidence && M.evidence.evidence) || [];
  var GAPS = (M.evidence && M.evidence.gaps) || [];
  var RULES = (M["causal-rules"] || {});
  var PROMPTS = M.prompts || {};
  var DIAGRAMS = (M["diagram-definitions"] && M["diagram-definitions"].diagrams) || [];

  var nodeById = {}, edgeById = {}, evById = {}, reqById = {}, caseById = {};
  ARCH.nodes.forEach(function (n) { nodeById[n.id] = n; });
  ARCH.edges.forEach(function (e) { edgeById[e.id] = e; });
  EVID.forEach(function (e) { evById[e.id] = e; });
  REQS.forEach(function (r) { reqById[r.id] = r; });
  CASES.forEach(function (c) { caseById[c.id] = c; });

  // 依赖方向：edge.source --e--> edge.target 表示 source 依赖 target。
  // 因此「改了 X 会波及谁」= 沿**入边**向上（dependents）；
  // 「X 的契约变了要跟谁确认」= 沿**出边**向下（dependencies）。
  var dependents = {}, dependencies = {};
  ARCH.edges.forEach(function (e) {
    (dependents[e.targetId] = dependents[e.targetId] || []).push(e.sourceId);
    (dependencies[e.sourceId] = dependencies[e.sourceId] || []).push(e.targetId);
  });

  var matrixByReq = {};
  (TRACE.matrix || []).forEach(function (m) { matrixByReq[m.requirementId] = m; });

  // ---------------------------------------------------------------- 状态
  var S = {
    selected: null,
    tab: "detail",
    filterText: "",
    filterCat: "all",
    filterDirty: false,
    edits: {},          // reqId -> {intent, acceptance[], nonGoals[], status, changeType, reason, owner}
    impactCache: {}
  };

  function esc(s) {
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  }
  function el(tag, attrs, children) {
    var a = attrs || {}, parts = [];
    Object.keys(a).forEach(function (k) {
      if (k === "class") parts.push(' class="' + a[k] + '"');
      else if (k === "html") parts.push('>' + a[k]);
      else if (a[k] !== null && a[k] !== undefined) parts.push(' ' + k + '="' + esc(a[k]) + '"');
    });
    return "<" + tag + parts.join("") + ">" + (children || "") + "</" + tag + ">";
  }
  function $(sel) { return document.querySelector(sel); }
  function $$(sel) { return Array.prototype.slice.call(document.querySelectorAll(sel)); }

  // ---------------------------------------------------------------- 影响面
  // 依赖深度上限：改一个组件，真正会被"打破"的是**直接调用方**与它们的调用方。
  // 不设上限时，闭包会顺着外壳链条一路吃到 25/46 个节点，数字失去区分度。
  var MAX_DEPTH = 2;

  function closure(startNodes, startEdges, depthLimit) {
    // 入边向上的依赖方闭包 = 需要跟着改的部分
    var nodes = new Set(), edges = new Set(), queue = [];
    var depth = new Map();
    startNodes.forEach(function (n) { if (nodeById[n]) { nodes.add(n); depth.set(n, 0); queue.push(n); } });
    startEdges.forEach(function (e) {
      if (!edgeById[e]) return;
      edges.add(e);
      var ed = edgeById[e];
      nodes.add(ed.sourceId); nodes.add(ed.targetId);
      depth.set(ed.sourceId, 0); depth.set(ed.targetId, 0);
      queue.push(ed.sourceId);
    });
    var limit = depthLimit === undefined ? MAX_DEPTH : depthLimit;
    while (queue.length) {
      var cur = queue.shift();
      var d = depth.get(cur) || 0;
      if (d >= limit) continue;
      (dependents[cur] || []).forEach(function (dep) {
        ARCH.edges.forEach(function (e) {
          if (e.targetId === cur && e.sourceId === dep) edges.add(e.id);
        });
        if (!nodes.has(dep)) { nodes.add(dep); depth.set(dep, d + 1); queue.push(dep); }
      });
    }
    return { nodes: nodes, edges: edges };
  }

  function impactOf(reqId) {
    if (S.impactCache[reqId]) return S.impactCache[reqId];
    var req = reqById[reqId] || {};
    var row = matrixByReq[reqId] || {};
    var seeds = (row.ddNodeIds || []).slice();
    var seedEdges = (row.ddEdgeIds || []).slice();
    var origin = "matrix";
    // 追踪矩阵没有登记时：需求自身的**证据路径**就是它的真实落点（证据是事实，不是猜测），
    // 用证据反查节点只用于算依赖波及面。
    if (!seeds.length) {
      origin = "evidence";
      (req.evidenceIds || []).forEach(function (eid) {
        ARCH.nodes.forEach(function (n) {
          if ((n.evidenceIds || []).indexOf(eid) !== -1 && seeds.indexOf(n.id) === -1) seeds.push(n.id);
        });
      });
    }
    // 「新增」类变更：落点由使用者在页面上指定（没有落点就没有影响面可言）
    var e0 = currentEdit(reqId);
    if (e0.landingNodeIds && e0.landingNodeIds.length) {
      origin = "landing";
      seeds = e0.landingNodeIds.filter(function (n) { return !!nodeById[n]; });
      seedEdges = [];
    }
    var cl = closure(seeds, seedEdges);
    var seedSet = new Set(seeds);
    (seedEdges || []).forEach(function (eid) {
      var ed = edgeById[eid];
      if (ed) { seedSet.add(ed.sourceId); seedSet.add(ed.targetId); }
    });

    // 被依赖方（需确认契约）：种子节点的出边一跳
    var callees = new Set();
    seeds.forEach(function (n) { (dependencies[n] || []).forEach(function (c) { callees.add(c); }); });

    // 受影响 Case：**直接**（需求落点自身）与**连带**（下游）分开计，
    // 否则所有经过外壳的需求都会得到"几乎全部 Case 要重跑"的无效信号。
    var seedNodesSet = seedSet;
    var hit = function (c, set) {
      return (c.nodeIds || []).some(function (n) { return set.has(n); })
        || (c.edgeIds || []).some(function (e) { return set.has(e); });
    };
    var directCases = CASES.filter(function (c) { return hit(c, seedNodesSet); });
    var downCases = CASES.filter(function (c) { return !hit(c, seedNodesSet) && hit(c, cl.nodes); });
    var cases = directCases.concat(downCases);

    // 受影响测试
    var tests = TRACE.tests.filter(function (t) {
      if ((t.linkedRequirementIds || []).indexOf(reqId) !== -1) return true;
      return (t.id || "").indexOf(reqId) !== -1;
    });

    // 真实代码触点：受影响节点/边的证据 → 文件:行
    // 区分「直接触点」（需求落点自身的文件）与「连带触点」（下游依赖方的文件）：
    // 真正要动手的是直接触点，连带触点是需要复核/回归的范围。
    // 再按 **代码 / 文档** 分开 —— 改 1 个 .rs 与改 1 个 .md 不是一回事。
    var directFiles = {}, downFiles = {};
    function addEvidence(id, isDirect) {
      var e = evById[id];
      if (!e || !e.path) return;
      var bag = isDirect ? directFiles : downFiles;
      if (!bag[e.path]) bag[e.path] = { path: e.path, lines: [], evidence: [], summary: e.summary, status: e.status, kind: e.kind, direct: isDirect, isCode: /^crates\//.test(e.path) };
      if (e.startLine) bag[e.path].lines.push(e.startLine);
      bag[e.path].evidence.push(id);
    }
    cl.nodes.forEach(function (nid) {
      var direct = seedSet.has(nid);
      (nodeById[nid] ? nodeById[nid].evidenceIds || [] : []).forEach(function (eid) { addEvidence(eid, direct); });
    });
    cl.edges.forEach(function (eid) {
      var ed = edgeById[eid];
      var direct = seedEdges.indexOf(eid) !== -1 || (ed && seedSet.has(ed.sourceId) && seedSet.has(ed.targetId));
      (ed ? ed.evidenceIds || [] : []).forEach(function (eid) { addEvidence(eid, direct); });
    });
    var normBag = function (bag) {
      return Object.keys(bag).map(function (k) {
        var f = bag[k];
        f.lines = f.lines.filter(function (v, i, a) { return a.indexOf(v) === i; }).sort(function (a, b) { return a - b; });
        return f;
      }).sort(function (a, b) { return a.path.localeCompare(b.path); });
    };
    var directList = normBag(directFiles);
    var downList = normBag(downFiles).filter(function (f) { return !directFiles[f.path]; });
    // 无追踪行时：需求自身的证据路径就是**直接落点**（证据是事实，不是推断）
    if (origin === "evidence") {
      (req.evidenceIds || []).forEach(function (eid) {
        var ev = evById[eid];
        if (!ev || !ev.path) return;
        if (!directFiles[ev.path]) {
          directFiles[ev.path] = { path: ev.path, lines: ev.startLine ? [ev.startLine] : [], evidence: [eid], summary: ev.summary, status: ev.status, kind: ev.kind, direct: true, isCode: /^crates\//.test(ev.path) };
        }
      });
      directList = normBag(directFiles);
      downList = normBag(downFiles).filter(function (f) { return !directFiles[f.path]; });
    }
    var fileList = directList.concat(downList);

    // 需同步的图与 Prompt（同样分直接/连带）
    var diagrams = DIAGRAMS.filter(function (d) {
      return (d.nodeIds || []).some(function (n) { return cl.nodes.has(n); })
        || (d.edgeIds || []).some(function (e) { return cl.edges.has(e); });
    });
    var directDiagrams = diagrams.filter(function (d) {
      return (d.nodeIds || []).some(function (n) { return seedSet.has(n); })
        || (d.edgeIds || []).some(function (e) { return seedEdges.indexOf(e) !== -1; });
    });
    var promptIds = [];
    ["groupA_targetSystemBuilders", "groupB_atlasImplementationPrompts"].forEach(function (g) {
      var grp = PROMPTS[g];
      if (!grp) return;
      (grp.prompts || []).forEach(function (p) {
        if ((p.requirementIds || []).indexOf(reqId) !== -1) promptIds.push(p.promptId);
      });
    });

    // 相关缺口 / 已知漂移
    var gapHits = GAPS.filter(function (g) {
      return (g.relatedEvidence || []).some(function (eid) {
        return (req.evidenceIds || []).indexOf(eid) !== -1
          || Object.keys(directFiles).some(function (p) { return (evById[eid] || {}).path === p; });
      });
    });

    // 成本估算：改真实代码的"面" —— 直接代码触点权重最高
    var codeOf = function (list) { return list.filter(function (f) { return f.isCode; }); };
    var cost = {
      directFiles: directList.length,
      downstreamFiles: downList.length,
      directCode: codeOf(directList).length,
      downstreamCode: codeOf(downList).length,
      directDocs: directList.length - codeOf(directList).length,
      downstreamDocs: downList.length - codeOf(downList).length,
      files: fileList.length,
      nodes: cl.nodes.size,
      edges: cl.edges.size,
      cases: cases.length,
      directCases: directCases.length,
      downstreamCases: downCases.length,
      tests: tests.length,
      diagrams: diagrams.length,
      prompts: promptIds.length
    };
    cost.score = cost.directCode * 5 + cost.downstreamCode * 2 + cost.directDocs + cost.downstreamDocs
      + cost.directCases * 3 + cost.downstreamCases + cost.tests + cost.diagrams + cost.prompts;
    cost.level = cost.score >= 70 ? "high" : (cost.score >= 30 ? "mid" : "low");   // 阈值按实测 score 分布标定（约 37/37/26 分布）
    cost.directDiagrams = directDiagrams.length;
    cost.downstreamDiagrams = diagrams.length - directDiagrams.length;

    var res = {
      req: req, row: row, seeds: seeds, origin: origin, nodes: Array.from(cl.nodes), edges: Array.from(cl.edges),
      callees: Array.from(callees), cases: cases, directCases: directCases, downCases: downCases,
      tests: tests, files: fileList, directFiles: directList, downstreamFiles: downList,
      diagrams: diagrams, directDiagrams: directDiagrams, promptIds: promptIds, gaps: gapHits, cost: cost
    };
    S.impactCache[reqId] = res;
    return res;
  }

  // ---------------------------------------------------------------- 变更类型
  var CHANGE_TYPES = {
    clarify: {
      label: "澄清（Clarify）",
      desc: "语义澄清，期望不改变外部行为",
      model: ["补文档与模型说明", "确认既有 Case 仍成立（不新增）"],
      code: ["通常不触及真实代码", "若澄清与实现不符，则升级为 tighten"]
    },
    tighten: {
      label: "收紧（Tighten）",
      desc: "加强约束（更多守卫、更严边界）",
      model: ["新增或加强一条 4D 规则（guardAst）", "新增至少 1 个 Case 覆盖被收紧的边界", "更新受影响 Case 的 expected"],
      code: ["在守卫位置加判定（auth / path_safety / 大小 / 编码）", "补一条拒绝路径的测试"]
    },
    relax: {
      label: "放宽（Relax）",
      desc: "放宽约束，允许原先被拒的输入",
      model: ["放宽对应规则守卫", "既有 Case 的预期可能翻转（blocked → completed）", "复核安全类需求的关联缺口"],
      code: ["移除/放宽一处守卫", "确认不会打开越权通道（对照 auth / path_safety）"]
    },
    deprecate: {
      label: "废弃（Deprecate）",
      desc: "不再要求该行为",
      model: ["标记关联节点/边/Case 为 Obsolete 或移除", "更新追踪矩阵行", "记录决策（ADR）"],
      code: ["删除或归档对应实现路径", "同步删除对应路由/命令"]
    },
    add: {
      label: "新增（Add）",
      desc: "新增一条此前不存在的要求",
      model: ["补充 Requirement 记录与证据位（此时为 TBD）", "为目标组件建立追踪行", "预留 Case 与测试位"],
      code: ["需新实现（无既有实现可改）", "需先确定落点模块（可从影响面板指定）"]
    }
  };

  function currentEdit(reqId) {
    if (!S.edits[reqId]) {
      var r = reqById[reqId] || {};
      S.edits[reqId] = {
        intent: r.intent || "",
        acceptance: (r.measurableAcceptance || []).slice(),
        nonGoals: (r.nonGoals || []).slice(),
        status: r.status || "",
        owner: r.owner || "",
        changeType: "",
        reason: "",
        landingNodeIds: []      // 「新增」类变更的落点（由使用者指定）
      };
    }
    return S.edits[reqId];
  }
  function isEdited(reqId) {
    var e = S.edits[reqId], r = reqById[reqId] || {};
    if (!e) return false;
    return !!e.changeType || e.intent !== (r.intent || "")
      || JSON.stringify(e.acceptance) !== JSON.stringify(r.measurableAcceptance || [])
      || e.status !== (r.status || "") || !!e.reason;
  }
  function editedCount() {
    return Object.keys(S.edits).filter(isEdited).length;
  }

  // ---------------------------------------------------------------- 工单生成
  function buildWorkOrder(reqId) {
    var imp = impactOf(reqId);
    var e = currentEdit(reqId);
    var ct = CHANGE_TYPES[e.changeType] || null;
    var base = reqById[reqId] || {};

    var objective = e.changeType
      ? ("调整需求 " + reqId + "（" + ct.label + "）：" + (e.intent || base.intent))
      : ("实现需求 " + reqId + "：" + (base.intent || ""));
    if (e.reason) objective += "｜理由：" + e.reason;

    var sourceEvidence = imp.directFiles.map(function (f) {
      return f.path + (f.lines.length ? ":" + f.lines.join(",") : "") + "（" + f.status + "）";
    });
    if (imp.row.apiRefs && imp.row.apiRefs.length) sourceEvidence.push("API: " + imp.row.apiRefs.join(", "));

    var targetModules = imp.nodes.map(function (n) {
      var nd = nodeById[n];
      return nd ? (nd.qualifiedName + " [" + n + "]") : n;
    });
    if (imp.row.screenIds && imp.row.screenIds.length) targetModules.push("屏幕: " + imp.row.screenIds.join(", "));

    var rules = (ct ? ct.model : []).slice();
    (RULES.rules || []).forEach(function (r) {
      if (imp.edges.indexOf(r.edgeId) !== -1) {
        rules.push("既有规则 " + r.id + "（when=" + r.when + ", then=" + r.then + "）需复核");
      }
    });

    var failureModes = [];
    imp.cases.forEach(function (c) {
      if ((c.expected || []).length) failureModes.push(c.id + " 的预期可能变化：" + c.expected.join(" / "));
    });
    if (imp.gaps.length) failureModes.push("已知缺口关联：" + imp.gaps.map(function (g) { return g.id; }).join(", "));

    var acceptance = (e.acceptance && e.acceptance.length ? e.acceptance : (base.measurableAcceptance || [])).slice();
    if (ct) ct.model.forEach(function (m) { acceptance.push("[变更要求] " + m); });

    var dependency = [];
    if (imp.callees.length) dependency.push("需确认契约的被依赖方: " + imp.callees.join(", "));
    if (imp.downstreamFiles.length) {
      dependency.push("连带影响（需复核/回归）: " + imp.downstreamFiles.map(function (f) {
        return f.path + (f.isCode ? "" : "(文档)");
      }).join(", "));
    }
    if (imp.downCases.length) dependency.push("连带 Case（建议抽样回归）: " + imp.downCases.map(function (c) { return c.id; }).join(", "));
    if (imp.diagrams.length) dependency.push("需同步的工程图: " + imp.diagrams.map(function (d) { return d.id; }).join(", "));
    if (imp.promptIds.length) dependency.push("需同步的 1D Prompt: " + imp.promptIds.join(", "));
    if (imp.row.promptIds && imp.row.promptIds.length) dependency.push("追踪矩阵登记的 Prompt: " + imp.row.promptIds.join(", "));

    var rollback = "本变更只改模型与需求记录；真实代码回滚 = 撤销对应文件的提交（触点清单见 sourceEvidence，共 " + imp.files.length + " 处）。";
    if (imp.cases.length) rollback += " 需回滚并重跑的 Case：" + imp.cases.map(function (c) { return c.id; }).join(", ");

    return {
      taskId: "REQ-CHG-" + reqId.replace(/[^A-Za-z0-9]/g, ""),
      requirementIds: [reqId],
      objective: objective,
      changeType: e.changeType || "(未选择变更类型)",
      changeReason: e.reason || "",
      sourceEvidence: sourceEvidence,
      targetModules: targetModules,
      inputOutput: {
        input: "当前实现行为：" + (imp.row.apiRefs || []).join(", "),
        output: "变更后的可观察行为：" + (e.intent || base.intent || "")
      },
      rules: rules,
      failureModes: failureModes,
      acceptanceTests: acceptance,
      dependency: dependency,
      rollback: rollback,
      cost: imp.cost,
      affectedCases: imp.cases.map(function (c) { return c.id; }),
      affectedTests: imp.tests.map(function (t) { return t.id; }),
      affectedFiles: imp.directFiles.map(function (f) { return f.path; }),
      downstreamFiles: imp.downstreamFiles.map(function (f) { return f.path; })
    };
  }

  function workOrderMarkdown(wo, imp) {
    var L = [];
    L.push("### " + wo.taskId + " — " + wo.objective);
    L.push("");
    L.push("- **objective**: " + wo.objective);
    L.push("- **requirementIds**: " + wo.requirementIds.join(", "));
    L.push("- **changeType**: " + wo.changeType);
    if (wo.changeReason) L.push("- **changeReason**: " + wo.changeReason);
    L.push("- **sourceEvidence**: " + (wo.sourceEvidence.join("; ") || "—"));
    L.push("- **targetModules**: " + (wo.targetModules.join(", ") || "—"));
    L.push("- **inputOutput**: " + wo.inputOutput.input + " → " + wo.inputOutput.output);
    L.push("- **rules**: " + (wo.rules.join("; ") || "—"));
    L.push("- **failureModes**: " + (wo.failureModes.join("; ") || "—"));
    L.push("- **acceptanceTests**: " + (wo.acceptanceTests.join("; ") || "—"));
    L.push("- **dependency**: " + (wo.dependency.join("; ") || "—"));
    L.push("- **rollback**: " + wo.rollback);
    L.push("- **cost**: 直接代码 " + wo.cost.directCode + " 文件 + 连带代码 " + wo.cost.downstreamCode + " / 文档 " + (wo.cost.directDocs + wo.cost.downstreamDocs) + " / 节点 " + wo.cost.nodes +
      " / 直接Case " + wo.cost.directCases + " + 连带Case " + wo.cost.downstreamCases + " / 测试 " + wo.cost.tests +
      " / 图 " + wo.cost.diagrams + " / Prompt " + wo.cost.prompts + "（等级 " + wo.cost.level + "）");
    L.push("");
    return L.join("\n");
  }

  function proposalMarkdown() {
    var ids = Object.keys(S.edits).filter(isEdited);
    var L = [];
    L.push("# 需求变更提案 — IDE1.0");
    L.push("");
    L.push("> 由需求调整工作台生成；事实正本为 5D 图集 models/*.json。本提案**尚未**并回 models/。");
    L.push("> 变更需求数：" + ids.length);
    L.push("");
    var totalFiles = {}, totalCases = {}, totalTests = {};
    ids.forEach(function (id) {
      var imp = impactOf(id);
      var wo = buildWorkOrder(id);
      L.push(workOrderMarkdown(wo, imp));
      imp.files.forEach(function (f) { totalFiles[f.path] = 1; });
      imp.cases.forEach(function (c) { totalCases[c.id] = 1; });
      imp.tests.forEach(function (t) { totalTests[t.id] = 1; });
    });
    L.push("## 汇总：合并变更的总成本面");
    L.push("");
    L.push("- 触及真实代码文件 **" + Object.keys(totalFiles).length + "** 个");
    Object.keys(totalFiles).sort().forEach(function (p) { L.push("  - " + p); });
    L.push("- 需重跑的 Case **" + Object.keys(totalCases).length + "** 个：" + Object.keys(totalCases).join(", "));
    L.push("- 需更新的测试 **" + Object.keys(totalTests).length + "** 个：" + Object.keys(totalTests).join(", "));
    L.push("");
    return L.join("\n");
  }

  // ---------------------------------------------------------------- 渲染
  function catOf(id) {
    if (id.indexOf("FR-") === 0) return "FR";
    if (id.indexOf("NFR-") === 0) return "NFR";
    if (id.indexOf("IR-") === 0) return "IR";
    if (id.indexOf("CR-") === 0) return "CR";
    return "OTHER";
  }
  var CATS = ["all", "FR", "NFR", "IR", "CR"];

  function reqListHtml() {
    var q = S.filterText.toLowerCase();
    var rows = REQS.filter(function (r) {
      if (S.filterCat !== "all" && catOf(r.id) !== S.filterCat) return false;
      if (!q) return true;
      return (r.id + " " + r.intent + " " + (r.status || "")).toLowerCase().indexOf(q) !== -1;
    });
    if (!rows.length) return el("div", { class: "g-empty" }, ["没有匹配的需求"]);
    return rows.map(function (r) {
      var imp = impactOf(r.id);
      var c = imp.cost;
      var edited = isEdited(r.id);
      return el("button", {
        class: "g-req" + (S.selected === r.id ? " active" : "") + (edited ? " edited" : ""),
        "data-req": r.id, type: "button"
      }, [
        el("span", { class: "g-req-id" }, [r.id]),
        el("span", { class: "g-req-intent" }, [r.intent]),
        el("span", { class: "g-req-meta" }, [
          el("span", { class: "g-chip g-chip-" + c.level }, ["成本 " + c.level]),
          el("span", { class: "g-chip" }, [c.directCode + " 代码"]),
          el("span", { class: "g-chip" }, [c.directCases + " Case"]),
          edited ? el("span", { class: "g-chip g-chip-edited" }, ["已改"]) : ""
        ])
      ]);
    }).join("");
  }

  function detailHtml(reqId) {
    var r = reqById[reqId] || {};
    var e = currentEdit(reqId);
    var imp = impactOf(reqId);
    var ctOpts = ["<option value=''>（未选择）</option>"].concat(Object.keys(CHANGE_TYPES).map(function (k) {
      return "<option value='" + k + "'" + (e.changeType === k ? " selected" : "") + ">" + esc(CHANGE_TYPES[k].label) + "</option>";
    })).join("");
    var h = [];
    h.push(el("div", { class: "g-h" }, [r.id + " · " + catOf(r.id) + "　" + esc(r.status || "")]));
    h.push(el("div", { class: "g-grid2" }, [
      el("label", { class: "g-fld" }, [
        el("span", {}, ["意图（可改）"]),
        el("textarea", { id: "fIntent", rows: 3 }, [esc(e.intent)])
      ]),
      el("label", { class: "g-fld" }, [
        el("span", {}, ["变更类型"]),
        el("select", { id: "fType" }, [ctOpts]),
        el("span", { class: "g-hint" }, ["变更类型决定模型侧要同步什么（规则/Case/测试/图）"])
      ])
    ]));
    h.push(el("label", { class: "g-fld" }, [
      el("span", {}, ["变更理由（会写进工单）"]),
      el("textarea", { id: "fReason", rows: 2 }, [esc(e.reason)])
    ]));
    h.push(el("label", { class: "g-fld" }, [
      el("span", {}, ["可验证判据（每行一条，可增删）"]),
      el("textarea", { id: "fAcc", rows: 4 }, [esc(e.acceptance.join("\n"))])
    ]));
    h.push(el("div", { class: "g-grid2" }, [
      el("label", { class: "g-fld" }, [
        el("span", {}, ["状态"]),
        el("input", { id: "fStatus", type: "text", value: esc(e.status) })
      ]),
      el("label", { class: "g-fld" }, [
        el("span", {}, ["负责人"]),
        el("input", { id: "fOwner", type: "text", value: esc(e.owner) })
      ])
    ]));
    h.push(el("div", { class: "g-actions" }, [
      el("button", { class: "g-btn g-btn-primary", id: "bSave", type: "button" }, ["保存到变更集"]),
      el("button", { class: "g-btn", id: "bReset", type: "button" }, ["还原"]),
      el("button", { class: "g-btn", id: "bCopy", type: "button" }, ["复制工单 Markdown"])
    ]));
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["原需求记录（只读正本）"]),
      el("pre", { class: "g-pre" }, [esc(JSON.stringify({
        id: r.id, intent: r.intent, measurableAcceptance: r.measurableAcceptance, nonGoals: r.nonGoals,
        status: r.status, owner: r.owner, provenance: r.provenance, evidenceIds: r.evidenceIds,
        affectedDimensions: r.affectedDimensions, linkedTestIds: r.linkedTestIds
      }, null, 2))])
    ]));
    if (CHANGE_TYPES[e.changeType]) {
      var ct = CHANGE_TYPES[e.changeType];
      h.push(el("div", { class: "g-note" }, [
        el("b", {}, [ct.label + " — " + ct.desc]),
        el("div", {}, ["模型侧：" + ct.model.join("；")]),
        el("div", {}, ["代码侧：" + ct.code.join("；")])
      ]));
    }
    // 「新增」必须能指定落点，否则影响面无从谈起
    if (e.changeType === "add") {
      var allNodeIds = ARCH.nodes.map(function (n) { return n.id; });
      h.push(el("label", { class: "g-fld" }, [
        el("span", {}, ["落点节点（逗号分隔，用于计算影响面）"]),
        el("input", {
          id: "fLanding", type: "text", value: esc((e.landingNodeIds || []).join(", ")),
          placeholder: "如 N-WB-SERVER, N-KN-DOCSTORE", list: "nodeIds", "aria-label": "落点节点"
        }),
        el("datalist", { id: "nodeIds" }, allNodeIds.map(function (id) {
          var nd = nodeById[id];
          return el("option", { value: id }, [nd ? nd.name : id]);
        })),
        el("span", { class: "g-hint" }, ["不填则无影响面（新增需求尚未决定落在哪个模块）"])
      ]));
    }
    return h.join("");
  }

  function impactHtml(reqId) {
    var imp = impactOf(reqId);
    var c = imp.cost;
    var h = [];
    h.push(el("div", { class: "g-h" }, ["影响面分析 — " + reqId]));
    h.push(el("div", { class: "g-stats" }, [
      stat("直接代码文件", c.directCode, c.directCode >= 3 ? "high" : ""),
      stat("连带代码文件", c.downstreamCode, ""),
      stat("直接文档", c.directDocs, ""), stat("连带文档", c.downstreamDocs, ""),
      stat("受影响节点", c.nodes, ""), stat("受影响边", c.edges, ""),
      stat("直接 Case", c.directCases, ""), stat("连带 Case", c.downstreamCases, ""),
      stat("需更新测试", c.tests, ""), stat("直接图", c.directDiagrams, ""), stat("连带图", c.downstreamDiagrams, ""), stat("需同步 Prompt", c.prompts, "")
    ]));
    h.push(el("div", { class: "g-note" }, ["成本口径：直接代码×5 + 连带代码×2 + 文档 + 直接Case×3 + 连带Case + 测试 + 图 + Prompt。" +
      "**「直接代码文件」= 你大概率要动手改的 .rs；「连带」= 需要复核/回归的范围。** 改需求前先看这张表 —— 它就是改真实代码要付出的面。"]));
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["真实代码触点（file:line，来自证据链）"]),
      imp.files.length ? el("div", { class: "g-filelist" }, imp.files.map(function (f) {
        return el("div", { class: "g-file" + (f.direct ? " g-file-direct" : " g-file-down") }, [
          el("code", { class: "g-path" }, [f.path]),
          el("span", { class: "g-lines" }, [f.lines.length ? ":" + f.lines.join(",") : ""]),
          el("span", { class: "g-chip " + (f.direct ? (f.isCode ? "g-chip-high" : "g-chip-mid") : "g-chip-low") },
            [f.direct ? (f.isCode ? "直接·代码" : "直接·文档") : (f.isCode ? "连带·代码" : "连带·文档")]),
          el("span", { class: "g-ev" }, [esc(f.summary || "")])
        ]);
      })) : el("div", { class: "g-empty" }, ["本次改动没有可定位的代码触点。"])
    ]));
    // 落点来源必须说清楚，否则「0 触点」会被误读成「这条需求没代码」
    var originNote = {
      matrix: "落点来自追踪矩阵的 ddNodeIds（已登记的落点）。",
      evidence: "这条需求**没有登记追踪矩阵行**，落点由需求自身的证据路径反推 —— 直接触点可信，依赖波及面属推断，建议补一行追踪。",
      landing: "落点由你在「需求详情」中指定（新增类变更）。"
    }[imp.origin] || "";
    h.push(el("div", { class: "g-note" }, ["**落点来源**：" + originNote]));
    if (!imp.files.length) {
      h.push(el("div", { class: "g-note g-note-warn" }, [
        "没有触点的原因：" + (
          imp.origin === "matrix" ? "追踪矩阵登记的节点没有带实现证据（证据缺失或未关联）"
            : imp.origin === "evidence" ? "该需求的证据里没有带路径的条目，无法定位代码"
              : "尚未指定落点节点"
        ) + "。这本身是一条待补的追踪缺口，不是「无需改代码」。"
      ]));
    }
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["依赖方（改了要跟着改）" ]),
      imp.nodes.length ? el("div", { class: "g-taglist" }, imp.nodes.map(function (n) {
        var nd = nodeById[n];
        return el("span", { class: "g-tag g-tag-dom-" + (nd ? nd.domainId : "Unknown") }, [n + (nd ? " · " + nd.name : "")]);
      })) : el("div", { class: "g-empty" }, ["无"])
    ]));
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["被依赖方（契约变了要确认）"]),
      imp.callees.length ? el("div", { class: "g-taglist" }, imp.callees.map(function (n) {
        var nd = nodeById[n];
        return el("span", { class: "g-tag" }, [n + (nd ? " · " + nd.name : "")]);
      })) : el("div", { class: "g-empty" }, ["无"])
    ]));
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["需重跑的 Case"]),
      imp.cases.length ? el("div", { class: "g-taglist" }, imp.cases.map(function (c2) {
        return el("span", { class: "g-tag g-tag-case" }, [c2.id + " · " + c2.origin]);
      })) : el("div", { class: "g-empty" }, ["无受影响 Case（说明这条需求还没被 4D 覆盖）"])
    ]));
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["需更新的测试"]),
      imp.tests.length ? el("div", { class: "g-taglist" }, imp.tests.map(function (t) {
        return el("span", { class: "g-tag g-tag-" + (t.status === "PASS" ? "ok" : (t.status === "NOT_TESTED" ? "warn" : "fail")) }, [t.id + " · " + t.status]);
      })) : el("div", { class: "g-empty" }, ["无关联测试"])
    ]));
    if (imp.gaps.length) {
      h.push(el("div", { class: "g-sec" }, [
        el("div", { class: "g-sec-t" }, ["已知缺口 / 漂移（改前必读）"]),
        el("div", { class: "g-taglist" }, imp.gaps.map(function (g) {
          return el("span", { class: "g-tag g-tag-fail" }, [g.id + " · " + g.type]);
        })),
        el("div", { class: "g-evlist" }, imp.gaps.map(function (g) { return esc(g.summary); }))
      ]));
    }
    return h.join("");
  }

  function stat(label, v, cls) {
    return el("div", { class: "g-stat " + (cls || "") }, [
      el("div", { class: "g-stat-v" }, [String(v)]),
      el("div", { class: "g-stat-l" }, [label])
    ]);
  }

  function workOrderHtml(reqId) {
    var wo = buildWorkOrder(reqId);
    var h = [];
    h.push(el("div", { class: "g-h" }, ["实现工单 — " + wo.taskId]));
    h.push(el("div", { class: "g-note" }, ["格式与 docs/AI_IMPLEMENTATION_HANDOFF.md 一致，可直接交给实现者（人或 AI）。"]));
    h.push(el("pre", { class: "g-pre g-pre-big", id: "woBox" }, [esc(workOrderMarkdown(wo, impactOf(reqId)))]));
    h.push(el("div", { class: "g-actions" }, [
      el("button", { class: "g-btn", id: "bWoCopy", type: "button" }, ["复制工单"]),
      el("button", { class: "g-btn g-btn-primary", id: "bProposal", type: "button" }, ["生成全部变更提案 Markdown"])
    ]));
    h.push(el("pre", { class: "g-pre g-pre-big", id: "proposalBox" }, ["（点击上方按钮生成）"]));
    return h.join("");
  }

  function changeSetHtml() {
    var ids = Object.keys(S.edits).filter(isEdited);
    var h = [];
    h.push(el("div", { class: "g-h" }, ["变更集（" + ids.length + " 条需求）"]));
    if (!ids.length) {
      h.push(el("div", { class: "g-empty" }, ["还没有改动。选一条需求 → 填变更类型与理由 → 「保存到变更集」。"]));
      return h.join("");
    }
    h.push(el("table", { class: "g-table" }, [el("tbody", {}, ids.map(function (id) {
      var e = S.edits[id];
      var imp = impactOf(id);
      return el("tr", {}, [
        el("td", {}, [id]),
        el("td", {}, [CHANGE_TYPES[e.changeType] ? CHANGE_TYPES[e.changeType].label : "—"]),
        el("td", {}, [String(imp.cost.files)]),
        el("td", {}, [String(imp.cost.cases)]),
        el("td", {}, [esc(e.reason || "")]),
        el("td", {}, [el("button", { class: "g-btn g-btn-xs", "data-unedit": id, type: "button" }, ["撤销"])])
      ]);
    }))]));
    h.push(el("div", { class: "g-actions" }, [
      el("button", { class: "g-btn g-btn-primary", id: "bExportMd", type: "button" }, ["导出 Markdown 提案"]),
      el("button", { class: "g-btn", id: "bExportJson", type: "button" }, ["导出 JSON（可合回 models/）"])
    ]));
    h.push(el("pre", { class: "g-pre g-pre-big", id: "exportBox" }, ["（导出内容会显示在这里）"]));
    return h.join("");
  }

  function sideHtml() {
    var h = [];
    h.push(el("div", { class: "g-h" }, ["变更摘要"]));
    h.push(el("div", { class: "g-stats" }, [
      stat("已改需求", editedCount(), ""),
      stat("总需求", REQS.length, "")
    ]));
    if (S.selected) {
      var imp = impactOf(S.selected);
      h.push(el("div", { class: "g-sec" }, [
        el("div", { class: "g-sec-t" }, ["当前需求的成本面"]),
        el("div", { class: "g-mini" }, [
          el("div", {}, ["直接代码 " + imp.cost.directCode + " 文件 · 连带代码 " + imp.cost.downstreamCode]),
          el("div", {}, ["直接 Case " + imp.cost.directCases + " · 连带 " + imp.cost.downstreamCases + " · 测试 " + imp.cost.tests]),
          el("div", {}, ["图 " + imp.cost.diagrams + " · Prompt " + imp.cost.prompts])
        ])
      ]));
    }
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["怎么用"]),
      el("ol", { class: "g-ol" }, [
        el("li", {}, ["左侧选需求 → 右侧「影响面」先看成本（改真实代码要动几个文件、几个 Case）"]),
        el("li", {}, ["在「需求详情」改意图/判据，选变更类型并写理由"]),
        el("li", {}, ["「实现工单」自动生成交接契约，复制即用"]),
        el("li", {}, ["「变更集」累积多条 → 导出 Markdown 提案 / JSON，合回 models/ 后再走 ADR 流程"]),
        el("li", {}, ["改真实代码时按「真实代码触点」的 file:line 逐个落地，不要凭记忆找"])
      ])
    ]));
    h.push(el("div", { class: "g-sec" }, [
      el("div", { class: "g-sec-t" }, ["事实来源"]),
      el("div", { class: "g-mini" }, [
        el("div", {}, ["模型正本：models/*.json（" + (M.architecture ? M.architecture.version : "?") + "）"]),
        el("div", {}, ["配套图集：index.html（1D–5D + Prototype）"]),
        el("div", {}, ["本工作台只产出提案，不改事实"])
      ])
    ]));
    return h.join("");
  }

  // ---------------------------------------------------------------- 交互
  function render() {
    $("#reqList").innerHTML = reqListHtml();
    $$("#reqList .g-req").forEach(function (b) {
      b.addEventListener("click", function () { S.selected = b.getAttribute("data-req"); render(); });
    });
    var center = $("#centerBody");
    if (!S.selected) {
      center.innerHTML = el("div", { class: "g-empty g-empty-lg" }, [
        "从左侧选一条需求开始。先看影响面，再改内容 —— 这样改真实代码时不用凭记忆找。"
      ]);
    } else {
      var html = S.tab === "impact" ? impactHtml(S.selected)
        : S.tab === "work" ? workOrderHtml(S.selected)
          : S.tab === "set" ? changeSetHtml()
            : detailHtml(S.selected);
      center.innerHTML = html;
      bindCenter();
    }
    $("#sideBody").innerHTML = sideHtml();
    $$("#tabs .g-tab").forEach(function (b) {
      b.classList.toggle("active", b.getAttribute("data-tab") === S.tab);
      b.setAttribute("aria-selected", b.getAttribute("data-tab") === S.tab ? "true" : "false");
    });
    $("#dirty").textContent = editedCount() ? ("变更集 " + editedCount() + " 条") : "";
  }

  function bindCenter() {
    var reqId = S.selected;
    var e = currentEdit(reqId);
    var fi = $("#fIntent"), fr = $("#fReason"), fa = $("#fAcc"), ft = $("#fType"), fs = $("#fStatus"), fo = $("#fOwner");
    if (fi) fi.addEventListener("input", function () { e.intent = this.value; });
    if (fr) fr.addEventListener("input", function () { e.reason = this.value; });
    if (fa) fa.addEventListener("input", function () { e.acceptance = this.value.split("\n").filter(function (x) { return x.trim(); }); });
    if (ft) ft.addEventListener("change", function () { e.changeType = this.value; render(); });
    var fl = $("#fLanding");
    if (fl) fl.addEventListener("input", function () {
      e.landingNodeIds = this.value.split(",").map(function (x) { return x.trim(); }).filter(Boolean);
      S.impactCache = {};
    });
    if (fs) fs.addEventListener("input", function () { e.status = this.value; });
    if (fo) fo.addEventListener("input", function () { e.owner = this.value; });
    var bSave = $("#bSave");
    if (bSave) bSave.addEventListener("click", function () {
      e.changeType = ft ? ft.value : e.changeType;
      S.impactCache = {};   // 变更类型会影响建议，重新算
      render();
    });
    var bReset = $("#bReset");
    if (bReset) bReset.addEventListener("click", function () { delete S.edits[reqId]; render(); });
    var bCopy = $("#bCopy");
    if (bCopy) bCopy.addEventListener("click", function () {
      copy(workOrderMarkdown(buildWorkOrder(reqId), impactOf(reqId)));
    });
    var bWo = $("#bWoCopy");
    if (bWo) bWo.addEventListener("click", function () { copy($("#woBox").textContent); });
    var bProp = $("#bProposal");
    if (bProp) bProp.addEventListener("click", function () {
      var md = proposalMarkdown();
      $("#proposalBox").textContent = md;
      copy(md);
    });
    var bMd = $("#bExportMd");
    if (bMd) bMd.addEventListener("click", function () { var md = proposalMarkdown(); $("#exportBox").textContent = md; copy(md); });
    var bJson = $("#bExportJson");
    if (bJson) bJson.addEventListener("click", function () {
      var ids = Object.keys(S.edits).filter(isEdited);
      var payload = {
        schema: "ide1-requirement-change/v1.0",
        generatedAt: new Date().toISOString(),
        note: "需求变更提案（未并回 models/）",
        changes: ids.map(function (id) {
          var e2 = S.edits[id], imp = impactOf(id);
          return {
            requirementId: id, changeType: e2.changeType, reason: e2.reason,
            intent: e2.intent, measurableAcceptance: e2.acceptance, status: e2.status, owner: e2.owner,
            workOrder: buildWorkOrder(id),
            affected: { files: imp.files.map(function (f) { return f.path; }), cases: imp.cases.map(function (c) { return c.id; }), tests: imp.tests.map(function (t) { return t.id; }) }
          };
        })
      };
      var js = JSON.stringify(payload, null, 2);
      $("#exportBox").textContent = js;
      copy(js);
    });
    $$("#centerBody [data-unedit]").forEach(function (b) {
      b.addEventListener("click", function () { delete S.edits[b.getAttribute("data-unedit")]; render(); });
    });
  }

  function copy(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(function () { flash("已复制到剪贴板"); });
    } else {
      flash("剪贴板不可用，请手动选择复制");
    }
  }
  function flash(msg) {
    var t = $("#toast");
    t.textContent = msg;
    t.style.display = "";
    setTimeout(function () { t.style.display = "none"; }, 1800);
  }

  function init() {
    $("#reqList").addEventListener("click", function () { });
    var s = $("#fSearch");
    s.addEventListener("input", function () { S.filterText = this.value; render(); });
    $$("#cats .g-chipbtn").forEach(function (b) {
      b.addEventListener("click", function () { S.filterCat = b.getAttribute("data-cat"); render(); });
    });
    $$("#tabs .g-tab").forEach(function (b) {
      b.addEventListener("click", function () { S.tab = b.getAttribute("data-tab"); render(); });
    });
    render();
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init);
  else init();

  root.Ide1RequirementWorkbench = {
    impactOf: impactOf, buildWorkOrder: buildWorkOrder, proposalMarkdown: proposalMarkdown,
    state: S
  };
})(typeof globalThis !== "undefined" ? globalThis : this);
