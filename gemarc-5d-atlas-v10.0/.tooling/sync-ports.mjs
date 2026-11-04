/**
 * .tooling/sync-ports.mjs — 从真实代码抽取 API 面，生成 models/architecture.json 的端口清单
 *
 * 为什么要它：端口清单手写会漂移。本次一致性核对发现图集只登记了 40/72 条真实 HTTP 路由、
 * 26/68 条真实 Tauri command —— 2D 画布因此看不到一大半真实接口。
 * 由代码生成 + verify-consistency.mjs 门禁，模型就不再可能与真实接口面脱节。
 *
 * 权威来源：
 *   HTTP        crates/ide-shell-web/src/lib.rs 的 ("METHOD","/path") => 分支
 *   Tauri       crates/ide-shell-desktop/src/lib.rs 的 generate_handler![ ... ] 注册清单
 *   JSON-RPC    crates/ide-shell-protocol/src/lib.rs 的 handle_rpc 方法分发表
 *
 * 策略：**保留既有端口 ID**（边的 sourcePortId/targetPortId 依赖它们），只补新增、报告失效。
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const PROJ = process.argv[3] || "E:/IDE1.0";
const M = path.join(ROOT, "models");
const rd = (f) => JSON.parse(fs.readFileSync(path.join(M, f), "utf8"));
const arch = rd("architecture.json");

// ---------- 1. HTTP 路由 ----------
const webSrc = fs.readFileSync(path.join(PROJ, "crates/ide-shell-web/src/lib.rs"), "utf8");
const httpRoutes = [];
for (const m of webSrc.matchAll(/\("(GET|POST|PUT|DELETE)"\s*,\s*"([^"]+)"\)/g)) {
  httpRoutes.push({ method: m[1], path: m[2] });
}

// ---------- 2. Tauri 已注册 command ----------
const desktopSrc = fs.readFileSync(path.join(PROJ, "crates/ide-shell-desktop/src/lib.rs"), "utf8");
const gh = desktopSrc.indexOf("generate_handler![");
const ghEnd = desktopSrc.indexOf("]", gh);
const ghBlock = desktopSrc.slice(gh, ghEnd);
const tauriCmds = [];
for (const m of ghBlock.matchAll(/([a-z_]+(?:::[a-z_0-9]+)?)\s*,/g)) {
  const name = m[1].split("::").pop();
  if (name && !tauriCmds.includes(name)) tauriCmds.push(name);
}

// ---------- 3. JSON-RPC 方法 ----------
const protoSrc = fs.readFileSync(path.join(PROJ, "crates/ide-shell-protocol/src/lib.rs"), "utf8");
const rpcMethods = [];
{
  // 方法分发表在 dispatch() 的 `match method { ... }` 里；
  // 分支可能以 `{` 或直接表达式结尾，故用行首锚定的标签提取。
  const start = protoSrc.indexOf("match method {");
  const block = protoSrc.slice(start, start + 9000);
  for (const m of block.matchAll(/^\s+"([a-z_]+)"\s*=>/gm)) {
    if (!rpcMethods.includes(m[1])) rpcMethods.push(m[1]);
  }
}

// ---------- 4. 与既有端口对齐（保 ID、补缺、报失效） ----------
const existing = arch.ports || [];
const usedIds = new Set();
const byKey = new Map();          // key → port
const keyOf = (kind, val) => kind + ":" + val;
function indexPort(p, keys) {
  keys.forEach((k) => { if (k && !byKey.has(k)) byKey.set(k, p); });
  usedIds.add(p.id);
}
for (const p of existing) {
  const pr = p.properties || {};
  const keys = [];
  if (pr.path) keys.push(keyOf("http", pr.method + " " + pr.path));
  if (pr.command) keys.push(keyOf("tauri", pr.command));
  if (pr.method && /^P-RPC-/.test(p.id)) keys.push(keyOf("rpc", pr.method));
  // 兜底：按 name 匹配（人工登记的端口通常只写了 name）
  if (pr.path || pr.command || (pr.method && /^P-RPC-/.test(p.id))) {
    // 已有结构化 key
  } else if (/^(GET|POST|PUT|DELETE) /.test(p.name || "")) {
    keys.push(keyOf("http", p.name));
  } else if (/^P-TAURI-/.test(p.id)) {
    keys.push(keyOf("tauri", p.name));
  } else if (/^P-RPC-/.test(p.id)) {
    keys.push(keyOf("rpc", p.name));
  }
  indexPort(p, keys);
}
const slug = (s) => String(s).toUpperCase().replace(/[^A-Z0-9]+/g, "-").replace(/^-|-$/g, "");

const added = [], reused = [], stale = [];
function portFor(kind, val, extra) {
  const key = keyOf(kind, val);
  const hit = byKey.get(key);
  if (hit) {
    reused.push(hit.id);
    // 补齐可能被漏掉的属性
    Object.assign(hit.properties || (hit.properties = {}), extra.props || {});
    if (!hit.type) hit.type = extra.type;
    return hit;
  }
  const base = kind === "http" ? "P-HTTP-" : kind === "tauri" ? "P-TAURI-" : "P-RPC-";
  let id = base + slug(extra.slugVal);
  let i = 2;
  while (usedIds.has(id)) id = base + slug(extra.slugVal) + "-" + (i++);
  usedIds.add(id);
  const p = {
    id,
    name: val,
    direction: "in",
    type: extra.type,
    nodeId: extra.nodeId,
    properties: Object.assign({ source: extra.source }, extra.props || {})
  };
  added.push(p);
  return p;
}
const ports = [];
for (const r of httpRoutes) {
  ports.push(portFor("http", r.method + " " + r.path, {
    type: "HTTP " + r.method, nodeId: "N-WB-SERVER", slugVal: r.path,
    source: "crates/ide-shell-web/src/lib.rs", props: { method: r.method, path: r.path }
  }));
}
for (const c of tauriCmds) {
  ports.push(portFor("tauri", c, {
    type: "Tauri command", nodeId: "N-DT-TAURI", slugVal: c,
    source: "crates/ide-shell-desktop/src/lib.rs (generate_handler!)", props: { command: c }
  }));
}
for (const m of rpcMethods) {
  ports.push(portFor("rpc", m, {
    type: "JSON-RPC 2.0 method", nodeId: "N-PT-RPC", slugVal: m,
    source: "crates/ide-shell-protocol/src/lib.rs (handle_rpc)", props: { method: m }
  }));
}
// 保留既有的其它（人工）端口
const manual = existing.filter((p) => !/^P-(HTTP|TAURI|RPC)-/.test(p.id) || !reused.includes(p.id) && !added.includes(p));
const keptManual = existing.filter((p) => {
  if (added.includes(p)) return false;
  if (reused.includes(p)) return false;
  return true;
});
// 失效检测：既有端口对应的 route/command 已不存在
for (const p of existing) {
  const pr = p.properties || {};
  if (!pr.path && !pr.command && !(pr.method && /^P-RPC-/.test(p.id))) continue;
  const key = pr.path ? keyOf("http", pr.method + " " + pr.path) : pr.command ? keyOf("tauri", pr.command) : keyOf("rpc", pr.method);
  if (!byKey.has(key)) stale.push(p.id + "（" + (pr.path || pr.command || pr.method) + " 已不在代码中）");
}

const merged = ports.concat(keptManual);

// ---- 去重：同一个 (方法/命令/方法名) 只留一个端口 ----
// 优先保留**被边引用**的那个（边的 sourcePortId/targetPortId 依赖 ID），否则保留先出现的。
const referenced = new Set();
for (const e of arch.edges || []) {
  if (e.sourcePortId) referenced.add(e.sourcePortId);
  if (e.targetPortId) referenced.add(e.targetPortId);
}
const keyOfPort = (p) => {
  const pr = p.properties || {};
  if (pr.path) return keyOf("http", pr.method + " " + pr.path);
  if (pr.command) return keyOf("tauri", pr.command);
  if (pr.method && /^P-RPC-/.test(p.id)) return keyOf("rpc", pr.method);
  return null;
};
const byValue = new Map();
const finalPorts = [];
for (const p of merged) {
  const k = keyOfPort(p);
  if (!k) { finalPorts.push(p); continue; }     // 人工抽象端口：原样保留
  const prev = byValue.get(k);
  if (!prev) { byValue.set(k, p); finalPorts.push(p); continue; }
  // 同一接口出现多个端口：保留被边引用的那个，另一个并入备注后丢弃
  if (referenced.has(p.id) && !referenced.has(prev.id)) {
    const idx = finalPorts.indexOf(prev);
    finalPorts[idx] = p;
    byValue.set(k, p);
    console.log("  去重: " + prev.id + " → " + p.id + "（被边引用）");
  } else {
    console.log("  去重: 丢弃重复端口 " + p.id + "（与 " + prev.id + " 同接口）");
  }
}
arch.ports = finalPorts;

// 节点 portIds 同步
const byNode = {};
for (const p of arch.ports) {
  if (!p.nodeId) continue;
  (byNode[p.nodeId] = byNode[p.nodeId] || []).push(p.id);
}
for (const n of arch.nodes) {
  n.portIds = (byNode[n.id] || []).slice().sort();
}
fs.writeFileSync(path.join(M, "architecture.json"), JSON.stringify(arch, null, 2), "utf8");

console.log("端口清单已按真实代码对齐：");
console.log("  HTTP 路由      " + httpRoutes.length + " 条");
console.log("  Tauri command " + tauriCmds.length + " 条");
console.log("  JSON-RPC      " + rpcMethods.length + " 条");
console.log("  合计端口 " + arch.ports.length + "（唯一）");
if (stale.length) {
  console.log("  ⚠ 既有端口在代码中已不存在（需人工确认）:");
  stale.forEach((s) => console.log("    - " + s));
}
