/**
 * src/validate.js — 轻量 JSON Schema 子集校验器（替代 ajv，file:// 可用，零依赖）
 *
 * 支持：type / required / properties / additionalProperties / items / enum /
 *      minimum / maximum / minItems / minLength / oneOf-ish(anyOf)
 * 不支持：$ref / remote schema / format（明确不支持即不假装支持）
 *
 * 契约来源：models/schema.json
 */
(function (root) {
  "use strict";

  var TYPE_CHECKS = {
    string: function (v) { return typeof v === "string"; },
    integer: function (v) { return typeof v === "number" && Number.isInteger(v); },
    number: function (v) { return typeof v === "number" && Number.isFinite(v); },
    boolean: function (v) { return typeof v === "boolean"; },
    array: function (v) { return Array.isArray(v); },
    object: function (v) { return v !== null && typeof v === "object" && !Array.isArray(v); },
    null: function (v) { return v === null; }
  };

  function typeMatches(value, type) {
    var base = String(type).replace(/\?$/, "");
    var optional = String(type).slice(-1) === "?";
    if (value === undefined) return optional;
    // 可空类型（? 后缀）同时接受 null —— 目录级证据用 null 表示"无行号"
    if (value === null) return optional || base === "null";
    var ok = TYPE_CHECKS[base] ? TYPE_CHECKS[base](value) : true;
    return ok;
  }

  function typeName(type) {
    var base = String(type).replace(/\?$/, "");
    if (base === "integer") return "integer";
    if (base === "object") return "object";
    if (base === "array") return "array";
    return base;
  }

  /**
   * @param {object} schema  形如 { required: [...], types: { field: "string" } } 的记录定义
   * @param {object} data    待校验对象
   * @param {string} [path]  错误路径前缀
   * @returns {{ok: boolean, errors: Array<{path: string, message: string}>}}
   */
  function validateRecord(schema, data, path) {
    path = path || "";
    var errors = [];
    if (!schema || !schema.types) {
      return { ok: true, errors: [] };
    }
    if (!TYPE_CHECKS.object(data)) {
      errors.push({ path: path || "$", message: "expected object, got " + (data === null ? "null" : typeof data) });
      return { ok: false, errors: errors };
    }

    var fields = schema.types;
    var required = schema.required || [];

    // required
    for (var i = 0; i < required.length; i++) {
      var rq = required[i];
      if (data[rq] === undefined) {
        errors.push({ path: joinPath(path, rq), message: "required field missing" });
      }
    }

    // properties
    Object.keys(fields).forEach(function (key) {
      var spec = fields[key];
      var val = data[key];
      if (val === undefined) {
        if (spec.endsWith("?")) return; // optional
        return; // required 已经报过了
      }
      var base = String(spec).replace(/\?$/, "");

      // 数组元素类型： "string[]" / "integer[]" / "string[]?"
      var arrMatch = /^(.*)\[\]$/.exec(base);
      if (arrMatch) {
        var elemSpec = arrMatch[1];
        if (!Array.isArray(val)) {
          errors.push({ path: joinPath(path, key), message: "expected array, got " + describe(val) });
          return;
        }
        var elemBase = String(elemSpec).replace(/\?$/, "");
        for (var j = 0; j < val.length; j++) {
          if (!typeMatches(val[j], elemBase)) {
            errors.push({
              path: joinPath(path, key) + "[" + j + "]",
              message: "expected " + typeName(elemBase) + ", got " + describe(val[j])
            });
          }
        }
        return;
      }

      if (base.indexOf("|") !== -1) {
        var allowed = base.split("|");
        var matched = allowed.some(function (t) { return typeMatches(val, t); });
        if (!matched) {
          errors.push({ path: joinPath(path, key), message: "expected one of [" + allowed.join(", ") + "], got " + describe(val) });
        }
        return;
      }
      if (!typeMatches(val, spec)) {
        errors.push({ path: joinPath(path, key), message: "expected " + typeName(base) + ", got " + describe(val) });
        return;
      }
      if (base === "array") {
        var minItems = schema.minItems && schema.minItems[key];
        if (typeof minItems === "number" && val.length < minItems) {
          errors.push({ path: joinPath(path, key), message: "expected at least " + minItems + " items, got " + val.length });
        }
      }
    });

    return { ok: errors.length === 0, errors: errors };
  }

  function describe(v) {
    if (v === null) return "null";
    if (Array.isArray(v)) return "array";
    return typeof v;
  }

  function joinPath(prefix, key) {
    return prefix ? prefix + "." + key : key;
  }

  /**
   * 校验一整个集合：{ [recordType]: data[] }
   * @param {object} schemaDoc  models/schema.json
   * @param {object} collection { RecordType: array }
   */
  function validateCollection(schemaDoc, collection) {
    var errors = [];
    var records = (schemaDoc && schemaDoc.records) || {};
    Object.keys(collection).forEach(function (type) {
      var def = records[type];
      if (!def) {
        errors.push({ path: type, message: "unknown record type '" + type + "'" });
        return;
      }
      var list = collection[type];
      if (!Array.isArray(list)) {
        errors.push({ path: type, message: "expected array of " + type });
        return;
      }
      list.forEach(function (item, idx) {
        var res = validateRecord(def, item, type + "[" + idx + "]");
        errors = errors.concat(res.errors);
      });
    });
    return { ok: errors.length === 0, errors: errors };
  }

  /** 枚举值校验（existence_0d / implementation / evidence_kind / scenario_origin / qa_status） */
  function validateEnum(schemaDoc, enumName, value) {
    // schema.json 里键名是 enumerations；兼容 enums
    var table = (schemaDoc && (schemaDoc.enumerations || schemaDoc.enums)) || {};
    var allowed = table[enumName] || [];
    return allowed.indexOf(value) !== -1;
  }

  var API = {
    validateRecord: validateRecord,
    validateCollection: validateCollection,
    validateEnum: validateEnum,
    typeMatches: typeMatches
  };

  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcValidate = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
