/**
 * src/prompt.js — 1D Prompt 工作台逻辑
 *
 * 契约（v10.0 immutable_dimensions.1D）：
 *   actions = 生成 / 编辑 / 校验 / 复制 / 导出 / 版本差分 / 链接证据
 *   schema  = promptId, version, objective, requirementIds, nodeEdgeIds,
 *             caseIds, inputs, constraints, outputs, acceptance, evidenceIds
 *
 * 校验是真的：requirementIds / nodeEdgeIds / caseIds / evidenceIds 必须
 * 在当前模型里真实存在，否则报 UNKNOWN —— 不把不存在的 ID 写进 Prompt。
 * 四类引用 ID 全部是 **硬错误**（不阻断即等于允许引用不存在的事实）。
 */
(function (root) {
  "use strict";

  var REQUIRED = ["promptId", "version", "objective", "requirementIds", "nodeEdgeIds",
    "caseIds", "inputs", "constraints", "outputs", "acceptance", "evidenceIds"];

  /**
   * 生成 Prompt 草稿。
   * @param {object} ctx { objective, requirementIds, nodeEdgeIds, caseIds, inputs, constraints, outputs, acceptance, evidenceIds, version }
   * @returns {object} Prompt
   */
  function generate(ctx) {
    ctx = ctx || {};
    return {
      promptId: ctx.promptId || "P-" + (ctx.version || 1),
      version: typeof ctx.version === "number" ? ctx.version : 1,
      objective: ctx.objective || "",
      requirementIds: (ctx.requirementIds || []).slice(),
      nodeEdgeIds: (ctx.nodeEdgeIds || []).slice(),
      caseIds: (ctx.caseIds || []).slice(),
      inputs: (ctx.inputs || []).slice(),
      constraints: (ctx.constraints || []).slice(),
      outputs: (ctx.outputs || []).slice(),
      acceptance: (ctx.acceptance || []).slice(),
      evidenceIds: (ctx.evidenceIds || []).slice()
    };
  }

  /**
   * 校验 Prompt 与模型的一致性。
   * @param {object} prompt
   * @param {object} model { requirements, nodes, edges, cases, evidence }
   * @returns {{ok, errors, warnings, resolved}}
   */
  function validate(prompt, model) {
    var errors = [];
    var warnings = [];
    model = model || {};
    var reqIds = index(model.requirements, "id");
    var nodeIds = index(model.nodes, "id");
    var edgeIds = index(model.edges, "id");
    var caseIds = index(model.cases, "id");
    var evIds = index(model.evidence, "id");

    REQUIRED.forEach(function (f) {
      if (prompt[f] === undefined || prompt[f] === null) {
        errors.push({ field: f, message: "missing required field" });
      }
    });
    if (typeof prompt.version !== "number" || prompt.version < 1) {
      errors.push({ field: "version", message: "version must be a positive integer" });
    }
    if (!prompt.objective || !String(prompt.objective).trim()) {
      errors.push({ field: "objective", message: "objective must not be empty" });
    }

    var resolved = { requirementIds: [], nodeEdgeIds: [], caseIds: [], evidenceIds: [] };

    (prompt.requirementIds || []).forEach(function (id) {
      if (reqIds.has(id)) resolved.requirementIds.push(id);
      else errors.push({ field: "requirementIds", message: "unknown requirement id: " + id });
    });
    (prompt.nodeEdgeIds || []).forEach(function (id) {
      if (nodeIds.has(id) || edgeIds.has(id)) resolved.nodeEdgeIds.push(id);
      else errors.push({ field: "nodeEdgeIds", message: "unknown node/edge id: " + id });
    });
    (prompt.caseIds || []).forEach(function (id) {
      if (caseIds.has(id)) resolved.caseIds.push(id);
      else errors.push({ field: "caseIds", message: "unknown case id: " + id });
    });
    (prompt.evidenceIds || []).forEach(function (id) {
      if (evIds.has(id)) resolved.evidenceIds.push(id);
      else errors.push({ field: "evidenceIds", message: "unknown evidence id: " + id });
    });

    (prompt.acceptance || []).forEach(function (a, i) {
      if (!String(a || "").trim()) {
        errors.push({ field: "acceptance[" + i + "]", message: "acceptance criterion must not be empty" });
      }
    });

    return { ok: errors.length === 0, errors: errors, warnings: warnings, resolved: resolved };
  }

  function index(list, key) {
    var s = new Set();
    (list || []).forEach(function (x) { if (x && x[key] != null) s.add(x[key]); });
    return s;
  }

  /**
   * 版本差分：逐字段比较两个版本。
   * @returns {{changed: boolean, fields: Array<{field, from, to}>}}
   */
  function diff(a, b) {
    var fields = [];
    var all = new Set(REQUIRED.concat(["notes"]));
    Array.from(all).forEach(function (f) {
      var va = a[f], vb = b[f];
      if (JSON.stringify(va) !== JSON.stringify(vb)) {
        fields.push({ field: f, from: va, to: vb });
      }
    });
    return { changed: fields.length > 0, fields: fields };
  }

  /**
   * 渲染为可复制的纯文本 Prompt。
   * @param {object} prompt
   * @returns {string}
   */
  function render(prompt) {
    if (!prompt) return "";
    var L = [];
    L.push("# Prompt " + prompt.promptId + "  (v" + prompt.version + ")");
    L.push("");
    L.push("## Objective");
    L.push(prompt.objective || "(未填写)");
    L.push("");
    L.push("## Requirement IDs");
    L.push((prompt.requirementIds || []).length ? prompt.requirementIds.join(", ") : "(none)");
    L.push("");
    L.push("## Node / Edge IDs");
    L.push((prompt.nodeEdgeIds || []).length ? prompt.nodeEdgeIds.join(", ") : "(none)");
    L.push("");
    L.push("## Case IDs");
    L.push((prompt.caseIds || []).length ? prompt.caseIds.join(", ") : "(none)");
    L.push("");
    L.push("## Inputs");
    (prompt.inputs || []).forEach(function (i) { L.push("- " + i); });
    L.push("");
    L.push("## Constraints");
    (prompt.constraints || []).forEach(function (i) { L.push("- " + i); });
    L.push("");
    L.push("## Outputs");
    (prompt.outputs || []).forEach(function (i) { L.push("- " + i); });
    L.push("");
    L.push("## Acceptance");
    (prompt.acceptance || []).forEach(function (i) { L.push("- " + i); });
    L.push("");
    L.push("## Evidence IDs");
    L.push((prompt.evidenceIds || []).length ? prompt.evidenceIds.join(", ") : "(none)");
    return L.join("\n");
  }

  var API = { generate: generate, validate: validate, diff: diff, render: render, REQUIRED: REQUIRED };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcPrompt = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
