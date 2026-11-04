/**
 * src/spatial.js — 3D 空间图论渲染层（Three.js）
 *
 * 契约（v10.0 immutable_dimensions.3D）：
 *   - 语义坐标是左手系：+X 右、+Y 上、+Z 前。Three.js 是右手系，
 *     因此所有坐标变换集中在 Adapter 的 toThree()，渲染层不得自行翻转。
 *   - 语义层级 System→Building→Floor→Room→Node/Edge；虚拟空间不冒充物理部署事实。
 *   - 宝石形状 = 功能角色；颜色 = 业务领域；环 = 运行状态。
 *   - 字段/连线/关系符号不得宝石化。
 *   - 选中对象按图论相关性高亮子图，无关对象退避/弱化，可取消/恢复，
 *     且不改变真实模型或运行态。
 *   - Node/Edge/Port/Room 独立拾取；Edge 有独立名字与关系类型。
 */
(function (root) {
  "use strict";

  var THREE = root.THREE;
  var GD = root.GemArcDiagrams;

  // ---------------------------------------------------------- Adapter

  /**
   * 语义左手系 → Three.js 右手系。
   * 左手系 +Z 前 ⇄ 右手系 +Z 向观察者 ⇒ 取反 Z。
   */
  function toThree(v) {
    return { x: v[0], y: v[1], z: -v[2] };
  }

  function toSemantic(v) {
    return [v.x, v.y, -v.z];
  }

  // ------------------------------------------------------ 宝石几何

  /**
   * 按功能角色生成几何体。形状语义来自 design-system/visual-dictionary.json。
   * @param {string} role  Actor|Input|Processor|Decision|Storage|Message|Interface|Security|Agent|Composition|Output
   */
  function gemGeometry(role) {
    switch (role) {
      case "Actor": return new THREE.SphereGeometry(1, 20, 14);                      // 放射圆形
      case "Input": {                                                                 // 水滴切割
        var g = new THREE.ConeGeometry(1, 1.8, 18);
        g.rotateX(Math.PI);
        return g;
      }
      case "Processor": return new THREE.CylinderGeometry(1, 1, 1.6, 6);              // 六棱柱
      case "Decision": return new THREE.OctahedronGeometry(1.2);                       // 双锥八面体
      case "Storage": {                                                               // 阶梯矩形
        var s = new THREE.BoxGeometry(1.8, 0.5, 1.4);
        return s;
      }
      case "Message": {                                                               // 橄榄梭形
        var m = new THREE.SphereGeometry(1, 18, 12);
        m.scale(1.5, 0.7, 0.7);
        return m;
      }
      case "Interface": {                                                             // 三角晶体
        var t = new THREE.ConeGeometry(1.1, 1.7, 3);
        return t;
      }
      case "Security": {                                                              // 盾形
        var sh = new THREE.CylinderGeometry(1, 1.2, 1.4, 6);
        sh.scale(1, 1, 0.5);
        return sh;
      }
      case "Agent": {                                                                 // 五角晶体
        var a = new THREE.ConeGeometry(1.1, 1.7, 5);
        return a;
      }
      case "Composition": {                                                           // 皇冠晶体
        var c = new THREE.ConeGeometry(1.2, 1.6, 8);
        c.rotateX(Math.PI);
        return c;
      }
      case "Output": {                                                                // 梯形切割
        var o = new THREE.CylinderGeometry(0.6, 1.1, 1.5, 4);
        return o;
      }
      default: return new THREE.IcosahedronGeometry(1.05);
    }
  }

  // ------------------------------------------------------ 场景构建

  /**
   * @param {object} spatial  models/spatial.json
   * @param {object} arch     models/architecture.json
   * @param {object} opts     { container, reducedMotion }
   */
  function createScene(spatial, arch, opts) {
    opts = opts || {};
    if (!THREE) return null;

    var container = opts.container;
    var scene = new THREE.Scene();
    scene.background = new THREE.Color("#0B1220");

    var camera = new THREE.PerspectiveCamera(50, 1, 0.1, 2000);
    // 左手系默认视角：站在 -Z 外看向 +Z（语义前方）
    camera.position.set(0, 24, 90);
    camera.lookAt(0, -6, 0);

    var renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.setPixelRatio(Math.min(root.devicePixelRatio || 1, 2));
    if (container) container.appendChild(renderer.domElement);

    var ambient = new THREE.AmbientLight(0xffffff, 0.55);
    scene.add(ambient);
    var key = new THREE.DirectionalLight(0xffffff, 0.9);
    key.position.set(40, 60, 80);
    scene.add(key);
    var rim = new THREE.DirectionalLight(0x5AC7C3, 0.35);
    rim.position.set(-40, 20, -60);
    scene.add(rim);

    var palette = spatial && spatial.gemstoneMapping ? null : null;
    var domainColors = (spatial && spatial.gemstoneMapping) ? null : null;
    var colors = {
      Core: 0x4C83FF, Interaction: 0x19AFA3, Data: 0x32B67A, Security: 0xE8B34F,
      Intelligence: 0xA276E8, Infrastructure: 0x8397B7, External: 0x38A6D6,
      Operations: 0xD87792, Unknown: 0x81909F
    };

    var nodeGroup = new THREE.Group();
    var edgeGroup = new THREE.Group();
    var roomGroup = new THREE.Group();
    scene.add(nodeGroup, edgeGroup, roomGroup);

    var pickables = [];
    var nodeMeshes = {};
    var edgeMeshes = {};
    var roomMeshes = {};
    var labelEls = {};
    var edgeLabelEls = {};

    var nodeById = {};
    (arch.nodes || []).forEach(function (n) { nodeById[n.id] = n; });

    // ---- Room（半透明盒体，不是宝石）
    var rooms = (spatial && spatial.rooms) || {};
    Object.keys(rooms).forEach(function (rid) {
      var r = rooms[rid];
      var p = toThree(r.position);
      var size = r.size;
      var geo = new THREE.BoxGeometry(size[0], size[1], size[2]);
      var mat = new THREE.MeshBasicMaterial({
        color: 0x40536A, transparent: true, opacity: r.storageOnly ? 0.05 : 0.10,
        depthWrite: false, side: THREE.DoubleSide
      });
      var mesh = new THREE.Mesh(geo, mat);
      mesh.position.set(p.x, p.y, p.z);
      mesh.userData = { kind: "Room", id: rid, label: r.label };
      roomGroup.add(mesh);
      roomMeshes[rid] = mesh;
      pickables.push(mesh);
    });

    // ---- Node（宝石）
    var gm = (spatial && spatial.gemstoneMapping && spatial.gemstoneMapping.byNode) || {};
    (arch.nodes || []).forEach(function (n) {
      if (n.parentId && rooms[n.parentId]) {
        var off = (spatial.nodeOffsets || {})[n.id];
        if (!off) return;
        var rp = toThree(rooms[n.parentId].position);
        var tp = toThree(off);
        // 节点坐标已是建筑原点系；房间坐标也是；此处按房间偏移叠加
        var wp = { x: rp.x + tp.x, y: rp.y + tp.y, z: rp.z + tp.z };
        var geo = gemGeometry(n.role);
        var mat = new THREE.MeshStandardMaterial({
          color: colors[n.domainId] || colors.Unknown,
          roughness: 0.42, metalness: 0.18,
          transparent: true, opacity: 1
        });
        var mesh = new THREE.Mesh(geo, mat);
        mesh.position.set(wp.x, wp.y, wp.z);
        mesh.userData = { kind: "Node", id: n.id, name: n.name, role: n.role, domain: n.domainId, verification: n.verification };
        nodeGroup.add(mesh);
        nodeMeshes[n.id] = mesh;
        pickables.push(mesh);

        // 运行状态环（ring = 运行状态）
        var ringGeo = new THREE.TorusGeometry(1.45, 0.06, 8, 32);
        var ringMat = new THREE.MeshBasicMaterial({ color: 0x5AC7C3, transparent: true, opacity: 0.25 });
        var ring = new THREE.Mesh(ringGeo, ringMat);
        ring.position.copy(mesh.position);
        ring.rotation.x = Math.PI / 2;
        ring.userData = { kind: "Ring", id: n.id };
        nodeGroup.add(ring);
        mesh.userData.ring = ring;
      }
    });

    // ---- Edge（独立几何 + 独立名字，不是宝石）
    var edgeMat = new THREE.LineBasicMaterial({ color: 0x5C7290, transparent: true, opacity: 0.55 });
    var edgeMatHi = new THREE.LineBasicMaterial({ color: 0x5AC7C3, transparent: true, opacity: 1 });
    (arch.edges || []).forEach(function (e) {
      var a = nodeMeshes[e.sourceId], b = nodeMeshes[e.targetId];
      if (!a || !b) return;
      var geo = new THREE.BufferGeometry().setFromPoints([a.position.clone(), b.position.clone()]);
      var line = new THREE.Line(geo, edgeMat.clone());
      line.userData = { kind: "Edge", id: e.id, type: e.type, label: e.type, sourceId: e.sourceId, targetId: e.targetId };
      edgeGroup.add(line);
      edgeMeshes[e.id] = line;
      pickables.push(line);

      // 箭头：Edge 上的箭头必须可查阅
      var dir = new THREE.Vector3().subVectors(b.position, a.position).normalize();
      var arrow = new THREE.Mesh(
        new THREE.ConeGeometry(0.22, 0.7, 8),
        new THREE.MeshBasicMaterial({ color: 0x5C7290 })
      );
      arrow.position.copy(a.position).addScaledVector(dir, a.position.distanceTo(b.position) * 0.72);
      arrow.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir);
      arrow.userData = { kind: "EdgeArrow", id: e.id };
      edgeGroup.add(arrow);
      line.userData.arrow = arrow;
    });

    // ---- 标签（DOM 覆盖层，避免 examples/jsm 动态 import）
    var labelLayer = document.createElement("div");
    labelLayer.className = "gemarc-label-layer";
    if (container) container.appendChild(labelLayer);
    else if (container && container.parentElement) container.parentElement.appendChild(labelLayer);

    function addLabel(id, text, kind, x, y, z) {
      var el = document.createElement("div");
      el.className = "gemarc-node-label gemarc-label-" + kind;
      el.textContent = text;
      el.setAttribute("data-target", id);
      el.setAttribute("data-kind", kind);
      labelLayer.appendChild(el);
      labelEls[id] = { el: el, pos: new THREE.Vector3(x, y, z) };
      return el;
    }
    Object.keys(nodeMeshes).forEach(function (id) {
      var m = nodeMeshes[id];
      addLabel(id, m.userData.name, "node", m.position.x, m.position.y, m.position.z);
    });
    Object.keys(edgeMeshes).forEach(function (id) {
      var l = edgeMeshes[id];
      var mid = new THREE.Vector3().addVectors(
        nodeMeshes[l.userData.sourceId].position,
        nodeMeshes[l.userData.targetId].position
      ).multiplyScalar(0.5);
      addLabel(id, l.userData.label, "edge", mid.x, mid.y, mid.z);
    });

    /** 环状态色：design-system/tokens.json ring.states 的 10 态 */
  var RING_COLORS = {
    Idle: 0x40536A, Running: 0x5AC7C3, Waiting: 0xF0BA63,
    Completed: 0x74CA9D, Failed: 0xF08886, Recovered: 0x82ADFA, Unknown: 0x81909F,
    Authorized: 0x74CA9D, Rejected: 0xF08886, Blocked: 0xF0BA63
  };
  // ------------------------------------------------ 交互

    var state = {
      selected: null,
      related: null,
      retreat: false,
      view: "Overview",
      showLabels: "Smart",
      split: { active: false, a: null, b: null },
      disposed: false
    };

    var raycaster = new THREE.Raycaster();
    var pointer = new THREE.Vector2();
    var dragging = null;
    var lastPointer = { x: 0, y: 0 };

    function ndc(ev) {
      var rect = renderer.domElement.getBoundingClientRect();
      pointer.x = ((ev.clientX - rect.left) / rect.width) * 2 - 1;
      pointer.y = -((ev.clientY - rect.top) / rect.height) * 2 + 1;
    }

    function pick(ev) {
      ndc(ev);
      raycaster.setFromCamera(pointer, camera);
      var hits = raycaster.intersectObjects(pickables, false);
      if (!hits.length) return null;
      return hits[0].object;
    }

    /** 按图论相关性计算高亮子图 */
    function relatedSet(nodeId) {
      if (!nodeId) return null;
      var set = new Set([nodeId]);
      (arch.edges || []).forEach(function (e) {
        if (e.sourceId === nodeId) set.add(e.targetId);
        if (e.targetId === nodeId) set.add(e.sourceId);
      });
      // 二跳
      var once = Array.from(set);
      once.forEach(function (mid) {
        (arch.edges || []).forEach(function (e) {
          if (e.sourceId === mid) set.add(e.targetId);
          if (e.targetId === mid) set.add(e.sourceId);
        });
      });
      return set;
    }

    function applyEmphasis() {
      var rel = state.retreat ? state.related : null;
      Object.keys(nodeMeshes).forEach(function (id) {
        var m = nodeMeshes[id];
        var dim = rel && !rel.has(id);
        m.material.opacity = dim ? 0.16 : 1;
        if (m.userData.ring) m.userData.ring.material.opacity = dim ? 0.05 : 0.25;
      });
      Object.keys(edgeMeshes).forEach(function (id) {
        var l = edgeMeshes[id];
        var dim = rel && !(rel.has(l.userData.sourceId) && rel.has(l.userData.targetId));
        l.material.opacity = dim ? 0.08 : 0.55;
        if (l.userData.arrow) l.userData.arrow.material.opacity = dim ? 0.08 : 1;
      });
      Object.keys(labelEls).forEach(function (id) {
        var info = labelEls[id];
        var dim = rel && !rel.has(id);
        info.el.style.opacity = dim ? "0.15" : "1";
      });
    }

    function select(obj) {
      if (!obj) {
        state.selected = null;
        state.related = null;
        state.retreat = false;
        applyEmphasis();
        return null;
      }
      var kind = obj.userData.kind;
      if (kind === "Node") {
        state.selected = { kind: "Node", id: obj.userData.id };
        state.related = relatedSet(obj.userData.id);
        state.retreat = true;
      } else if (kind === "Edge") {
        state.selected = { kind: "Edge", id: obj.userData.id };
        state.related = relatedSet(obj.userData.sourceId);
        state.retreat = true;
      } else if (kind === "Room") {
        state.selected = { kind: "Room", id: obj.userData.id };
        state.related = new Set();
        (rooms[obj.userData.id].nodes || []).forEach(function (nid) { state.related.add(nid); });
        state.retreat = true;
      } else {
        return null;
      }
      applyEmphasis();
      // 3D 选中 → 通知宿主（检查器联动必须由宿主写回选中状态）
      if (typeof opts.onSelect === "function") {
        opts.onSelect(state.selected ? { kind: state.selected.kind, id: state.selected.id } : null);
      }
      return state.selected;
    }

    // 轨道 / 平移 / 缩放
    var yaw = 0, pitch = 0.35, dist = 110, target = new THREE.Vector3(0, -6, 0);

    function applyCamera() {
      var cp = Math.cos(pitch), sp = Math.sin(pitch);
      camera.position.set(
        target.x + dist * cp * Math.sin(yaw),
        target.y + dist * sp,
        target.z + dist * cp * Math.cos(yaw)
      );
      camera.lookAt(target);
    }

    function onPointerDown(ev) {
      var obj = pick(ev);
      lastPointer = { x: ev.clientX, y: ev.clientY };
      if (obj) {
        dragging = { mode: "pick", obj: obj, moved: false };
      } else {
        dragging = { mode: "orbit", moved: false };
      }
      renderer.domElement.setPointerCapture && renderer.domElement.setPointerCapture(ev.pointerId);
    }

    function onPointerMove(ev) {
      if (!dragging) {
        if (state.showLabels !== "All") updateLabelVisibility();
        return;
      }
      var dx = ev.clientX - lastPointer.x;
      var dy = ev.clientY - lastPointer.y;
      lastPointer = { x: ev.clientX, y: ev.clientY };
      if (Math.abs(dx) + Math.abs(dy) > 2) dragging.moved = true;
      if (dragging.mode === "orbit") {
        yaw -= dx * 0.005;
        pitch = Math.max(-1.2, Math.min(1.35, pitch + dy * 0.004));
        applyCamera();
      }
    }

    function onPointerUp(ev) {
      if (dragging && dragging.mode === "pick" && !dragging.moved) {
        var hit = pick(ev);
        if (hit) select(hit);
        else select(null);
      }
      dragging = null;
    }

    function onWheel(ev) {
      ev.preventDefault();
      dist = Math.max(24, Math.min(320, dist * (ev.deltaY > 0 ? 1.08 : 0.92)));
      applyCamera();
    }

    renderer.domElement.addEventListener("pointerdown", onPointerDown);
    renderer.domElement.addEventListener("pointermove", onPointerMove);
    renderer.domElement.addEventListener("pointerup", onPointerUp);
    renderer.domElement.addEventListener("pointerleave", function () { dragging = null; });
    renderer.domElement.addEventListener("wheel", onWheel, { passive: false });

    // 标签投影
    function updateLabelVisibility() {
      var camPos = camera.position;
      var camDist = camPos.distanceTo(target);
      Object.keys(labelEls).forEach(function (id) {
        var info = labelEls[id];
        var d = camPos.distanceTo(info.pos);
        var show = state.showLabels === "All" ||
          (state.showLabels === "RelatedOnly" && state.related && state.related.has(id)) ||
          (state.showLabels === "Smart" && d < 70);
        info.el.style.display = show ? "" : "none";
      });
    }

    function labelPriority(id) {
      if (state.selected && state.selected.id === id) return 0;
      if (state.related && state.related.has(id)) return 1;
      return 2;
    }

    function projectLabels() {
      var w = renderer.domElement.clientWidth, h = renderer.domElement.clientHeight;
      var v = new THREE.Vector3();
      var camPos = camera.position;
      var items = [];
      Object.keys(labelEls).forEach(function (id) {
        var info = labelEls[id];
        v.copy(info.pos).project(camera);
        items.push({
          id: id, info: info,
          x: (v.x * 0.5 + 0.5) * w,
          y: (-v.y * 0.5 + 0.5) * h - 8,
          behind: v.z > 1,
          depth: camPos.distanceTo(info.pos),
          text: info.el.textContent || ""
        });
      });

      // 优先级：选中 > 直接相关 > 上下文（近）> 无关（远）
      items.sort(function (a, b) {
        var pa = labelPriority(a.id), pb = labelPriority(b.id);
        if (pa !== pb) return pa - pb;
        return a.depth - b.depth;
      });

      // 屏幕空间避让：与已放置标签重叠者隐藏，保证可读（不做纯装饰堆叠）
      var placed = [];
      items.forEach(function (it) {
        var show;
        if (it.behind) {
          show = false;
        } else if (state.showLabels === "All") {
          show = true;
        } else {
          var lodOk = state.showLabels === "RelatedOnly"
            ? !!(state.related && state.related.has(it.id))
            : it.depth < Math.max(70, camPos.distanceTo(target) * 0.8);
          if (!lodOk) {
            show = false;
          } else {
            var halfW = it.text.length * 3.6 + 6;
            var r = { x1: it.x - halfW, x2: it.x + halfW, y1: it.y - 15, y2: it.y + 4 };
            var hit = false;
            for (var i = 0; i < placed.length; i++) {
              var p = placed[i];
              if (r.x1 < p.x2 && r.x2 > p.x1 && r.y1 < p.y2 && r.y2 > p.y1) { hit = true; break; }
            }
            show = !hit;
            if (show) placed.push(r);
          }
        }
        it.info.el.style.display = show ? "" : "none";
        it.info.el.style.transform = "translate(-50%,-100%) translate(" + it.x.toFixed(1) + "px," + it.y.toFixed(1) + "px)";
      });
    }

    function resize() {
      if (!container) return;
      var w = container.clientWidth || 800;
      var h = container.clientHeight || 600;
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    }

    function renderFrame() {
      if (state.disposed) return;
      resize();
      projectLabels();
      renderer.render(scene, camera);
    }

    function setView(viewId) {
      state.view = viewId;
      var v = (spatial.views || []).filter(function (x) { return x.id === viewId; })[0];
      switch (viewId) {
        case "BirdEye":
          pitch = 1.32; yaw = 0; dist = 120; break;
        case "ExplodedFloor":
          pitch = 0.18; yaw = 0.6; dist = 130; break;
        case "RoomCutaway":
          pitch = 0.1; yaw = 0.2; dist = 60; break;
        case "Data":
          pitch = 0.7; yaw = 0; target.set(0, -16, 0); dist = 90; break;
        case "Security":
          pitch = 0.3; yaw = -0.6; target.set(-6, -4, 0); dist = 80; break;
        case "Operations":
          pitch = 0.3; yaw = 1.2; target.set(12, -8, 0); dist = 80; break;
        default:
          pitch = 0.35; yaw = 0; target.set(0, -6, 0); dist = 110;
      }
      if (viewId === "Overview") fitScene(0.35, 0);
      else applyCamera();
      return v ? v.description : "";
    }

    function fitScene(pitchAngle, yawAngle) {
      // 首次进入 3D 必须自动 fit：按真实节点包围盒取景，不能用固定距离把内容挤出视野
      var box = new THREE.Box3();
      var any = false;
      Object.keys(nodeMeshes).forEach(function (id) {
        box.expandByPoint(nodeMeshes[id].position);
        any = true;
      });
      if (!any) return;
      var c = box.getCenter(new THREE.Vector3());
      var size = box.getSize(new THREE.Vector3());
      // 距离必须同时满足高度与宽度两个方向的取景（视口是窄高的，横向更容易被裁切）
      var vFov = (camera.fov || 45) * Math.PI / 180;
      var aspect = Math.max(0.2, (renderer.domElement.clientWidth || 800) / (renderer.domElement.clientHeight || 600));
      var distH = (size.y / 2) / Math.tan(vFov / 2);
      var distW = (size.x / 2) / Math.tan(vFov / 2) / aspect;
      var distD = (size.z / 2) / Math.tan(vFov / 2);
      target.set(c.x, c.y, c.z);
      dist = Math.max(70, Math.max(distH, distW, distD) * 1.35);
      if (pitchAngle !== undefined) pitch = pitchAngle;
      if (yawAngle !== undefined) yaw = yawAngle;
      applyCamera();
    }

    function resetCamera() {
      yaw = 0; pitch = 0.35;
      fitScene(0.35, 0);
    }

    function focusOn(nodeId) {
      var m = nodeMeshes[nodeId];
      if (!m) return false;
      target.copy(m.position);
      dist = 45;
      applyCamera();
      select(m);
      return true;
    }

    function setRingState(nodeId, stateName) {
      var m = nodeMeshes[nodeId];
      if (!m || !m.userData.ring) return;
      var color = RING_COLORS[stateName] || 0x40536A;
      m.userData.ring.material.color.setHex(color);
      m.userData.ring.material.opacity = stateName === "Idle" ? 0.2 : 0.95;
    }

    /** 5D 分屏：把 A/B 两支的终态交给场景，renderSplit 按左右视口分别上色 */
    function setBranchStates(mapA, mapB) {
      state.split.a = mapA || null;
      state.split.b = mapB || null;
      state.split.active = !!(mapA && mapB);
    }
    function clearBranchStates() {
      state.split.a = null;
      state.split.b = null;
      state.split.active = false;
    }
    function applyBranchRings(map) {
      Object.keys(nodeMeshes).forEach(function (id) {
        var m = nodeMeshes[id];
        if (!m.userData.ring) return;
        var st = map ? map[id] : null;
        m.userData.ring.material.color.setHex(RING_COLORS[st] || 0x40536A);
        m.userData.ring.material.opacity = st ? 0.95 : 0.18;
      });
    }
    /** 同一场景、同一相机，左右分屏渲染 A / B 两支（scissor 双视口） */
    function renderSplit() {
      var w = renderer.domElement.clientWidth, h = renderer.domElement.clientHeight;
      if (!w || !h) return;
      var half = Math.floor(w / 2);
      camera.aspect = half / h;
      camera.updateProjectionMatrix();
      renderer.setScissorTest(true);
      applyBranchRings(state.split.a);
      renderer.setClearColor(0x0B1220, 1);          // A 侧：基线底色
      renderer.setViewport(0, 0, half, h);
      renderer.setScissor(0, 0, half, h);
      renderer.render(scene, camera);
      applyBranchRings(state.split.b);
      renderer.setClearColor(0x182036, 1);          // B 侧：干预后底色（与 A 可区分）
      renderer.setViewport(half, 0, w - half, h);
      renderer.setScissor(half, 0, w - half, h);
      renderer.render(scene, camera);
      renderer.setScissorTest(false);
      renderer.setClearColor(0x0B1220, 1);
      renderer.setViewport(0, 0, w, h);
    }

    /**
     * 4D 用：把某条边标成"刚刚发生了转移" —— 边脉冲 + 箭头高亮。
     * 这是 4D 真正作用在 3D 场景上的证据（不只是改节点状态环）。
     */
    function markEdgeActive(edgeId, on) {
      var l = edgeMeshes[edgeId];
      if (!l) return;
      l.material.color.setHex(on ? 0x5AC7C3 : 0x5C7290);
      l.material.opacity = on ? 1 : 0.55;
      if (l.userData.arrow) {
        l.userData.arrow.material.color.setHex(on ? 0x5AC7C3 : 0x5C7290);
        l.userData.arrow.material.opacity = on ? 1 : 1;
      }
    }

    function clearEdgeMarks() {
      Object.keys(edgeMeshes).forEach(function (id) { markEdgeActive(id, false); });
    }

    /**
     * 5D 用：在节点上标出「首次分歧点」 —— 目标系统语义是分歧发生在节点上。
     */
    function markDivergence(nodeId, on) {
      var m = nodeMeshes[nodeId];
      if (!m) return;
      if (!m.userData.divMark) {
        var g = new THREE.Group();
        g.userData = { kind: "DivergenceMark", id: nodeId };
        var r1 = new THREE.Mesh(
          new THREE.TorusGeometry(1.9, 0.09, 8, 32),
          new THREE.MeshBasicMaterial({ color: 0xF08886, transparent: true, opacity: 0.95 })
        );
        r1.rotation.x = Math.PI / 2;
        g.add(r1);
        g.visible = false;
        nodeGroup.add(g);
        m.userData.divMark = g;
      }
      m.userData.divMark.visible = !!on;
    }

    function clearDivergenceMarks() {
      Object.keys(nodeMeshes).forEach(function (id) { markDivergence(id, false); });
    }

    function dispose() {
      state.disposed = true;
      renderer.domElement.removeEventListener("pointerdown", onPointerDown);
      renderer.domElement.removeEventListener("pointermove", onPointerMove);
      renderer.domElement.removeEventListener("pointerup", onPointerUp);
      renderer.domElement.removeEventListener("wheel", onWheel);
      if (renderer.domElement.parentElement) renderer.domElement.parentElement.removeChild(renderer.domElement);
      if (labelLayer && labelLayer.parentElement) labelLayer.parentElement.removeChild(labelLayer);
      try { renderer.dispose(); } catch (e) { /* noop */ }
    }

    fitScene(0.35, 0);
    resize();

    return {
      scene: scene, camera: camera, renderer: renderer,
      state: state,
      select: select,
      focusOn: focusOn,
      setView: setView,
      setRingState: setRingState,
      setBranchStates: setBranchStates,
      clearBranchStates: clearBranchStates,
      markEdgeActive: markEdgeActive,
      clearEdgeMarks: clearEdgeMarks,
      markDivergence: markDivergence,
      clearDivergenceMarks: clearDivergenceMarks,
      resetCamera: resetCamera,
      applyEmphasis: applyEmphasis,
      renderFrame: renderFrame,
      resize: resize,
      dispose: dispose,
      nodeMeshes: nodeMeshes,
      edgeMeshes: edgeMeshes,
      roomMeshes: roomMeshes,
      labelEls: labelEls,
      relatedSet: relatedSet,
      // 供测试与无 WebGL 环境断言
      hasWebGL: (function () {
        try {
          var c = document.createElement("canvas");
          return !!(c.getContext("webgl2") || c.getContext("webgl"));
        } catch (e) { return false; }
      })()
    };
  }

  var API = {
    createScene: createScene,
    toThree: toThree,
    toSemantic: toSemantic,
    gemGeometry: gemGeometry
  };
  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcSpatial = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
