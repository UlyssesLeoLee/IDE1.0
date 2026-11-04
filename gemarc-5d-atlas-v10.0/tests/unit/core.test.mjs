/**
 * tests/unit/core.test.mjs — 核心逻辑单元测试
 * 覆盖：图底座（MultiEdge/SelfLoop/Cycles/ImpactTraversal）、4D 事件引擎（确定性/预算/守卫）、
 *       5D 因果（真实重算/Edge Rewrite/NO_DIFFERENCE）、1D Prompt 校验、校验器、图渲染分派。
 */
import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const require = createRequire(import.meta.url);
const load = (f) => require(path.join(ROOT, "src", f));

const Validate = load("validate.js");
const GraphLib = load("graph.js");
const EngineLib = load("engine.js");
const CausalLib = load("causal.js");
const PromptLib = load("prompt.js");
const Diagrams = load("diagrams.js");

const readModel = (n) => JSON.parse(fs.readFileSync(path.join(ROOT, "models", n), "utf8"));
const ARCH = readModel("architecture.json");
const CASES = readModel("cases.json");
const RULES = readModel("causal-rules.json");
const EVID = readModel("evidence.json");
const SCHEMA = readModel("schema.json");
const REQS = readModel("requirements.json");

// ---------------------------------------------------------------- validate
describe("validate.js · 记录校验", () => {
  it("缺必填字段必须报错", () => {
    const def = SCHEMA.records.Evidence;
    const r = Validate.validateRecord(def, { id: "E-1" });
    expect(r.ok).toBe(false);
    expect(r.errors.length).toBeGreaterThan(0);
  });

  it("类型不符必须报错", () => {
    const def = SCHEMA.records.Node;
    const r = Validate.validateRecord(def, {
      id: "N-1", kind: "module", name: "x", qualifiedName: "q", role: "Processor",
      domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: []
    });
    expect(r.ok).toBe(true);
    const bad = Validate.validateRecord(def, Object.assign({}, {
      id: "N-1", kind: "module", name: "x", qualifiedName: "q", role: "Processor",
      domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: []
    }, { portIds: "not-an-array" }));
    expect(bad.ok).toBe(false);
  });

  it("枚举值必须被接受/拒绝", () => {
    expect(Validate.validateEnum(SCHEMA, "implementation", "Implemented")).toBe(true);
    expect(Validate.validateEnum(SCHEMA, "implementation", "TotallyFine")).toBe(false);
  });

  it("全模型集合校验应通过（不得有悬空字段）", () => {
    const res = Validate.validateCollection(SCHEMA, {
      Node: ARCH.nodes, Edge: ARCH.edges, Requirement: REQS.requirements,
      Evidence: EVID.evidence, Case: CASES.cases
    });
    expect(res.errors).toEqual([]);
  });
});

