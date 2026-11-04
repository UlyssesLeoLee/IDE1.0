/**
 * src/diagrams.js — 2D 独立专业工程图 SVG 渲染器
 *
 * 契约（v10.0 immutable_dimensions.2D）：
 *   - 14 种 diagramType 各有正确渲染器，不允许"所有图当 Sequence"。
 *   - 正式 UML/ER/BPMN 符号：生命线、激活条、同步/返回消息、
 *     alt/opt/loop/par 分支框、基数、PK/FK、Guard、Port。
 *   - 禁止用宝石或示意箭头替代规范符号。
 *   - 图窗可独立展开/浮窗/移动/缩放（容器由 app.js 负责）。
 *
 * 所有渲染输出 SVG 字符串；无外部依赖，file:// 可用。
 */
(function (root) {
  "use strict";

  var NS = "http://www.w3.org/2000/svg";
  var C = {
    fg: "#E8EFF7", sub: "#AAB9CB", mute: "#40536A",
    surface: "#121D2E", raised: "#1C2A3D",
    focus: "#5AC7C3", info: "#82ADFA", ok: "#74CA9D", warn: "#F0BA63", fail: "#F08886",
    line: "#5C7290"
  };

  // ---------------------------------------------------------- 基础图元

  function esc(s) {
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  }
  function el(tag, attrs, children) {
    var a = "";
    Object.keys(attrs || {}).forEach(function (k) {
      if (attrs[k] === undefined || attrs[k] === null) return;
      a += " " + k + '="' + esc(String(attrs[k])) + '"';
    });
    if (children && children.length) {
      return "<" + tag + a + ">" + children.join("") + "</" + tag + ">";
    }
    return "<" + tag + a + "></" + tag + ">";
  }

  function text(x, y, str, opts) {
    opts = opts || {};
    return el("text", {
      x: x, y: y,
      fill: opts.fill || C.fg,
      "font-size": opts.size || 12,
      "font-family": opts.mono ? "ui-monospace, Consolas, monospace" : "inherit",
      "font-weight": opts.weight || "normal",
      "text-anchor": opts.anchor || "start",
      "dominant-baseline": opts.baseline || "auto",
      "font-style": opts.italic ? "italic" : null
    }, [esc(str)]);
  }

  function line(x1, y1, x2, y2, opts) {
    opts = opts || {};
    return el("line", {
      x1: x1, y1: y1, x2: x2, y2: y2,
      stroke: opts.stroke || C.line,
      "stroke-width": opts.width || 1.2,
      "stroke-dasharray": opts.dash || null,
      "marker-end": opts.arrow ? "url(#" + (opts.arrowOpen ? "arrowOpen" : "arrow") + ")" : null
    });
  }

  function rect(x, y, w, h, opts) {
    opts = opts || {};
    return el("rect", {
      x: x, y: y, width: w, height: h,
      rx: opts.rx === undefined ? 4 : opts.rx,
      fill: opts.fill || "none",
      stroke: opts.stroke || C.line,
      "stroke-width": opts.width || 1.2,
      "stroke-dasharray": opts.dash || null
    });
  }

  function path(d, opts) {
    opts = opts || {};
    return el("path", {
      d: d,
      fill: opts.fill || "none",
      stroke: opts.stroke || C.line,
      "stroke-width": opts.width || 1.2,
      "stroke-dasharray": opts.dash || null,
      "marker-end": opts.arrow ? "url(#" + (opts.arrowOpen ? "arrowOpen" : "arrow") + ")" : null
    });
  }

  function polygon(points, opts) {
    opts = opts || {};
    return el("polygon", {
      points: points.map(function (p) { return p[0] + "," + p[1]; }).join(" "),
      fill: opts.fill || "none",
      stroke: opts.stroke || C.line,
      "stroke-width": opts.width || 1.2
    });
  }

  function ellipse(cx, cy, rx, ry, opts) {
    opts = opts || {};
    return el("ellipse", {
      cx: cx, cy: cy, rx: rx, ry: ry,
      fill: opts.fill || "none",
      stroke: opts.stroke || C.line,
      "stroke-width": opts.width || 1.2,
      "stroke-dasharray": opts.dash || null
    });
  }

  /** defs：箭头 marker（实心=同步/单向；空心=返回/开放） */
  function defs() {
    return el("defs", {}, [
      el("marker", {
        id: "arrow", viewBox: "0 0 10 10", refX: 9, refY: 5,
        markerWidth: 7, markerHeight: 7, orient: "auto-start-reverse"
      }, [el("path", { d: "M 0 0 L 10 5 L 0 10 z", fill: C.line })]),
      el("marker", {
        id: "arrowOpen", viewBox: "0 0 10 10", refX: 9, refY: 5,
        markerWidth: 7, markerHeight: 7, orient: "auto-start-reverse"
      }, [el("path", { d: "M 0 1 L 9 5 L 0 9", fill: "none", stroke: C.line, "stroke-width": 1.4 })])
    ]);
  }

  function svgWrap(w, h, body, title) {
    return el("svg", {
      xmlns: NS, width: w, height: h, viewBox: "0 0 " + w + " " + h,
      role: "img", "aria-label": title || "diagram"
    }, [defs()].concat(body));
  }

  /** 端口标记：UML Port（半圆） */
  function portMark(x, y, dir) {
    if (dir === "out") return path("M " + x + " " + y + " a 6 6 0 0 1 0 12", { stroke: C.focus, width: 1.4 });
    if (dir === "in") return path("M " + x + " " + (y + 12) + " a 6 6 0 0 1 0 -12", { stroke: C.focus, width: 1.4 });
    return "";
  }

  /** UML 分支框（alt / opt / loop / par） */
  function fragmentFrame(x, y, w, h, kind, label) {
    var out = [rect(x, y, w, h, { rx: 0, dash: "6 4", stroke: C.mute })];
    out.push(rect(x, y, Math.max(52, label.length * 8 + 16), 18, { rx: 0, fill: C.raised, stroke: C.mute }));
    out.push(text(x + 8, y + 13, "[" + kind + "]", { size: 11, mono: true, fill: C.warn }));
    if (label) out.push(text(x + 58, y + 13, label, { size: 11, fill: C.sub }));
    return out;
  }

  // --------------------------------------------------- 各图种渲染器

  var RENDERERS = {};

  /** UML 类图：三格框（名称/属性/方法） */
  RENDERERS.UML_Class = function (d, w, h) {
    var b = [];
    var classes = d.classes || [];
    var y = 30;
    var x = 30;
    var colW = Math.max(180, Math.floor((w - 80) / Math.max(1, Math.min(classes.length, 3))));
    classes.forEach(function (cls, ci) {
      var cx = x + ci * colW;
      var nameH = 26;
      var attrLines = cls.attributes || [];
      var methLines = cls.methods || [];
      var attrH = Math.max(20, attrLines.length * 16 + 8);
      var methH = Math.max(20, methLines.length * 16 + 8);
      var totalH = nameH + attrH + methH;
      b.push(rect(cx, y, colW - 20, totalH, { fill: C.surface, stroke: C.info }));
      b.push(rect(cx, y, colW - 20, nameH, { fill: C.raised, stroke: C.info }));
      b.push(text(cx + 10, y + 17, cls.name || "(unnamed)", { weight: "bold" }));
      b.push(line(cx, y + nameH, cx + colW - 20, y + nameH, { stroke: C.info }));
      attrLines.forEach(function (a, i) {
        b.push(text(cx + 10, y + nameH + 16 + i * 16, a.text, {
          size: 11, mono: true,
          italic: a.kind === "FK",
          weight: a.kind === "PK" ? "bold" : "normal",
          fill: a.kind === "PK" ? C.ok : (a.kind === "FK" ? C.warn : C.sub)
        }));
        if (a.kind === "PK") {
          b.push(line(cx + 10, y + nameH + 15 + i * 16, cx + 10 + (a.text.length * 6.2), y + nameH + 15 + i * 16, { stroke: C.ok, width: 1 }));
        }
      });
      b.push(line(cx, y + nameH + attrH, cx + colW - 20, y + nameH + attrH, { stroke: C.info }));
      methLines.forEach(function (m, i) {
        b.push(text(cx + 10, y + nameH + attrH + 16 + i * 16, m.text, { size: 11, mono: true, fill: C.sub }));
      });
      // Port
      (cls.ports || []).forEach(function (p, pi) {
        var px = cx + (pi + 1) * (colW - 20) / ((cls.ports || []).length + 1);
        b.push(portMark(px, y + totalH, p.direction || "out"));
      });
      // 关系
      (cls.relations || []).forEach(function (r) {
        b.push(line(cx + colW - 20, y + 30, cx + colW - 20 + (r.dx || 60), y + 30 + (r.dy || 0), { arrow: true }));
        if (r.label) b.push(text(cx + colW - 14, y + 26, r.label, { size: 10, fill: C.sub }));
      });
    });
    return svgWrap(w, h, b, d.title || "UML Class");
  };

  /** UML 组件图：组件图标（两个小矩形） */
  RENDERERS.UML_Component = function (d, w, h) {
    var b = [];
    (d.components || []).forEach(function (c, i) {
      var x = 40 + (i % 3) * 220;
      var y = 50 + Math.floor(i / 3) * 150;
      b.push(rect(x, y, 170, 90, { fill: C.surface, stroke: C.info }));
      b.push(rect(x + 12, y - 10, 16, 22, { fill: C.raised, stroke: C.info }));
      b.push(rect(x + 12, y + 22, 16, 22, { fill: C.raised, stroke: C.info }));
      b.push(text(x + 40, y + 30, c.name, { weight: "bold" }));
      if (c.stereotype) b.push(text(x + 40, y + 46, "«" + c.stereotype + "»", { size: 11, fill: C.sub }));
      (c.provided || []).forEach(function (p, pi) {
        b.push(rect(x + 40, y + 58 + pi * 14, 90, 12, { fill: "none", stroke: C.focus, rx: 2 }));
        b.push(text(x + 45, y + 68 + pi * 14, "provided " + p, { size: 10, fill: C.focus }));
      });
      (c.required || []).forEach(function (p, pi) {
        b.push(rect(x + 40, y + 58 + pi * 14, 90, 12, { fill: "none", stroke: C.warn, rx: 2 }));
        b.push(text(x + 45, y + 68 + pi * 14, "required " + p, { size: 10, fill: C.warn }));
      });
    });
    (d.relations || []).forEach(function (r) {
      b.push(line(r.x1, r.y1, r.x2, r.y2, { arrow: true, dash: r.dash || null }));
      if (r.label) b.push(text((r.x1 + r.x2) / 2, (r.y1 + r.y2) / 2 - 4, r.label, { size: 10, fill: C.sub, anchor: "middle" }));
    });
    return svgWrap(w, h, b, d.title || "UML Component");
  };

  /** UML 包图：文件夹 tab */
  RENDERERS.UML_Package = function (d, w, h) {
    var b = [];
    (d.packages || []).forEach(function (p, i) {
      var x = 40 + (i % 3) * 230;
      var y = 50 + Math.floor(i / 3) * 170;
      b.push(rect(x, y, 200, 140, { fill: C.surface, stroke: C.info }));
      b.push(rect(x, y - 16, 100, 18, { fill: C.raised, stroke: C.info, rx: 0 }));
      b.push(text(x + 10, y - 3, p.name, { size: 11, weight: "bold" }));
      (p.children || []).forEach(function (c, ci) {
        b.push(rect(x + 14, y + 16 + ci * 26, 170, 20, { fill: C.raised, stroke: C.mute, rx: 2 }));
        b.push(text(x + 22, y + 30 + ci * 26, c, { size: 11, fill: C.sub }));
      });
    });
    (d.relations || []).forEach(function (r) {
      b.push(line(r.x1, r.y1, r.x2, r.y2, { arrow: true }));
    });
    return svgWrap(w, h, b, d.title || "UML Package");
  };

  /** UML 时序图：生命线 + 激活条 + 消息 + alt/opt/loop/par */
  RENDERERS.UML_Sequence = function (d, w, h) {
    var b = [];
    var lifelines = d.lifelines || [];
    var top = 60;
    var colW = lifelines.length > 1 ? Math.floor((w - 100) / (lifelines.length - 1)) : 0;
    var xs = lifelines.map(function (l, i) { return 50 + i * colW; });

    lifelines.forEach(function (l, i) {
      b.push(rect(xs[i] - 62, 16, 124, 30, { fill: C.surface, stroke: C.info }));
      b.push(text(xs[i], 35, l.name, { weight: "bold", anchor: "middle" }));
      b.push(line(xs[i], 46, xs[i], h - 30, { stroke: C.mute, dash: "5 5" }));
    });

    var y = 80;
    (d.messages || []).forEach(function (m) {
      var si = m.from, ti = m.to;
      if (m.fragment) {
        var fw = Math.abs(xs[ti] - xs[si]) + 60;
        var fx = Math.min(xs[si], xs[ti]) - 30;
        b = b.concat(fragmentFrame(fx, y - 16, fw, m.fragmentHeight || 90, m.fragment, m.fragmentLabel));
        y += (m.fragmentHeight || 90) - 16;
      }
      var isReturn = m.kind === "return";
      var isAsync = m.kind === "async";
      b.push(line(xs[si], y, xs[ti], y, {
        arrow: true,
        dash: isReturn ? "5 4" : (isAsync ? "2 4" : null),
        arrowOpen: isReturn
      }));
      if (m.label) {
        b.push(text((xs[si] + xs[ti]) / 2, y - 6, m.label, { size: 11, anchor: "middle", fill: C.sub }));
      }
      if (m.guard) {
        b.push(text(xs[si] + 6, y + 14, "[" + m.guard + "]", { size: 10, fill: C.warn, mono: true }));
      }
      // 激活条
      if (m.activation) {
        b.push(rect(xs[ti] - 4, y, 8, m.activationHeight || 26, { fill: C.raised, stroke: C.focus, rx: 1 }));
      }
      y += 34;
    });
    return svgWrap(w, h, b, d.title || "UML Sequence");
  };

  /** UML 状态图：圆角状态 + 初始/终止 + 转换标签 */
  RENDERERS.UML_State = function (d, w, h) {
    var b = [];
    var states = d.states || [];
    states.forEach(function (s, i) {
      var x = s.x || (60 + (i % 3) * 200);
      var y = s.y || (50 + Math.floor(i / 3) * 120);
      var sw = s.w || 150, sh = s.h || 46;
      if (s.initial) b.push(ellipse(x + sw / 2, y - 24, 7, 7, { fill: C.fg, stroke: C.fg }));
      if (s.final) {
        b.push(ellipse(x + sw / 2, y + sh / 2, sw / 2 - 8, sh / 2 - 8, { stroke: C.fg }));
        b.push(ellipse(x + sw / 2, y + sh / 2, sw / 2 - 13, sh / 2 - 13, { stroke: C.fg }));
      } else {
        b.push(rect(x, y, sw, sh, { fill: C.surface, stroke: C.info, rx: 12 }));
      }
      b.push(text(x + sw / 2, y + sh / 2 + 4, s.name, { anchor: "middle", weight: s.initial || s.final ? "normal" : "bold" }));
    });
    (d.transitions || []).forEach(function (t) {
      var s1 = states[t.from], s2 = states[t.to];
      if (!s1 || !s2) return;
      var x1 = s1.x || 60, y1 = (s1.y || 50) + (s1.h || 46) / 2;
      var x2 = s2.x || 60, y2 = (s2.y || 50) + (s2.h || 46) / 2;
      b.push(line(x1 + (s1.w || 150) / 2, y1, x2 + (s2.w || 150) / 2, y2, { arrow: true }));
      b.push(text((x1 + x2) / 2 + (s1.w || 150) / 2, (y1 + y2) / 2 - 6, t.label || "", { size: 11, anchor: "middle", fill: C.sub }));
      if (t.guard) b.push(text((x1 + x2) / 2 + (s1.w || 150) / 2, (y1 + y2) / 2 + 9, "[" + t.guard + "]", { size: 10, anchor: "middle", fill: C.warn, mono: true }));
    });
    return svgWrap(w, h, b, d.title || "UML State");
  };

  /** UML 活动图：圆角活动 + 菱形判断 */
  RENDERERS.UML_Activity = function (d, w, h) {
    var b = [];
    var acts = d.activities || [];
    acts.forEach(function (a, i) {
      var x = a.x || (60 + (i % 3) * 220);
      var y = a.y || (50 + Math.floor(i / 3) * 130);
      var aw = a.w || 170, ah = a.h || 52;
      if (a.type === "decision") {
        b.push(polygon([[x + aw / 2, y], [x + aw, y + ah / 2], [x + aw / 2, y + ah], [x, y + ah / 2]], { fill: C.surface, stroke: C.warn }));
        b.push(text(x + aw / 2, y + ah / 2 + 4, a.name, { anchor: "middle", size: 11 }));
      } else if (a.type === "start") {
        b.push(ellipse(x + aw / 2, y + ah / 2, 12, 12, { fill: C.fg, stroke: C.fg }));
      } else if (a.type === "end") {
        b.push(ellipse(x + aw / 2, y + ah / 2, 13, 13, { fill: "none", stroke: C.fg, width: 1.6 }));
        b.push(ellipse(x + aw / 2, y + ah / 2, 8, 8, { fill: C.fg, stroke: C.fg }));
      } else {
        b.push(rect(x, y, aw, ah, { fill: C.surface, stroke: C.info, rx: 14 }));
        b.push(text(x + aw / 2, y + ah / 2 + 4, a.name, { anchor: "middle" }));
      }
    });
    (d.flows || []).forEach(function (f) {
      b.push(line(f.x1, f.y1, f.x2, f.y2, { arrow: true }));
      if (f.label) b.push(text((f.x1 + f.x2) / 2, (f.y1 + f.y2) / 2 - 5, f.label, { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "UML Activity");
  };

  /** UML 部署图：节点立体框 + 通信路径 */
  RENDERERS.UML_Deployment = function (d, w, h) {
    var b = [];
    (d.nodes || []).forEach(function (n) {
      var x = n.x, y = n.y, nw = n.w || 130, nh = n.h || 90;
      b.push(rect(x, y, nw, nh, { fill: C.surface, stroke: C.info }));
      // 立体顶面
      b.push(polygon([[x, y], [x + 14, y - 14], [x + nw + 14, y - 14], [x + nw, y]], { fill: C.raised, stroke: C.info }));
      b.push(polygon([[x + nw, y], [x + nw + 14, y - 14], [x + nw + 14, y + nh - 14], [x + nw, y + nh]], { fill: "none", stroke: C.info }));
      b.push(text(x + 10, y + 26, "«" + (n.stereotype || "node") + "»", { size: 10, fill: C.sub }));
      b.push(text(x + 10, y + 44, n.name, { weight: "bold" }));
      (n.nested || []).forEach(function (c, ci) {
        b.push(rect(x + 12, y + 54 + ci * 16, nw - 24, 14, { fill: C.raised, stroke: C.mute, rx: 2 }));
        b.push(text(x + 18, y + 64 + ci * 16, c, { size: 10, fill: C.sub }));
      });
    });
    (d.paths || []).forEach(function (p) {
      b.push(line(p.x1, p.y1, p.x2, p.y2, { arrow: p.arrow !== false, dash: p.dash || null }));
      if (p.label) b.push(text((p.x1 + p.x2) / 2, (p.y1 + p.y2) / 2 - 5, p.label, { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "UML Deployment");
  };

  /** UML 用例图：椭圆 + 参与者火柴人 */
  RENDERERS.UML_UseCase = function (d, w, h) {
    var b = [];
    (d.useCases || []).forEach(function (u, i) {
      var x = 120 + (i % 2) * 220;
      var y = 60 + Math.floor(i / 2) * 110;
      b.push(ellipse(x + 70, y + 26, 68, 26, { fill: C.surface, stroke: C.info }));
      b.push(text(x + 70, y + 30, u.name, { anchor: "middle", size: 11 }));
    });
    (d.actors || []).forEach(function (a, i) {
      var x = a.x || 40, y = a.y || 80;
      b.push(ellipse(x, y, 9, 9, { fill: C.fg, stroke: C.fg }));           // 头
      b.push(line(x, y + 9, x, y + 42, { stroke: C.fg }));                // 躯干
      b.push(line(x - 16, y + 18, x + 16, y + 18, { stroke: C.fg }));    // 手臂
      b.push(line(x, y + 42, x - 13, y + 62, { stroke: C.fg }));
      b.push(line(x, y + 42, x + 13, y + 62, { stroke: C.fg }));
      b.push(text(x, y + 78, a.name, { anchor: "middle", size: 11 }));
    });
    (d.relations || []).forEach(function (r) {
      b.push(line(r.x1, r.y1, r.x2, r.y2, { arrow: r.kind !== "extend", dash: r.kind === "extend" ? "5 4" : null }));
      if (r.label) b.push(text((r.x1 + r.x2) / 2, (r.y1 + r.y2) / 2 - 5, "«" + r.kind + "»", { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "UML UseCase");
  };

  /** ER 图：实体 + PK/FK + 基数 + crow's foot */
  RENDERERS.ER = function (d, w, h) {
    var b = [];
    // 实体框的默认布局必须**只有一处定义** —— 关系锚点若另算一套默认值，
    // 两者会错位（关系线画到别处去）
    var entityBox = function (e, i) {
      return {
        x: e.x || (50 + (i % 2) * 260),
        y: e.y || (50 + Math.floor(i / 2) * 170),
        w: e.w || 210,
        h: 30 + (e.attributes || []).length * 16 + 8
      };
    };
    var indexOfEntity = function (id) {
      var list = d.entities || [];
      for (var i = 0; i < list.length; i++) if (list[i].id === id) return i;
      return -1;
    };
    (d.entities || []).forEach(function (e, i) {
      var box = entityBox(e, i);
      var x = box.x, y = box.y, ew = box.w;
      b.push(rect(x, y, ew, box.h, { fill: C.surface, stroke: C.data || C.ok }));
      b.push(rect(x, y, ew, 26, { fill: C.raised, stroke: C.ok }));
      b.push(text(x + 10, y + 17, e.name, { weight: "bold" }));
      (e.attributes || []).forEach(function (a, ai) {
        var ay = y + 26 + 16 + ai * 16;
        var lbl = a.name + (a.kind ? " : " + a.kind : "");
        b.push(text(x + 10, ay, lbl, {
          size: 11, mono: true,
          italic: a.kind === "FK",
          weight: a.kind === "PK" ? "bold" : "normal",
          fill: a.kind === "PK" ? C.ok : (a.kind === "FK" ? C.warn : C.sub)
        }));
        if (a.kind === "PK") {
          b.push(line(x + 10, ay - 3, x + 10 + a.name.length * 6.3, ay - 3, { stroke: C.ok, width: 1 }));
        }
      });
    });
    (d.relations || []).forEach(function (r) {
      var e1 = d.entities.filter(function (e) { return e.id === r.from; })[0];
      var e2 = d.entities.filter(function (e) { return e.id === r.to; })[0];
      if (!e1 || !e2) return;
      var b1 = entityBox(e1, indexOfEntity(r.from));
      var b2 = entityBox(e2, indexOfEntity(r.to));
      var x1 = b1.x + b1.w / 2, y1 = b1.y + 60;
      var x2 = b2.x + b2.w / 2, y2 = b2.y + 60;
      b.push(line(x1, y1, x2, y2, { stroke: C.ok }));
      // crow's foot 端
      b.push(crowFoot(x2, y2, x1, y1));
      // 基数
      if (r.fromCardinality) b.push(text(x1 + 6, y1 - 6, r.fromCardinality, { size: 10, mono: true, fill: C.sub }));
      if (r.toCardinality) b.push(text(x2 - 6, y2 - 6, r.toCardinality, { size: 10, mono: true, fill: C.sub, anchor: "end" }));
      if (r.label) b.push(text((x1 + x2) / 2, (y1 + y2) / 2 - 6, r.label, { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "ER");
  };

  function crowFoot(x, y, fromX, fromY) {
    var dx = x - fromX, dy = y - fromY;
    var len = Math.sqrt(dx * dx + dy * dy) || 1;
    var ux = dx / len, uy = dy / len;
    var px = -uy, py = ux;
    return path(
      "M " + x + " " + y +
      " L " + (x - ux * 12 + px * 6) + " " + (y - uy * 12 + py * 6) +
      " M " + x + " " + y +
      " L " + (x - ux * 12 - px * 6) + " " + (y - uy * 12 - py * 6) +
      " M " + x + " " + y +
      " L " + (x - ux * 12) + " " + (y - uy * 12),
      { stroke: C.ok, width: 1.3 }
    );
  }

  /** DFD：加工=圆 / 外部实体=矩形 / 数据存储=开放矩形 / 数据流=箭头 */
  RENDERERS.DFD = function (d, w, h) {
    var b = [];
    (d.processes || []).forEach(function (p, i) {
      var x = p.x || (60 + (i % 3) * 220), y = p.y || (50 + Math.floor(i / 3) * 140);
      var r = p.r || 42;
      b.push(ellipse(x, y, r, r * 0.6, { fill: C.surface, stroke: C.info }));
      b.push(text(x, y + 4, p.name, { anchor: "middle", size: 11 }));
    });
    (d.externals || []).forEach(function (e, i) {
      var x = e.x || (60 + (i % 3) * 220), y = e.y || (50 + Math.floor(i / 3) * 140);
      b.push(rect(x - 50, y - 22, 100, 44, { fill: C.surface, stroke: C.sub }));
      b.push(text(x, y + 4, e.name, { anchor: "middle", size: 11 }));
    });
    (d.stores || []).forEach(function (s, i) {
      var x = s.x || (60 + (i % 3) * 220), y = s.y || (50 + Math.floor(i / 3) * 140);
      b.push(path("M " + (x - 60) + " " + (y - 24) + " L " + (x + 60) + " " + (y - 24) +
        " L " + (x + 60) + " " + (y + 24) + " L " + (x - 60) + " " + (y + 24), { stroke: C.ok }));
      b.push(path("M " + (x - 60) + " " + (y - 24) + " L " + (x - 60) + " " + (y + 24) + " M " + (x + 60) + " " + (y - 24) + " L " + (x + 60) + " " + (y + 24), { stroke: C.ok }));
      b.push(text(x, y + 4, s.name, { anchor: "middle", size: 11 }));
    });
    (d.flows || []).forEach(function (f) {
      b.push(line(f.x1, f.y1, f.x2, f.y2, { arrow: true }));
      if (f.label) b.push(text((f.x1 + f.x2) / 2, (f.y1 + f.y2) / 2 - 5, f.label, { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "DFD");
  };

  /** BPMN：池/泳道 + 事件 + 任务 + 网关 */
  RENDERERS.BPMN = function (d, w, h) {
    var b = [];
    var poolY = 40;
    (d.pools || []).forEach(function (p, pi) {
      var py = poolY + pi * 150;
      b.push(rect(30, py, w - 60, 130, { fill: "none", stroke: C.sub }));
      b.push(rect(30, py, 130, 130, { fill: C.raised, stroke: C.sub }));
      b.push(text(95, py + 68, p.name, { anchor: "middle", size: 12, weight: "bold" }));
      (p.lanes || []).forEach(function (l, li) {
        var ly = py + 26 + li * 34;
        b.push(line(160, ly, w - 30, ly, { stroke: C.line }));
        b.push(text(165, ly - 4, l.name, { size: 10, fill: C.sub }));
      });
    });
    (d.nodes || []).forEach(function (n) {
      var x = n.x, y = n.y;
      if (n.type === "event") {
        b.push(ellipse(x, y, 13, 13, { fill: "none", stroke: C.info }));
        if (n.eventType === "end") b.push(ellipse(x, y, 17, 17, { fill: "none", stroke: C.info }));
        if (n.eventType === "message") {
          b.push(path("M " + (x - 9) + " " + (y - 9) + " L " + (x + 9) + " " + (y - 9), { stroke: C.info }));
        }
      } else if (n.type === "gateway") {
        b.push(polygon([[x, y - 15], [x + 15, y], [x, y + 15], [x - 15, y]], { fill: C.surface, stroke: C.warn }));
        if (n.gatewayType === "exclusive") b.push(text(x, y + 4, "×", { anchor: "middle", size: 14, fill: C.warn }));
        if (n.gatewayType === "parallel") {
          b.push(line(x - 9, y, x + 9, y, { stroke: C.warn, width: 1.4 }));
          b.push(line(x, y - 9, x, y + 9, { stroke: C.warn, width: 1.4 }));
        }
      } else if (n.type === "subprocess") {
        b.push(rect(x - 44, y - 22, 88, 44, { fill: C.surface, stroke: C.info, rx: 6 }));
        b.push(text(x, y + 4, n.name, { anchor: "middle", size: 11 }));
      } else {
        b.push(rect(x - 42, y - 20, 84, 40, { fill: C.surface, stroke: C.info, rx: 16 }));
        b.push(text(x, y + 4, n.name, { anchor: "middle", size: 11 }));
      }
    });
    (d.sequenceFlows || []).forEach(function (f) {
      b.push(line(f.x1, f.y1, f.x2, f.y2, { arrow: true, dash: f.dash || null }));
      if (f.label) b.push(text((f.x1 + f.x2) / 2, (f.y1 + f.y2) / 2 - 5, f.label, { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "BPMN");
  };

  /** 安全视图：信任边界 + 攻击向量 */
  RENDERERS.Security = function (d, w, h) {
    var b = [];
    (d.boundaries || []).forEach(function (bd) {
      b.push(rect(bd.x, bd.y, bd.w, bd.h, { fill: "none", stroke: C.warn, dash: "8 6" }));
      b.push(text(bd.x + 10, bd.y + 16, bd.label, { size: 11, fill: C.warn }));
    });
    (d.assets || []).forEach(function (a, i) {
      var x = a.x || (60 + (i % 4) * 170), y = a.y || 90;
      // 盾形
      b.push(path("M " + x + " " + (y - 22) + " L " + (x + 20) + " " + (y - 12) +
        " L " + (x + 20) + " " + (y + 8) + " Q " + x + " " + (y + 28) + " " + (x - 20) + " " + (y + 8) +
        " L " + (x - 20) + " " + (y - 12) + " Z", { fill: C.surface, stroke: C.warn }));
      b.push(text(x, y + 44, a.name, { anchor: "middle", size: 10 }));
    });
    (d.attackVectors || []).forEach(function (v) {
      b.push(line(v.x1, v.y1, v.x2, v.y2, { arrow: true, stroke: C.fail }));
      b.push(text((v.x1 + v.x2) / 2, (v.y1 + v.y2) / 2 - 6, v.label, { size: 10, anchor: "middle", fill: C.fail }));
    });
    (d.controls || []).forEach(function (c) {
      b.push(rect(c.x, c.y, c.w || 120, 26, { fill: C.surface, stroke: C.ok, rx: 4 }));
      b.push(text(c.x + 8, c.y + 17, c.label, { size: 11, fill: C.ok }));
    });
    return svgWrap(w, h, b, d.title || "Security");
  };

  /** 网络图：子网 + 节点 + 链路 */
  RENDERERS.Network = function (d, w, h) {
    var b = [];
    (d.subnets || []).forEach(function (s) {
      b.push(rect(s.x, s.y, s.w, s.h, { fill: "none", stroke: C.mute, dash: "4 4" }));
      b.push(text(s.x + 8, s.y + 16, s.label, { size: 11, fill: C.sub }));
    });
    (d.nodes || []).forEach(function (n) {
      var x = n.x, y = n.y;
      if (n.type === "router") {
        b.push(rect(x - 14, y - 9, 28, 18, { fill: C.surface, stroke: C.info, rx: 3 }));
        b.push(text(x, y + 26, n.name, { anchor: "middle", size: 10 }));
      } else if (n.type === "server") {
        b.push(rect(x - 13, y - 16, 26, 32, { fill: C.surface, stroke: C.info, rx: 3 }));
        b.push(line(x - 9, y - 6, x + 9, y - 6, { stroke: C.info }));
        b.push(text(x, y + 28, n.name, { anchor: "middle", size: 10 }));
      } else if (n.type === "firewall") {
        b.push(rect(x - 16, y - 10, 32, 20, { fill: C.surface, stroke: C.fail, rx: 2 }));
        b.push(line(x - 16, y - 10, x + 16, y + 10, { stroke: C.fail }));
        b.push(text(x, y + 28, n.name, { anchor: "middle", size: 10 }));
      } else {
        b.push(ellipse(x, y, 12, 12, { fill: C.surface, stroke: C.info }));
        b.push(text(x, y + 26, n.name, { anchor: "middle", size: 10 }));
      }
    });
    (d.links || []).forEach(function (l) {
      b.push(line(l.x1, l.y1, l.x2, l.y2, { stroke: C.line, dash: l.dash || null }));
      if (l.label) b.push(text((l.x1 + l.x2) / 2, (l.y1 + l.y2) / 2 - 5, l.label, { size: 10, anchor: "middle", fill: C.sub }));
    });
    return svgWrap(w, h, b, d.title || "Network");
  };

  /** 数据流图（带存储与处理标签） */
  RENDERERS.DataFlow = function (d, w, h) {
    var b = [];
    (d.stores || []).forEach(function (s, i) {
      var x = s.x || (60 + (i % 3) * 220), y = s.y || (60 + Math.floor(i / 3) * 140);
      b.push(path("M " + (x - 70) + " " + (y - 26) + " L " + (x + 70) + " " + (y - 26) +
        " L " + (x + 70) + " " + (y + 26) + " L " + (x - 70) + " " + (y + 26), { stroke: C.ok }));
      b.push(path("M " + (x - 70) + " " + (y - 26) + " L " + (x - 70) + " " + (y + 26) +
        " M " + (x + 70) + " " + (y - 26) + " L " + (x + 70) + " " + (y + 26), { stroke: C.ok }));
      b.push(text(x, y + 4, s.name, { anchor: "middle", size: 11 }));
    });
    (d.flows || []).forEach(function (f) {
      b.push(line(f.x1, f.y1, f.x2, f.y2, { arrow: true }));
      if (f.label) {
        b.push(text((f.x1 + f.x2) / 2, (f.y1 + f.y2) / 2 - 6, f.label, { size: 10, anchor: "middle", fill: C.sub }));
        if (f.dataLabel) {
          b.push(text((f.x1 + f.x2) / 2, (f.y1 + f.y2) / 2 + 10, f.dataLabel, { size: 10, anchor: "middle", fill: C.mute }));
        }
      }
    });
    return svgWrap(w, h, b, d.title || "DataFlow");
  };

  var TYPES = Object.keys(RENDERERS);

  /**
   * 按 diagramType 分派渲染 —— 未知类型必须显式失败，不静默退化成 Sequence。
   */
  function render(diagram, width, height) {
    if (!diagram) return "";
    var type = diagram.diagramType;
    var fn = RENDERERS[type];
    if (!fn) {
      return svgWrap(width || 800, height || 500, [
        text(24, 40, "Unsupported diagramType: " + String(type), { fill: C.fail, weight: "bold" }),
        text(24, 62, "Supported: " + TYPES.join(", "), { size: 11, fill: C.sub })
      ], "unsupported");
    }
    return fn(diagram, width || 800, height || 500);
  }

  var API = { render: render, TYPES: TYPES, RENDERERS: RENDERERS };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcDiagrams = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
