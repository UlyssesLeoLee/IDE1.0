/**
 * src/causal.js — 5D 因果实验引擎
 *
 * 契约（v10.0 immutable_dimensions.5D / simulation_and_causality）：
 *   - 必须复用 4D 引擎与 3D 场景；因果结论来自"真实重算"，不是预制光轨。
 *   - 正式事实图不可变：所有新增/屏蔽/改变都落在 ScenarioGraphDelta。
 *   - 插入节点必须真正把 A→B 改写成 A→X→B，并明确旧 Edge 的去留。
 *   - 无效果 ⇒ NO_DIFFERENCE；没有执行规则 ⇒ UNKNOWN；不伪造分支。
 *   - 仅模型内因果，不宣称经验验证因果。
 */
(function (root) {
  "use strict";

  function clone(o) { return JSON.parse(JSON.stringify(o)); }

  // ---------------------------------------------------- 干预 → 模型改写

  /**
   * 把干预施加到模型副本上，返回 { model, delta }。
   * 绝不修改传入的 model（原对象）。
   */
  function applyIntervention(model, intervention) {
    var m = clone(model);
    var delta = {
      id: "SGD-" + (intervention.id || "auto"),
      baseScenarioId: intervention.caseId || null,
      operations: [],
      mutates: false
    };
    if (!intervention) return { model: m, delta: delta };

    var kind = intervention.kind;
    var targetId = intervention.targetId;
    var params = intervention.params || {};

    if (kind === "DisableNode") {
      // 引擎侧处理（不改模型结构），但记录 delta
      delta.operations.push({ op: "DisableNode", targetId: targetId });
    } else if (kind === "DisableEdge") {
      delta.operations.push({ op: "DisableEdge", targetId: targetId });
    } else if (kind === "ChangeGuard") {
      m.edges = m.edges.map(function (e) {
        if (e.id !== targetId) return e;
        var copy = Object.assign({}, e, { guardAst: params.guardAst });
        return copy;
      });
      delta.operations.push({ op: "ChangeGuard", targetId: targetId, guardAst: params.guardAst });
    } else if (kind === "ChangeTimeout") {
      m.rules = m.rules.map(function (r) {
        if (r.edgeId === targetId || (r.sourceNodeId + "->" + r.targetNodeId) === targetId) {
          return Object.assign({}, r, { timeoutMs: params.timeoutMs });
        }
        return r;
      });
      delta.operations.push({ op: "ChangeTimeout", targetId: targetId, timeoutMs: params.timeoutMs });
    } else if (kind === "ChangeRetry") {
      m.rules = m.rules.map(function (r) {
        if (r.edgeId === targetId || (r.sourceNodeId + "->" + r.targetNodeId) === targetId) {
          return Object.assign({}, r, { retry: params.retry });
        }
        return r;
      });
      delta.operations.push({ op: "ChangeRetry", targetId: targetId, retry: params.retry });
    } else if (kind === "BypassNode") {
      delta.operations.push({ op: "BypassNode", targetId: targetId });
    } else if (kind === "InjectResourceFailure") {
      delta.operations.push({ op: "InjectResourceFailure", targetId: targetId });
    } else if (kind === "InsertElement") {
      // 真正的 Edge Rewrite：A→B 改为 A→X→B，旧 Edge 标记 removed
      var newNode = {
        id: params.nodeId || "X-" + targetId,
        kind: params.kind || "Processor",
        name: params.name || "Injected Element",
        qualifiedName: params.name || "Injected Element",
        role: params.role || "Processor",
        domainId: params.domainId || "Core",
        parentId: params.parentId || null,
        portIds: [],
        properties: { injected: true, template: params.template || null },
        verification: "Designed",
        evidenceIds: []
      };
      var inEdge = null, outEdge = null;
      m.edges.forEach(function (e) {
        if (e.id === targetId) {
          if (!inEdge && m.nodes.some(function (n) { return n.id === e.sourceId; })) inEdge = e;
          if (!outEdge && m.nodes.some(function (n) { return n.id === e.targetId; })) outEdge = e;
        }
      });
      // targetId 可以是 edgeId，也可以是 "A->B" 形式
      if (!inEdge && !outEdge && typeof targetId === "string" && targetId.indexOf("->") !== -1) {
        var parts = targetId.split("->");
        inEdge = m.edges.find(function (e) { return e.sourceId === parts[0] && e.targetId === parts[1]; });
      }
      if (!inEdge || !outEdge) {
        return { model: m, delta: delta, error: "target edge not found: " + targetId };
      }
      var nodeId = newNode.id;
      var rewriteEdge = function (e, newTarget, suffix) {
        return Object.assign({}, e, {
          id: e.id + suffix,
          targetId: newTarget,
          sourcePortId: null,
          targetPortId: null,
          properties: Object.assign({}, e.properties || {}, { rewrittenFrom: e.id })
        });
      };
      var newEdges = [];
      // 找到真正被重写的 A→B
      var ab = m.edges.find(function (e) {
        return e.sourceId === inEdge.sourceId && e.targetId === outEdge.targetId && !e.removed;
      });
      if (!ab) ab = outEdge;
      newEdges.push(rewriteEdge(ab, nodeId, "-to-X"));
      newEdges.push(rewriteEdge(ab, outEdge.targetId, "-X-to"));
      newEdges[0].id = ab.id + "->X";
      newEdges[1].id = ab.id + "-X->";
      newEdges[1].sourceId = nodeId;
      newEdges[1].sourcePortId = null;
      // 旧边必须**真正移除** —— A→B 要变成 A→X→B，而不是新旧并联。
      // 只打 removed 标记而留在边集里，会让引擎与图查询照常触发旧边（= 改写没发生）。
      // 旧边的去留信息保留在 ScenarioGraphDelta.operations.removedEdgeIds（可查）。
      m.edges = m.edges.filter(function (e) { return e.id !== ab.id; });
      m.edges = m.edges.concat(newEdges);
      m.nodes.push(newNode);
      // 为新边补规则
      // baseRule 必须选该边的**正常路径**规则 —— 数组顺序是优先级，
      // 第一条往往是"拒绝类"规则（如路径穿越拒绝）。若直接继承它，
      // 新插入的节点就会继承穿越守卫，正常写入路径反而被挡住。
      var edgeRules = (m.rules || []).filter(function (r) {
        return (r.edgeId === ab.id) || (r.sourceNodeId === ab.sourceId && r.targetNodeId === ab.targetId);
      });
      var baseRule = edgeRules.filter(function (r) { return r.then !== "Failed"; })[0] || edgeRules[0];
      if (baseRule) {
        m.rules.push(Object.assign({}, baseRule, {
          id: baseRule.id + "-to-X", edgeId: ab.id + "->X", sourceNodeId: ab.sourceId, targetNodeId: nodeId
        }));
        m.rules.push(Object.assign({}, baseRule, {
          id: baseRule.id + "-X-to", edgeId: ab.id + "-X->", sourceNodeId: nodeId, targetNodeId: outEdge.targetId
        }));
      }
      delta.operations.push({
        op: "InsertElement", nodeId: nodeId,
        rewroteEdgeId: ab.id,
        removedEdgeIds: [ab.id],
        newEdgeIds: [ab.id + "->X", ab.id + "-X->"]
      });
      delta.mutates = true;
    } else if (kind === "ReplaceRoute") {
      var oldTarget = params.fromTargetId;
      m.edges = m.edges.map(function (e) {
        if (e.id === targetId) {
          return Object.assign({}, e, {
            targetId: params.toTargetId,
            properties: Object.assign({}, e.properties || {}, { rewrittenFrom: e.id })
          });
        }
        return e;
      });
      delta.operations.push({ op: "ReplaceRoute", targetId: targetId, toTargetId: params.toTargetId, fromTargetId: oldTarget });
      delta.mutates = true;
    }

    return { model: m, delta: delta };
  }

  // ---------------------------------------------------- 轨迹比较

  /** 用事件序列的 (kind, sourceNodeId, targetNodeId, nextState) 对齐，找首次分歧 */
  function findFirstDivergence(a, b) {
    var n = Math.min(a.events.length, b.events.length);
    for (var i = 0; i < n; i++) {
      var ea = a.events[i], eb = b.events[i];
      var ka = ea.kind + "|" + ea.sourceNodeId + "|" + ea.targetNodeId + "|" + ea.nextState;
      var kb = eb.kind + "|" + eb.sourceNodeId + "|" + eb.targetNodeId + "|" + eb.nextState;
      if (ka !== kb) {
        return {
          atSequence: i,
          branchA: summarizeEvent(ea),
          branchB: summarizeEvent(eb),
          reason: "event signature diverged at sequence " + i
        };
      }
    }
    if (a.events.length !== b.events.length) {
      var idx = n;
      return {
        atSequence: idx,
        branchA: a.events[idx] ? summarizeEvent(a.events[idx]) : null,
        branchB: b.events[idx] ? summarizeEvent(b.events[idx]) : null,
        reason: "event count diverged (A=" + a.events.length + ", B=" + b.events.length + ")"
      };
    }
    return null;
  }

  function summarizeEvent(e) {
    if (!e) return null;
    return {
      sequence: e.sequence, logicalTime: e.logicalTime, kind: e.kind,
      sourceNodeId: e.sourceNodeId, targetNodeId: e.targetNodeId, edgeId: e.edgeId,
      previousState: e.previousState, nextState: e.nextState, ruleId: e.ruleId
    };
  }

  /**
   * 比较两个运行的终态，得出受影响节点。
   * affected     = 全部状态不同的节点
   * terminalDiff = **双方都处于终态**且状态不同的节点（瞬时差异不进这里）
   * @param {function} [isTerminalFn] (nodeId, state) => boolean
   */
  function diffStates(a, b, isTerminalFn) {
    var ids = new Set(Object.keys(a.finalStates).concat(Object.keys(b.finalStates)));
    var affected = [];
    var terminalDiff = [];
    ids.forEach(function (id) {
      var sa = a.finalStates[id] ? a.finalStates[id].state : null;
      var sb = b.finalStates[id] ? b.finalStates[id].state : null;
      if (sa !== sb) {
        affected.push({ nodeId: id, branchA: sa, branchB: sb });
        var ta = isTerminalFn ? !!isTerminalFn(id, sa) : false;
        var tb = isTerminalFn ? !!isTerminalFn(id, sb) : false;
        // 只有"双方都停在终态"才是真正的终态差异；瞬时差异只进 affected
        if (ta && tb) terminalDiff.push({ nodeId: id, branchA: sa, branchB: sb });
      }
    });
    return { affected: affected, terminalDiff: terminalDiff };
  }

  /**
   * 比较 baseline（A）与干预后（B），产出 CausalExplanation。
   * @param {object} engine  GemArcEngine.createEngine(...) 返回值
   * @param {string} caseId
   * @param {object} intervention
   * @param {object} [opts] { inputs, seed }
   */
  function compare(engine, caseId, intervention, opts) {
    opts = opts || {};
    var seed = typeof opts.seed === "number" ? opts.seed : 1;
    var inputs = opts.inputs || {};

    // 干预目标必须真实存在 —— 否则「无效果」会被误报成 NO_DIFFERENCE，
    // 而真相是 UNKNOWN（根本没干预到任何东西）
    if (intervention && intervention.targetId) {
      var model = engine.model;
      var nodeExists = (model.nodes || []).some(function (n) { return n.id === intervention.targetId; });
      var edgeExists = (model.edges || []).some(function (e) { return e.id === intervention.targetId; });
      var ruleTargets = (model.rules || []).some(function (r) {
        return r.id === intervention.targetId ||
          r.edgeId === intervention.targetId ||
          r.sourceNodeId === intervention.targetId ||
          r.targetNodeId === intervention.targetId;
      });
      if (!nodeExists && !edgeExists && !ruleTargets) {
        return buildUnknown(
          engine.run(caseId, { seed: seed, inputs: inputs }),
          intervention,
          "intervention target does not exist in the fact graph: " + intervention.targetId
        );
      }
    }

    // A：baseline
    var runA = engine.run(caseId, { seed: seed, inputs: inputs });

    // B：干预后（真实重算）
    var applied = applyIntervention(engine.model, intervention);
    if (applied.error) {
      return buildUnknown(runA, intervention, applied.error);
    }
    var EngineFactory = root.GemArcEngine;
    var engineB = EngineFactory.createEngine(applied.model, engine.limits);
    var runB;
    try {
      runB = engineB.run(caseId, { seed: seed, inputs: inputs, intervention: intervention });
    } catch (err) {
      return buildUnknown(runA, intervention, "intervention run failed: " + err.message);
    }

    var divergence = findFirstDivergence(runA, runB);
    var stateDiff = diffStates(runA, runB, engine.isTerminal);

    // sharedPath：共同前缀
    var shared = [];
    if (divergence) {
      for (var i = 0; i < divergence.atSequence; i++) shared.push(summarizeEvent(runA.events[i]));
    } else {
      runA.events.forEach(function (e) { shared.push(summarizeEvent(e)); });
    }

    // 受影响子图：分歧点 + 终态差异节点
    var affectedNodeIds = new Set();
    stateDiff.affected.forEach(function (x) { affectedNodeIds.add(x.nodeId); });
    if (divergence) {
      if (divergence.branchA) {
        if (divergence.branchA.sourceNodeId) affectedNodeIds.add(divergence.branchA.sourceNodeId);
        if (divergence.branchA.targetNodeId) affectedNodeIds.add(divergence.branchA.targetNodeId);
      }
      if (divergence.branchB) {
        if (divergence.branchB.sourceNodeId) affectedNodeIds.add(divergence.branchB.sourceNodeId);
        if (divergence.branchB.targetNodeId) affectedNodeIds.add(divergence.branchB.targetNodeId);
      }
    }

    // propagationPath：从分歧节点沿 B 分支的实际事件链
    var propagationPath = [];
    if (divergence) {
      for (var j = divergence.atSequence; j < runB.events.length; j++) {
        var e = runB.events[j];
        if (e.sourceNodeId && affectedNodeIds.has(e.sourceNodeId)) {
          propagationPath.push(e.sourceNodeId);
        }
      }
    }

    // 阻断 / 反馈 / 恢复 / 补偿
    // blocking 必须看 **A/B 差集** —— 基线本身就常带 GuardBlocked（鉴权拒绝等），
    // 直接看 B 里有没有会被基线污染，任何干预都会显示"阻断"。
    var blockedA = runA.events.filter(function (e) { return e.kind === "GuardBlocked"; }).length;
    var blockedB = runB.events.filter(function (e) { return e.kind === "GuardBlocked"; }).length;
    var blocking = blockedB > blockedA ? "guard-blocked+" + (blockedB - blockedA) : null;
    var feedback = (runB.events.filter(function (e) { return e.kind === "Retry"; }).length >
                   runA.events.filter(function (e) { return e.kind === "Retry"; }).length) ? "retry-loop" : null;
    var recovery = runB.events.some(function (e) { return e.nextState === "Recovered"; }) ? "recovered" : null;
    var compensation = runB.events.some(function (e) { return e.kind === "Compensation"; }) ? "compensated" : null;

    // 收敛：终态一致 ⇒ 收敛（但保留历史差异）
    var converged = stateDiff.terminalDiff.length === 0 && !!divergence;
    var outcome;
    if (!divergence && stateDiff.terminalDiff.length === 0) {
      outcome = "NO_DIFFERENCE";
    } else if (divergence || stateDiff.terminalDiff.length) {
      outcome = "DIVERGED";
    } else {
      outcome = "UNKNOWN";
    }

    var affectedEdges = [];
    runB.events.forEach(function (e) {
      if (e.edgeId && affectedNodeIds.has(e.sourceNodeId)) affectedEdges.push(e.edgeId);
    });

    var ruleIds = Array.from(new Set(runB.events.map(function (e) { return e.ruleId; }).filter(Boolean)));

    return {
      id: "CE-" + caseId + "-" + (intervention.id || intervention.kind),
      outcome: outcome,
      baselineConditions: {
        caseId: caseId, seed: seed, inputs: inputs,
        modelVersion: runA.modelVersion,
        eventCount: runA.eventCount
      },
      intervention: {
        kind: intervention.kind, targetId: intervention.targetId,
        params: intervention.params || {},
        delta: applied.delta
      },
      ruleIds: ruleIds,
      sharedPath: shared,
      firstDivergence: divergence,
      branchA: runA.events.slice(divergence ? divergence.atSequence : runA.events.length).map(summarizeEvent),
      branchB: runB.events.slice(divergence ? divergence.atSequence : runB.events.length).map(summarizeEvent),
      affectedNodes: Array.from(affectedNodeIds),
      affectedEdges: Array.from(new Set(affectedEdges)),
      propagationPath: propagationPath,
      blocking: blocking,
      feedback: feedback,
      recovery: recovery,
      compensation: compensation,
      converged: converged,
      terminalDiff: stateDiff.terminalDiff,
      assumptions: [
        "因果结论仅来自本模型内的规则重算，非经验验证的因果",
        "未执行的规则不产生结论（UNKNOWN 而非猜测）"
      ],
      evidenceIds: (applied.delta.operations || []).reduce(function (acc, op) {
        return acc.concat(op.evidenceIds || []);
      }, []),
      runA: runA,
      runB: runB
    };
  }

  function buildUnknown(runA, intervention, reason) {
    return {
      id: "CE-UNKNOWN",
      outcome: "UNKNOWN",
      baselineConditions: { caseId: runA.caseId, seed: runA.seed, modelVersion: runA.modelVersion },
      intervention: { kind: intervention ? intervention.kind : null, targetId: intervention ? intervention.targetId : null, params: {} },
      ruleIds: [],
      sharedPath: [],
      firstDivergence: null,
      branchA: [], branchB: [],
      affectedNodes: [], affectedEdges: [],
      propagationPath: [],
      blocking: null, feedback: null, recovery: null, compensation: null,
      converged: false,
      terminalDiff: [],
      assumptions: [],
      evidenceIds: [],
      unknownReason: reason,
      runA: runA,
      runB: null
    };
  }

  var API = {
    applyIntervention: applyIntervention,
    compare: compare,
    findFirstDivergence: findFirstDivergence,
    diffStates: diffStates
  };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcCausal = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
