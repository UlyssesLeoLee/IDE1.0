/**
 * src/engine.js — 4D 确定性离散事件引擎 + 节点状态机
 *
 * 契约（v10.0 immutable_dimensions.4D / simulation_and_causality）：
 *   - 引擎与 Three.js 完全解耦；动画只回放 Event Trace，业务结果不由 FPS 决定。
 *   - 事件顺序 = (logicalTime, sequence) 严格升序。
 *   - 相同 model + 相同 case + 相同 seed ⇒ 相同规范轨迹。
 *   - safety_limits：maxEvents / maxSteps / maxIterations / maxMillis 全部生效，
 *     超限抛 BudgetExceeded，绝不静默截断后假装跑完。
 */
(function (root) {
  "use strict";

  var DEFAULT_LIMITS = {
    maxEvents: 500,
    maxSteps: 5000,
    maxIterations: 20000,
    maxMillis: 2000
  };

  function BudgetExceeded(message, limit) {
    var e = new Error(message);
    e.name = "BudgetExceeded";
    e.limit = limit;
    return e;
  }

  /** 深拷贝，保证干预不会污染正式事实图 */
  function clone(obj) {
    return JSON.parse(JSON.stringify(obj));
  }

  // ---------------------------------------------------------------- guard

  /**
   * 求值 guardAst。
   * 变量取值：{"var":"state:<nodeId>"} / {"var":"counter"} / {"var":"input:<key>"}
   */
  function evalGuard(ast, ctx) {
    if (!ast) return true;
    switch (ast.type) {
      case "always": return true;
      case "never": return false;
      case "not": return !evalGuard(ast.arg, ctx);
      case "and": return (ast.args || []).every(function (a) { return evalGuard(a, ctx); });
      case "or": return (ast.args || []).some(function (a) { return evalGuard(a, ctx); });
      case "equals": {
        var l = readVar(ast.left, ctx);
        var r = readVar(ast.right, ctx);
        // 变量缺失 ⇒ 判否（缺标比错标安全）
        if (l === undefined || r === undefined) return false;
        return l === r;
      }
      case "notEquals": {
        var lv = readVar(ast.left, ctx);
        if (lv === undefined) return false; // 缺标 ⇒ 判否，不当作"与某值不同"
        return lv !== readVar(ast.right, ctx);
      }
      case "gte": {
        var g = readVar(ast.left, ctx);
        if (g === undefined) return false;
        return Number(g) >= Number(readVar(ast.right, ctx));
      }
      case "lte": {
        var le = readVar(ast.left, ctx);
        if (le === undefined) return false;
        return Number(le) <= Number(readVar(ast.right, ctx));
      }
      case "gt": {
        var gt = readVar(ast.left, ctx);
        if (gt === undefined) return false;
        return Number(gt) > Number(readVar(ast.right, ctx));
      }
      case "contains": {
        var cs = readVar(ast.left, ctx);
        var cn = readVar(ast.right, ctx);
        if (cs === undefined || cn === undefined) return false;
        return String(cs).indexOf(String(cn)) !== -1;
      }
      case "startsWith": {
        var sw = readVar(ast.left, ctx);
        var sn = readVar(ast.right, ctx);
        if (sw === undefined || sn === undefined) return false;
        return String(sw).indexOf(String(sn)) === 0;
      }
      case "matchesDriveLetter": {
        // Windows 盘符前缀判定（如 "C:\\Windows" 或 "C|foo"）：
        // 首字符为 ASCII 字母，且第二字符为 ':' 或 '|'。
        // 与 IDE1.0 的 path_safety 对 "\\?\" 前缀与盘符的处理一致。
        // 注：当前 IDE1.0 模型的规则未使用该算子（保留为引擎通用算子）。
        var dv = ast.value ? readVar(ast.value, ctx) : readVar(ast.left, ctx);
        if (dv === undefined) return false;
        return /^[A-Za-z][:|]/.test(String(dv));
      }
      default:
        // 未知 guard 一律不通过（缺标比错标安全）
        return false;
    }
  }

  function readVar(ref, ctx) {
    if (ref === null || ref === undefined) return undefined;
    if (typeof ref !== "object") return ref;
    if (ref.type === "var") {
      var path = String(ref.var);
      if (path.indexOf("state:") === 0) {
        var nid = path.slice(6);
        var st = ctx.states[nid];
        return st ? st.state : undefined;
      }
      if (path.indexOf("input:") === 0) {
        var key = path.slice(6);
        return ctx.inputs ? ctx.inputs[key] : undefined;
      }
      if (path.indexOf("counter:") === 0) {
        var cid = path.slice(8);
        return ctx.counters[cid] || 0;
      }
      return ctx.inputs ? ctx.inputs[path] : undefined;
    }
    if (ref.type === "literal") return ref.value;
    return undefined;
  }

  /** 把 guardAst 渲染成人类可读串（用于 5D 因果解释） */
  function describeGuard(ast) {
    if (!ast) return "always";
    switch (ast.type) {
      case "always": return "always";
      case "never": return "never";
      case "not": return "NOT(" + describeGuard(ast.arg) + ")";
      case "and": return "(" + (ast.args || []).map(describeGuard).join(" AND ") + ")";
      case "or": return "(" + (ast.args || []).map(describeGuard).join(" OR ") + ")";
      case "equals": return describeRef(ast.left) + " == " + JSON.stringify(ast.right);
      case "notEquals": return describeRef(ast.left) + " != " + JSON.stringify(ast.right);
      case "gte": return describeRef(ast.left) + " >= " + JSON.stringify(ast.right);
      case "lte": return describeRef(ast.left) + " <= " + JSON.stringify(ast.right);
      case "gt": return describeRef(ast.left) + " > " + JSON.stringify(ast.right);
      default: return "unknown(" + ast.type + ")";
    }
  }

  function describeRef(ref) {
    if (ref && typeof ref === "object" && ref.type === "var") return String(ref.var);
    return JSON.stringify(ref);
  }

  // ------------------------------------------------------------- engine

  /**
   * @param {object} model { nodes, edges, rules, cases, stateMachines, version }
   * @param {object} [limits]
   */
  function createEngine(model, limits) {
    var LIM = Object.assign({}, DEFAULT_LIMITS, limits || {});
    // 一条 edge 可以挂多条互斥守卫的规则 —— 必须全部可达，不能被后写覆盖
    var ruleByEdge = new Map();
    function pushRule(key, r) {
      var list = ruleByEdge.get(key);
      if (!list) { list = []; ruleByEdge.set(key, list); }
      list.push(r);
    }
    (model.rules || []).forEach(function (r) {
      // 规则通过 edgeId 或 (sourceNodeId→targetNodeId) 绑定
      if (r.edgeId) pushRule(r.edgeId, r);
      else pushRule(r.sourceNodeId + "->" + r.targetNodeId, r);
    });

    var smByNode = new Map();
    (model.stateMachines || []).forEach(function (sm) { smByNode.set(sm.nodeId || sm.id, sm); });

    var nodeById = new Map();
    (model.nodes || []).forEach(function (n) { nodeById.set(n.id, n); });

    var edgeById = new Map();
    (model.edges || []).forEach(function (e) { edgeById.set(e.id, e); });

    var outByNode = new Map();
    (model.edges || []).forEach(function (e) {
      if (!outByNode.has(e.sourceId)) outByNode.set(e.sourceId, []);
      outByNode.get(e.sourceId).push(e.id);
    });
    // 确定性：出边按 edge id 字典序
    outByNode.forEach(function (list) { list.sort(); });

    /** 一条 edge 上的全部候选规则（按注册顺序，确定性） */
    function rulesForEdge(edge) {
      var list = ruleByEdge.get(edge.id);
      if (!list) list = ruleByEdge.get(edge.sourceId + "->" + edge.targetId);
      return list || [];
    }

    /**
     * 规则的 `when` 是源节点的前置状态约束 —— 必须被强制，不能只作装饰。
     * 空 when 表示不限。
     */
    function whenSatisfied(rule, cur) {
      if (!rule || !rule.when) return true;
      if (!cur) return false;
      return cur.state === rule.when;
    }

    function initialStateOf(nodeId) {
      var sm = smByNode.get(nodeId);
      if (sm && sm.initial) return sm.initial;
      return "Idle";
    }

    function isTerminal(nodeId, state) {
      var sm = smByNode.get(nodeId);
      if (!sm || !sm.states) return false;
      return sm.states.indexOf(state) !== -1 && !!sm.terminalStates &&
        sm.terminalStates.indexOf(state) !== -1;
    }

    /**
     * 运行一个 Case。
     * @param {string} caseId
     * @param {object} [opts] { inputs, seed, intervention }
     * @returns {{events, snapshots, finalStates, seed, modelVersion, caseId, terminatedBy, counters}}
     */
    function run(caseId, opts) {
      opts = opts || {};
      var cdef = null;
      (model.cases || []).forEach(function (c) { if (c.id === caseId) cdef = c; });
      if (!cdef) throw new Error("unknown case: " + caseId);

      var seed = typeof opts.seed === "number" ? opts.seed : (cdef.seed || 1);
      var inputs = Object.assign({}, cdef.inputs || {}, opts.inputs || {});
      var states = {}; // nodeId -> { state, attempts }
      var counters = {};
      var usedEdgeIds = new Set(); // 每条边在同一 Case 内只触发一次（防止自激循环）
      var events = [];
      var snapshots = [];
      var seq = 0;
      var logicalTime = 0;
      var steps = 0;
      var iterations = 0;
      var startedAt = Date.now();
      var terminatedBy = "completed";

      function setState(nodeId, st) {
        states[nodeId] = { state: st, attempts: (states[nodeId] ? states[nodeId].attempts : 0) };
      }

      // 入口
      var entryId = cdef.entryNodeId || (cdef.entryNodes && cdef.entryNodes[0]);
      if (entryId && nodeById.has(entryId)) {
        setState(entryId, cdef.entryState || initialStateOf(entryId));
      }
      (cdef.entryNodes || []).forEach(function (nid) {
        if (!states[nid]) setState(nid, initialStateOf(nid));
      });

      var queue = (cdef.entryNodes || (entryId ? [entryId] : [])).slice();
      if (!queue.length && entryId) queue = [entryId];
      var queued = new Set(queue);

      var ctx = { states: states, inputs: inputs, counters: counters };

      function pushEvent(evt) {
        evt.sequence = seq++;
        evt.logicalTime = logicalTime;
        evt.caseId = caseId;
        events.push(evt);
        if (events.length > LIM.maxEvents) {
          throw BudgetExceeded("maxEvents exceeded (" + LIM.maxEvents + ")", "maxEvents");
        }
        return evt;
      }

      function snapshot(label) {
        snapshots.push({
          label: label,
          logicalTime: logicalTime,
          sequence: seq,
          states: JSON.parse(JSON.stringify(states)),
          eventCount: events.length
        });
      }

      snapshot("init");

      var guardBlocked = [];
      var maxHops = 0;

      while (queue.length) {
        steps++;
        iterations++;
        if (steps > LIM.maxSteps) {
          throw BudgetExceeded("maxSteps exceeded (" + LIM.maxSteps + ")", "maxSteps");
        }
        if (iterations > LIM.maxIterations) {
          throw BudgetExceeded("maxIterations exceeded (" + LIM.maxIterations + ")", "maxIterations");
        }
        if (Date.now() - startedAt >= LIM.maxMillis) {
          throw BudgetExceeded("maxMillis exceeded (" + LIM.maxMillis + ")", "maxMillis");
        }

        var nodeId = queue.shift();
        queued.delete(nodeId);
        var node = nodeById.get(nodeId);
        if (!node) continue;

        var cur = states[nodeId];
        if (!cur) { setState(nodeId, initialStateOf(nodeId)); cur = states[nodeId]; }

        // 已终止的节点不再扩散
        if (isTerminal(nodeId, cur.state)) continue;

        var edgeIds = outByNode.get(nodeId) || [];
        var advanced = false;

        for (var i = 0; i < edgeIds.length; i++) {
          var eid = edgeIds[i];
          var edge = edgeById.get(eid);
          if (!edge) continue;
          if (usedEdgeIds.has(eid)) continue; // 该边本 Case 已触发过
          var ruleList = rulesForEdge(edge);
          if (!ruleList.length) continue;
          var edgeFired = false;

          // ---- 边级干预（与具体规则无关，先算一次） ----
          var edgeGuardOverride = null;
          var edgeDisabled = false;
          if (opts.intervention) {
            var iv0 = opts.intervention;
            if (iv0.kind === "DisableEdge" && iv0.targetId === eid) edgeDisabled = true;
            if (iv0.kind === "ChangeGuard" && iv0.targetId === eid) edgeGuardOverride = iv0.params.guardAst;
          }

          // 一条边上可能有多条互斥守卫的规则：逐条求值，命中即止
          for (var ri = 0; ri < ruleList.length; ri++) {
          var rule = ruleList[ri];

          // ---- 规则级干预 ----
          if (opts.intervention) {
            var iv = opts.intervention;
            if (iv.kind === "ChangeTimeout" && iv.targetId === eid && iv.params.timeoutMs != null) {
              rule = Object.assign({}, rule, { timeoutMs: iv.params.timeoutMs });
            }
            if (iv.kind === "ChangeRetry" && iv.targetId === eid && iv.params.retry) {
              rule = Object.assign({}, rule, { retry: iv.params.retry });
            }
            if (iv.kind === "InjectResourceFailure" && iv.targetId === nodeId) {
              // 资源故障注入：该节点进入 Failed 并停止扩散（不伪造时间推进）
              setState(nodeId, "Failed");
              pushEvent({
                eventId: "EV-" + events.length,
                sourceNodeId: nodeId, targetNodeId: nodeId, edgeId: null,
                previousState: cur.state, nextState: "Failed",
                ruleId: iv.params && iv.params.ruleId ? iv.params.ruleId : "INTERVENTION-RESOURCE-FAILURE",
                evidenceIds: [],
                kind: "InjectedFailure"
              });
              snapshot("injected-failure:" + nodeId);
              advanced = true;
              break;
            }
            if (iv.kind === "BypassNode" && iv.targetId === nodeId) {
              // 直接跳过该节点，走它的出边
              var bypassTargets = (outByNode.get(nodeId) || []).map(function (x) {
                var ed = edgeById.get(x); return ed ? ed.targetId : null;
              }).filter(Boolean);
              bypassTargets.forEach(function (t) {
                if (!queued.has(t) && !states[t]) { queue.push(t); queued.add(t); }
              });
              advanced = true;
              break;
            }
          }

          // when 约束：源节点必须处于规则声明的前置状态（不是装饰字段）
          if (!whenSatisfied(rule, cur)) {
            guardBlocked.push({ edgeId: eid, nodeId: nodeId, guard: "when == " + rule.when });
            pushEvent({
              eventId: "EV-" + events.length,
              sourceNodeId: nodeId, targetNodeId: edge.targetId, edgeId: eid,
              previousState: cur.state, nextState: cur.state,
              ruleId: rule.id, evidenceIds: rule.evidenceIds || [],
              kind: "WhenBlocked", guard: "when == " + rule.when
            });
            continue;
          }

          // guard 求值
          var guardAst = edgeGuardOverride || edge.guardAst || rule.guardAst || { type: "always" };
          var disabled = edgeDisabled ||
            (opts.intervention && opts.intervention.kind === "DisableNode" &&
              (opts.intervention.targetId === nodeId || opts.intervention.targetId === edge.targetId));
          if (disabled) {
            pushEvent({
              eventId: "EV-" + events.length,
              sourceNodeId: nodeId, targetNodeId: edge.targetId, edgeId: eid,
              previousState: cur.state, nextState: cur.state,
              ruleId: rule.id, evidenceIds: rule.evidenceIds || [],
              kind: "InterventionBlocked"
            });
            continue;
          }

          var ok = evalGuard(guardAst, ctx);
          if (!ok) {
            guardBlocked.push({ edgeId: eid, nodeId: nodeId, guard: describeGuard(guardAst) });
            pushEvent({
              eventId: "EV-" + events.length,
              sourceNodeId: nodeId, targetNodeId: edge.targetId, edgeId: eid,
              previousState: cur.state, nextState: cur.state,
              ruleId: rule.id, evidenceIds: rule.evidenceIds || [],
              kind: "GuardBlocked", guard: describeGuard(guardAst)
            });
            continue;   // 试下一条规则
          }

          // 计数器（用于可重试/可计数的语义）
          if (rule.counterId) counters[rule.counterId] = (counters[rule.counterId] || 0) + 1;

          var duration = typeof rule.durationMs === "number" ? rule.durationMs : 10;
          logicalTime += duration;

          var targetState = rule.then || "Running";
          var prevState = cur.state;

          // Timeout 规则：目标节点未在时限内到达终态 ⇒ 进入 Failed
          if (typeof rule.timeoutMs === "number" && rule.timeoutMs > 0 && duration >= rule.timeoutMs) {
            targetState = "Failed";
          }

          pushEvent({
            eventId: "EV-" + events.length,
            sourceNodeId: nodeId, targetNodeId: edge.targetId, edgeId: eid,
            previousState: prevState, nextState: targetState,
            ruleId: rule.id, evidenceIds: rule.evidenceIds || [],
            kind: "Transition", guard: describeGuard(guardAst), logicalTime: logicalTime
          });

          setState(nodeId, targetState === "Failed" ? "Failed" : cur.state);
          setState(edge.targetId, targetState);
          snapshot("after:" + eid);
          usedEdgeIds.add(eid);

          // Retry：Failed 时按退避重试，受 max 限制
          if (targetState === "Failed" && rule.retry && rule.retry.max > 0) {
            var attempts = (states[nodeId].attempts || 0);
            if (attempts < rule.retry.max) {
              states[nodeId].attempts = attempts + 1;
              logicalTime += typeof rule.retry.backoffMs === "number" ? rule.retry.backoffMs : 0;
              if (!queued.has(nodeId)) { queue.push(nodeId); queued.add(nodeId); }
              pushEvent({
                eventId: "EV-" + events.length,
                sourceNodeId: nodeId, targetNodeId: edge.targetId, edgeId: eid,
                previousState: "Failed", nextState: "Running",
                ruleId: rule.id, evidenceIds: rule.evidenceIds || [],
                kind: "Retry", attempt: states[nodeId].attempts
              });
            }
          }

          advanced = true;

          // 继续扩散（目标非终态）
          var tstate = states[edge.targetId];
          if (tstate && !isTerminal(edge.targetId, tstate.state)) {
            if (!queued.has(edge.targetId)) { queue.push(edge.targetId); queued.add(edge.targetId); }
          }
          // 源节点也可能还有后续边：重新入队（每条边在同一 Case 内只触发一次）
          if (!queued.has(nodeId)) { queue.push(nodeId); queued.add(nodeId); }
          edgeFired = true;
          break; // 命中一条规则即止（一条边只走一次，确定性）
          }
          if (edgeFired) break; // 每个节点每轮只走一条成功的边
        }

        if (!advanced) {
          // 无可用出边：节点停在此状态
          if (!isTerminal(nodeId, states[nodeId] && states[nodeId].state)) {
            setState(nodeId, (states[nodeId] && states[nodeId].state) || cur.state);
          }
        }
      }

      // 收尾：把未终态节点标成 Completed 或 Unknown（无证据）
      Object.keys(states).forEach(function (nid) {
        if (!isTerminal(nid, states[nid].state)) {
          states[nid].state = "Completed";
        }
      });
      snapshot("final");

      return {
        caseId: caseId,
        seed: seed,
        modelVersion: model.version || "unversioned",
        events: events,
        snapshots: snapshots,
        finalStates: states,
        counters: counters,
        guardBlocked: guardBlocked,
        terminatedBy: terminatedBy,
        eventCount: events.length
      };
    }

    return {
      model: model,
      limits: LIM,
      run: run,
      evalGuard: evalGuard,
      describeGuard: describeGuard,
      isTerminal: isTerminal,
      cloneModel: function () { return clone(model); }
    };
  }

  var API = { createEngine: createEngine, evalGuard: evalGuard, describeGuard: describeGuard, DEFAULT_LIMITS: DEFAULT_LIMITS };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcEngine = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