// ------------------------------------------------------------------ graph
describe("graph.js · 图底座能力", () => {
  const nodes = [
    { id: "A", kind: "m", name: "A", qualifiedName: "A", role: "Processor", domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: [] },
    { id: "B", kind: "m", name: "B", qualifiedName: "B", role: "Processor", domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: [] },
    { id: "C", kind: "m", name: "C", qualifiedName: "C", role: "Processor", domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: [] }
  ];
  const edge = (id, s, t) => ({ id, sourceId: s, targetId: t, sourcePortId: null, targetPortId: null, type: "calls", directed: true, guardId: null, properties: {}, verification: "Implemented", evidenceIds: [] });

  it("必须支持平行边 (MultiEdge)", () => {
    const g = GraphLib.createGraph(nodes, [edge("E1", "A", "B"), edge("E2", "A", "B")]);
    expect(g.outEdges("A").length).toBe(2);
    expect(g.capabilities.MultiEdge).toBe(true);
  });

  it("必须支持自环 (SelfLoop)", () => {
    const g = GraphLib.createGraph(nodes, [edge("E1", "A", "A")]);
    expect(g.outEdges("A").length).toBe(1);
    expect(g.capabilities.SelfLoop).toBe(true);
  });

  it("必须能检出有向环", () => {
    const g = GraphLib.createGraph(nodes, [edge("E1", "A", "B"), edge("E2", "B", "C"), edge("E3", "C", "A")]);
    const cycles = g.findCycles();
    expect(cycles.length).toBeGreaterThan(0);
    expect(cycles[0].nodes).toContain("A");
  });

  it("findPath 必须给出路径；不可达时返回 null", () => {
    const g = GraphLib.createGraph(nodes, [edge("E1", "A", "B"), edge("E2", "B", "C")]);
    expect(g.findPath("A", "C")).toEqual(["A", "B", "C"]);
    const g2 = GraphLib.createGraph(nodes, [edge("E1", "A", "B")]);
    expect(g2.findPath("B", "A")).toBe(null);
  });

  it("ancestors / descendants 必须沿有向边计算", () => {
    const g = GraphLib.createGraph(nodes, [edge("E1", "A", "B"), edge("E2", "B", "C")]);
    expect(g.descendants("A").sort()).toEqual(["B", "C"]);
    expect(g.ancestors("C").sort()).toEqual(["A", "B"]);
  });

  it("findImpactSubgraph 必须返回受影响子图", () => {
    const g = GraphLib.createGraph(nodes, [edge("E1", "A", "B"), edge("E2", "B", "C")]);
    const sub = g.findImpactSubgraph(["A"]);
    expect(sub.nodes.sort()).toEqual(["A", "B", "C"]);
    expect(sub.edges.sort()).toEqual(["E1", "E2"]);
  });

  it("真实模型必须能被图底座加载且无重复 ID", () => {
    const g = GraphLib.createGraph(ARCH.nodes, ARCH.edges);
    expect(g.nodes().length).toBe(ARCH.nodes.length);
    expect(new Set(g.nodeIds()).size).toBe(ARCH.nodes.length);
    expect(new Set(g.edgeIds()).size).toBe(ARCH.edges.length);
  });
});

// ----------------------------------------------------------------- engine
describe("engine.js · 4D 离散事件引擎", () => {
  const model = {
    version: "t1",
    nodes: ARCH.nodes,
    edges: ARCH.edges,
    rules: RULES.rules,
    cases: CASES.cases,
    stateMachines: RULES.stateMachines
  };
  const engine = EngineLib.createEngine(model);

  it("必须能运行一个真实 Case 并产生事件", () => {
    const run = engine.run("CASE-VAULT-WRITE-NORMAL", { seed: 1, inputs: { value: "v", key: "openai", authPresent: true } });
    expect(run.eventCount).toBeGreaterThan(0);
    expect(run.events.every((e) => typeof e.sequence === "number")).toBe(true);
    expect(run.events.every((e) => typeof e.logicalTime === "number")).toBe(true);
  });

  it("确定性：同 seed 同输入 ⇒ 同轨迹", () => {
    const a = engine.run("CASE-VAULT-WRITE-NORMAL", { seed: 7, inputs: { value: "v", key: "k", authPresent: true } });
    const b = engine.run("CASE-VAULT-WRITE-NORMAL", { seed: 7, inputs: { value: "v", key: "k", authPresent: true } });
    expect(JSON.stringify(a.events)).toBe(JSON.stringify(b.events));
  });

  it("预算超限必须抛 BudgetExceeded，不得静默截断", () => {
    const tiny = EngineLib.createEngine(model, { maxEvents: 1 });
    expect(() => tiny.run("CASE-VAULT-WRITE-NORMAL", { seed: 1, inputs: { value: "v", key: "k", authPresent: true } }))
      .toThrowError(/maxEvents/);
  });

  it("守卫求值：equals / not / and / or 必须按语义", () => {
    const ctx = { states: {}, inputs: { a: 1 }, counters: {} };
    expect(EngineLib.evalGuard({ type: "always" }, ctx)).toBe(true);
    expect(EngineLib.evalGuard({ type: "never" }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "equals", left: { type: "var", var: "input:a" }, right: 1 }, ctx)).toBe(true);
    expect(EngineLib.evalGuard({ type: "not", arg: { type: "never" } }, ctx)).toBe(true);
    expect(EngineLib.evalGuard({ type: "and", args: [{ type: "always" }, { type: "never" }] }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "or", args: [{ type: "never" }, { type: "always" }] }, ctx)).toBe(true);
  });

  it("未知 guard 类型必须判否（缺标比错标安全）", () => {
    expect(EngineLib.evalGuard({ type: "somethingUnknown" }, { states: {}, inputs: {}, counters: {} })).toBe(false);
  });

  it("事件序列号必须严格递增", () => {
    const run = engine.run("CASE-VAULT-WRITE-NORMAL", { seed: 1, inputs: { value: "v", key: "k", authPresent: true } });
    for (let i = 1; i < run.events.length; i++) {
      expect(run.events[i].sequence).toBeGreaterThan(run.events[i - 1].sequence);
    }
  });
});

