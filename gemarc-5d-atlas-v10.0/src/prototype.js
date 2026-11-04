/**
 * src/prototype.js — 原型工作区：Action DSL + 状态机 + Konva 编辑器 + 真实 DOM 预览
 *
 * 契约（v10.0 prototype_workbench）：
 *   - required_now：图集生成时就必须带可操作的多标签原型，不是空菜单。
 *   - Konva 负责 WYSIWYG 编辑；预览必须是真实 HTML DOM/CSS 表单，
 *     不用 Konva shape 假装可以原生输入；文本输入用真实 input/textarea。
 *   - 序列化是 Prototype JSON 规则，不只序列化 Konva Stage.toJSON。
 *   - 支持 ImportAndReplay：导入后 Action 不丢失。
 */
(function (root) {
  "use strict";

  var Konva = root.Konva;

  // ------------------------------------------------------- Action DSL

  var ACTION_IMPLS = {
    Navigate: function (a, ctx) { ctx.navigate(a.to); return "navigate:" + a.to; },
    OpenTab: function (a, ctx) { ctx.openTab(a.screenId); return "openTab:" + a.screenId; },
    SwitchTab: function (a, ctx) { ctx.switchTab(a.screenId); return "switchTab:" + a.screenId; },
    SetState: function (a, ctx) { ctx.setState(a.target, a.value); return "setState:" + a.target + "=" + a.value; },
    SetVariable: function (a, ctx) { ctx.setVariable(a.name, a.value); return "setVar:" + a.name + "=" + a.value; },
    ShowHide: function (a, ctx) { ctx.showHide(a.target, a.visible); return "showHide:" + a.target; },
    Validate: function (a, ctx) { return "validate:" + (ctx.validate(a.target) ? "ok" : "fail"); },
    SubmitMock: function (a, ctx) { ctx.submitMock(a.target); return "submitMock:" + a.target; },
    OpenDialog: function (a, ctx) { ctx.openDialog(a.dialogId); return "openDialog:" + a.dialogId; },
    CloseDialog: function (a, ctx) { ctx.closeDialog(a.dialogId); return "closeDialog:" + a.dialogId; },
    Filter: function (a, ctx) { ctx.filter(a.collection, a.value); return "filter:" + a.collection; },
    Sort: function (a, ctx) { ctx.sort(a.collection, a.by, a.dir); return "sort:" + a.collection; },
    Load: function (a, ctx) { ctx.load(a.fixtureId); return "load:" + a.fixtureId; },
    Fail: function (a, ctx) { ctx.fail(a.message || "failed"); return "fail"; },
    Retry: function (a, ctx) { ctx.retry(a.actionId); return "retry:" + a.actionId; },
    TriggerCase: function (a, ctx) { ctx.triggerCase(a.caseId); return "triggerCase:" + a.caseId; }
  };

  /**
   * 执行一个 Action。未知 Action 类型抛错 —— 不静默忽略。
   * @returns {{ok: boolean, result: string, error?: string}}
   */
  function runAction(action, ctx) {
    var impl = ACTION_IMPLS[action.type];
    if (!impl) {
      return { ok: false, result: "", error: "unknown action type: " + action.type };
    }
    try {
      return { ok: true, result: impl(action, ctx) };
    } catch (e) {
      return { ok: false, result: "", error: e.message };
    }
  }

  /** 求值 guard：支持 {type:"always"} / {type:"equals"} / {type:"not"} */
  function evalGuard(guard, vars) {
    if (!guard) return true;
    switch (guard.type) {
      case "always": return true;
      case "never": return false;
      case "not": return !evalGuard(guard.arg, vars);
      case "equals": return vars[guard.left] === guard.right;
      case "notEquals": return vars[guard.left] !== guard.right;
      case "and": return (guard.args || []).every(function (g) { return evalGuard(g, vars); });
      case "or": return (guard.args || []).some(function (g) { return evalGuard(g, vars); });
      default: return false;
    }
  }

  // ------------------------------------------------------ 状态机

  /**
   * 显式状态转移表。必须有显式 transition 与 guard，不靠隐式推断。
   */
  function createStateMachine(def) {
    var states = (def.states || []).slice();
    var transitions = (def.transitions || []).slice();
    var current = def.initial || states[0];
    var vars = Object.assign({}, def.vars || {});
    var history = [];

    function fire(event, payload) {
      var t = transitions.filter(function (tr) {
        return tr.from === current && tr.event === event &&
          (tr.guard === undefined || evalGuard(tr.guard, Object.assign({}, vars, payload || {})));
      })[0];
      if (!t) return { ok: false, error: "no transition from '" + current + "' on '" + event + "'" };
      history.push({ from: current, to: t.to, event: event });
      current = t.to;
      return { ok: true, from: history[history.length - 1].from, to: current };
    }

    return {
      states: states,
      transitions: transitions,
      get current() { return current; },
      vars: vars,
      history: history,
      fire: fire,
      setVar: function (k, v) { vars[k] = v; },
      toTable: function () {
        return transitions.map(function (t) {
          return { from: t.from, event: t.event, to: t.to, guard: t.guard ? JSON.stringify(t.guard) : "" };
        });
      }
    };
  }

  // -------------------------------------------------- Konva 编辑器

  /**
   * Konva Stage 封装：Stage/Layer/Transformer/Drag/Resize/Selection/MultiSelection/
   * Snap/Grid/Alignment/Group/ZOrder/Undo/Redo/Hotspot
   */
  function createEditor(container, opts) {
    opts = opts || {};
    if (!Konva) return null;

    var stage = new Konva.Stage({
      container: container,
      width: container.clientWidth || 600,
      height: container.clientHeight || 420
    });
    var layer = new Konva.Layer({ id: "main" });
    stage.add(layer);

    var transformer = new Konva.Transformer({ rotateEnabled: false, keepRatio: false });
    layer.add(transformer);

    var gridEnabled = false;
    var snapEnabled = true;
    var GRID = 16;
    var undoStack = [];
    var redoStack = [];

    function snapshot() {
      undoStack.push(JSON.stringify(serialise()));
      if (undoStack.length > 80) undoStack.shift();
      redoStack.length = 0;
    }

    function undo() {
      if (!undoStack.length) return false;
      var prev = undoStack.pop();
      redoStack.push(JSON.stringify(serialise()));
      restore(JSON.parse(prev));
      return true;
    }

    function redo() {
      if (!redoStack.length) return false;
      var next = redoStack.pop();
      undoStack.push(JSON.stringify(serialise()));
      restore(JSON.parse(next));
      return true;
    }

    /** Prototype JSON：不只序列化 Konva Stage */
    function serialise() {
      var widgets = [];
      // 不依赖 Konva 的 CSS 选择器语义 —— 直接遍历 layer 子节点并按 widgetId 过滤
      layer.getChildren().forEach(function (n) {
        if (!n.getAttr("widgetId")) return;
        widgets.push({
          id: n.getAttr("widgetId"),
          kind: n.getAttr("kind"),
          x: n.x(), y: n.y(), width: n.width(), height: n.height(),
          text: n.getAttr("text") || "",
          name: n.getAttr("name") || "",
          state: n.getAttr("state") || "",
          actions: n.getAttr("actions") || [],
          guards: n.getAttr("guards") || null,
          hotspot: n.getAttr("hotspot") || null
        });
      });
      return {
        format: "ide1-prototype/v10.0",
        screenId: opts.screenId || "SC-UNSPECIFIED",
        revision: 1,
        widgets: widgets,
        variables: opts.variables ? opts.variables() : {},
        stateTable: opts.stateTable ? opts.stateTable() : [],
        fixtures: opts.fixtures ? opts.fixtures() : {}
      };
    }

    function restore(data) {
      // 先收集再删除 —— 边遍历边 destroy 会漏删元素（数组被就地修改）
      var toRemove = [];
      layer.getChildren().forEach(function (n) {
        if (n.getAttr("widgetId")) toRemove.push(n);
      });
      toRemove.forEach(function (n) { n.destroy(); });
      (data.widgets || []).forEach(function (w) {
        addWidget(w.kind, w.x, w.y, {
          widgetId: w.id, name: w.name, text: w.text,
          actions: w.actions, guards: w.guards, hotspot: w.hotspot
        });
      });
      layer.draw();
    }

    function addWidget(kind, x, y, meta) {
      meta = meta || {};
      var group = new Konva.Group({
        x: x, y: y, name: "widget",
        className: "widget",
        draggable: true
      });
      group.setAttr("widgetId", meta.widgetId || ("W-" + Date.now() + "-" + Math.floor(Math.random() * 1e4)));
      group.setAttr("kind", kind);
      group.setAttr("name", meta.name || kind);
      group.setAttr("text", meta.text || "");
      group.setAttr("actions", meta.actions || []);
      group.setAttr("guards", meta.guards || null);
      group.setAttr("hotspot", meta.hotspot || null);

      var w = 140, h = 44;
      var shape;
      if (kind === "Input" || kind === "Textarea") {
        shape = new Konva.Rect({ width: w, height: kind === "Textarea" ? 72 : h, fill: "#121D2E", stroke: "#5AC7C3", cornerRadius: 6, strokeWidth: 1 });
      } else if (kind === "Button") {
        shape = new Konva.Rect({ width: w, height: h, fill: "#1C2A3D", stroke: "#5AC7C3", cornerRadius: 6, strokeWidth: 1 });
      } else if (kind === "Card") {
        shape = new Konva.Rect({ width: 190, height: 110, fill: "#121D2E", stroke: "#40536A", cornerRadius: 10, strokeWidth: 1 });
      } else {
        shape = new Konva.Rect({ width: w, height: h, fill: "#121D2E", stroke: "#82ADFA", cornerRadius: 6, strokeWidth: 1 });
      }
      group.add(shape);
      var label = new Konva.Text({
        text: meta.text || kind, x: 10, y: kind === "Textarea" ? 10 : 14,
        fontSize: 13, fontFamily: "system-ui, sans-serif", fill: "#E8EFF7",
        width: shape.width() - 20
      });
      group.add(label);

      group.on("dragstart", function () { snapshot(); });
      group.on("dragmove", function () {
        if (!snapEnabled) return;
        group.position({
          x: Math.round(group.x() / GRID) * GRID,
          y: Math.round(group.y() / GRID) * GRID
        });
      });
      group.on("click", function (ev) {
        ev.cancelBubble = true;
        var t = transformer.nodes()[0];
        if (ev.evt.shiftKey) {
          var nodes = t ? t.nodes() : [];
          if (nodes.indexOf(group) === -1) nodes.push(group);
          transformer.nodes(nodes);
        } else {
          transformer.nodes([group]);
        }
        layer.draw();
      });

      layer.add(group);
      layer.draw();
      return group;
    }

    function selectedIds() {
      return transformer.nodes().map(function (n) { return n.getAttr("widgetId"); });
    }

    function align(kind) {
      var nodes = transformer.nodes();
      if (nodes.length < 2) return false;
      snapshot();
      var xs = nodes.map(function (n) { return n.x(); });
      var ys = nodes.map(function (n) { return n.y(); });
      var minX = Math.min.apply(null, xs), maxX = Math.max.apply(null, xs);
      var minY = Math.min.apply(null, ys), maxY = Math.max.apply(null, ys);
      nodes.forEach(function (n) {
        if (kind === "left") n.x(minX);
        if (kind === "right") n.x(maxX);
        if (kind === "center-x") n.x((minX + maxX) / 2);
        if (kind === "top") n.y(minY);
        if (kind === "bottom") n.y(maxY);
        if (kind === "center-y") n.y((minY + maxY) / 2);
      });
      layer.draw();
      return true;
    }

    function groupWidgets() {
      var nodes = transformer.nodes();
      if (nodes.length < 2) return false;
      snapshot();
      var g = new Konva.Group({ name: "widget-group" });
      var minX = Infinity, minY = Infinity;
      nodes.forEach(function (n) { minX = Math.min(minX, n.x()); minY = Math.min(minY, n.y()); });
      nodes.forEach(function (n) {
        n.position({ x: n.x() - minX, y: n.y() - minY });
        g.add(n);
      });
      g.setAttr("kind", "Group");
      g.setAttr("widgetId", "GRP-" + Date.now());
      g.setAttr("actions", []);
      g.setAttr("text", "");
      layer.add(g);
      transformer.nodes([g]);
      layer.draw();
      return true;
    }

    function zOrder(dir) {
      var nodes = transformer.nodes();
      if (!nodes.length) return false;
      snapshot();
      nodes.forEach(function (n) {
        if (dir === "front") layer.add(n);
        else layer.moveToBottom(n);
      });
      layer.draw();
      return true;
    }

    function toggleGrid() {
      gridEnabled = !gridEnabled;
      drawGrid();
      return gridEnabled;
    }

    function drawGrid() {
      layer.find(".gridline").forEach(function (l) { l.destroy(); });
      if (!gridEnabled) { layer.draw(); return; }
      for (var x = 0; x < stage.width(); x += GRID) {
        layer.add(new Konva.Line({ points: [x, 0, x, stage.height()], stroke: "#1C2A3D", strokeWidth: 1, name: "gridline", listening: false }));
      }
      for (var y = 0; y < stage.height(); y += GRID) {
        layer.add(new Konva.Line({ points: [0, y, stage.width(), y], stroke: "#1C2A3D", strokeWidth: 1, name: "gridline", listening: false }));
      }
      layer.draw();
    }

    function addHotspot(targetId, x, y, w, h, actions) {
      var r = new Konva.Rect({
        x: x, y: y, width: w, height: h,
        fill: "rgba(90,199,195,0.12)", stroke: "#5AC7C3", strokeWidth: 1,
        dash: [4, 3], name: "hotspot"
      });
      r.setAttr("hotspotTarget", targetId);
      r.setAttr("actions", actions || []);
      layer.add(r);
      layer.draw();
      return r;
    }

    function resize() {
      stage.width(container.clientWidth || 600);
      stage.height(container.clientHeight || 420);
      layer.draw();
    }

    function destroy() {
      stage.destroy();
    }

    return {
      stage: stage, layer: layer, transformer: transformer,
      addWidget: addWidget, serialise: serialise, restore: restore,
      undo: undo, redo: redo, snapshot: snapshot,
      selectedIds: selectedIds, align: align, groupWidgets: groupWidgets,
      zOrder: zOrder, toggleGrid: toggleGrid, addHotspot: addHotspot,
      resize: resize, destroy: destroy,
      isGridOn: function () { return gridEnabled; },
      isSnapOn: function () { return snapEnabled; },
      setSnap: function (v) { snapEnabled = !!v; }
    };
  }

  // -------------------------------------------------- DOM 预览

  /**
   * 真实 DOM 预览：按 widget 列表渲染原生控件。
   * Konva 只负责编辑；预览区是真正的 HTML 元素，因此可原生输入。
   */
  function renderPreview(host, proto, ctx) {
    host.innerHTML = "";
    var state = {
      vars: Object.assign({}, (proto && proto.variables) || {}),
      hidden: {},
      dialogs: {}
    };
    var sm = createStateMachine({
      initial: (proto && proto.stateTable && proto.stateTable.initial) || "Idle",
      states: (proto && proto.stateTable && proto.stateTable.states) || ["Idle", "Running", "Error"],
      transitions: (proto && proto.stateTable && proto.stateTable.transitions) || [],
      vars: state.vars
    });

    var api = {
      navigate: function (to) { state.vars.__nav = to; },
      openTab: function (id) { state.vars.__tab = id; },
      switchTab: function (id) { state.vars.__tab = id; },
      setState: function (t, v) { state.vars[t] = v; },
      setVariable: function (k, v) { state.vars[k] = v; },
      showHide: function (t, vis) { state.hidden[t] = vis !== false; },
      validate: function (t) {
        var el = host.querySelector('[data-field="' + t + '"]');
        if (!el) return true;
        return el.value.trim().length > 0;
      },
      submitMock: function (t) { state.vars.__submitted = t; },
      openDialog: function (id) { state.dialogs[id] = true; },
      closeDialog: function (id) { state.dialogs[id] = false; },
      filter: function (c, v) { state.vars.__filter = v; },
      sort: function (c, by, dir) { state.vars.__sort = by + ":" + dir; },
      load: function (id) { state.vars.__fixture = id; },
      fail: function (msg) { state.vars.__error = msg; },
      retry: function (id) { state.vars.__retry = id; },
      triggerCase: function (id) { state.vars.__case = id; }
    };

    (proto && proto.widgets || []).forEach(function (w) {
      if (state.hidden[w.id]) return;
      var el;
      if (w.kind === "Input") {
        el = document.createElement("input");
        el.type = "text";
        el.className = "proto-input";
        el.dataset.field = w.id;
        el.placeholder = w.text || "输入";
        el.setAttribute("aria-label", w.name || w.id);
      } else if (w.kind === "Textarea") {
        el = document.createElement("textarea");
        el.className = "proto-textarea";
        el.dataset.field = w.id;
        el.placeholder = w.text || "输入";
        el.setAttribute("aria-label", w.name || w.id);
      } else if (w.kind === "Button") {
        el = document.createElement("button");
        el.type = "button";
        el.className = "proto-button";
        el.textContent = w.text || "按钮";
        el.addEventListener("click", function () {
          (w.actions || []).forEach(function (a) {
            var r = runAction(a, api);
            if (!r.ok) state.vars.__error = r.error;
          });
          api.__changed = true;
        });
      } else if (w.kind === "Checkbox") {
        el = document.createElement("input");
        el.type = "checkbox";
        el.className = "proto-checkbox";
        el.dataset.field = w.id;
        el.setAttribute("aria-label", w.name || w.id);
      } else if (w.kind === "Select") {
        el = document.createElement("select");
        el.className = "proto-select";
        el.dataset.field = w.id;
        el.setAttribute("aria-label", w.name || w.id);
        (w.options || ["选项 A", "选项 B"]).forEach(function (o) {
          var opt = document.createElement("option");
          opt.textContent = o;
          el.appendChild(opt);
        });
      } else if (w.kind === "Table") {
        el = document.createElement("table");
        el.className = "proto-table";
        var tb = document.createElement("tbody");
        (w.rows || []).forEach(function (row) {
          var tr = document.createElement("tr");
          row.forEach(function (cell) {
            var td = document.createElement("td");
            td.textContent = cell;
            tr.appendChild(td);
          });
          tb.appendChild(tr);
        });
        el.appendChild(tb);
      } else {
        el = document.createElement("div");
        el.className = "proto-card";
        el.textContent = w.text || w.kind;
      }
      el.setAttribute("data-widget", w.id);
      el.setAttribute("data-kind", w.kind);
      host.appendChild(el);
    });

    return {
      state: state,
      machine: sm,
      api: api,
      vars: function () { return state.vars; },
      html: host.innerHTML
    };
  }

  var API = {
    runAction: runAction,
    evalGuard: evalGuard,
    createStateMachine: createStateMachine,
    createEditor: createEditor,
    renderPreview: renderPreview,
    ACTION_TYPES: Object.keys(ACTION_IMPLS)
  };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcPrototype = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
