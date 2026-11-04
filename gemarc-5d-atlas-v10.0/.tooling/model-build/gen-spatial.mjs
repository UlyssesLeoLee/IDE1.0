/**
 * .atlas-work/gen-spatial.mjs
 * 1) 规范化 architecture.json 的 node.role —— 3D 引擎用 role 决定宝石几何，
 *    role 必须是 11 个功能角色之一，否则退化为默认体（静默失败）。
 * 2) 由 containers/nodes 推导 spatial.json（房间位置、节点偏移、宝石映射）。
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const archPath = path.join(ROOT, "models/architecture.json");
const arch = JSON.parse(fs.readFileSync(archPath, "utf8"));

// ---- 1) role 规范化 --------------------------------------------------
const ROLE_MAP = {
  Actor: "Actor", Input: "Input", Processor: "Processor", Decision: "Decision",
  Storage: "Storage", Message: "Message", Interface: "Interface",
  Security: "Security", Agent: "Agent", Composition: "Composition", Output: "Output",
  External: "Actor" // 外部系统/进程在语义上是边界参与者
};
let roleFixed = 0;
for (const n of arch.nodes) {
  const mapped = ROLE_MAP[n.kind] || "Processor";
  if (mapped !== n.role) {
    n.properties = n.properties || {};
    n.properties.purpose = n.properties.purpose || n.role;
    n.role = mapped;
    roleFixed++;
  }
  // kind 保留宝石角色语义（graph.js 会把它带到图查询层）
  n.kind = mapped;
}
fs.writeFileSync(archPath, JSON.stringify(arch, null, 2), "utf8");
console.log("role normalized:", roleFixed, "nodes; total", arch.nodes.length);

// ---- 2) 空间投影 ------------------------------------------------------
const ROOM_SIZE = 26;
const FLOOR_GAP = 22;
const BUILDING_GAP = 46;
const NODE_SPACING = 7.2;

const containers = arch.containers;
const byParent = {};
for (const c of containers) {
  const k = c.parentId || "_root";
  (byParent[k] = byParent[k] || []).push(c);
}
const nodesByParent = {};
for (const n of arch.nodes) {
  const k = n.parentId || "_none";
  (nodesByParent[k] = nodesByParent[k] || []).push(n);
}

const buildings = containers.filter((c) => c.kind === "Building");
const buildingOrigins = {};
buildings.forEach((b, bi) => {
  // 后端建筑垂直下沉；前端在原点；OS 在左；外部在右
  const pos = b.id === "BLD-FRONTEND" ? [0, 0, 0]
    : b.id === "BLD-BACKEND" ? [0, 0, -BUILDING_GAP]
      : b.id === "BLD-OS" ? [-BUILDING_GAP, 0, 0]
        : [BUILDING_GAP, 0, 0];
  buildingOrigins[b.id] = pos;
});

const rooms = {};
const nodeOffsets = {};
const floorInfo = [];

for (const b of buildings) {
  const bo = buildingOrigins[b.id];
  const floors = (byParent[b.id] || []).filter((c) => c.kind === "Floor");
  floors.forEach((f, fi) => {
    const fy = bo[1] - fi * FLOOR_GAP;
    const roomList = (byParent[f.id] || []).filter((c) => c.kind === "Room");
    const perRow = Math.max(1, Math.ceil(roomList.length / 2));
    roomList.forEach((r, ri) => {
      const col = ri % perRow;
      const row = Math.floor(ri / perRow);
      const rx = bo[0] + (col - (Math.min(roomList.length, perRow) - 1) / 2) * ROOM_SIZE;
      const rz = bo[2] + (row - 0.5) * ROOM_SIZE * 0.8;
      const members = nodesByParent[r.id] || [];
      const isStorage = r.id === "RM-DOC" || r.id === "RM-SESSION" || r.id === "RM-BUFFER" || r.id === "RM-UNDO" || r.id === "RM-IO";
      rooms[r.id] = {
        position: [+rx.toFixed(2), +fy.toFixed(2), +rz.toFixed(2)],
        size: [ROOM_SIZE - 3, 8, ROOM_SIZE * 0.8 - 3],
        floor: f.id,
        label: r.name,
        nodes: members.map((m) => m.id),
        storageOnly: !!isStorage
      };
      // 房间内节点网格排布
      const cols = Math.max(1, Math.ceil(Math.sqrt(members.length)));
      members.forEach((m, mi) => {
        const c = mi % cols;
        const rw = Math.floor(mi / cols);
        const off = [
          +(c - (cols - 1) / 2) * NODE_SPACING,
          +((rw % 2) * 3.2),
          +((mi % 2) * 2.2 - 1.1)
        ];
        nodeOffsets[m.id] = off;
      });
    });
    floorInfo.push({ id: f.id, name: f.name, buildingId: b.id, y: fy });
  });
}

// 未落在任何房间的节点 —— 显式登记，不静默丢弃
const orphanNodes = arch.nodes.filter((n) => !nodeOffsets[n.id]).map((n) => n.id);

const gemstoneMapping = {
  note: "shape = 功能角色（node.role）；color = 业务域（node.domainId）。宝石只用于 3D 节点，字段/连线/关系符号一律不用宝石化。",
  byNode: {},
  unknownReason: {},
  scopeRule: "只有出现在 architecture.nodes 且带 role 的元素才可宝石化；Ports / Edges / Evidence / Requirement 不进 3D 宝石层。"
};
for (const n of arch.nodes) {
  gemstoneMapping.byNode[n.id] = { shape: n.role, domain: n.domainId };
  if (n.verification === "Designed") {
    gemstoneMapping.unknownReason[n.id] = "契约已定义但生产实现未落地（按 Designed 渲染为半透明描边态）";
  }
}

const spatial = {
  schema: "gemarc-5d-atlas/spatial/v10.0",
  version: arch.version,
  note: "IDE1.0 三维空间投影：System→Building→Floor→Room→Node/Edge。容器语义来自 IDE1.0 实际分层（前端/后端/OS/外部），不是物理机房臆造。",
  handedness: {
    semantic: "左手语义坐标：+X 右，+Y 上，+Z 前",
    "+X": "右",
    "+Y": "上",
    "+Z": "前",
    renderer: "Three.js（右手系）",
    adapterRule: "统一在 Adapter 内做 X 翻转与法线/面绕序修正，语义层不出现负缩放，避免法线与拾取错乱。"
  },
  units: { roomSize: ROOM_SIZE, floorGap: FLOOR_GAP, buildingGap: BUILDING_GAP, nodeRadius: 1.6 },
  buildings: buildings.map((b) => ({
    id: b.id,
    name: b.name,
    origin: buildingOrigins[b.id],
    floors: (byParent[b.id] || []).filter((c) => c.kind === "Floor").map((f) => f.id),
    runtimePort: b.id === "BLD-FRONTEND" ? "editor.html" : b.id === "BLD-BACKEND" ? "127.0.0.1:8123 / Tauri invoke" : b.id === "BLD-OS" ? "Windows 10+ runtime" : "external process",
    evidenceIds: b.evidenceIds || []
  })),
  floors: floorInfo,
  rooms,
  nodeOffsets,
  orphanBuildings: orphanNodes.length ? [{ id: "ORPHAN-NODES", note: "未归属房间的节点：" + orphanNodes.join(", "), evidenceIds: [] }] : [],
  views: [
    { id: "Overview", description: "全系统鸟瞰：四个建筑（前端/后端/OS/外部）与其房间", default: true },
    { id: "BirdEye", description: "俯视：看清楼层切分与房间排布", default: false },
    { id: "ExplodedFloor", description: "爆炸楼层：把后端五个楼层拉开对比", default: false },
    { id: "RoomCutaway", description: "房间剖切：聚焦单个房间内的节点与连线", default: false },
    { id: "Logical", description: "逻辑视图：按调用/依赖关系而非空间位置理解组件", default: false },
    { id: "Development", description: "开发视图：crate 依赖与源码归属", default: false },
    { id: "Deployment", description: "部署视图：MSI / Web 两种运行形态", default: false },
    { id: "Data", description: "数据视图：Document/DocStore/Session/Buffer/Ope 持久态聚集区", default: false },
    { id: "Process", description: "过程视图：请求从外壳到内核的路径", default: false },
    { id: "Security", description: "安全视图：认证与沙箱边界", default: false },
    { id: "Operations", description: "运维视图：外部进程（git/rg/powershell）与自动备份", default: false },
    { id: "Dependency", description: "依赖视图：影响面与反向依赖", default: false }
  ],
  viewIndependence: "Logical/Development/Process/Deployment/Data/Security/Operations/Dependency 视图与 1D–5D 表达维度相互独立：换视图不改变事实图、规则或因果结论。",
  gemstoneMapping
};

fs.writeFileSync(path.join(ROOT, "models/spatial.json"), JSON.stringify(spatial, null, 2), "utf8");
console.log("spatial.json written: rooms=" + Object.keys(rooms).length, "offsets=" + Object.keys(nodeOffsets).length, "orphan=" + orphanNodes.length);
if (orphanNodes.length) console.log("ORPHANS:", orphanNodes.join(", "));
