/**
 * src/app.js — 图集主应用壳
 *
 * 约束（v10.0）：
 *   - 入口 index.html；被设计业务系统的入口只是原型/设计对象。
 *   - file:// 直接打开可用：无 fetch、无 CDN、无 ES module import、无 eval。
 *   - 模型数据由构建期内联为 window.GemArcModels。
 */
(function (root) {
  "use strict";

  var M = root.GemArcModels || {};
  var Graph = root.GemArcGraph;
  var Engine = root.GemArcEngine;
  var Causal = root.GemArcCausal;
  var PromptLib = root.GemArcPrompt;
  var Diagrams = root.GemArcDiagrams;
  var Spatial = root.GemArcSpatial;
  var Proto = root.GemArcPrototype;
  var Tour = root.GemArcTour;
  var Validate = root.GemArcValidate;

  var $ = function (sel, ctx) { return (ctx || document).querySelector(sel); };
  var $$ = function (sel, ctx) { return Array.prototype.slice.call((ctx || document).querySelectorAll(sel)); };

  /** 事实源 revision 短码（models/evidence.json） */
  function revShort() {
    var revs = (M.evidence && M.evidence.sourceRevisions) || [];
    var sha = revs.length ? (revs[0].sha || '') : '';
    return sha.slice(0, 7);
  }

  function el(tag, attrs, children) {
    var a = "";
    Object.keys(attrs || {}).forEach(function (k) {
      if (attrs[k] === undefined || attrs[k] === null) return;
      a += " " + k + '="' + esc(String(attrs[k])) + '"';
    });
    if (children && children.length) return "<" + tag + a + ">" + children.join("") + "</" + tag + ">";
    return "<" + tag + a + "></" + tag + ">";
  }

  function esc(s) {
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  }

  // ------------------------------------------------------------ 状态

  var S = {
    view: "1D",
    leftTab: "dims",
    rightTab: "inspect",
    search: "",
    graph: null,
    engine: null,
    scene3d: null,
    ivMode: "Overlay3D",
    runs: {},
    currentCase: null,
    currentIntervention: null,
    causal: null,
    baselineRun: null,          // 4D 的基线轨迹 —— 5D 建立在它之上
    prompt: null,
    promptVersion: 1,
    promptHistory: [],
    canvas: { scale: 1, tx: 0, ty: 0, selected: [], collapsed: {}, hiddenLayers: {}, groups: [] },
    focusSet: null,             // 2D 由 1D 驱动的聚焦子图 { nodes:Set, edges:Set }
    diagWindows: [],
    protoTabs: [],
    activeProtoTab: null,
    tour: null,                 // 当前导览步骤的解说内容
    promptIndex: new Map()      // nodeId/edgeId -> 引用它的 Prompt 列表（1D→2D 反向链路）
  };

  var VIEWS = [
    { id: "1D", label: "1D · Prompt", key: "1d" },
    { id: "2D", label: "2D · 画布 / 工程图", key: "2d" },
    { id: "3D", label: "3D · 空间图论", key: "3d" },
    { id: "4D", label: "4D · 动态 Case", key: "4d" },
    { id: "5D", label: "5D · 因果实验", key: "5d" },
    { id: "P", label: "Prototype", key: "prototype" }
  ];

  // ------------------------------------------------------- 初始化

  function init() {
    var arch = M.architecture || { nodes: [], edges: [], containers: [], ports: [] };
    S.graph = Graph.createGraph(arch.nodes, arch.edges);

    // 模型键名是 "causal-rules"（带连字符），不是 causalRules
    var cr = M["causal-rules"] || {};
    var rules = cr.rules || [];
    var cases = (M.cases && M.cases.cases) || [];
    var sms = cr.stateMachines || [];
    S.engine = Engine.createEngine({
      nodes: arch.nodes, edges: arch.edges, rules: rules,
      cases: cases, stateMachines: sms, version: (M.architecture && M.architecture.version) || "unversioned"
    });

    // 初始 Prompt 一律取自模型（models/prompts.json），不在代码里硬编码任何目标项目的事实
    var seedPrompt = null;
    if (M.prompts) {
      var gB = M.prompts.groupB_atlasImplementationPrompts;
      var gA = M.prompts.groupA_targetSystemBuilders;
      if (gB && gB.prompts && gB.prompts.length) seedPrompt = gB.prompts[0];
      else if (gA && gA.prompts && gA.prompts.length) seedPrompt = gA.prompts[0];
    }
    S.prompt = PromptLib.generate(seedPrompt || {});
    S.promptVersion = (seedPrompt && seedPrompt.version) || 1;

    buildPromptIndex();

    // ---- 导览（流程动画）装载与接线 ----
    if (root.GemArcTour && M.tours) {
      Tour.load(M.tours);
      Tour.setHandlers({
        onView: function (view) {
          var v = VIEWS.filter(function (x) { return x.id === view; })[0];
          if (!v) return;
          if (S.view !== view) {
            S.view = view;
            location.hash = "#" + v.key;
          }
        },
        onHighlight: function (focus) { applyTourHighlight(focus); },
        onCamera: function (cam, instant) { applyTourCamera(cam, instant); },
        onState: function (st) { renderTourBar(st); }
      });
    }

    buildShell();
    bindGlobal();
    route();
  }

  /**
   * 建立 nodeId/edgeId → 引用它的 Prompt 的反向索引。
   * 这是 2D 反查 1D 的通道：点节点就能知道"哪些 Prompt 在用它"。
   */
  function buildPromptIndex() {
    S.promptIndex = new Map();
    var all = [];
    ["groupA_targetSystemBuilders", "groupB_atlasImplementationPrompts"].forEach(function (g) {
      var grp = M.prompts && M.prompts[g];
      if (!grp) return;
      all = all.concat(grp.prompts);
    });
    all.forEach(function (p) {
      (p.nodeEdgeIds || []).forEach(function (id) {
        if (!S.promptIndex.has(id)) S.promptIndex.set(id, []);
        S.promptIndex.get(id).push(p.promptId);
      });
    });
  }

  /**
   * 由当前 1D Prompt 计算 2D 的聚焦子图。
   * 这就是「2D 建立在 1D 基础上」的具体机制：Prompt 引用的节点/边决定画布取景。
   */
  function computeFocusSet() {
    var p = S.prompt;
    if (!p || !p.nodeEdgeIds || !p.nodeEdgeIds.length) return null;
    var nodes = new Set();
    var edges = new Set();
    p.nodeEdgeIds.forEach(function (id) {
      if (S.graph.getNode(id)) nodes.add(id);
      else if (S.graph.getEdge(id)) edges.add(id);
    });
    if (!nodes.size && !edges.size) return null;
    // 聚焦节点的直接邻居也纳入（否则图会碎成孤点，认知负荷反而上升）
    nodes.forEach(function (nid) {
      S.graph.outEdges(nid).forEach(function (e) { edges.add(e.id); nodes.add(e.targetId); });
      S.graph.inEdges(nid).forEach(function (e) { edges.add(e.id); nodes.add(e.sourceId); });
    });
    return { nodes: nodes, edges: edges, promptId: p.promptId };
  }

  // ------------------------------------------------------- 外壳

  function buildShell() {
    var app = $("#app");
    var head = el("header", { class: "g-head" }, [
      el("div", { class: "g-head-left" }, [
        el("span", { class: "g-mark" }, ["5D"]),
        el("div", {}, [
          el("div", { class: "g-title" }, ["IDE1.0 · 5D 工程设计图集"]),
          el("div", { class: "g-rev" }, ["IDE1.0 @ " + revShort() + " · dev"])
        ])
      ]),
      el("nav", { class: "g-views", id: "viewNav" }, VIEWS.map(function (v) {
        return el("button", { class: "g-viewbtn", "data-view": v.id, type: "button" }, [v.label]);
      })),
      el("div", { class: "g-head-right" }, [
        el("button", { class: "g-btn g-drawer-btn", id: "drawerLeft", type: "button", "aria-label": "打开/关闭目录抽屉", title: "目录" }, ["目录"]),
        el("button", { class: "g-btn g-drawer-btn", id: "drawerRight", type: "button", "aria-label": "打开/关闭检查器抽屉", title: "检查器" }, ["检查器"]),
        el("input", { class: "g-search", id: "globalSearch", type: "search", placeholder: "搜索 ID / 名称 / 证据…", "aria-label": "全局搜索" }),
        el("span", { class: "g-rev-badge", id: "revBadge" }, ["rev " + revShort()])
      ])
    ]);

    var left = el("aside", { class: "g-left", id: "leftPane" }, [
      el("div", { class: "g-tabs" }, [
        el("button", { class: "g-tab active", "data-left": "dims", type: "button" }, ["维度"]),
        el("button", { class: "g-tab", "data-left": "model", type: "button" }, ["模型"])
      ]),
      el("div", { class: "g-leftbody", id: "leftBody" })
    ]);

    var center = el("main", { class: "g-center", id: "center" }, [el("div", { id: "viewRoot", class: "g-viewroot" })]);

    var right = el("aside", { class: "g-right", id: "rightPane" }, [
      el("div", { class: "g-tabs" }, [
        el("button", { class: "g-tab active", "data-right": "inspect", type: "button" }, ["检查器"]),
        el("button", { class: "g-tab", "data-right": "evidence", type: "button" }, ["证据"]),
        el("button", { class: "g-tab", "data-right": "gaps", type: "button" }, ["缺口"])
      ]),
      el("div", { class: "g-rightbody", id: "rightBody" })
    ]);

    var timeline = el("section", { class: "g-timeline", id: "timeline" }, [
      el("div", { class: "g-tl-head" }, [
        el("span", { class: "g-tl-title" }, ["Event Trace"]),
        el("span", { class: "g-tl-meta", id: "tlMeta" }, ["—"]),
        el("div", { class: "g-tl-controls", id: "tlControls" })
      ]),
      el("div", { class: "g-tl-track", id: "tlTrack" })
    ]);

    app.innerHTML = "";
    app.innerHTML = head + left + center + right + timeline +
      el("div", { class: "g-scrim", id: "gScrim" }) +
      el("div", { class: "g-tourbar", id: "tourBar", style: "display:none" }, []);

    renderLeft();
    renderRight();

    // 窄屏抽屉：侧栏与检查器改为可开合的抽屉，绝不隐藏（可访问性契约）
    var dL = $("#drawerLeft"), dR = $("#drawerRight"), scrim = $("#gScrim");
    if (dL) dL.addEventListener("click", function () {
      document.body.classList.toggle("drawer-left");
      document.body.classList.remove("drawer-right");
    });
    if (dR) dR.addEventListener("click", function () {
      document.body.classList.toggle("drawer-right");
      document.body.classList.remove("drawer-left");
    });
    if (scrim) scrim.addEventListener("click", function () {
      document.body.classList.remove("drawer-left", "drawer-right");
    });
  }

  function bindGlobal() {
    $("#viewNav").addEventListener("click", function (ev) {
      var b = ev.target.closest("[data-view]");
      if (!b) return;
      S.view = b.getAttribute("data-view");
      location.hash = "#" + VIEWS.filter(function (v) { return v.id === S.view; })[0].key;
    });

    $$("[data-left]").forEach(function (b) {
      b.addEventListener("click", function () {
        $$("[data-left]").forEach(function (x) { x.classList.remove("active"); });
        b.classList.add("active");
        S.leftTab = b.getAttribute("data-left");
        renderLeft();
      });
    });
    $$("[data-right]").forEach(function (b) {
      b.addEventListener("click", function () {
        $$("[data-right]").forEach(function (x) { x.classList.remove("active"); });
        b.classList.add("active");
        S.rightTab = b.getAttribute("data-right");
        renderRight();
      });
    });

    var search = $("#globalSearch");
    search.addEventListener("input", function () {
      S.search = search.value.trim();
      renderLeft();
      if (S.view === "2D") renderView();
    });

    root.addEventListener("hashchange", route);
  }

  function route() {
    var h = (location.hash || "#1d").replace("#", "");
    var v = VIEWS.filter(function (x) { return x.key === h; })[0];
    S.view = v ? v.id : "1D";
    $$(".g-viewbtn").forEach(function (b) {
      b.classList.toggle("active", b.getAttribute("data-view") === S.view);
    });
    $("#timeline").style.display = (S.view === "4D" || S.view === "5D") ? "" : "none";
    renderView();
  }

  // ------------------------------------------------------- 左侧

  function renderLeft() {
    var host = $("#leftBody");
    host.innerHTML = "";
    if (S.leftTab === "dims") {
      var items = VIEWS.map(function (v) {
        return el("button", {
          class: "g-leftitem" + (v.id === S.view ? " active" : ""),
          "data-goto": v.id, type: "button"
        }, [el("span", { class: "g-li-key" }, [v.key]), el("span", {}, [v.label.split("· ")[1] || v.label])]);
      });
      host.insertAdjacentHTML("beforeend", el("div", { class: "g-leftgroup" }, [el("div", { class: "g-lg-title" }, ["表达维度"])].concat(items)));
      host.querySelectorAll("[data-goto]").forEach(function (b) {
        b.addEventListener("click", function () {
          S.view = b.getAttribute("data-goto");
          location.hash = "#" + VIEWS.filter(function (v) { return v.id === S.view; })[0].key;
        });
      });

      // ---- 维度依赖链：明确标注谁建立在谁之上 ----
      host.insertAdjacentHTML("beforeend", el("div", { class: "g-leftgroup g-chain" }, [
        el("div", { class: "g-lg-title" }, ["维度依赖链"]),
        el("div", { class: "g-chain-row" }, [
          el("span", { class: "g-chain-k" }, ["1D"]),
          el("span", { class: "g-chain-t" }, ["Prompt 是源头"]),
          el("span", { class: "g-chain-note" }, ["→ 2D 的取景由它的 nodeEdgeIds 决定"])
        ]),
        el("div", { class: "g-chain-row" }, [
          el("span", { class: "g-chain-k" }, ["2D"]),
          el("span", { class: "g-chain-t" }, ["画布 + 14 种工程图"]),
          el("span", { class: "g-chain-note" }, ["与 3D 同源 FactGraph"])
        ]),
        el("div", { class: "g-chain-row" }, [
          el("span", { class: "g-chain-k" }, ["3D"]),
          el("span", { class: "g-chain-t" }, ["空间图论投影"]),
          el("span", { class: "g-chain-note" }, ["→ 4D / 5D 复用它的场景"])
        ]),
        el("div", { class: "g-chain-row" }, [
          el("span", { class: "g-chain-k" }, ["4D"]),
          el("span", { class: "g-chain-t" }, ["事件轨迹"]),
          el("span", { class: "g-chain-note" }, ["→ 5D 的基线来自它"])
        ]),
        el("div", { class: "g-chain-row" }, [
          el("span", { class: "g-chain-k" }, ["5D"]),
          el("span", { class: "g-chain-t" }, ["因果干预"]),
          el("span", { class: "g-chain-note" }, ["真实重算，非预制光轨"])
        ])
      ]));

      // ---- 导览入口（流程动画） ----
      var tours = renderTourEntries();
      if (tours.length) {
        host.insertAdjacentHTML("beforeend", el("div", { class: "g-leftgroup" }, [
          el("div", { class: "g-lg-title" }, ["导览（流程动画）"])
        ].concat(tours)));
        host.querySelectorAll("[data-tour]").forEach(function (b) {
          b.addEventListener("click", function () {
            Tour.start(b.getAttribute("data-tour"));
          });
        });
      }

      if (S.search) {
        var hits = searchAll(S.search);
        host.insertAdjacentHTML("beforeend", el("div", { class: "g-leftgroup" }, [
          el("div", { class: "g-lg-title" }, ["搜索结果 (" + hits.length + ")"])
        ].concat(hits.slice(0, 40).map(function (h) {
          return el("div", { class: "g-hit" }, [
            el("span", { class: "g-hit-kind" }, [h.kind]),
            el("span", { class: "g-hit-id" }, [h.id]),
            // 摘要会被省略号截断 —— 必须保留完整文本的悬停/读屏可读性
            el("span", { class: "g-hit-sum", title: h.summary }, [h.summary])
          ]);
        }))));
      }
    } else {
      var models = ["architecture", "evidence", "spatial", "cases", "causal-rules", "prompts", "prototype", "diagram-definitions", "schema"];
      host.insertAdjacentHTML("beforeend", el("div", { class: "g-leftgroup" }, [
        el("div", { class: "g-lg-title" }, ["模型文件（单一真相源）"])
      ].concat(models.map(function (m) {
        var n = modelCount(m);
        return el("div", { class: "g-modelitem" }, [
          el("code", {}, [m + ".json"]),
          el("span", { class: "g-modelcount" }, [n])
        ]);
      }))));
    }
  }

  function modelCount(m) {
    var d = M[m];
    if (!d) return "—";
    if (Array.isArray(d.evidence)) return d.evidence.length + " 条证据";
    if (Array.isArray(d.nodes)) return d.nodes.length + " 节点 / " + (d.edges || []).length + " 边";
    if (Array.isArray(d.cases)) return d.cases.length + " Case";
    if (Array.isArray(d.rules)) return d.rules.length + " 规则";
    if (Array.isArray(d.diagrams)) return d.diagrams.length + " 图";
    if (d.prompts) return "2 组";
    return "—";
  }

  function searchAll(q) {
    q = q.toLowerCase();
    var out = [];
    (M.architecture && M.architecture.nodes || []).forEach(function (n) {
      if ((n.id + " " + n.name + " " + n.qualifiedName).toLowerCase().indexOf(q) !== -1) {
        out.push({ kind: "Node", id: n.id, summary: n.name + " · " + n.qualifiedName });
      }
    });
    (M.architecture && M.architecture.edges || []).forEach(function (e) {
      if ((e.id + " " + e.type).toLowerCase().indexOf(q) !== -1) out.push({ kind: "Edge", id: e.id, summary: e.type });
    });
    (M.evidence && M.evidence.evidence || []).forEach(function (e) {
      if ((e.id + " " + e.summary).toLowerCase().indexOf(q) !== -1) out.push({ kind: "Evidence", id: e.id, summary: e.summary });
    });
    (M.cases && M.cases.cases || []).forEach(function (c) {
      if ((c.id + " " + c.name).toLowerCase().indexOf(q) !== -1) out.push({ kind: "Case", id: c.id, summary: c.name });
    });
    return out;
  }

  // ------------------------------------------------------- 右侧

function openDiagramWindow(diagId) {
  var host = $("#diagHost");
  if (!host) return;
  var def = ((M["diagram-definitions"] && M["diagram-definitions"].diagrams) || []).filter(function (d) { return d.id === diagId; })[0];
  if (!def) return;
  var win = el("div", { class: "g-diagwin" }, [
    el("div", { class: "g-dw-head" }, [
      el("span", { class: "g-dw-title" }, [def.diagramType + " · " + def.title]),
      el("button", { class: "g-dw-close", type: "button", "aria-label": "关闭" }, ["×"])
    ]),
    el("div", { class: "g-dw-body" })
  ]);
  host.insertAdjacentHTML("beforeend", win);
  var winEl = host.lastElementChild;          // 必须重新查询真实 DOM 元素
  var body = $(".g-dw-body", winEl);
  body.innerHTML = Diagrams.render(def, 720, 460);
  var close = $(".g-dw-close", winEl);
  close.addEventListener("click", function () { winEl.remove(); });
  // 可拖动
  var head = $(".g-dw-head", winEl);
  var drag = null;
  head.addEventListener("pointerdown", function (ev) {
    drag = { x: ev.clientX, y: ev.clientY, l: win.offsetLeft, t: win.offsetTop };
    head.setPointerCapture(ev.pointerId);
  });
  head.addEventListener("pointermove", function (ev) {
    if (!drag) return;
    winEl.style.left = (drag.l + ev.clientX - drag.x) + "px";
    winEl.style.top = (drag.t + ev.clientY - drag.y) + "px";
  });
  head.addEventListener("pointerup", function () { drag = null; });
}

  /** 选中对象 → 关联工程图（按真实 nodeIds/edgeIds 关联，不是按名字猜） */
  function relatedDiagrams(id) {
    var defs = (M["diagram-definitions"] && M["diagram-definitions"].diagrams) || [];
    return defs.filter(function (d) {
      return (d.nodeIds || []).indexOf(id) !== -1 || (d.edgeIds || []).indexOf(id) !== -1;
    });
  }
  /** 选中对象 → 关联 4D Case */
  function relatedCases(id) {
    var cs = (M.cases && M.cases.cases) || [];
    return cs.filter(function (c) {
      return (c.nodeIds || []).indexOf(id) !== -1 || (c.edgeIds || []).indexOf(id) !== -1;
    }).map(function (c) { return c.id; });
  }
  /** 选中对象 → 状态机 */
  function relatedStateMachine(id) {
    var sms = (M["causal-rules"] && M["causal-rules"].stateMachines) || [];
    return sms.filter(function (s) { return s.nodeId === id; })[0] || null;
  }

  S.openDiagramWindow = openDiagramWindow;

  /** 5D 三种模式：Overlay3D / SplitSynchronized3D / BranchTimeline */
  function stateMap(run) {
    var m = {};
    if (!run || !run.finalStates) return m;
    Object.keys(run.finalStates).forEach(function (k) { m[k] = run.finalStates[k].state; });
    return m;
  }
  function applyIvMode() {
    var mode = S.ivMode || "Overlay3D";
    var ov = $("#gSplitOv");
    if (ov) ov.style.display = mode === "SplitSynchronized3D" ? "" : "none";
    if (S.scene3d) {
      if (mode === "SplitSynchronized3D" && S.causal && S.causal.runA && S.causal.runB) {
        S.scene3d.setBranchStates(stateMap(S.causal.runA), stateMap(S.causal.runB));
      } else {
        S.scene3d.clearBranchStates();
      }
    }
    if (mode === "BranchTimeline" && S.causal && S.causal.runB) renderTimeline(S.causal.runB);
    var st = $("#s3dStatus");
    if (st && S.view === "5D") st.textContent = "5D 模式 " + mode + " · " + (S.causal ? (S.causal.outcome || "—") : "尚未重算");
  }

  function renderRight() {
    var host = $("#rightBody");
    host.innerHTML = "";
    if (S.rightTab === "inspect") {
      var sel = S.canvas.selected;
      var blocks = [];
      if (!sel.length) {
        blocks.push(el("div", { class: "g-insp-empty" }, ["未选择对象。在 2D 画布或 3D 场景中点击 Node / Edge / Port。"]));
        blocks.push(el("div", { class: "g-insp-section" }, [
          el("div", { class: "g-insp-title" }, ["图统计"]),
          el("table", { class: "g-table" }, [
            el("tbody", {}, [
              el("tr", {}, [el("td", {}, ["节点"]), el("td", {}, [String(S.graph.nodes().length)])]),
              el("tr", {}, [el("td", {}, ["边"]), el("td", {}, [String(S.graph.edges().length)])]),
              el("tr", {}, [el("td", {}, ["有向环"]), el("td", {}, [String(S.graph.findCycles().length)])])
            ])
          ])
        ]));
      } else {
        sel.forEach(function (id) {
          var n = S.graph.getNode(id);
          var e = S.graph.getEdge(id);
          if (n) {
            var refs = S.promptIndex.get(n.id) || [];
            blocks.push(el("div", { class: "g-insp-section" }, [
              el("div", { class: "g-insp-title" }, [n.id + " · Node"]),
              el("table", { class: "g-table" }, [el("tbody", {}, [
                el("tr", {}, [el("td", {}, ["name"]), el("td", {}, [esc(n.name)])]),
                el("tr", {}, [el("td", {}, ["qualifiedName"]), el("td", {}, [esc(n.qualifiedName)])]),
                el("tr", {}, [el("td", {}, ["role / domain"]), el("td", {}, [n.role + " / " + n.domainId])]),
                el("tr", {}, [el("td", {}, ["verification"]), el("td", {}, [n.verification])]),
                el("tr", {}, [el("td", {}, ["证据"]), el("td", {}, [(n.evidenceIds || []).join(", ")])]),
                el("tr", {}, [
                  el("td", {}, ["被 1D Prompt 引用"]),
                  el("td", {}, [refs.length
                    ? refs.map(function (pid) {
                        return el("button", { class: "g-inline-link", "data-goprompt": pid, type: "button" }, [pid]);
                      })
                    : "—"])
                ]),
                el("tr", {}, [
                  el("td", {}, ["关联工程图"]),
                  el("td", {}, [relatedDiagrams(id).length
                    ? relatedDiagrams(id).map(function (d) {
                        return el("button", { class: "g-inline-link", "data-godiag": d.id, type: "button", title: d.title }, [d.id]);
                      })
                    : "—"])
                ]),
                el("tr", {}, [
                  el("td", {}, ["关联 Case"]),
                  el("td", {}, [relatedCases(id).length
                    ? relatedCases(id).map(function (cid) {
                        return el("button", { class: "g-inline-link", "data-gocase": cid, type: "button" }, [cid]);
                      })
                    : "—"])
                ]),
                el("tr", {}, [
                  el("td", {}, ["状态机"]),
                  el("td", {}, [relatedStateMachine(id)
                    ? relatedStateMachine(id).id + "（初态 " + relatedStateMachine(id).initial + "）"
                    : "—"])
                ]),
                el("tr", {}, [
                  el("td", {}, ["端口"]),
                  el("td", {}, [(n.portIds || []).length ? (n.portIds || []).join(", ") : "—"])
                ])
              ])])
            ]));
          } else if (e) {
            blocks.push(el("div", { class: "g-insp-section" }, [
              el("div", { class: "g-insp-title" }, [e.id + " · Edge"]),
              el("table", { class: "g-table" }, [el("tbody", {}, [
                el("tr", {}, [el("td", {}, ["type"]), el("td", {}, [e.type])]),
                el("tr", {}, [el("td", {}, ["source → target"]), el("td", {}, [e.sourceId + " → " + e.targetId])]),
                el("tr", {}, [el("td", {}, ["directed"]), el("td", {}, [String(e.directed)])]),
                el("tr", {}, [el("td", {}, ["verification"]), el("td", {}, [e.verification])])
              ])])
            ]));
          }
        });
      }
      host.innerHTML = blocks.join("");
      // 2D → 1D 反向通道：点 Prompt ID 直接跳到 1D 并切换到该 Prompt
      host.querySelectorAll("[data-goprompt]").forEach(function (b) {
        b.addEventListener("click", function () {
          var found = findPrompt(b.getAttribute("data-goprompt"));
          if (!found) return;
          S.promptHistory.push(JSON.parse(JSON.stringify(S.prompt)));
          S.prompt = PromptLib.generate(found);
          S.promptVersion = found.version || 1;
          S.view = "1D";
          location.hash = "#1d";
        });
      });
      // 检查器 → 工程图：按真实 ID 打开对应图窗（ER/UML/时序等，不是猜名字）
      host.querySelectorAll("[data-godiag]").forEach(function (b) {
        b.addEventListener("click", function () {
          var did = b.getAttribute("data-godiag");
          if (typeof S.openDiagramWindow === "function") {
            S.view = "2D";
            location.hash = "#2d";
            setTimeout(function () { S.openDiagramWindow(did); }, 60);
          }
        });
      });
      // 检查器 → 4D Case：跳到该 Case 并立即执行
      host.querySelectorAll("[data-gocase]").forEach(function (b) {
        b.addEventListener("click", function () {
          var cid = b.getAttribute("data-gocase");
          S.view = "4D";
          location.hash = "#4d";
          setTimeout(function () {
            var sel = $("#sCase");
            if (sel) {
              sel.value = cid;
              var runBtn = $("#sRun");
              if (runBtn) runBtn.click();
            }
          }, 80);
        });
      });
    } else if (S.rightTab === "evidence") {
      var evs = (M.evidence && M.evidence.evidence) || [];
      var shown = S.search ? evs.filter(function (e) {
        return (e.id + " " + e.summary).toLowerCase().indexOf(S.search.toLowerCase()) !== -1;
      }) : evs;
      host.innerHTML = shown.map(function (e) {
        var loc = e.path ? (e.path + (e.startLine ? ":" + e.startLine + "-" + e.endLine : "")) : "—";
        return el("div", { class: "g-ev" }, [
          el("div", { class: "g-ev-head" }, [
            el("code", {}, [e.id]),
            el("span", { class: "g-badge g-badge-" + (e.status || "").toLowerCase() }, [e.status]),
            el("span", { class: "g-badge g-badge-kind" }, [e.kind])
          ]),
          el("div", { class: "g-ev-sum" }, [esc(e.summary)]),
          el("div", { class: "g-ev-loc" }, [esc(loc) + "  ·  " + e.revision]),
          e.note ? el("div", { class: "g-ev-note" }, [esc(e.note)]) : ""
        ]);
      }).join("");
    } else {
      var gaps = (M.evidence && M.evidence.gaps) || [];
      host.innerHTML = gaps.map(function (g) {
        return el("div", { class: "g-gap" }, [
          el("div", { class: "g-gap-head" }, [el("code", {}, [g.id]), el("span", { class: "g-badge g-badge-gap" }, [g.type])]),
          el("div", {}, [esc(g.summary)]),
          el("div", { class: "g-ev-loc" }, ["相关证据: " + (g.relatedEvidence || []).join(", ")])
        ]);
      }).join("");
    }
  }

  // ------------------------------------------------------- 导览（流程动画）

  /** 应用导览高亮：2D 用节点/边 ID，其余视图用 CSS 选择器 */
  function applyTourHighlight(focus) {
    focus = focus || {};
    // 清掉上一轮
    $$(".g-tour-hit, .g-tour-ring").forEach(function (e2) {
      e2.classList.remove("g-tour-hit");
      e2.classList.remove("g-tour-ring");
    });
    if (focus.nodeIds || focus.edgeIds) {
      (focus.nodeIds || []).forEach(function (id) {
        var e2 = document.querySelector('#cSvg [data-node="' + id + '"]');
        if (e2) e2.classList.add("g-tour-hit");
      });
      (focus.edgeIds || []).forEach(function (id) {
        var e2 = document.querySelector('#cSvg [data-edge="' + id + '"]');
        if (e2) e2.classList.add("g-tour-hit");
      });
    }
    if (focus.selector) {
      var t = document.querySelector(focus.selector);
      if (t) t.classList.add("g-tour-ring");
    }
  }

  /** 应用导览镜头：2D 用 SVG 相机，3D 用场景聚焦 */
  function applyTourCamera(cam, instant) {
    if (!cam) return;
    if (S.view === "2D") {
      var from = { x: S.canvas.tx, y: S.canvas.ty, scale: S.canvas.scale };
      var to = { x: cam.x || 0, y: cam.y || 0, scale: cam.scale || 1 };
      if (instant) {
        S.canvas.tx = to.x; S.canvas.ty = to.y; S.canvas.scale = to.scale;
        drawCanvas();
      } else {
        Tour.animateCamera(from, to, 620, function (v) {
          S.canvas.tx = v.x; S.canvas.ty = v.y; S.canvas.scale = v.scale;
          drawCanvas();
        });
      }
    } else if (S.scene3d && cam.nodeId) {
      S.scene3d.focusOn(cam.nodeId);
    }
  }

  /** 导览条：解说卡 + 播放控制 */
  function renderTourBar(st) {
    var bar = $("#tourBar");
    if (!bar) return;
    if (!st || !st.active) {
      bar.style.display = "none";
      bar.innerHTML = "";
      return;
    }
    bar.style.display = "";
    bar.innerHTML = [
      '<div class="g-tour-card">',
      '<div class="g-tour-head"><span class="g-tour-title"></span>',
      '<span class="g-tour-prog"></span>',
      '<button class="g-tour-x" type="button" aria-label="退出导览">×</button></div>',
      '<div class="g-tour-narr"></div>',
      '<div class="g-tour-hint"></div>',
      '<div class="g-tour-ctl"></div>',
      '</div>'
    ].join("");

    var step = st.step || {};
    $(".g-tour-title", bar).textContent = (st.tourTitle || "") + " · " + (step.title || "");
    $(".g-tour-prog", bar).textContent = (st.index + 1) + " / " + st.total;
    $(".g-tour-narr", bar).textContent = step.narration || "";
    $(".g-tour-hint", bar).textContent = step.hint ? ("提示：" + step.hint) : "";

    var ctl = $(".g-tour-ctl", bar);
    ["prev", "playpause", "next"].forEach(function (k) {
      var b = document.createElement("button");
      b.className = "g-tour-btn";
      b.type = "button";
      b.setAttribute("data-tourctl", k);
      b.textContent = k === "prev" ? "← 上一步" : (k === "next" ? "下一步 →" : (st.playing ? "⏸ 暂停" : "▶ 继续"));
      b.addEventListener("click", function () {
        if (k === "prev") Tour.prev();
        else if (k === "next") Tour.next();
        else Tour.toggle();
      });
      ctl.appendChild(b);
    });
    $(".g-tour-x", bar).addEventListener("click", function () { Tour.stop(); });
  }

  /** 左侧目录里的导览入口 */
  function renderTourEntries() {
    if (!root.GemArcTour) return [];
    return Tour.list().map(function (t) {
      return el("button", { class: "g-leftitem", "data-tour": t.id, type: "button" }, [
        el("span", { class: "g-li-key" }, ["▶"]),
        el("span", {}, [t.title])
      ]);
    });
  }

  // ------------------------------------------------------- 视图分发

  function renderView() {
    var host = $("#viewRoot");
    host.innerHTML = "";
    if (S.scene3d) { S.scene3d.dispose(); S.scene3d = null; }
    if (S.view === "1D") render1D(host);
    else if (S.view === "2D") render2D(host);
    else if (S.view === "3D" || S.view === "4D" || S.view === "5D") render3DLike(host);
    else if (S.view === "P") renderPrototype(host);
    renderLeft();
    renderRight();
  }

  // ------------------------------------------------------- 1D

  function render1D(host) {
    var wrap = el("div", { class: "g-1d" }, [
      el("div", { class: "g-1d-editor" }, [
        el("div", { class: "g-1d-toolbar" }, [
          el("select", { class: "g-select", id: "promptPick", "aria-label": "选择 Prompt" }, promptOptions()),
          el("button", { class: "g-btn", id: "pValidate", type: "button" }, ["校验"]),
          el("button", { class: "g-btn", id: "pCopy", type: "button" }, ["复制"]),
          el("button", { class: "g-btn", id: "pExport", type: "button" }, ["导出"]),
          el("button", { class: "g-btn", id: "pDiff", type: "button" }, ["版本差分"]),
          el("span", { class: "g-1d-status", id: "pStatus" }, ["—"])
        ]),
        el("textarea", { class: "g-1d-text", id: "pObjective", rows: 3, "aria-label": "objective" }),
        el("div", { class: "g-1d-fields", id: "pFields" }),
        el("div", { class: "g-1d-out" }, [
          el("div", { class: "g-lg-title" }, ["渲染结果"]),
          el("pre", { class: "g-pre", id: "pRender" })
        ])
      ]),
      el("div", { class: "g-1d-side" }, [
        el("div", { class: "g-lg-title" }, ["Prompt 库"]),
        el("div", { id: "pLibrary", class: "g-lib" })
      ])
    ]);
    host.insertAdjacentHTML("beforeend", wrap);

    $("#promptPick").addEventListener("change", function () {
      var id = this.value;
      var found = findPrompt(id);
      if (!found) return;
      S.prompt = PromptLib.generate(found);
      S.promptVersion = found.version || 1;
      S.promptHistory.push(JSON.parse(JSON.stringify(S.prompt)));
      fillPromptForm();
    });
    $("#pValidate").addEventListener("click", function () {
      var res = PromptLib.validate(S.prompt, {
        requirements: (M.requirements && M.requirements.requirements) || [],
        nodes: M.architecture.nodes, edges: M.architecture.edges,
        cases: M.cases.cases, evidence: M.evidence.evidence
      });
      var st = $("#pStatus");
      st.textContent = res.ok ? "PASS · 校验通过" : ("FAIL · " + res.errors.length + " 项错误");
      st.className = "g-1d-status " + (res.ok ? "ok" : "bad");
      if (!res.ok) {
        st.textContent += "：" + res.errors.map(function (e) { return e.field + " " + e.message; }).join("；");
      }
    });
    $("#pCopy").addEventListener("click", function () {
      var txt = PromptLib.render(S.prompt);
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(txt).then(function () {
          $("#pStatus").textContent = "PASS · 已复制到剪贴板";
          $("#pStatus").className = "g-1d-status ok";
        }, function () {
          $("#pStatus").textContent = "FAIL · 剪贴板不可用（file:// 下需授权）";
          $("#pStatus").className = "g-1d-status bad";
        });
      } else {
        $("#pStatus").textContent = "FAIL · 无 clipboard API";
        $("#pStatus").className = "g-1d-status bad";
      }
    });
    $("#pExport").addEventListener("click", function () {
      var blob = new Blob([JSON.stringify(S.prompt, null, 2)], { type: "application/json" });
      var a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = S.prompt.promptId + ".v" + S.prompt.version + ".json";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      $("#pStatus").textContent = "PASS · 已导出 " + a.download;
      $("#pStatus").className = "g-1d-status ok";
    });
    $("#pDiff").addEventListener("click", function () {
      var prev = S.promptHistory[S.promptHistory.length - 2];
      if (!prev) {
        $("#pStatus").textContent = "FAIL · 只有 1 个版本，无可差分的历史";
        $("#pStatus").className = "g-1d-status bad";
        return;
      }
      var d = PromptLib.diff(prev, S.prompt);
      $("#pStatus").textContent = (d.changed ? "PASS · " : "PASS · 无变化（") + d.fields.length + " 字段" + (d.changed ? "差异" : ")");
      $("#pStatus").className = "g-1d-status ok";
      $("#pFields").innerHTML = d.fields.map(function (f) {
        return el("div", { class: "g-diffrow" }, [
          el("code", {}, [f.field]),
          el("span", { class: "g-diff-from" }, [esc(JSON.stringify(f.from))]),
          el("span", { class: "g-diff-to" }, [esc(JSON.stringify(f.to))])
        ]);
      }).join("");
    });

    renderLibrary();
    fillPromptForm();
  }

  function promptOptions() {
    var out = [];
    ["groupA_targetSystemBuilders", "groupB_atlasImplementationPrompts"].forEach(function (g) {
      var grp = M.prompts && M.prompts[g];
      if (!grp) return;
      out.push(el("optgroup", { label: g === "groupA_targetSystemBuilders" ? "目标系统自带" : "本图集实现契约" },
        grp.prompts.map(function (p) { return el("option", { value: p.promptId }, [p.promptId + " — " + p.objective.slice(0, 42)]); })));
    });
    return out;
  }

  function findPrompt(id) {
    var found = null;
    ["groupA_targetSystemBuilders", "groupB_atlasImplementationPrompts"].forEach(function (g) {
      var grp = M.prompts && M.prompts[g];
      if (!grp) return;
      grp.prompts.forEach(function (p) { if (p.promptId === id) found = p; });
    });
    return found;
  }

  function renderLibrary() {
    var host = $("#pLibrary");
    if (!host) return;
    host.innerHTML = "";
    ["groupA_targetSystemBuilders", "groupB_atlasImplementationPrompts"].forEach(function (g) {
      var grp = M.prompts && M.prompts[g];
      if (!grp) return;
      host.insertAdjacentHTML("beforeend", el("div", { class: "g-libgroup" }, [el("div", { class: "g-lg-title" }, [
        g === "groupA_targetSystemBuilders" ? "目标系统自带 builder" : "本图集实现契约"
      ])].concat(grp.prompts.map(function (p) {
        return el("button", {
          class: "g-libitem" + (p.promptId === S.prompt.promptId ? " active" : ""),
          "data-pid": p.promptId, type: "button"
        }, [
          el("code", {}, [p.promptId]),
          el("span", { class: "g-libsum" }, [esc(p.objective)])
        ]);
      }))));
    });
    host.querySelectorAll("[data-pid]").forEach(function (b) {
      b.addEventListener("click", function () {
        var id = b.getAttribute("data-pid");
        var found = findPrompt(id);
        if (!found) return;
        S.promptHistory.push(JSON.parse(JSON.stringify(S.prompt)));
        S.prompt = PromptLib.generate(found);
        S.promptVersion = found.version || 1;
        fillPromptForm();
        renderLibrary();
      });
    });
  }

  function fillPromptForm() {
    var p = S.prompt;
    if (!$("#pObjective")) return;
    $("#pObjective").value = p.objective;
    $("#promptPick").value = p.promptId;
    var fields = [
      ["requirementIds", p.requirementIds], ["nodeEdgeIds", p.nodeEdgeIds], ["caseIds", p.caseIds],
      ["inputs", p.inputs], ["constraints", p.constraints], ["outputs", p.outputs],
      ["acceptance", p.acceptance], ["evidenceIds", p.evidenceIds]
    ];
    $("#pFields").innerHTML = fields.map(function (f) {
      return el("div", { class: "g-field" }, [
        el("div", { class: "g-field-label" }, [f[0]]),
        el("div", { class: "g-field-val" }, [f[1].length ? f[1].map(function (x) { return el("code", {}, [x]); }).join(" ") : "—"])
      ]);
    }).join("");
    $("#pRender").textContent = PromptLib.render(p);
    $("#pStatus").textContent = "—";
    $("#pStatus").className = "g-1d-status";
  }

  // ------------------------------------------------------- 2D

  function render2D(host) {
    // 2D 的取景由 1D 的 Prompt 决定 —— 这就是「2D 建立在 1D 基础上」的具体机制
    S.focusSet = computeFocusSet();

    var upstream = S.prompt
      ? el("button", {
          class: "g-upstream", id: "cUpstream", type: "button",
          title: "回到 1D 编辑这段 Prompt"
        }, [
          el("span", { class: "g-upstream-k" }, ["上游 1D"]),
          el("span", { class: "g-upstream-v" }, [S.prompt.promptId]),
          el("span", { class: "g-upstream-n" }, [
            S.focusSet
              ? ("聚焦 " + S.focusSet.nodes.size + " 节点 / " + S.focusSet.edges.size + " 边")
              : "未声明 nodeEdgeIds · 显示全图"
          ])
        ])
      : "";

    var wrap = el("div", { class: "g-2d" }, [
      el("div", { class: "g-2d-toolbar" }, [
        el("input", { class: "g-input", id: "cSearch", placeholder: "筛选节点…", "aria-label": "画布筛选" }),
        el("button", { class: "g-btn", id: "cZoomIn", type: "button" }, ["+"]),
        el("button", { class: "g-btn", id: "cZoomOut", type: "button" }, ["−"]),
        el("button", { class: "g-btn", id: "cFit", type: "button" }, ["Fit"]),
        el("button", { class: "g-btn", id: "cUndo", type: "button" }, ["撤销"]),
        el("button", { class: "g-btn", id: "cRedo", type: "button" }, ["重做"]),
        el("button", { class: "g-btn", id: "cCollapse", type: "button" }, ["折叠/展开"]),
        el("button", { class: "g-btn", id: "cGroup", type: "button" }, ["分组"]),
        el("button", { class: "g-btn", id: "cUngroup", type: "button" }, ["解组"]),
        el("button", { class: "g-btn g-btn-focus" + (S.focusSet ? " on" : ""), id: "cFocus", type: "button" }, [
          S.focusSet ? "聚焦：开" : "聚焦：关"
        ]),
        upstream,
        el("span", { class: "g-2d-meta", id: "cMeta" }, ["—"])
      ]),
      el("div", { class: "g-2d-stage", id: "cStage" }, [
        el("svg", { id: "cSvg", class: "g-canvas", xmlns: "http://www.w3.org/2000/svg" }),
        el("div", { class: "g-minimap", id: "cMinimap" })
      ]),
      el("div", { class: "g-diagwin-host", id: "diagHost" })
    ]);
    host.insertAdjacentHTML("beforeend", wrap);

    var up = $("#cUpstream");
    if (up) {
      up.addEventListener("click", function () {
        S.view = "1D";
        location.hash = "#1d";
      });
    }
    $("#cFocus").addEventListener("click", function () {
      S.focusSet = S.focusSet ? null : computeFocusSet();
      drawCanvas();
      this.textContent = S.focusSet ? "聚焦：开" : "聚焦：关";
      this.className = "g-btn g-btn-focus" + (S.focusSet ? " on" : "");
    });

    var undoStack = [], redoStack = [];
    function snap() { undoStack.push(canvasState()); redoStack.length = 0; }
    function canvasState() { return JSON.stringify({ sel: S.canvas.selected, collapsed: S.canvas.collapsed }); }
    function restore(st) {
      var d = JSON.parse(st);
      S.canvas.selected = d.sel || [];
      S.canvas.collapsed = d.collapsed || {};
      drawCanvas();
    }

    function drawCanvas() {
      var svg = $("#cSvg");
      // 固定逻辑坐标空间：容器的 clientHeight 不可靠（flex 塌陷时只有 150px），
      // 用固定 viewBox + preserveAspectRatio 保证所有节点始终在可视区且可点击
      var W = 1560, H = Math.max(900, 90 + Math.ceil(M.architecture.nodes.length / 6) * 130 + 90);
      svg.setAttribute("viewBox", "0 0 " + W + " " + H);
      svg.setAttribute("preserveAspectRatio", "xMinYMin meet");
      var g = [];
      var tf = "translate(" + S.canvas.tx + "," + S.canvas.ty + ") scale(" + S.canvas.scale + ")";
      g.push(el("g", { transform: tf }, []));

      var q = (S.search || ($("#cSearch") ? $("#cSearch").value : "") || "").toLowerCase();
      var arch = M.architecture;
      var byId = {};
      arch.nodes.forEach(function (n) { byId[n.id] = n; });

      // 分层：按 domainId
      var layer = "Core";
      var layers = {};
      arch.nodes.forEach(function (n) {
        (layers[n.domainId] = layers[n.domainId] || []).push(n);
      });

      Object.keys(layers).forEach(function (dom) {
        if (S.canvas.hiddenLayers[dom]) return;
        g.push(el("g", { class: "g-layer", "data-layer": dom }, [
          el("text", { x: 20, y: 30, class: "g-layer-label" }, [dom + " (" + layers[dom].length + ")"])
        ]));
      });

      // 边
      var edges = arch.edges.map(function (e) {
        var a = byId[e.sourceId], b = byId[e.targetId];
        if (!a || !b) return "";
        var pa = nodePos(a), pb = nodePos(b);
        var x1 = pa.x, y1 = pa.y, x2 = pb.x, y2 = pb.y;
        var mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
        var sel = S.canvas.selected.indexOf(e.id) !== -1;
        var dim = q && !(a.id.toLowerCase().indexOf(q) !== -1 || b.id.toLowerCase().indexOf(q) !== -1);
        // 由 1D 驱动的聚焦：不在子图里的边淡出（降低认知负荷）
        var focusOff = !!(S.focusSet && !S.focusSet.edges.has(e.id) &&
          !S.focusSet.nodes.has(a.id) && !S.focusSet.nodes.has(b.id));
        // 边类型标签：默认不显示（降低认知负荷），仅在放大或选中/相关时出现
        var showLabel = S.canvas.scale >= 0.85 || sel || (S.focusSet && S.focusSet.edges.has(e.id));
        return el("g", { class: "g-edge" + (sel ? " sel" : "") + (dim || focusOff ? " dim" : ""), "data-edge": e.id }, [
          el("line", { x1: x1, y1: y1, x2: x2, y2: y2, class: "g-edge-hit" }),
          el("line", { x1: x1, y1: y1, x2: x2, y2: y2, class: "g-edge-line" }),
          el("circle", { cx: mx, cy: my, r: 3, class: "g-edge-mid" }),
          showLabel ? el("text", { x: mx, y: my - 6, class: "g-edge-label" }, [e.type]) : ""
        ]);
      });

      // 节点
      var nodes = arch.nodes.map(function (n) {
        var p = nodePos(n);
        var sel = S.canvas.selected.indexOf(n.id) !== -1;
        var dim = q && n.id.toLowerCase().indexOf(q) === -1 && n.name.toLowerCase().indexOf(q) === -1;
        // 由 1D 驱动的聚焦：不在子图里的节点淡出
        var focusOff = !!(S.focusSet && !S.focusSet.nodes.has(n.id));
        var role = n.role;
        var w = 168, h = 52;
        // 角色副标题默认隐藏（默认仅显示关键标签），放大或选中时才出现
        var showRole = S.canvas.scale >= 0.85 || sel || (S.focusSet && S.focusSet.nodes.has(n.id));
        var shape = role === "Storage"
          ? el("rect", { x: p.x - w / 2, y: p.y - h / 2, width: w, height: h, rx: 4, class: "g-node-shape g-domain-" + n.domainId })
          : role === "Security"
            ? el("path", { d: "M " + (p.x - w / 2) + " " + (p.y - h / 2) + " h " + w + " v " + (h - 14) + " l -" + (w / 2) + " 14 l -" + (w / 2) + " -14 z", class: "g-node-shape g-domain-" + n.domainId })
            : el("rect", { x: p.x - w / 2, y: p.y - h / 2, width: w, height: h, rx: 10, class: "g-node-shape g-domain-" + n.domainId });
        var body = [
          shape,
          // 标签超宽会让 <g> 的包围盒大于图形本体 → 点击中心时会被邻近节点拦截。
          // 因此按框宽截断显示，并用 <title> 保留完整名（hover/读屏可见）。
          el("text", { x: p.x, y: p.y - 4, class: "g-node-name" }, [fitLabel(n.name, 150)]),
          el("title", {}, [n.name + "（" + n.id + "）"]),
          showRole ? el("text", { x: p.x, y: p.y + 12, class: "g-node-role" }, [n.role + " · " + n.verification]) : ""
        ];
        (n.portIds || []).forEach(function (pid, pi) {
          body.push(el("circle", { cx: p.x - w / 2 + 8 + pi * 14, cy: p.y + h / 2, r: 4, class: "g-port", "data-port": pid }));
        });
        return el("g", { class: "g-node" + (sel ? " sel" : "") + (dim || focusOff ? " dim" : ""), "data-node": n.id, tabindex: "0", role: "button" }, body);
      });

      // 分组框（视图态：只画框，不进入事实图）
      var groups = (S.canvas.groups || []).map(function (gr) {
        return el("g", { class: "g-group", "data-group": gr.id }, [
          el("rect", { x: gr.x, y: gr.y, width: gr.w, height: gr.h, rx: 10, class: "g-group-box" }),
          el("text", { x: gr.x + 10, y: gr.y + 16, class: "g-group-name" }, [gr.name + " · " + gr.nodeIds.length + " 节点"])
        ]);
      });

      svg.innerHTML = g.join("") + groups.join("") + edges.join("") + nodes.join("");

      svg.querySelectorAll("[data-node]").forEach(function (gEl) {
        gEl.addEventListener("click", function (ev) {
          var id = gEl.getAttribute("data-node");
          if (ev.shiftKey) {
            var i = S.canvas.selected.indexOf(id);
            if (i === -1) S.canvas.selected.push(id); else S.canvas.selected.splice(i, 1);
          } else {
            S.canvas.selected = [id];
          }
          snap();
          drawCanvas();
          renderRight();
        });
      });
      svg.querySelectorAll("[data-edge]").forEach(function (gEl) {
        gEl.addEventListener("click", function (ev) {
          ev.stopPropagation();
          S.canvas.selected = [gEl.getAttribute("data-edge")];
          snap();
          drawCanvas();
          renderRight();
        });
      });
      svg.querySelectorAll("[data-port]").forEach(function (pEl) {
        pEl.addEventListener("click", function (ev) {
          ev.stopPropagation();
          $("#cMeta").textContent = "Port " + pEl.getAttribute("data-port") + "（独立可选中）";
        });
      });

      $("#cMeta").textContent = S.graph.nodes().length + " 节点 · " + S.graph.edges().length +
        " 边 · " + S.graph.findCycles().length + " 环 · zoom " + S.canvas.scale.toFixed(2);
      drawMinimap();
    }

    function nodePos(n) {
      var idx = M.architecture.nodes.indexOf(n);
      var col = idx % 6, row = Math.floor(idx / 6);
      return { x: 140 + col * 230, y: 90 + row * 130 };
    }


    function drawMinimap() {
      var mm = $("#cMinimap");
      if (!mm) return;
      mm.innerHTML = "";
      M.architecture.nodes.forEach(function (n) {
        var p = nodePos(n);
        mm.insertAdjacentHTML("beforeend", el("div", {
          class: "g-mm-node" + (S.canvas.selected.indexOf(n.id) !== -1 ? " sel" : ""),
          style: "left:" + (p.x / 9) + "px;top:" + (p.y / 9) + "px",
          "data-mm": n.id
        }, []));
      });
      mm.querySelectorAll("[data-mm]").forEach(function (d) {
        d.addEventListener("click", function () {
          S.canvas.selected = [d.getAttribute("data-mm")];
          drawCanvas();
          renderRight();
        });
      });
    }

    $("#cSearch").addEventListener("input", drawCanvas);
    $("#cZoomIn").addEventListener("click", function () { S.canvas.scale = Math.min(2.4, S.canvas.scale * 1.15); drawCanvas(); });
    $("#cZoomOut").addEventListener("click", function () { S.canvas.scale = Math.max(0.3, S.canvas.scale / 1.15); drawCanvas(); });
    $("#cFit").addEventListener("click", function () { S.canvas.scale = 1; S.canvas.tx = 0; S.canvas.ty = 0; drawCanvas(); });
    $("#cUndo").addEventListener("click", function () { if (undoStack.length) { redoStack.push(canvasState()); restore(undoStack.pop()); } });
    $("#cRedo").addEventListener("click", function () { if (redoStack.length) { undoStack.push(canvasState()); restore(redoStack.pop()); } });
    // 分组：把当前多选节点框成一个命名组（**视图态**，不改事实图）
    $("#cGroup").addEventListener("click", function () {
      var all = M.architecture.nodes || [];
      var picked = S.canvas.selected.filter(function (id) {
        return all.some(function (n) { return n.id === id; });
      });
      if (picked.length < 2) { $("#cMeta").textContent = "分组需要至少选中 2 个节点（Shift 多选）"; return; }
      var pts = picked.map(function (id) { return nodePos(all.filter(function (n) { return n.id === id; })[0]); });
      var xs = pts.map(function (p) { return p.x; });
      var ys = pts.map(function (p) { return p.y; });
      var x0 = Math.min.apply(null, xs), x1 = Math.max.apply(null, xs);
      var y0 = Math.min.apply(null, ys), y1 = Math.max.apply(null, ys);
      var g = {
        id: "G-" + (S.canvas.groups.length + 1),
        name: "分组 " + (S.canvas.groups.length + 1),
        nodeIds: picked.slice(),
        x: x0 - 90, y: y0 - 46,
        w: (x1 - x0) + 180,
        h: (y1 - y0) + 92
      };
      S.canvas.groups.push(g);
      $("#cMeta").textContent = "已分组 " + g.id + "（" + picked.length + " 节点 · 视图态，不改事实图）";
      drawCanvas();
    });
    $("#cUngroup").addEventListener("click", function () {
      if (!S.canvas.groups.length) { $("#cMeta").textContent = "没有可解的分组"; return; }
      var g = S.canvas.groups.pop();
      $("#cMeta").textContent = "已解组 " + g.id;
      drawCanvas();
    });    $("#cCollapse").addEventListener("click", function () {
      var sel = S.canvas.selected[0];
      if (!sel) { $("#cMeta").textContent = "先选中一个节点再折叠"; return; }
      S.canvas.collapsed[sel] = !S.canvas.collapsed[sel];
      snap();
      drawCanvas();
      $("#cMeta").textContent = sel + " 折叠 = " + S.canvas.collapsed[sel];
    });

    // Pan
    var svg = $("#cSvg"), panning = null;
    svg.addEventListener("pointerdown", function (ev) {
      if (ev.target.closest("[data-node]") || ev.target.closest("[data-edge]")) return;
      panning = { x: ev.clientX, y: ev.clientY, tx: S.canvas.tx, ty: S.canvas.ty };
      svg.setPointerCapture(ev.pointerId);
    });
    svg.addEventListener("pointermove", function (ev) {
      if (!panning) return;
      S.canvas.tx = panning.tx + (ev.clientX - panning.x);
      S.canvas.ty = panning.ty + (ev.clientY - panning.y);
      drawCanvas();
    });
    svg.addEventListener("pointerup", function () { panning = null; });

    // 独立工程图窗
    // 模型键名是 "diagram-definitions"（带连字符）
    var diagDefs = (M["diagram-definitions"] && M["diagram-definitions"].diagrams) || [];
    var diagBar = el("div", { class: "g-diagbar" }, diagDefs.map(function (d) {
      return el("button", { class: "g-diagbtn", "data-diag": d.id, type: "button" }, [d.diagramType]);
    }));
    $("#diagHost").insertAdjacentHTML("beforeend", diagBar);
    // diagBar 是 HTML 字符串 —— 必须重新查询真实 DOM 元素再绑定事件
    var diagBarEl = $("#diagHost .g-diagbar");
    diagBarEl.querySelectorAll("[data-diag]").forEach(function (b) {
      b.addEventListener("click", function () { openDiagramWindow(b.getAttribute("data-diag")); });
    });

    drawCanvas();
  }


  // ------------------------------------------------------- 3D / 4D / 5D

  function render3DLike(host) {
    var wrap = el("div", { class: "g-3d" }, [
      el("div", { class: "g-3d-side" }, [
        el("div", { class: "g-3d-tools" }, [
          el("select", { class: "g-select", id: "s3dView", "aria-label": "视图" },
            (M.spatial.views || []).map(function (v) { return el("option", { value: v.id }, [v.id]); })),
          el("button", { class: "g-btn", id: "s3dReset", type: "button" }, ["重置相机"]),
          el("select", { class: "g-select", id: "s3dLabel", "aria-label": "标签模式" },
            ["Smart", "All", "RelatedOnly"].map(function (v) { return el("option", { value: v }, [v]); }))
        ]),
        S.view === "4D" || S.view === "5D" ? el("div", { class: "g-3d-cases" }, [
          el("div", { class: "g-lg-title" }, ["Case"]),
          el("select", { class: "g-select", id: "sCase", "aria-label": "选择 Case" },
            (M.cases.cases || []).map(function (c) { return el("option", { value: c.id }, [c.id + " — " + c.name]); })),
          el("button", { class: "g-btn", id: "sRun", type: "button" }, ["运行"]),
          el("div", { class: "g-3d-runinfo", id: "runInfo" }, ["—"])
        ]) : "",
        S.view === "5D" ? el("div", { class: "g-3d-cases" }, [
          el("div", { class: "g-lg-title" }, ["5D 模式"]),
          el("select", { class: "g-select", id: "sIvMode", "aria-label": "5D 模式" },
            ["Overlay3D", "SplitSynchronized3D", "BranchTimeline"].map(function (m) {
              return el("option", { value: m }, [m]);
            })),
          el("div", { class: "g-lg-title" }, ["干预"]),
          el("select", { class: "g-select", id: "sIv", "aria-label": "干预类型" },
            ((M["causal-rules"] && M["causal-rules"].interventions) || []).map(function (i) { return el("option", { value: i.kind }, [i.kind]); })),
          el("select", { class: "g-select", id: "sIvTarget", "aria-label": "干预目标" }, [
            el("optgroup", { label: "节点" }, (M.architecture.nodes || []).map(function (n) {
              return el("option", { value: n.id }, [n.id]);
            })),
            el("optgroup", { label: "边" }, (M.architecture.edges || []).map(function (e2) {
              return el("option", { value: e2.id }, [e2.id + " (" + e2.type + ")"]);
            }))
          ]),
          el("button", { class: "g-btn", id: "sCompare", type: "button" }, ["A/B 重算"]),
          el("div", { class: "g-3d-runinfo", id: "causalInfo" }, ["—"])
        ]) : ""
      ]),
      el("div", { class: "g-3d-stage", id: "s3dStage" }, [
        el("div", { class: "g-split-ov", id: "gSplitOv", style: "display:none" }, [
          el("div", { class: "g-split-cap g-split-a" }, ["A · 基线"]),
          el("div", { class: "g-split-cap g-split-b" }, ["B · 干预后"]),
          el("div", { class: "g-split-line" }, [])
        ])
      ]),
      el("div", { class: "g-3d-status", id: "s3dStatus" }, ["—"])
    ]);
    host.insertAdjacentHTML("beforeend", wrap);

    var container = $("#s3dStage");
    if (!Spatial || !root.THREE) {
      container.innerHTML = el("div", { class: "g-degraded" }, ["DEGRADED · Three.js 不可用，3D 视图降级为 2D 拓扑。"]);
      $("#s3dStatus").textContent = "DEGRADED";
      drawFallback2D(container);
      return;
    }
    try {
      S.scene3d = Spatial.createScene(M.spatial, M.architecture, {
        container: container,
        // 3D 拾取 → 写回选中状态 → 检查器联动（ER/UML/时序/Case/证据按真实 ID）
        onSelect: function (sel) {
          S.canvas.selected = sel ? [sel.id] : [];
          renderRight();
        }
      });
    } catch (e) {
      container.innerHTML = el("div", { class: "g-degraded" }, ["DEGRADED · WebGL 初始化失败：" + esc(e.message)]);
      $("#s3dStatus").textContent = "DEGRADED · " + e.message;
      drawFallback2D(container);
      return;
    }
    if (!S.scene3d || !S.scene3d.renderer) {
      container.innerHTML = el("div", { class: "g-degraded" }, ["DEGRADED · WebGL 不可用，已切换到可交互 2D 拓扑。"]);
      $("#s3dStatus").textContent = "DEGRADED · WebGL unavailable";
      drawFallback2D(container);
      return;
    }

    var nodeCount = S.scene3d ? Object.keys(S.scene3d.nodeMeshes).length : 0;
    var roomCount = S.scene3d ? Object.keys(S.scene3d.roomMeshes).length : 0;
    $("#s3dStatus").textContent = "PASS · WebGL 可用 · 节点 " + nodeCount + " · 房间 " + roomCount;

    $("#s3dView").addEventListener("change", function () {
      var desc = S.scene3d.setView(this.value);
      $("#s3dStatus").textContent = "视图 " + this.value + " · " + desc;
    });
    $("#s3dReset").addEventListener("click", function () { S.scene3d.resetCamera(); });
    $("#s3dLabel").addEventListener("change", function () {
      S.scene3d.state.showLabels = this.value;
    });

    var runBtn = $("#sRun");
    if (runBtn) {
      runBtn.addEventListener("click", function () {
        var cid = $("#sCase").value;
        runCase(cid);
      });
    }
    var modeSel = $("#sIvMode");
    if (modeSel) {
      modeSel.value = S.ivMode || "Overlay3D";
      modeSel.addEventListener("change", function () {
        S.ivMode = this.value;
        applyIvMode();
      });
    }
    var cmpBtn = $("#sCompare");
    if (cmpBtn) {
      cmpBtn.addEventListener("click", function () {
        var kind = $("#sIv").value;
        var target = $("#sIvTarget").value;
        var iv = { id: "IV-" + kind + "-" + target, kind: kind, targetId: target, params: {} };
        if (kind === "ChangeGuard") {
          iv.params = { guardAst: { type: "always" } };
        }
        if (kind === "ChangeTimeout") {
          iv.params = { timeoutMs: 0 };
        }
        S.currentIntervention = iv;
        runCase(S.currentCase, iv);
      });
    }

    var loop = function () {
      if (!S.scene3d) return;
      S.scene3d.renderFrame();
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  /** 按可用宽度截断标签（CJK 记 1 个字宽，ASCII 记 0.56），避免文本溢出方框 */
  function fitLabel(text, maxPx) {
    var s = String(text == null ? "" : text);
    var w = 0, out = "";
    for (var i = 0; i < s.length; i++) {
      var cw = s.charCodeAt(i) > 0x2e80 ? 11 : 6.2;
      if (w + cw > maxPx) return out + "…";
      w += cw;
      out += s[i];
    }
    return out;
  }
  function drawFallback2D(container) {
    // 注意：el() 返回**HTML 字符串**，不能对其赋 innerHTML —— 必须用 insertAdjacentHTML 写入。
    // （此前把字符串当 DOM 用，导致 WebGL 降级路径直接抛错，降级形同虚设。）
    // 另：**不能清空容器** —— 降级横幅（g-degraded）必须留在页面上，降级才是"显式"的。
    var parts = [];
    M.architecture.nodes.forEach(function (n, i) {
      parts.push(el("circle", { cx: 100 + (i % 8) * 110, cy: 70 + Math.floor(i / 8) * 100, r: 22, class: "g-node-shape g-domain-" + n.domainId }));
      parts.push(el("text", { x: 100 + (i % 8) * 110, y: 74 + Math.floor(i / 8) * 100, class: "g-node-name" }, [fitLabel(n.name, 110)]));
    });
    var svg = el("svg", { class: "g-canvas", xmlns: "http://www.w3.org/2000/svg", width: "100%", height: "100%" },
      [parts.join("")]);
    container.insertAdjacentHTML("beforeend", svg);
  }

  function runCase(caseId, intervention) {
    if (!caseId) return;
    S.currentCase = caseId;
    var cdef = (M.cases.cases || []).filter(function (c) { return c.id === caseId; })[0];
    if (!cdef) return;
    var inputs = Object.assign({}, cdef.inputs || {});
    // 把 auth / operation 等条件补进输入，便于 guard 求值
    if (caseId.indexOf("AUTH") !== -1 || caseId.indexOf("LOGIN") !== -1) inputs.authPresent = true;
    if (caseId === "CASE-AUTH-MISSING") inputs.authPresent = false;
    if (caseId === "CASE-GIT-RECEIVE-UNAUTH") { inputs.authPresent = false; inputs.operation = "receive-pack"; }
    if (caseId === "CASE-GIT-UPLOADPACK-OPEN") { inputs.authPresent = false; inputs.operation = "upload-pack"; }
    if (caseId === "CASE-AI-ANTHROPIC-STREAM-REJECT") { inputs.provider = "anthropic"; inputs.streaming = true; }

    try {
      var run = S.engine.run(caseId, { seed: cdef.seed || 1, inputs: inputs, intervention: intervention });
      S.runs[caseId] = run;
      // 4D 的基线轨迹 —— 5D 建立在它之上
      if (!intervention) S.baselineRun = run;
      if (intervention) {
        S.causal = Causal.compare(S.engine, caseId, intervention, { seed: cdef.seed || 1, inputs: inputs });
        renderCausal(S.causal);
        applyIvMode();   // 按当前 5D 模式呈现（叠加 / 分屏 / 分支时间轴）
        // 5D 的首次分歧点标在同一个 3D 场景上（不是另一张图）
        if (S.scene3d) {
          S.scene3d.clearDivergenceMarks();
          var fd = S.causal.firstDivergence;
          if (fd) {
            var dn = (fd.branchA && fd.branchA.targetNodeId) || (fd.branchB && fd.branchB.targetNodeId);
            if (dn) S.scene3d.markDivergence(dn, true);
          }
        }
      }
      renderTimeline(run);
      applyRunStates(run);
      var info = $("#runInfo");
      if (info) {
        info.textContent = "PASS · " + run.eventCount + " 事件 · " + run.events.length +
          " 条快照 · 终态 " + Object.keys(run.finalStates).length + " 节点";
      }
    } catch (e) {
      var info2 = $("#runInfo");
      if (info2) { info2.textContent = "BLOCKED · " + e.name + ": " + e.message; }
      renderTimeline({ events: [], snapshots: [] });
    }
  }

  /**
   * 4D 作用在 3D 之上：把场景推进到第 upTo 个事件之后的状态。
   * 这是「4D 建立在 3D 基础上」的具体证据 —— 时间轴点一下，3D 场景真的回到那一刻：
   * 节点状态环按快照还原，被触发的边高亮。
   */
  function applyRunStates(run, upTo) {
    if (!S.scene3d || !run) return;
    var limit = (typeof upTo === "number") ? upTo : ((run.events || []).length - 1);
    // 取最后一个「事件数不超过 limit+1」的快照
    var snap = null;
    (run.snapshots || []).forEach(function (s) {
      if (s.eventCount <= limit + 1) snap = s;
    });
    var states = snap ? snap.states : run.finalStates;
    Object.keys(states || {}).forEach(function (nid) {
      S.scene3d.setRingState(nid, states[nid].state);
    });
    // 被触发的边高亮（4D 的事件在 3D 上留下痕迹）
    S.scene3d.clearEdgeMarks();
    for (var i = 0; i <= limit && i < (run.events || []).length; i++) {
      var ev = run.events[i];
      if (ev.kind === "Transition" && ev.edgeId) S.scene3d.markEdgeActive(ev.edgeId, true);
    }
  }

  function renderCausal(c) {
    var host = $("#causalInfo");
    if (!host || !c) return;
    if (c.outcome === "UNKNOWN") {
      host.innerHTML = el("div", { class: "g-causal unknown" }, [
        el("div", { class: "g-lg-title" }, ["UNKNOWN"]),
        el("div", {}, [esc(c.unknownReason || "没有可执行的规则，未产生结论")])
      ]);
      return;
    }
    if (c.outcome === "NO_DIFFERENCE") {
      host.innerHTML = el("div", { class: "g-causal nodiff" }, [
        el("div", { class: "g-lg-title" }, ["NO_DIFFERENCE"]),
        el("div", {}, ["干预未改变任何事件或终态 —— 不伪造分支。"])
      ]);
      return;
    }
    host.innerHTML = el("div", { class: "g-causal diverged" }, [
      el("div", { class: "g-lg-title" }, ["DIVERGED"]),
      el("table", { class: "g-table" }, [el("tbody", {}, [
        el("tr", {}, [el("td", {}, ["首次分歧序号"]), el("td", {}, [String(c.firstDivergence ? c.firstDivergence.atSequence : "—")])]),
        el("tr", {}, [el("td", {}, ["受影响节点"]), el("td", {}, [c.affectedNodes.join(", ") || "—"])]),
        el("tr", {}, [el("td", {}, ["传播路径"]), el("td", {}, [c.propagationPath.join(" → ") || "—"])]),
        el("tr", {}, [el("td", {}, ["终态差异"]), el("td", {}, [c.terminalDiff.length ? c.terminalDiff.map(function (t) { return t.nodeId + ": " + t.branchA + "→" + t.branchB; }).join("; ") : "—"])]),
        el("tr", {}, [el("td", {}, ["阻断 / 反馈 / 恢复 / 补偿"]), el("td", {}, [String(c.blocking || "—") + " / " + String(c.feedback || "—") + " / " + String(c.recovery || "—") + " / " + String(c.compensation || "—")])]),
        el("tr", {}, [el("td", {}, ["规则"]), el("td", {}, [c.ruleIds.join(", ") || "—"])])
      ])])
    ]);
  }

  function renderTimeline(run) {
    var track = $("#tlTrack");
    if (!track) return;
    track.innerHTML = "";
    var meta = $("#tlMeta");
    if (meta) meta.textContent = run && run.events ? (run.events.length + " 事件 · " + (run.snapshots || []).length + " 快照") : "—";
    var ctrl = $("#tlControls");
    if (ctrl) {
      ctrl.innerHTML = "";
      ["Play", "Pause", "StepForward", "StepBack", "Seek", "Speed", "Reset", "Snapshot", "Replay", "CaseSwitch"].forEach(function (c) {
        ctrl.insertAdjacentHTML("beforeend", el("button", { class: "g-tlbtn", type: "button", "data-tl": c }, [c]));
      });
    }
    if (!run || !run.events) return;
    // 5D BranchTimeline：A/B 双泳道 + 首次分歧标记
    if (S.ivMode === "BranchTimeline" && S.causal && S.causal.runA && S.causal.runB) {
      var divAt = S.causal.firstDivergence ? S.causal.firstDivergence.atSequence : -1;
      var laneHtml = function (label, r, markAt) {
        var items = (r.events || []).map(function (ev, i) {
          return el("div", {
            class: "g-tl-ev g-tl-" + String(ev.kind || "Transition").toLowerCase() + (i === markAt ? " g-tl-div" : ""),
            title: label + " #" + ev.sequence + " t=" + ev.logicalTime + " " + ev.kind + " " + ev.sourceNodeId + "→" + ev.targetNodeId,
            "data-seq": ev.sequence, "data-idx": i, "data-lane": label, role: "button", tabindex: "0"
          }, [String(i + 1)]);
        }).join("");
        return el("div", { class: "g-tl-lane" }, [
          el("span", { class: "g-tl-lane-name" }, [label + (markAt >= 0 ? " · 首次分歧 @" + markAt : "")]),
          el("div", { class: "g-tl-lane-track" }, [items])
        ]);
      };
      track.innerHTML = laneHtml("A · 基线", S.causal.runA, -1) + laneHtml("B · 干预后", S.causal.runB, divAt);
      track.querySelectorAll(".g-tl-ev").forEach(function (d) {
        var apply = function () {
          var lane = d.getAttribute("data-lane");
          var r = lane === "A · 基线" ? S.causal.runA : S.causal.runB;
          var idx = Number(d.getAttribute("data-idx"));
          track.querySelectorAll(".g-tl-ev").forEach(function (x) { x.classList.remove("cur"); });
          d.classList.add("cur");
          applyRunStates(r, idx);
        };
        d.addEventListener("click", apply);
        d.addEventListener("keydown", function (ev) {
          if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); apply(); }
        });
      });
      return;
    }
    run.events.forEach(function (ev, i) {
      track.insertAdjacentHTML("beforeend", el("div", {
        class: "g-tl-ev g-tl-" + String(ev.kind || "Transition").toLowerCase(),
        title: "#" + ev.sequence + " t=" + ev.logicalTime + " " + ev.kind + " " + ev.sourceNodeId + "→" + ev.targetNodeId,
        "data-seq": ev.sequence,
        "data-idx": i,
        role: "button", tabindex: "0"
      }, [String(i + 1)]));
    });
    // 4D → 3D：点时间轴上的事件，3D 场景真的回到那一刻
    track.querySelectorAll(".g-tl-ev").forEach(function (d) {
      var apply = function () {
        var idx = Number(d.getAttribute("data-idx"));
        track.querySelectorAll(".g-tl-ev").forEach(function (x) { x.classList.remove("cur"); });
        d.classList.add("cur");
        applyRunStates(run, idx);
        var meta = $("#tlMeta");
        if (meta) {
          var ev = run.events[idx];
          meta.textContent = "#" + ev.sequence + " · " + ev.kind + " · " +
            ev.sourceNodeId + " → " + ev.targetNodeId + " · t=" + ev.logicalTime;
        }
      };
      d.addEventListener("click", apply);
      d.addEventListener("keydown", function (ev) {
        if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); apply(); }
      });
    });
  }

  // ------------------------------------------------------- Prototype

  function renderPrototype(host) {
    // 原型 Tab 来自模型：primaryScreenGroup 优先，否则取第一个含 screens 的分组
    var tabs = [];
    var srcScreens = (M.prototype && M.prototype.sourceScreens) || {};
    var groupKey = (M.prototype && M.prototype.primaryScreenGroup) || null;
    if (groupKey && srcScreens[groupKey] && srcScreens[groupKey].screens) {
      tabs = srcScreens[groupKey].screens;
    } else {
      Object.keys(srcScreens).forEach(function (k) {
        if (!tabs.length && srcScreens[k] && srcScreens[k].screens) tabs = srcScreens[k].screens;
      });
    }
    var tabBar = el("div", { class: "g-proto-tabs", role: "tablist" }, tabs.map(function (t) {
      return el("button", {
        class: "g-proto-tab", role: "tab", "data-screen": t.id, type: "button",
        "aria-selected": "false"
      }, [t.route]);
    }));

    var SUBS = ["页面与屏幕", "可视编辑", "交互预览", "状态与 Case", "组件库", "设计与测试"];
    // 子标签是**真的工作台页签**：每页有独立内容块，切换即改显隐（DOM 状态不丢）
    var block = function (subs, children) {
      return el("div", { class: "g-proto-block", "data-subs": subs.join(" ") }, children);
    };
    var wrap = el("div", { class: "g-proto" }, [
      el("div", { class: "g-proto-subtabs", role: "tablist" }, SUBS.map(function (s, i) {
        return el("button", {
          class: "g-subtab" + (i === 0 ? " active" : ""), "data-sub": s, type: "button",
          role: "tab", "aria-selected": i === 0 ? "true" : "false"
        }, [s]);
      })),
      tabBar,
      el("div", { class: "g-proto-body" }, [
        el("div", { class: "g-proto-editor" }, [
          el("div", { class: "g-proto-tools" }, [
            el("button", { class: "g-btn", id: "prAdd", type: "button" }, ["添加控件"]),
            el("button", { class: "g-btn", id: "prUndo", type: "button" }, ["撤销"]),
            el("button", { class: "g-btn", id: "prRedo", type: "button" }, ["重做"]),
            el("button", { class: "g-btn", id: "prGrid", type: "button" }, ["网格"]),
            el("button", { class: "g-btn", id: "prSnap", type: "button" }, ["吸附"]),
            el("button", { class: "g-btn", id: "prGroup", type: "button" }, ["分组"]),
            el("button", { class: "g-btn", id: "prFront", type: "button" }, ["置顶"]),
            el("select", { class: "g-select", id: "prAlign", "aria-label": "对齐" },
              ["left", "right", "center-x", "top", "bottom", "center-y"].map(function (v) { return el("option", { value: v }, [v]); })),
            el("button", { class: "g-btn", id: "prExport", type: "button" }, ["导出 JSON"])
          ]),
          el("div", { class: "g-proto-canvas", id: "prStage" })
        ]),
        el("div", { class: "g-proto-preview" }, [
          block(["页面与屏幕"], [
            el("div", { class: "g-lg-title" }, ["屏幕元信息（来源：models/prototype.json）"]),
            el("pre", { class: "g-pre", id: "prScreenMeta" })
          ]),
          block(["页面与屏幕", "交互预览"], [
            el("div", { class: "g-lg-title" }, ["DOM 预览（真实控件）"]),
            el("div", { class: "g-proto-dom", id: "prPreview" })
          ]),
          block(["页面与屏幕", "可视编辑"], [
            el("div", { class: "g-lg-title" }, ["原型 JSON（序列化产物）"]),
            el("pre", { class: "g-pre", id: "prJson" })
          ]),
          block(["可视编辑"], [
            el("div", { class: "g-lg-title" }, ["控件清单（当前画布）"]),
            el("div", { class: "g-proto-list", id: "prWidgetList" })
          ]),
          block(["交互预览"], [
            el("div", { class: "g-lg-title" }, ["Action DSL（声明式动作与守卫）"]),
            el("div", { class: "g-proto-list", id: "prActionList" })
          ]),
          block(["状态与 Case"], [
            el("div", { class: "g-lg-title" }, ["状态表 / 变量 / Fixture"]),
            el("div", { class: "g-proto-list", id: "prStateList" })
          ]),
          block(["状态与 Case"], [
            el("div", { class: "g-lg-title" }, ["5D 绑定（Case → 屏幕 DOM 状态）"]),
            el("div", { class: "g-proto-list", id: "prBindList" })
          ]),
          block(["组件库"], [
            el("div", { class: "g-lg-title" }, ["组件库（点击插入到画布）"]),
            el("div", { class: "g-proto-list", id: "prLibrary" })
          ]),
          block(["设计与测试"], [
            el("div", { class: "g-lg-title" }, ["追踪矩阵（Requirement → 节点 → Case → 测试）"]),
            el("div", { class: "g-proto-list", id: "prTrace" })
          ])
        ])
      ])
    ]);
    host.insertAdjacentHTML("beforeend", wrap);

    var editor = null;
    try {
      editor = Proto.createEditor($("#prStage"), {
        screenId: tabs[0] ? tabs[0].id : "SC",
        variables: function () { return S.protoVars || {}; },
        stateTable: function () { return S.protoStateTable || { initial: "Idle", states: ["Idle", "Running", "Error"], transitions: [] }; },
        fixtures: function () { return S.protoFixtures || {}; }
      });
    } catch (e) {
      $("#prStage").innerHTML = el("div", { class: "g-degraded" }, ["DEGRADED · Konva 不可用：" + esc(e.message)]);
    }
    S.protoEditor = editor;

    // ---- 子标签内容渲染（每页真实内容，不是装饰按钮）----
    // 注意：本代码库的 el() 返回**HTML 字符串**（不是 DOM 节点），
    // 因此这里一律用 innerHTML 写入，绝不对 el() 的结果调用 appendChild。
    function row(cells, cls) {
      return el("div", { class: "g-proto-row" + (cls ? " " + cls : "") }, cells.map(function (c) {
        return el("span", { class: "g-proto-cell" }, [String(c)]);
      }));
    }
    function setHtml(sel, html) {
      var host = $(sel);
      if (host) host.innerHTML = html;
    }
    function renderProtoPanels() {
      var p = currentProto();
      var proto = M.prototype || {};
      // 控件清单
      var wl = [];
      if (!p || !(p.widgets || []).length) {
        wl.push(el("div", { class: "g-proto-empty" }, ["画布上还没有控件（用「添加控件」或组件库插入）"]));
      } else {
        p.widgets.forEach(function (w) {
          wl.push(row([w.id, w.kind, w.name || "", w.text || "", "actions=" + ((w.actions || []).length)]));
        });
      }
      setHtml("#prWidgetList", wl.join(""));
      // Action DSL
      var al = [];
      var acts = [];
      if (p) (p.widgets || []).forEach(function (w) {
        (w.actions || []).forEach(function (a) { acts.push([w.id, a.type, a.target || "", a.value || a.caseId || a.fixtureId || ""]); });
      });
      if (!acts.length) al.push(el("div", { class: "g-proto-empty" }, ["当前控件未绑定动作"]));
      acts.forEach(function (a) { al.push(row(a)); });
      al.push(el("div", { class: "g-lg-title" }, ["可用动作目录（models/prototype.json）"]));
      (proto.actions || []).forEach(function (t) { al.push(row([t], "g-proto-row-dim")); });
      setHtml("#prActionList", al.join(""));
      // 状态表 / 变量 / Fixture
      var sl = [];
      var st = S.protoStateTable || { initial: "Idle", states: ["Idle", "Running", "Error"], transitions: [] };
      sl.push(row(["初始状态", st.initial]));
      sl.push(row(["状态集合", (st.states || []).join(" / ")]));
      (st.transitions || []).forEach(function (t) {
        sl.push(row([(t.from || "?") + " → " + (t.to || "?"), t.event || "", t.guard || ""], "g-proto-row-dim"));
      });
      var vars = S.protoVars || {};
      sl.push(row(["变量", Object.keys(vars).length ? Object.keys(vars).map(function (k) { return k + "=" + vars[k]; }).join(", ") : "（无）"], "g-proto-row-dim"));
      var fx = S.protoFixtures || {};
      sl.push(row(["Fixture", Object.keys(fx).length ? Object.keys(fx).join(", ") : "（无）"], "g-proto-row-dim"));
      setHtml("#prStateList", sl.join(""));
      // 5D 绑定
      var bl = [];
      var binds = (proto.fiveDBinding && proto.fiveDBinding.bindings) || [];
      if (!binds.length) bl.push(el("div", { class: "g-proto-empty" }, ["未登记 5D 绑定"]));
      binds.forEach(function (b) {
        bl.push(row([b.caseId, b.screenId, b.domState], b.screenId === (p && p.screenId) ? "g-proto-row-hl" : ""));
        if (b.note) bl.push(row([b.note], "g-proto-row-dim"));
      });
      setHtml("#prBindList", bl.join(""));
      // 组件库（按钮事件在写入 DOM 后再绑定 —— el() 只是字符串）
      var lib = [];
      (proto.widgets || []).forEach(function (kind) {
        lib.push(el("button", { class: "g-btn g-proto-lib-btn", type: "button", "data-lib": kind }, [kind]));
      });
      if (proto.workbench && proto.workbench.editor) {
        lib.push(el("div", { class: "g-lg-title" }, ["编辑引擎能力"]));
        (proto.workbench.editor.capabilities || []).forEach(function (c) {
          lib.push(row([c], "g-proto-row-dim"));
        });
      }
      setHtml("#prLibrary", lib.join(""));
      $$("#prLibrary .g-proto-lib-btn").forEach(function (b) {
        b.addEventListener("click", function () {
          if (!editor) return;
          editor.snapshot();
          editor.addWidget(b.getAttribute("data-lib"), 60 + Math.floor(Math.random() * 160), 60 + Math.floor(Math.random() * 200), {
            kind: b.getAttribute("data-lib"), name: b.getAttribute("data-lib"), text: b.getAttribute("data-lib"), actions: []
          });
          refreshPreview();
        });
      });
      // 设计与测试：追踪矩阵
      var tr = [];
      var sid = p ? p.screenId : null;
      var mrows = ((M.traceability && M.traceability.matrix) || []).filter(function (m) {
        return !sid || (m.screenIds || []).indexOf(sid) !== -1;
      });
      if (!mrows.length) tr.push(el("div", { class: "g-proto-empty" }, ["该屏幕暂无追踪行"]));
      mrows.forEach(function (m) {
        tr.push(row([m.requirementId, (m.promptIds || []).join(",")], "g-proto-row-hl"));
        tr.push(row(["DD 节点", (m.ddNodeIds || []).join(", ")], "g-proto-row-dim"));
        tr.push(row(["Case", (m.caseIds || []).join(", "), "测试", (m.testIds || []).join(", ")], "g-proto-row-dim"));
      });
      setHtml("#prTrace", tr.join(""));
    }

    function applyProtoSub(sub) {
      S.protoSub = sub;
      $$(".g-subtab").forEach(function (b) {
        var on = b.getAttribute("data-sub") === sub;
        b.classList.toggle("active", on);
        b.setAttribute("aria-selected", on ? "true" : "false");
      });
      $$(".g-proto-block").forEach(function (bl) {
        var subs = (bl.getAttribute("data-subs") || "").split(/\s+/);
        bl.style.display = subs.indexOf(sub) === -1 ? "none" : "";
      });
      renderProtoPanels();
    }
    $$(".g-subtab").forEach(function (b) {
      b.addEventListener("click", function () { applyProtoSub(b.getAttribute("data-sub")); });
    });
    applyProtoSub(SUBS[0]);

    function currentProto() {
      if (!editor) return null;
      var p = editor.serialise();
      var sc = tabs.filter(function (t) { return t.id === p.screenId; })[0] || tabs[0];
      p.screenId = sc ? sc.id : p.screenId;
      p.sourceStatus = sc ? "Implemented" : "Designed";
      return p;
    }

    function refreshPreview() {
      var p = currentProto();
      if (p) {
        var r = Proto.renderPreview($("#prPreview"), p, {});
        $("#prJson").textContent = JSON.stringify(p, null, 2);
        S.protoLast = r;
      }
      renderProtoPanels();
    }

    if (editor) {
      // 预置控件一律来自模型（models/prototype.json 的 seedWidgets），不写死任何项目内容
      var seed = (M.prototype && M.prototype.seedWidgets) || [];
      seed.forEach(function (w) {
        if (w.kind === "Hotspot") {
          editor.addHotspot(w.targetId, w.x, w.y, w.w || 30, w.h || 30, w.actions || []);
        } else {
          editor.addWidget(w.kind, w.x, w.y, {
            widgetId: w.id, kind: w.kind, name: w.name, text: w.text,
            actions: w.actions || [], guards: w.guards || null, hotspot: !!w.hotspot
          });
        }
      });
    }

    $$("[data-screen]").forEach(function (b) {
      b.addEventListener("click", function () {
        $$("[data-screen]").forEach(function (x) { x.setAttribute("aria-selected", "false"); });
        b.setAttribute("aria-selected", "true");
        var sc = tabs.filter(function (t) { return t.id === b.getAttribute("data-screen"); })[0];
        if (editor) editor.serialise().screenId = b.getAttribute("data-screen");
        // 屏幕元信息写入独立面板 —— #prJson 恒为 Prototype JSON（面板语义不得漂移）
        var meta = $("#prScreenMeta");
        if (meta) {
          meta.textContent = sc
            ? JSON.stringify({
                screenId: sc.id, route: sc.route, component: sc.component,
                status: "Implemented", sections: sc.sections,
                knownIssue: sc.knownIssue || null, evidenceIds: sc.evidenceIds || []
              }, null, 2)
            : "(该屏幕不在 models/prototype.json 声明的屏幕集合内)";
        }
      });
    });

    $("#prAdd").addEventListener("click", function () {
      if (!editor) return;
      editor.snapshot();               // 变更前落快照，否则 undo 无内容可回退
      editor.addWidget("Button", 60 + Math.floor(Math.random() * 120), 60 + Math.floor(Math.random() * 160), { kind: "Button", name: "新按钮", text: "新按钮", actions: [{ type: "SetState", target: "status", value: "Running" }] });
      refreshPreview();
    });
    $("#prUndo").addEventListener("click", function () { if (editor) { editor.undo(); refreshPreview(); } });
    $("#prRedo").addEventListener("click", function () { if (editor) { editor.redo(); refreshPreview(); } });
    $("#prGrid").addEventListener("click", function () { if (editor) editor.toggleGrid(); });
    $("#prSnap").addEventListener("click", function () { if (editor) editor.setSnap(!editor.isSnapOn()); });
    $("#prGroup").addEventListener("click", function () { if (editor) { editor.snapshot(); editor.groupWidgets(); refreshPreview(); } });
    $("#prFront").addEventListener("click", function () { if (editor) { editor.snapshot(); editor.zOrder("front"); refreshPreview(); } });
    $("#prAlign").addEventListener("change", function () { if (editor) { editor.snapshot(); editor.align(this.value); refreshPreview(); } });
    $("#prExport").addEventListener("click", function () {
      if (!editor) return;
      var blob = new Blob([JSON.stringify(editor.serialise(), null, 2)], { type: "application/json" });
      var a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "prototype." + editor.serialise().screenId + ".json";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
    });

    refreshPreview();
  }

  var API = { init: init, state: S, render: renderView, runCase: runCase };
  root.GemArcApp = API;
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})(typeof globalThis !== "undefined" ? globalThis : this);