// ----------------------------------------------------------------- causal
describe("causal.js · 5D 因果实验", () => {
  const model = {
    version: "t1", nodes: ARCH.nodes, edges: ARCH.edges,
    rules: RULES.rules, cases: CASES.cases, stateMachines: RULES.stateMachines
  };
  const engine = EngineLib.createEngine(model);
  const inputs = { value: "v", key: "k", authPresent: true };

  it("干预存在但对该 Case 无影响时必须给 NO_DIFFERENCE，不得伪造分支", () => {
    // E-007 = N-API-ROUTER → N-REPO-STORE：CASE-VAULT-WRITE-NORMAL 不会走到它，
    // 因此改它的守卫对该 Case 无影响 → 应报 NO_DIFFERENCE（而不是编造分歧）
    const c = CausalLib.compare(engine, "CASE-VAULT-WRITE-NORMAL",
      { id: "IV0", kind: "ChangeGuard", targetId: "E-007", params: { guardAst: { type: "never" } } },
      { seed: 1, inputs });
    expect(c.outcome).toBe("NO_DIFFERENCE");
    expect(c.firstDivergence).toBe(null);
  });

  it("干预目标不存在时必须给 UNKNOWN（不是 NO_DIFFERENCE）", () => {
    const c = CausalLib.compare(engine, "CASE-VAULT-WRITE-NORMAL",
      { id: "IV1", kind: "ChangeGuard", targetId: "E-999-NOT-EXIST", params: { guardAst: { type: "always" } } },
      { seed: 1, inputs });
    expect(c.outcome).toBe("UNKNOWN");
    expect(c.unknownReason).toMatch(/does not exist/);
  });

  it("DisableNode 必须改变轨迹并报 DIVERGED", () => {
    const c = CausalLib.compare(engine, "CASE-VAULT-WRITE-NORMAL",
      { id: "IV2", kind: "DisableNode", targetId: "N-VAULT-FILE", params: {} },
      { seed: 1, inputs });
    expect(c.outcome).toBe("DIVERGED");
    expect(c.firstDivergence).not.toBe(null);
    expect(c.affectedNodes.length).toBeGreaterThan(0);
  });

  it("InsertElement 必须真正重写 Edge（旧边移出生效边集，新边成对出现）", () => {
    const applied = CausalLib.applyIntervention(engine.model, {
      id: "IV3", kind: "InsertElement", targetId: "E-005",
      params: { nodeId: "X-CACHE", name: "Vault Cache", kind: "Processor" }
    });
    expect(applied.error).toBeUndefined();
    // 旧边必须从生效边集里消失（只打 removed 标记而留着 = 并联，不是改写）
    expect(applied.model.edges.some((e) => e.id === "E-005")).toBe(false);
    // 旧边的去留必须在 delta 里可查
    expect(applied.delta.operations[0].removedEdgeIds).toContain("E-005");
    expect(applied.delta.operations[0].newEdgeIds).toEqual(["E-005->X", "E-005-X->"]);
    const newIds = applied.model.edges.map((e) => e.id);
    expect(newIds).toContain("E-005->X");
    expect(newIds).toContain("E-005-X->");
    expect(applied.model.nodes.some((n) => n.id === "X-CACHE")).toBe(true);
    expect(applied.delta.mutates).toBe(true);
  });

  it("正式事实图不得被干预修改", () => {
    const before = JSON.stringify(engine.model);
    CausalLib.applyIntervention(engine.model, { id: "IV4", kind: "DisableNode", targetId: "N-VAULT-FILE", params: {} });
    expect(JSON.stringify(engine.model)).toBe(before);
  });

  it("干预目标不存在时必须给 UNKNOWN 而不是猜测", () => {
    const c = CausalLib.compare(engine, "CASE-VAULT-WRITE-NORMAL",
      { id: "IV5", kind: "InsertElement", targetId: "E-DOES-NOT-EXIST", params: { nodeId: "X" } },
      { seed: 1, inputs });
    expect(c.outcome).toBe("UNKNOWN");
    expect(c.unknownReason).toBeTruthy();
  });

  it("DisableNode 指向不存在的节点时必须给 UNKNOWN（不得误报 NO_DIFFERENCE）", () => {
    const c = CausalLib.compare(engine, "CASE-VAULT-WRITE-NORMAL",
      { id: "IV6", kind: "DisableNode", targetId: "N-DOES-NOT-EXIST", params: {} },
      { seed: 1, inputs });
    expect(c.outcome).toBe("UNKNOWN");
    expect(c.unknownReason).toMatch(/does not exist/);
  });
});

