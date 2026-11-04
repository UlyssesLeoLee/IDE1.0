// 修 app.js：剩余的「HTML 字符串当 DOM 节点」误用（minimap / 时间轴 / 图窗 / 降级 2D）
import fs from "node:fs";
import path from "node:path";

const file = path.resolve(process.argv[2]);
let src = fs.readFileSync(file, "utf8");
const before = src.length;
const changes = [];

function swap(oldStr, newStr, label) {
  if (!src.includes(oldStr)) { changes.push("MISS: " + label); return; }
  src = src.replace(oldStr, newStr);
  changes.push("ok: " + label);
}

// ---- 1) minimap 节点 ----
swap(
`        var d = el("div", { class: "g-mm-node" + (S.canvas.selected.indexOf(n.id) !== -1 ? " sel" : ""), style: "left:" + (p.x / 9) + "px;top:" + (p.y / 9) + "px" });
        d.setAttribute("data-mm", n.id);
        d.addEventListener("click", function () {
          S.canvas.selected = [n.id];
          drawCanvas();
          renderRight();
        });
        mm.appendChild(d);
      });
    }`,
`        mm.insertAdjacentHTML("beforeend", el("div", {
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
    }`,
"minimap 节点");

// ---- 2) 时间轴事件 ----
swap(
`      var d = el("div", {
        class: "g-tl-ev g-tl-" + String(ev.kind || "Transition").toLowerCase(),
        title: "#" + ev.sequence + " t=" + ev.logicalTime + " " + ev.kind + " " + ev.sourceNodeId + "→" + ev.targetNodeId
      }, [String(i + 1)]);
      d.setAttribute("data-seq", ev.sequence);
      track.appendChild(d);`,
`      track.insertAdjacentHTML("beforeend", el("div", {
        class: "g-tl-ev g-tl-" + String(ev.kind || "Transition").toLowerCase(),
        title: "#" + ev.sequence + " t=" + ev.logicalTime + " " + ev.kind + " " + ev.sourceNodeId + "→" + ev.targetNodeId,
        "data-seq": ev.sequence
      }, [String(i + 1)]));`,
"时间轴事件");

// ---- 3) 工程图窗 ----
swap(
`    host.insertAdjacentHTML("beforeend", win);
    var body = $(".g-dw-body", win);
    body.innerHTML = Diagrams.render(def, 720, 460);
    var close = $(".g-dw-close", win);
    close.addEventListener("click", function () { win.remove(); });
    // 可拖动
    var head = $(".g-dw-head", win);`,
`    host.insertAdjacentHTML("beforeend", win);
    var winEl = host.lastElementChild;          // 必须重新查询真实 DOM 元素
    var body = $(".g-dw-body", winEl);
    body.innerHTML = Diagrams.render(def, 720, 460);
    var close = $(".g-dw-close", winEl);
    close.addEventListener("click", function () { winEl.remove(); });
    // 可拖动
    var head = $(".g-dw-head", winEl);`,
"工程图窗");

swap(
`    head.addEventListener("pointermove", function (ev) {
      if (!drag) return;
      win.style.left = (drag.l + ev.clientX - drag.x) + "px";
      win.style.top = (drag.t + ev.clientY - drag.y) + "px";
    });`,
`    head.addEventListener("pointermove", function (ev) {
      if (!drag) return;
      winEl.style.left = (drag.l + ev.clientX - drag.x) + "px";
      winEl.style.top = (drag.t + ev.clientY - drag.y) + "px";
    });`,
"图窗拖动");

// ---- 4) 降级 2D：不能对字符串设 innerHTML ----
swap(
`    var svg = el("svg", { class: "g-canvas", xmlns: "http://www.w3.org/2000/svg" });
    var parts = [];
    M.architecture.nodes.forEach(function (n, i) {
      parts.push(el("circle", { cx: 100 + (i % 8) * 110, cy: 70 + Math.floor(i / 8) * 100, r: 22, class: "g-node-shape g-domain-" + n.domainId }));
      parts.push(el("text", { x: 100 + (i % 8) * 110, y: 74 + Math.floor(i / 8) * 100, class: "g-node-name" }, [n.name]));
    });
    svg.innerHTML = parts.join("");
    container.innerHTML = "";
    container.appendChild(svg);`,
`    var parts = [];
    M.architecture.nodes.forEach(function (n, i) {
      parts.push(el("circle", { cx: 100 + (i % 8) * 110, cy: 70 + Math.floor(i / 8) * 100, r: 22, class: "g-node-shape g-domain-" + n.domainId }));
      parts.push(el("text", { x: 100 + (i % 8) * 110, y: 74 + Math.floor(i / 8) * 100, class: "g-node-name" }, [n.name]));
    });
    container.innerHTML = el("svg", { class: "g-canvas", xmlns: "http://www.w3.org/2000/svg" }, parts);`,
"降级 2D");

fs.writeFileSync(file, src, "utf8");
console.log("bytes " + before + " -> " + src.length);
changes.forEach((c) => console.log("  " + c));
