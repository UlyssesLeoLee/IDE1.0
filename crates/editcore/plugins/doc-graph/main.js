// 文档关系图: 纯函数 (文本进, SVG 出); 需要 editor.read 权限才会拿到文档文本
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

function parse(text) {
  const nodes = [];           // { id, level, title, line }
  let fence = false;
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  lines.forEach((l, i) => {
    if (/^\s*```/.test(l)) { fence = !fence; return; }
    if (fence) return;
    const m = /^(#{1,6}) +(.*?)\s*#*\s*$/.exec(l);
    if (m) nodes.push({ id: nodes.length, level: m[1].length, title: m[2], line: i + 1 });
  });
  // 层级边: 每个标题连到最近的上一级标题
  const edges = [];
  const stack = [];
  for (const n of nodes) {
    while (stack.length && stack[stack.length - 1].level >= n.level) stack.pop();
    if (stack.length) edges.push({ from: stack[stack.length - 1].id, to: n.id, kind: "child" });
    stack.push(n);
  }
  // 引用边: 某章节里的 [[标题]] / [..](#标题) 指向同名标题
  const byTitle = new Map(nodes.map((n) => [n.title.toLowerCase(), n.id]));
  let cur = -1, fence2 = false;
  lines.forEach((l, i) => {
    if (/^\s*```/.test(l)) { fence2 = !fence2; return; }
    if (fence2) return;
    const h = nodes.find((n) => n.line === i + 1);
    if (h) { cur = h.id; return; }
    if (cur < 0) return;
    for (const m of l.matchAll(/\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|[^\]]*)?\]\]|\]\(#([^)]+)\)/g)) {
      const t = (m[1] || decodeURIComponent(m[2] || "")).trim().toLowerCase().replace(/-/g, " ");
      const to = byTitle.get(t) ?? [...byTitle].find(([k]) => k.replace(/[^\p{L}\p{N}]+/gu, " ").trim() === t)?.[1];
      if (to !== undefined && to !== cur && !edges.some((e) => e.from === cur && e.to === to && e.kind === "ref")) edges.push({ from: cur, to, kind: "ref" });
    }
  });
  return { nodes, edges };
}

function layout(nodes) {
  // 纵向列表式树: 缩进 = 层级, 每个节点一行
  const rowH = 30, indent = 26, x0 = 20;
  return nodes.map((n, i) => ({ ...n, x: x0 + (n.level - 1) * indent, y: 20 + i * rowH }));
}

function render(text) {
  const { nodes, edges } = parse(text);
  if (!nodes.length) return { error: "文档里没有标题, 画不出关系图" };
  const pos = layout(nodes), W = Math.max(480, ...pos.map((p) => p.x + 40 + p.title.length * 9)), H = 40 + nodes.length * 30;
  const at = (id) => pos[id];
  let s = `<svg viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" xmlns="http://www.w3.org/2000/svg">`;
  for (const e of edges) {
    const a = at(e.from), b = at(e.to);
    if (e.kind === "child") s += `<path d="M ${a.x + 6} ${a.y + 6} V ${b.y} H ${b.x - 6}" fill="none" stroke="#888" stroke-width="1"/>`;
    else s += `<path d="M ${a.x + 6} ${a.y} C ${W - 20} ${a.y}, ${W - 20} ${b.y}, ${b.x + 6} ${b.y}" fill="none" stroke="#d29922" stroke-width="1.4"><title>${esc(a.title)} → ${esc(b.title)}</title></path>`;
  }
  for (const n of pos) {
    s += `<g data-line="${n.line}" class="gnode"><circle cx="${n.x}" cy="${n.y}" r="6" fill="${n.level === 1 ? "#58a6ff" : "#8b949e"}"/><text x="${n.x + 12}" y="${n.y + 4}" font-size="13" fill="#c9d1d9">${esc(n.title)}</text><title>第 ${n.line} 行 · H${n.level}</title></g>`;
  }
  s += "</svg>";
  const refs = edges.filter((e) => e.kind === "ref").length;
  return { svg: s, summary: `${nodes.length} 个标题, ${refs} 条跨章节引用 (橙色)` };
}

ide.register({
  "command:show": async (ctx) => render((ctx && ctx.text) || ""),
  // agent.tool: 给内建对话 / 外部 Agent 用的只读工具
  "agent.tool:stats": async (args) => {
    const { nodes, edges } = parse(String((args && args.text) || ""));
    const levels = {};
    for (const n of nodes) levels["H" + n.level] = (levels["H" + n.level] || 0) + 1;
    return { headings: nodes.length, levels, refs: edges.filter((e) => e.kind === "ref").length };
  },
});