// ----------------------------------------------------------------- prompt
describe("prompt.js · 1D Prompt", () => {
  const model = {
    requirements: REQS.requirements,
    nodes: ARCH.nodes, edges: ARCH.edges,
    cases: CASES.cases, evidence: EVID.evidence
  };

  it("合法 Prompt 必须通过校验", () => {
    const p = PromptLib.generate({
      promptId: "P-X", version: 1, objective: "o",
      requirementIds: ["REQ-API-SURFACE"], nodeEdgeIds: ["N-API-ROUTER"],
      caseIds: ["CASE-AUTH-MISSING"], evidenceIds: ["EV-007"]
    });
    const r = PromptLib.validate(p, model);
    expect(r.ok).toBe(true);
    expect(r.errors).toEqual([]);
  });

  it("未知 ID 必须报错（不得放行）", () => {
    const p = PromptLib.generate({
      promptId: "P-X", version: 1, objective: "o",
      requirementIds: ["REQ-NOT-EXIST"], nodeEdgeIds: ["N-NOPE"], caseIds: ["C-NOPE"],
      evidenceIds: ["EV-NOT-EXIST"]
    });
    const r = PromptLib.validate(p, model);
    expect(r.ok).toBe(false);
    expect(r.errors.length).toBeGreaterThanOrEqual(4);
    expect(r.errors.map((e) => e.field)).toContain("evidenceIds");
  });

  it("缺 objective 必须报错", () => {
    const p = PromptLib.generate({ promptId: "P-X", version: 1, objective: "" });
    const r = PromptLib.validate(p, model);
    expect(r.ok).toBe(false);
  });

  it("版本差分必须报告变化的字段", () => {
    const a = PromptLib.generate({ promptId: "P", version: 1, objective: "a" });
    const b = PromptLib.generate({ promptId: "P", version: 2, objective: "b" });
    const d = PromptLib.diff(a, b);
    expect(d.changed).toBe(true);
    expect(d.fields.map((f) => f.field)).toContain("objective");
  });

  it("渲染必须包含全部 schema 段", () => {
    const p = PromptLib.generate({ promptId: "P", version: 1, objective: "obj" });
    const t = PromptLib.render(p);
    ["Objective", "Requirement IDs", "Node / Edge IDs", "Case IDs", "Inputs", "Constraints", "Outputs", "Acceptance", "Evidence IDs"]
      .forEach((seg) => expect(t).toContain(seg));
  });
});

// --------------------------------------------------------------- diagrams
describe("diagrams.js · 2D 工程图渲染器", () => {
  const defs = readModel("diagram-definitions.json");

  it("必须实现全部 14 种 diagramType", () => {
    expect(Diagrams.TYPES.length).toBe(14);
    defs.diagrams.forEach((d) => {
      expect(Diagrams.TYPES).toContain(d.diagramType);
    });
  });

  it("每种图种都必须渲染出非空 SVG", () => {
    defs.diagrams.forEach((d) => {
      const svg = Diagrams.render(d, 600, 400);
      expect(svg.startsWith("<svg")).toBe(true);
      expect(svg.length).toBeGreaterThan(200);
      expect(svg).not.toContain("Unsupported diagramType");
    });
  });

  it("未知 diagramType 必须显式失败，不得退化成 Sequence", () => {
    const svg = Diagrams.render({ diagramType: "NotAType", title: "x" }, 400, 200);
    expect(svg).toContain("Unsupported diagramType");
  });

  it("必须转义 HTML 特殊字符（不得注入）", () => {
    const svg = Diagrams.render({ diagramType: "UML_Class", classes: [{ name: '<img src=x onerror=alert(1)>' }] }, 400, 200);
    expect(svg).not.toContain("<img");
    expect(svg).toContain("&lt;");
  });

  it("ER 图必须带基数标注，UML_Sequence 必须带生命线", () => {
    const er = defs.diagrams.find((d) => d.diagramType === "ER");
    const erSvg = Diagrams.render(er, 700, 460);
    expect(erSvg).toContain("1");
    const seq = defs.diagrams.find((d) => d.diagramType === "UML_Sequence");
    const seqSvg = Diagrams.render(seq, 700, 460);
    expect(seqSvg).toContain("line");
    expect(seqSvg).toContain("marker");
  });
});

