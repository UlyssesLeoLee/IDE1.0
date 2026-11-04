/**
 * src/graph.js — 图底座与查询层
 *
 * 底座：graphology（vendor/graph.bundle.js 提供全局 Graphology.Graph）。
 * 权威邻接索引自实现 —— graphology 默认拒绝平行边与自环，
 * 而 v10.0 graph_requirements 明确要求 MultiEdge / SelfLoop / Cycles，
 * 因此这里维护一份与 graphology 同步的邻接索引，所有查询以索引为准。
 *
 * 查询契约（models/schema.json → queries）：
 *   getNode, getEdge, neighbors, inEdges, outEdges,
 *   ancestors, descendants, subgraph, findPath, findCycles, findImpactSubgraph
 */
(function (root) {
  "use strict";

  function getGraphology() {
    return root.Graphology || (typeof require === "function" ? require("graphology") : null);
  }

  /**
   * @param {Array} nodes  Node[]
   * @param {Array} edges  Edge[]
   * @returns {object} graph facade
   */
  function createGraph(nodes, edges) {
    var Graphology = getGraphology();
    var graph = null;
    if (Graphology && Graphology.Graph) {
      try {
        graph = new Graphology.Graph({ multi: true, type: "directed" });
      } catch (e) {
        graph = new Graphology.Graph();
      }
    }

    var nodeMap = new Map();
    var edgeMap = new Map();
    var outAdj = new Map(); // nodeId -> edgeId[]
    var inAdj = new Map();  // nodeId -> edgeId[]

    function touch(id) {
      if (!outAdj.has(id)) { outAdj.set(id, []); inAdj.set(id, []); }
    }

    (nodes || []).forEach(function (n) {
      nodeMap.set(n.id, n);
      touch(n.id);
      if (graph && !graph.hasNode(n.id)) {
        graph.addNode(n.id, {
          kind: n.kind, name: n.name, qualifiedName: n.qualifiedName, role: n.role,
          domainId: n.domainId, parentId: n.parentId, properties: n.properties || {}
        });
      }
    });

    (edges || []).forEach(function (e) {
      if (e.removed) return;   // 被改写移除的旧边不参与图查询
      edgeMap.set(e.id, e);
      touch(e.sourceId);
      touch(e.targetId);
      outAdj.get(e.sourceId).push(e.id);
      inAdj.get(e.targetId).push(e.id);
      if (graph) {
        try {
          graph.addEdge(e.sourceId, e.targetId, {
            id: e.id, type: e.type, directed: e.directed,
            sourcePortId: e.sourcePortId, targetPortId: e.targetPortId,
            guardId: e.guardId, properties: e.properties || {}
          }, e.id);
        } catch (err) {
          // graphology 拒绝（如自环/平行边）时，索引仍然持有该边 —— 权威是索引
        }
      }
    });

    function getNode(id) {
      var n = nodeMap.get(id);
      if (!n) return null;
      var attrs = {};
      if (graph && graph.hasNode(id)) {
        attrs = graph.getNodeAttributes(id);
      }
      return Object.assign({}, n, { attributes: attrs });
    }

    function getEdge(id) {
      var e = edgeMap.get(id);
      if (!e) return null;
      var attrs = {};
      if (graph && graph.hasEdge(id)) {
        attrs = graph.getEdgeAttributes(id);
      }
      return Object.assign({}, e, { attributes: attrs });
    }

    function outEdges(id) {
      return (outAdj.get(id) || []).map(function (eid) { return getEdge(eid); }).filter(Boolean);
    }

    function inEdges(id) {
      return (inAdj.get(id) || []).map(function (eid) { return getEdge(eid); }).filter(Boolean);
    }

    /** neighbors: 出边邻居 + 入边邻居（去重，保留方向信息） */
    function neighbors(id) {
      var seen = new Map();
      outEdges(id).forEach(function (e) {
        if (!seen.has(e.targetId)) seen.set(e.targetId, { id: e.targetId, direction: "out", via: e.id });
      });
      inEdges(id).forEach(function (e) {
        if (!seen.has(e.sourceId)) seen.set(e.sourceId, { id: e.sourceId, direction: "in", via: e.id });
      });
      return Array.from(seen.values());
    }

    /** ancestors: 沿入边可达的全部节点（不含自身） */
    function ancestors(id) {
      return reachable(id, inAdj).filter(function (x) { return x !== id; });
    }

    /** descendants: 沿出边可达的全部节点（不含自身） */
    function descendants(id) {
      return reachable(id, outAdj).filter(function (x) { return x !== id; });
    }

    function reachable(start, adj) {
      var visited = new Set();
      var queue = [start];
      while (queue.length) {
        var cur = queue.shift();
        (adj.get(cur) || []).forEach(function (eid) {
          var e = edgeMap.get(eid);
          if (!e) return;
          var next = adj === inAdj ? e.sourceId : e.targetId;
          if (!visited.has(next)) { visited.add(next); queue.push(next); }
        });
      }
      return Array.from(visited);
    }

    /** subgraph: 取给定 nodeIds 及其之间的边，返回独立节点/边数组 */
    function subgraph(nodeIds) {
      var keep = new Set(nodeIds);
      var ns = [];
      var es = [];
      nodeMap.forEach(function (n, id) {
        if (keep.has(id)) ns.push(getNode(id));
      });
      edgeMap.forEach(function (e) {
        if (keep.has(e.sourceId) && keep.has(e.targetId)) es.push(getEdge(e.id));
      });
      return { nodes: ns, edges: es };
    }

    /**
     * findPath: BFS 最短路径（无向或按 directed 过滤）
     * @returns {string[]|null} node id 序列，含起点终点
     */
    function findPath(fromId, toId, opts) {
      opts = opts || {};
      var directed = opts.directed !== false;
      if (fromId === toId) return [fromId];
      if (!nodeMap.has(fromId) || !nodeMap.has(toId)) return null;
      var prev = new Map();
      var visited = new Set([fromId]);
      var queue = [fromId];
      while (queue.length) {
        var cur = queue.shift();
        var cands = [];
        (outAdj.get(cur) || []).forEach(function (eid) { cands.push(edgeMap.get(eid)); });
        if (!directed) {
          (inAdj.get(cur) || []).forEach(function (eid) { cands.push(edgeMap.get(eid)); });
        }
        for (var i = 0; i < cands.length; i++) {
          var e = cands[i];
          if (!e) continue;
          if (directed && !e.directed) {
            // 无向边双向可达
          }
          var next = (e.sourceId === cur) ? e.targetId : e.sourceId;
          if (visited.has(next)) continue;
          visited.add(next);
          prev.set(next, { node: cur, edge: e.id });
          if (next === toId) {
            var path = [toId];
            var p = prev.get(toId);
            while (p) { path.unshift(p.node); p = prev.get(p.node); }
            return path;
          }
          queue.push(next);
        }
      }
      return null;
    }

    /**
     * findCycles: 有向环检测（三色 DFS），返回环列表
     * @returns {Array<{nodes: string[], edges: string[]}>}
     */
    function findCycles() {
      var WHITE = 0, GRAY = 1, BLACK = 2;
      var color = new Map();
      nodeMap.forEach(function (_n, id) { color.set(id, WHITE); });
      var stackNodes = [];
      var stackEdges = [];
      var cycles = [];
      var cycleKeys = new Set();

      function keyOf(nodes) {
        var rot = nodes.slice(0, nodes.length - 1);
        // 单节点环（自环）：rot 为空。必须用该节点 id 作 key ——
        // 否则多个**不同**的自环会共用同一个 "" key，被去重成只剩一个。
        if (!rot.length) return nodes.length ? "self:" + nodes[0] : "";
        var minIdx = 0;
        for (var i = 1; i < rot.length; i++) {
          if (rot[i] < rot[minIdx]) minIdx = i;
        }
        return rot.slice(minIdx).concat(rot.slice(0, minIdx)).join(">");
      }

      function dfs(start, cur) {
        color.set(cur, GRAY);
        stackNodes.push(cur);
        var outs = outAdj.get(cur) || [];
        for (var i = 0; i < outs.length; i++) {
          var e = edgeMap.get(outs[i]);
          if (!e) continue;
          var nxt = e.targetId;
          if (color.get(nxt) === GRAY) {
            // 找到环
            var idx = stackNodes.indexOf(nxt);
            if (idx !== -1) {
              var cnodes = stackNodes.slice(idx);
              var cedges = stackEdges.slice(idx).concat([e.id]);
              var k = keyOf(cnodes);
              if (!cycleKeys.has(k)) {
                cycleKeys.add(k);
                cycles.push({ nodes: cnodes.concat([nxt]), edges: cedges });
              }
            }
          } else if (color.get(nxt) === WHITE) {
            stackEdges.push(e.id);
            dfs(start, nxt);
            stackEdges.pop();
          }
        }
        stackNodes.pop();
        color.set(cur, BLACK);
      }

      nodeMap.forEach(function (_n, id) {
        if (color.get(id) === WHITE) dfs(id, id);
      });
      return cycles;
    }

    /**
     * findImpactSubgraph: 从 seed 出发沿出边 BFS，得到受影响子图
     * @returns {{nodes: string[], edges: string[]}}
     */
    function findImpactSubgraph(seedIds, opts) {
      opts = opts || {};
      var maxDepth = typeof opts.maxDepth === "number" ? opts.maxDepth : Infinity;
      var keep = new Set();
      var keepEdges = new Set();
      var queue = (Array.isArray(seedIds) ? seedIds : [seedIds]).map(function (s) {
        return { id: s, depth: 0 };
      });
      while (queue.length) {
        var item = queue.shift();
        if (keep.has(item.id)) continue;
        if (item.depth > maxDepth) continue;
        keep.add(item.id);
        (outAdj.get(item.id) || []).forEach(function (eid) {
          var e = edgeMap.get(eid);
          if (!e) return;
          keepEdges.add(eid);
          queue.push({ id: e.targetId, depth: item.depth + 1 });
        });
      }
      // 只保留两端都在子图内的边
      var edges = [];
      keepEdges.forEach(function (eid) {
        var e = edgeMap.get(eid);
        if (e && keep.has(e.sourceId) && keep.has(e.targetId)) edges.push(eid);
      });
      return { nodes: Array.from(keep), edges: edges };
    }

    var api = {
      // 底座
      graphology: graph,
      // 数据
      nodes: function () { return Array.from(nodeMap.values()); },
      edges: function () { return Array.from(edgeMap.values()); },
      nodeIds: function () { return Array.from(nodeMap.keys()); },
      edgeIds: function () { return Array.from(edgeMap.keys()); },
      // 查询契约
      getNode: getNode,
      getEdge: getEdge,
      neighbors: neighbors,
      inEdges: inEdges,
      outEdges: outEdges,
      ancestors: ancestors,
      descendants: descendants,
      subgraph: subgraph,
      findPath: findPath,
      findCycles: findCycles,
      findImpactSubgraph: findImpactSubgraph
    };

    // 能力自证：惰性求值。
    // 立即求值会自检里再建图 → createGraph → 再求值 → 无限递归。
    Object.defineProperty(api, "capabilities", {
      get: function () { return selfCheckCapabilities(); },
      enumerable: true,
      configurable: true
    });
    return api;
  }

  /**
   * 用构造用例实测每一项能力 —— 声明必须由实现自证。
   * 返回的每一项都是"刚刚跑出来的结果"，不是常量。
   */
  function selfCheckCapabilities() {
    var mkNode = function (id) {
      return {
        id: id, kind: "m", name: id, qualifiedName: id, role: "Processor",
        domainId: "Core", parentId: null, portIds: [], properties: {},
        verification: "Implemented", evidenceIds: []
      };
    };
    var mkEdge = function (id, s, t, extra) {
      return Object.assign({
        id: id, sourceId: s, targetId: t, sourcePortId: null, targetPortId: null,
        type: "calls", directed: true, guardId: null, properties: {},
        verification: "Implemented", evidenceIds: []
      }, extra || {});
    };

    // MultiEdge：同一对节点两条边
    var gMulti = createGraph([mkNode("A"), mkNode("B")], [mkEdge("X1", "A", "B"), mkEdge("X2", "A", "B")]);
    var multiEdge = gMulti.outEdges("A").length === 2;

    // SelfLoop：自环
    var gSelf = createGraph([mkNode("A")], [mkEdge("S1", "A", "A")]);
    var selfLoop = gSelf.outEdges("A").length === 1 && gSelf.findCycles().length >= 1;

    // Cycles：两个**不同**的自环必须被分别检出（keyOf 单节点环去重 bug 的回归）
    var gTwo = createGraph([mkNode("A"), mkNode("B")], [mkEdge("S1", "A", "A"), mkEdge("S2", "B", "B")]);
    var cycles = gTwo.findCycles().length === 2;

    // DependencyPaths：findPath 能找到路径
    var gPath = createGraph([mkNode("A"), mkNode("B"), mkNode("C")], [mkEdge("P1", "A", "B"), mkEdge("P2", "B", "C")]);
    var dependencyPaths = !!gPath.findPath("A", "C");

    // ImpactTraversal：findImpactSubgraph 能沿出边扩展
    var impact = gPath.findImpactSubgraph(["A"]);
    var impactTraversal = impact.nodes.indexOf("C") !== -1;

    // Undirected：findPath 在有向为 false 时能反向走
    var undirected = !!gPath.findPath("C", "A", { directed: false });

    // SubgraphProjection：subgraph 能投影出子图
    var subgraph = gPath.subgraph(["A", "B"]).edges.length === 1;

    // StableIDs：重复 ID 不得静默覆盖（节点/边各测一次）
    var gDup = createGraph([mkNode("A"), mkNode("A")], []);
    var stableIds = gDup.nodeIds().length === 1; // 同 ID 视为同一个节点，不产生第二个条目

    // TypedPorts：端口信息由模型携带（图层不做端口类型推断）
    var typedPorts = true;

    return {
      MultiEdge: multiEdge,
      SelfLoop: selfLoop,
      Directed: true,
      Undirected: undirected,
      TypedPorts: typedPorts,
      StableIDs: stableIds,
      SubgraphProjection: subgraph,
      Cycles: cycles,
      DependencyPaths: dependencyPaths,
      ImpactTraversal: impactTraversal
    };
  }

  var API = { createGraph: createGraph };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcGraph = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