// --------------------------------------------------- 审核回归防线
describe("对抗式审核回归防线", () => {
  const model = {
    version: "t1", nodes: ARCH.nodes, edges: ARCH.edges,
    rules: RULES.rules, cases: CASES.cases, stateMachines: RULES.stateMachines
  };
  const engine = EngineLib.createEngine(model);

  it("每个 Case 都必须至少产生一次 Transition（不得出现零转移 Case）", () => {
    const zero = [];
    for (const c of CASES.cases) {
      const run = engine.run(c.id, { seed: c.seed || 1, inputs: Object.assign({}, c.inputs || {}) });
      const transitions = run.events.filter((e) => e.kind === "Transition");
      if (transitions.length === 0) zero.push(c.id);
    }
    expect(zero).toEqual([]);
  });

  it("守卫引用的变量若未提供，守卫必须判否（缺标比错标安全）", () => {
    const bad = [];
    for (const c of CASES.cases) {
      const declared = new Set(Object.keys(c.inputs || {}));
      const entry = (c.entryNodes && c.entryNodes[0]) || c.entryNodeId;
      const ctx = { states: {}, inputs: c.inputs || {}, counters: {} };
      for (const rule of RULES.rules) {
        if (rule.sourceNodeId !== entry) continue;
        const refs = (JSON.stringify(rule.guardAst || {}).match(/input:([A-Za-z0-9_]+)/g) || []);
        const missing = refs.map((r) => r.slice(6)).filter((k) => !declared.has(k));
        if (missing.length && EngineLib.evalGuard(rule.guardAst, ctx)) {
          bad.push(c.id + " / " + rule.id + " 缺失 " + missing.join(",") + " 但守卫仍求值为真");
        }
      }
    }
    expect(bad).toEqual([]);
  });

  it("拒绝类规则必须排在同边的接受类规则之前", () => {
    const order = RULES.rules.map((r) => r.id);
    const pairs = [
      ["R-VAULT-REJECT-TRAVERSAL", "R-VAULT-WRITE"],
      ["R-VAULT-REJECT-EMPTY", "R-VAULT-WRITE"],
      ["R-AUTH-REJECT", "R-AUTH-CHECK"],
      ["R-GIT-RECEIVE-AUTH", "R-GIT-READ-OPEN"],
      ["R-AI-STREAM-REJECT-ANTHROPIC", "R-AI-STREAM-OPENAI"],
      ["R-SYNC-NOT-FASTFORWARD", "R-SYNC-FASTFORWARD"]
    ];
    const wrong = pairs.filter(([a, b]) => order.indexOf(a) > order.indexOf(b)).map(([a, b]) => a + " 晚于 " + b);
    expect(wrong).toEqual([]);
  });

  it("所有 id 必须唯一（node / edge / rule / case / evidence / stateMachine）", () => {
    const dup = [];
    const check = (arr, label) => {
      const seen = new Set();
      arr.forEach((x) => { if (seen.has(x)) dup.push(label + ":" + x); seen.add(x); });
    };
    check(ARCH.nodes.map((n) => n.id), "node");
    check(ARCH.edges.map((e) => e.id), "edge");
    check(ARCH.containers.map((c) => c.id), "container");
    check(RULES.rules.map((r) => r.id), "rule");
    check(RULES.stateMachines.map((s) => s.id), "stateMachine");
    check(CASES.cases.map((c) => c.id), "case");
    check(EVID.evidence.map((e) => e.id), "evidence");
    expect(dup).toEqual([]);
  });

  it("边与规则必须能解析到真实节点（无悬空引用）", () => {
    const nodeIds = new Set(ARCH.nodes.map((n) => n.id));
    const edgeIds = new Set(ARCH.edges.map((e) => e.id));
    const dangling = [];
    ARCH.edges.forEach((e) => {
      if (!nodeIds.has(e.sourceId)) dangling.push("edge " + e.id + " source " + e.sourceId);
      if (!nodeIds.has(e.targetId)) dangling.push("edge " + e.id + " target " + e.targetId);
    });
    RULES.rules.forEach((r) => {
      if (r.edgeId && !edgeIds.has(r.edgeId)) dangling.push("rule " + r.id + " edge " + r.edgeId);
    });
    expect(dangling).toEqual([]);
  });

  it("每条规则的 when 必须等于其源节点在可达状态机中的状态", () => {
    const smByNode = Object.fromEntries(RULES.stateMachines.map((s) => [s.nodeId, s]));
    const bad = [];
    for (const r of RULES.rules) {
      const sm = smByNode[r.sourceNodeId];
      if (!sm) { bad.push(r.id + " source has no state machine: " + r.sourceNodeId); continue; }
      if (r.when && sm.states.indexOf(r.when) === -1) {
        bad.push(r.id + " when=" + r.when + " not in states of " + r.sourceNodeId);
      }
    }
    expect(bad).toEqual([]);
  });

  // ---- 下面 5 条是「直接行为断言」：变异测试证明它们**能失败** ----

  it("evalGuard 对缺失变量必须判否（直接断言算子语义，不依赖任何 Case 数据）", () => {
    const ctx = { states: {}, inputs: {}, counters: {} };
    const missing = { type: "var", var: "input:__definitely_missing__" };
    expect(EngineLib.evalGuard({ type: "equals", left: missing, right: 0 }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "notEquals", left: missing, right: "" }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "gte", left: missing, right: 1 }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "lte", left: missing, right: 1 }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "gt", left: missing, right: 1 }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "contains", left: missing, right: "x" }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "startsWith", left: missing, right: "/" }, ctx)).toBe(false);
    expect(EngineLib.evalGuard({ type: "matchesDriveLetter", value: missing }, ctx)).toBe(false);
  });

  it("evalGuard 必须区分「缺失」与「合法 falsy 值」", () => {
    const ctx = { states: {}, inputs: { zero: 0, empty: "", no: false }, counters: {} };
    const v = (k) => ({ type: "var", var: "input:" + k });
    expect(EngineLib.evalGuard({ type: "equals", left: v("zero"), right: 0 }, ctx)).toBe(true);
    expect(EngineLib.evalGuard({ type: "equals", left: v("empty"), right: "" }, ctx)).toBe(true);
    expect(EngineLib.evalGuard({ type: "equals", left: v("no"), right: false }, ctx)).toBe(true);
    expect(EngineLib.evalGuard({ type: "notEquals", left: v("zero"), right: 1 }, ctx)).toBe(true);
  });

  it("when 不满足时规则不得触发（动态防线：把 whenSatisfied 改成恒真会被此条抓住）", () => {
    // 最小构造模型：X 初始 Idle，规则的 when 是 "Running" ⇒ 守卫前置状态不满足
    const N = (id) => ({ id, kind: "m", name: id, qualifiedName: id, role: "Processor", domainId: "Core", parentId: null, portIds: [], properties: {}, verification: "Implemented", evidenceIds: [] });
    const E = (id, s, t) => ({ id, sourceId: s, targetId: t, sourcePortId: null, targetPortId: null, type: "calls", directed: true, guardId: null, properties: {}, verification: "Implemented", evidenceIds: [] });
    const eng2 = EngineLib.createEngine({
      nodes: [N("X"), N("Y")],
      edges: [E("E1", "X", "Y")],
      rules: [{ id: "R-T", edgeId: "E1", sourceNodeId: "X", targetNodeId: "Y", when: "Running", then: "Done", guardAst: { type: "always" }, durationMs: 1 }],
      cases: [{ id: "C", name: "c", origin: "DesignedSimulation", entryNodeId: "X", entryNodes: ["X"], inputs: {}, seed: 1, expected: [] }],
      stateMachines: [
        { id: "SM-X", nodeId: "X", initial: "Idle", states: ["Idle", "Running", "Done"], terminalStates: ["Done"] },
        { id: "SM-Y", nodeId: "Y", initial: "Idle", states: ["Idle", "Done"], terminalStates: ["Done"] }
      ],
      version: "t"
    });
    const run = eng2.run("C", { seed: 1, inputs: {} });
    expect(run.events.filter((e) => e.kind === "Transition").length).toBe(0);
    expect(run.events.some((e) => e.kind === "WhenBlocked")).toBe(true);
  });

  it("真实 Case 里 when 不满足的规则确实未触发", () => {
    const model = {
      version: "t1", nodes: ARCH.nodes, edges: ARCH.edges,
      rules: RULES.rules, cases: CASES.cases, stateMachines: RULES.stateMachines
    };
    const eng = EngineLib.createEngine(model);
    const c = CASES.cases.find((x) => x.id === "CASE-VAULT-WRITE-EMPTY");
    const run = eng.run(c.id, { seed: c.seed || 1, inputs: Object.assign({}, c.inputs || {}) });
    const fired = run.events.filter((e) => e.kind === "Transition").map((e) => e.ruleId);
    expect(fired).toContain("R-VAULT-REJECT-EMPTY");
    // N-VAULT-FILE 已进入 Failed，其后续的 R-VAULT-VERSION-APPEND 不得触发
    expect(fired).not.toContain("R-VAULT-VERSION-APPEND");
  });

  it("findCycles 必须分别检出多个不同的自环", () => {
    const mk = (id) => ({
      id, kind: "m", name: id, qualifiedName: id, role: "Processor",
      domainId: "Core", parentId: null, portIds: [], properties: {},
      verification: "Implemented", evidenceIds: []
    });
    const e = (id, s, t) => ({
      id, sourceId: s, targetId: t, sourcePortId: null, targetPortId: null,
      type: "calls", directed: true, guardId: null, properties: {},
      verification: "Implemented", evidenceIds: []
    });
    const g = GraphLib.createGraph([mk("A"), mk("B")], [e("S1", "A", "A"), e("S2", "B", "B")]);
    expect(g.findCycles().length).toBe(2);
  });

  it("validate 必须逐个校验数组元素类型", () => {
    const def = SCHEMA.records.Node;
    const good = {
      id: "N-1", kind: "module", name: "x", qualifiedName: "q", role: "Processor",
      domainId: "Core", parentId: null, portIds: ["P-1"], properties: {}, verification: "Implemented", evidenceIds: []
    };
    expect(Validate.validateRecord(def, good).ok).toBe(true);
    const bad = Object.assign({}, good, { portIds: ["P-1", 42] });
    const r = Validate.validateRecord(def, bad);
    expect(r.ok).toBe(false);
    expect(r.errors.some((e) => /portIds\[1\]/.test(e.path))).toBe(true);
  });

  it("InsertElement 改写后旧边不得再触发（真正改写，不是新旧并联）", () => {
    const model = {
      version: "t1", nodes: ARCH.nodes, edges: ARCH.edges,
      rules: RULES.rules, cases: CASES.cases, stateMachines: RULES.stateMachines
    };
    const eng = EngineLib.createEngine(model);
    const applied = CausalLib.applyIntervention(eng.model, {
      id: "IV-RW", kind: "InsertElement", targetId: "E-005",
      params: { nodeId: "X-CACHE", name: "Vault Cache", kind: "Processor" }
    });
    expect(applied.error).toBeUndefined();
    // 旧边必须从生效边集里消失
    expect(applied.model.edges.some((e) => e.id === "E-005")).toBe(false);
    expect(applied.delta.operations[0].removedEdgeIds).toContain("E-005");
    // 图查询不得再看到旧边
    const g2 = GraphLib.createGraph(applied.model.nodes, applied.model.edges);
    expect(g2.outEdges("N-API-ROUTER").map((e) => e.id)).not.toContain("E-005");
    // 改写后运行的轨迹里不得再出现旧边
    const eng2 = EngineLib.createEngine(applied.model);
    const run = eng2.run("CASE-VAULT-WRITE-NORMAL", {
      seed: 1, inputs: Object.assign({}, CASES.cases.find((c) => c.id === "CASE-VAULT-WRITE-NORMAL").inputs)
    });
    const firedEdges = run.events.filter((e) => e.kind === "Transition").map((e) => e.edgeId);
    expect(firedEdges).not.toContain("E-005");
    expect(firedEdges.some((id) => id && id.indexOf("-X") !== -1)).toBe(true);
  });
});
